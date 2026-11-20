use super::{Part, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    let amber = [0.88, 0.55, 0.13];
    let dark = [0.13, 0.10, 0.07];
    // The authored body stays .095 x .095 x .115; appendages remain visual detail.
    for i in 0..7 {
        let outer = i == 0 || i == 6;
        shape::rounded(
            &mut body,
            [0.0, 0.053, (i as f32 - 3.0) * 0.016],
            [
                if outer { 0.077 } else { 0.095 },
                if outer { 0.074 } else { 0.095 },
                0.019,
            ],
            0.006,
            if i == 2 || i == 4 { dark } else { amber },
        )?;
    }
    shape::rounded(
        &mut body,
        [0.0, 0.055, -0.054],
        [0.079, 0.074, 0.020],
        0.006,
        amber,
    )?;
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut body,
            [side * 0.027, 0.064, -0.067],
            [0.026, 0.032, 0.014],
            0.004,
            dark,
        )?;
        body.voxel(
            [side * 0.025, 0.073, -0.076],
            [0.008, 0.009, 0.004],
            [1.0, 0.96, 0.80],
        )?;
        body.voxel(
            [side * 0.033, 0.057, -0.075],
            [0.004, 0.005, 0.003],
            [0.43, 0.31, 0.16],
        )?;
        shape::segment(
            &mut body,
            Vec3::new(side * 0.024, 0.082, -0.047),
            Vec3::new(side * 0.031, 0.111, -0.052),
            0.005,
            0.005,
            dark,
        )?;
        shape::segment(
            &mut body,
            Vec3::new(side * 0.031, 0.110, -0.052),
            Vec3::new(side * 0.039, 0.115, -0.063),
            0.005,
            0.005,
            dark,
        )?;
        for z in [-0.032, 0.0, 0.032] {
            shape::segment(
                &mut body,
                Vec3::new(side * 0.031, 0.024, z),
                Vec3::new(side * 0.050, 0.014, z + 0.004),
                0.006,
                0.006,
                dark,
            )?;
            shape::segment(
                &mut body,
                Vec3::new(side * 0.050, 0.014, z + 0.004),
                Vec3::new(side * 0.049, 0.004, z - 0.004),
                0.005,
                0.005,
                dark,
            )?;
        }
        shape::rounded(
            &mut body,
            [side * 0.048, 0.017, 0.032],
            [0.011, 0.014, 0.014],
            0.003,
            [0.96, 0.68, 0.18],
        )?;
        for i in 0..6 {
            body.voxel(
                [
                    side * 0.046,
                    0.069 + (i % 2) as f32 * 0.013,
                    -0.035 + i as f32 * 0.014,
                ],
                [0.006, 0.010, 0.006],
                if i == 2 || i == 4 {
                    dark
                } else {
                    [0.96, 0.69, 0.27]
                },
            )?;
        }
    }
    for i in 0..5 {
        body.voxel(
            [(i as f32 - 2.0) * 0.015, 0.100, -0.025],
            [0.007, 0.009 + (i % 2) as f32 * 0.004, 0.008],
            [0.96, 0.70, 0.29],
        )?;
    }
    shape::segment(
        &mut body,
        Vec3::new(0.0, 0.043, 0.052),
        Vec3::new(0.0, 0.036, 0.072),
        0.013,
        0.013,
        dark,
    )?;
    shape::segment(
        &mut body,
        Vec3::new(0.0, 0.036, 0.068),
        Vec3::new(0.0, 0.032, 0.086),
        0.005,
        0.005,
        dark,
    )?;
    let mut output = vec![(Part::Body, Vec3::ZERO, body.finish()?)];
    let mut tongue = Builder::default();
    tongue.voxel(
        [0.0, -0.017, -0.006],
        [0.006, 0.034, 0.006],
        [0.31, 0.15, 0.065],
    )?;
    tongue.voxel(
        [0.0, -0.034, -0.006],
        [0.009, 0.005, 0.008],
        [0.52, 0.29, 0.13],
    )?;
    output.push((
        Part::Proboscis,
        Vec3::new(0.0, 0.04, -0.067),
        tongue.finish()?,
    ));
    for sign in [-1.0, 1.0] {
        let mut wing = Builder::default();
        for i in 0..5 {
            let extent = [0.033, 0.052, 0.056, 0.045, 0.026][i];
            wing.voxel(
                [sign * (0.012 + i as f32 * 0.016), 0.0, i as f32 * 0.004],
                [0.018, 0.0025, extent],
                [
                    0.82 + i as f32 * 0.025,
                    0.89 + i as f32 * 0.014,
                    0.86 + i as f32 * 0.018,
                ],
            )?;
            shape::segment(
                &mut wing,
                Vec3::new(sign * 0.004, 0.002, 0.0),
                Vec3::new(
                    sign * (0.018 + i as f32 * 0.016),
                    0.002,
                    i as f32 * 0.004 - extent * 0.35,
                ),
                0.0016,
                0.0016,
                [0.62, 0.73, 0.69],
            )?;
        }
        wing.voxel(
            [sign * 0.029, -0.002, 0.029],
            [0.046, 0.002, 0.028],
            [0.74, 0.84, 0.80],
        )?;
        output.push((
            Part::Wing(sign),
            Vec3::new(sign * 0.035, 0.082, 0.0),
            wing.finish()?,
        ));
    }
    Ok(output)
}
