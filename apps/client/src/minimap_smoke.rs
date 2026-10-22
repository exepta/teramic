//! Real integrated authority, remote peers, loaded cave cells and TiltUI images.
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use terra_core::{GameAction, GameConfig, GameplayConfig, MapMode, PlayerProfile};
use terra_network::client::{ClientChatCommands, ClientSession, ReplicatedCreatures};
use terra_ui::{
    ElementId,
    menu::{MenuAction, MenuPage, MenuPaths, MenuState},
    minimap::{MiniMapView, WorldWaypointView, fullmap::FullMapView},
};
use terra_world::streaming::{ChunkStore, WorldTerrain};

const DONE: usize = 52;

struct Companion {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl Drop for Companion {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => error!(%error, "MiniMap companion failed"),
                Err(_) => error!("MiniMap companion thread terminated unexpectedly"),
            }
        }
    }
}

#[derive(Resource)]
struct Smoke {
    started: Instant,
    since: Instant,
    stage: usize,
    requested: bool,
    captures: usize,
    configured: bool,
    companion: Option<Companion>,
    cave: Option<Vec3>,
    searched: Instant,
    generation: u32,
    pending_key: Option<KeyCode>,
    waypoint_name: String,
    waypoint_count: usize,
    waypoint_id: Option<u64>,
    origin: Option<[f32; 3]>,
    pan_origin: String,
    pan_moved: bool,
    pan_progress: u8,
    pan_started: Option<Instant>,
    pan_locked: String,
    range_before_wheel: String,
    pointer_release: bool,
    waypoint_edit_requested: bool,
    scaled_form_stage: u8,
    flight_target: Option<Vec3>,
    flight_origin: Option<Vec3>,
    flight_heading: f32,
    flight_samples: u8,
    flight_captures: usize,
    flight_climbing: bool,
    flight_released: bool,
    flight_error: Option<String>,
    waypoint_select_requested: bool,
    map3d_centered: bool,
    map3d_captures: usize,
    tool_hover_requested: bool,
    map3d_target: Option<[f32; 3]>,
    map3d_wait_interval: u64,
    quick_position: Option<[f32; 3]>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-minimap") {
        return;
    }
    app.insert_resource(Smoke {
        started: Instant::now(),
        since: Instant::now(),
        stage: 0,
        requested: false,
        captures: 0,
        configured: false,
        companion: None,
        cave: None,
        searched: Instant::now(),
        generation: 0,
        pending_key: None,
        waypoint_name: format!("MapCheck {}", std::process::id()),
        waypoint_count: 0,
        waypoint_id: None,
        origin: None,
        pan_origin: String::new(),
        pan_moved: false,
        pan_progress: 0,
        pan_started: None,
        pan_locked: String::new(),
        range_before_wheel: String::new(),
        pointer_release: false,
        waypoint_edit_requested: false,
        scaled_form_stage: 0,
        flight_target: None,
        flight_origin: None,
        flight_heading: 0.0,
        flight_samples: 0,
        flight_captures: 0,
        flight_climbing: false,
        flight_released: false,
        flight_error: None,
        waypoint_select_requested: false,
        map3d_centered: false,
        map3d_captures: 0,
        tool_hover_requested: false,
        map3d_target: None,
        map3d_wait_interval: 0,
        quick_position: None,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, synthetic_focus.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}

fn synthetic_focus(
    smoke: Res<Smoke>,
    controls: Res<terra_core::ControlsConfig>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    if smoke.stage == DONE {
        return;
    }
    // This fixture owns synthetic input, not OS focus or the user's desktop.
    // Keep its logical focus/held keys stable when another window is activated.
    for mut window in &mut windows {
        window.bypass_change_detection().focused = true;
    }
    if (matches!(smoke.stage, 16 | 38) || (smoke.stage == 20 && smoke.pan_started.is_some()))
        && smoke.pan_progress < 5
    {
        mouse.press(MouseButton::Left);
    }
    if smoke.stage == 23 && !smoke.flight_released {
        for action in [GameAction::Forward, GameAction::Sprint, GameAction::Jump] {
            if action == GameAction::Jump && !smoke.flight_climbing {
                continue;
            }
            match controls.binding(action) {
                terra_core::InputBinding::Key(key) => keys.press(key),
                terra_core::InputBinding::Mouse(button) => mouse.press(button),
                terra_core::InputBinding::Chord { modifier, key } => {
                    keys.press(modifier);
                    keys.press(key);
                }
            }
        }
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut smoke: Mut<Smoke>| {
        if let Some(key) = smoke.pending_key.take() {
            key_event(world, key, false);
        }
        if smoke.pointer_release {
            if let Err(error) = point_pointer(world, "", false) {
                error!(%error, "FullMap waypoint pointer release failed");
                smoke.stage = DONE;
                smoke.companion = None;
                world.write_message(AppExit::error());
                return;
            }
            smoke.pointer_release = false;
        }
        match advance(world, &mut smoke) {
            Ok(true) => {
                info!("Native MiniMap and FullMap: real peer/mob markers, all FPS presets, icon scaling, cave teleport, M/Escape, pan/zoom/center, waypoint CRUD/icons/world labels/edge pin distance, TOML reload and real held-input boost flight with yaw/altitude changes passed");
                smoke.stage = DONE;
                smoke.companion = None;
                world.write_message(AppExit::Success);
            }
            Ok(false) => {}
            Err(error) => {
                let _ = flight_controls(world, false);
                error!(%error, stage = smoke.stage, "MiniMap native test failed");
                smoke.stage = DONE;
                smoke.companion = None;
                world.write_message(AppExit::error());
            }
        }
    });
}

fn advance(world: &mut World, smoke: &mut Smoke) -> Result<bool, String> {
    if smoke.stage == DONE {
        return Ok(false);
    }
    if smoke.started.elapsed() > Duration::from_secs(330) {
        return Err("Timed out waiting for map tiles, markers or a loaded cave".into());
    }
    if let Some(error) = &smoke.flight_error {
        return Err(error.clone());
    }
    if let Some(error) = world.resource::<ClientSession>().error.as_ref().or(world
        .resource::<terra_render::RenderStatus>()
        .error
        .as_ref())
    {
        return Err(error.clone());
    }
    if !world.resource::<ClientSession>().ready
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return Ok(false);
    }
    if world.resource::<MenuState>().page != MenuPage::Playing {
        if smoke.configured {
            return Err("Map input unexpectedly opened a menu".into());
        }
        world.write_message(MenuAction::Command("resume".into()));
        return Ok(false);
    }
    if !smoke.configured {
        let mut gameplay = world.resource_mut::<GameplayConfig>();
        gameplay.minimap.enabled = true;
        gameplay.minimap.mode = MapMode::Surface;
        gameplay.minimap.mobs = true;
        gameplay.minimap.players = true;
        gameplay.minimap.coordinates = true;
        gameplay.minimap.biome = true;
        gameplay.minimap.clock = true;
        gameplay.minimap.size = 260;
        gameplay.minimap.zoom = 1;
        gameplay.minimap.fps = 24;
        gameplay.minimap.icon_scale = 1.0;
        smoke.configured = true;
    }
    if smoke.companion.is_none() {
        let server = world
            .get_resource::<terra_server::IntegratedServer>()
            .ok_or("MiniMap smoke requires a real integrated server")?;
        let Some(origin) = world
            .resource::<ClientSession>()
            .confirmed_state()
            .map(|s| Vec3::from_array(s.position))
        else {
            return Ok(false);
        };
        smoke.companion = Some(start_companion(
            world.resource::<GameConfig>().clone(),
            server.address,
            companion_destination(world.resource::<ChunkStore>(), origin),
        )?);
    }
    if smoke
        .companion
        .as_ref()
        .and_then(|c| c.thread.as_ref())
        .is_some_and(JoinHandle::is_finished)
    {
        return Err("The real companion client exited before marker verification".into());
    }
    if smoke.cave.is_none() && smoke.stage < 5 && smoke.searched.elapsed() >= Duration::from_secs(2)
    {
        smoke.cave = cave_destination(world);
        smoke.searched = Instant::now();
    }
    if smoke.requested {
        if smoke.captures <= smoke.stage {
            return Ok(false);
        }
        smoke.stage += 1;
        smoke.requested = false;
        smoke.since = Instant::now();
        match smoke.stage {
            1 => {
                world.resource_mut::<GameplayConfig>().minimap.fps = 30;
                pulse_binding(world, smoke, GameAction::MapZoomOut)?;
            }
            2 => click(world, "map-toggle")?,
            3 => {
                click(world, "map-toggle")?;
                resize(world, 960.0, 720.0, 1.5);
                world.resource_mut::<GameplayConfig>().minimap.fps = 40;
            }
            4 => {
                resize(world, 1280.0, 900.0, 1.0);
                let mut gameplay = world.resource_mut::<GameplayConfig>();
                gameplay.minimap.zoom = 2;
                gameplay.minimap.relief = true;
                gameplay.minimap.fps = 60;
                gameplay.minimap.icon_scale = 2.5;
            }
            5 => {
                let target = smoke
                    .cave
                    .ok_or("No real loaded walkable cave with a roof was found near spawn")?;
                smoke.generation = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .ok_or("Missing authoritative player state")?
                    .generation;
                world.resource_mut::<GameplayConfig>().minimap.mode = MapMode::Auto;
                world.resource_mut::<GameplayConfig>().minimap.zoom = 0;
                world.resource_mut::<ClientChatCommands>().0.push_back(
                    terra_network::client::ChatCommand::Send(format!(
                        "/tp {:.2} {:.2} {:.2}",
                        target.x, target.y, target.z
                    )),
                );
                info!(
                    ?target,
                    "MiniMap native test requested authoritative cave teleport"
                );
            }
            6 => {
                smoke.waypoint_count = waypoint_book(world)?.entries.len();
                pulse_binding(world, smoke, GameAction::FullMap)?;
            }
            7 => map_click(world, "fullmap-layer")?,
            8 => map_click(world, "fullmap-add")?,
            9 => {
                map_change(world, "fullmap-name", &smoke.waypoint_name)?;
                let origin = smoke.origin.ok_or("Missing surface spawn coordinates")?;
                for (id, value) in ["fullmap-x", "fullmap-y", "fullmap-z"]
                    .into_iter()
                    .zip(origin)
                {
                    map_change(world, id, &format!("{value:.1}"))?;
                }
                map_change(world, "fullmap-color", "#d594eb")?;
                map_change(world, "fullmap-icon", "home")?;
                map_change(world, "fullmap-world", "true")?;
            }
            10 => map_click(world, "fullmap-save")?,
            11 => {
                map_click(world, "fullmap-layer")?;
                map_click(world, "fullmap-list")?;
            }
            12 => {
                map_change(
                    world,
                    "fullmap-name",
                    &format!("{} edited", smoke.waypoint_name),
                )?;
                map_click(world, "fullmap-save")?;
            }
            13 => map_click(world, "fullmap-delete")?,
            14 => map_click(world, "fullmap-delete-confirm")?,
            15 => map_click(world, "fullmap-out")?,
            16 => {
                smoke.pan_origin = world.resource::<FullMapView>().coordinates.clone();
                smoke.pan_started = Some(Instant::now());
                map_pointer(world, Vec2::splat(0.5))?;
                mouse_button(world, MouseButton::Left, true)?;
            }
            17 => {
                mouse_button(world, MouseButton::Left, false)?;
                map_click(world, "fullmap-center")?;
            }
            18 => {
                smoke.range_before_wheel = world.resource::<FullMapView>().range.clone();
                map_pointer(world, Vec2::splat(0.5))?;
                let window = primary_window(world)?;
                world.write_message(MouseWheel {
                    unit: MouseScrollUnit::Line,
                    x: 0.0,
                    y: 1.0,
                    window,
                    phase: bevy::input::touch::TouchPhase::Moved,
                });
            }
            19 => pulse_binding(world, smoke, GameAction::FullMap)?,
            20 => {
                resize(world, 960.0, 640.0, 1.5);
                smoke.pan_origin.clear();
                smoke.pan_progress = 0;
                smoke.pan_moved = false;
                smoke.pan_started = None;
                smoke.pan_locked.clear();
                smoke.scaled_form_stage = 0;
                pulse_binding(world, smoke, GameAction::FullMap)?;
                for control in ["fullmap-mode", "fullmap-cave"] {
                    world.write_message(terra_ui::minimap::fullmap::FullMapAction::Command {
                        control: control.into(),
                        id: None,
                    });
                }
            }
            21 => {
                pulse_binding(world, smoke, GameAction::Pause)?;
                world.resource_mut::<GameplayConfig>().minimap.fps = 30;
            }
            22 => {
                let origin = smoke
                    .origin
                    .ok_or("Missing surface spawn for boost flight")?;
                let target = Vec3::from_array(origin) + Vec3::Y * 20.0;
                smoke.flight_target = Some(target);
                smoke.generation = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .ok_or("Missing authoritative player before boost flight")?
                    .generation;
                world.resource_mut::<ClientChatCommands>().0.push_back(
                    terra_network::client::ChatCommand::Send(format!(
                        "/tp {:.2} {:.2} {:.2}",
                        target.x, target.y, target.z
                    )),
                );
                *world.resource_mut::<terra_player::ControllerMode>() =
                    terra_player::ControllerMode::Fly;
                // Native fixtures suppress OS grabs; retain the real gameplay input lock.
                world
                    .resource_mut::<terra_player::CameraInputState>()
                    .captured = true;
                resize(world, 1280.0, 900.0, 1.0);
                let mut gameplay = world.resource_mut::<GameplayConfig>();
                gameplay.minimap.mode = MapMode::Surface;
                gameplay.minimap.zoom = 0;
                gameplay.minimap.rotate = true;
                gameplay.minimap.relief = true;
                gameplay.minimap.fps = 24;
                info!(
                    ?target,
                    "MiniMap native test requested authoritative surface flight teleport"
                );
            }
            23 => {
                let player = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .ok_or("Missing authoritative boost flight origin")?;
                smoke.flight_origin = Some(Vec3::from_array(player.position));
                smoke.flight_heading = player.look[0];
                world
                    .resource_mut::<terra_player::CameraInputState>()
                    .captured = true;
                flight_controls(world, true)?;
                smoke.flight_climbing = true;
            }
            25 => {
                pulse_binding(world, smoke, GameAction::FullMap)?;
                world.write_message(terra_ui::minimap::fullmap::FullMapAction::Command {
                    control: "fullmap-surface".into(),
                    id: None,
                });
            }
            30 => pulse_binding(world, smoke, GameAction::FullMap)?,
            26 => map_click(world, "fullmap-add")?,
            27 => {
                let target = world
                    .query_filtered::<&GlobalTransform, With<terra_player::TerraCamera>>()
                    .iter(world)
                    .next()
                    .map(|camera| camera.translation() + camera.forward() * 1200.0 - Vec3::Y)
                    .ok_or("No camera for world waypoint fixture")?;
                map_change(
                    world,
                    "fullmap-name",
                    &format!("{} navigation", smoke.waypoint_name),
                )?;
                for (id, coordinate) in ["fullmap-x", "fullmap-y", "fullmap-z"]
                    .into_iter()
                    .zip(target.to_array())
                {
                    map_change(world, id, &format!("{coordinate:.3}"))?;
                }
                map_change(world, "fullmap-icon", "camp")?;
                map_change(world, "fullmap-world", "true")?;
            }
            28 => map_click(world, "fullmap-save")?,
            29 | 34 => pulse_binding(world, smoke, GameAction::FullMap)?,
            35 => {
                let target = smoke
                    .origin
                    .ok_or("Missing original ground spawn for live miniature")?;
                smoke.map3d_target = Some(target);
                smoke.generation = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .ok_or("Missing authoritative player before miniature teleport")?
                    .generation;
                world.resource_mut::<ClientChatCommands>().0.push_back(
                    terra_network::client::ChatCommand::Send(format!(
                        "/tp {:.3} {:.3} {:.3}",
                        target[0], target[1], target[2]
                    )),
                );
                info!(
                    ?target,
                    "Native live miniature returns to actual loaded ground after flight checks"
                );
            }
            31 => {
                point_pointer(world, &format!("{} navigation", smoke.waypoint_name), true)?;
                smoke.pointer_release = true;
            }
            32 => map_click(world, "fullmap-delete")?,
            33 => map_click(world, "fullmap-delete-confirm")?,
            36 | 47 => pulse_binding(world, smoke, GameAction::FullMap)?,
            37 => {
                smoke.map3d_wait_interval = 0;
                map_click(world, "fullmap-mode")?;
                map_click(world, "fullmap-grid")?;
                map_click(world, "fullmap-mobs")?;
            }
            38 => {
                smoke.map3d_wait_interval = 0;
                map_click(world, "fullmap-mobs")?;
                smoke.pan_origin = world.resource::<FullMapView>().coordinates.clone();
                smoke.pan_progress = 0;
                smoke.pan_moved = false;
                smoke.pan_started = Some(Instant::now());
                map_pointer(world, Vec2::splat(0.5))?;
                mouse_button(world, MouseButton::Left, true)?;
            }
            39 => {
                mouse_button(world, MouseButton::Left, false)?;
                map_click(world, "fullmap-mode")?;
            }
            40 | 51 => pulse_binding(world, smoke, GameAction::FullMap)?,
            41 => {
                smoke.quick_position = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .map(|s| s.position);
                pulse_binding(world, smoke, GameAction::QuickWaypoint)?;
            }
            42 => map_click(world, "fullmap-icon")?,
            43 => map_change(world, "fullmap-icon-search", "compass")?,
            44 => {
                ui_pointer(world, "fullmap-icon-option", None, true)?;
                smoke.pointer_release = true;
            }
            45 => map_change(
                world,
                "fullmap-name",
                &format!("{} quick", smoke.waypoint_name),
            )?,
            46 => map_click(world, "fullmap-save")?,
            48 => {
                point_pointer(world, &format!("{} quick", smoke.waypoint_name), true)?;
                smoke.pointer_release = true;
            }
            49 => map_click(world, "fullmap-delete")?,
            50 => map_click(world, "fullmap-delete-confirm")?,
            DONE => return Ok(true),
            _ => {}
        }
    }
    if smoke.stage == 23 {
        if !world.resource::<ClientSession>().allow_flight
            || *world.resource::<terra_player::ControllerMode>()
                != terra_player::ControllerMode::Fly
            || world.resource::<terra_player::PlayerInputBlock>().0
            || world.resource::<FullMapView>().visible
            || !world.resource::<MiniMapView>().visible
        {
            return Err(
                "Real boost flight lost flight authorization, its MiniMap or gameplay input".into(),
            );
        }
        world
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        let elapsed = smoke.since.elapsed();
        if smoke.flight_climbing && elapsed >= Duration::from_millis(700) {
            hold_binding(world, GameAction::Jump, false)?;
            smoke.flight_climbing = false;
        }
        if smoke.flight_samples < 3
            && elapsed >= Duration::from_millis(600 + u64::from(smoke.flight_samples) * 700)
        {
            let sensitivity = world.resource::<GameConfig>().client.mouse_sensitivity;
            world.write_message(MouseMotion {
                delta: Vec2::new(-0.12 / sensitivity, 0.0),
            });
            capture_flight(world, smoke.flight_samples)?;
            smoke.flight_samples += 1;
        }
        if !smoke.flight_released && elapsed >= Duration::from_millis(2600) {
            flight_controls(world, false)?;
            smoke.flight_released = true;
        }
        if !smoke.flight_released || smoke.flight_captures < 3 {
            return Ok(false);
        }
    }
    if smoke.stage == 20
        && smoke.pan_started.is_none()
        && smoke.scaled_form_stage == 2
        && smoke.since.elapsed() >= Duration::from_millis(400)
        && world.resource::<FullMapView>().visible
        && world.resource::<FullMapView>().cave
        && !world.resource::<FullMapView>().editing
        && !world.resource::<FullMapView>().deleting
    {
        smoke.pan_origin = world.resource::<FullMapView>().coordinates.clone();
        smoke.pan_started = Some(Instant::now());
        map_pointer(world, Vec2::splat(0.5))?;
        mouse_button(world, MouseButton::Left, true)?;
        return Ok(false);
    }
    if matches!(smoke.stage, 16 | 38) || (smoke.stage == 20 && smoke.pan_started.is_some()) {
        let elapsed = smoke
            .pan_started
            .map_or_else(|| smoke.since.elapsed(), |start| start.elapsed());
        let points = [
            Vec2::new(0.54, 0.53),
            Vec2::new(0.60, 0.56),
            Vec2::new(0.65, 0.62),
        ];
        if smoke.pan_progress < 3
            && elapsed >= Duration::from_millis(250 * u64::from(smoke.pan_progress + 1))
        {
            map_pointer(world, points[usize::from(smoke.pan_progress)])?;
            smoke.pan_progress += 1;
            if smoke.stage == 20 && smoke.pan_progress == 2 {
                world.write_message(terra_ui::minimap::fullmap::FullMapAction::Command {
                    control: "fullmap-mode".into(),
                    id: None,
                });
            }
            smoke.pan_moved = smoke.pan_progress == 3;
        } else if smoke.pan_progress == 3 && elapsed >= Duration::from_millis(1200) {
            smoke.pan_locked = world.resource::<FullMapView>().coordinates.clone();
            smoke.pan_progress = 4;
        } else if smoke.pan_progress >= 4 {
            if world.resource::<FullMapView>().coordinates != smoke.pan_locked {
                return Err(
                    "FullMap pan snapped back during cache updates or after mouse release".into(),
                );
            }
            if smoke.pan_progress == 4 && elapsed >= Duration::from_millis(1600) {
                mouse_button(world, MouseButton::Left, false)?;
                smoke.pan_progress = 5;
            }
        }
    }
    if smoke.stage == 11
        && !smoke.waypoint_select_requested
        && smoke.since.elapsed() >= Duration::from_millis(300)
        && world.resource::<FullMapView>().book_open
    {
        point_pointer(world, &smoke.waypoint_name, true)?;
        smoke.pointer_release = true;
        smoke.waypoint_select_requested = true;
    }
    if smoke.stage == 36 && !smoke.map3d_centered && world.resource::<FullMapView>().visible {
        map_click(world, "fullmap-center")?;
        smoke.map3d_centered = true;
    }
    if smoke.stage == 11
        && !smoke.waypoint_edit_requested
        && smoke.since.elapsed() >= Duration::from_millis(700)
        && world.resource::<FullMapView>().surface
    {
        map_click(world, "fullmap-edit")?;
        smoke.waypoint_edit_requested = true;
    }
    if smoke.stage == 20 && world.resource::<FullMapView>().visible {
        if smoke.scaled_form_stage == 0 && smoke.since.elapsed() >= Duration::from_millis(300) {
            map_click(world, "fullmap-add")?;
            smoke.scaled_form_stage = 1;
        } else if smoke.scaled_form_stage == 1
            && world.resource::<FullMapView>().editing
            && smoke.since.elapsed() >= Duration::from_millis(800)
        {
            validate_waypoint_editor_bounds(world)?;
            let path = std::env::var("TERRA_SCREENSHOT_DIR")
                .unwrap_or_else(|_| "/tmp/terra70-minimap".into());
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!("{path}/map-20-editor.png")));
            map_click(world, "fullmap-cancel")?;
            smoke.scaled_form_stage = 2;
        }
        if smoke.scaled_form_stage < 2 || world.resource::<FullMapView>().editing {
            return Ok(false);
        }
    }
    if smoke.stage == 6
        && !smoke.tool_hover_requested
        && smoke.since.elapsed() >= Duration::from_millis(800)
        && world.resource::<FullMapView>().visible
    {
        hover_map_tool(world, "fullmap-grid")?;
        smoke.tool_hover_requested = true;
    }
    if smoke.since.elapsed() < Duration::from_secs(if smoke.stage == 0 { 12 } else { 3 }) {
        return Ok(false);
    }
    if smoke.stage == 5 {
        let session = world.resource::<ClientSession>();
        let Some(state) = session.confirmed_state() else {
            return Ok(false);
        };
        let position = Vec3::from_array(state.position);
        if session.teleporting
            || state.generation <= smoke.generation
            || smoke
                .cave
                .is_none_or(|target| target.distance(position) > 3.0)
        {
            return Ok(false);
        }
    }
    if matches!(smoke.stage, 22 | 35) {
        let session = world.resource::<ClientSession>();
        let Some(player) = session.confirmed_state() else {
            return Ok(false);
        };
        if session.teleporting
            || player.generation <= smoke.generation
            || (if smoke.stage == 35 {
                smoke.map3d_target.map(Vec3::from_array)
            } else {
                smoke.flight_target
            })
            .is_none_or(|target| target.distance(Vec3::from_array(player.position)) > 3.0)
        {
            return Ok(false);
        }
        if smoke.stage == 35
            && world
                .resource::<terra_render::PropRenderStatus>()
                .visible_props
                == 0
        {
            return Ok(false);
        }
    }
    if smoke.stage == 24 {
        let player = world
            .resource::<ClientSession>()
            .confirmed_state()
            .ok_or("Missing authoritative player after boost flight")?;
        let origin = smoke
            .flight_origin
            .ok_or("Missing boost flight starting point")?;
        let position = Vec3::from_array(player.position);
        let distance = Vec2::new(position.x - origin.x, position.z - origin.z).length();
        let yaw = (player.look[0] - smoke.flight_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        if distance <= 60.0 || (position.y - origin.y).abs() < 5.0 || yaw.abs() < 0.1 {
            return Err(format!(
                "Boost flight did not travel/turn/climb through real input: distance={distance:.2}, height={:.2}, yaw={yaw:.3}",
                position.y - origin.y
            ));
        }
        info!(
            distance,
            altitude_change = position.y - origin.y,
            yaw,
            captures = smoke.flight_captures,
            "Real boosted flight with nonblank rendered MiniMap samples passed"
        );
    }
    if (6..=18).contains(&smoke.stage)
        || smoke.stage == 20
        || (25..=28).contains(&smoke.stage)
        || (30..=33).contains(&smoke.stage)
        || (36..=39).contains(&smoke.stage)
        || (47..=50).contains(&smoke.stage)
    {
        if !validate_fullmap(world, smoke)? {
            return Ok(false);
        }
        capture(world, smoke)?;
        return Ok(false);
    }
    if (41..=45).contains(&smoke.stage) {
        validate_quick_waypoint(world, smoke)?;
        capture(world, smoke)?;
        return Ok(false);
    }
    if smoke.stage == 46 {
        let book = waypoint_book(world)?;
        let point = book
            .entries
            .iter()
            .find(|point| point.name == format!("{} quick", smoke.waypoint_name))
            .ok_or("Quick waypoint did not persist")?;
        if point.icon.key() != "compass"
            || !point.show_in_world
            || book.entries.len() != smoke.waypoint_count + 1
        {
            return Err(
                "Quick waypoint catalog icon/default world visibility did not persist".into(),
            );
        }
        smoke.waypoint_id = Some(point.id);
    }
    if matches!(smoke.stage, 19 | 21 | 29 | 34 | 35 | 40 | 46 | 51)
        && (world.resource::<FullMapView>().visible
            || world.resource::<terra_player::PlayerInputBlock>().0)
    {
        return Err(
            "Closing FullMap must release the gameplay lock and preserve Playing state".into(),
        );
    }
    let view = world.resource::<MiniMapView>();
    if smoke.stage == 29 {
        let id = smoke
            .waypoint_id
            .ok_or("Missing navigation waypoint ID")?
            .to_string();
        let Some(point) = view.waypoints.iter().find(|point| point.id == id) else {
            return Ok(false);
        };
        if !point.pinned || point.icon != "camp" || !point.distance.is_empty() || point.show_label {
            return Err(format!(
                "Far MiniMap waypoint must pin its icon without a label or distance: {point:?}"
            ));
        }
        if !world
            .resource::<WorldWaypointView>()
            .points
            .iter()
            .any(|point| point.id == id && point.icon == "camp" && point.distance == "1.2km")
        {
            return Err(
                "Enabled distant waypoint label is missing from the actual camera view".into(),
            );
        }
    }
    if smoke.stage == 35 {
        let id = smoke
            .waypoint_id
            .ok_or("Missing removed navigation waypoint ID")?
            .to_string();
        if view.waypoints.iter().any(|point| point.id == id)
            || world
                .resource::<WorldWaypointView>()
                .points
                .iter()
                .any(|point| point.id == id)
            || waypoint_book(world)?.entries.len() != smoke.waypoint_count
        {
            return Err("Deleted waypoint must disappear from both maps and the world HUD".into());
        }
    }
    if !view.visible
        || (view.expanded == (smoke.stage == 2))
        || !view.error.is_empty()
        || (view.cave != matches!(smoke.stage, 5 | 19 | 21))
    {
        return Ok(false);
    }
    if smoke.stage == 4 {
        let mobs = world.resource::<ReplicatedCreatures>().creatures.len();
        if view.players == 0 || view.mobs == 0 || mobs == 0 {
            return Ok(false);
        }
        if view.peers.is_empty() || view.peers.iter().any(|peer| peer.name.is_empty()) {
            return Err("Replicated map players need visible identifying labels".into());
        }
        info!(
            mobs = view.mobs,
            players = view.players,
            "Real replicated map markers checked"
        );
    }
    if smoke.stage == 0 {
        smoke.origin = world
            .resource::<ClientSession>()
            .confirmed_state()
            .map(|s| s.position);
    }
    if smoke.stage != 2 {
        if view.biome.is_empty() || view.clock.is_empty() {
            return Ok(false);
        }
        let nodes: Vec<_> = world
            .query::<(&ElementId, &ComputedNode, &ImageNode)>()
            .iter(world)
            .filter(|(id, _, _)| id.0 == "map-surface")
            .map(|(_, node, image)| (node.size() * node.inverse_scale_factor, image.image.clone()))
            .collect();
        let Some((size, handle)) = nodes.first() else {
            return Ok(false);
        };
        if size.min_element() < 100.0 || size.max_element() > 360.0 || (size.x - size.y).abs() > 2.0
        {
            return Ok(false);
        }
        let Some(image) = world.resource::<Assets<Image>>().get(handle) else {
            return Ok(false);
        };
        if image.texture_descriptor.size.width != 320 || image.texture_descriptor.size.height != 320
        {
            return Ok(false);
        }
        let Some(pixels) = image.data.as_ref() else {
            return Ok(false);
        };
        let terrain = pixels
            .chunks_exact(4)
            .filter(|p| p[0].max(p[1]).max(p[2]) > 42 && p[3] == 255)
            .count();
        let colors: std::collections::HashSet<_> = pixels
            .chunks_exact(4)
            .map(|p| [p[0] / 8, p[1] / 8, p[2] / 8])
            .collect();
        if terrain < 100 || colors.len() < 8 {
            return Ok(false);
        }
        info!(
            stage = smoke.stage,
            ?size,
            terrain,
            colors = colors.len(),
            "MiniMap layout and nonblank relief checked"
        );
    }
    capture(world, smoke)?;
    Ok(false)
}

fn capture(world: &mut World, smoke: &mut Smoke) -> Result<(), String> {
    let path =
        std::env::var("TERRA_SCREENSHOT_DIR").unwrap_or_else(|_| "/tmp/terra70-minimap".into());
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let stage = smoke.stage;
    let check_3d = if matches!(stage, 37 | 38) {
        world
            .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
            .iter(world)
            .find(|(id, _, _)| id.0 == "full-map-viewport")
            .map(|(_, node, transform)| {
                (
                    transform.transform_point2(Vec2::ZERO),
                    node.size().min_element() * 0.32,
                )
            })
    } else {
        None
    };
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{path}/map-{stage}.png")))
        .observe(
            move |event: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
                smoke.captures += 1;
                if let Some((center, radius)) = check_3d {
                    match nonblank_flight_map(&event.image, center, radius) {
                        Ok((terrain, colors)) => {
                            smoke.map3d_captures += 1;
                            info!(
                                stage,
                                terrain, colors, "Native 3D FullMap rendered pixels checked"
                            );
                        }
                        Err(error) => {
                            smoke.flight_error = Some(format!("3D FullMap rendered blank: {error}"))
                        }
                    }
                }
            },
        );
    smoke.requested = true;
    Ok(())
}

fn capture_flight(world: &mut World, sample: u8) -> Result<(), String> {
    let (center, radius) = world
        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .find(|(id, node, _)| id.0 == "map-surface" && !node.is_empty())
        .map(|(_, node, transform)| {
            (
                transform.transform_point2(Vec2::ZERO),
                node.size().min_element() * 0.34,
            )
        })
        .ok_or("MiniMap disappeared during real boost flight")?;
    let path =
        std::env::var("TERRA_SCREENSHOT_DIR").unwrap_or_else(|_| "/tmp/terra70-minimap".into());
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    world.spawn(Screenshot::primary_window())
        .observe(save_to_disk(format!("{path}/map-23-flight-{sample}.png")))
        .observe(move |event: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
            match nonblank_flight_map(&event.image, center, radius) {
                Ok((terrain, colors)) => {
                    info!(sample, terrain, colors, "Rendered MiniMap remains nonblank during real boost flight");
                    smoke.flight_captures += 1;
                }
                Err(error) => smoke.flight_error = Some(error),
            }
        });
    Ok(())
}

fn nonblank_flight_map(image: &Image, center: Vec2, radius: f32) -> Result<(usize, usize), String> {
    let image = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?
        .to_rgb8();
    let low = (center - Vec2::splat(radius)).max(Vec2::ZERO).as_uvec2();
    let high = (center + Vec2::splat(radius))
        .min(Vec2::new(image.width() as f32, image.height() as f32))
        .as_uvec2();
    let mut terrain = 0;
    let mut colors = std::collections::HashSet::new();
    for y in (low.y..high.y).step_by(2) {
        for x in (low.x..high.x).step_by(2) {
            let distance = (Vec2::new(x as f32, y as f32) - center).length();
            if distance > radius || distance < radius * 0.24 {
                continue;
            }
            let color = image.get_pixel(x, y).0;
            if color.into_iter().max().unwrap_or(0) > 42 {
                terrain += 1;
                colors.insert(color.map(|value| value / 8));
            }
        }
    }
    if terrain < 100 || colors.len() < 8 {
        return Err(format!(
            "Rendered MiniMap became blank during real boost flight: terrain={terrain}, colors={}",
            colors.len()
        ));
    }
    Ok((terrain, colors.len()))
}

fn map3d_wait_diagnostic(world: &mut World, smoke: &mut Smoke, reason: &str) {
    let interval = smoke.since.elapsed().as_secs() / 5;
    if interval == 0 || interval <= smoke.map3d_wait_interval {
        return;
    }
    smoke.map3d_wait_interval = interval;
    let status = world
        .resource::<terra_ui::minimap::Map3dRenderStatus>()
        .clone();
    let player = world
        .resource::<ClientSession>()
        .confirmed_state()
        .map(|p| p.position);
    let transform = status
        .camera
        .and_then(|camera| world.get::<GlobalTransform>(camera))
        .copied();
    let projection = status
        .camera
        .and_then(|camera| world.get::<Projection>(camera))
        .cloned();
    let target_size = status
        .camera
        .and_then(|camera| world.get::<Camera>(camera))
        .and_then(Camera::logical_target_size);
    info!(
        stage = smoke.stage,
        reason,
        ?status,
        ?player,
        ?transform,
        ?projection,
        ?target_size,
        "Native live miniature still waiting for actual visible world models"
    );
    if interval == 1 {
        let path =
            std::env::var("TERRA_SCREENSHOT_DIR").unwrap_or_else(|_| "/tmp/terra70-minimap".into());
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(format!(
                "{path}/map-{}-waiting.png",
                smoke.stage
            )));
    }
}

fn validate_fullmap(world: &mut World, smoke: &mut Smoke) -> Result<bool, String> {
    if world.resource::<FullMapView>().visible {
        validate_tool_rail(world)?;
        if smoke.stage == 20 {
            let disabled = world
                .query::<(
                    &ElementId,
                    Option<&bevy::ui::InteractionDisabled>,
                    Option<&tilt_ui::ElementState>,
                )>()
                .iter(world)
                .any(|(id, interaction, state)| {
                    id.0 == "fullmap-mode"
                        && interaction.is_some()
                        && state.is_some_and(|s| s.disabled)
                });
            if !disabled {
                return Err("The cave 3D tool must be genuinely disabled in TiltUI, not only visually dimmed".into());
            }
        }
        if smoke.stage == 6 {
            let tooltip = world
                .query::<(&ElementId, &Node, &ComputedNode)>()
                .iter(world)
                .any(|(id, node, layout)| {
                    id.0 == "fullmap-grid-tooltip"
                        && node.display != Display::None
                        && layout.size().min_element() > 12.0
                });
            if !tooltip {
                return Err("Hovering a map tool did not show its TiltUI tooltip".into());
            }
        }
    }
    let view = world.resource::<FullMapView>();
    if !view.visible {
        return Ok(false);
    }
    if !view.error.is_empty() {
        return Err(view.error.clone());
    }
    if !world.resource::<terra_player::PlayerInputBlock>().0
        || world.resource::<MiniMapView>().visible
    {
        return Err("FullMap must hide the MiniMap and block gameplay input".into());
    }
    let editing = view.editing;
    let expected_size = Vec2::new(view.map_width, view.map_height);
    let three_dimensional = view.three_dimensional;
    if smoke.stage == 20 {
        if !view.cave || view.three_dimensional || view.three_dimensional_available {
            return Err(
                "Cave maps must reject 3D even when requested through a queued action".into(),
            );
        }
        if world
            .resource::<terra_ui::minimap::Map3dRenderStatus>()
            .active
            || world
                .resource::<terra_core::miniature::MiniatureView>()
                .active
        {
            return Err("Cave map left the miniature render path active".into());
        }
        if smoke.pan_progress < 5 {
            return Ok(false);
        }
        if !smoke.pan_moved || view.coordinates == smoke.pan_origin {
            return Err(
                "Held cave-map drag did not move after the waypoint dialog was closed".into(),
            );
        }
    }
    match smoke.stage {
        36 if !smoke.map3d_centered || view.three_dimensional => return Ok(false),
        37 if !view.three_dimensional || !view.grid || view.mobs => return Ok(false),
        38 if !view.three_dimensional
            || !smoke.pan_moved
            || view.coordinates == smoke.pan_origin =>
        {
            return Ok(false);
        }
        39 if view.three_dimensional || smoke.map3d_captures < 2 => return Ok(false),
        47 if view.editing => return Ok(false),
        48 if !view.selected => return Ok(false),
        49 if !view.deleting => return Ok(false),
        50 if waypoint_book(world)?.entries.len() != smoke.waypoint_count
            || view.selected
            || view.deleting =>
        {
            return Err("Quick waypoint deletion did not preserve prior annotations".into());
        }
        25 if !view.surface || view.editing => return Ok(false),
        26 if !view.editing => return Ok(false),
        27 if !view.editing
            || view.icon != "camp"
            || !view.show_in_world
            || view.name != format!("{} navigation", smoke.waypoint_name) =>
        {
            return Ok(false);
        }
        28 => {
            let book = waypoint_book(world)?;
            let Some(point) = book
                .entries
                .iter()
                .find(|point| point.name == format!("{} navigation", smoke.waypoint_name))
            else {
                return Ok(false);
            };
            if book.entries.len() != smoke.waypoint_count + 1
                || point.icon != terra_storage::waypoints::WaypointIcon::Camp
                || !point.show_in_world
                || view.editing
            {
                return Err("Navigation waypoint options failed to save".into());
            }
            if !view.markers.iter().any(|marker| {
                marker.id == point.id.to_string() && marker.icon == "camp" && marker.pinned
            }) {
                return Err("FullMap is missing the distant waypoint icon and edge pin".into());
            }
            smoke.waypoint_id = Some(point.id);
        }
        30 if view.editing => return Ok(false),
        31 if !view.selected => return Ok(false),
        32 if !view.deleting => return Ok(false),
        33 if waypoint_book(world)?.entries.len() != smoke.waypoint_count
            || view.selected
            || view.deleting =>
        {
            return Err(
                "Navigation waypoint deletion did not preserve previous annotations".into(),
            );
        }
        6 if !view.automatic => return Ok(false),
        7 if !view.surface => return Ok(false),
        8 if !view.editing => return Ok(false),
        9 if !view.editing
            || view.name != smoke.waypoint_name
            || view.color != "#d594eb"
            || view.icon != "home"
            || !view.show_in_world =>
        {
            return Ok(false);
        }
        10 => {
            let book = waypoint_book(world)?;
            let Some(point) = book
                .entries
                .iter()
                .find(|point| point.name == smoke.waypoint_name)
            else {
                return Ok(false);
            };
            if book.entries.len() != smoke.waypoint_count + 1 || !view.selected || view.editing {
                return Err(
                    "FullMap waypoint create must save and select exactly one new point".into(),
                );
            }
            if point.icon != terra_storage::waypoints::WaypointIcon::Home || !point.show_in_world {
                return Err("Waypoint icon/world visibility were not persisted".into());
            }
            if !view.markers.iter().any(|marker| {
                marker.id == point.id.to_string()
                    && marker.name == point.name
                    && marker.icon == "home"
            }) {
                return Err("FullMap must show the selected waypoint's own icon and label".into());
            }
            smoke.waypoint_id = Some(point.id);
        }
        11 if !view.editing || !view.surface || !smoke.waypoint_edit_requested => return Ok(false),
        12 => {
            let book = waypoint_book(world)?;
            let Some(point) = book
                .entries
                .iter()
                .find(|point| Some(point.id) == smoke.waypoint_id)
            else {
                return Ok(false);
            };
            if point.name != format!("{} edited", smoke.waypoint_name) || view.editing {
                return Ok(false);
            }
            if book.entries.len() != smoke.waypoint_count + 1 {
                return Err("Editing a waypoint must retain its ID and count".into());
            }
        }
        13 if !view.deleting || view.editing => return Ok(false),
        14 => {
            let book = waypoint_book(world)?;
            if book.entries.len() != smoke.waypoint_count
                || book
                    .entries
                    .iter()
                    .any(|point| Some(point.id) == smoke.waypoint_id)
                || view.selected
                || view.deleting
            {
                return Err("Deleting the waypoint must preserve all previous annotations".into());
            }
        }
        15 if view.range != "768 m" => return Ok(false),
        16 if !smoke.pan_moved || view.coordinates == smoke.pan_origin => return Ok(false),
        17 => {
            let state = world
                .resource::<ClientSession>()
                .confirmed_state()
                .ok_or("Missing player for map centering")?;
            let [x, y, z] = state.position;
            if view.coordinates != format!("X {x:.0}  Y {y:.0}  Z {z:.0}") {
                return Ok(false);
            }
        }
        18 if view.range == smoke.range_before_wheel => return Ok(false),
        _ => {}
    }
    if editing {
        validate_waypoint_editor_bounds(world)?;
    }
    if matches!(smoke.stage, 10 | 28) {
        let glyphs: Vec<_> = world
            .query::<(&ElementId, &ComputedNode)>()
            .iter(world)
            .filter(|(id, _)| id.0 == "waypoint-glyph")
            .map(|(_, node)| node.size() * node.inverse_scale_factor)
            .collect();
        if glyphs.is_empty()
            || glyphs.iter().any(|size| {
                ((size.x - 14.0).abs() > 1.1 && (size.x - 18.0).abs() > 1.1)
                    || (size.x - size.y).abs() > 1.1
            })
        {
            return Err(format!(
                "Waypoint symbols must be14px on maps or18px in list rows: {glyphs:?}"
            ));
        }
    }
    let nodes: Vec<_> = world
        .query::<(
            &ElementId,
            &ComputedNode,
            &ImageNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .filter(|(id, _, _, _)| id.0 == "full-map-surface")
        .map(|(_, node, image, transform)| {
            (
                node.size() * node.inverse_scale_factor,
                image.image.clone(),
                transform.transform_point2(-node.size() * 0.5),
                transform.transform_point2(node.size() * 0.5),
            )
        })
        .collect();
    let Some((size, handle, corner, end)) = nodes.first() else {
        return Ok(false);
    };
    if size.min_element() < 100.0 || size.distance(expected_size) > 3.0 {
        return Err(format!(
            "FullMap viewport must use the available rectangular area: {size:?}, expected {expected_size:?}"
        ));
    }
    let window = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing FullMap window")?;
    let viewport = Vec2::new(
        window.resolution.physical_width() as f32,
        window.resolution.physical_height() as f32,
    );
    if corner.min_element() < -2.0 || end.x > viewport.x + 2.0 || end.y > viewport.y + 2.0 {
        return Err("FullMap canvas extends outside the native viewport".into());
    }
    let Some(image) = world.resource::<Assets<Image>>().get(handle) else {
        return Ok(false);
    };
    if three_dimensional {
        let expected_height = (1024.0 * expected_size.y / expected_size.x)
            .round()
            .clamp(256.0, 2048.0) as u32;
        if image.texture_descriptor.size.width != 1024
            || image.texture_descriptor.size.height != expected_height
        {
            map3d_wait_diagnostic(
                world,
                smoke,
                "render target has not reached expected rectangular dimensions",
            );
            return Ok(false);
        }
        let status = world.resource::<terra_ui::minimap::Map3dRenderStatus>();
        if !status.active || status.terrain_meshes == 0 || status.player_meshes == 0 {
            map3d_wait_diagnostic(
                world,
                smoke,
                "live miniature camera or source terrain/player meshes not ready",
            );
            return Ok(false);
        }
        if !status.environment_map_shared {
            map3d_wait_diagnostic(
                world,
                smoke,
                "shared gameplay environment lighting not ready",
            );
            return Ok(false);
        }
        if world
            .get_resource::<terra_core::GraphicsConfig>()
            .is_some_and(|config| config.shadow_quality != terra_core::ShadowQuality::Off)
            && (status.shadow_cascades == 0 || status.shadow_depth_band.is_none())
        {
            map3d_wait_diagnostic(world, smoke, "miniature shadow depth band not ready");
            return Ok(false);
        }
        let Some(visible) = status
            .camera
            .and_then(|camera| world.get::<bevy::camera::visibility::VisibleEntities>(camera))
        else {
            map3d_wait_diagnostic(
                world,
                smoke,
                "miniature camera has no VisibleEntities component",
            );
            return Ok(false);
        };
        let mut terrain = 0;
        let mut players = 0;
        let mut creatures = 0;
        let mut props = 0;
        for entity in visible.iter(std::any::TypeId::of::<Mesh3d>()) {
            if world
                .get::<Name>(*entity)
                .is_some_and(|name| name.as_str() == "FullMap explored terrain cache")
            {
                return Err("The artificial far-terrain curtain must not be rendered".into());
            }
            match world.get::<terra_core::miniature::MiniatureMesh>(*entity) {
                Some(terra_core::miniature::MiniatureMesh::Terrain) => terrain += 1,
                Some(terra_core::miniature::MiniatureMesh::Player) => players += 1,
                Some(terra_core::miniature::MiniatureMesh::Creature) => creatures += 1,
                Some(terra_core::miniature::MiniatureMesh::Prop) => props += 1,
                _ => (),
            }
        }
        if terrain == 0 || players == 0 || props == 0 {
            map3d_wait_diagnostic(
                world,
                smoke,
                &format!(
                    "visible terrain={terrain}, players={players}, props={props}, creatures={creatures}"
                ),
            );
            return Ok(false);
        }
        if smoke.stage == 37 && creatures != 0 {
            return Err("The live miniature mob toggle did not filter actual animal models".into());
        }
        if smoke.stage == 38 && status.creature_meshes > 0 && creatures == 0 {
            map3d_wait_diagnostic(
                world,
                smoke,
                "mob models exist but none is actually visible after enabling mobs",
            );
            return Ok(false);
        }
        info!(
            terrain,
            players,
            creatures,
            props,
            shadow_cascades = status.shadow_cascades,
            shadow_depth = ?status.shadow_depth_band,
            shared_environment = status.environment_map_shared,
            "Live miniature uses actual gameplay meshes and view visibility"
        );
        return Ok(true);
    }
    if image.texture_descriptor.size.width != 640 || image.texture_descriptor.size.height != 640 {
        return Ok(false);
    }
    let Some(pixels) = image.data.as_ref() else {
        return Ok(false);
    };
    let terrain = pixels
        .chunks_exact(4)
        .filter(|p| p[0].max(p[1]).max(p[2]) > 42 && p[3] == 255)
        .count();
    if terrain < 100 {
        let expected_unexplored = matches!(smoke.stage, 31..=33)
            && pixels
                .chunks_exact(4)
                .all(|pixel| pixel == [20, 26, 25, 255]);
        if !expected_unexplored {
            return Ok(false);
        }
    }
    info!(
        stage = smoke.stage,
        ?size,
        terrain,
        "FullMap native canvas, controls and persistence checked"
    );
    Ok(true)
}

fn validate_quick_waypoint(world: &mut World, smoke: &Smoke) -> Result<(), String> {
    let view = world.resource::<FullMapView>();
    let picker = view.icon_picker;
    if view.visible
        || !view.editing
        || !view.show_in_world
        || !world.resource::<terra_player::PlayerInputBlock>().0
    {
        return Err("N must open only the independent waypoint modal and block gameplay".into());
    }
    let expected = smoke
        .quick_position
        .ok_or("Quick waypoint player position missing")?;
    for (value, expected) in [&view.x, &view.y, &view.z].into_iter().zip(expected) {
        let value = value
            .parse::<f64>()
            .map_err(|_| "Quick waypoint coordinate is invalid")?;
        if (value - f64::from(expected)).abs() > 0.001 {
            return Err("Quick waypoint defaults do not match exact player XYZ".into());
        }
    }
    match smoke.stage {
        42 if !view.icon_picker || view.icons.len() != tilt_ui::Icon::ALL.len() => {
            return Err("Waypoint icon button did not open the actual TiltUI catalog".into());
        }
        43 if !view.icon_picker
            || view.icons.is_empty()
            || view.icons.iter().any(|icon| !icon.key.contains("compass")) =>
        {
            return Err("Waypoint icon search did not filter the real catalog".into());
        }
        44 | 45
            if view.icon_picker
                || view.icon != "compass"
                || view.icon_source != "tilt-icon:compass@32" =>
        {
            return Err("Real catalog icon click did not update and close the icon picker".into());
        }
        _ => {}
    }
    if picker {
        let nodes: Vec<_> = world
            .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
            .iter(world)
            .filter(|(id, _, _)| {
                matches!(
                    id.0.as_str(),
                    "waypoint-icon-picker"
                        | "fullmap-icon-search"
                        | "fullmap-icons-close"
                        | "fullmap-icon-option"
                )
            })
            .map(|(id, node, transform)| {
                (
                    id.0.clone(),
                    Rect::from_corners(
                        transform.transform_point2(-node.size() * 0.5),
                        transform.transform_point2(node.size() * 0.5),
                    ),
                )
            })
            .collect();
        let panel = nodes
            .iter()
            .find(|(id, _)| id == "waypoint-icon-picker")
            .map(|(_, rect)| rect)
            .ok_or("Waypoint icon picker has no native layout")?;
        for (id, rect) in &nodes {
            if rect.min.x < panel.min.x - 1.0
                || rect.max.x > panel.max.x + 1.0
                || rect.min.y < panel.min.y - 1.0
            {
                return Err(format!(
                    "Icon picker control overflows its panel: {id} {rect:?}"
                ));
            }
        }
        Ok(())
    } else {
        validate_waypoint_editor_bounds(world)
    }
}

fn validate_waypoint_editor_bounds(world: &mut World) -> Result<(), String> {
    let nodes: std::collections::HashMap<_, _> = world
        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(id, _, _)| {
            matches!(
                id.0.as_str(),
                "waypoint-dialog"
                    | "fullmap-name"
                    | "fullmap-x"
                    | "fullmap-y"
                    | "fullmap-z"
                    | "fullmap-color"
                    | "fullmap-icon"
                    | "fullmap-world"
                    | "fullmap-save"
                    | "fullmap-cancel"
            )
        })
        .map(|(id, node, transform)| {
            (
                id.0.clone(),
                Rect::from_corners(
                    transform.transform_point2(-node.size() * 0.5),
                    transform.transform_point2(node.size() * 0.5),
                ),
            )
        })
        .collect();
    let panel = nodes
        .get("waypoint-dialog")
        .ok_or("Independent waypoint dialog has no layout")?;
    for id in [
        "fullmap-name",
        "fullmap-x",
        "fullmap-y",
        "fullmap-z",
        "fullmap-color",
        "fullmap-icon",
        "fullmap-world",
        "fullmap-save",
        "fullmap-cancel",
    ] {
        let rect = nodes
            .get(id)
            .ok_or_else(|| format!("FullMap editor control has no layout: {id}"))?;
        if rect.min.x < panel.min.x - 1.0
            || rect.max.x > panel.max.x - 2.0
            || rect.min.y < panel.min.y
            || rect.max.y > panel.max.y - 2.0
        {
            return Err(format!(
                "Waypoint dialog control exceeds panel bounds: {id} {rect:?}"
            ));
        }
    }
    for pair in ["fullmap-x", "fullmap-y", "fullmap-z"].windows(2) {
        if nodes[pair[0]].max.x > nodes[pair[1]].min.x + 1.0 {
            return Err("FullMap coordinate inputs overlap".into());
        }
    }
    // Header and footer both cancel; validate every instance, not only the last ID.
    for (_, node, transform) in world
        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(id, _, _)| id.0 == "fullmap-cancel")
    {
        let rect = Rect::from_corners(
            transform.transform_point2(-node.size() * 0.5),
            transform.transform_point2(node.size() * 0.5),
        );
        if rect.min.x < panel.min.x - 1.0
            || rect.max.x > panel.max.x + 1.0
            || rect.min.y < panel.min.y - 1.0
            || rect.max.y > panel.max.y + 1.0
        {
            return Err(format!(
                "Waypoint close/cancel button exceeds dialog bounds: {rect:?}"
            ));
        }
    }
    Ok(())
}

fn waypoint_book(world: &World) -> Result<terra_storage::waypoints::WaypointBook, String> {
    let config = world.resource::<GameConfig>();
    let player = world
        .resource::<ClientSession>()
        .uuid
        .ok_or("Missing authoritative map player UUID")?;
    let identity = terra_storage::maps::MapIdentity {
        world: config.world.id,
        seed: config.world.seed,
        generator: config.world.generator_version,
        player,
    };
    let root = identity.directory(&world.resource::<MenuPaths>().configs.join("maps"));
    terra_storage::waypoints::WaypointBook::load_directory(&root)
}

fn primary_window(world: &mut World) -> Result<Entity, String> {
    world
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or_else(|| "Missing primary map window".into())
}

fn key_event(world: &mut World, key: KeyCode, pressed: bool) {
    let Ok(window) = primary_window(world) else {
        return;
    };
    world.write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Character("map-test".into()),
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat: false,
        window,
    });
}

fn pulse_binding(world: &mut World, smoke: &mut Smoke, action: GameAction) -> Result<(), String> {
    let binding = world
        .resource::<terra_core::ControlsConfig>()
        .binding(action);
    let terra_core::InputBinding::Key(key) = binding else {
        return Err(format!(
            "Native map fixture needs a keyboard binding for {}",
            action.label()
        ));
    };
    key_event(world, key, true);
    smoke.pending_key = Some(key);
    Ok(())
}

fn hold_binding(world: &mut World, action: GameAction, pressed: bool) -> Result<(), String> {
    let binding = world
        .resource::<terra_core::ControlsConfig>()
        .binding(action);
    match binding {
        terra_core::InputBinding::Key(key) => key_event(world, key, pressed),
        terra_core::InputBinding::Mouse(button) => mouse_button(world, button, pressed)?,
        terra_core::InputBinding::Chord { modifier, key } => {
            key_event(world, modifier, pressed);
            key_event(world, key, pressed);
        }
    }
    Ok(())
}

fn flight_controls(world: &mut World, pressed: bool) -> Result<(), String> {
    for action in [GameAction::Forward, GameAction::Sprint, GameAction::Jump] {
        hold_binding(world, action, pressed)?;
    }
    Ok(())
}

fn mouse_button(world: &mut World, button: MouseButton, pressed: bool) -> Result<(), String> {
    let window = primary_window(world)?;
    world.write_message(MouseButtonInput {
        button,
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        window,
    });
    Ok(())
}

fn map_pointer(world: &mut World, uv: Vec2) -> Result<(), String> {
    let target = world
        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .find(|(id, _, _)| id.0 == "full-map-viewport")
        .map(|(_, node, transform)| {
            transform.transform_point2((uv - Vec2::splat(0.5)) * node.size())
        })
        .ok_or("FullMap viewport is not materialized")?;
    let window = primary_window(world)?;
    let (position, delta) = {
        let mut viewport = world
            .get_mut::<Window>(window)
            .ok_or("Primary map window disappeared")?;
        let position = target / viewport.resolution.scale_factor();
        let delta = viewport
            .cursor_position()
            .map(|previous| position - previous);
        viewport
            .bypass_change_detection()
            .set_physical_cursor_position(Some(target.as_dvec2()));
        (position, delta)
    };
    world.write_message(bevy::window::CursorMoved {
        window,
        position,
        delta,
    });
    Ok(())
}

fn validate_tool_rail(world: &mut World) -> Result<(), String> {
    let ids = [
        "fullmap-add",
        "fullmap-grid",
        "fullmap-layer",
        "fullmap-mobs",
        "fullmap-mode",
        "fullmap-center",
        "fullmap-list",
    ];
    let sizes: Vec<_> = world
        .query::<(&ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(id, _)| ids.contains(&id.0.as_str()))
        .map(|(_, layout)| layout.size() * layout.inverse_scale_factor)
        .collect();
    if sizes.len() != ids.len()
        || sizes.iter().any(|size| {
            (size.x - size.y).abs() > 1.0
                || (size.x - sizes[0].x).abs() > 1.0
                || !(40.0..=66.0).contains(&size.x)
        })
    {
        return Err(format!(
            "Map tools must be seven equal fixed squares: {sizes:?}"
        ));
    }
    Ok(())
}

fn hover_map_tool(world: &mut World, id: &str) -> Result<(), String> {
    let target = world
        .query::<(&ElementId, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .find(|(element, _)| element.0 == id)
        .map(|(_, transform)| transform.translation)
        .ok_or_else(|| format!("Missing map hover target: {id}"))?;
    let window = primary_window(world)?;
    let (position, delta) = {
        let mut viewport = world
            .get_mut::<Window>(window)
            .ok_or("Missing map window")?;
        let position = target / viewport.resolution.scale_factor();
        let delta = viewport
            .cursor_position()
            .map(|previous| position - previous);
        viewport
            .bypass_change_detection()
            .set_physical_cursor_position(Some(target.as_dvec2()));
        (position, delta)
    };
    let event = bevy::window::CursorMoved {
        window,
        position,
        delta,
    };
    world.write_message(event.clone());
    // Picking reads the aggregate window channel, unlike the map drag handler.
    world.write_message(bevy::window::WindowEvent::CursorMoved(event));
    Ok(())
}

fn resize(world: &mut World, width: f32, height: f32, scale: f32) {
    for mut window in world.query::<&mut Window>().iter_mut(world) {
        window.resolution.set(width, height);
    }
    world.resource_mut::<terra_core::GraphicsConfig>().ui_scale = scale;
}

fn companion_destination(chunks: &ChunkStore, origin: Vec3) -> Vec3 {
    for offset in [[8, 0], [-8, 0], [0, 8], [0, -8]] {
        let x = origin.x.floor() as i64 + offset[0];
        let z = origin.z.floor() as i64 + offset[1];
        for dy in -8..=8 {
            let y = origin.y.floor() as i64 + dy;
            if chunks.block_at([x, y, z]).is_some_and(|b| {
                terra_voxel::MaterialId::from_raw(b)
                    .is_some_and(terra_voxel::MaterialId::is_natural_ground)
            }) {
                let target = Vec3::new(x as f32 + 0.5, y as f32 + 1.92, z as f32 + 0.5);
                if terra_physics::character::space_clear(chunks, target) {
                    return target;
                }
            }
        }
    }
    origin
}

fn cave_destination(world: &World) -> Option<Vec3> {
    let chunks = world.resource::<ChunkStore>();
    let origin = world
        .resource::<ClientSession>()
        .confirmed_state()?
        .position;
    let terrain = &world.resource::<WorldTerrain>().0;
    let journal = world.resource::<terra_world::edits::EditJournal>();
    let mut best = None;
    let mut best_score = f64::NEG_INFINITY;
    for (position, section) in &chunks.loaded {
        let Some(column) = chunks.prepared_column([position.x, position.z]) else {
            continue;
        };
        for z in (0..16).step_by(2) {
            for x in (0..16).step_by(2) {
                let wx = position.x * 16 + x as i64;
                let wz = position.z * 16 + z as i64;
                let distance = (wx as f64 - f64::from(origin[0])).powi(2)
                    + (wz as f64 - f64::from(origin[2])).powi(2);
                if distance > 96.0_f64.powi(2) {
                    continue;
                }
                let Some(surface) = column.surface_at(wx, wz) else {
                    continue;
                };
                for y in 0..16 {
                    let wy = position.y * 16 + y as i64;
                    if wy as f64 > surface.height - 10.0
                        || wy as f64 > f64::from(origin[1]) - 7.0
                        || !section.data.block([x, y, z]).is_ok_and(|block| {
                            terra_voxel::MaterialId::from_raw(block)
                                .is_some_and(terra_voxel::MaterialId::is_natural_ground)
                        })
                    {
                        continue;
                    }
                    let target = Vec3::new(wx as f32 + 0.5, wy as f32 + 1.92, wz as f32 + 0.5);
                    if !terra_physics::character::space_clear(chunks, target)
                        || chunks
                            .fluid_cell_at(terrain, journal, [wx, wy + 1, wz], 0)
                            .amount
                            > 0
                    {
                        continue;
                    }
                    let roof = (4..=24).any(|up| {
                        chunks.block_at([wx, wy + up, wz]).is_some_and(|block| {
                            terra_voxel::MaterialId::from_raw(block)
                                .is_some_and(terra_voxel::MaterialId::is_opaque)
                        })
                    });
                    if !roof {
                        continue;
                    }
                    let mut open = 0;
                    for dx in -3..=3 {
                        for dz in -3..=3 {
                            if chunks.block_at([wx + dx, wy + 2, wz + dz]) == Some(0) {
                                open += 1;
                            }
                        }
                    }
                    if open < 16 {
                        continue;
                    }
                    let score = f64::from(open) * 100.0 - distance;
                    if score > best_score {
                        best = Some(target);
                        best_score = score;
                    }
                }
            }
        }
    }
    best
}

fn start_companion(
    mut config: GameConfig,
    address: std::net::SocketAddr,
    destination: Vec3,
) -> Result<Companion, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let thread = std::thread::Builder::new()
        .name("terra-minimap-companion".into())
        .spawn(move || {
            let profile = PlayerProfile {
                name: "MapObserver".into(),
                ..Default::default()
            };
            let token = terra_network::browser::join(address, &profile)?;
            config.client.view_distance = 1;
            config.client.vertical_view_distance = 1;
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin))
                .add_plugins(terra_core::TerraCorePlugin(config))
                .add_plugins((
                    terra_world::TerraWorldPlugin {
                        mode: terra_world::WorldMode::Client,
                    },
                    terra_physics::TerraPhysicsPlugin,
                    terra_network::TerraNetworkPlugin(
                        terra_network::connection::NetworkRole::Client,
                    ),
                ))
                .insert_resource(profile)
                .insert_resource(terra_network::client::ClientLaunch { token })
                .add_systems(Update, companion_avatar);
            app.world_mut()
                .resource_mut::<terra_world::streaming::StreamingSettings>()
                .meshes = false;
            app.finish();
            app.cleanup();
            let mut moved = false;
            while !signal.load(Ordering::Acquire) {
                let started = Instant::now();
                app.update();
                let session = app.world().resource::<ClientSession>();
                if let Some(error) = &session.error {
                    return Err(error.clone());
                }
                if session.ready && session.confirmed_state().is_some() && !moved {
                    app.world_mut()
                        .resource_mut::<ClientChatCommands>()
                        .0
                        .push_back(terra_network::client::ChatCommand::Send(format!(
                            "/tp {:.2} {:.2} {:.2}",
                            destination.x, destination.y, destination.z
                        )));
                    moved = true;
                }
                if let Some(remaining) =
                    Duration::from_secs_f64(1.0 / 60.0).checked_sub(started.elapsed())
                {
                    std::thread::sleep(remaining);
                }
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    Ok(Companion {
        stop,
        thread: Some(thread),
    })
}

fn companion_avatar(
    mut commands: Commands,
    session: Res<ClientSession>,
    mut bodies: Query<&mut Transform, With<terra_physics::character::CharacterBody>>,
    mut focus: ResMut<terra_world::streaming::ChunkFocus>,
) {
    let Some(state) = session.confirmed_state().filter(|_| session.ready) else {
        return;
    };
    let position = Vec3::from_array(state.position);
    focus.0 = state.position.map(f64::from);
    if let Some(mut body) = bodies.iter_mut().next() {
        body.translation = position;
    } else {
        commands.spawn(terra_physics::character::body_bundle(position));
    }
}

fn click(world: &mut World, id: &str) -> Result<(), String> {
    emit(world, id, "minimap_action", None)
}

fn map_click(world: &mut World, id: &str) -> Result<(), String> {
    emit(world, id, "fullmap_action", None)
}

fn map_change(world: &mut World, id: &str, value: &str) -> Result<(), String> {
    emit(world, id, "fullmap_change", Some(value))
}

fn emit(world: &mut World, id: &str, handler: &str, value: Option<&str>) -> Result<(), String> {
    let entity = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find(|(_, element)| element.0 == id)
        .map(|(entity, _)| entity)
        .ok_or_else(|| format!("Map UI target missing: {id}"))?;
    world.write_message(terra_ui::HtmlEvent {
        target: entity,
        kind: if value.is_some() { "change" } else { "click" },
        value: value.map(str::to_owned),
        submitter: None,
        data: default(),
        form_data: None,
        handler: handler.into(),
    });
    Ok(())
}

fn subtree_has_text(world: &World, entity: Entity, title: &str, depth: usize) -> bool {
    depth <= 16
        && (world
            .get::<bevy::ui::widget::Text>(entity)
            .is_some_and(|text| text.0 == title)
            || world.get::<Children>(entity).is_some_and(|children| {
                children
                    .iter()
                    .any(|child| subtree_has_text(world, child, title, depth + 1))
            }))
}

fn point_pointer(world: &mut World, title: &str, pressed: bool) -> Result<(), String> {
    ui_pointer(world, "fullmap-point", Some(title), pressed)
}

fn ui_pointer(
    world: &mut World,
    target_id: &str,
    title: Option<&str>,
    pressed: bool,
) -> Result<(), String> {
    use bevy::{
        camera::RenderTarget,
        picking::pointer::{
            Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
        },
        window::WindowRef,
    };
    let window = primary_window(world)?;
    let location = if pressed {
        let entity = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .find_map(|(entity, id)| {
                (id.0 == target_id
                    && title.is_none_or(|title| subtree_has_text(world, entity, title, 0)))
                .then_some(entity)
            })
            .ok_or_else(|| format!("Map pointer target missing: {target_id} {title:?}"))?;
        let position = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Waypoint row has no layout")?
            .translation;
        let window_data = world.get::<Window>(window).ok_or("Missing map window")?;
        Location {
            target: RenderTarget::Window(WindowRef::Entity(window))
                .normalize(Some(window))
                .ok_or("Map window has no render target")?,
            position: position / window_data.resolution.scale_factor(),
        }
    } else {
        world
            .query::<(&PointerId, &PointerLocation)>()
            .iter(world)
            .find_map(|(id, location)| {
                (*id == PointerId::Mouse)
                    .then(|| location.location.clone())
                    .flatten()
            })
            .ok_or("Map mouse pointer location missing")?
    };
    if pressed {
        world.write_message(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        if pressed {
            PointerAction::Press(PointerButton::Primary)
        } else {
            PointerAction::Release(PointerButton::Primary)
        },
    ));
    Ok(())
}
