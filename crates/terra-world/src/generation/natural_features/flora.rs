use super::*;
use noise::NoiseFn;

fn plantable(material: MaterialId) -> bool {
    matches!(
        material,
        MaterialId::Grass
            | MaterialId::Dirt
            | MaterialId::BloodGrass
            | MaterialId::BloodDirt
            | MaterialId::CoagulatedMud
            | MaterialId::BlightedRootSoil
            | MaterialId::BloodMoss
            | MaterialId::CrimsonMycelium
    )
}

/// Jitter stays inside an inset 8m grid cell. Clumps have open space between
/// them; large landmarks use a separate 16m grid with at least 12m separation.
pub(super) fn build(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    if habitat.terrain.config().blood_forest_revision == 0 {
        return;
    }
    let modern = habitat.terrain.config().ecology_revision >= 12;
    for gz in 0..4 {
        for gx in 0..4 {
            let grid = [tile[0] * 4 + gx, tile[1] * 4 + gz];
            let seed = ticket(habitat.terrain.config().seed.0, grid, 1510);
            if !seed.is_multiple_of(if modern { 2 } else { 3 }) {
                continue;
            }
            let x_jitter = if modern { seed >> 13 } else { seed };
            let anchor = [
                grid[0] * 8 + 3 + i64::from(x_jitter % 2),
                grid[1] * 8 + 3 + i64::from((seed >> 8) % 2),
            ];
            let clearing = habitat.terrain.hills.get([
                anchor[0] as f64 * 0.023 + 47.2,
                anchor[1] as f64 * 0.023 - 81.8,
            ]);
            if clearing < if modern { -0.36 } else { -0.12 } {
                continue;
            }
            let Some((_, column)) = habitat.ground(anchor) else {
                continue;
            };
            if !plantable(column.surface_material)
                || column.biome.weights.blood_forest < 0.35
                || column.slope > 0.8
                || habitat.reserved(anchor, if modern { 2 } else { 5 })
            {
                continue;
            }
            if f64::from(seed >> 16) / 65536.0
                > super::super::biomes::smoothstep(0.3, 0.85, column.biome.weights.blood_forest)
            {
                continue;
            }
            let near_root = habitat.root_distance_squared(anchor);
            let material = if modern && seed.is_multiple_of(19) {
                MaterialId::FallenBloodBranches
            } else if (9..=81).contains(&near_root) && seed % 5 < 2 {
                MaterialId::BloodMushrooms
            } else if (9..=36).contains(&near_root) {
                MaterialId::BloodRoots
            } else {
                match (if modern { seed >> 3 } else { seed }) % 4 {
                    0 => MaterialId::BloodThornBush,
                    1 => MaterialId::BloodFlower,
                    _ => MaterialId::BloodFern,
                }
            };
            if !habitat.terrain.in_prop_patch(anchor, material as u16) {
                continue;
            }
            // One botanical species per group, irregularly feathered at its edge.
            for [dx, dz] in [[0, 0], [1, 0], [-1, 1], [0, -1], [1, 2], [-2, -1]] {
                if (dx != 0 || dz != 0) && ticket(u64::from(seed), [dx, dz], 1511).is_multiple_of(4)
                {
                    continue;
                }
                let point = [anchor[0] + dx, anchor[1] + dz];
                let Some((ground, c)) = habitat.ground(point) else {
                    continue;
                };
                let cell = [point[0], ground + 1, point[1]];
                if !plantable(c.surface_material)
                    || c.slope > 0.9
                    || habitat.root_distance_squared(point) < 9
                    || habitat.tree_occupied(cell)
                    || cells.contains_key(&cell)
                    || cells.contains_key(&[cell[0], cell[1] - 1, cell[2]])
                    || habitat
                        .terrain
                        .cave_density_with_column(cell.map(|v| v as f64 + 0.5), c)
                        .0
                        <= 0.0
                {
                    continue;
                }
                cells.insert(cell, material as u16);
            }
        }
    }
    for gz in 0..2 {
        for gx in 0..2 {
            let grid = [tile[0] * 2 + gx, tile[1] * 2 + gz];
            let seed = ticket(habitat.terrain.config().seed.0, grid, 1520);
            if !seed.is_multiple_of(7) {
                continue;
            }
            let point = [
                grid[0] * 16 + 6 + i64::from(seed % 5),
                grid[1] * 16 + 6 + i64::from((seed >> 8) % 5),
            ];
            if habitat.reserved(point, 7) || habitat.root_distance_squared(point) < 25 {
                continue;
            }
            let Some((ground, c)) = habitat.ground(point) else {
                continue;
            };
            if c.biome.weights.blood_forest < 0.7 || c.slope > 0.45 {
                continue;
            }
            let material = match (seed >> 4)
                % if habitat.terrain.config().ecology_revision >= 12 {
                    6
                } else {
                    3
                } {
                0 => MaterialId::BloodRuinedArch,
                1 => MaterialId::BloodCrystalPillar,
                3 => MaterialId::BloodCrystalGrowth,
                4 => MaterialId::WeatheredRibcage,
                5 => MaterialId::BoneStake,
                _ => MaterialId::GiantBloodMushroom,
            };
            let cell = [point[0], ground + 1, point[1]];
            if !habitat.tree_occupied(cell)
                && habitat
                    .terrain
                    .cave_density_with_column(cell.map(|v| v as f64 + 0.5), c)
                    .0
                    > 0.0
                && (-2..=2).all(|dz| {
                    (-2..=2).all(|dx| {
                        !cells
                            .keys()
                            .any(|p| p[0] == point[0] + dx && p[2] == point[1] + dz)
                    })
                })
            {
                cells.insert(cell, material as u16);
            }
        }
    }
}
