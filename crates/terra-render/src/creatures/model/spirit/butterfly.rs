use super::{CREAM, INK, Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

// Preserve the seven saved variant IDs while giving each a distinct readable palette.
const PALETTES: [[f32; 3]; 7] = [
    [1.0, 0.42, 0.58],
    [0.96, 0.94, 1.0],
    [0.56, 0.22, 0.85],
    [0.08, 0.51, 1.0],
    [1.0, 0.67, 0.08],
    [0.13, 0.67, 0.40],
    [1.0, 0.26, 0.025],
];

pub(crate) fn butterfly(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let tint = PALETTES[usize::from(variant % 7)];
    let dark = tint.map(|v| v * 0.32);
    let mut body = Builder::default();
    for i in 0..4 {
        shape::rounded(
            &mut body,
            [0.0, 0.105, 0.015 + i as f32 * 0.045],
            [0.085 - i as f32 * 0.01, 0.085, 0.055],
            0.013,
            if i % 2 == 0 { dark } else { tint },
        )?;
    }
    shape::rounded(
        &mut body,
        [0.0, 0.14, -0.09],
        [0.13, 0.12, 0.12],
        0.022,
        tint,
    )?;
    for side in [-1.0, 1.0] {
        body.voxel([side * 0.036, 0.155, -0.154], [0.025, 0.04, 0.013], INK)?;
        body.voxel([side * 0.031, 0.168, -0.163], [0.009, 0.011, 0.006], CREAM)?;
        shape::segment(
            &mut body,
            Vec3::new(side * 0.04, 0.185, -0.08),
            Vec3::new(side * 0.095, 0.33, -0.14),
            0.009,
            0.009,
            dark,
        )?;
        shape::rounded(
            &mut body,
            [side * 0.095, 0.33, -0.14],
            [0.032; 3],
            0.005,
            tint,
        )?;
    }
    let mut out = vec![(Part::Body, Vec3::ZERO, body.finish()?)];
    for side in [-1.0, 1.0] {
        let mut wing = Builder::default();
        // Two closed tapered lobes. Fans have no stacked coplanar strips; inset
        // relief veins/edge gems are separate geometry on both visible sides.
        for (center, radii, start, end) in [
            ([0.23, -0.15], [0.25, 0.25], -0.95, 2.9),
            ([0.19, 0.145], [0.19, 0.18], -2.8, 1.1),
        ] {
            let hub = Vec3::new(side * 0.025, 0.0, 0.015);
            let points: Vec<Vec3> = (0..=12)
                .map(|i| {
                    let a = start + (end - start) * i as f32 / 12.0;
                    Vec3::new(
                        side * (center[0] + a.cos() * radii[0]),
                        0.0,
                        center[1] + a.sin() * radii[1],
                    )
                })
                .collect();
            for pair in points.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                for y in [-0.012, 0.012] {
                    let mut triangle = [hub + Vec3::Y * y, a + Vec3::Y * y, b + Vec3::Y * y];
                    let normal = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]);
                    if normal.y * y < 0.0 {
                        triangle.swap(1, 2);
                    }
                    wing.triangle(triangle, tint)?;
                }
                // Side winding is chosen from the radial direction.
                let mut quad = [
                    a - Vec3::Y * 0.012,
                    b - Vec3::Y * 0.012,
                    b + Vec3::Y * 0.012,
                    a + Vec3::Y * 0.012,
                ];
                if (quad[1] - quad[0])
                    .cross(quad[2] - quad[0])
                    .dot((a + b) * 0.5 - hub)
                    < 0.0
                {
                    quad.reverse();
                }
                wing.quad(quad, dark)?;
            }
            for edge in [points[0], points[12]] {
                let mut cap = [
                    hub - Vec3::Y * 0.012,
                    edge - Vec3::Y * 0.012,
                    edge + Vec3::Y * 0.012,
                    hub + Vec3::Y * 0.012,
                ];
                let inside = (hub + points[6]) * 0.5;
                if (cap[1] - cap[0])
                    .cross(cap[2] - cap[0])
                    .dot(inside - (hub + edge) * 0.5)
                    > 0.0
                {
                    cap.reverse();
                }
                wing.quad(cap, dark)?;

                shape::segment(&mut wing, hub, edge, 0.018, 0.022, dark)?;
            }
            for (i, edge) in points.iter().enumerate() {
                for y in [-0.018, 0.018] {
                    shape::segment(
                        &mut wing,
                        hub + Vec3::Y * y,
                        *edge * 0.80 + Vec3::Y * y,
                        0.008,
                        0.010,
                        if i % 3 == 0 { dark } else { CREAM },
                    )?;
                }
                if i % 2 == 0 {
                    shape::rounded(
                        &mut wing,
                        (*edge * 0.94).to_array(),
                        [0.037, 0.032, 0.037],
                        0.005,
                        if i % 4 == 0 { CREAM } else { tint },
                    )?;
                }
            }
        }
        out.push((
            Part::Wing(side),
            Vec3::new(side * 0.025, 0.14, 0.0),
            wing.finish()?,
        ));
    }
    Ok(out)
}
