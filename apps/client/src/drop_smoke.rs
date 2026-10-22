//! Native 3D-drop acceptance through production keyboard handling and real UDP authority.
use bevy::{
    app::AppExit,
    asset::AssetId,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    mesh::VertexAttributeValues,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::PrimaryWindow,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameAction, InputBinding, ItemKind, WorldDrop};
use terra_network::{
    client::{ClientSession, InventoryRequest, SelectedInventorySlot},
    protocol::inventory::{InventoryAction, InventoryCommand, InventoryOutcome},
};
use terra_player::{CameraInputState, ControllerMode, PlayerInputBlock, TerraCamera};
use terra_ui::menu::{MenuAction, MenuPage, MenuState};

#[derive(Component)]
struct DropStudio;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Loading,
    Grant,
    Select,
    FirstPress,
    FirstFlight,
    FirstRest,
    FirstCapture,
    SecondPress,
    SecondFlight,
    SecondRest,
    Finish,
    Done,
}

#[derive(Resource)]
struct DropSmoke {
    started: Instant,
    entered: Instant,
    stage: Stage,
    captured: u8,
    failed_capture: bool,
    first: Option<u64>,
    second: Option<u64>,
    flight_rotation: Option<Quat>,
    rotation_changed: bool,
    stable: Option<(Instant, [f64; 3], Quat)>,
    move_until: Option<Instant>,
    checked_meshes: HashMap<AssetId<Mesh>, (Vec3, Vec3, usize)>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-drops") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(DropSmoke {
        started: now,
        entered: now,
        stage: Stage::Loading,
        captured: 0,
        failed_capture: false,
        first: None,
        second: None,
        flight_rotation: None,
        rotation_changed: false,
        stable: None,
        move_until: None,
        checked_meshes: HashMap::new(),
    })
    .insert_resource(ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, drive.after(bevy::input::InputSystems))
    .add_systems(PostUpdate, inspect);
}

fn drive(world: &mut World) {
    let backward = world
        .resource::<DropSmoke>()
        .move_until
        .is_some_and(|until| Instant::now() < until);
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || GameAction::Backward.default_binding(),
            |controls| controls.binding(GameAction::Backward),
        );
    if let InputBinding::Key(key) = binding
        && let Some(mut keys) = world.get_resource_mut::<ButtonInput<KeyCode>>()
    {
        if backward {
            keys.press(key);
        } else {
            keys.release(key);
        }
    }
    if world
        .get_resource::<MenuState>()
        .is_some_and(|menu| menu.page == MenuPage::Playing)
        && world
            .get_resource::<PlayerInputBlock>()
            .is_some_and(|blocked| !blocked.0)
        && let Some(mut camera) = world.get_resource_mut::<CameraInputState>()
    {
        camera.captured = true;
    }
}

fn inspect(world: &mut World) {
    if world.resource::<DropSmoke>().stage == Stage::Done {
        return;
    }
    if let Err(error) = record_models(world).and_then(|()| advance(world)) {
        let state = world.resource::<DropSmoke>();
        error!(%error, stage=?state.stage, elapsed=?state.started.elapsed(), inventory=?world.get_resource::<ClientSession>().map(|session| &session.inventory), drops=?world.get_resource::<ClientSession>().map(|session| &session.drops), "Native Q/Shift+Q and 3D-drop check failed");
        world.resource_mut::<DropSmoke>().stage = Stage::Done;
        world.write_message(AppExit::error());
    }
}

fn advance(world: &mut World) -> Result<(), String> {
    let state = world.resource::<DropSmoke>();
    if state.started.elapsed() > Duration::from_secs(90) || state.failed_capture {
        return Err("Timed out or screenshot was blank".into());
    }
    if !world.contains_resource::<ClientSession>()
        || !world.contains_resource::<MenuState>()
        || !world.contains_resource::<terra_core::ControlsConfig>()
    {
        return Ok(());
    }
    if let Some(error) = &world.resource::<ClientSession>().error {
        return Err(error.clone());
    }
    let stage = state.stage;
    let elapsed = state.entered.elapsed();
    if stage == Stage::Loading {
        if !world.contains_resource::<terra_server::IntegratedServer>() {
            return Err("Drop check requires a real integrated server".into());
        }
        if !world.resource::<ClientSession>().ready
            || !world
                .get_resource::<terra_render::RenderStatus>()
                .is_some_and(|status| status.ready)
            || !world
                .get_resource::<terra_render::PropRenderStatus>()
                .is_some_and(|status| status.ready)
            || !world
                .resource::<ClientSession>()
                .confirmed_state()
                .is_some_and(|state| state.grounded)
            || world.resource::<DropSmoke>().started.elapsed() < Duration::from_secs(12)
        {
            return Ok(());
        }
        if world.resource::<MenuState>().page != MenuPage::Playing {
            world.write_message(MenuAction::Command("resume".into()));
            return Ok(());
        }
        if world.resource::<ClientSession>().game_mode != terra_core::GameMode::Creative {
            return Err("Preparation requires a Creative session".into());
        }
        // Only fixture input bindings change; no settings file or gameplay state is fabricated.
        let mut controls = world.resource_mut::<terra_core::ControlsConfig>();
        controls
            .bindings
            .insert(GameAction::Drop, InputBinding::Key(KeyCode::KeyQ));
        controls.bindings.insert(
            GameAction::DropStack,
            InputBinding::Chord {
                modifier: KeyCode::ShiftLeft,
                key: KeyCode::KeyQ,
            },
        );
        let revision = world.resource::<ClientSession>().inventory.revision;
        world.write_message(InventoryRequest(InventoryCommand {
            revision,
            action: InventoryAction::CreativeSet {
                slot: 0,
                item: ItemKind::Block(3),
                count: 4,
            },
        }));
        next(world, Stage::Grant);
        return Ok(());
    }
    match stage {
        Stage::Grant
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_some_and(|s| s.item == ItemKind::Block(3) && s.count == 4) =>
        {
            key(world, KeyCode::Digit1, true)?;
            next(world, Stage::Select);
        }
        Stage::Select if elapsed > Duration::from_millis(100) => {
            key(world, KeyCode::Digit1, false)?;
            if world.resource::<SelectedInventorySlot>().0 != Some(0) {
                return Err("Production hotbar key did not select slot one".into());
            }
            key(world, KeyCode::KeyQ, true)?;
            next(world, Stage::FirstPress);
        }
        Stage::FirstPress if elapsed > Duration::from_millis(70) => {
            key(world, KeyCode::KeyQ, false)?;
            world.resource_mut::<DropSmoke>().move_until =
                Some(Instant::now() + Duration::from_millis(1100));
            next(world, Stage::FirstFlight);
        }
        Stage::FirstFlight => {
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_none_or(|s| s.count != 3)
            {
                return Ok(());
            }
            let Some(drop) = world
                .resource::<ClientSession>()
                .drops
                .iter()
                .find(|drop| drop.stack.item == ItemKind::Block(3) && drop.stack.count == 1)
                .cloned()
            else {
                return Ok(());
            };
            let first = world.resource::<DropSmoke>().first;
            if first.is_none() {
                if drop.pickup_delay_seconds <= 0.0 {
                    return Err("Thrown item has no authoritative pickup delay".into());
                }
                let mut state = world.resource_mut::<DropSmoke>();
                state.first = Some(drop.id);
                state.flight_rotation = Some(Quat::from_array(drop.rotation));
                info!(
                    id = drop.id,
                    count = drop.stack.count,
                    delay = drop.pickup_delay_seconds,
                    "Production Q removed one item and spawned a real server drop"
                );
            }
            if !has_model(world, &drop) {
                return Ok(());
            }
            require_model(world, &drop)?;
            studio(world, &drop);
            let changed = world
                .resource::<DropSmoke>()
                .flight_rotation
                .is_some_and(|rotation| {
                    rotation.angle_between(Quat::from_array(drop.rotation)) > 0.04
                });
            if changed && !drop.grounded {
                world.resource_mut::<DropSmoke>().rotation_changed = true;
                capture(world, "air")?;
                next(world, Stage::FirstRest);
            }
        }
        Stage::FirstRest => {
            let drop = find_drop(world, world.resource::<DropSmoke>().first)?;
            studio(world, &drop);
            if stable_rest(world, &drop)? {
                require_model(world, &drop)?;
                capture(world, "rest")?;
                next(world, Stage::FirstCapture);
            }
        }
        Stage::FirstCapture if world.resource::<DropSmoke>().captured >= 2 => {
            key(world, KeyCode::ShiftLeft, true)?;
            key(world, KeyCode::KeyQ, true)?;
            next(world, Stage::SecondPress);
        }
        Stage::SecondPress if elapsed > Duration::from_millis(70) => {
            key(world, KeyCode::KeyQ, false)?;
            key(world, KeyCode::ShiftLeft, false)?;
            world.resource_mut::<DropSmoke>().move_until =
                Some(Instant::now() + Duration::from_millis(1100));
            next(world, Stage::SecondFlight);
        }
        Stage::SecondFlight => {
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_some()
            {
                return Ok(());
            }
            let first = world.resource::<DropSmoke>().first;
            let Some(drop) = world
                .resource::<ClientSession>()
                .drops
                .iter()
                .find(|drop| {
                    Some(drop.id) != first
                        && drop.stack.item == ItemKind::Block(3)
                        && drop.stack.count == 3
                })
                .cloned()
            else {
                return Ok(());
            };
            if drop.pickup_delay_seconds <= 0.0 {
                return Err("Shift+Q stack has no pickup delay".into());
            }
            if !has_model(world, &drop) {
                return Ok(());
            }
            require_model(world, &drop)?;
            let total: u16 = world
                .resource::<ClientSession>()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == ItemKind::Block(3))
                .map(|drop| drop.stack.count)
                .sum();
            if total != 4 {
                return Err(format!(
                    "Q/Shift+Q did not conserve four resources: {total}"
                ));
            }
            world.resource_mut::<DropSmoke>().second = Some(drop.id);
            info!(
                id = drop.id,
                count = drop.stack.count,
                "Production Shift+Q dropped the complete remaining stack"
            );
            next(world, Stage::SecondRest);
        }
        Stage::SecondRest => {
            let drop = find_drop(world, world.resource::<DropSmoke>().second)?;
            studio(world, &drop);
            if stable_rest(world, &drop)? {
                require_model(world, &drop)?;
                capture(world, "stack-rest")?;
                next(world, Stage::Finish);
            }
        }
        Stage::Finish if world.resource::<DropSmoke>().captured >= 3 => {
            let session = world.resource::<ClientSession>();
            if session.last_inventory != Some(InventoryOutcome::Applied)
                || session.inventory.slot(0).is_some()
                || !world.resource::<DropSmoke>().rotation_changed
            {
                return Err("Final authoritative inventory or tumble check failed".into());
            }
            info!(
                captures = world.resource::<DropSmoke>().captured,
                remaining = session.inventory.count(ItemKind::Block(3)),
                "Native production Q/Shift+Q, real 3D meshes, tumbling and flat resting drops passed"
            );
            next(world, Stage::Done);
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

fn find_drop(world: &World, id: Option<u64>) -> Result<WorldDrop, String> {
    world
        .resource::<ClientSession>()
        .drops
        .iter()
        .find(|drop| Some(drop.id) == id)
        .cloned()
        .ok_or_else(|| {
            "Real drop disappeared before the rest check (unexpected pickup or replication loss)"
                .into()
        })
}

fn stable_rest(world: &mut World, drop: &WorldDrop) -> Result<bool, String> {
    let rotation = Quat::from_array(drop.rotation);
    let resting = drop.grounded
        && drop.velocity.iter().all(|v| v.abs() < 0.02)
        && drop.angular_velocity.iter().all(|v| v.abs() < 0.02);
    if !resting {
        world.resource_mut::<DropSmoke>().stable = None;
        return Ok(false);
    }
    let axes = Mat3::from_quat(rotation);
    if axes
        .x_axis
        .y
        .abs()
        .max(axes.y_axis.y.abs())
        .max(axes.z_axis.y.abs())
        < 0.995
    {
        return Ok(false);
    }
    let x = drop.position[0].floor() as i64;
    let z = drop.position[2].floor() as i64;
    let y = drop.position[1].floor() as i64;
    let terrain = world.resource::<terra_world::streaming::ChunkStore>();
    let floor = (y - 2..=y)
        .rev()
        .find(|&y| {
            terrain
                .block_at([x, y, z])
                .is_some_and(|material| material != 0)
        })
        .ok_or("Resting drop has no solid terrain below it")?;
    let clearance = drop.position[1] - (floor + 1) as f64;
    if !(0.105..=0.17).contains(&clearance) {
        return Err(format!(
            "Resting model is floating or clipping terrain: clearance={clearance}"
        ));
    }
    let stable = world.resource::<DropSmoke>().stable;
    if let Some((since, position, previous)) = stable {
        let moved = (0..3)
            .map(|i| (position[i] - drop.position[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if moved < 0.003 && previous.angle_between(rotation) < 0.003 {
            if since.elapsed() >= Duration::from_millis(800) {
                info!(
                    id = drop.id,
                    clearance,
                    "Replicated drop rests flat without cosmetic hovering or perpetual rotation"
                );
                return Ok(true);
            }
            return Ok(false);
        }
    }
    world.resource_mut::<DropSmoke>().stable = Some((Instant::now(), drop.position, rotation));
    Ok(false)
}

fn has_model(world: &mut World, drop: &WorldDrop) -> bool {
    let target = Vec3::from_array(drop.position.map(|v| v as f32));
    world
        .query::<(&Name, &Transform)>()
        .iter(world)
        .any(|(name, transform)| {
            name.as_str() == "Dropped resource" && transform.translation.distance(target) < 0.8
        })
}

fn require_model(world: &mut World, drop: &WorldDrop) -> Result<(), String> {
    let target = Vec3::from_array(drop.position.map(|v| v as f32));
    let entity = world
        .query::<(Entity, &Name, &Transform)>()
        .iter(world)
        .filter(|(_, name, _)| name.as_str() == "Dropped resource")
        .min_by(|(_, _, a), (_, _, b)| {
            a.translation
                .distance_squared(target)
                .total_cmp(&b.translation.distance_squared(target))
        })
        .map(|(entity, _, _)| entity)
        .ok_or("Server drop has no rendered resource root")?;
    let children: Vec<_> = world
        .get::<Children>(entity)
        .ok_or("Resource drop has no mesh children")?
        .iter()
        .collect();
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    let mut vertices = 0;
    for child in children {
        let handle = world
            .get::<Mesh3d>(child)
            .ok_or("Resource child is not a real 3D mesh")?;
        let &(part_low, part_high, count) = world
            .resource::<DropSmoke>()
            .checked_meshes
            .get(&handle.0.id())
            .ok_or("Resource geometry was never validated before GPU extraction")?;
        low = low.min(part_low);
        high = high.max(part_high);
        vertices += count;
    }
    let size = high - low;
    if vertices < 24 || size.min_element() < 0.20 || size.max_element() > 0.27 {
        return Err(format!(
            "Resource mesh is flat or incorrectly sized: {vertices} vertices, size={size:?}"
        ));
    }
    if drop.grounded && drop.angular_velocity.iter().all(|v| v.abs() < 0.02) {
        let transform = world
            .get::<Transform>(entity)
            .ok_or("Drop transform is missing")?;
        if transform.translation.distance(target) > 0.005
            || transform
                .rotation
                .angle_between(Quat::from_array(drop.rotation))
                > 0.005
        {
            return Err("Resting renderer does not match the authoritative 3D pose".into());
        }
    }
    Ok(())
}

fn record_models(world: &mut World) -> Result<(), String> {
    if !world.contains_resource::<Assets<Mesh>>() {
        return Ok(());
    }
    let roots: Vec<_> = world
        .query::<(Entity, &Name)>()
        .iter(world)
        .filter(|(_, name)| name.as_str() == "Dropped resource")
        .map(|(entity, _)| entity)
        .collect();
    let mut handles = Vec::new();
    for root in roots {
        if let Some(children) = world.get::<Children>(root) {
            for child in children.iter() {
                if let Some(mesh) = world.get::<Mesh3d>(child) {
                    handles.push(mesh.0.clone());
                }
            }
        }
    }
    for handle in handles {
        if world
            .resource::<DropSmoke>()
            .checked_meshes
            .contains_key(&handle.id())
        {
            continue;
        }
        let Some(geometry) = world.resource::<Assets<Mesh>>().get(&handle) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(points)) =
            geometry.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return Err("Drop mesh has no 3D vertices".into());
        };
        if points.is_empty()
            || points.iter().flatten().any(|v| !v.is_finite())
            || geometry.primitive_topology() != bevy::mesh::PrimitiveTopology::TriangleList
            || geometry.indices().is_none_or(|indices| {
                indices.iter().next().is_none() || indices.iter().any(|index| index >= points.len())
            })
        {
            return Err("Drop mesh has invalid geometry".into());
        }
        let low = points
            .iter()
            .copied()
            .map(Vec3::from_array)
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let high = points
            .iter()
            .copied()
            .map(Vec3::from_array)
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let count = points.len();
        // CPU vertices intentionally leave Assets<Mesh> after extraction. Keep only
        // checked handle IDs and tiny bounds, then validate the same live handles at rest.
        world
            .resource_mut::<DropSmoke>()
            .checked_meshes
            .insert(handle.id(), (low, high, count));
    }
    Ok(())
}

fn studio(world: &mut World, drop: &WorldDrop) {
    let target = Vec3::from_array(drop.position.map(|v| v as f32));
    let pose =
        Transform::from_translation(target + Vec3::new(1.5, 1.15, 2.2)).looking_at(target, Vec3::Y);
    if let Some(entity) = world
        .query_filtered::<Entity, With<DropStudio>>()
        .iter(world)
        .next()
    {
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            *transform = pose;
        }
        return;
    }
    for mut camera in world
        .query_filtered::<&mut Camera, With<TerraCamera>>()
        .iter_mut(world)
    {
        camera.order = -1;
    }
    world.spawn((
        DropStudio,
        Camera3d::default(),
        Camera::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 1.35,
            },
            near: 0.01,
            far: 100.0,
            ..OrthographicProjection::default_3d()
        }),
        pose,
        AmbientLight {
            brightness: 700.0,
            ..default()
        },
        Exposure { ev100: 13.7 },
        Hdr,
        Tonemapping::AcesFitted,
        Msaa::Off,
    ));
}

fn next(world: &mut World, stage: Stage) {
    let mut state = world.resource_mut::<DropSmoke>();
    state.stage = stage;
    state.entered = Instant::now();
    state.stable = None;
}

fn key(world: &mut World, key_code: KeyCode, pressed: bool) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No native primary window")?;
    world.write_message(KeyboardInput {
        key_code,
        logical_key: Key::Character(
            match key_code {
                KeyCode::Digit1 => "1",
                KeyCode::KeyQ => "q",
                _ => "",
            }
            .into(),
        ),
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat: false,
        window,
    });
    Ok(())
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let base = std::env::var_os("TERRA_SCREENSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-drops.png"));
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("drops");
    let path = base.with_file_name(format!("{stem}-{name}.png"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<DropSmoke>| {
                let mut colors = std::collections::HashSet::new();
                if let Some(data) = &event.image.data {
                    for pixel in data.chunks_exact(4) {
                        colors.insert([pixel[0], pixel[1], pixel[2]]);
                        if colors.len() >= 32 {
                            break;
                        }
                    }
                }
                state.failed_capture |= colors.len() < 32;
                state.captured += 1;
            },
        );
    Ok(())
}
