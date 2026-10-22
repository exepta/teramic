//! Layered coat is baked into eight shared rigid meshes, never individual fur entities.
use super::{Part, shape};
use crate::players::model::Builder;
use bevy::prelude::*;

const COAT: [f32; 3] = [0.53, 0.275, 0.105];
const DARK: [f32; 3] = [0.18, 0.105, 0.055];
const MUZZLE: [f32; 3] = [0.83, 0.66, 0.39];
const CLAW: [f32; 3] = [0.56, 0.51, 0.40];
const HEAD: Vec3 = Vec3::new(0.0, 0.94, -0.53);
const LEG: f32 = 0.57;

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    shape::rounded(&mut body, [0.0, 0.73, 0.14], [0.86, 0.77, 1.06], 0.17, COAT)?;
    shape::rounded(
        &mut body,
        [0.0, 0.89, -0.25],
        [0.94, 0.69, 0.69],
        0.15,
        COAT,
    )?;
    shape::rounded(&mut body, [0.0, 0.80, 0.47], [0.77, 0.64, 0.60], 0.15, COAT)?;
    // Broad irregular overlapping plates give the back a stepped, rather than gridded, coat.
    for row in 0..7 {
        let z = -0.47 + row as f32 * 0.17;
        let shoulder = (1.0 - ((z + 0.20) / 0.80).abs()).max(0.0);
        for column in -2i32..=2 {
            let x = column as f32 * 0.16 + if row % 2 == 0 { 0.015 } else { -0.02 };
            let edge = column.abs() as f32;
            let h = 1.12 + shoulder * 0.17 - edge * 0.047;
            plate(
                &mut body,
                [x, h, z],
                [0.19, 0.075, 0.22],
                (row * 7 + column + 2) as u32,
            )?;
        }
        for side in [-1.0, 1.0] {
            for tier in 0..3 {
                let y = 0.48 + tier as f32 * 0.17 + (row % 2) as f32 * 0.025;
                let x = side * (0.39 + shoulder * 0.055 - (tier == 0) as u8 as f32 * 0.035);
                plate(
                    &mut body,
                    [x, y, z],
                    [0.12, 0.22, 0.23],
                    (row * 11 + tier) as u32,
                )?;
            }
        }
    }
    for row in 0..4 {
        let width = 0.43 - row as f32 * 0.075;
        shape::rounded(
            &mut body,
            [0.0, 0.82 - row as f32 * 0.12, -0.585 + row as f32 * 0.015],
            [width, 0.16, 0.055],
            0.014,
            MUZZLE.map(|v| v * (0.83 + row as f32 * 0.025)),
        )?;
    }

    let mut head = Builder::default();
    shape::rounded(
        &mut head,
        [0.0, 0.0, -0.015],
        [0.62, 0.48, 0.48],
        0.075,
        COAT,
    )?;
    for side in [-1.0, 1.0] {
        for tier in 0..3 {
            plate(
                &mut head,
                [
                    side * (0.255 + tier as f32 * 0.006),
                    0.11 - tier as f32 * 0.125,
                    -0.065,
                ],
                [0.14, 0.17, 0.34 - tier as f32 * 0.035],
                31 + tier,
            )?;
        }
        // Faceted round ears and inset warm inner ear, without additional animated entities.
        shape::rounded(
            &mut head,
            [side * 0.23, 0.25, 0.025],
            [0.19, 0.21, 0.13],
            0.055,
            DARK,
        )?;
        shape::rounded(
            &mut head,
            [side * 0.23, 0.257, -0.046],
            [0.11, 0.13, 0.022],
            0.025,
            MUZZLE,
        )?;
        shape::rounded(
            &mut head,
            [side * 0.18, 0.035, -0.252],
            [0.14, 0.105, 0.025],
            0.014,
            DARK,
        )?;
        head.voxel(
            [side * 0.178, 0.036, -0.269],
            [0.059, 0.058, 0.012],
            [0.89, 0.80, 0.59],
        )?;
        head.voxel(
            [side * 0.162, 0.031, -0.278],
            [0.035, 0.048, 0.009],
            [0.045, 0.033, 0.024],
        )?;
        head.voxel(
            [side * 0.156, 0.045, -0.284],
            [0.012, 0.014, 0.006],
            [0.98, 0.95, 0.84],
        )?;
        head.rotated(
            [side * 0.18, 0.104, -0.245],
            [0.19, 0.065, 0.075],
            COAT.map(|v| v * 0.88),
            Quat::from_rotation_z(side * 0.12),
        )?;
    }
    for i in -1i32..=1 {
        plate(
            &mut head,
            [i as f32 * 0.17, 0.23 - i.abs() as f32 * 0.012, -0.09],
            [0.20, 0.07, 0.28],
            (i + 5) as u32,
        )?;
    }
    shape::rounded(
        &mut head,
        [0.0, -0.092, -0.29],
        [0.38, 0.24, 0.23],
        0.045,
        MUZZLE,
    )?;
    shape::rounded(
        &mut head,
        [0.0, -0.04, -0.419],
        [0.18, 0.105, 0.065],
        0.025,
        DARK,
    )?;
    head.voxel([0.0, -0.112, -0.437], [0.045, 0.058, 0.022], DARK)?;
    shape::rounded(
        &mut head,
        [0.0, -0.215, -0.285],
        [0.29, 0.07, 0.19],
        0.015,
        DARK,
    )?;
    shape::rounded(
        &mut head,
        [0.0, -0.257, -0.285],
        [0.32, 0.052, 0.20],
        0.016,
        MUZZLE,
    )?;
    head.voxel(
        [0.0, -0.231, -0.384],
        [0.10, 0.018, 0.023],
        [0.42, 0.15, 0.11],
    )?;
    for side in [-1.0, 1.0] {
        head.rotated(
            [side * 0.11, -0.22, -0.38],
            [0.027, 0.05, 0.025],
            [0.92, 0.84, 0.66],
            Quat::from_rotation_z(side * 0.18),
        )?;
    }
    let mut lids = Builder::default();
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut lids,
            [side * 0.18, 0.035, -0.286],
            [0.15, 0.11, 0.016],
            0.015,
            COAT,
        )?;
        lids.voxel([side * 0.18, 0.015, -0.297], [0.10, 0.01, 0.01], DARK)?;
    }
    let mut output = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, HEAD, head.finish()?),
        (Part::Eyelids, HEAD, lids.finish()?),
    ];
    for x in [-1.0, 1.0] {
        for z in [-1.0, 1.0] {
            let mut leg = Builder::default();
            let front = z < 0.0;
            shape::rounded(&mut leg, [0.0, -0.16, 0.0], [0.31, 0.44, 0.36], 0.06, COAT)?;
            shape::rounded(
                &mut leg,
                [0.0, -0.38, -0.01],
                [0.25, 0.24, 0.30],
                0.04,
                COAT.map(|v| v * 0.80),
            )?;
            for row in 0..3 {
                for side in [-1.0, 1.0] {
                    plate(
                        &mut leg,
                        [side * 0.125, -0.09 - row as f32 * 0.13, -0.035],
                        [0.09, 0.17, 0.31],
                        row + u32::from(front) * 7,
                    )?;
                }
                plate(
                    &mut leg,
                    [0.0, -0.09 - row as f32 * 0.13, -0.18],
                    [0.29, 0.16, 0.08],
                    row + 11,
                )?;
            }
            shape::rounded(
                &mut leg,
                [0.0, -LEG + 0.082, -0.035],
                [0.31, 0.165, 0.38],
                0.035,
                DARK,
            )?;
            for toe in -2i32..=2 {
                shape::rounded(
                    &mut leg,
                    [toe as f32 * 0.057, -LEG + 0.062, -0.192],
                    [0.052, 0.102, 0.12],
                    0.012,
                    DARK.map(|v| v * 1.12),
                )?;
                shape::rounded(
                    &mut leg,
                    [toe as f32 * 0.057, -LEG + 0.044, -0.257],
                    [0.028, 0.075, 0.065],
                    0.008,
                    CLAW,
                )?;
            }
            output.push((
                Part::Leg(x * z),
                Vec3::new(x * 0.32, LEG, z * 0.43),
                leg.finish()?,
            ));
        }
    }
    let mut tail = Builder::default();
    shape::rounded(&mut tail, [0.0, 0.0, 0.0], [0.15, 0.14, 0.17], 0.045, DARK)?;
    output.push((Part::Tail, Vec3::new(0.0, 0.67, 0.79), tail.finish()?));
    Ok(output)
}

fn plate(builder: &mut Builder, p: [f32; 3], size: [f32; 3], seed: u32) -> Result<(), String> {
    let gain = 0.86 + ((seed.wrapping_mul(13) ^ seed.rotate_left(3)) % 17) as f32 * 0.015;
    builder.voxel(p, size, COAT.map(|v| v * gain))
}
