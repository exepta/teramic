//! Short native check of saved RGB fabrics, real filter widgets and authoritative recoloring.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, GameMode, ItemKind, PlayerProfile};
use terra_network::{
    client::{ClientSession, InventoryRequest},
    protocol::inventory::{InventoryAction, InventoryCommand},
};

const FILTER: [i64; 3] = [0, 17, 3];
const FILTERED_TUNNEL: [i64; 3] = [6, 18, 3];
const DISTRIBUTOR: [i64; 3] = [3, 18, 5];
const RGB: [u8; 3] = [71, 93, 184];
const FABRICS: [([i64; 3], u16, [u8; 3]); 6] = [
    ([-3, 17, 0], 103, RGB),
    ([-1, 17, 0], 106, [218, 91, 54]),
    ([1, 17, 0], 71, [161, 66, 193]),
    ([3, 17, 0], 148, [47, 183, 123]),
    ([-5, 17, 0], 104, [214, 89, 161]),
    ([-5, 17, 3], 105, [57, 168, 196]),
];

fn wool() -> ItemKind {
    terra_core::tints::recolored(ItemKind::Block(103), terra_core::beds::DyeColor::Rgb(RGB))
        .unwrap()
}
fn dye() -> ItemKind {
    ItemKind::Dye(terra_core::beds::DyeColor::Rgb(RGB))
}

fn distributor_config() -> terra_core::logistics::DistributorConfig {
    use terra_core::logistics::{DistributorConfig, FilterConfig};
    DistributorConfig {
        filters: [
            FilterConfig {
                item: Some(wool()),
                allowed: true,
            },
            FilterConfig {
                item: Some(ItemKind::Block(3)),
                allowed: false,
            },
            FilterConfig::default(),
        ],
    }
}

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Creative;
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
    let path = std::env::temp_dir().join(format!("terra-rgb-logistics-{}", std::process::id()));
    if path.exists() {
        return Err("RGB fixture requires a fresh temporary directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    config.server.player_permissions.insert(
        profile.uuid,
        terra_core::PlayerPermissions {
            game_mode: false,
            build: true,
            flight: true,
            environment: true,
        },
    );
    let mut journal = terra_world::edits::EditJournal::default();
    for x in -5..=8 {
        for z in -3..=9 {
            journal
                .record([x, 16, z], 3)
                .map_err(|error| error.to_string())?;
            for y in 17..=21 {
                journal
                    .record([x, y, z], 0)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    for (position, material, rgb) in FABRICS {
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        if material == 71 {
            let peer = [position[0], position[1], position[2] + 1];
            journal
                .record(peer, material)
                .map_err(|error| error.to_string())?;
            journal.set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(peer),
                    ..default()
                }),
            )?;
            journal.set_object_state(
                peer,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(position),
                    secondary: true,
                    ..default()
                }),
            )?;
            journal.set_tint(peer, Some(rgb))?;
        } else if material == 148 {
            journal.set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    wood: terra_core::WoodSpecies::Cherry,
                    ..default()
                }),
            )?;
        } else if matches!(material, 104 | 105) {
            journal.set_object_state(position, Some(Default::default()))?;
        }
        journal.set_tint(position, Some(rgb))?;
    }
    for (position, material) in [
        (FILTER, 157),
        ([3, 18, 3], 158),
        ([3, 17, 3], 148),
        ([2, 17, 3], 148),
        ([4, 17, 3], 148),
        ([3, 17, 2], 148),
        ([3, 17, 4], 148),
        (DISTRIBUTOR, 160),
        ([3, 17, 5], 148),
        ([2, 17, 5], 148),
        ([4, 17, 5], 148),
        ([3, 17, 6], 148),
    ] {
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(position, Some(Default::default()))?;
    }
    // A real side feed exercises both open shaft bearings and the merger tongue.
    let start = [6, 17, 3];
    let controller = [5, 17, 3];
    let end = [4, 17, 3];
    for (position, peer) in [(start, controller), (controller, end), (end, start)] {
        journal
            .record(position, 144)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(
            position,
            Some(terra_world::edits::BlockObjectState {
                facing: 1,
                peer: Some(peer),
                wood: if position == controller {
                    terra_core::WoodSpecies::Spruce
                } else {
                    terra_core::WoodSpecies::Birch
                },
                ..default()
            }),
        )?;
    }
    journal.set_belt_owner(
        controller,
        Some(terra_world::edits::BeltOwner {
            start,
            end,
            item: ItemKind::TintedBlock {
                material: 148,
                wood: terra_core::WoodSpecies::Cherry,
                color: RGB,
            },
        }),
    )?;
    // The outer shaft also carries a real inventory-mounted terminal tunnel.
    // Its ribbon must continue beneath the open housing to the inventory side.
    for (position, material, peer) in [
        ([7, 17, 3], 3, None),
        ([7, 18, 3], 29, None),
        (FILTERED_TUNNEL, 159, Some([7, 18, 3])),
    ] {
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        if material != 3 {
            journal.set_object_state(
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing: 1,
                    peer,
                    ..default()
                }),
            )?;
        }
    }
    journal.set_logistics_filter(FILTER, terra_core::logistics::FilterConfig::default())?;
    let tunnel_filter = terra_core::logistics::FilterConfig {
        item: Some(ItemKind::Block(3)),
        allowed: false,
    };
    journal.set_logistics_filter(FILTERED_TUNNEL, tunnel_filter)?;
    journal.set_distributor(DISTRIBUTOR, distributor_config())?;
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let loaded = terra_storage::load_edits(&path, config.world.id)?;
    if loaded.logistics_filter_at(FILTERED_TUNNEL) != Some(&tunnel_filter)
        || loaded.distributor_at(DISTRIBUTOR) != Some(&distributor_config())
    {
        return Err("Native logistics filter save/reload lost exact output or RGB metadata".into());
    }
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
    original: Option<terra_core::Inventory>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-rgb-logistics") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        captures: 0,
        requested: 0,
        bad_capture: false,
        original: None,
    })
    .insert_resource(terra_player::ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, aim.after(bevy::input::InputSystems))
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
fn aim(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    let captured = !world.resource::<terra_ui::InventoryView>().open
        && !world.resource::<terra_player::PlayerInputBlock>().0;
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = captured;
    let target = if world.resource::<Inspection>().stage >= 13 {
        Vec3::new(6.5, 17.65, 3.5)
    } else {
        Vec3::new(0.5, 17.35, 1.2)
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(target, Vec3::Y);
    }
}
fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    info!(stage, "RGB logistics native stage");
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
            .ok_or("Set TERRA_CAPTURE_DIR for RGB captures")?;
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(format!("{name}.png"))))
            .observe(
                |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                    state.bad_capture |= event.image.data.as_ref().is_none_or(|data| {
                        data.chunks_exact(4)
                            .filter(|pixel| pixel[..3].iter().any(|value| *value > 20))
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
fn element(world: &mut World, id: &str) -> Result<Entity, String> {
    world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, actual, node)| {
            (actual.0 == id && node.size().min_element() > 1.0).then_some(entity)
        })
        .ok_or_else(|| format!("Native element {id} is not laid out"))
}
fn event(
    world: &mut World,
    target: Entity,
    attribute: &str,
    value: Option<String>,
) -> Result<(), String> {
    let handler = world
        .get::<tilt_ui::StaticAttributes>(target)
        .and_then(|attrs| attrs.attributes.iter().find(|attr| attr.name == attribute))
        .map(|attr| attr.value.clone())
        .ok_or_else(|| format!("Missing {attribute} native handler"))?;
    world.write_message(tilt_ui::HtmlEvent {
        target,
        kind: if attribute == "onchange" {
            "change"
        } else {
            "mousedown"
        },
        value,
        submitter: None,
        data: [("button".into(), "Primary".into())].into(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn descendants(world: &World, root: Entity, output: &mut Vec<Entity>) {
    if let Some(children) = world.get::<Children>(root) {
        for child in children {
            output.push(*child);
            descendants(world, *child, output);
        }
    }
}
fn press_slot(world: &mut World, root: &str, index: usize) -> Result<(), String> {
    let root = element(world, root)?;
    let mut nodes = Vec::new();
    descendants(world, root, &mut nodes);
    let target = nodes
        .into_iter()
        .filter(|entity| {
            world
                .get::<tilt_ui::ElementId>(*entity)
                .is_some_and(|id| id.0 == "inventory-cell")
        })
        .nth(index)
        .ok_or("Missing native inventory cell")?;
    event(world, target, "onmousedown", None)
}
fn request(world: &mut World, action: InventoryAction) {
    let revision = world.resource::<ClientSession>().inventory.revision;
    world.write_message(InventoryRequest(InventoryCommand { revision, action }));
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
            error!(%error,stage=state.stage,"RGB logistics native smoke failed");
            state.stage = 255;
            world.write_message(AppExit::error());
        }
    });
}
fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.start.elapsed() > Duration::from_secs(180) || state.bad_capture {
        return Err(format!(
            "RGB fixture timeout/blank capture at {}",
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
                == 0 =>
        {
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 0.5 17.91 7".into(),
                ));
            advance(state, 1);
        }
        1 if elapsed > 2.0
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|pose| pose.grounded && (pose.position[2] - 7.0).abs() < 0.2) =>
        {
            for (position, material, rgb) in FABRICS {
                if world
                    .resource::<terra_world::edits::EditJournal>()
                    .tint_at(position)
                    != Some(rgb)
                    || world
                        .resource::<terra_world::streaming::ChunkStore>()
                        .block_at(position)
                        != Some(material)
                {
                    return Ok(());
                }
            }
            if !capture(world, state, "rgb-world-filter-merger", 1)? {
                return Ok(());
            }
            let item = terra_world::placement::picked_item(
                world.resource::<terra_world::streaming::ChunkStore>(),
                world.resource::<terra_world::edits::EditJournal>(),
                FABRICS[0].0,
            )
            .ok_or("RGB wool is not loaded")?;
            if item != wool() {
                return Err("Native creative pick discarded RGB metadata".into());
            }
            world.write_message(terra_ui::InventoryAction::PickItem(item));
            advance(state, 2);
        }
        2 if session.inventory.count(wool()) == 1 => {
            state.original = Some(session.inventory.clone());
            world.write_message(terra_ui::InventoryAction::OpenChest(FILTER));
            advance(state, 3);
        }
        3 if world.resource::<terra_ui::InventoryView>().filtered_funnel && elapsed > 0.5 => {
            if !session.container.as_ref().is_some_and(|container| {
                container.filter == Some(Default::default()) && container.halves.is_empty()
            }) {
                return Err("Native filter did not open its ghost-only authority snapshot".into());
            }
            press_slot(world, "funnel-filter-slot", 0)?;
            advance(state, 4);
        }
        4 if world.resource::<terra_ui::InventoryView>().filter_selecting => {
            press_slot(world, "inventory-hotbar", 0)?;
            advance(state, 5);
        }
        5 if session.container.as_ref().is_some_and(|container| {
            container
                .filter
                .is_some_and(|filter| filter.item == Some(wool()) && filter.allowed)
        }) =>
        {
            if !world
                .resource::<terra_ui::ItemPreviewImages>()
                .0
                .contains_key(&wool())
            {
                return Ok(());
            }
            if !capture(world, state, "filtered-funnel-allowed", 2)? {
                return Ok(());
            }
            let switch = element(world, "funnel-filter-mode")?;
            event(world, switch, "onchange", Some("false".into()))?;
            advance(state, 6);
        }
        6 if session.container.as_ref().is_some_and(|container| {
            container
                .filter
                .is_some_and(|filter| filter.item == Some(wool()) && !filter.allowed)
        }) =>
        {
            if state.original.as_ref() != Some(&session.inventory) {
                return Err("Filter ghost selection changed the player's items".into());
            }
            if !capture(world, state, "filtered-funnel-blocked", 3)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 7);
        }
        7 if session.container.is_none()
            && !world.resource::<terra_ui::InventoryView>().open
            && !world.resource::<terra_player::PlayerInputBlock>().0 =>
        {
            request(
                world,
                InventoryAction::CreativeSet {
                    slot: 1,
                    item: ItemKind::Block(103),
                    count: 1,
                },
            );
            advance(state, 8);
        }
        8 if session
            .inventory
            .slot(1)
            .is_some_and(|stack| stack.item == ItemKind::Block(103)) =>
        {
            request(
                world,
                InventoryAction::CreativeSet {
                    slot: 2,
                    item: dye(),
                    count: 1,
                },
            );
            advance(state, 9);
        }
        9 if session
            .inventory
            .slot(2)
            .is_some_and(|stack| stack.item == dye()) =>
        {
            let mut cells = [None; 9];
            cells[0] = Some(terra_core::crafting::CraftGridCell { slot: 1, count: 1 });
            cells[1] = Some(terra_core::crafting::CraftGridCell { slot: 2, count: 1 });
            request(
                world,
                InventoryAction::CraftReserved {
                    cells,
                    size: 2,
                    station: None,
                    count: 1,
                    destination: terra_core::crafting::CraftDestination::Inventory,
                },
            );
            advance(state, 10);
        }
        10 if session.inventory.count(wool()) == 2 => {
            if session.inventory.count(ItemKind::Block(103)) != 0
                || session.inventory.count(dye()) != 0
            {
                return Err("Native RGB recipe failed atomic ingredient consumption".into());
            }
            world.write_message(terra_ui::InventoryAction::Toggle);
            advance(state, 11);
        }
        11 if world.resource::<terra_ui::InventoryView>().open && elapsed > 0.4 => {
            world.write_message(terra_ui::InventoryAction::Recipe(format!(
                "item-{}",
                wool().identifier().trim_start_matches("teramic::")
            )));
            advance(state, 12);
        }
        12 if world.resource::<terra_ui::CraftingView>().recipe_open && elapsed > 0.4 => {
            let recipe = world.resource::<terra_ui::CraftingView>();
            if recipe.recipe_heading != "Coloring Recipe"
                || recipe.recipe_grid.len() != 2
                || recipe
                    .recipe_output
                    .first()
                    .is_none_or(|slot| slot.item != Some(wool()))
            {
                return Err("Native RGB recipe inspector mismatch".into());
            }
            if !capture(world, state, "rgb-coloring-recipe", 4)? {
                return Ok(());
            }
            world.write_message(terra_ui::CraftingAction::CloseRecipe);
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 13);
        }
        13 if !world.resource::<terra_ui::InventoryView>().open
            && !world.resource::<terra_player::PlayerInputBlock>().0 =>
        {
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 8.5 17.91 6.5".into(),
                ));
            advance(state, 14);
        }
        14 if elapsed > 2.0
            && !session.teleporting
            && session.confirmed_state().is_some_and(|pose| {
                pose.grounded
                    && (pose.position[0] - 8.5).abs() < 0.2
                    && (pose.position[2] - 6.5).abs() < 0.2
            }) =>
        {
            if !capture(world, state, "belt-inventory-contact-detail", 5)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::OpenChest(FILTERED_TUNNEL));
            advance(state, 15);
        }
        15 if world.resource::<terra_ui::InventoryView>().filtered_funnel
            && session
                .container
                .as_ref()
                .is_some_and(|container| container.anchor == FILTERED_TUNNEL)
            && elapsed > 0.4 =>
        {
            let view = world.resource::<terra_ui::InventoryView>();
            if view.title != "Tunnel (Filtered)"
                || view.filter_allowed
                || view
                    .funnel_filter
                    .first()
                    .is_none_or(|slot| slot.item != Some(ItemKind::Block(3)) || !slot.empty)
            {
                return Err(
                    "Saved filtered tunnel did not display its exact ghost and mode".into(),
                );
            }
            if !capture(world, state, "filtered-tunnel-ghost", 6)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 16);
        }
        16 if !world.resource::<terra_ui::InventoryView>().open
            && !world.resource::<terra_player::PlayerInputBlock>().0 =>
        {
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 3.5 17.91 8.5".into(),
                ));
            advance(state, 17);
        }
        17 if elapsed > 2.0
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|pose| pose.grounded && (pose.position[2] - 8.5).abs() < 0.2) =>
        {
            world.write_message(terra_ui::InventoryAction::OpenChest(DISTRIBUTOR));
            advance(state, 18);
        }
        18 if world.resource::<terra_ui::InventoryView>().distributor && elapsed > 0.4 => {
            if session
                .container
                .as_ref()
                .and_then(|container| container.distributor)
                != Some(distributor_config())
                || world
                    .resource::<terra_ui::InventoryView>()
                    .distributor_filters
                    .len()
                    != 3
            {
                return Err(
                    "Saved distributor did not expose all three independent filters".into(),
                );
            }
            state.original = Some(session.inventory.clone());
            press_slot(world, "distributor-filter-slots", 2)?;
            advance(state, 19);
        }
        19 if world.resource::<terra_ui::InventoryView>().filter_selecting => {
            press_slot(world, "inventory-hotbar", 0)?;
            advance(state, 20);
        }
        20 if session
            .container
            .as_ref()
            .and_then(|container| container.distributor)
            .is_some_and(|config| {
                config.filters[2].item == Some(wool()) && config.filters[2].allowed
            }) =>
        {
            let switch = element(world, "distributor-filter-mode-2")?;
            event(world, switch, "onchange", Some("false".into()))?;
            advance(state, 21);
        }
        21 if session
            .container
            .as_ref()
            .and_then(|container| container.distributor)
            .is_some_and(|config| {
                config.filters[2].item == Some(wool()) && !config.filters[2].allowed
            }) =>
        {
            let config = session.container.as_ref().unwrap().distributor.unwrap();
            if config.filters[..2] != distributor_config().filters[..2]
                || state.original.as_ref() != Some(&session.inventory)
            {
                return Err(
                    "Configuring one distributor output changed another output or real inventory"
                        .into(),
                );
            }
            if !capture(world, state, "belt-distributor-three-filters", 7)? {
                return Ok(());
            }
            info!(
                captures = state.captures,
                "RGB logistics native smoke passed: RGB world/crafting, saved filtered tunnel, three independent distributor ghosts and real switches without inventory mutation"
            );
            state.stage = 255;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}
