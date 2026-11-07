use super::{Builder, Palette, palette};
use bevy::prelude::*;

pub(super) fn build(builder: &mut Builder, p: &Palette) -> Result<(), String> {
    let mut part = |center, size, color| builder.bevel_box(center, size, color, Quat::IDENTITY);
    part([0.5, 0.36, 0.5], [0.105, 0.70, 0.105], p.body)?;
    part([0.5, 0.035, 0.5], [0.145, 0.07, 0.15], p.dark)?;
    for z in [0.438, 0.562] {
        part([0.5, 0.265, z], [0.13, 0.42, 0.035], palette::WOOD)?;
    }
    for y in [0.075, 0.455] {
        part([0.5, y, 0.5], [0.15, 0.04, 0.155], p.fitting_dark)?;
    }
    for z in [0.414, 0.586] {
        for y in [0.13, 0.395] {
            part([0.5, y, z], [0.034, 0.034, 0.012], p.edge)?;
        }
    }
    for side in [-1.0, 1.0] {
        for row in 0..7 {
            builder.voxel(
                [
                    0.477 + (row % 3) as f32 * 0.019,
                    0.12 + row as f32 * 0.043,
                    0.5 + side * 0.081,
                ],
                [0.008, 0.025, 0.004],
                if row % 2 == 0 {
                    palette::WOOD_LIGHT
                } else {
                    palette::WOOD_DARK
                },
            )?;
        }
    }
    for z in [0.44, 0.56] {
        builder.bevel_box(
            [0.5, 0.585, z],
            [0.045, 0.12, 0.014],
            p.light,
            Quat::IDENTITY,
        )?;
    }

    // A single open annular forging leaves a real gap between the two jaws.
    let center_y = 1.0 - 0.20 * std::f32::consts::FRAC_1_SQRT_2;
    let section = |angle: f32| {
        [
            (0.108, -0.075),
            (0.192, -0.075),
            (0.200, -0.067),
            (0.200, 0.067),
            (0.192, 0.075),
            (0.108, 0.075),
            (0.100, 0.067),
            (0.100, -0.067),
        ]
        .map(|(radius, z)| {
            Vec3::new(
                0.5 + angle.sin() * radius,
                center_y + angle.cos() * radius,
                0.5 + z,
            )
        })
    };
    let start = std::f32::consts::FRAC_PI_4;
    let sweep = std::f32::consts::PI * 1.5;
    for segment in 0..18 {
        let a = section(start + sweep * segment as f32 / 18.0);
        let b = section(start + sweep * (segment + 1) as f32 / 18.0);
        for side in 0..8 {
            let next = (side + 1) % 8;
            builder.quad(
                [a[side], a[next], b[next], b[side]],
                match side {
                    0 | 4 => p.light,
                    2 => p.body,
                    6 => p.dark,
                    _ => p.edge,
                },
            )?;
        }
    }
    for (angle, end) in [(start, false), (start + sweep, true)] {
        let ring = section(angle);
        let center = Vec3::new(0.5 + angle.sin() * 0.15, center_y + angle.cos() * 0.15, 0.5);
        for side in 0..8 {
            let next = (side + 1) % 8;
            builder.triangle(
                if end {
                    [center, ring[side], ring[next]]
                } else {
                    [center, ring[next], ring[side]]
                },
                p.edge,
            )?;
        }
    }
    Ok(())
}
