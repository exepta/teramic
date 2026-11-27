//! Tufted grassland, termite fields and the remains of sandstone gateways.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

pub(super) fn build(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let origin = tile.map(|v| v * TILE_SIZE);
    let seed = habitat.terrain.config().seed.0;
    if habitat
        .terrain
        .climate_biome_at((origin[0] + 16) as f64, (origin[1] + 16) as f64)
        .weights
        .savanna
        < 0.25
    {
        return;
    }
    let site = ticket(seed, tile, 0xacac1a);
    if site.is_multiple_of(19) {
        let center = [
            origin[0] + 12 + i64::from(site % 9),
            origin[1] + 12 + i64::from(site.rotate_left(13) % 9),
        ];
        gateway(habitat, center, site, cells);
    }
    for gz in 0..8 {
        for gx in 0..8 {
            let key = ticket(seed, [tile[0] * 8 + gx, tile[1] * 8 + gz], 0xba0bab);
            let point = [
                origin[0] + gx * 4 + i64::from(key % 4),
                origin[1] + gz * 4 + i64::from((key >> 5) % 4),
            ];
            let kind = match (key >> 9) % 100 {
                48..=50 => M::SavannaThornBush,
                51 => M::AcaciaSapling,
                52 => M::SavannaAloe,
                53 => M::SavannaAgave,
                54..=55 => M::YellowSavannaFlower,
                56 => M::RedSavannaFlower,
                57 => M::SavannaSucculent,
                58 => M::SmallSavannaCactus,
                59 => M::FloweringSavannaCactus,
                60..=61 => M::SmallSavannaBoulder,
                62 => M::LargeSavannaBoulder,
                63 => M::SavannaRockPile,
                64 if (key >> 23).is_multiple_of(8) => M::SavannaRibs,
                65 => M::SmallTermiteMound,
                66 if (key >> 23).is_multiple_of(4) => M::TallTermiteMound,
                67 if (key >> 23).is_multiple_of(6) => M::DryAcaciaSnag,
                68 if (key >> 23).is_multiple_of(4) => M::WeatheredAcaciaStump,
                69 if (key >> 23).is_multiple_of(8) => M::SavannaStoneArch,
                70 => M::SavannaPlateauStone,
                71 if (key >> 23).is_multiple_of(8) => M::HornedSkull,
                _ => continue,
            };
            let Some((y, column)) = habitat.ground(point) else {
                continue;
            };
            let cell = [point[0], y + 1, point[1]];
            if column.biome.weights.savanna < 0.44
                || habitat.reserved(point, 1)
                || habitat.tree_occupied(cell)
            {
                continue;
            }
            if matches!(
                kind,
                M::AcaciaSapling | M::YellowSavannaFlower | M::RedSavannaFlower
            ) && !matches!(
                column.surface_material,
                M::Grass | M::SavannaTurf | M::DrySavannaTurf | M::DrySavannaSoil
            ) {
                continue;
            }
            if !habitat.terrain.in_prop_patch(point, kind as u16) {
                continue;
            }
            cells.entry(cell).or_insert(kind as u16);
        }
    }
    grassland(habitat, tile, cells);
}

/// Jittered two-metre plots form dense fields along the same continuous path
/// mask used by woodland. Check the cheap cover mask before querying a column.
fn grassland(habitat: &mut Habitat<'_>, tile: Tile, cells: &mut BTreeMap<Cell, u16>) {
    let seed = habitat.terrain.config().seed.0;
    for z in 0..16 {
        for x in 0..16 {
            let plot = [tile[0] * 16 + x, tile[1] * 16 + z];
            let key = ticket(seed, plot, 0x922a);
            let point = [
                plot[0] * 2 + i64::from(key & 1),
                plot[1] * 2 + i64::from((key >> 1) & 1),
            ];
            let (patch, path) = habitat
                .terrain
                .climate
                .ground_masks(point[0] as f64, point[1] as f64);
            let cover = (0.36 + 0.63 * patch) * path;
            if f64::from((key >> 3) & 255) >= cover * 256. {
                continue;
            }
            let Some((y, column)) = habitat.ground(point) else {
                continue;
            };
            let cell = [point[0], y + 1, point[1]];
            if column.biome.weights.savanna < 0.44
                || !matches!(
                    column.surface_material,
                    M::SavannaTurf | M::DrySavannaTurf | M::DrySavannaSoil | M::Grass
                )
                || habitat.tree_occupied(cell)
                || habitat.reserved(point, 1)
            {
                continue;
            }
            let tall = f64::from((key >> 14) & 255) < (0.15 + 0.63 * patch) * path * 256.;
            cells.entry(cell).or_insert(if tall {
                M::SavannaTallGrass
            } else {
                M::SavannaShortGrass
            } as u16);
        }
    }
}

/// Validate the entire footprint before stamping either pier. The passage and
/// broken approach follow the natural ground even across chunk boundaries.
fn gateway(habitat: &mut Habitat<'_>, center: Tile, key: u32, cells: &mut BTreeMap<Cell, u16>) {
    let Some((base, column)) = habitat.ground(center) else {
        return;
    };
    if column.biome.weights.savanna < 0.74
        || habitat.reserved(center, 8)
        || habitat.root_distance_squared(center) < 90
    {
        return;
    }
    let mut footprint = Vec::new();
    for along in -6i64..=6 {
        for across in -6i64..=6 {
            let p = if key & 1 == 0 {
                [center[0] + along, center[1] + across]
            } else {
                [center[0] + across, center[1] + along]
            };
            if along.abs() > 2 && across.abs() > 1 {
                continue;
            }
            let Some((floor, _)) = habitat.ground(p) else {
                return;
            };
            if (floor - base).abs() > 2 || cells.contains_key(&[p[0], floor + 1, p[1]]) {
                return;
            }
            footprint.push((along, across, p, floor));
        }
    }
    for (along, across, p, floor) in footprint {
        let age = ticket(u64::from(key), p, 0x4d0);
        if across.abs() <= 1 {
            let a = along.abs();
            let arch_spring = match a {
                0..=1 => Some(6),
                2 => Some(5),
                3 => Some(4),
                4 => Some(1),
                _ => None,
            };
            if let Some(start) = arch_spring {
                let bottom = if start == 1 { floor + 1 } else { base + start };
                for y in bottom..=base + 7 - i64::from(a >= 3) {
                    let stone = if y == base + 2 || y == base + 6 {
                        M::CarvedSandstone
                    } else if age.wrapping_add(y as u32).is_multiple_of(6) {
                        M::WeatheredSavannaStone
                    } else {
                        M::RedSandstone
                    };
                    cells.insert([p[0], y, p[1]], stone as u16);
                }
            } else if a == 5 && across == 0 {
                for y in floor + 1..=base + 2 {
                    cells.insert([p[0], y, p[1]], M::OchreStone as u16);
                }
            }
        }
        if along.abs() <= 2 && !age.is_multiple_of(5) {
            cells.insert(
                [p[0], floor, p[1]],
                if age.is_multiple_of(7) {
                    M::WeatheredSavannaStone
                } else {
                    M::CarvedSandstone
                } as u16,
            );
        }
    }
}
