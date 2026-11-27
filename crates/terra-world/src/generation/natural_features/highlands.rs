//! Wind-scoured rock on the high ridges, winter cover and ice in sheltered valleys.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(h: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let start = tile.map(|n| n * TILE_SIZE);
    if h.terrain
        .climate_biome_at((start[0] + 16) as f64, (start[1] + 16) as f64)
        .weights
        .highland
        < 0.28
    {
        return;
    }
    for dz in 0..6 {
        for dx in 0..6 {
            let seed = ticket(
                h.terrain.config().seed.0,
                [tile[0] * 6 + dx, tile[1] * 6 + dz],
                7120,
            );
            let p = [
                start[0] + dx * 5 + i64::from(seed % 5),
                start[1] + dz * 5 + i64::from((seed >> 8) % 5),
            ];
            let c = h.column(p);
            if c.biome.weights.highland < 0.5 || h.reserved(p, 1) {
                continue;
            }
            let Some((y, _)) = h.ground(p) else { continue };
            let cell = [p[0], y + 1, p[1]];
            if h.tree_occupied(cell) || cells.contains_key(&[p[0], y, p[1]]) {
                continue;
            }
            let scenery = if c.height > 178.0 || c.slope > 0.85 {
                if seed.is_multiple_of(3) {
                    M::MoraineRocks
                } else {
                    M::SnowMound
                }
            } else {
                match (seed >> 16) % 18 {
                    0..=1 => M::AlpineConifer,
                    2 => M::DeadWinterBush,
                    3..=4 => M::FrozenShrub,
                    5..=7 => M::MoraineRocks,
                    8..=9 => M::SnowMound,
                    10..=11 if h.root_distance_squared(p) < 144 => M::PineCones,
                    12 => M::MountainFlowerRemnants,
                    _ => M::FrostedTussock,
                }
            };
            if !h.terrain.in_prop_patch(p, scenery as u16) {
                continue;
            }
            cells.entry(cell).or_insert(scenery as u16);
            if seed.is_multiple_of(4) && h.root_distance_squared(p) < 144 {
                // The icicle roots attach to a real overhanging branch in the same column.
                if let Some(ceiling) =
                    (y + 4..=y + 21).find(|&yy| h.tree_occupied([p[0], yy, p[1]]))
                {
                    cells
                        .entry([p[0], ceiling - 1, p[1]])
                        .or_insert(M::AlpineIcicles as u16);
                }
            }
        }
    }
}
