use crate::conveyors::{PROFILE_RADIUS, Segment, Span};
use bevy_math::DVec3;
use terra_core::block_shapes::ShapeBox;

pub fn conveyor_casing_supported(chunks: &crate::streaming::ChunkStore, segment: Segment) -> bool {
    conveyor_casing_supported_with(segment, |position| chunks.block_at(position))
}

pub(super) fn conveyor_casing_supported_with(
    segment: Segment,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    if !segment.span.linked || segment.span.step[1] != 0 || segment.span.normal()[1] <= 0.0 {
        return false;
    }
    floor_supported_with(segment.position(), &mut block_at)
}

pub(super) fn floor_supported_with(
    position: [i64; 3],
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> bool {
    let mut below = position;
    let Some(y) = below[1].checked_sub(1) else {
        return false;
    };
    below[1] = y;
    block_at(below).is_some_and(terra_core::beds::supports_bed_floor)
}

pub fn conveyor_casing_boxes(segment: Segment) -> Vec<ShapeBox> {
    let mut local = Vec::with_capacity(19);
    for x in [0.035, 0.835] {
        local.push(([x, 0.27, 0.0], [x + 0.13, 0.36, 1.0]));
        for (near, far) in [(0.0, 0.33), (0.67, 1.0)] {
            local.push(([x, 0.36, near], [x + 0.13, 0.60, far]));
        }
        for z in [0.045, 0.825] {
            local.push(([x + 0.01, 0.0, z], [x + 0.12, 0.30, z + 0.13]));
        }
    }
    local.push(([0.16, 0.325, 0.1], [0.84, 0.35, 0.9]));
    for (near, far) in [(0.125, 0.18), (0.82, 0.875)] {
        for (lo, hi) in [(0.375, 0.389), (0.611, 0.625)] {
            local.push(([near, 0.375, lo], [far, 0.625, hi]));
            local.push(([near, lo, 0.389], [far, hi, 0.611]));
        }
    }
    let position = segment.position();
    local
        .into_iter()
        .map(|(minimum, maximum)| {
            let mut bounds = ShapeBox {
                minimum,
                maximum,
                hinged: false,
            }
            .rotated_y(segment.span.facing);
            for (axis, value) in position.into_iter().enumerate() {
                bounds.minimum[axis] += value as f64;
                bounds.maximum[axis] += value as f64;
            }
            bounds
        })
        .collect()
}

const SLICES: usize = 8;
const HALF_WIDTH: f64 = 0.32;
const THICKNESS: f64 = 0.02;

/// One continuous hollow profile per span, without internal segment or wedge end caps.
pub fn conveyor_triangles(span: Span) -> Vec<[[f64; 3]; 3]> {
    if !span.linked {
        return Vec::new();
    }
    let direction = DVec3::from_array(span.direction().map(f64::from));
    let normal = DVec3::from_array(span.normal().map(f64::from));
    let width = DVec3::from_array(terra_core::kinetic::axis(span.facing).map(|v| v as f64));
    let start = DVec3::from_array(span.start.map(|v| v as f64 + 0.5));
    let end = DVec3::from_array(span.end.map(|v| v as f64 + 0.5));
    let radius = f64::from(PROFILE_RADIUS);
    let mut triangles = Vec::with_capacity(144);
    let mut append = |points: [DVec3; 8]| {
        let center = points.into_iter().sum::<DVec3>() / 8.0;
        for face in [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3]] {
            let mut corners = face.map(|index| points[index]);
            let face_center = corners.into_iter().sum::<DVec3>() / 4.0;
            if (corners[1] - corners[0])
                .cross(corners[2] - corners[0])
                .dot(face_center - center)
                < 0.0
            {
                corners.reverse();
            }
            for indices in [[0, 1, 2], [0, 2, 3]] {
                triangles.push(indices.map(|index| corners[index].to_array()));
            }
        }
    };
    for side in [-1.0, 1.0] {
        append(std::array::from_fn(|corner| {
            let center = if corner & 4 == 0 { start } else { end };
            let radial = if corner & 2 == 0 {
                radius - THICKNESS
            } else {
                radius
            };
            let lateral = if corner & 1 == 0 {
                -HALF_WIDTH
            } else {
                HALF_WIDTH
            };
            center + normal * radial * side + width * lateral
        }));
    }
    for (center, angle_start) in [(start, std::f64::consts::PI), (end, 0.0)] {
        for slice in 0..SLICES {
            append(std::array::from_fn(|corner| {
                let index = slice + usize::from(corner & 4 != 0);
                let angle = angle_start + std::f64::consts::PI * index as f64 / SLICES as f64;
                let radial = if corner & 2 == 0 {
                    radius - THICKNESS
                } else {
                    radius
                };
                let lateral = if corner & 1 == 0 {
                    -HALF_WIDTH
                } else {
                    HALF_WIDTH
                };
                center
                    + direction * radial * angle.sin()
                    + normal * radial * angle.cos()
                    + width * lateral
            }));
        }
    }
    triangles
}

pub(super) fn bare_legacy_conveyor_box(position: [i64; 3], raw_facing: u8) -> ShapeBox {
    let top = f64::from(terra_core::kinetic::CONVEYOR_TOP);
    let mut bounds = ShapeBox {
        minimum: [0.5 - HALF_WIDTH, top - THICKNESS, 0.0],
        maximum: [0.5 + HALF_WIDTH, top, 1.0],
        hinged: false,
    }
    .rotated_y(terra_core::kinetic::belt_facing(raw_facing));
    for (axis, value) in position.into_iter().enumerate() {
        bounds.minimum[axis] += value as f64;
        bounds.maximum[axis] += value as f64;
    }
    bounds
}

/// The same bounded belt profile serves contact, picking and prospective clearance.
pub fn conveyor_boxes(segment: Segment) -> Vec<ShapeBox> {
    let span = segment.span;
    if !span.linked || segment.index < -1 || segment.index > i16::from(span.length) {
        return Vec::new();
    }
    let center = segment.center();
    let direction = span.direction().map(f64::from);
    let normal = span.normal().map(f64::from);
    let width = terra_core::kinetic::axis(span.facing).map(|v| v as f64);
    let step = span
        .step
        .iter()
        .map(|v| (*v as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    let (start, end) = if segment.index == -1 {
        (0.0, step * 0.5)
    } else if segment.index == i16::from(span.length) {
        (-step * 0.5, 0.0)
    } else {
        (-step * 0.5, step * 0.5)
    };
    let mut shapes = Vec::with_capacity(SLICES * 3);
    let mut append = |positions: Vec<[f64; 3]>| {
        let mut shape = ShapeBox {
            minimum: [f64::INFINITY; 3],
            maximum: [f64::NEG_INFINITY; 3],
            hinged: false,
        };
        for p in positions {
            for (axis, coordinate) in p.into_iter().enumerate() {
                shape.minimum[axis] = shape.minimum[axis].min(coordinate);
                shape.maximum[axis] = shape.maximum[axis].max(coordinate);
            }
        }
        shapes.push(shape);
    };
    let radius = f64::from(PROFILE_RADIUS);
    for side in [-1.0, 1.0] {
        for slice in 0..SLICES {
            let lo = start + (end - start) * slice as f64 / SLICES as f64;
            let hi = start + (end - start) * (slice + 1) as f64 / SLICES as f64;
            let mut points = Vec::with_capacity(8);
            for z in [lo, hi] {
                for r in [radius - THICKNESS, radius] {
                    for x in [-HALF_WIDTH, HALF_WIDTH] {
                        points.push(std::array::from_fn(|axis| {
                            center[axis]
                                + direction[axis] * z
                                + normal[axis] * r * side
                                + width[axis] * x
                        }));
                    }
                }
            }
            append(points);
        }
    }
    let arc_start = if segment.index == -1 {
        Some(std::f64::consts::PI)
    } else if segment.index == i16::from(span.length) {
        Some(0.0)
    } else {
        None
    };
    if let Some(start) = arc_start {
        for slice in 0..SLICES {
            let mut points = Vec::with_capacity(8);
            for angle in [
                start + std::f64::consts::PI * slice as f64 / SLICES as f64,
                start + std::f64::consts::PI * (slice + 1) as f64 / SLICES as f64,
            ] {
                for r in [radius - THICKNESS, radius] {
                    for x in [-HALF_WIDTH, HALF_WIDTH] {
                        points.push(std::array::from_fn(|axis| {
                            center[axis]
                                + direction[axis] * r * angle.sin()
                                + normal[axis] * r * angle.cos()
                                + width[axis] * x
                        }));
                    }
                }
            }
            append(points);
        }
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conveyors::Span;

    #[test]
    fn grounded_casings_follow_loaded_full_support_and_leave_the_carrying_run_clear() {
        let span = Span {
            controller: [0, 0, 0],
            first: [0, 0, 0],
            start: [0, 0, -1],
            end: [0, 0, 2],
            step: [0, 0, 1],
            length: 2,
            facing: 0,
            wood: Default::default(),
            linked: true,
        };
        for index in -1..=2 {
            let segment = Segment { span, index };
            let position = segment.position();
            for (floor, supported) in [
                (Some(4), true),
                (Some(0), false),
                (Some(terra_voxel::MaterialId::SolidWater as u16), true),
                (Some(terra_voxel::MaterialId::FluidWater as u16), false),
                (None, false),
            ] {
                assert_eq!(
                    conveyor_casing_supported_with(segment, |cell| {
                        assert_eq!(cell, [position[0], position[1] - 1, position[2]]);
                        floor
                    }),
                    supported
                );
            }
            let boxes = conveyor_casing_boxes(segment);
            assert_eq!(boxes.len(), 19);
            let point = segment.center();
            assert!(
                !boxes.iter().any(|bounds| bounds.intersects(
                    [position[0] as f64, point[1] - 0.01, point[2] - 0.01],
                    [position[0] as f64 + 1.0, point[1] + 0.01, point[2] + 0.01]
                )),
                "the bearing center stays open for real endpoint and guide shafts"
            );
            assert!(!boxes.iter().any(|bounds| bounds.intersects(
                [point[0] - 0.12, 0.625, point[2] - 0.12],
                [point[0] + 0.12, 0.9, point[2] + 0.12]
            )));
            assert!(!conveyor_casing_supported_with(
                Segment {
                    span: Span {
                        step: [0, 1, 1],
                        ..span
                    },
                    index
                },
                |_| Some(4)
            ));
        }
    }

    #[test]
    fn sloped_belts_and_terminal_wraps_share_a_closed_profile_without_filled_centers() {
        for step in [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, -1, -1]] {
            let span = Span {
                controller: [0, 0, 0],
                first: [0, 0, 0],
                start: step.map(|v| -v),
                end: step.map(|v| v * 2),
                step,
                length: 2,
                facing: 0,
                wood: Default::default(),
                linked: true,
            };
            for index in -1..=2 {
                let segment = Segment { span, index };
                let boxes = conveyor_boxes(segment);
                assert_eq!(boxes.len(), if matches!(index, -1 | 2) { 24 } else { 16 });
                let center = segment.center();
                assert!(!boxes.iter().any(|b| b.intersects(
                    center.map(|v| v - 0.001), center.map(|v| v + 0.001)
                )), "the space between the two carrying runs remains empty");
                let upper: [f64; 3] = std::array::from_fn(|axis| {
                    center[axis] + f64::from(span.normal()[axis]) * 0.115
                });
                assert!(
                    boxes
                        .iter()
                        .any(|b| b.intersects(upper.map(|v| v - 0.002), upper.map(|v| v + 0.002)))
                );
            }
        }
    }
}
