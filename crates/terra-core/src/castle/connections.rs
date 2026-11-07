//! Neighbor-derived masonry profiles. Stored state remains only the existing pose.
use super::*;
use crate::block_shapes::ShapeBox;
use std::{collections::BTreeMap, sync::OnceLock};

pub const PROFILE_COUNT: usize = 16;
/// Seven reusable authored modules: default, center, four arms and an end crown.
pub const MODULE_COUNT: usize = 7;
pub const MODULE_PRIMITIVES: usize = 5;

pub const STACK_BELOW: u8 = 16;
pub const STACK_ABOVE: u8 = 32;

pub const fn profile_count(material: u16) -> usize {
    if material == BATTLEMENT_PILLAR {
        64
    } else {
        PROFILE_COUNT
    }
}

pub fn modules(material: u16, key: u8) -> Vec<usize> {
    let mask = key & 15;
    if material == BATTLEMENT_PILLAR {
        let mut parts = if key & STACK_ABOVE != 0 {
            if key & STACK_BELOW != 0 {
                vec![1]
            } else {
                vec![1, 2]
            }
        } else {
            vec![0]
        };
        parts.extend(
            (0..4)
                .filter(|port| mask & (1 << port) != 0)
                .map(|port| 3 + port),
        );
        return parts;
    }
    if mask == 0 {
        return vec![0];
    }
    let mut parts = vec![1];
    parts.extend(
        (0..4)
            .filter(|port| mask & (1 << port) != 0)
            .map(|port| 2 + port),
    );
    if mask.count_ones() == 1 {
        parts.push(6);
    }
    parts
}

pub const fn is_connecting(material: u16) -> bool {
    matches!(
        material,
        BERGFRIED_WALL | BATTLEMENT_STRAIGHT | BATTLEMENT_CORNER | BATTLEMENT_PILLAR
    )
}

/// Local ports -X,+X,-Z,+Z, transformed by the same 24 poses as the mesh.
pub fn port_side(port: usize, facing: u8) -> usize {
    let basis = crate::construction::stair_basis(facing);
    let axis = if port < 2 { 0 } else { 2 };
    let sign = if port.is_multiple_of(2) { -1 } else { 1 };
    let direction = basis[axis].map(|v| v * sign);
    let world_axis = direction.iter().position(|v| *v != 0).unwrap();
    world_axis * 2 + usize::from(direction[world_axis] > 0)
}

pub fn profile_boxes(material: u16, mask: u8) -> &'static [ShapeBox] {
    if !is_connecting(material) {
        return boxes(material);
    }
    static PROFILES: OnceLock<BTreeMap<u16, Vec<Vec<ShapeBox>>>> = OnceLock::new();
    &PROFILES.get_or_init(|| {
        let profiles: BTreeMap<u16, Vec<Vec<ShapeBox>>> = serde_json::from_str(include_str!(
            "../../../../assets/terra/blocks/models/castle_connections.json"
        ))
        .expect("Embedded connected castle profiles");
        for material in [
            BERGFRIED_WALL,
            BATTLEMENT_STRAIGHT,
            BATTLEMENT_CORNER,
            BATTLEMENT_PILLAR,
        ] {
            let shapes = &profiles[&material];
            assert_eq!(shapes.len(), profile_count(material));
            assert_eq!(shapes[0], boxes(material));
            assert!(shapes.iter().all(|shape| !shape.is_empty()
                && shape.len() <= 128
                && shape.iter().all(|b| b.valid_for(material))));
        }
        profiles
    })[&material][usize::from(mask) % profile_count(material)]
}

/// A family neighbor creates its reciprocal arm; other blocks connect only
/// where their actual shape reaches the matching port face.
pub fn connection_mask(material: u16, facing: u8, neighbors: [Option<(u16, u8)>; 6]) -> u8 {
    if !is_connecting(material) {
        return 0;
    }
    let up = crate::construction::stair_basis(facing)[1];
    let mut mask = 0;
    for port in 0..4 {
        let side = port_side(port, facing);
        let Some((other, pose)) = neighbors[side] else {
            continue;
        };
        let connects = if is_connecting(other) {
            crate::construction::stair_basis(pose)[1] == up
        } else {
            let bounds =
                if crate::construction::is_slab(other) || crate::construction::is_stair(other) {
                    crate::construction::placed_boxes(other, pose, [None; 6])
                } else if is_castle(other) {
                    boxes(other).iter().map(|b| rotated_box(*b, pose)).collect()
                } else if matches!(other, 15 | 49)
                    || terra_voxel::MaterialId::from_raw(other)
                        .is_some_and(terra_voxel::MaterialId::is_opaque)
                {
                    vec![ShapeBox {
                        minimum: [0.0; 3],
                        maximum: [1.0; 3],
                        hinged: false,
                    }]
                } else {
                    Vec::new()
                };
            let arm = rotated_box(
                ShapeBox {
                    minimum: [0.25, 0.0, 0.25],
                    maximum: [0.75, 0.68, 0.75],
                    hinged: false,
                },
                facing,
            );
            let axis = side / 2;
            let plane = if side.is_multiple_of(2) { 1.0 } else { 0.0 };
            // Authored solid masonry deliberately recesses its outer face by4mm
            // for mortar joints. That seam belongs to the same logical contact
            // face; a window/slab a quarter-cell away still has no such contact.
            let seam = if is_castle(other) { 0.004 + 1e-8 } else { 1e-8 };
            bounds.iter().any(|b| {
                b.minimum[axis] <= plane + seam
                    && b.maximum[axis] >= plane - seam
                    && (0..3).filter(|i| *i != axis).all(|i| {
                        b.minimum[i] < arm.maximum[i] - 1e-8 && b.maximum[i] > arm.minimum[i] + 1e-8
                    })
            })
        };
        if connects {
            mask |= 1 << port;
        }
    }
    mask
}

/// Local stacking follows the pillar's actual support axis, including ceiling
/// and side poses. Horizontal connectors never replace the original crowns.
pub fn profile_key(material: u16, facing: u8, neighbors: [Option<(u16, u8)>; 6]) -> u8 {
    let mut key = connection_mask(material, facing, neighbors);
    if material != BATTLEMENT_PILLAR {
        return key;
    }
    let up = crate::construction::stair_basis(facing)[1];
    let axis = up.iter().position(|v| *v != 0).unwrap();
    let above = axis * 2 + usize::from(up[axis] > 0);
    for (side, bit) in [(above, STACK_ABOVE), (above ^ 1, STACK_BELOW)] {
        if neighbors[side].is_some_and(|(id, pose)| {
            id == BATTLEMENT_PILLAR && crate::construction::stair_basis(pose)[1] == up
        }) {
            key |= bit;
        }
    }
    key
}

pub fn placed_boxes(material: u16, facing: u8, neighbors: [Option<(u16, u8)>; 6]) -> Vec<ShapeBox> {
    profile_boxes(material, profile_key(material, facing, neighbors))
        .iter()
        .map(|b| rotated_box(*b, facing))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const FAMILY: [u16; 4] = [
        BERGFRIED_WALL,
        BATTLEMENT_STRAIGHT,
        BATTLEMENT_CORNER,
        BATTLEMENT_PILLAR,
    ];
    fn contains(boxes: &[ShapeBox], point: [f64; 3]) -> bool {
        boxes.iter().any(|b| {
            (0..3).all(|i| point[i] >= b.minimum[i] - 1e-8 && point[i] <= b.maximum[i] + 1e-8)
        })
    }
    #[test]
    fn every_end_straight_corner_t_and_cross_profile_matches_all_twenty_four_poses() {
        let tips = [
            [0.10, 0.85, 0.5],
            [0.90, 0.85, 0.5],
            [0.5, 0.85, 0.10],
            [0.5, 0.85, 0.90],
        ];
        for material in [BERGFRIED_WALL, BATTLEMENT_STRAIGHT, BATTLEMENT_CORNER] {
            for facing in 0..24 {
                for mask in 1_u8..16 {
                    let mut neighbors = [None; 6];
                    for port in 0..4 {
                        if mask & (1 << port) != 0 {
                            neighbors[port_side(port, facing)] =
                                Some((FAMILY[port], facing / 4 * 4 + (facing + 1) % 4));
                        }
                    }
                    assert_eq!(connection_mask(material, facing, neighbors), mask);
                    let profile = placed_boxes(material, facing, neighbors);
                    assert!(profile.iter().all(|b| b.valid_for(material)));
                    for (port, tip) in tips.into_iter().enumerate() {
                        assert_eq!(
                            contains(&profile, point(facing, tip)),
                            mask & (1 << port) != 0,
                            "wrong crown {material}/{facing}/{mask}/{port}"
                        );
                    }
                    assert_eq!(
                        contains(&profile, point(facing, [0.5, 0.85, 0.5])),
                        mask.count_ones() == 1,
                        "only a free end has a center crown"
                    );
                }
            }
        }
    }
    #[test]
    fn adjacent_arms_meet_the_same_exact_face_at_every_cube_rotation() {
        for facing in 0..24 {
            for port in 0..4 {
                let side = port_side(port, facing);
                let mut neighbor = [None; 6];
                neighbor[side] = Some((BATTLEMENT_CORNER, facing));
                let first = placed_boxes(BATTLEMENT_STRAIGHT, facing, neighbor);
                let mut back = [None; 6];
                back[side ^ 1] = Some((BATTLEMENT_STRAIGHT, facing));
                let second = placed_boxes(BATTLEMENT_CORNER, facing, back);
                let axis = side / 2;
                let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
                let mut seam = point(
                    facing,
                    match port {
                        0 => [0.0, 0.35, 0.5],
                        1 => [1.0, 0.35, 0.5],
                        2 => [0.5, 0.35, 0.0],
                        _ => [0.5, 0.35, 1.0],
                    },
                );
                assert!(contains(&first, seam));
                seam[axis] -= sign;
                assert!(
                    contains(&second, seam),
                    "broken reciprocal face {facing}/{port}"
                );
            }
        }
    }
    #[test]
    fn corner_has_exactly_two_crowns_and_no_solid_third_tooth() {
        let source = profile_boxes(BATTLEMENT_CORNER, 0);
        assert_eq!(source.iter().filter(|b| b.minimum[1] >= 0.68).count(), 2);
        assert_eq!(source.len(), profile_boxes(BATTLEMENT_CORNER, 5).len());
        assert!(
            source
                .iter()
                .all(|b| profile_boxes(BATTLEMENT_CORNER, 5).contains(b))
        );
        assert!(!contains(source, [0.5, 0.85, 0.5]));
        assert!(contains(source, [0.1, 0.35, 0.5]));
        assert!(contains(source, [0.5, 0.35, 0.1]));
        assert!(!contains(source, [0.9, 0.35, 0.9]));
    }
    #[test]
    fn attachments_use_actual_neighbor_faces_and_the_same_support_plane() {
        let mut neighbors = [None; 6];
        for id in [0, 14, 27, 40, 193, 194] {
            neighbors[1] = Some((id, 0));
            assert_eq!(connection_mask(BATTLEMENT_STRAIGHT, 0, neighbors), 0);
        }
        for id in [3, 15, STONE_WALL, WINDOW] {
            neighbors[1] = Some((id, 0));
            assert_eq!(
                connection_mask(BATTLEMENT_STRAIGHT, 0, neighbors),
                2,
                "missing actual contact with material{id}"
            );
        }
        // The4mm masonry seam is joinable on both axes and in all24poses.
        for facing in 0..24 {
            for port in 0..4 {
                let mut adjacent = [None; 6];
                adjacent[port_side(port, facing)] = Some((STONE_WALL, facing));
                assert_eq!(
                    connection_mask(BATTLEMENT_STRAIGHT, facing, adjacent),
                    1 << port,
                    "mortar seam lost its port: {facing}/{port}"
                );
            }
        }
        // The window's thin plane is inset a quarter-cell along its normal,
        // much farther than a mortar joint: no full-block attachment there.
        let mut across_window = [None; 6];
        across_window[5] = Some((WINDOW, 0));
        assert_eq!(connection_mask(BATTLEMENT_STRAIGHT, 0, across_window), 0);
        neighbors[1] = Some((82, 6)); // A vertical slab on its far half has no contact.
        assert_eq!(connection_mask(BATTLEMENT_STRAIGHT, 0, neighbors), 0);
        neighbors[1] = Some((82, 5));
        assert_eq!(connection_mask(BATTLEMENT_STRAIGHT, 0, neighbors), 2);
        for other in 4..24 {
            neighbors[1] = Some((BATTLEMENT_CORNER, other));
            assert_eq!(connection_mask(BATTLEMENT_STRAIGHT, 0, neighbors), 0);
        }
    }
}

#[cfg(test)]
mod pillar_tests {
    use super::*;
    fn contains(boxes: &[ShapeBox], p: [f64; 3]) -> bool {
        boxes.iter().any(|b| {
            (0..3)
                .all(|axis| p[axis] >= b.minimum[axis] - 1e-8 && p[axis] <= b.maximum[axis] + 1e-8)
        })
    }
    fn neighbors(facing: u8, key: u8) -> [Option<(u16, u8)>; 6] {
        let mut result = [None; 6];
        for port in 0..4 {
            if key & (1 << port) != 0 {
                result[port_side(port, facing)] = Some((BATTLEMENT_PILLAR, facing));
            }
        }
        let up = crate::construction::stair_basis(facing)[1];
        let axis = up.iter().position(|v| *v != 0).unwrap();
        let top = axis * 2 + usize::from(up[axis] > 0);
        if key & STACK_ABOVE != 0 {
            result[top] = Some((BATTLEMENT_PILLAR, facing / 4 * 4 + (facing + 1) % 4));
        }
        if key & STACK_BELOW != 0 {
            result[top ^ 1] = Some((BATTLEMENT_PILLAR, facing / 4 * 4 + (facing + 2) % 4));
        }
        result
    }
    #[test]
    fn pillars_preserve_all_original_crowns_with_every_horizontal_connector_and_pose() {
        let original = boxes(BATTLEMENT_PILLAR);
        assert_eq!(original.iter().filter(|b| b.minimum[1] >= 0.78).count(), 4);
        assert_eq!(profile_boxes(BATTLEMENT_PILLAR, 0), original);
        for facing in 0..24 {
            for mask in 0_u8..16 {
                let adjacent = neighbors(facing, mask);
                assert_eq!(profile_key(BATTLEMENT_PILLAR, facing, adjacent), mask);
                let profile = placed_boxes(BATTLEMENT_PILLAR, facing, adjacent);
                for crown in original.iter().filter(|b| b.minimum[1] >= 0.78) {
                    assert!(
                        profile.contains(&rotated_box(*crown, facing)),
                        "connector removed original crown {facing}/{mask}"
                    );
                }
                for port in 0..4 {
                    let face = point(
                        facing,
                        match port {
                            0 => [0.0, 0.35, 0.5],
                            1 => [1.0, 0.35, 0.5],
                            2 => [0.5, 0.35, 0.0],
                            _ => [0.5, 0.35, 1.0],
                        },
                    );
                    assert_eq!(contains(&profile, face), mask & (1 << port) != 0);
                }
            }
        }
    }
    #[test]
    fn stacked_pillars_keep_only_the_outer_foot_and_top_crowns_in_all_twenty_four_poses() {
        for facing in 0..24 {
            for key in 0_u8..64 {
                let adjacent = neighbors(facing, key);
                assert_eq!(profile_key(BATTLEMENT_PILLAR, facing, adjacent), key);
                let profile = placed_boxes(BATTLEMENT_PILLAR, facing, adjacent);
                let above = key & STACK_ABOVE != 0;
                let below = key & STACK_BELOW != 0;
                assert_eq!(
                    contains(&profile, point(facing, [0.15, 0.70, 0.15])),
                    !above,
                    "interior frieze {facing}/{key}"
                );
                assert_eq!(
                    contains(&profile, point(facing, [0.15, 0.85, 0.15])),
                    !above,
                    "interior crown {facing}/{key}"
                );
                assert_eq!(
                    contains(&profile, point(facing, [0.15, 0.05, 0.15])),
                    above && !below,
                    "interior foot {facing}/{key}"
                );
                assert_eq!(
                    contains(&profile, point(facing, [0.5, 0.95, 0.5])),
                    above,
                    "gap in continuous shaft {facing}/{key}"
                );
                assert!(profile.iter().all(|b| b.valid_for(BATTLEMENT_PILLAR)));
            }
        }
        let middle = profile_boxes(BATTLEMENT_PILLAR, STACK_ABOVE | STACK_BELOW);
        assert_eq!(
            middle,
            &[ShapeBox {
                minimum: [0.2, 0.0, 0.2],
                maximum: [0.8, 1.0, 0.8],
                hinged: false
            }]
        );
    }
    #[test]
    fn pillar_stack_only_merges_matching_support_axes_and_rebuilds_after_neighbor_changes() {
        for facing in 0..24 {
            let mut adjacent = neighbors(facing, STACK_ABOVE | STACK_BELOW);
            let up = crate::construction::stair_basis(facing)[1];
            let axis = up.iter().position(|v| *v != 0).unwrap();
            let top = axis * 2 + usize::from(up[axis] > 0);
            for (other, id) in [
                (facing, BATTLEMENT_STRAIGHT),
                ((facing + 4) % 24, BATTLEMENT_PILLAR),
            ] {
                adjacent[top] = Some((id, other));
                assert_eq!(
                    profile_key(BATTLEMENT_PILLAR, facing, adjacent),
                    STACK_BELOW
                );
            }
            adjacent[top] = None;
            adjacent[top ^ 1] = None;
            assert_eq!(profile_key(BATTLEMENT_PILLAR, facing, adjacent), 0);
        }
    }
    #[test]
    fn old_corner_identity_remains_readable_but_has_the_main_wall_item_alias() {
        assert_eq!(
            terra_voxel::MaterialId::from_raw(BATTLEMENT_CORNER).unwrap() as u16,
            181
        );
        assert!(definition(BATTLEMENT_CORNER).is_some());
        assert!(retired_item(BATTLEMENT_CORNER));
        assert_eq!(item_material(BATTLEMENT_CORNER), BATTLEMENT_STRAIGHT);
        assert!(!retired_item(BATTLEMENT_STRAIGHT));
        assert_eq!(
            profile_boxes(BATTLEMENT_STRAIGHT, 5)
                .iter()
                .filter(|b| b.minimum[1] >= 0.68)
                .count(),
            2
        );
    }
}
