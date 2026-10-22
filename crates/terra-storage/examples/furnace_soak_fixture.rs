//! Offline saved fixture for a native, integrated furnace/smoke soak.
//!
//! `cargo run --locked -p terra-storage --example furnace_soak_fixture --
//! /absolute/copied/configs /absolute/new/fixture [seed]`
//! The output must not exist. Source configs and production worlds are never changed.
//! Player saves have no position field: the sealed room encloses the real world spawn.
//! This validates geometry/storage/fuel offline, not rendering or runtime ventilation.
use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};
use terra_core::{
    Config, GameMode, Inventory, ItemKind, ItemStack, MineralItem, PlayerProfile, PlayerStats,
    ServerConfig, WorldConfig, WorldSeed, furnace,
};
use terra_storage::WorldLibrary;
use terra_world::{
    TerraGenerator,
    edits::{BlockObjectState, EditJournal},
    spawn::{WorldSpawn, find_land_spawn},
};

type Cell = [i64; 3];
const DEFAULT_SEED: u64 = 2_262_437_629_133_911_810;
const RADIUS: i64 = 4;
const HEIGHT: i64 = 7;
const RAW: ItemKind = ItemKind::Mineral(MineralItem::RawIron);
const COAL: ItemKind = ItemKind::Mineral(MineralItem::Coal);

fn copy_configs(source: &Path, target: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if kind.is_symlink() {
            return Err(format!(
                "Config fixture refuses symbolic link: {}",
                entry.path().display()
            )
            .into());
        } else if kind.is_dir() {
            copy_configs(&entry.path(), &destination)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(format!("Unsupported config entry: {}", entry.path().display()).into());
        }
    }
    Ok(())
}

fn bounds(spawn: WorldSpawn) -> (Cell, Cell) {
    let [x, y, z] = spawn.ground;
    (
        [x - RADIUS, y, z - RADIUS],
        [x + RADIUS, y + HEIGHT, z + RADIUS],
    )
}

fn build_room(spawn: WorldSpawn, seed: u64) -> Result<(EditJournal, Cell), String> {
    let (low, high) = bounds(spawn);
    let mut journal = EditJournal::default();
    for x in low[0]..=high[0] {
        for y in low[1] - 1..=high[1] {
            for z in low[2]..=high[2] {
                let shell = x == low[0]
                    || x == high[0]
                    || z == low[2]
                    || z == high[2]
                    || y <= low[1]
                    || y == high[1];
                journal
                    .record([x, y, z], if shell { 3 } else { 0 })
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    let anchor = [spawn.ground[0], spawn.ground[1] + 1, spawn.ground[2] - 3];
    let upper = [anchor[0], anchor[1] + 1, anchor[2]];
    for (cell, peer, secondary) in [(anchor, upper, false), (upper, anchor, true)] {
        journal
            .record(cell, 59)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(
            cell,
            Some(BlockObjectState {
                facing: 0,
                peer: Some(peer),
                secondary,
                ..Default::default()
            }),
        )?;
    }
    let mut inventory = Inventory {
        slots: vec![ItemStack::EMPTY; furnace::SLOTS],
        ..Default::default()
    };
    inventory.slots[furnace::INPUT] = ItemStack {
        item: RAW,
        count: terra_core::STACK_LIMIT,
    };
    inventory.slots[furnace::FUEL] = ItemStack {
        item: COAL,
        count: terra_core::STACK_LIMIT,
    };
    let mut state = furnace::FurnaceState::default();
    // Ignition consumes real fuel through the same rule as the authority.
    furnace::advance(&mut state, &mut inventory, 0.1, seed);
    state.outlet = [anchor[0], anchor[1] + 2, anchor[2]];
    if state.burn_remaining <= 0.0 || !state.smoke_cells.is_empty() {
        return Err("Fixture could not ignite its real furnace recipe".into());
    }
    let mut test_state = state.clone();
    let mut test_inventory = inventory.clone();
    for _ in 0..900 {
        furnace::advance(&mut test_state, &mut test_inventory, 1.0, seed);
        if test_state.burn_remaining <= 0.0 {
            return Err("Fixture input/fuel does not keep the furnace lit for 15 minutes".into());
        }
    }
    journal.set_container(anchor, inventory)?;
    journal.set_furnace(anchor, state)?;
    Ok((journal, anchor))
}

fn validate_room(journal: &EditJournal, spawn: WorldSpawn, anchor: Cell) -> Result<usize, String> {
    let (low, high) = bounds(spawn);
    let mut air = HashSet::new();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let cell = [x, y, z];
                let shell = x == low[0]
                    || x == high[0]
                    || y == low[1]
                    || y == high[1]
                    || z == low[2]
                    || z == high[2];
                match journal.block(cell) {
                    Some(3) if shell => {}
                    Some(0) if !shell => {
                        air.insert(cell);
                    }
                    Some(59) if cell == anchor || cell == [anchor[0], anchor[1] + 1, anchor[2]] => {
                    }
                    _ => {
                        return Err(format!(
                            "Fixture has a hole or invalid room cell at {cell:?}"
                        ));
                    }
                }
            }
        }
    }
    let start = [spawn.ground[0], spawn.ground[1] + 1, spawn.ground[2]];
    let mut visited = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        for direction in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let next = std::array::from_fn(|axis| cell[axis] + direction[axis]);
            if air.contains(&next) && visited.insert(next) {
                queue.push_back(next);
            }
        }
    }
    if visited != air || air.len() <= furnace::MAX_SMOKE_CELLS {
        return Err(
            "Fixture must have one sealed, navigable air region larger than the smoke cap".into(),
        );
    }
    Ok(air.len())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&arguments.len()) {
        return Err(
            "Usage: furnace_soak_fixture <absolute configs> <fresh absolute output> [seed]".into(),
        );
    }
    let configs = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    let seed = arguments
        .get(2)
        .map(|value| {
            value
                .to_str()
                .ok_or("Seed must be UTF-8")?
                .parse::<u64>()
                .map_err(|_| "Invalid seed")
        })
        .transpose()?
        .unwrap_or(DEFAULT_SEED);
    if !configs.is_absolute() || !output.is_absolute() {
        return Err("Config and output paths must be absolute".into());
    }
    let source = fs::canonicalize(configs)?;
    let parent = fs::canonicalize(output.parent().ok_or("Output needs a parent directory")?)?;
    let output = parent.join(output.file_name().ok_or("Output needs a directory name")?);
    if output.exists()
        || output.starts_with(&source)
        || output
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return Err("Output must be a fresh directory outside source configs, without '..'".into());
    }
    let mut template = WorldConfig::load(source.join("world.toml"))?;
    template.seed = WorldSeed(seed);
    let profile = PlayerProfile::load(source.join("profile.toml"))?;
    let mut server = ServerConfig::load(source.join("server.toml"))?;
    let terrain = TerraGenerator::new(template.clone())?;
    let original_spawn = find_land_spawn(&terrain, &EditJournal::default(), [8, 24])?;
    let (mut journal, anchor) = build_room(original_spawn, seed)?;
    let air_cells = validate_room(&journal, original_spawn, anchor)?;
    let spawn = find_land_spawn(&terrain, &journal, [8, 24])?;
    let (low, high) = bounds(original_spawn);
    if !(0..3).all(|axis| spawn.ground[axis] >= low[axis] && spawn.ground[axis] < high[axis])
        || !spawn.is_valid(&terrain, &journal)
    {
        return Err("Saved geometry moved the real server spawn outside the test room".into());
    }
    journal
        .gameplay_mut()
        .inventories
        .insert(profile.uuid, Inventory::default());
    journal.gameplay_mut().player_stats.insert(
        profile.uuid,
        PlayerStats {
            game_mode: Some(GameMode::Creative),
            ..Default::default()
        },
    );
    fs::create_dir(&output)?;
    let root = fs::canonicalize(output)?;
    let configs = root.join("configs");
    copy_configs(&source, &configs)?;
    let library = WorldLibrary {
        root: root.join("library"),
    };
    let mut entry = library.create(
        "Furnace Smoke Soak",
        Some(seed),
        GameMode::Creative,
        &template,
    )?;
    template.id = entry.manifest.world.id;
    entry.manifest.world = template.clone();
    library.save(&entry)?;
    let world = library.path(template.id);
    server.game_mode = GameMode::Creative;
    server.save(configs.join("server.toml"))?;
    template.save(configs.join("world.toml"))?;
    terra_storage::save_edits(&world, template.id, &journal)?;
    let loaded = terra_storage::load_edits(&world, template.id)?;
    if loaded.gameplay() != journal.gameplay()
        || loaded.furnace_at(anchor) != journal.furnace_at(anchor)
        || loaded.container_at(anchor) != journal.container_at(anchor)
        || validate_room(&loaded, original_spawn, anchor)? != air_cells
    {
        return Err("Fixture save roundtrip changed the furnace, player or room".into());
    }
    println!(
        "Fixture: air_cells={air_cells}, smoke_cap={}, furnace={anchor:?}, spawn={:?}",
        furnace::MAX_SMOKE_CELLS,
        spawn.ground
    );
    println!("Initial fuel stack was 128; normal ignition consumed one coal. No smoke prefill.");
    println!("TERRA_CONFIG_DIR={}", configs.display());
    println!("TERRA_WORLD_DIR={}", library.root.display());
    println!("TERRA_BENCH_WORLD_ID={}", template.id.0);
    println!("TERRA_BENCH_SAVE_DIR={}", world.display());
    println!(
        "TERRA_BENCH_CAMERA={},{},{}",
        spawn.feet()[0],
        spawn.feet()[1] + 1.63,
        spawn.feet()[2]
    );
    println!(
        "TERRA_BENCH_LOOK_AT={},{},{}",
        anchor[0] as f64 + 0.5,
        anchor[1] as f64 + 1.2,
        anchor[2] as f64 + 0.5
    );
    println!(
        "TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_SECONDS=900 TERRA_SOAK_REPORT=1"
    );
    println!(
        "Load with --smoke-test-benchmark. Runtime smoke/ventilation and 15-minute GPU stability remain native test work."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_room_exceeds_smoke_cap_and_contains_a_real_burning_furnace() {
        for ground in [[520, 9, 536], [-17, -20, -33]] {
            let spawn = WorldSpawn { ground };
            let (journal, anchor) = build_room(spawn, DEFAULT_SEED).unwrap();
            assert_eq!(validate_room(&journal, spawn, anchor).unwrap(), 292);
            let state = journal.furnace_at(anchor).unwrap();
            assert!(state.burn_remaining > 0.0 && state.smoke_cells.is_empty());
            let inventory = journal.container_at(anchor).unwrap();
            assert_eq!(inventory.slot(furnace::INPUT).unwrap().count, 128);
            assert_eq!(inventory.slot(furnace::FUEL).unwrap().count, 127);
            assert_eq!(
                journal.object_state(anchor).unwrap().peer,
                Some([anchor[0], anchor[1] + 1, anchor[2]])
            );
            assert!(journal.gameplay().valid());
        }
    }

    #[test]
    fn validation_rejects_a_roof_hole() {
        let spawn = WorldSpawn {
            ground: [520, 9, 536],
        };
        let (mut journal, anchor) = build_room(spawn, DEFAULT_SEED).unwrap();
        journal.record([520, 16, 536], 0).unwrap();
        assert!(validate_room(&journal, spawn, anchor).is_err());
    }
}
