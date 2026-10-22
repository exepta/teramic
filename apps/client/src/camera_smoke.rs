//! Native production F5 and visual reconciliation checks against an integrated host.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_network::client::ClientSession;
use terra_player::{
    CameraInputState, CameraLook, CameraPerspective, CharacterPresentation, ControllerMode,
    PlayerBody, PlayerInputBlock, TerraCamera,
};
use terra_ui::menu::{MenuAction, MenuPage, MenuState};

#[derive(Resource)]
struct CameraSmoke {
    started: Instant,
    since: Instant,
    stage: u8,
    captures: u8,
    failed_capture: bool,
    samples: usize,
    max_visual_error: f32,
    camera_key: bool,
    pick_key: bool,
    picked: Option<u16>,
    gait_samples: usize,
    gait_max_offset: f32,
    gait_previous: Option<Vec3>,
    gait_distance: f32,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-camera") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(CameraSmoke {
        started: now,
        since: now,
        stage: 0,
        captures: 0,
        failed_capture: false,
        samples: 0,
        max_visual_error: 0.0,
        camera_key: false,
        pick_key: false,
        picked: None,
        gait_samples: 0,
        gait_max_offset: 0.0,
        gait_previous: None,
        gait_distance: 0.0,
    })
    .insert_resource(ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, drive.after(bevy::input::InputSystems))
    .add_systems(PostUpdate, inspect);
}

fn drive(world: &mut World) {
    let stage = world.resource::<CameraSmoke>().stage;
    let walking = matches!(stage, 4 | 9)
        && world.resource::<CameraSmoke>().since.elapsed() < Duration::from_secs(1);
    let camera_key = std::mem::take(&mut world.resource_mut::<CameraSmoke>().camera_key);
    let pick_key = std::mem::take(&mut world.resource_mut::<CameraSmoke>().pick_key);
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || terra_core::GameAction::Camera.default_binding(),
            |controls| controls.binding(terra_core::GameAction::Camera),
        );
    let apply = |keys: &mut ButtonInput<KeyCode>, key| {
        keys.release(key);
        if camera_key {
            keys.press(key);
        }
    };
    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
    match binding {
        terra_core::InputBinding::Key(key) => apply(&mut keys, key),
        terra_core::InputBinding::Chord { modifier, key } => {
            apply(&mut keys, modifier);
            apply(&mut keys, key);
        }
        terra_core::InputBinding::Mouse(_) => {}
    }
    if walking {
        keys.press(KeyCode::KeyD);
    } else {
        keys.release(KeyCode::KeyD);
    }
    if let terra_core::InputBinding::Mouse(button) = binding {
        let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
        mouse.release(button);
        if camera_key {
            mouse.press(button);
        }
    }
    let pick_binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || terra_core::GameAction::PickBlock.default_binding(),
            |controls| controls.binding(terra_core::GameAction::PickBlock),
        );
    match pick_binding {
        terra_core::InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            mouse.release(button);
            if pick_key {
                mouse.press(button);
            }
        }
        terra_core::InputBinding::Key(key) => {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            keys.release(key);
            if pick_key {
                keys.press(key);
            }
        }
        terra_core::InputBinding::Chord { modifier, key } => {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            keys.release(key);
            keys.release(modifier);
            if pick_key {
                keys.press(modifier);
                keys.press(key);
            }
        }
    }
    world.resource_mut::<CameraInputState>().captured = !world.resource::<PlayerInputBlock>().0;
}

fn inspect(world: &mut World) {
    if world.resource::<CameraSmoke>().stage == 6 {
        return;
    }
    let result = advance(world);
    if let Err(error) = result {
        error!(%error, "Native F5/reconciliation check failed");
        world.resource_mut::<CameraSmoke>().stage = 6;
        world.write_message(AppExit::error());
    }
}

fn advance(world: &mut World) -> Result<(), String> {
    if world.resource::<CameraSmoke>().started.elapsed() > Duration::from_secs(90)
        || world.resource::<CameraSmoke>().failed_capture
    {
        return Err("Timed out or screenshot was blank".into());
    }
    if let Some(error) = &world.resource::<ClientSession>().error {
        return Err(error.clone());
    }
    let stage = world.resource::<CameraSmoke>().stage;
    let elapsed = world.resource::<CameraSmoke>().since.elapsed();
    if !world.contains_resource::<MenuState>() {
        return Ok(());
    }
    if stage == 0 {
        if !world.contains_resource::<terra_server::IntegratedServer>() {
            return Err("Expected real integrated server".into());
        }
        if !world.resource::<ClientSession>().ready
            || !world.resource::<terra_render::RenderStatus>().ready
            || !world.resource::<terra_render::PropRenderStatus>().ready
            || !world
                .resource::<ClientSession>()
                .confirmed_state()
                .is_some_and(|state| state.grounded)
        {
            return Ok(());
        }
        if world.resource::<MenuState>().page != MenuPage::Playing {
            world.write_message(MenuAction::Command("resume".into()));
            return Ok(());
        }
        let aim =
            terrain_aim(world).ok_or("No unobstructed real terrain direction near the player")?;
        if let Some(mut settings) = world.get_resource_mut::<terra_core::GameplayConfig>() {
            settings.view_bobbing = true;
        }
        if let Some(mut camera) = world
            .query_filtered::<&mut Transform, With<TerraCamera>>()
            .iter_mut(world)
            .next()
        {
            camera.rotation = aim;
        }
        next(world, 7);
        return Ok(());
    }
    if world.resource::<CameraPerspective>().is_third_person() {
        let body = world
            .query_filtered::<(&Transform, &CharacterPresentation), With<PlayerBody>>()
            .iter(world)
            .next()
            .map(|(body, pose)| {
                pose.visual_position(
                    body.translation,
                    world.resource::<Time<Fixed>>().overstep_fraction(),
                )
            });
        let avatar = world
            .query::<(&Name, &Transform)>()
            .iter(world)
            .find(|(name, _)| name.as_str() == "Local player avatar")
            .map(|(_, t)| t.translation);
        if let (Some(body), Some(avatar)) = (body, avatar) {
            let error = body.distance(avatar);
            let mut state = world.resource_mut::<CameraSmoke>();
            state.samples += 1;
            state.max_visual_error = state.max_visual_error.max(error);
            if error > 0.001 {
                return Err(format!("Avatar used unpresented physics position: {error}"));
            }
        }
    }
    match stage {
        7 => {
            let target = world.resource::<terra_player::TargetWorld>().0;
            if let Some(terra_world::targeting::WorldTarget::Block(hit)) = target
                && let Some(material) = world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .block_at(hit.position)
            {
                let mut smoke = world.resource_mut::<CameraSmoke>();
                smoke.picked = Some(material);
                smoke.pick_key = true;
                next(world, 8);
            } else if elapsed > Duration::from_secs(5) {
                let pose = world
                    .query_filtered::<(&Transform, &CameraLook), With<TerraCamera>>()
                    .iter(world)
                    .next()
                    .map(|(t, l)| (t.translation, t.rotation, l.rotation()));
                return Err(format!(
                    "No real terrain block targeted for middle pick: {target:?}; pose={pose:?}; menu={:?}; blocked={}",
                    world.resource::<MenuState>().page,
                    world.resource::<PlayerInputBlock>().0
                ));
            }
        }
        8 => {
            let material = world
                .resource::<CameraSmoke>()
                .picked
                .ok_or("Missing pick material")?;
            let stack = world.resource::<ClientSession>().inventory.slot(0);
            if stack.is_some_and(|stack| {
                stack.item == terra_core::ItemKind::Block(material)
                    && stack.count == terra_core::STACK_LIMIT
            }) {
                info!(
                    material,
                    ?stack,
                    "Native middle pick received authoritative creative stack"
                );
                if let Some(mut camera) = world
                    .query_filtered::<&mut Transform, With<TerraCamera>>()
                    .iter_mut(world)
                    .next()
                {
                    camera.rotation = Quat::IDENTITY;
                }
                tap_camera(world);
                next(world, 1);
            } else if elapsed > Duration::from_secs(5) {
                return Err(format!(
                    "Middle pick did not receive authoritative stack: {stack:?}"
                ));
            }
        }
        1 if elapsed > Duration::from_millis(500) => {
            if *world.resource::<CameraPerspective>() != CameraPerspective::ThirdPerson {
                return Err("First F5 did not enter rear view".into());
            }
            capture(world, "camera-rear")?;
            tap_camera(world);
            next(world, 2);
        }
        2 if elapsed > Duration::from_millis(500) => {
            if *world.resource::<CameraPerspective>() != CameraPerspective::ThirdPersonFront {
                return Err("Second F5 did not enter front view".into());
            }
            let (camera, look) = world
                .query_filtered::<(&Transform, &CameraLook), With<TerraCamera>>()
                .iter(world)
                .next()
                .ok_or("Missing camera")?;
            if camera.forward().dot(look.rotation() * Vec3::NEG_Z) > -0.99 {
                return Err("Front camera did not look back at player".into());
            }
            capture(world, "camera-front")?;
            tap_camera(world);
            next(world, 3);
        }
        3 if elapsed > Duration::from_millis(500) => {
            if *world.resource::<CameraPerspective>() != CameraPerspective::FirstPerson {
                return Err("Third F5 did not restore first person".into());
            }
            if world
                .query::<&Name>()
                .iter(world)
                .any(|name| name.as_str() == "Local player avatar")
            {
                return Err("First-person avatar was not hidden".into());
            }
            capture(world, "camera-first")?;
            tap_camera(world);
            next(world, 4);
        }
        4 if elapsed > Duration::from_millis(1500) => {
            let state = world.resource::<CameraSmoke>();
            if state.samples < 10 {
                return Err("Not enough presented movement samples".into());
            }
            info!(
                samples = state.samples,
                max_visual_error = state.max_visual_error,
                pending_inputs = world.resource::<ClientSession>().pending_inputs,
                "Native avatar and camera use identical presented physics poses"
            );
            capture(world, "camera-walking")?;
            next(world, 5);
        }
        5 if world.resource::<CameraSmoke>().captures >= 4 => {
            tap_camera(world);
            next(world, 11);
        }
        11 if elapsed > Duration::from_millis(150) => {
            if *world.resource::<CameraPerspective>() != CameraPerspective::ThirdPersonFront {
                return Err("Gait check did not pass through the front camera".into());
            }
            tap_camera(world);
            next(world, 9);
        }
        9 => {
            if elapsed < Duration::from_millis(150) {
                return Ok(());
            }
            if *world.resource::<CameraPerspective>() != CameraPerspective::FirstPerson {
                return Err("Gait check did not return to first person".into());
            }
            let visual = world
                .query_filtered::<(&Transform, &CharacterPresentation), With<PlayerBody>>()
                .iter(world)
                .next()
                .map(|(body, pose)| {
                    pose.visual_position(
                        body.translation,
                        world.resource::<Time<Fixed>>().overstep_fraction(),
                    )
                })
                .ok_or("Missing presented body during gait check")?;
            let (camera, look) = world
                .query_filtered::<(&Transform, &CameraLook), With<TerraCamera>>()
                .iter(world)
                .next()
                .ok_or("Missing first-person camera during gait check")?;
            let offset = camera
                .translation
                .distance(visual + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET);
            if offset > 0.065 || !camera.rotation.abs_diff_eq(look.rotation(), 0.00001) {
                return Err(format!(
                    "Gait displaced the aim or exceeded its budget: {offset}"
                ));
            }
            if elapsed < Duration::from_secs(1) {
                let mut state = world.resource_mut::<CameraSmoke>();
                state.gait_samples += 1;
                state.gait_max_offset = state.gait_max_offset.max(offset);
                if let Some(previous) = state.gait_previous.replace(visual) {
                    state.gait_distance += (visual - previous).xz().length();
                }
            }
            if elapsed > Duration::from_millis(1500) {
                let state = world.resource::<CameraSmoke>();
                if state.gait_samples < 10
                    || state.gait_distance < 0.25
                    || state.gait_max_offset < 0.015
                {
                    return Err(format!(
                        "Insufficient real first-person gait: samples={} distance={} offset={}",
                        state.gait_samples, state.gait_distance, state.gait_max_offset
                    ));
                }
                info!(
                    samples = state.gait_samples,
                    distance = state.gait_distance,
                    max_camera_offset = state.gait_max_offset,
                    "Native distance-driven first-person gait and unchanged aim passed"
                );
                capture(world, "camera-gait")?;
                next(world, 10);
            }
        }
        10 if world.resource::<CameraSmoke>().captures >= 5 => {
            info!("Native F5 rear/front/first, gait and authoritative presentation check passed");
            next(world, 6);
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

fn tap_camera(world: &mut World) {
    world.resource_mut::<CameraSmoke>().camera_key = true;
}

fn terrain_aim(world: &mut World) -> Option<Quat> {
    let origin = world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()?
        .translation
        .to_array()
        .map(f64::from);
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    for pitch in [-0.65, -0.85, -1.05, -1.3] {
        for sample in 0..64 {
            let yaw = sample as f32 * std::f32::consts::TAU / 64.0 - std::f32::consts::PI;
            let rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            let (_, selected) = terra_world::targeting::pick_world(
                chunks,
                origin,
                (rotation * Vec3::NEG_Z).to_array().map(f64::from),
            );
            if matches!(
                selected,
                Some(terra_world::targeting::WorldTarget::Block(_))
            ) {
                return Some(rotation);
            }
        }
    }
    None
}

fn next(world: &mut World, stage: u8) {
    let mut state = world.resource_mut::<CameraSmoke>();
    state.stage = stage;
    state.since = Instant::now();
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-camera-v2"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<CameraSmoke>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(101)
                    .take(4096)
                    .map(|p| [p[0], p[1], p[2]])
                    .collect();
                state.failed_capture |= colors.len() < 16;
                state.captures += 1;
            },
        );
    Ok(())
}
