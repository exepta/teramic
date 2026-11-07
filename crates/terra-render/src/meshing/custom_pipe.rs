//! Round pipe bodies share the authored material and flange batches.
use crate::blocks::PrimitiveTemplate;
use bevy::math::Vec3;
use std::f32::consts::{FRAC_PI_2, TAU};

const RADIUS: f32 = 0.15;
const BORE: f32 = 0.118;
const SIDES: usize = 16;

struct Body {
    mesh: PrimitiveTemplate,
    radius: f32,
    bore: Option<f32>,
    surround: bool,
}
impl Body {
    fn new(material: usize, radius: f32, bore: Option<f32>) -> Self {
        Self {
            mesh: PrimitiveTemplate {
                material,
                positions: Vec::new(),
                normals: Vec::new(),
                tangents: Vec::new(),
                uvs: Vec::new(),
                indices: Vec::new(),
            },
            radius,
            bore,
            surround: false,
        }
    }
    fn quad(
        &mut self,
        points: [Vec3; 4],
        normals: [Vec3; 4],
        uvs: [[f32; 2]; 4],
        tangent_u: [Vec3; 4],
        tangent_v: [Vec3; 4],
    ) {
        let base = self.mesh.positions.len() as u32;
        self.mesh.positions.extend(points.map(|v| v.to_array()));
        self.mesh.normals.extend(normals.map(|v| v.to_array()));
        self.mesh.tangents.extend((0..4).map(|i| {
            crate::blocks::tangent_frame(
                tangent_u[i],
                normals[i],
                normals[i].cross(tangent_u[i]).dot(tangent_v[i]),
            )
        }));
        self.mesh.uvs.extend(uvs);
        // Sphere poles use a collapsed quad; skip the collapsed triangle.
        for [a, mut b, mut c] in [[0, 1, 2], [0, 2, 3]] {
            let cross = (points[b] - points[a]).cross(points[c] - points[a]);
            if cross.length_squared() < 1e-14 {
                continue;
            }
            if cross.dot(normals[a] + normals[b] + normals[c]) < 0.0 {
                std::mem::swap(&mut b, &mut c);
            }
            self.mesh
                .indices
                .extend([base + a as u32, base + b as u32, base + c as u32]);
        }
    }
    fn planar(&mut self, points: [Vec3; 4], normals: [Vec3; 4], u: Vec3, v: Vec3) {
        // Keep a whole hub patch / annular end in one chart. Selecting a new
        // projection for every quad rotates the grain and its normal map.
        let axis = u.cross(v);
        let derivative = |normal: Vec3, direction: Vec3| {
            direction - axis * (normal.dot(direction) / normal.dot(axis))
        };
        self.quad(
            points,
            normals,
            points.map(|point| [point.dot(u) * 2.0, point.dot(v) * 2.0]),
            normals.map(|normal| derivative(normal, u)),
            normals.map(|normal| derivative(normal, v)),
        );
    }
    fn segment(
        &mut self,
        centers: [Vec3; 2],
        radial: [[Vec3; 2]; 2],
        angular: [[Vec3; 2]; 2],
        tangent: [Vec3; 2],
        distance: [f32; 2],
        side: usize,
    ) {
        // U advances along the centerline; V wraps twice around the tube.
        // A transported ring frame keeps these coordinates continuous across
        // straight-to-bend joins and every individual bend segment.
        let uvs = [
            [distance[0] * 2.0, side as f32 * 2.0 / SIDES as f32],
            [distance[1] * 2.0, side as f32 * 2.0 / SIDES as f32],
            [distance[1] * 2.0, (side + 1) as f32 * 2.0 / SIDES as f32],
            [distance[0] * 2.0, (side + 1) as f32 * 2.0 / SIDES as f32],
        ];
        for (radius, sign) in
            std::iter::once((self.radius, 1.0)).chain(self.bore.map(|bore| (bore, -1.0)))
        {
            self.quad(
                [
                    centers[0] + radial[0][0] * radius,
                    centers[1] + radial[1][0] * radius,
                    centers[1] + radial[1][1] * radius,
                    centers[0] + radial[0][1] * radius,
                ],
                [
                    radial[0][0] * sign,
                    radial[1][0] * sign,
                    radial[1][1] * sign,
                    radial[0][1] * sign,
                ],
                uvs,
                [tangent[0], tangent[1], tangent[1], tangent[0]],
                [angular[0][0], angular[1][0], angular[1][1], angular[0][1]],
            );
        }
    }
    fn path(&mut self, path: &[(Vec3, Vec3)], binormal: Vec3) {
        // Terrain behind glass must not duplicate the opaque fitting's inward
        // wall. Split only straight arms at the authored flange/collar planes;
        // the actual pipe body and its curved centerline remain unchanged.
        let mut split = Vec::new();
        if self.surround {
            for pair in path.windows(2) {
                split.push(pair[0]);
                let delta = pair[1].0 - pair[0].0;
                if pair[0].1.abs_diff_eq(pair[1].1, 1e-6) {
                    let axis = (0..3)
                        .max_by(|&a, &b| delta[a].abs().total_cmp(&delta[b].abs()))
                        .unwrap();
                    let mut parameters: Vec<_> = [0.01, 0.08, 0.20, 0.25, 0.75, 0.80, 0.92, 0.99]
                        .into_iter()
                        .map(|plane| (plane - pair[0].0[axis]) / delta[axis])
                        .filter(|&t| t > 1e-6 && t < 1. - 1e-6)
                        .collect();
                    parameters.sort_by(f32::total_cmp);
                    split.extend(
                        parameters
                            .into_iter()
                            .map(|t| (pair[0].0.lerp(pair[1].0, t), pair[0].1)),
                    );
                }
            }
            split.push(*path.last().unwrap());
        }
        let path = if self.surround {
            split.as_slice()
        } else {
            path
        };
        let forward = (path[1].0 - path[0].0)
            .dot(path[0].1.cross(binormal))
            .signum();
        let mut distance = 0.0;
        for pair in path.windows(2) {
            let end = distance + pair[0].0.distance(pair[1].0);
            if self.surround && pair[0].1.abs_diff_eq(pair[1].1, 1e-6) {
                let delta = pair[1].0 - pair[0].0;
                let axis = (0..3)
                    .max_by(|&a, &b| delta[a].abs().total_cmp(&delta[b].abs()))
                    .unwrap();
                let coordinate = (pair[0].0[axis] + pair[1].0[axis]) * 0.5;
                if [(0.01, 0.08), (0.20, 0.25), (0.75, 0.80), (0.92, 0.99)]
                    .into_iter()
                    .any(|(low, high)| coordinate > low && coordinate < high)
                {
                    distance = end;
                    continue;
                }
            }
            let tangent = [pair[0], pair[1]].map(|(_, normal)| normal.cross(binormal) * forward);
            for side in 0..SIDES {
                let radial = [pair[0], pair[1]].map(|(_, normal)| {
                    [side, side + 1].map(|i| {
                        let angle = i as f32 * TAU / SIDES as f32;
                        normal * angle.cos() + binormal * angle.sin()
                    })
                });
                let angular = [pair[0], pair[1]].map(|(_, normal)| {
                    [side, side + 1].map(|i| {
                        let angle = i as f32 * TAU / SIDES as f32;
                        -normal * angle.sin() + binormal * angle.cos()
                    })
                });
                self.segment(
                    [pair[0].0, pair[1].0],
                    radial,
                    angular,
                    tangent,
                    [distance, end],
                    side,
                );
            }
            distance = end;
        }
    }
}

fn direction(side: usize) -> Vec3 {
    Vec3::from_array(terra_core::plumbing::DIRECTIONS[side].map(|v| v as f32))
}

pub(crate) fn body(mask: u8, material: usize, rims: u8) -> PrimitiveTemplate {
    geometry(mask, material, RADIUS, Some(BORE), rims)
}

/// The network liquid follows exactly the same sweep as its surrounding glass.
/// Connected ports omit internal caps, so adjacent transparent nodes do not
/// receive overlapping coplanar liquid faces at their shared voxel boundary.
pub(crate) fn contents(mask: u8, rims: u8) -> PrimitiveTemplate {
    geometry(mask, 0, BORE - 0.008, None, rims)
}

/// A confirmed moving stream is not a stored cell-sized liquid reservoir.
/// Follow the full sweep so an empty transit buffer cannot break vertical flow.
pub(crate) fn transit_contents(mask: u8, rims: u8) -> PrimitiveTemplate {
    geometry(mask, 0, 0.035, None, rims)
}

/// The opaque surround follows the pipe's outer skin, leaving the existing
/// opaque fittings as the sole visible tunnel wall in their own axial bands.
pub(crate) fn surround_boundary(mask: u8, material: usize) -> PrimitiveTemplate {
    geometry_with_surround(mask, material, RADIUS, None, 0, true)
}

fn geometry(
    mask: u8,
    material: usize,
    radius: f32,
    bore: Option<f32>,
    rims: u8,
) -> PrimitiveTemplate {
    geometry_with_surround(mask, material, radius, bore, rims, false)
}

fn geometry_with_surround(
    mask: u8,
    material: usize,
    radius: f32,
    bore: Option<f32>,
    rims: u8,
    surround: bool,
) -> PrimitiveTemplate {
    let center = Vec3::splat(0.5);
    let ports: Vec<_> = (0..6).filter(|side| mask & (1 << side) != 0).collect();
    let mut body = Body::new(material, radius, bore);
    body.surround = surround;
    if ports.len() == 2 {
        let a = direction(ports[0]);
        let b = direction(ports[1]);
        if ports[0] ^ 1 == ports[1] {
            let normal = direction((ports[0] + 2) % 6);
            body.path(
                &[(center + a * 0.5, normal), (center + b * 0.5, normal)],
                a.cross(normal),
            );
        } else {
            // A quarter-circle bend, tangent to both straight arms.
            let pivot = center + (a + b) * 0.25;
            let mut path = vec![(center + a * 0.5, -b)];
            for step in 0..=8 {
                let angle = step as f32 * FRAC_PI_2 / 8.0;
                let radial = -b * angle.cos() - a * angle.sin();
                path.push((pivot + radial * 0.25, radial));
            }
            path.push((center + b * 0.5, -a));
            body.path(&path, a.cross(b));
        }
    } else {
        // Spherical junction with six matching patches: active faces have a
        // circular opening, inactive faces close the hub. There is no box core
        // and no hidden planar cap across a branch's bore.
        let outer = radius * (0.23 / RADIUS);
        let cut = (outer * outer - radius * radius).sqrt();
        for side in 0..6 {
            let axis = side / 2;
            let outward = direction(side);
            let u = direction(((axis + 1) % 3) * 2 + 1);
            let v = direction(((axis + 2) % 3) * 2 + 1);
            let active = mask & (1 << side) != 0;
            for (sphere_radius, bore, sign) in std::iter::once((outer, radius, 1.0))
                .chain(bore.map(|bore| ((cut * cut + bore * bore).sqrt(), bore, -1.0)))
            {
                for ring in 0..3 {
                    for sector in 0..SIDES {
                        let point = |ring: usize, sector: usize| {
                            let angle = sector as f32 * TAU / SIDES as f32;
                            let (s, c) = angle.sin_cos();
                            let boundary =
                                sphere_radius / (1.0 + c.abs().max(s.abs()).powi(2)).sqrt();
                            let start = if active { bore } else { 0.0 };
                            let radius = start + (boundary - start) * ring as f32 / 3.0;
                            outward * (sphere_radius * sphere_radius - radius * radius).sqrt()
                                + (u * c + v * s) * radius
                        };
                        let points = [
                            point(ring, sector),
                            point(ring + 1, sector),
                            point(ring + 1, sector + 1),
                            point(ring, sector + 1),
                        ];
                        body.planar(
                            points.map(|p| center + p),
                            points.map(|p| p.normalize() * sign),
                            u,
                            v,
                        );
                    }
                }
            }
            if active {
                body.path(
                    &[(center + outward * cut, u), (center + outward * 0.5, u)],
                    v,
                );
            }
        }
    }
    for side in ports.into_iter().filter(|side| rims & (1 << side) != 0) {
        let outward = direction(side);
        let axis = side / 2;
        let u = direction(((axis + 1) % 3) * 2 + 1);
        let v = direction(((axis + 2) % 3) * 2 + 1);
        let end = center + outward * 0.5;
        for sector in 0..SIDES {
            let radial = [sector, sector + 1].map(|i| {
                let angle = i as f32 * TAU / SIDES as f32;
                u * angle.cos() + v * angle.sin()
            });
            body.planar(
                [
                    end + radial[0] * radius,
                    end + radial[1] * radius,
                    end + radial[1] * bore.unwrap_or(0.),
                    end + radial[0] * bore.unwrap_or(0.),
                ],
                [outward; 4],
                u,
                v,
            );
        }
    }
    body.mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_connection_profile_has_finite_round_watertight_port_rims_and_valid_winding() {
        for mask in 1u8..64 {
            if mask.count_ones() < 2 {
                continue;
            }
            let model = body(mask, 7, mask);
            assert_eq!(model.material, 7);
            assert!(
                model.positions.len() <= 4000,
                "mask {mask}: {}",
                model.positions.len()
            );
            assert_eq!(model.positions.len(), model.normals.len());
            assert_eq!(model.positions.len(), model.uvs.len());
            assert_eq!(model.positions.len(), model.tangents.len());
            for (normal, tangent) in model.normals.iter().zip(&model.tangents) {
                let direction = Vec3::from_slice(&tangent[..3]);
                assert!(direction.is_finite());
                assert!((direction.length_squared() - 1.0).abs() < 1e-4);
                assert!(direction.dot(Vec3::from_array(*normal)).abs() < 1e-4);
            }
            assert!(
                model
                    .positions
                    .iter()
                    .flatten()
                    .all(|p| p.is_finite() && (-0.0001..=1.0001).contains(p))
            );
            let mut edges = std::collections::BTreeMap::new();
            for triangle in model.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn::<_, 3, _>(|i| {
                    Vec3::from_array(model.positions[triangle[i] as usize])
                });
                let key =
                    |i: u32| model.positions[i as usize].map(|v| (v * 100_000.0).round() as i32);
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let mut edge = [key(triangle[a]), key(triangle[b])];
                    edge.sort();
                    *edges.entry(edge).or_insert(0usize) += 1;
                }
                let n = triangle
                    .iter()
                    .map(|&i| Vec3::from_array(model.normals[i as usize]))
                    .sum::<Vec3>();
                assert!((b - a).cross(c - a).dot(n) > 0.0, "mask {mask}");
            }
            assert!(
                edges.values().all(|&count| count == 2),
                "open body seam in mask {mask}"
            );
            for side in 0..6 {
                let axis = side / 2;
                let end = (side % 2) as f32;
                let points: Vec<_> = model
                    .positions
                    .iter()
                    .filter(|p| (p[axis] - end).abs() < 1e-5)
                    .collect();
                assert_eq!(
                    !points.is_empty(),
                    mask & (1 << side) != 0,
                    "mask {mask}, side {side}"
                );
                for point in points {
                    let delta = Vec3::from_array(*point) - Vec3::splat(0.5) - direction(side) * 0.5;
                    assert!(
                        (delta.length() - RADIUS).abs() < 1e-5
                            || (delta.length() - BORE).abs() < 1e-5
                    );
                }
            }
        }
    }

    #[test]
    fn every_body_chart_has_non_degenerate_uvs_and_matching_tangent_handedness() {
        use bevy::math::Vec2;
        for mask in 1u8..64 {
            if mask.count_ones() < 2 {
                continue;
            }
            let mesh = body(mask, 0, mask);
            for face in mesh.indices.chunks_exact(3) {
                let points = [face[0], face[1], face[2]]
                    .map(|i| Vec3::from_array(mesh.positions[i as usize]));
                let uv =
                    [face[0], face[1], face[2]].map(|i| Vec2::from_array(mesh.uvs[i as usize]));
                let [a, b] = [uv[1] - uv[0], uv[2] - uv[0]];
                let determinant = a.perp_dot(b);
                assert!(determinant.abs() > 1e-9, "collapsed UV, mask {mask}");
                let [p, q] = [points[1] - points[0], points[2] - points[0]];
                let du = (p * b.y - q * a.y) / determinant;
                let dv = (q * a.x - p * b.x) / determinant;
                for &index in face {
                    let n = Vec3::from_array(mesh.normals[index as usize]);
                    let t = mesh.tangents[index as usize];
                    let tangent = Vec3::new(t[0], t[1], t[2]);
                    // Derivatives are from actual triangles, independently of
                    // the analytic sweep frames used to construct the mesh.
                    let projected_u = (du - n * n.dot(du)).normalize();
                    let projected_v =
                        (dv - n * n.dot(dv) - projected_u * projected_u.dot(dv)).normalize();
                    assert!(tangent.dot(projected_u) > 0.96, "UV tangent, mask {mask}");
                    assert!(
                        (n.cross(tangent) * t[3]).dot(projected_v) > 0.96,
                        "UV handedness, mask {mask}"
                    );
                }
            }
        }
    }

    #[test]
    fn straight_to_bend_uvs_and_normal_frames_are_continuous_inside_and_outside() {
        use std::collections::BTreeMap;
        for first in 0..6 {
            for second in first + 1..6 {
                let mask = (1 << first) | (1 << second);
                let mesh = body(mask, 0, 0);
                let mut shared: BTreeMap<_, ([f32; 2], [f32; 4])> = BTreeMap::new();
                let mut duplicates = 0;
                for i in 0..mesh.positions.len() {
                    let key = (
                        mesh.positions[i].map(|v| (v * 100_000.).round() as i32),
                        mesh.normals[i].map(|v| (v * 100_000.).round() as i32),
                    );
                    let uv = mesh.uvs[i];
                    let tangent = mesh.tangents[i];
                    if let Some(&(old_uv, old_tangent)) = shared.get(&key) {
                        duplicates += 1;
                        for axis in 0..2 {
                            let delta: f32 = uv[axis] - old_uv[axis];
                            assert!(
                                (delta - delta.round()).abs() < 1e-5,
                                "texture seam at {key:?}, mask {mask}"
                            );
                        }
                        for axis in 0..4 {
                            assert!(
                                (tangent[axis] - old_tangent[axis]).abs() < 1e-5,
                                "normal-map seam at {key:?}, mask {mask}"
                            );
                        }
                    } else {
                        shared.insert(key, (uv, tangent));
                    }
                }
                assert!(
                    duplicates > SIDES,
                    "both tube walls have shared chart edges"
                );
            }
        }
    }

    #[test]
    fn terrain_tunnel_omits_exactly_the_opaque_fitting_bands_on_straight_runs() {
        for axis in 0..3 {
            let mesh = surround_boundary(3 << (axis * 2), 4);
            assert!(!mesh.indices.is_empty());
            for face in mesh.indices.chunks_exact(3) {
                let coordinate = face
                    .iter()
                    .map(|&i| mesh.positions[i as usize][axis])
                    .sum::<f32>()
                    / 3.;
                assert!(
                    ![(0.01, 0.08), (0.20, 0.25), (0.75, 0.80), (0.92, 0.99)]
                        .into_iter()
                        .any(|(a, b)| coordinate > a + 1e-6 && coordinate < b - 1e-6)
                );
            }
            for end in [0., 1.] {
                let points: Vec<_> = mesh
                    .positions
                    .iter()
                    .filter(|p| (p[axis] - end).abs() < 1e-6)
                    .collect();
                assert!(!points.is_empty());
                for point in points {
                    let mut radial = Vec3::from_array(*point) - Vec3::splat(0.5);
                    radial[axis] = 0.;
                    assert!((radial.length() - RADIUS).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn connected_tubes_have_no_shared_planar_rings_but_free_ends_remain_open() {
        for axis in 0..3 {
            let mask = 3 << (axis * 2);
            let connected = body(mask, 0, 0);
            for triangle in connected.indices.chunks_exact(3) {
                let points = [triangle[0], triangle[1], triangle[2]]
                    .map(|i| connected.positions[i as usize]);
                assert!(
                    ![0., 1.]
                        .into_iter()
                        .any(|end| { points.iter().all(|p| (p[axis] - end).abs() < 1e-5) })
                );
            }
            let open_side = axis * 2 + 1;
            let exposed = body(mask, 0, 1 << open_side);
            let planar: Vec<_> = exposed
                .indices
                .chunks_exact(3)
                .filter(|triangle| {
                    triangle
                        .iter()
                        .all(|&i| (exposed.positions[i as usize][axis] - 1.).abs() < 1e-5)
                })
                .collect();
            assert_eq!(planar.len(), SIDES * 2);
            for triangle in planar {
                for &index in triangle {
                    let mut radial =
                        Vec3::from_array(exposed.positions[index as usize]) - Vec3::splat(0.5);
                    radial[axis] = 0.;
                    assert!(radial.length() >= BORE - 1e-5);
                }
            }
        }
    }

    #[test]
    fn glass_contents_follow_all_bends_and_branches_with_finite_outward_triangles() {
        for mask in 1u8..64 {
            if mask.count_ones() < 2 {
                continue;
            }
            let mesh = contents(mask, mask);
            assert!(!mesh.indices.is_empty() && mesh.positions.len() < 4000);
            assert!(
                mesh.positions
                    .iter()
                    .flatten()
                    .all(|p| p.is_finite() && (-1e-5..=1.00001).contains(p))
            );
            for triangle in mesh.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn::<_, 3, _>(|i| {
                    Vec3::from_array(mesh.positions[triangle[i] as usize])
                });
                let n = triangle
                    .iter()
                    .map(|&i| Vec3::from_array(mesh.normals[i as usize]))
                    .sum::<Vec3>();
                assert!((b - a).cross(c - a).dot(n) > 0., "mask {mask}");
            }
            for side in 0..6 {
                if mask & (1 << side) == 0 {
                    continue;
                }
                let axis = side / 2;
                let end = (side % 2) as f32;
                for point in mesh
                    .positions
                    .iter()
                    .filter(|p| (p[axis] - end).abs() < 1e-5)
                {
                    let mut radial = Vec3::from_array(*point) - Vec3::splat(0.5);
                    radial[axis] = 0.;
                    assert!(radial.length() < BORE);
                }
            }
        }
    }

    #[test]
    fn straight_body_is_a_continuous_cylinder_without_a_box_or_bulb_in_the_middle() {
        for axis in 0..3 {
            let model = body(3 << (axis * 2), 0, 3 << (axis * 2));
            for point in model.positions {
                let mut radial = Vec3::from_array(point) - Vec3::splat(0.5);
                radial[axis] = 0.0;
                assert!(
                    (radial.length() - RADIUS).abs() < 1e-5
                        || (radial.length() - BORE).abs() < 1e-5
                );
            }
        }
    }
}
