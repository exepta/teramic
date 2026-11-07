//! Static scene models occupy editable cells but do not hide neighboring surfaces.
use super::builder::{MeshBatch, RenderOptions, compose_section_with_segments};
use crate::blocks::TemplateSet;
#[path = "custom_castle.rs"]
mod castle;
#[path = "custom_filling.rs"]
mod filling;
#[path = "custom_heraldry.rs"]
mod heraldry;
#[path = "custom_pipe_surround.rs"]
mod pipe_surround;
#[path = "custom_plumbing.rs"]
pub(crate) mod plumbing;
#[path = "custom_water_contacts.rs"]
mod water_contacts;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
};
use terra_voxel::{
    BlockSection, MaterialId, SectionPos,
    meshing::{BlockFace, BlockMesh},
};

#[derive(Clone, Copy)]
struct ModelCell {
    cell: [i32; 3],
    material: u16,
    state: terra_world::edits::BlockObjectState,
    peer_wood: terra_core::WoodSpecies,
    neighbors: [Option<u16>; 6],
    neighbor_facing: [u8; 6],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ConstructionKey {
    material: u16,
    facing: u8,
    neighbors: [Option<u8>; 6],
}

#[derive(Default)]
struct ConstructionProfiles(HashMap<ConstructionKey, Vec<crate::blocks::PrimitiveTemplate>>);

type ChimneyInfill = Vec<(terra_core::block_shapes::ShapeBox, u16)>;

#[cfg(test)]
#[path = "custom_castle_connections_tests.rs"]
mod castle_connections_tests;
#[cfg(test)]
#[path = "custom_profile_tests.rs"]
mod profile_tests;
#[cfg(test)]
#[path = "custom_snow_tests.rs"]
mod snow_tests;
#[cfg(test)]
#[path = "custom_support_tests.rs"]
mod support_tests;
#[cfg(test)]
#[path = "custom_textile_tests.rs"]
mod textile_tests;

impl ConstructionProfiles {
    const LIMIT: usize = 64;

    fn get<'a>(
        &'a mut self,
        model: &ModelCell,
        templates: &TemplateSet,
    ) -> Result<Cow<'a, [crate::blocks::PrimitiveTemplate]>, String> {
        // Only stair poses affect a neighbouring stair profile. Ordinary walls,
        // materials and empty cells share the same shape key.
        let key = ConstructionKey {
            material: model.material,
            facing: model.state.facing,
            neighbors: std::array::from_fn(|side| {
                (terra_core::construction::is_stair(model.material)
                    && model.neighbors[side].is_some_and(terra_core::construction::is_stair))
                .then_some(model.neighbor_facing[side])
            }),
        };
        let full = self.0.len() == Self::LIMIT;
        match self.0.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                Ok(Cow::Borrowed(entry.into_mut()))
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let profile = construction_profile(model, templates)?;
                if full {
                    Ok(Cow::Owned(profile))
                } else {
                    Ok(Cow::Borrowed(entry.insert(profile)))
                }
            }
        }
    }
}

fn construction_profile(
    model: &ModelCell,
    templates: &TemplateSet,
) -> Result<Vec<crate::blocks::PrimitiveTemplate>, String> {
    let template = templates
        .blocks
        .get(&model.material)
        .ok_or("Missing construction template")?;
    let mut profile: Vec<crate::blocks::PrimitiveTemplate> = if model.material == 98 {
        crate::blocks::cube::box_model(
            &[terra_core::construction::snow_bounds(model.state.facing)],
            template.bevel_width,
            template.face_materials,
        )
    } else if terra_core::construction::is_slab(model.material) {
        terra_core::construction::slab_parts(model.material, model.state.facing)
            .into_iter()
            .flat_map(|(id, bounds)| {
                let part = templates.blocks.get(&id).unwrap_or(template);
                crate::blocks::cube::box_model(&[bounds], part.bevel_width, part.face_materials)
            })
            .collect()
    } else {
        let neighbors = std::array::from_fn(|side| {
            model.neighbors[side].map(|id| (id, model.neighbor_facing[side]))
        });
        crate::blocks::cube::box_model(
            &terra_core::construction::placed_boxes(model.material, model.state.facing, neighbors),
            template.bevel_width,
            template.face_materials,
        )
    };
    for primitive in &mut profile {
        if templates.authored_terrain_material(primitive.material) {
            primitive.tangents = primitive
                .normals
                .iter()
                .map(|&normal| {
                    let normal = bevy::math::Vec3::from_array(normal);
                    let axis = (0..3)
                        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
                        .unwrap();
                    crate::blocks::projected_tangent(normal, axis * 2)
                })
                .collect();
        }
    }
    Ok(profile)
}

#[derive(Clone)]
pub(crate) struct ModelSnapshot {
    standalone: bool,
    heraldry: HashMap<[i32; 3], terra_core::kingdom::KingdomHeraldry>,
    domains: Vec<terra_core::kingdom::SettlementDomain>,
    include_moving: bool,
    blocks: Vec<u16>,
    models: Vec<ModelCell>,
    border_models: Vec<[i32; 3]>,
    border_glass: HashSet<[i32; 3]>,
    water_contacts: HashMap<[i32; 3], bool>,
    chimney_infill: BTreeMap<[i32; 3], ChimneyInfill>,
    pipe_surround: BTreeMap<[i32; 3], pipe_surround::Surround>,
    funnel_supports: HashSet<[i32; 3]>,
}

impl ModelSnapshot {
    pub(super) fn campfire_supports_changed(
        &self,
        section: SectionPos,
        chunks: &terra_world::streaming::ChunkStore,
        journal: &terra_world::edits::EditJournal,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        self.models.iter().any(|model| {
            if model.material != terra_core::campfire::MATERIAL || model.cell[1] != 15 {
                return false;
            }
            let above = std::array::from_fn(|axis| {
                origin[axis] + i64::from(model.cell[axis]) + i64::from(axis == 1)
            });
            (model.neighbors[3] == Some(terra_core::brewing::MATERIAL))
                != (chunks
                    .block_at(above)
                    .map(|material| journal.block(above).unwrap_or(material))
                    == Some(terra_core::brewing::MATERIAL))
        })
    }

    pub(super) fn refresh_campfire_supports(
        &mut self,
        section: SectionPos,
        chunks: &terra_world::streaming::ChunkStore,
        journal: &terra_world::edits::EditJournal,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        let mut changed = false;
        for model in &mut self.models {
            if model.material != terra_core::campfire::MATERIAL {
                continue;
            }
            let above = std::array::from_fn(|axis| {
                origin[axis] + i64::from(model.cell[axis]) + i64::from(axis == 1)
            });
            let material = chunks
                .block_at(above)
                .map(|material| journal.block(above).unwrap_or(material));
            changed |= (model.neighbors[3] == Some(terra_core::brewing::MATERIAL))
                != (material == Some(terra_core::brewing::MATERIAL));
            model.neighbors[3] = material;
        }
        changed
    }

    pub(super) fn funnel_supports_changed(
        &self,
        section: SectionPos,
        chunks: &terra_world::streaming::ChunkStore,
        journal: &terra_world::edits::EditJournal,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        self.models.iter().any(|model| {
            if model.cell[1] != 0 || !terra_core::logistics::is_funnel(model.material) {
                return false;
            }
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            let before = model.neighbors[2].is_some_and(terra_core::beds::supports_bed_floor)
                || self.funnel_supports.contains(&model.cell);
            before
                != terra_world::funnels::supported_with(position, journal, |cell| {
                    chunks
                        .block_at(cell)
                        .map(|material| journal.block(cell).unwrap_or(material))
                })
        })
    }

    pub(super) fn refresh_funnel_supports(
        &mut self,
        section: SectionPos,
        chunks: &terra_world::streaming::ChunkStore,
        journal: &terra_world::edits::EditJournal,
    ) -> bool {
        let Ok(origin) = section.origin() else {
            return false;
        };
        let mut changed = false;
        for model in &mut self.models {
            if !terra_core::logistics::is_funnel(model.material) {
                continue;
            }
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            let was_supported = model.neighbors[2]
                .is_some_and(terra_core::beds::supports_bed_floor)
                || self.funnel_supports.contains(&model.cell);
            let sample = |cell| {
                chunks
                    .block_at(cell)
                    .map(|material| journal.block(cell).unwrap_or(material))
            };
            let supported = terra_world::funnels::supported_with(position, journal, sample);
            model.neighbors[2] = position[1]
                .checked_sub(1)
                .and_then(|y| sample([position[0], y, position[2]]));
            if supported {
                self.funnel_supports.insert(model.cell);
            } else {
                self.funnel_supports.remove(&model.cell);
            }
            changed |= was_supported != supported;
        }
        changed
    }
}

/// Standalone objects and placement ghosts share the terrain's exact model transform.
pub(crate) fn model_geometry(
    material: u16,
    state: terra_world::edits::BlockObjectState,
    peer_wood: terra_core::WoodSpecies,
    neighbors: [Option<u16>; 6],
    neighbor_facing: [u8; 6],
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    let snapshot = ModelSnapshot {
        standalone: true,
        heraldry: HashMap::new(),
        domains: Vec::new(),
        include_moving: true,
        blocks: vec![0; 4096],
        models: vec![ModelCell {
            cell: [0; 3],
            material,
            state,
            peer_wood,
            neighbors,
            neighbor_facing,
        }],
        border_models: Vec::new(),
        border_glass: HashSet::new(),
        water_contacts: HashMap::new(),
        chimney_infill: BTreeMap::new(),
        pipe_surround: BTreeMap::new(),
        funnel_supports: HashSet::new(),
    };
    compose(
        &BlockMesh::default(),
        &snapshot,
        templates,
        SectionPos::default(),
        &RenderOptions {
            water_level: None,
            detail_level: super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
}

/// Reuse the exact static-model transform, material slots, UVs and split wood
/// colors for the single asynchronously isolated mining target.
pub(crate) fn isolate_model(
    geometry: &BlockMesh,
    snapshot: &ModelSnapshot,
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
    cell: [u32; 3],
    half: Option<terra_world::slabs::HalfRemoval>,
) -> Result<(Vec<MeshBatch>, Vec<MeshBatch>), String> {
    let cell = cell.map(|value| value as i32);
    let mut model = snapshot
        .models
        .iter()
        .find(|model| model.cell == cell && !model.state.secondary)
        .copied()
        .ok_or("Missing primary mining model")?;
    let mut remaining = snapshot.clone();
    remaining.models.retain(|entry| entry.cell != cell);
    if let Some(half) = half {
        let mut preserved = model;
        preserved.material = half.remaining;
        preserved.state = half.remaining_state(model.state);
        remaining.models.push(preserved);
        model.material = half.removed;
        model.state.facing = half.removed_facing;
    }
    // Its supporting terrain must stay exposed after removing the scene model.
    remaining.border_models.push(cell);
    let section_geometry = compose(geometry, &remaining, templates, section, options)?;
    let isolated = ModelSnapshot {
        standalone: true,
        heraldry: snapshot.heraldry.clone(),
        domains: snapshot.domains.clone(),
        include_moving: false,
        blocks: vec![0; 16 * 16 * 16],
        models: vec![model],
        border_models: Vec::new(),
        border_glass: HashSet::new(),
        water_contacts: HashMap::new(),
        chimney_infill: snapshot.chimney_infill.clone(),
        pipe_surround: snapshot.pipe_surround.clone(),
        funnel_supports: snapshot.funnel_supports.clone(),
    };
    let target = compose(
        &BlockMesh::default(),
        &isolated,
        templates,
        section,
        options,
    )?;
    Ok((section_geometry, target))
}

pub(crate) fn compose_without_cell(
    geometry: &BlockMesh,
    snapshot: &ModelSnapshot,
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
    cell: [u32; 3],
) -> Result<Vec<MeshBatch>, String> {
    let mut snapshot = snapshot.clone();
    snapshot.blocks[cell[0] as usize + 16 * (cell[1] as usize + 16 * cell[2] as usize)] = 0;
    compose(geometry, &snapshot, templates, section, options)
}

pub(crate) fn compose_without_cells(
    geometry: &BlockMesh,
    snapshot: &ModelSnapshot,
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
    mask: &[bool; 4096],
) -> Result<Vec<MeshBatch>, String> {
    let mut snapshot = snapshot.clone();
    for (index, remove) in mask.iter().enumerate() {
        if *remove {
            snapshot.blocks[index] = 0;
        }
    }
    compose(geometry, &snapshot, templates, section, options)
}

fn is_model(templates: &TemplateSet, material: u16) -> bool {
    templates
        .blocks
        .get(&material)
        .is_some_and(|block| !block.model.is_empty())
}

fn chimney_template_pose(facing: u8) -> Option<(u16, bevy::math::Quat)> {
    use bevy::math::{Mat3, Quat, Vec3};
    let (id, basis) = terra_world::objects::chimney_basis(facing)?;
    let [x, y, z] = basis.map(|direction| Vec3::from_array(direction.map(|v| v as f32)));
    Some((id, Quat::from_mat3(&Mat3::from_cols(x, y, z))))
}

/// Called only when the registry contains scene models, once per section epoch.
/// No padded voxel copy or alternative registry is added to ordinary cube jobs.
pub(crate) fn capture(
    section: &BlockSection,
    templates: &TemplateSet,
    journal: &terra_world::edits::EditJournal,
    mut sample_neighbor: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<ModelSnapshot> {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.model_snapshot");
    if section.is_empty() {
        return None;
    }
    let origin = section.position().origin().ok()?;
    let models: Vec<_> = section
        .blocks()
        .iter()
        .enumerate()
        .filter_map(|(index, &material)| {
            if !is_model(templates, material) {
                return None;
            }
            let position = std::array::from_fn(|i| {
                origin[i]
                    + i64::from(
                        [
                            (index % 16) as i32,
                            ((index / 16) % 16) as i32,
                            (index / 256) as i32,
                        ][i],
                    )
            });
            let state = journal.object_state(position).unwrap_or_default();
            Some(ModelCell {
                cell: [
                    (index % 16) as i32,
                    ((index / 16) % 16) as i32,
                    (index / 256) as i32,
                ],
                material,
                state,
                neighbor_facing: std::array::from_fn(|side| {
                    let mut p = position;
                    p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                    journal.object_state(p).unwrap_or_default().facing
                }),
                neighbors: std::array::from_fn(|side| {
                    let mut p = position;
                    p[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                    sample_neighbor(p)
                }),
                peer_wood: state
                    .peer
                    .and_then(|peer| journal.object_state(peer))
                    .map_or(state.wood, |peer| peer.wood),
            })
        })
        .collect();
    let mut border_models = Vec::new();
    let mut border_glass = HashSet::new();
    let funnel_supports = models
        .iter()
        .filter(|model| terra_core::logistics::is_funnel(model.material))
        .filter_map(|model| {
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            terra_world::funnels::supported_with(position, journal, &mut sample_neighbor)
                .then_some(model.cell)
        })
        .collect();
    let chimney_infill = models
        .iter()
        .filter(|model| {
            matches!(model.material, 60..=64) || model.material == 59 && model.state.secondary
        })
        .map(|model| {
            let facing = if model.material == 59 {
                0
            } else {
                model.state.facing
            };
            let material = if model.material == 59 {
                60
            } else {
                model.material
            };
            let pieces =
                terra_world::objects::chimney_infill_with_states(material, facing, |offset| {
                    let position = std::array::from_fn(|axis| {
                        origin[axis] + i64::from(model.cell[axis]) + offset[axis]
                    });
                    sample_neighbor(position).map(|id| {
                        (
                            id,
                            journal.object_state(position).unwrap_or_default().facing,
                        )
                    })
                });
            (model.cell, pieces)
        })
        .collect();
    let pipe_surround = models
        .iter()
        .filter_map(|model| {
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            pipe_surround::capture(model, position, templates, journal, &mut sample_neighbor)
                .map(|surround| (model.cell, surround))
        })
        .collect();
    for axis in 0..3 {
        for boundary in [-1, 16] {
            for u in 0..16 {
                for v in 0..16 {
                    let mut cell = [0; 3];
                    cell[axis] = boundary;
                    cell[(axis + 1) % 3] = u;
                    cell[(axis + 2) % 3] = v;
                    let global = std::array::from_fn(|i| origin[i] + i64::from(cell[i]));
                    let material = sample_neighbor(global);
                    if material.is_some_and(|material| is_model(templates, material)) {
                        border_models.push(cell);
                    }
                    if material == Some(MaterialId::Glass as u16) {
                        border_glass.insert(cell);
                    }
                }
            }
        }
    }
    let water_contacts = models
        .iter()
        .map(|model| model.cell)
        .chain(border_models.iter().copied())
        .filter(|cell| {
            (0..6).any(|side| {
                let mut neighbor = *cell;
                neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
                let global = std::array::from_fn(|axis| origin[axis] + i64::from(neighbor[axis]));
                sample_neighbor(global) == Some(MaterialId::SolidWater as u16)
            })
        })
        .map(|cell| {
            let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
            (
                cell,
                journal.fluid(global).is_some_and(|water| water.amount == 8),
            )
        })
        .collect();
    if models.is_empty() && border_models.is_empty() {
        None
    } else {
        let mut snapshot = ModelSnapshot {
            standalone: false,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: section.blocks().to_vec(),
            models,
            border_models,
            border_glass,
            water_contacts,
            chimney_infill,
            pipe_surround,
            funnel_supports,
        };
        snapshot.refresh_heraldry(section.position(), &journal.gameplay().kingdoms);
        Some(snapshot)
    }
}

pub(crate) fn compose(
    geometry: &BlockMesh,
    snapshot: &ModelSnapshot,
    templates: &TemplateSet,
    section: SectionPos,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    let mut faces: Vec<_> = geometry
        .faces
        .iter()
        .copied()
        .filter(|face| !is_model(templates, face.material))
        .collect();
    // Masked model IDs can already expose their neighbors in the voxel mesher.
    // Restore missing contacts only, never duplicate coplanar support surfaces.
    let mut exposed = [0_u8; 16 * 16 * 16];
    for face in &faces {
        for u in 0..face.width {
            for v in 0..face.height {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let [x, y, z] = cell.map(|p| p as usize);
                exposed[x + 16 * (y + 16 * z)] |= 1 << face.side;
            }
        }
    }
    for model in snapshot
        .models
        .iter()
        .map(|model| model.cell)
        .chain(snapshot.border_models.iter().copied())
    {
        for axis in 0..3 {
            for step in [-1, 1] {
                let mut neighbor = model;
                neighbor[axis] += step;
                if neighbor.iter().any(|&v| !(0..16).contains(&v)) {
                    continue;
                }
                let [x, y, z] = neighbor.map(|v| v as usize);
                let material = snapshot.blocks[x + 16 * (y + 16 * z)];
                let side = axis * 2 + usize::from(step < 0);
                // Flowing water has its own clipped fluid mesh. Restoring its
                // discarded voxel faces manufactures a solid-water cube around
                // every immersed model, including across section boundaries.
                if matches!(material, 0 | 27)
                    || is_model(templates, material)
                    || exposed[x + 16 * (y + 16 * z)] & (1 << side) != 0
                {
                    continue;
                }
                exposed[x + 16 * (y + 16 * z)] |= 1 << side;
                faces.push(BlockFace {
                    occupied_neighbor: false,
                    edge_mask: if material == MaterialId::Glass as u16 {
                        glass_edges(snapshot, neighbor, (axis + 1) % 3, (axis + 2) % 3)
                    } else {
                        15
                    },
                    minimum: neighbor.map(|v| v as u32),
                    width: 1,
                    height: 1,
                    u_axis: (axis + 1) % 3,
                    v_axis: (axis + 2) % 3,
                    side,
                    material,
                });
            }
        }
    }
    let faces = water_contacts::visible_faces(faces, &snapshot.water_contacts);
    let terrain = compose_section_with_segments(
        &faces,
        templates,
        section,
        options,
        &geometry.mineral_growth,
        &geometry.segments,
    )?;
    let mut batches: BTreeMap<_, _> = terrain
        .into_iter()
        .map(|batch| (batch.material, batch))
        .collect();
    // Per-job ownership: repeated roof rows reuse profiles, then all cached CPU
    // geometry is released with this compose. No persistent asset cache grows.
    let mut construction_profiles = ConstructionProfiles::default();
    let mut castle_profiles = castle::Profiles::default();
    let mut plumbing_profiles = plumbing::Profiles::default();
    let mut pot_profiles = super::pots::Profiles::default();
    for model in &snapshot.models {
        if model.material == 59 && model.state.secondary {
            append_chimney_infill(
                &mut batches,
                &ModelCell {
                    material: 60,
                    state: Default::default(),
                    ..*model
                },
                templates,
                snapshot.chimney_infill.get(&model.cell),
            )?;
        }
        // Hinged joinery uses shared authored meshes so moving a leaf never
        // rebuilds or duplicates its fixed frame in a terrain batch.
        if model.state.secondary
            || (model.material == terra_core::kinetic::CONVEYOR
                && model.state.peer.is_some()
                && !snapshot.standalone)
            || (!snapshot.standalone
                && (terra_world::objects::is_opening(model.material)
                    || (model.material == 29 && templates.animated_chests)))
        {
            continue;
        }
        if let Some(surround) = snapshot.pipe_surround.get(&model.cell) {
            pipe_surround::append(&mut batches, model, templates, surround)?;
        }
        append_chimney_infill(
            &mut batches,
            model,
            templates,
            snapshot.chimney_infill.get(&model.cell),
        )?;
        let width = if matches!(model.material, 28 | 29 | 99) && model.state.peer.is_some() {
            2.0
        } else {
            1.0
        };
        let adaptive_chimney = model.material == 60 && model.state.facing >= 8;
        let rotary = terra_core::kinetic::is_rotary(model.material)
            || model.material == terra_core::crusher::HAND_CRANK_MATERIAL;
        let castle = terra_core::castle::is_castle(model.material);
        let centered = adaptive_chimney || rotary || model.material == 166 || castle;
        let crank_offset = if model.material == terra_core::crusher::HAND_CRANK_MATERIAL {
            let origin = section.origin().map_err(|error| error.to_string())?;
            let position = std::array::from_fn(|axis| origin[axis] + i64::from(model.cell[axis]));
            bevy::math::Vec3::from_array(terra_core::kinetic::crank_local_offset(
                position,
                model.state.peer,
            ))
        } else {
            bevy::math::Vec3::ZERO
        };
        let construction = terra_core::construction::is_slab(model.material)
            || terra_core::construction::is_stair(model.material)
            || model.material == 98;
        let (template_id, rotation) =
            if construction || model.material == terra_core::flower_pot::MATERIAL {
                (model.material, bevy::math::Quat::IDENTITY)
            } else if castle {
                let basis = terra_core::construction::stair_basis(model.state.facing);
                (
                    model.material,
                    bevy::math::Quat::from_mat3(&bevy::math::Mat3::from_cols(
                        bevy::math::Vec3::from_array(basis[0].map(f32::from)),
                        bevy::math::Vec3::from_array(basis[1].map(f32::from)),
                        bevy::math::Vec3::from_array(basis[2].map(f32::from)),
                    )),
                )
            } else if adaptive_chimney {
                chimney_template_pose(model.state.facing).ok_or("Invalid adaptive chimney pose")?
            } else if terra_core::lantern::is_part(model.material) {
                (
                    model.material,
                    bevy::math::Quat::from_rotation_y(
                        f32::from(terra_core::lantern::yaw(model.state.facing))
                            * std::f32::consts::FRAC_PI_2,
                    ),
                )
            } else if terra_core::plumbing::is_pipe(model.material) || model.material == 169 {
                (model.material, bevy::math::Quat::IDENTITY)
            } else if model.material == 166 {
                (
                    model.material,
                    match terra_core::plumbing::orientation(model.material, model.state.facing) {
                        4 => bevy::math::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                        5 => bevy::math::Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
                        facing => bevy::math::Quat::from_rotation_y(
                            f32::from(facing) * std::f32::consts::FRAC_PI_2,
                        ),
                    },
                )
            } else if rotary {
                let axis = bevy::math::Vec3::from_array(
                    terra_core::kinetic::axis(model.state.facing).map(|value| value as f32),
                );
                if axis.length_squared() < 0.5 {
                    return Err("Invalid rotary model facing".into());
                }
                (
                    model.material,
                    bevy::math::Quat::from_rotation_arc(bevy::math::Vec3::X, axis),
                )
            } else {
                (
                    model.material,
                    bevy::math::Quat::from_rotation_y(
                        f32::from(if model.material == 148 {
                            terra_core::kinetic::belt_facing(model.state.facing)
                        } else if terra_core::logistics::is_port(model.material)
                            || model.material == terra_core::brewing::MATERIAL
                            || matches!(model.material, 167 | 168)
                        {
                            model.state.facing % 4
                        } else {
                            model.state.facing
                        }) * std::f32::consts::FRAC_PI_2,
                    ),
                )
            };
        let template = templates
            .blocks
            .get(&template_id)
            .ok_or("Missing static model template")?;
        let profile;
        let primitives = if construction {
            profile = construction_profiles.get(model, templates)?;
            profile.as_ref()
        } else if terra_core::castle::connections::is_connecting(model.material) {
            profile = castle_profiles.get(model, &template.model)?;
            profile.as_ref()
        } else if model.material == terra_core::infusion::FLUID_INFUSER {
            profile = std::borrow::Cow::Owned(filling::profile(
                model,
                &template.model,
                snapshot.standalone,
            ));
            profile.as_ref()
        } else if terra_core::plumbing::is_part(model.material) {
            profile = plumbing_profiles.get(model, &template.model, snapshot.standalone);
            profile.as_ref()
        } else {
            &template.model
        };
        let cell = model.cell.map(|value| value as u32);
        let condition = options
            .conditions
            .get(&cell.map(|value| value as usize))
            .copied()
            .unwrap_or_default();
        let exposure = options
            .exposure
            .as_ref()
            .map_or(0.0, |exposure| exposure.payload(cell, section.y * 16));
        let snow_exposed = options
            .exposure
            .as_ref()
            .is_some_and(|exposure| exposure.snow_exposed(cell, section.y * 16));
        let surface = crate::weather::storage::snow::model_surface_payload(
            model.material,
            condition,
            exposure,
            snow_exposed,
        );
        let mut hearth_supports = if model.material == terra_core::campfire::MATERIAL
            && !snapshot.standalone
            && model.neighbors[3] == Some(terra_core::brewing::MATERIAL)
        {
            let slot = templates
                .campfire_support_material
                .ok_or("Missing campfire stone-ring support material")?;
            crate::blocks::cube::box_model(
                &terra_world::interaction_shapes::campfire_support_boxes(),
                0.0,
                [slot; 6],
            )
        } else {
            Vec::new()
        };
        if let Some(glyph) = heraldry::glyph(model, snapshot.castle_heraldry(model.cell), templates)
        {
            hearth_supports.push(glyph);
        }
        for primitive in &mut hearth_supports {
            primitive.tangents = primitive
                .normals
                .iter()
                .map(|&normal| {
                    let normal = bevy::math::Vec3::from_array(normal);
                    let axis = (0..3)
                        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
                        .unwrap();
                    crate::blocks::projected_tangent(normal, axis * 2)
                })
                .collect();
        }
        if model.material == terra_core::brewing::MATERIAL
            && terra_core::brewing::has_fire(model.state.facing)
        {
            let fire = templates
                .blocks
                .get(&terra_core::campfire::MATERIAL)
                .ok_or("Missing attached campfire template")?;
            for primitive in &fire.model {
                let mut primitive = primitive.clone();
                for point in &mut primitive.positions {
                    *point = terra_world::interaction_shapes::attached_campfire_point(
                        point.map(f64::from),
                    )
                    .map(|value| value as f32);
                }
                let scale = terra_world::interaction_shapes::ATTACHED_FIRE_SCALE as f32;
                for normal in &mut primitive.normals {
                    *normal = (bevy::math::Vec3::from_array(*normal)
                        / bevy::math::Vec3::new(scale, 1.0, scale))
                    .normalize_or_zero()
                    .to_array();
                }
                for (tangent, normal) in primitive.tangents.iter_mut().zip(&primitive.normals) {
                    let direction =
                        bevy::math::Vec3::new(tangent[0] * scale, tangent[1], tangent[2] * scale);
                    *tangent = crate::blocks::tangent_frame(
                        direction,
                        bevy::math::Vec3::from_array(*normal),
                        tangent[3],
                    );
                }
                hearth_supports.push(primitive);
            }
        }
        for (primitive_index, primitive) in primitives.iter().chain(&hearth_supports).enumerate() {
            if model.material == terra_core::lantern::LANTERN
                && templates.lantern_variants.get(&primitive.material)
                    != Some(&terra_core::lantern::variant(model.state.facing))
            {
                continue;
            }
            let heraldry = snapshot.castle_heraldry(model.cell);
            let heraldry_cloth = templates
                .castle_cloth_materials
                .contains(&primitive.material);
            let heraldry_icon = templates.castle_icon_materials.get(&primitive.material);
            let catalog_glyph = primitive_index >= primitives.len()
                && matches!(heraldry.icon, terra_core::kingdom::KingdomIcon::Catalog(_));
            if heraldry_icon.is_some_and(|icon| *icon != heraldry.icon && !catalog_glyph)
                || templates
                    .castle_arch_join_materials
                    .contains(&primitive.material)
                    && !snapshot.standalone
                    && castle::paired_arch(model)
            {
                continue;
            }
            if model.material == terra_core::infusion::HOLD_BLOCK
                && !snapshot.standalone
                && templates
                    .conveyor_ribbon_materials
                    .contains(&primitive.material)
            {
                continue;
            }
            let rotation = if matches!(model.material, 152 | 153 | 157 | 159)
                && model.state.facing >= 4
                && templates
                    .logistics_arrow_materials
                    .contains(&primitive.material)
            {
                rotation * bevy::math::Quat::from_rotation_y(std::f32::consts::PI)
            } else {
                rotation
            };
            let rotation = if templates
                .castle_decor_materials
                .contains(&primitive.material)
            {
                rotation * castle::detail_rotation(section, model.cell)
            } else {
                rotation
            };
            if matches!(model.material, 153 | 157)
                && !snapshot.standalone
                && templates
                    .logistics_support_materials
                    .contains(&primitive.material)
                && !model.neighbors[2].is_some_and(terra_core::beds::supports_bed_floor)
                && !snapshot.funnel_supports.contains(&model.cell)
            {
                continue;
            }
            if model.material == 148
                && !snapshot.standalone
                && !templates
                    .crusher_moving_materials
                    .contains_key(&primitive.material)
                && !model.neighbors[2].is_some_and(terra_core::beds::supports_bed_floor)
            {
                continue;
            }
            if !snapshot.include_moving
                && crate::crushers::articulated_material(model.material)
                && (model.state.peer.is_some()
                    || terra_core::kinetic::is_rotary(model.material)
                    || terra_core::kinetic::is_gearbox(model.material)
                    || model.material == terra_core::kinetic::CONVEYOR
                    || model.material == 168)
                && templates
                    .crusher_moving_materials
                    .contains_key(&primitive.material)
            {
                continue;
            }
            if terra_core::construction::is_fence(model.material)
                && let Some(arm) = terra_core::construction::fence_arm(primitive_index / 6)
                && !terra_core::construction::connects(
                    model.neighbors
                        [terra_core::construction::fence_neighbor_side(arm, model.state.facing)],
                )
            {
                continue;
            }
            let batch = batches
                .entry(primitive.material)
                .or_insert_with(|| MeshBatch {
                    material: primitive.material,
                    ..Default::default()
                });
            if model.material == 29
                && model.state.peer.is_some()
                && model.peer_wood != model.state.wood
                && templates.wood_materials.contains(&primitive.material)
            {
                append_split_wood(batch, primitive, model, width, rotation)?;
                continue;
            }
            let base =
                u32::try_from(batch.positions.len()).map_err(|_| "Static model batch overflow")?;
            let wool_fabric = construction && templates.untinted_wool_material(primitive.material);
            if model.material == 40
                || heraldry_cloth
                || heraldry_icon.is_some()
                || wool_fabric && options.tints.contains_key(&model.cell.map(|v| v as usize))
                || model.material == 106
                    && options.tints.contains_key(&model.cell.map(|v| v as usize))
                || templates.wood_materials.contains(&primitive.material)
                || templates.blanket_materials.contains(&primitive.material)
                || templates
                    .conveyor_ribbon_materials
                    .contains(&primitive.material)
                || !batch.colors.is_empty()
            {
                batch.colors.resize(batch.positions.len(), [1.0; 4]);
                let tint = if heraldry_cloth {
                    crate::dye_tint::rgb(heraldry.color)
                } else if heraldry_icon.is_some() {
                    castle::icon_color(heraldry)
                } else if templates
                    .conveyor_ribbon_materials
                    .contains(&primitive.material)
                {
                    options
                        .tints
                        .get(&model.cell.map(|v| v as usize))
                        .copied()
                        .map(crate::dye_tint::rgb)
                        .unwrap_or_else(|| {
                            terra_core::kinetic::belt_color(model.state.facing)
                                .map_or([1.0; 4], crate::dye_tint::color)
                        })
                } else if templates.blanket_materials.contains(&primitive.material) {
                    options
                        .tints
                        .get(&model.cell.map(|v| v as usize))
                        .copied()
                        .map_or_else(
                            || {
                                crate::dye_tint::color(terra_core::beds::bed_color(
                                    model.material,
                                    model.state.facing,
                                ))
                            },
                            crate::dye_tint::rgb,
                        )
                } else if model.material == 106 || wool_fabric {
                    options
                        .tints
                        .get(&model.cell.map(|v| v as usize))
                        .copied()
                        .map_or([1.0; 4], crate::dye_tint::rgb)
                } else if templates.wood_materials.contains(&primitive.material) {
                    model.state.wood.tint()
                } else {
                    [1.0; 4]
                };
                batch
                    .colors
                    .extend(std::iter::repeat_n(tint, primitive.positions.len()));
            }
            batch
                .positions
                .extend(primitive.positions.iter().map(|point| {
                    if model.material == 40 {
                        let p = terra_core::block_shapes::torch_point(
                            model.state.facing,
                            point.map(f64::from),
                        );
                        return std::array::from_fn(|i| p[i] as f32 + model.cell[i] as f32);
                    }
                    let mut point = bevy::math::Vec3::from_array(*point);
                    point.x = point.x * width - 0.5;
                    point.z -= 0.5;
                    if centered {
                        point.y -= 0.5;
                    }
                    (rotation * (point + crank_offset)
                        + bevy::math::Vec3::new(0.5, if centered { 0.5 } else { 0.0 }, 0.5)
                        + bevy::math::Vec3::from_array(model.cell.map(|p| p as f32)))
                    .to_array()
                }));
            batch.normals.extend(primitive.normals.iter().map(|normal| {
                if model.material == 40 {
                    return terra_core::block_shapes::torch_vector(
                        model.state.facing,
                        normal.map(f64::from),
                    )
                    .map(|v| v as f32);
                }
                let mut normal = bevy::math::Vec3::from_array(*normal);
                normal.x /= width;
                (rotation * normal).normalize_or_zero().to_array()
            }));
            batch.append_tangents(
                base as usize,
                primitive
                    .tangents
                    .iter()
                    .enumerate()
                    .map(|(index, tangent)| {
                        let mut direction =
                            bevy::math::Vec3::from_array([tangent[0], tangent[1], tangent[2]]);
                        let mut normal = bevy::math::Vec3::from_array(primitive.normals[index]);
                        if model.material == 40 {
                            direction = bevy::math::Vec3::from_array(
                                terra_core::block_shapes::torch_vector(
                                    model.state.facing,
                                    direction.to_array().map(f64::from),
                                )
                                .map(|v| v as f32),
                            );
                            normal = bevy::math::Vec3::from_array(
                                terra_core::block_shapes::torch_vector(
                                    model.state.facing,
                                    normal.to_array().map(f64::from),
                                )
                                .map(|v| v as f32),
                            );
                        } else {
                            direction.x *= width;
                            normal.x /= width;
                            direction = rotation * direction;
                            normal = rotation * normal;
                        }
                        crate::blocks::tangent_frame(direction, normal, tangent[3])
                    }),
            );
            batch.uvs.extend_from_slice(&primitive.uvs);
            if model.material == 40 {
                batch.rooted_wind.resize(base as usize, [0.0; 2]);
                batch.rooted_wind.extend(
                    primitive
                        .positions
                        .iter()
                        .map(|p| [0.0, ((p[1] - 0.62) / 0.30).clamp(0.0, 1.0)]),
                );
            }
            for &index in &primitive.indices {
                batch.indices.push(
                    base.checked_add(index)
                        .ok_or("Static model index overflow")?,
                );
            }
            if let Some(surface) = surface.filter(|_| batch.rooted_wind.is_empty()) {
                batch.surface_water.resize(batch.positions.len(), [0.0; 2]);
                for payload in &mut batch.surface_water[base as usize..] {
                    *payload = surface;
                }
            } else if !batch.surface_water.is_empty() {
                batch.surface_water.resize(batch.positions.len(), [0.0; 2]);
            }
        }
        if model.material == terra_core::flower_pot::MATERIAL {
            pot_profiles.append(&mut batches, model.state.facing, model.cell, templates)?;
        }
    }
    Ok(batches.into_values().collect())
}

fn append_chimney_infill(
    batches: &mut BTreeMap<usize, MeshBatch>,
    model: &ModelCell,
    templates: &TemplateSet,
    captured: Option<&ChimneyInfill>,
) -> Result<(), String> {
    use bevy::math::Vec3;
    let fallback;
    let pieces = if let Some(pieces) = captured {
        pieces
    } else {
        fallback = terra_world::objects::chimney_infill_with_states(
            model.material,
            model.state.facing,
            |direction| {
                if direction.iter().map(|value| value.abs()).sum::<i64>() != 1 {
                    return None;
                }
                let axis = direction.iter().position(|v| *v != 0)?;
                let side = axis * 2 + usize::from(direction[axis] > 0);
                model.neighbors[side].map(|id| (id, model.neighbor_facing[side]))
            },
        );
        &fallback
    };
    for (bounds, material) in pieces {
        let Some(template) = templates
            .blocks
            .get(material)
            .filter(|b| b.model.is_empty())
        else {
            continue;
        };
        let low = Vec3::from_array(bounds.minimum.map(|v| v as f32));
        let size = Vec3::from_array(bounds.maximum.map(|v| v as f32)) - low;
        for primitives in &template.faces {
            for primitive in primitives {
                let batch = batches
                    .entry(primitive.material)
                    .or_insert_with(|| MeshBatch {
                        material: primitive.material,
                        ..Default::default()
                    });
                let base = u32::try_from(batch.positions.len())
                    .map_err(|_| "Chimney infill index overflow")?;
                let has_colors = !batch.colors.is_empty();
                for (i, p) in primitive.positions.iter().enumerate() {
                    let point = low + Vec3::from_array(*p) * size;
                    batch
                        .positions
                        .push((point + Vec3::from_array(model.cell.map(|v| v as f32))).to_array());
                    let normal = Vec3::from_array(primitive.normals[i]);
                    batch
                        .normals
                        .push((normal / size).normalize_or_zero().to_array());
                    let axis = normal.abs().max_element();
                    let normal_axis = if normal.x.abs() == axis {
                        0
                    } else if normal.y.abs() == axis {
                        1
                    } else {
                        2
                    };
                    batch
                        .uvs
                        .push([point[(normal_axis + 1) % 3], point[(normal_axis + 2) % 3]]);
                    if has_colors {
                        batch.colors.push([1.0; 4]);
                    }
                }
                batch.append_tangents(
                    base as usize,
                    primitive.tangents.iter().enumerate().map(|(index, _)| {
                        let normal = Vec3::from_array(primitive.normals[index]);
                        let axis = if normal.x.abs() >= normal.y.abs()
                            && normal.x.abs() >= normal.z.abs()
                        {
                            0
                        } else if normal.y.abs() >= normal.z.abs() {
                            1
                        } else {
                            2
                        };
                        let mut u = Vec3::ZERO;
                        let mut v = Vec3::ZERO;
                        u[(axis + 1) % 3] = 1.0;
                        v[(axis + 2) % 3] = 1.0;
                        let normal = (normal / size).normalize_or_zero();
                        crate::blocks::tangent_frame(u, normal, normal.cross(u).dot(v))
                    }),
                );
                for &index in &primitive.indices {
                    batch.indices.push(
                        base.checked_add(index)
                            .ok_or("Chimney infill index overflow")?,
                    );
                }
                if !batch.surface_water.is_empty() {
                    batch.surface_water.resize(batch.positions.len(), [0.0; 2]);
                }
                if !batch.rooted_wind.is_empty() {
                    batch.rooted_wind.resize(batch.positions.len(), [0.0; 2]);
                }
                if !batch.growth_anchors.is_empty() {
                    batch.growth_anchors.resize(batch.positions.len(), [0.0; 3]);
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SplitVertex {
    position: bevy::math::Vec3,
    normal: bevy::math::Vec3,
    uv: bevy::math::Vec2,
    tangent: bevy::math::Vec4,
}
impl SplitVertex {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            position: self.position.lerp(other.position, t),
            normal: self.normal.lerp(other.normal, t).normalize_or_zero(),
            uv: self.uv.lerp(other.uv, t),
            tangent: self.tangent.lerp(other.tangent, t),
        }
    }
}

fn clip_wood(triangle: [SplitVertex; 3], left: bool) -> ([SplitVertex; 4], usize) {
    let mut result = [triangle[0]; 4];
    let mut length = 0;
    if !left && triangle.iter().all(|vertex| vertex.position.x == 0.5) {
        return (result, 0);
    }
    let inside = |v: SplitVertex| {
        if left {
            v.position.x <= 0.5
        } else {
            v.position.x >= 0.5
        }
    };
    let mut previous = triangle[2];
    for current in triangle {
        if inside(previous) != inside(current) {
            result[length] = previous.lerp(
                current,
                (0.5 - previous.position.x) / (current.position.x - previous.position.x),
            );
            length += 1;
        }
        if inside(current) {
            result[length] = current;
            length += 1;
        }
        previous = current;
    }
    (result, length)
}

/// Split surfaces at the shared cell boundary, preserving UVs and winding.
/// Both sides remain present; only their constant wood tint differs.
fn append_split_wood(
    batch: &mut MeshBatch,
    primitive: &crate::blocks::PrimitiveTemplate,
    model: &ModelCell,
    width: f32,
    rotation: bevy::math::Quat,
) -> Result<(), String> {
    batch.colors.resize(batch.positions.len(), [1.0; 4]);
    for indices in primitive.indices.chunks_exact(3) {
        let mut triangle = [SplitVertex {
            position: bevy::math::Vec3::ZERO,
            normal: bevy::math::Vec3::ZERO,
            uv: bevy::math::Vec2::ZERO,
            tangent: bevy::math::Vec4::ZERO,
        }; 3];
        for (corner, &index) in indices.iter().enumerate() {
            let index = index as usize;
            triangle[corner] = SplitVertex {
                position: bevy::math::Vec3::from_array(
                    *primitive
                        .positions
                        .get(index)
                        .ok_or("Invalid split model position")?,
                ),
                normal: bevy::math::Vec3::from_array(
                    *primitive
                        .normals
                        .get(index)
                        .ok_or("Invalid split model normal")?,
                ),
                uv: bevy::math::Vec2::from_array(
                    *primitive.uvs.get(index).ok_or("Invalid split model UV")?,
                ),
                tangent: bevy::math::Vec4::from_array(
                    primitive
                        .tangents
                        .get(index)
                        .copied()
                        .unwrap_or([1.0, 0.0, 0.0, 1.0]),
                ),
            };
        }
        for (left, wood) in [(true, model.state.wood), (false, model.peer_wood)] {
            let (polygon, length) = clip_wood(triangle, left);
            for i in 1..length.saturating_sub(1) {
                let points = [polygon[0], polygon[i], polygon[i + 1]];
                if (points[1].position - points[0].position)
                    .cross(points[2].position - points[0].position)
                    .length_squared()
                    < 1.0e-12
                {
                    continue;
                }
                let base = u32::try_from(batch.positions.len())
                    .map_err(|_| "Split model batch overflow")?;
                for vertex in points {
                    let mut point = vertex.position;
                    point.x = point.x * width - 0.5;
                    point.z -= 0.5;
                    batch.positions.push(
                        (rotation * point
                            + bevy::math::Vec3::new(0.5, 0.0, 0.5)
                            + bevy::math::Vec3::from_array(model.cell.map(|v| v as f32)))
                        .to_array(),
                    );
                    let mut normal = vertex.normal;
                    normal.x /= width;
                    let normal = (rotation * normal).normalize_or_zero();
                    batch.normals.push(normal.to_array());
                    let tangent = vertex.tangent;
                    let direction =
                        rotation * bevy::math::Vec3::new(tangent.x * width, tangent.y, tangent.z);
                    let frame = crate::blocks::tangent_frame(direction, normal, tangent.w);
                    batch.append_tangents(
                        batch.positions.len() - 1,
                        (!primitive.tangents.is_empty())
                            .then_some(frame)
                            .into_iter(),
                    );
                    batch.uvs.push(vertex.uv.to_array());
                    batch.colors.push(wood.tint());
                }
                batch.indices.extend([
                    base,
                    base.checked_add(1).ok_or("Split model index overflow")?,
                    base.checked_add(2).ok_or("Split model index overflow")?,
                ]);
            }
        }
    }
    Ok(())
}

fn glass_edges(snapshot: &ModelSnapshot, cell: [i32; 3], u_axis: usize, v_axis: usize) -> u8 {
    let mut edges = 0;
    for (bit, axis, step) in [
        (1, u_axis, -1),
        (2, u_axis, 1),
        (4, v_axis, -1),
        (8, v_axis, 1),
    ] {
        let mut neighbor = cell;
        neighbor[axis] += step;
        let glass = if neighbor.iter().all(|&value| (0..16).contains(&value)) {
            let [x, y, z] = neighbor.map(|value| value as usize);
            snapshot.blocks[x + 16 * (y + 16 * z)] == MaterialId::Glass as u16
        } else {
            snapshot.border_glass.contains(&neighbor)
        };
        if !glass {
            edges |= bit;
        }
    }
    edges
}

#[cfg(test)]
mod wood_tests {
    use super::*;
    use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};

    #[test]
    fn placed_rgb_textiles_change_only_fabric_and_keep_pose_uvs_and_wood_for_negative_sections() {
        let primitive = |material| PrimitiveTemplate {
            material,
            positions: vec![[0.2, 0.3, 0.4], [0.3, 0.3, 0.4], [0.2, 0.4, 0.4]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
        };
        let mut templates = TemplateSet::default();
        templates.wood_materials.insert(0);
        templates.blanket_materials.insert(1);
        templates.conveyor_ribbon_materials.insert(3);
        for (material, slots) in [(71, vec![0, 1, 2]), (106, vec![4]), (148, vec![0, 2, 3])] {
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: slots.into_iter().map(primitive).collect(),
                },
            );
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: Default::default(),
            tints: std::sync::Arc::new([([4, 4, 4], [71, 93, 184])].into()),
        };
        let expected = crate::dye_tint::rgb([71, 93, 184]);
        for material in [71, 106, 148] {
            for facing in 0..4 {
                let snapshot = ModelSnapshot {
                    standalone: true,
                    heraldry: HashMap::new(),
                    domains: Vec::new(),
                    include_moving: true,
                    blocks: vec![0; 4096],
                    models: vec![ModelCell {
                        cell: [4; 3],
                        material,
                        state: terra_world::edits::BlockObjectState {
                            facing,
                            wood: terra_core::WoodSpecies::Birch,
                            ..Default::default()
                        },
                        peer_wood: Default::default(),
                        neighbors: [None; 6],
                        neighbor_facing: [0; 6],
                    }],
                    border_models: Vec::new(),
                    border_glass: HashSet::new(),
                    water_contacts: HashMap::new(),
                    chimney_infill: BTreeMap::new(),
                    pipe_surround: BTreeMap::new(),
                    funnel_supports: HashSet::new(),
                };
                let plain_options = RenderOptions {
                    tints: Default::default(),
                    ..options.clone()
                };
                let section = SectionPos::new(-1, -2, -1);
                let plain = compose(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    section,
                    &plain_options,
                )
                .unwrap();
                let actual = compose(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    section,
                    &options,
                )
                .unwrap();
                assert_eq!(actual.len(), plain.len());
                for (actual, plain) in actual.iter().zip(&plain) {
                    assert_eq!(actual.positions, plain.positions);
                    assert_eq!(actual.normals, plain.normals);
                    assert_eq!(actual.uvs, plain.uvs);
                    assert_eq!(actual.indices, plain.indices);
                    if matches!(actual.material, 1 | 3 | 4) {
                        assert_eq!(actual.colors, vec![expected; actual.positions.len()]);
                    } else {
                        assert_eq!(
                            actual.colors, plain.colors,
                            "wood/metal must never inherit fabric RGB"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn funnel_supports_render_only_over_a_loaded_full_floor_but_remain_in_previews() {
        let primitive = |material| PrimitiveTemplate {
            material,
            positions: vec![[0.1, 0.0, 0.1], [0.2, 0.0, 0.1], [0.1, 0.1, 0.1]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
        };
        let mut templates = TemplateSet::default();
        templates.logistics_support_materials.extend([1, 2]);
        templates.blocks.insert(
            153,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1), primitive(2)],
            },
        );
        for (below, supported) in [(None, false), (Some(0), false), (Some(3), true)] {
            for standalone in [false, true] {
                let mut neighbors = [None; 6];
                neighbors[2] = below;
                let snapshot = ModelSnapshot {
                    standalone,
                    heraldry: HashMap::new(),
                    domains: Vec::new(),
                    include_moving: false,
                    blocks: vec![0; 4096],
                    models: vec![ModelCell {
                        cell: [4, 4, 4],
                        material: 153,
                        state: Default::default(),
                        peer_wood: Default::default(),
                        neighbors,
                        neighbor_facing: [0; 6],
                    }],
                    border_models: Vec::new(),
                    border_glass: HashSet::new(),
                    water_contacts: HashMap::new(),
                    chimney_infill: BTreeMap::new(),
                    pipe_surround: BTreeMap::new(),
                    funnel_supports: HashSet::new(),
                };
                let batches = compose(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    SectionPos::new(-1, 0, -1),
                    &RenderOptions {
                        water_level: None,
                        detail_level: super::super::DetailLevel::Near,
                        terrain: None,
                        exposure: None,
                        tints: Default::default(),
                        conditions: Default::default(),
                    },
                )
                .unwrap();
                let materials: HashSet<_> = batches.iter().map(|batch| batch.material).collect();
                assert!(materials.contains(&0), "the actual funnel is never removed");
                assert_eq!(materials.contains(&1), supported || standalone);
                assert_eq!(materials.contains(&2), supported || standalone);
            }
        }
    }

    #[test]
    fn packed_belt_dyes_and_port_modes_preserve_housing_pose_and_only_reverse_the_arrow() {
        let primitive = |material| PrimitiveTemplate {
            material,
            positions: vec![[0.2, 0.3, 0.1], [0.3, 0.3, 0.1], [0.2, 0.4, 0.1]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            tangents: Vec::new(),
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
        };
        let mut templates = TemplateSet::default();
        templates.conveyor_ribbon_materials.insert(1);
        templates.logistics_arrow_materials.insert(2);
        for material in [148, 152, 153] {
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: vec![primitive(0), primitive(if material == 148 { 1 } else { 2 })],
                },
            );
        }
        let render = |material, facing| {
            model_geometry(
                material,
                terra_world::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                },
                Default::default(),
                [None; 6],
                [0; 6],
                &templates,
            )
            .unwrap()
        };
        for facing in 0..4 {
            let plain = render(148, facing);
            for color in terra_core::beds::DyeColor::ALL {
                let dyed = render(148, terra_core::kinetic::dyed_belt_facing(facing, color));
                for (original, current) in plain.iter().zip(&dyed) {
                    for (a, b) in original.positions.iter().zip(&current.positions) {
                        assert!(
                            bevy::math::Vec3::from_array(*a)
                                .abs_diff_eq(bevy::math::Vec3::from_array(*b), 1e-6)
                        );
                    }
                }
                assert!(
                    dyed.iter()
                        .find(|b| b.material == 1)
                        .unwrap()
                        .colors
                        .iter()
                        .all(|&tint| tint == color.tint())
                );
            }
            for material in [152, 153] {
                let base = render(material, facing);
                let reversed = render(material, facing + 4);
                assert_eq!(
                    base.iter().find(|b| b.material == 0).unwrap().positions,
                    reversed.iter().find(|b| b.material == 0).unwrap().positions
                );
                assert_ne!(
                    base.iter().find(|b| b.material == 2).unwrap().positions,
                    reversed.iter().find(|b| b.material == 2).unwrap().positions
                );
            }
        }
    }
    #[test]
    fn mounted_crank_static_geometry_uses_the_same_centered_axis_in_negative_sections() {
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            100,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![PrimitiveTemplate {
                    positions: vec![
                        [0.22, 0.455, 0.515],
                        [0.22, 0.555, 0.515],
                        [0.22, 0.455, 0.615],
                    ],
                    normals: vec![[1.0, 0.0, 0.0]; 3],
                    tangents: Vec::new(),
                    uvs: vec![[0.0; 2]; 3],
                    indices: vec![0, 1, 2],
                    material: 0,
                }],
            },
        );
        let section = SectionPos::new(-1, 0, -1);
        let cell = [15, 4, 15];
        let origin = section.origin().unwrap();
        let position: [i64; 3] = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
        for facing in 0..6 {
            let axis = terra_core::kinetic::axis(facing);
            let snapshot = ModelSnapshot {
                standalone: false,
                heraldry: HashMap::new(),
                domains: Vec::new(),
                include_moving: false,
                blocks: vec![0; 4096],
                models: vec![ModelCell {
                    cell,
                    material: 100,
                    state: terra_world::edits::BlockObjectState {
                        facing,
                        peer: Some(std::array::from_fn(|i| position[i] - axis[i])),
                        ..Default::default()
                    },
                    peer_wood: Default::default(),
                    neighbors: [None; 6],
                    neighbor_facing: [0; 6],
                }],
                border_models: Vec::new(),
                border_glass: HashSet::new(),
                water_contacts: HashMap::new(),
                chimney_infill: BTreeMap::new(),
                pipe_surround: BTreeMap::new(),
                funnel_supports: HashSet::new(),
            };
            let batches = compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                section,
                &RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .unwrap();
            let rotation = bevy::math::Quat::from_rotation_arc(
                bevy::math::Vec3::X,
                bevy::math::Vec3::from_array(axis.map(|v| v as f32)),
            );
            let expected = bevy::math::Vec3::from_array(cell.map(|v| v as f32))
                + bevy::math::Vec3::splat(0.5)
                + rotation * bevy::math::Vec3::new(-0.28, 0.0, 0.0);
            assert!(
                bevy::math::Vec3::from_array(batches[0].positions[0]).distance(expected) < 0.00001
            );
        }
    }

    #[test]
    fn machine_ghost_includes_rotors_but_mining_isolation_does_not_duplicate_dynamic_parts() {
        let primitive = |material| PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material,
        };
        let mut templates = TemplateSet::default();
        templates.crusher_moving_materials.insert(1, 0);
        templates.blocks.insert(
            99,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1)],
            },
        );
        let state = terra_world::edits::BlockObjectState {
            peer: Some([1, 0, 0]),
            ..Default::default()
        };
        let snapshot = ModelSnapshot {
            standalone: false,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: vec![0; 4096],
            models: vec![ModelCell {
                cell: [0; 3],
                material: 99,
                state,
                peer_wood: Default::default(),
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
            }],
            border_models: Vec::new(),
            border_glass: HashSet::new(),
            water_contacts: HashMap::new(),
            chimney_infill: BTreeMap::new(),
            pipe_surround: BTreeMap::new(),
            funnel_supports: HashSet::new(),
        };
        let preview =
            model_geometry(99, state, Default::default(), [None; 6], [0; 6], &templates).unwrap();
        assert!(preview.iter().any(|b| b.material == 0));
        assert!(preview.iter().any(|b| b.material == 1));
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let (_, isolated) = isolate_model(
            &BlockMesh::default(),
            &snapshot,
            &templates,
            SectionPos::default(),
            &options,
            [0; 3],
            None,
        )
        .unwrap();
        assert!(isolated.iter().any(|b| b.material == 0));
        assert!(
            !isolated.iter().any(|b| b.material == 1),
            "the existing shared dynamic assembly remains the sole rotor owner during mining"
        );
    }
    #[test]
    fn pipe_infill_uses_neighbor_face_materials_and_valid_batches() {
        let mut templates = TemplateSet::default();
        for (material, slot) in [(3, 10), (4, 11), (16, 12), (58, 13)] {
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    faces: crate::blocks::cube_faces(0.0, [slot; 6], None),
                    face_materials: [slot; 6],
                    side_cap: None,
                    details: vec![],
                    model: vec![],
                },
            );
        }
        let model = ModelCell {
            cell: [15, 2, 15],
            material: 60,
            state: Default::default(),
            peer_wood: Default::default(),
            neighbors: [Some(3), Some(4), None, None, Some(16), Some(58)],
            neighbor_facing: [0; 6],
        };
        let mut batches = BTreeMap::new();
        batches.insert(
            10,
            MeshBatch {
                material: 10,
                positions: vec![[0.0; 3]; 3],
                normals: vec![[0.0, 1.0, 0.0]; 3],
                uvs: vec![[0.0; 2]; 3],
                colors: vec![[1.0; 4]; 3],
                rooted_wind: vec![[0.0; 2]; 3],
                surface_water: vec![[0.0; 2]; 3],
                ..Default::default()
            },
        );
        append_chimney_infill(&mut batches, &model, &templates, None).unwrap();
        assert_eq!(batches.len(), 4);
        for batch in batches.values() {
            assert_eq!(batch.positions.len(), batch.normals.len());
            assert_eq!(batch.positions.len(), batch.uvs.len());
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|i| (*i as usize) < batch.positions.len())
            );
            if batch.material == 10 {
                assert_eq!(batch.positions.len(), batch.colors.len());
                assert_eq!(batch.positions.len(), batch.rooted_wind.len());
                assert_eq!(batch.positions.len(), batch.surface_water.len());
            }
        }
    }

    #[test]
    fn pipe_capture_reuses_exact_roof_corner_shapes_across_negative_section_borders() {
        use terra_world::edits::{BlockObjectState, EditJournal};
        let origin = [-1, 2, -1];
        let mixed = terra_core::construction::merged_slab(82, 0, 85).unwrap();
        let cells = [
            (origin, (60, 0)),
            ([-2, 2, -1], (77, 0)),
            ([0, 2, -1], (85, 0)),
            ([-1, 2, -2], (82, mixed)),
            ([-1, 2, 0], (77, 1)),
            ([0, 2, 0], (77, 2)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let mut journal = EditJournal::default();
        for (&position, &(id, facing)) in &cells {
            journal.record(position, id).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let mut templates = TemplateSet::default();
        for id in [3, 16, 60] {
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    render_kind: if id == 60 {
                        BlockRenderKind::CustomMesh
                    } else {
                        BlockRenderKind::Cube
                    },
                    bevel_width: 0.0,
                    faces: crate::blocks::cube_faces(0.0, [usize::from(id); 6], None),
                    face_materials: [usize::from(id); 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: if id == 60 {
                        vec![PrimitiveTemplate {
                            tangents: Vec::new(),
                            material: 60,
                            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                            normals: vec![[0.0, 0.0, 1.0]; 3],
                            uvs: vec![[0.0; 2]; 3],
                            indices: vec![0, 1, 2],
                        }]
                    } else {
                        Vec::new()
                    },
                },
            );
        }
        let (section, local) = terra_voxel::global_to_section(origin);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, 60).unwrap();
        let snapshot = capture(&data, &templates, &journal, |position| {
            cells.get(&position).map(|cell| cell.0).or(Some(0))
        })
        .unwrap();
        let expected = terra_world::objects::chimney_infill_with_states(60, 0, |offset| {
            cells
                .get(&std::array::from_fn(|axis| origin[axis] + offset[axis]))
                .copied()
        });
        assert!(!expected.is_empty());
        let local = local.map(|axis| axis as i32);
        assert_eq!(snapshot.chimney_infill.get(&local), Some(&expected));
        let mut batches = BTreeMap::new();
        append_chimney_infill(
            &mut batches,
            &snapshot.models[0],
            &templates,
            snapshot.chimney_infill.get(&local),
        )
        .unwrap();
        assert!(!batches.is_empty());
        assert!(batches.keys().all(|slot| matches!(slot, 3 | 16)));
        for batch in batches.values() {
            assert_eq!(batch.positions.len(), batch.normals.len());
            assert_eq!(batch.positions.len(), batch.uvs.len());
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&index| (index as usize) < batch.positions.len())
            );
        }
    }
    #[test]
    fn mixed_double_chest_splits_surface_colors_without_gaps_or_material_batches() {
        use terra_core::WoodSpecies;
        let primitive = PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 1, 2],
            material: 5,
        };
        let model = ModelCell {
            cell: [0; 3],
            material: 29,
            state: terra_world::edits::BlockObjectState {
                wood: WoodSpecies::Birch,
                peer: Some([1, 0, 0]),
                ..Default::default()
            },
            peer_wood: WoodSpecies::Spruce,
            neighbors: [None; 6],
            neighbor_facing: [0; 6],
        };
        let mut batch = MeshBatch {
            material: 5,
            ..Default::default()
        };
        append_split_wood(
            &mut batch,
            &primitive,
            &model,
            2.0,
            bevy::math::Quat::IDENTITY,
        )
        .unwrap();
        assert_eq!(batch.material, 5);
        assert_eq!(batch.positions.len(), 9);
        assert_eq!(batch.colors.len(), batch.positions.len());
        let mut area = 0.0;
        for indices in batch.indices.chunks_exact(3) {
            let points: [bevy::math::Vec3; 3] = std::array::from_fn(|corner| {
                bevy::math::Vec3::from_array(batch.positions[indices[corner] as usize])
            });
            let colors: [[f32; 4]; 3] =
                std::array::from_fn(|corner| batch.colors[indices[corner] as usize]);
            assert!(colors.iter().all(|color| *color == colors[0]));
            if colors[0] == WoodSpecies::Birch.tint() {
                assert!(points.iter().all(|point| point.x <= 1.0));
            } else {
                assert_eq!(colors[0], WoodSpecies::Spruce.tint());
                assert!(points.iter().all(|point| point.x >= 1.0));
            }
            let cross = (points[1] - points[0]).cross(points[2] - points[0]);
            assert!(cross.z > 0.0);
            area += cross.length() * 0.5;
        }
        assert!((area - 1.0).abs() < 1.0e-6);
        for (point, uv) in batch.positions.iter().zip(&batch.uvs) {
            assert!((uv[0] - point[0] * 0.5).abs() < 1.0e-6);
            assert!((uv[1] - point[1]).abs() < 1.0e-6);
        }
    }
    #[test]
    fn species_tints_only_wood_vertices_without_adding_batches_or_changing_geometry() {
        let mut templates = TemplateSet::default();
        let primitive = PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material: 0,
        };
        let metal = PrimitiveTemplate {
            material: 1,
            ..primitive.clone()
        };
        templates.wood_materials.insert(0);
        templates.blocks.insert(
            28,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive, metal],
            },
        );
        let section = SectionPos::default();
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block([0; 3], 28).unwrap();
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record([0; 3], 28).unwrap();
        let mut previous: Option<Vec<MeshBatch>> = None;
        for wood in [
            terra_core::WoodSpecies::Oak,
            terra_core::WoodSpecies::Birch,
            terra_core::WoodSpecies::Spruce,
        ] {
            journal
                .set_object_state(
                    [0; 3],
                    Some(terra_world::edits::BlockObjectState {
                        wood,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let snapshot = capture(&data, &templates, &journal, |_| Some(0)).unwrap();
            let batches = compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                section,
                &RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
            )
            .unwrap();
            assert_eq!(batches.len(), 2);
            assert!(batches[0].colors.iter().all(|color| *color == wood.tint()));
            assert_eq!(batches[0].colors.len(), batches[0].positions.len());
            assert!(batches[1].colors.is_empty());
            let (remaining, isolated) = isolate_model(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                section,
                &RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: Default::default(),
                },
                [0; 3],
                None,
            )
            .unwrap();
            assert!(remaining.is_empty());
            assert_eq!(
                isolated, batches,
                "mining must retain original imported geometry, UVs and wood tint"
            );
            if let Some(previous) = &previous {
                for (before, after) in previous.iter().zip(&batches) {
                    assert_eq!(before.positions, after.positions);
                    assert_eq!(before.normals, after.normals);
                    assert_eq!(before.indices, after.indices);
                }
            }
            previous = Some(batches);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn custom_mechanics_and_ports_carry_explicit_snow_shedding_flags() {
        use super::*;
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        for material in [99, 100].into_iter().chain(141..=162) {
            let mut templates = TemplateSet::default();
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: vec![PrimitiveTemplate {
                        positions: vec![[0.2, 0.5, 0.2], [0.8, 0.5, 0.2], [0.2, 0.5, 0.8]],
                        normals: vec![[0.0, 1.0, 0.0]; 3],
                        tangents: Vec::new(),
                        uvs: vec![[0.0; 2]; 3],
                        indices: vec![0, 1, 2],
                        material: 0,
                    }],
                },
            );
            let snapshot = ModelSnapshot {
                standalone: false,
                heraldry: HashMap::new(),
                domains: Vec::new(),
                include_moving: false,
                blocks: vec![0; 4096],
                models: vec![ModelCell {
                    cell: [15, 2, 15],
                    material,
                    state: Default::default(),
                    peer_wood: Default::default(),
                    neighbors: if material == 148 {
                        [None, None, Some(4), None, None, None]
                    } else {
                        [None; 6]
                    },
                    neighbor_facing: [0; 6],
                }],
                border_models: Vec::new(),
                border_glass: HashSet::new(),
                water_contacts: HashMap::new(),
                chimney_infill: BTreeMap::new(),
                pipe_surround: BTreeMap::new(),
                funnel_supports: HashSet::new(),
            };
            let batches = compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                SectionPos::new(-1, 0, -1),
                &RenderOptions {
                    water_level: None,
                    detail_level: super::super::DetailLevel::Near,
                    terrain: None,
                    exposure: None,
                    tints: Default::default(),
                    conditions: [(
                        [15, 2, 15],
                        terra_world::environment::SurfaceCondition {
                            snow: 1.0,
                            ..Default::default()
                        },
                    )]
                    .into(),
                },
            )
            .unwrap();
            assert!(!batches.is_empty(), "model {material}");
            for batch in batches {
                assert_eq!(batch.surface_water.len(), batch.positions.len());
                assert!(
                    batch
                        .surface_water
                        .iter()
                        .all(|payload| payload[0] == 2048.0),
                    "model {material}"
                );
            }
        }
    }

    #[test]
    fn mixed_slab_mining_isolates_only_the_aimed_material_and_keeps_every_remaining_plane_static() {
        use super::*;
        use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
        let mut templates = TemplateSet::default();
        for (material, slot) in [(82, 0), (85, 1)] {
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Boxes,
                    bevel_width: 0.0,
                    faces: crate::blocks::cube_faces(0.0, [slot; 6], None),
                    face_materials: [slot; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: crate::blocks::cube::box_model(
                        &[terra_core::construction::slab_bounds(0)],
                        0.0,
                        [slot; 6],
                    ),
                },
            );
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let cell = [15, 1, 0];
        for facing in [0, 4, 5, 6, 7, 8] {
            let merged = terra_core::construction::merged_slab(82, facing, 85).unwrap();
            let halves = terra_core::construction::slab_halves(82, merged).unwrap();
            let snapshot = ModelSnapshot {
                standalone: false,
                heraldry: HashMap::new(),
                domains: Vec::new(),
                include_moving: false,
                blocks: vec![0; 4096],
                models: vec![ModelCell {
                    cell: cell.map(|v| v as i32),
                    material: 82,
                    state: terra_world::edits::BlockObjectState {
                        facing: merged,
                        ..Default::default()
                    },
                    peer_wood: Default::default(),
                    neighbors: [None; 6],
                    neighbor_facing: [0; 6],
                }],
                border_models: Vec::new(),
                border_glass: HashSet::new(),
                water_contacts: HashMap::new(),
                chimney_infill: BTreeMap::new(),
                pipe_surround: BTreeMap::new(),
                funnel_supports: HashSet::new(),
            };
            for selected in 0..2 {
                let half = terra_world::slabs::HalfRemoval {
                    selected: selected as u8,
                    removed: halves[selected].0,
                    removed_facing: halves[selected].1,
                    remaining: halves[1 - selected].0,
                    remaining_facing: halves[1 - selected].1,
                };
                let (remaining, moving) = isolate_model(
                    &BlockMesh::default(),
                    &snapshot,
                    &templates,
                    SectionPos::new(-1, 0, 1),
                    &options,
                    cell,
                    Some(half),
                )
                .unwrap();
                for (batches, material, facing) in [
                    (&remaining, half.remaining, half.remaining_facing),
                    (&moving, half.removed, half.removed_facing),
                ] {
                    assert!(!batches.is_empty());
                    let bounds = terra_core::construction::slab_bounds(facing);
                    assert!(
                        batches
                            .iter()
                            .all(|batch| batch.material == usize::from(material == 85))
                    );
                    let (low, high) = batches.iter().flat_map(|batch| &batch.positions).fold(
                        ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
                        |(low, high), point| {
                            (
                                std::array::from_fn(|axis| low[axis].min(f64::from(point[axis]))),
                                std::array::from_fn(|axis| high[axis].max(f64::from(point[axis]))),
                            )
                        },
                    );
                    for axis in 0..3 {
                        assert!(
                            (low[axis] - f64::from(cell[axis]) - bounds.minimum[axis]).abs() < 1e-6
                        );
                        assert!(
                            (high[axis] - f64::from(cell[axis]) - bounds.maximum[axis]).abs()
                                < 1e-6
                        );
                    }
                    assert!(batches.iter().all(|batch| {
                        batch
                            .indices
                            .iter()
                            .all(|&index| (index as usize) < batch.positions.len())
                    }));
                }
            }
        }
    }

    #[test]
    fn adaptive_chimneys_align_every_authored_port_with_the_simulation() {
        use bevy::math::Vec3;
        let mut angled = 0;
        for facing in (8..23).chain(40..55) {
            let Some(ports) = terra_world::objects::chimney_ports(60, facing) else {
                assert!(facing >= 40);
                continue;
            };
            let (template, rotation) = super::chimney_template_pose(facing).unwrap();
            if facing >= 40 {
                assert_eq!(template, 63);
                angled += 1;
            }
            let authored = if template == 60 {
                [-Vec3::Y, Vec3::Y]
            } else {
                [-Vec3::Y, Vec3::Z]
            };
            for (port, expected) in authored.into_iter().zip(ports) {
                assert!(
                    (rotation * port - Vec3::from_array(expected.map(|v| v as f32))).length()
                        < 1.0e-5,
                    "pose {facing}"
                );
            }
            assert!((rotation.length() - 1.0).abs() < 1.0e-5);
        }
        assert_eq!(angled, 12);
    }

    use super::*;

    #[test]
    fn tall_furnace_is_not_stretched_and_secondary_section_does_not_duplicate_it() {
        use crate::blocks::{BlockTemplate, PrimitiveTemplate, definitions::BlockRenderKind};
        use terra_world::edits::{BlockObjectState, EditJournal};
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            59,
            BlockTemplate {
                surface: None,
                model: vec![PrimitiveTemplate {
                    tangents: Vec::new(),
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 2.0, 0.0]],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    uvs: vec![[0.0; 2]; 3],
                    indices: vec![0, 1, 2],
                    material: 0,
                }],
                render_kind: BlockRenderKind::CustomMesh,
                face_materials: [0; 6],
                side_cap: None,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                details: Vec::new(),
            },
        );
        let origin = [2, 15, 2];
        let upper = [2, 16, 2];
        let mut journal = EditJournal::default();
        for (position, peer, secondary) in [(origin, upper, false), (upper, origin, true)] {
            journal.record(position, 59).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        for (position, primary) in [(origin, true), (upper, false)] {
            let (section, local) = terra_voxel::global_to_section(position);
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block(local, 59).unwrap();
            let snapshot = capture(&data, &templates, &journal, |_| Some(0)).unwrap();
            let batches = compose(
                &BlockMesh::default(),
                &snapshot,
                &templates,
                section,
                &options,
            )
            .unwrap();
            if primary {
                assert_eq!(batches.len(), 1);
                assert_eq!(
                    batches[0].positions,
                    vec![[2.0, 15.0, 2.0], [3.0, 15.0, 2.0], [3.0, 17.0, 2.0]]
                );
            } else {
                assert!(
                    batches.is_empty(),
                    "Upper-only loaded section must not draw another furnace"
                );
            }
        }
    }

    #[test]
    fn model_support_is_restored_once_even_when_voxel_mesher_already_exposes_it() {
        use crate::blocks::{BlockTemplate, cube_faces, definitions::BlockRenderKind};
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            2,
            BlockTemplate {
                surface: None,
                model: Vec::new(),
                render_kind: BlockRenderKind::Cube,
                face_materials: [0; 6],
                side_cap: None,
                bevel_width: 0.0,
                faces: cube_faces(0.0, [0; 6], None),
                details: Vec::new(),
            },
        );
        let mut snapshot = ModelSnapshot {
            standalone: false,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: vec![0; 4096],
            models: Vec::new(),
            border_models: vec![[6, 6, 6]],
            border_glass: HashSet::new(),
            water_contacts: HashMap::new(),
            chimney_infill: BTreeMap::new(),
            pipe_surround: BTreeMap::new(),
            funnel_supports: HashSet::new(),
        };
        snapshot.blocks[6 + 16 * (5 + 16 * 6)] = 2;
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let missing = BlockMesh::default();
        let restored = compose(
            &missing,
            &snapshot,
            &templates,
            SectionPos::default(),
            &options,
        )
        .unwrap();
        assert_eq!(
            restored
                .iter()
                .map(|batch| batch.indices.len())
                .sum::<usize>(),
            6
        );
        let mut existing = BlockMesh::default();
        existing.faces.push(BlockFace {
            minimum: [6, 5, 6],
            width: 1,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 2,
            occupied_neighbor: false,
            edge_mask: 15,
        });
        let reused = compose(
            &existing,
            &snapshot,
            &templates,
            SectionPos::default(),
            &options,
        )
        .unwrap();
        assert_eq!(reused, restored);
    }

    #[test]
    fn restored_glass_face_keeps_connections_to_local_and_neighbor_sections() {
        let mut snapshot = ModelSnapshot {
            standalone: false,
            heraldry: HashMap::new(),
            domains: Vec::new(),
            include_moving: false,
            blocks: vec![0; 16 * 16 * 16],
            models: Vec::new(),
            border_models: Vec::new(),
            border_glass: [[16, 8, 8]].into(),
            water_contacts: HashMap::new(),
            chimney_infill: BTreeMap::new(),
            pipe_surround: BTreeMap::new(),
            funnel_supports: HashSet::new(),
        };
        for [x, y, z] in [[15, 8, 8], [14, 8, 8], [15, 7, 8]] {
            snapshot.blocks[x + 16 * (y + 16 * z)] = MaterialId::Glass as u16;
        }
        assert_eq!(glass_edges(&snapshot, [15, 8, 8], 0, 1), 8);
        snapshot.border_glass.clear();
        assert_eq!(glass_edges(&snapshot, [15, 8, 8], 0, 1), 10);
    }
}
