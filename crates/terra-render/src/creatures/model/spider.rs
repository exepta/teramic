use super::{Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

const DARK: [f32; 3] = [0.115, 0.105, 0.135];
const SHELL: [f32; 3] = [0.23, 0.20, 0.255];
const ORANGE: [f32; 3] = [0.33, 0.245, 0.19];
const RED: [f32; 3] = [0.65, 0.055, 0.065];

pub(super) fn parts(kind: Kind) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let [w, h, l] = match kind {
        Kind::SmallSpider => [0.145, 0.1, 0.195],
        Kind::MediumSpider => [0.645, 0.5, 0.695],
        Kind::LargeSpider => [1.345, 1.25, 1.895],
        _ => return Err("Not a spider species".to_owned()),
    };
    // A closed abdomen with a narrower thorax, rather than a hollow tiled shell.
    // Its exact bounds still match the authoritative body dimensions.
    let mut body = Builder::with_resolution(18);
    body.ellipsoid(
        Vec3::new(0.0, h * 0.85, l * 0.17),
        Vec3::new(w * 0.5, h * 0.5, l * 0.33),
        DARK,
    )?;
    body.ellipsoid(
        Vec3::new(0.0, h * 0.70, -l * 0.30),
        Vec3::new(w * 0.35, h * 0.33, l * 0.20),
        SHELL,
    )?;
    let body = body.finish_painted(|p, tint| {
        let dorsal = p.y > h * 1.08 && p.z > -l * 0.15;
        let stripe = p.x.abs() < w * 0.11;
        let mark = (p.z / l * 9.0).floor() as i32;
        if dorsal && (stripe || mark.rem_euclid(3) == 1 && p.x.abs() < w * 0.30) {
            RED
        } else if p.y > h * 0.88 && mark.rem_euclid(4) == 0 {
            SHELL
        } else {
            tint
        }
    })?;
    let mut head = Builder::default();
    let mut eyes = Builder::default();
    for i in 0..3 {
        head.voxel(
            [0.0, (i as f32 - 1.0) * h * 0.085, 0.0],
            [w * [0.57, 0.65, 0.52][i], h * 0.12, l * 0.24],
            if i == 2 { SHELL } else { DARK },
        )?;
    }
    for side in [-1.0, 1.0] {
        for i in 0..2 {
            eyes.voxel(
                [
                    side * w * (0.12 + i as f32 * 0.12),
                    h * (0.025 + i as f32 * 0.10),
                    -l * (0.145 - i as f32 * 0.005),
                ],
                [
                    w * if i == 0 { 0.13 } else { 0.065 },
                    h * if i == 0 { 0.16 } else { 0.08 },
                    l * 0.035,
                ],
                [0.95, 0.035, 0.012],
            )?;
            eyes.voxel(
                [
                    side * w * (0.12 + i as f32 * 0.12),
                    h * (0.03 + i as f32 * 0.10),
                    -l * (0.165 - i as f32 * 0.005),
                ],
                [w * if i == 0 { 0.035 } else { 0.02 }, h * 0.035, l * 0.009],
                [1.0, 0.65, 0.16],
            )?;
        }
        shape::segment(
            &mut head,
            Vec3::new(side * w * 0.20, -h * 0.04, -l * 0.10),
            Vec3::new(side * w * 0.22, -h * 0.25, -l * 0.20),
            w * 0.10,
            w * 0.11,
            RED,
        )?;
        shape::segment(
            &mut head,
            Vec3::new(side * w * 0.22, -h * 0.25, -l * 0.20),
            Vec3::new(side * w * 0.11, -h * 0.30, -l * 0.22),
            w * 0.075,
            w * 0.08,
            DARK,
        )?;
    }
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body),
        (
            Part::Head,
            Vec3::new(0.0, h * 0.66, -l * 0.46),
            head.finish()?,
        ),
        (
            Part::SpiderEyes,
            Vec3::new(0.0, h * 0.66, -l * 0.46),
            eyes.finish()?,
        ),
    ];
    let mut thread = Builder::default();
    thread.voxel(
        [0.0, 0.5, 0.0],
        [w * 0.012, 1.0, w * 0.012],
        [0.88, 0.82, 0.70],
    )?;
    parts.push((Part::Silk, Vec3::ZERO, thread.finish()?));
    for side in [-1.0, 1.0] {
        for index in 0..4 {
            let z = (index as f32 - 1.5) * l * 0.19;
            let origin = Vec3::new(side * w * 0.32, h * 0.71, z);
            let fan = (index as f32 - 1.5) * l * 0.20;
            let knee = Vec3::new(side * w * 0.49, h * (0.18 + index as f32 * 0.018), fan);
            let foot = Vec3::new(side * w * 0.87, -origin.y + w * 0.025, fan * 1.60);
            let ankle = knee.lerp(foot, 0.72) + Vec3::new(side * w * 0.035, h * 0.025, 0.0);
            let mut leg = Builder::with_resolution(16);
            for (a, b, thickness, color) in [
                (Vec3::ZERO, knee, 0.13, DARK),
                (knee, ankle, 0.105, ORANGE),
                (ankle, foot, 0.065, DARK),
            ] {
                shape::segment(&mut leg, a, b, w * thickness, w * thickness, color)?;
                // A wide proximal armor band and the exposed darker joint make
                // each bent segment readable without eight noisy tiny bands.
                let end = a.lerp(b, 0.27);
                shape::segment(
                    &mut leg,
                    a,
                    end,
                    w * (thickness + 0.025),
                    w * (thickness + 0.025),
                    SHELL,
                )?;
            }
            leg.voxel(knee.to_array(), [w * 0.17; 3], SHELL)?;
            leg.voxel(ankle.to_array(), [w * 0.10; 3], SHELL)?;
            parts.push((
                Part::SpiderLeg {
                    side,
                    index: index as u8,
                },
                origin,
                leg.finish()?,
            ));
        }
    }
    Ok(parts)
}

#[cfg(test)]
mod tests;
