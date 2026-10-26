//! Rear-only stepped cloth and tier-specific shoulder fasteners in torso bind space.
use crate::{
    meshing::builder::MeshBatch,
    players::{AvatarPart, model::Builder},
};
use bevy::prelude::*;
use terra_core::{ArmorMaterial, CapeItem, PlayerGender};

fn cloth(cape: CapeItem, shade: f32) -> [f32; 3] {
    let tint = cape.color.tint();
    [tint[0] * shade, tint[1] * shade, tint[2] * shade].map(|value| value.min(1.0))
}

fn trim(cape: CapeItem, shade: f32) -> [f32; 3] {
    let tint = cape.material.color();
    [tint[0], tint[1], tint[2]].map(|value| (value * shade).min(1.0))
}

fn tile(
    builder: &mut Builder,
    position: [f32; 3],
    size: [f32; 3],
    color: [f32; 3],
) -> Result<(), String> {
    builder.voxel(position, size, color)
}

pub(super) const SHOULDER_PIVOT: Vec3 = Vec3::new(0.0, 0.26, 0.24);
pub(super) const HEM_PIVOT: Vec3 = Vec3::new(0.0, -0.2945, 0.273);

fn shoulders(cape: CapeItem, female: bool) -> Result<MeshBatch, String> {
    let mut b = Builder::default();
    let width = if female { 0.45 } else { 0.48 };
    for row in 0..3 {
        tile(
            &mut b,
            [0.0, 0.315 - row as f32 * 0.033, 0.238],
            [width - row as f32 * 0.014, 0.039, 0.057],
            cloth(cape, 0.94 - row as f32 * 0.025),
        )?;
    }
    for side in [-1.0, 1.0] {
        for step in 0..3 {
            b.bevel_box(
                [
                    side * (width * 0.39 - step as f32 * 0.012),
                    0.326 - step as f32 * 0.035,
                    0.281,
                ],
                [0.065, 0.044, 0.028],
                trim(cape, 0.96 + 0.08 * (step % 2) as f32),
                Quat::from_rotation_z(-side * 0.22),
            )?;
        }
        tile(
            &mut b,
            [side * width * 0.39, 0.324, 0.301],
            [0.018, 0.018, 0.012],
            trim(cape, 1.4),
        )?;
    }
    b.bevel_box(
        [0.0, 0.287, 0.277],
        [0.078, 0.078, 0.025],
        trim(cape, 1.25),
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
    )?;
    let gem = match cape.material {
        ArmorMaterial::Diamond => Some([0.06, 0.88, 0.98]),
        ArmorMaterial::Ruby => Some([0.97, 0.045, 0.13]),
        ArmorMaterial::Gold => Some([1.0, 0.75, 0.20]),
        _ => None,
    };
    if let Some(color) = gem {
        b.bevel_box(
            [0.0, 0.287, 0.298],
            [0.048, 0.048, 0.024],
            color,
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
        )?;
    }
    b.finish_batch()
}

fn panel(cape: CapeItem, female: bool, first: usize, end: usize) -> Result<MeshBatch, String> {
    let mut b = Builder::default();
    for row in first..end {
        let t = row as f32 / 13.0;
        let span = (if female { 0.43 } else { 0.46 }) + t * 0.31;
        let y = 0.219 - row as f32 * 0.079;
        let columns = 9;
        for column in 0..columns {
            let u = (column as f32 - 4.0) / 4.0;
            let hem = if row == 13 {
                (column % 3) as f32 * 0.016
            } else {
                0.0
            };
            let z = 0.226 + t * 0.081 + (column % 2) as f32 * 0.013;
            let shade = 0.84 + (column % 3) as f32 * 0.043 + (row % 2) as f32 * 0.016;
            tile(
                &mut b,
                [u * span * 0.46, y + hem, z],
                [
                    span / columns as f32 + 0.003,
                    if row == 6 || row == 7 { 0.097 } else { 0.083 },
                    0.035,
                ],
                cloth(cape, shade),
            )?;
            if row == 12 || column == 0 || column == 8 {
                tile(
                    &mut b,
                    [u * span * 0.46, y + hem, z + 0.021],
                    [
                        span / columns as f32 + 0.004,
                        if row == 12 { 0.024 } else { 0.075 },
                        0.017,
                    ],
                    trim(cape, if row % 2 == 0 { 1.15 } else { 0.95 }),
                )?;
            }
        }
    }
    b.finish_batch()
}

pub(super) fn parts(
    cape: CapeItem,
    gender: PlayerGender,
) -> Result<Vec<(AvatarPart, MeshBatch)>, String> {
    let female = gender == PlayerGender::Female;
    Ok(vec![
        (AvatarPart::Body, shoulders(cape, female)?),
        (AvatarPart::Body, panel(cape, female, 0, 7)?),
        (AvatarPart::Body, panel(cape, female, 7, 14)?),
    ])
}
