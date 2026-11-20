use super::{Kind, Part, eyes, shape};
use crate::creatures::model::voxel::Builder;
use bevy::prelude::*;

const CREAM: [f32; 3] = [0.91, 0.85, 0.72];
const DARK: [f32; 3] = [0.19, 0.145, 0.12];

pub(super) fn parts(kind: Kind) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let (coat, width, height, length, leg, head_at) = match kind {
        Kind::Sheep => (CREAM, 0.76, 0.70, 1.02, 0.42, Vec3::new(0.0, 1.05, -0.48)),
        Kind::Cow => (
            [0.48, 0.27, 0.15],
            0.85,
            0.76,
            1.30,
            0.62,
            Vec3::new(0.0, 1.23, -0.67),
        ),
        Kind::Pig => (
            [0.88, 0.49, 0.43],
            0.72,
            0.55,
            0.98,
            0.32,
            Vec3::new(0.0, 0.74, -0.48),
        ),
        _ => return Err("Expected a hoofed creature".into()),
    };
    let mut body = Builder::default();
    shape::rounded(
        &mut body,
        [0.0, leg + height * 0.5, 0.0],
        [width, height, length],
        0.12,
        coat,
    )?;
    match kind {
        Kind::Sheep => wool(&mut body, leg, height, width)?,
        Kind::Cow => cow_coat(&mut body, leg, height, width)?,
        Kind::Pig => pig_coat(&mut body, leg, width)?,
        _ => {}
    }
    let face = if kind == Kind::Sheep {
        [0.34, 0.29, 0.24]
    } else {
        coat
    };
    let mut head = Builder::default();
    shape::rounded(&mut head, [0.0, 0.0, 0.0], [0.46, 0.43, 0.39], 0.055, face)?;
    let muzzle = if kind == Kind::Sheep {
        [0.44, 0.36, 0.29]
    } else {
        [0.83, 0.49, 0.43]
    };
    shape::rounded(
        &mut head,
        [0.0, -0.105, -0.245],
        [0.38, 0.20, 0.20],
        0.035,
        muzzle,
    )?;
    head.voxel(
        [0.0, -0.175, -0.349],
        [0.21, 0.012, 0.008],
        muzzle.map(|v| v * 0.65),
    )?;
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut head,
            [side * 0.095, -0.09, -0.35],
            [0.051, 0.057, 0.018],
            0.006,
            DARK,
        )?;
        head.voxel(
            [side * 0.10, -0.075, -0.362],
            [0.021, 0.013, 0.006],
            muzzle.map(|v| v * 0.75),
        )?;
        shape::rounded(
            &mut head,
            [side * 0.145, 0.105, -0.222],
            [0.12, 0.13, 0.08],
            0.012,
            face.map(|v| v * 0.8),
        )?;
        ears(&mut head, side, kind, coat)?;
        if kind == Kind::Cow {
            shape::segment(
                &mut head,
                Vec3::new(side * 0.17, 0.17, 0.035),
                Vec3::new(side * 0.205, 0.32, 0.035),
                0.085,
                0.08,
                [0.66, 0.55, 0.39],
            )?;
            shape::segment(
                &mut head,
                Vec3::new(side * 0.205, 0.30, 0.035),
                Vec3::new(side * 0.175, 0.40, 0.025),
                0.048,
                0.045,
                CREAM,
            )?;
        }
    }
    if kind == Kind::Cow {
        for (y, w) in [(-0.06, 0.08), (0.03, 0.11), (0.12, 0.16)] {
            head.voxel([0.0, y, -0.199], [w, 0.095, 0.014], CREAM)?;
        }
        for i in 0..5 {
            head.voxel(
                [
                    (i as f32 - 2.0) * 0.062,
                    0.205,
                    -0.11 + (i % 2) as f32 * 0.025,
                ],
                [0.074, 0.055, 0.10],
                coat.map(|v| v * 0.75),
            )?;
        }
    } else if kind == Kind::Sheep {
        for x in -1i32..=1 {
            for z in -1i32..=1 {
                shape::rounded(
                    &mut head,
                    [
                        x as f32 * 0.15,
                        0.22 + (x.abs() + z.abs()) as f32 * -0.015,
                        z as f32 * 0.13,
                    ],
                    [0.20, 0.16, 0.19],
                    0.026,
                    CREAM,
                )?;
            }
        }
    } else {
        head.voxel(
            [0.0, -0.085, -0.354],
            [0.29, 0.018, 0.008],
            [0.93, 0.61, 0.54],
        )?;
        for side in [-1.0, 1.0] {
            for i in 0..3 {
                head.voxel(
                    [
                        side * (0.17 + i as f32 * 0.012),
                        -0.06 - i as f32 * 0.022,
                        -0.201,
                    ],
                    [0.018, 0.012, 0.01],
                    [0.72, 0.38, 0.34],
                )?;
            }
        }
    }
    let eye_specs = [-1.0, 1.0].map(|side| eyes::Eye {
        center: Vec3::new(side * 0.145, 0.105, -0.267),
        width: 0.10,
        side: 0.0,
        skin: face,
    });
    let (head, lids) = eyes::attach(head.finish()?, &eye_specs)?;
    let mut output = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, head_at, head),
        (Part::Eyelids, head_at, lids),
    ];
    for x in [-1.0, 1.0] {
        for z in [-1.0, 1.0] {
            let mut limb = Builder::default();
            let skin = if kind == Kind::Sheep {
                [0.44, 0.35, 0.26]
            } else {
                coat
            };
            shape::rounded(
                &mut limb,
                [0.0, -(leg - 0.12) * 0.5, 0.0],
                [0.18, leg - 0.10, 0.19],
                0.027,
                skin,
            )?;
            shape::rounded(
                &mut limb,
                [0.0, -leg * 0.48, -0.016],
                [0.19, 0.12, 0.20],
                0.022,
                skin.map(|v| v * 1.06),
            )?;
            for toe in [-1.0, 1.0] {
                shape::rounded(
                    &mut limb,
                    [toe * 0.052, -leg + 0.067, -0.024],
                    [0.098, 0.13, 0.23],
                    0.02,
                    DARK,
                )?;
                limb.voxel(
                    [toe * 0.052, -leg + 0.115, -0.06],
                    [0.075, 0.017, 0.12],
                    [0.31, 0.24, 0.19],
                )?;
            }
            output.push((
                Part::Leg(x * z),
                Vec3::new(x * width * 0.34, leg, z * length * 0.32),
                limb.finish()?,
            ));
        }
    }
    let mut tail = Builder::default();
    if kind == Kind::Pig {
        for i in 0..11 {
            let angle = i as f32 * 0.62;
            tail.voxel(
                [
                    angle.cos() * 0.055,
                    angle.sin() * 0.055 - 0.05,
                    0.035 + i as f32 * 0.007,
                ],
                [0.031, 0.031, 0.031],
                coat.map(|v| v * 1.05),
            )?;
        }
    } else {
        shape::segment(
            &mut tail,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, -0.25, 0.07),
            0.065,
            0.065,
            coat,
        )?;
        for i in -1i32..=1 {
            shape::rounded(
                &mut tail,
                [i as f32 * 0.035, -0.26 - (1 - i.abs()) as f32 * 0.025, 0.07],
                [0.06, 0.12, 0.09],
                0.015,
                if kind == Kind::Cow { DARK } else { CREAM },
            )?;
        }
    }
    output.push((
        Part::Tail,
        Vec3::new(0.0, leg + height * 0.65, length * 0.5),
        tail.finish()?,
    ));
    Ok(output)
}

fn ears(head: &mut Builder, side: f32, kind: Kind, coat: [f32; 3]) -> Result<(), String> {
    let rotation = Quat::from_rotation_z(side * if kind == Kind::Pig { -0.40 } else { 0.18 });
    let center = if kind == Kind::Pig {
        [side * 0.24, 0.21, -0.035]
    } else {
        [side * 0.29, 0.10, 0.005]
    };
    let size = if kind == Kind::Pig {
        [0.17, 0.21, 0.075]
    } else {
        [0.23, 0.14, 0.085]
    };
    head.rotated(center, size, coat, rotation)?;
    head.rotated(
        [center[0], center[1], center[2] - 0.046],
        [size[0] * 0.65, size[1] * 0.65, 0.015],
        [0.70, 0.39, 0.33],
        rotation,
    )?;
    head.rotated(
        [
            center[0] + side * size[0] * 0.39,
            center[1] - 0.012,
            center[2],
        ],
        [0.045, size[1] * 0.65, 0.065],
        coat.map(|v| v * 0.85),
        rotation,
    )
}

fn wool(body: &mut Builder, leg: f32, height: f32, width: f32) -> Result<(), String> {
    for side in [-1.0, 1.0] {
        for z in 0..7 {
            for y in 0..3 {
                let h = (z * 7 + y * 3) % 5;
                shape::rounded(
                    body,
                    [
                        side * (width * 0.5 - 0.005),
                        leg + 0.14 + y as f32 * 0.21,
                        (z as f32 - 3.0) * 0.14,
                    ],
                    [0.15, 0.24, 0.185],
                    0.03,
                    CREAM.map(|v| v * (0.94 + h as f32 * 0.014)),
                )?;
                if (y + z) % 3 == 0 {
                    body.voxel(
                        [
                            side * (width * 0.5 + 0.075),
                            leg + 0.17 + y as f32 * 0.21,
                            (z as f32 - 3.0) * 0.14,
                        ],
                        [0.024, 0.064, 0.075],
                        CREAM.map(|v| v * 1.035),
                    )?;
                }
            }
        }
    }
    for x in -1i32..=1 {
        for z in 0..7 {
            shape::rounded(
                body,
                [
                    x as f32 * 0.23,
                    leg + height - 0.005,
                    (z as f32 - 3.0) * 0.14,
                ],
                [0.27, 0.16, 0.185],
                0.027,
                CREAM.map(|v| v * (0.97 + ((x + z) & 3) as f32 * 0.009)),
            )?;
        }
    }
    Ok(())
}

fn cow_coat(body: &mut Builder, leg: f32, height: f32, width: f32) -> Result<(), String> {
    for side in [-1.0, 1.0] {
        for (y, z, w, h) in [
            (0.30, -0.31, 0.32, 0.32),
            (0.50, 0.15, 0.40, 0.26),
            (0.24, 0.40, 0.20, 0.20),
        ] {
            body.voxel(
                [side * (width * 0.5 + 0.004), leg + y, z],
                [0.014, h, w],
                CREAM,
            )?;
            for i in [-1.0, 1.0] {
                body.voxel(
                    [
                        side * (width * 0.5 + 0.004),
                        leg + y + i * h * 0.34,
                        z + i * w * 0.50,
                    ],
                    [0.014, h * 0.40, w * 0.25],
                    CREAM,
                )?;
            }
        }
        for i in 0..8 {
            body.voxel(
                [
                    side * (width * 0.5 + 0.013),
                    leg + 0.24 + (i % 3) as f32 * 0.13,
                    -0.39 + i as f32 * 0.105,
                ],
                [0.012, 0.019, 0.043],
                [0.39, 0.22, 0.13],
            )?;
        }
    }
    body.voxel(
        [0.0, leg + height + 0.004, 0.12],
        [0.39, 0.012, 0.58],
        CREAM,
    )?;
    shape::rounded(
        body,
        [0.0, leg - 0.025, 0.27],
        [0.28, 0.17, 0.34],
        0.04,
        [0.78, 0.46, 0.39],
    )?;
    for x in [-0.075, 0.075] {
        for z in [0.18, 0.34] {
            shape::rounded(
                body,
                [x, leg - 0.12, z],
                [0.047, 0.065, 0.047],
                0.01,
                [0.70, 0.36, 0.30],
            )?;
        }
    }
    Ok(())
}

fn pig_coat(body: &mut Builder, leg: f32, width: f32) -> Result<(), String> {
    for side in [-1.0, 1.0] {
        for i in 0..12 {
            let z = -0.32 + (i % 6) as f32 * 0.12;
            let y = leg + 0.18 + (i / 6) as f32 * 0.16;
            body.voxel(
                [side * (width * 0.5 + 0.004), y, z],
                [0.012, 0.025 + (i % 3) as f32 * 0.008, 0.046],
                if i % 3 == 0 {
                    [0.78, 0.41, 0.37]
                } else {
                    [0.92, 0.55, 0.48]
                },
            )?;
        }
    }
    for i in 0..9 {
        body.voxel(
            [0.0, leg + 0.56, (i as f32 - 4.0) * 0.075],
            [0.013, 0.038, 0.012],
            [0.71, 0.39, 0.33],
        )?;
    }
    Ok(())
}
