//! Muscular stepped silhouette with a curved neck, long face and sixteen stable coats.
use super::{Part, eyes, shape, voxel::Builder};
use bevy::prelude::*;

mod coat;
mod head;
#[cfg(test)]
mod tests;
use coat::coat;

const HEAD: Vec3 = Vec3::new(0.0, 1.55, -0.61);
const HOOF: [f32; 3] = [0.105, 0.105, 0.105];

fn mane(variant: u8) -> [f32; 3] {
    if matches!(variant, 3 | 6 | 14) {
        [0.81, 0.75, 0.58]
    } else if matches!(variant, 2 | 9) {
        [0.34, 0.10, 0.035]
    } else {
        [0.065, 0.045, 0.032]
    }
}

pub(super) fn parts(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let body_coat = coat(variant, [0.0, 1.0, 0.0]);
    let hair = mane(variant);
    let mut body = Builder::default();
    // A tapering belly, broad chest and muscular haunches form one closed union,
    // inside the existing 0.595 m reference width and the original joint pivots.
    for (center, radius) in [
        ([0.0, 1.055, 0.035], [0.274, 0.335, 0.64]),
        ([0.0, 1.09, -0.43], [0.292, 0.32, 0.29]),
        ([0.0, 1.075, 0.46], [0.288, 0.315, 0.265]),
        ([0.0, 1.245, -0.43], [0.23, 0.275, 0.255]),
        ([0.0, 1.435, -0.56], [0.188, 0.23, 0.215]),
        ([0.0, 1.60, -0.66], [0.145, 0.125, 0.165]),
    ] {
        body.ellipsoid(
            Vec3::from_array(center),
            Vec3::from_array(radius),
            body_coat,
        )?;
    }
    // The mane follows the curved rear neck ridge and drapes down its sides.
    for i in 0..8 {
        let t = i as f32 / 7.0;
        let y = 1.29 + t * 0.40;
        let z = -0.22 - t * 0.32;
        body.voxel([0.0, y, z], [0.13, 0.12, 0.14], hair)?;
        for side in [-1.0, 1.0] {
            body.voxel(
                [side * (0.16 - t * 0.035), y - 0.07, z + 0.015],
                [0.075, 0.18 - t * 0.04, 0.115],
                hair.map(|v| v * if i % 3 == 0 { 1.13 } else { 1.0 }),
            )?;
        }
    }
    let body = body.finish_painted(|p, tint| {
        if tint == body_coat {
            coat(variant, p.to_array())
        } else {
            tint
        }
    })?;
    let (head, lids) = head::meshes(variant)?;
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body),
        (Part::Head, HEAD, head),
        (Part::Eyelids, HEAD, lids),
    ];
    for (i, (x, z)) in [
        (-0.175, -0.46),
        (0.175, -0.46),
        (-0.175, 0.46),
        (0.175, 0.46),
    ]
    .into_iter()
    .enumerate()
    {
        let mut leg = Builder::with_resolution(16);
        let hind = z > 0.0;
        let joint = if hind { 0.045 } else { -0.005 };
        // Broad upper muscle, defined knee/hock and thin cannon ending in a
        // wider flat hoof. The four original gait joints remain unchanged.
        for (center, size) in [
            (
                [0.0, -0.10, if hind { 0.025 } else { 0.0 }],
                [0.23, 0.28, 0.24],
            ),
            ([0.0, -0.31, joint], [0.145, 0.18, 0.17]),
            ([0.0, -0.465, joint * 0.5], [0.11, 0.22, 0.125]),
            ([0.0, -0.565, -0.012], [0.14, 0.095, 0.16]),
        ] {
            shape::rounded(
                &mut leg,
                center,
                size,
                0.025,
                coat(variant, [x, 0.68 + center[1], z]),
            )?;
        }
        shape::rounded(
            &mut leg,
            [0.0, -0.63, -0.025],
            [0.18, 0.10, 0.23],
            0.018,
            HOOF,
        )?;
        parts.push((
            Part::Leg(if i == 0 || i == 3 { 1.0 } else { -1.0 }),
            Vec3::new(x, 0.68, z),
            leg.finish()?,
        ));
    }
    let mut tail = Builder::with_resolution(16);
    for i in 0..7 {
        let t = i as f32 / 6.0;
        tail.voxel(
            [0.0, -t * 0.52, 0.02 + t * 0.17 - t * t * 0.05],
            [0.105 + (t * std::f32::consts::PI).sin() * 0.07, 0.13, 0.12],
            hair,
        )?;
    }
    parts.push((Part::Tail, Vec3::new(0.0, 1.20, 0.64), tail.finish()?));
    Ok(parts)
}
