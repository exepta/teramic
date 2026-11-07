//! Clip liquid faces against the same faceted outer sweep as the visible pipe.
//! Collision boxes remain deliberately simpler and must never cut square holes
//! into the rendered lake. The bore belongs to the separate hydraulic network.
use bevy_math::{DMat3, DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, TAU};

const RADIUS: f64 = 0.15;
const BORE: f64 = 0.118;
const SIDES: usize = 16;
const EPS: f64 = 1e-8;

mod fixtures;
pub(super) use fixtures::fragments as fixture_fragments;

fn direction(side: usize) -> DVec3 {
    DVec3::from_array(terra_core::plumbing::DIRECTIONS[side].map(|v| v as f64))
}

fn hull(mut points: Vec<DVec2>) -> Vec<DVec2> {
    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.distance_squared(*b) < EPS * EPS);
    if points.len() < 3 {
        return vec![];
    }
    let mut result: Vec<DVec2> = Vec::with_capacity(points.len() * 2);
    for point in &points {
        while result.len() >= 2
            && (result[result.len() - 1] - result[result.len() - 2])
                .perp_dot(*point - result[result.len() - 1])
                <= EPS * EPS
        {
            result.pop();
        }
        result.push(*point);
    }
    let lower = result.len();
    for point in points[..points.len() - 1].iter().rev() {
        while result.len() > lower
            && (result[result.len() - 1] - result[result.len() - 2])
                .perp_dot(*point - result[result.len() - 1])
                <= EPS * EPS
        {
            result.pop();
        }
        result.push(*point);
    }
    result.pop();
    result
}

struct Slice {
    axis: usize,
    plane: f64,
    tangents: [usize; 2],
    cuts: Vec<Vec<DVec2>>,
    rotation: DMat3,
}
impl Slice {
    fn project(&self, p: DVec3) -> DVec2 {
        DVec2::new(p[self.tangents[0]], p[self.tangents[1]])
    }

    fn prism<const N: usize>(&mut self, rings: [[DVec3; N]; 2]) {
        let rings = rings
            .map(|ring| ring.map(|p| DVec3::splat(0.5) + self.rotation * (p - DVec3::splat(0.5))));
        let minimum = rings
            .iter()
            .flatten()
            .map(|p| p[self.axis])
            .fold(f64::INFINITY, f64::min);
        let maximum = rings
            .iter()
            .flatten()
            .map(|p| p[self.axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if self.plane < minimum - EPS || self.plane > maximum + EPS {
            return;
        }
        let mut intersections = Vec::with_capacity(N * 4);
        for i in 0..N {
            let next = (i + 1) % N;
            for [a, b] in [
                [rings[0][i], rings[0][next]],
                [rings[1][i], rings[1][next]],
                [rings[0][i], rings[1][i]],
                [rings[0][i], rings[1][next]],
            ] {
                let da = a[self.axis] - self.plane;
                let db = b[self.axis] - self.plane;
                if da.abs() <= EPS {
                    intersections.push(self.project(a));
                }
                if db.abs() <= EPS {
                    intersections.push(self.project(b));
                }
                if da * db < 0.0 {
                    intersections.push(self.project(a.lerp(b, da / (da - db))));
                }
            }
        }
        let polygon = hull(intersections);
        if polygon.len() >= 3 {
            self.cuts.push(polygon);
        }
    }

    fn cylinder(&mut self, axis: usize, start: f64, end: f64, radius: f64, sides: usize) {
        self.cylinder_at(axis, start, end, radius, sides, DVec3::splat(0.5));
    }

    fn cylinder_at(
        &mut self,
        axis: usize,
        start: f64,
        end: f64,
        radius: f64,
        sides: usize,
        center: DVec3,
    ) {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let rings: [[DVec3; SIDES]; 2] = [start, end].map(|t| {
            std::array::from_fn(|i| {
                let angle = (i * sides / SIDES) as f64 * TAU / sides as f64;
                let mut p = center;
                p[axis] = t;
                p[u] += angle.cos() * radius;
                p[v] += angle.sin() * radius;
                p
            })
        });
        self.prism(rings);
    }

    fn sweep(&mut self, path: &[(DVec3, DVec3)], binormal: DVec3) {
        for pair in path.windows(2) {
            let rings: [[DVec3; SIDES]; 2] = [pair[0], pair[1]].map(|(center, normal)| {
                std::array::from_fn(|i| {
                    let angle = i as f64 * TAU / SIDES as f64;
                    center + (normal * angle.cos() + binormal * angle.sin()) * RADIUS
                })
            });
            self.prism(rings);
        }
    }

    fn tetrahedron(&mut self, triangle: [DVec3; 3]) {
        let points = [DVec3::splat(0.5), triangle[0], triangle[1], triangle[2]];
        let minimum = points
            .iter()
            .map(|p| p[self.axis])
            .fold(f64::INFINITY, f64::min);
        let maximum = points
            .iter()
            .map(|p| p[self.axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if self.plane < minimum - EPS || self.plane > maximum + EPS {
            return;
        }
        let mut intersections = Vec::with_capacity(8);
        for a in 0..4 {
            for b in a + 1..4 {
                let a = points[a];
                let b = points[b];
                let da = a[self.axis] - self.plane;
                let db = b[self.axis] - self.plane;
                if da.abs() <= EPS {
                    intersections.push(self.project(a));
                }
                if db.abs() <= EPS {
                    intersections.push(self.project(b));
                }
                if da * db < 0. {
                    intersections.push(self.project(a.lerp(b, da / (da - db))));
                }
            }
        }
        let polygon = hull(intersections);
        if polygon.len() >= 3 {
            self.cuts.push(polygon);
        }
    }

    fn hub(&mut self, mask: u8) {
        // The actual three-ring patch mesh has small concave facets. Subtract
        // its tetrahedra separately; a global convex hull or ideal sphere would
        // remove visible lake water beyond those facets and leave a dry slit.
        for side in 0..6 {
            let outward = direction(side);
            let u = direction(((side / 2 + 1) % 3) * 2 + 1);
            let v = direction(((side / 2 + 2) % 3) * 2 + 1);
            let active = mask & (1 << side) != 0;
            let point = |ring: usize, sector: usize| {
                let angle = sector as f64 * TAU / SIDES as f64;
                let (s, c) = angle.sin_cos();
                let boundary = 0.23 / (1. + c.abs().max(s.abs()).powi(2)).sqrt();
                let start = if active { RADIUS } else { 0. };
                let radius = start + (boundary - start) * ring as f64 / 3.;
                DVec3::splat(0.5)
                    + outward * (0.23_f64.powi(2) - radius * radius).sqrt()
                    + (u * c + v * s) * radius
            };
            for ring in 0..3 {
                for sector in 0..SIDES {
                    let quad = [
                        point(ring, sector),
                        point(ring + 1, sector),
                        point(ring + 1, sector + 1),
                        point(ring, sector + 1),
                    ];
                    for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                        self.tetrahedron([quad[a], quad[b], quad[c]]);
                    }
                }
            }
            if active {
                let cap = DVec3::splat(0.5) + outward * (0.23_f64.powi(2) - RADIUS * RADIUS).sqrt();
                for sector in 0..SIDES {
                    self.tetrahedron([cap, point(0, sector), point(0, sector + 1)]);
                }
            }
        }
    }

    fn body(&mut self, mask: u8) {
        let center = DVec3::splat(0.5);
        let ports: Vec<_> = (0..6).filter(|side| mask & (1 << side) != 0).collect();
        if ports.len() == 2 {
            let a = direction(ports[0]);
            let b = direction(ports[1]);
            if ports[0] ^ 1 == ports[1] {
                let normal = direction((ports[0] + 2) % 6);
                self.sweep(
                    &[(center + a * 0.5, normal), (center + b * 0.5, normal)],
                    a.cross(normal),
                );
            } else {
                let pivot = center + (a + b) * 0.25;
                let mut path = vec![(center + a * 0.5, -b)];
                for step in 0..=8 {
                    let angle = step as f64 * FRAC_PI_2 / 8.0;
                    let radial = -b * angle.cos() - a * angle.sin();
                    path.push((pivot + radial * 0.25, radial));
                }
                path.push((center + b * 0.5, -a));
                self.sweep(&path, a.cross(b));
            }
        } else {
            self.hub(mask);
            let cut = (0.23_f64.powi(2) - RADIUS * RADIUS).sqrt();
            for &side in &ports {
                let outward = direction(side);
                let normal = direction(((side / 2 + 1) % 3) * 2 + 1);
                let binormal = direction(((side / 2 + 2) % 3) * 2 + 1);
                self.sweep(
                    &[
                        (center + outward * cut, normal),
                        (center + outward * 0.5, normal),
                    ],
                    binormal,
                );
            }
        }
        for side in ports {
            let axis = side / 2;
            let positive = side % 2 == 1;
            let t = if positive { 0.955 } else { 0.045 };
            self.cylinder(axis, t - 0.035, t + 0.035, 0.215, 8);
            self.cylinder(
                axis,
                if positive { 0.75 } else { 0.20 },
                if positive { 0.80 } else { 0.25 },
                0.163,
                8,
            );
            for bolt in 0..4 {
                let angle = (bolt as f64 + 0.5) * TAU / 4.0;
                let mut bolt_center = center;
                bolt_center[axis] = t + if positive { -0.04 } else { 0.04 };
                bolt_center[(axis + 1) % 3] += angle.cos() * 0.215 * 0.82;
                bolt_center[(axis + 2) % 3] += angle.sin() * 0.215 * 0.82;
                let mut size = DVec3::splat(0.043);
                size[axis] = 0.027;
                if (self.plane - bolt_center[self.axis]).abs() <= size[self.axis] * 0.5 + EPS {
                    self.cuts.push(
                        [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
                            .map(|corner| {
                                let p = self.project(bolt_center);
                                p + DVec2::new(
                                    (corner[0] - 0.5) * size[self.tangents[0]],
                                    (corner[1] - 0.5) * size[self.tangents[1]],
                                )
                            })
                            .to_vec(),
                    );
                }
            }
        }
    }

    fn mouth(&mut self, radius: f64, bore: f64, sides: usize) {
        // At a dry pipe's open end only the wall rim touches the lake. Keeping
        // water in the open bore avoids a hole when looking into the intake.
        for i in 0..sides {
            let radial = [i, i + 1].map(|i| {
                let angle = i as f64 * TAU / sides as f64;
                DVec2::new(angle.cos(), angle.sin())
            });
            self.cuts.push(hull(vec![
                DVec2::splat(0.5) + radial[0] * radius,
                DVec2::splat(0.5) + radial[1] * radius,
                DVec2::splat(0.5) + radial[1] * bore,
                DVec2::splat(0.5) + radial[0] * bore,
            ]));
        }
    }
}

fn clip(
    polygon: &[[f32; 3]],
    signed: impl Fn([f32; 3]) -> f64,
    inside: bool,
    tangents: [usize; 2],
) -> Vec<[f32; 3]> {
    let mut output = Vec::new();
    let mut previous = *polygon.last().unwrap();
    let mut da = signed(previous);
    for &current in polygon {
        let db = signed(current);
        if (da >= 0.) != (db >= 0.) {
            let t = (da / (da - db)) as f32;
            output.push(std::array::from_fn(|i| {
                previous[i] + t * (current[i] - previous[i])
            }));
        }
        if (db >= 0.) == inside {
            output.push(current);
        }
        previous = current;
        da = db;
    }
    let project = |p: [f32; 3]| DVec2::new(f64::from(p[tangents[0]]), f64::from(p[tangents[1]]));
    let anchor = project(polygon[0]);
    let signed_area: f64 = polygon[1..]
        .windows(2)
        .map(|p| (project(p[0]) - anchor).perp_dot(project(p[1]) - anchor))
        .sum();
    // Every half-plane fragment is convex. Restore that exact topology after
    // f32 quantization so rounded duplicate/collinear corners cannot produce
    // empty or inward fan triangles, especially at negative section borders.
    let points = hull(output.into_iter().map(project).collect());
    let mut output: Vec<_> = points
        .into_iter()
        .map(|point| {
            let mut p = polygon[0];
            p[tangents[0]] = point.x as f32;
            p[tangents[1]] = point.y as f32;
            p
        })
        .collect();
    if signed_area < 0. {
        output.reverse();
    }
    output
}

fn has_area(polygon: &[[f32; 3]], tangents: [usize; 2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let point = |p: [f32; 3]| DVec2::new(f64::from(p[tangents[0]]), f64::from(p[tangents[1]]));
    let anchor = point(polygon[0]);
    polygon[1..]
        .windows(2)
        .any(|p| (point(p[0]) - anchor).perp_dot(point(p[1]) - anchor).abs() > 1e-10)
}

/// Existing face winding survives every subtraction. No offset water sheets or
/// faces against the pipe's interior are introduced.
pub(super) fn fragments(
    visible: Vec<Vec<[f32; 3]>>,
    face: [[f32; 3]; 4],
    normal: [f32; 3],
    pipe_origin: [f32; 3],
    mask: u8,
    own_cell: bool,
) -> Vec<Vec<[f32; 3]>> {
    let axis = normal.iter().position(|v| *v != 0.).unwrap();
    let plane = f64::from(face[0][axis] - pipe_origin[axis]);
    if !(-EPS..=1. + EPS).contains(&plane) || visible.is_empty() {
        return visible;
    }
    let tangents: Vec<_> = (0..3).filter(|i| *i != axis).collect();
    let tangents = [tangents[0], tangents[1]];
    let mut slice = Slice {
        axis,
        plane,
        tangents,
        cuts: vec![],
        rotation: DMat3::IDENTITY,
    };
    let end = if plane.abs() < EPS {
        Some(axis * 2)
    } else if (plane - 1.).abs() < EPS {
        Some(axis * 2 + 1)
    } else {
        None
    };
    if !own_cell && end.is_some_and(|side| mask & (1 << side) != 0) {
        slice.mouth(RADIUS, BORE, SIDES);
    } else {
        slice.body(mask);
    }
    subtract(visible, slice.cuts, tangents, pipe_origin)
}

fn subtract(
    mut visible: Vec<Vec<[f32; 3]>>,
    cuts: Vec<Vec<DVec2>>,
    tangents: [usize; 2],
    pipe_origin: [f32; 3],
) -> Vec<Vec<[f32; 3]>> {
    for cut in cuts {
        let mut remaining = Vec::new();
        for polygon in visible {
            let mut inside = polygon;
            for i in 0..cut.len() {
                if !has_area(&inside, tangents) {
                    break;
                }
                let a = cut[i];
                let edge = cut[(i + 1) % cut.len()] - a;
                let signed = |p: [f32; 3]| {
                    edge.perp_dot(
                        DVec2::new(
                            f64::from(p[tangents[0]] - pipe_origin[tangents[0]]),
                            f64::from(p[tangents[1]] - pipe_origin[tangents[1]]),
                        ) - a,
                    )
                };
                let outside = clip(&inside, signed, false, tangents);
                if has_area(&outside, tangents) {
                    remaining.push(outside);
                }
                inside = clip(&inside, signed, true, tangents);
            }
        }
        visible = remaining;
        if visible.is_empty() {
            break;
        }
    }
    visible
}

#[cfg(test)]
#[path = "pipes/tests.rs"]
mod tests;
