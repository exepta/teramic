//! Layered armor solids, in the bind space of the existing animated body joints.
use crate::{
    meshing::builder::MeshBatch,
    players::{AvatarPart, model::Builder},
};
use bevy::prelude::*;
use terra_core::{AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, ItemKind, PlayerGender};

#[derive(Clone, Copy)]
struct Palette {
    plate: [f32; 3],
    edge: [f32; 3],
    dark: [f32; 3],
    cloth: [f32; 3],
    gem: Option<[f32; 3]>,
}

impl Palette {
    fn new(material: ArmorMaterial) -> Self {
        match material {
            ArmorMaterial::Leather => Self {
                plate: [0.61, 0.34, 0.16],
                edge: [0.80, 0.54, 0.29],
                dark: [0.32, 0.15, 0.07],
                cloth: [0.45, 0.25, 0.12],
                gem: None,
            },
            ArmorMaterial::Iron => Self {
                plate: [0.67, 0.72, 0.78],
                edge: [0.88, 0.91, 0.94],
                dark: [0.28, 0.34, 0.42],
                cloth: [0.25, 0.22, 0.20],
                gem: None,
            },
            ArmorMaterial::Gold => Self {
                plate: [0.93, 0.58, 0.12],
                edge: [1.0, 0.84, 0.36],
                dark: [0.48, 0.27, 0.065],
                cloth: [0.055, 0.22, 0.47],
                gem: Some([0.04, 0.64, 0.94]),
            },
            ArmorMaterial::Diamond => Self {
                plate: [0.09, 0.47, 0.61],
                edge: [0.70, 0.90, 0.95],
                dark: [0.035, 0.20, 0.29],
                cloth: [0.055, 0.31, 0.41],
                gem: Some([0.03, 0.85, 0.98]),
            },
            ArmorMaterial::Ruby => Self {
                plate: [0.45, 0.055, 0.10],
                edge: [0.96, 0.55, 0.20],
                dark: [0.20, 0.025, 0.055],
                cloth: [0.31, 0.025, 0.07],
                gem: Some([0.98, 0.035, 0.16]),
            },
        }
    }
}

fn block(b: &mut Builder, p: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Result<(), String> {
    b.bevel_box(p, size, color, Quat::IDENTITY)
}

fn tilted(
    b: &mut Builder,
    p: [f32; 3],
    size: [f32; 3],
    color: [f32; 3],
    angle: f32,
) -> Result<(), String> {
    b.bevel_box(p, size, color, Quat::from_rotation_z(angle))
}

fn rivet(b: &mut Builder, p: [f32; 3], color: [f32; 3]) -> Result<(), String> {
    block(b, p, [0.016, 0.016, 0.013], color)
}

fn buckle(b: &mut Builder, p: [f32; 3], width: f32, color: [f32; 3]) -> Result<(), String> {
    for s in [-1.0, 1.0] {
        block(
            b,
            [p[0] + s * width * 0.5, p[1], p[2]],
            [0.009, width * 0.8, 0.014],
            color,
        )?;
        block(
            b,
            [p[0], p[1] + s * width * 0.4, p[2]],
            [width, 0.009, 0.014],
            color,
        )?;
    }
    block(
        b,
        [p[0], p[1], p[2] - 0.007],
        [width * 0.7, 0.007, 0.012],
        color,
    )
}

fn gem(b: &mut Builder, p: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Result<(), String> {
    let center = Vec3::from_array(p);
    let ring = [
        Vec3::new(0.0, size[1] * 0.5, 0.0),
        Vec3::new(size[0] * 0.5, 0.0, 0.0),
        Vec3::new(0.0, -size[1] * 0.5, 0.0),
        Vec3::new(-size[0] * 0.5, 0.0, 0.0),
    ];
    let tip = center - Vec3::Z * size[2];
    let back = center + Vec3::Z * size[2] * 0.15;
    for side in 0..4 {
        let next = (side + 1) % 4;
        b.triangle(
            [center + ring[side], center + ring[next], tip],
            color.map(|v| (v * [1.13, 0.88, 0.72, 1.02][side]).min(1.0)),
        )?;
        b.triangle(
            [center + ring[next], center + ring[side], back],
            color.map(|v| v * 0.75),
        )?;
    }
    Ok(())
}

fn band(b: &mut Builder, y: f32, w: f32, d: f32, h: f32, color: [f32; 3]) -> Result<(), String> {
    for s in [-1.0, 1.0] {
        block(b, [0.0, y, s * d * 0.5], [w, h, 0.024], color)?;
        block(b, [s * w * 0.5, y, 0.0], [0.024, h, d], color)?;
    }
    Ok(())
}

fn helmet(material: ArmorMaterial) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    // Open face and layered stepped shell keep the eyes and mouth unobstructed.
    block(
        &mut builder,
        [0.0, 0.217, 0.015],
        [0.397, 0.070, 0.364],
        palette.dark,
    )?;
    for x in -3..=3 {
        let height = 0.244 - (x as f32).abs() * 0.013;
        for z in -2..=2 {
            let y = height - (z as f32).abs() * 0.010;
            block(
                &mut builder,
                [x as f32 * 0.059, y, z as f32 * 0.065 + 0.012],
                [0.061, 0.059, 0.069],
                palette
                    .plate
                    .map(|v| v * (0.96 + ((x + z + 7) % 3) as f32 * 0.025)),
            )?;
        }
    }
    for side in [-1.0, 1.0] {
        for row in 0..4 {
            block(
                &mut builder,
                [
                    side * (0.202 + (row % 2) as f32 * 0.005),
                    0.151 - row as f32 * 0.062,
                    0.012,
                ],
                [0.045, 0.066, 0.37 - row as f32 * 0.014],
                palette.plate,
            )?;
        }
        block(
            &mut builder,
            [side * 0.198, -0.084, -0.094],
            [0.060, 0.088, 0.123],
            palette.plate,
        )?;
        block(
            &mut builder,
            [side * 0.205, -0.134, -0.096],
            [0.067, 0.022, 0.131],
            palette.edge,
        )?;
        for row in 0..3 {
            rivet(
                &mut builder,
                [side * 0.19, 0.155 - row as f32 * 0.104, -0.182],
                palette.edge,
            )?;
        }
    }
    for row in 0..5 {
        block(
            &mut builder,
            [0.0, 0.17 - row as f32 * 0.061, 0.190],
            [0.386 - (row > 2) as u8 as f32 * 0.03, 0.065, 0.039],
            if row % 2 == 0 {
                palette.plate
            } else {
                palette.dark
            },
        )?;
    }
    for x in -3..=3 {
        block(
            &mut builder,
            [x as f32 * 0.061, 0.122 + (x as f32).abs() * 0.011, -0.187],
            [0.064, 0.056, 0.046],
            palette.edge,
        )?;
    }
    if material == ArmorMaterial::Iron {
        for side in [-1.0, 1.0] {
            for slit in 0..3 {
                block(
                    &mut builder,
                    [side * (0.060 + slit as f32 * 0.045), 0.191, -0.192],
                    [0.015, 0.051, 0.011],
                    palette.dark,
                )?;
            }
        }
    }
    if let Some(color) = palette.gem {
        let jeweled = matches!(material, ArmorMaterial::Diamond | ArmorMaterial::Ruby);
        gem(
            &mut builder,
            [0.0, if jeweled { 0.238 } else { 0.164 }, -0.224],
            if jeweled {
                [0.132, 0.242, 0.058]
            } else {
                [0.065, 0.10, 0.027]
            },
            color,
        )?;
        for side in [-1.0, 1.0] {
            tilted(
                &mut builder,
                [side * 0.112, 0.143, -0.214],
                [0.153, 0.043, 0.037],
                palette.edge,
                -side * 0.29,
            )?;
            if jeweled {
                gem(
                    &mut builder,
                    [side * 0.229, 0.173, -0.073],
                    [0.077, 0.160, 0.055],
                    color,
                )?;
            }
        }
    }
    if material == ArmorMaterial::Leather {
        for x in -2..=2 {
            block(
                &mut builder,
                [x as f32 * 0.072, 0.244 - (x as f32).abs() * 0.012, -0.154],
                [0.024, 0.020, 0.019],
                palette.edge,
            )?;
        }
    }
    Ok(builder)
}

fn torso(material: ArmorMaterial, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let w = if female { 0.385 } else { 0.466 };
    let front = if female { -0.176 } else { -0.164 };
    // A shell rather than a torso-sized cube leaves the neckline and arm hinges open.
    for row in 0..4 {
        let width = w - row as f32 * 0.015;
        let y = 0.206 - row as f32 * 0.079;
        block(
            &mut builder,
            [0.0, y, front + row as f32 * 0.004],
            [width, 0.083, 0.035],
            palette.plate,
        )?;
        block(
            &mut builder,
            [0.0, y, 0.151],
            [width, 0.083, 0.035],
            palette.plate.map(|v| v * 0.91),
        )?;
        for side in [-1.0, 1.0] {
            block(
                &mut builder,
                [side * width * 0.48, y, 0.0],
                [0.037, 0.082, 0.293],
                palette.dark,
            )?;
        }
    }
    for row in 0..4 {
        for column in 0..3 {
            let x = (column as f32 - 1.0) * w * 0.29;
            let y = 0.198 - row as f32 * 0.074 + if column == 1 { 0.006 } else { 0.0 };
            block(
                &mut builder,
                [x, y, front - 0.024 + row as f32 * 0.002],
                [w * 0.278, 0.071, 0.019],
                palette
                    .plate
                    .map(|v| v * (0.97 + ((row + column) % 3) as f32 * 0.025)),
            )?;
            if material == ArmorMaterial::Leather {
                for sign in [-1.0, 1.0] {
                    rivet(
                        &mut builder,
                        [
                            x + sign * w * 0.10,
                            y + 0.021,
                            front - 0.039 + row as f32 * 0.002,
                        ],
                        palette.edge,
                    )?;
                }
            }
        }
    }
    for side in [-1.0, 1.0] {
        block(
            &mut builder,
            [side * (w * 0.5 - 0.048), 0.274, 0.0],
            [0.092, 0.129, 0.300],
            palette.plate,
        )?;
        tilted(
            &mut builder,
            [side * w * 0.22, 0.111, front - 0.024],
            [w * 0.53, 0.039, 0.025],
            palette.edge,
            side * 0.48,
        )?;
        tilted(
            &mut builder,
            [side * w * 0.20, 0.028, front - 0.024],
            [w * 0.48, 0.031, 0.021],
            palette.edge,
            side * 0.42,
        )?;
        rivet(
            &mut builder,
            [side * w * 0.37, 0.239, front - 0.026],
            palette.edge,
        )?;
    }
    if material == ArmorMaterial::Leather {
        tilted(
            &mut builder,
            [-0.030, 0.124, front - 0.036],
            [0.057, 0.388, 0.032],
            palette.dark,
            -0.60,
        )?;
        buckle(
            &mut builder,
            [-0.029, 0.126, front - 0.060],
            0.060,
            palette.edge,
        )?;
        for side in [-1.0, 1.0] {
            for stitch in 0..5 {
                rivet(
                    &mut builder,
                    [side * w * 0.43, 0.17 - stitch as f32 * 0.045, front - 0.019],
                    palette.edge,
                )?;
            }
        }
    } else if let Some(color) = palette.gem {
        gem(
            &mut builder,
            [0.0, 0.162, front - 0.055],
            [0.103, 0.147, 0.035],
            color,
        )?;
    }
    band(&mut builder, -0.075, w * 0.91, 0.315, 0.033, palette.edge)?;
    Ok(builder)
}

fn shoulder(material: ArmorMaterial, side: f32, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let width = if female { 0.16 } else { 0.185 };
    for row in 0..3 {
        let y = 0.026 - row as f32 * 0.051;
        let span = width + (2 - row) as f32 * 0.026;
        tilted(
            &mut builder,
            [side * row as f32 * 0.011, y, 0.0],
            [span, 0.061, 0.243 - row as f32 * 0.015],
            palette
                .plate
                .map(|value| value * (1.02 - row as f32 * 0.025)),
            side * 0.18,
        )?;
        for column in 0..3 {
            let x = side * row as f32 * 0.011 + (column as f32 - 1.0) * span * 0.30;
            tilted(
                &mut builder,
                [x, y, -0.130 + row as f32 * 0.007],
                [span * 0.31, 0.051, 0.020],
                palette
                    .plate
                    .map(|value| value * (0.97 + ((row + column) % 3) as f32 * 0.025)),
                side * 0.18,
            )?;
            if column != 1 {
                rivet(
                    &mut builder,
                    [x, y + 0.010, -0.146 + row as f32 * 0.007],
                    palette.edge,
                )?;
            }
        }
        for z in [-0.116, 0.116] {
            rivet(&mut builder, [side * 0.062, y, z], palette.edge)?;
        }
    }
    band(&mut builder, -0.15, width * 0.8, 0.17, 0.031, palette.dark)?;
    if matches!(material, ArmorMaterial::Diamond | ArmorMaterial::Ruby) {
        gem(
            &mut builder,
            [side * 0.080, 0.066, -0.094],
            [0.092, 0.156, 0.058],
            palette.gem.unwrap(),
        )?;
    } else if material == ArmorMaterial::Gold {
        block(
            &mut builder,
            [0.0, -0.12, -0.10],
            [width * 0.75, 0.059, 0.017],
            palette.cloth,
        )?;
    }
    Ok(builder)
}

fn glove(material: ArmorMaterial, side: f32, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let width = if female { 0.133 } else { 0.153 };
    for row in 0..3 {
        let y = -0.079 - row as f32 * 0.046;
        block(
            &mut builder,
            [0.0, y, 0.0],
            [width - row as f32 * 0.009, 0.053, 0.152],
            palette
                .plate
                .map(|value| value * (1.03 - row as f32 * 0.035)),
        )?;
    }
    block(
        &mut builder,
        [side * 0.008, -0.245, -0.034],
        [width * 0.91, 0.105, 0.105],
        palette.dark,
    )?;
    for finger in 0..4 {
        let x = side * 0.008 + (finger as f32 - 1.5) * width * 0.208;
        for joint in 0..2 {
            block(
                &mut builder,
                [
                    x,
                    -0.267 - joint as f32 * 0.025,
                    -0.071 - joint as f32 * 0.005,
                ],
                [width * 0.19, 0.027, 0.043],
                if joint == 0 {
                    palette.plate
                } else {
                    palette.edge
                },
            )?;
        }
    }
    tilted(
        &mut builder,
        [-side * width * 0.43, -0.253, -0.061],
        [0.041, 0.073, 0.050],
        palette.plate,
        -side * 0.35,
    )?;
    for row in 0..2 {
        band(
            &mut builder,
            -0.081 - row as f32 * 0.083,
            width * 1.03,
            0.161,
            0.021,
            palette.edge,
        )?;
    }
    if let Some(color) = palette.gem {
        gem(
            &mut builder,
            [0.0, -0.117, -0.091],
            [0.061, 0.090, 0.029],
            color,
        )?;
    } else {
        buckle(
            &mut builder,
            [side * 0.018, -0.090, -0.094],
            0.036,
            palette.edge,
        )?;
    }
    Ok(builder)
}

fn waist(material: ArmorMaterial, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let w = if female { 0.443 } else { 0.451 };
    band(&mut builder, -0.124, w, 0.323, 0.062, palette.dark)?;
    buckle(&mut builder, [0.0, -0.12, -0.182], 0.064, palette.edge)?;
    for side in [-1.0, 1.0] {
        for row in 0..3 {
            tilted(
                &mut builder,
                [
                    side * (w * 0.35 + row as f32 * 0.006),
                    -0.183 - row as f32 * 0.048,
                    -0.160,
                ],
                [w * 0.34, 0.057, 0.034],
                if row % 2 == 0 {
                    palette.plate
                } else {
                    palette.edge
                },
                -side * 0.17,
            )?;
        }
        for stud in 0..2 {
            rivet(
                &mut builder,
                [side * (0.070 + stud as f32 * 0.080), -0.122, -0.181],
                palette.edge,
            )?;
        }
    }
    if let Some(color) = palette.gem {
        gem(
            &mut builder,
            [0.0, -0.122, -0.204],
            [0.052, 0.056, 0.021],
            color,
        )?;
    }
    if material == ArmorMaterial::Gold || material == ArmorMaterial::Ruby {
        block(
            &mut builder,
            [0.0, -0.236, -0.174],
            [0.093, 0.178, 0.025],
            palette.cloth,
        )?;
        for side in [-1.0, 1.0] {
            block(
                &mut builder,
                [side * 0.051, -0.232, -0.184],
                [0.012, 0.185, 0.013],
                palette.edge,
            )?;
        }
        gem(
            &mut builder,
            [0.0, -0.323, -0.191],
            [0.092, 0.065, 0.012],
            palette.edge,
        )?;
    }
    Ok(builder)
}

fn leg(material: ArmorMaterial, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let w = if female { 0.190 } else { 0.215 };
    for row in 0..5 {
        let y = -0.080 - row as f32 * 0.046;
        let width = w - row as f32 * 0.008;
        block(
            &mut builder,
            [0.0, y, -0.120],
            [width, 0.052, 0.034],
            if row == 4 {
                palette.edge
            } else {
                palette.plate
            },
        )?;
        for side in [-1.0, 1.0] {
            block(
                &mut builder,
                [side * width * 0.47, y, 0.0],
                [0.034, 0.051, 0.230],
                palette.cloth,
            )?;
        }
        block(
            &mut builder,
            [0.0, y, 0.121],
            [width, 0.052, 0.023],
            palette.dark,
        )?;
    }
    for row in 0..2 {
        band(
            &mut builder,
            -0.108 - row as f32 * 0.122,
            w,
            0.255,
            0.019,
            palette.edge,
        )?;
    }
    Ok(builder)
}

fn boot(material: ArmorMaterial, female: bool) -> Result<Builder, String> {
    let palette = Palette::new(material);
    let mut builder = Builder::default();
    let w = if female { 0.201 } else { 0.223 };
    for row in 0..5 {
        let y = -0.020 - row as f32 * 0.049;
        let width = w - row.min(3) as f32 * 0.012;
        block(
            &mut builder,
            [0.0, y, -0.110],
            [width, 0.055, 0.040],
            palette
                .plate
                .map(|value| value * (1.04 - row as f32 * 0.018)),
        )?;
        for side in [-1.0, 1.0] {
            block(
                &mut builder,
                [side * width * 0.45, y, 0.004],
                [0.035, 0.055, 0.221],
                palette.plate,
            )?;
        }
        block(
            &mut builder,
            [0.0, y, 0.119],
            [width, 0.056, 0.024],
            palette.dark,
        )?;
    }
    // Knee shield and layered toe-cap, both following the calf joint.
    block(
        &mut builder,
        [0.0, 0.010, -0.133],
        [w * 0.9, 0.091, 0.051],
        palette.plate,
    )?;
    block(
        &mut builder,
        [0.0, -0.322, -0.051],
        [w, 0.059, 0.313],
        palette.dark,
    )?;
    for row in 0..4 {
        block(
            &mut builder,
            [
                0.0,
                -0.263 - row as f32 * 0.017,
                -0.057 - row as f32 * 0.039,
            ],
            [w * (0.80 + row as f32 * 0.055), 0.045, 0.065],
            if row == 3 {
                palette.edge
            } else {
                palette.plate
            },
        )?;
    }
    for row in 0..2 {
        band(
            &mut builder,
            -0.082 - row as f32 * 0.143,
            w * 0.94,
            0.259,
            0.026,
            palette.edge,
        )?;
        buckle(
            &mut builder,
            [0.0, -0.082 - row as f32 * 0.143, -0.150],
            0.036,
            palette.edge,
        )?;
    }
    if let Some(color) = palette.gem {
        gem(
            &mut builder,
            [0.0, 0.016, -0.171],
            [w * 0.65, 0.129, 0.032],
            color,
        )?;
        if material != ArmorMaterial::Gold {
            gem(
                &mut builder,
                [0.0, -0.235, -0.147],
                [w * 0.4, 0.063, 0.020],
                color,
            )?;
        }
    }
    Ok(builder)
}

pub(super) fn parts(
    item: ItemKind,
    gender: PlayerGender,
) -> Result<Vec<(AvatarPart, MeshBatch)>, String> {
    let female = gender == PlayerGender::Female;
    let mut out = Vec::new();
    match item {
        ItemKind::Armor(ArmorItem { material, piece }) => match piece {
            ArmorPiece::Helmet => out.push((AvatarPart::Head, helmet(material)?.finish_batch()?)),
            ArmorPiece::Chestplate => {
                out.push((AvatarPart::Body, torso(material, female)?.finish_batch()?));
                for side in [-1, 1] {
                    out.push((
                        AvatarPart::Arm(side),
                        shoulder(material, f32::from(side), female)?.finish_batch()?,
                    ));
                }
            }
            ArmorPiece::Gloves => {
                for side in [-1, 1] {
                    out.push((
                        AvatarPart::Forearm(side),
                        glove(material, f32::from(side), female)?.finish_batch()?,
                    ));
                }
            }
            ArmorPiece::Pants => {
                out.push((AvatarPart::Body, waist(material, female)?.finish_batch()?));
                for side in [-1, 1] {
                    out.push((
                        AvatarPart::Leg(side),
                        leg(material, female)?.finish_batch()?,
                    ));
                }
            }
            ArmorPiece::Boots => {
                for side in [-1, 1] {
                    out.push((
                        AvatarPart::Calf(side),
                        boot(material, female)?.finish_batch()?,
                    ));
                }
            }
        },
        ItemKind::Accessory(kind) => accessory(&mut out, kind, female)?,
        ItemKind::Cape(cape) => return super::cape::parts(cape, gender, None),
        ItemKind::HeraldicCape { cape, heraldry } => {
            return super::cape::parts(cape, gender, Some(heraldry));
        }
        _ => return Err("Not an armor or accessory item".into()),
    }
    Ok(out)
}

fn accessory(
    out: &mut Vec<(AvatarPart, MeshBatch)>,
    kind: AccessoryKind,
    female: bool,
) -> Result<(), String> {
    let mut b = Builder::default();
    let metal = [0.96, 0.76, 0.28];
    match kind {
        AccessoryKind::Earrings => {
            for side in [-1.0, 1.0] {
                block(
                    &mut b,
                    [side * 0.205, -0.075, -0.015],
                    [0.026, 0.049, 0.035],
                    metal,
                )?;
                gem(
                    &mut b,
                    [side * 0.212, -0.098, -0.037],
                    [0.033, 0.048, 0.014],
                    [0.1, 0.74, 0.92],
                )?;
            }
            out.push((AvatarPart::Head, b.finish_batch()?));
        }
        AccessoryKind::Necklace => {
            for side in [-1.0, 1.0] {
                for bead in 0..7 {
                    block(
                        &mut b,
                        [
                            side * (0.016 + bead as f32 * 0.020),
                            0.198 + bead as f32 * 0.014,
                            -0.183,
                        ],
                        [0.020, 0.019, 0.017],
                        metal,
                    )?;
                }
            }
            gem(
                &mut b,
                [0.0, 0.181, -0.201],
                [0.047, 0.069, 0.021],
                [0.75, 0.07, 0.21],
            )?;
            out.push((AvatarPart::Body, b.finish_batch()?));
        }
        AccessoryKind::Belt => {
            band(
                &mut b,
                -0.103,
                if female { 0.45 } else { 0.46 },
                0.341,
                0.040,
                [0.31, 0.14, 0.065],
            )?;
            buckle(&mut b, [0.0, -0.102, -0.193], 0.072, metal)?;
            out.push((AvatarPart::Body, b.finish_batch()?));
        }
        AccessoryKind::Socks => {
            for side in [-1, 1] {
                let mut sock = Builder::default();
                let width = if female { 0.167 } else { 0.189 };
                for row in 0..4 {
                    band(
                        &mut sock,
                        -0.16 - row as f32 * 0.040,
                        width,
                        0.190,
                        0.042,
                        if row % 2 == 0 {
                            [0.89, 0.85, 0.74]
                        } else {
                            [0.69, 0.65, 0.56]
                        },
                    )?;
                }
                block(
                    &mut sock,
                    [0.0, -0.319, -0.067],
                    [width, 0.048, 0.256],
                    [0.89, 0.85, 0.74],
                )?;
                out.push((AvatarPart::Calf(side), sock.finish_batch()?));
            }
        }
    }
    Ok(())
}

fn bind_position(part: AvatarPart) -> Vec3 {
    match part {
        AvatarPart::Body => Vec3::ZERO,
        AvatarPart::Head => Vec3::new(0.0, 0.549, 0.0),
        AvatarPart::Arm(side) => Vec3::new(f32::from(side) * 0.283, 0.305, 0.0),
        AvatarPart::Forearm(side) => Vec3::new(f32::from(side) * 0.283, 0.040, 0.0),
        AvatarPart::Leg(side) => Vec3::new(f32::from(side) * 0.115, -0.23, 0.0),
        AvatarPart::Calf(side) => Vec3::new(f32::from(side) * 0.115, -0.54, 0.0),
    }
}

/// Item solids are precisely the worn geometry, joined in bind pose and fitted
/// into the existing unit preview/drop space. No image-only armor substitutes.
pub(crate) fn item_batches(item: ItemKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let mut merged = MeshBatch {
        material,
        ..default()
    };
    for (part, batch) in parts(item, PlayerGender::Male)? {
        if matches!(
            item,
            ItemKind::Armor(ArmorItem {
                piece: ArmorPiece::Gloves | ArmorPiece::Boots,
                ..
            })
        ) && matches!(part, AvatarPart::Forearm(-1) | AvatarPart::Calf(-1))
        {
            continue;
        }
        let offset =
            u32::try_from(merged.positions.len()).map_err(|_| "Armor preview vertex limit")?;
        merged
            .positions
            .extend(batch.positions.into_iter().map(|mut p| {
                if item == ItemKind::Accessory(AccessoryKind::Earrings) {
                    p[0] -= p[0].signum() * 0.177;
                }
                (Vec3::from_array(p) + bind_position(part)).to_array()
            }));
        merged.normals.extend(batch.normals);
        merged.uvs.extend(batch.uvs);
        merged.colors.extend(batch.colors);
        merged
            .indices
            .extend(batch.indices.into_iter().map(|i| i + offset));
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &p in &merged.positions {
        min = min.min(Vec3::from_array(p));
        max = max.max(Vec3::from_array(p));
    }
    let scale = 0.84 / (max - min).max_element().max(0.01);
    let center = (min + max) * 0.5;
    // The shared inventory studio views +Z; worn avatars face -Z. Preserve the
    // decorative front in previews instead of showing plain backs of plates.
    let rotation = Quat::from_rotation_y(std::f32::consts::PI);
    for p in &mut merged.positions {
        *p = (rotation * (Vec3::from_array(*p) - center) * scale + Vec3::splat(0.5)).to_array();
    }
    for normal in &mut merged.normals {
        *normal = (rotation * Vec3::from_array(*normal)).to_array();
    }
    Ok(vec![merged])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_facets_have_outward_normals_and_visible_front_faces() {
        let mut builder = Builder::default();
        let center = Vec3::new(0.12, 0.24, -0.19);
        gem(
            &mut builder,
            center.to_array(),
            [0.13, 0.24, 0.05],
            [0.03, 0.85, 0.98],
        )
        .unwrap();
        let batch = builder.finish_batch().unwrap();
        let mut front = 0;
        for face in batch.indices.chunks_exact(3) {
            let [a, b, c] = [face[0], face[1], face[2]]
                .map(|index| Vec3::from_array(batch.positions[index as usize]));
            let normal = Vec3::from_array(batch.normals[face[0] as usize]);
            let centroid = (a + b + c) / 3.0;
            assert!(normal.dot(centroid - center) > 0.0);
            if centroid.z < center.z {
                assert!(normal.z < 0.0);
                front += 1;
            }
        }
        assert_eq!(front, 4);
    }

    #[test]
    fn gauntlet_and_boot_icons_show_one_readable_representative_not_far_apart_pairs() {
        for piece in [ArmorPiece::Gloves, ArmorPiece::Boots] {
            let item = ItemKind::Armor(ArmorItem {
                material: ArmorMaterial::Iron,
                piece,
            });
            let worn = parts(item, PlayerGender::Male).unwrap();
            assert_eq!(worn.len(), 2);
            let icon = item_batches(item, 0).unwrap().remove(0);
            assert_eq!(icon.positions.len(), worn[1].1.positions.len());
        }
        let pants = ItemKind::Armor(ArmorItem {
            material: ArmorMaterial::Iron,
            piece: ArmorPiece::Pants,
        });
        let worn = parts(pants, PlayerGender::Male).unwrap();
        assert_eq!(
            item_batches(pants, 0).unwrap()[0].positions.len(),
            worn.iter()
                .map(|(_, part)| part.positions.len())
                .sum::<usize>()
        );
    }

    #[test]
    fn earrings_pack_the_worn_pair_into_a_nonblank_inventory_silhouette() {
        let batch = item_batches(ItemKind::Accessory(AccessoryKind::Earrings), 0)
            .unwrap()
            .remove(0);
        let view = Transform::from_xyz(2.9, 2.5, 3.5)
            .looking_at(Vec3::splat(0.5), Vec3::Y)
            .compute_affine()
            .inverse();
        let vertices: Vec<_> = batch
            .positions
            .iter()
            .map(|p| view.transform_point3(Vec3::from_array(*p)).truncate())
            .collect();
        let mut pixels = std::collections::HashSet::new();
        for face in batch.indices.chunks_exact(3) {
            let [a, b, c] = [face[0], face[1], face[2]]
                .map(|i| vertices[i as usize] * (128.0 / 1.95) + Vec2::splat(64.0));
            let min = a.min(b).min(c).floor().max(Vec2::ZERO).as_ivec2();
            let max = a.max(b).max(c).ceil().min(Vec2::splat(127.0)).as_ivec2();
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let sides = [
                        (b - a).perp_dot(p - a),
                        (c - b).perp_dot(p - b),
                        (a - c).perp_dot(p - c),
                    ];
                    if sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0) {
                        pixels.insert((x, y));
                    }
                }
            }
        }
        assert!(
            pixels.len() > 128,
            "Inventory silhouette must comfortably exceed the 64-pixel readiness threshold, got {}",
            pixels.len()
        );
    }
}
