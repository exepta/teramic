//! Frozen vehicle openings use the same authored leaves and derived joinery as world openings.
use crate::{
    blocks::{BlockAssets, PrimitiveTemplate, TemplateSet},
    meshing::builder::MeshBatch,
};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_core::trains::{TrainBlockView, TrainPart};
use terra_world::{edits::BlockObjectState, objects};

pub(super) type Palettes = BTreeMap<u16, [usize; 5]>;

/// Snapshot only immutable material identities before geometry leaves the main thread.
/// The classification deliberately matches `openings::windows::parts`.
pub(super) fn palettes(assets: &BlockAssets, materials: &Assets<StandardMaterial>) -> Palettes {
    let Some(templates) = assets.templates.as_ref() else {
        return Palettes::new();
    };
    templates
        .blocks
        .iter()
        .filter_map(|(&id, block)| {
            if !objects::is_opening(id) {
                return None;
            }
            let material = |primitive: &PrimitiveTemplate| {
                assets
                    .materials
                    .get(primitive.material)
                    .and_then(|handle| materials.get(handle))
            };
            let wood = block
                .model
                .iter()
                .filter(|primitive| {
                    !templates.hinge_materials.contains(&primitive.material)
                        && material(primitive).is_some_and(|material| {
                            material.alpha_mode != AlphaMode::Blend && material.metallic < 0.25
                        })
                })
                .map(|primitive| primitive.material)
                .collect::<Vec<_>>();
            let base = *wood.first()?;
            let metal = block
                .model
                .iter()
                .find(|primitive| {
                    material(primitive).is_some_and(|material| material.metallic >= 0.25)
                })
                .map_or(base, |primitive| primitive.material);
            let glass = block
                .model
                .iter()
                .find(|primitive| {
                    material(primitive)
                        .is_some_and(|material| material.alpha_mode == AlphaMode::Blend)
                })
                .map_or(base, |primitive| primitive.material);
            Some((
                id,
                [
                    base,
                    *wood.get(1).unwrap_or(&base),
                    *wood.get(2).unwrap_or(&base),
                    metal,
                    glass,
                ],
            ))
        })
        .collect()
}

struct Cell<'a> {
    index: u16,
    part: &'a TrainPart,
    presentation: Option<&'a TrainBlockView>,
    state: BlockObjectState,
}

/// Returned batches are relative to the indexed owner's cell. Empty entries
/// consume the other cells of a grouped opening, preventing duplicate leaves.
pub(super) fn frozen_openings(
    parts: &[TrainPart],
    blocks: &BTreeMap<u16, TrainBlockView>,
    templates: &TemplateSet,
    palettes: &Palettes,
) -> Result<BTreeMap<u16, Vec<MeshBatch>>, String> {
    let mut cells = BTreeMap::new();
    for (index, part) in parts.iter().enumerate() {
        if !objects::is_opening(part.material) {
            continue;
        }
        let presentation = blocks.get(&(index as u16));
        let position = part.offset.map(i64::from);
        let state = BlockObjectState {
            facing: part.facing,
            wood: part.wood,
            peer: presentation
                .and_then(|view| view.peer)
                .map(|peer| std::array::from_fn(|axis| position[axis] + i64::from(peer[axis]))),
            secondary: presentation.is_some_and(|view| view.secondary),
            ..default()
        };
        if part.facing >= 8
            || cells
                .insert(
                    position,
                    Cell {
                        index: index as u16,
                        part,
                        presentation,
                        state,
                    },
                )
                .is_some()
        {
            return Err("Invalid frozen opening pose".into());
        }
    }
    let lookup = |position| {
        cells
            .get(&position)
            .map(|cell| (cell.part.material, cell.state))
    };
    let mut result = BTreeMap::new();
    for (&position, cell) in &cells {
        if result.contains_key(&cell.index) {
            continue;
        }
        let (group, opened) = if objects::is_window(cell.part.material) {
            let group = objects::windows::group(
                position,
                cell.part.material,
                cell.part.facing,
                |position| lookup(position).map(|(material, state)| (material, state.facing)),
            );
            (group, [group.facing >= 4; 2])
        } else {
            let door = objects::doors::group(position, cell.part.material, cell.state, lookup)
                .ok_or("Frozen door is missing its reciprocal half")?;
            (door.geometry(), door.opened)
        };
        let owner = cells
            .get(&group.anchor)
            .ok_or("Missing frozen opening anchor")?;
        for position in group.cells() {
            let member = cells
                .get(&position)
                .ok_or("Missing frozen opening member")?;
            result.insert(member.index, Vec::new());
        }
        let grouped = group.width != 1 || objects::is_window(group.material) && group.height != 1;
        let batches = if grouped {
            grouped_geometry(group, opened, owner, templates, palettes)?
        } else {
            authored_geometry(owner, templates, palettes)?
        };
        result.insert(owner.index, batches);
    }
    Ok(result)
}

fn tint(
    cell: &Cell<'_>,
    material: usize,
    templates: &TemplateSet,
    palettes: &Palettes,
) -> [f32; 4] {
    let mut color = if templates.wood_materials.contains(&material) {
        cell.part.wood.tint()
    } else {
        [1.0; 4]
    };
    if let Some(tint) = cell.presentation.and_then(|view| view.tint)
        && palettes
            .get(&cell.part.material)
            .is_none_or(|palette| material != palette[3] && material != palette[4])
    {
        let tint = crate::dye_tint::rgb(tint);
        for axis in 0..3 {
            color[axis] *= tint[axis];
        }
    }
    color
}

fn pose(group: objects::windows::Group, leaf: Option<u8>, opened: [bool; 2], point: Vec3) -> Vec3 {
    let mut point = point;
    if let Some(leaf) = leaf {
        let hinge = Vec3::from_array(group.hinge(leaf).map(|value| value as f32));
        if opened[usize::from(leaf)] {
            point = hinge
                + Quat::from_rotation_y(objects::windows::Group::leaf_angle(leaf) as f32)
                    * (point - hinge);
        }
    }
    let center = Vec3::new(0.5, 0.0, 0.5);
    Vec3::from_array(group.anchor.map(|value| value as f32))
        + center
        + Quat::from_rotation_y(f32::from(group.facing % 4) * std::f32::consts::FRAC_PI_2)
            * (point - center)
}

fn rotation(group: objects::windows::Group, leaf: Option<u8>, opened: [bool; 2]) -> Quat {
    Quat::from_rotation_y(f32::from(group.facing % 4) * std::f32::consts::FRAC_PI_2)
        * leaf
            .filter(|leaf| opened[usize::from(*leaf)])
            .map_or(Quat::IDENTITY, |leaf| {
                Quat::from_rotation_y(objects::windows::Group::leaf_angle(leaf) as f32)
            })
}

fn authored_geometry(
    cell: &Cell<'_>,
    templates: &TemplateSet,
    palettes: &Palettes,
) -> Result<Vec<MeshBatch>, String> {
    let template = templates
        .blocks
        .get(&cell.part.material)
        .ok_or("Missing frozen opening model")?;
    let mut batches = BTreeMap::<usize, MeshBatch>::new();
    let group = objects::windows::Group {
        anchor: [0; 3],
        material: cell.part.material,
        facing: cell.part.facing,
        width: 1,
        height: if objects::is_door(cell.part.material) {
            2
        } else {
            1
        },
    };
    let opened = [cell.part.facing >= 4; 2];
    for primitive in &template.model {
        if primitive.positions.len() != primitive.normals.len()
            || primitive.positions.len() != primitive.uvs.len()
            || !primitive.tangents.is_empty()
                && primitive.tangents.len() != primitive.positions.len()
        {
            return Err("Invalid frozen opening attributes".into());
        }
        let leaf = templates
            .hinge_materials
            .contains(&primitive.material)
            .then_some(0);
        let rotate = rotation(group, leaf, opened);
        let batch = batches
            .entry(primitive.material)
            .or_insert_with(|| MeshBatch {
                material: primitive.material,
                ..default()
            });
        let base =
            u32::try_from(batch.positions.len()).map_err(|_| "Frozen opening index overflow")?;
        batch.positions.extend(
            primitive
                .positions
                .iter()
                .map(|point| pose(group, leaf, opened, Vec3::from_array(*point)).to_array()),
        );
        batch.normals.extend(
            primitive
                .normals
                .iter()
                .map(|normal| (rotate * Vec3::from_array(*normal)).to_array()),
        );
        batch.uvs.extend_from_slice(&primitive.uvs);
        batch.append_tangents(
            base as usize,
            primitive.tangents.iter().map(|tangent| {
                let vector = rotate * Vec3::from_array([tangent[0], tangent[1], tangent[2]]);
                [vector.x, vector.y, vector.z, tangent[3]]
            }),
        );
        batch.colors.extend(std::iter::repeat_n(
            tint(cell, primitive.material, templates, palettes),
            primitive.positions.len(),
        ));
        for &index in &primitive.indices {
            if index as usize >= primitive.positions.len() {
                return Err("Invalid frozen opening index".into());
            }
            batch.indices.push(
                base.checked_add(index)
                    .ok_or("Frozen opening index overflow")?,
            );
        }
    }
    Ok(batches.into_values().collect())
}

fn grouped_geometry(
    group: objects::windows::Group,
    opened: [bool; 2],
    cell: &Cell<'_>,
    templates: &TemplateSet,
    palettes: &Palettes,
) -> Result<Vec<MeshBatch>, String> {
    let palette = palettes
        .get(&group.material)
        .ok_or("Missing frozen opening material palette")?;
    let pieces = if objects::is_door(group.material) {
        objects::doors::pieces()
    } else {
        objects::windows::pieces(group.width, group.height)
    };
    let mut batches = BTreeMap::<usize, MeshBatch>::new();
    let origin = Vec3::from_array(group.anchor.map(|value| value as f32));
    for piece in pieces {
        let material = *palette
            .get(usize::from(piece.surface))
            .ok_or("Invalid frozen opening surface")?;
        let color = tint(cell, material, templates, palettes);
        let batch = batches.entry(material).or_insert_with(|| MeshBatch {
            material,
            ..default()
        });
        let rotate = rotation(group, piece.leaf, opened);
        for axis in 0..3 {
            for sign in [-1.0_f32, 1.0] {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let base = u32::try_from(batch.positions.len())
                    .map_err(|_| "Frozen opening index overflow")?;
                let mut normal = [0.0; 3];
                normal[axis] = sign;
                let normal = rotate * Vec3::from_array(normal);
                let mut tangent = [0.0; 3];
                tangent[u] = 1.0;
                let tangent = rotate * Vec3::from_array(tangent);
                for [a, b] in [[0, 0], [1, 0], [1, 1], [0, 1]] {
                    let mut point = piece.bounds.minimum.map(|value| value as f32);
                    point[axis] = if sign > 0.0 {
                        piece.bounds.maximum[axis] as f32
                    } else {
                        piece.bounds.minimum[axis] as f32
                    };
                    point[u] = if a == 1 {
                        piece.bounds.maximum[u] as f32
                    } else {
                        piece.bounds.minimum[u] as f32
                    };
                    point[v] = if b == 1 {
                        piece.bounds.maximum[v] as f32
                    } else {
                        piece.bounds.minimum[v] as f32
                    };
                    batch.positions.push(
                        (pose(group, piece.leaf, opened, Vec3::from_array(point)) - origin)
                            .to_array(),
                    );
                    batch.normals.push(normal.to_array());
                    batch
                        .tangents
                        .push(crate::blocks::tangent_frame(tangent, normal, sign));
                    batch.uvs.push([a as f32, b as f32]);
                    batch.colors.push(color);
                }
                for index in if sign > 0.0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                } {
                    batch.indices.push(base + index);
                }
            }
        }
    }
    Ok(batches.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
    use std::sync::Arc;
    use terra_core::{WoodSpecies, block_shapes::ShapeBox};

    fn cube(material: usize, bounds: ShapeBox) -> PrimitiveTemplate {
        let mut primitive = PrimitiveTemplate {
            positions: Vec::new(),
            normals: Vec::new(),
            tangents: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            material,
        };
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let base = primitive.positions.len() as u32;
                for [a, b] in [[0, 0], [1, 0], [1, 1], [0, 1]] {
                    let mut point = bounds.minimum.map(|value| value as f32);
                    point[axis] = if sign > 0.0 {
                        bounds.maximum[axis] as f32
                    } else {
                        bounds.minimum[axis] as f32
                    };
                    point[u] = if a == 1 {
                        bounds.maximum[u] as f32
                    } else {
                        bounds.minimum[u] as f32
                    };
                    point[v] = if b == 1 {
                        bounds.maximum[v] as f32
                    } else {
                        bounds.minimum[v] as f32
                    };
                    primitive.positions.push(point);
                    let mut normal = [0.0; 3];
                    normal[axis] = sign;
                    primitive.normals.push(normal);
                    primitive.uvs.push([a as f32, b as f32]);
                }
                for index in if sign > 0.0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                } {
                    primitive.indices.push(base + index);
                }
            }
        }
        primitive
    }

    fn fixture() -> (BlockAssets, Assets<StandardMaterial>, Palettes) {
        let mut templates = TemplateSet::default();
        for id in [65, 66, 67, 68, 69, 70, 139, 140] {
            let height = if objects::is_door(id) { 2.0 } else { 1.0 };
            let bounds = [
                ([0.04, 0.0, 0.34], [0.15, height, 0.66]),
                ([0.03, 0.2, 0.325], [0.16, 0.3, 0.675]),
                ([0.025, height - 0.10, 0.32], [0.975, height - 0.025, 0.68]),
                ([0.08, 0.2, 0.32], [0.12, 0.3, 0.33]),
                ([0.25, 0.14, 0.487], [0.85, height - 0.15, 0.513]),
                ([0.15, 0.085, 0.455], [0.205, height - 0.135, 0.545]),
            ];
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: bounds
                        .into_iter()
                        .enumerate()
                        .map(|(slot, (minimum, maximum))| {
                            cube(
                                slot,
                                ShapeBox {
                                    minimum,
                                    maximum,
                                    hinged: slot >= 4,
                                },
                            )
                        })
                        .collect(),
                },
            );
        }
        templates.hinge_materials.extend([4, 5]);
        let mut materials = Assets::default();
        let mut assets = BlockAssets::default();
        assets.materials = (0..6)
            .map(|slot| {
                materials.add(StandardMaterial {
                    metallic: if slot == 3 { 0.5 } else { 0.0 },
                    alpha_mode: if slot == 4 {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    ..default()
                })
            })
            .collect();
        assets.templates = Some(Arc::new(templates));
        let palette = palettes(&assets, &materials);
        (assets, materials, palette)
    }

    fn parts(
        group: objects::windows::Group,
        opened: [bool; 2],
    ) -> (Vec<TrainPart>, BTreeMap<u16, TrainBlockView>) {
        let mut parts = Vec::new();
        let mut blocks = BTreeMap::new();
        for position in group.cells() {
            let upper = position[1] != group.anchor[1];
            let right = group.right();
            let column = (position[0] - group.anchor[0]) * right[0]
                + (position[2] - group.anchor[2]) * right[2];
            let facing = group.facing % 4 + u8::from(opened[column as usize]) * 4;
            if objects::is_door(group.material) {
                blocks.insert(
                    parts.len() as u16,
                    TrainBlockView {
                        boxes: Vec::new(),
                        tint: None,
                        heraldry: None,
                        peer: Some([0, if upper { -1 } else { 1 }, 0]),
                        secondary: upper,
                    },
                );
            }
            parts.push(TrainPart {
                offset: position.map(|value| value as i16),
                material: group.material,
                facing,
                wood: WoodSpecies::Birch,
            });
        }
        (parts, blocks)
    }

    fn bounds(points: impl Iterator<Item = [f32; 3]>) -> [[f64; 3]; 2] {
        points.fold(
            [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]],
            |mut bounds, point| {
                for axis in 0..3 {
                    bounds[0][axis] = bounds[0][axis].min(f64::from(point[axis]));
                    bounds[1][axis] = bounds[1][axis].max(f64::from(point[axis]));
                }
                bounds
            },
        )
    }
    fn equal_bounds(actual: [[f64; 3]; 2], expected: [[f64; 3]; 2]) {
        for edge in 0..2 {
            for axis in 0..3 {
                assert!(
                    (actual[edge][axis] - expected[edge][axis]).abs() < 0.00001,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }
    fn valid(batches: &[MeshBatch]) {
        for batch in batches {
            assert!(
                batch
                    .positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert_eq!(batch.positions.len(), batch.normals.len());
            assert_eq!(batch.positions.len(), batch.uvs.len());
            assert_eq!(batch.positions.len(), batch.colors.len());
            assert!(batch.tangents.is_empty() || batch.tangents.len() == batch.positions.len());
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&index| (index as usize) < batch.positions.len())
            );
            assert!(
                batch
                    .normals
                    .iter()
                    .all(
                        |normal| (Vec3::from_array(*normal).length_squared() - 1.0).abs() < 0.00001
                    )
            );
        }
    }

    #[test]
    fn grouped_windows_and_paired_doors_match_world_joinery_in_every_yaw_and_leaf_state() {
        let (assets, _, palettes) = fixture();
        let templates = assets.templates.as_deref().unwrap();
        assert!(palettes.values().all(|palette| *palette == [0, 1, 2, 3, 4]));
        for material in [65, 66, 67, 68, 69, 70, 139, 140] {
            for facing in 0..4 {
                for opened in [[false, false], [true, true], [false, true], [true, false]] {
                    if objects::is_window(material) && opened[0] != opened[1] {
                        continue;
                    }
                    let group = objects::windows::Group {
                        anchor: [-2, 3, -1],
                        material,
                        facing,
                        width: 2,
                        height: 2,
                    };
                    let (parts, blocks) = parts(group, opened);
                    let actual = frozen_openings(&parts, &blocks, templates, &palettes).unwrap();
                    assert_eq!(actual.len(), 4);
                    assert_eq!(
                        actual
                            .values()
                            .filter(|batches| !batches.is_empty())
                            .count(),
                        1,
                        "one joined model, no cell duplicates"
                    );
                    let batches = actual.values().find(|batches| !batches.is_empty()).unwrap();
                    valid(batches);
                    let pieces = if objects::is_door(material) {
                        objects::doors::pieces()
                    } else {
                        objects::windows::pieces(2, 2)
                    };
                    let mut expected = BTreeMap::<usize, [[f64; 3]; 2]>::new();
                    for piece in pieces {
                        let open = piece.leaf.is_some_and(|leaf| opened[usize::from(leaf)]);
                        let bounds =
                            objects::windows::world_box(piece, group, f64::from(u8::from(open)));
                        let target = expected
                            .entry(palettes[&material][usize::from(piece.surface)])
                            .or_insert([[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]]);
                        for (value, minimum) in target[0].iter_mut().zip(bounds.minimum) {
                            *value = value.min(minimum);
                        }
                        for (value, maximum) in target[1].iter_mut().zip(bounds.maximum) {
                            *value = value.max(maximum);
                        }
                    }
                    for batch in batches {
                        let actual = bounds(batch.positions.iter().map(|point| {
                            std::array::from_fn(|axis| point[axis] + group.anchor[axis] as f32)
                        }));
                        equal_bounds(actual, expected[&batch.material]);
                    }
                    if objects::is_window(material) {
                        assert!(
                            batches.iter().any(|batch| batch.material == 4),
                            "retain transparent glass material"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn single_authored_leaves_keep_open_pose_while_the_stationary_frame_stays_put() {
        let (assets, _, palettes) = fixture();
        let templates = assets.templates.as_deref().unwrap();
        for material in [65, 66, 67, 68, 69, 70, 139, 140] {
            for facing in 0..8 {
                let group = objects::windows::Group {
                    anchor: [-2, 3, -1],
                    material,
                    facing: facing % 4,
                    width: 1,
                    height: if objects::is_door(material) { 2 } else { 1 },
                };
                let (parts, blocks) = parts(group, [facing >= 4; 2]);
                let actual = frozen_openings(&parts, &blocks, templates, &palettes).unwrap();
                let batches = actual.values().find(|batches| !batches.is_empty()).unwrap();
                valid(batches);
                for primitive in &templates.blocks[&material].model {
                    let source = bounds(primitive.positions.iter().copied());
                    let source = ShapeBox {
                        minimum: source[0],
                        maximum: source[1],
                        hinged: templates.hinge_materials.contains(&primitive.material),
                    };
                    let expected = objects::opening_box(
                        source,
                        [0; 3],
                        BlockObjectState {
                            facing,
                            ..default()
                        },
                    );
                    let actual = bounds(
                        batches
                            .iter()
                            .find(|batch| batch.material == primitive.material)
                            .unwrap()
                            .positions
                            .iter()
                            .copied(),
                    );
                    equal_bounds(actual, [expected.minimum, expected.maximum]);
                }
            }
        }
    }

    #[test]
    fn different_window_poses_remain_separate_and_missing_door_halves_are_rejected() {
        let (assets, _, palettes) = fixture();
        let templates = assets.templates.as_deref().unwrap();
        let group = objects::windows::Group {
            anchor: [0; 3],
            material: 68,
            facing: 0,
            width: 2,
            height: 1,
        };
        let (parts, blocks) = parts(group, [false, true]);
        let actual = frozen_openings(&parts, &blocks, templates, &palettes).unwrap();
        assert_eq!(
            actual
                .values()
                .filter(|batches| !batches.is_empty())
                .count(),
            2
        );
        let broken = [TrainPart {
            offset: [0; 3],
            material: 65,
            facing: 4,
            wood: WoodSpecies::Oak,
        }];
        assert!(frozen_openings(&broken, &BTreeMap::new(), templates, &palettes).is_err());
    }

    #[test]
    fn opening_color_metadata_does_not_tint_glass_or_iron() {
        let (assets, _, palettes) = fixture();
        let templates = assets.templates.as_deref().unwrap();
        let group = objects::windows::Group {
            anchor: [0; 3],
            material: 68,
            facing: 0,
            width: 2,
            height: 1,
        };
        let (parts, mut blocks) = parts(group, [true; 2]);
        blocks.insert(
            0,
            TrainBlockView {
                boxes: Vec::new(),
                tint: Some([128, 64, 32]),
                heraldry: None,
                peer: None,
                secondary: false,
            },
        );
        let actual = frozen_openings(&parts, &blocks, templates, &palettes).unwrap();
        for batch in &actual[&0] {
            let expected = if batch.material == 3 || batch.material == 4 {
                [1.0; 4]
            } else {
                crate::dye_tint::rgb([128, 64, 32])
            };
            assert!(batch.colors.iter().all(|color| *color == expected));
        }
    }
}
