//! Flower meadows and small garden landmarks follow real banks and clearings.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use crate::{edits::BlockObjectState, generation::structures::StructureObject};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;
mod pagoda;

fn register_pose(objects: &mut Vec<StructureObject>, position: Cell, material: M, turn: u8) {
    let facing = match material {
        M::Lantern => terra_core::lantern::HANGING,
        M::SakuraSilkBanner => turn,
        M::SakuraFoxStatue => (turn + 2) % 4,
        _ => return,
    };
    objects.push(StructureObject {
        position,
        material: material as u16,
        state: Some(BlockObjectState {
            facing,
            ..Default::default()
        }),
        loot: None,
        crop: None,
    });
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) {
    // Canopies may cross a biome or tile boundary. The tile containing the
    // lamp owns its ceiling-mount pose, regardless of where the trunk grows.
    for tree in &h.trees {
        if tree.wood_material == M::CherryLog
            && let Some(p) = tree.lantern
            && [p[0].div_euclid(TILE_SIZE), p[2].div_euclid(TILE_SIZE)] == tile
        {
            cells.insert(p, M::Lantern as u16);
            register_pose(objects, p, M::Lantern, 0);
        }
    }
    let start = tile.map(|n| n * TILE_SIZE);
    let center = [start[0] + 16, start[1] + 16];
    if h.terrain
        .climate_biome_at(center[0] as f64, center[1] as f64)
        .weights
        .cherry_forest
        < 0.32
    {
        return;
    }
    let draw = ticket(h.terrain.config().seed.0, tile, 7810);
    if h.terrain.config().structures_revision > 0
        && h.terrain.config().structures.ruins
        && !h.reserved(center, 15)
        && h.column(center).biome.weights.cherry_forest > 0.55
    {
        if h.column(center).water_level.is_some() && draw.is_multiple_of(3) {
            if !bridge(h, center, 0, cells) {
                bridge(h, center, 1, cells);
            }
        } else if !(pagoda::chosen(h.terrain.config().seed.0, tile)
            && pagoda::build(h, center, cells, objects))
            && draw.is_multiple_of(16)
        {
            gate(h, center, (draw >> 8) as usize % 2, cells, objects);
        }
    }
    for gz in 0..6 {
        for gx in 0..6 {
            let hash = ticket(u64::from(draw), [gx, gz], 7811);
            let point = [
                start[0] + 1 + gx * 5 + i64::from(hash % 5),
                start[1] + 1 + gz * 5 + i64::from((hash >> 5) % 5),
            ];
            let column = h.column(point);
            if column.biome.weights.cherry_forest < 0.45
                || h.reserved(point, 1)
                || column.cave.open_to_sky()
                || column.river_cavity.is_some()
            {
                continue;
            }
            if let Some(water) = column.water_level {
                if (0.6..2.6).contains(&(water - column.height))
                    && column.ocean.ice_depth == 0
                    && column.water_flow[0].hypot(column.water_flow[1]) < 0.68
                    && hash.is_multiple_of(5)
                {
                    cells
                        .entry([point[0], water.ceil() as i64, point[1]])
                        .or_insert(M::LilyPad as u16);
                }
                continue;
            }
            let Some((y, _)) = h.ground(point) else {
                continue;
            };
            let cell = [point[0], y + 1, point[1]];
            if h.tree_occupied(cell) || cells.contains_key(&[point[0], y, point[1]]) {
                continue;
            }
            let material = match column.surface_material {
                M::Grass | M::PetalTurf if h.root_distance_squared(point) < 20 => {
                    M::FallenCherryPetals
                }
                M::Grass | M::PetalTurf => {
                    let (_, path) = h
                        .terrain
                        .climate
                        .ground_masks(point[0] as f64, point[1] as f64);
                    if path < 0.18 {
                        M::FallenCherryPetals
                    } else {
                        match (hash >> 12) % 10 {
                            0..=2 => M::SakuraBellflowers,
                            3..=5 => M::SakuraFlowerTuft,
                            6 => M::SakuraPetalShrub,
                            _ => continue,
                        }
                    }
                }
                M::BlossomStone | M::MossyBlossomStone if hash.is_multiple_of(4) => {
                    M::FallenCherryPetals
                }
                _ => continue,
            };
            if !h.terrain.in_prop_patch(point, material as u16) {
                continue;
            }
            cells.entry(cell).or_insert(material as u16);
        }
    }
}

fn offset(center: Tile, along: i64, across: i64, axis: usize) -> Tile {
    if axis == 0 {
        [center[0] + along, center[1] + across]
    } else {
        [center[0] + across, center[1] + along]
    }
}

// The whole footprint is inspected before emitting anything. Both abutments
// meet dry ground, and half-block rises keep the three central lanes walkable.
fn bridge(h: &mut Habitat<'_>, center: Tile, axis: usize, cells: &mut BTreeMap<Cell, u16>) -> bool {
    let mut banks = [0i64; 2];
    let mut heights = [0i64; 2];
    let mut highest_water = f64::NEG_INFINITY;
    for (i, sign) in [-1, 1].into_iter().enumerate() {
        for step in 0..=13 {
            let point = offset(center, sign * step, 0, axis);
            let column = h.column(point);
            if column.cave.open_to_sky() || column.river_cavity.is_some() {
                return false;
            }
            if let Some(level) = column.water_level {
                highest_water = highest_water.max(level);
            } else {
                if step < 3 {
                    return false;
                }
                let Some((y, _)) = h.ground(point) else {
                    return false;
                };
                heights[i] = y;
                banks[i] = sign * step;
                break;
            }
        }
    }
    if banks[0] == 0 || banks[1] == 0 || heights[0].abs_diff(heights[1]) > 2 {
        return false;
    }
    let crest = heights[0].max(heights[1]).max(highest_water.ceil() as i64) + 1;
    if crest - heights[0].min(heights[1]) > 4 {
        return false;
    }
    let mut structure = Vec::new();
    for d in banks[0]..=banks[1] {
        let half_height = (2 * (crest + 1))
            .min(2 * (heights[0] + 1) + d - banks[0])
            .min(2 * (heights[1] + 1) + banks[1] - d);
        let deck = (half_height - 1).div_euclid(2);
        for across in -2i64..=2 {
            let p = offset(center, d, across, axis);
            let column = h.column(p);
            let bed = (column.height - 0.5).floor() as i64;
            if bed > deck
                || column
                    .water_level
                    .is_some_and(|level| level > half_height as f64 * 0.5)
                || h.root_distance_squared(p) <= 4
                || h.reserved(p, 1)
                || cells.contains_key(&[p[0], deck + 1, p[1]])
                || h.tree_occupied([p[0], deck + 2, p[1]])
            {
                return false;
            }
            let end = d == banks[0] || d == banks[1];
            let plank = if across.abs() < 2 && half_height.rem_euclid(2) == 1 {
                M::CherrySlab
            } else {
                M::CherryPlanks
            };
            structure.push(([p[0], deck, p[1]], plank));
            if across.abs() == 2 {
                structure.push(([p[0], deck + 1, p[1]], M::CherryFence));
                if end {
                    for y in bed + 1..deck {
                        structure.push(([p[0], y, p[1]], M::BlossomStone));
                    }
                    structure.push(([p[0], deck + 2, p[1]], M::SakuraGardenLantern));
                }
            }
        }
    }
    for (pos, material) in structure {
        cells.insert(pos, material as u16);
    }
    true
}

fn gate(
    h: &mut Habitat<'_>,
    center: Tile,
    axis: usize,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) {
    let Some((foot, _)) = h.ground(center) else {
        return;
    };
    let mut pads = Vec::new();
    for x in -5..=5 {
        for z in -2..=2 {
            let p = offset(center, x, z, axis);
            let Some((bed, _)) = h.ground(p) else {
                return;
            };
            if bed.abs_diff(foot) > 1
                || h.root_distance_squared(p) < 5
                || (bed + 1..=foot + 6).any(|y| {
                    h.tree_occupied([p[0], y, p[1]])
                        || cells
                            .get(&[p[0], y, p[1]])
                            .is_some_and(|&id| M::from_raw(id).is_some_and(M::is_solid))
                })
            {
                return;
            }
            if [-3, 3].contains(&x) && z == 0 {
                pads.push((x, bed));
            }
        }
    }
    let mut put = |x, z, y, m: M| {
        let p = offset(center, x, z, axis);
        let pos = [p[0], y, p[1]];
        cells.insert(pos, m as u16);
        register_pose(objects, pos, m, axis as u8);
    };
    for (x, bed) in pads {
        put(x, 0, bed + 1, M::MossyBlossomStone);
        for y in bed + 2..=foot + 5 {
            put(x, 0, y, M::CherryLog);
        }
        let p = offset(center, x, -2, axis);
        if let Some((y, _)) = h.ground(p) {
            put(x, -2, y + 1, M::SakuraFoxStatue);
        }
    }
    for x in -4..=4 {
        put(x, 0, foot + 4, M::CherryPlanks);
    }
    for x in -5i64..=5 {
        put(
            x,
            0,
            foot + 6,
            if x.abs() >= 4 {
                M::CherryPlanks
            } else {
                M::CherrySlab
            },
        );
    }
    for x in [-3, 0, 3] {
        put(x, 0, foot + 5, M::CherryLog);
    }
    put(0, 0, foot + 3, M::SakuraSilkBanner);
    for x in [-2, 2] {
        put(x, 0, foot + 3, M::Lantern);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use terra_core::{BiomeDefinitions, WorldConfig};

    fn shipped_config() -> WorldConfig {
        WorldConfig {
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..WorldConfig::default()
        }
    }

    #[test]
    fn gate_is_supported_and_materializes_in_the_editable_section() {
        // Default seed, flower meadow, nearby mature cherries.
        let cfg = shipped_config();
        let g = TerraGenerator::new(cfg.clone()).unwrap();
        let center = [-144, 40, 2800];
        assert_eq!(
            g.natural_material_at(center),
            Some(M::SakuraSilkBanner as u16)
        );
        let patch = g.natural_patch([-5, 87]);
        for dx in [-2, 2] {
            let p = [center[0] + dx, center[1], center[2]];
            assert!(patch.objects.iter().any(|o| {
                o.position == p
                    && o.state
                        .is_some_and(|s| s.facing == terra_core::lantern::HANGING)
            }));
        }
        let props = g.surface_flora([-5, 87]);
        let statues: Vec<_> = props
            .iter()
            .filter(|(_, m)| *m == M::SakuraFoxStatue as u16)
            .collect();
        assert_eq!(statues.len(), 2);
        for &(p, _) in statues {
            let c = g.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
            assert_eq!(p[1], (c.height - 0.5).floor() as i64 + 1);
        }
        let sections = SectionGenerator::new(g);
        assert_eq!(
            sections.block_at(center).unwrap(),
            M::SakuraSilkBanner as u16
        );
        let mut old = cfg;
        old.ecology_revision = 14;
        let old = TerraGenerator::new(old).unwrap();
        assert!(
            old.surface_flora([-5, 87])
                .iter()
                .all(|(_, id)| !(596..=605).contains(id))
        );
    }

    #[test]
    fn hillside_pagoda_is_anchored_accessible_and_respects_structure_settings() {
        let cfg = shipped_config();
        let seed = cfg.seed.0;
        assert!(pagoda::chosen(seed, [-11, 83]));
        for dz in -3i64..=3 {
            for dx in -3i64..=3 {
                if (1..16).contains(&(dx * dx + dz * dz)) {
                    assert!(!pagoda::chosen(seed, [-11 + dx, 83 + dz]));
                }
            }
        }
        let t = TerraGenerator::new(cfg.clone()).unwrap();
        let g = SectionGenerator::new(t.clone());
        let lamp = [-336, 58, 2672];
        assert_eq!(g.block_at(lamp).unwrap(), M::SakuraGardenLantern as u16);
        let floor = lamp[1] - 16;
        assert_eq!(
            g.block_at([lamp[0] - 4, floor + 1, lamp[2]]).unwrap(),
            M::Air as u16
        );
        let mut steps = Vec::new();
        for x in lamp[0] - 10..=lamp[0] - 4 {
            let ground = t.surface_column(x as f64 + 0.5, lamp[2] as f64 + 0.5);
            assert!(ground.water_level.is_none());
            let bed = (ground.height - 0.5).floor() as i64;
            let top = (bed..=floor)
                .rev()
                .find_map(
                    |y| match t.natural_material_at([x, y, lamp[2]]).and_then(M::from_raw) {
                        Some(M::BlossomStone) => Some(y as f64 + 1.0),
                        Some(M::CherrySlab) => Some(y as f64 + 0.5),
                        _ => None,
                    },
                )
                .expect("supported west approach");
            steps.push(top);
        }
        assert_eq!(steps.last(), Some(&(floor as f64 + 1.0)));
        assert!(steps.windows(2).all(|p| (p[1] - p[0]).abs() <= 0.5));
        let mut disabled = cfg;
        disabled.structures.ruins = false;
        assert_ne!(
            TerraGenerator::new(disabled)
                .unwrap()
                .natural_material_at(lamp),
            Some(M::SakuraGardenLantern as u16)
        );
    }

    #[test]
    fn bridge_walking_surface_meets_both_banks_with_half_block_steps() {
        let mut cfg = shipped_config();
        cfg.rivers = true;
        let g = TerraGenerator::new(cfg).unwrap();
        let tile = [-4, 86];
        let center = tile.map(|n| n * TILE_SIZE + 16);
        let props = g.surface_flora(tile);
        let lanterns: Vec<_> = props
            .iter()
            .filter(|(_, id)| *id == M::SakuraGardenLantern as u16)
            .map(|(p, _)| p)
            .collect();
        assert_eq!(lanterns.len(), 4);
        let xmin = lanterns.iter().map(|p| p[0]).min().unwrap();
        let xmax = lanterns.iter().map(|p| p[0]).max().unwrap();
        let mut heights = Vec::new();
        for x in xmin..=xmax {
            let c = g.surface_column(x as f64 + 0.5, center[1] as f64 + 0.5);
            let bed = (c.height - 0.5).floor() as i64;
            let h = (bed..=bed + 10)
                .find_map(|y| match g.natural_material_at([x, y, center[1]]) {
                    Some(id) if id == M::CherryPlanks as u16 => Some(y as f64 + 1.0),
                    Some(id) if id == M::CherrySlab as u16 => Some(y as f64 + 0.5),
                    _ => None,
                })
                .expect("continuous bridge deck");
            if x == xmin || x == xmax {
                assert_eq!(h, bed as f64 + 1.0);
            }
            if let Some(level) = c.water_level {
                assert!(h >= level);
            }
            heights.push(h);
        }
        assert!(heights.len() >= 7 && heights.iter().any(|h| h.fract() == 0.5));
        assert!(
            heights
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() <= 0.5)
        );
    }
}
