//! Attached shelf clusters; each cluster remains one removable, batched prop.
use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;

pub(super) fn append(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    detail: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let color = match (variant - 128) % 3 {
        0 => palette.flowers[0],
        1 => palette.mushroom,
        _ => palette.flowers[2],
    };
    let near = detail == PropDetail::Near;
    for tier in 0..if detail == PropDetail::Far { 2 } else { 3 } {
        let y = 0.06 + tier as f32 * 0.13;
        let x = if tier % 2 == 0 { -0.045 } else { 0.055 };
        let width = 0.29 - tier as f32 * 0.035;
        let root = Vec3::new(x, y, -0.07);
        add_box(
            batches,
            palette.flowers[2],
            root,
            Vec3::new(width * 0.78, 0.025, 0.19),
            Quat::IDENTITY,
        )?;
        add_box(
            batches,
            color,
            root + Vec3::new(0.0, 0.027, 0.015),
            Vec3::new(width, 0.038, 0.23),
            Quat::IDENTITY,
        )?;
        add_box(
            batches,
            color,
            root + Vec3::new(0.0, 0.055, -0.015),
            Vec3::new(width * 0.68, 0.024, 0.16),
            Quat::IDENTITY,
        )?;
        if near {
            for offset in [-0.065, 0.025, 0.075] {
                add_box(
                    batches,
                    palette.flowers[2],
                    root + Vec3::new(offset, 0.07, -offset * 0.3),
                    Vec3::new(0.027, 0.014, 0.025),
                    Quat::IDENTITY,
                )?;
            }
        }
    }
    Ok(())
}
