//! Botanic groups follow the same continuous glade/path fields as grass.
//! One species per small patch, separated by open ground rather than a uniform
//! mixture of individually hashed plants. All geometry stays inside its tile.
use super::super::biomes::BiomeWeights;
use super::*;

#[derive(Clone, Copy)]
enum Palette {
    Meadow,
    Woodland,
    Cherry,
    Alpine,
    Spirit,
    Desert,
    Coast,
    Blood,
}

fn palette(weights: BiomeWeights, seed: u32) -> Palette {
    let choices = [
        (weights.blood_forest, Palette::Blood),
        (weights.spirit_blossom, Palette::Spirit),
        (weights.cherry_forest, Palette::Cherry),
        (weights.highland, Palette::Alpine),
        (weights.forest + weights.mushroom_park, Palette::Woodland),
        (weights.desert + weights.cliffted_desert, Palette::Desert),
        (weights.ocean, Palette::Coast),
        (weights.plains, Palette::Meadow),
    ];
    let mut choice = f64::from(seed) / (f64::from(u32::MAX) + 1.0);
    for (weight, kind) in choices {
        choice -= weight;
        if choice < 0.0 {
            return kind;
        }
    }
    Palette::Meadow
}

fn near_water(habitat: &mut Habitat<'_>, point: Tile, ground: i64) -> bool {
    if !habitat.terrain.config().water {
        return false;
    }
    [[-3, 0], [3, 0], [0, -3], [0, 3]]
        .into_iter()
        .any(|[dx, dz]| {
            let c = habitat.column([point[0] + dx, point[1] + dz]);
            c.water_level
                .is_some_and(|level| level >= c.height && (ground as f64 + 1.0 - level).abs() < 3.0)
        })
}

fn supports(material: MaterialId, plant: MaterialId) -> bool {
    match plant {
        MaterialId::DesertShrub => material == MaterialId::Sand,
        MaterialId::AlpineHeather => matches!(
            material,
            MaterialId::Grass | MaterialId::Dirt | MaterialId::Snow
        ),
        MaterialId::ShoreReeds => matches!(
            material,
            MaterialId::Grass | MaterialId::Dirt | MaterialId::Sand | MaterialId::Mud
        ),
        MaterialId::GlowingSpiritFlowers
        | MaterialId::BlossomBush
        | MaterialId::PetalGroundCover
        | MaterialId::SpiritLanternPost
        | MaterialId::SpiritShrineStone => matches!(
            material,
            MaterialId::SpiritBlossomGrass | MaterialId::Grass | MaterialId::SpiritBlossomDirt
        ),
        _ => matches!(material, MaterialId::Grass | MaterialId::Dirt),
    }
}

pub(super) fn build(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    for gz in 0..4 {
        for gx in 0..4 {
            let grid = [tile[0] * 4 + gx, tile[1] * 4 + gz];
            let seed = ticket(habitat.terrain.config().seed.0, grid, 1610);
            let point = [
                grid[0] * 8 + 2 + i64::from(seed % 4),
                grid[1] * 8 + 2 + i64::from((seed >> 8) % 4),
            ];
            let Some((ground, column)) = habitat.ground(point) else {
                continue;
            };
            let (_, path) = habitat
                .terrain
                .climate
                .ground_masks(point[0] as f64 + 0.5, point[1] as f64 + 0.5);
            let [glade, meadow, shrubs, _] = habitat
                .terrain
                .climate
                .vegetation_masks(point[0] as f64 + 0.5, point[1] as f64 + 0.5);
            let kind = palette(column.biome.weights, ticket(u64::from(seed), grid, 1612));
            let density = match kind {
                Palette::Blood => continue, // Existing revision-five Blood groups own this habitat.
                Palette::Desert => 0.16 + column.oasis_moisture * 0.24,
                Palette::Alpine => 0.36,
                Palette::Woodland => 0.42 + shrubs * 0.30,
                Palette::Spirit | Palette::Cherry => 0.48 + meadow * 0.24,
                Palette::Meadow | Palette::Coast => 0.30 + meadow * 0.40,
            };
            if f64::from(seed >> 16) / 65536.0 >= density * path * glade
                || path < 0.32
                || glade < 0.28
                || column.slope > 0.85
                || habitat.reserved(point, 4)
            {
                continue;
            }
            let water = near_water(habitat, point, ground);
            let plant = if water && seed.is_multiple_of(3) && column.biome.temperature > 0.25 {
                MaterialId::ShoreReeds
            } else {
                match kind {
                    Palette::Meadow | Palette::Coast => match seed % 5 {
                        0 => MaterialId::FloweringShrub,
                        1 => MaterialId::Lupine,
                        _ => MaterialId::MeadowDaisies,
                    },
                    Palette::Woodland => {
                        if habitat.root_distance_squared(point) < 225 || seed & 1 == 0 {
                            MaterialId::ForestFern
                        } else {
                            MaterialId::FloweringShrub
                        }
                    }
                    Palette::Cherry => match seed % 4 {
                        0 => MaterialId::Lupine,
                        1 => MaterialId::MeadowDaisies,
                        _ => MaterialId::FloweringShrub,
                    },
                    Palette::Alpine => MaterialId::AlpineHeather,
                    Palette::Desert => MaterialId::DesertShrub,
                    Palette::Spirit => match seed % 5 {
                        0 => MaterialId::BlossomBush,
                        1 => MaterialId::PetalGroundCover,
                        _ => MaterialId::GlowingSpiritFlowers,
                    },
                    Palette::Blood => continue,
                }
            };
            if !supports(column.surface_material, plant) {
                continue;
            }
            let offsets: Vec<_> = if habitat.terrain.config().ecology_revision >= 9 {
                // Curled ribbons and incomplete fairy rings replace the same
                // eight-cell stamp; rotation and handedness differ per clump.
                let turn = (seed >> 10) % 4;
                let curl = if seed & 1 == 0 {
                    &[
                        [0, 0],
                        [1, 0],
                        [2, 1],
                        [2, 2],
                        [1, 3],
                        [0, 3],
                        [-1, 2],
                        [-2, 1],
                        [-2, 0],
                        [-1, -1],
                        [0, -2],
                        [1, -2],
                    ][..]
                } else {
                    &[
                        [0, 0],
                        [1, 0],
                        [2, 1],
                        [2, 2],
                        [1, 3],
                        [0, 3],
                        [-1, 2],
                        [-1, 1],
                        [-2, 0],
                        [-3, 0],
                        [-3, -1],
                        [-2, -2],
                    ][..]
                };
                curl.iter()
                    .map(|&[x, z]| match turn {
                        0 => [x, z],
                        1 => [-z, x],
                        2 => [-x, -z],
                        _ => [z, -x],
                    })
                    .collect()
            } else {
                vec![
                    [0, 0],
                    [1, 0],
                    [-1, 1],
                    [0, -1],
                    [1, 2],
                    [-2, -1],
                    [2, 1],
                    [-1, -2],
                ]
            };
            for [dx, dz] in offsets {
                if (dx != 0 || dz != 0) && ticket(u64::from(seed), [dx, dz], 1611).is_multiple_of(3)
                {
                    continue;
                }
                let p = [point[0] + dx, point[1] + dz];
                if habitat.terrain.config().ecology_revision >= 9
                    && [p[0].div_euclid(32), p[1].div_euclid(32)] != tile
                {
                    continue;
                }
                let Some((y, c)) = habitat.ground(p) else {
                    continue;
                };
                if !supports(c.surface_material, plant)
                    || c.slope > 0.95
                    || (y - ground).abs() > 1
                    || habitat.reserved(p, 1)
                {
                    continue;
                }
                insert(habitat, [p[0], y + 1, p[1]], c, plant, cells);
            }
        }
    }
    spirit_landmark(habitat, tile, cells);
}

fn insert(
    habitat: &Habitat<'_>,
    cell: Cell,
    column: SurfaceColumn,
    material: MaterialId,
    cells: &mut BTreeMap<Cell, u16>,
) {
    if habitat.root_distance_squared([cell[0], cell[2]]) < 9
        || habitat.tree_occupied(cell)
        || cells.contains_key(&cell)
        || cells.contains_key(&[cell[0], cell[1] - 1, cell[2]])
        || habitat
            .terrain
            .cave_density_with_column(cell.map(|v| v as f64 + 0.5), column)
            .0
            <= 0.0
    {
        return;
    }
    cells.insert(cell, material as u16);
}

fn spirit_landmark(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let seed = ticket(habitat.terrain.config().seed.0, tile, 1619);
    if !seed.is_multiple_of(13) {
        return;
    }
    let p = [
        tile[0] * 32 + 8 + i64::from(seed % 16),
        tile[1] * 32 + 8 + i64::from((seed >> 8) % 16),
    ];
    let Some((y, c)) = habitat.ground(p) else {
        return;
    };
    if c.biome.weights.spirit_blossom < 0.75
        || habitat.reserved(p, 6)
        || habitat.root_distance_squared(p) < 36
        || c.slope > 0.35
    {
        return;
    }
    let plant = if seed & 1 == 0 {
        MaterialId::SpiritShrineStone
    } else {
        MaterialId::SpiritLanternPost
    };
    if supports(c.surface_material, plant) {
        insert(habitat, [p[0], y + 1, p[1]], c, plant, cells);
    }
}
