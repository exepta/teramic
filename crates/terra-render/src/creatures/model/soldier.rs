//! Shared military rigs with real articulated sword, spear and bow silhouettes.
use super::{Builder, Part, villager};
use bevy::prelude::*;
use terra_core::{creatures::soldiers::SoldierLoadout, kingdom::KingdomHeraldry};

const WOOD: [f32; 3] = [0.36, 0.20, 0.09];
const STEEL: [f32; 3] = [0.73, 0.77, 0.81];
const GOLD: [f32; 3] = [0.88, 0.66, 0.26];

pub(super) fn parts(loadout: SoldierLoadout) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let base = match loadout {
        SoldierLoadout::Sword => 4,
        SoldierLoadout::Spear => 2,
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
    // The existing emblem is already axis-aligned voxel artwork. Keep its
    // 32-pixel glyph detail and exact banner colors rather than resampling it.
    let mut b = crate::players::model::Builder::default();
    // Builder decodes authored sRGB colors once when emitting vertex colors.
    let color = heraldry.color.map(|value| f32::from(value) / 255.0);
    for z in [-0.245, 0.245] {
        cloth_panel(&mut b, z, color)?;
    }
    let mut mesh = b.finish()?;
    if let Some(mask) = heraldry.icon.glyph_mask() {
        // Adjacent glyph rows are one solid extrusion, not closed cuboids
        // with two coincident contact faces. Keep every authored pixel and
        // the original 4 mm relief; only hidden interior faces disappear.
        let cells = mask
            .iter()
            .enumerate()
            .flat_map(|(row, bits)| {
                (0..32_i16).filter_map(move |x| {
                    (bits & (1 << x) != 0).then_some((
                        [x, 31 - row as i16, 0],
                        [0.97_f32, 0.91, 0.72].map(f32::to_bits),
                    ))
                })
            })
            .collect();
        for z in [-0.26, 0.26] {
            let origin = Vec3::new(-0.096, 1.013 - 32.0 * 0.006, z - 0.002);
            let emblem = super::voxel::surface::mesh(
                &cells,
                origin,
                0.006,
                origin + Vec3::new(0.192, 0.192, 0.004),
            )?;
            mesh.merge(&emblem)
                .map_err(|error| format!("Cannot combine military emblem: {error}"))?;
        }
    }
    Ok(mesh)
}

fn cloth_panel(
    builder: &mut crate::players::model::Builder,
    z: f32,
    color: [f32; 3],
) -> Result<(), String> {
    // The narrow hem overlaps the upper cloth in the authored design. Emit
    // the boundary of their union, retaining its exact outline and thickness.
    let outline = [
        [-0.115, 0.66],
        [0.115, 0.66],
        [0.115, 0.695],
        [0.1375, 0.695],
        [0.1375, 1.125],
        [-0.1375, 1.125],
        [-0.1375, 0.695],
        [-0.115, 0.695],
    ];
    for (width, bottom, top) in [(0.115, 0.66, 0.695), (0.1375, 0.695, 1.125)] {
        for sign in [-1.0, 1.0] {
            let mut points = [
                Vec3::new(-width, bottom, z + sign * 0.009),
                Vec3::new(width, bottom, z + sign * 0.009),
                Vec3::new(width, top, z + sign * 0.009),
                Vec3::new(-width, top, z + sign * 0.009),
            ];
            if sign < 0.0 {
                points.reverse();
            }
            builder.quad(points, color)?;
        }
    }
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        builder.quad(
            [
                Vec3::new(a[0], a[1], z - 0.009),
                Vec3::new(b[0], b[1], z - 0.009),
                Vec3::new(b[0], b[1], z + 0.009),
                Vec3::new(a[0], a[1], z + 0.009),
            ],
            color,
        )?;
    }
    Ok(())
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
            assert_eq!(parts.len(), 12);
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
                    < 60000
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
