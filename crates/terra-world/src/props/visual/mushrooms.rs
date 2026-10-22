use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;

pub(super) fn cap(palette: &PropPalette, variant: usize) -> usize {
    [
        palette.flowers[5],
        palette.mushroom,
        palette.flowers[1],
        palette.foliage[1],
        palette.flowers[3],
        palette.flowers[2],
        palette.flowers[0],
        palette.flowers[1],
    ][variant % 8]
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    if level == PropDetail::Near {
        return super::botanical_detail::mushroom(batches, variant, palette);
    }
    let cap = cap(palette, variant);
    let fine = level == PropDetail::Near;
    let count = if level == PropDetail::Far { 1 } else { 2 };
    for child in 0..count {
        let scale = if child == 0 { 1.0 } else { 0.58 };
        let root = if child == 0 {
            Vec3::new(-0.04, 0.0, -0.025)
        } else {
            Vec3::new(0.16, 0.0, 0.10)
        };
        let mut voxel = |material, center: [f32; 3], size: [f32; 3]| {
            add_box(
                batches,
                material,
                root + Vec3::from_array(center) * scale,
                Vec3::from_array(size) * scale,
                Quat::IDENTITY,
            )
        };
        let h = 0.24 + (variant % 3) as f32 * 0.025;
        voxel(palette.flowers[2], [0.0, h * 0.5, 0.0], [0.11, h, 0.11])?;
        voxel(cap, [0.0, h + 0.035, 0.0], [0.36, 0.09, 0.32])?;
        voxel(cap, [0.0, h + 0.105, 0.0], [0.27, 0.07, 0.25])?;
        if fine {
            voxel(cap, [0.0, h + 0.155, 0.0], [0.16, 0.035, 0.16])?;
            voxel(
                palette.flowers[2],
                [0.0, h - 0.015, 0.0],
                [0.24, 0.025, 0.22],
            )?;
            for (i, [x, z]) in [[-0.075, -0.045], [0.065, 0.055], [-0.035, 0.07]]
                .into_iter()
                .enumerate()
            {
                voxel(
                    if variant % 8 == 5 {
                        palette.twig
                    } else {
                        palette.flowers[2]
                    },
                    [x, h + if i == 2 { 0.145 } else { 0.15 }, z],
                    [0.055, 0.018, 0.05],
                )?;
            }
        }
    }
    if variant % 8 == 7 {
        super::botanical_detail::tint_material(batches, cap, [1.1, 0.38, 0.055]);
    }
    Ok(())
}
