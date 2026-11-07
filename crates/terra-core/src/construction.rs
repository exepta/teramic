//! Shared construction profiles; the renderer and collision read the same JSON boxes.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const FIRST: u16 = 74;
pub const LAST: u16 = 97;
pub const BALLAST: u16 = 206;
pub const BALLAST_STAIRS: u16 = 207;
pub const BALLAST_SLAB: u16 = 208;

/// Loose gravel and all compacted ballast forms share railway foundation semantics.
pub const fn is_ballast(material: u16) -> bool {
    matches!(material, 42 | BALLAST | BALLAST_STAIRS | BALLAST_SLAB)
}

fn stair_steps(material: u16) -> usize {
    if material == BALLAST_STAIRS { 4 } else { 3 }
}
fn shared_steps(first: usize, second: usize) -> usize {
    if first == second { first } else { 12 }
}
pub fn is_shape(material: u16) -> bool {
    (FIRST..=LAST).contains(&material)
        || matches!(material, 104..=105 | 136..=138 | BALLAST_STAIRS | BALLAST_SLAB)
}
pub fn is_fence(material: u16) -> bool {
    (90..=97).contains(&material) || material == 138
}
pub fn is_slab(material: u16) -> bool {
    (82..=89).contains(&material) || matches!(material, 105 | 137 | BALLAST_SLAB)
}
pub fn is_stair(material: u16) -> bool {
    (74..=81).contains(&material) || matches!(material, 104 | 136 | BALLAST_STAIRS)
}
/// Saved sand fences keep their ID and shape, but cannot be obtained anymore.
pub fn obtainable(material: u16) -> bool {
    material != 95
}

pub const DOUBLE_SLAB: u8 = 9;
const HALF_FACINGS: [u8; 6] = [0, 4, 5, 6, 7, 8];
pub const SLAB_MATERIALS: [u16; 11] = [82, 83, 84, 85, 86, 87, 88, 89, 105, 137, BALLAST_SLAB];
pub const LAST_MIXED_SLAB: u8 = 75;

/// Weather retains legacy 1..=7 poses. New player layers use a disjoint range,
/// so later weather reconciliation never mistakes their height for ownership.
pub fn snow_layers(state: u8) -> u8 {
    match state {
        17..=23 => state - 16,
        _ => state.clamp(1, 8),
    }
}

pub fn manual_snow_facing(layers: u8) -> Option<u8> {
    (1..=7).contains(&layers).then(|| 16 + layers)
}

pub fn snow_bounds(state: u8) -> ShapeBox {
    ShapeBox {
        minimum: [0.0; 3],
        maximum: [1.0, f64::from(snow_layers(state)) / 8.0, 1.0],
        hinged: false,
    }
}

/// A complete cell retains both materials and its original split in the saved
/// facing byte. Legacy facing 9 remains an identical-material double slab.
pub fn merged_slab(first: u16, facing: u8, second: u16) -> Option<u8> {
    if !is_slab(first) || !is_slab(second) || full_slab(facing) {
        return None;
    }
    let half = HALF_FACINGS
        .iter()
        .position(|&f| f == facing.min(8))
        .or_else(|| (facing < 4).then_some(0))?;
    let second = SLAB_MATERIALS.iter().position(|&m| m == second)?;
    Some(10 + second as u8 * 6 + half as u8)
}

pub fn full_slab(facing: u8) -> bool {
    (DOUBLE_SLAB..=LAST_MIXED_SLAB).contains(&facing)
}

/// Both halves of a combined cell. Legacy state 9 did not retain its split
/// plane and is interpreted as the original horizontal lower/upper pair.
pub fn slab_halves(material: u16, facing: u8) -> Option<[(u16, u8); 2]> {
    if !is_slab(material) || !full_slab(facing) {
        return None;
    }
    if facing == DOUBLE_SLAB {
        return Some([(material, 0), (material, 4)]);
    }
    let code = facing - 10;
    let first = HALF_FACINGS[usize::from(code % 6)];
    let opposite = match first {
        0 => 4,
        4 => 0,
        5 => 6,
        6 => 5,
        7 => 8,
        _ => 7,
    };
    Some([
        (material, first),
        (SLAB_MATERIALS[usize::from(code / 6)], opposite),
    ])
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
            (SLAB_MATERIALS[usize::from(code / 6)], slab_bounds(opposite)),
        ]
    } else {
        vec![(material, slab_bounds(facing))]
    }
}

/// Edge clicks keep horizontal stacking available. The center of a floor or
/// ceiling places a vertical half facing the player, just like a wall's center.
pub fn placement_at(material: u16, normal: [i8; 3], direction: [f64; 3], hit: [f64; 3]) -> u8 {
    if is_slab(material) && normal[1] != 0 {
        let edge = [hit[0], hit[2]].into_iter().any(|v| {
            let local = v.rem_euclid(1.0);
            local <= 0.15 || local >= 0.85
        });
        if !edge {
            return if direction[0].abs() > direction[2].abs() {
                if direction[0] >= 0.0 { 5 } else { 6 }
            } else if direction[2] >= 0.0 {
                7
            } else {
                8
            };
        }
    }
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
    let steps = stair_steps(material);
    // Wall mouldings can meet on perpendicular support planes while sharing
    // their rise direction. Union their actual treads, never stretch a mesh.
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
            .then_some((other, stair_steps(*id)))
    }) {
        let (other, other_steps) = other;
        let resolution = shared_steps(steps, other_steps);
        let inside = |p: [f64; 3], frame: [[i8; 3]; 3], count: usize| {
            let along = |axis: usize| {
                (0..3)
                    .map(|i| (p[i] - 0.5) * f64::from(frame[axis][i]))
                    .sum::<f64>()
                    + 0.5
            };
            along(1) < (along(2) * count as f64).floor() / count as f64 + 1.0 / count as f64
        };
        let mut result = Vec::new();
        for x in 0..resolution {
            for z in 0..resolution {
                let mut y = 0;
                while y < resolution {
                    let occupied = |y: usize| {
                        let p = [
                            (x as f64 + 0.5) / resolution as f64,
                            (y as f64 + 0.5) / resolution as f64,
                            (z as f64 + 0.5) / resolution as f64,
                        ];
                        inside(p, basis, steps) || inside(p, other, other_steps)
                    };
                    if !occupied(y) {
                        y += 1;
                        continue;
                    }
                    let start = y;
                    y += 1;
                    while y < resolution && occupied(y) {
                        y += 1;
                    }
                    result.push(ShapeBox {
                        minimum: [
                            x as f64 / resolution as f64,
                            start as f64 / resolution as f64,
                            z as f64 / resolution as f64,
                        ],
                        maximum: [
                            (x + 1) as f64 / resolution as f64,
                            y as f64 / resolution as f64,
                            (z + 1) as f64 / resolution as f64,
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
            .then_some((other[2], stair_steps(id)))
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
    // Mixed thirds/quarters use their common twelfth-grid; at most144 columns.
    let resolution = shared_steps(steps, outer.or(inner).map_or(steps, |(_, count)| count));
    for x in 0..resolution {
        for z in 0..resolution {
            let p = [
                (x as f64 + 0.5) / resolution as f64,
                0.5,
                (z as f64 + 0.5) / resolution as f64,
            ];
            let turned_height = |(forward, count): ([i8; 3], usize)| {
                let world: [f64; 3] = std::array::from_fn(|axis| {
                    (0..3)
                        .map(|i| (p[i] - 0.5) * f64::from(basis[i][axis]))
                        .sum()
                });
                let along = (0..3)
                    .map(|i| world[i] * f64::from(forward[i]))
                    .sum::<f64>()
                    + 0.5;
                ((along * count as f64).floor() + 1.0) / count as f64
            };
            let mut height = ((p[2] * steps as f64).floor() + 1.0) / steps as f64;
            if let Some(direction) = outer {
                height = height.min(turned_height(direction));
            }
            if let Some(direction) = inner {
                height = height.max(turned_height(direction));
            }
            result.push(rotate_box(
                ShapeBox {
                    minimum: [
                        x as f64 / resolution as f64,
                        0.0,
                        z as f64 / resolution as f64,
                    ],
                    maximum: [
                        (x + 1) as f64 / resolution as f64,
                        height,
                        (z + 1) as f64 / resolution as f64,
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
    if matches!(material, BALLAST_STAIRS | BALLAST_SLAB) {
        return Some(BALLAST);
    }
    if matches!(material, 136..=138) {
        Some(135)
    } else if matches!(material, 104..=105) {
        Some(103)
    } else {
        (FIRST..=LAST)
            .contains(&material)
            .then(|| [16, 17, 18, 3, 2, 4, 15, 49][usize::from((material - FIRST) % 8)])
    }
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
                include_str!("../../../assets/terra/blocks/ballast_stairs.json"),
                include_str!("../../../assets/terra/blocks/ballast_slab.json"),
                include_str!("../../../assets/terra/blocks/cherry_stairs.json"),
                include_str!("../../../assets/terra/blocks/cherry_slab.json"),
                include_str!("../../../assets/terra/blocks/cherry_fence.json"),
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
                include_str!("../../../assets/terra/blocks/wool_stairs.json"),
                include_str!("../../../assets/terra/blocks/wool_slab.json"),
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
        for first in SLAB_MATERIALS {
            for second in SLAB_MATERIALS {
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
    fn floor_and_ceiling_centers_place_vertical_slabs_but_all_edges_remain_horizontal() {
        for base in [-17.0, 0.0, 32.0] {
            for normal in [[0, 1, 0], [0, -1, 0]] {
                for (direction, facing) in [
                    ([0.6, -0.8, 0.0], 5),
                    ([-0.6, 0.8, 0.0], 6),
                    ([0.0, -0.8, 0.6], 7),
                    ([0.0, 0.8, -0.6], 8),
                ] {
                    assert_eq!(
                        placement_at(82, normal, direction, [base + 0.5, 2.0, base + 0.5]),
                        facing
                    );
                    for edge in [0.05, 0.95] {
                        for axis in [0, 2] {
                            let mut hit = [base + 0.5, 2.0, base + 0.5];
                            hit[axis] = base + edge;
                            assert_eq!(
                                placement_at(82, normal, direction, hit),
                                if normal[1] > 0 { 0 } else { 4 }
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn new_identical_slab_pairs_retain_the_split_plane_and_legacy_pairs_remain_readable() {
        for material in 82..=89 {
            assert_eq!(
                slab_halves(material, DOUBLE_SLAB),
                Some([(material, 0), (material, 4)])
            );
            for facing in HALF_FACINGS {
                let merged = merged_slab(material, facing, material).unwrap();
                let halves = slab_halves(material, merged).unwrap();
                assert_eq!(halves[0], (material, facing));
                assert_eq!(halves[1].0, material);
                assert!(!slab_bounds(halves[0].1).intersects(
                    slab_bounds(halves[1].1).minimum,
                    slab_bounds(halves[1].1).maximum
                ));
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
        for layer in 1..=7 {
            let facing = manual_snow_facing(layer).unwrap();
            assert_eq!(snow_layers(facing), layer);
            assert_eq!(snow_bounds(facing), snow_bounds(layer));
        }
        assert_eq!(snow_layers(0), 1);
        assert_eq!(manual_snow_facing(0), None);
        assert_eq!(manual_snow_facing(8), None);
        assert_eq!(manual_snow_facing(255), None);
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
        for id in (FIRST..=LAST).chain([104, 105]) {
            assert!(!boxes(id).is_empty(), "missing shape {id}");
            assert!(boxes(id).iter().all(|shape| shape.valid_for(id)));
            assert!(base_material(id).is_some());
        }
        assert!(boxes(82).iter().all(|shape| shape.maximum[1] == 0.5));
        assert_eq!(boxes(74).len(), 3);
        assert_eq!(boxes(90).len(), 11);
        assert_eq!(base_material(81), Some(49));
        assert_eq!(base_material(104), Some(103));
        assert_eq!(base_material(105), Some(103));
        assert!(!is_shape(102));
        assert!(!is_shape(103));
        for material in 82..=89 {
            for facing in HALF_FACINGS {
                assert_eq!(
                    merged_slab(82, facing, material),
                    Some(
                        10 + (material - 82) as u8 * 6
                            + HALF_FACINGS.iter().position(|v| *v == facing).unwrap() as u8
                    )
                );
            }
        }
        assert!(!connects(Some(27)));
        assert!(!connects(Some(65)));
        assert!(connects(Some(3)));
        assert!(connects(Some(97)));
    }
}

#[cfg(test)]
#[path = "construction/ballast_tests.rs"]
mod ballast_tests;
