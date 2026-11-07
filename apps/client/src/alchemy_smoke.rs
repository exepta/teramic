//! Short integrated-authority check of brewing, campfire widgets and real recipe previews.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    GameConfig, GameMode, Inventory, ItemKind, ItemStack, PlayerProfile, ToolKind, brewing,
    campfire,
};
use terra_network::client::ClientSession;

const KETTLE: [i64; 3] = [0, 17, 0];
const FIRE: [i64; 3] = [3, 17, 0];
const TUNNELS: [[i64; 3]; 2] = [[-2, 17, 0], [-2, 17, 1]];
const MIXED: u16 = 6;

fn path() -> PathBuf {
    std::env::temp_dir().join(format!("terra-alchemy-{}", std::process::id()))
}

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
    if path().exists() {
        return Err("Alchemy fixture requires a fresh temporary directory".into());
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
            flight: false,
            environment: true,
        },
    );
    let mut journal = terra_world::edits::EditJournal::default();
    for x in -4..=7 {
        for z in -3..=7 {
            journal
                .record([x, 16, z], 3)
                .map_err(|error| error.to_string())?;
            for y in 17..=22 {
                journal
                    .record([x, y, z], 0)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    for (position, material) in [
        (FIRE, campfire::MATERIAL),
        (KETTLE, brewing::MATERIAL),
        (TUNNELS[0], terra_core::logistics::TUNNEL),
        (TUNNELS[1], terra_core::logistics::FILTERED_TUNNEL),
    ] {
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(
            position,
            Some(terra_world::edits::BlockObjectState {
                facing: if position == KETTLE { 4 } else { 0 },
                ..Default::default()
            }),
        )?;
    }
    let mut kettle = Inventory {
        slots: vec![ItemStack::EMPTY; brewing::SLOTS],
        ..default()
    };
    let recipe = brewing::recipe(MIXED).ok_or("Missing mixed-potion recipe")?;
    for (slot, ingredient) in recipe.inputs.iter().enumerate() {
        kettle.slots[slot] = *ingredient;
    }
    kettle.slots[brewing::BOTTLES] = ItemStack {
        item: ItemKind::GlassBottle,
        count: 1,
    };
    kettle.slots[brewing::SPOON] = ItemStack {
        item: ItemKind::Tool(ToolKind::GoldSpoon),
        count: 1,
    };
    journal.commit_cauldron(
        KETTLE,
        brewing::CauldronState {
            recipe: Some(MIXED),
            water: 0,
            progress_seconds: 0.0,
        },
        kettle.clone(),
    )?;
    let heat = campfire::CampfireState {
        lit: true,
        burn_remaining: 240.0,
        burn_total: 240.0,
        ..default()
    };
    journal.commit_attached_campfire(
        KETTLE,
        campfire::AttachedCampfire {
            state: heat.clone(),
            inventory: Inventory {
                slots: vec![ItemStack::EMPTY; campfire::SLOTS],
                ..default()
            },
        },
    )?;
    let mut cooking = Inventory {
        slots: vec![ItemStack::EMPTY; campfire::SLOTS],
        ..default()
    };
    cooking.slots[campfire::INPUT] = ItemStack {
        item: campfire::recipes()
            .first()
            .ok_or("Missing campfire recipe")?
            .input,
        count: 2,
    };
    cooking.slots[campfire::FUEL] = ItemStack {
        item: ItemKind::Stick,
        count: 3,
    };
    journal.commit_campfire(FIRE, campfire::CampfireState::default(), cooking.clone())?;
    journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(ItemKind::WaterBottle, 1);
    terra_storage::save_edits(&path(), config.world.id, &journal)?;
    let loaded = terra_storage::load_edits(&path(), config.world.id)?;
    if loaded.container_at(KETTLE) != Some(&kettle)
        || loaded.container_at(FIRE) != Some(&cooking)
        || loaded.hearth_at(KETTLE) != Some(&heat)
        || !loaded.has_attached_campfire(KETTLE)
        || loaded.block([0, 16, 0]) != Some(3)
        || loaded.cauldron_at(KETTLE).and_then(|state| state.recipe) != Some(MIXED)
    {
        return Err("Alchemy save/reload lost station inputs, heat or selected recipe".into());
    }
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path()))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    requested: u8,
    captured: u8,
    blank: bool,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-alchemy") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        requested: 0,
        captured: 0,
        blank: false,
    })
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
    let stage = world.resource::<Inspection>().stage;
    let target = if stage >= 20 {
        Vec3::from_array(TUNNELS[usize::from(stage >= 22)].map(|v| v as f32)) + Vec3::splat(0.5)
    } else if (8..=12).contains(&stage) {
        Vec3::new(3.5, 17.4, 0.5)
    } else {
        Vec3::new(0.5, 17.65, 0.5)
    };
    let captured = !world.resource::<terra_ui::InventoryView>().open
        && !world.resource::<terra_player::PlayerInputBlock>().0;
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = captured;
    for mut transform in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        transform.look_at(target, Vec3::Y);
    }
}
fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    info!(stage, "Alchemy native stage");
}
fn element(world: &mut World, id: &str) -> Result<Entity, String> {
    world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, actual, node)| {
            (actual.0 == id && node.size().min_element() > 1.0).then_some(entity)
        })
        .ok_or_else(|| format!("Alchemy element {id} is not laid out"))
}
fn rendered_text(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity) {
        return text.0.clone();
    }
    world
        .get::<Children>(entity)
        .map_or_else(String::new, |children| {
            children
                .iter()
                .map(|child| rendered_text(world, child))
                .collect()
        })
}
fn event(world: &mut World, target: Entity, attribute: &str) -> Result<(), String> {
    let handler = world
        .get::<tilt_ui::StaticAttributes>(target)
        .and_then(|attributes| {
            attributes
                .attributes
                .iter()
                .find(|value| value.name == attribute)
        })
        .map(|value| value.value.clone())
        .ok_or("Alchemy widget has no event handler")?;
    world.write_message(tilt_ui::HtmlEvent {
        target,
        kind: if attribute == "onclick" {
            "click"
        } else {
            "mousedown"
        },
        value: None,
        submitter: None,
        data: [("button".into(), "Primary".into())].into(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn press_slot(world: &mut World, id: &str, index: usize) -> Result<(), String> {
    let root = element(world, id)?;
    fn descendants(world: &World, root: Entity, nodes: &mut Vec<Entity>) {
        if let Some(children) = world.get::<Children>(root) {
            for child in children {
                nodes.push(*child);
                descendants(world, *child, nodes);
            }
        }
    }
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
        .ok_or("Alchemy inventory slot is not materialized")?;
    event(world, target, "onmousedown")
}
fn validate_recipe_choice(world: &mut World) -> Result<(), String> {
    let choice = element(world, "cauldron-recipe")?;
    let parts = *world
        .get::<tilt_ui::ChoiceBoxParts>(choice)
        .ok_or("Cauldron recipe dropdown has no widget parts")?;
    let mut entries = Vec::new();
    for (entity, option, checked) in world
        .query::<(Entity, &tilt_ui::OptionData, &tilt_ui::ControlChecked)>()
        .iter(world)
    {
        let mut ancestor = Some(entity);
        while let Some(parent) = ancestor {
            if parent == parts.popup {
                entries.push((option.value.clone(), checked.0));
                break;
            }
            ancestor = world.get::<ChildOf>(parent).map(ChildOf::parent);
        }
    }
    entries.sort_unstable();
    let mut expected: Vec<_> = brewing::recipes()
        .iter()
        .map(|recipe| (recipe.id.to_string(), recipe.id == MIXED))
        .collect();
    expected.sort_unstable();
    if entries != expected
        || world.get::<Text>(parts.value).map(|text| text.0.as_str())
            != brewing::recipe(MIXED).map(|recipe| recipe.name.as_str())
    {
        return Err(format!(
            "Cauldron dropdown did not materialize its real selected popup options: {entries:?}"
        ));
    }
    Ok(())
}
fn capture(
    world: &mut World,
    state: &mut Inspection,
    name: &str,
    count: u8,
) -> Result<bool, String> {
    if state.requested < count {
        let previews = world.resource::<terra_render::RenderBlockPreviews>();
        if let Some(error) = &previews.error {
            return Err(error.clone());
        }
        if previews.pending != 0 {
            return Ok(false);
        }
        let folder = std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .ok_or("Set TERRA_CAPTURE_DIR for alchemy captures")?;
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(format!("{name}.png"))))
            .observe(
                |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                    state.blank |= event.image.data.as_ref().is_none_or(|data| {
                        data.chunks_exact(4)
                            .filter(|pixel| pixel[..3].iter().any(|value| *value > 20))
                            .count()
                            < 1000
                    });
                    state.captured += 1;
                },
            );
        state.requested = count;
    }
    Ok(state.captured >= count)
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
            error!(%error,stage=state.stage,"Alchemy native smoke failed");
            state.stage = 255;
            world.write_message(AppExit::error());
        }
    });
}
fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.start.elapsed() > Duration::from_secs(240) || state.blank {
        return Err(format!(
            "Alchemy fixture timeout or blank capture at stage {}; preview jobs remaining: {}",
            state.stage,
            world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending,
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
                    "/tp 0.5 17.91 1.5".into(),
                ));
            advance(state, 1);
        }
        1 if elapsed > 2.0
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|pose| pose.grounded && (pose.position[2] - 1.5).abs() < 0.2) =>
        {
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(KETTLE)
                != Some(brewing::MATERIAL)
            {
                return Ok(());
            }
            if !capture(world, state, "alchemy-world", 1)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::OpenChest(KETTLE));
            advance(state, 2);
        }
        2 if world.resource::<terra_ui::InventoryView>().cauldron.active && elapsed > 0.7 => {
            validate_recipe_choice(world)?;
            let view = world.resource::<terra_ui::InventoryView>();
            if view.container_items.len() != brewing::SLOTS
                || view.cauldron.recipe != MIXED.to_string()
                || view.cauldron.status != "No water"
            {
                return Err(
                    "Saved cauldron UI lost recipe, slots or empty-water requirement".into(),
                );
            }
            press_slot(world, "inventory-hotbar", 0)?;
            advance(state, 3);
        }
        3 if session
            .inventory
            .cursor
            .is_some_and(|stack| stack.item == ItemKind::WaterBottle) =>
        {
            press_slot(world, "cauldron-water", 0)?;
            advance(state, 4);
        }
        4 if session
            .container
            .as_ref()
            .and_then(|container| container.cauldron.as_ref())
            .is_some_and(|machine| machine.working && machine.progress > 0.01)
            && session.inventory.cursor.is_none() =>
        {
            if !capture(world, state, "cauldron-brewing", 2)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 16);
        }
        16 if session.container.is_none()
            && !world.resource::<terra_ui::InventoryView>().open
            && session
                .cauldrons
                .iter()
                .any(|bowl| bowl.position == KETTLE && bowl.heated && bowl.water > 0)
            && elapsed > 0.7 =>
        {
            if !capture(world, state, "compact-cauldron-boiling-a", 3)? {
                return Ok(());
            }
            advance(state, 17);
        }
        17 if elapsed > 0.25 => {
            if !capture(world, state, "compact-cauldron-boiling-b", 4)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::OpenChest(KETTLE));
            advance(state, 18);
        }
        18 if world.resource::<terra_ui::InventoryView>().cauldron.active && elapsed > 0.7 => {
            world.write_message(terra_ui::InventoryAction::Recipe(format!(
                "item-{}",
                ItemKind::Potion(MIXED)
                    .identifier()
                    .trim_start_matches("teramic::")
            )));
            advance(state, 5);
        }
        5 if world.resource::<terra_ui::CraftingView>().brewing && elapsed > 0.7 => {
            let view = world.resource::<terra_ui::CraftingView>();
            let recipe = brewing::recipe(MIXED).unwrap();
            if view.recipe_output.first().and_then(|slot| slot.item)
                != Some(ItemKind::Potion(MIXED))
                || recipe.effects.iter().any(|effect| {
                    !view
                        .description
                        .contains(&terra_core::effect_definition(effect.kind).name)
                })
            {
                return Err("Mixed potion TMI view lost individual effects or output".into());
            }
            if !capture(world, state, "mixed-potion-recipe", 5)? {
                return Ok(());
            }
            world.write_message(terra_ui::CraftingAction::CloseRecipe);
            advance(state, 6);
        }
        6 if session
            .container
            .as_ref()
            .and_then(|container| container.halves.first())
            .and_then(|half| half.inventory.slot(brewing::OUTPUT))
            .is_some_and(|stack| stack.item == ItemKind::Potion(MIXED)) =>
        {
            if !capture(world, state, "cauldron-potion-result", 6)? {
                return Ok(());
            }
            let button = element(world, "hearth-view-switch")?;
            event(world, button, "onclick")?;
            advance(state, 13);
        }
        13 if world.resource::<terra_ui::InventoryView>().campfire.active && elapsed > 0.7 => {
            let container = session
                .container
                .as_ref()
                .ok_or("Missing attached fire view")?;
            if container.anchor != KETTLE
                || container.view
                    != terra_network::protocol::inventory::ContainerView::AttachedCampfire
                || container.halves[0].inventory.slots.len() != campfire::SLOTS
                || !container.attached_campfire
            {
                return Err("Attached fire lost its one-cell owner or separate inventory".into());
            }
            if !capture(world, state, "compact-attached-fire-inventory", 7)? {
                return Ok(());
            }
            let button = element(world, "hearth-view-switch")?;
            event(world, button, "onclick")?;
            advance(state, 14);
        }
        14 if world.resource::<terra_ui::InventoryView>().cauldron.active && elapsed > 0.7 => {
            if session.container.as_ref().is_none_or(|container| {
                container.anchor != KETTLE
                    || container.halves[0].inventory.slots.len() != brewing::SLOTS
            }) {
                return Err("Returning from the fire lost the real cauldron inventory".into());
            }
            world.write_message(terra_ui::InventoryAction::Recipe(
                "item-poison_potion".into(),
            ));
            advance(state, 15);
        }
        15 if world.resource::<terra_ui::CraftingView>().brewing && elapsed > 0.7 => {
            let view = world.resource::<terra_ui::CraftingView>();
            if view.recipe_output.first().and_then(|slot| slot.item) != Some(ItemKind::Potion(7))
                || !view.description.contains(
                    &terra_core::effect_definition(terra_core::effects::EffectKind::Poison).name,
                )
            {
                return Err("Poison potion recipe or its actual debuff is missing from TMI".into());
            }
            if !capture(world, state, "poison-potion-recipe", 8)? {
                return Ok(());
            }
            world.write_message(terra_ui::CraftingAction::CloseRecipe);
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 7);
        }
        7 if session.container.is_none() && !world.resource::<terra_ui::InventoryView>().open => {
            world.write_message(terra_ui::InventoryAction::OpenChest(FIRE));
            advance(state, 8);
        }
        8 if world.resource::<terra_ui::InventoryView>().campfire.active && elapsed > 0.7 => {
            let view = world.resource::<terra_ui::InventoryView>();
            if view.container_items.len() != campfire::SLOTS
                || view.campfire.lit
                || !view.campfire.can_toggle
            {
                return Err("Cold campfire UI lost its fuel, slots or light control".into());
            }
            let fuel = element(world, "campfire-fuel")?;
            let meter = element(world, "campfire-fuel-level")?;
            let slot_height = world.get::<ComputedNode>(fuel).unwrap().size().y;
            let meter_height = world.get::<ComputedNode>(meter).unwrap().size().y;
            if (slot_height - meter_height).abs() > 1.0 {
                return Err("Campfire fuel meter does not fit its inventory slot".into());
            }
            let button = element(world, "campfire-light")?;
            event(world, button, "onclick")?;
            advance(state, 9);
        }
        9 if session
            .container
            .as_ref()
            .and_then(|container| container.campfire.as_ref())
            .is_some_and(|fire| fire.lit && fire.progress > 0.01) =>
        {
            if !capture(world, state, "campfire-roasting", 9)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 10);
        }
        10 if session.container.is_none()
            && !world.resource::<terra_ui::InventoryView>().open
            && session
                .campfires
                .iter()
                .any(|fire| fire.position == FIRE && fire.lit)
            && elapsed > 0.7 =>
        {
            if !capture(world, state, "campfire-burning-world", 10)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::OpenChest(FIRE));
            advance(state, 12);
        }
        12 if session
            .container
            .as_ref()
            .and_then(|container| container.halves.first())
            .and_then(|half| half.inventory.slot(campfire::OUTPUT))
            .is_some() =>
        {
            let button = element(world, "campfire-extinguish")?;
            event(world, button, "onclick")?;
            advance(state, 11);
        }
        11 if session
            .container
            .as_ref()
            .and_then(|container| container.campfire.as_ref())
            .is_some_and(|fire| !fire.lit)
            && elapsed > 0.7 =>
        {
            if !capture(world, state, "campfire-extinguished-result", 11)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Close);
            advance(state, 20);
        }
        20 | 22 if session.container.is_none() && elapsed > 0.7 => {
            world.write_message(terra_ui::InventoryAction::OpenChest(
                TUNNELS[usize::from(state.stage == 22)],
            ));
            advance(state, state.stage + 1);
        }
        21 | 23 if world.resource::<terra_ui::InventoryView>().tunnel_options && elapsed > 0.7 => {
            let filtered = state.stage == 23;
            let view = world.resource::<terra_ui::InventoryView>();
            let container = session
                .container
                .as_ref()
                .ok_or("Tunnel snapshot missing")?;
            if view.filtered_funnel != filtered || container.filter.is_some() != filtered {
                return Err("Only the filtered tunnel may expose a filter".into());
            }
            let label = element(world, "tunnel-random-label")?;
            if rendered_text(world, label) != "Random" {
                return Err("Tunnel random mode has no visible Random label".into());
            }
            element(world, "tunnel-random-extraction")?;
            if filtered {
                element(world, "funnel-filter-slot")?;
            } else if element(world, "funnel-filter-slot").is_ok() {
                return Err("Ordinary tunnel rendered a ghost filter slot".into());
            }
            if !capture(
                world,
                state,
                if filtered {
                    "filtered-tunnel-random"
                } else {
                    "tunnel-random"
                },
                if filtered { 13 } else { 12 },
            )? {
                return Ok(());
            }
            if !filtered {
                world.write_message(terra_ui::InventoryAction::Close);
                advance(state, 22);
                return Ok(());
            }
            info!(
                captures = state.captured,
                seconds = state.start.elapsed().as_secs_f32(),
                "Alchemy native smoke passed: compact saved cauldron/fire views, real water transfer, mixed/poison recipes, campfire cooking, Random labels on both tunnels"
            );
            state.stage = 255;
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
    fn direct_alchemy_setup_does_not_require_the_frontend_menu() {
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
