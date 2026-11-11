//! Additional silhouettes keep previous hairstyle indices stable in saved profiles.
use super::{Builder, Palette, braid, cluster, curl};
use bevy::prelude::*;

pub(super) fn build(
    builder: &mut Builder,
    female: bool,
    style: u8,
    palette: Palette,
) -> Result<(), String> {
    match (female, style) {
        (false, 7) => {
            // M8: swept pompadour with a low, connected rear crown.
            for row in 0..3 {
                for lock in 0..6 {
                    curl(
                        builder,
                        Vec3::new(
                            -0.12 + lock as f32 * 0.044,
                            0.247 + (lock as f32 * 0.55).sin() * 0.045 - row as f32 * 0.017,
                            -0.095 + row as f32 * 0.07,
                        ),
                        Vec3::new(0.075, 0.061, 0.094),
                        palette,
                        3100 + row * 61 + lock * 7,
                    )?;
                }
            }
        }
        (false, 8) => {
            // M9: center part with two broad curtains beside the brows.
            for side in [-1.0, 1.0] {
                for lock in 0..6 {
                    let t = lock as f32 / 5.0;
                    curl(
                        builder,
                        Vec3::new(side * (0.035 + t * 0.15), 0.256 - t * 0.14, -0.15),
                        Vec3::new(0.067, 0.079, 0.068),
                        palette,
                        3300 + lock * 13 + u32::from(side > 0.0),
                    )?;
                }
            }
        }
        (false, 9) => {
            // M10: tied back crown and a short plait down the rear, clear of the face.
            cluster(
                builder,
                Vec3::new(0.0, 0.185, 0.204),
                Vec3::new(0.067, 0.065, 0.065),
                palette,
                3500,
            )?;
            braid(
                builder,
                Vec3::new(0.0, 0.13, 0.23),
                0.26,
                0.0,
                palette,
                3600,
            )?;
        }
        (false, 10) => {
            // M11: close crop, with a neat raised strip following the crown.
            for lock in 0..7 {
                let angle = lock as f32 / 6.0 * std::f32::consts::PI;
                curl(
                    builder,
                    Vec3::new(
                        0.0,
                        0.095 + angle.sin() * 0.18,
                        -angle.cos() * 0.155 + 0.035,
                    ),
                    Vec3::new(0.060, 0.042, 0.059),
                    palette,
                    3800 + lock * 17,
                )?;
            }
        }
        (false, 11) => {
            // M12: half bun above a layered, shoulder-length nape.
            cluster(
                builder,
                Vec3::new(0.0, 0.25, 0.15),
                Vec3::new(0.075, 0.062, 0.074),
                palette,
                3900,
            )?;
            long_locks(builder, palette, 6, 0.19, 4000)?;
        }
        (true, 10) => {
            // F11: an interwoven crown braid follows the head, with a small low bun.
            for lock in 0..22 {
                let angle = lock as f32 / 22.0 * std::f32::consts::TAU;
                curl(
                    builder,
                    Vec3::new(
                        angle.cos() * 0.205,
                        0.145 + angle.sin() * 0.027,
                        0.03 + angle.sin() * 0.17,
                    ),
                    Vec3::new(0.060, 0.052, 0.066),
                    palette,
                    4300 + lock * 19,
                )?;
            }
            cluster(
                builder,
                Vec3::new(0.0, 0.015, 0.202),
                Vec3::new(0.083, 0.061, 0.060),
                palette,
                4400,
            )?;
        }
        (true, 11) => {
            // F12: loose, long waves, connected at the sides and at the back.
            long_locks(builder, palette, 10, 0.203, 4600)?;
        }
        (true, 12) => {
            // F13: short asymmetric pixie with a swept fringe and one long temple lock.
            for lock in 0..6 {
                curl(
                    builder,
                    Vec3::new(
                        -0.125 + lock as f32 * 0.046,
                        0.251 - lock as f32 * 0.016,
                        -0.13,
                    ),
                    Vec3::new(0.072, 0.068, 0.075),
                    palette,
                    4900 + lock * 23,
                )?;
            }
            for lock in 0..4 {
                curl(
                    builder,
                    Vec3::new(0.19, 0.105 - lock as f32 * 0.043, -0.115),
                    Vec3::new(0.044, 0.059, 0.058),
                    palette,
                    5000 + lock * 29,
                )?;
            }
        }
        (true, 13) => {
            // F14: two individually interwoven braids, tied below the shoulders.
            for side in [-1.0, 1.0] {
                braid(
                    builder,
                    Vec3::new(side * 0.187, -0.015, -0.075),
                    0.36,
                    side,
                    palette,
                    5200 + u32::from(side > 0.0) * 100,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn long_locks(
    builder: &mut Builder,
    palette: Palette,
    rows: u32,
    width: f32,
    seed: u32,
) -> Result<(), String> {
    for side in [-1.0, 1.0] {
        for strand in 0..3 {
            for row in 0..rows {
                curl(
                    builder,
                    Vec3::new(
                        side * (width + (row as f32 * 0.65 + strand as f32).sin() * 0.018),
                        0.015 - row as f32 * 0.040,
                        0.015 + strand as f32 * 0.067,
                    ),
                    Vec3::new(0.058, 0.063, 0.077),
                    palette,
                    seed + strand * 101 + row * 31 + u32::from(side > 0.0),
                )?;
            }
        }
    }
    Ok(())
}
