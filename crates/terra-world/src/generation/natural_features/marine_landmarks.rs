//! Remains of a beacon on the seafloor. Every footing is surveyed before placement.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use crate::generation::biomes::BiomeKind;
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let config = h.terrain.config();
    if !config.structures.ruins || config.structures_revision == 0 {
        return;
    }
    let roll = ticket(config.seed.0, tile, 3921);
    if !roll.is_multiple_of(83) {
        return;
    }
    let sea = config.sea_level;
    let [x, z] = tile.map(|v| v * TILE_SIZE + TILE_SIZE / 2);
    let center = h.column([x, z]);
    let floor = center.water_bed() as i64;
    if center.biome_kind(sea) != BiomeKind::Ocean
        || !(10.0..=56.0).contains(&(sea - floor as f64))
        || h.reserved([x, z], 5)
        || cells
            .range([x - 4, i64::MIN, i64::MIN]..=[x + 4, i64::MAX, i64::MAX])
            .any(|(p, _)| (p[2] - z).abs() <= 4 && (p[1] - floor).abs() <= 8)
    {
        return;
    }

    let mut bed = Vec::new();
    for dx in -3_i64..=3 {
        for dz in -3_i64..=3 {
            if dx.abs() + dz.abs() > 5 {
                continue;
            }
            let c = h.column([x + dx, z + dz]);
            let support = c.water_bed() as i64;
            if c.biome_kind(sea) != BiomeKind::Ocean
                || (support - floor).abs() > 2
                || c.river_cavity.is_some()
                || c.cave.open_to_sky()
            {
                return;
            }
            bed.push(([x + dx, z + dz], support));
        }
    }
    let deck = floor + 2;
    for ([px, pz], support) in bed {
        for y in support..=deck {
            let material = if y == deck && (px + pz).rem_euclid(4) == 0 {
                M::MossyStone
            } else {
                M::MarineLimestone
            };
            cells.insert([px, y, pz], material as u16);
        }
    }
    // A collapsed corner opens the chamber to swimmers. The surviving lintels
    // frame a small luminous coral visible through the stonework.
    for (dx, dz, height) in [(-2, -2, 4), (2, -2, 4), (-2, 2, 4), (2, 2, 1)] {
        for dy in 1..=height {
            let material = if dy == 1 || dy == 4 {
                M::CarvedSunstone
            } else {
                M::ReefStone
            };
            cells.insert([x + dx, deck + dy, z + dz], material as u16);
        }
    }
    for t in -2..=2 {
        cells.insert([x + t, deck + 5, z - 2], M::MarineLimestone as u16);
        cells.insert([x - 2, deck + 5, z + t], M::MarineLimestone as u16);
    }
    cells.insert([x, deck + 1, z], M::ReefStone as u16);
    cells.insert([x, deck + 2, z], M::LuminousCoral as u16);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::TerraGenerator;
    use terra_core::WorldConfig;

    #[test]
    fn ruined_beacon_rises_from_the_floor_and_obeys_the_ruins_setting() {
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let [x, z] = [752, -1968];
        let floor = terrain
            .surface_column(x as f64 + 0.5, z as f64 + 0.5)
            .water_bed() as i64;
        let tile = [23, -62];
        let beacon = terrain.natural_patch(tile);
        assert_eq!(
            beacon.block_at([x, floor + 4, z]),
            Some(M::LuminousCoral as u16)
        );
        assert_eq!(
            beacon.block_at([x - 2, floor + 7, z]),
            Some(M::MarineLimestone as u16)
        );
        assert!((floor + 8) as f64 <= terrain.config().sea_level - 2.0);
        let mut disabled = terrain.config().clone();
        disabled.structures.ruins = false;
        let bare = TerraGenerator::new(disabled).unwrap().natural_patch(tile);
        assert_ne!(
            bare.block_at([x - 2, floor + 7, z]),
            Some(M::MarineLimestone as u16)
        );
    }
}
