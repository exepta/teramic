use super::{WaterPatch, quad};
use crate::water::tongue::{BANDS, Tongue};

pub(super) fn append(water: &mut WaterPatch, shape: &Tongue, base: [f32; 3], flow: [f32; 2]) {
    let start = water.positions.len() as u32;
    let point = |r, c| {
        let p = shape.point(r, c);
        std::array::from_fn(|i| base[i] + p[i])
    };
    let parameter = |r: usize, c: usize, level: f32| {
        [
            r as f32 / BANDS as f32,
            c as f32 / BANDS as f32,
            level,
            crate::water::POUR_SURFACE,
        ]
    };
    for row in 0..=BANDS {
        for col in 0..=BANDS {
            let p = point(row, col);
            water.positions.push(p);
            water.normals.push(shape.normal(row, col));
            water.flow.push(flow);
            water.depth_shore.push([(p[1] - base[1]).max(0.), 0.]);
            water.surface_effects.push(parameter(row, col, 1.));
        }
    }
    let reverse = (shape.front.axis == 0) != shape.front.positive;
    let vertex = |r, c| start + (r * (BANDS + 1) + c) as u32;
    for row in 0..BANDS {
        for col in 0..BANDS {
            let a = vertex(row, col);
            let b = vertex(row, col + 1);
            let c = vertex(row + 1, col + 1);
            let d = vertex(row + 1, col);
            water.indices.extend(if reverse {
                [a, c, b, a, d, c]
            } else {
                [a, b, c, a, c, d]
            });
        }
    }
    let mut wall = |first: (usize, usize), last: (usize, usize), n: [f32; 3]| {
        let a = point(first.0, first.1);
        let b = point(last.0, last.1);
        let bottom = |p: [f32; 3]| [p[0], base[1], p[2]];
        // Top edges wind counterclockwise as seen from outside.
        let edge = [b[0] - a[0], b[2] - a[2]];
        let (a, b, i, j) = if edge[1] * n[0] - edge[0] * n[2] > 0. {
            (a, b, first, last)
        } else {
            (b, a, last, first)
        };
        let start = water.positions.len();
        quad(
            water,
            [a, b, bottom(b), bottom(a)],
            n,
            flow,
            (a[1] + b[1]) * 0.5 - base[1],
            [0.; 4],
        );
        if water.positions.len() == start + 4 {
            water.surface_effects[start..].copy_from_slice(&[
                parameter(i.0, i.1, 1.),
                parameter(j.0, j.1, 1.),
                parameter(j.0, j.1, 0.),
                parameter(i.0, i.1, 0.),
            ]);
        }
    };
    for (i, shown) in shape.exposed.into_iter().enumerate() {
        if shown {
            let mut n = [0.; 3];
            n[if shape.front.axis == 0 { 2 } else { 0 }] = if i == 0 { -1. } else { 1. };
            for row in 0..BANDS {
                wall((row, i * BANDS), (row + 1, i * BANDS), n);
            }
        }
    }
    if shape.tip_exposed && shape.front.tip > 0. {
        let mut n = [0.; 3];
        n[shape.front.axis * 2] = if shape.front.positive { 1. } else { -1. };
        for col in 0..BANDS {
            wall((BANDS, col), (BANDS, col + 1), n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::fluid_surface::Front;

    #[test]
    fn supported_pours_have_a_rounded_contact_line_and_the_sampled_surface_pays_for_its_volume() {
        for axis in [0, 1] {
            for positive in [true, false] {
                for units in 1..8 {
                    let volume = units as f32 / 8.;
                    let front = Front::new(volume, 1., axis, positive).unwrap();
                    let shape = Tongue::on_bed(
                        front,
                        [0; 3],
                        |q| Some(if q[1] < 0 { 1 } else { 0 }),
                        |_| false,
                    )
                    .unwrap();
                    let mut mesh = super::super::empty_patch();
                    append(&mut mesh, &shape, [0.; 3], [0.; 2]);
                    assert_eq!(mesh.positions.len(), mesh.normals.len());
                    let mut paid = 0.;
                    for indices in mesh.indices.chunks_exact(3) {
                        let [a, b, c] = [0, 1, 2].map(|i| mesh.positions[indices[i] as usize]);
                        let area =
                            ((b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2])) * 0.5;
                        if area.abs() < 1e-8 {
                            continue;
                        }
                        assert!(area > 0.);
                        let centroid = [0, 1, 2].map(|i| (a[i] + b[i] + c[i]) / 3.);
                        let sampled = shape.height_at([centroid[0], centroid[2]]).unwrap();
                        assert!((centroid[1] - sampled).abs() < 0.00001);
                        paid += area * centroid[1];
                    }
                    assert!((paid - volume).abs() < 0.00001, "{paid} {volume}");
                    if front.length < 1. {
                        let mid = shape.point(BANDS, BANDS / 2);
                        let edge = shape.point(BANDS, 0);
                        assert!((mid[axis * 2] - edge[axis * 2]).abs() > 0.04);
                        let mut dry = [edge[0], edge[2]];
                        dry[axis] = (mid[axis * 2] + edge[axis * 2]) * 0.5;
                        assert!(shape.height_at(dry).is_none());
                    }
                }
            }
        }
    }
}
