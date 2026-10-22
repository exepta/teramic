//! Short native calendar/air check using a real integrated Survival authority and pool.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, GameMode, PlayerProfile, PlayerStats, Season, SurvivalVitals};
use terra_ui::{ElementId, SurvivalView};

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.sea_level = -20.0;
    config.world.water = true;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::temp_dir().join(format!("terra-calendar-hud-{}", std::process::id()));
    if path.exists() {
        return Err("Calendar fixture needs a fresh save directory".into());
    }
    let profile_path = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|e| e.to_string())?;
    let profile = PlayerProfile::load_for_client(profile_path).map_err(|e| e.to_string())?;
    let mut permissions = config.server.permissions_for(profile.uuid);
    permissions.environment = true;
    permissions.flight = true;
    config
        .server
        .player_permissions
        .insert(profile.uuid, permissions);
    let mut journal = terra_world::edits::EditJournal::default();
    journal.update_player_stats(
        profile.uuid,
        PlayerStats {
            survival: SurvivalVitals {
                health: 78.0,
                hunger: 95.0,
                ..default()
            },
            ..default()
        },
    );
    // Only this explicit test save contains the bounded walled pool.
    for x in 2..=14 {
        for z in 2..=14 {
            for y in 12..=18 {
                let wall = y == 12 || x == 2 || x == 14 || z == 2 || z == 14;
                journal
                    .record([x, y, z], if wall { 3 } else { 0 })
                    .map_err(|e| e.to_string())?;
                if !wall && y <= 17 {
                    journal
                        .set_fluid(
                            [x, y, z],
                            terra_world::water::FluidCell {
                                amount: 8,
                                flow: [0, 0],
                                source: y == 17,
                            },
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: usize,
    configured: bool,
    pending: bool,
    meter: Option<Vec2>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-calendar-hud") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Walk)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            stage: 0,
            configured: false,
            pending: false,
            meter: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(PreUpdate, input.after(bevy::input::InputSystems))
        .add_systems(Last, run);
}

fn setup(world: &mut World) {
    {
        let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = true;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .size = 320;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}

fn input(
    state: Res<Inspection>,
    controls: Res<terra_core::ControlsConfig>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    for mut window in &mut windows {
        window.bypass_change_detection().focused = true;
    }
    let down = state.stage == 1;
    let mut key = |value| {
        if down {
            keys.press(value);
        } else {
            keys.release(value);
        }
    };
    match controls.binding(terra_core::GameAction::FlyDown) {
        terra_core::InputBinding::Key(value) => key(value),
        terra_core::InputBinding::Mouse(value) => {
            if down {
                mouse.press(value);
            } else {
                mouse.release(value);
            }
        }
        terra_core::InputBinding::Chord {
            modifier,
            key: value,
        } => {
            key(modifier);
            key(value);
        }
    }
}

fn check(world: &mut World, stage: usize) -> Result<(), String> {
    let view = world.resource::<SurvivalView>();
    if !view.visible || view.show_oxygen != (stage == 1) {
        return Err("Wrong survival meter mode".into());
    }
    let name = if stage == 1 {
        "survival-oxygen"
    } else {
        "survival-stamina"
    };
    let other = if stage == 1 {
        "survival-stamina"
    } else {
        "survival-oxygen"
    };
    let nodes: Vec<_> = world
        .query::<(&ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(id, _)| id.0 == name || id.0 == other || id.0 == "map-season-ring")
        .map(|(id, node)| (id.0.clone(), node.size()))
        .collect();
    if nodes.iter().any(|(id, _)| id == other) {
        return Err("Two stamina/air meters at once".into());
    }
    let size = nodes
        .iter()
        .find(|(id, _)| id == name)
        .map(|(_, size)| *size)
        .filter(|size| size.x > 100.0 && size.y > 3.0)
        .ok_or("Missing meter geometry")?;
    let old = world.resource::<Inspection>().meter;
    if old.is_some_and(|old| !old.abs_diff_eq(size, 0.01)) {
        return Err("Air resized the stamina rail".into());
    }
    world.resource_mut::<Inspection>().meter = Some(size);
    if !nodes
        .iter()
        .any(|(id, size)| id == "map-season-ring" && size.min_element() > 100.0)
    {
        return Err("Missing native calendar ring".into());
    }
    let map = world.resource::<terra_ui::minimap::MiniMapView>();
    if !map.seasons || (map.season_cycle * 4.0).floor() as usize != stage {
        return Err("Wrong calendar quarter".into());
    }
    let values = world.resource::<terra_ui::VitalRenderStatus>();
    let v = world
        .resource::<terra_network::client::ClientSession>()
        .stats
        .survival;
    if stage == 1 && (v.oxygen >= 99.0 || v.stamina != 100.0) {
        return Err(format!(
            "Real diving did not use air instead of stamina: {v:?}"
        ));
    }
    if stage != 1 && v.oxygen != 100.0 {
        return Err("Real surfacing did not refill air".into());
    }
    let expected = if stage == 1 {
        v.oxygen / 100.0
    } else {
        v.stamina / 100.0
    };
    if !values.bound || (values.stamina - expected).abs() > 0.01 {
        return Err("Wrong native air/stamina material value".into());
    }
    Ok(())
}

fn run(world: &mut World) {
    if world.resource::<Inspection>().start.elapsed() > Duration::from_secs(90) {
        error!("Calendar/HUD presentation timed out");
        world.write_message(AppExit::error());
        return;
    }
    let stage = world.resource::<Inspection>().stage;
    if stage == 4 {
        // Inventory preview readbacks also own screenshot channels; let them drain.
        let blocks = world.resource::<terra_render::RenderBlockPreviews>();
        let portrait = world.resource::<terra_render::PlayerPreviewImage>();
        if blocks.error.is_some() || portrait.error.is_some() {
            error!("Calendar fixture encountered an inventory preview error");
            world.write_message(AppExit::error());
            return;
        }
        if blocks.pending != 0 || !portrait.ready {
            return;
        }
        if !world.resource::<Inspection>().pending {
            let mut state = world.resource_mut::<Inspection>();
            state.pending = true;
            state.since = Instant::now();
            return;
        }
        if world.resource::<Inspection>().since.elapsed() < Duration::from_millis(600) {
            return;
        }
        info!(
            "Calendar/HUD native presentation passed: four seasons, shared air/stamina rail, refill and return"
        );
        world.write_message(AppExit::Success);
        return;
    }
    let session = world.resource::<terra_network::client::ClientSession>();
    if !session.ready || session.teleporting {
        return;
    }
    if !world.resource::<Inspection>().configured {
        let season = [
            Season::Spring,
            Season::Summer,
            Season::Autumn,
            Season::Winter,
        ][stage];
        let commands = &mut world
            .resource_mut::<terra_network::client::ClientChatCommands>()
            .0;
        commands.push_back(terra_network::client::ChatCommand::Send(format!(
            "/season set {}",
            season.name().to_lowercase()
        )));
        commands.push_back(terra_network::client::ChatCommand::Send(
            if stage == 1 {
                "/tp 8 14 8"
            } else {
                "/tp 24 17 24"
            }
            .into(),
        ));
        let mut state = world.resource_mut::<Inspection>();
        state.configured = true;
        state.since = Instant::now();
        return;
    }
    if world.resource::<Inspection>().pending
        || world.resource::<Inspection>().since.elapsed() < Duration::from_secs(2)
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return;
    }
    if let Err(error) = check(world, stage) {
        error!(%error, stage, "Calendar/HUD check failed");
        world.write_message(AppExit::error());
        return;
    }
    let folder = PathBuf::from(
        std::env::var_os("TERRA_CAPTURE_DIR").unwrap_or_else(|| "/tmp/terra-calendar-hud".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    world.resource_mut::<Inspection>().pending = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            folder.join(format!("calendar-hud-{stage}.png")),
        ))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut state: ResMut<Inspection>,
             mut exit: MessageWriter<AppExit>| {
                let lit = event.image.data.as_ref().is_some_and(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|&v| v > 30))
                        .count()
                        > 1000
                });
                if !lit {
                    error!("Blank HUD capture");
                    exit.write(AppExit::error());
                    return;
                }
                state.stage += 1;
                state.since = Instant::now();
                state.configured = false;
                state.pending = false;
            },
        );
}
