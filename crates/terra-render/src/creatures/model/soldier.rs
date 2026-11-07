//! Shared military rigs with real articulated sword, spear and bow silhouettes.
use super::{Builder, Part, villager};
use bevy::prelude::*;
use terra_core::{creatures::soldiers::SoldierLoadout, kingdom::KingdomHeraldry};

const WOOD: [f32; 3] = [0.36, 0.20, 0.09];
const STEEL: [f32; 3] = [0.73, 0.77, 0.81];
const GOLD: [f32; 3] = [0.88, 0.66, 0.26];

pub(super) fn parts(loadout: SoldierLoadout) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let base = match loadout {
        SoldierLoadout::Sword => 2,
        SoldierLoadout::Spear => 4,
        SoldierLoadout::Bow => 3,
    };
    let mut parts = villager::parts(base)?;
    parts.push((
        Part::MilitaryWeapon(1.0),
        Vec3::new(0.30, 1.15, 0.0),
        weapon(loadout)?,
    ));
    parts.push((
        Part::MilitaryTabard,
        Vec3::ZERO,
        tabard(KingdomHeraldry::default())?,
    ));
    Ok(parts)
}

fn weapon(loadout: SoldierLoadout) -> Result<Mesh, String> {
    let mut b = Builder::default();
    match loadout {
        SoldierLoadout::Sword => {
            b.voxel([0.008, -0.43, -0.028], [0.055, 0.20, 0.055], WOOD)?;
            b.voxel([0.008, -0.32, -0.028], [0.28, 0.055, 0.075], GOLD)?;
            b.voxel([0.008, -0.03, -0.028], [0.095, 0.55, 0.040], STEEL)?;
            b.voxel([0.008, 0.25, -0.028], [0.055, 0.08, 0.040], STEEL)?;
            b.voxel([0.008, 0.30, -0.028], [0.025, 0.045, 0.035], STEEL)?;
            b.voxel([0.008, -0.55, -0.028], [0.09, 0.075, 0.08], GOLD)?;
        }
        SoldierLoadout::Spear => {
            b.voxel([0.008, -0.12, -0.028], [0.045, 1.85, 0.045], WOOD)?;
            for y in [-0.52, -0.43, -0.34] {
                b.voxel([0.008, y, -0.028], [0.06, 0.045, 0.06], GOLD)?;
            }
            b.voxel([0.008, 0.82, -0.028], [0.10, 0.14, 0.065], STEEL)?;
            b.voxel([0.008, 0.94, -0.028], [0.075, 0.12, 0.05], STEEL)?;
            b.voxel([0.008, 1.025, -0.028], [0.035, 0.065, 0.04], STEEL)?;
        }
        SoldierLoadout::Bow => {
            for i in 0..15 {
                let t = i as f32 / 14.0;
                let at = [
                    0.008,
                    -0.43 + (t - 0.5) * 1.1,
                    -0.028 - (std::f32::consts::PI * t).sin() * 0.22,
                ];
                b.voxel(at, [0.055, 0.09, 0.06], WOOD)?;
            }
            b.voxel(
                [0.008, -0.43, -0.028],
                [0.012, 1.1, 0.012],
                [0.93, 0.88, 0.71],
            )?;
            b.voxel([0.008, -0.43, -0.235], [0.075, 0.16, 0.075], GOLD)?;
        }
    }
    b.finish()
}

/// Front and rear cloth share one small cached mesh per concrete realm emblem.
pub(crate) fn tabard(heraldry: KingdomHeraldry) -> Result<Mesh, String> {
    let mut b = Builder::default();
    // Builder decodes authored sRGB colors once when emitting vertex colors.
    let color = heraldry.color.map(|value| f32::from(value) / 255.0);
    for z in [-0.245, 0.245] {
        b.voxel([0.0, 0.91, z], [0.275, 0.43, 0.018], color)?;
        b.voxel([0.0, 0.69, z], [0.23, 0.06, 0.018], color)?;
    }
    if let Some(mask) = heraldry.icon.glyph_mask() {
        for (row, &bits) in mask.iter().enumerate() {
            let mut x = 0;
            while x < 32 {
                if bits & (1 << x) == 0 {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < 32 && bits & (1 << x) != 0 {
                    x += 1;
                }
                let cell = 0.006;
                let p = [
                    (start as f32 + (x - start) as f32 * 0.5 - 16.0) * cell,
                    1.01 - row as f32 * cell,
                    -0.26,
                ];
                for z in [-0.26, 0.26] {
                    b.voxel(
                        [p[0], p[1], z],
                        [(x - start) as f32 * cell, cell, 0.004],
                        [0.97, 0.91, 0.72],
                    )?;
                }
            }
        }
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    #[test]
    fn three_military_loadouts_are_articulated_distinct_bounded_and_retain_real_emblems() {
        let mut shapes = std::collections::BTreeSet::new();
        for loadout in SoldierLoadout::ALL {
            let parts = parts(loadout).unwrap();
            assert_eq!(parts.len(), 8);
            let (_, offset, mesh) = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::MilitaryWeapon(_)))
                .unwrap();
            assert_eq!(*offset, Vec3::new(0.3, 1.15, 0.0));
            let Some(VertexAttributeValues::Float32x3(p)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("weapon positions")
            };
            assert!(p.iter().flatten().all(|v| v.is_finite()));
            shapes.insert((
                p.len(),
                p.iter().map(|p| p[1].to_bits()).collect::<Vec<_>>(),
            ));
            assert!(
                parts
                    .iter()
                    .map(|(_, _, m)| m.count_vertices())
                    .sum::<usize>()
                    < 24000
            );
        }
        assert_eq!(shapes.len(), 3);
        let a = tabard(terra_core::kingdom::BotKingdom::Lutera.heraldry()).unwrap();
        let b = tabard(terra_core::kingdom::BotKingdom::Petora.heraldry()).unwrap();
        let Some(VertexAttributeValues::Float32x4(colors)) = a.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("tabard palette")
        };
        let [red, green, blue] = terra_core::kingdom::BotKingdom::Lutera.heraldry().color;
        let expected = Color::srgb_u8(red, green, blue).to_linear().to_f32_array();
        assert!(
            colors.iter().any(|color| color
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (*actual - expected).abs() < 2e-7)),
            "Tabard and world banners must decode the same picker RGB once"
        );
        assert_ne!(
            a.attribute(Mesh::ATTRIBUTE_COLOR),
            b.attribute(Mesh::ATTRIBUTE_COLOR)
        );
        assert_ne!(
            a.attribute(Mesh::ATTRIBUTE_POSITION),
            b.attribute(Mesh::ATTRIBUTE_POSITION)
        );
    }
}
