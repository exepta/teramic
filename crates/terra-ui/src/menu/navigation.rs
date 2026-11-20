use super::{library, model::*, options};
use bevy::prelude::*;
use terra_core::{GameAction, GameplayInput};
use terra_player::{CaptureRequest, PlayerInputBlock};

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct GameplayPanels<'w> {
    inventory: Option<Res<'w, crate::inventory::InventoryState>>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    fullmap: Option<Res<'w, crate::minimap::fullmap::FullMapState>>,
    pets: Option<Res<'w, crate::pets::PetMenuState>>,
    kingdom: Option<Res<'w, crate::kingdom::KingdomMenuState>>,
    trains: Option<Res<'w, crate::trains::TrainUiState>>,
    npc_work: Option<Res<'w, crate::npc_work::WorkUiState>>,
}

pub(super) fn keyboard_navigation(
    input: GameplayInput,
    mut menu: ResMut<MenuState>,
    draft: Res<options::OptionsDraft>,
    worlds: Res<WorldMenu>,
    mut actions: MessageWriter<MenuAction>,
    panels: GameplayPanels,
    startup: Option<Res<StartupProgress>>,
) {
    if startup.as_ref().is_some_and(|progress| progress.active) {
        return;
    }
    if draft.rebinding.is_some() || draft.consumed_escape {
        return;
    }
    if panels
        .npc_work
        .as_ref()
        .is_some_and(|work| work.blocks_input() || work.consumed_escape)
    {
        return;
    }
    if panels
        .trains
        .as_ref()
        .is_some_and(|trains| trains.blocks_input())
    {
        return;
    }
    if panels
        .kingdom
        .as_ref()
        .is_some_and(|k| k.blocks_input() || k.consumed_escape)
    {
        return;
    }
    if panels
        .pets
        .as_ref()
        .is_some_and(|p| p.blocks_input() || p.consumed_escape)
    {
        return;
    }
    if panels
        .fullmap
        .as_ref()
        .is_some_and(|map| map.blocks_input() || map.consumed_escape)
    {
        return;
    }
    if panels.chat.as_ref().is_some_and(|chat| chat.blocks_input())
        || panels
            .inventory
            .as_ref()
            .is_some_and(|inventory| inventory.blocks_input())
    {
        return;
    }
    if !input.just_pressed(GameAction::Pause) {
        return;
    }
    if menu.dialog == MenuDialog::CreateWorld && worlds.biome_picker_open {
        actions.write(MenuAction::Command("world-biome-picker-close".into()));
    } else if menu.dialog == MenuDialog::Display {
        actions.write(MenuAction::Command("display-revert".into()));
    } else if menu.dialog != MenuDialog::None {
        menu.dialog = MenuDialog::None;
    } else {
        actions.write(MenuAction::Command(
            match menu.page {
                MenuPage::Playing => "pause",
                MenuPage::Pause => "resume",
                MenuPage::Loading => "cancel-connect",
                MenuPage::HudLayout => "hud-cancel",
                _ => "back",
            }
            .into(),
        ));
    }
}

pub(super) fn actions(world: &mut World) {
    let actions: Vec<_> = world
        .resource_mut::<Messages<MenuAction>>()
        .drain()
        .collect();
    let startup = world
        .get_resource::<StartupProgress>()
        .is_some_and(|progress| progress.active);
    for action in actions {
        if startup {
            continue;
        }
        let result = match action {
            MenuAction::Change { control, value } => change(world, &control, &value),
            MenuAction::Command(command) => command_action(world, &command),
        };
        if let Err(error) = result {
            world.resource_mut::<MenuState>().error = error;
        }
    }
    let playing = !startup
        && !world
            .get_resource::<crate::npc_work::WorkUiState>()
            .is_some_and(|work| work.blocks_input() && !work.selection_active())
        && !world
            .get_resource::<crate::trains::TrainUiState>()
            .is_some_and(|trains| trains.blocks_input())
        && !world
            .get_resource::<crate::kingdom::KingdomMenuState>()
            .is_some_and(|k| k.blocks_input())
        && world.resource::<MenuState>().page == MenuPage::Playing
        && !world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|map| map.blocks_input())
        && !world
            .get_resource::<crate::inventory::InventoryState>()
            .is_some_and(|inventory| inventory.blocks_input())
        && !world
            .get_resource::<crate::chat::ChatState>()
            .is_some_and(|chat| chat.blocks_input())
        && !world
            .get_resource::<crate::pets::PetMenuState>()
            .is_some_and(|p| p.blocks_input())
        && !world
            .get_resource::<terra_network::client::ClientSession>()
            .is_some_and(|session| session.teleporting || session.dead());
    if world.resource::<PlayerInputBlock>().0 == playing {
        world.resource_mut::<PlayerInputBlock>().0 = !playing;
    }
}
fn command_action(world: &mut World, command: &str) -> Result<(), String> {
    if crate::character::command(world, command)? {
        return Ok(());
    }
    let modal = world.resource::<MenuState>().dialog.clone();
    if modal == MenuDialog::CreateWorld && world.resource::<WorldMenu>().biome_picker_open {
        if command == "dialog-cancel" {
            world.resource_mut::<WorldMenu>().biome_picker_open = false;
            return Ok(());
        }
        if !command.starts_with("world-biome-") {
            return Ok(());
        }
    }
    if modal == MenuDialog::CreateWorld && super::create_world::command(world, command)? {
        world.resource_mut::<MenuState>().error.clear();
        return Ok(());
    }
    if modal != MenuDialog::None
        && !matches!(
            command,
            "dialog-cancel"
                | "world-create-confirm"
                | "world-delete-confirm"
                | "server-save"
                | "discard-confirm"
                | "display-keep"
                | "display-revert"
                | "minimap-reset"
                | "mob-health-reset"
        )
    {
        return Ok(());
    }
    if library::select(world, command) {
        return Ok(());
    }
    if let Some(index) = command
        .strip_prefix("bind-")
        .and_then(|s| s.parse::<usize>().ok())
    {
        if let Some(&action) = GameAction::ALL.get(index) {
            let mut draft = world.resource_mut::<options::OptionsDraft>();
            draft.rebinding = Some(action);
            draft.armed = false;
            world.resource_mut::<MenuState>().error.clear();
        }
        return Ok(());
    }
    // Dialog commands are not interchangeable with commands behind the modal.
    match command {
        "hud-edit" => return crate::hud_layout::open(world),
        "hud-save" => return crate::hud_layout::save(world),
        "hud-cancel" => crate::hud_layout::cancel(world),
        "hud-reset" => crate::hud_layout::reset(world),
        "minimap-settings" if world.resource::<MenuState>().page == MenuPage::Options => {
            world.resource_mut::<MenuState>().dialog = MenuDialog::Minimap;
        }
        "minimap-reset" if modal == MenuDialog::Minimap => {
            world
                .resource_mut::<options::OptionsDraft>()
                .gameplay
                .minimap = terra_core::MinimapConfig::default();
        }
        "mob-health-settings" if world.resource::<MenuState>().page == MenuPage::Options => {
            world.resource_mut::<MenuState>().dialog = MenuDialog::MobHealth;
        }
        "mob-health-reset" if modal == MenuDialog::MobHealth => {
            let defaults = terra_core::GameplayConfig::default();
            let mut draft = world.resource_mut::<options::OptionsDraft>();
            draft.gameplay.mob_health_bars = defaults.mob_health_bars;
            draft.gameplay.mob_health_friendly_color = defaults.mob_health_friendly_color;
            draft.gameplay.mob_health_hostile_color = defaults.mob_health_hostile_color;
            draft.gameplay.mob_health_font_color = defaults.mob_health_font_color;
        }
        "surface-wet" | "surface-snow" | "surface-fire" | "surface-clear" => {
            if !world.resource::<MenuState>().session_active
                || !world
                    .get_resource::<terra_network::client::ClientSession>()
                    .is_some_and(|session| session.ready && session.allow_environment)
            {
                return Err("Creative world permission is required".into());
            }
            let hit = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("No surface is selected")?;
            let material = world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(hit.position)
                .ok_or("Surface is no longer loaded")?;
            let mut condition = world
                .resource::<terra_world::edits::EditJournal>()
                .surface_condition(hit.position);
            match command {
                "surface-wet" => {
                    condition.wetness = 1.0;
                    condition.fire = 0.0;
                    condition.snow = 0.0;
                }
                "surface-snow" => {
                    condition.snow = 1.0;
                    condition.fire = 0.0;
                }
                "surface-fire"
                    if terra_voxel::MaterialId::from_raw(material)
                        .is_some_and(|id| id.flammable()) =>
                {
                    condition.fire = 1.0;
                    condition.snow = 0.0;
                    condition.wetness = 0.0;
                }
                "surface-fire" => return Err("This material is not flammable".into()),
                _ => condition = Default::default(),
            }
            world.write_message(terra_world::environment::SurfaceConditionRequest {
                position: hit.position,
                condition,
            });
            world.resource_mut::<MenuState>().status = "Surface command sent to server".into();
        }
        "singleplayer" => {
            world.resource_mut::<MenuState>().page = MenuPage::Worlds;
            library::refresh_worlds(world);
        }
        "multiplayer" => {
            world.resource_mut::<MenuState>().page = MenuPage::Servers;
            library::refresh_servers(world);
        }
        "options" => options::open(world),
        "graphics" | "gameplay-options" | "controls" | "audio" | "multiplayer-options" => {
            let mut draft = world.resource_mut::<options::OptionsDraft>();
            draft.tab = match command {
                "controls" => options::OptionsTab::Controls,
                "gameplay-options" => options::OptionsTab::Gameplay,
                "audio" => options::OptionsTab::Audio,
                "multiplayer-options" => options::OptionsTab::Multiplayer,
                _ => options::OptionsTab::Graphics,
            };
            draft.rebinding = None;
        }
        "apply" => return options::apply(world),
        "reset" => options::reset(world),
        "back" => {
            if world.resource::<MenuState>().page == MenuPage::Options {
                if options::dirty(world) {
                    world.resource_mut::<MenuState>().dialog = MenuDialog::Discard;
                } else {
                    options::back(world);
                }
            } else {
                world.resource_mut::<MenuState>().page = MenuPage::Main;
                world.resource_mut::<MenuState>().error.clear();
            }
        }
        "world-create" => {
            super::create_world::reset(world);
            world.resource_mut::<MenuState>().dialog = MenuDialog::CreateWorld;
        }
        "world-create-confirm" if modal == MenuDialog::CreateWorld => {
            library::create_world(world)?;
            world.resource_mut::<MenuState>().dialog = MenuDialog::None;
        }
        "world-delete" => {
            if world.resource::<WorldMenu>().selected.is_none() {
                return Err("Select a world first".into());
            }
            world.resource_mut::<MenuState>().dialog = MenuDialog::DeleteWorld;
        }
        "world-delete-confirm" if modal == MenuDialog::DeleteWorld => {
            library::delete_world(world)?;
            world.resource_mut::<MenuState>().dialog = MenuDialog::None;
        }
        "world-play" => {
            let id = world
                .resource::<WorldMenu>()
                .selected
                .ok_or("Select a world first")?;
            world
                .resource::<WorldMenu>()
                .entries
                .iter()
                .find(|e| e.manifest.world.id == id)
                .ok_or("World is no longer available")?
                .compatible()?;
            begin(
                world,
                SessionRequest::PlayWorld(id),
                "Starting integrated server ...",
            );
        }
        "server-add" | "server-edit" => {
            library::edit_server(world, command == "server-edit")?;
            world.resource_mut::<MenuState>().dialog = MenuDialog::EditServer;
        }
        "server-save" if modal == MenuDialog::EditServer => {
            library::save_server(world)?;
            world.resource_mut::<MenuState>().dialog = MenuDialog::None;
        }
        "server-join" => {
            let menu = world.resource::<ServerMenu>();
            if let Some(address) = menu.lan_selected {
                let server = world
                    .resource::<super::browser::ServerBrowser>()
                    .lan
                    .iter()
                    .find(|s| s.address == address)
                    .ok_or("This LAN server is no longer available")?;
                if !server.status.compatible() {
                    return Err("Incompatible TERAMIC server version".into());
                }
                begin(
                    world,
                    SessionRequest::JoinAddress(address),
                    "Connecting to LAN server ...",
                );
                return Ok(());
            }
            let id = menu.selected.ok_or("Select a server first")?;
            let entry = menu
                .list
                .servers
                .iter()
                .find(|s| s.id == id)
                .ok_or("Server is no longer available")?;
            entry.validate().map_err(|e| e.to_string())?;
            begin(
                world,
                SessionRequest::JoinServer(id),
                "Connecting to dedicated server ...",
            );
        }
        "pause" => {
            world.resource_mut::<MenuState>().page = MenuPage::Pause;
            world.write_message(CaptureRequest(false));
        }
        "resume" => {
            world.resource_mut::<MenuState>().page = MenuPage::Playing;
            world.write_message(CaptureRequest(true));
        }
        "leave" | "cancel-connect" => begin(
            world,
            SessionRequest::Leave,
            "Closing session and saving world ...",
        ),
        "save-world" => {
            world.write_message(SessionRequest::Save);
        }
        "quit" => {
            world.resource_mut::<MenuState>().quitting = true;
            begin(world, SessionRequest::Quit, "Saving and closing ...");
        }
        "dialog-cancel" => {
            world.resource_mut::<MenuState>().dialog = MenuDialog::None;
            world.resource_mut::<MenuState>().error.clear();
        }
        "discard-confirm" if modal == MenuDialog::Discard => options::back(world),
        "display-keep" if modal == MenuDialog::Display => return options::keep_display(world),
        "display-revert" if modal == MenuDialog::Display => options::revert_display(world),
        _ => {}
    }
    Ok(())
}
fn begin(world: &mut World, request: SessionRequest, status: &str) {
    let mut menu = world.resource_mut::<MenuState>();
    menu.page = MenuPage::Loading;
    menu.error.clear();
    menu.status = status.into();
    menu.dialog = MenuDialog::None;
    world.write_message(CaptureRequest(false));
    world.write_message(request);
}
fn change(world: &mut World, control: &str, value: &str) -> Result<(), String> {
    if crate::character::change(world, control, value) {
        return Ok(());
    }
    if world.resource::<MenuState>().dialog == MenuDialog::CreateWorld
        && world.resource::<WorldMenu>().biome_picker_open
    {
        return Ok(());
    }

    if super::create_world::change(world, control, value)? {
        if world.resource::<MenuState>().dialog == MenuDialog::CreateWorld {
            world.resource_mut::<MenuState>().error.clear();
        }
        return Ok(());
    }
    match control {
        "world-name" => world.resource_mut::<WorldMenu>().name = value.chars().take(65).collect(),
        "world-seed" => world.resource_mut::<WorldMenu>().seed = value.chars().take(32).collect(),
        "world-mode" => world.resource_mut::<WorldMenu>().mode = value.into(),
        "world-mob-limit" => {
            world.resource_mut::<WorldMenu>().natural_mob_limit =
                value.parse::<f32>().unwrap_or(256.0).clamp(0.0, 4096.0) as u16
        }
        "server-name" => world.resource_mut::<ServerMenu>().name = value.chars().take(65).collect(),
        "server-address" => {
            world.resource_mut::<ServerMenu>().address = value.chars().take(254).collect()
        }
        "server-port" => world.resource_mut::<ServerMenu>().port = value.chars().take(6).collect(),
        _ => options::change(world, control, value)?,
    }
    Ok(())
}
