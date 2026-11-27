//! Basalt scutes overlap a hot flexible core. Each ring is a cached rigid mesh.
use super::{Part, voxel::Builder};
use bevy::prelude::*;
use terra_core::creatures::lava_worm::{SEGMENTS, segment_radius};

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut result = Vec::new();
    for i in 0..SEGMENTS {
        let r = segment_radius(i);
        let mut shell = Builder::with_resolution(if i < 3 { 22 } else { 15 });
        let mut core = Builder::with_resolution(if i < 3 { 16 } else { 12 });
        let plates = if i < 8 { 10 } else { 8 };
        for n in 0..plates {
            let angle = n as f32 * std::f32::consts::TAU / plates as f32 + f32::from(i % 2) * 0.24;
            let (s, c) = angle.sin_cos();
            let rot = Quat::from_rotation_z(angle) * Quat::from_rotation_x(-0.08);
            let color = match (n + usize::from(i)) % 3 {
                0 => [0.42, 0.29, 0.34],
                1 => [0.55, 0.34, 0.32],
                _ => [0.34, 0.22, 0.26],
            };
            shell.rotated(
                [c * r * 0.81, s * r * 0.81, 0.02],
                [r * 0.38, r * 0.56, 0.57],
                color,
                rot,
            )?;
            // Lighter overlapping scutes catch the rim light. Break their
            // alignment between rings so the basalt reads as cracked armour.
            if (n + usize::from(i)) % 3 != 0 {
                let center = Vec3::new(c, s, 0.) * (r * 0.98) + Vec3::new(-s, c, 0.) * (r * 0.065);
                shell.rotated(
                    [center.x, center.y, -0.095],
                    [r * 0.12, r * 0.34, 0.26],
                    color.map(|v| v * 1.24),
                    rot,
                )?;
            }
            // A narrow hot seam rises between cracked, overlapping armour plates.
            if (n + usize::from(i)).is_multiple_of(4) {
                core.rotated(
                    [r * c * 0.94, r * s * 0.94, 0.18],
                    [0.07, r * 0.26, 0.24],
                    [1., 0.32, 0.018],
                    rot,
                )?;
            }
        }
        core.ellipsoid(
            Vec3::new(0., 0., 0.09),
            Vec3::new(r * 0.79, r * 0.79, 0.32),
            [1., 0.25, 0.012],
        )?;
        if i == 0 {
            core.ellipsoid(
                Vec3::new(0., 0., -0.22),
                Vec3::new(r * 0.4, r * 0.4, 0.145),
                [1., 0.70, 0.085],
            )?;
        }
        if matches!(i, 0 | 2 | 5 | 8 | 11 | 13) {
            for (x, y, width, tilt) in [(-0.16, 1.05, 0.24, 0.25), (0.03, 0.99, 0.16, -0.28)] {
                core.rotated(
                    [r * x, r * y, 0.10],
                    [r * width, r * width * 1.75, r * width * 1.2],
                    [1., 0.48, 0.03],
                    Quat::from_rotation_z(tilt),
                )?;
            }
        }
        result.push((
            Part::LavaRing {
                index: i,
                glowing: false,
            },
            Vec3::ZERO,
            shell.finish()?,
        ));
        result.push((
            Part::LavaRing {
                index: i,
                glowing: true,
            },
            Vec3::ZERO,
            core.finish()?,
        ));
    }
    for blade in 0..6 {
        let mut jaw = Builder::with_resolution(12);
        jaw.ellipsoid(
            Vec3::new(0., -0.06, -0.15),
            Vec3::new(0.175, 0.12, 0.25),
            [0.50, 0.29, 0.25],
        )?;
        jaw.rotated(
            [0., -0.15, -0.31],
            [0.125, 0.16, 0.17],
            [0.77, 0.47, 0.27],
            Quat::from_rotation_x(-0.54),
        )?;
        result.push((Part::LavaJaw(blade), Vec3::ZERO, jaw.finish()?));
    }
    for i in 0..4 {
        let mut flame = Builder::with_resolution(10);
        let radius = 0.35 - f32::from(i) * 0.063;
        flame.ellipsoid(
            Vec3::ZERO,
            Vec3::new(radius, radius, radius * 1.1),
            if i < 2 {
                [1., 0.66, 0.06]
            } else {
                [1., 0.19, 0.01]
            },
        )?;
        result.push((Part::WormSpit(i), Vec3::ZERO, flame.finish()?));
    }
    Ok(result)
}
