//! Meter-authored aquatic models: shared meshes, a few articulated groups, no voxel entities.
use super::{Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

type Parts = Result<Vec<(Part, Vec3, Mesh)>, String>;

fn tint(color: [f32; 3], amount: f32) -> [f32; 3] {
    color.map(|v| (v * amount).min(1.0))
}

pub(super) fn squid() -> Parts {
    let red = [0.78, 0.15, 0.19];
    let rose = [0.97, 0.31, 0.33];
    let mut body = Builder::default();
    for row in 0..9 {
        let t = row as f32 / 8.0;
        let radius = 0.23 + (std::f32::consts::PI * t).sin() * 0.10;
        shape::rounded(
            &mut body,
            [0.0, 0.475, -0.92 + t * 0.88],
            [radius * 2.0, radius * 2.0, 0.115],
            0.035,
            tint(red, 0.91 + (row % 3) as f32 * 0.07),
        )?;
        for side in [-1.0, 1.0] {
            body.voxel(
                [side * (radius + 0.004), 0.55, -0.92 + t * 0.88],
                [0.018, 0.085, 0.035],
                rose,
            )?;
        }
    }
    for side in [-1.0, 1.0] {
        for step in 0..4 {
            let t = step as f32 / 3.0;
            body.voxel(
                [side * (0.29 + 0.14 * t), 0.475, -0.65 + t * 0.18],
                [0.085, 0.032, 0.22 - t * 0.10],
                tint(rose, 0.86),
            )?;
        }
        shape::rounded(
            &mut body,
            [side * 0.29, 0.49, -0.12],
            [0.075, 0.18, 0.17],
            0.022,
            [0.98, 0.67, 0.60],
        )?;
        body.voxel(
            [side * 0.33, 0.49, -0.145],
            [0.018, 0.12, 0.09],
            [0.025, 0.09, 0.12],
        )?;
        body.voxel(
            [side * 0.341, 0.522, -0.17],
            [0.012, 0.03, 0.03],
            [0.85, 0.96, 0.96],
        )?;
    }
    let mut parts = vec![(
        Part::Body,
        Vec3::ZERO,
        body.finish()?.scaled_by(Vec3::new(0.8, 1.0, 0.8)),
    )];
    for group in 0..4 {
        let mut arms = Builder::default();
        for arm in [group * 2, group * 2 + 1] {
            let angle = arm as f32 * std::f32::consts::TAU / 8.0;
            let radial = Vec3::new(angle.cos(), angle.sin(), 0.0);
            let start = radial * 0.18;
            let mut previous = start;
            for step in 1..=9 {
                let t = step as f32 / 9.0;
                let spread = 0.18 + (t * std::f32::consts::PI * 0.72).sin() * 0.20;
                let curl = Vec3::new(-radial.y, radial.x, 0.0) * t.powi(2) * 0.075;
                let point = radial * spread + curl + Vec3::Z * (t * 1.0);
                let width = 0.112 * (1.0 - t * 0.73);
                shape::segment(
                    &mut arms,
                    previous,
                    point,
                    width,
                    width,
                    if step % 3 == 0 { rose } else { red },
                )?;
                if step % 2 == 0 {
                    let suction = point - radial * (width * 0.49);
                    arms.voxel(suction.to_array(), [width * 0.55; 3], [1.0, 0.56, 0.49])?;
                }
                previous = point;
            }
        }
        parts.push((
            Part::Tentacles((group as f32 * 2. + 0.5) * std::f32::consts::FRAC_PI_4),
            Vec3::new(0.0, 0.475, -0.02),
            arms.finish()?,
        ));
    }
    Ok(parts)
}

fn jelly_palette(kind: Kind) -> ([f32; 3], [f32; 3], [f32; 3]) {
    match kind {
        Kind::FireJelly => ([0.98, 0.24, 0.055], [1.0, 0.72, 0.16], [1.0, 0.91, 0.53]),
        Kind::VoidJelly => ([0.35, 0.055, 0.65], [0.70, 0.27, 0.95], [0.86, 0.62, 1.0]),
        Kind::SunJelly => ([0.97, 0.65, 0.20], [1.0, 0.86, 0.48], [1.0, 0.98, 0.79]),
        Kind::CrystalJelly => ([0.02, 0.57, 0.83], [0.16, 0.88, 0.98], [0.69, 0.99, 1.0]),
        _ => ([0.18, 0.49, 0.87], [0.48, 0.76, 1.0], [0.86, 0.96, 1.0]),
    }
}

pub(super) fn jelly(kind: Kind) -> Parts {
    let (shade, color, pale) = jelly_palette(kind);
    let mut bell = Builder::default();
    let ring = |radius: f32, height: f32| -> [Vec3; 12] {
        std::array::from_fn(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 12.0;
            Vec3::new(angle.cos() * radius, height, angle.sin() * radius)
        })
    };
    let tall = matches!(kind, Kind::FireJelly | Kind::VoidJelly | Kind::CrystalJelly);
    let profile = if tall {
        [
            (0.166, 0.62),
            (0.174, 0.68),
            (0.154, 0.77),
            (0.122, 0.86),
            (0.065, 0.92),
            (0.020, 0.94),
        ]
    } else {
        [
            (0.173, 0.62),
            (0.175, 0.67),
            (0.148, 0.74),
            (0.110, 0.79),
            (0.060, 0.83),
            (0.020, 0.84),
        ]
    };
    for (row, pair) in profile.windows(2).enumerate() {
        let a = ring(pair[0].0, pair[0].1);
        let b = ring(pair[1].0, pair[1].1);
        for side in 0..12 {
            let next = (side + 1) % 12;
            bell.quad(
                [a[next], a[side], b[side], b[next]],
                if row == 0 {
                    pale
                } else if (side + row) % 4 == 0 {
                    shade
                } else {
                    color
                },
            )?;
        }
    }
    shape::rounded(
        &mut bell,
        [0.0, profile[5].1, 0.0],
        [0.041, 0.018, 0.041],
        0.005,
        pale,
    )?;
    for side in 0..12 {
        let angle = side as f32 * std::f32::consts::TAU / 12.0;
        bell.rotated(
            [angle.cos() * 0.160, 0.635, angle.sin() * 0.160],
            [0.041, 0.030, 0.026],
            pale,
            Quat::from_rotation_y(-angle),
        )?;
        if side % 2 == 0 {
            bell.voxel(
                [angle.cos() * 0.111, 0.785, angle.sin() * 0.111],
                [0.025, 0.036, 0.025],
                pale,
            )?;
        }
    }
    let mut core = Builder::default();
    for petal in 0..4 {
        let a = petal as f32 * std::f32::consts::FRAC_PI_2;
        shape::rounded(
            &mut core,
            [a.cos() * 0.035, 0.655, a.sin() * 0.035],
            [0.064, 0.085, 0.064],
            0.014,
            pale,
        )?;
    }
    let mut result = vec![
        (Part::Bell, Vec3::ZERO, bell.finish()?),
        (Part::Body, Vec3::ZERO, core.finish()?),
    ];
    let species = match kind {
        Kind::MoonJelly => 0,
        Kind::FireJelly => 1,
        Kind::VoidJelly => 2,
        Kind::SunJelly => 3,
        _ => 4,
    };
    for group in 0..3 {
        let mut arms = Builder::default();
        for arm in 0..3 {
            let index = group * 3 + arm;
            let a = index as f32 * std::f32::consts::TAU / 9.0;
            let length = 0.38 + ((index * 5 + species * 3) % 9) as f32 * 0.028;
            let radius = 0.075 + (index % 2) as f32 * 0.033;
            let mut previous = Vec3::new(a.cos() * radius, 0.0, a.sin() * radius);
            for step in 1..=9 {
                let t = step as f32 / 9.0;
                let wave = (t * 8.0 + index as f32 * 1.9).sin() * (0.012 + t * 0.010);
                let point = Vec3::new(
                    a.cos() * (radius + wave) + t * 0.015 * a.sin(),
                    -length * t,
                    a.sin() * (radius + wave),
                );
                let width = if species == 2 { 0.015 } else { 0.018 } * (1.0 - t * 0.46);
                shape::segment(
                    &mut arms,
                    previous,
                    point,
                    width,
                    width,
                    if (step + index) % 3 == 0 { pale } else { color },
                )?;
                if species == 4 && step % 3 == 0 {
                    arms.voxel(point.to_array(), [width * 1.7; 3], pale)?;
                }
                previous = point;
            }
        }
        result.push((
            Part::Tentacles(group as f32 * 2.1),
            Vec3::new(0.0, 0.615, 0.0),
            arms.finish()?,
        ));
    }
    Ok(result)
}

pub(super) fn shark(kind: Kind) -> Parts {
    let [width, height, length] = kind.size().map(|v| v as f32);
    let whale = kind == Kind::WhaleShark;
    let hammer = kind == Kind::HammerheadShark;
    let coat = if whale {
        [0.10, 0.25, 0.51]
    } else if hammer {
        [0.47, 0.43, 0.36]
    } else {
        [0.32, 0.40, 0.55]
    };
    let belly = if whale {
        [0.88, 0.88, 0.81]
    } else {
        [0.80, 0.82, 0.77]
    };
    let mut body = Builder::default();
    let half = if whale { 0.38 } else { 0.25 } * width;
    let center_y = height * 0.45;
    for row in 0..15 {
        let t = row as f32 / 14.0;
        let profile = if t < 0.22 {
            0.76 + t * 1.08
        } else {
            1.0 - ((t - 0.22) / 0.78).powf(1.6) * 0.80
        };
        let w = half * 2.0 * profile;
        let h = height * (if whale { 0.40 } else { 0.32 }) * profile;
        let z = length * (-0.465 + t * 0.76);
        shape::rounded(
            &mut body,
            [0.0, center_y, z],
            [w, h, length * 0.06],
            h * 0.18,
            tint(coat, 0.94 + (row % 3) as f32 * 0.035),
        )?;
        shape::rounded(
            &mut body,
            [0.0, center_y - h * 0.32, z],
            [w * 0.94, h * 0.37, length * 0.061],
            h * 0.075,
            belly,
        )?;
        if whale && row < 12 {
            for spot in 0..5 {
                let x = (spot as f32 - 2.0) * w * 0.16;
                let y = center_y + h * 0.5 + 0.004;
                body.voxel(
                    [x, y, z + ((row + spot) % 3) as f32 * 0.013],
                    [width * 0.027, 0.012, length * 0.017],
                    [0.84, 0.91, 0.96],
                )?;
            }
            for side in [-1.0, 1.0] {
                for spot in 0..2 {
                    body.voxel(
                        [
                            side * (w * 0.5 + 0.004),
                            center_y + h * (0.04 + spot as f32 * 0.20),
                            z,
                        ],
                        [0.014, height * 0.026, length * 0.017],
                        [0.72, 0.85, 0.94],
                    )?;
                }
            }
        }
    }
    for step in 0..8 {
        let t = step as f32 / 7.0;
        let rise = height * (0.34 * (1.0 - t) + 0.025);
        body.voxel(
            [
                0.0,
                center_y + height * 0.13 + rise * 0.5,
                length * (-0.03 + t * 0.18),
            ],
            [width * 0.032, rise, length * 0.031],
            tint(coat, 0.85 + t * 0.15),
        )?;
    }
    let eye_x = if hammer { width * 0.43 } else { half * 0.80 };
    if hammer {
        shape::rounded(
            &mut body,
            [0.0, center_y, -length * 0.39],
            [width * 0.94, height * 0.19, length * 0.16],
            height * 0.035,
            coat,
        )?;
        body.voxel(
            [0.0, center_y - height * 0.045, -length * 0.477],
            [width * 0.82, height * 0.025, length * 0.012],
            belly,
        )?;
    }
    for side in [-1.0, 1.0] {
        let z = -length * if hammer { 0.425 } else { 0.405 };
        body.voxel(
            [side * eye_x, center_y + height * 0.035, z],
            [width * 0.035, height * 0.065, length * 0.040],
            [0.035, 0.045, 0.055],
        )?;
        body.voxel(
            [
                side * (eye_x + width * 0.018),
                center_y + height * 0.05,
                z - length * 0.008,
            ],
            [width * 0.009, height * 0.014, length * 0.014],
            [0.77, 0.90, 0.95],
        )?;
        for slit in 0..5 {
            let z = length * (-0.23 + slit as f32 * 0.027);
            body.rotated(
                [side * (half * 0.96), center_y - height * 0.018, z],
                [
                    width * 0.012,
                    height * (0.14 - slit as f32 * 0.01),
                    length * 0.009,
                ],
                tint(coat, 0.48),
                Quat::from_rotation_x(-0.14),
            )?;
        }
    }
    let mut jaw = Builder::default();
    let jaw_width = if hammer {
        0.43
    } else if whale {
        0.61
    } else {
        0.35
    } * width;
    shape::rounded(
        &mut jaw,
        [0.0, -height * 0.025, -length * 0.047],
        [jaw_width, height * 0.074, length * 0.12],
        height * 0.018,
        belly,
    )?;
    jaw.voxel(
        [0.0, height * 0.018, -length * 0.105],
        [jaw_width * 0.83, height * 0.016, length * 0.014],
        [0.10, 0.10, 0.12],
    )?;
    for tooth in 0..7 {
        jaw.voxel(
            [
                (tooth as f32 - 3.0) * jaw_width * 0.10,
                height * 0.012,
                -length * 0.113,
            ],
            [jaw_width * 0.045, height * 0.021, length * 0.012],
            [0.98, 0.97, 0.90],
        )?;
    }
    let mut tail = Builder::default();
    for side in [-1.0, 1.0] {
        let mut previous = Vec3::ZERO;
        for step in 1..=7 {
            let t = step as f32 / 7.0;
            let point = Vec3::new(0.0, side * height * 0.30 * t, length * (t * 0.15 + 0.015));
            shape::segment(
                &mut tail,
                previous,
                point,
                width * (0.055 - t * 0.027),
                length * (0.13 - t * 0.095),
                tint(coat, 0.91 + t * 0.08),
            )?;
            previous = point;
        }
    }
    let mut result = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (
            Part::Jaw,
            Vec3::new(0.0, center_y - height * 0.12, -length * 0.36),
            jaw.finish()?,
        ),
        (
            Part::Tail,
            Vec3::new(0.0, center_y, length * 0.30),
            tail.finish()?,
        ),
    ];
    for side in [-1.0, 1.0] {
        let mut fin = Builder::default();
        for step in 0..7 {
            let t = step as f32 / 6.0;
            let x = side * (width * 0.015 + t * (width * 0.45 - half * 0.69));
            fin.voxel(
                [x, -height * t * 0.07, length * (t * 0.09)],
                [
                    width * 0.07,
                    height * (0.032 - t * 0.014),
                    length * (0.20 - t * 0.13),
                ],
                tint(coat, 0.92 + t * 0.07),
            )?;
            fin.voxel(
                [x, -height * t * 0.07 - height * 0.017, length * (t * 0.09)],
                [width * 0.068, height * 0.012, length * (0.18 - t * 0.12)],
                belly,
            )?;
        }
        result.push((
            Part::Fin(side),
            Vec3::new(
                side * half * 0.69,
                center_y - height * 0.055,
                -length * 0.14,
            ),
            fin.finish()?,
        ));
    }
    Ok(result)
}
