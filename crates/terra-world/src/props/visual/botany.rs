use super::geometry::{add_blade, add_box, add_ribbon, add_stalk};
use super::{PropDetail, PropMesh, PropPalette};
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

pub(super) fn grass(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    if level == PropDetail::Near {
        return super::botanical_detail::grass(batches, variant, palette);
    }
    let foliage = if variant >= 128 {
        palette.dry_grass
    } else {
        palette.foliage
    };
    let dry = variant >= 128;
    let variant = variant % 8;
    let silhouette = variant % 3;
    // Corners stay open for other plants sharing this soil cell.
    // Thick, upright square shoots; gaps remain real geometry, including for picking.
    let blades = [
        (-0.23, 0.0, 0.46),
        (0.23, 0.0, 0.52),
        (0.0, -0.23, 0.40),
        (0.0, 0.23, 0.45),
        (-0.065, 0.025, 0.59),
        (0.075, -0.035, 0.53),
        (-0.10, -0.045, 0.43),
        (0.10, 0.045, 0.48),
        (-0.215, 0.06, 0.28),
        (0.215, -0.06, 0.31),
        (-0.06, 0.215, 0.30),
        (0.06, -0.215, 0.27),
    ];
    let count = match level {
        PropDetail::Near => blades.len(),
        PropDetail::Middle => 8,
        PropDetail::Far => 4,
    };
    for (i, &(x, z, height)) in blades[..count].iter().enumerate() {
        let root = Vec3::new(
            x + (((i * 7 + variant * 3) % 5) as f32 - 2.0) * 0.002,
            0.0,
            z + (((i * 3 + variant * 7) % 5) as f32 - 2.0) * 0.002,
        );
        let phase = if i % 2 == 0 {
            0.0
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let height =
            height * [0.80, 1.0, 0.89][silhouette] + ((i * 5 + variant) % 3) as f32 * 0.009;
        let width =
            (0.040 + ((i * 3 + variant) % 4) as f32 * 0.004) * if i >= 8 { 0.82 } else { 1.0 };
        let bend = Vec3::ZERO;
        let material = foliage[(i + variant) % 3];
        let start = batches.get(&material).map_or(0, |b| b.positions.len());
        if level == PropDetail::Far {
            add_ribbon(
                batches,
                material,
                root,
                width,
                Vec3::Y * height + bend,
                phase,
            )?;
        } else {
            add_stalk(
                batches,
                material,
                root,
                Vec2::splat(width),
                &[Vec3::ZERO, Vec3::Y * height + bend],
                phase,
            )?;
        }
        if let Some(batch) = batches.get_mut(&material) {
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            let (base, tip) = if dry {
                ([0.84, 0.84, 0.80], [1.14, 1.10, 0.94])
            } else {
                ([0.77, 0.87, 0.70], [1.22, 1.17, 0.81])
            };
            for (color, point) in batch.colors[start..]
                .iter_mut()
                .zip(&batch.positions[start..])
            {
                let weight = (point[1] / height).clamp(0.0, 1.0);
                *color = [
                    base[0] + (tip[0] - base[0]) * weight,
                    base[1] + (tip[1] - base[1]) * weight,
                    base[2] + (tip[2] - base[2]) * weight,
                    1.0,
                ];
            }
        }
    }
    if level == PropDetail::Near {
        for (i, [x, z]) in [
            [-1.0, 0.0],
            [1.0, 0.0],
            [0.0, -1.0],
            [0.0, 1.0],
            [-1.0, 0.0],
            [1.0, 0.0],
        ]
        .into_iter()
        .enumerate()
        {
            let direction = Vec3::new(x, 0.0, z);
            let offset = if i >= 4 {
                Vec3::Z * (-x * 0.055)
            } else {
                Vec3::ZERO
            };
            add_box(
                batches,
                foliage[(i + variant) % 3],
                direction * 0.267 + offset + Vec3::Y * (0.07 + (i % 3) as f32 * 0.065),
                Vec3::new(0.10, 0.11, 0.15),
                Quat::from_rotation_y(x.atan2(z)),
            )?;
        }
        if !dry && variant % 3 == 2 {
            let root = Vec3::new(-0.13, 0.0, 0.07);
            add_blade(batches, palette.stem, root, 0.22, 0.035, Vec3::ZERO, 0.0)?;
            for size in [
                Vec3::new(0.095, 0.026, 0.032),
                Vec3::new(0.032, 0.026, 0.095),
            ] {
                add_box(
                    batches,
                    palette.flowers[2],
                    root + Vec3::Y * 0.23,
                    size,
                    Quat::IDENTITY,
                )?;
            }
            add_box(
                batches,
                palette.flowers[1],
                root + Vec3::Y * 0.249,
                Vec3::new(0.027, 0.015, 0.027),
                Quat::IDENTITY,
            )?;
        }
    }
    Ok(())
}

pub(super) fn bush(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    if variant >= 128 {
        return dead_bush(batches, variant % 8, level, palette);
    }
    if level == PropDetail::Near {
        return super::botanical_detail::bush(batches, variant, palette);
    }
    add_box(
        batches,
        palette.twig,
        Vec3::new(0.0, 0.16, 0.0),
        Vec3::new(0.085, 0.32, 0.085),
        Quat::IDENTITY,
    )?;
    // Overlapping cubic leaf cushions form a connected, stepped crown.
    let cushions = [
        [0.0, 0.36, 0.0],
        [-0.15, 0.32, 0.0],
        [0.15, 0.32, 0.0],
        [0.0, 0.32, -0.15],
        [0.0, 0.32, 0.15],
        [0.0, 0.53, 0.0],
        [-0.13, 0.47, -0.12],
        [0.13, 0.47, 0.12],
        [-0.13, 0.47, 0.12],
        [0.13, 0.47, -0.12],
        [-0.12, 0.20, 0.12],
        [0.12, 0.20, -0.12],
    ];
    let count = match level {
        PropDetail::Near => 12,
        PropDetail::Middle => 6,
        PropDetail::Far => 3,
    };
    for (i, center) in cushions[..count].iter().enumerate() {
        let broad = if level == PropDetail::Far { 0.36 } else { 0.26 };
        let size = Vec3::new(broad, if i == 0 { 0.34 } else { 0.24 }, broad);
        let center = Vec3::from_array(*center);
        add_box(
            batches,
            palette.foliage[(i + variant) % 3],
            center,
            size,
            Quat::IDENTITY,
        )?;
        if level == PropDetail::Near && i >= 6 && matches!(variant % 8, 2..=5) {
            let material = palette.flowers[match variant % 8 {
                2 => 0,
                3 => 3,
                4 => 4,
                _ => 2,
            }];
            add_box(
                batches,
                material,
                center + Vec3::new(0.0, 0.065, if i % 2 == 0 { 0.13 } else { -0.13 }),
                Vec3::splat(0.06),
                Quat::IDENTITY,
            )?;
        }
    }
    super::botanical_detail::tint_bush(batches, variant, palette);
    Ok(())
}

fn dead_bush(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let mut branch = |root: Vec3, tip: Vec3, width: f32| {
        let delta = tip - root;
        add_box(
            batches,
            palette.twig,
            (root + tip) * 0.5,
            Vec3::new(width, delta.length(), width),
            Quat::from_rotation_arc(Vec3::Y, delta.normalize()),
        )
    };
    branch(Vec3::ZERO, Vec3::new(0.0, 0.54, 0.0), 0.05)?;
    let count = match level {
        PropDetail::Near => 5,
        PropDetail::Middle => 4,
        PropDetail::Far => 3,
    };
    for i in 0..count {
        let angle = i as f32 * 2.399_963 + variant as f32 * 0.53;
        let direction = Vec3::new(angle.sin(), 0.0, angle.cos());
        let root = Vec3::Y * (0.12 + (i % 3) as f32 * 0.08);
        let elbow = direction * (0.16 + (i % 2) as f32 * 0.03) + Vec3::Y * 0.35;
        let tip = direction * 0.24 + Vec3::Y * (0.52 + (i % 3) as f32 * 0.05);
        branch(root, elbow, 0.032)?;
        branch(elbow, tip, 0.023)?;
        if level == PropDetail::Near && i % 2 == 0 {
            branch(elbow, elbow + direction * 0.09 + Vec3::Y * 0.05, 0.018)?;
        }
    }
    Ok(())
}
