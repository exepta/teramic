use super::{Part, shape};
use crate::players::model::Builder;
use bevy::prelude::*;

const ORANGE: [f32; 3] = [0.95, 0.40, 0.055];
const PALE: [f32; 3] = [0.96, 0.92, 0.80];
const EDGE: [f32; 3] = [0.12, 0.105, 0.085];

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    // Solid tapered segments mirror the river fish's silhouette, with three edged bands.
    for i in 0..13 {
        let t = i as f32 / 12.0;
        let arc = (std::f32::consts::PI * t).sin();
        let width = 0.12 + arc * 0.13;
        let height = 0.15 + arc * 0.15;
        let z = -0.245 + t * 0.49;
        let color = match i {
            1 | 2 | 6 | 7 | 11 => PALE,
            0 | 3 | 5 | 8 | 10 | 12 => ORANGE.map(|v| v * 0.87),
            _ => ORANGE,
        };
        shape::rounded(
            &mut body,
            [0.0, 0.16, z],
            [width, height, 0.047],
            0.015,
            color,
        )?;
        if [1, 3, 6, 8, 11, 12].contains(&i) {
            let stripe_z = z - 0.022;
            for side in [-1.0, 1.0] {
                body.voxel(
                    [side * (width * 0.5 + 0.002), 0.16, stripe_z],
                    [0.009, height * 0.73, 0.011],
                    EDGE,
                )?;
            }
        }
        if (3..=10).contains(&i) {
            let rise = 0.045 + arc * 0.05;
            body.voxel(
                [0.0, 0.16 + height * 0.5 + rise * 0.5, z],
                [0.026, rise, 0.042],
                ORANGE,
            )?;
            body.voxel(
                [0.0, 0.16 + height * 0.5 + rise, z],
                [0.027, 0.016, 0.042],
                EDGE,
            )?;
            body.voxel(
                [0.0, 0.16 + height * 0.5 + rise * 0.5, z + 0.014],
                [0.028, rise * 0.72, 0.007],
                [0.99, 0.57, 0.12],
            )?;
            if (4..=8).contains(&i) {
                body.voxel(
                    [0.0, 0.16 - height * 0.5 - 0.018, z],
                    [0.024, 0.05, 0.04],
                    ORANGE,
                )?;
            }
        }
    }
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut body,
            [side * 0.095, 0.20, -0.212],
            [0.041, 0.102, 0.081],
            0.012,
            PALE,
        )?;
        shape::rounded(
            &mut body,
            [side * 0.116, 0.20, -0.222],
            [0.014, 0.069, 0.054],
            0.004,
            EDGE,
        )?;
        body.voxel([side * 0.126, 0.22, -0.235], [0.006, 0.019, 0.015], PALE)?;
        for i in 0..4 {
            let out = i as f32 * 0.021;
            body.rotated(
                [side * (0.12 + out), 0.105 - out * 0.20, -0.025 + out * 0.48],
                [0.033, 0.017, 0.115 - i as f32 * 0.014],
                if i == 3 { EDGE } else { ORANGE },
                Quat::from_rotation_y(side * -0.35),
            )?;
            body.voxel(
                [side * (0.12 + out), 0.115 - out * 0.20, -0.033 + out * 0.48],
                [0.026, 0.009, 0.008],
                PALE,
            )?;
        }
        for i in 0..3 {
            body.voxel(
                [side * 0.125, 0.17 + i as f32 * 0.028, -0.095],
                [0.008, 0.012, 0.02],
                [0.69, 0.27, 0.055],
            )?;
        }
    }
    shape::rounded(
        &mut body,
        [0.0, 0.12, -0.271],
        [0.09, 0.055, 0.028],
        0.01,
        [0.98, 0.56, 0.17],
    )?;
    body.voxel([0.0, 0.122, -0.289], [0.059, 0.008, 0.006], EDGE)?;
    let mut tail = Builder::default();
    for i in 0..5 {
        let height = 0.12 + i as f32 * 0.046;
        tail.voxel(
            [0.0, 0.0, 0.025 + i as f32 * 0.037],
            [0.037, height, 0.042],
            if i == 3 {
                PALE
            } else if i == 4 {
                EDGE
            } else {
                ORANGE
            },
        )?;
        if i < 3 {
            for y in [-1.0, 1.0] {
                tail.voxel(
                    [0.0, y * height * 0.31, 0.025 + i as f32 * 0.037],
                    [0.04, 0.01, 0.042],
                    [0.99, 0.58, 0.18],
                )?;
            }
        }
    }
    Ok(vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Tail, Vec3::new(0.0, 0.16, 0.25), tail.finish()?),
    ])
}
