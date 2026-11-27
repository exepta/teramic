//! Low-density marine patches are planned once per global tile and shared by its sections.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use crate::generation::OceanArea;
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

fn community(temp: f64, colony: f32, cover: f32, roll: u32) -> M {
    let shell = [
        M::ClamShell,
        M::ReefOyster,
        M::SpiralShell,
        M::ConeShell,
        M::ScallopShell,
        M::Starfish,
        M::SeaUrchin,
        M::SandDollar,
    ];
    let rock = [
        M::TidalBoulders,
        M::BarnacledBoulder,
        M::ReefLedge,
        M::ReefPillar,
        M::ReefArch,
    ];
    let choice = roll % 100;
    let variant = (roll / 100) as usize;
    if choice < 23 {
        return shell[variant % shell.len()];
    }
    if choice < 35 {
        return rock[variant % rock.len()];
    }
    if choice == 35 {
        return M::BubbleVent;
    }
    if choice == 36 {
        return M::SeaCrystal;
    }
    if choice < 40 {
        return M::MarineDriftwood;
    }
    if temp > 0.55 && colony > 0.32 && choice < 89 {
        let reef = [
            M::BrainCoral,
            M::TableCoral,
            M::StaghornCoral,
            M::OrganPipeCoral,
            M::SeaFan,
            M::MushroomCoral,
            M::CoralBush,
            M::LuminousCoral,
        ];
        return reef[variant % reef.len()];
    }
    if temp < 0.36 {
        if choice < 62 {
            M::BarnacledBoulder
        } else if choice < 76 {
            M::BrownAlgae
        } else {
            M::RibbonKelp
        }
    } else if cover > 0.41 && choice > 56 {
        if choice.is_multiple_of(3) {
            M::Bladderwrack
        } else {
            M::GiantKelp
        }
    } else {
        [M::RedAlgae, M::BrownAlgae, M::GreenAlgae, M::RibbonKelp][variant % 4]
    }
}

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|i| i * TILE_SIZE);
    let center = [origin[0] + 16, origin[1] + 16];
    if h.terrain
        .climate_biome_at(center[0] as f64, center[1] as f64)
        .weights
        .ocean
        < 0.38
    {
        return;
    }
    let key = h.terrain.config().seed.0;
    let sea = h.terrain.config().sea_level;
    let feature = ticket(key, tile, 0xa8819);
    if feature.is_multiple_of(73)
        && h.terrain.config().structures_revision > 0
        && h.terrain.config().structures.ruins
        && !h.reserved(center, 12)
    {
        let col = h.column(center);
        let depth = sea - col.water_bed();
        if col.biome.weights.ocean > 0.64
            && col.water_level == Some(sea)
            && (9.0..76.0).contains(&depth)
            && col.slope < 0.8
        {
            let floor = col.water_bed() as i64;
            let fits = (-8..=8)
                .flat_map(|x| (-4..=4).map(move |z| [x, z]))
                .all(|[x, z]| {
                    let c = h.column([center[0] + x, center[1] + z]);
                    (c.water_bed() as i64 - floor).abs() <= 2
                        && c.water_level == Some(sea)
                        && c.river_cavity.is_none()
                        && !c.cave.open_to_sky()
                        && c.biome.weights.ocean > 0.45
                });
            if fits
                && !cells.keys().any(|p| {
                    (p[0] - center[0]).abs() <= 8
                        && (p[2] - center[1]).abs() <= 4
                        && (p[1] - floor).abs() <= 8
                })
            {
                let [x, z] = center;
                if feature & 1 == 0 {
                    wreck(cells, [x, floor, z]);
                } else {
                    arcade(cells, [x, floor, z]);
                }
            }
        }
    }
    if !feature.is_multiple_of(73) && feature.is_multiple_of(3) {
        let anchor = [
            origin[0] + 7 + i64::from((feature >> 5) % 18),
            origin[1] + 7 + i64::from((feature >> 13) % 18),
        ];
        if !h.reserved(anchor, 5) {
            reef_garden(h, anchor, feature, cells);
        }
    }
    for iz in 0..6 {
        for ix in 0..6 {
            let grid = [tile[0] * 6 + ix, tile[1] * 6 + iz];
            let roll = ticket(key, grid, 0x5ea090);
            let point = [
                origin[0] + ix * 5 + i64::from(roll % 5),
                origin[1] + iz * 5 + i64::from((roll >> 4) % 5),
            ];
            if h.reserved(point, 1) {
                continue;
            }
            let c = h.column(point);
            let floor = c.water_bed() as i64;
            let depth = sea - floor as f64;
            if c.water_level != Some(sea)
                || c.biome.weights.ocean < 0.53
                || c.seagrass_height > 0
                || c.river_cavity.is_some()
                || !(2.0..=88.).contains(&depth)
                || c.slope > 2.2
                || c.cave.open_to_sky()
                || cells.contains_key(&[point[0], floor, point[1]])
            {
                continue;
            }
            if depth > 48. && !roll.is_multiple_of(3) {
                continue;
            }
            let id = community(
                c.biome.temperature,
                c.ocean.colony,
                c.ocean.cover,
                roll >> 7,
            );
            if !h.terrain.in_prop_patch(point, id as u16) {
                continue;
            }
            let height = if id == M::GiantKelp {
                (2 + i64::from(roll % 5)).min(depth.floor() as i64 - 1)
            } else {
                1
            };
            for dy in 0..height {
                cells
                    .entry([point[0], floor + dy, point[1]])
                    .or_insert(id as u16);
            }
        }
    }
}

/// A crescent of living ledges follows the bed. Shallows, kelp forests and
/// deep cold water each have a different silhouette, with open swimming lanes.
fn reef_garden(h: &mut Habitat<'_>, [cx, cz]: Tile, roll: u32, cells: &mut BTreeMap<Cell, u16>) {
    let center = h.column([cx, cz]);
    let sea = h.terrain.config().sea_level;
    let Some(area) = center.ocean.area() else {
        return;
    };
    if area == OceanArea::Swamp
        || center.water_level != Some(sea)
        || !(5.0..=42.0).contains(&(sea - center.water_bed()))
        || center.ocean.cover < 0.20
    {
        return;
    }
    let stone = match area {
        OceanArea::ColdOcean => M::SeaBasalt,
        OceanArea::SeagrassForest => M::MarineLimestone,
        _ => M::ReefStone,
    };
    for (dx, dz, relief) in [
        (-3, -1, 0),
        (-2, -1, 1),
        (-2, 0, 2),
        (-1, -1, 2),
        (-1, 0, 3),
        (-1, 1, 1),
        (0, -2, 1),
        (0, -1, 2),
        (0, 1, 0),
        (1, -2, 2),
        (1, -1, 1),
        (2, -2, 1),
        (2, -1, 0),
        (3, -1, 0),
    ] {
        let dx = if roll & 8 == 0 { dx } else { -dx };
        let (x, z) = if roll & 16 == 0 {
            (cx + dx, cz + dz)
        } else {
            (cx + dz, cz + dx)
        };
        let c = h.column([x, z]);
        let bed = c.water_bed() as i64;
        let top = bed + relief;
        if c.ocean.area() != Some(area)
            || c.water_level != Some(sea)
            || (top + 2) as f64 >= sea
            || (c.water_bed() - center.water_bed()).abs() > 2.0
            || c.river_cavity.is_some()
            || c.cave.open_to_sky()
            || c.seagrass_height > 0
            || (bed..=top).any(|y| cells.contains_key(&[x, y, z]))
        {
            continue;
        }
        for y in bed..top {
            cells.insert([x, y, z], stone as u16);
        }
        // The crowns form small communities, sharing a palette and a support.
        let pick = ticket(u64::from(roll), [dx, dz], 922) as usize;
        let crown = match area {
            OceanArea::CoralReef => [
                M::SeaFan,
                M::StaghornCoral,
                M::MushroomCoral,
                M::TableCoral,
                M::OrganPipeCoral,
                M::CoralBush,
            ][pick % 6],
            OceanArea::ColdOcean if relief == 3 => M::BubbleVent,
            OceanArea::ColdOcean => [
                M::SeaCrystal,
                M::BarnacledBoulder,
                M::BrownAlgae,
                M::SeaUrchin,
            ][pick % 4],
            _ => [M::GiantKelp, M::RibbonKelp, M::Bladderwrack, M::ReefOyster][pick % 4],
        };
        let segments = if crown == M::GiantKelp {
            (2 + pick % 3) as i64
        } else {
            1
        };
        for y in top..(top + segments).min(sea.floor() as i64 - 1) {
            cells.insert([x, y, z], crown as u16);
        }
    }
}

// Local coordinates fit wholly within one tile; the submerged foundations reach two
// blocks below the surveyed bed, including across vertical section boundaries.
fn wreck(cells: &mut BTreeMap<Cell, u16>, [x, y, z]: Cell) {
    let mut put = |dx, dy, dz, m: M| {
        cells.insert([x + dx, y + dy, z + dz], m as u16);
    };
    for dx in -8i64..=8 {
        let beam = if dx.abs() > 6 {
            1
        } else if dx.abs() > 4 {
            2
        } else {
            3
        };
        put(dx, 0, 0, M::WeatheredWood);
        for side in [-1, 1] {
            for j in 1..=beam {
                if dx > 1 && side == 1 && dx % 3 != 0 {
                    continue;
                }
                let lift = i64::from(j == beam) + i64::from(dx.abs() > 6);
                put(dx, lift, side * j, M::OakPlanks);
                if dx % 3 == 0 {
                    put(dx, lift + 1, side * j, M::WeatheredWood);
                }
            }
        }
    }
    for dy in 1..=6 {
        put(-2, dy, 0, M::WeatheredWood);
    }
    for dz in -3..=2 {
        put(-2, 5, dz, M::OakFence);
    }
    for (dx, dy, dz, m) in [
        (-2, 4, -2, M::WoolSlab),
        (-2, 3, -1, M::FishingNet),
        (6, 1, -3, M::ShipAnchor),
        (3, 1, 1, M::BarnacledBarrel),
        (5, 1, 3, M::WreckTimbers),
        (-5, 1, -1, M::LobsterPot),
        (-6, 1, 0, M::MarineDriftwood),
    ] {
        put(dx, dy, dz, m);
    }
}

fn arcade(cells: &mut BTreeMap<Cell, u16>, [x, y, z]: Cell) {
    for dx in -6i64..=6 {
        for dz in -3i64..=3 {
            if dx.abs() == 6 || dz.abs() == 3 || (dx + dz).rem_euclid(4) == 0 {
                for dy in -2..=0 {
                    cells.insert([x + dx, y + dy, z + dz], M::MarineLimestone as u16);
                }
            }
        }
    }
    for dx in [-5, 0, 5] {
        for dz in [-3, 3] {
            let height = if dx == 5 && dz == 3 { 2 } else { 5 };
            for dy in 1..=height {
                let m = if dy == 1 || dy == height {
                    M::CarvedSunstone
                } else if dy == 3 {
                    M::MossyStone
                } else {
                    M::MarineLimestone
                };
                cells.insert([x + dx, y + dy, z + dz], m as u16);
            }
        }
    }
    for dx in -4i64..5 {
        for dz in [-3, 3] {
            if dz == 3 && dx > 0 {
                continue;
            }
            let dy = if dx.rem_euclid(5) == 2 || dx.rem_euclid(5) == 3 {
                7
            } else {
                6
            };
            cells.insert([x + dx, y + dy, z + dz], M::ReefStone as u16);
            if dy == 6 {
                cells.insert([x + dx, y + 5, z + dz], M::MarineLimestone as u16);
            }
        }
        if dx == -1 || dx == 1 {
            cells.insert([x + dx, y + 1, z], M::LuminousCoral as u16);
        }
    }
    cells.insert([x, y + 1, z], M::SeaCrystal as u16);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marine_props_roundtrip_through_real_generated_sections_in_all_three_climates() {
        use crate::generation::{SectionGenerator, TerraGenerator};
        use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};
        let config = WorldConfig {
            seed: WorldSeed(42042),
            ecology_revision: 15,
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        };
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 14,
            ..config.clone()
        })
        .unwrap();
        let terrain = TerraGenerator::new(config).unwrap();
        let sections = SectionGenerator::new(terrain.clone());
        for (x, z) in [(-128i64, -128i64), (1024, 1024), (1024, 1408)] {
            let tile = [x.div_euclid(TILE_SIZE), z.div_euclid(TILE_SIZE)];
            let patch = terrain.natural_patch(tile);
            let (p, m) = patch
                .cells
                .iter()
                .find(|(_, m)| (545..=580).contains(m))
                .expect("marine habitat");
            let (owner, local) = terra_voxel::global_to_section(*p);
            assert_eq!(sections.block_at(*p).unwrap(), *m);
            assert_eq!(sections.generate(owner).unwrap().block(local).unwrap(), *m);
            assert!(
                old.natural_patch(tile)
                    .cells
                    .iter()
                    .all(|(_, m)| !(541..=580).contains(m))
            );
        }
    }
    #[test]
    fn sea_surface_stays_clear_at_reported_coordinates_and_former_coastal_landmarks() {
        use crate::generation::{TerraGenerator, biomes::BiomeKind};
        use terra_core::{WorldConfig, WorldSeed};
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(7863640744109448198),
            rivers: true,
            ..Default::default()
        })
        .unwrap();
        let sea = terrain.config().sea_level;
        let mut wet = 0;
        for z in (5979..=6235).step_by(8) {
            for x in (-4579..=-4323).step_by(8) {
                let c = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
                if c.biome_kind(sea) == BiomeKind::Ocean {
                    wet += 1;
                    assert_eq!(c.ice_bottom(), None);
                }
            }
        }
        assert!(wet > 800);
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            // The original landmarks predate shells and driftwood on dry beaches.
            ecology_revision: 21,
            ..Default::default()
        })
        .unwrap();
        for [x, z] in [
            [-774_i64, -495],
            [1392, 1972],
            [2734, 1844],
            [848, -2608],
            [784, -432],
        ] {
            let patch = terrain.natural_patch([x.div_euclid(TILE_SIZE), z.div_euclid(TILE_SIZE)]);
            for &(p, id) in &patch.cells {
                if matches!(id, 545..=580) {
                    let c = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
                    assert_eq!(c.biome_kind(sea), BiomeKind::Ocean);
                    assert!(((p[1] + 1) as f64) < sea);
                }
                assert_ne!(id, M::MarkerBuoy as u16);
            }
        }
    }

    #[test]
    fn misty_coast_keeps_the_surface_clear_and_grounds_wrack_on_the_dry_beach() {
        use crate::generation::{SectionGenerator, TerraGenerator};
        use terra_core::{WorldConfig, WorldSeed};
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(42042),
            ..Default::default()
        })
        .unwrap();
        let sections = SectionGenerator::new(terrain.clone());
        let sea = terrain.config().sea_level;
        // This one-cell-deep shelf used to place the tip of ribbon kelp on the waterline.
        let shallow = [-770, 7, -485];
        let tile = [shallow[0], shallow[2]].map(|n: i64| n.div_euclid(TILE_SIZE));
        assert_eq!(sea, (shallow[1] + 1) as f64);
        assert_eq!(terrain.natural_patch(tile).block_at(shallow), None);

        for (p, material, submerged) in [
            ([-798, 4, -607], M::GreenAlgae, true),
            ([-795, 5, -583], M::Bladderwrack, true),
            ([-813, 10, -539], M::MarineDriftwood, false),
            ([-818, 11, -565], M::ScallopShell, false),
            ([-797, 8, -429], M::ClamShell, false),
            ([-859, 46, -566], M::SeaThrift, false),
        ] {
            let column = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
            assert_eq!(p[1] as f64, column.water_bed());
            if submerged {
                assert!(
                    column
                        .water_level
                        .is_some_and(|level| ((p[1] + 1) as f64) < level)
                );
            } else {
                assert!(column.water_level.is_none_or(|level| level <= p[1] as f64));
            }
            assert_eq!(sections.block_at(p).unwrap(), material as u16);
        }
    }

    #[test]
    fn coral_gardens_and_wrecks_have_distinct_physical_silhouettes() {
        let seen = |temp| {
            (0..10000)
                .map(|i| community(temp, 0.85, 0.72, i) as u16)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let warm = seen(0.85);
        let cold = seen(0.18);
        assert!((545..=552).all(|id| warm.contains(&id) && !cold.contains(&id)));
        assert!(seen(0.49).contains(&(M::GiantKelp as u16)));
        for (builder, feature) in [
            (wreck as fn(&mut BTreeMap<Cell, u16>, Cell), M::ShipAnchor),
            (arcade, M::SeaCrystal),
        ] {
            let mut cells = BTreeMap::new();
            builder(&mut cells, [-16, 14, -16]);
            assert!(cells.len() > 70 && cells.values().any(|m| *m == feature as u16));
            assert!(
                cells
                    .keys()
                    .all(|p| p[0].div_euclid(32) == -1 && p[2].div_euclid(32) == -1)
            );
        }
    }
}
