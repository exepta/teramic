use super::{Beam, PartMaterial, Piece, STRAIGHT, valid_facing};
use std::{collections::BTreeMap, sync::OnceLock};

const EPSILON: f64 = 1.0e-10;
const PLANE_SCALE: f64 = 100_000_000.0;

/// A convex, outward-wound visible face from the canonical beam geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub beam: usize,
    pub points: Vec<[f64; 3]>,
    pub normal: [f64; 3],
    pub tangent: [f64; 3],
    pub material: PartMaterial,
}
impl Surface {
    pub fn area(&self) -> f64 {
        polygon_area(&self.points)
    }
}
static SURFACES: [OnceLock<Vec<Surface>>; 80] = [const { OnceLock::new() }; 80];

pub(super) fn local_surfaces(material: u16, facing: u8) -> &'static [Surface] {
    if !valid_facing(material, facing) {
        return &[];
    }
    let index = usize::from(material - STRAIGHT) * 8 + usize::from(facing);
    SURFACES[index].get_or_init(|| {
        let piece = Piece::new([0; 3], material, facing).expect("validated rail pose");
        visible_surfaces(piece.local_geometry())
    })
}

fn raw_surfaces(beams: &[Beam]) -> Vec<Surface> {
    let mut surfaces = Vec::with_capacity(beams.len() * 6);
    for (beam_index, beam) in beams.iter().enumerate() {
        let corners = beam.corners();
        for indices in [
            [0, 4, 6, 2],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 2, 3, 1],
            [4, 5, 7, 6],
        ] {
            let points = indices.map(|index| corners[index]);
            let edge = subtract(points[1], points[0]);
            let normal = normalized(cross(edge, subtract(points[2], points[0])));
            surfaces.push(Surface {
                beam: beam_index,
                points: points.to_vec(),
                normal,
                tangent: normalized(edge),
                material: beam.material,
            });
        }
    }
    surfaces
}

fn plane(surface: &Surface) -> [i64; 4] {
    let normal = surface.normal;
    [
        (normal[0] * PLANE_SCALE).round() as i64,
        (normal[1] * PLANE_SCALE).round() as i64,
        (normal[2] * PLANE_SCALE).round() as i64,
        (dot(normal, surface.points[0]) * PLANE_SCALE).round() as i64,
    ]
}

fn projected_bounds(points: &[[f64; 3]], normal: [f64; 3]) -> [[f64; 2]; 2] {
    let dropped = (0..3)
        .max_by(|&first, &second| normal[first].abs().total_cmp(&normal[second].abs()))
        .unwrap();
    let axes = match dropped {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    std::array::from_fn(|index| {
        let axis = axes[index];
        [
            points
                .iter()
                .map(|point| point[axis])
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point[axis])
                .fold(f64::NEG_INFINITY, f64::max),
        ]
    })
}
fn bounds_overlap(first: [[f64; 2]; 2], second: [[f64; 2]; 2]) -> bool {
    (0..2).all(|axis| {
        first[axis][1].min(second[axis][1]) - first[axis][0].max(second[axis][0]) > EPSILON
    })
}

fn visible_surfaces(beams: &[Beam]) -> Vec<Surface> {
    struct EarlierFace {
        face: Surface,
        bounds: [[f64; 2]; 2],
    }
    let mut planes: BTreeMap<[i64; 4], Vec<EarlierFace>> = BTreeMap::new();
    let mut visible = Vec::new();
    for face in raw_surfaces(beams) {
        let previous = planes.entry(plane(&face)).or_default();
        let bounds = projected_bounds(&face.points, face.normal);
        let mut fragments = vec![face.points.clone()];
        for earlier in previous.iter() {
            if !bounds_overlap(bounds, earlier.bounds) {
                continue;
            }
            let mut uncovered = Vec::new();
            for fragment in fragments {
                if bounds_overlap(projected_bounds(&fragment, face.normal), earlier.bounds) {
                    uncovered.extend(subtract_convex(
                        &fragment,
                        &earlier.face.points,
                        face.normal,
                    ));
                } else {
                    uncovered.push(fragment);
                }
            }
            fragments = uncovered;
            if fragments.is_empty() {
                break;
            }
        }
        for points in fragments {
            if polygon_area(&points) > EPSILON {
                visible.push(Surface {
                    points,
                    ..face.clone()
                });
            }
        }
        previous.push(EarlierFace { face, bounds });
    }
    visible
}

/// Partition a convex subject into disjoint convex pieces outside the clipper.
/// Each emitted part first exits one clip edge, so no two parts overlap.
fn subtract_convex(
    subject: &[[f64; 3]],
    clipper: &[[f64; 3]],
    normal: [f64; 3],
) -> Vec<Vec<[f64; 3]>> {
    let mut remaining = subject.to_vec();
    let mut outside = Vec::new();
    for index in 0..clipper.len() {
        if remaining.len() < 3 {
            break;
        }
        let origin = clipper[index];
        let edge = subtract(clipper[(index + 1) % clipper.len()], origin);
        let inward = cross(normal, edge);
        let (inside, excluded) = split_polygon(&remaining, origin, inward);
        if polygon_area(&excluded) > EPSILON {
            outside.push(excluded);
        }
        remaining = inside;
    }
    outside
}
fn split_polygon(
    points: &[[f64; 3]],
    origin: [f64; 3],
    inward: [f64; 3],
) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        let first_distance = dot(inward, subtract(first, origin));
        let second_distance = dot(inward, subtract(second, origin));
        let first_inside = first_distance >= -EPSILON;
        let second_inside = second_distance >= -EPSILON;
        if first_inside {
            inside.push(first);
        } else {
            outside.push(first);
        }
        if first_inside != second_inside {
            let ratio = (first_distance / (first_distance - second_distance)).clamp(0.0, 1.0);
            let intersection =
                std::array::from_fn(|axis| first[axis] + (second[axis] - first[axis]) * ratio);
            inside.push(intersection);
            outside.push(intersection);
        }
    }
    (inside, outside)
}
fn polygon_area(points: &[[f64; 3]]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    (1..points.len() - 1)
        .map(|index| {
            let product = cross(
                subtract(points[index], points[0]),
                subtract(points[index + 1], points[0]),
            );
            dot(product, product).sqrt() * 0.5
        })
        .sum()
}
fn subtract(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| first[axis] - second[axis])
}
fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    (0..3).map(|axis| first[axis] * second[axis]).sum()
}
fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}
fn normalized(vector: [f64; 3]) -> [f64; 3] {
    let length = dot(vector, vector).sqrt();
    vector.map(|value| value / length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rails::{CROSSING, SWITCH_LEFT, SWITCH_RIGHT};
    fn overlap_area(first: &Surface, second: &Surface) -> f64 {
        if plane(first) != plane(second)
            || !bounds_overlap(
                projected_bounds(&first.points, first.normal),
                projected_bounds(&second.points, first.normal),
            )
        {
            return 0.0;
        }
        let mut intersection = first.points.clone();
        for index in 0..second.points.len() {
            if intersection.len() < 3 {
                return 0.0;
            }
            let origin = second.points[index];
            let inward = cross(
                first.normal,
                subtract(second.points[(index + 1) % second.points.len()], origin),
            );
            intersection = split_polygon(&intersection, origin, inward).0;
        }
        polygon_area(&intersection)
    }
    fn overlaps(faces: &[Surface]) -> f64 {
        let mut groups: BTreeMap<[i64; 4], Vec<&Surface>> = BTreeMap::new();
        let mut overlap = 0.0;
        for face in faces {
            let earlier = groups.entry(plane(face)).or_default();
            for other in earlier.iter() {
                overlap += overlap_area(face, other);
            }
            earlier.push(face);
        }
        overlap
    }
    #[test]
    fn turnout_frogs_have_no_remaining_coplanar_overlap_in_any_material_or_pose() {
        for material in [SWITCH_LEFT, SWITCH_RIGHT] {
            for facing in 0..8 {
                let piece = Piece::new([0; 3], material, facing).unwrap();
                let original = raw_surfaces(piece.local_geometry());
                assert!(
                    overlaps(&original) > 0.001,
                    "the regression must exercise actual intersecting rail heads"
                );
                let visible = piece.local_surfaces();
                assert!(std::ptr::eq(visible, piece.local_surfaces()));
                assert!(
                    overlaps(visible) < 1.0e-8,
                    "material {material} pose {facing}"
                );
                assert!(visible.iter().all(|surface| {
                    surface.area() > EPSILON
                        && surface
                            .points
                            .iter()
                            .flatten()
                            .all(|value| value.is_finite())
                }));
                for surface in visible {
                    let source = original
                        .iter()
                        .find(|face| face.beam == surface.beam && face.normal == surface.normal)
                        .unwrap();
                    for point in &surface.points {
                        assert!(
                            (dot(surface.normal, *point) - dot(surface.normal, source.points[0]))
                                .abs()
                                < 1.0e-8
                        );
                    }
                }
            }
        }
        let crossing = Piece::new([0; 3], CROSSING, 0).unwrap();
        assert!(overlaps(crossing.local_surfaces()) < 1.0e-8);
    }
}
