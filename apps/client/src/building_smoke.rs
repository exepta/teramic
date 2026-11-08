//! Native acceptance through real input, UDP authority and replicated inventory.
use bevy::{app::AppExit, prelude::*, window::PrimaryWindow};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{Config, GameAction, GameMode, InputBinding, ItemKind};
use terra_network::client::ClientSession;
use terra_player::{
    CameraInputState, ControllerMode, PlayerBody, PlayerInputBlock, TerraCamera,
    interaction::{BlockPalette, MiningVisual, PlacementPreview},
};
use terra_render::{BuildingStatus, RenderStatus};
use terra_ui::{InventoryAction, InventoryView};
use terra_world::streaming::{ChunkStore, GenerationStatus, WorldGeneration};

pub(crate) fn prepare(
    mut config: terra_core::GameConfig,
) -> Result<
    (
        terra_server::IntegratedServer,
        Vec<u8>,
        terra_core::PlayerProfile,
    ),
    String,
> {
    let profile_path =
        terra_core::PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|error| error.to_string())?;
    let profile = terra_core::PlayerProfile::load_or_create(profile_path)
        .map_err(|error| error.to_string())?;
    config.server.game_mode = GameMode::Creative;
    // Only the explicit native furniture fixture grants its persisted test
    // identity permission to switch into the authoritative survival phase.
    config.server.player_permissions.insert(
        profile.uuid,
        terra_core::PlayerPermissions {
            game_mode: true,
            build: true,
            flight: true,
            environment: true,
        },
    );
    let (host, token) = terra_server::IntegratedServer::start(config)?;
    Ok((host, token, profile))
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Loading,
    FurnitureGrant,
    FurniturePlace,
    FurniturePulse,
    FurnitureCommit,
    FurniturePlaced,
    ChestOpenStart,
    ChestOpenWait,
    ChestCloseWait,
    FurnitureAim,
    FurnitureMine,
    FurnitureRelease,
    FurnitureCancel,
    BedDyeGrant,
    BedDyeAim,
    BedDyed,
    BedSleepStart,
    BedSleepPose,
    BedWake,
    Center,
    Aim,
    ClearProps,
    DebugEnable,
    InvalidWait,
    CancelStart,
    CancelCapture,
    CancelWait,
    Mine,
    Pickup,
    InventoryOpen,
    InventoryCursor,
    InventorySelect,
    InventoryCapture,
    InventorySmall,
    Resume,
    Preview,
    Narrow,
    NarrowCapture,
    NarrowRestore,
    Place,
    Placed,
    PlacedCapture,
    Finish,
    Done,
}
#[derive(Resource)]
struct Fixture {
    started: Instant,
    entered: Instant,
    stage: Stage,
    target: [i64; 3],
    point: Vec3,
    material: u16,
    count: u16,
    creative: bool,
    resolution: (u32, u32),
    peak_particles: usize,
    peak_drops: usize,
    centered: bool,
    fov: f32,
    fracture_captures: u8,
    furniture: Option<u16>,
    sleep_day: u64,
    placement_confirmed: Option<Instant>,
    placement_visible: bool,
    outline_only: bool,
    outline_alpha: Option<(f32, f32)>,
    outline_mesh: Option<AssetId<Mesh>>,
}
impl Fixture {
    fn advance(&mut self, stage: Stage) {
        self.stage = stage;
        self.entered = Instant::now();
    }
    fn elapsed(&self, seconds: f32) -> bool {
        self.entered.elapsed().as_secs_f32() >= seconds
    }
}
#[derive(Default)]
struct Input {
    mine: bool,
    place: bool,
    inventory: bool,
    pause: bool,
    forward: bool,
    debug_f1: bool,
    debug_b: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-building") {
        return;
    }
    let now = Instant::now();
    let outline_fixture = std::env::var("TERRA_PLACEMENT_OUTLINE_SMOKE").ok();
    let furniture_fixture = std::env::var("TERRA_FURNITURE_MINING_SMOKE").ok();
    // This native fixture omits the menu, which normally initializes graphics state.
    app.init_resource::<terra_core::GraphicsConfig>()
        .insert_resource(ControllerMode::Walk)
        .insert_resource(Fixture {
            started: now,
            entered: now,
            stage: Stage::Loading,
            target: [0; 3],
            point: Vec3::ZERO,
            material: 0,
            count: 0,
            creative: false,
            resolution: (1280, 720),
            peak_particles: 0,
            peak_drops: 0,
            centered: false,
            fov: 90.0,
            fracture_captures: 0,
            furniture: outline_fixture.as_ref().or(furniture_fixture.as_ref()).map(
                |name| match name.as_str() {
                    "slab" => 82,
                    "stairs" => 74,
                    "chest" => 29,
                    "bed" => 71,
                    _ => 28,
                },
            ),
            sleep_day: 0,
            placement_confirmed: None,
            placement_visible: false,
            outline_only: outline_fixture.is_some(),
            outline_alpha: None,
            outline_mesh: None,
        })
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if matches!(state.stage, Stage::Done) {
            return;
        }
        if std::env::var_os("TERRA_PLACEMENT_TIMING").is_some() && matches!(state.stage, Stage::Place | Stage::Placed) {
            tracing::info!(frame_ms=world.resource::<Time<bevy::time::Real>>().delta_secs_f64()*1000.0, stage=?state.stage, "Native placement frame");
        }
        let mut input = Input::default();
        match advance(world, &mut state, &mut input) {
            Ok(true) => {
                state.stage = Stage::Done;
                world.write_message(AppExit::Success);
            }
            Err(error) => {
                tracing::error!(%error, stage = ?state.stage, "Native building fixture failed");
                state.stage = Stage::Done;
                world.write_message(AppExit::error());
            }
            Ok(false) => {}
        }
        for (action, pressed) in [
            (GameAction::Break, input.mine),
            (GameAction::Place, input.place),
            (GameAction::Inventory, input.inventory),
            (GameAction::Pause, input.pause),
            (GameAction::Forward, input.forward),
        ] {
            let binding = world
                .get_resource::<terra_core::ControlsConfig>()
                .map_or_else(|| action.default_binding(), |config| config.binding(action));
            match binding {
                InputBinding::Chord { modifier, key } => {
                    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
                    for key in [modifier, key] {
                        if pressed {
                            keys.press(key);
                        } else {
                            keys.release(key);
                        }
                    }
                }
                InputBinding::Key(key) => {
                    let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
                    if pressed {
                        keys.press(key);
                    } else {
                        keys.release(key);
                    }
                }
                InputBinding::Mouse(button) => {
                    let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
                    if pressed {
                        buttons.press(button);
                    } else {
                        buttons.release(button);
                    }
                }
            }
        }
        for (key, pressed) in [
            (KeyCode::F1, input.debug_f1),
            (KeyCode::KeyB, input.debug_b),
        ] {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            if pressed {
                keys.press(key);
            } else {
                keys.release(key);
            }
        }
        // Native smoke mode suppresses OS grabs; gameplay still obeys the real UI input lock.
        let capture = !world.resource::<PlayerInputBlock>().0;
        world.resource_mut::<CameraInputState>().captured = capture;
    });
}

fn advance(world: &mut World, state: &mut Fixture, input: &mut Input) -> Result<bool, String> {
    if state.started.elapsed() > Duration::from_secs(100) {
        return Err("Timed out".into());
    }
    if let Some(error) = &world.resource::<ClientSession>().error {
        return Err(error.clone());
    }
    if let Some(error) = &world.resource::<RenderStatus>().error {
        return Err(error.clone());
    }
    if let Some(error) = &world.resource::<terra_render::RenderBlockPreviews>().error {
        return Err(error.clone());
    }
    state.peak_particles = state
        .peak_particles
        .max(world.resource::<BuildingStatus>().particles);
    state.peak_drops = state
        .peak_drops
        .max(world.resource::<BuildingStatus>().drops);
    if !matches!(state.stage, Stage::Loading) {
        for mut camera in world
            .query_filtered::<&mut Transform, With<TerraCamera>>()
            .iter_mut(world)
        {
            camera.look_at(state.point, Vec3::Y);
        }
    }
    match state.stage {
        Stage::Loading => {
            if !world.contains_resource::<terra_server::IntegratedServer>() {
                return Err("Building acceptance requires the real integrated host".into());
            }
            let session = world.resource::<ClientSession>();
            if !session.ready
                || !world.resource::<RenderStatus>().ready
                || world.resource::<WorldGeneration>().status != GenerationStatus::Ready
                || world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .pending
                    != 0
                || world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .images
                    .is_empty()
                || session.confirmed_state().is_none_or(|body| !body.grounded)
            {
                return Ok(false);
            }
            if world
                .resource::<BlockPalette>()
                .choices
                .iter()
                .any(|choice| {
                    !world
                        .resource::<terra_render::RenderBlockPreviews>()
                        .images
                        .contains_key(&choice.id)
                })
            {
                return Err(
                    "Not every registry block has a verified rendered inventory preview".into(),
                );
            }
            state.creative = session.game_mode == GameMode::Creative;
            let body = world
                .query_filtered::<&Transform, With<PlayerBody>>()
                .iter(world)
                .next()
                .ok_or("No authoritative player body")?
                .translation;
            if let Some(material) = state.furniture {
                if world.resource::<ClientSession>().game_mode != GameMode::Creative {
                    return Err("Furniture mining fixture requires TERRA_BUILD_CREATIVE=1 for its real placement setup".into());
                }
                let (position, aim) = furniture_site(
                    world.resource::<ChunkStore>(),
                    world.resource::<terra_world::edits::EditJournal>(),
                    world.resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                    body,
                    material,
                )
                .ok_or("No unobstructed, supported furniture site within real interaction reach")?;
                state.target = position;
                state.point = aim;
                state.material = material;
                let revision = world.resource::<ClientSession>().inventory.revision;
                world.write_message(terra_network::client::InventoryRequest(
                    terra_network::protocol::inventory::InventoryCommand {
                        revision,
                        action: terra_network::protocol::inventory::InventoryAction::CreativeSet {
                            slot: 0,
                            item: ItemKind::for_wood(material, terra_core::WoodSpecies::Birch),
                            count: 1,
                        },
                    },
                ));
                state.advance(Stage::FurnitureGrant);
                return Ok(false);
            }
            if !state.centered {
                state.centered = true;
                state.point = Vec3::new(
                    body.x.floor() + 0.5,
                    body.y + terra_physics::character::PLAYER_EYE_OFFSET,
                    body.z.floor() + 0.5,
                );
                if Vec2::new(body.x - state.point.x, body.z - state.point.z).length() > 0.07 {
                    state.advance(Stage::Center);
                    return Ok(false);
                }
            }
            let (target, point, material) = select_ground(world.resource::<ChunkStore>(), body)
                .ok_or_else(|| format!("No nearby exposed ground block with unobstructed pickup and replacement at {body:?}"))?;
            state.target = target;
            state.point = point;
            state.material = material;
            state.count = world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(material));
            state.fov = world
                .resource::<terra_core::GameConfig>()
                .client
                .fov_degrees;
            if let Some(window) = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
            {
                state.resolution = (window.physical_width(), window.physical_height());
            }
            tracing::info!(
                ?target,
                material,
                creative = state.creative,
                "Native building session ready"
            );
            state.advance(Stage::Aim);
        }
        Stage::FurnitureGrant if state.elapsed(0.4) => {
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_some_and(|stack| {
                    stack.item == ItemKind::for_wood(state.material, terra_core::WoodSpecies::Birch)
                        && stack.count == 1
                })
            {
                world.write_message(InventoryAction::Hotbar(0));
                state.advance(Stage::FurniturePlace);
            } else if state.elapsed(4.0) {
                return Err(
                    "Furniture grant was not acknowledged by the integrated authority".into(),
                );
            }
        }
        Stage::FurniturePlace if state.elapsed(0.5) => {
            let hit = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("Missing actual furniture placement target")?;
            let (position, _) =
                terra_world::targeting::block_placement(world.resource::<ChunkStore>(), hit)
                    .ok_or("Invalid actual furniture placement target")?;
            if position != state.target {
                return Err(format!(
                    "Furniture placement moved: {position:?} / {:?}",
                    state.target
                ));
            }
            let preview = world.resource::<PlacementPreview>();
            if preview.position != Some(position) || preview.plan.is_none() || !preview.valid {
                return Err("Shared furniture placement silhouette is missing or disagrees with the actual resolver".into());
            }
            if terra_core::construction::is_slab(state.material)
                && preview.plan.as_ref().is_none_or(|plan| {
                    !plan.states.iter().any(|(cell, object)| {
                        *cell == position
                            && object.is_some_and(|object| (5..=8).contains(&object.facing))
                    })
                })
            {
                let facings: Vec<_> = preview
                    .plan
                    .as_ref()
                    .into_iter()
                    .flat_map(|plan| &plan.states)
                    .map(|(cell, state)| (*cell, state.map(|state| state.facing)))
                    .collect();
                let ray_point = world
                    .query_filtered::<(&Transform, &terra_player::CameraLook), With<TerraCamera>>()
                    .iter(world)
                    .next()
                    .map(|(camera, look)| {
                        camera.translation + (look.rotation() * Vec3::NEG_Z) * hit.distance as f32
                    });
                return Err(format!(
                    "Actual floor-center slab preview is not vertical: target={position:?}, normal={:?}, aim={:?}, ray_point={ray_point:?}, facings={facings:?}",
                    hit.normal, state.point
                ));
            }
            let sample = require_visible_outline(world, position, state.material)?;
            state.outline_alpha = Some((sample.alpha, sample.alpha));
            state.outline_mesh = Some(sample.mesh);
            capture(world, &format!("furniture-{}-placement", state.material))?;
            state.advance(Stage::FurniturePulse);
        }
        Stage::FurniturePulse if state.elapsed(0.05) => {
            let sample = require_visible_outline(world, state.target, state.material)?;
            if state.outline_mesh != Some(sample.mesh) {
                return Err("Pulsing placement outline replaced its cached mesh".into());
            }
            let alpha = sample.alpha;
            let (minimum, maximum) = state.outline_alpha.unwrap_or((alpha, alpha));
            let range = (minimum.min(alpha), maximum.max(alpha));
            state.outline_alpha = Some(range);
            if state.elapsed(0.2) && range.1 - range.0 >= 0.025 {
                tracing::info!(
                    material = state.material,
                    minimum_alpha = range.0,
                    maximum_alpha = range.1,
                    "Native orange placement outline pulses without filled faces"
                );
                capture(
                    world,
                    &format!("furniture-{}-outline-pulse", state.material),
                )?;
                state.advance(
                    if state.outline_only && !terra_core::construction::is_slab(state.material) {
                        Stage::Finish
                    } else {
                        Stage::FurnitureCommit
                    },
                );
            } else if state.elapsed(1.5) {
                return Err(format!(
                    "Placement outline opacity did not visibly pulse: {range:?}"
                ));
            }
        }
        Stage::FurnitureCommit if state.elapsed(0.25) => {
            let position = state.target;
            let expected_material = world
                .resource::<ChunkStore>()
                .block_at(position)
                .ok_or("Furniture preview section unloaded before commit")?;
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material,
                material: state.material,
            });
            state.advance(Stage::FurniturePlaced);
        }
        Stage::FurniturePlaced if state.elapsed(0.7) => {
            if world.resource::<ChunkStore>().block_at(state.target) != Some(state.material) {
                if state.elapsed(4.0) {
                    return Err("Furniture was not actually placed by the authority".into());
                }
                return Ok(false);
            }
            if !world.resource::<RenderStatus>().ready {
                return Ok(false);
            }
            if state.outline_only && terra_core::construction::is_slab(state.material) {
                let facing = world
                    .resource::<terra_world::edits::EditJournal>()
                    .object_state(state.target)
                    .ok_or("Committed floor slab lost its orientation")?
                    .facing;
                if !(5..=8).contains(&facing) {
                    return Err(format!(
                        "Authority did not preserve vertical floor placement: {facing}"
                    ));
                }
                capture(world, "slab-floor-center-committed")?;
                tracing::info!(?state.target, facing, "Native floor-center slab placement committed its vertical preview orientation");
                state.advance(Stage::Finish);
                return Ok(false);
            }
            let peer = world
                .resource::<terra_world::edits::EditJournal>()
                .object_state(state.target)
                .and_then(|state| state.peer);
            if state.material != 29 {
                state.point = Vec3::from_array(state.target.map(|v| v as f32 + 0.5));
                if let Some(peer) = peer {
                    state.point =
                        (state.point + Vec3::from_array(peer.map(|v| v as f32 + 0.5))) * 0.5;
                }
                state.point.y =
                    state.target[1] as f32 + if state.material == 28 { 0.70 } else { 0.40 };
            }
            if terra_core::beds::is_bed(state.material) {
                state.point.y = state.target[1] as f32 + 0.62;
                capture(world, "bed-intact")?;
                world.write_message(terra_network::client::InventoryRequest(
                    terra_network::protocol::inventory::InventoryCommand {
                        revision: world.resource::<ClientSession>().inventory.revision,
                        action: terra_network::protocol::inventory::InventoryAction::CreativeSet {
                            slot: 1,
                            item: ItemKind::Dye(terra_core::beds::DyeColor::Purple),
                            count: 2,
                        },
                    },
                ));
                state.advance(Stage::BedDyeGrant);
                return Ok(false);
            }
            if state.material == 29 {
                let empty = (0..9)
                    .find(|&slot| {
                        world
                            .resource::<ClientSession>()
                            .inventory
                            .slot(slot)
                            .is_none()
                    })
                    .ok_or("Chest visual fixture needs one actually empty hotbar slot")?
                    as u8;
                world
                    .resource_mut::<terra_network::client::SelectedInventorySlot>()
                    .0 = Some(empty);
                world.write_message(InventoryAction::Hotbar(empty));
                world
                    .resource_mut::<terra_player::interaction::ActiveItem>()
                    .0 = None;
                let mut palette = world.resource_mut::<BlockPalette>();
                palette.selected = palette.choices.len();
                state.advance(Stage::ChestOpenStart);
                return Ok(false);
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/gamemode survival".into(),
                ));
            state.advance(Stage::FurnitureAim);
        }
        Stage::ChestOpenStart if state.elapsed(0.5) => {
            let selected = world
                .resource::<terra_network::client::SelectedInventorySlot>()
                .0
                .ok_or("Missing empty hotbar selection")?;
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(usize::from(selected))
                .is_some()
                || world
                    .resource::<terra_player::interaction::ActiveItem>()
                    .0
                    .is_some()
                || world.resource::<PlacementPreview>().position.is_some()
                || world.resource::<BuildingStatus>().preview_visible
            {
                return Err("Empty-hand chest inspection still has a held item or phantom placement silhouette".into());
            }
            world.write_message(terra_network::client::InventoryRequest(
                terra_network::protocol::inventory::InventoryCommand {
                    revision: world.resource::<ClientSession>().inventory.revision,
                    action: terra_network::protocol::inventory::InventoryAction::OpenContainer {
                        position: state.target,
                    },
                },
            ));
            state.advance(Stage::ChestOpenWait);
        }
        Stage::ChestOpenWait if state.elapsed(0.7) => {
            let session = world.resource::<ClientSession>();
            if !session.open_chests.contains(&state.target) || session.container.is_none() {
                if state.elapsed(4.0) {
                    return Err(
                        "Authoritative chest viewer state did not open the actual world lid".into(),
                    );
                }
                return Ok(false);
            }
            if world
                .resource::<crate::creature_audio::ChestAudioStatus>()
                .opened
                == 0
            {
                if state.elapsed(4.0) {
                    return Err(
                        "Authoritative chest opening did not enqueue its positional hinge sound"
                            .into(),
                    );
                }
                return Ok(false);
            }
            capture(world, "chest-lid-open")?;
            world.write_message(terra_network::client::InventoryRequest(
                terra_network::protocol::inventory::InventoryCommand {
                    revision: world.resource::<ClientSession>().inventory.revision,
                    action: terra_network::protocol::inventory::InventoryAction::CloseContainer,
                },
            ));
            state.advance(Stage::ChestCloseWait);
        }
        Stage::ChestCloseWait if state.elapsed(0.7) => {
            let session = world.resource::<ClientSession>();
            if session.open_chests.contains(&state.target) || session.container.is_some() {
                if state.elapsed(4.0) {
                    return Err(
                        "Chest viewer state did not close after the last viewer left".into(),
                    );
                }
                return Ok(false);
            }
            if world
                .resource::<crate::creature_audio::ChestAudioStatus>()
                .closed
                == 0
            {
                if state.elapsed(4.0) {
                    return Err(
                        "Authoritative chest closing did not enqueue its positional latch sound"
                            .into(),
                    );
                }
                return Ok(false);
            }
            info!(status = ?world.resource::<crate::creature_audio::ChestAudioStatus>(), "Native authoritative chest opening/closing audio cues passed");
            capture(world, "chest-lid-closed")?;
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/gamemode survival".into(),
                ));
            state.advance(Stage::FurnitureAim);
        }
        Stage::BedDyeGrant if state.elapsed(0.4) => {
            if world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Dye(terra_core::beds::DyeColor::Purple))
                != 2
            {
                if state.elapsed(4.0) {
                    return Err("Bed dye grant was not acknowledged".into());
                }
                return Ok(false);
            }
            world.write_message(InventoryAction::Hotbar(1));
            for command in ["/gamemode survival", "/time set 23:00"] {
                world
                    .resource_mut::<terra_network::client::ClientChatCommands>()
                    .0
                    .push_back(terra_network::client::ChatCommand::Send(command.into()));
            }
            state.advance(Stage::BedDyeAim);
        }
        Stage::BedDyeAim if state.elapsed(0.5) => {
            if world.resource::<ClientSession>().game_mode != GameMode::Survival
                || world.resource::<terra_core::WorldClock>().hour() < 22.5
            {
                if state.elapsed(4.0) {
                    return Err(
                        "Bed fixture could not enter authoritative night and Survival mode".into(),
                    );
                }
                return Ok(false);
            }
            interact_with_bed(world)?;
            state.advance(Stage::BedDyed);
        }
        Stage::BedDyed if state.elapsed(0.4) => {
            let journal = world.resource::<terra_world::edits::EditJournal>();
            let bed = journal
                .object_state(state.target)
                .ok_or("Placed bed lost its metadata")?;
            let dyed = terra_core::beds::bed_color(state.material, bed.facing)
                == terra_core::beds::DyeColor::Purple
                && bed
                    .peer
                    .and_then(|peer| journal.object_state(peer))
                    .is_some_and(|other| other.facing == bed.facing);
            if !dyed
                || world
                    .resource::<ClientSession>()
                    .inventory
                    .count(ItemKind::Dye(terra_core::beds::DyeColor::Purple))
                    != 1
            {
                if state.elapsed(4.0) {
                    return Err("Dyeing failed to recolor both bed cells and consume exactly one Survival dye".into());
                }
                return Ok(false);
            }
            capture(world, "bed-purple-survival")?;
            world.write_message(InventoryAction::Hotbar(0));
            state.advance(Stage::BedSleepStart);
        }
        Stage::BedSleepStart if state.elapsed(0.4) => {
            state.sleep_day = world.resource::<terra_core::WorldClock>().day;
            interact_with_bed(world)?;
            state.advance(Stage::BedSleepPose);
        }
        Stage::BedSleepPose => {
            if world.resource::<ClientSession>().sleeping().is_some() && state.elapsed(1.1) {
                if !world.resource::<terra_ui::ChatView>().open
                    || world.resource::<terra_ui::SleepView>().opacity < 0.2
                {
                    return Err(
                        "Sleeping did not open chat and progressively darken with Old Film".into(),
                    );
                }
                capture(world, "bed-sleeping-pose")?;
                state.advance(Stage::BedWake);
            } else if state.elapsed(4.0) {
                return Err("Server never replicated the resting bed pose".into());
            }
        }
        Stage::BedWake => {
            let clock = *world.resource::<terra_core::WorldClock>();
            if world.resource::<ClientSession>().sleeping().is_none()
                && clock.day == state.sleep_day + 1
                && (8.0..8.5).contains(&clock.hour())
                && state.elapsed(3.0)
            {
                capture(world, "bed-morning-awake")?;
                tracing::info!(
                    day = clock.day,
                    hour = clock.hour(),
                    "Actual bed dyed in Survival, entered, and woke at the next morning via UDP authority"
                );
                state.advance(Stage::Finish);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Sleep failed to wake at next-day 08:00: day {}, hour {}",
                    clock.day,
                    clock.hour()
                ));
            }
        }
        Stage::FurnitureAim if state.elapsed(0.5) => {
            if world.resource::<ClientSession>().game_mode != GameMode::Survival {
                if state.elapsed(4.0) {
                    return Err(
                        "Furniture fixture could not enter authoritative survival mode".into(),
                    );
                }
                return Ok(false);
            }
            let hit = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("Furniture model did not become selectable")?;
            if world.resource::<ChunkStore>().block_at(hit.position) != Some(state.material) {
                return Err("Real furniture mining target is occluded".into());
            }
            state.target = hit.position;
            capture(world, &format!("furniture-{}-intact", state.material))?;
            input.mine = true;
            state.advance(Stage::FurnitureMine);
        }
        Stage::FurnitureMine => {
            input.mine = true;
            require_block(world, state, state.material)?;
            let progress = world.resource::<MiningVisual>().progress;
            if world.resource::<BuildingStatus>().crack_segments != 0
                || world.resource::<BuildingStatus>().particles > 48
            {
                return Err(
                    "Imported furniture produced a cube fracture overlay or exceeded the chip budget".into(),
                );
            }
            if let Some(threshold) =
                [0.08, 0.30, 0.55, 0.75].get(usize::from(state.fracture_captures))
                && progress >= *threshold
                && world.resource::<BuildingStatus>().isolated_block
            {
                capture(
                    world,
                    &format!(
                        "furniture-{}-mining-{}",
                        state.material, state.fracture_captures
                    ),
                )?;
                tracing::info!(
                    material = state.material,
                    progress,
                    stage = state.fracture_captures,
                    "Actual furniture surface mining captured without cube overlay"
                );
                state.fracture_captures += 1;
                if state.fracture_captures == 4 {
                    state.advance(Stage::FurnitureRelease);
                }
            }
            if state.elapsed(3.0) {
                return Err(
                    "Imported furniture never exposed its isolated actual-surface mining geometry"
                        .into(),
                );
            }
        }
        Stage::FurnitureRelease => {
            input.mine = !state.elapsed(0.035);
            if !input.mine {
                state.advance(Stage::FurnitureCancel);
            }
        }
        Stage::FurnitureCancel if state.elapsed(0.4) => {
            require_block(world, state, state.material)?;
            if world.resource::<BuildingStatus>().isolated_block
                || world.resource::<MiningVisual>().progress != 0.0
            {
                return Err(
                    "Cancelled model mining did not restore its original render batch".into(),
                );
            }
            capture(world, &format!("furniture-{}-restored", state.material))?;
            state.advance(Stage::Finish);
        }
        Stage::Center => {
            let body = world
                .query_filtered::<&Transform, With<PlayerBody>>()
                .iter(world)
                .next()
                .ok_or("No player during real centering movement")?
                .translation;
            input.forward =
                Vec2::new(body.x - state.point.x, body.z - state.point.z).length() > 0.07;
            if !input.forward {
                state.advance(Stage::Loading);
            } else if state.elapsed(3.0) {
                return Err("Could not walk off the spawn corner".into());
            }
        }
        Stage::Aim if state.elapsed(0.8) => {
            if world
                .resource::<terra_player::TargetBlock>()
                .0
                .is_none_or(|hit| hit.position != state.target)
            {
                return Err("Actual targeting did not select the natural fixture block".into());
            }
            if matches!(
                world.resource::<terra_player::TargetWorld>().0,
                Some(terra_player::WorldTarget::Prop(_))
            ) {
                input.mine = true;
                state.advance(Stage::ClearProps);
                return Ok(false);
            }
            if world.resource::<terra_core::DebugSettings>().enabled
                || world.resource::<BuildingStatus>().preview_visible
            {
                return Err("Debug and the empty-hand placement outline must start hidden".into());
            }
            capture(world, "building-debug-default-off")?;
            input.debug_f1 = true;
            input.debug_b = true;
            state.advance(Stage::DebugEnable);
        }
        Stage::DebugEnable if state.elapsed(0.2) => {
            let debug = world.resource::<terra_core::DebugSettings>();
            if !debug.enabled || !debug.block_placement {
                return Err("F1+B did not enable the placement debug setting".into());
            }
            if state.creative {
                input.mine = true;
                state.advance(Stage::Mine);
            } else {
                if world.resource::<PlacementPreview>().valid {
                    return Err("Empty survival bag allowed placement".into());
                }
                capture(world, "building-preview-unavailable")?;
                input.place = true;
                state.advance(Stage::InvalidWait);
            }
        }
        Stage::ClearProps => {
            input.mine = matches!(
                world.resource::<terra_player::TargetWorld>().0,
                Some(terra_player::WorldTarget::Prop(_))
            );
            if !input.mine {
                state.advance(Stage::Aim);
            } else if state.elapsed(8.0) {
                return Err("Could not clear a real foreground prop through normal mining".into());
            }
        }
        Stage::InvalidWait if state.elapsed(0.3) => {
            require_block(world, state, state.material)?;
            if world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(state.material))
                != state.count
            {
                return Err("Invalid placement fabricated resources".into());
            }
            input.mine = true;
            state.advance(Stage::CancelStart);
        }
        Stage::CancelStart => {
            input.mine = true;
            let progress = world.resource::<MiningVisual>().progress;
            if progress >= 0.1
                && world.resource::<BuildingStatus>().crack_segments > 0
                && world.resource::<BuildingStatus>().isolated_block
            {
                require_block(world, state, state.material)?;
                capture(world, "building-cracks")?;
                tracing::info!(
                    progress,
                    segments = world.resource::<BuildingStatus>().crack_segments,
                    "Authoritative mining isolated target with staged fracture geometry"
                );
                state.advance(Stage::CancelCapture);
            } else if state.elapsed(1.6) {
                return Err("Survival mining did not expose staged cracks".into());
            }
        }
        Stage::CancelCapture => {
            input.mine = !state.elapsed(0.035);
            if !input.mine {
                state.advance(Stage::CancelWait);
            }
        }
        Stage::CancelWait if state.elapsed(0.35) => {
            require_block(world, state, state.material)?;
            if world.resource::<ClientSession>().mining.is_some() {
                return Err("Released mining remained active".into());
            }
            if world.resource::<MiningVisual>().progress != 0.0
                || world.resource::<BuildingStatus>().isolated_block
            {
                return Err("Released cracks did not reset".into());
            }
            input.mine = true;
            state.advance(Stage::Mine);
        }
        Stage::Mine => {
            input.mine = !state.creative;
            let progress = world.resource::<MiningVisual>().progress;
            let thresholds = [0.15, 0.5, 0.85];
            if !state.creative
                && let Some(threshold) = thresholds.get(usize::from(state.fracture_captures))
                && progress >= *threshold
                && world.resource::<BuildingStatus>().isolated_block
            {
                capture(
                    world,
                    &format!("building-fracture-{}", state.fracture_captures),
                )?;
                state.fracture_captures += 1;
            }
            if state.creative
                && (world.resource::<MiningVisual>().progress != 0.0
                    || world.resource::<ClientSession>().mining.is_some())
            {
                return Err("Creative break entered survival timer".into());
            }
            if world.resource::<ChunkStore>().block_at(state.target) == Some(0) {
                input.mine = false;
                capture(world, "building-broken")?;
                tracing::info!(
                    latency_ms = state.entered.elapsed().as_millis(),
                    creative = state.creative,
                    "Authoritative block removal received"
                );
                state.advance(Stage::Pickup);
            } else if state.elapsed(3.0) {
                return Err("Held mining or creative click failed".into());
            }
        }
        Stage::Pickup => {
            let count = world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(state.material));
            if state.creative || count == state.count + 1 {
                if state.creative && count != state.count {
                    return Err("Creative generated survival resources".into());
                }
                tracing::info!(count, "Replicated pickup count verified");
                input.inventory = true;
                state.advance(Stage::InventoryOpen);
            } else if state.elapsed(3.0) {
                return Err("Real nearby drop was not picked up".into());
            }
        }
        Stage::InventoryOpen => {
            if world.resource::<InventoryView>().open {
                if !world.resource::<PlayerInputBlock>().0 {
                    return Err("Inventory did not block gameplay".into());
                }
                if state.creative
                    && !std::env::var("TERRA_BUILD_LEGACY_SELECT").is_ok_and(|value| value == "1")
                {
                    world.write_message(InventoryAction::Pointer {
                        command: "catalog".into(),
                        index: None,
                        item: format!("block-{}", state.material),
                        secondary: false,
                    });
                    state.advance(Stage::InventoryCursor);
                } else {
                    world.write_message(InventoryAction::Select(format!(
                        "block-{}",
                        state.material
                    )));
                    state.advance(Stage::InventorySelect);
                }
            } else if state.elapsed(2.0) {
                return Err("Inventory binding did not open TiltUI panel".into());
            }
        }
        Stage::InventoryCursor => {
            let session = world.resource::<ClientSession>();
            if session.inventory.cursor.is_some_and(|stack| {
                stack.item == ItemKind::Block(state.material) && stack.count > 0
            }) {
                world.write_message(InventoryAction::Pointer {
                    command: "hotbar".into(),
                    index: Some(0),
                    item: String::new(),
                    secondary: false,
                });
                state.advance(Stage::InventorySelect);
            } else if session.inventory.slot(0).is_some_and(|stack| {
                stack.item == ItemKind::Block(state.material) && stack.count > 0
            }) {
                // Current catalog grants fill free hotbar slots directly.
                state.advance(Stage::InventorySelect);
            } else if state.elapsed(5.0) {
                let view = world.resource::<InventoryView>();
                return Err(format!(
                    "Creative catalog cursor was not acknowledged: inventory={:?}, response={}, outcome={:?}, open={}, error={}",
                    session.inventory,
                    session.inventory_response,
                    session.last_inventory,
                    view.open,
                    view.error
                ));
            }
        }
        Stage::InventorySelect if state.elapsed(0.7) => {
            if state.creative {
                let granted = world
                    .resource::<ClientSession>()
                    .inventory
                    .slot(0)
                    .filter(|stack| stack.item == ItemKind::Block(state.material))
                    .map_or(0, |stack| stack.count);
                if !(1..=terra_core::STACK_LIMIT).contains(&granted)
                    || world.resource::<ClientSession>().inventory.cursor.is_some()
                {
                    if state.elapsed(5.0) {
                        let session = world.resource::<ClientSession>();
                        let view = world.resource::<InventoryView>();
                        return Err(format!(
                            "Creative hotbar did not receive its authoritative stack: inventory={:?}, response={}, outcome={:?}, open={}, error={}",
                            session.inventory,
                            session.inventory_response,
                            session.last_inventory,
                            view.open,
                            view.error
                        ));
                    }
                    return Ok(false);
                }
                // Creative placement must preserve the stack granted by the catalog.
                state.count = granted;
            }
            if world
                .resource::<BlockPalette>()
                .selected()
                .is_none_or(|choice| choice.id != state.material)
            {
                return Err("Inventory selection failed to update placement palette".into());
            }
            let view = world.resource::<InventoryView>();
            if view.items.is_empty() || view.creative != state.creative {
                return Err("Inventory view did not reflect authoritative mode/items".into());
            }
            let expected = format!("Block preview {}", state.material);
            let image = world
                .query::<(&tilt_ui::ImageMetadata, &ImageNode)>()
                .iter(world)
                .find(|(metadata, _)| metadata.alt.as_deref() == Some(expected.as_str()))
                .map(|(_, node)| node.image.clone())
                .ok_or("Missing TiltUI rendered block preview")?;
            if world
                .resource::<terra_render::RenderBlockPreviews>()
                .images
                .get(&state.material)
                != Some(&image)
            {
                return Err("Inventory is not using the real rendered block image".into());
            }
            capture(world, "building-inventory")?;
            state.advance(Stage::InventoryCapture);
        }
        Stage::InventoryCapture if state.elapsed(0.5) => {
            for mut window in world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .iter_mut(world)
            {
                window.set_cursor_position(None);
                window.resolution.set_physical_resolution(960, 640);
            }
            state.advance(Stage::InventorySmall);
        }
        Stage::InventorySmall if state.elapsed(1.0) => {
            capture(world, "building-inventory-small")?;
            state.advance(Stage::Resume);
        }
        Stage::Resume if state.elapsed(0.4) => {
            for mut window in world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .iter_mut(world)
            {
                window.set_cursor_position(None);
                window
                    .resolution
                    .set_physical_resolution(state.resolution.0, state.resolution.1);
            }
            input.pause = true;
            state.advance(Stage::Preview);
        }
        Stage::Preview if state.elapsed(0.6) => {
            if world.resource::<InventoryView>().open || world.resource::<PlayerInputBlock>().0 {
                return Err("Escape did not resume gameplay".into());
            }
            let preview = world.resource::<PlacementPreview>();
            if preview.position != Some(state.target)
                || !preview.valid
                || preview.material != state.material
            {
                return Err(format!("Replacement preview invalid: {preview:?}"));
            }
            require_no_outline(world)?;
            capture(world, "building-normal-block-no-outline")?;
            state.advance(Stage::Narrow);
        }
        Stage::Narrow if state.elapsed(0.3) => {
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .fov_degrees = 45.0;
            for mut window in world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .iter_mut(world)
            {
                window.set_cursor_position(None);
                window.resolution.set_physical_resolution(960, 640);
            }
            state.advance(Stage::NarrowCapture);
        }
        Stage::NarrowCapture if state.elapsed(0.7) => {
            require_no_outline(world)?;
            capture(world, "building-normal-block-no-outline-fov45-small")?;
            state.advance(Stage::NarrowRestore);
        }
        Stage::NarrowRestore if state.elapsed(0.4) => {
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .fov_degrees = state.fov;
            for mut window in world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .iter_mut(world)
            {
                window.set_cursor_position(None);
                window
                    .resolution
                    .set_physical_resolution(state.resolution.0, state.resolution.1);
            }
            state.advance(Stage::Place);
        }
        Stage::Place if state.elapsed(0.25) => {
            input.place = true;
            state.placement_confirmed = None;
            state.placement_visible = false;
            state.advance(Stage::Placed);
        }
        Stage::Placed => {
            let material = world.resource::<ChunkStore>().block_at(state.target);
            let count = world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(state.material));
            if material == Some(state.material) && count == state.count {
                let confirmed = *state.placement_confirmed.get_or_insert_with(Instant::now);
                let owner = terra_voxel::global_to_section(state.target).0;
                let visible = world
                    .resource::<terra_render::RenderedSections>()
                    .sections
                    .get(&owner)
                    .is_some_and(|section| {
                        section.mesh_epoch
                            == world
                                .resource::<terra_world::edits::EditJournal>()
                                .mesh_epoch(owner)
                            && world.get::<Visibility>(section.entity) != Some(&Visibility::Hidden)
                            && !section.meshes.is_empty()
                    });
                if !visible && state.elapsed(5.0) {
                    return Err("Confirmed placement has no visible current terrain mesh".into());
                }
                if visible && !state.placement_visible {
                    tracing::info!(
                        request_to_visible_ms = state.entered.elapsed().as_secs_f64() * 1000.0,
                        collision_to_visible_ms = confirmed.elapsed().as_secs_f64() * 1000.0,
                        ?owner,
                        material = state.material,
                        "Native placement latency: authoritative block and current visible mesh"
                    );
                    state.placement_visible = true;
                }
                if visible && state.elapsed(0.8) {
                    let mut debug = world.resource_mut::<terra_core::DebugSettings>();
                    debug.block_placement = false;
                    debug.enabled = false;
                    tracing::info!(
                        ?material,
                        count,
                        creative = state.creative,
                        "Native building: real break, inventory and placement verified"
                    );
                    state.advance(Stage::PlacedCapture);
                }
            } else if state.elapsed(3.0) {
                return Err(format!(
                    "Placement/consumption failed: {material:?}, count {count}"
                ));
            }
        }
        Stage::PlacedCapture if state.elapsed(0.3) => {
            capture(world, "building-restored")?;
            state.advance(Stage::Finish);
        }
        Stage::Finish if state.elapsed(0.7) => {
            if state.outline_only {
                tracing::info!(
                    material = state.material,
                    "Native oriented placement outline passed: actual resolver, orange unfilled model edges, bounded retained mesh and visible pulse"
                );
                return Ok(true);
            }
            if terra_core::beds::is_bed(state.material) {
                tracing::info!(
                    "Native bed flow passed: paired placement, one Survival dye, replicated lying pose, and next-day 08:00 wake"
                );
                return Ok(true);
            }
            if state.furniture.is_some() {
                if state.peak_particles == 0 {
                    return Err("Furniture mining never emitted its surface chips".into());
                }
                tracing::info!(
                    material = state.material,
                    particles = state.peak_particles,
                    "Native furniture surface mining passed: real placement, authoritative survival progress, four model geometry stages, bounded surface chips and original restored on cancel"
                );
                return Ok(true);
            }
            if state.peak_particles == 0 {
                return Err("Confirmed removal never emitted its bounded break particles".into());
            }
            tracing::info!(
                particles = state.peak_particles,
                drops = state.peak_drops,
                "Native building render effects observed"
            );
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

fn furniture_site(
    chunks: &ChunkStore,
    journal: &terra_world::edits::EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    body: Vec3,
    material: u16,
) -> Option<([i64; 3], Vec3)> {
    let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let center = body.floor().as_i64vec3();
    for z in center.z - 5..=center.z - 1 {
        for x in center.x - 4..=center.x + 4 {
            for y in (center.y - 3..=center.y).rev() {
                let ground = [x, y, z];
                let position = [x, y + 1, z];
                if chunks.block_at(position) != Some(0)
                    || !chunks
                        .block_at(ground)
                        .and_then(terra_voxel::MaterialId::from_raw)
                        .is_some_and(terra_voxel::MaterialId::is_natural_ground)
                {
                    continue;
                }
                let offsets = if terra_core::construction::is_slab(material) {
                    [0.5, 0.15, 0.85]
                } else {
                    [0.15, 0.5, 0.85]
                };
                for dx in offsets {
                    for dz in offsets {
                        let aim = Vec3::new(x as f32 + dx, y as f32 + 0.99, z as f32 + dz);
                        if aim.distance(eye) > 5.5 {
                            continue;
                        }
                        let direction = (aim - eye).normalize_or_zero();
                        let yaw = (-direction.x).atan2(-direction.z);
                        let (block, _) = terra_world::targeting::pick_world(
                            chunks,
                            eye.to_array().map(f64::from),
                            direction.to_array().map(f64::from),
                        );
                        let Some(hit) = block
                            .filter(|hit| hit.position == ground && hit.adjacent() == position)
                        else {
                            continue;
                        };
                        let Ok(plan) = terra_world::placement::propose(
                            chunks,
                            journal,
                            hit,
                            ItemKind::Block(material),
                            (
                                eye.to_array().map(f64::from),
                                direction.to_array().map(f64::from),
                            ),
                            [yaw, direction.y.asin()],
                        ) else {
                            continue;
                        };
                        if terra_core::construction::is_slab(material)
                            && !plan.states.iter().any(|(cell, state)| {
                                *cell == position
                                    && state.is_some_and(|state| (5..=8).contains(&state.facing))
                            })
                        {
                            continue;
                        }
                        if terra_world::placement::environment(&plan, chunks, journal, true).is_ok()
                            && clear_model_view(chunks, journal, properties, &plan, eye)
                        {
                            return Some((position, aim));
                        }
                    }
                }
            }
        }
    }
    None
}

fn clear_model_view(
    chunks: &ChunkStore,
    journal: &terra_world::edits::EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    plan: &terra_world::placement::Layout,
    eye: Vec3,
) -> bool {
    let bounds = terra_world::placement::boxes(plan, chunks, journal, properties);
    if bounds.is_empty() {
        return false;
    }
    let minimum = std::array::from_fn::<_, 3, _>(|i| {
        bounds
            .iter()
            .map(|b| b.minimum[i])
            .fold(f64::INFINITY, f64::min)
    });
    let maximum = std::array::from_fn::<_, 3, _>(|i| {
        bounds
            .iter()
            .map(|b| b.maximum[i])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    // Ground-contact feet are not a visibility requirement; inspect body and roof.
    let support_top = plan
        .edits
        .iter()
        .filter(|edit| edit.material != 0)
        .map(|edit| edit.position[1] as f64)
        .fold(f64::INFINITY, f64::min);
    let body_bottom = (minimum[1] + 0.18)
        .max(support_top + 0.18)
        .min(maximum[1] - 0.08);
    (0..8).all(|corner| {
        let point = Vec3::from_array(
            std::array::from_fn(|i| {
                if corner & (1 << i) == 0 {
                    if i == 1 {
                        body_bottom
                    } else {
                        minimum[i] + 0.03
                    }
                } else {
                    maximum[i] - 0.03
                }
            })
            .map(|v| v as f32),
        );
        let delta = point - eye;
        terra_voxel::blocks::raycast_blocks(
            eye.to_array().map(f64::from),
            delta.to_array().map(f64::from),
            f64::from(delta.length() - 0.04),
            |p| {
                Some(u16::from(chunks.block_at(p).is_none_or(|m| {
                    terra_voxel::MaterialId::from_raw(m)
                        .is_some_and(terra_voxel::MaterialId::is_solid)
                })))
            },
        )
        .is_none()
    })
}

fn require_no_outline(world: &mut World) -> Result<(), String> {
    use bevy::camera::visibility::ViewVisibility;
    let status = world.resource::<BuildingStatus>();
    if status.preview_visible || status.preview_valid {
        return Err("A normal full block enabled the orientation-only placement outline".into());
    }
    let visible = world
        .query_filtered::<&ViewVisibility, With<terra_render::PlacementOutline>>()
        .iter(world)
        .any(|visibility| visibility.get());
    if visible {
        return Err("A normal full block retained a render-visible placement outline".into());
    }
    Ok(())
}

struct OutlineSample {
    alpha: f32,
    mesh: AssetId<Mesh>,
}

fn require_visible_outline(
    world: &mut World,
    position: [i64; 3],
    material_id: u16,
) -> Result<OutlineSample, String> {
    use bevy::{camera::visibility::ViewVisibility, mesh::VertexAttributeValues};
    let status = world.resource::<BuildingStatus>();
    if !status.preview_visible || !status.preview_valid {
        return Err("The actual oriented placement outline is not enabled and valid".into());
    }
    let candidates: Vec<_> = world
        .query_filtered::<(
            &Mesh3d,
            &MeshMaterial3d<terra_render::PlacementOutlineMaterial>,
            &GlobalTransform,
            &ViewVisibility,
        ), With<terra_render::PlacementOutline>>()
        .iter(world)
        .map(|(mesh, material, transform, visible)| {
            (
                mesh.0.clone(),
                material.0.clone(),
                *transform,
                visible.get(),
            )
        })
        .collect();
    if candidates.len() != 1 {
        return Err(format!(
            "Expected one bounded placement outline entity, found {}",
            candidates.len()
        ));
    }
    let camera = world
        .query_filtered::<(&Camera, &GlobalTransform), With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|(camera, transform)| (camera.clone(), *transform))
        .ok_or("Missing actual placement render camera")?;
    let materials = world.resource::<Assets<terra_render::PlacementOutlineMaterial>>();
    let meshes = world.resource::<Assets<Mesh>>();
    for (mesh, material, transform, visible) in candidates {
        let material = materials
            .get(&material)
            .ok_or("The actual placement outline material is unavailable")?;
        let tint = Color::from(material.color).to_srgba();
        if material.alpha_mode() != AlphaMode::Blend
            || material.parameters.x != 2.0
            || (tint.red - 1.0).abs() > 0.001
            || (tint.green - 0.68).abs() > 0.001
            || (tint.blue - 0.2).abs() > 0.001
            || !(0.399..=0.951).contains(&tint.alpha)
        {
            return Err(format!(
                "Placement outline is not the orange pulsing unlit material: {tint:?}"
            ));
        }
        if !visible
            || !transform
                .translation()
                .abs_diff_eq(Vec3::from_array(position.map(|v| v as f32 + 0.5)), 0.01)
        {
            continue;
        }
        let mesh_id = mesh.id();
        let mesh = meshes
            .get(&mesh)
            .ok_or("The actual outline mesh asset is unavailable")?;
        if mesh.primitive_topology() != bevy::mesh::PrimitiveTopology::TriangleList {
            return Err("Placement preview is missing its screen-expanded edge strips".into());
        }
        if !mesh
            .asset_usage
            .contains(bevy::asset::RenderAssetUsages::MAIN_WORLD)
        {
            return Err("Placement outline did not retain its reusable CPU mesh".into());
        }
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return Err("Placement outline has no actual model positions".into());
        };
        let indices = mesh.indices();
        let endpoints = points.len() / 2;
        if endpoints < 24
            || points.len() % 4 != 0
            || indices.is_none_or(|indices| indices.len() != points.len() / 4 * 6)
            || indices.is_some_and(|indices| indices.iter().any(|i| i >= points.len()))
            || points
                .chunks_exact(4)
                .any(|quad| quad[0] != quad[1] || quad[2] != quad[3])
            || points
                .iter()
                .flatten()
                .any(|coordinate| !coordinate.is_finite())
        {
            return Err("Placement outline has invalid or incomplete line geometry".into());
        }
        let local_low = points.iter().fold(Vec3::splat(f32::INFINITY), |a, &p| {
            a.min(Vec3::from_array(p))
        });
        let local_high = points.iter().fold(Vec3::splat(f32::NEG_INFINITY), |a, &p| {
            a.max(Vec3::from_array(p))
        });
        if terra_core::construction::is_slab(material_id) {
            if (local_high - local_low).min_element() >= 0.8 {
                return Err("Half-slab outline incorrectly uses a full-block bounding cube".into());
            }
        } else if endpoints <= 24 {
            return Err("Placement outline uses a fallback box rather than the actual stairs/furniture profile".into());
        }
        let projected: Vec<_> = points
            .iter()
            .filter_map(|p| {
                camera
                    .0
                    .world_to_viewport(&camera.1, transform.transform_point(Vec3::from_array(*p)))
                    .ok()
            })
            .collect();
        if projected.len() != points.len() {
            return Err("Placement model is partly outside the actual render camera".into());
        }
        let low = projected
            .iter()
            .fold(Vec2::splat(f32::INFINITY), |a, &p| a.min(p));
        let high = projected
            .iter()
            .fold(Vec2::splat(f32::NEG_INFINITY), |a, &p| a.max(p));
        let viewport = camera
            .0
            .logical_viewport_rect()
            .ok_or("Placement camera has no actual viewport")?;
        if low.cmplt(viewport.min + Vec2::splat(20.0)).any()
            || high.cmpgt(viewport.max - Vec2::splat(20.0)).any()
        {
            return Err("The placement outline is clipped by the actual viewport".into());
        }
        if (high - low).min_element() < 20.0 {
            return Err("The actual placement outline is too small to inspect".into());
        }
        info!(
            vertices = points.len(),
            line_segments = endpoints / 2,
            width_pixels = material.parameters.x,
            alpha = tint.alpha,
            ?position,
            ?low,
            ?high,
            "Native orange placement outline is unfilled actual model geometry, render-visible and unobstructed"
        );
        return Ok(OutlineSample {
            alpha: tint.alpha,
            mesh: mesh_id,
        });
    }
    Err("No valid, render-visible authored placement outline was found".into())
}

fn interact_with_bed(world: &mut World) -> Result<(), String> {
    let hit = world
        .resource::<terra_player::TargetBlock>()
        .0
        .ok_or("Missing real bed target")?;
    let expected = world
        .resource::<ChunkStore>()
        .block_at(hit.position)
        .filter(|material| terra_core::beds::is_bed(*material))
        .ok_or("Bed interaction target is occluded")?;
    let facing = world
        .resource::<terra_world::edits::EditJournal>()
        .object_state(hit.position)
        .ok_or("Bed interaction metadata missing")?
        .facing;
    world.write_message(terra_network::client::ObjectInteractionRequest {
        position: hit.position,
        expected,
        facing,
        closing: false,
    });
    Ok(())
}

fn require_block(world: &World, state: &Fixture, material: u16) -> Result<(), String> {
    if world.resource::<ChunkStore>().block_at(state.target) == Some(material) {
        Ok(())
    } else {
        Err("Target block changed before confirmed completion".into())
    }
}

fn select_ground(chunks: &ChunkStore, body: Vec3) -> Option<([i64; 3], Vec3, u16)> {
    let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let center = body.floor().as_i64vec3();
    for y in center.y - 2..=center.y {
        for x in center.x - 2..=center.x + 2 {
            for z in center.z - 2..=center.z + 2 {
                let position = [x, y, z];
                let Some(material @ 1..=6) = chunks.block_at(position) else {
                    continue;
                };
                let midpoint = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                if midpoint.distance(body) > 1.98
                    || chunks.block_at([x, y + 1, z]) != Some(0)
                    || (midpoint.x - body.x).abs() < 0.7 && (midpoint.z - body.z).abs() < 0.7
                {
                    continue;
                }
                let steps = (midpoint.distance(body) / 0.1).ceil() as usize;
                if !(0..=steps).all(|step| {
                    let sample = body
                        .lerp(midpoint, step as f32 / steps.max(1) as f32)
                        .floor()
                        .as_i64vec3()
                        .to_array();
                    sample == position || chunks.block_at(sample) == Some(0)
                }) {
                    continue;
                }
                for dx in [0.25, 0.5, 0.75] {
                    for dz in [0.25, 0.5, 0.75] {
                        let point = Vec3::new(x as f32 + dx, y as f32 + 0.99, z as f32 + dz);
                        let direction = (point - eye).normalize();
                        let (target, _) = terra_world::targeting::pick_world(
                            chunks,
                            eye.to_array().map(f64::from),
                            direction.to_array().map(f64::from),
                        );
                        if target.is_none_or(|hit| hit.position != position) {
                            continue;
                        }
                        let replacement = terra_voxel::blocks::raycast_blocks(
                            eye.to_array().map(f64::from),
                            direction.to_array().map(f64::from),
                            9.0,
                            |p| {
                                if p == position {
                                    Some(0)
                                } else {
                                    chunks.block_at(p)
                                }
                            },
                        );
                        if replacement.is_some_and(|hit| hit.adjacent() == position) {
                            return Some((position, point, material));
                        }
                    }
                }
            }
        }
    }
    None
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-building"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            directory.join(format!("{name}.png")),
        ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_furniture_visibility_accepts_flat_ground_and_low_feet_but_rejects_an_occluding_log() {
        let section = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 0, z], 1).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let journal = terra_world::edits::EditJournal::default();
        let mut properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        let body = Vec3::new(8.5, 1.90, 8.5);
        let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
        let plan =
            terra_world::placement::layout(&chunks, &journal, [8, 1, 5], 0, 29, [0.0; 2], None)
                .unwrap();
        assert!(clear_model_view(&chunks, &journal, &properties, &plan, eye));
        assert!(furniture_site(&chunks, &journal, &properties, body, 29).is_some());
        let (slab_position, slab_aim) = furniture_site(&chunks, &journal, &properties, body, 82)
            .expect("flat ground offers a genuinely visible vertical floor slab site");
        let direction = (slab_aim - eye).normalize();
        let hit = terra_world::targeting::pick_world(
            &chunks,
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
        )
        .0
        .unwrap();
        assert_eq!(hit.normal, [0, 1, 0]);
        let slab = terra_world::placement::propose(
            &chunks,
            &journal,
            hit,
            ItemKind::Block(82),
            (
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            ),
            [(-direction.x).atan2(-direction.z), direction.y.asin()],
        )
        .unwrap();
        assert!(
            slab.states
                .iter()
                .any(|(cell, state)| *cell == slab_position
                    && state.is_some_and(|state| (5..=8).contains(&state.facing)))
        );
        properties.1.insert(
            29,
            std::sync::Arc::from(vec![terra_core::block_shapes::ShapeBox {
                minimum: [0.075, -0.01, 0.075],
                maximum: [0.925, 0.842, 0.97],
                hinged: false,
            }]),
        );
        assert!(
            clear_model_view(&chunks, &journal, &properties, &plan, eye),
            "low trim at the floor cannot invalidate a clearly visible body and roof"
        );
        for y in 1..=2 {
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block([8, y, 7], 6)
                .unwrap();
        }
        assert!(
            !clear_model_view(&chunks, &journal, &properties, &plan, eye),
            "a real tree in front of the body still rejects the shot"
        );
    }
}
