//! Locate real chambers and keep the teleport on their dry underground floor.
use super::{
    CaveEcology, EditJournal, LocateTarget, Located, SEARCH_RINGS, SEARCH_STEP, TerraGenerator,
    WorldSpawn, creative_clearance, spawn::LoadedSpawn,
};
use terra_voxel::MaterialId;
use terra_world::{SectionGenerator, streaming::ChunkStore, water::fluid_cell_at};

fn landing_state(
    terrain: &TerraGenerator,
    journal: &EditJournal,
    spawn: WorldSpawn,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> LoadedSpawn {
    let [x, y, z] = spawn.ground;
    let Some(ground) = block(spawn.ground) else {
        return LoadedSpawn::Waiting;
    };
    if !MaterialId::from_raw(ground).is_some_and(|m| {
        m.is_opaque()
            || matches!(
                m,
                MaterialId::ClearCaveIce | MaterialId::TranslucentSaltBlock
            )
    }) {
        return LoadedSpawn::Invalid;
    }
    for headroom in 1..=3 {
        let p = [x, y + headroom, z];
        let Some(id) = block(p) else {
            return LoadedSpawn::Waiting;
        };
        if id != MaterialId::Air as u16 || fluid_cell_at(terrain, journal, p, id).amount > 0 {
            return LoadedSpawn::Invalid;
        }
    }
    LoadedSpawn::Ready
}

pub(super) fn clear(terrain: &TerraGenerator, journal: &EditJournal, spawn: WorldSpawn) -> bool {
    let generator = SectionGenerator::new(terrain.clone());
    matches!(
        landing_state(terrain, journal, spawn, |p| journal
            .block(p)
            .or_else(|| generator.block_at(p).ok())),
        LoadedSpawn::Ready
    )
}

pub(super) fn loaded(
    chunks: &ChunkStore,
    terrain: &TerraGenerator,
    journal: &EditJournal,
    spawn: WorldSpawn,
) -> LoadedSpawn {
    landing_state(terrain, journal, spawn, |p| chunks.block_at(p))
}

pub(super) fn find(
    terrain: &TerraGenerator,
    origin: [i64; 2],
    kind: CaveEcology,
) -> Option<Located> {
    let config = terrain.config();
    if !config.caves
        || config.cave_structure_revision == 0
        || config.geology_revision < 4
        || origin.iter().any(|a| a.unsigned_abs() > 990_000)
    {
        return None;
    }
    let generator = SectionGenerator::new(terrain.clone());
    let journal = EditJournal::default();
    let radius = SEARCH_RINGS * SEARCH_STEP;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
    let try_bank = |x: i64, z: i64| {
        if x.abs() > 990_000
            || z.abs() > 990_000
            || (origin[0] - x).pow(2) + (origin[1] - z).pow(2) > radius.pow(2)
        {
            return None;
        }
        let room = terrain.cave_room_at(x as f64 + 0.5, z as f64 + 0.5);
        if room.kind != kind
            || !room.active()
            || room.ceiling - room.floor < 5.
            || room.water.is_some_and(|water| water.depth > 1.)
        {
            return None;
        }
        let floor = room.floor.floor() as i64;
        for dy in (-5..=5).rev() {
            let spawn = WorldSpawn {
                ground: [x, floor + dy, z],
            };
            if spawn.ground[1] as f64 + 3. >= room.ceiling {
                continue;
            }
            if matches!(
                landing_state(terrain, &journal, spawn, |p| generator.block_at(p).ok()),
                LoadedSpawn::Ready
            ) {
                return Some(Located {
                    target: LocateTarget::Cave(kind),
                    label: None,
                    position: [x, spawn.ground[1] + 1, z],
                    spawn: Some(spawn),
                });
            }
        }
        None
    };
    if let Some(found) = try_bank(origin[0], origin[1]) {
        return Some(found);
    }

    // The generator plans chambers in 192-block cells. Sampling their true
    // centres also finds narrow chambers missed by the surface-biome grid.
    const CELL: i64 = 192;
    let cell = origin.map(|v| v.div_euclid(CELL));
    let mut visited = std::collections::HashSet::new();
    let mut submerged = None;
    for ring in 0..=radius / CELL + 1 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                if std::time::Instant::now() >= deadline {
                    return submerged;
                }
                let [x, z] = [
                    (cell[0] + dx) * CELL + CELL / 2,
                    (cell[1] + dz) * CELL + CELL / 2,
                ];
                if x.abs() > 990_000 + CELL || z.abs() > 990_000 + CELL {
                    continue;
                }
                let plan = terrain.cave_room_at(x as f64, z as f64);
                let center = [plan.center[0].floor() as i64, plan.center[2].floor() as i64];
                if plan.kind != kind || !visited.insert(center) {
                    continue;
                }
                if submerged.is_none() {
                    let [cx, cz] = center;
                    let center_room = terrain.cave_room_at(cx as f64 + 0.5, cz as f64 + 0.5);
                    let y = ((center_room.floor + center_room.ceiling) * 0.5).floor() as i64;
                    let feet = [cx, y, cz];
                    if center_room.active()
                        && center_room.kind == kind
                        && center_room.water.is_some()
                        && (y + 3) as f64 <= center_room.ceiling
                        && cx.abs().max(cz.abs()) <= 990_000
                        && (cx - origin[0]).pow(2) + (cz - origin[1]).pow(2) <= radius.pow(2)
                        && creative_clearance(terrain, &journal, feet)
                    {
                        submerged = Some(Located {
                            target: LocateTarget::Cave(kind),
                            label: None,
                            position: feet,
                            spawn: None,
                        });
                    }
                }
                for fraction in [0., 0.32, 0.58, 0.78] {
                    let step = (plan.radius * fraction).round() as i64;
                    for [sx, sz] in [
                        [1, 0],
                        [-1, 0],
                        [0, 1],
                        [0, -1],
                        [1, 1],
                        [-1, 1],
                        [1, -1],
                        [-1, -1],
                    ] {
                        if let Some(found) = try_bank(center[0] + sx * step, center[1] + sz * step)
                        {
                            return Some(found);
                        }
                        if step == 0 {
                            break;
                        }
                    }
                }
            }
        }
    }
    submerged
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_cave_commands_find_dry_chambers_and_recheck_edits_below_the_surface() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            geology_revision: 4,
            cave_structure_revision: 12,
            ..Default::default()
        })
        .unwrap();
        for &(slug, target) in crate::protocol::commands::BIOMES {
            let super::LocateTarget::Cave(kind) = target else {
                continue;
            };
            let target = super::super::parse_command(&format!("/locate biome {slug}"))
                .unwrap()
                .unwrap();
            assert_eq!(target, LocateTarget::Cave(kind));
            let found = find(&terrain, [0, 0], kind).unwrap_or_else(|| panic!("missing {slug}"));
            let spawn = found.spawn.unwrap();
            let [x, y, z] = found.position;
            let room = terrain.cave_room_at(x as f64 + 0.5, z as f64 + 0.5);
            assert!(room.active() && room.kind == kind && room.ceiling > y as f64);
            assert!(y as f64 <= terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5));
            let mut journal = EditJournal::default();
            assert!(clear(&terrain, &journal, spawn));
            journal
                .record(found.position, MaterialId::Stone as u16)
                .unwrap();
            assert!(!clear(&terrain, &journal, spawn));
        }
    }
}
