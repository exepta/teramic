//! Sixteen coats share a bounded eight-part silhouette and the common creature material.
use super::{Part, shape};
use crate::players::model::Builder;
use bevy::prelude::*;

fn coat(variant: u8, p: [f32; 3]) -> [f32; 3] {
    let base: [f32; 3] = [
        [0.38, 0.15, 0.065],
        [0.055, 0.065, 0.075],
        [0.61, 0.26, 0.095],
        [0.91, 0.90, 0.85],
        [0.48, 0.51, 0.53],
        [0.67, 0.48, 0.24],
        [0.78, 0.62, 0.32],
        [0.45, 0.32, 0.28],
        [0.055, 0.065, 0.075],
        [0.61, 0.26, 0.095],
        [0.40, 0.18, 0.085],
        [0.055, 0.065, 0.075],
        [0.88, 0.86, 0.76],
        [0.65, 0.68, 0.68],
        [0.89, 0.82, 0.61],
        [0.29, 0.13, 0.055],
    ][usize::from(variant.min(15))];
    let cell = p.map(|v| (v * 9.0).floor() as i32);
    let hash = cell[0].wrapping_mul(73_856_093)
        ^ cell[1].wrapping_mul(19_349_663)
        ^ cell[2].wrapping_mul(83_492_791);
    let patch = (p[2] * 5.4 + p[1] * 2.8).sin() + (p[1] * 7.5 - p[0] * 4.0).cos() * 0.65;
    if matches!(variant, 10 | 11) && patch > 0.15 && p[2] > -0.58
        || matches!(variant, 2 | 3 | 6 | 8 | 9 | 10 | 11) && p[1] < 0.29
    {
        return [0.91, 0.90, 0.84];
    }
    if variant == 12 && hash.rem_euclid(7) < 2 {
        return [0.22, 0.12, 0.08];
    }
    let tint = if variant == 4 && hash.rem_euclid(5) < 2 {
        1.33
    } else if variant == 7 && hash.rem_euclid(4) == 0 {
        1.40
    } else {
        0.95 + hash.rem_euclid(9) as f32 * 0.012
    };
    base.map(|v| (v * tint).min(1.0))
}

fn covered(
    builder: &mut Builder,
    center: [f32; 3],
    size: [f32; 3],
    variant: u8,
    world: [f32; 3],
) -> Result<(), String> {
    shape::rounded(builder, center, size, 0.035, coat(variant, world))
}

pub(super) fn parts(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    // One continuous barrel: surface coat tiles have no gaps or repeated ribs.
    covered(
        &mut body,
        [0.0, 1.035, 0.0],
        [0.483, 0.65, 1.30],
        variant,
        [0.0, 1.0, 0.0],
    )?;
    for side in [-1.0, 1.0] {
        for row in 0..4 {
            for column in 0..10 {
                let y = 0.81 + row as f32 * 0.15;
                let z = -0.585 + column as f32 * 0.13;
                let p = [side * 0.2436, y, z];
                let highlight = coat(variant, p);
                body.voxel(p, [0.012, 0.15, 0.13], highlight)?;
            }
        }
    }
    covered(
        &mut body,
        [0.0, 1.10, 0.48],
        [0.5145, 0.55, 0.32],
        variant,
        [0.3, 1.1, 0.48],
    )?;
    covered(
        &mut body,
        [0.0, 1.10, -0.48],
        [0.497, 0.58, 0.31],
        variant,
        [0.3, 1.1, -0.48],
    )?;
    for i in 0..5 {
        let y = 1.20 + i as f32 * 0.10;
        let z = -0.47 - i as f32 * 0.055;
        covered(
            &mut body,
            [0.0, y, z],
            [0.39 - i as f32 * 0.025, 0.16, 0.33],
            variant,
            [0.0, y, z],
        )?;
    }
    let mane = if matches!(variant, 3 | 6 | 14) {
        [0.81, 0.75, 0.58]
    } else if matches!(variant, 2 | 9) {
        [0.34, 0.10, 0.035]
    } else {
        [0.065, 0.045, 0.032]
    };
    for i in 0..7 {
        for side in [-1.0, 1.0] {
            body.voxel(
                [
                    side * 0.16,
                    1.28 + i as f32 * 0.076,
                    -0.29 - i as f32 * 0.058,
                ],
                [0.085, 0.14, 0.115],
                mane,
            )?;
        }
    }
    let mut head = Builder::default();
    covered(
        &mut head,
        [0.0, 0.02, -0.04],
        [0.30, 0.37, 0.34],
        variant,
        [0.0, 1.60, -0.72],
    )?;
    covered(
        &mut head,
        [0.0, -0.105, -0.24],
        [0.26, 0.24, 0.32],
        variant,
        [0.0, 1.47, -0.91],
    )?;
    shape::rounded(
        &mut head,
        [0.0, -0.12, -0.39],
        [0.27, 0.19, 0.13],
        0.024,
        if matches!(variant, 0..=4 | 7..=14) {
            [0.83, 0.68, 0.55]
        } else {
            coat(variant, [0.0, 1.44, -1.02])
        },
    )?;
    for side in [-1.0, 1.0] {
        covered(
            &mut head,
            [side * 0.10, 0.265, 0.01],
            [0.092, 0.24, 0.10],
            variant,
            [side * 0.10, 1.81, -0.60],
        )?;
        head.voxel(
            [side * 0.102, 0.27, -0.043],
            [0.047, 0.14, 0.013],
            [0.39, 0.25, 0.22],
        )?;
        head.voxel(
            [side * 0.153, 0.055, -0.12],
            [0.016, 0.073, 0.085],
            [0.085, 0.063, 0.043],
        )?;
        head.voxel(
            [side * 0.164, 0.068, -0.132],
            [0.012, 0.022, 0.025],
            [0.94, 0.91, 0.79],
        )?;
        head.voxel(
            [side * 0.111, -0.105, -0.452],
            [0.044, 0.036, 0.013],
            [0.13, 0.095, 0.075],
        )?;
    }
    if matches!(variant, 0 | 1 | 9..=12) {
        head.voxel(
            [0.0, 0.005, -0.215],
            [
                if variant == 9 { 0.08 } else { 0.045 },
                if variant == 9 { 0.27 } else { 0.14 },
                0.018,
            ],
            [0.94, 0.92, 0.85],
        )?;
    }
    let mut lids = Builder::default();
    for side in [-1.0, 1.0] {
        lids.voxel(
            [side * 0.167, 0.055, -0.12],
            [0.014, 0.076, 0.089],
            coat(variant, [side * 0.15, 1.65, -0.72]),
        )?;
    }
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, Vec3::new(0.0, 1.55, -0.61), head.finish()?),
        (Part::Eyelids, Vec3::new(0.0, 1.55, -0.61), lids.finish()?),
    ];
    for (i, (x, z)) in [
        (-0.175, -0.46),
        (0.175, -0.46),
        (-0.175, 0.46),
        (0.175, 0.46),
    ]
    .into_iter()
    .enumerate()
    {
        let mut leg = Builder::default();
        for (y, width, height) in [
            (-0.11, 0.22, 0.27),
            (-0.34, 0.14, 0.24),
            (-0.52, 0.12, 0.16),
        ] {
            covered(
                &mut leg,
                [0.0, y, 0.0],
                [width, height, 0.19],
                variant,
                [x, 0.68 + y, z],
            )?;
        }
        shape::rounded(
            &mut leg,
            [0.0, -0.63, -0.025],
            [0.17, 0.10, 0.23],
            0.018,
            [0.105, 0.105, 0.105],
        )?;
        parts.push((
            Part::Leg(if i == 0 || i == 3 { 1.0 } else { -1.0 }),
            Vec3::new(x, 0.68, z),
            leg.finish()?,
        ));
    }
    let mut tail = Builder::default();
    for i in 0..6 {
        tail.voxel(
            [0.0, -(i as f32) * 0.085, 0.02 + i as f32 * 0.036],
            [0.12 + i as f32 * 0.009, 0.13, 0.11],
            mane,
        )?;
    }
    parts.push((Part::Tail, Vec3::new(0.0, 1.20, 0.64), tail.finish()?));
    Ok(parts)
}
