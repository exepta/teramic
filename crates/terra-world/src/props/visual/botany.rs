use super::geometry::{add_box, add_ribbon, add_stalk};
use super::{PropDetail, PropMesh, PropPalette};
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

pub(super) fn grass(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let dry = variant >= 128;
    let foliage = if dry {
        palette.dry_grass
    } else {
        palette.foliage
    };
    let variant = variant % 8;
    // Four outer leaves establish the silhouette, then shorter offshoots fill
    // the heart of the tuft. Space between the fans belongs to companion props.
    // (root x, root z, outward axis, height, reach), all in template metres.
    let blades = [
        (-0.154, -0.036, Vec3::NEG_X, 0.38, 0.188),
        (0.155, 0.057, Vec3::X, 0.48, 0.189),
        (0.029, -0.151, Vec3::NEG_Z, 0.42, 0.193),
        (-0.045, 0.159, Vec3::Z, 0.33, 0.184),
        (-0.035, 0.024, Vec3::NEG_X, 0.57, 0.114),
        (0.038, -0.015, Vec3::Z, 0.53, 0.110),
        (0.124, -0.058, Vec3::X, 0.30, 0.143),
        (-0.065, -0.091, Vec3::NEG_Z, 0.39, 0.137),
        (0.062, 0.127, Vec3::Z, 0.22, 0.110),
        (-0.147, 0.060, Vec3::NEG_X, 0.25, 0.105),
    ];
    let count = match level {
        PropDetail::Near => blades.len(),
        PropDetail::Middle => 8,
        PropDetail::Far => 4,
    };
    for (i, &(x, z, axis, height, reach)) in blades[..count].iter().enumerate() {
        let root = Vec3::new(x, 0., z);
        let height = height * [0.91, 1.03, 0.97][(i + variant) % 3];
        let reach = reach * [0.98, 1.02, 1.0][(i * 5 + variant) % 3];
        let thickness =
            (0.032 + ((i * 3 + variant) % 4) as f32 * 0.005) * if dry { 0.83 } else { 1.0 };
        let material = foliage[(i + variant) % 3];
        let start = batches.get(&material).map_or(0, |b| b.positions.len());
        if level == PropDetail::Near {
            super::leaf::add_profile(
                batches,
                material,
                root,
                &super::leaf::grass_profile(reach, height, i < 4),
                axis,
                Vec3::Y,
                thickness,
            )?;
        } else {
            // Join the centres of the same blunt root and tip as the stepped near mesh.
            let tip = Vec3::Y * height + axis * (0.86 * reach);
            let width = 0.46 * reach;
            let yaw = if axis.x != 0.0 {
                0.0
            } else {
                std::f32::consts::FRAC_PI_2
            };
            if level == PropDetail::Middle {
                add_stalk(
                    batches,
                    material,
                    root,
                    Vec2::new(width, thickness),
                    &[Vec3::ZERO, tip],
                    yaw,
                )?;
            } else {
                add_ribbon(batches, material, root, width, tip, yaw)?;
            }
        }
        if let Some(mesh) = batches.get_mut(&material) {
            mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
            let (base, tip) = if dry {
                (Vec3::new(0.80, 0.75, 0.61), Vec3::new(1.12, 1.05, 0.83))
            } else {
                (Vec3::new(0.69, 0.78, 0.59), Vec3::new(1.07, 1.15, 0.77))
            };
            for (color, p) in mesh.colors[start..]
                .iter_mut()
                .zip(&mesh.positions[start..])
            {
                let shade = base.lerp(tip, (p[1] / height).clamp(0., 1.));
                *color = [shade.x, shade.y, shade.z, 1.];
            }
        }
    }
    if level == PropDetail::Near && !dry && variant % 3 == 2 {
        super::botanical_detail::grass_flowers(batches, palette)?;
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
        let lobe_scale = [1.0, 0.94, 1.035, 0.965, 1.06, 0.915][i % 6];
        let broad = if level == PropDetail::Far { 0.36 } else { 0.26 };
        let size = Vec3::new(broad, if i == 0 { 0.34 } else { 0.24 }, broad) * lobe_scale;
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
    // Neutral vertex color preserves bleached wood through all biome seasons.
    let mesh = batches.entry(palette.crop).or_insert_with(|| PropMesh {
        material: palette.crop,
        ..Default::default()
    });
    let yaw = Quat::from_rotation_y(variant as f32 * 0.61);
    let stretch = [0.82, 1.04, 0.92, 1.12][variant % 4];
    let warp = |p: Vec3| yaw * Vec3::new(p.x, p.y * stretch, p.z);
    let profile = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]];
    let mut branch = |points: &[(Vec3, f32)]| -> Result<(), String> {
        let ring = |point: Vec3, radius: f32| -> Vec<Vec3> {
            profile
                .iter()
                .map(|[x, z]| warp(point + Vec3::new(x * radius, 0., z * radius)))
                .collect()
        };
        let mut face = |vertices: &[Vec3], tone: f32| -> Result<(), String> {
            let normal = (vertices[1] - vertices[0])
                .cross(vertices[2] - vertices[0])
                .normalize_or_zero();
            let start =
                u32::try_from(mesh.positions.len()).map_err(|_| "Dead bush mesh overflow")?;
            mesh.positions.extend(vertices.iter().map(|p| p.to_array()));
            mesh.normals
                .extend(std::iter::repeat_n(normal.to_array(), vertices.len()));
            mesh.uvs
                .extend(std::iter::repeat_n([0., 0.], vertices.len()));
            mesh.colors.resize(start as usize, [1.; 4]);
            mesh.colors.extend(std::iter::repeat_n(
                [tone * 0.64, tone * 0.39, tone * 0.17, 1.],
                vertices.len(),
            ));
            for i in 1..vertices.len() - 1 {
                mesh.indices
                    .extend([start, start + i as u32, start + i as u32 + 1]);
            }
            Ok(())
        };
        for (index, segment) in points.windows(2).enumerate() {
            let a = ring(segment[0].0, segment[0].1);
            let b = ring(segment[1].0, segment[1].1);
            for edge in 0..a.len() {
                let next = (edge + 1) % a.len();
                face(
                    &[a[edge], b[edge], b[next], a[next]],
                    [0.92, 1.08, 0.97, 0.87][edge] * (0.97 + index as f32 * 0.015),
                )?;
            }
        }
        let &(tip, radius) = points.last().expect("Dry branch has points");
        let mut cap = ring(tip, radius);
        cap.reverse();
        face(&cap, 1.18)
    };
    let trunk = [
        (Vec3::new(0., 0., 0.), 0.075),
        (Vec3::new(-0.04, 0.10, 0.02), 0.056),
        (Vec3::new(-0.015, 0.23, 0.025), 0.046),
        (Vec3::new(0.055, 0.34, 0.005), 0.036),
        (Vec3::new(0.09, 0.47, -0.02), 0.026),
        (Vec3::new(0.055, 0.61, -0.04), 0.016),
    ];
    branch(&trunk)?;
    if level != PropDetail::Far {
        for root in [
            Vec3::new(-0.15, 0., 0.055),
            Vec3::new(0.14, 0., -0.035),
            Vec3::new(0.035, 0., 0.14),
        ] {
            branch(&[(root, 0.028), (Vec3::new(-0.03, 0.10, 0.02), 0.042)])?;
        }
    }
    let count = if level == PropDetail::Far { 3 } else { 5 };
    for i in 0..count {
        let angle = i as f32 * 2.399_963 + variant as f32 * 0.17;
        let direction = Vec3::new(angle.sin(), 0., angle.cos());
        let root = trunk[1 + i % 4].0;
        let elbow = root + direction * (0.11 + (i % 2) as f32 * 0.025) + Vec3::Y * 0.035;
        let wrist = elbow + direction * 0.055 + Vec3::Y * 0.075;
        let tip = wrist + direction * 0.055 + Vec3::Y * 0.045;
        branch(&[(root, 0.028), (elbow, 0.023), (wrist, 0.016), (tip, 0.010)])?;
        if level == PropDetail::Near {
            let side = Vec3::new(direction.z, 0., -direction.x) * if i % 2 == 0 { 1. } else { -1. };
            let fork = elbow + direction * 0.055 + side * 0.045 + Vec3::Y * 0.015;
            branch(&[
                (elbow, 0.017),
                (fork, 0.013),
                (fork + side * 0.035 + Vec3::Y * 0.07, 0.009),
            ])?;
        }
    }
    Ok(())
}
