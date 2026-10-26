use super::{PropId, PropKind, WorldProp};
use bevy_math::Vec3;

pub const FIRST_CONSTRUCTION: u8 = 10;
pub const CONSTRUCTION_VARIANTS: usize = 4;
pub const MAX_SPAN: f64 = 20.0;
pub const TEMPLATE_SPAN: f32 = 10.0;

fn solid_anchor(raw: u16) -> bool {
    super::solid_anchor(raw) && terra_core::block_shapes::builtin_boxes(raw).is_empty()
}

pub fn is_construction(prop: &WorldProp) -> bool {
    prop.kind == PropKind::SpiderWeb
        && (FIRST_CONSTRUCTION..FIRST_CONSTRUCTION + CONSTRUCTION_VARIANTS as u8)
            .contains(&prop.variant)
}

/// Both endpoints retain the existing face offset; the scale encodes the gap,
/// not an invented anchor or a second persisted geometry representation.
pub fn spanning(
    anchor: [i64; 3],
    face: [i8; 3],
    separation: u8,
    variant: u8,
    id: PropId,
) -> Option<WorldProp> {
    if !(8..=21).contains(&separation)
        || face[1] != 0
        || face.iter().map(|v| i16::from(*v).abs()).sum::<i16>() != 1
        || !(FIRST_CONSTRUCTION..FIRST_CONSTRUCTION + CONSTRUCTION_VARIANTS as u8)
            .contains(&variant)
    {
        return None;
    }
    let prop = WorldProp {
        id,
        kind: PropKind::SpiderWeb,
        position: std::array::from_fn(|i| anchor[i] as f64 + 0.5 + f64::from(face[i]) * 0.512),
        rotation: (-f32::from(face[2])).atan2(f32::from(face[0])),
        scale: (f32::from(separation) - 1.024) / TEMPLATE_SPAN,
        variant,
        section: terra_voxel::global_to_section(anchor).0,
        removed: false,
    };
    prop.valid().then_some(prop)
}

pub fn world_point(prop: &WorldProp, point: Vec3) -> [f64; 3] {
    let (sin, cos) = f64::from(prop.rotation).sin_cos();
    let [x, y, z] = point.to_array().map(|v| f64::from(v * prop.scale));
    [
        prop.position[0] + x * cos + z * sin,
        prop.position[1] + y,
        prop.position[2] - x * sin + z * cos,
    ]
}

pub fn anchors(prop: &WorldProp) -> Option<[[i64; 3]; 4]> {
    if !is_construction(prop) {
        return None;
    }
    let normal = super::normal(prop).map(f64::from);
    Some(std::array::from_fn(|index| {
        let far = index & 1 != 0;
        let point = world_point(
            prop,
            Vec3::new(
                if far { TEMPLATE_SPAN } else { 0.0 },
                if index >= 2 { -0.45 } else { 0.0 },
                0.0,
            ),
        );
        std::array::from_fn(|i| {
            (point[i] + normal[i] * if far { 0.04 } else { -0.04 }).floor() as i64
        })
    }))
}

/// One cached batch contains load-bearing cables and three interconnected,
/// differently tilted capture surfaces. No entity or material per strand.
pub fn visit_strands(variant: u8, detail: u8, mut strand: impl FnMut(Vec3, Vec3, f32)) {
    let style = (variant.saturating_sub(FIRST_CONSTRUCTION) % 4) as usize;
    let mirror = if style & 1 == 0 { 1.0 } else { -1.0 };
    let rings = match detail {
        0 => 4,
        1 => 3,
        _ => 2,
    };
    let spokes = if detail == 2 { 6 } else { 8 };
    let thick = if detail == 2 { 0.017 } else { 0.010 };
    let panels = [
        (Vec3::new(2.3, -0.75, -0.32 * mirror), [1.65, 0.62]),
        (Vec3::new(5.1, -1.02, 0.32 * mirror), [2.0, 0.88]),
        (Vec3::new(7.7, -0.70, -0.20 * mirror), [1.62, 0.58]),
    ];
    let points = |panel: usize, spoke: usize| {
        let angle = spoke as f32 / spokes as f32 * std::f32::consts::TAU;
        let (center, radius) = panels[panel];
        center
            + Vec3::new(
                angle.cos() * radius[0],
                angle.sin() * radius[1],
                angle.sin() * (0.16 + panel as f32 * 0.05) * mirror,
            )
    };
    let anchors = [
        Vec3::ZERO,
        Vec3::new(0.0, -0.45, 0.0),
        Vec3::new(TEMPLATE_SPAN, 0.0, 0.0),
        Vec3::new(TEMPLATE_SPAN, -0.45, 0.0),
    ];
    let cable = |a: Vec3, b: Vec3, out: &mut dyn FnMut(Vec3, Vec3, f32)| {
        let mid = (a + b) * 0.5 - Vec3::Y * 0.045;
        out(a, mid, 0.024);
        out(mid, b, 0.024);
    };
    cable(anchors[0], points(0, 2), &mut strand);
    cable(anchors[1], points(0, spokes / 2), &mut strand);
    cable(anchors[0], points(1, 2), &mut strand);
    cable(anchors[2], points(1, 2), &mut strand);
    cable(anchors[2], points(2, 0), &mut strand);
    cable(anchors[3], points(2, spokes - 2), &mut strand);
    cable(points(0, 0), points(1, spokes / 2), &mut strand);
    cable(points(1, 0), points(2, spokes / 2), &mut strand);
    if style >= 2 {
        cable(anchors[1], points(1, spokes - 2), &mut strand);
        cable(anchors[3], points(0, spokes - 2), &mut strand);
    }
    for (panel, (center, _)) in panels.into_iter().enumerate() {
        for spoke in 0..spokes {
            let edge = points(panel, spoke);
            strand(center, edge, thick * 1.4);
            for ring in 1..=rings {
                let t = ring as f32 / rings as f32;
                let a = center.lerp(edge, t);
                let b = center.lerp(points(panel, (spoke + 1) % spokes), t);
                let sag = (a + b) * 0.5 - Vec3::Y * 0.022;
                strand(a, sag, thick);
                strand(sag, b, thick);
            }
        }
    }
}

pub(super) fn fits(prop: &WorldProp, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> bool {
    let Some(anchors) = anchors(prop) else {
        return false;
    };
    if !anchors
        .into_iter()
        .all(|anchor| block(anchor).is_some_and(solid_anchor))
    {
        return false;
    }
    let mut clear = true;
    let mut checked = std::collections::HashSet::with_capacity(128);
    visit_strands(prop.variant, 0, |a, b, _| {
        if !clear {
            return;
        }
        let steps = ((b - a).length() * prop.scale / 0.3).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let point = world_point(prop, a.lerp(b, step as f32 / steps as f32));
            let cell = point.map(|v| v.floor() as i64);
            if checked.insert(cell) && block(cell) != Some(0) {
                clear = false;
                break;
            }
        }
    });
    // A low ceiling may carry a tiny legacy web, but never a twenty-metre
    // barrier that seals its only walkable connection.
    clear
        && [0.22, 0.5, 0.78].into_iter().all(|fraction| {
            (0..=7).all(|step| {
                let mut point = world_point(prop, Vec3::new(TEMPLATE_SPAN * fraction, -1.95, 0.0));
                point[1] -= 0.1 + f64::from(step) * 0.3;
                block(point.map(|v| v.floor() as i64)) == Some(0)
            })
        })
}

pub fn find_spanning(
    anchor: [i64; 3],
    face: [i8; 3],
    id: PropId,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<WorldProp> {
    if face[1] != 0 || !block(anchor).is_some_and(solid_anchor) {
        return None;
    }
    for distance in 1..=21 {
        let end = std::array::from_fn(|i| anchor[i] + i64::from(face[i]) * distance);
        match block(end)? {
            0 => {}
            raw if distance >= 8 && solid_anchor(raw) => {
                let prop = spanning(
                    anchor,
                    face,
                    distance as u8,
                    FIRST_CONSTRUCTION + ((id.0 >> 13) % CONSTRUCTION_VARIANTS as u64) as u8,
                    id,
                )?;
                return fits(&prop, block).then_some(prop);
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_opposing_anchors_create_deterministic_varied_spans_across_negative_sections() {
        for face in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for separation in [8, 12, 16, 21] {
                let anchor = [-17, -9, 15];
                let axis = face.iter().position(|v| *v != 0).unwrap();
                let opposite = anchor[axis] + i64::from(face[axis]) * i64::from(separation);
                let room = |p: [i64; 3]| {
                    Some(if p[axis] == anchor[axis] || p[axis] == opposite {
                        3
                    } else {
                        0
                    })
                };
                let a = find_spanning(anchor, face, PropId(4117), room).unwrap();
                assert_eq!(a, find_spanning(anchor, face, PropId(4117), room).unwrap());
                assert_eq!(super::super::support(&a), anchor);
                assert!(a.valid() && fits(&a, room));
                let span = f64::from(a.scale * TEMPLATE_SPAN);
                assert!((6.0..=MAX_SPAN).contains(&span));
                assert!((span - (f64::from(separation) - 1.024)).abs() < 0.001);
                assert!(anchors(&a).unwrap().iter().all(|p| room(*p) == Some(3)));
                let far = anchors(&a).unwrap()[1];
                assert_eq!(far[axis], opposite);
                assert_eq!(a.section, terra_voxel::global_to_section(anchor).0);
            }
        }
    }

    #[test]
    fn silk_requires_loaded_dry_strands_both_anchors_and_a_walkable_clearance() {
        let anchor = [14, 11, 8];
        let prop = spanning(anchor, [1, 0, 0], 21, 12, PropId(4118)).unwrap();
        let room = |p: [i64; 3]| Some(if matches!(p[0], 14 | 35) { 3 } else { 0 });
        assert!(fits(&prop, room));
        assert!(!fits(&prop, |p| if p[0] == 35 { None } else { room(p) }));
        assert!(!fits(&prop, |p| if p[0] == 35 { Some(0) } else { room(p) }));
        assert!(!fits(&prop, |p| if p[0] == 35 {
            Some(terra_voxel::MaterialId::Leaves as u16)
        } else {
            room(p)
        }));
        let silk = world_point(&prop, Vec3::new(5.1, -1.02, 0.32)).map(|v| v.floor() as i64);
        for obstruction in [3, terra_voxel::MaterialId::FluidWater as u16] {
            assert!(!fits(&prop, |p| if p == silk {
                Some(obstruction)
            } else {
                room(p)
            }));
        }
        let passage = world_point(&prop, Vec3::new(5.0, -1.95, 0.0));
        let blocked = [
            passage[0].floor() as i64,
            (passage[1] - 1.0).floor() as i64,
            passage[2].floor() as i64,
        ];
        assert!(!fits(&prop, |p| if p == blocked {
            Some(3)
        } else {
            room(p)
        }));
        let mut calls = 0;
        assert!(fits(&prop, |p| {
            calls += 1;
            room(p)
        }));
        assert!(calls < 256, "deduplicated world lookups, got {calls}");
    }
}
