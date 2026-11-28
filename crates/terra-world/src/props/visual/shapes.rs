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
    if !matches!(
        kind,
        PropKind::Grass | PropKind::SpiderWeb | PropKind::AppleFruit
    ) {
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
    if kind == PropKind::AppleFruit {
        return Ok(super::apple(variant as u8, level, palette));
    }
    if let Some(species) = kind.sapling() {
        return super::saplings::template(species, level, palette);
    }
    if kind == PropKind::Coconut {
        return Ok(super::fruit::coconut(variant as u8, palette));
    }
    if kind == PropKind::SpiderWeb {
        return super::webs::template(variant as u8, level, palette);
    }
    if kind == PropKind::Scarecrow {
        super::scarecrow::append(&mut batches, variant as u8, level, palette)?;
        return Ok(batches.into_values().collect());
    }
    if kind.crop().is_some() {
        super::crops::append(&mut batches, kind, variant as u8, level, palette)?;
        return Ok(batches.into_values().collect());
    }
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
    if kind == PropKind::Fern {
        super::fern::append(&mut batches, variant, level, palette)?;
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
        | PropKind::Fern
        | PropKind::Bush
        | PropKind::Seagrass
        | PropKind::SeaCucumber
        | PropKind::BranchingCoral
        | PropKind::TubeCoral
        | PropKind::PlateCoral
        | PropKind::ReefFan
        | PropKind::HangingPearlKelp
        | PropKind::SeaAnemone
        | PropKind::SmallStones
        | PropKind::Wheat
        | PropKind::Corn
        | PropKind::Lettuce
        | PropKind::Scarecrow
        | PropKind::SpiderWeb
        | PropKind::Mushroom
        | PropKind::OakSapling
        | PropKind::SpruceSapling
        | PropKind::BirchSapling
        | PropKind::CherrySapling
        | PropKind::AppleSapling
        | PropKind::AcaciaSapling
        | PropKind::Coconut
        | PropKind::AppleFruit => {}
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
            super::small_plant::append(&mut batches, variant, level, palette)?;
        }
    }
    Ok(batches.into_values().collect())
}
