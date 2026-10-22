//! Real held-use input, authority, rendered poses, cancellation and saved food state.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    Config, ControlsConfig, GameAction, GameConfig, GameMode, InputBinding, ItemKind,
    PlayerProfile, PlayerStats, SurvivalVitals, creatures::AnimalProduct,
};
use terra_network::client::{ClientSession, SelectedInventorySlot};
use terra_player::{CameraPerspective, CaptureRequest, ControllerMode};

const FOOD: ItemKind = ItemKind::Animal(AnimalProduct::CookedFish);
const INITIAL_COUNT: u16 = 3;
const INITIAL_HEALTH: f32 = 64.0;

fn root() -> PathBuf {
    std::env::var_os("TERRA_FOOD_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("terra-food-{}", std::process::id())))
}

pub(crate) fn menu_directory() -> PathBuf {
    root().join("menu")
}

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.water = false;
    config.world.rivers = false;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let root = root();
    let path = root.join("world");
    if path.exists() {
        return Err("Food fixture requires a fresh TERRA_FOOD_SMOKE_DIR".into());
    }
    if std::env::var_os("TERRA_PROFILE_FILE").is_some()
        || std::env::args().any(|argument| argument == "--profile")
    {
        return Err("Food fixture owns its test profile; omit profile overrides".into());
    }
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let profile = PlayerProfile {
        name: "Food-Smoke".into(),
        ..default()
    };
    profile
        .save(menu_directory().join("profile.toml"))
        .map_err(|error| error.to_string())?;
    terra_core::GraphicsConfig {
        width: config.client.width,
        height: config.client.height,
        render_distance: config.client.view_distance,
        ..default()
    }
    .save(menu_directory().join("graphics.toml"))
    .map_err(|error| error.to_string())?;
    let mut journal = terra_world::edits::EditJournal::default();
    journal.update_player_stats(
        profile.uuid,
        PlayerStats {
            blocks_broken: 19,
            survival: SurvivalVitals {
                health: INITIAL_HEALTH,
                hunger: 20.0,
                ..default()
            },
            ..default()
        },
    );
    journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(FOOD, INITIAL_COUNT);
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    info!(path = %root.display(), "Food fixture prepared a separate profile and world");
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Fixture {
    started: Instant,
    entered: Instant,
    stage: u8,
    captures: u8,
    waiting_release_capture: bool,
}
impl Fixture {
    fn advance(&mut self, label: &str) {
        self.stage += 1;
        self.entered = Instant::now();
        info!(stage = self.stage, %label, "Food smoke advanced");
    }
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    if !std::env::args().any(|arg| arg == "--smoke-test-food") {
        return Ok(());
    }
    {
        let mut menu = app.world_mut().resource_mut::<terra_ui::menu::MenuState>();
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    let mut controls = ControlsConfig::default();
    controls.rebind(GameAction::Place, InputBinding::Key(KeyCode::KeyU))?;
    let now = Instant::now();
    app.insert_resource(controls)
        .insert_resource(ControllerMode::Walk)
        .insert_resource(Fixture {
            started: now,
            entered: now,
            stage: 0,
            captures: 0,
            waiting_release_capture: false,
        })
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
    Ok(())
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("captures"));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>,
             mut fixture: ResMut<Fixture>,
             mut exit: MessageWriter<AppExit>| {
                if event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .filter(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)
                        .count()
                        < 1000
                }) {
                    error!("Food capture is blank");
                    fixture.stage = 255;
                    exit.write(AppExit::error());
                } else {
                    fixture.captures += 1;
                }
            },
        );
    Ok(())
}

fn use_input(world: &mut World, held: bool) {
    let binding = world
        .resource::<ControlsConfig>()
        .binding(GameAction::Place);
    let key = |world: &mut World, code| {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(code);
        } else {
            keys.release(code);
        }
    };
    match binding {
        InputBinding::Key(code) => key(world, code),
        InputBinding::Chord {
            modifier,
            key: code,
        } => {
            key(world, modifier);
            key(world, code);
        }
        InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            if held {
                mouse.press(button);
            } else {
                mouse.release(button);
            }
        }
    }
}

fn eating_audio(world: &mut World) -> (usize, usize) {
    use bevy::audio::{AudioSink, AudioSinkPlayback};
    let mut query = world.query::<(&AudioPlayer, Option<&AudioSink>)>();
    let assets = world.resource::<AssetServer>();
    query
        .iter(world)
        .filter(|(player, _)| {
            assets
                .get_path(player.0.id())
                .is_some_and(|path| path.path().ends_with("audio/animals/eat.wav"))
        })
        .fold((0, 0), |(sources, sinks), (_, sink)| {
            (
                sources + 1,
                sinks + usize::from(sink.is_some_and(|sink| !sink.empty())),
            )
        })
}

fn food_preview_ready(world: &mut World) -> bool {
    let Some(handle) = world
        .resource::<terra_ui::ItemPreviewImages>()
        .0
        .get(&FOOD)
        .cloned()
    else {
        return false;
    };
    world
        .query::<(
            &tilt_ui::ImageMetadata,
            &bevy::ui::widget::ImageNode,
            &ComputedNode,
            &Visibility,
        )>()
        .iter(world)
        .any(|(metadata, image, node, visibility)| {
            metadata.alt.as_deref() == Some("Item preview teramic::cooked_fish")
                && image.image == handle
                && !node.is_empty()
                && *visibility != Visibility::Hidden
        })
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut fixture) {
            error!(%error, stage = fixture.stage, "Native food smoke failed");
            use_input(world, false);
            fixture.stage = 255;
            if let Some(host) = world.remove_resource::<terra_server::IntegratedServer>()
                && let Err(error) = host.finish()
            {
                error!(%error, "Food fixture shutdown failed");
            }
            world.write_message(AppExit::error());
        }
    });
}

fn step(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    if fixture.started.elapsed() > Duration::from_secs(90) {
        return Err(format!(
            "Food fixture timed out in stage {}; pending previews: {}, preview error: {:?}",
            fixture.stage,
            world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending,
            world.resource::<terra_render::RenderBlockPreviews>().error,
        ));
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    let ready = session.ready && session.stats.blocks_broken == 19;
    let mode = session.game_mode;
    let count = session.inventory.count(FOOD);
    let vitals = session.stats.survival;
    let consumption = session.consumption;
    let elapsed = fixture.entered.elapsed().as_secs_f32();
    let duration = terra_core::food(FOOD)
        .ok_or("Cooked fish food definition missing")?
        .eat_seconds;
    match fixture.stage {
        0 if ready
            && world.resource::<terra_render::RenderStatus>().ready
            && world.resource::<terra_ui::SurvivalView>().visible =>
        {
            if mode != GameMode::Survival || count != INITIAL_COUNT {
                return Err("Initial Survival inventory did not reach the client".into());
            }
            world
                .resource_mut::<ControlsConfig>()
                .rebind(GameAction::Place, InputBinding::Key(KeyCode::KeyU))?;
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            world.write_message(CaptureRequest(true));
            *world.resource_mut::<CameraPerspective>() = CameraPerspective::FirstPerson;
            fixture.advance("warm first-person assets");
        }
        1 if elapsed > 1.0
            && world.resource::<terra_player::CameraInputState>().captured
            && food_preview_ready(world) =>
        {
            if world.resource::<SelectedInventorySlot>().0 != Some(0) {
                return Err("Food hotbar slot was not selected through the UI".into());
            }
            capture(world, "food-idle")?;
            fixture.advance("hold rebound U key");
        }
        2 if consumption.is_some_and(|progress| progress.progress >= 0.30) => {
            if count != INITIAL_COUNT || vitals.hunger > 20.1 {
                return Err("Food was consumed before the held-use duration".into());
            }
            let (sources, sinks) = eating_audio(world);
            if sources != 1 {
                return Err(format!(
                    "Expected one recorded eating source, found {sources}"
                ));
            }
            info!(
                sources,
                sinks, "Food audio source active; this is not a listening assessment"
            );
            capture(world, "food-first-person-bite")?;
            fixture.waiting_release_capture = true;
            fixture.advance("capture bite before releasing");
        }
        3 if fixture.waiting_release_capture => {
            if count != INITIAL_COUNT {
                return Err("First-person screenshot took longer than the remaining bite".into());
            }
            if fixture.captures >= 2 {
                fixture.waiting_release_capture = false;
                fixture.entered = Instant::now();
                info!("Food first-person bite captured; releasing Use");
            }
        }
        3 if elapsed > duration + 0.3 => {
            if count != INITIAL_COUNT || consumption.is_some() || eating_audio(world).0 != 0 {
                return Err(
                    "Release did not cancel the in-progress bite without consumption".into(),
                );
            }
            world
                .resource_mut::<ControlsConfig>()
                .rebind(GameAction::Place, InputBinding::Mouse(MouseButton::Right))?;
            *world.resource_mut::<CameraPerspective>() = CameraPerspective::ThirdPersonFront;
            fixture.advance("warm front-facing third-person view");
        }
        4 if elapsed > 0.5 => {
            fixture.advance("hold rebound right mouse button");
        }
        5 if consumption.is_some_and(|progress| progress.progress >= 0.30) => {
            if count != INITIAL_COUNT {
                return Err("Second bite consumed food prematurely".into());
            }
            capture(world, "food-third-person-bite")?;
            fixture.advance("finish one complete bite");
        }
        6 if count == INITIAL_COUNT - 1 && vitals.hunger >= 55.0 && vitals.saturation >= 10.0 => {
            info!(
                ?vitals,
                count, "Food authority consumed exactly one cooked fish"
            );
            fixture.advance("release after completion");
        }
        7 if elapsed > 0.5 && consumption.is_none() => {
            if count != INITIAL_COUNT - 1 {
                return Err("Holding through completion consumed more than one item".into());
            }
            fixture.advance("begin bite for menu interruption");
        }
        8 if consumption.is_some_and(|progress| progress.progress >= 0.25) => {
            world.write_message(terra_ui::menu::MenuAction::Command("pause".into()));
            fixture.advance("keep Use held while pause menu is open");
        }
        9 if elapsed > duration + 0.5 => {
            if consumption.is_some()
                || count != INITIAL_COUNT - 1
                || eating_audio(world).0 != 0
                || world.resource::<terra_ui::menu::MenuState>().page
                    != terra_ui::menu::MenuPage::Pause
            {
                return Err("Opening the menu failed to cancel held food use".into());
            }
            capture(world, "food-menu-cancelled")?;
            fixture.advance("wait for menu capture");
        }
        10 if fixture.captures >= 4 => {
            world.write_message(terra_ui::menu::MenuAction::Command("resume".into()));
            fixture.advance("release and verify persistent result");
        }
        11 if elapsed > 0.5 => {
            if count != INITIAL_COUNT - 1
                || consumption.is_some()
                || vitals.health <= INITIAL_HEALTH
            {
                return Err(format!(
                    "Final inventory/regeneration mismatch: count={count}, {vitals:?}"
                ));
            }
            use_input(world, false);
            let uuid = world.resource::<PlayerProfile>().uuid;
            let world_id = world.resource::<GameConfig>().world.id;
            world
                .remove_resource::<terra_server::IntegratedServer>()
                .ok_or("Missing integrated fixture server")?
                .finish()?;
            let saved = terra_storage::load_edits(&root().join("world"), world_id)?;
            let inventory = saved
                .gameplay()
                .inventories
                .get(&uuid)
                .ok_or("Saved food inventory missing")?;
            let stats = saved
                .gameplay()
                .player_stats
                .get(&uuid)
                .ok_or("Saved survival vitals missing")?;
            if inventory.count(FOOD) != INITIAL_COUNT - 1
                || stats.survival.hunger < 50.0
                || stats.survival.saturation <= 0.0
                || stats.survival.health <= INITIAL_HEALTH
            {
                return Err("Food inventory and expanded vitals failed native save/reload".into());
            }
            info!(
                captures = fixture.captures,
                "Native food smoke passed: rebound Place key/mouse held eating, no premature consumption, release/menu cancellation, one-item consumption, regeneration, first/third-person captures and save/reload"
            );
            fixture.stage = 255;
            world.write_message(AppExit::Success);
            return Ok(());
        }
        _ => {}
    }
    if ready && fixture.stage >= 2 && count < INITIAL_COUNT - 1 {
        return Err("Food fixture consumed an unexpected additional item".into());
    }
    use_input(
        world,
        (matches!(fixture.stage, 2 | 5 | 6 | 8 | 9)
            && (fixture.stage != 6 || count == INITIAL_COUNT))
            || fixture.stage == 3 && fixture.waiting_release_capture,
    );
    Ok(())
}
