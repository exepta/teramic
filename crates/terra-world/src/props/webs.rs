//! Anchored silk uses the same persisted prop and section-batching lifecycle as plants.
use super::{PropId, PropKind, WorldProp};
mod construction;
pub use construction::{
    FIRST_CONSTRUCTION, MAX_SPAN, TEMPLATE_SPAN, anchors, find_spanning, is_construction, spanning,
    visit_strands, world_point,
};

pub const VARIANTS: usize = 10 + construction::CONSTRUCTION_VARIANTS;

fn solid_anchor(raw: u16) -> bool {
    terra_voxel::MaterialId::from_raw(raw).is_some_and(|m| {
        m.is_solid()
            && !matches!(
                m,
                terra_voxel::MaterialId::Leaves
                    | terra_voxel::MaterialId::AutumnLeaves
                    | terra_voxel::MaterialId::SilverLeaves
                    | terra_voxel::MaterialId::CherryLeaves
                    | terra_voxel::MaterialId::WhiteCherryLeaves
            )
    })
}

pub fn attached(anchor: [i64; 3], face: [i8; 3], variant: u8, id: PropId) -> Option<WorldProp> {
    if variant >= FIRST_CONSTRUCTION || face.iter().map(|v| i16::from(*v).abs()).sum::<i16>() != 1 {
        return None;
    }
    if (matches!(variant, 2 | 3) && face != [0, -1, 0])
        || (variant == 5 && face != [0, 1, 0])
        || (!matches!(variant, 2 | 3 | 5) && face[1] != 0)
    {
        return None;
    }
    let mut position = anchor.map(|v| v as f64 + 0.5);
    for axis in 0..3 {
        position[axis] += f64::from(face[axis]) * 0.512;
    }
    Some(WorldProp {
        id,
        kind: PropKind::SpiderWeb,
        position,
        rotation: if face[1] == 0 {
            f32::from(face[0]).atan2(f32::from(face[2]))
        } else {
            0.0
        },
        scale: 1.0,
        variant,
        section: terra_voxel::global_to_section(anchor).0,
        removed: false,
    })
}

pub fn support(prop: &WorldProp) -> [i64; 3] {
    let normal = normal(prop);
    std::array::from_fn(|axis| {
        (prop.position[axis] - f64::from(normal[axis]) * 0.512).floor() as i64
    })
}

pub fn normal(prop: &WorldProp) -> [i8; 3] {
    if is_construction(prop) {
        let (sin, cos) = prop.rotation.sin_cos();
        return [cos.round() as i8, 0, -sin.round() as i8];
    }
    match prop.variant {
        2 | 3 => [0, -1, 0],
        5 => [0, 1, 0],
        _ => {
            let (sin, cos) = prop.rotation.sin_cos();
            [sin.round() as i8, 0, cos.round() as i8]
        }
    }
}

/// The world's exterior is not cave air. Include the complete hanging model,
/// not just its anchor, so a bedrock underside can never carry visible silk.
pub fn within_world_bounds(prop: &WorldProp, bounded_world: bool) -> bool {
    if prop.kind != PropKind::SpiderWeb || !prop.valid() {
        return false;
    }
    if !bounded_world {
        return true;
    }
    let Some(bounds) = super::visual::template_bounds(prop.kind, prop.variant) else {
        return false;
    };
    (-128..=356).contains(&support(prop)[1])
        && prop.position[1] + f64::from(bounds[0][1] * prop.scale) >= -128.0
        && prop.position[1] + f64::from(bounds[1][1] * prop.scale) <= 357.0
}

pub fn fits_in_world(
    prop: &WorldProp,
    bounded_world: bool,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    within_world_bounds(prop, bounded_world)
        && fits(prop, |p| {
            if bounded_world && !(-128..=356).contains(&p[1]) {
                None
            } else {
                block(p)
            }
        })
}

/// Rock/tree contact plus a clear hanging volume. Unknown cells are never empty.
/// Tests a small fixed set, not every strand, both during generation and spinning.
pub fn fits(prop: &WorldProp, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> bool {
    if prop.kind != PropKind::SpiderWeb || !prop.valid() {
        return false;
    }
    if is_construction(prop) {
        return construction::fits(prop, block);
    }
    let anchor = support(prop);
    let solid = solid_anchor;
    if !block(anchor).is_some_and(solid) {
        return false;
    }
    let face = normal(prop).map(i64::from);
    let open = std::array::from_fn::<_, 3, _>(|axis| anchor[axis] + face[axis]);
    if block(open) != Some(0) {
        return false;
    }
    let (sin, cos) = f64::from(prop.rotation).sin_cos();
    let point = |p: [f64; 3]| {
        [
            (prop.position[0] + p[0] * cos + p[2] * sin).floor() as i64,
            (prop.position[1] + p[1]).floor() as i64,
            (prop.position[2] - p[0] * sin + p[2] * cos).floor() as i64,
        ]
    };
    if prop.variant == 3 {
        return [0.35, 0.85, 1.35, 1.85]
            .into_iter()
            .all(|depth| block(point([0.0, -depth, 0.0])) == Some(0));
    }
    if prop.variant == 2 {
        return [-0.8, 0.0, 0.8].into_iter().all(|x| {
            [-0.65, 0.0, 0.65].into_iter().all(|z| {
                block(point([x, 0.02, z])).is_some_and(solid)
                    && block(point([x, -0.7, z])) == Some(0)
                    && block(point([x, -1.35, z])) == Some(0)
            })
        });
    }
    if prop.variant == 5 {
        return [-0.8, 0.0, 0.8].into_iter().all(|x| {
            [-0.8, 0.8].into_iter().all(|z| {
                block(point([x, -0.05, z])).is_some_and(solid)
                    && block(point([x, 0.2, z])) == Some(0)
            })
        });
    }
    // At least three anchors hold an actual draped web, rather than a floating patch.
    let anchors = [
        [-0.9, 0.0, -0.03],
        [0.9, 0.0, -0.03],
        [-0.9, -1.8, -0.03],
        [0.9, -1.8, -0.03],
    ];
    let contacts = anchors
        .into_iter()
        .filter(|&p| block(point(p)).is_some_and(solid))
        .count();
    contacts >= 3
        && [-0.8, 0.0, 0.8].into_iter().all(|x| {
            [-0.3, -0.95, -1.6]
                .into_iter()
                .all(|y| block(point([x, y, 0.1])) == Some(0))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_world_exterior_is_not_a_spider_ceiling_even_with_solid_bedrock() {
        for anchor in [[-17, -128, -1], [16, -129, -16]] {
            let prop = attached(anchor, [0, -1, 0], 3, PropId(87)).unwrap();
            let underside = |p| Some(if p == anchor { 1 } else { 0 });
            assert!(
                fits(&prop, underside),
                "reproduces the old exterior attachment"
            );
            assert!(!within_world_bounds(&prop, true));
            assert!(!fits_in_world(&prop, true, underside));
            assert!(
                fits_in_world(&prop, false, underside),
                "legacy unbounded worlds keep their coordinates"
            );
        }
        let prop = attached([-17, -122, -1], [0, -1, 0], 3, PropId(88)).unwrap();
        assert!(fits_in_world(&prop, true, |p| Some(
            if p == support(&prop) { 1 } else { 0 }
        )));
        assert!(!fits_in_world(&prop, true, |p| (p == support(&prop)).then_some(1)));
        let span = spanning([-17, -126, -1], [1, 0, 0], 21, 10, PropId(89)).unwrap();
        assert!(
            !within_world_bounds(&span, true),
            "large capture surfaces must also stay inside the floor"
        );
    }

    #[test]
    fn web_anchors_roundtrip_negative_coordinates_and_all_orientations() {
        for face in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 0, 1],
            [0, 0, -1],
            [0, -1, 0],
            [0, 1, 0],
        ] {
            let variant = if face[1] == -1 {
                3
            } else if face[1] == 1 {
                5
            } else {
                0
            };
            for anchor in [[-16, -17, -1], [15, 15, 16], [0; 3]] {
                let prop = attached(anchor, face, variant, PropId(41)).unwrap();
                assert_eq!(support(&prop), anchor);
                assert!(prop.valid());
                assert_eq!(normal(&prop), face);
            }
        }
        assert!(attached([0; 3], [1, 1, 0], 0, PropId(1)).is_none());
        assert!(attached([0; 3], [0, 1, 0], 3, PropId(1)).is_none());
    }

    #[test]
    fn hanging_cocoon_requires_a_real_ceiling_and_complete_clearance() {
        let anchor = [-16, 0, -1];
        let prop = attached(anchor, [0, -1, 0], 3, PropId(2)).unwrap();
        assert!(fits(&prop, |p| Some(if p == anchor { 1 } else { 0 })));
        assert!(!fits(&prop, |_| Some(0)));
        assert!(!fits(&prop, |p| if p == anchor { Some(1) } else { None }));
        assert!(!fits(&prop, |p| Some(if p[1] <= -2 || p == anchor {
            1
        } else {
            0
        })));
    }

    #[test]
    fn ceiling_web_requires_side_anchors_and_lateral_strand_clearance() {
        let prop = attached([0, 10, 0], [0, -1, 0], 2, PropId(3)).unwrap();
        let room = |p: [i64; 3]| Some(if p[1] == 10 { 1 } else { 0 });
        assert!(fits(&prop, room));
        for side in [-1, 1] {
            let obstructed = [0, 9, side];
            assert!(!fits(&prop, |p| if p == obstructed {
                Some(1)
            } else {
                room(p)
            }));
            let missing_anchor = [0, 10, side];
            assert!(!fits(&prop, |p| if p == missing_anchor {
                Some(0)
            } else {
                room(p)
            }));
        }
        assert!(fits(&prop, room));
    }

    #[test]
    fn corner_web_retains_the_clear_wall_and_rejects_an_occupied_side_cell() {
        let prop = attached([0, 10, 0], [0, 0, 1], 1, PropId(4)).unwrap();
        let wall = |p: [i64; 3]| Some(if p[2] == 0 { 1 } else { 0 });
        assert!(fits(&prop, wall));
        for side in [-1, 1] {
            let obstructed = [side, 9, 1];
            assert!(!fits(&prop, |p| if p == obstructed {
                Some(1)
            } else {
                wall(p)
            }));
        }
    }
}
