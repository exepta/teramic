//! Draft settings for a fresh world; persisted through the existing world library.
use super::model::{MenuDialog, MenuState, WorldCreateTab, WorldMenu};
use bevy::input_focus::{FocusCause, InputFocus, tab_navigation::TabGroup};
use bevy::prelude::*;
use terra_core::{BiomeKind, GameConfig, GameRules, StructureSettings};

pub(super) const BIOMES: [(Option<BiomeKind>, &str, &str, &str); 11] = [
    (None, "random", "Any biome", "dices"),
    (Some(BiomeKind::Plains), "plains", "Plains", "leaf"),
    (Some(BiomeKind::Forest), "forest", "Forest", "trees"),
    (Some(BiomeKind::Desert), "desert", "Desert", "sun"),
    (
        Some(BiomeKind::Highland),
        "highlands",
        "Highlands",
        "mountain",
    ),
    (
        Some(BiomeKind::MushroomPark),
        "mushroom-park",
        "Mushroom Park",
        "mushroom",
    ),
    (
        Some(BiomeKind::ClifftedDesert),
        "cliffted-desert",
        "Cliffted Desert",
        "mountain",
    ),
    (
        Some(BiomeKind::CherryForest),
        "cherry-forest",
        "Cherry Forest",
        "flower",
    ),
    (
        Some(BiomeKind::SpiritBlossom),
        "spirit-blossom",
        "Spirit Blossom",
        "flower",
    ),
    (
        Some(BiomeKind::BloodForest),
        "blood-forest",
        "Blood Forest",
        "trees",
    ),
    (
        Some(BiomeKind::Ocean),
        "ocean",
        "Ocean coast",
        "waves-horizontal",
    ),
];

pub(super) fn reset(world: &mut World) {
    let days = world.resource::<GameConfig>().world.seasons.spring_days;
    let mut form = world.resource_mut::<WorldMenu>();
    form.create_tab = WorldCreateTab::General;
    form.kingdom_rules = terra_core::kingdom::KingdomRules::default();
    form.name.clear();
    form.seed.clear();
    form.mode = "survival".into();
    form.natural_mob_limit = 256;
    form.game_rules = GameRules {
        keep_inventory: false,
        pvp: false,
        ..Default::default()
    };
    form.season_days = days;
    form.preferred_spawn_biome = None;
    form.biome_picker_open = false;
    form.structures = StructureSettings::default();
}

pub(super) fn command(world: &mut World, command: &str) -> Result<bool, String> {
    let mut form = world.resource_mut::<WorldMenu>();
    if let Some(tab) = command.strip_prefix("world-create-tab-") {
        form.create_tab = match tab {
            "general" => WorldCreateTab::General,
            "biomes" => WorldCreateTab::Biomes,
            "kingdom" => WorldCreateTab::Kingdom,
            _ => return Err("Choose a valid world settings tab".into()),
        };
        form.biome_picker_open = false;
    } else if command == "world-biome-picker-open" {
        form.create_tab = WorldCreateTab::Biomes;
        form.biome_picker_open = true;
    } else if command == "world-biome-picker-close" {
        form.biome_picker_open = false;
    } else if let Some(mode) = command.strip_prefix("world-mode-") {
        if !matches!(mode, "survival" | "creative" | "spectator") {
            return Err("Choose a valid game mode".into());
        }
        form.mode = mode.into();
    } else if let Some(biome) = command.strip_prefix("world-biome-") {
        form.preferred_spawn_biome = BIOMES
            .iter()
            .find(|(_, key, _, _)| *key == biome)
            .ok_or("Choose a valid starting biome")?
            .0;
        form.biome_picker_open = false;
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// Keep Tab navigation and editable focus inside the nested biome dialog.
pub(super) fn focus_biome_picker(
    world: &mut World,
    mut previous: Local<Option<(Entity, Option<Entity>)>>,
) {
    let active = world
        .get_resource::<WorldMenu>()
        .is_none_or(|form| form.biome_picker_open)
        && world
            .get_resource::<MenuState>()
            .is_none_or(|menu| menu.dialog == MenuDialog::CreateWorld);
    if !active && previous.is_none() {
        return;
    }
    let Some(focus) = world.get_resource::<InputFocus>().map(InputFocus::get) else {
        return;
    };
    let root = active
        .then(|| {
            world
                .query::<(Entity, &tilt_ui::ElementId)>()
                .iter(world)
                .find_map(|(entity, id)| (id.0 == "world-biome-picker").then_some(entity))
        })
        .flatten();
    let Some(root) = root else {
        if let Some((_, saved)) = previous.take() {
            let saved = saved.filter(|entity| world.get_entity(*entity).is_ok());
            if let Some(saved) = saved {
                world
                    .resource_mut::<InputFocus>()
                    .set(saved, FocusCause::Navigated);
            } else {
                world.resource_mut::<InputFocus>().clear();
            }
        }
        return;
    };
    if previous
        .as_ref()
        .is_none_or(|(old_root, _)| *old_root != root)
    {
        *previous = Some((root, focus));
        world.entity_mut(root).insert(TabGroup::modal());
    }
    let inside = focus.is_some_and(|entity| {
        let mut current = entity;
        while let Some(parent) = world.get::<ChildOf>(current) {
            current = parent.parent();
            if current == root {
                return true;
            }
        }
        false
    });
    if !inside {
        let done = world
            .query::<(Entity, &tilt_ui::ElementId)>()
            .iter(world)
            .find_map(|(entity, id)| (id.0 == "world-biome-picker-close").then_some(entity));
        if let Some(done) = done {
            world
                .resource_mut::<InputFocus>()
                .set(done, FocusCause::Navigated);
        }
    }
}

pub(super) fn change(world: &mut World, control: &str, value: &str) -> Result<bool, String> {
    if !control.starts_with("world-rule-")
        && !control.starts_with("world-kingdom-")
        && !control.starts_with("world-structure")
        && !matches!(
            control,
            "world-sleep-percentage" | "world-time-speed" | "world-season-days"
        )
    {
        return Ok(false);
    }
    if world.resource::<MenuState>().dialog != MenuDialog::CreateWorld {
        return Ok(true);
    }
    let mut form = world.resource_mut::<WorldMenu>();
    if let Some(key) = control.strip_prefix("world-kingdom-") {
        form.kingdom_rules = super::kingdom_settings::change(form.kingdom_rules, key, value)?;
        return Ok(true);
    }
    let flag = || {
        value
            .parse::<bool>()
            .map_err(|_| "Choose On or Off".to_string())
    };
    match control {
        "world-rule-keep-inventory" => form.game_rules.keep_inventory = flag()?,
        "world-rule-daylight-cycle" => form.game_rules.daylight_cycle = flag()?,
        "world-rule-weather-cycle" => form.game_rules.weather_cycle = flag()?,
        "world-rule-mob-griefing" => form.game_rules.mob_griefing = flag()?,
        "world-rule-fire-spread" => form.game_rules.fire_spread = flag()?,
        "world-rule-pvp" => form.game_rules.pvp = flag()?,
        "world-structures-enabled" => form.structures.set_all(flag()?),
        "world-structure-villages" => form.structures.villages = flag()?,
        "world-structure-kingdoms" => form.structures.kingdoms = flag()?,
        "world-structure-outposts" => form.structures.outposts = flag()?,
        "world-structure-ruins" => form.structures.ruins = flag()?,
        "world-structure-mines" => form.structures.mines = flag()?,
        "world-sleep-percentage" | "world-time-speed" => {
            let number = value
                .parse::<f64>()
                .map_err(|_| "Enter a valid rule value")?;
            let rule = if control == "world-sleep-percentage" {
                terra_core::GameRule::SleepPercentage
            } else {
                terra_core::GameRule::GameTimeSpeed
            };
            if !rule.valid(number) {
                return Err(rule.description().into());
            }
            if control == "world-sleep-percentage" {
                form.game_rules.sleep_percentage = number;
            } else {
                form.game_rules.game_time_speed = number;
            }
        }
        "world-season-days" => {
            let days = value
                .parse::<u32>()
                .map_err(|_| "Season length must be 1..3650 days")?;
            if !(1..=3650).contains(&days) {
                return Err("Season length must be 1..3650 days".into());
            }
            form.season_days = days;
        }
        _ => return Err("Unknown world setting".into()),
    }
    Ok(true)
}
