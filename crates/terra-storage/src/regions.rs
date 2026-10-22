//! Versioned binary region/player files with a replayable multi-file commit.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::Path,
};
use terra_core::{GameplayState, Inventory, PlayerStats, PlayerUuid, WorldDrop, WorldId};
use terra_world::edits::{EditJournal, SectionDelta};

const MAX_FILE: u64 = 64 * 1024 * 1024;
const TRANSACTION: &str = ".save-transaction";
const INDEX: &str = "level.data";
const MAGIC: &[u8; 8] = b"TERAMIC\0";
mod creature_legacy;

#[derive(Serialize, Deserialize)]
struct Index {
    world: WorldId,
    regions: Vec<[i64; 2]>,
    players: Vec<PlayerUuid>,
    next_drop: u64,
    revision: u64,
}
#[derive(Serialize, Deserialize)]
struct TimedIndex {
    index: Index,
    clock: Option<terra_core::WorldClock>,
    ecology_seconds: f64,
    weather: Option<terra_core::WorldWeather>,
}
#[derive(Serialize, Deserialize)]
struct Weather4 {
    sequence: u64,
    elapsed_seconds: f64,
    elapsed_total: f64,
    duration_seconds: f64,
    transition_seconds: f64,
    from_intensity: f32,
    target_intensity: f32,
    from_wind: [f32; 2],
    target_wind: [f32; 2],
    wetness: f32,
}
impl From<Weather4> for terra_core::WorldWeather {
    fn from(old: Weather4) -> Self {
        Self {
            sequence: old.sequence,
            elapsed_seconds: old.elapsed_seconds,
            elapsed_total: old.elapsed_total,
            duration_seconds: old.duration_seconds,
            transition_seconds: old.transition_seconds,
            from_intensity: old.from_intensity,
            target_intensity: old.target_intensity,
            from_wind: old.from_wind,
            target_wind: old.target_wind,
            wetness: old.wetness,
            pattern: terra_core::WeatherPattern::Climate,
            snow_level: 1,
            latest_lightning: None,
        }
    }
}
// Bincode structs have no framing: the nested prior fields have exactly the
// IDX5 weather field order, followed by its original pattern discriminant.
#[derive(Serialize, Deserialize)]
struct Weather5 {
    prior: Weather4,
    pattern: terra_core::WeatherPattern,
}
impl From<Weather5> for terra_core::WorldWeather {
    fn from(old: Weather5) -> Self {
        Self {
            pattern: old.pattern,
            ..old.prior.into()
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Index5 {
    index: Index,
    clock: Option<terra_core::WorldClock>,
    ecology_seconds: f64,
    weather: Option<Weather5>,
}
#[derive(Serialize, Deserialize)]
struct Index4 {
    index: Index,
    clock: Option<terra_core::WorldClock>,
    ecology_seconds: f64,
    weather: Option<Weather4>,
}
#[derive(Serialize, Deserialize)]
struct Index3 {
    index: Index,
    clock: Option<terra_core::WorldClock>,
    ecology_seconds: f64,
}
#[derive(Serialize, Deserialize)]
struct Index2 {
    index: Index,
    clock: Option<terra_core::WorldClock>,
}
fn read_index(root: &Path) -> Result<TimedIndex, String> {
    let bytes = read(&root.join(INDEX))?;
    let timed: TimedIndex = if bytes.get(12..16) == Some(b"IDX6") {
        decode(b"IDX6", &bytes)?
    } else if bytes.get(12..16) == Some(b"IDX5") {
        let old: Index5 = decode(b"IDX5", &bytes)?;
        TimedIndex {
            index: old.index,
            clock: old.clock,
            ecology_seconds: old.ecology_seconds,
            weather: old.weather.map(Into::into),
        }
    } else if bytes.get(12..16) == Some(b"IDX4") {
        let old: Index4 = decode(b"IDX4", &bytes)?;
        TimedIndex {
            index: old.index,
            clock: old.clock,
            ecology_seconds: old.ecology_seconds,
            weather: old.weather.map(Into::into),
        }
    } else if bytes.get(12..16) == Some(b"IDX3") {
        let old: Index3 = decode(b"IDX3", &bytes)?;
        TimedIndex {
            index: old.index,
            clock: old.clock,
            ecology_seconds: old.ecology_seconds,
            weather: None,
        }
    } else if bytes.get(12..16) == Some(b"IDX2") {
        let old: Index2 = decode(b"IDX2", &bytes)?;
        TimedIndex {
            index: old.index,
            clock: old.clock,
            ecology_seconds: 0.0,
            weather: None,
        }
    } else {
        TimedIndex {
            index: decode(b"INDX", &bytes)?,
            clock: None,
            ecology_seconds: 0.0,
            weather: None,
        }
    };
    if timed.clock.is_some_and(|clock| !clock.valid())
        || timed.weather.is_some_and(|weather| !weather.valid())
        || !timed.ecology_seconds.is_finite()
        || timed.ecology_seconds < 0.0
    {
        return Err("Invalid saved world clock or weather".into());
    }
    Ok(timed)
}
#[derive(Serialize, Deserialize)]
struct Region {
    world: WorldId,
    position: [i64; 2],
    sections: Vec<SectionDelta>,
    drops: Vec<WorldDrop>,
    containers: Vec<([i64; 3], Inventory)>,
    furniture_order: Vec<([i64; 3], u64)>,
}
#[derive(Serialize, Deserialize)]
struct CreatureRegion {
    region: Region,
    creatures: terra_core::creatures::CreaturePopulation,
}
#[derive(Serialize, Deserialize)]
struct FurnaceRegion {
    prior: CreatureRegion,
    furnaces: Vec<([i64; 3], terra_core::furnace::FurnaceState)>,
}
fn read_creature_region(bytes: &[u8]) -> Result<CreatureRegion, String> {
    if bytes.get(12..16) == Some(b"RGA0") {
        return decode::<FurnaceRegion>(b"RGA0", bytes).map(|data| data.prior);
    }
    if bytes.get(12..16) == Some(b"REG9") {
        return decode(b"REG9", bytes);
    }
    if bytes.get(12..16) == Some(b"REG8") {
        let old: Reg8 = decode(b"REG8", bytes)?;
        return Ok(CreatureRegion {
            region: old.region,
            creatures: terra_core::creatures::CreaturePopulation {
                animals: old.creatures.animals,
                explored: old.creatures.explored,
                ecology: old.creatures.ecology,
                ..Default::default()
            },
        });
    }
    if bytes.get(12..16) == Some(b"REG7") {
        let previous: Reg7 = decode(b"REG7", bytes)?;
        Ok(CreatureRegion {
            region: previous.region,
            creatures: terra_core::creatures::CreaturePopulation {
                animals: previous.creatures.animals,
                explored: previous.creatures.explored,
                ..Default::default()
            },
        })
    } else {
        creature_legacy::migrate(decode(b"REG6", bytes)?)
    }
}
#[derive(Serialize, Deserialize)]
struct Population7 {
    animals: BTreeMap<u64, terra_core::creatures::Creature>,
    explored: BTreeSet<[i64; 2]>,
}
#[derive(Serialize, Deserialize)]
struct Population8 {
    animals: BTreeMap<u64, terra_core::creatures::Creature>,
    explored: BTreeSet<[i64; 2]>,
    ecology: terra_core::creatures::ecology::Ecology,
}
#[derive(Serialize, Deserialize)]
struct Reg8 {
    region: Region,
    creatures: Population8,
}
#[derive(Serialize, Deserialize)]
struct Reg7 {
    region: Region,
    creatures: Population7,
}
#[derive(Serialize, Deserialize)]
struct ContainerRegion {
    world: WorldId,
    position: [i64; 2],
    sections: Vec<SectionDelta>,
    drops: Vec<WorldDrop>,
    containers: Vec<([i64; 3], Inventory)>,
}
#[derive(Serialize, Deserialize)]
struct WoodRegion {
    world: WorldId,
    position: [i64; 2],
    sections: Vec<SectionDelta>,
    drops: Vec<WorldDrop>,
}
#[derive(Serialize, Deserialize)]
struct LegacySectionDelta {
    position: terra_voxel::SectionPos,
    revision: u64,
    prop_epoch: u64,
    appearance_epoch: u64,
    fluid_epoch: u64,
    blocks: Vec<([u8; 3], u16)>,
    removed_props: Vec<terra_world::props::PropId>,
    placed_props: Vec<terra_world::props::WorldProp>,
    conditions: Vec<([u8; 3], terra_world::environment::SurfaceCondition)>,
    fluids: Vec<([u8; 3], terra_world::water::FluidCell)>,
}
#[derive(Serialize, Deserialize)]
struct LegacyRegion {
    world: WorldId,
    position: [i64; 2],
    sections: Vec<LegacySectionDelta>,
    drops: Vec<WorldDrop>,
}
#[derive(Serialize, Deserialize)]
struct PreviousObjectState {
    facing: u8,
    peer: Option<[i64; 3]>,
    secondary: bool,
}
#[derive(Serialize, Deserialize)]
struct PreviousSectionDelta {
    position: terra_voxel::SectionPos,
    revision: u64,
    prop_epoch: u64,
    appearance_epoch: u64,
    fluid_epoch: u64,
    blocks: Vec<([u8; 3], u16)>,
    removed_props: Vec<terra_world::props::PropId>,
    placed_props: Vec<terra_world::props::WorldProp>,
    conditions: Vec<([u8; 3], terra_world::environment::SurfaceCondition)>,
    fluids: Vec<([u8; 3], terra_world::water::FluidCell)>,
    objects: Vec<([u8; 3], PreviousObjectState)>,
}
#[derive(Serialize, Deserialize)]
struct PreviousRegion {
    world: WorldId,
    position: [i64; 2],
    sections: Vec<PreviousSectionDelta>,
    drops: Vec<WorldDrop>,
}
fn read_region(bytes: &[u8]) -> Result<Region, String> {
    if matches!(
        bytes.get(12..16),
        Some(b"REG6" | b"REG7" | b"REG8" | b"REG9" | b"RGA0")
    ) {
        return read_creature_region(bytes).map(|data| data.region);
    }
    if bytes.get(12..16) == Some(b"REG5") {
        return decode(b"REG5", bytes);
    }
    if bytes.get(12..16) == Some(b"REG4") {
        let old: ContainerRegion = decode(b"REG4", bytes)?;
        return Ok(Region {
            world: old.world,
            position: old.position,
            sections: old.sections,
            drops: old.drops,
            containers: old.containers,
            furniture_order: Vec::new(),
        });
    }
    if bytes.get(12..16) == Some(b"REG3") {
        let old: WoodRegion = decode(b"REG3", bytes)?;
        return Ok(Region {
            world: old.world,
            position: old.position,
            sections: old.sections,
            drops: old.drops,
            containers: Vec::new(),
            furniture_order: Vec::new(),
        });
    }
    if bytes.get(12..16) == Some(b"REG2") {
        let old: PreviousRegion = decode(b"REG2", bytes)?;
        return Ok(Region {
            world: old.world,
            position: old.position,
            drops: old.drops,
            containers: Vec::new(),
            furniture_order: Vec::new(),
            sections: old
                .sections
                .into_iter()
                .map(|s| SectionDelta {
                    position: s.position,
                    revision: s.revision,
                    prop_epoch: s.prop_epoch,
                    appearance_epoch: s.appearance_epoch,
                    fluid_epoch: s.fluid_epoch,
                    blocks: s.blocks,
                    removed_props: s.removed_props,
                    placed_props: s.placed_props,
                    conditions: s.conditions,
                    fluids: s.fluids,
                    objects: s
                        .objects
                        .into_iter()
                        .map(|(p, s)| {
                            (
                                p,
                                terra_world::edits::BlockObjectState {
                                    facing: s.facing,
                                    peer: s.peer,
                                    secondary: s.secondary,
                                    wood: terra_core::WoodSpecies::Oak,
                                },
                            )
                        })
                        .collect(),
                })
                .collect(),
        });
    }
    let old: LegacyRegion = decode(b"REGN", bytes)?;
    Ok(Region {
        world: old.world,
        position: old.position,
        drops: old.drops,
        containers: Vec::new(),
        furniture_order: Vec::new(),
        sections: old
            .sections
            .into_iter()
            .map(|s| SectionDelta {
                position: s.position,
                revision: s.revision,
                prop_epoch: s.prop_epoch,
                appearance_epoch: s.appearance_epoch,
                fluid_epoch: s.fluid_epoch,
                blocks: s.blocks,
                removed_props: s.removed_props,
                placed_props: s.placed_props,
                conditions: s.conditions,
                fluids: s.fluids,
                objects: Vec::new(),
            })
            .collect(),
    })
}
#[derive(Serialize, Deserialize)]
struct PlayerData {
    world: WorldId,
    player: PlayerUuid,
    inventory: Inventory,
    stats: PlayerStats,
    has_inventory: bool,
    has_stats: bool,
}
#[derive(Serialize, Deserialize)]
struct LegacyPlayerStats {
    blocks_broken: u64,
    blocks_placed: u64,
    props_harvested: u64,
}
#[derive(Serialize, Deserialize)]
struct LegacyPlayerData {
    world: WorldId,
    player: PlayerUuid,
    inventory: Inventory,
    stats: LegacyPlayerStats,
    has_inventory: bool,
    has_stats: bool,
}
fn read_player(bytes: &[u8]) -> Result<PlayerData, String> {
    if bytes.get(12..16) == Some(b"PLY5") {
        let mut data: PlayerData = decode(b"PLY5", bytes)?;
        data.stats
            .survival
            .effects
            .normalize_to_definitions()
            .map_err(str::to_string)?;
        data.stats.survival.effects.set_smoke_exposure(false);
        return Ok(data);
    }
    if bytes.get(12..16) == Some(b"PLY4") {
        let old: FoodPlayerData = decode(b"PLY4", bytes)?;
        return Ok(PlayerData {
            world: old.world,
            player: old.player,
            inventory: old.inventory,
            stats: PlayerStats {
                game_mode: old.stats.game_mode,
                blocks_broken: old.stats.blocks_broken,
                blocks_placed: old.stats.blocks_placed,
                props_harvested: old.stats.props_harvested,
                deaths: old.stats.deaths,
                survival: old.stats.survival.into(),
            },
            has_inventory: old.has_inventory,
            has_stats: old.has_stats,
        });
    }
    if bytes.get(12..16) == Some(b"PLY3") {
        let old: GameModePlayerData = decode(b"PLY3", bytes)?;
        return Ok(PlayerData {
            world: old.world,
            player: old.player,
            inventory: old.inventory,
            stats: PlayerStats {
                blocks_broken: old.stats.blocks_broken,
                blocks_placed: old.stats.blocks_placed,
                props_harvested: old.stats.props_harvested,
                deaths: old.stats.deaths,
                survival: old.stats.survival.into(),
                game_mode: old.stats.game_mode,
            },
            has_inventory: old.has_inventory,
            has_stats: old.has_stats,
        });
    }
    if bytes.get(12..16) == Some(b"PLY2") {
        let old: SurvivalPlayerData = decode(b"PLY2", bytes)?;
        return Ok(PlayerData {
            world: old.world,
            player: old.player,
            inventory: old.inventory,
            stats: PlayerStats {
                blocks_broken: old.stats.blocks_broken,
                blocks_placed: old.stats.blocks_placed,
                props_harvested: old.stats.props_harvested,
                deaths: old.stats.deaths,
                survival: old.stats.survival.into(),
                game_mode: None,
            },
            has_inventory: old.has_inventory,
            has_stats: old.has_stats,
        });
    }
    let old: LegacyPlayerData = decode(b"PLYR", bytes)?;
    Ok(PlayerData {
        world: old.world,
        player: old.player,
        inventory: old.inventory,
        stats: PlayerStats {
            blocks_broken: old.stats.blocks_broken,
            blocks_placed: old.stats.blocks_placed,
            props_harvested: old.stats.props_harvested,
            ..Default::default()
        },
        has_inventory: old.has_inventory,
        has_stats: old.has_stats,
    })
}

// Keep the exact positional PLY4 layout: serde defaults cannot migrate postcard structs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct FoodSurvivalVitals {
    health: f32,
    stamina: f32,
    hunger: f32,
    oxygen: f32,
    exhausted: bool,
    stamina_delay: f32,
    recovery_delay: f32,
    last_damage: Option<terra_core::DamageCause>,
    saturation: f32,
    regeneration_seconds: f32,
    regeneration_rate: f32,
}

impl From<FoodSurvivalVitals> for terra_core::SurvivalVitals {
    fn from(old: FoodSurvivalVitals) -> Self {
        Self {
            health: old.health,
            stamina: old.stamina,
            hunger: old.hunger,
            oxygen: old.oxygen,
            exhausted: old.exhausted,
            stamina_delay: old.stamina_delay,
            recovery_delay: old.recovery_delay,
            last_damage: old.last_damage,
            saturation: old.saturation,
            regeneration_seconds: old.regeneration_seconds,
            regeneration_rate: old.regeneration_rate,
            effects: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct FoodPlayerStats {
    game_mode: Option<terra_core::GameMode>,
    blocks_broken: u64,
    blocks_placed: u64,
    props_harvested: u64,
    deaths: u64,
    survival: FoodSurvivalVitals,
}

#[derive(Serialize, Deserialize)]
struct FoodPlayerData {
    world: WorldId,
    player: PlayerUuid,
    inventory: Inventory,
    stats: FoodPlayerStats,
    has_inventory: bool,
    has_stats: bool,
}

// Postcard is positional: freeze the PLY2/PLY3 layout before adding food reserves.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct LegacySurvivalVitals {
    health: f32,
    stamina: f32,
    hunger: f32,
    oxygen: f32,
    exhausted: bool,
    stamina_delay: f32,
    recovery_delay: f32,
    last_damage: Option<terra_core::DamageCause>,
}
impl From<LegacySurvivalVitals> for terra_core::SurvivalVitals {
    fn from(old: LegacySurvivalVitals) -> Self {
        Self {
            health: old.health,
            stamina: old.stamina,
            hunger: old.hunger,
            oxygen: old.oxygen,
            exhausted: old.exhausted,
            stamina_delay: old.stamina_delay,
            recovery_delay: old.recovery_delay,
            last_damage: old.last_damage,
            ..Default::default()
        }
    }
}
#[derive(Serialize, Deserialize)]
struct GameModePlayerStats {
    game_mode: Option<terra_core::GameMode>,
    blocks_broken: u64,
    blocks_placed: u64,
    props_harvested: u64,
    deaths: u64,
    survival: LegacySurvivalVitals,
}
#[derive(Serialize, Deserialize)]
struct GameModePlayerData {
    world: WorldId,
    player: PlayerUuid,
    inventory: Inventory,
    stats: GameModePlayerStats,
    has_inventory: bool,
    has_stats: bool,
}

#[derive(Serialize, Deserialize)]
struct SurvivalPlayerStats {
    blocks_broken: u64,
    blocks_placed: u64,
    props_harvested: u64,
    deaths: u64,
    survival: LegacySurvivalVitals,
}
#[derive(Serialize, Deserialize)]
struct SurvivalPlayerData {
    world: WorldId,
    player: PlayerUuid,
    inventory: Inventory,
    stats: SurvivalPlayerStats,
    has_inventory: bool,
    has_stats: bool,
}
#[derive(Serialize, Deserialize)]
struct Commit {
    files: Vec<(String, u64)>,
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn encode<T: Serialize>(kind: &[u8; 4], value: &T) -> Result<Vec<u8>, String> {
    let body = postcard::to_allocvec(value).map_err(|e| e.to_string())?;
    if body.len() as u64 > MAX_FILE - 32 {
        return Err("Save record exceeds 64 MiB".into());
    }
    let mut bytes = Vec::with_capacity(32 + body.len());
    bytes.extend(MAGIC);
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(kind);
    bytes.extend((body.len() as u64).to_le_bytes());
    bytes.extend(checksum(&body).to_le_bytes());
    bytes.extend(body);
    Ok(bytes)
}
fn decode<T: DeserializeOwned>(kind: &[u8; 4], bytes: &[u8]) -> Result<T, String> {
    if bytes.len() < 32
        || &bytes[..8] != MAGIC
        || bytes[8..12] != 1_u32.to_le_bytes()
        || &bytes[12..16] != kind
        || bytes.len() as u64 > MAX_FILE
    {
        return Err("Unsupported or damaged TERAMIC binary save header".into());
    }
    let integer = |start| -> Result<u64, String> {
        let array = bytes[start..start + 8]
            .try_into()
            .map_err(|_| "Invalid save header")?;
        Ok(u64::from_le_bytes(array))
    };
    if integer(16)? != (bytes.len() - 32) as u64 || integer(24)? != checksum(&bytes[32..]) {
        return Err("Save length/checksum mismatch; refusing partial state".into());
    }
    let (value, rest) = postcard::take_from_bytes(&bytes[32..]).map_err(|e| e.to_string())?;
    if !rest.is_empty() {
        return Err("Unexpected trailing save data".into());
    }
    Ok(value)
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_FILE {
        return Err(format!("Unsafe or oversized save file: {}", path.display()));
    }
    fs::read(path).map_err(|e| e.to_string())
}
fn directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Save directories must not be symlinks".into());
    }
    Ok(())
}
fn sync(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Save path needs a parent")?;
    directory(parent)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}
fn safe_relative(name: &str) -> bool {
    if name == INDEX {
        return true;
    }
    let parts: Vec<_> = name.split('/').collect();
    parts.len() == 2
        && matches!(parts[0], "regions" | "players")
        && !parts[1].is_empty()
        && !matches!(parts[1], "." | "..")
        && parts[1]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && ((parts[0] == "regions" && parts[1].ends_with(".region"))
            || (parts[0] == "players" && parts[1].ends_with(".data")))
}
fn recover(root: &Path) -> Result<(), String> {
    let pending = root.join(TRANSACTION);
    if !pending.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    directory(&pending)?;
    let plan = pending.join("commit.data");
    for child in ["regions", "players"] {
        let path = pending.join(child);
        if path.try_exists().map_err(|e| e.to_string())? {
            directory(&path)?;
        }
    }
    if !plan.try_exists().map_err(|e| e.to_string())? {
        return fs::remove_dir_all(pending).map_err(|e| e.to_string());
    }
    let commit: Commit = decode(b"CMIT", &read(&plan)?)?;
    let mut seen = BTreeSet::new();
    if commit.files.len() > 100_000 || commit.files.last().is_none_or(|(p, _)| p != INDEX) {
        return Err("Invalid save transaction index".into());
    }
    // Verify the complete transaction before moving any of its remaining files.
    for (name, expected) in &commit.files {
        if !safe_relative(name) || !seen.insert(name) {
            return Err("Unsafe save transaction path".into());
        }
        let staged = pending.join(name);
        let path = if staged.try_exists().map_err(|e| e.to_string())? {
            staged
        } else {
            root.join(name)
        };
        if checksum(&read(&path)?) != *expected {
            return Err("Incomplete or corrupt save transaction".into());
        }
    }
    for (name, _) in &commit.files {
        let staged = pending.join(name);
        if staged.try_exists().map_err(|e| e.to_string())? {
            let destination = root.join(name);
            let parent = destination.parent().ok_or("Invalid save destination")?;
            directory(parent)?;
            fs::rename(staged, &destination).map_err(|e| e.to_string())?;
            sync(parent)?;
        }
    }
    sync(root)?;
    fs::remove_dir_all(pending).map_err(|e| e.to_string())?;
    sync(root)
}
fn region_name([x, z]: [i64; 2]) -> String {
    format!("regions/r.{x}.{z}.region")
}
fn reject_orphans(root: &Path) -> Result<(), String> {
    if !root.join(INDEX).try_exists().map_err(|e| e.to_string())? {
        for child in ["regions", "players"] {
            if fs::read_dir(root.join(child))
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err("Binary save index is missing; refusing to load or overwrite orphaned world/player data".into());
            }
        }
    }
    Ok(())
}
fn region_at(position: [i64; 3]) -> [i64; 2] {
    [position[0].div_euclid(512), position[2].div_euclid(512)]
}

fn commit(root: &Path, world: WorldId, journal: &EditJournal) -> Result<(), String> {
    if journal.clock.is_some_and(|clock| !clock.valid()) {
        return Err("Cannot save invalid world clock".into());
    }
    if journal.weather.is_some_and(|weather| !weather.valid()) {
        return Err("Cannot save invalid world weather".into());
    }
    if !journal.gameplay().valid() {
        return Err("Cannot save invalid gameplay state".into());
    }
    let mut regions: BTreeMap<[i64; 2], Region> = BTreeMap::new();
    let new_region = |position| Region {
        world,
        position,
        sections: Vec::new(),
        drops: Vec::new(),
        containers: Vec::new(),
        furniture_order: Vec::new(),
    };
    let mut sections: Vec<_> = journal.changed_sections().collect();
    sections.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    for p in sections {
        let position = [p.x.div_euclid(32), p.z.div_euclid(32)];
        regions
            .entry(position)
            .or_insert_with(|| new_region(position))
            .sections
            .push(journal.section_delta(p));
    }
    let gameplay = journal.gameplay();
    let mut populations: BTreeMap<[i64; 2], terra_core::creatures::CreaturePopulation> =
        BTreeMap::new();
    for home in &gameplay.creatures.explored {
        let owner = [home[0].div_euclid(16), home[1].div_euclid(16)];
        populations.entry(owner).or_default().explored.insert(*home);
        regions.entry(owner).or_insert_with(|| new_region(owner));
    }
    for (&id, animal) in &gameplay.creatures.animals {
        let owner = [animal.home[0].div_euclid(16), animal.home[1].div_euclid(16)];
        regions.entry(owner).or_insert_with(|| new_region(owner));
        populations
            .entry(owner)
            .or_default()
            .animals
            .insert(id, animal.clone());
        if let Some(&player) = gameplay.creatures.owners.get(&id) {
            populations
                .entry(owner)
                .or_default()
                .owners
                .insert(id, player);
        }
    }
    let ecology = &gameplay.creatures.ecology;
    for (&id, flower) in &ecology.flowers {
        let owner = region_at(flower.position.map(|v| v.floor() as i64));
        regions.entry(owner).or_insert_with(|| new_region(owner));
        populations
            .entry(owner)
            .or_default()
            .ecology
            .flowers
            .insert(id, flower.clone());
    }
    for (&id, nest) in &ecology.nests {
        let owner = region_at(nest.position.map(|v| v.floor() as i64));
        regions.entry(owner).or_insert_with(|| new_region(owner));
        populations
            .entry(owner)
            .or_default()
            .ecology
            .nests
            .insert(id, nest.clone());
    }
    for (&id, bee) in &ecology.bees {
        if let Some(animal) = gameplay.creatures.animals.get(&id) {
            let owner = [animal.home[0].div_euclid(16), animal.home[1].div_euclid(16)];
            populations
                .entry(owner)
                .or_default()
                .ecology
                .bees
                .insert(id, bee.clone());
        }
    }
    for drop in &gameplay.drops {
        let position = region_at(drop.position.map(|v| v.floor() as i64));
        regions
            .entry(position)
            .or_insert_with(|| new_region(position))
            .drops
            .push(drop.clone());
    }
    let mut containers: Vec<_> = journal.containers().collect();
    containers.sort_unstable_by_key(|(position, _)| *position);
    for (position, inventory) in containers {
        if position
            .iter()
            .any(|value| value.unsigned_abs() > 1_000_000)
            || !journal.valid_container(position, inventory)
        {
            return Err("Cannot save invalid or orphaned container contents".into());
        }
        let owner = region_at(position);
        regions
            .entry(owner)
            .or_insert_with(|| new_region(owner))
            .containers
            .push((position, inventory.clone()));
    }
    let mut furnaces: BTreeMap<[i64; 2], Vec<_>> = BTreeMap::new();
    let mut stations: Vec<_> = journal.furnaces().collect();
    stations.sort_unstable_by_key(|(position, _)| *position);
    for (position, state) in stations {
        if !state.valid()
            || journal.block(position) != Some(59)
            || !journal.valid_container(
                position,
                journal
                    .container_at(position)
                    .unwrap_or(&Inventory::default()),
            )
        {
            return Err("Cannot save invalid or orphaned furnace state".into());
        }
        let owner = region_at(position);
        regions.entry(owner).or_insert_with(|| new_region(owner));
        furnaces
            .entry(owner)
            .or_default()
            .push((position, state.clone()));
    }
    let mut furniture_order: Vec<_> = journal.furniture_orders().collect();
    furniture_order.sort_unstable_by_key(|(position, _)| *position);
    for (position, order) in furniture_order {
        if order == 0
            || !journal
                .block(position)
                .is_some_and(|block| matches!(block, 28..=29))
        {
            return Err("Cannot save orphaned furniture ordering".into());
        }
        let owner = region_at(position);
        regions
            .entry(owner)
            .or_insert_with(|| new_region(owner))
            .furniture_order
            .push((position, order));
    }
    let players: BTreeSet<_> = gameplay
        .inventories
        .keys()
        .chain(gameplay.player_stats.keys())
        .copied()
        .collect();
    let index = Index {
        world,
        regions: regions.keys().copied().collect(),
        players: players.iter().copied().collect(),
        next_drop: gameplay.next_drop,
        revision: gameplay.revision,
    };
    let pending = root.join(TRANSACTION);
    directory(&pending)?;
    let mut plan = Commit { files: Vec::new() };
    let mut stage = |name: String, bytes: Vec<u8>| -> Result<(), String> {
        let target = root.join(&name);
        if target.try_exists().map_err(|e| e.to_string())? && read(&target)? == bytes {
            return Ok(());
        }
        write(&pending.join(&name), &bytes)?;
        plan.files.push((name, checksum(&bytes)));
        Ok(())
    };
    for (p, region) in regions {
        let data = FurnaceRegion {
            prior: CreatureRegion {
                region,
                creatures: populations.remove(&p).unwrap_or_default(),
            },
            furnaces: furnaces.remove(&p).unwrap_or_default(),
        };
        stage(region_name(p), encode(b"RGA0", &data)?)?;
    }
    for player in players {
        let mut data = PlayerData {
            world,
            player,
            inventory: gameplay
                .inventories
                .get(&player)
                .cloned()
                .unwrap_or_default(),
            stats: gameplay
                .player_stats
                .get(&player)
                .copied()
                .unwrap_or_default(),
            has_inventory: gameplay.inventories.contains_key(&player),
            has_stats: gameplay.player_stats.contains_key(&player),
        };
        data.stats.survival.effects.set_smoke_exposure(false);
        stage(format!("players/{player}.data"), encode(b"PLY5", &data)?)?;
    }
    let bytes = encode(
        b"IDX6",
        &TimedIndex {
            index,
            clock: journal.clock,
            ecology_seconds: ecology.seconds,
            weather: journal.weather,
        },
    )?;
    // The small index is installed last, even when its contents are unchanged.
    write(&pending.join(INDEX), &bytes)?;
    plan.files.push((INDEX.into(), checksum(&bytes)));
    sync(&pending)?;
    for child in ["regions", "players"] {
        if pending.join(child).exists() {
            sync(&pending.join(child))?;
        }
    }
    write(&pending.join("commit.data"), &encode(b"CMIT", &plan)?)?;
    sync(&pending)?;
    sync(root)?;
    recover(root)
}

fn load(root: &Path, world: WorldId) -> Result<EditJournal, String> {
    let timed = read_index(root)?;
    let index = timed.index;
    if index.world != world || index.regions.len() > 100_000 || index.players.len() > 8192 {
        return Err("Region index belongs to an incompatible world".into());
    }
    let mut journal = EditJournal::default();
    journal.clock = timed.clock;
    journal.weather = timed.weather;
    let mut gameplay = GameplayState {
        next_drop: index.next_drop,
        revision: index.revision,
        ..Default::default()
    };
    gameplay.creatures.ecology.seconds = timed.ecology_seconds;
    let mut owners = BTreeSet::new();
    let mut regions = BTreeSet::new();
    let mut container_positions = BTreeSet::new();
    let mut furniture_positions = BTreeSet::new();
    let mut furniture_orders = BTreeSet::new();
    let mut furnace_positions = BTreeSet::new();
    let mut furnaces = Vec::new();
    for p in index.regions {
        if !regions.insert(p) {
            return Err("Duplicate region index".into());
        }
        let bytes = read(&root.join(region_name(p)))?;
        let region = read_region(&bytes)?;
        if bytes.get(12..16) == Some(b"RGA0") {
            let data: FurnaceRegion = decode(b"RGA0", &bytes)?;
            if data.furnaces.len() > 131_072 {
                return Err("Too many furnace records in region".into());
            }
            for (position, state) in data.furnaces {
                if region_at(position) != p || !furnace_positions.insert(position) {
                    return Err("Duplicate or misplaced furnace state".into());
                }
                furnaces.push((position, state));
            }
        }
        if matches!(
            bytes.get(12..16),
            Some(b"REG6" | b"REG7" | b"REG8" | b"REG9" | b"RGA0")
        ) {
            let mut data = read_creature_region(&bytes)?;
            for animal in data.creatures.animals.values_mut() {
                animal.apply_definition();
            }
            if !data.creatures.valid() {
                return Err("Invalid saved creature population".into());
            }
            for (id, owner) in data.creatures.owners {
                if gameplay.creatures.owners.insert(id, owner).is_some() {
                    return Err("Duplicate animal owner".into());
                }
            }
            let ecology = data.creatures.ecology;
            gameplay.creatures.ecology.seconds =
                gameplay.creatures.ecology.seconds.max(ecology.seconds);
            for (id, f) in ecology.flowers {
                if region_at(f.position.map(|v| v.floor() as i64)) != p
                    || gameplay.creatures.ecology.flowers.insert(id, f).is_some()
                {
                    return Err("Misplaced or duplicate flower state".into());
                }
            }
            for (id, n) in ecology.nests {
                if region_at(n.position.map(|v| v.floor() as i64)) != p
                    || gameplay.creatures.ecology.nests.insert(id, n).is_some()
                {
                    return Err("Misplaced or duplicate nest state".into());
                }
            }
            for (id, b) in ecology.bees {
                if !data.creatures.animals.contains_key(&id)
                    || gameplay.creatures.ecology.bees.insert(id, b).is_some()
                {
                    return Err("Orphaned or duplicate bee state".into());
                }
            }
            for home in data.creatures.explored {
                if [home[0].div_euclid(16), home[1].div_euclid(16)] != p
                    || !gameplay.creatures.explored.insert(home)
                {
                    return Err("Misplaced or duplicate creature habitat".into());
                }
            }
            for (id, animal) in data.creatures.animals {
                if [animal.home[0].div_euclid(16), animal.home[1].div_euclid(16)] != p
                    || gameplay.creatures.animals.insert(id, animal).is_some()
                {
                    return Err("Misplaced or duplicate saved creature identity".into());
                }
            }
        }
        if region.world != world || region.position != p {
            return Err("Region coordinate/world mismatch".into());
        }
        for delta in region.sections {
            let owner = delta.position;
            if [owner.x.div_euclid(32), owner.z.div_euclid(32)] != p
                || !owners.insert((owner.x, owner.y, owner.z))
            {
                return Err("Duplicate or misplaced section in region".into());
            }
            journal.import_delta(delta)?;
        }
        for drop in region.drops {
            if region_at(drop.position.map(|v| v.floor() as i64)) != p {
                return Err("Misplaced resource drop".into());
            }
            gameplay.drops.push(drop);
        }
        if region.containers.len() > 131_072 {
            return Err("Too many container records in region".into());
        }
        for (position, inventory) in region.containers {
            if region_at(position) != p || !container_positions.insert(position) {
                return Err("Duplicate or misplaced container contents".into());
            }
            journal.set_container(position, inventory)?;
        }
        if region.furniture_order.len() > 131_072 {
            return Err("Too many furniture placement records in region".into());
        }
        for (position, order) in region.furniture_order {
            if region_at(position) != p
                || !furniture_positions.insert(position)
                || !furniture_orders.insert(order)
            {
                return Err("Duplicate or misplaced furniture placement record".into());
            }
            journal.import_furniture_order(position, order)?;
        }
    }
    // Multi-block stations may refer to another section; validate after all imports.
    for (position, state) in furnaces {
        journal.set_furnace(position, state)?;
    }
    let mut players = BTreeSet::new();
    for id in index.players {
        if id.is_nil() || !players.insert(id) {
            return Err("Duplicate/invalid player in save".into());
        }
        let data = read_player(&read(&root.join(format!("players/{id}.data")))?)?;
        if data.world != world
            || data.player != id
            || !data.inventory.valid()
            || !data.stats.survival.valid()
        {
            return Err("Invalid player save".into());
        }
        if data.has_inventory {
            gameplay.inventories.insert(id, data.inventory);
        }
        if data.has_stats {
            gameplay.player_stats.insert(id, data.stats);
        }
    }
    gameplay.drops.sort_unstable_by_key(|drop| drop.id);
    journal.import_gameplay(gameplay)?;
    Ok(journal)
}

pub(crate) fn root(path: &Path) -> &Path {
    if path.file_name().is_some_and(|n| n == "edits.toml") {
        path.parent().unwrap_or(Path::new("."))
    } else {
        path
    }
}
pub fn lock_world(directory: &Path) -> Result<fs::File, String> {
    directory_lock(root(directory), "session.lock")
}
fn directory_lock(root: &Path, name: &str) -> Result<fs::File, String> {
    directory(root)?;
    let path = root.join(name);
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Save lock cannot be a symlink".into());
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|e| format!("Save is already open: {e}"))?;
    Ok(lock)
}
pub fn load_edits(path: &Path, world: WorldId) -> Result<EditJournal, String> {
    let root = root(path);
    let _lock = directory_lock(root, ".write.lock")?;
    crate::manifest::protect_metadata(root, world)?;
    directory(&root.join("regions"))?;
    directory(&root.join("players"))?;
    recover(root)?;
    reject_orphans(root)?;
    if root.join(INDEX).try_exists().map_err(|e| e.to_string())? {
        return load(root, world);
    }
    let legacy = root.join("edits.toml");
    if legacy.try_exists().map_err(|e| e.to_string())? {
        if fs::symlink_metadata(&legacy)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Legacy save must not be a symbolic link".into());
        }
        let journal = crate::library::load_legacy(&legacy, world)?;
        commit(root, world, &journal)?;
        let backup = root.join("edits.toml.legacy-backup");
        if !backup.exists() {
            fs::rename(legacy, backup).map_err(|e| e.to_string())?;
            sync(root)?;
        }
        return Ok(journal);
    }
    Ok(EditJournal::default())
}
pub fn save_edits(path: &Path, world: WorldId, journal: &EditJournal) -> Result<(), String> {
    let _scope = terra_core::profiling::ScopeTimer::new("storage.save_edits");
    let root = root(path);
    let _lock = directory_lock(root, ".write.lock")?;
    crate::manifest::protect_metadata(root, world)?;
    directory(&root.join("regions"))?;
    directory(&root.join("players"))?;
    recover(root)?;
    reject_orphans(root)?;
    if root.join(INDEX).exists() {
        let index = read_index(root)?.index;
        if index.world != world {
            return Err("Cannot overwrite another world's region save".into());
        }
    }
    commit(root, world, journal)?;
    if let Some(rules) = journal.world_rules {
        crate::manifest::save_rules(root, rules)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
