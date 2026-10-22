//! Shared construction profiles; the renderer and collision read the same JSON boxes.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const FIRST: u16 = 74;
pub const LAST: u16 = 97;
pub fn is_shape(material: u16) -> bool {
    (FIRST..=LAST).contains(&material)
}
pub fn is_fence(material: u16) -> bool {
    (90..=97).contains(&material)
}
pub fn is_slab(material: u16) -> bool {
    (82..=89).contains(&material)
}
pub fn is_stair(material: u16) -> bool {
    (74..=81).contains(&material)
}
/// Saved sand fences keep their ID and shape, but cannot be obtained anymore.
pub fn obtainable(material: u16) -> bool {
    material != 95
}

pub const DOUBLE_SLAB: u8 = 9;
const HALF_FACINGS: [u8; 6] = [0, 4, 5, 6, 7, 8];
pub const LAST_MIXED_SLAB: u8 = 57;

pub fn snow_bounds(state: u8) -> ShapeBox {
    ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0, f64::from(state.clamp(1, 8)) / 8.0, 1.0],
        hinged: false,
    }
}

/// A complete cell retains both materials and its original split in the saved
/// facing byte. Legacy facing 9 remains an identical-material double slab.
pub fn merged_slab(first: u16, facing: u8, second: u16) -> Option<u8> {
    if !is_slab(first) || !is_slab(second) || full_slab(facing) {
        return None;
    }
    if first == second {
        return Some(DOUBLE_SLAB);
    }
    let half = HALF_FACINGS
        .iter()
        .position(|&f| f == facing.min(8))
        .or_else(|| (facing < 4).then_some(0))?;
    Some(10 + (second - 82) as u8 * 6 + half as u8)
}

pub fn full_slab(facing: u8) -> bool {
    (DOUBLE_SLAB..=LAST_MIXED_SLAB).contains(&facing)
}

pub fn slab_parts(material: u16, facing: u8) -> Vec<(u16, ShapeBox)> {
    if (10..=LAST_MIXED_SLAB).contains(&facing) {
        let code = facing - 10;
        let half = HALF_FACINGS[usize::from(code % 6)];
        let opposite = match half {
            0 => 4,
            4 => 0,
            5 => 6,
            6 => 5,
            7 => 8,
            _ => 7,
        };
        vec![
            (material, slab_bounds(half)),
            (82 + u16::from(code / 6), slab_bounds(opposite)),
        ]
    } else {
        vec![(material, slab_bounds(facing))]
    }
}

/// Side faces use their upper/lower edge for horizontal placement. The central
/// band attaches a vertical half; top/bottom clicks always stack horizontally.
pub fn placement_at(material: u16, normal: [i8; 3], direction: [f64; 3], hit: [f64; 3]) -> u8 {
    if is_slab(material) && normal[1] == 0 {
        let height = hit[1].rem_euclid(1.0);
        if height <= 0.15 {
            return 0;
        }
        if height >= 0.85 {
            return 4;
        }
    }
    placement_facing(material, normal, direction)
}

pub fn valid_facing(material: u16, facing: u8) -> bool {
    if is_slab(material) {
        facing <= LAST_MIXED_SLAB
    } else if is_stair(material) {
        facing < 24
    } else {
        facing < 4
    }
}

fn cross(a: [i8; 3], b: [i8; 3]) -> [i8; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Proper cube rotations. Codes 0..4 retain the original upright yaw semantics.
pub fn stair_basis(facing: u8) -> [[i8; 3]; 3] {
    let up = [
        [0, 1, 0],
        [0, -1, 0],
        [1, 0, 0],
        [-1, 0, 0],
        [0, 0, 1],
        [0, 0, -1],
    ][usize::from(facing.min(23) / 4)];
    let forward = if up[2] == 0 { [0, 0, 1] } else { [0, 1, 0] };
    let right = cross(up, forward);
    let forward = match facing % 4 {
        0 => forward,
        1 => right,
        2 => forward.map(|v| -v),
        _ => right.map(|v| -v),
    };
    [cross(up, forward), up, forward]
}

pub fn placement_facing(material: u16, support_normal: [i8; 3], direction: [f64; 3]) -> u8 {
    if is_slab(material) {
        return match support_normal {
            [0, -1, 0] => 4,
            [1, 0, 0] => 5,
            [-1, 0, 0] => 6,
            [0, 0, 1] => 7,
            [0, 0, -1] => 8,
            _ => 0,
        };
    }
    let group = match support_normal {
        [0, -1, 0] => 1,
        [1, 0, 0] => 2,
        [-1, 0, 0] => 3,
        [0, 0, 1] => 4,
        [0, 0, -1] => 5,
        _ => 0,
    };
    (0..4)
        .map(|twist| group * 4 + twist)
        .max_by(|a, b| {
            let dot = |facing| {
                stair_basis(facing)[2]
                    .iter()
                    .zip(direction)
                    .map(|(a, b)| f64::from(*a) * b)
                    .sum::<f64>()
            };
            dot(*a).total_cmp(&dot(*b))
        })
        .unwrap_or(0)
}

pub fn slab_bounds(facing: u8) -> ShapeBox {
    let mut result = ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0; 3],
        hinged: false,
    };
    match facing {
        4 => result.minimum[1] = 0.5,
        5 => result.maximum[0] = 0.5,
        6 => result.minimum[0] = 0.5,
        7 => result.maximum[2] = 0.5,
        8 => result.minimum[2] = 0.5,
        DOUBLE_SLAB..=LAST_MIXED_SLAB => {}
        _ => result.maximum[1] = 0.5,
    }
    result
}

pub fn can_merge_slab(facing: u8, normal: [i8; 3]) -> bool {
    if full_slab(facing) {
        return false;
    }
    let b = slab_bounds(facing);
    (0..3).any(|axis| {
        normal[axis] > 0 && b.maximum[axis] == 0.5 || normal[axis] < 0 && b.minimum[axis] == 0.5
    })
}

fn rotate_box(shape: ShapeBox, basis: [[i8; 3]; 3]) -> ShapeBox {
    let mut result = ShapeBox {
        minimum: [f64::INFINITY; 3],
        maximum: [f64::NEG_INFINITY; 3],
        hinged: false,
    };
    for corner in 0..8 {
        let p: [f64; 3] = std::array::from_fn(|i| {
            if corner & (1 << i) == 0 {
                shape.minimum[i] - 0.5
            } else {
                shape.maximum[i] - 0.5
            }
        });
        let rotated: [f64; 3] = std::array::from_fn(|axis| {
            0.5 + (0..3)
                .map(|i| p[i] * f64::from(basis[i][axis]))
                .sum::<f64>()
        });
        for ((minimum, maximum), value) in result
            .minimum
            .iter_mut()
            .zip(&mut result.maximum)
            .zip(rotated)
        {
            *minimum = minimum.min(value);
            *maximum = maximum.max(value);
        }
    }
    result
}

/// Shared bounded profile for rendering, ray casting and physics. Neighbor order:
/// -X,+X,-Y,+Y,-Z,+Z. Corners are evaluated in the stair's support plane.
pub fn placed_boxes(material: u16, facing: u8, neighbors: [Option<(u16, u8)>; 6]) -> Vec<ShapeBox> {
    if is_slab(material) {
        return vec![slab_bounds(facing)];
    }
    if !is_stair(material) {
        return boxes(material).to_vec();
    }
    let basis = stair_basis(facing);
    // Wall mouldings can meet on perpendicular support planes while sharing
    // their rise direction. Union their solid thirds, never stretch a mesh.
    if let Some(other) = neighbors.iter().flatten().find_map(|(id, f)| {
        let other = stair_basis(*f);
        (is_stair(*id)
            && other[2] == basis[2]
            && basis[1]
                .iter()
                .zip(other[1])
                .map(|(a, b)| i32::from(*a) * i32::from(b))
                .sum::<i32>()
                == 0)
            .then_some(other)
    }) {
        let inside = |p: [f64; 3], frame: [[i8; 3]; 3]| {
            let along = |axis: usize| {
                (0..3)
                    .map(|i| (p[i] - 0.5) * f64::from(frame[axis][i]))
                    .sum::<f64>()
                    + 0.5
            };
            along(1) < (along(2) * 3.0).floor() / 3.0 + 1.0 / 3.0
        };
        let mut result = Vec::new();
        for x in 0..3 {
            for z in 0..3 {
                let mut y = 0;
                while y < 3 {
                    let occupied = |y: i32| {
                        let p = [
                            (f64::from(x) + 0.5) / 3.0,
                            (f64::from(y) + 0.5) / 3.0,
                            (f64::from(z) + 0.5) / 3.0,
                        ];
                        inside(p, basis) || inside(p, other)
                    };
                    if !occupied(y) {
                        y += 1;
                        continue;
                    }
                    let start = y;
                    y += 1;
                    while y < 3 && occupied(y) {
                        y += 1;
                    }
                    result.push(ShapeBox {
                        minimum: [
                            f64::from(x) / 3.0,
                            f64::from(start) / 3.0,
                            f64::from(z) / 3.0,
                        ],
                        maximum: [
                            f64::from(x + 1) / 3.0,
                            f64::from(y) / 3.0,
                            f64::from(z + 1) / 3.0,
                        ],
                        hinged: false,
                    });
                }
            }
        }
        return result;
    }
    let side = |v: [i8; 3]| {
        (0..3)
            .find(|&i| v[i] != 0)
            .map(|i| i * 2 + usize::from(v[i] > 0))
            .unwrap_or(0)
    };
    let turn = |direction| {
        neighbors[side(direction)].and_then(|(id, f)| {
            let other = stair_basis(f);
            (is_stair(id)
                && other[1] == basis[1]
                && other[2] != basis[2]
                && other[2] != basis[2].map(|v| -v))
            .then_some(other[2])
        })
    };
    let outer = turn(basis[2]);
    let inner = if outer.is_none() {
        turn(basis[2].map(|v| -v))
    } else {
        None
    };
    if outer.is_none() && inner.is_none() {
        return boxes(material)
            .iter()
            .map(|b| rotate_box(*b, basis))
            .collect();
    }
    let mut result = Vec::new();
    // Three treads, merged vertically per column: at most nine boxes, no per-voxel entities.
    for x in 0..3 {
        for z in 0..3 {
            let p = [
                f64::from(x) / 3.0 + 1.0 / 6.0,
                0.5,
                f64::from(z) / 3.0 + 1.0 / 6.0,
            ];
            let turned_height = |forward: [i8; 3]| {
                let world: [f64; 3] = std::array::from_fn(|axis| {
                    (0..3)
                        .map(|i| (p[i] - 0.5) * f64::from(basis[i][axis]))
                        .sum()
                });
                (((0..3)
                    .map(|i| world[i] * f64::from(forward[i]))
                    .sum::<f64>()
                    + 0.5)
                    * 3.0)
                    .floor() as i32
                    + 1
            };
            let mut height = z + 1;
            if let Some(v) = outer {
                height = height.min(turned_height(v));
            }
            if let Some(v) = inner {
                height = height.max(turned_height(v));
            }
            result.push(rotate_box(
                ShapeBox {
                    minimum: [f64::from(x) / 3.0, 0.0, f64::from(z) / 3.0],
                    maximum: [
                        f64::from(x + 1) / 3.0,
                        f64::from(height) / 3.0,
                        f64::from(z + 1) / 3.0,
                    ],
                    hinged: false,
                },
                basis,
            ));
        }
    }
    result
}
pub fn base_material(material: u16) -> Option<u16> {
    is_shape(material).then(|| [16, 17, 18, 3, 2, 4, 15, 49][usize::from((material - FIRST) % 8)])
}
pub fn boxes(material: u16) -> &'static [ShapeBox] {
    #[derive(Deserialize)]
    struct Definition {
        id: u16,
        collision_boxes: Vec<ShapeBox>,
    }
    static PROFILES: OnceLock<BTreeMap<u16, Vec<ShapeBox>>> = OnceLock::new();
    PROFILES
        .get_or_init(|| {
            [
                include_str!("../../../assets/terra/blocks/oak_stairs.json"),
                include_str!("../../../assets/terra/blocks/birch_stairs.json"),
                include_str!("../../../assets/terra/blocks/spruce_stairs.json"),
                include_str!("../../../assets/terra/blocks/stone_stairs.json"),
                include_str!("../../../assets/terra/blocks/dirt_stairs.json"),
                include_str!("../../../assets/terra/blocks/fine_sand_stairs.json"),
                include_str!("../../../assets/terra/blocks/glass_stairs.json"),
                include_str!("../../../assets/terra/blocks/ice_stairs.json"),
                include_str!("../../../assets/terra/blocks/oak_slab.json"),
                include_str!("../../../assets/terra/blocks/birch_slab.json"),
                include_str!("../../../assets/terra/blocks/spruce_slab.json"),
                include_str!("../../../assets/terra/blocks/stone_slab.json"),
                include_str!("../../../assets/terra/blocks/dirt_slab.json"),
                include_str!("../../../assets/terra/blocks/fine_sand_slab.json"),
                include_str!("../../../assets/terra/blocks/glass_slab.json"),
                include_str!("../../../assets/terra/blocks/ice_slab.json"),
                include_str!("../../../assets/terra/blocks/oak_fence.json"),
                include_str!("../../../assets/terra/blocks/birch_fence.json"),
                include_str!("../../../assets/terra/blocks/spruce_fence.json"),
                include_str!("../../../assets/terra/blocks/stone_fence.json"),
                include_str!("../../../assets/terra/blocks/dirt_fence.json"),
                include_str!("../../../assets/terra/blocks/fine_sand_fence.json"),
                include_str!("../../../assets/terra/blocks/glass_fence.json"),
                include_str!("../../../assets/terra/blocks/ice_fence.json"),
            ]
            .into_iter()
            .filter_map(|source| {
                let definition: Definition = serde_json::from_str(source).ok()?;
                (is_shape(definition.id)
                    && !definition.collision_boxes.is_empty()
                    && definition.collision_boxes.len() <= 128
                    && definition
                        .collision_boxes
                        .iter()
                        .all(|shape| shape.valid_for(definition.id)))
                .then_some((definition.id, definition.collision_boxes))
            })
            .collect()
        })
        .get(&material)
        .map_or(&[], Vec::as_slice)
}

/// First three boxes are the post/cap. Two rail boxes belong to each -X,+X,-Z,+Z arm.
pub fn fence_arm(box_index: usize) -> Option<usize> {
    (box_index >= 3)
        .then(|| (box_index - 3) / 2)
        .filter(|arm| *arm < 4)
}
pub fn connects(material: Option<u16>) -> bool {
    material.is_some_and(|id| {
        is_fence(id)
            || matches!(id, 15 | 49)
            || terra_voxel::MaterialId::from_raw(id).is_some_and(terra_voxel::MaterialId::is_opaque)
    })
}

pub fn fence_neighbor_side(arm: usize, facing: u8) -> usize {
    const SIDES: [[usize; 4]; 4] = [[0, 1, 4, 5], [5, 4, 0, 1], [1, 0, 5, 4], [4, 5, 1, 0]];
    SIDES[usize::from(facing % 4)][arm % 4]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn volume(boxes: &[ShapeBox]) -> f64 {
        boxes
            .iter()
            .map(|b| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
            .sum()
    }
    #[test]
    fn mixed_slab_pairs_preserve_both_materials_and_all_split_planes() {
        for first in 82..=89 {
            for second in 82..=89 {
                for facing in HALF_FACINGS {
                    let saved = merged_slab(first, facing, second).unwrap();
                    assert!(valid_facing(first, saved));
                    assert!(full_slab(saved));
                    assert!(!can_merge_slab(saved, [0, 1, 0]));
                    let parts = slab_parts(first, saved);
                    assert!(
                        (volume(&parts.iter().map(|(_, b)| *b).collect::<Vec<_>>()) - 1.0).abs()
                            < 1e-9
                    );
                    if first != second {
                        assert_eq!(parts[0], (first, slab_bounds(facing)));
                        assert_eq!(parts[1].0, second);
                        assert!(
                            !parts[0]
                                .1
                                .intersects(parts[1].1.minimum, parts[1].1.maximum)
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn side_edges_place_horizontal_slabs_while_the_middle_attaches_to_a_wall() {
        for (normal, middle) in [
            ([1, 0, 0], 5),
            ([-1, 0, 0], 6),
            ([0, 0, 1], 7),
            ([0, 0, -1], 8),
        ] {
            for base in [-17.0, 0.0, 32.0] {
                assert_eq!(
                    placement_at(82, normal, [0.0; 3], [0.5, base + 0.1, 0.5]),
                    0
                );
                assert_eq!(
                    placement_at(82, normal, [0.0; 3], [0.5, base + 0.9, 0.5]),
                    4
                );
                assert_eq!(
                    placement_at(82, normal, [0.0; 3], [0.5, base + 0.5, 0.5]),
                    middle
                );
            }
        }
    }
    #[test]
    fn snow_layers_are_exact_eighths_of_a_block() {
        for state in 1..=8 {
            let b = snow_bounds(state);
            assert!(b.valid());
            assert_eq!(volume(&[b]), f64::from(state) / 8.0);
        }
    }
    #[test]
    fn slab_halves_cover_all_six_faces_and_merge_only_on_the_inner_face() {
        for (facing, normal) in [
            (0, [0, 1, 0]),
            (4, [0, -1, 0]),
            (5, [1, 0, 0]),
            (6, [-1, 0, 0]),
            (7, [0, 0, 1]),
            (8, [0, 0, -1]),
        ] {
            let b = slab_bounds(facing);
            assert_eq!(volume(&[b]), 0.5);
            assert!(can_merge_slab(facing, normal));
            assert!(!can_merge_slab(facing, normal.map(|v| -v)));
            assert!(b.valid_for(82));
        }
        assert_eq!(volume(&[slab_bounds(DOUBLE_SLAB)]), 1.0);
        assert!(!can_merge_slab(DOUBLE_SLAB, [0, 1, 0]));
        // Historical yaw codes did not rotate slabs out of the lower half.
        for facing in 0..4 {
            assert_eq!(slab_bounds(facing), slab_bounds(0));
        }
    }
    #[test]
    fn stairs_have_twenty_four_proper_rotations_and_face_the_approaching_player() {
        let mut orientations = std::collections::HashSet::new();
        for facing in 0..24 {
            let basis = stair_basis(facing);
            assert_eq!(cross(basis[0], basis[1]), basis[2]);
            assert!(orientations.insert(basis));
            let profile = placed_boxes(74, facing, [None; 6]);
            assert_eq!(profile.len(), 3);
            assert!(profile.iter().all(|b| b.valid_for(74)));
            assert!((volume(&profile) - volume(boxes(74))).abs() < 1e-6);
        }
        assert_eq!(
            stair_basis(placement_facing(74, [0, 1, 0], [0.0, 0.0, -1.0]))[2],
            [0, 0, -1]
        );
        for normal in [
            [0, 1, 0],
            [0, -1, 0],
            [1, 0, 0],
            [-1, 0, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            assert_eq!(
                stair_basis(placement_facing(74, normal, [0.3, -0.4, -0.8]))[1],
                normal
            );
        }
    }
    #[test]
    fn inside_and_outside_corners_work_on_floors_ceilings_and_walls() {
        for facing in 0..24 {
            let b = stair_basis(facing);
            let side = |v: [i8; 3]| {
                let axis = v.iter().position(|v| *v != 0).unwrap();
                axis * 2 + usize::from(v[axis] > 0)
            };
            let mut outer = [None; 6];
            outer[side(b[2])] = Some((77, (facing / 4) * 4 + (facing + 1) % 4));
            let mut inner = [None; 6];
            inner[side(b[2].map(|v| -v))] = outer[side(b[2])];
            let a = placed_boxes(74, facing, outer);
            let c = placed_boxes(74, facing, inner);
            assert_eq!(a.len(), 9);
            assert_eq!(c.len(), 9);
            assert!(volume(&a) < volume(&c));
            assert!(a.iter().chain(&c).all(|b| b.valid_for(74)));
        }
        assert!(!obtainable(95));
        assert!(obtainable(94));
        let mut wall = [None; 6];
        wall[5] = Some((74, 16));
        let corner = placed_boxes(74, 11, wall);
        assert!(corner.len() <= 27);
        assert!(volume(&corner) > volume(boxes(74)));
        assert!(corner.iter().all(|b| b.valid_for(74)));
    }
    #[test]
    fn all_construction_profiles_are_valid_and_keep_material_identity() {
        for id in FIRST..=LAST {
            assert!(!boxes(id).is_empty(), "missing shape {id}");
            assert!(boxes(id).iter().all(|shape| shape.valid_for(id)));
            assert!(base_material(id).is_some());
        }
        assert!(boxes(82).iter().all(|shape| shape.maximum[1] == 0.5));
        assert_eq!(boxes(74).len(), 3);
        assert_eq!(boxes(90).len(), 11);
        assert_eq!(base_material(81), Some(49));
        assert!(!connects(Some(27)));
        assert!(!connects(Some(65)));
        assert!(connects(Some(3)));
        assert!(connects(Some(97)));
    }
}
