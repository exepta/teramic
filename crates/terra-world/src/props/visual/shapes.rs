use super::geometry::add_box;
use super::{PropDetail, PropMesh, PropPalette};
use crate::props::PropKind;
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;

pub fn template(
    kind: PropKind,
    variant: u8,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<Vec<PropMesh>, String> {
    let mut meshes = raw_template(kind, variant, level, palette)?;
    if kind != PropKind::Grass {
        super::sizing::normalize(kind, variant, level, &mut meshes)?;
    }
    Ok(meshes)
}

pub(super) fn raw_template(
    kind: PropKind,
    variant: u8,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<Vec<PropMesh>, String> {
    let mut batches = BTreeMap::new();
    let fine = level == PropDetail::Near;
    let variant = usize::from(variant);
    let leaf = palette.foliage[variant % 3];
    if kind.aquatic() {
        super::aquatic::append(&mut batches, kind, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind == PropKind::Mushroom && variant >= 128 {
        super::trunk_fungi::append(&mut batches, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind == PropKind::Mushroom {
        super::mushrooms::append(&mut batches, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind == PropKind::Grass {
        super::botany::grass(&mut batches, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind == PropKind::Flower {
        super::flowers::add_flowers(&mut batches, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind == PropKind::Bush {
        super::botany::bush(&mut batches, variant, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    let mut voxel = |material, center: [f32; 3], size: [f32; 3], rotation| {
        add_box(
            &mut batches,
            material,
            Vec3::from_array(center),
            Vec3::from_array(size),
            rotation,
        )
    };
    match kind {
        PropKind::BeeNest => {
            // A stepped papery comb, attached to the supporting tree face.
            for (y, width) in [
                (0.05, 0.26),
                (0.14, 0.36),
                (0.23, 0.40),
                (0.32, 0.36),
                (0.405, 0.28),
            ] {
                voxel(
                    palette.twig,
                    [0.0, y, 0.0],
                    [width, 0.09, width],
                    Quat::IDENTITY,
                )?;
                voxel(
                    palette.mushroom,
                    [0.0, y + 0.03, 0.0],
                    [width + 0.002, 0.018, width + 0.002],
                    Quat::IDENTITY,
                )?;
            }
            voxel(
                palette.stone,
                [0.0, 0.19, 0.201],
                [0.095, 0.065, 0.012],
                Quat::IDENTITY,
            )?;
        }
        PropKind::Grass
        | PropKind::Flower
        | PropKind::Bush
        | PropKind::Seagrass
        | PropKind::SeaCucumber
        | PropKind::BranchingCoral
        | PropKind::TubeCoral
        | PropKind::PlateCoral
        | PropKind::SeaAnemone
        | PropKind::SmallStones
        | PropKind::Mushroom => {}
        PropKind::Fern => {
            voxel(
                palette.stem,
                [0.0, 0.16, 0.0],
                [0.035, 0.32, 0.035],
                Quat::IDENTITY,
            )?;
            for i in 0..if fine { 5 } else { 4 } {
                let yaw = i as f32 * std::f32::consts::TAU / 5.0 + variant as f32 * 0.3;
                let rotation = Quat::from_rotation_y(yaw);
                for step in 1..=if fine { 3 } else { 1 } {
                    let reach = if fine { step as f32 * 0.085 } else { 0.18 };
                    let p = rotation * Vec3::new(reach, 0.27 + reach * 0.35, 0.0);
                    voxel(
                        palette.foliage[(i + variant) % 3],
                        p.to_array(),
                        [if fine { 0.1 } else { 0.28 }, 0.035, 0.19 - reach * 0.35],
                        rotation,
                    )?;
                }
            }
        }
        PropKind::SmallPlant if variant >= 128 => {
            voxel(
                palette.stone,
                [0.0, 0.07, 0.0],
                [0.34, 0.14, 0.29],
                Quat::IDENTITY,
            )?;
            if fine {
                voxel(
                    palette.stone,
                    [0.12, 0.04, 0.09],
                    [0.16, 0.08, 0.16],
                    Quat::IDENTITY,
                )?;
            }
        }
        PropKind::SmallPlant => {
            for i in 0..if fine { 4 } else { 2 } {
                let h = 0.6 + ((i + variant) % 3) as f32 * 0.13;
                let x = i as f32 * 0.085 - 0.13;
                voxel(leaf, [x, h * 0.5, 0.0], [0.035, h, 0.035], Quat::IDENTITY)?;
                voxel(
                    palette.stem,
                    [x, h - 0.04, 0.0],
                    [0.065, 0.16, 0.065],
                    Quat::IDENTITY,
                )?;
            }
        }
    }
    Ok(batches.into_values().collect())
}
