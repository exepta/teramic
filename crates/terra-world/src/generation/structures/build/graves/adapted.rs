//! Revision ten preserves natural hills between individual graves and rooms.
use super::*;
use crate::generation::structures::graves::adapted_layout;

mod chambers;
pub(in crate::generation::structures::build) mod ground;
#[cfg(test)]
mod tests;

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    let kind = terrain.grave_site_candidate(region)?;
    let x = region[0] * REGION_SIZE + 192 + i64::from((seed >> 7) % 33) - 16;
    let z = region[1] * REGION_SIZE + 192 + i64::from((seed >> 15) % 33) - 16;
    let mut lowest = i64::MAX;
    for dz in (-RADIUS..=RADIUS).step_by(4) {
        for dx in (-RADIUS..=RADIUS).step_by(4) {
            let column = terrain.surface_column((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5);
            lowest = lowest.min((column.height - 0.5).floor() as i64 + 1);
            if column.biome.weights.spirit_blossom < 0.70
                || column.water_level.is_some()
                || column.river_cavity.is_some()
            {
                return None;
            }
        }
    }
    let mut b = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ region[0] as u64 ^ (region[1] as u64).rotate_left(17),
            kind,
            style: VillageStyle::Highland,
            center: [x, 0, z],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
            grave: None,
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed,
    };
    let y = ground::fitted_floor(&b, [x - 11, z - 12], [x + 11, z + 4], 3)?;
    if !(-65..=260).contains(&y) {
        return None;
    }
    b.summary.center[1] = y;
    ground::perimeter(&mut b);
    if kind == StructureKind::GraveDungeon {
        let initial_drop = (y - lowest + 8).max(12);
        if initial_drop > 24 {
            return None;
        }
        let mut info = adapted_layout(&b.summary, initial_drop)?;
        for floor in &info.floors {
            for room in &floor.rooms {
                for rz in room.bounds[0][2]..=room.bounds[1][2] {
                    for rx in room.bounds[0][0]..=room.bounds[1][0] {
                        if b.ground(rx, rz) <= room.bounds[1][1] + 2 {
                            return None;
                        }
                    }
                }
            }
        }
        ground::foundation(&mut b, [x - 11, y, z - 12], [x + 11, y + 15, z + 4], 3)?;
        chambers::mausoleum(&mut b, info.variant);
        chambers::dungeon(&mut b, &info);
        let approach_height = b.ground(x, z - 31);
        let approach = ground::path(&mut b, [x, y, z - 12], [x, approach_height, z - 31])?;
        let mut connected = approach.into_iter().rev().collect::<Vec<_>>();
        connected.extend(info.route.into_iter().skip(1));
        info.route = connected;
        b.summary.grave = Some(info);
        ground::grave_groups(&mut b, true);
    } else {
        ground::grave_groups(&mut b, false);
        ground::chapel(&mut b)?;
    }
    b.finish()
}
