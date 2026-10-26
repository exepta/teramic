//! Short native crank placement, real held input, TMI and processing regression.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameAction, GameConfig, GameMode, InputBinding, ItemKind, PlayerProfile};
use terra_network::{
    client::{ClientSession, ContainerRequest},
    protocol::inventory::{ContainerCommand, InventoryOutcome},
};

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::temp_dir().join(format!("terra-crusher-{}", std::process::id()));
    if path.exists() {
        return Err("Crusher fixture requires a fresh directory".into());
    }
    let profile_path = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|e| e.to_string())?;
    let profile = PlayerProfile::load_for_client(profile_path).map_err(|e| e.to_string())?;
    config.server.player_permissions.insert(
        profile.uuid,
        terra_core::PlayerPermissions {
            game_mode: false,
            build: true,
            flight: false,
            environment: true,
        },
    );
    let mut journal = terra_world::edits::EditJournal::default();
    let inventory = journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default();
    inventory.insert(ItemKind::Block(terra_core::crusher::MATERIAL), 1);
    inventory.insert(ItemKind::Block(terra_core::crusher::CRANK_MATERIAL), 1);
    for recipe in terra_core::crusher::recipes() {
        inventory.insert(recipe.input, 1);
    }
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    recipe: usize,
    anchor: Option<[i64; 3]>,
    crank: Option<[i64; 3]>,
    standing: Option<Vec3>,
    paused_progress: f32,
    drive_seen: bool,
    recipe_drive_seen: bool,
    audio_seen: bool,
    foley_seen: bool,
    original_audio: Option<terra_core::AudioConfig>,
    captures: usize,
    requested: usize,
    bad_capture: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-crusher") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        recipe: 0,
        anchor: None,
        crank: None,
        standing: None,
        paused_progress: 0.0,
        drive_seen: false,
        recipe_drive_seen: false,
        audio_seen: false,
        foley_seen: false,
        original_audio: None,
        captures: 0,
        requested: 0,
        bad_capture: false,
    })
    .insert_resource(terra_player::ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, fixture_input.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    let original_audio = world.resource::<terra_core::AudioConfig>().clone();
    world.resource_mut::<Inspection>().original_audio = Some(original_audio);
    let mut audio = world.resource_mut::<terra_core::AudioConfig>();
    audio.muted = false;
    audio.master_volume = 1.0;
    audio.ambience_volume = 1.0;
    world.write_message(terra_player::CaptureRequest(true));
}
fn advance(inspection: &mut Inspection, stage: u8) {
    inspection.stage = stage;
    inspection.since = Instant::now();
    info!(stage, recipe = inspection.recipe, "Crusher native stage");
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for crusher captures")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut inspection: ResMut<Inspection>| {
                inspection.bad_capture |= event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .filter(|pixel| pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)
                        .count()
                        < 1000
                });
                inspection.captures += 1;
            },
        );
    Ok(())
}
fn captured(
    world: &mut World,
    inspection: &mut Inspection,
    name: &str,
    count: usize,
) -> Result<bool, String> {
    if inspection.requested < count {
        capture(world, name)?;
        inspection.requested = count;
    }
    Ok(inspection.captures >= count)
}
fn aim(world: &mut World, anchor: [i64; 3]) {
    aim_at(
        world,
        Vec3::from_array(anchor.map(|v| v as f32)) + Vec3::new(0.75, 0.65, 0.5),
    );
}
fn aim_at(world: &mut World, point: Vec3) {
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(point, Vec3::Y);
    }
}

fn machine_point(anchor: [i64; 3], facing: u8, point: Vec3) -> Vec3 {
    let center = Vec3::new(0.5, 0.0, 0.5);
    Vec3::from_array(anchor.map(|v| v as f32))
        + center
        + Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2) * (point - center)
}

fn use_input(world: &mut World, held: bool) {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || GameAction::Place.default_binding(),
            |controls| controls.binding(GameAction::Place),
        );
    match binding {
        InputBinding::Key(code) => {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            if held {
                keys.press(code);
            } else {
                keys.release(code);
            }
        }
        InputBinding::Chord { modifier, key } => {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            for code in [modifier, key] {
                if held {
                    keys.press(code);
                } else {
                    keys.release(code);
                }
            }
        }
        InputBinding::Mouse(button) => {
            let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
            if held {
                buttons.press(button);
            } else {
                buttons.release(button);
            }
        }
    }
}

fn fixture_input(world: &mut World) {
    let (stage, anchor) = {
        let inspection = world.resource::<Inspection>();
        (inspection.stage, inspection.anchor)
    };
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    let gameplay = !world.resource::<terra_ui::InventoryView>().open
        && !world.resource::<terra_player::PlayerInputBlock>().0;
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = gameplay;
    use_input(world, gameplay && matches!(stage, 17 | 20 | 22..=24));
    if stage >= 12
        && stage != 255
        && let Some(anchor) = anchor
        && let Some(object) = world
            .resource::<terra_world::edits::EditJournal>()
            .object_state(anchor)
    {
        let point = if stage == 25 {
            Vec3::new(1.0, 0.65, 0.5)
        } else if matches!(stage, 12 | 13) {
            Vec3::new(1.997, 0.455, 0.515)
        } else {
            Vec3::new(2.60, 0.64, 0.515)
        };
        aim_at(world, machine_point(anchor, object.facing, point));
    }
}

fn visual(
    world: &World,
    inspection: &Inspection,
) -> Option<terra_network::protocol::session::CrusherVisual> {
    world
        .resource::<ClientSession>()
        .crushers
        .iter()
        .find(|visual| Some(visual.position) == inspection.anchor)
        .cloned()
}

fn verify_drive(
    world: &mut World,
    inspection: &mut Inspection,
    grinding: bool,
) -> Result<(), String> {
    if world.resource::<terra_ui::InventoryView>().open {
        return Err("Crank fixture held Place while its inventory was open".into());
    }
    if world.resource::<terra_network::client::CrankInput>().0 != inspection.crank {
        return Err("Real Place input did not resolve the mounted crank".into());
    }
    let target = world.resource::<terra_player::TargetWorld>().0;
    if !matches!(target, Some(terra_player::WorldTarget::Block(hit)) if Some(hit.position) == inspection.crank)
    {
        return Err("Crank interaction did not use a real material-100 world ray".into());
    }
    let current = visual(world, inspection).ok_or("Missing authoritative crusher visual")?;
    if !current.manual_active || current.working != grinding {
        return Err(format!("Wrong acknowledged crank drive state: {current:?}"));
    }
    inspection.drive_seen = true;
    if let Some(audio) = world.get_resource::<crate::crusher_audio::CrusherAudioStatus>() {
        if audio.nearby != 1 || audio.turning != 1 || audio.grinding != usize::from(grinding) {
            return Err(format!(
                "Mechanical audio did not follow the acknowledged crank: {audio:?}"
            ));
        }
        let config = world.resource::<terra_core::AudioConfig>();
        let audible = !config.muted && config.master_volume > 0.0 && config.ambience_volume > 0.0;
        if audio.foley_voices > 12 || (audible && !audio.foley_ready) {
            return Err(format!(
                "Recorded crusher Foley is missing or over budget: {audio:?}"
            ));
        }
        inspection.foley_seen |= audible && grinding && audio.foley_voices > 0;
        if audible && (!audio.playback_started || audio.output_rms <= 0.000001) {
            return Err(format!("Mechanical sound source is not playing: {audio:?}"));
        }
        if !audible && audio.output_rms > 0.000001 {
            return Err("Mechanical sound ignored the user's audio mute/volume settings".into());
        }
        inspection.audio_seen = true;
    }
    Ok(())
}
fn run(world: &mut World) {
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    world.resource_scope(|world,mut inspection:Mut<Inspection>| {
        if inspection.stage == 255 { return; }
        if let Err(error) = step(world,&mut inspection) {
            error!(%error,stage=inspection.stage,recipe=inspection.recipe,"Native crusher fixture failed");
            inspection.stage=255;
            use_input(world, false);
            if let Some(audio) = inspection.original_audio.take() { world.insert_resource(audio); }
            world.write_message(bevy::app::AppExit::error());
        }
    });
}
fn validate_layout(world: &mut World) -> Result<(), String> {
    let mut query = world.query::<(&tilt_ui::ElementId, &ComputedNode)>();
    for id in [
        "crusher-input",
        "crusher-output",
        "crusher-workspace",
        "inventory-panel",
    ] {
        let node = query
            .iter(world)
            .find(|(element, _)| element.0 == id)
            .ok_or_else(|| format!("Missing native crusher node {id}"))?
            .1;
        if node.size().min_element() < 1.0 {
            return Err(format!("Unmeasured crusher node {id}"));
        }
    }
    Ok(())
}
fn step(world: &mut World, inspection: &mut Inspection) -> Result<(), String> {
    if inspection.start.elapsed() > Duration::from_secs(240) || inspection.bad_capture {
        return Err(format!(
            "Crusher fixture timeout or blank capture: stage={}, recipe={}, crank={:?}, target={:?}, last_edit={:?}",
            inspection.stage,
            inspection.recipe,
            world.resource::<terra_network::client::CrankInput>().0,
            world.resource::<terra_player::TargetWorld>().0,
            world.resource::<ClientSession>().last_edit,
        ));
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready {
        return Ok(());
    }
    let elapsed = inspection.since.elapsed().as_secs_f32();
    match inspection.stage {
        0 if world.resource::<terra_render::RenderStatus>().ready
            && world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                == 0
            && session.confirmed_state().is_some_and(|s| s.grounded) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            world.write_message(terra_player::CaptureRequest(true));
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -0.65, 0.0);
            }
            advance(inspection, 1);
        }
        1 if elapsed > 0.8 => {
            let target = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("No crusher placement target")?;
            let (position, expected_material) = terra_world::targeting::block_placement(
                world.resource::<terra_world::streaming::ChunkStore>(),
                target,
            )
            .ok_or("No crusher placement cell")?;
            inspection.anchor = Some(position);
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material,
                material: terra_core::crusher::MATERIAL,
            });
            advance(inspection, 2);
        }
        2 if session
            .inventory
            .count(ItemKind::Block(terra_core::crusher::MATERIAL))
            == 0 =>
        {
            let anchor = inspection.anchor.ok_or("Missing crusher anchor")?;
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(anchor)
                != Some(terra_core::crusher::MATERIAL)
            {
                return Ok(());
            }
            aim(world, anchor);
            advance(inspection, 3);
        }
        3 if elapsed > 0.8 => {
            let render = world.resource::<terra_render::CrusherRenderStatus>();
            if render.machines == 0 && elapsed < 5.0 {
                return Ok(());
            }
            if render.machines != 1 || render.cached_meshes != 9 {
                return Err(format!("Crusher moving geometry missing: {render:?}"));
            }
            let anchor = inspection.anchor.unwrap();
            let journal = world.resource::<terra_world::edits::EditJournal>();
            let state = journal
                .object_state(anchor)
                .ok_or("Missing placed crusher orientation")?;
            inspection.crank = journal.crusher_crank_position(anchor);
            let standing = machine_point(
                anchor,
                state.facing,
                Vec3::new(4.2, terra_physics::character::PLAYER_HALF_HEIGHT, 0.515),
            );
            inspection.standing = Some(standing);
            world.write_message(terra_ui::InventoryAction::Hotbar(1));
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(format!(
                    "/tp {:.3} {:.3} {:.3}",
                    standing.x, standing.y, standing.z
                )));
            advance(inspection, 12);
        }
        12 if elapsed > 0.5
            && !session.teleporting
            && session.confirmed_state().is_some_and(|state| {
                state.grounded
                    && inspection.standing.is_some_and(|point| {
                        Vec3::from_array(state.position).distance(point) < 0.35
                    })
            }) =>
        {
            let eye = Vec3::from_array(session.confirmed_state().unwrap().position)
                + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            let Some(rotation) = world
                .query_filtered::<&terra_player::CameraLook, With<terra_player::TerraCamera>>()
                .iter(world)
                .next()
                .map(|look| look.rotation())
            else {
                return Ok(());
            };
            let Some(hit) = world.resource::<terra_player::TargetBlock>().0 else {
                return Ok(());
            };
            let chunks = world.resource::<terra_world::streaming::ChunkStore>();
            let journal = world.resource::<terra_world::edits::EditJournal>();
            let Some((position, expected_material)) =
                terra_world::targeting::item_placement_with_state(
                    chunks,
                    journal,
                    hit,
                    terra_core::crusher::CRANK_MATERIAL,
                )
            else {
                return Ok(());
            };
            if Some(position) != inspection.crank
                || !terra_world::targeting::hand_crank_mount_hit(
                    journal,
                    hit,
                    (
                        eye.to_array().map(f64::from),
                        (rotation * Vec3::NEG_Z).to_array().map(f64::from),
                    ),
                )
            {
                return Ok(());
            }
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material,
                material: terra_core::crusher::CRANK_MATERIAL,
            });
            advance(inspection, 13);
        }
        13 if session
            .inventory
            .count(ItemKind::Block(terra_core::crusher::CRANK_MATERIAL))
            == 0 =>
        {
            let crank = inspection.crank.ok_or("Missing crank attachment cell")?;
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(crank)
                != Some(terra_core::crusher::CRANK_MATERIAL)
                || world
                    .resource::<terra_world::edits::EditJournal>()
                    .crank_anchor(crank)
                    != inspection.anchor
            {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            advance(inspection, 14);
        }
        14 if elapsed > 0.7 => {
            let render = world.resource::<terra_render::CrusherRenderStatus>();
            if render.machines != 1 || render.cranks != 1 || render.cached_meshes != 9 {
                return Err(format!("Mounted crank geometry missing: {render:?}"));
            }
            if !captured(world, inspection, "crusher-placed", 1)? {
                return Ok(());
            }
            advance(inspection, 22);
        }
        22 if elapsed > 1.0 => {
            verify_drive(world, inspection, false)?;
            world.resource_mut::<terra_core::AudioConfig>().muted = true;
            advance(inspection, 23);
        }
        23 if elapsed > 0.3 => {
            verify_drive(world, inspection, false)?;
            world.resource_mut::<terra_core::AudioConfig>().muted = false;
            advance(inspection, 24);
        }
        24 if elapsed > 0.3 => {
            verify_drive(world, inspection, false)?;
            world.write_message(terra_ui::InventoryAction::OpenChest(
                inspection.anchor.unwrap(),
            ));
            advance(inspection, 4);
        }
        4 if session
            .container
            .as_ref()
            .is_some_and(|c| c.crusher.is_some())
            && world.resource::<terra_ui::InventoryView>().crusher
            && elapsed > 0.5 =>
        {
            validate_layout(world)?;
            let recipe = &terra_core::crusher::recipes()[inspection.recipe];
            world.write_message(terra_ui::CraftingAction::Search(recipe.key.clone()));
            advance(inspection, 5);
        }
        5 if world.resource::<terra_ui::CraftingView>().recipe_open && elapsed > 0.5 => {
            let view = world.resource::<terra_ui::CraftingView>();
            if !view.crushing || !view.can_apply_recipe || view.recipe_station != "Crusher" {
                return Err(format!("TMI crusher recipe unavailable: {}", view.message));
            }
            if inspection.recipe == 0 && !captured(world, inspection, "crusher-recipe", 2)? {
                return Ok(());
            }
            world.write_message(terra_ui::CraftingAction::ApplyRecipe);
            advance(inspection, 6);
        }
        6 if elapsed > 0.7
            && session
                .container
                .as_ref()
                .and_then(|c| c.halves.first())
                .and_then(|half| half.inventory.slot(terra_core::crusher::INPUT))
                .is_some_and(|stack| {
                    stack.item == terra_core::crusher::recipes()[inspection.recipe].input
                        && stack.count == 1
                }) =>
        {
            if world.resource::<terra_ui::CraftingView>().recipe_open {
                return Err("Crusher Plus left recipe popup open".into());
            }
            let container = session.container.as_ref().unwrap();
            let crusher = container.crusher.as_ref().unwrap();
            if crusher.working
                || crusher.progress != 0.0
                || container.halves[0]
                    .inventory
                    .slot(terra_core::crusher::OUTPUT)
                    .is_some()
                || world
                    .resource::<terra_network::client::CrankInput>()
                    .0
                    .is_some()
            {
                return Err("Crusher processed material without holding its mounted crank".into());
            }
            inspection.recipe_drive_seen = false;
            world.write_message(terra_ui::InventoryAction::Close);
            advance(inspection, 15);
        }
        15 if elapsed > 0.3 && !world.resource::<terra_ui::InventoryView>().open => {
            advance(inspection, if inspection.recipe == 0 { 17 } else { 20 });
        }
        17 if elapsed > 1.0 => {
            verify_drive(world, inspection, true)?;
            inspection.recipe_drive_seen = true;
            let render = world.resource::<terra_render::CrusherRenderStatus>();
            let active = render.working == 1 && render.particles > 0;
            let moved =
                world
                    .query::<(&Name, &Transform)>()
                    .iter(world)
                    .any(|(name, transform)| {
                        name.as_str().starts_with("Hand crank rotor ")
                            && !transform.rotation.abs_diff_eq(Quat::IDENTITY, 0.001)
                    });
            if !active || !moved {
                return Err("Manual crusher drive or chip animation did not start".into());
            }
            if !captured(world, inspection, "crusher-running", 3)? {
                return Ok(());
            }
            advance(inspection, 18);
        }
        18 if elapsed > 0.65 => {
            let current = visual(world, inspection).ok_or("Missing released crusher visual")?;
            if current.manual_active
                || current.working
                || world
                    .resource::<terra_network::client::CrankInput>()
                    .0
                    .is_some()
            {
                return Err("Releasing Place did not stop the crank authority lease".into());
            }
            if current.progress <= 0.0 || current.progress >= 1.0 {
                return Err("Release probe did not pause a partially processed input".into());
            }
            inspection.paused_progress = current.progress;
            advance(inspection, 19);
        }
        19 if elapsed > 0.7 => {
            let current = visual(world, inspection).ok_or("Missing paused crusher visual")?;
            if current.manual_active
                || current.working
                || current.progress != inspection.paused_progress
            {
                return Err("Crusher progress continued after the hand crank was released".into());
            }
            advance(inspection, 20);
        }
        20 => {
            if elapsed > 0.8 && !inspection.recipe_drive_seen {
                verify_drive(world, inspection, true)?;
                inspection.recipe_drive_seen = true;
            }
            if elapsed > terra_core::crusher::recipes()[inspection.recipe].seconds + 0.8 {
                let current =
                    visual(world, inspection).ok_or("Missing completed crusher visual")?;
                if !current.manual_active || current.working || !inspection.recipe_drive_seen {
                    return Err(format!(
                        "Four seconds of real cranking did not finish its input: {current:?}"
                    ));
                }
                world.write_message(terra_ui::InventoryAction::OpenChest(
                    inspection.anchor.unwrap(),
                ));
                advance(inspection, 21);
            }
        }
        21 if elapsed > 0.4
            && session
                .container
                .as_ref()
                .is_some_and(|c| c.crusher.is_some()) =>
        {
            advance(inspection, 7);
        }
        7 if session
            .container
            .as_ref()
            .and_then(|c| c.halves.first())
            .and_then(|h| h.inventory.slot(1))
            .is_some() =>
        {
            let container = session.container.as_ref().unwrap();
            let expected = terra_core::crusher::recipes()[inspection.recipe].output;
            if container.halves[0].inventory.slot(1) != Some(expected) {
                return Err("Crusher output differs from its JSON recipe".into());
            }
            if container.halves[0]
                .inventory
                .slot(terra_core::crusher::INPUT)
                .is_some()
            {
                return Err("Crusher retained its consumed input".into());
            }
            world.write_message(ContainerRequest(ContainerCommand {
                inventory_revision: session.inventory.revision,
                reserved: [0; terra_core::INVENTORY_SLOTS],
                anchor: container.anchor,
                revisions: container
                    .halves
                    .iter()
                    .map(|h| h.inventory.revision)
                    .collect(),
                action: terra_core::containers::ContainerGesture::QuickMove {
                    slot: (terra_core::INVENTORY_SLOTS + terra_core::crusher::OUTPUT) as u8,
                },
            }));
            advance(inspection, 8);
        }
        8 if session
            .container
            .as_ref()
            .and_then(|c| c.halves.first())
            .is_some_and(|h| h.inventory.slot(1).is_none())
            && elapsed > 0.25 =>
        {
            if session.last_inventory != Some(InventoryOutcome::Applied)
                || session.inventory.cursor.is_some()
            {
                return Err("Crusher output transfer failed".into());
            }
            inspection.recipe += 1;
            if inspection.recipe < terra_core::crusher::recipes().len() {
                advance(inspection, 4);
            } else {
                let mut expected = std::collections::BTreeMap::new();
                for recipe in terra_core::crusher::recipes() {
                    *expected.entry(recipe.output.item).or_insert(0_u16) += recipe.output.count;
                }
                for recipe in terra_core::crusher::recipes() {
                    // Some outputs are inputs to a later pigment recipe. The
                    // fixture supplies one input per recipe and keeps produced stock.
                    if session.inventory.count(recipe.input)
                        != expected.get(&recipe.input).copied().unwrap_or(0)
                    {
                        return Err("Crusher did not consume exactly one input".into());
                    }
                }
                for (item, count) in expected {
                    if session.inventory.count(item) != count {
                        return Err("Crusher lost or duplicated a processed result".into());
                    }
                }
                if !inspection.drive_seen
                    || !inspection.audio_seen
                    || !inspection.foley_seen
                    || session
                        .inventory
                        .count(ItemKind::Block(terra_core::crusher::MATERIAL))
                        != 0
                    || session
                        .inventory
                        .count(ItemKind::Block(terra_core::crusher::CRANK_MATERIAL))
                        != 0
                {
                    return Err("Crusher fixture skipped manual/audio verification or lost its placed station".into());
                }
                let journal = world.resource::<terra_world::edits::EditJournal>();
                if !journal.valid_crusher(inspection.anchor.unwrap())
                    || journal.crank_anchor(inspection.crank.unwrap()) != inspection.anchor
                    || world
                        .resource::<terra_render::CrusherRenderStatus>()
                        .cached_meshes
                        != 9
                {
                    return Err(
                        "Mounted crank or bounded moving-mesh cache changed during recipe loops"
                            .into(),
                    );
                }
                for mut window in world
                    .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
                    .iter_mut(world)
                {
                    window.resolution.set(960.0, 720.0);
                }
                advance(inspection, 9);
            }
        }
        9 if elapsed > 0.8 => {
            validate_layout(world)?;
            if !captured(world, inspection, "crusher-compact", 4)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(inspection, 10);
        }
        10 if elapsed > 0.8 && !world.resource::<terra_ui::InventoryView>().open => {
            let anchor = inspection.anchor.ok_or("Missing overview anchor")?;
            let facing = world
                .resource::<terra_world::edits::EditJournal>()
                .object_state(anchor)
                .ok_or("Missing overview orientation")?
                .facing;
            let standing = machine_point(
                anchor,
                facing,
                Vec3::new(2.55, terra_physics::character::PLAYER_HALF_HEIGHT, 2.7),
            );
            inspection.standing = Some(standing);
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(format!(
                    "/tp {:.3} {:.3} {:.3}",
                    standing.x, standing.y, standing.z
                )));
            advance(inspection, 25);
        }
        25 if elapsed > 1.2
            && !session.teleporting
            && session.confirmed_state().is_some_and(|state| {
                state.grounded
                    && inspection.standing.is_some_and(|point| {
                        Vec3::from_array(state.position).distance(point) < 0.35
                    })
            }) =>
        {
            if !captured(world, inspection, "crusher-results-world", 5)? {
                return Ok(());
            }
            advance(inspection, 11);
        }
        11 if inspection.captures >= 5 && elapsed > 0.5 => {
            info!(
                recipes = inspection.recipe,
                captures = inspection.captures,
                "Native mounted crank, real Place input, release pause, TMI, audio and all recipe outputs passed"
            );
            inspection.stage = 255;
            use_input(world, false);
            if let Some(audio) = inspection.original_audio.take() {
                world.insert_resource(audio);
            }
            world.write_message(bevy::app::AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_and_mount_points_follow_every_crusher_orientation() {
        let anchor = [-18, 7, -32];
        for facing in 0..4 {
            let state = terra_world::edits::BlockObjectState {
                facing,
                ..default()
            };
            let right = Vec3::from_array(state.right().map(|axis| axis as f32));
            let forward =
                Quat::from_rotation_y(f32::from(facing) * std::f32::consts::FRAC_PI_2) * Vec3::Z;
            let center =
                Vec3::from_array(anchor.map(|axis| axis as f32)) + Vec3::new(0.5, 0.0, 0.5);
            let gear = machine_point(anchor, facing, Vec3::new(1.997, 0.455, 0.515));
            assert!(
                gear.distance(center + right * 1.497 + Vec3::Y * 0.455 + forward * 0.015) < 0.00001
            );
            let operator = machine_point(anchor, facing, Vec3::new(4.2, 0.9, 0.515));
            assert!((operator - gear).dot(right) > 2.2);
            assert!((operator - gear).dot(right) < 2.21);
        }
    }

    #[test]
    fn fixture_drives_the_configured_place_key_mouse_and_chord_without_crank_shortcuts() {
        let mut world = World::new();
        world.init_resource::<terra_core::ControlsConfig>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        for binding in [
            InputBinding::Key(KeyCode::KeyU),
            InputBinding::Mouse(MouseButton::Right),
            InputBinding::Chord {
                modifier: KeyCode::ControlLeft,
                key: KeyCode::KeyU,
            },
        ] {
            world
                .resource_mut::<terra_core::ControlsConfig>()
                .rebind(GameAction::Place, binding)
                .unwrap();
            use_input(&mut world, true);
            match binding {
                InputBinding::Key(code) => {
                    assert!(world.resource::<ButtonInput<KeyCode>>().pressed(code))
                }
                InputBinding::Mouse(button) => {
                    assert!(world.resource::<ButtonInput<MouseButton>>().pressed(button))
                }
                InputBinding::Chord { modifier, key } => {
                    assert!(world.resource::<ButtonInput<KeyCode>>().pressed(modifier));
                    assert!(world.resource::<ButtonInput<KeyCode>>().pressed(key));
                }
            }
            use_input(&mut world, false);
            assert!(
                world
                    .resource::<ButtonInput<KeyCode>>()
                    .get_pressed()
                    .next()
                    .is_none()
            );
            assert!(
                world
                    .resource::<ButtonInput<MouseButton>>()
                    .get_pressed()
                    .next()
                    .is_none()
            );
        }
        assert!(!world.contains_resource::<terra_network::client::CrankInput>());
    }
}
