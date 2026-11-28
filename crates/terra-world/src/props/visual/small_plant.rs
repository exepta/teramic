//! The live SmallPlant is a shore reed. Its blades and russet seed spikes
//! share these silhouettes between section batches, previews and picking.
use super::{
    PropDetail, PropMesh, PropPalette, geometry::add_box, geometry::add_stalk, leaf::add_profile,
};
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

fn shade(mesh: &mut PropMesh, start: usize, bottom: f32, height: f32, base: Vec3, tip: Vec3) {
    mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
    for (color, p) in mesh.colors[start..]
        .iter_mut()
        .zip(&mesh.positions[start..])
    {
        let t = ((p[1] - bottom) / height).clamp(0.0, 1.0);
        *color = base.lerp(tip, t).extend(1.0).to_array();
    }
}

pub(super) fn append(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let variant = variant % 8;
    let yaw = variant as f32 * 0.41;
    let rotation = Quat::from_rotation_y(yaw);
    let height = [0.90, 0.86, 0.87, 0.83][variant % 4];
    let heads = if level != PropDetail::Far && variant % 3 != 1 {
        2
    } else {
        1
    };
    for i in 0..heads {
        let (root, height) = if i == 0 {
            (Vec3::ZERO, height)
        } else {
            (rotation * Vec3::new(-0.075, 0.0, 0.048), height * 0.74)
        };
        // A thin awn continues just above the ripe head.
        add_stalk(
            batches,
            palette.stem,
            root,
            Vec2::splat(0.022),
            &[Vec3::ZERO, Vec3::Y * height],
            yaw,
        )?;
        let bottom = root + Vec3::Y * height * 0.70;
        let length = height * 0.235;
        let width = if i == 0 { 0.067 } else { 0.054 };
        let start = batches
            .get(&palette.twig)
            .map_or(0, |mesh| mesh.positions.len());
        if level == PropDetail::Far {
            add_box(
                batches,
                palette.twig,
                bottom + Vec3::Y * length * 0.5,
                Vec3::new(width, length, width * 0.78),
                rotation,
            )?;
        } else {
            let outline = [
                [-0.30, 0.00],
                [0.49, 0.00],
                [0.49, 0.83],
                [0.25, 0.83],
                [0.25, 1.00],
                [-0.49, 1.00],
                [-0.49, 0.13],
                [-0.30, 0.13],
            ]
            .map(|[x, y]| Vec2::new(x * width, y * length));
            add_profile(
                batches,
                palette.twig,
                bottom,
                &outline,
                rotation * Vec3::X,
                Vec3::Y,
                width * 0.78,
            )?;
        }
        if let Some(mesh) = batches.get_mut(&palette.twig) {
            shade(
                mesh,
                start,
                bottom.y,
                length,
                Vec3::new(0.82, 0.64, 0.48),
                Vec3::new(1.33, 1.06, 0.87),
            );
        }
    }

    // Long outside leaves frame the taller seedheads. Vary their lengths and
    // turn the whole crown together so their spacing survives a change of LOD.
    let count = match level {
        PropDetail::Near => 5,
        PropDetail::Middle => 4,
        PropDetail::Far => 3,
    };
    for (i, &(angle, reach, rise)) in [
        (0.12, 0.175, 0.58),
        (2.49, 0.195, 0.43),
        (4.43, 0.17, 0.51),
        (1.42, 0.13, 0.34),
        (3.47, 0.14, 0.30),
    ][..count]
        .iter()
        .enumerate()
    {
        let angle = yaw + angle;
        let direction = Quat::from_rotation_y(angle) * Vec3::X;
        let root = direction * 0.018;
        let height = rise * (0.88 + ((variant + 2 * i) % 4) as f32 * 0.07);
        let slot = palette.foliage[(variant + i) % 3];
        let start = batches.get(&slot).map_or(0, |part| part.positions.len());
        if level == PropDetail::Far {
            add_stalk(
                batches,
                slot,
                root,
                Vec2::new(0.043, 0.032),
                &[Vec3::ZERO, direction * reach + Vec3::Y * height],
                angle,
            )?;
        } else {
            // Small squared shoulders interrupt the sweep. The upper blade
            // narrows to a blunt tip like the larger underwater leaves.
            let outline = [
                [-0.13, 0.0],
                [0.18, 0.0],
                [0.25, 0.25],
                [0.42, 0.25],
                [0.64, 0.51],
                [0.78, 0.51],
                [1.0, 1.0],
                [0.86, 1.0],
                [0.56, 0.75],
                [0.42, 0.75],
                [0.15, 0.47],
                [0.03, 0.47],
            ]
            .map(|[x, y]| Vec2::new(x * reach, y * height));
            let coarse = [
                outline[0],
                outline[1],
                outline[4],
                outline[5],
                outline[6],
                outline[7],
                outline[9],
                outline[11],
            ];
            add_profile(
                batches,
                slot,
                root,
                if level == PropDetail::Near {
                    &outline
                } else {
                    &coarse
                },
                direction,
                Vec3::Y,
                0.032,
            )?;
        }
        if let Some(mesh) = batches.get_mut(&slot) {
            shade(
                mesh,
                start,
                0.0,
                height,
                Vec3::new(0.70, 0.85, 0.88),
                Vec3::new(1.18, 1.11, 0.76),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::{PropKind, visual::template};

    #[test]
    fn living_reed_keeps_a_leaf_fan_and_seed_spike_at_every_lod() {
        let palette = PropPalette {
            foliage: [1, 2, 3],
            stem: 4,
            twig: 5,
            stone: 6,
            ..Default::default()
        };
        for variant in 0..8 {
            for level in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let meshes = template(PropKind::SmallPlant, variant, level, &palette).unwrap();
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                for leaf in meshes
                    .iter()
                    .filter(|m| palette.foliage.contains(&m.material))
                {
                    assert_eq!(leaf.colors.len(), leaf.positions.len());
                    for point in &leaf.positions {
                        let point = Vec3::from_array(*point);
                        min = min.min(point);
                        max = max.max(point);
                    }
                }
                assert_eq!(min.y, 0.0);
                assert!(
                    max.x - min.x > 0.25 && max.z - min.z > 0.25,
                    "reed blades must fan out on both axes"
                );
                let spike = meshes.iter().find(|m| m.material == palette.twig).unwrap();
                assert!(spike.positions.iter().any(|p| p[1] > max.y));
                let stones =
                    template(PropKind::SmallPlant, variant + 128, level, &palette).unwrap();
                assert_eq!(stones.len(), 1);
                assert_eq!(stones[0].material, palette.stone);
            }
        }
    }
}
