//! Clip the actual rounded liquid volume at a horizontal, gravity-aligned level.
use super::Geometry;
use crate::blocks::PrimitiveTemplate;
use bevy::prelude::*;

fn triangle(out: &mut Geometry, points: [Vec3; 3], normals: [Vec3; 3]) {
    if (points[1] - points[0])
        .cross(points[2] - points[0])
        .length_squared()
        < 1e-14
    {
        return;
    }
    let base = out.positions.len() as u32;
    out.positions.extend(points.map(|v| v.to_array()));
    out.normals.extend(normals.map(|v| v.to_array()));
    out.indices.extend([base, base + 1, base + 2]);
}

/// `shape` includes its port caps so each level contour is closed. Caps at
/// connected ports are discarded only after clipping, avoiding duplicate faces.
pub(super) fn clipped(
    shape: &PrimitiveTemplate,
    fraction: f32,
    neighbors: [Option<f32>; 6],
) -> Geometry {
    let mut out = Geometry::default();
    if fraction <= 0. {
        return out;
    }
    let low = shape
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let high = shape
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    let level = low + (high - low) * fraction.clamp(0., 1.);
    let mut edges = Vec::new();
    for face in shape.indices.chunks_exact(3) {
        let points: [(Vec3, Vec3); 3] = std::array::from_fn(|i| {
            let index = face[i] as usize;
            (
                Vec3::from_array(shape.positions[index]),
                Vec3::from_array(shape.normals[index]),
            )
        });
        let mut polygon = Vec::new();
        let mut cut = Vec::new();
        for i in 0..3 {
            let (a, na) = points[i];
            let (b, nb) = points[(i + 1) % 3];
            let inside = a.y <= level;
            if inside {
                polygon.push((a, na));
            }
            if inside != (b.y <= level) {
                let t = (level - a.y) / (b.y - a.y);
                let mut p = a.lerp(b, t);
                p.y = level;
                polygon.push((p, na.lerp(nb, t).normalize_or_zero()));
                cut.push(p);
            }
        }
        if cut.len() == 2 && cut[0].distance_squared(cut[1]) > 1e-12 {
            edges.push([cut[0], cut[1]]);
        }
        if let Some(side) = (0..6).find(|side| {
            let axis = side / 2;
            let plane = (side % 2) as f32;
            points.iter().all(|(p, _)| (p[axis] - plane).abs() < 1e-6)
        }) && let Some(neighbor_level) = neighbors[side]
        {
            if side / 2 == 1 {
                // Only a neighboring liquid that reaches the shared Y plane
                // covers a vertical port. A lower neighbor leaves its bottom rim visible.
                if neighbor_level >= (side % 2) as f32 - 1e-6 {
                    polygon.clear();
                }
            } else {
                let old = std::mem::take(&mut polygon);
                for i in 0..old.len() {
                    let (a, na) = old[i];
                    let (b, nb) = old[(i + 1) % old.len()];
                    let inside = a.y > neighbor_level + 1e-6;
                    if inside {
                        polygon.push((a, na));
                    }
                    if inside != (b.y > neighbor_level + 1e-6) {
                        let t = (neighbor_level - a.y) / (b.y - a.y);
                        let mut point = a.lerp(b, t);
                        point.y = neighbor_level;
                        polygon.push((point, na.lerp(nb, t).normalize_or_zero()));
                    }
                }
            }
        }
        for i in 1..polygon.len().saturating_sub(1) {
            let vertices = [polygon[0], polygon[i], polygon[i + 1]];
            triangle(&mut out, vertices.map(|v| v.0), vertices.map(|v| v.1));
        }
    }
    while let Some(edge) = edges.pop() {
        let mut contour = vec![edge[0], edge[1]];
        while contour.last().unwrap().distance_squared(contour[0]) > 1e-10 {
            let last = *contour.last().unwrap();
            let Some(index) = edges
                .iter()
                .position(|e| e.iter().any(|p| p.distance_squared(last) < 1e-10))
            else {
                break;
            };
            let edge = edges.swap_remove(index);
            contour.push(if edge[0].distance_squared(last) < 1e-10 {
                edge[1]
            } else {
                edge[0]
            });
        }
        if contour.last().unwrap().distance_squared(contour[0]) > 1e-10 {
            continue;
        }
        contour.pop();
        cap(&mut out, contour);
    }
    out
}

fn cross(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (b - a).cross(c - a).y
}

// Ear clipping retains concave branch contours instead of filling their hull.
fn cap(out: &mut Geometry, mut points: Vec<Vec3>) {
    // Mesh-triangle diagonals add collinear contour vertices. Removing them
    // first keeps ear diagonals from passing through another boundary vertex.
    loop {
        if points.len() < 3 {
            return;
        }
        let len = points.len();
        let Some(i) = (0..len).find(|&i| {
            let a = points[(i + len - 1) % len];
            let b = points[i];
            let c = points[(i + 1) % len];
            a.distance_squared(b) < 1e-12
                || b.distance_squared(c) < 1e-12
                || (cross(a, b, c).abs() < 1e-9 && (b - a).dot(b - c) <= 1e-9)
        }) else {
            break;
        };
        points.remove(i);
    }
    let area: f32 = (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            a.z * b.x - a.x * b.z
        })
        .sum();
    if area < 0. {
        points.reverse();
    }
    while points.len() >= 3 {
        let len = points.len();
        let Some(i) = (0..len).find(|&i| {
            let a = points[(i + len - 1) % len];
            let b = points[i];
            let c = points[(i + 1) % len];
            if cross(a, b, c) <= 1e-10 {
                return false;
            }
            !(0..len)
                .filter(|&j| j != i && j != (i + len - 1) % len && j != (i + 1) % len)
                .any(|j| {
                    let p = points[j];
                    cross(a, b, p) >= -1e-9 && cross(b, c, p) >= -1e-9 && cross(c, a, p) >= -1e-9
                })
        }) else {
            break;
        };
        triangle(
            out,
            [
                points[(i + len - 1) % len],
                points[i],
                points[(i + 1) % len],
            ],
            [Vec3::Y; 3],
        );
        points.remove(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meshing::custom::plumbing::pipe;
    #[test]
    fn joined_ports_keep_only_the_vertical_liquid_edge_above_the_neighbor_level() {
        let shape = pipe::contents(3, 3);
        let low = 0.5 - 0.110;
        let lower = low + 0.220 * 0.25;
        let own = low + 0.220 * 0.75;
        let cap_points = |out: &Geometry| {
            out.indices
                .chunks_exact(3)
                .filter(|face| {
                    face.iter()
                        .all(|&i| (out.positions[i as usize][0] - 1.).abs() < 1e-5)
                })
                .flat_map(|face| face.iter().map(|&i| out.positions[i as usize]))
                .collect::<Vec<_>>()
        };
        let mut neighbors = [None; 6];
        neighbors[1] = Some(lower);
        let out = clipped(&shape, 0.75, neighbors);
        let points = cap_points(&out);
        assert!(!points.is_empty());
        assert!(
            points
                .iter()
                .all(|p| p[1] >= lower - 1e-6 && p[1] <= own + 1e-6)
        );
        for level in [own, low + 0.220] {
            neighbors[1] = Some(level);
            assert!(cap_points(&clipped(&shape, 0.75, neighbors)).is_empty());
        }
        let vertical = pipe::contents(12, 12);
        let mut neighbors = [None; 6];
        neighbors[2] = Some(-0.5);
        neighbors[3] = Some(1.5);
        let out = clipped(&vertical, 1., neighbors);
        assert!(
            out.positions
                .iter()
                .zip(&out.normals)
                .any(|(p, n)| p[1].abs() < 1e-6 && n[1] < -0.99)
        );
        assert!(
            !out.positions
                .iter()
                .zip(&out.normals)
                .any(|(p, n)| (p[1] - 1.).abs() < 1e-6 && n[1] > 0.99)
        );
    }

    #[test]
    fn concave_caps_remove_collinear_vertices_without_filling_missing_corners() {
        let points = [
            (0., 0.),
            (1., 0.),
            (2., 0.),
            (2., 0.5),
            (2., 1.),
            (1., 1.),
            (1., 2.),
            (0., 2.),
            (0., 1.),
        ]
        .map(|(x, z)| Vec3::new(x, 0.5, z))
        .to_vec();
        let mut out = Geometry::default();
        cap(&mut out, points);
        let mut area = 0.;
        for face in out.indices.chunks_exact(3) {
            let p: [Vec3; 3] =
                std::array::from_fn(|i| Vec3::from_array(out.positions[face[i] as usize]));
            area += (p[1] - p[0]).cross(p[2] - p[0]).y * 0.5;
            let center = (p[0] + p[1] + p[2]) / 3.;
            assert!(!(center.x > 1. && center.z > 1.));
        }
        assert!((area - 3.).abs() < 1e-6);
    }

    #[test]
    fn stored_levels_shrink_actual_round_volumes_with_upward_free_surfaces() {
        for mask in 1u8..64 {
            if mask.count_ones() < 2 {
                continue;
            }
            let shape = pipe::contents(mask, mask);
            let mut previous = f32::NEG_INFINITY;
            for amount in 1..=8 {
                let out = clipped(&shape, amount as f32 / 8., [None; 6]);
                assert!(!out.indices.is_empty(), "mask {mask}, amount {amount}");
                let high = out
                    .positions
                    .iter()
                    .map(|p| p[1])
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(high > previous, "fill must change height, not opacity");
                previous = high;
                let mut top = 0.;
                let mut boundary = 0.;
                for face in out.indices.chunks_exact(3) {
                    let p: [Vec3; 3] =
                        std::array::from_fn(|i| Vec3::from_array(out.positions[face[i] as usize]));
                    assert!(p.iter().all(|p| p.is_finite()));
                    let cross = (p[1] - p[0]).cross(p[2] - p[0]);
                    assert!(cross.length_squared() > 1e-14);
                    let n = Vec3::from_array(out.normals[face[0] as usize]);
                    assert!(cross.dot(n) > -1e-8);
                    if p.iter().all(|p| (p.y - high).abs() < 1e-7) && n.y > 0.99 {
                        top += cross.y;
                    }
                }
                if amount < 8 {
                    // Independently integrate oriented plane/triangle intersections.
                    // This checks every cap's area, including all disjoint contours,
                    // without depending on contour chaining or the ear triangulator.
                    for face in shape.indices.chunks_exact(3) {
                        let p: [Vec3; 3] = std::array::from_fn(|i| {
                            Vec3::from_array(shape.positions[face[i] as usize])
                        });
                        let n = (p[1] - p[0]).cross(p[2] - p[0]);
                        let mut intersections = Vec::new();
                        for i in 0..3 {
                            let (a, b) = (p[i], p[(i + 1) % 3]);
                            if (a.y <= high) != (b.y <= high) {
                                intersections.push(a.lerp(b, (high - a.y) / (b.y - a.y)));
                            }
                        }
                        if intersections.len() == 2 {
                            let (mut a, mut b) = (intersections[0], intersections[1]);
                            if (b - a).dot(Vec3::Y.cross(n)) < 0. {
                                std::mem::swap(&mut a, &mut b);
                            }
                            boundary += a.z * b.x - a.x * b.z;
                        }
                    }
                    assert!(
                        top > 0.,
                        "missing liquid surface for mask {mask}, amount {amount}"
                    );
                    assert!(
                        (top - boundary.abs()).abs() < 1e-5,
                        "incomplete top cap for mask {mask}, amount {amount}: {top} != {}",
                        boundary.abs()
                    );
                }
            }
        }
    }
}
