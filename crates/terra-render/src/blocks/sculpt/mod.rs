//! Every solid occupies its complete unit cell. Only the render mesh adds relief
//! on free faces. Families share four immutable variants across chunks and biomes.
mod crafted;
mod natural;
mod patch;
mod reference_gravel;
mod reference_leaves;
pub(crate) use reference_leaves::shadow_cards;

use super::definitions::{BlockDefinition, BlockLibrary, BlockRenderKind};
use super::{BlockTemplate, PrimitiveTemplate, TemplateSet, cube_faces, projected_tangent};
use bevy::math::Vec3;
use crafted::CraftedForm;
use natural::NaturalForm;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

const VARIANTS: usize = 4;

pub(crate) fn variant(position: [i64; 3], id: u16) -> usize {
    (crate::meshing::builder::variation(position, u64::from(id)) % VARIANTS as u64) as usize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Form {
    Natural(NaturalForm),
    ReferenceGravel,
    ReferenceLeaves,
    Crafted(CraftedForm),
}

fn form(def: &BlockDefinition) -> Option<Form> {
    if def.render_kind != BlockRenderKind::Cube && def.coarse_faces.is_none() {
        return None;
    }
    // These volumes have their own joined pane, fog, or liquid surface renderer.
    if def.frame_material.is_some()
        || matches!(def.id, 14 | 27 | 399 | 430 | terra_world::lava::LAVA)
    {
        return None;
    }
    // These ores already carry exposed, growing mineral geometry owned by the world.
    if def.crystal_material.is_some() {
        return Some(Form::Natural(NaturalForm::Rock));
    }
    Some(
        crafted::classify(&def.name, def.surface)
            .map(Form::Crafted)
            .unwrap_or_else(|| {
                Form::Natural(
                    natural::classify(&def.name, def.surface).unwrap_or(NaturalForm::Rock),
                )
            }),
    )
}

impl Form {
    fn wraps_u(self, side: usize) -> bool {
        matches!(side, 1 | 4)
            && matches!(
                self,
                Self::Natural(NaturalForm::ReferenceTurf | NaturalForm::ReferenceRock)
                    | Self::ReferenceGravel
                    | Self::ReferenceLeaves
            )
    }
}

#[derive(Default)]
pub(crate) struct ReferencePalette {
    pub grass_top: Option<usize>,
    pub grass_side: Option<usize>,
    pub soil: Option<usize>,
    pub rock: Option<usize>,
    pub gravel: Option<usize>,
    pub leaves: Option<usize>,
    /// Largest transparent square in each canopy texture, in normalized tile UVs.
    pub leaf_openings: HashMap<usize, [f32; 3]>,
}

pub(crate) fn register(
    library: &BlockLibrary,
    templates: &mut TemplateSet,
    palette: &ReferencePalette,
) {
    let mut families = HashMap::new();
    for def in &library.blocks {
        let Some(catalog_form) = form(def) else {
            continue;
        };
        let form = match def.id {
            1 if palette.grass_side.is_some() => Form::Natural(NaturalForm::ReferenceTurf),
            3 if palette.rock.is_some() => Form::Natural(NaturalForm::ReferenceRock),
            7 if palette.leaves.is_some() => Form::ReferenceLeaves,
            42 if palette.gravel.is_some() => Form::ReferenceGravel,
            _ => catalog_form,
        };
        let block = templates
            .cube_lods
            .get(&def.id)
            .unwrap_or(&templates.blocks[&def.id]);
        let mut materials = block.face_materials;
        let mut cap = block.side_cap;
        let reference = match def.id {
            2 => palette.soil,
            3 => palette.rock,
            7 => palette.leaves,
            42 => palette.gravel,
            _ => None,
        };
        if let Some(slot) = reference {
            materials.fill(slot);
        } else if def.id == 1
            && let (Some(top), Some(side), Some(soil)) =
                (palette.grass_top, palette.grass_side, palette.soil)
        {
            materials = [side, side, soil, top, side, side];
            cap = None;
        }
        let mut slots = [0; 8];
        slots[..6].copy_from_slice(if terra_voxel::MaterialId::FOLIAGE.contains(&def.id) {
            &block.face_materials
        } else {
            &materials
        });
        let reference_grass = def.id == 1 && palette.grass_side.is_some();
        if !reference_grass
            && templates.cube_lods.contains_key(&def.id)
            && form == Form::Natural(NaturalForm::Turf)
        {
            for side in [0, 1, 4, 5] {
                slots[side] = slots[2];
            }
        }
        // Project the same turf/soil transition across the backing and its raised
        // sods, so an individual root or an embedded pebble crosses their bevels.
        slots[6] = if reference_grass {
            materials[0]
        } else {
            cap.map_or(slots[3], |cap| cap.material)
        };
        slots[7] = match form {
            Form::Natural(NaturalForm::Turf | NaturalForm::ReferenceTurf) => palette
                .rock
                .or_else(|| templates.blocks.get(&3).map(|b| b.face_materials[3]))
                .or(Some(slots[2])),
            Form::Crafted(CraftedForm::RootStone) => {
                let wood = match def.id {
                    473 => 6,
                    492 => 405,
                    _ => 705,
                };
                templates.blocks.get(&wood).map(|b| b.face_materials[0])
            }
            Form::Crafted(CraftedForm::CrystalRock) => {
                templates.ore_materials.get(&def.id).copied().or_else(|| {
                    let accent_id = match def.id {
                        739 | 740 | 746 => 746,
                        745 | 747 => 745,
                        _ => return None,
                    };
                    templates
                        .blocks
                        .get(&accent_id)
                        .map(|b| b.face_materials[3])
                })
            }
            _ => None,
        }
        .unwrap_or(slots[3]);
        let geometry = families
            .entry(form)
            .or_insert_with(|| {
                Arc::new(Family {
                    form,
                    faces: Default::default(),
                })
            })
            .clone();
        let mut faces = cube_faces(0.0, materials, cap);
        for (side, parts) in faces.iter_mut().enumerate() {
            for part in parts {
                if templates.normal_mapped_materials.contains(&part.material)
                    || block.faces[side]
                        .iter()
                        .any(|source| !source.tangents.is_empty())
                {
                    part.tangents = part
                        .normals
                        .iter()
                        .map(|&n| projected_tangent(Vec3::from_array(n), side))
                        .collect();
                }
            }
        }
        for (side, parts) in faces.iter_mut().enumerate() {
            if form.wraps_u(side) {
                for part in parts {
                    for uv in &mut part.uvs {
                        uv[0] = -uv[0];
                    }
                    for tangent in &mut part.tangents {
                        *tangent = tangent.map(|v| -v);
                    }
                }
            }
        }
        let core = BlockTemplate {
            face_materials: materials,
            faces,
            side_cap: cap,
            bevel_width: 0.0,
            render_kind: BlockRenderKind::Cube,
            surface: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        let shape = SculptedBlock {
            geometry,
            slots,
            core: Arc::new(core),
            reference_grass,
            leaf_openings: materials.map(|slot| palette.leaf_openings.get(&slot).copied()),
        };
        if terra_voxel::MaterialId::FOLIAGE.contains(&def.id)
            && let Some(names) = &def.face_materials
        {
            // A leaf's wind material overrides sidedness and roughness.
            // Compare all remaining paint, normal-map and PBR bindings.
            let wind = names.each_ref().map(|name| {
                let mut material = def.materials[name].clone();
                material.roughness = crate::foliage::WIND_LEAF_ROUGHNESS;
                material.double_sided = true;
                (material, def.textures.get(name))
            });
            let mut slots = shape.slots;
            for side in 1..6 {
                if let Some(first) = wind[..side]
                    .iter()
                    .position(|binding| *binding == wind[side])
                {
                    slots[side] = slots[first];
                }
            }
            if slots != shape.slots {
                templates.batched_wind_leaves.insert(
                    def.id,
                    SculptedBlock {
                        slots,
                        ..shape.clone()
                    },
                );
            }
        }
        templates.sculpted_blocks.insert(def.id, shape);
    }
}

#[derive(Clone)]
pub(crate) struct SculptedBlock {
    geometry: Arc<Family>,
    pub(crate) slots: [usize; 8],
    pub(crate) core: Arc<BlockTemplate>,
    pub(crate) reference_grass: bool,
    pub(crate) leaf_openings: [Option<[f32; 3]>; 6],
}

impl SculptedBlock {
    pub(crate) fn reference_leaves(&self) -> bool {
        self.geometry.form == Form::ReferenceLeaves
    }

    pub(crate) fn wraps_u(&self, side: usize) -> bool {
        self.geometry.form.wraps_u(side)
    }

    pub(crate) fn relief(
        &self,
        side: usize,
        variant: usize,
        open: u8,
        folded_leaves: bool,
    ) -> Option<&[PrimitiveTemplate]> {
        if open & (1 << side) == 0 {
            return None;
        }
        if !folded_leaves && matches!(self.geometry.form, Form::ReferenceLeaves) {
            // Geometry depends only on the face and variant; material slots resolve at upload.
            static DISTANT: [[OnceLock<Vec<PrimitiveTemplate>>; 6]; VARIANTS] =
                [const { [const { OnceLock::new() }; 6] }; VARIANTS];
            return Some(DISTANT[variant][side].get_or_init(|| {
                self.finish_face(side, reference_leaves::distant_face(side, variant))
                    .iter()
                    .map(reference_leaves::distant_facets)
                    .collect()
            }));
        }
        Some(self.face(side, variant))
    }

    pub(crate) fn face(&self, side: usize, variant: usize) -> &[PrimitiveTemplate] {
        self.geometry.faces[variant][side].get_or_init(|| {
            let pieces = match self.geometry.form {
                Form::Natural(f) => natural::face(f, side, variant),
                Form::ReferenceGravel => reference_gravel::face(side, variant),
                Form::ReferenceLeaves => reference_leaves::face(side, variant),
                Form::Crafted(f) => crafted::face(f, side, variant),
            };
            self.finish_face(side, pieces)
        })
    }

    fn finish_face(
        &self,
        side: usize,
        mut pieces: Vec<PrimitiveTemplate>,
    ) -> Vec<PrimitiveTemplate> {
        for piece in &mut pieces {
            // Walk the artwork around the four vertical faces in one direction.
            // Reflect the measured fracture lines with their paint, including
            // handedness and winding, so adjacent walls never mirror one seam.
            if self.wraps_u(side) {
                let u = if side / 2 == 0 { 2 } else { 0 };
                for p in &mut piece.positions {
                    p[u] = 1.0 - p[u];
                }
                for n in &mut piece.normals {
                    n[u] = -n[u];
                }
                for tangent in &mut piece.tangents {
                    tangent[u] = -tangent[u];
                    tangent[3] = -tangent[3];
                }
                for triangle in piece.indices.chunks_exact_mut(3) {
                    triangle.swap(1, 2);
                }
            }
            super::surface::index_vertices(piece);
        }
        pieces
    }
}

struct Family {
    form: Form,
    faces: [[OnceLock<Vec<PrimitiveTemplate>>; 6]; VARIANTS],
}
