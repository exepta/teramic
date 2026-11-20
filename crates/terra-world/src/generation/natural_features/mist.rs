//! Ecology-eight Spirit Forest fog stones. One inset candidate per 32m tile
//! leaves at least 17m between sources; old patches never gain new cells.
use super::*;

pub(super) fn build(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    if habitat.terrain.config().ecology_revision < 8
        || habitat.terrain.config().spirit_blossom_revision == 0
    {
        return;
    }
    let seed = ticket(habitat.terrain.config().seed.0, tile, 1680);
    if !seed.is_multiple_of(3) {
        return;
    }
    let point = [
        tile[0] * TILE_SIZE + 8 + i64::from((seed >> 4) % 16),
        tile[1] * TILE_SIZE + 8 + i64::from((seed >> 12) % 16),
    ];
    if habitat.reserved(point, 10) || habitat.root_distance_squared(point) < 25 {
        return;
    }
    let Some((ground, column)) = habitat.ground(point) else {
        return;
    };
    if column.biome.weights.spirit_blossom < 0.72
        || column.surface_material != MaterialId::SpiritBlossomGrass
        || column.slope > 0.8
        || column.water_level.is_some()
    {
        return;
    }
    // Most are mineable discoveries directly below the grass cap. Occasional
    // exposed sources make the origin of a fog patch discoverable from above.
    let exposed = (seed >> 20).is_multiple_of(7);
    let cell = [point[0], ground - i64::from(!exposed), point[1]];
    if (ground - 1..=ground + 1).any(|y| {
        let p = [point[0], y, point[1]];
        cells.contains_key(&p) || habitat.tree_occupied(p)
    }) {
        return;
    }
    cells.insert(cell, MaterialId::FogBlock as u16);
}

#[cfg(test)]
mod tests;
