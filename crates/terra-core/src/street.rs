//! Shared sidewalk profiles: the visible curb and its collision always agree.
use crate::block_shapes::ShapeBox;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub const BLOCK: u16 = 441;
pub const NORMAL: u16 = 442;
pub const LEFT: u16 = 443;
pub const RIGHT: u16 = 444;
pub const ARC_LEFT: u16 = 445;
pub const ARC_RIGHT: u16 = 446;
pub const LOWERED: u16 = 447;
pub const STRAW: u16 = 448;

pub const fn is_shape(material: u16) -> bool {
    matches!(material, BLOCK..=LOWERED)
}

pub const fn is_arc(material: u16) -> bool {
    matches!(material, ARC_LEFT | ARC_RIGHT)
}

/// The free corner touches two cardinal edges. A path under this corner may
/// receive tar from those edges, even after the player installs the paver.
pub fn arc_opens_to(material: u16, facing: u8, [dx, _, dz]: [i64; 3]) -> bool {
    let (x, z) = match facing % 4 {
        0 => (dx, dz),
        1 => (-dz, dx),
        2 => (-dx, -dz),
        _ => (dz, -dx),
    };
    z < 0 || x == if material == ARC_LEFT { -1 } else { 1 }
}

/// The authored street edge is local -Z. Prefer a neighboring road; otherwise
/// face the edge toward the horizontal direction in which the player looks.
pub fn placement_facing(
    material: u16,
    position: [i64; 3],
    direction: [f64; 3],
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> u8 {
    let directions = [[0, 0, -1], [-1, 0, 0], [0, 0, 1], [1, 0, 0]];
    let roads = directions.map(|d| {
        let p = std::array::from_fn(|i| position[i] + d[i]);
        [p, [p[0], p[1] - 1, p[2]]]
            .into_iter()
            .any(|cell| block(cell).is_some_and(|id| id == 102 || (431..=438).contains(&id)))
    });
    let score = |facing: usize| {
        let contacts = if is_arc(material) {
            directions
                .iter()
                .zip(roads)
                .filter(|(side, road)| *road && arc_opens_to(material, facing as u8, **side))
                .count()
        } else {
            usize::from(roads[facing])
        };
        let alignment = directions[facing]
            .into_iter()
            .zip(direction)
            .map(|(axis, look)| axis as f64 * look)
            .sum::<f64>();
        (contacts, roads[facing], alignment)
    };
    (0..4)
        .max_by(|&a, &b| {
            let a = score(a);
            let b = score(b);
            a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2))
        })
        .unwrap_or(0) as u8
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
                include_str!("../../../assets/terra/blocks/sidewalk_block.json"),
                include_str!("../../../assets/terra/blocks/sidewalk.json"),
                include_str!("../../../assets/terra/blocks/sidewalk_left.json"),
                include_str!("../../../assets/terra/blocks/sidewalk_right.json"),
                include_str!("../../../assets/terra/blocks/sidewalk_arc_left.json"),
                include_str!("../../../assets/terra/blocks/sidewalk_arc_right.json"),
                include_str!("../../../assets/terra/blocks/sidewalk_lowered.json"),
            ]
            .into_iter()
            .map(|source| {
                let definition: Definition =
                    serde_json::from_str(source).expect("valid sidewalk profile");
                assert!(
                    definition
                        .collision_boxes
                        .iter()
                        .all(|b| b.valid_for(definition.id))
                );
                (definition.id, definition.collision_boxes)
            })
            .collect()
        })
        .get(&material)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn street_edge_follows_adjacent_roads_and_all_shapes_remain_upright() {
        for (f, delta) in [[0, 0, -1], [-1, 0, 0], [0, 0, 1], [1, 0, 0]]
            .into_iter()
            .enumerate()
        {
            let facing = placement_facing(NORMAL, [0; 3], [0., 0., -1.], |p| {
                (p == delta).then_some(431)
            });
            assert_eq!(usize::from(facing), f);
            for id in NORMAL..=LOWERED {
                let placed = crate::construction::placed_boxes(id, facing, [None; 6]);
                assert!(!placed.is_empty());
                assert!(
                    placed
                        .iter()
                        .all(|b| b.minimum[1] == 0. || b.minimum[1] == 0.5)
                );
                assert!(placed.iter().all(|b| b.valid_for(id)));
                assert_eq!(crate::construction::base_material(id), Some(BLOCK));
            }
        }
        assert_eq!(placement_facing(NORMAL, [0; 3], [1., 0., 0.], |_| None), 3);
    }

    #[test]
    fn lowered_sidewalk_meets_road_cover_and_paved_deck_without_an_abrupt_step() {
        let profile = boxes(LOWERED);
        assert_eq!(profile.len(), 16);
        assert!(
            profile
                .windows(2)
                .all(|p| p[0].maximum[2] == p[1].minimum[2])
        );
        assert!(profile[0].maximum[1] - 0.125 < 0.025);
        assert_eq!(profile.last().unwrap().maximum[1], 0.25);
        let curb = boxes(NORMAL);
        assert_eq!(
            curb.len(),
            1,
            "the dark curb is flush with the walking deck"
        );
        assert_eq!(curb[0].minimum, [0.; 3]);
        assert_eq!(curb[0].maximum, [1., 0.25, 1.]);
        assert_eq!(boxes(BLOCK)[0].maximum, [1., 0.5, 1.]);
        assert!(!crate::construction::is_slab(BLOCK));
        for id in NORMAL..LOWERED {
            assert!(boxes(id).iter().all(|part| part.maximum[1] == 0.25));
        }
    }
}
