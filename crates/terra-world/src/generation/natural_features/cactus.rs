use super::*;
use noise::NoiseFn;

/// Each plant remains within its owner's 32m tile. Adjacent 16m cells leave
/// at least 9m between stems; arms never overlap the next cactus or patch.
pub(super) fn build(
    habitat: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
) -> Vec<CactusPlant> {
    let mut plants = Vec::new();
    for gz in 0..2 {
        for gx in 0..2 {
            let grid = [tile[0] * 2 + gx, tile[1] * 2 + gz];
            let seed = ticket(habitat.terrain.config().seed.0, grid, 1620);
            if seed % 5 > 1 {
                continue;
            }
            let point = [
                grid[0] * 16 + 4 + i64::from(seed % 7),
                grid[1] * 16 + 4 + i64::from((seed >> 8) % 7),
            ];
            let Some((ground, column)) = habitat.ground(point) else {
                continue;
            };
            let dry = column.biome.weights.desert + column.biome.weights.cliffted_desert;
            if column.surface_material != MaterialId::Sand
                || column.biome.temperature < 0.50
                || column.oasis_moisture > 0.22
                || column.slope > 0.65
                || f64::from(seed >> 16) / 65536.0
                    > super::super::biomes::smoothstep(0.35, 0.85, dry)
                || habitat.reserved(point, 5)
                || habitat.root_distance_squared(point) < 49
                || habitat
                    .terrain
                    .hills
                    .get([point[0] as f64 * 0.018, point[1] as f64 * 0.018])
                    < -0.18
            {
                continue;
            }
            let origin = [point[0], ground + 1, point[1]];
            let height = (2 + (seed >> 12) % 7) as u8;
            let plant = shape(origin, height, seed);
            if plant.iter().any(|&cell| {
                let c = habitat.column([cell[0], cell[2]]);
                cells.contains_key(&cell)
                    || habitat.tree_occupied(cell)
                    || habitat
                        .terrain
                        .cave_density_with_column(cell.map(|v| v as f64 + 0.5), c)
                        .0
                        <= 0.0
                    || c.water_level.is_some_and(|level| level > cell[1] as f64)
            }) {
                continue;
            }
            let bounds = [
                std::array::from_fn(|a| plant.iter().map(|p| p[a]).min().unwrap_or(origin[a])),
                std::array::from_fn(|a| plant.iter().map(|p| p[a]).max().unwrap_or(origin[a])),
            ];
            plants.push(CactusPlant {
                origin,
                height,
                bounds,
                voxel_count: plant.len(),
            });
            for cell in plant {
                cells.insert(cell, MaterialId::Cactus as u16);
            }
        }
    }
    plants
}

/// Axis-aligned segments make every branch face-connected, including at
/// negative section boundaries. Arms terminate below the main stem's crown.
pub(super) fn shape(origin: Cell, height: u8, seed: u32) -> Vec<Cell> {
    let height = i64::from(height.clamp(2, 8));
    let mut cells: Vec<_> = (0..height)
        .map(|dy| [origin[0], origin[1] + dy, origin[2]])
        .collect();
    if height >= 4 && seed & 1 != 0 {
        let direction = [[1, 0], [0, 1], [-1, 0], [0, -1]][((seed >> 5) % 4) as usize];
        for arm in 0..1 + i64::from(height >= 6 && seed & 2 != 0) {
            let sign = if arm == 0 { 1 } else { -1 };
            let y = height / 2 - arm;
            let reach = 1 + i64::from(seed & 4 != 0);
            for d in 1..=reach {
                cells.push([
                    origin[0] + direction[0] * d * sign,
                    origin[1] + y,
                    origin[2] + direction[1] * d * sign,
                ]);
            }
            for dy in y + 1..height - 1 - arm {
                cells.push([
                    origin[0] + direction[0] * reach * sign,
                    origin[1] + dy,
                    origin[2] + direction[1] * reach * sign,
                ]);
            }
        }
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}
