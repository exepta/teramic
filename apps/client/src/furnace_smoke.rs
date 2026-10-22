//! Native furnace placement, input routing, TiltUI gestures and saved authority state.
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
    BellowsKind, Config, ControlsConfig, GameAction, GameConfig, GameMode, InputBinding, ItemKind,
    MineralItem, PlayerProfile, creatures::AnimalProduct,
};
use terra_network::client::ClientSession;

const RAW: ItemKind = ItemKind::Mineral(MineralItem::RawIron);
const RESULT: ItemKind = ItemKind::Mineral(MineralItem::IronIngot);
const FOOD: ItemKind = ItemKind::Animal(AnimalProduct::CookedFish);
const BELLOWS: ItemKind = ItemKind::Bellows(BellowsKind::Ruby);
const COAL: ItemKind = ItemKind::Mineral(MineralItem::Coal);

fn directory() -> PathBuf {
    std::env::var_os("TERRA_FURNACE_GAMEPLAY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-furnace-gameplay-{}", std::process::id()))
        })
}
pub(crate) fn menu_directory() -> PathBuf {
    directory().join("menu")
}

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.allow_flight = false;
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.water = false;
    config.world.rivers = false;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    if directory().join("world").exists() {
        return Err("Furnace gameplay fixture requires a fresh TERRA_FURNACE_GAMEPLAY_DIR".into());
    }
    if std::env::var_os("TERRA_PROFILE_FILE").is_some()
        || std::env::args().any(|arg| arg == "--profile")
    {
        return Err("Furnace gameplay fixture owns its profile; omit profile overrides".into());
    }
    let profile = PlayerProfile {
        name: "Furnace-Smoke".into(),
        ..default()
    };
    let mut permissions = config.server.permissions_for(profile.uuid);
    permissions.game_mode = true;
    config
        .server
        .player_permissions
        .insert(profile.uuid, permissions);
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
    let bag = journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default();
    for (item, count) in [
        (ItemKind::Block(59), 1),
        (RAW, 4),
        (COAL, 2),
        (BELLOWS, 1),
        (FOOD, 3),
        (ItemKind::Block(65), 1),
        (ItemKind::Block(68), 1),
    ] {
        bag.insert(item, count);
    }
    terra_storage::save_edits(&directory().join("world"), config.world.id, &journal)?;
    let (host, token) =
        terra_server::IntegratedServer::start_with_save(config, Some(directory().join("world")))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Fixture {
    start: Instant,
    entered: Instant,
    stage: u8,
    anchor: Option<[i64; 3]>,
    captures: usize,
    requested_captures: usize,
    failed_capture: bool,
    initialized: bool,
    last_diagnostic: Instant,
    inspected_inventory: Option<terra_core::Inventory>,
    placement_response_after: u64,
    compact_measurements: Vec<(Vec2, Vec2)>,
    compact_stable_frames: u8,
    capture_recovery: Option<Instant>,
    retreat: Option<(Instant, Vec3, Vec3)>,
}
impl Fixture {
    fn advance(&mut self, label: &str) {
        self.stage += 1;
        self.entered = Instant::now();
        info!(stage = self.stage, %label, "Furnace gameplay smoke advanced");
    }
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    if !std::env::args().any(|arg| arg == "--smoke-test-furnace-gameplay") {
        return Ok(());
    }
    let now = Instant::now();
    app.insert_resource(Fixture {
        start: now,
        entered: now,
        stage: 0,
        anchor: None,
        captures: 0,
        requested_captures: 0,
        failed_capture: false,
        initialized: false,
        last_diagnostic: now,
        inspected_inventory: None,
        placement_response_after: 0,
        compact_measurements: Vec::new(),
        compact_stable_frames: 0,
        capture_recovery: None,
        retreat: None,
    })
    .insert_resource(terra_player::ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
    Ok(())
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    if world.resource::<terra_ui::InventoryView>().open
        && world.resource::<terra_ui::InventoryView>().furnace
    {
        validate_furnace_layout(world)?;
    }
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory().join("captures"));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut fixture: ResMut<Fixture>| {
                fixture.failed_capture |= event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
                        .count()
                        < 1000
                });
                fixture.captures += 1;
            },
        );
    Ok(())
}

fn validate_furnace_layout(world: &mut World) -> Result<(), String> {
    if world
        .resource::<terra_ui::InventoryView>()
        .category_dropdown
    {
        let expected = world.resource::<terra_ui::InventoryView>().categories.len();
        let choice = element(world, "inventory-category-select")?;
        let parts = *world
            .get::<tilt_ui::ChoiceBoxParts>(choice)
            .ok_or("Category dropdown has no TiltUI choice control")?;
        let selected = world
            .resource::<terra_ui::InventoryView>()
            .categories
            .iter()
            .find(|c| c.selected)
            .map(|c| c.name.clone())
            .ok_or("No selected category")?;
        if world
            .get::<Text>(parts.value)
            .is_none_or(|text| text.0 != selected)
        {
            return Err(format!(
                "Category dropdown value does not match selected category {selected}"
            ));
        }
        if !parts.open
            && world
                .get::<ComputedNode>(parts.popup)
                .is_some_and(|node| !node.is_empty())
        {
            return Err("Closed category dropdown exposes its option popup".into());
        }
        let options: Vec<_> = world
            .query::<(Entity, &tilt_ui::OptionData)>()
            .iter(world)
            .map(|(entity, _)| entity)
            .collect();
        let count = options
            .into_iter()
            .filter(|&entity| {
                let mut current = entity;
                for _ in 0..24 {
                    if current == parts.popup {
                        return true;
                    }
                    let Some(parent) = world.get::<ChildOf>(current) else {
                        break;
                    };
                    current = parent.parent();
                }
                false
            })
            .count();
        if count != expected {
            return Err(format!(
                "Category dropdown has {count} options, expected {expected}"
            ));
        }
    }
    let body = element(world, "inventory-scroll")?;
    let node = world
        .get::<Node>(body)
        .ok_or("Furnace body has no layout")?;
    if node.overflow != Overflow::visible() {
        return Err("Furnace content must not create a scrollbar or clip its slots".into());
    }
    let computed = world
        .get::<ComputedNode>(body)
        .ok_or("Furnace body is not measured")?;
    let content = computed.content_size();
    let viewport = computed.size();
    if content.y > viewport.y + 2.0 || content.x > viewport.x + 2.0 {
        let children = furnace_layout_measurements(world)?;
        return Err(format!(
            "Furnace content exceeds its body: content={content:?}, viewport={viewport:?}, panel/body/workspace/grid/hotbar={children:?}"
        ));
    }
    for (_, computed) in world
        .query::<(&tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .filter(|(id, computed)| id.0 == "inventory-category" && !computed.is_empty())
    {
        let size = computed.size() * computed.inverse_scale_factor;
        if (size.x - 80.0).abs() > 1.0 || (size.y - 80.0).abs() > 1.0 {
            return Err(format!("Catalog category is not an 80px square: {size:?}"));
        }
    }
    Ok(())
}

fn furnace_layout_measurements(world: &mut World) -> Result<Vec<(Vec2, Vec2)>, String> {
    [
        "inventory-panel",
        "inventory-scroll",
        "furnace-workspace",
        "inventory-grid",
        "inventory-hotbar",
    ]
    .into_iter()
    .map(|id| {
        let entity = element(world, id)?;
        let node = world
            .get::<ComputedNode>(entity)
            .ok_or_else(|| format!("Furnace layout node {id} is not measured"))?;
        Ok((node.size(), node.content_size()))
    })
    .collect()
}

fn captured(
    world: &mut World,
    fixture: &mut Fixture,
    name: &str,
    count: usize,
) -> Result<bool, String> {
    if fixture.captures >= count {
        return Ok(true);
    }
    if fixture.requested_captures < count {
        capture(world, name)?;
        fixture.requested_captures = count;
    }
    Ok(false)
}

fn previews_ready(world: &World) -> bool {
    world
        .resource::<terra_ui::BlockPreviewImages>()
        .0
        .contains_key(&59)
        && [RAW, RESULT, COAL, BELLOWS, FOOD].into_iter().all(|item| {
            world
                .resource::<terra_ui::ItemPreviewImages>()
                .0
                .contains_key(&item)
        })
}

fn visible_item_preview(world: &mut World, item: ItemKind) -> bool {
    let Some(handle) = world
        .resource::<terra_ui::ItemPreviewImages>()
        .0
        .get(&item)
        .cloned()
    else {
        return false;
    };
    let alt = format!("Item preview {}", item.identifier());
    world
        .query::<(
            &tilt_ui::ImageMetadata,
            &bevy::ui::widget::ImageNode,
            &ComputedNode,
            &Visibility,
        )>()
        .iter(world)
        .any(|(metadata, image, node, visibility)| {
            metadata.alt.as_deref() == Some(alt.as_str())
                && image.image == handle
                && !node.is_empty()
                && *visibility != Visibility::Hidden
        })
}

fn element(world: &mut World, id: &str) -> Result<Entity, String> {
    world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, candidate, node)| {
            (candidate.0 == id && !node.is_empty()).then_some(entity)
        })
        .ok_or_else(|| format!("Missing visible furnace UI element {id}"))
}

fn click(world: &mut World, root: &str, slot: Option<usize>) -> Result<(), String> {
    let target = slot_element(world, root, slot)?;
    let attribute = if slot.is_some() {
        "onmousedown"
    } else {
        "onclick"
    };
    emit_ui_event(
        world,
        target,
        attribute,
        if slot.is_some() { "mousedown" } else { "click" },
    )
}

fn slot_element(world: &mut World, root: &str, slot: Option<usize>) -> Result<Entity, String> {
    let root = element(world, root)?;
    if let Some(index) = slot {
        let mut queue = vec![root];
        let mut cells = Vec::new();
        let mut i = 0;
        while i < queue.len() {
            let current = queue[i];
            if world
                .get::<tilt_ui::ElementId>(current)
                .is_some_and(|id| id.0 == "inventory-cell")
            {
                cells.push(current);
            }
            if let Some(children) = world.get::<Children>(current) {
                queue.extend(children.iter());
            }
            i += 1;
        }
        cells
            .get(index)
            .copied()
            .ok_or("Missing furnace slot button".into())
    } else {
        Ok(root)
    }
}

fn emit_ui_event(
    world: &mut World,
    target: Entity,
    attribute: &str,
    kind: &'static str,
) -> Result<(), String> {
    let handler = world
        .get::<tilt_ui::StaticAttributes>(target)
        .and_then(|a| a.attributes.iter().find(|a| a.name == attribute))
        .map(|a| a.value.clone())
        .ok_or("Missing furnace UI event handler")?;
    world.write_message(tilt_ui::HtmlEvent {
        target,
        kind,
        value: None,
        submitter: None,
        data: [("button".into(), "Primary".into())].into(),
        form_data: None,
        handler,
    });
    Ok(())
}

fn fixture_object(fixture: &Fixture, offset: i64) -> Result<[i64; 3], String> {
    let p = fixture.anchor.ok_or("Missing placed furnace")?;
    Ok([p[0] + offset, p[1], p[2]])
}

fn aim_object(world: &mut World, position: [i64; 3], height: f32) -> bool {
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(
            Vec3::new(
                position[0] as f32 + 0.5,
                position[1] as f32 + height,
                position[2] as f32 + 0.5,
            ),
            Vec3::Y,
        );
    }
    world
        .resource::<terra_player::TargetBlock>()
        .0
        .is_some_and(|hit| {
            hit.position == position || hit.position == [position[0], position[1] + 1, position[2]]
        })
}

fn aimed_placement(
    world: &mut World,
    destination: [i64; 3],
    material: u16,
) -> Option<terra_world::streaming::BlockEditRequest> {
    let support = [destination[0], destination[1] - 1, destination[2]];
    aim_object(world, support, 0.99);
    let hit = world.resource::<terra_player::TargetBlock>().0?;
    if hit.position != support || hit.normal != [0, 1, 0] {
        return None;
    }
    let (position, expected_material) = terra_world::targeting::block_placement(
        world.resource::<terra_world::streaming::ChunkStore>(),
        hit,
    )?;
    (position == destination).then_some(terra_world::streaming::BlockEditRequest {
        position,
        expected_material,
        material,
    })
}

fn object_open(world: &World, position: [i64; 3]) -> bool {
    world
        .resource::<terra_world::edits::EditJournal>()
        .object_state(position)
        .is_some_and(|state| state.facing >= 4)
}

fn retreat_after_opening(world: &mut World, fixture: &mut Fixture) -> Result<bool, String> {
    let body = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .ok_or("Missing movement probe body")?
        .translation;
    let confirmed = world
        .resource::<ClientSession>()
        .confirmed_state()
        .ok_or("Missing authoritative movement probe")?;
    let authoritative = Vec3::from_array(confirmed.position);
    let (started, local_start, server_start) =
        *fixture
            .retreat
            .get_or_insert((Instant::now(), body, authoritative));
    if started.elapsed() < Duration::from_millis(900) {
        return Ok(false);
    }
    if body.distance(local_start) < 1.3 || authoritative.distance(server_start) < 1.3 {
        return Err(format!(
            "Opening froze or reconciled player movement: local={local_start:?}->{body:?}, authority={server_start:?}->{authoritative:?}"
        ));
    }
    fixture.retreat = None;
    Ok(true)
}

fn gamemode(world: &mut World, mode: &str) {
    world
        .resource_mut::<terra_network::client::ClientChatCommands>()
        .0
        .push_back(terra_network::client::ChatCommand::Send(format!(
            "/gamemode {mode}"
        )));
}

fn maintain_fixture_focus(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    // All fixture input is synthetic. Desktop focus must not cancel its actions.
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        if !window.focused {
            window.focused = true;
        }
    }
    let gameplay = matches!(fixture.stage, 0..=3 | 15..=23)
        && !world.resource::<terra_ui::InventoryView>().open
        && world.resource::<ClientSession>().ready;
    if gameplay && !world.resource::<terra_player::CameraInputState>().captured {
        world.write_message(terra_player::CaptureRequest(true));
        let recovery = fixture.capture_recovery.get_or_insert_with(Instant::now);
        if fixture.stage != 0 && recovery.elapsed() > Duration::from_secs(4) {
            return Err(format!(
                "Fixture camera capture could not recover in stage {}: blocked={}, menu={:?}",
                fixture.stage,
                world.resource::<terra_player::PlayerInputBlock>().0,
                world.resource::<terra_ui::menu::MenuState>().page
            ));
        }
    } else {
        fixture.capture_recovery = None;
    }
    if matches!(fixture.stage, 26 | 27 | 30 | 31)
        && !world.resource::<terra_player::CameraInputState>().captured
    {
        return Err("Door/window interaction released the gameplay cursor".into());
    }
    if fixture.stage == 17 && fixture.entered.elapsed() < Duration::from_millis(650) {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
    }
    if fixture.stage == 20 && fixture.entered.elapsed() < Duration::from_millis(650) {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
    }
    if let Some((started, _, _)) = fixture.retreat {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if started.elapsed() < Duration::from_millis(450) {
            keys.press(KeyCode::KeyS);
        } else {
            keys.release(KeyCode::KeyS);
        }
    }
    Ok(())
}

fn run(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.stage == 255 {
            return;
        }
        if let Err(error) =
            maintain_fixture_focus(world, &mut fixture).and_then(|()| step(world, &mut fixture))
        {
            error!(%error, stage = fixture.stage, "Native furnace gameplay smoke failed");
            fixture.stage = 255;
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyU);
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::ShiftLeft);
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyS);
            if let Some(host) = world.remove_resource::<terra_server::IntegratedServer>() {
                let _ = host.finish();
            }
            world.write_message(AppExit::error());
        }
    });
}

fn step(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    if fixture.last_diagnostic.elapsed() >= Duration::from_secs(5) {
        fixture.last_diagnostic = Instant::now();
        let session = world.resource::<ClientSession>();
        let render = world.resource::<terra_render::RenderStatus>();
        info!(
            stage = fixture.stage,
            initialized = fixture.initialized,
            controls_present = world.contains_resource::<ControlsConfig>(),
            menu_present = world.contains_resource::<terra_ui::menu::MenuState>(),
            session_ready = session.ready,
            mode = ?session.game_mode,
            inventory_open = world.resource::<terra_ui::InventoryView>().open,
            inventory_layout = %world.resource::<terra_ui::InventoryView>().layout_style,
            food_count = session.inventory.count(FOOD),
            render_ready = render.ready,
            render_pending = render.pending_jobs,
            render_error = ?render.error,
            grounded = ?session.confirmed_state().map(|state| state.grounded),
            position = ?session.confirmed_state().map(|state| state.position),
            captured = world.resource::<terra_player::CameraInputState>().captured,
            preview_pending = world.resource::<terra_render::RenderBlockPreviews>().pending,
            preview_error = ?world.resource::<terra_render::RenderBlockPreviews>().error,
            required_previews_ready = previews_ready(world),
            cursor = ?session.inventory.cursor,
            furnace_slots = ?session.container.as_ref().map(|s| &s.halves[0].inventory.slots),
            last_inventory = ?session.last_inventory,
            last_edit = ?session.last_edit,
            last_chat = ?world.resource::<terra_network::client::ClientChat>().entries.back().map(|entry| &entry.text),
            "Furnace gameplay smoke readiness"
        );
    }
    if fixture.start.elapsed() > Duration::from_secs(150) || fixture.failed_capture {
        return Err(format!(
            "Furnace gameplay fixture timed out or captured a blank image in stage {}",
            fixture.stage
        ));
    }
    if !fixture.initialized {
        let Some(mut controls) = world.get_resource_mut::<ControlsConfig>() else {
            return Ok(());
        };
        controls.rebind(GameAction::Place, InputBinding::Key(KeyCode::KeyU))?;
        let Some(mut menu) = world.get_resource_mut::<terra_ui::menu::MenuState>() else {
            return Ok(());
        };
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
        fixture.initialized = true;
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    let elapsed = fixture.entered.elapsed().as_secs_f32();
    if matches!(fixture.stage, 32 | 33) && elapsed > 8.0 {
        return Err(format!(
            "Creative inventory transition timed out in stage {}: mode={:?}, ready={}, teleporting={}, open={}, layout={}, captured={}, menu={:?}, queued_commands={}, last_chat={:?}",
            fixture.stage,
            session.game_mode,
            session.ready,
            session.teleporting,
            world.resource::<terra_ui::InventoryView>().open,
            world.resource::<terra_ui::InventoryView>().layout_style,
            world.resource::<terra_player::CameraInputState>().captured,
            world.resource::<terra_ui::menu::MenuState>().page,
            world
                .resource::<terra_network::client::ClientChatCommands>()
                .0
                .len(),
            world
                .resource::<terra_network::client::ClientChat>()
                .entries
                .back()
                .map(|entry| &entry.text),
        ));
    }
    if !session.ready {
        return Ok(());
    }
    if fixture.stage == 0 && session.inventory.count(FOOD) != 3 {
        return Ok(());
    }
    if session.inventory.count(FOOD) != 3 || session.consumption.is_some() {
        return Err("Station interaction consumed food or started a bite".into());
    }
    if let Some(error) = &world.resource::<terra_render::RenderBlockPreviews>().error {
        return Err(format!("Furnace fixture item preview failed: {error}"));
    }
    if matches!(fixture.stage, 8 | 9) && session.inventory.cursor.is_some() {
        return Err("Shift quick-move picked up a stack instead of transferring it".into());
    }
    let snapshot = session.container.clone();
    let furnace = snapshot.as_ref().and_then(|s| s.furnace.as_ref());
    if matches!(fixture.stage, 21 | 23) {
        if let Some((id, outcome)) = session.last_edit
            && id > fixture.placement_response_after
            && outcome != terra_network::protocol::session::EditOutcome::Applied
        {
            return Err(format!(
                "Opening placement rejected in stage {}: {outcome:?}, player={:?}, target={:?}",
                fixture.stage,
                session.confirmed_state().map(|state| state.position),
                world.resource::<terra_player::TargetBlock>().0
            ));
        }
        if elapsed > 6.0 {
            return Err(format!(
                "Opening placement response timed out in stage {}: last_edit={:?}, player={:?}",
                fixture.stage,
                session.last_edit,
                session.confirmed_state().map(|state| state.position)
            ));
        }
    }
    match fixture.stage {
        0 if world.resource::<terra_render::RenderStatus>().ready
            && previews_ready(world)
            && session
                .confirmed_state()
                .is_some_and(|state| state.grounded) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            world.write_message(terra_player::CaptureRequest(true));
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -0.65, 0.0);
            }
            fixture.advance("aim placement at the ground");
        }
        1 if elapsed > 0.7 && world.resource::<terra_player::CameraInputState>().captured => {
            let hit = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("No furnace placement target")?;
            let (position, expected_material) = terra_world::targeting::block_placement(
                world.resource::<terra_world::streaming::ChunkStore>(),
                hit,
            )
            .ok_or("No furnace placement cell")?;
            fixture.anchor = Some(position);
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material,
                material: 59,
            });
            fixture.advance("await authoritative two-block placement");
        }
        2 if fixture.anchor.is_some_and(|p| {
            world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(p)
                == Some(59)
        }) && session.inventory.count(ItemKind::Block(59)) == 0 =>
        {
            let p = fixture.anchor.unwrap();
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at([p[0], p[1] + 1, p[2]])
                != Some(59)
            {
                return Ok(());
            }
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.look_at(
                    Vec3::new(p[0] as f32 + 0.5, p[1] as f32 + 0.75, p[2] as f32 + 0.5),
                    Vec3::Y,
                );
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(4));
            fixture.advance("aim at furnace with food selected");
        }
        3 if elapsed > 0.5 => {
            let p = fixture.anchor.ok_or("Missing placed furnace")?;
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.look_at(
                    Vec3::new(p[0] as f32 + 0.5, p[1] as f32 + 0.75, p[2] as f32 + 0.5),
                    Vec3::Y,
                );
            }
            let target = world.resource::<terra_player::TargetBlock>().0;
            let furnace_targeted = target.is_some_and(|hit| {
                (hit.position == p || hit.position == [p[0], p[1] + 1, p[2]])
                    && world
                        .resource::<terra_world::streaming::ChunkStore>()
                        .block_at(hit.position)
                        == Some(59)
            });
            if !furnace_targeted {
                if elapsed > 8.0 {
                    let cameras: Vec<_> = world
                        .query_filtered::<&Transform, With<terra_player::TerraCamera>>()
                        .iter(world)
                        .map(|camera| (camera.translation, *camera.forward()))
                        .collect();
                    return Err(format!(
                        "Placed furnace did not become targeted within 8s: anchor={p:?}, target={target:?}, cameras(position, forward)={cameras:?}"
                    ));
                }
                return Ok(());
            }
            if !captured(world, fixture, "furnace-placed", 1)? {
                return Ok(());
            }
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyU);
            fixture.advance("open via rebound Place while food is held");
        }
        4 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyU);
            if furnace.is_some()
                && world.resource::<terra_ui::InventoryView>().furnace
                && elapsed > 0.5
            {
                world.write_message(terra_ui::InventoryAction::Search("iron_ingot".into()));
                world.write_message(terra_ui::InventoryAction::Category("materials".into()));
                fixture.advance("filter Too Many Items");
            }
        }
        5 if elapsed > 0.4 => {
            click(world, "inventory-browser-grid", Some(0))?;
            fixture.advance("inspect the smelting recipe");
        }
        6 if elapsed > 0.4 && world.resource::<terra_ui::CraftingView>().recipe_open => {
            let view = world.resource::<terra_ui::CraftingView>();
            if !view.smelting || !view.can_apply_recipe {
                return Err("TMI did not select the available furnace recipe".into());
            }
            if ![RAW, RESULT, COAL, BELLOWS, FOOD]
                .into_iter()
                .all(|item| visible_item_preview(world, item))
            {
                return Ok(());
            }
            if !captured(world, fixture, "furnace-smelting-recipe", 2)? {
                return Ok(());
            }
            click(world, "craft-apply-recipe", None)?;
            fixture.advance("load material through recipe Plus");
        }
        7 if snapshot.as_ref().is_some_and(|s| {
            s.halves[0]
                .inventory
                .slot(0)
                .is_some_and(|s| s.item == RAW && s.count == 1)
        }) && elapsed > 0.3 =>
        {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ShiftLeft);
            click(world, "inventory-hotbar", Some(2))?;
            fixture.advance("quick-move coal into fuel");
        }
        8 if furnace.is_some_and(|f| f.burn_remaining > 0.0) && elapsed > 0.3 => {
            click(world, "inventory-hotbar", Some(3))?;
            fixture.advance("quick-move Ruby Bellows");
        }
        9 if snapshot.as_ref().is_some_and(|s| {
            s.halves[0]
                .inventory
                .slot(3)
                .is_some_and(|s| s.item == BELLOWS)
        }) && furnace.is_some_and(|f| f.progress > 0.1)
            && elapsed > 0.4 =>
        {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::ShiftLeft);
            if ![RAW, COAL, BELLOWS, FOOD]
                .into_iter()
                .all(|item| visible_item_preview(world, item))
            {
                return Ok(());
            }
            capture(world, "furnace-running-wide")?;
            fixture.advance("capture live progress");
        }
        10 if fixture.captures >= 3 => {
            if let Ok(mut window) = world
                .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
                .single_mut(world)
            {
                window.resolution.set(960.0, 720.0);
            }
            fixture.advance("resize native furnace UI");
        }
        11 if elapsed > 0.7 => {
            for id in [
                "furnace-input",
                "furnace-fuel",
                "furnace-output",
                "furnace-bellows",
            ] {
                element(world, id)?;
            }
            if let Err(error) = validate_furnace_layout(world) {
                fixture.compact_stable_frames = 0;
                fixture.compact_measurements.clear();
                if elapsed > 4.0 {
                    return Err(format!("Compact layout failed to settle: {error}"));
                }
                return Ok(());
            }
            let measured = furnace_layout_measurements(world)?;
            if fixture.compact_measurements == measured {
                fixture.compact_stable_frames += 1;
            } else {
                fixture.compact_measurements = measured;
                fixture.compact_stable_frames = 0;
            }
            if fixture.compact_stable_frames < 4 {
                if elapsed > 4.0 {
                    return Err(format!(
                        "Compact layout keeps changing: {:?}",
                        fixture.compact_measurements
                    ));
                }
                return Ok(());
            }
            capture(world, "furnace-running-compact")?;
            fixture.advance("wait for authoritative smelting output");
        }
        12 if fixture.captures >= 4
            && snapshot.as_ref().is_some_and(|s| {
                s.halves[0]
                    .inventory
                    .slot(2)
                    .is_some_and(|s| s.item == RESULT)
            }) =>
        {
            if world.resource::<ClientSession>().inventory.count(RAW) != 3 {
                return Err("Recipe loading removed an unexpected material count".into());
            }
            capture(world, "furnace-result")?;
            fixture.advance("take the smelting output through its real slot");
        }
        13 if fixture.captures >= 5 => {
            click(world, "furnace-output", Some(0))?;
            fixture.advance("await authoritative output pickup");
        }
        14 if session
            .inventory
            .cursor
            .is_some_and(|stack| stack.item == RESULT && (1..=2).contains(&stack.count))
            && snapshot
                .as_ref()
                .is_some_and(|s| s.halves[0].inventory.slot(2).is_none()) =>
        {
            click(world, "inventory-hotbar", Some(0))?;
            fixture.advance("place the result in the empty player slot");
        }
        15 if session.inventory.cursor.is_none()
            && (1..=2).contains(&session.inventory.count(RESULT)) =>
        {
            world.write_message(terra_ui::InventoryAction::Close);
            world.write_message(terra_player::CaptureRequest(true));
            if let Ok(mut window) = world
                .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
                .single_mut(world)
            {
                window.resolution.set(1280.0, 800.0);
            }
            fixture.advance("frame the burning furnace in the world");
        }
        16 if !world.resource::<terra_ui::InventoryView>().open
            && world.resource::<terra_player::CameraInputState>().captured =>
        {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyS);
            fixture.advance("step back from the furnace");
        }
        17 if elapsed > 0.65 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyS);
            let p = fixture.anchor.ok_or("Missing placed furnace")?;
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.look_at(
                    Vec3::new(p[0] as f32 + 0.5, p[1] as f32 + 1.2, p[2] as f32 + 0.5),
                    Vec3::Y,
                );
            }
            fixture.advance("wait for the world view to settle");
        }
        18 if elapsed > 0.7 && !world.resource::<terra_ui::InventoryView>().open => {
            if !captured(world, fixture, "furnace-burning-world", 6)? {
                return Ok(());
            }
            fixture.advance("place a door and window through authority");
        }
        19 if fixture.captures >= 6 => {
            world.write_message(terra_ui::InventoryAction::Hotbar(5));
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyW);
            fixture.advance("return to placement reach and select the oak door");
        }
        20 if elapsed > 0.65 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyW);
            let destination = fixture_object(fixture, -2)?;
            let placement = aimed_placement(world, destination, 65);
            if elapsed < 1.0 {
                return Ok(());
            }
            let Some(placement) = placement else {
                if elapsed > 4.0 {
                    return Err(format!(
                        "Door support is not targetable: destination={destination:?}, target={:?}",
                        world.resource::<terra_player::TargetBlock>().0
                    ));
                }
                return Ok(());
            };
            fixture.placement_response_after = world
                .resource::<ClientSession>()
                .last_edit
                .map_or(0, |(id, _)| id);
            world.write_message(placement);
            fixture.advance("await authoritative door placement");
        }
        21 if session.inventory.count(ItemKind::Block(65)) == 0
            && world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(fixture_object(fixture, -2)?)
                == Some(65) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(6));
            aim_object(world, fixture_object(fixture, 2)?, -0.01);
            fixture.advance("select the oak window");
        }
        22 if elapsed > 0.4 => {
            let destination = fixture_object(fixture, 2)?;
            let Some(placement) = aimed_placement(world, destination, 68) else {
                if elapsed > 4.0 {
                    return Err(format!(
                        "Window support is not targetable: destination={destination:?}, target={:?}",
                        world.resource::<terra_player::TargetBlock>().0
                    ));
                }
                return Ok(());
            };
            fixture.placement_response_after = world
                .resource::<ClientSession>()
                .last_edit
                .map_or(0, |(id, _)| id);
            world.write_message(placement);
            fixture.advance("await authoritative window placement");
        }
        23 if session.inventory.count(ItemKind::Block(68)) == 0
            && world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(fixture_object(fixture, 2)?)
                == Some(68) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(4));
            aim_object(world, fixture_object(fixture, -2)?, 0.9);
            fixture.advance("aim at the closed door while holding food");
        }
        24 if elapsed > 0.5 => {
            if !aim_object(world, fixture_object(fixture, -2)?, 0.9) {
                return Ok(());
            }
            if !captured(world, fixture, "door-closed", 7)? {
                return Ok(());
            }
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyU);
            fixture.advance("open door through Place binding");
        }
        25 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyU);
            if object_open(world, fixture_object(fixture, -2)?) {
                if world.resource::<terra_ui::InventoryView>().open {
                    return Err("Door interaction incorrectly opened an inventory".into());
                }
                fixture.advance("capture the animated door hinge");
            }
        }
        26 if elapsed > 0.16 => {
            if !captured(world, fixture, "door-opening", 8)? {
                return Ok(());
            }
            fixture.advance("wait for the door to finish opening");
        }
        27 if elapsed > 0.6 => {
            if !captured(world, fixture, "door-open", 9)? {
                return Ok(());
            }
            if !retreat_after_opening(world, fixture)? {
                return Ok(());
            }
            aim_object(world, fixture_object(fixture, 2)?, 0.5);
            fixture.advance("aim at the closed window");
        }
        28 if elapsed > 0.5 => {
            if !aim_object(world, fixture_object(fixture, 2)?, 0.5) {
                return Ok(());
            }
            if !captured(world, fixture, "window-closed", 10)? {
                return Ok(());
            }
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyU);
            fixture.advance("open window through Place binding");
        }
        29 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyU);
            if object_open(world, fixture_object(fixture, 2)?) {
                if world.resource::<terra_ui::InventoryView>().open {
                    return Err("Window interaction incorrectly opened an inventory".into());
                }
                fixture.advance("capture the animated window hinge");
            }
        }
        30 if elapsed > 0.16 => {
            if !captured(world, fixture, "window-opening", 11)? {
                return Ok(());
            }
            fixture.advance("wait for the window to finish opening");
        }
        31 if elapsed > 0.6 => {
            if !captured(world, fixture, "window-open", 12)? {
                return Ok(());
            }
            if !retreat_after_opening(world, fixture)? {
                return Ok(());
            }
            gamemode(world, "creative");
            fixture.advance("await authoritative Creative mode");
        }
        32 if session.game_mode == GameMode::Creative && elapsed > 0.5 => {
            world.write_message(terra_ui::InventoryAction::Toggle);
            fixture.advance("wait for the Creative inventory layout");
        }
        33 if world.resource::<terra_ui::InventoryView>().open
            && world.resource::<terra_ui::InventoryView>().layout_style
                == "opacity: 1; pointer-events: auto;"
            && !world.resource::<terra_player::CameraInputState>().captured =>
        {
            world.write_message(terra_ui::InventoryAction::Search("iron_ingot".into()));
            world.write_message(terra_ui::InventoryAction::Category("materials".into()));
            fixture.advance("hover the Creative catalog item");
        }
        34 if elapsed > 0.5 => {
            let target = slot_element(world, "inventory-browser-grid", Some(0))?;
            emit_ui_event(world, target, "onmouseenter", "mouseenter")?;
            fixture.inspected_inventory = Some(world.resource::<ClientSession>().inventory.clone());
            fixture.advance("inspect Creative recipe with R");
        }
        35 if elapsed > 0.3 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyR);
            fixture.advance("await read-only Creative recipe");
        }
        36 => {
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyR);
            if world.resource::<terra_ui::CraftingView>().recipe_open && elapsed > 0.3 {
                if !world.resource::<terra_ui::CraftingView>().smelting
                    || fixture.inspected_inventory.as_ref()
                        != Some(&world.resource::<ClientSession>().inventory)
                {
                    return Err("Creative R did not inspect smelting read-only".into());
                }
                if !captured(world, fixture, "creative-recipe-shortcut", 13)? {
                    return Ok(());
                }
                world.write_message(terra_ui::InventoryAction::Close);
                gamemode(world, "survival");
                fixture.advance("verify saved furnace and open objects");
            }
        }
        37 if fixture.captures >= 13 && session.game_mode == GameMode::Survival => {
            let anchor = fixture.anchor.ok_or("Missing placed furnace")?;
            let id = world.resource::<GameConfig>().world.id;
            let uuid = world.resource::<PlayerProfile>().uuid;
            world
                .remove_resource::<terra_server::IntegratedServer>()
                .ok_or("Missing fixture server")?
                .finish()?;
            let saved = terra_storage::load_edits(&directory().join("world"), id)?;
            let contents = saved
                .container_at(anchor)
                .ok_or("Saved furnace inventory missing")?;
            let state = saved
                .furnace_at(anchor)
                .ok_or("Saved furnace progress missing")?;
            if contents.slot(2).is_some()
                || contents.slot(3).is_none_or(|stack| stack.item != BELLOWS)
                || state.completed != 1
                || state.burn_remaining <= 0.0
                || saved.gameplay().inventories.get(&uuid).is_none_or(|bag| {
                    bag.count(RAW) != 3
                        || bag.count(FOOD) != 3
                        || !(1..=2).contains(&bag.count(RESULT))
                        || bag.cursor.is_some()
                })
            {
                return Err(
                    "Furnace inventory, fuel, completion or food did not survive save/reload"
                        .into(),
                );
            }
            for position in [fixture_object(fixture, -2)?, fixture_object(fixture, 2)?] {
                if saved
                    .object_state(position)
                    .is_none_or(|state| state.facing < 4)
                {
                    return Err("Open door/window state did not survive save/reload".into());
                }
            }
            info!(
                captures = fixture.captures,
                "Native furnace gameplay passed: server placement, rebound interaction, TMI recipe and Creative R, slot gestures, progress/result, door/window animation and save/reload"
            );
            fixture.stage = 255;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}
