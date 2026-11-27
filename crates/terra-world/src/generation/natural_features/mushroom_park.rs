//! Mushroom glades, creek crossings and small timber lookouts fit the local
//! topography. Everything is stamped in the same bounded patch as its footing.
use super::{Cell, Habitat, TILE_SIZE, Tile, ticket};
use crate::{edits::BlockObjectState, generation::structures::StructureObject};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

type Cells = BTreeMap<Cell, u16>;

fn pavilion_ticket(seed: u32, ecology_revision: u8) -> bool {
    let period = if ecology_revision >= 13 { 32 } else { 16 };
    seed % period < 3
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    cells: &mut Cells,
    objects: &mut Vec<StructureObject>,
) {
    let center = tile.map(|v| v * TILE_SIZE + TILE_SIZE / 2);
    if h.terrain
        .biome_at(center[0] as f64, center[1] as f64)
        .weights
        .mushroom_park
        < 0.25
    {
        return;
    }
    let seed = ticket(h.terrain.config().seed.0, tile, 2821);
    let settings = h.terrain.config().structures;
    if h.terrain.config().structures_revision > 0 && !h.reserved(center, 16) {
        // Try each crossing direction before trying a dry-land landmark.
        let bridge = settings.villages && (0..2).any(|axis| bridge(h, center, axis, cells));
        if !bridge
            && settings.outposts
            && pavilion_ticket(seed, h.terrain.config().ecology_revision)
        {
            pavilion(h, center, cells, objects);
        } else if !bridge && settings.ruins && seed % 16 == 3 {
            glade_arch(h, center, cells);
        }
    }
    for gz in 0..6 {
        for gx in 0..6 {
            let hash = ticket(u64::from(seed), [gx, gz], 2823);
            let point = [
                center[0] - 14 + gx * 5 + i64::from(hash % 3),
                center[1] - 14 + gz * 5 + i64::from((hash >> 5) % 3),
            ];
            if h.reserved(point, 2) {
                continue;
            }
            let c = h.column(point);
            if c.biome.weights.mushroom_park < 0.38
                || c.cave.open_to_sky()
                || c.river_cavity.is_some()
            {
                continue;
            }
            if let Some(level) = c.water_level {
                if level - c.height > 0.45
                    && level - c.height < 3.3
                    && c.slope < 0.95
                    && hash.is_multiple_of(3)
                {
                    cells
                        .entry([point[0], level.ceil() as i64, point[1]])
                        .or_insert(M::PondBlossoms as u16);
                }
                continue;
            }
            let Some((floor, _)) = h.ground(point) else {
                continue;
            };
            let cell = [point[0], floor + 1, point[1]];
            if cells.contains_key(&cell)
                || cells.contains_key(&[point[0], floor, point[1]])
                || h.tree_occupied(cell)
            {
                continue;
            }
            let m = match hash % 32 {
                0 => M::MossyNurseLog,
                1 => M::AmberShelfStump,
                2..=3 => M::ExposedMycelialRoots,
                4..=6 => M::GoldenSporePods,
                7..=13 => M::WoodlandToadstools,
                14..=18 => M::MushroomMossTuft,
                19..=21 => M::ForestFern,
                22..=24 => M::CaveWildflowers,
                25 => M::CavePebbles,
                _ => continue,
            };
            if !h.terrain.in_prop_patch(point, m as u16) {
                continue;
            }
            cells.insert(cell, m as u16);
            if hash.is_multiple_of(7) {
                hanging_vines(h, cell, hash, cells);
            }
        }
    }
}

/// Walk down from a real cap. Every strand is attached to a gill at its top.
fn hanging_vines(h: &Habitat<'_>, floor: Cell, seed: u32, cells: &mut Cells) {
    let reach = if h.terrain.config().ecology_revision >= 13 {
        15
    } else {
        10
    };
    for tree in h.trees.iter().filter(|t| {
        t.wood_material == M::MushroomStem
            && (t.origin[0] - floor[0]).abs() <= reach
            && (t.origin[2] - floor[2]).abs() <= reach
    }) {
        let Some(y) = (floor[1] + 4..=tree.origin[1] + i64::from(tree.bounds.max[1]))
            .find(|y| tree.leaves_at([floor[0], *y, floor[2]]) == Some(M::MushroomGill))
        else {
            continue;
        };
        for dy in 1..=i64::from(1 + (seed >> 6) % 3) {
            let cell = [floor[0], y - dy, floor[2]];
            if cell[1] <= floor[1] + 1 || h.tree_occupied(cell) || cells.contains_key(&cell) {
                break;
            }
            cells.insert(cell, M::CaveVine as u16);
        }
        break;
    }
}

fn oriented(center: Tile, axis: usize, t: i64, w: i64) -> Tile {
    if axis == 0 {
        [center[0] + t, center[1] + w]
    } else {
        [center[0] + w, center[1] + t]
    }
}

fn bridge(h: &mut Habitat<'_>, center: Tile, axis: usize, cells: &mut Cells) -> bool {
    // A nearby wet sample selects this planning pass. Three offsets cover streams
    // passing off-centre, while keeping every possible abutment inside the tile.
    for shift in [-7, 0, 7] {
        let mid = oriented(center, axis, 0, shift);
        let water = h.column(mid);
        let Some(level) = water.water_level else {
            continue;
        };
        if water.height > level - 0.55 || water.river_cavity.is_some() {
            continue;
        }
        let mut ends = [0; 2];
        let mut footing = [0; 2];
        for (i, sign) in [-1, 1].into_iter().enumerate() {
            if let Some((t, y)) = (2..=14).find_map(|d| {
                let p = oriented(mid, axis, sign * d, 0);
                let (y, c) = h.ground(p)?;
                (c.water_level.is_none() && (y as f64 - level).abs() < 2.5).then_some((sign * d, y))
            }) {
                ends[i] = t;
                footing[i] = y;
            }
        }
        if ends[0] == 0
            || ends[1] == 0
            || ends[1] - ends[0] < 6
            || (footing[0] - footing[1]).abs() > 2
        {
            continue;
        }
        let mut plan = Cells::new();
        let mut clear = true;
        for t in ends[0]..=ends[1] {
            let span = (ends[1] - ends[0]) as f64;
            let u = (t - ends[0]) as f64 / span;
            let ramp = (4.0 * u * (1. - u) * 2.2).round() as i64;
            let y = (footing[0] as f64 * (1. - u) + footing[1] as f64 * u).round() as i64 + ramp;
            for w in -2_i64..=2 {
                let [x, z] = oriented(mid, axis, t, w);
                let c = h.column([x, z]);
                if c.river_cavity.is_some()
                    || c.height > y as f64 + 1.3
                    || y as f64 <= level - 0.01 && t != ends[0] && t != ends[1]
                    || (y..=y + 3)
                        .any(|yy| h.tree_occupied([x, yy, z]) || cells.contains_key(&[x, yy, z]))
                {
                    clear = false;
                    break;
                }
                plan.insert([x, y, z], M::OakPlanks as u16);
                if w.abs() == 2 {
                    plan.insert([x, y + 1, z], M::OakFence as u16);
                    if t == ends[0] + 1 || t == ends[1] - 1 {
                        for yy in (c.height - 0.5).floor() as i64..y {
                            plan.insert([x, yy, z], M::ParkTimber as u16);
                        }
                    }
                }
            }
            if !clear {
                break;
            }
        }
        if clear {
            cells.extend(plan);
            return true;
        }
    }
    false
}

/// A fully walkable foundation, slightly raised at most a metre above the
/// natural approach. Reject obstructed and unsupported sites before stamping.
fn dry_footprint(
    h: &mut Habitat<'_>,
    center: Tile,
    radius: i64,
    cells: &Cells,
) -> Option<(i64, Vec<(Tile, i64)>)> {
    let mut grounds = Vec::new();
    let mut low = i64::MAX;
    let mut high = i64::MIN;
    if h.root_distance_squared(center) < (radius + 2).pow(2) {
        return None;
    }
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let p = [center[0] + dx, center[1] + dz];
            let (y, _) = h.ground(p)?;
            low = low.min(y);
            high = high.max(y);
            if high - low > 1
                || (y + 1..=y + 8).any(|yy| {
                    h.tree_occupied([p[0], yy, p[1]]) || cells.contains_key(&[p[0], yy, p[1]])
                })
            {
                return None;
            }
            grounds.push((p, y));
        }
    }
    Some((high, grounds))
}

fn pavilion(
    h: &mut Habitat<'_>,
    center: Tile,
    cells: &mut Cells,
    objects: &mut Vec<StructureObject>,
) {
    let Some((y, ground)) = dry_footprint(h, center, 3, cells) else {
        return;
    };
    for ([x, z], local) in ground {
        for yy in local..=y {
            cells.insert([x, yy, z], M::MossyStone as u16);
        }
        cells.insert([x, y + 1, z], M::OakPlanks as u16);
    }
    // Leave both entrances open. Timber corner braces and lanterns hang
    // under the eaves; the balustrades stay below a standing player's eye.
    for dx in [-3, 3] {
        for dz in -2..=2 {
            cells.insert(
                [center[0] + dx, y + 2, center[1] + dz],
                M::SpruceFence as u16,
            );
        }
    }
    for dz in [-2, 2] {
        for dx in [-2, 2] {
            for dy in 2..=5 {
                cells.insert(
                    [center[0] + dx, y + dy, center[1] + dz],
                    M::ParkTimber as u16,
                );
            }
        }
    }
    // Lower flared eaves, clerestory and a raised ridge with turned-up corners.
    for (dy, r) in [(5, 4_i64), (6, 3), (7, 2), (8, 1)] {
        for dx in -r..=r {
            for dz in -r..=r {
                if dx.abs().max(dz.abs()) == r || dy == 8 {
                    let lift = i64::from(dx.abs() == r && dz.abs() == r && r >= 3);
                    cells.insert(
                        [center[0] + dx, y + dy + lift, center[1] + dz],
                        M::ParkCedarShingles as u16,
                    );
                }
            }
        }
    }
    for dx in -2..=2 {
        for dz in [-2, 0, 2] {
            cells.insert(
                [center[0] + dx, y + 6, center[1] + dz],
                M::ParkTimber as u16,
            );
        }
    }
    for dx in [-1, 1] {
        hang_lantern(cells, objects, [center[0] + dx, y + 5, center[1]]);
        let x = center[0] + dx * 2;
        for sign in [-1, 1] {
            cells.insert([x, y + 5, center[1] + sign * 3], M::ParkTimber as u16);
            hang_lantern(cells, objects, [x, y + 4, center[1] + sign * 3]);
        }
    }
    for dz in [-4, 4] {
        let p = [center[0], center[1] + dz];
        if let Some((local, _)) = h.ground(p)
            && y - local == 1
        {
            for dx in -1..=1 {
                cells.insert([p[0] + dx, y, p[1]], M::MossyStone as u16);
            }
        }
    }
}

// Streaming initializes this pose before checking the rafter for support.
fn hang_lantern(cells: &mut Cells, objects: &mut Vec<StructureObject>, position: Cell) {
    cells.insert(position, M::Lantern as u16);
    objects.push(StructureObject {
        position,
        material: M::Lantern as u16,
        state: Some(BlockObjectState {
            facing: terra_core::lantern::HANGING,
            ..Default::default()
        }),
        loot: None,
        crop: None,
    });
}

fn glade_arch(h: &mut Habitat<'_>, center: Tile, cells: &mut Cells) {
    let Some((base, _)) = dry_footprint(h, center, 3, cells) else {
        return;
    };
    for dz in -1..=1 {
        for dx in -3_i64..=3 {
            let span = dx.abs();
            let bottom = match span {
                3 => 0,
                2 => 4,
                1 => 5,
                _ => 6,
            };
            for dy in bottom..=6 + i64::from(span < 2) {
                let m = if (dx + dz + dy) % 4 == 0 {
                    M::MossyStone
                } else {
                    M::StructureStone
                };
                cells.insert([center[0] + dx, base + dy, center[1] + dz], m as u16);
            }
        }
    }
    cells.insert([center[0] - 2, base + 3, center[1] - 1], M::CaveVine as u16);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerraGenerator;
    use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};

    #[test]
    fn pavilion_lottery_retains_half_the_old_sites_and_no_new_sites() {
        let old: Vec<_> = (0..32).filter(|&n| pavilion_ticket(n, 12)).collect();
        let current: Vec<_> = (0..32).filter(|&n| pavilion_ticket(n, 13)).collect();
        assert_eq!(current.len() * 2, old.len());
        assert!(current.iter().all(|site| old.contains(site)));
    }

    #[test]
    fn real_park_landmarks_generate_in_sections_on_the_same_footing_as_point_queries() {
        use crate::SectionGenerator;
        use terra_voxel::global_to_section;
        let config = WorldConfig {
            seed: WorldSeed(42042),
            ecology_revision: 12,
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        };
        let t = TerraGenerator::new(config.clone()).unwrap();
        let generator = SectionGenerator::new(t.clone());
        let legacy = TerraGenerator::new(WorldConfig {
            ecology_revision: 11,
            ..config
        })
        .unwrap();
        for (tile, cell, m) in [
            ([-30, -12], [-947, 49, -368], M::StructureStone),
            ([-34, -7], [-1073, 62, -208], M::Lantern),
            ([-36, -4], [-1141, 57, -114], M::OakFence),
        ] {
            let patch = t.natural_patch(tile);
            assert_eq!(patch.block_at(cell), Some(m as u16));
            assert_ne!(legacy.natural_patch(tile).block_at(cell), Some(m as u16));
            assert_eq!(generator.block_at(cell).unwrap(), m as u16);
            let (owner, local) = global_to_section(cell);
            let section = generator
                .build(owner, 0, &Default::default(), true)
                .unwrap();
            assert_eq!(section.data.block(local).unwrap(), m as u16);
            assert!(
                patch
                    .cells
                    .iter()
                    .all(|(p, _)| p[0].div_euclid(32) == tile[0] && p[2].div_euclid(32) == tile[1])
            );
            if m == M::OakFence {
                let start = [tile[0] * TILE_SIZE, 0, tile[1] * TILE_SIZE];
                let trees = super::super::trees_in_column(&t, start, TILE_SIZE as usize);
                let deck: Vec<_> = patch
                    .cells
                    .iter()
                    .filter(|(_, material)| *material == M::OakPlanks as u16)
                    .collect();
                assert!(deck.len() >= 30);
                for &([x, y, z], _) in deck {
                    for height in y..=y + 3 {
                        let cell = [x, height, z];
                        assert!(
                            trees.iter().all(|tree| {
                                tree.wood_at(cell).is_none() && tree.leaves_at(cell).is_none()
                            }),
                            "mushroom intrudes on the bridge at {cell:?}"
                        );
                    }
                }
            }
        }
    }
}
