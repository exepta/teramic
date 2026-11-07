//! Rear-only stepped cloth and tier-specific shoulder fasteners in torso bind space.
use crate::{
    meshing::builder::MeshBatch,
    players::{AvatarPart, model::Builder},
};
use bevy::prelude::*;
use terra_core::{ArmorMaterial, CapeItem, PlayerGender};

fn cloth(cape: CapeItem, shade: f32) -> [f32; 3] {
    // Avatar Builder accepts sRGB and decodes once when writing its vertices.
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

fn panel(
    cape: CapeItem,
    female: bool,
    first: usize,
    end: usize,
    heraldry: Option<terra_core::kingdom::KingdomHeraldry>,
) -> Result<MeshBatch, String> {
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
    if let Some(heraldry) = heraldry {
        emblem(&mut b, heraldry, first, end)?;
    }
    b.finish_batch()
}

fn emblem(
    b: &mut Builder,
    heraldry: terra_core::kingdom::KingdomHeraldry,
    first: usize,
    end: usize,
) -> Result<(), String> {
    let Some(mask) = heraldry.icon.glyph_mask() else {
        return Ok(());
    };
    let [r, g, blue] = heraldry.color.map(f32::from);
    let ink = if r * 0.2126 + g * 0.7152 + blue * 0.0722 > 154.0 {
        [0.11, 0.10, 0.085]
    } else {
        [0.96, 0.92, 0.79]
    };
    for (y, &pixels) in mask.iter().enumerate() {
        let world_y = 0.08 - y as f32 * 0.012;
        let cloth_row = ((0.219 - world_y) / 0.079).floor() as usize;
        if !(first..end).contains(&cloth_row) {
            continue;
        }
        let mut x = 0;
        while x < 32 {
            if pixels & (1 << x) == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < 32 && pixels & (1 << x) != 0 {
                x += 1;
            }
            let width = (x - start) as f32 * 0.012;
            tile(
                b,
                [
                    ((start + x) as f32 * 0.5 - 16.0) * 0.012,
                    world_y,
                    0.282 + cloth_row as f32 / 13.0 * 0.081,
                ],
                [width, 0.0125, 0.009],
                ink,
            )?;
        }
    }
    Ok(())
}

pub(super) fn parts(
    cape: CapeItem,
    gender: PlayerGender,
    heraldry: Option<terra_core::kingdom::KingdomHeraldry>,
) -> Result<Vec<(AvatarPart, MeshBatch)>, String> {
    let female = gender == PlayerGender::Female;
    Ok(vec![
        (AvatarPart::Body, shoulders(cape, female)?),
        (AvatarPart::Body, panel(cape, female, 0, 7, heraldry)?),
        (AvatarPart::Body, panel(cape, female, 7, 14, heraldry)?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heraldic_cape_renders_the_saved_icon_on_the_existing_animated_cloth() {
        use terra_core::kingdom::{KingdomHeraldry, KingdomIcon};
        let heraldry = KingdomHeraldry {
            color: [27, 109, 181],
            icon: KingdomIcon::Anchor,
        };
        let cape = CapeItem {
            color: terra_core::beds::DyeColor::Rgb(heraldry.color),
            ..CapeItem::ALL_WHITE[0]
        };
        for gender in [PlayerGender::Male, PlayerGender::Female] {
            let plain = parts(cape, gender, None).unwrap();
            let decorated = parts(cape, gender, Some(heraldry)).unwrap();
            let other = parts(
                cape,
                gender,
                Some(KingdomHeraldry {
                    icon: KingdomIcon::from_key("flame").unwrap(),
                    ..heraldry
                }),
            )
            .unwrap();
            assert_eq!(decorated.len(), 3);
            assert_eq!(plain[0].1, decorated[0].1);
            assert!(decorated[1].1.positions.len() > plain[1].1.positions.len());
            assert_ne!(decorated[1].1.positions, other[1].1.positions);
            for (_, mesh) in decorated {
                assert_eq!(mesh.positions.len(), mesh.colors.len());
                assert!(
                    mesh.positions
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                );
            }
        }
    }

    #[test]
    fn neutral_rgb_cape_cloth_is_decoded_once_by_the_real_avatar_builder() {
        let cape = CapeItem {
            material: ArmorMaterial::Leather,
            color: terra_core::beds::DyeColor::Rgb([71, 93, 184]),
        };
        let mut builder = Builder::default();
        tile(&mut builder, [0.0; 3], [0.5; 3], cloth(cape, 1.0)).unwrap();
        let batch = builder.finish_batch().unwrap();
        let expected = crate::dye_tint::color(cape.color);
        assert!(!batch.colors.is_empty());
        for actual in &batch.colors {
            for (a, b) in actual.iter().zip(expected) {
                assert!((*a - b).abs() < 2e-7);
            }
        }
    }
}
