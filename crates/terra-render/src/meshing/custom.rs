//! Static scene models occupy editable cells but do not hide neighboring surfaces.
use super::builder::{MeshBatch, RenderOptions, compose_section_with_segments};
use crate::blocks::TemplateSet;
use std::collections::{BTreeMap, HashSet};
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

#[derive(Clone)]
pub(crate) struct ModelSnapshot {
    blocks: Vec<u16>,
    models: Vec<ModelCell>,
    border_models: Vec<[i32; 3]>,
    border_glass: HashSet<[i32; 3]>,
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
) -> Result<(Vec<MeshBatch>, Vec<MeshBatch>), String> {
    let cell = cell.map(|value| value as i32);
    let model = snapshot
        .models
        .iter()
        .find(|model| model.cell == cell && !model.state.secondary)
        .copied()
        .ok_or("Missing primary mining model")?;
    let mut remaining = snapshot.clone();
    remaining.models.retain(|entry| entry.cell != cell);
    // Its supporting terrain must stay exposed after removing the scene model.
    remaining.border_models.push(cell);
    let section_geometry = compose(geometry, &remaining, templates, section, options)?;
    let isolated = ModelSnapshot {
        blocks: vec![0; 16 * 16 * 16],
        models: vec![model],
        border_models: Vec::new(),
        border_glass: HashSet::new(),
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
    if models.is_empty() && border_models.is_empty() {
        None
    } else {
        Some(ModelSnapshot {
            blocks: section.blocks().to_vec(),
            models,
            border_models,
            border_glass,
        })
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
                if material == 0
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
            )?;
        }
        // Hinged joinery uses shared authored meshes so moving a leaf never
        // rebuilds or duplicates its fixed frame in a terrain batch.
        if model.state.secondary || (65..=70).contains(&model.material) {
            continue;
        }
        append_chimney_infill(&mut batches, model, templates)?;
        let width = if matches!(model.material, 28 | 29) && model.state.peer.is_some() {
            2.0
        } else {
            1.0
        };
        let adaptive_chimney = model.material == 60 && model.state.facing >= 8;
        let construction = terra_core::construction::is_slab(model.material)
            || terra_core::construction::is_stair(model.material)
            || model.material == 98;
        let (template_id, rotation) = if construction {
            (model.material, bevy::math::Quat::IDENTITY)
        } else if adaptive_chimney {
            chimney_template_pose(model.state.facing).ok_or("Invalid adaptive chimney pose")?
        } else {
            (
                model.material,
                bevy::math::Quat::from_rotation_y(
                    f32::from(model.state.facing) * std::f32::consts::FRAC_PI_2,
                ),
            )
        };
        let template = templates
            .blocks
            .get(&template_id)
            .ok_or("Missing static model template")?;
        let profile;
        let primitives = if construction {
            let neighbors = std::array::from_fn(|side| {
                model.neighbors[side].map(|id| (id, model.neighbor_facing[side]))
            });
            profile = if model.material == 98 {
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
                        crate::blocks::cube::box_model(
                            &[bounds],
                            part.bevel_width,
                            part.face_materials,
                        )
                    })
                    .collect()
            } else {
                crate::blocks::cube::box_model(
                    &terra_core::construction::placed_boxes(
                        model.material,
                        model.state.facing,
                        neighbors,
                    ),
                    template.bevel_width,
                    template.face_materials,
                )
            };
            &profile
        } else {
            &template.model
        };
        for (primitive_index, primitive) in primitives.iter().enumerate() {
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
            if model.material == 40
                || templates.wood_materials.contains(&primitive.material)
                || templates.blanket_materials.contains(&primitive.material)
                || !batch.colors.is_empty()
            {
                batch.colors.resize(batch.positions.len(), [1.0; 4]);
                let tint = if templates.blanket_materials.contains(&primitive.material) {
                    terra_core::beds::bed_color(model.material, model.state.facing).tint()
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
                    if adaptive_chimney {
                        point.y -= 0.5;
                    }
                    (rotation * point
                        + bevy::math::Vec3::new(0.5, if adaptive_chimney { 0.5 } else { 0.0 }, 0.5)
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
        }
    }
    Ok(batches.into_values().collect())
}

fn append_chimney_infill(
    batches: &mut BTreeMap<usize, MeshBatch>,
    model: &ModelCell,
    templates: &TemplateSet,
) -> Result<(), String> {
    use bevy::math::Vec3;
    let pieces =
        terra_world::objects::chimney_infill(model.material, model.state.facing, |direction| {
            let axis = direction.iter().position(|v| *v != 0)?;
            model.neighbors[axis * 2 + usize::from(direction[axis] > 0)]
        });
    for (bounds, material) in pieces {
        let Some(template) = templates
            .blocks
            .get(&material)
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
}
impl SplitVertex {
    fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            position: self.position.lerp(other.position, t),
            normal: self.normal.lerp(other.normal, t).normalize_or_zero(),
            uv: self.uv.lerp(other.uv, t),
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
                    batch
                        .normals
                        .push((rotation * normal).normalize_or_zero().to_array());
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
        append_chimney_infill(&mut batches, &model, &templates).unwrap();
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
    fn mixed_double_chest_splits_surface_colors_without_gaps_or_material_batches() {
        use terra_core::WoodSpecies;
        let primitive = PrimitiveTemplate {
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
                    conditions: Default::default(),
                },
                [0; 3],
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
            blocks: vec![0; 4096],
            models: Vec::new(),
            border_models: vec![[6, 6, 6]],
            border_glass: HashSet::new(),
        };
        snapshot.blocks[6 + 16 * (5 + 16 * 6)] = 2;
        let options = RenderOptions {
            water_level: None,
            detail_level: super::super::DetailLevel::Near,
            terrain: None,
            exposure: None,
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
            blocks: vec![0; 16 * 16 * 16],
            models: Vec::new(),
            border_models: Vec::new(),
            border_glass: [[16, 8, 8]].into(),
        };
        for [x, y, z] in [[15, 8, 8], [14, 8, 8], [15, 7, 8]] {
            snapshot.blocks[x + 16 * (y + 16 * z)] = MaterialId::Glass as u16;
        }
        assert_eq!(glass_edges(&snapshot, [15, 8, 8], 0, 1), 8);
        snapshot.border_glass.clear();
        assert_eq!(glass_edges(&snapshot, [15, 8, 8], 0, 1), 10);
    }
}
