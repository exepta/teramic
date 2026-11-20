//! Bounded near-field plant voxels. Shared section batches, no per-detail entities.
use super::{
    PropDetail, PropMesh, PropPalette,
    geometry::{add_box, add_stalk},
    leaf::{add_profile, grass_profile},
};
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

pub(super) fn tint_material(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    tint: [f32; 3],
) {
    if let Some(mesh) = batches.get_mut(&material) {
        mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
        for color in &mut mesh.colors {
            for axis in 0..3 {
                color[axis] *= tint[axis];
            }
        }
    }
}
pub(super) fn tint_bush(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    palette: &PropPalette,
) {
    let tint = match variant % 8 {
        1 => [1.13, 1.08, 0.68],
        2 => [0.48, 0.65, 0.60],
        _ => [1.0; 3],
    };
    for (i, material) in palette.foliage.into_iter().enumerate() {
        if !palette.foliage[..i].contains(&material) {
            tint_material(batches, material, tint);
        }
    }
}

fn voxel(
    batches: &mut BTreeMap<usize, PropMesh>,
    material: usize,
    center: Vec3,
    size: Vec3,
    shade: f32,
) -> Result<(), String> {
    let start = batches.get(&material).map_or(0, |b| b.positions.len());
    add_box(batches, material, center, size, Quat::IDENTITY)?;
    if let Some(mesh) = batches.get_mut(&material) {
        mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
        mesh.colors[start..].fill([shade, shade, shade, 1.0]);
    }
    Ok(())
}

pub(super) fn grass(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    palette: &PropPalette,
) -> Result<(), String> {
    super::botany::grass(batches, variant, PropDetail::Middle, palette)?;
    let dry = variant >= 128;
    let colors = if dry {
        palette.dry_grass
    } else {
        palette.foliage
    };
    let variant = variant % 8;
    for i in 8..22 {
        let angle = i as f32 * 2.399963 + variant as f32 * 0.13;
        let radius = 0.055 + (i as f32 / 23.0).sqrt() * 0.205;
        let mut root = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        if i < 12 {
            root = [
                Vec3::new(-0.32, 0.0, 0.0),
                Vec3::new(0.32, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -0.32),
                Vec3::new(0.0, 0.0, 0.32),
            ][i - 8];
        }
        if root.x.abs() < root.z.abs() {
            root.x = root.x.clamp(-0.07, 0.07);
        } else {
            root.z = root.z.clamp(-0.07, 0.07);
        }
        let height =
            (0.24 + ((i * 13 + variant * 7) % 29) as f32 * 0.012) * [0.85, 1.0, 0.94][variant % 3];
        let width = 0.14 + ((i + variant) % 4) as f32 * 0.012;
        let material = colors[(i + variant) % 3];
        let start = batches.get(&material).map_or(0, |b| b.positions.len());
        let x_leaf = root.x.abs() >= root.z.abs();
        let sign = if if x_leaf { root.x } else { root.z } > 0.0 {
            -1.0
        } else {
            1.0
        };
        let axis = if x_leaf { Vec3::X } else { Vec3::Z } * sign;
        add_profile(
            batches,
            material,
            root,
            &grass_profile(width, height),
            axis,
            Vec3::Y,
            0.018,
        )?;
        if let Some(mesh) = batches.get_mut(&material) {
            mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
            for (color, p) in mesh.colors[start..]
                .iter_mut()
                .zip(&mesh.positions[start..])
            {
                let t = (p[1] / height).clamp(0.0, 1.0);
                *color = [0.77 + 0.45 * t, 0.87 + 0.30 * t, 0.70 + 0.11 * t, 1.0];
            }
        }
    }
    if !dry && variant % 3 == 2 {
        for (i, x) in [-0.12, 0.12].into_iter().enumerate() {
            let center = Vec3::new(x, 0.27 + i as f32 * 0.09, 0.06);
            add_stalk(
                batches,
                palette.stem,
                Vec3::new(x, 0.0, 0.06),
                Vec2::splat(0.025),
                &[Vec3::ZERO, Vec3::Y * center.y],
                0.0,
            )?;
            for size in [Vec3::new(0.10, 0.03, 0.035), Vec3::new(0.035, 0.03, 0.10)] {
                voxel(batches, palette.flowers[2], center, size, 1.0)?;
            }
            voxel(
                batches,
                palette.flowers[1],
                center + Vec3::Y * 0.02,
                Vec3::splat(0.03),
                1.0,
            )?;
        }
    }
    Ok(())
}

pub(super) fn bush(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    palette: &PropPalette,
) -> Result<(), String> {
    voxel(
        batches,
        palette.twig,
        Vec3::Y * 0.16,
        Vec3::new(0.065, 0.32, 0.065),
        0.9,
    )?;
    for i in 0..6 {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let center = Vec3::new(
            angle.cos() * 0.10,
            0.27 + (i % 2) as f32 * 0.08,
            angle.sin() * 0.10,
        );
        voxel(
            batches,
            palette.foliage[(i + variant) % 3],
            center,
            Vec3::splat(0.23),
            0.80,
        )?;
    }
    // Multiple levels and open pockets instead of twelve solid cushions.
    for i in 0..54 {
        let angle = i as f32 * 2.399963 + (variant % 8) as f32 * 0.17;
        let y = 0.13 + (i % 9) as f32 * 0.048;
        let fullness = (1.0 - ((y - 0.34) / 0.32).powi(2)).max(0.1).sqrt();
        let radius = (0.21 + (i % 3) as f32 * 0.025) * fullness;
        let center = Vec3::new(angle.cos() * radius, y, angle.sin() * radius);
        let size = 0.075 + ((i * 7 + variant) % 4) as f32 * 0.013;
        voxel(
            batches,
            palette.foliage[(i + variant) % 3],
            center,
            Vec3::new(size, size * 0.65, size * 0.85),
            0.88 + (i % 5) as f32 * 0.055,
        )?;
        if i % 6 == 0 {
            voxel(
                batches,
                palette.foliage[(i + variant + 1) % 3],
                center + Vec3::new(0.035, 0.032, -0.025),
                Vec3::splat(size * 0.65),
                1.12,
            )?;
        }
    }
    let color = match variant % 8 {
        3 => Some(0),
        4 => Some(3),
        5 => Some(4),
        6 => Some(2),
        _ => None,
    };
    if let Some(color) = color {
        for i in 0..6 {
            let a = i as f32 * std::f32::consts::TAU / 6.0;
            let p = Vec3::new(a.cos() * 0.25, 0.27 + (i % 3) as f32 * 0.09, a.sin() * 0.25);
            if variant % 8 < 5 {
                voxel(batches, palette.flowers[color], p, Vec3::splat(0.047), 1.0)?;
                voxel(
                    batches,
                    palette.flowers[color],
                    p + Vec3::new(0.035, -0.022, 0.01),
                    Vec3::splat(0.04),
                    0.9,
                )?;
            } else {
                for size in [Vec3::new(0.10, 0.032, 0.035), Vec3::new(0.035, 0.032, 0.10)] {
                    voxel(batches, palette.flowers[color], p, size, 1.0)?;
                }
                voxel(
                    batches,
                    palette.flowers[1],
                    p + Vec3::Y * 0.025,
                    Vec3::splat(0.026),
                    1.0,
                )?;
            }
        }
    }
    tint_bush(batches, variant, palette);
    Ok(())
}

pub(super) fn flower(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    palette: &PropPalette,
) -> Result<(), String> {
    let species = variant % 8;
    let color = palette.flowers[species];
    for child in 0..3 {
        let root = [
            Vec3::new(-0.10, 0.0, -0.055),
            Vec3::new(0.11, 0.0, 0.06),
            Vec3::new(0.04, 0.0, -0.11),
        ][child];
        let height = 0.43 + ((variant + child * 2) % 4) as f32 * 0.043;
        let tip = root + Vec3::Y * height;
        add_stalk(
            batches,
            palette.stem,
            root,
            Vec2::splat(0.028),
            &[Vec3::ZERO, (tip - root) * 0.55, tip - root],
            0.0,
        )?;
        for leaf in 0..3 {
            let sign = if leaf % 2 == 0 { 1.0 } else { -1.0 };
            let p = root + (tip - root) * (0.22 + leaf as f32 * 0.18);
            voxel(
                batches,
                palette.foliage[(variant + leaf) % 3],
                p + Vec3::new(sign * 0.038, 0.008, 0.0),
                Vec3::new(0.095, 0.04, 0.055),
                0.9,
            )?;
            voxel(
                batches,
                palette.foliage[(variant + leaf + 1) % 3],
                p + Vec3::new(sign * 0.078, 0.035, 0.0),
                Vec3::new(0.05, 0.055, 0.04),
                1.05,
            )?;
        }
        if species == 5 {
            for bell in 0..3 {
                let sign = if bell % 2 == 0 { 1.0 } else { -1.0 };
                let p = tip + Vec3::new(sign * 0.065, -0.17 + bell as f32 * 0.085, 0.0);
                voxel(
                    batches,
                    palette.stem,
                    p + Vec3::new(-sign * 0.03, 0.045, 0.0),
                    Vec3::new(0.07, 0.025, 0.025),
                    0.9,
                )?;
                voxel(
                    batches,
                    color,
                    p + Vec3::Y * 0.04,
                    Vec3::new(0.075, 0.035, 0.075),
                    1.08,
                )?;
                for (x, z, size) in [
                    (-0.042, 0.0, Vec3::new(0.025, 0.075, 0.10)),
                    (0.042, 0.0, Vec3::new(0.025, 0.075, 0.10)),
                    (0.0, -0.042, Vec3::new(0.065, 0.075, 0.025)),
                    (0.0, 0.042, Vec3::new(0.065, 0.075, 0.025)),
                ] {
                    voxel(batches, color, p + Vec3::new(x, -0.005, z), size, 0.92)?;
                }
            }
        } else if species == 6 {
            for (i, [x, z]) in [[-0.04, 0.0], [0.04, 0.0], [0.0, -0.04], [0.0, 0.04]]
                .into_iter()
                .enumerate()
            {
                voxel(
                    batches,
                    color,
                    tip + Vec3::new(x, 0.025 + (i % 2) as f32 * 0.025, z),
                    Vec3::new(0.065, 0.15, 0.065),
                    0.8 + i as f32 * 0.1,
                )?;
            }
        } else {
            let count = if species == 0 || species == 4 { 6 } else { 10 };
            for petal in 0..count {
                let a = petal as f32 * std::f32::consts::TAU / count as f32;
                let cupped = species == 0 || species == 4;
                let p =
                    tip + Vec3::new(a.cos() * 0.074, (petal % 2) as f32 * 0.018, a.sin() * 0.074);
                let radial = if a.cos().abs() >= a.sin().abs() {
                    Vec3::X * a.cos().signum()
                } else {
                    Vec3::Z * a.sin().signum()
                };
                if petal % 2 == 0 {
                    let lateral = if cupped {
                        Vec3::Y
                    } else {
                        radial.cross(Vec3::Y)
                    };
                    let profile = if cupped {
                        [
                            [0.0, -0.035],
                            [0.068, -0.035],
                            [0.068, 0.005],
                            [0.105, 0.005],
                            [0.105, 0.035],
                            [0.055, 0.035],
                            [0.055, 0.018],
                            [0.0, 0.018],
                        ]
                    } else {
                        [
                            [0.0, -0.032],
                            [0.068, -0.032],
                            [0.068, -0.018],
                            [0.105, -0.018],
                            [0.105, 0.018],
                            [0.068, 0.018],
                            [0.068, 0.032],
                            [0.0, 0.032],
                        ]
                    }
                    .map(Vec2::from_array);
                    let start = batches.get(&color).map_or(0, |mesh| mesh.positions.len());
                    add_profile(
                        batches,
                        color,
                        p - radial * 0.034,
                        &profile,
                        radial,
                        lateral,
                        if cupped { 0.045 } else { 0.035 },
                    )?;
                    if let Some(mesh) = batches.get_mut(&color) {
                        mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
                        for (tint, position) in mesh.colors[start..]
                            .iter_mut()
                            .zip(&mesh.positions[start..])
                        {
                            let reach = (Vec3::from_array(*position) - p).dot(radial);
                            let shade = 0.97 + (reach / 0.105).clamp(0.0, 1.0) * 0.15;
                            *tint = [shade, shade, shade, 1.0];
                        }
                    }
                } else {
                    voxel(
                        batches,
                        color,
                        p,
                        if cupped {
                            Vec3::new(0.065, 0.07, 0.065)
                        } else if radial.x != 0.0 {
                            Vec3::new(0.085, 0.035, 0.045)
                        } else {
                            Vec3::new(0.045, 0.035, 0.085)
                        },
                        0.92 + (petal % 3) as f32 * 0.06,
                    )?;
                }
            }
            voxel(
                batches,
                if species == 0 {
                    palette.mushroom
                } else {
                    palette.flowers[1]
                },
                tip + Vec3::Y * 0.025,
                Vec3::new(0.052, 0.048, 0.052),
                1.0,
            )?;
        }
    }
    Ok(())
}

pub(super) fn mushroom(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    palette: &PropPalette,
) -> Result<(), String> {
    let cap = super::mushrooms::cap(palette, variant);
    for child in 0..2 {
        let scale = if child == 0 { 1.0 } else { 0.57 };
        let root = if child == 0 {
            Vec3::new(-0.045, 0.0, -0.025)
        } else {
            Vec3::new(0.16, 0.0, 0.10)
        };
        let h = 0.24 + (variant % 3) as f32 * 0.025;
        let mut part = |material, p: Vec3, size: Vec3, shade| {
            voxel(batches, material, root + p * scale, size * scale, shade)
        };
        part(
            palette.flowers[2],
            Vec3::Y * (h * 0.5),
            Vec3::new(0.10, h, 0.10),
            0.9,
        )?;
        part(
            palette.flowers[2],
            Vec3::new(-0.012, 0.045, 0.008),
            Vec3::new(0.14, 0.09, 0.13),
            1.0,
        )?;
        for (tier, size) in [(0, 0.116), (1, 0.086)] {
            for x in -1_i32..=1 {
                for z in -1_i32..=1 {
                    if x.abs() + z.abs() == 2 && tier == 0 {
                        continue;
                    }
                    let p = Vec3::new(
                        x as f32 * size * 0.95,
                        h + 0.025 + tier as f32 * 0.065,
                        z as f32 * size * 0.95,
                    );
                    part(
                        cap,
                        p,
                        Vec3::new(size, 0.07, size),
                        0.87 + ((x + z + 4) % 3) as f32 * 0.08,
                    )?;
                    if child == 0 && tier == 0 && (x == 0 || z == 0) {
                        part(
                            palette.flowers[2],
                            p - Vec3::Y * 0.045,
                            Vec3::new(size * 0.8, 0.025, size * 0.8),
                            0.83,
                        )?;
                    }
                }
            }
        }
        part(cap, Vec3::Y * (h + 0.14), Vec3::new(0.15, 0.035, 0.14), 1.1)?;
        for (i, [x, z]) in [[-0.07, -0.04], [0.06, 0.055], [-0.03, 0.07], [0.02, -0.07]]
            .into_iter()
            .enumerate()
        {
            part(
                if variant % 8 == 5 {
                    palette.twig
                } else {
                    palette.flowers[2]
                },
                Vec3::new(x, h + if i == 2 { 0.16 } else { 0.13 }, z),
                Vec3::new(0.045, 0.018, 0.04),
                1.0,
            )?;
        }
    }
    if variant % 8 == 7 {
        tint_material(batches, cap, [1.1, 0.38, 0.055]);
    }
    Ok(())
}
