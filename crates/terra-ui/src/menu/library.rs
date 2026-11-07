use super::model::*;
use bevy::prelude::*;
use terra_core::{
    Config, GameConfig, WorldId,
    config::servers::{ServerEntry, ServerList},
};
use terra_storage::GameMode;

pub(super) fn refresh_worlds(world: &mut World) {
    let result = world.resource::<MenuPaths>().worlds.list();
    match result {
        Ok((entries, errors)) => {
            let mut menu = world.resource_mut::<WorldMenu>();
            if !entries
                .iter()
                .any(|e| Some(e.manifest.world.id) == menu.selected)
            {
                menu.selected = None;
            }
            menu.entries = entries;
            world.resource_mut::<MenuState>().error = errors.join("\n");
        }
        Err(error) => world.resource_mut::<MenuState>().error = error,
    }
}
pub(super) fn refresh_servers(world: &mut World) {
    let result =
        ServerList::load_or_create(world.resource::<MenuPaths>().configs.join("servers.toml"));
    match result {
        Ok(list) => {
            if list
                .servers
                .iter()
                .any(|server| !server.token_file.as_os_str().is_empty())
                && let Err(error) =
                    list.save(world.resource::<MenuPaths>().configs.join("servers.toml"))
            {
                world
                    .resource_mut::<MenuState>()
                    .error
                    .push_str(&format!("\nCannot remove obsolete token paths: {error}"));
            }
            let mut menu = world.resource_mut::<ServerMenu>();
            if !list.servers.iter().any(|s| Some(s.id) == menu.selected) {
                menu.selected = None;
            }
            menu.list = list;
        }
        Err(e) => world
            .resource_mut::<MenuState>()
            .error
            .push_str(&format!("\nservers.toml: {e}")),
    }
}
pub(super) fn create_world(world: &mut World) -> Result<(), String> {
    let menu = world.resource::<WorldMenu>();
    let seed = if menu.seed.trim().is_empty() {
        None
    } else {
        Some(
            menu.seed
                .trim()
                .parse::<u64>()
                .map_err(|_| "Seed must be an unsigned 64-bit number")?,
        )
    };
    let mode = match menu.mode.as_str() {
        "creative" => GameMode::Creative,
        "survival" => GameMode::Survival,
        "spectator" => GameMode::Spectator,
        _ => return Err("Choose a game mode".into()),
    };
    let mut template = world.resource::<GameConfig>().world.clone();
    template.rivers = true;
    template.bounded_world = true;
    template.natural_mob_limit = menu.natural_mob_limit;
    template.game_rules = menu.game_rules;
    template.kingdom_rules = menu.kingdom_rules;
    template.structure_border_gap = menu.kingdom_rules.realm_min_border_gap;
    template.preferred_spawn_biome = menu.preferred_spawn_biome;
    template.structures = menu.structures;
    template.seasons.spring_days = menu.season_days;
    template.seasons.summer_days = menu.season_days;
    template.seasons.autumn_days = menu.season_days;
    template.seasons.winter_days = menu.season_days;
    template.seasons.transition_days = template
        .seasons
        .transition_days
        .min(f64::from(menu.season_days));
    template.time.paused = false;
    template.validate().map_err(|error| error.to_string())?;
    let entry = world
        .resource::<MenuPaths>()
        .worlds
        .create(&menu.name, seed, mode, &template)?;
    world.resource_mut::<WorldMenu>().selected = Some(entry.manifest.world.id);
    refresh_worlds(world);
    Ok(())
}
pub(super) fn delete_world(world: &mut World) -> Result<(), String> {
    let id = world
        .resource::<WorldMenu>()
        .selected
        .ok_or("Select a world first")?;
    world.resource::<MenuPaths>().worlds.delete(id)?;
    world.resource_mut::<WorldMenu>().selected = None;
    refresh_worlds(world);
    Ok(())
}
pub(super) fn edit_server(world: &mut World, edit: bool) -> Result<(), String> {
    let lan = world
        .get_resource::<super::browser::ServerBrowser>()
        .and_then(|browser| {
            browser
                .lan
                .iter()
                .find(|server| Some(server.address) == world.resource::<ServerMenu>().lan_selected)
        })
        .cloned();
    let mut menu = world.resource_mut::<ServerMenu>();
    let entry = if edit {
        Some(
            menu.list
                .servers
                .iter()
                .find(|s| Some(s.id) == menu.selected)
                .ok_or("Select a server first")?
                .clone(),
        )
    } else {
        None
    };
    menu.editing = entry.as_ref().map(|s| s.id);
    menu.name = entry.as_ref().map_or_else(String::new, |s| s.name.clone());
    menu.address = entry
        .as_ref()
        .map_or_else(String::new, |s| s.address.clone());
    menu.port = entry.as_ref().map_or(5000, |s| s.port).to_string();
    if !edit && let Some(lan) = lan {
        menu.name = lan.status.title;
        menu.address = lan.address.ip().to_string();
        menu.port = lan.address.port().to_string();
    }
    Ok(())
}
pub(super) fn save_server(world: &mut World) -> Result<(), String> {
    let menu = world.resource::<ServerMenu>();
    let id = match menu.editing {
        Some(id) => id,
        None => menu
            .list
            .servers
            .iter()
            .map(|s| s.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Cannot allocate another saved server ID")?,
    };
    let entry = ServerEntry {
        id,
        name: menu.name.trim().into(),
        address: menu.address.trim().into(),
        port: menu.port.parse().map_err(|_| "Port must be 1..65535")?,
        token_file: Default::default(),
    };
    entry.validate().map_err(|e| e.to_string())?;
    let mut list = menu.list.clone();
    if let Some(previous) = list.servers.iter_mut().find(|s| s.id == id) {
        *previous = entry;
    } else {
        list.servers.push(entry);
    }
    list.save(world.resource::<MenuPaths>().configs.join("servers.toml"))
        .map_err(|e| e.to_string())?;
    let mut menu = world.resource_mut::<ServerMenu>();
    menu.list = list;
    menu.selected = Some(id);
    menu.lan_selected = None;
    if let Some(mut browser) = world.get_resource_mut::<super::browser::ServerBrowser>() {
        browser.records.remove(&id);
    }
    Ok(())
}
pub(super) fn select(world: &mut World, id: &str) -> bool {
    if let Some(address) = id
        .strip_prefix("lan-")
        .and_then(|s| s.parse::<std::net::SocketAddr>().ok())
    {
        if world
            .get_resource::<super::browser::ServerBrowser>()
            .is_some_and(|browser| browser.lan.iter().any(|server| server.address == address))
        {
            let mut menu = world.resource_mut::<ServerMenu>();
            menu.lan_selected = Some(address);
            menu.selected = None;
        }
        return true;
    }
    if let Some(id) = id
        .strip_prefix("world-")
        .and_then(|s| u64::from_str_radix(s, 16).ok())
    {
        if world
            .resource::<WorldMenu>()
            .entries
            .iter()
            .any(|e| e.manifest.world.id == WorldId(id))
        {
            world.resource_mut::<WorldMenu>().selected = Some(WorldId(id));
        }
        return true;
    }
    if let Some(id) = id.strip_prefix("server-").and_then(|s| s.parse().ok()) {
        if world
            .resource::<ServerMenu>()
            .list
            .servers
            .iter()
            .any(|s| s.id == id)
        {
            world.resource_mut::<ServerMenu>().selected = Some(id);
            world.resource_mut::<ServerMenu>().lan_selected = None;
        }
        return true;
    }
    false
}
