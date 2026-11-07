//! Real integrated Mixer, color-picker, crank and conserved pigment output.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    GameAction, GameConfig, GameMode, InputBinding, ItemKind, ItemStack, PlayerProfile, mixer,
};
use terra_network::{
    client::{ClientSession, ContainerRequest},
    protocol::inventory::ContainerCommand,
};

const ANCHOR: [i64; 3] = [0, 17, 0];
const CRANK: [i64; 3] = [1, 17, 0];
const TARGET: [u8; 3] = [71, 93, 184];

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
    let path = std::env::temp_dir().join(format!("terra-mixer-{}", std::process::id()));
    if path.exists() {
        return Err("Mixer fixture requires a fresh temporary directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
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
    for x in -3..=5 {
        for z in -3..=5 {
            journal.record([x, 16, z], 3).map_err(|e| e.to_string())?;
            for y in 17..=20 {
                journal.record([x, y, z], 0).map_err(|e| e.to_string())?;
            }
        }
    }
    journal
        .record(ANCHOR, mixer::MATERIAL)
        .map_err(|e| e.to_string())?;
    journal.set_object_state(ANCHOR, Some(Default::default()))?;
    journal
        .record(CRANK, terra_core::crusher::HAND_CRANK_MATERIAL)
        .map_err(|e| e.to_string())?;
    journal.set_object_state(
        CRANK,
        Some(terra_world::edits::BlockObjectState {
            peer: Some(ANCHOR),
            ..default()
        }),
    )?;
    let mut inventory = terra_core::Inventory::default();
    inventory.slots.resize(mixer::SLOTS, ItemStack::EMPTY);
    for (slot, stack) in mixer::recipe(TARGET).inputs.into_iter().enumerate() {
        inventory.slots[slot] = stack;
    }
    journal.commit_mixer(ANCHOR, mixer::MixerState::default(), inventory)?;
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    captures: u8,
    requested: u8,
    bad_capture: bool,
    pause: Option<f32>,
    picker: Option<Entity>,
    preview_count: usize,
    recipe_requested: bool,
    front_requested: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-mixer") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        captures: 0,
        requested: 0,
        bad_capture: false,
        pause: None,
        picker: None,
        preview_count: 0,
        recipe_requested: false,
        front_requested: false,
    })
    .insert_resource(terra_player::ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, input.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    if let Some(mut menu) = world.get_resource_mut::<terra_ui::menu::MenuState>() {
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}
fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    info!(stage, "Mixer native stage");
}
fn place(world: &mut World, held: bool) {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || GameAction::Place.default_binding(),
            |c| c.binding(GameAction::Place),
        );
    match binding {
        InputBinding::Mouse(button) => {
            let mut input = world.resource_mut::<ButtonInput<MouseButton>>();
            if held {
                input.press(button);
            } else {
                input.release(button);
            }
        }
        InputBinding::Key(key) => {
            let mut input = world.resource_mut::<ButtonInput<KeyCode>>();
            if held {
                input.press(key);
            } else {
                input.release(key);
            }
        }
        InputBinding::Chord { modifier, key } => {
            let mut input = world.resource_mut::<ButtonInput<KeyCode>>();
            for key in [modifier, key] {
                if held {
                    input.press(key);
                } else {
                    input.release(key);
                }
            }
        }
    }
}
fn input(world: &mut World) {
    let stage = world.resource::<Inspection>().stage;
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    let gameplay = !world.resource::<terra_ui::InventoryView>().open
        && !world.resource::<terra_player::PlayerInputBlock>().0;
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = gameplay;
    place(world, gameplay && matches!(stage, 5 | 7));
    let point = if stage >= 4 {
        Vec3::new(1.60, 17.685, 0.5)
    } else {
        Vec3::new(0.5, 17.55, 0.72)
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(point, Vec3::Y);
    }
}
#[derive(Clone, Copy)]
enum Viewpoint {
    ThreeQuarter,
    Front,
    Crank,
}
impl Viewpoint {
    fn position(self) -> Vec3 {
        let [x, z] = match self {
            Self::ThreeQuarter => [2.8, 3.2],
            Self::Front => [0.5, 3.2],
            Self::Crank => [3.2, 0.5],
        };
        Vec3::new(x, 17.0 + terra_physics::character::PLAYER_HALF_HEIGHT, z)
    }
}
fn teleport(world: &mut World, viewpoint: Viewpoint) {
    let p = viewpoint.position();
    world
        .resource_mut::<terra_network::client::ClientChatCommands>()
        .0
        .push_back(terra_network::client::ChatCommand::Send(format!(
            "/tp {} {} {}",
            p.x, p.y, p.z
        )));
}
fn settled(session: &ClientSession, viewpoint: Viewpoint) -> bool {
    let point = viewpoint.position();
    !session.teleporting
        && session.confirmed_state().is_some_and(|s| {
            s.grounded
                && (s.position[0] - point.x).abs() < 0.35
                && (s.position[2] - point.z).abs() < 0.35
        })
}
fn capture(
    world: &mut World,
    state: &mut Inspection,
    name: &str,
    count: u8,
) -> Result<bool, String> {
    if state.requested < count {
        let folder = std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .ok_or("Set TERRA_CAPTURE_DIR for Mixer captures")?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(format!("{name}.png"))))
            .observe(
                |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                    state.bad_capture |= event.image.data.as_ref().is_none_or(|data| {
                        data.chunks_exact(4)
                            .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
                            .count()
                            < 1000
                    });
                    state.captures += 1;
                },
            );
        state.requested = count;
    }
    Ok(state.captures >= count)
}
fn picker(world: &mut World) -> Result<Entity, String> {
    world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, id, node)| {
            (id.0 == "mixer-color" && node.size().min_element() > 1.0).then_some(entity)
        })
        .ok_or("Real Mixer ColorPicker layout missing".into())
}
fn choose(world: &mut World, entity: Entity) -> Result<(), String> {
    world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|a| a.attributes.iter().find(|a| a.name == "onchange"))
        .ok_or("Mixer picker onchange handler missing")?;
    let mut value = world
        .get::<tilt_ui::ColorPickerState>(entity)
        .ok_or("Mixer ColorPicker state missing")?
        .value;
    [value.red, value.green, value.blue, value.alpha] = [
        f32::from(TARGET[0]) / 255.0,
        f32::from(TARGET[1]) / 255.0,
        f32::from(TARGET[2]) / 255.0,
        1.0,
    ];
    tilt_ui::set_color_value(world, entity, value);
    world.write_message(tilt_ui::ColorPickerChanged { entity, value });
    Ok(())
}
fn output_icon(world: &mut World, state: &Inspection) -> Result<(), String> {
    let previews = world.resource::<terra_ui::ItemPreviewImages>();
    if previews.0.len() != state.preview_count
        || previews
            .0
            .contains_key(&ItemKind::Dye(terra_core::beds::DyeColor::Rgb(TARGET)))
    {
        return Err("Mixer allocated a per-RGB preview registry entry".into());
    }
    let white = previews
        .0
        .get(&ItemKind::Dye(terra_core::beds::DyeColor::White))
        .ok_or("Neutral dye preview missing")?
        .clone();
    let image = world
        .query::<(&tilt_ui::ImageMetadata, &ImageNode)>()
        .iter(world)
        .find_map(|(meta, image)| {
            (meta.alt.as_deref() == Some("Item preview teramic::mixed_dye#475DB8")).then_some(image)
        })
        .ok_or("Real RGB output image missing")?;
    let tint = image.color.to_srgba();
    if image.image != white
        || [tint.red, tint.green, tint.blue]
            .into_iter()
            .zip(TARGET)
            .any(|(a, b)| (a - f32::from(b) / 255.0).abs() > 0.001)
    {
        return Err("RGB output did not tint the shared neutral dye image".into());
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
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut state) {
            error!(%error,stage=state.stage,"Mixer native smoke failed");
            state.stage = 255;
            place(world, false);
            world.write_message(AppExit::error());
        }
    });
}
fn drive_diagnostics(world: &mut World) -> String {
    let session = world.resource::<ClientSession>();
    let journal = world.resource::<terra_world::edits::EditJournal>();
    let state = format!(
        "crank_input={:?}, target={:?}, blocked={}, captured={}, inventory_open={}, view_mixer={}, last_edit={:?}, last_inventory={:?}, look={:?}, confirmed={:?}, kinetics={:?}, crank_state={:?}, crank_anchor={:?}, mixer_state={:?}, mixer_block={:?}, crank_block={:?}",
        world.resource::<terra_network::client::CrankInput>().0,
        world.resource::<terra_player::TargetWorld>().0,
        world.resource::<terra_player::PlayerInputBlock>().0,
        world.resource::<terra_player::CameraInputState>().captured,
        world.resource::<terra_ui::InventoryView>().open,
        world.resource::<terra_ui::InventoryView>().mixer,
        session.last_edit,
        session.last_inventory,
        session.look,
        session.confirmed_state(),
        session.kinetics,
        journal.object_state(CRANK),
        journal.crank_anchor(CRANK),
        journal.mixer_at(ANCHOR),
        world
            .resource::<terra_world::streaming::ChunkStore>()
            .block_at(ANCHOR),
        world
            .resource::<terra_world::streaming::ChunkStore>()
            .block_at(CRANK),
    );
    let cameras: Vec<_> = world
        .query_filtered::<(&Transform, &terra_player::CameraLook), With<terra_player::TerraCamera>>(
        )
        .iter(world)
        .map(|(pose, look)| (pose.translation, pose.rotation, look.rotation()))
        .collect();
    let open_pickers = world
        .query::<&tilt_ui::ColorPickerState>()
        .iter(world)
        .filter(|picker| picker.open)
        .count();
    let bodies: Vec<_> = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .map(|pose| {
            (
                pose.translation,
                pose.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET,
            )
        })
        .collect();
    format!(
        "{state}, body_and_eye={bodies:?}, cameras={cameras:?}, open_pickers={open_pickers}, mouse={:?}, keys={:?}",
        world
            .resource::<ButtonInput<MouseButton>>()
            .get_pressed()
            .collect::<Vec<_>>(),
        world
            .resource::<ButtonInput<KeyCode>>()
            .get_pressed()
            .collect::<Vec<_>>()
    )
}
fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.start.elapsed() > Duration::from_secs(180) || state.bad_capture {
        return Err(format!(
            "Mixer timeout/blank capture at stage{}",
            state.stage
        ));
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready {
        return Ok(());
    }
    let elapsed = state.since.elapsed().as_secs_f32();
    match state.stage {
        0 if world.resource::<terra_render::RenderStatus>().ready
            && world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                == 0
            && world.resource::<terra_ui::ItemPreviewImages>().0.len()
                == world
                    .resource::<terra_render::RenderItemPreviews>()
                    .images
                    .len() =>
        {
            state.preview_count = world.resource::<terra_ui::ItemPreviewImages>().0.len();
            teleport(world, Viewpoint::ThreeQuarter);
            advance(state, 1);
        }
        1 if elapsed > 0.5 => {
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(ANCHOR)
                != Some(mixer::MATERIAL)
            {
                return Ok(());
            }
            if !state.front_requested {
                if !settled(session, Viewpoint::ThreeQuarter)
                    || !capture(world, state, "mixer-world", 1)?
                {
                    return Ok(());
                }
                teleport(world, Viewpoint::Front);
                state.front_requested = true;
                state.since = Instant::now();
                return Ok(());
            }
            if !settled(session, Viewpoint::Front) {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::OpenChest(ANCHOR));
            advance(state, 2);
        }
        2 if world.resource::<terra_ui::InventoryView>().mixer && elapsed > 0.5 => {
            let entity = picker(world)?;
            if !world
                .get::<tilt_ui::ColorPickerState>(entity)
                .is_some_and(|p| p.open)
                && !tilt_ui::set_color_picker_open(world, entity, true)
            {
                return Err("Mixer picker failed to open".into());
            }
            state.picker = Some(entity);
            choose(world, entity)?;
            advance(state, 3);
        }
        3 if session
            .container
            .as_ref()
            .and_then(|c| c.mixer.as_ref())
            .is_some_and(|m| m.target == TARGET)
            && elapsed > 1.0 =>
        {
            let snapshot = session.container.as_ref().unwrap();
            let machine = snapshot.mixer.as_ref().unwrap();
            if machine.progress != 0.0 || snapshot.halves[0].inventory.slot(mixer::OUTPUT).is_some()
            {
                return Err("Unpowered Mixer made pigment/progress".into());
            }
            for ingredient in mixer::recipe(TARGET).inputs {
                if snapshot.halves[0].inventory.count(ingredient.item) != ingredient.count {
                    return Err("Unpowered Mixer consumed its pigment inputs".into());
                }
            }
            if !capture(world, state, "mixer-color-picker", 2)? {
                return Ok(());
            }
            if let Some(picker) = state.picker.take() {
                tilt_ui::set_color_picker_open(world, picker, false);
            }
            world.write_message(terra_ui::InventoryAction::Close);
            teleport(world, Viewpoint::Crank);
            advance(state, 4);
        }
        4 if !world.resource::<terra_ui::InventoryView>().open
            && settled(session, Viewpoint::Crank)
            && elapsed > 0.5 =>
        {
            world.write_message(terra_player::CaptureRequest(true));
            advance(state, 5);
        }
        5 | 7 if elapsed > 0.7 => {
            let turning = session
                .kinetics
                .iter()
                .any(|v| v.position == ANCHOR && !v.overloaded && v.rpm.abs() > 0.01);
            if !turning || world.resource::<terra_network::client::CrankInput>().0 != Some(CRANK) {
                if elapsed > 3.0 {
                    return Err(format!(
                        "Real crank input did not power the acknowledged Mixer: {}",
                        drive_diagnostics(world)
                    ));
                }
                return Ok(());
            }
            if elapsed > if state.stage == 5 { 1.5 } else { 10.0 } {
                world.write_message(terra_ui::InventoryAction::OpenChest(ANCHOR));
                advance(state, if state.stage == 5 { 6 } else { 8 });
            }
        }
        6 if world.resource::<terra_ui::InventoryView>().mixer && elapsed > 0.5 => {
            let machine = session
                .container
                .as_ref()
                .and_then(|c| c.mixer.as_ref())
                .ok_or("Paused Mixer snapshot missing")?;
            if !(0.0..1.0).contains(&machine.progress) || machine.progress == 0.0 {
                return Err("Mixer did not pause with real partial progress".into());
            }
            if let Some(progress) = state.pause {
                if (machine.progress - progress).abs() > 0.00001 {
                    return Err("Mixer progressed without held crank input".into());
                }
            } else {
                state.pause = Some(machine.progress);
                state.since = Instant::now();
                return Ok(());
            }
            if elapsed > 1.0 {
                world.write_message(terra_ui::InventoryAction::Close);
                world.write_message(terra_player::CaptureRequest(true));
                advance(state, 7);
            }
        }
        8 if world.resource::<terra_ui::InventoryView>().mixer && elapsed > 0.7 => {
            let snapshot = session
                .container
                .as_ref()
                .ok_or("Final Mixer snapshot missing")?;
            let inventory = &snapshot.halves[0].inventory;
            if inventory.slot(mixer::OUTPUT) != Some(mixer::recipe(TARGET).output) {
                if elapsed < 5.0 {
                    return Ok(());
                }
                return Err(
                    "Powered Mixer did not produce the requested byte-exact RGB pigment".into(),
                );
            }
            if (0..mixer::INPUT_SLOTS).any(|slot| inventory.slot(slot).is_some()) {
                return Err("Mixer output duplicated unconsumed ingredients".into());
            }
            let view = world.resource::<terra_ui::InventoryView>();
            if !view.mixer_input.iter().all(|slot| slot.empty)
                || view
                    .mixer_input
                    .iter()
                    .filter(|slot| !slot.ghost_preview.is_empty())
                    .count()
                    != mixer::recipe(TARGET).inputs.len()
            {
                return Err(
                    "Consumed Mixer inputs did not return to truthful recipe ghosts".into(),
                );
            }
            output_icon(world, state)?;
            if !state.recipe_requested {
                world.write_message(terra_ui::InventoryAction::Recipe(
                    "item-mixed_dye#475DB8".into(),
                ));
                state.recipe_requested = true;
                return Ok(());
            }
            let recipe_view = world.resource::<terra_ui::CraftingView>();
            if !recipe_view.recipe_open || !recipe_view.mixing {
                return Ok(());
            }
            if recipe_view.recipe_station != "Mixer"
                || recipe_view.recipe_heading != "Mixing Recipe"
                || recipe_view.recipe_grid.len() != mixer::recipe(TARGET).inputs.len()
                || recipe_view.recipe_output.first().is_none_or(|output| {
                    output.count != mixer::recipe(TARGET).output.count.to_string()
                        || output.preview_key != "Item preview teramic::mixed_dye#475DB8"
                })
            {
                return Err("Native TMI did not show the real RGB mixing recipe".into());
            }
            if !capture(world, state, "mixer-rgb-output", 3)? {
                return Ok(());
            }
            world.write_message(terra_ui::CraftingAction::CloseRecipe);
            let snapshot = world
                .resource::<ClientSession>()
                .container
                .as_ref()
                .unwrap();
            world.write_message(ContainerRequest(ContainerCommand {
                view: snapshot.view,
                inventory_revision: world.resource::<ClientSession>().inventory.revision,
                reserved: [0; terra_core::INVENTORY_SLOTS],
                anchor: ANCHOR,
                revisions: snapshot
                    .halves
                    .iter()
                    .map(|h| h.inventory.revision)
                    .collect(),
                action: terra_core::containers::ContainerGesture::QuickMove {
                    slot: (terra_core::INVENTORY_SLOTS + mixer::OUTPUT) as u8,
                },
            }));
            advance(state, 9);
        }
        9 if session.inventory.count(mixer::recipe(TARGET).output.item)
            == mixer::recipe(TARGET).output.count =>
        {
            if session
                .container
                .as_ref()
                .and_then(|c| c.halves[0].inventory.slot(mixer::OUTPUT))
                .is_some()
            {
                return Err("RGB result remained in both inventories".into());
            }
            info!(
                captures = state.captures,
                "Mixer native smoke passed: real picker/authority, paused progress, crank RPM, RGB output and shared icon"
            );
            state.stage = 255;
            place(world, false);
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrated_setup_without_frontend_menu_still_configures_and_captures_gameplay() {
        let mut app = App::new();
        app.init_resource::<terra_core::DebugSettings>()
            .init_resource::<terra_core::GameplayConfig>()
            .add_message::<terra_player::CaptureRequest>();
        setup(app.world_mut());
        assert!(!app.world().contains_resource::<terra_ui::menu::MenuState>());
        assert!(!app.world().resource::<terra_core::DebugSettings>().enabled);
        assert!(
            !app.world()
                .resource::<terra_core::GameplayConfig>()
                .minimap
                .enabled
        );
        assert_eq!(
            app.world()
                .resource::<Messages<terra_player::CaptureRequest>>()
                .len(),
            1
        );
    }
}
