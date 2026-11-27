use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|i| i * TILE_SIZE);
    if h.terrain
        .climate_biome_at((origin[0] + 16) as f64, (origin[1] + 16) as f64)
        .aridity()
        < 0.35
    {
        return;
    }
    for dz in 0..4 {
        for dx in 0..4 {
            let grid = [tile[0] * 4 + dx, tile[1] * 4 + dz];
            let seed = ticket(h.terrain.config().seed.0, grid, 3072);
            let p = [
                grid[0] * 8 + i64::from(seed & 7),
                grid[1] * 8 + i64::from((seed >> 3) & 7),
            ];
            if h.reserved(p, 2) {
                continue;
            }
            let c = h.column(p);
            if c.biome.aridity() < 0.50 {
                continue;
            }
            let lush = c.oasis_moisture > 0.35;
            if let Some(level) = c.water_level {
                if lush && (0.75..2.5).contains(&(level - c.height)) && seed.is_multiple_of(5) {
                    cells
                        .entry([p[0], level.ceil() as i64, p[1]])
                        .or_insert(M::LilyPad as u16);
                }
                continue;
            }
            if c.slope > 1.0 || (!lush && seed % 5 > 1) {
                continue;
            }
            let Some((ground, _)) = h.ground(p) else {
                continue;
            };
            let cell = [p[0], ground + 1, p[1]];
            if h.tree_occupied(cell) || cells.contains_key(&[p[0], ground, p[1]]) {
                continue;
            }
            let species = match ((seed >> 12) % 16, lush) {
                (0..=5, true) => M::ShoreReeds,
                (6..=9, true) => M::FloweringYucca,
                (0, false) => M::GraveSkull,
                (1, false) => M::BloodBones,
                (2..=4, false) => M::CavePebbles,
                (10..=12, _) => M::PricklyPear,
                (13..=14, _) => M::FloweringYucca,
                _ => M::DesertAgave,
            };
            if !h.terrain.in_prop_patch(p, species as u16) {
                continue;
            }
            cells.entry(cell).or_insert(species as u16);
        }
    }
}
