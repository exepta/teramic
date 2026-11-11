//! Twenty-six authored hairstyles share a closed scalp and individually sculpted locks.
//! Positions follow the front/three-quarter character sheet; rear volumes complete the rig.
use super::{Builder, CharacterAppearance, noise};
use bevy::prelude::*;
#[path = "hair/extended.rs"]
mod extended;

#[derive(Clone, Copy)]
struct Palette {
    base: [f32; 3],
    highlight: Option<[f32; 3]>,
}

impl Palette {
    fn at(self, position: Vec3) -> [f32; 3] {
        // Continuous offset bands follow the same locks around the crown and braids.
        // Tint each entire lock, so highlights never make noisy per-face patches.
        let strand = (position.x + (position.y * 9.0 + position.z * 7.0).sin() * 0.015) / 0.046;
        if (strand.floor() as i32).rem_euclid(4) == 1 {
            self.highlight.unwrap_or(self.base)
        } else {
            self.base
        }
    }
}

fn curl(b: &mut Builder, at: Vec3, scale: Vec3, color: Palette, seed: u32) -> Result<(), String> {
    let value = noise(seed);
    let jitter = |shift| (((value >> shift) & 7_u32) as f32 - 3.5) * 0.012;
    b.bevel_box(
        at.to_array(),
        scale.to_array(),
        color
            .at(at)
            .map(|c| (c * (0.94 + (value & 15) as f32 * 0.008)).min(1.0)),
        Quat::from_euler(EulerRot::XYZ, jitter(4), jitter(7), jitter(10)),
    )
}

fn cluster(
    b: &mut Builder,
    center: Vec3,
    radii: Vec3,
    color: Palette,
    seed: u32,
) -> Result<(), String> {
    let unit = 0.035;
    let bounds = (radii / unit).ceil().as_ivec3();
    for x in -bounds.x..=bounds.x {
        for y in -bounds.y..=bounds.y {
            for z in -bounds.z..=bounds.z {
                let p = Vec3::new(x as f32, y as f32, z as f32) * unit;
                let distance = (p / radii).length_squared();
                if distance > 1.12
                    || [
                        Vec3::X,
                        Vec3::NEG_X,
                        Vec3::Y,
                        Vec3::NEG_Y,
                        Vec3::Z,
                        Vec3::NEG_Z,
                    ]
                    .iter()
                    .all(|normal| ((p + *normal * unit) / radii).length_squared() <= 1.12)
                {
                    // Neighbouring curls cover every face of an interior cell.
                    // Retain only the outer shell of buns and ponytail clusters.
                    continue;
                }
                curl(
                    b,
                    center + p,
                    Vec3::splat(0.048),
                    color,
                    seed.wrapping_add(((x + 12) * 37 + (y + 12) * 173 + (z + 12) * 431) as u32),
                )?;
            }
        }
    }
    Ok(())
}

fn braid(
    b: &mut Builder,
    start: Vec3,
    length: f32,
    side: f32,
    color: Palette,
    seed: u32,
) -> Result<(), String> {
    let count = (length / 0.036).ceil() as u32;
    for step in 0..count {
        let t = step as f32 / count as f32;
        let p = start + Vec3::new(side * t * 0.025, -length * t, -0.05 * t);
        let size = 0.067 * (1.0 - t * 0.38);
        for strand in [-1.0, 1.0] {
            curl(
                b,
                p + Vec3::new(
                    strand * 0.017,
                    strand * 0.009,
                    (step as f32 * 1.4).sin() * 0.009,
                ),
                Vec3::new(size * 0.65, 0.048, size * 0.8),
                color,
                seed + step * 7 + u32::from(strand > 0.0),
            )?;
        }
    }
    b.bevel_box(
        (start + Vec3::new(side * 0.025, -length, -0.05)).to_array(),
        [0.047, 0.026, 0.050],
        [0.38, 0.44, 0.18],
        Quat::IDENTITY,
    )
}

pub(super) fn build(
    b: &mut Builder,
    female: bool,
    appearance: CharacterAppearance,
) -> Result<(), String> {
    let style = appearance.hairstyle % if female { 14 } else { 12 };
    let base = Palette {
        base: appearance.hair_color.map(|c| f32::from(c) / 255.0),
        highlight: appearance.hair_highlights.then(|| {
            appearance
                .hair_highlight_color
                .map(|c| f32::from(c) / 255.0)
        }),
    };
    let cropped = !female && matches!(style, 5 | 10);
    let full =
        (!female && matches!(style, 0 | 4 | 6)) || (female && matches!(style, 1 | 5 | 7 | 9));
    let radii = match (female, style) {
        (false, 4) => Vec3::new(0.224, 0.211, 0.211),
        (false, 6) => Vec3::new(0.214, 0.245, 0.192),
        (true, 1) => Vec3::new(0.231, 0.219, 0.205),
        (true, 9) => Vec3::new(0.202, 0.211, 0.181),
        _ => Vec3::new(
            if full { 0.218 } else { 0.202 },
            if cropped { 0.190 } else { 0.224 },
            0.194,
        ),
    };
    let occupied = |x: i32, y: i32, z: i32| {
        let p = Vec3::new(x as f32, y as f32, z as f32) * 0.044;
        (p / radii).length_squared() <= 1.0
            // A scalp ends above the jaw. Extending the ellipsoid beneath the
            // cheeks makes its rear shell look like a beard from the front.
            && p.y >= -0.044
            && !(p.z < -0.101 && p.y < if cropped { 0.096 } else { 0.072 })
            && !(p.x.abs() > 0.166 && p.y < -0.020 && p.z < 0.045)
    };
    for y in -4..=5 {
        for x in -5..=5 {
            for z in -5..=5 {
                if !occupied(x, y, z)
                    || [
                        [1, 0, 0],
                        [-1, 0, 0],
                        [0, 1, 0],
                        [0, -1, 0],
                        [0, 0, 1],
                        [0, 0, -1],
                    ]
                    .iter()
                    .all(|d| occupied(x + d[0], y + d[1], z + d[2]))
                {
                    continue;
                }
                let seed =
                    ((x + 7) * 317 + (y + 7) * 673 + (z + 7) * 137 + i32::from(style) * 199) as u32;
                let random = noise(seed);
                let jitter = |shift| (((random >> shift) & 7_u32) as f32 - 3.5) * 0.0014;
                let p = Vec3::new(
                    x as f32 * 0.044 + jitter(3),
                    0.048 + y as f32 * 0.044 + jitter(6),
                    0.035 + z as f32 * 0.044 + jitter(9),
                );
                curl(
                    b,
                    p,
                    Vec3::new(0.055, if cropped { 0.045 } else { 0.061 }, 0.057),
                    base,
                    seed,
                )?;
            }
        }
    }
    // Fringes are separate authored silhouettes, never a texture pasted on the forehead.
    for column in -3_i32..=3 {
        let x = column as f32 * 0.045;
        let drop = match (female, style) {
            (false, 5 | 10) => 0.0,
            (false, 8) => (3 - column.abs()) as f32 * -0.008,
            (true, 12) => (column + 3) as f32 * 0.009,
            (false, 1) => (3 - column.abs()) as f32 * 0.008,
            (false, 2) | (false, 6) => (column + 3) as f32 * 0.012,
            (true, 6) => column.abs() as f32 * 0.007,
            (true, 9) => (column + 3) as f32 * 0.012,
            (true, 1) => 0.015 + column.abs() as f32 * 0.008,
            _ => (noise((column + 8) as u32 + u32::from(style) * 17) & 7) as f32 * 0.007,
        };
        curl(
            b,
            Vec3::new(x, 0.187 - drop, -0.158),
            Vec3::new(0.058, if cropped { 0.032 } else { 0.072 }, 0.067),
            base,
            970 + column.unsigned_abs() + u32::from(style) * 11,
        )?;
    }
    let side_steps = match (female, style) {
        (false, 4) => 7,
        (false, 6) => 3,
        (false, 11) => 5,
        (false, _) => 2,
        (true, 12) => 2,
        (true, 10 | 13) => 4,
        (true, 1) => 8,
        (true, 9) => 5,
        (true, 0 | 3 | 4 | 6 | 8) => 5,
        _ => 7,
    };
    for side in [-1.0, 1.0] {
        for step in 0..side_steps {
            let wave = (step as f32 * 1.7 + side).sin();
            curl(
                b,
                Vec3::new(
                    side * (if female && style == 1 {
                        0.204 + wave * 0.023
                    } else {
                        0.181 + wave * 0.017
                    }),
                    0.092 - step as f32 * 0.043,
                    -0.12,
                ),
                Vec3::new(0.045, 0.058, 0.050),
                base,
                200 + step as u32 * 9 + u32::from(side > 0.0),
            )?;
        }
    }
    match (female, style) {
        (false, 4) => {
            // M5 has a distinctly longer layered nape behind the ears.
            for x in -3_i32..=3 {
                for layer in 0..4 {
                    if layer > 0 && x.abs() < 2 {
                        continue;
                    }
                    curl(
                        b,
                        Vec3::new(
                            x as f32 * 0.055,
                            -0.045 - layer as f32 * 0.045,
                            0.177 - x.abs() as f32 * 0.014,
                        ),
                        Vec3::new(0.067, 0.074, 0.063),
                        base,
                        2000 + (x + 4) as u32 * 41 + layer * 13,
                    )?;
                }
            }
        }
        (false, 6) => {
            // M7 keeps the ears clear and sweeps a raised tuft across the crown.
            for tuft in 0..5 {
                curl(
                    b,
                    Vec3::new(
                        -0.11 + tuft as f32 * 0.046,
                        0.267 + (tuft as f32 * 0.7).sin() * 0.035,
                        -0.075,
                    ),
                    Vec3::new(0.080, 0.070, 0.085),
                    base,
                    2200 + tuft * 19,
                )?;
            }
        }
        (false, 1) => {
            for i in 0..5 {
                curl(
                    b,
                    Vec3::new(
                        -0.07 + i as f32 * 0.035,
                        0.28 + (2_i32 - i).abs() as f32 * -0.015,
                        -0.02,
                    ),
                    Vec3::new(0.064, 0.091, 0.075),
                    base,
                    800 + i as u32,
                )?;
            }
        }
        (false, 3) => {
            cluster(
                b,
                Vec3::new(-0.014, 0.293, 0.103),
                Vec3::new(0.091, 0.094, 0.087),
                base,
                500,
            )?;
            b.bevel_box(
                [-0.014, 0.228, 0.10],
                [0.14, 0.025, 0.14],
                [0.61, 0.51, 0.23],
                Quat::IDENTITY,
            )?;
        }
        (true, 0 | 8) => {
            // A wide rounded bun sits into the rear crown, not on a tall stem.
            cluster(
                b,
                Vec3::new(-0.014, 0.250, 0.122),
                Vec3::new(0.106, 0.064, 0.092),
                base,
                500,
            )?;
            b.bevel_box(
                [-0.014, 0.215, 0.111],
                [0.15, 0.020, 0.14],
                [0.61, 0.51, 0.23],
                Quat::IDENTITY,
            )?;
        }
        (true, 3) => {
            for side in [-1.0, 1.0] {
                cluster(
                    b,
                    Vec3::new(side * 0.197, 0.218, 0.055),
                    Vec3::splat(0.083),
                    base,
                    670 + u32::from(side > 0.0),
                )?;
            }
        }
        (true, 4) => {
            cluster(
                b,
                Vec3::new(-0.097, 0.245, 0.105),
                Vec3::new(0.080, 0.060, 0.073),
                base,
                710,
            )?;
            for i in 0..9 {
                cluster(
                    b,
                    Vec3::new(
                        -0.179 - (i as f32 * 0.36).sin() * 0.095,
                        0.245 - i as f32 * 0.055,
                        0.085 + (i as f32 * 0.7).sin() * 0.02,
                    ),
                    Vec3::new(0.052, 0.058, 0.048),
                    base,
                    720 + i * 47,
                )?;
            }
        }
        _ => {}
    }
    if female {
        match style {
            2 => braid(b, Vec3::new(0.193, -0.064, -0.09), 0.30, 1.0, base, 1300)?,
            7 => braid(b, Vec3::new(-0.19, -0.064, -0.09), 0.31, -1.0, base, 1400)?,
            5 | 8 => {
                for side in [-1.0, 1.0] {
                    for strand in 0..3 {
                        for step in 0..9 {
                            curl(
                                b,
                                Vec3::new(
                                    side * (0.175
                                        + (step as f32 * 0.8 + strand as f32).sin() * 0.022),
                                    -0.03 - step as f32 * 0.042,
                                    0.02 + strand as f32 * 0.067,
                                ),
                                Vec3::new(0.052, 0.070, 0.067),
                                base,
                                1500 + strand * 101 + step * 17 + u32::from(side > 0.0),
                            )?;
                        }
                    }
                }
            }
            6 => {
                // Cream headband follows the whole crown, leaving the hair visible behind it.
                for i in 0..15 {
                    let angle = i as f32 / 14.0 * std::f32::consts::PI;
                    b.bevel_box(
                        [angle.cos() * 0.207, 0.05 + angle.sin() * 0.213, -0.026],
                        [0.038, 0.038, 0.061],
                        [0.90, 0.84, 0.70],
                        Quat::from_rotation_z(angle),
                    )?;
                }
            }
            _ => {}
        }
        // Bob cuts have a rounded, layered nape; no long rear slab or bald back.
        if matches!(style, 1 | 2 | 7 | 9) {
            for x in -3_i32..=3 {
                for y in 0..if style == 9 {
                    2
                } else if style == 1 {
                    5
                } else {
                    4
                } {
                    // Keep the back neckline open below the jaw; side locks
                    // supply the rounded bob without a brown chin ring.
                    if y > 0 && x.abs() < 2 {
                        continue;
                    }
                    curl(
                        b,
                        Vec3::new(
                            x as f32 * if style == 9 { 0.049 } else { 0.054 },
                            -0.04 - y as f32 * 0.045,
                            if style == 9 { 0.153 } else { 0.17 } - (x.abs() as f32) * 0.015,
                        ),
                        Vec3::new(0.066, 0.062, 0.064),
                        base,
                        1800 + (x + 4) as u32 * 17 + y * 31,
                    )?;
                }
            }
        }
    }
    extended::build(b, female, style, base)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_recolor_coherent_locks_in_every_style_without_changing_geometry() {
        for female in [false, true] {
            for hairstyle in 0..if female { 14 } else { 12 } {
                let mut plain = Builder::default();
                let look = CharacterAppearance {
                    hairstyle,
                    ..default()
                };
                build(&mut plain, female, look).unwrap();
                let plain = plain.finish_batch().unwrap();
                let mut highlighted = Builder::default();
                build(
                    &mut highlighted,
                    female,
                    CharacterAppearance {
                        hair_highlights: true,
                        hair_highlight_color: [180, 35, 210],
                        ..look
                    },
                )
                .unwrap();
                let highlighted = highlighted.finish_batch().unwrap();
                assert_eq!(plain.positions, highlighted.positions);
                assert_eq!(plain.normals, highlighted.normals);
                let changes = plain
                    .colors
                    .iter()
                    .zip(&highlighted.colors)
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(changes > 100, "female={female}, hairstyle={hairstyle}");
                assert!(
                    changes < plain.colors.len() / 2,
                    "Highlights must retain the main hair color"
                );
            }
        }
    }

    #[test]
    fn hairstyles_keep_the_chin_clear_and_match_reference_side_landmarks() {
        for female in [false, true] {
            for hairstyle in 0..if female { 14 } else { 12 } {
                let mut builder = Builder::default();
                build(
                    &mut builder,
                    female,
                    CharacterAppearance {
                        hairstyle,
                        ..default()
                    },
                )
                .unwrap();
                let positions = builder.finish_batch().unwrap().positions;
                assert!(
                    positions
                        .iter()
                        .all(|p| p[1] > -0.10 || p[0].abs() > 0.055 || p[2] > 0.08),
                    "Hair must not form a beard beneath the chin: female={female}, style={hairstyle}"
                );
                if female && matches!(hairstyle, 2 | 7) {
                    let braid = positions.iter().filter(|p| p[1] < -0.25);
                    // The frontal camera sees positive model X on its left.
                    let side = if hairstyle == 2 { 1.0 } else { -1.0 };
                    assert!(braid.clone().count() > 20);
                    assert!(braid.into_iter().all(|p| p[0] * side > 0.10));
                }
                if female && hairstyle == 4 {
                    assert!(positions.iter().any(|p| p[0] < -0.23 && p[1] < -0.12));
                }
                if female && matches!(hairstyle, 0 | 8) {
                    assert!(positions.iter().all(|p| p[1] < 0.34));
                }
            }
        }
    }
}
