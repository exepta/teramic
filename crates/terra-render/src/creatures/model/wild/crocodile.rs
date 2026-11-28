use super::*;

const HIDE: Tint = [0.30, 0.345, 0.175];
const ARMOR: Tint = [0.185, 0.235, 0.115];
const EDGE: Tint = [0.465, 0.475, 0.235];
const BELLY: Tint = [0.66, 0.61, 0.385];
const TOOTH: Tint = [0.89, 0.84, 0.64];
const DARK: Tint = [0.115, 0.13, 0.08];
const IRIS: Tint = [0.57, 0.38, 0.075];
const PUPIL: Tint = [0.035, 0.041, 0.023];

pub(super) fn parts() -> Result<Parts, String> {
    let mut body = Builder::with_resolution(26);
    mass(&mut body, [0., 0.269, 0.055], [0.356, 0.196, 0.765], HIDE)?;
    mass(&mut body, [0., 0.212, -0.014], [0.32, 0.130, 0.66], BELLY)?;
    mass(&mut body, [0., 0.32, -0.43], [0.305, 0.152, 0.346], HIDE)?;
    for i in 0..8 {
        let z = -0.56 + i as f32 * 0.164;
        let crest = 0.446 + 0.028 * (1. - (z / 0.8).abs());
        for side in [-1., 1.] {
            for (x, y, w) in [(0.089, crest, 0.114), (0.214, crest - 0.051, 0.082)] {
                body.rounded([side * x, y, z], [w, 0.062, 0.143], 0.022, ARMOR)?;
                body.rounded(
                    [side * x, y + 0.04, z + 0.018],
                    [w * 0.50, 0.057, 0.102],
                    0.012,
                    EDGE,
                )?;
            }
        }
    }
    let neck = CROCODILE_NECK;
    let mut head = Builder::with_cell_size(0.024);
    mass(&mut head, [0., 0.020, -0.156], [0.251, 0.139, 0.282], HIDE)?;
    // Interlocking teeth line the outside of the lower jaw. The nose widens again at the nostrils.
    for (z, w, h, d) in [
        (-0.309, 0.394, 0.147, 0.299),
        (-0.475, 0.294, 0.124, 0.235),
        (-0.643, 0.231, 0.104, 0.223),
        (-0.802, 0.199, 0.091, 0.192),
        (-0.932, 0.216, 0.102, 0.159),
    ] {
        head.rounded([0., 0.002, z], [w, h, d], 0.035, HIDE)?;
    }
    for side in [-1., 1.] {
        head.rounded(
            [side * 0.151, 0.086, -0.239],
            [0.107, 0.095, 0.18],
            0.035,
            HIDE,
        )?;
        head.rounded(
            [side * 0.182, 0.079, -0.276],
            [0.053, 0.040, 0.105],
            0.015,
            DARK,
        )?;
        head.voxel([side * 0.202, 0.082, -0.272], [0.017, 0.020, 0.066], IRIS)?;
        head.voxel([side * 0.209, 0.082, -0.283], [0.010, 0.020, 0.013], PUPIL)?;
        head.rounded(
            [side * 0.168, 0.111, -0.248],
            [0.094, 0.045, 0.163],
            0.017,
            ARMOR,
        )?;
        head.rounded(
            [side * 0.056, 0.048, -0.948],
            [0.057, 0.041, 0.058],
            0.013,
            ARMOR,
        )?;
        head.voxel([side * 0.059, 0.068, -0.957], [0.028, 0.009, 0.032], DARK)?;
        for (i, (x, z)) in [
            (0.155, -0.39),
            (0.130, -0.51),
            (0.105, -0.68),
            (0.09, -0.83),
            (0.085, -0.92),
        ]
        .into_iter()
        .enumerate()
        {
            head.rounded(
                [side * x, -0.064, z],
                [0.029, if i == 1 { 0.076 } else { 0.045 }, 0.032],
                0.007,
                TOOTH,
            )?;
        }
    }
    let mut jaw = Builder::with_resolution(23);
    for (z, w, d) in [
        (-0.11, 0.359, 0.27),
        (-0.32, 0.260, 0.24),
        (-0.52, 0.200, 0.21),
        (-0.72, 0.172, 0.23),
    ] {
        jaw.rounded([0., -0.031, z], [w, 0.081, d], 0.026, BELLY)?;
        jaw.rounded(
            [0., 0.007, z],
            [w * 0.83, 0.021, d * 0.87],
            0.008,
            [0.49, 0.315, 0.235],
        )?;
    }
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, neck, head.finish()?),
        (
            Part::Jaw,
            neck + Vec3::new(0., -0.069, -0.166),
            jaw.finish()?,
        ),
    ];
    for hind in [false, true] {
        for side in [-1., 1.] {
            let hip = Vec3::new(side * 0.262, 0.248, if hind { 0.454 } else { -0.410 });
            let mut leg = Builder::with_cell_size(0.026);
            let elbow = Vec3::new(side * 0.183, -0.069, if hind { -0.090 } else { 0.083 });
            let heel = Vec3::new(side * 0.219, -0.194, if hind { 0.061 } else { -0.029 });
            segment(&mut leg, Vec3::ZERO, elbow, 0.117, 0.113, HIDE)?;
            segment(&mut leg, elbow, heel, 0.078, 0.078, HIDE)?;
            leg.rounded(
                [heel.x, -hip.y + 0.029, heel.z - 0.039],
                [0.102, 0.058, 0.112],
                0.022,
                HIDE,
            )?;
            for toe in [-1., 0., 1.] {
                let p = Vec3::new(heel.x + toe * 0.044, -hip.y + 0.023, heel.z - 0.118);
                segment(
                    &mut leg,
                    Vec3::new(heel.x, -hip.y + 0.025, heel.z - 0.056),
                    p,
                    0.027,
                    0.025,
                    BELLY,
                )?;
            }
            parts.push((
                Part::Leg(if hind { side } else { -side }),
                hip,
                leg.finish()?,
            ));
        }
    }
    for index in 0..3 {
        let mut tail = Builder::with_resolution(18);
        for j in 0..4 {
            let f = (index * 4 + j) as f32 / 12.;
            let width = 0.021 + 0.178 * (1. - f);
            let y = 0.012 - 0.017 * f;
            let z = (j as f32 + 0.5) / 4. * CROCODILE_TAIL_STEP;
            tail.rounded(
                [0., y, z],
                [width * 2., width * 1.26, CROCODILE_TAIL_STEP * 0.32],
                width * 0.22,
                HIDE,
            )?;
            for side in [-1., 1.] {
                tail.rounded(
                    [side * width * 0.40, y + width * 0.65, z],
                    [width * 0.29, width * 0.47, 0.084],
                    0.012,
                    ARMOR,
                )?;
            }
        }
        parts.push((
            Part::ReptileJoint(index),
            Vec3::new(
                0.,
                0.26 - index as f32 * 0.040,
                CROCODILE_TAIL + index as f32 * CROCODILE_TAIL_STEP,
            ),
            tail.finish()?,
        ));
    }
    Ok(parts)
}
