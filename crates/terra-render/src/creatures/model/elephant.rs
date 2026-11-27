//! Heavy barrel torso, fan ears, tusks and an independently swinging trunk.
use super::{Part, shape, voxel::Builder};
use bevy::prelude::*;

type Color = [f32; 3];
const HIDE: Color = [0.47, 0.445, 0.432];
const SHADE: Color = [0.385, 0.36, 0.348];
const DUST: Color = [0.55, 0.498, 0.452];
const IVORY: Color = [0.89, 0.84, 0.69];

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::with_cell_size(0.079);
    for (center, radius) in [
        ([0., 1.70, 0.24], [0.72, 0.81, 1.27]),
        ([0., 1.92, -0.43], [0.64, 0.77, 0.72]),
        ([0., 1.75, 0.86], [0.68, 0.79, 0.67]),
    ] {
        body.ellipsoid(Vec3::from_array(center), Vec3::from_array(radius), HIDE)?;
    }
    let mut head = Builder::with_cell_size(0.045);
    head.ellipsoid(
        Vec3::new(0., 0.12, -0.14),
        Vec3::new(0.51, 0.58, 0.57),
        HIDE,
    )?;
    for s in [-1., 1.] {
        head.ellipsoid(
            Vec3::new(s * 0.24, 0.34, -0.24),
            Vec3::new(0.29, 0.39, 0.36),
            HIDE,
        )?;
        head.voxel(
            [s * 0.45, 0.14, -0.46],
            [0.075, 0.105, 0.11],
            [0.085, 0.065, 0.05],
        )?;
        head.voxel([s * 0.488, 0.155, -0.476], [0.022, 0.031, 0.029], IVORY)?;
        let mut last = Vec3::new(s * 0.28, -0.12, -0.52);
        for i in 1..=5 {
            let t = i as f32 / 5.;
            let next = Vec3::new(
                s * (0.28 + t * 0.175),
                -0.12 - 0.47 * t + 0.46 * t * t,
                -0.52 - t * 0.52,
            );
            shape::segment(
                &mut head,
                last,
                next,
                0.125 * (1.06 - t * 0.82),
                0.13 * (1.06 - t * 0.82),
                IVORY,
            )?;
            last = next;
        }
    }
    let head_root = super::ELEPHANT_HEAD;
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, head_root, head.finish()?),
    ];
    for s in [-1., 1.] {
        let mut ear = Builder::with_cell_size(0.053);
        for (x, y, w, h) in [
            (0.15, 0.20, 0.41, 0.56),
            (0.24, -0.09, 0.54, 0.50),
            (0.18, -0.38, 0.35, 0.36),
            (0.09, -0.55, 0.20, 0.20),
        ] {
            ear.ellipsoid(
                Vec3::new(s * x, y, 0.09 + x * 0.46),
                Vec3::new(w * 0.5, h * 0.64, 0.14),
                DUST,
            )?;
        }
        parts.push((
            Part::ElephantEar(s),
            head_root + Vec3::new(s * 0.43, 0.16, -0.12),
            ear.finish()?,
        ));
        for (z, phase) in [(-0.54, s), (0.86, -s)] {
            let mut leg = Builder::with_cell_size(0.058);
            leg.ellipsoid(Vec3::new(0., -0.24, 0.), Vec3::new(0.25, 0.46, 0.28), HIDE)?;
            shape::rounded(&mut leg, [0., -0.75, 0.], [0.40, 0.86, 0.43], 0.08, HIDE)?;
            shape::rounded(
                &mut leg,
                [0., -1.17, -0.015],
                [0.46, 0.25, 0.50],
                0.07,
                SHADE,
            )?;
            for n in -1..=1 {
                leg.voxel(
                    [n as f32 * 0.114, -1.232, -0.236],
                    [0.086, 0.101, 0.10],
                    DUST,
                )?;
            }
            parts.push((
                Part::Leg(phase),
                Vec3::new(s * 0.47, 1.295, z),
                leg.finish()?,
            ));
        }
    }
    let mut trunk = Builder::with_cell_size(0.041);
    for i in 0..14 {
        let t = i as f32 / 13.;
        let offset = -0.15 * t - 0.23 * t * t + 0.23 * t.powi(5);
        let r = 0.226 - t * 0.122;
        trunk.ellipsoid(
            Vec3::new(t * t * 0.09, -1.55 * t, offset),
            Vec3::new(r, r * 0.85, r),
            if i % 4 == 2 { SHADE } else { HIDE },
        )?;
    }
    parts.push((
        Part::ElephantTrunk,
        head_root + Vec3::new(0., -0.10, -0.54),
        trunk.finish()?,
    ));
    let mut tail = Builder::with_cell_size(0.045);
    shape::segment(
        &mut tail,
        Vec3::ZERO,
        Vec3::new(0., -0.83, 0.22),
        0.081,
        0.09,
        HIDE,
    )?;
    tail.ellipsoid(
        Vec3::new(0., -0.88, 0.23),
        Vec3::new(0.069, 0.18, 0.085),
        SHADE,
    )?;
    parts.push((Part::Tail, Vec3::new(0., 2.03, 1.39), tail.finish()?));
    Ok(parts)
}
