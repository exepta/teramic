use super::geometry::add_box;
use super::{PropDetail, PropMesh, PropPalette};
use bevy_math::{Quat, Vec3};
use std::{collections::BTreeMap, f32::consts::TAU};

pub(super) fn add_flowers(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    if level == PropDetail::Near {
        return super::botanical_detail::flower(batches, variant, palette);
    }
    let species = variant % 7;
    let fine = level == PropDetail::Near;
    let far = level == PropDetail::Far;
    for flower in 0..if far { 1 } else { 2 } {
        let root = if flower == 0 {
            Vec3::new(-0.095, 0.0, -0.035)
        } else {
            Vec3::new(0.115, 0.0, 0.055)
        };
        let height = 0.45 + ((variant + flower) % 3) as f32 * 0.065;
        let scale = if flower == 0 { 1.0 } else { 0.85 };
        let mut voxel = |material, center: Vec3, size: Vec3| {
            add_box(batches, material, root + center, size, Quat::IDENTITY)
        };
        voxel(
            palette.stem,
            Vec3::Y * (height * 0.5),
            Vec3::new(0.046, height, 0.046),
        )?;
        if !far {
            for leaf in 0..if fine { 2 } else { 1 } {
                let sign = if leaf == 0 { 1.0 } else { -1.0 };
                voxel(
                    palette.foliage[(variant + leaf) % 3],
                    Vec3::new(sign * 0.055, height * (0.29 + leaf as f32 * 0.27), 0.0),
                    Vec3::new(0.14, 0.065, 0.075) * scale,
                )?;
            }
        }
        let color = palette.flowers[if species == 6 { 1 } else { species }];
        if !fine {
            voxel(
                color,
                Vec3::Y * (height + 0.02),
                Vec3::new(0.24, 0.11, 0.22) * scale,
            )?;
            continue;
        }
        // Six petals/bells, two leaves and a thick stem keep each tuft <=480 vertices.
        if species == 5 {
            for tier in 0..3 {
                for side in [-1.0, 1.0] {
                    voxel(
                        color,
                        Vec3::new(side * 0.058, height - 0.15 + tier as f32 * 0.095, 0.0),
                        Vec3::new(0.10, 0.12, 0.10) * scale,
                    )?;
                }
            }
        } else {
            for petal in 0..6 {
                let angle = petal as f32 * TAU / 6.0;
                let radius = if species == 0 || species == 4 {
                    0.065
                } else {
                    0.085
                };
                let lift = if species == 0 || species == 4 {
                    0.03 + (petal % 2) as f32 * 0.025
                } else {
                    (petal % 2) as f32 * 0.012
                };
                let thickness = if species == 0 || species == 4 {
                    0.09
                } else {
                    0.055
                };
                voxel(
                    color,
                    Vec3::new(
                        angle.cos() * radius * scale,
                        height + lift,
                        angle.sin() * radius * scale,
                    ),
                    Vec3::new(0.115, thickness, 0.09) * scale,
                )?;
            }
        }
        let pollen = if species == 0 {
            palette.mushroom
        } else {
            palette.flowers[1]
        };
        voxel(
            pollen,
            Vec3::Y * (height + 0.055),
            Vec3::new(0.07, 0.065, 0.07) * scale,
        )?;
    }
    if species == 6 {
        super::botanical_detail::tint_material(batches, palette.flowers[1], [1.1, 0.38, 0.055]);
    }
    Ok(())
}
