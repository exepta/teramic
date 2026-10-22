//! Native recipe UI, real server transactions, placement and binary save/reload.
use bevy::prelude::*;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{Config, GameConfig, GameMode, ItemKind, PlayerProfile, ToolKind};
use terra_network::client::ClientSession;

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.allow_flight = false;
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.water = false;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let profile = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .and_then(PlayerProfile::load_or_create)
        .map_err(|e| e.to_string())?;
    let root = directory();
    if root.join("world").exists() {
        return Err("Crafting smoke requires a fresh world directory".into());
    }
    let mut journal = terra_world::edits::EditJournal::default();
    let bag = journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default();
    bag.insert(ItemKind::Block(10), 8);
    bag.insert(ItemKind::Block(3), 12);
    bag.insert(ItemKind::Block(4), 12);
    terra_storage::save_edits(&root.join("world"), config.world.id, &journal)?;
    let (host, token) =
        terra_server::IntegratedServer::start_with_save(config, Some(root.join("world")))?;
    Ok((host, token, profile))
}
fn directory() -> PathBuf {
    std::env::var_os("TERRA_CRAFTING_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-crafting-{}", std::process::id()))
        })
}
#[derive(Resource)]
struct Fixture {
    start: Instant,
    entered: Instant,
    stage: u8,
    operation: usize,
    phase: u8,
    response: u64,
    bench: Option<[i64; 3]>,
    failure_started: Option<Instant>,
    page_probes: u8,
    hover_attempts: u8,
}
pub(crate) fn install(app: &mut App) {
    let now = Instant::now();
    app.insert_resource(Fixture {
        start: now,
        entered: now,
        stage: 0,
        operation: 0,
        phase: 0,
        response: 0,
        bench: None,
        failure_started: None,
        page_probes: 0,
        hover_attempts: 0,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
    let mut menu = app.world_mut().resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
}
fn advance(f: &mut Fixture) {
    f.stage += 1;
    f.entered = Instant::now();
    f.operation = 0;
    f.phase = 0;
    f.page_probes = 0;
    f.hover_attempts = 0;
    info!(stage = f.stage, "Crafting smoke advanced");
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let view = world.resource::<terra_ui::InventoryView>();
    info!(capture=name, open=view.open, visible=view.visible, assembly=view.assembly, container=view.container,
        browser_items=view.browser_items.len(), hotbar=view.hotbar.len(), items=view.items.len(),
        menu=?world.resource::<terra_ui::menu::MenuState>().page,
        blocked=world.resource::<terra_player::PlayerInputBlock>().0,
        "Crafting UI resource state");
    for (id, node, transform) in world
        .query::<(
            &tilt_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .filter(|(id, _, _)| {
            matches!(
                id.0.as_str(),
                "inventory-backdrop" | "inventory-shell" | "inventory-panel" | "item-browser"
            )
        })
    {
        info!(capture=name, element=id.0, size=?node.size(), center=?transform.translation, "Inventory frame bounds");
    }
    for (id, classes, node, background) in world
        .query::<(
            &tilt_ui::ElementId,
            Option<&tilt_ui::ElementClasses>,
            &ComputedNode,
            Option<&BackgroundColor>,
        )>()
        .iter(world)
        .filter(|(id, _, _, _)| id.0 == "inventory-cell")
        .take(3)
    {
        info!(capture = name, element = id.0, ?classes, size = ?node.size(), ?background, "Crafting slot layout");
    }
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory().join("captures"));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            root.join(format!("{name}.png")),
        ));
    Ok(())
}
fn diagnose_ui(world: &mut World, label: &str) {
    let mut ids = std::collections::BTreeMap::<String, usize>::new();
    for id in world.query::<&tilt_ui::ElementId>().iter(world) {
        *ids.entry(id.0.clone()).or_default() += 1;
    }
    warn!(reason=label, ids=?ids, "Crafting diagnostic TiltUI element IDs");
    let relevant: Vec<_> = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .filter(|(_, id)| {
            matches!(
                id.0.as_str(),
                "inventory-cell"
                    | "inventory-panel"
                    | "inventory-browser-grid"
                    | "item-browser"
                    | "inventory-hud-hotbar"
            )
        })
        .map(|(entity, _)| entity)
        .take(6)
        .collect();
    for entity in relevant {
        let mut chain = Vec::new();
        let mut current = Some(entity);
        for _ in 0..16 {
            let Some(entity) = current else {
                break;
            };
            chain.push(format!(
                "{entity:?} id={:?} classes={:?} size={:?} display={:?}",
                world.get::<tilt_ui::ElementId>(entity).map(|id| &id.0),
                world.get::<tilt_ui::ElementClasses>(entity),
                world.get::<ComputedNode>(entity).map(ComputedNode::size),
                world.get::<Node>(entity).map(|node| node.display),
            ));
            current = world.get::<ChildOf>(entity).map(ChildOf::parent);
        }
        warn!(reason=label, ancestry=?chain, "Crafting diagnostic element ancestry");
    }
}
fn descendants(world: &World, root: Entity, output: &mut Vec<Entity>) {
    if let Some(children) = world.get::<Children>(root) {
        for child in children {
            output.push(*child);
            descendants(world, *child, output);
        }
    }
}
fn slot_target(world: &mut World, root: &str, alt: Option<&str>) -> Result<Entity, String> {
    let root_entity = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == root).then_some(entity));
    let Some(root_entity) = root_entity else {
        diagnose_ui(world, root);
        return Err(format!("Missing TiltUI root {root}"));
    };
    let mut nodes = Vec::new();
    descendants(world, root_entity, &mut nodes);
    nodes
        .into_iter()
        .find(|entity| {
            if world
                .get::<tilt_ui::ElementId>(*entity)
                .is_none_or(|id| id.0 != "inventory-cell")
            {
                return false;
            }
            alt.is_none_or(|alt| {
                let mut images = Vec::new();
                descendants(world, *entity, &mut images);
                images.into_iter().any(|image| {
                    world
                        .get::<tilt_ui::ImageMetadata>(image)
                        .and_then(|m| m.alt.as_deref())
                        == Some(alt)
                })
            })
        })
        .ok_or_else(|| format!("Missing TiltUI slot {alt:?}"))
}
fn press_slot(world: &mut World, root: &str, alt: Option<&str>) -> Result<(), String> {
    let entity = slot_target(world, root, alt)?;
    press_entity(world, entity)
}
fn press_slot_at(world: &mut World, root: &str, index: usize) -> Result<(), String> {
    let root_entity = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == root).then_some(entity))
        .ok_or_else(|| format!("Missing TiltUI root {root}"))?;
    let mut nodes = Vec::new();
    descendants(world, root_entity, &mut nodes);
    let entity = nodes
        .into_iter()
        .filter(|entity| {
            world
                .get::<tilt_ui::ElementId>(*entity)
                .is_some_and(|id| id.0 == "inventory-cell")
        })
        .nth(index)
        .ok_or_else(|| format!("Missing TiltUI slot {root}[{index}]"))?;
    press_entity(world, entity)
}
fn press_entity(world: &mut World, entity: Entity) -> Result<(), String> {
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|a| a.attributes.iter().find(|a| a.name == "onmousedown"))
        .map(|a| a.value.clone())
        .ok_or("Missing slot pointer handler")?;
    world.write_message(tilt_ui::HtmlEvent {
        target: entity,
        kind: "mousedown",
        value: None,
        submitter: None,
        data: [("button".into(), "Primary".into())].into(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn craft(
    world: &mut World,
    f: &mut Fixture,
    operations: &[(&str, ItemKind, u16, bool)],
) -> Result<bool, String> {
    let Some((name, item, expected, _maximum)) = operations.get(f.operation) else {
        return Ok(true);
    };
    if f.phase == 5 {
        if f.entered.elapsed() < Duration::from_millis(350) {
            return Ok(false);
        }
        capture(
            world,
            &format!(
                "crafting-browser-for-{}",
                name.to_lowercase().replace(' ', "-")
            ),
        )?;
        f.phase = 0;
    }
    if f.phase == 0 {
        let recipe = world
            .resource::<terra_core::crafting::RecipeBook>()
            .iter()
            .find(|r| r.name == *name)
            .ok_or_else(|| format!("Recipe {name} missing"))?;
        let alt = match recipe.output.item {
            ItemKind::Block(id) => format!("Block preview {id}"),
            item => format!("Item preview {}", item.identifier()),
        };
        if !world
            .resource::<terra_ui::InventoryView>()
            .browser_items
            .iter()
            .any(|item| item.preview_key == alt)
        {
            if f.page_probes >= 8 {
                return Err(format!(
                    "Recipe item {alt} was not found after bounded catalog paging"
                ));
            }
            let view = world.resource::<terra_ui::InventoryView>();
            let id = if view.browser_next {
                "inventory-page-next"
            } else if view.browser_previous {
                "inventory-page-previous"
            } else {
                return Err(format!("Recipe item {alt} is missing from the catalog"));
            };
            info!(recipe = *name, page = %view.browser_page, button = id, "Navigating real TiltUI catalog pagination");
            let button = world
                .query::<(Entity, &tilt_ui::ElementId)>()
                .iter(world)
                .find_map(|(entity, element)| (element.0 == id).then_some(entity))
                .ok_or("Missing catalog pagination button")?;
            let handler = world
                .get::<tilt_ui::StaticAttributes>(button)
                .and_then(|attributes| {
                    attributes
                        .attributes
                        .iter()
                        .find(|attribute| attribute.name == "onclick")
                })
                .map(|attribute| attribute.value.clone())
                .ok_or("Missing catalog pagination handler")?;
            world.write_message(tilt_ui::HtmlEvent {
                target: button,
                kind: "click",
                value: None,
                submitter: None,
                data: default(),
                form_data: None,
                handler,
            });
            f.page_probes += 1;
            f.phase = 5;
            f.entered = Instant::now();
            return Ok(false);
        }
        f.page_probes = 0;
        press_slot(world, "inventory-browser-grid", Some(&alt))?;
        f.phase = 1;
        f.entered = Instant::now();
    } else if f.phase == 1 && f.entered.elapsed() > Duration::from_millis(300) {
        let view = world.resource::<terra_ui::CraftingView>();
        if view.name != *name {
            return Err(format!(
                "Recipe selection failed: expected {name}, got {}",
                view.name
            ));
        }
        if !view.can_apply_recipe || view.enabled {
            return Err(format!("Recipe disabled: {}", view.message));
        }
        if *name == "Chest" {
            capture(world, "chest-recipe")?;
        }
        let button = world
            .query::<(Entity, &tilt_ui::ElementId)>()
            .iter(world)
            .find_map(|(entity, id)| (id.0 == "craft-apply-recipe").then_some(entity))
            .ok_or("Missing recipe add button")?;
        let handler = world
            .get::<tilt_ui::StaticAttributes>(button)
            .and_then(|a| a.attributes.iter().find(|a| a.name == "onclick"))
            .map(|a| a.value.clone())
            .ok_or("Missing recipe add handler")?;
        world.write_message(tilt_ui::HtmlEvent {
            target: button,
            kind: "click",
            value: None,
            submitter: None,
            data: default(),
            form_data: None,
            handler,
        });
        f.phase = 3;
        f.entered = Instant::now();
    } else if (f.phase == 3 && f.entered.elapsed() > Duration::from_millis(250))
        || (f.phase == 4 && f.entered.elapsed() > Duration::from_millis(700))
    {
        if world.resource::<terra_ui::CraftingView>().recipe_open {
            return Err("Adding the recipe did not close its popup".into());
        }
        let root = if world.resource::<terra_ui::CraftingView>().workbench {
            "crafting-output"
        } else {
            "inventory-craft-result"
        };
        if *name == "Chest" {
            if std::env::var_os("TERRA_NATIVE_OS_POINTER").is_some() {
                let target = slot_target(world, root, None)?;
                if f.phase == 3 {
                    crate::inventory_smoke::pointer(world, target, None)?;
                    f.hover_attempts = 1;
                    f.phase = 4;
                    f.entered = Instant::now();
                    return Ok(false);
                }
                let center = world
                    .get::<bevy::ui::UiGlobalTransform>(target)
                    .ok_or("Missing output hover transform")?
                    .translation;
                let size = world
                    .get::<ComputedNode>(target)
                    .ok_or("Missing output hover geometry")?
                    .size();
                let pointer = world
                    .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .and_then(Window::physical_cursor_position);
                if pointer.is_none_or(|pointer| {
                    !Rect::from_center_size(center, size * 0.8).contains(pointer)
                }) {
                    if f.hover_attempts >= 4 {
                        return Err(format!(
                            "Native pointer did not reach the current workbench output: pointer={pointer:?}, center={center:?}, size={size:?}"
                        ));
                    }
                    info!(
                        ?pointer,
                        ?center,
                        ?size,
                        attempts = f.hover_attempts,
                        "Repositioning real pointer after workbench output layout settled"
                    );
                    crate::inventory_smoke::pointer(world, target, None)?;
                    f.hover_attempts += 1;
                    f.entered = Instant::now();
                    return Ok(false);
                }
                crate::inventory_smoke::require_tooltip(world, target, "teramic::chest")?;
                capture(world, "workbench-tooltip")?;
            }
            capture(world, "workbench-valid-pattern")?;
        }
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        press_slot(world, root, None)?;
        f.response = world.resource::<ClientSession>().inventory_response;
        f.phase = 2;
        f.entered = Instant::now();
    } else if f.phase == 2
        && f.entered.elapsed() > Duration::from_millis(350)
        && world.resource::<ClientSession>().inventory_response != f.response
    {
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ShiftLeft);
        let count = world.resource::<ClientSession>().inventory.count(*item);
        if world.resource::<ClientSession>().last_inventory
            != Some(terra_network::protocol::inventory::InventoryOutcome::Applied)
        {
            return Err("Craft request was rejected".into());
        }
        if count == *expected {
            f.operation += 1;
        }
        if count <= *expected {
            f.phase = 0;
            f.entered = Instant::now();
        } else {
            return Err(format!("Crafted too many {name}: {count}/{expected}"));
        }
    }
    Ok(false)
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut f: Mut<Fixture>| {
        if let Some(started) = f.failure_started {
            if started.elapsed() > Duration::from_secs(2) {
                f.failure_started = None;
                world.write_message(AppExit::error());
            }
            return;
        }
        if f.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut f) {
            error!(%error, stage=f.stage, "Crafting native smoke failed");
            diagnose_ui(world, &error);
            let _ = capture(world, &format!("crafting-failure-stage-{}", f.stage));
            f.stage = 255;
            f.failure_started = Some(Instant::now());
        }
    });
}
fn step(world: &mut World, f: &mut Fixture) -> Result<(), String> {
    if f.start.elapsed() > Duration::from_secs(150) {
        return Err("Crafting smoke timed out".into());
    }
    if let Some(error) = &world.resource::<ClientSession>().error {
        return Err(error.clone());
    }
    if let Some(error) = &world.resource::<terra_render::RenderStatus>().error {
        return Err(error.clone());
    }
    let elapsed = f.entered.elapsed();
    let stage = f.stage;
    match stage {
        0 if world.resource::<ClientSession>().ready
            && world.resource::<terra_render::RenderStatus>().ready
            && world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending
                == 0
            && world
                .resource::<terra_render::RenderBlockPreviews>()
                .images
                .contains_key(&29)
            && world
                .resource::<terra_render::RenderItemPreviews>()
                .images
                .len()
                >= 9 =>
        {
            world.write_message(terra_ui::InventoryAction::Toggle);
            advance(f);
        }
        1 if elapsed > Duration::from_millis(700) => {
            if f.phase == 0 {
                let mounted = world
                    .query::<&tilt_ui::ElementId>()
                    .iter(world)
                    .any(|id| id.0 == "inventory-browser-grid");
                if !mounted {
                    if elapsed < Duration::from_secs(3) {
                        return Ok(());
                    }
                    return Err(
                        "Inventory browser did not mount after real inventory opening".into(),
                    );
                }
                capture(world, "inventory-large-slots")?;
                press_slot(
                    world,
                    "inventory-browser-grid",
                    Some("Item preview teramic::stone_pickaxe"),
                )?;
                f.phase = 1;
                f.entered = Instant::now();
            } else {
                let view = world.resource::<terra_ui::CraftingView>();
                if !view.recipe_open || !view.missing || view.enabled {
                    return Err("Missing ingredients or wrong station not represented by the recipe preview".into());
                }
                let filtered = world
                    .query::<&tilt_ui::RuntimeComputedStyle>()
                    .iter(world)
                    .any(|style| {
                        style.0.animated_effects.as_ref().is_some_and(|effects| {
                            effects.iter().any(|effect| {
                                effect.kind == tilt_ui::tilt_ui_css::AnimatedEffectKind::SignalLost
                            })
                        })
                    });
                if !filtered {
                    return Err(
                        "Missing recipe images have no real TiltUI distortion filter".into(),
                    );
                }
                capture(world, "missing-recipe-filter")?;
                world.write_message(terra_ui::CraftingAction::CloseRecipe);
                advance(f);
            }
        }
        2 if elapsed > Duration::from_millis(400)
            && craft(
                world,
                f,
                &[
                    ("Birch Planks", ItemKind::Block(17), 32, true),
                    ("Sticks", ItemKind::Stick, 4, false),
                    (
                        "Workbench",
                        ItemKind::for_wood(28, terra_core::WoodSpecies::Birch),
                        1,
                        false,
                    ),
                ],
            )? =>
        {
            capture(world, "hand-crafting")?;
            advance(f);
        }
        3 => {
            let slot = world
                .resource::<ClientSession>()
                .inventory
                .slots
                .iter()
                .position(|s| {
                    s.item == ItemKind::for_wood(28, terra_core::WoodSpecies::Birch) && s.count > 0
                })
                .ok_or("Crafted bench missing")?;
            world.write_message(terra_ui::InventoryAction::Hotbar(slot as u8));
            world.write_message(terra_ui::InventoryAction::Close);
            for mut camera in world
                .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -0.65, 0.0);
            }
            advance(f);
        }
        4 if elapsed > Duration::from_millis(700) => {
            let hit = world
                .resource::<terra_player::TargetBlock>()
                .0
                .ok_or("No placement target")?;
            let (position, expected_material) = terra_world::targeting::block_placement(
                world.resource::<terra_world::streaming::ChunkStore>(),
                hit,
            )
            .ok_or("No workbench placement")?;
            f.bench = Some(position);
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material,
                material: 28,
            });
            advance(f);
        }
        5 if f.bench.is_some_and(|p| {
            world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at(p)
                == Some(28)
        }) =>
        {
            if world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::for_wood(28, terra_core::WoodSpecies::Birch))
                != 0
            {
                return Ok(());
            }
            capture(world, "placed-workbench")?;
            world.write_message(terra_ui::InventoryAction::OpenWorkbench(
                f.bench.ok_or("Missing bench")?,
            ));
            advance(f);
        }
        6 if elapsed > Duration::from_millis(500) => {
            if world.resource::<terra_ui::CraftingView>().station != "Workbench" {
                return Err("Workbench station unavailable".into());
            }
            if craft(
                world,
                f,
                &[
                    (
                        "Wood Pickaxe",
                        ItemKind::Tool(ToolKind::WoodPickaxe),
                        1,
                        false,
                    ),
                    (
                        "Stone Pickaxe",
                        ItemKind::Tool(ToolKind::StonePickaxe),
                        1,
                        false,
                    ),
                    ("Sticks", ItemKind::Stick, 4, false),
                    ("Stone Axe", ItemKind::Tool(ToolKind::StoneAxe), 1, false),
                    (
                        "Stone Shovel",
                        ItemKind::Tool(ToolKind::StoneShovel),
                        1,
                        false,
                    ),
                    (
                        "Chest",
                        ItemKind::for_wood(29, terra_core::WoodSpecies::Birch),
                        2,
                        true,
                    ),
                ],
            )? {
                capture(world, "workbench-recipes")?;
                advance(f);
            }
        }
        7 if elapsed > Duration::from_millis(500) => {
            for mut window in world
                .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
                .iter_mut(world)
            {
                window.resolution.set(960.0, 640.0);
            }
            advance(f);
        }
        8 if elapsed > Duration::from_millis(900) => {
            capture(world, "crafting-compact")?;
            advance(f);
        }
        9 if elapsed > Duration::from_millis(900) => {
            world.write_message(terra_ui::InventoryAction::Assembly(false));
            advance(f);
        }
        10 if elapsed > Duration::from_millis(700) => {
            capture(world, "crafted-tools-inventory")?;
            advance(f);
        }
        11 if elapsed > Duration::from_millis(500) => {
            world.write_message(terra_ui::InventoryAction::Close);
            advance(f);
        }
        12 if elapsed > Duration::from_millis(700) => {
            capture(world, "large-hotbar")?;
            advance(f);
        }
        13 if elapsed > Duration::from_millis(700) => {
            let slot = world
                .resource::<ClientSession>()
                .inventory
                .slots
                .iter()
                .position(|s| {
                    s.item == ItemKind::for_wood(29, terra_core::WoodSpecies::Birch) && s.count == 2
                })
                .ok_or("Two crafted chests missing")?;
            world.write_message(terra_ui::InventoryAction::Hotbar(slot as u8));
            look_at_chest(world, f, false)?;
            advance(f);
        }
        14 if elapsed > Duration::from_millis(700) => {
            place_chest(world, f, false)?;
            advance(f);
        }
        15 if elapsed > Duration::from_millis(900)
            && world
                .resource::<terra_world::edits::EditJournal>()
                .object_state(chest_position(f, false)?)
                .is_some() =>
        {
            match f.phase {
                0 => {
                    capture(world, "single-chest")?;
                    for mut window in world
                        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
                        .iter_mut(world)
                    {
                        window.resolution.set(1920.0, 1080.0);
                    }
                    world.write_message(terra_ui::InventoryAction::OpenChest(chest_position(
                        f, false,
                    )?));
                    f.phase = 1;
                    f.entered = Instant::now();
                }
                1 if world
                    .resource::<ClientSession>()
                    .container
                    .as_ref()
                    .is_some_and(|view| view.halves.len() == 1)
                    && world
                        .resource::<terra_ui::InventoryView>()
                        .container_items
                        .len()
                        == terra_core::containers::CHEST_SLOTS =>
                {
                    capture(world, "single-chest-inventory")?;
                    f.response = world.resource::<ClientSession>().inventory_response;
                    press_slot(world, "inventory-hotbar", Some("Block preview 4"))?;
                    f.phase = 2;
                    f.entered = Instant::now();
                }
                2 if world.resource::<ClientSession>().inventory_response != f.response => {
                    let held = world.resource::<ClientSession>().inventory.cursor;
                    if held
                        != Some(terra_core::ItemStack {
                            item: ItemKind::Block(4),
                            count: 12,
                        })
                    {
                        return Err(format!(
                            "Single chest did not take the exact Sand stack: {held:?}"
                        ));
                    }
                    f.response = world.resource::<ClientSession>().inventory_response;
                    press_slot_at(world, "container-grid", 0)?;
                    f.phase = 3;
                    f.entered = Instant::now();
                }
                3 if world.resource::<ClientSession>().inventory_response != f.response => {
                    assert_chest_contents(world, f, false)?;
                    capture(world, "single-chest-inventory-filled")?;
                    world.write_message(terra_ui::InventoryAction::Close);
                    f.phase = 4;
                    f.entered = Instant::now();
                }
                4 if world.resource::<ClientSession>().container.is_none()
                    && !world.resource::<terra_ui::InventoryView>().open =>
                {
                    advance(f)
                }
                _ => {}
            }
        }
        16 if elapsed > Duration::from_millis(700) => {
            look_at_chest(world, f, true)?;
            advance(f);
        }
        17 if elapsed > Duration::from_millis(700) => {
            place_chest(world, f, true)?;
            advance(f);
        }
        18 if elapsed > Duration::from_millis(900)
            && world
                .resource::<terra_world::edits::EditJournal>()
                .object_state(chest_position(f, false)?)
                .is_some_and(|state| state.peer.is_some()) =>
        {
            match f.phase {
                0 => {
                    capture(world, "double-chest")?;
                    world.write_message(terra_ui::InventoryAction::OpenChest(chest_position(
                        f, true,
                    )?));
                    f.phase = 1;
                    f.entered = Instant::now();
                }
                1 if world
                    .resource::<ClientSession>()
                    .container
                    .as_ref()
                    .is_some_and(|view| view.halves.len() == 2)
                    && world
                        .resource::<terra_ui::InventoryView>()
                        .container_items
                        .len()
                        == terra_core::containers::CHEST_SLOTS * 2 =>
                {
                    require_unclipped_double_chest(world)?;
                    let view = world
                        .resource::<ClientSession>()
                        .container
                        .as_ref()
                        .ok_or("Missing double chest snapshot")?;
                    if view.anchor != chest_position(f, false)?
                        || view.halves[0].position != chest_position(f, false)?
                        || view.halves[1].position != chest_position(f, true)?
                        || view.halves[0].inventory.count(ItemKind::Block(4)) != 12
                        || view.halves[1].inventory.count(ItemKind::Block(4)) != 0
                    {
                        return Err("Connecting a chest changed its physical contents or canonical ordering".into());
                    }
                    capture(world, "double-chest-inventory")?;
                    f.response = world.resource::<ClientSession>().inventory_response;
                    press_slot_at(world, "container-grid", 0)?;
                    f.phase = 2;
                    f.entered = Instant::now();
                }
                2 if world.resource::<ClientSession>().inventory_response != f.response => {
                    if world.resource::<ClientSession>().inventory.cursor
                        != Some(terra_core::ItemStack {
                            item: ItemKind::Block(4),
                            count: 12,
                        })
                    {
                        return Err(
                            "Double chest failed to pick its primary stack through TiltUI".into(),
                        );
                    }
                    f.response = world.resource::<ClientSession>().inventory_response;
                    press_slot_at(world, "container-grid", terra_core::containers::CHEST_SLOTS)?;
                    f.phase = 3;
                    f.entered = Instant::now();
                }
                3 if world.resource::<ClientSession>().inventory_response != f.response => {
                    assert_chest_contents(world, f, true)?;
                    capture(world, "double-chest-inventory-filled")?;
                    world.write_message(terra_ui::InventoryAction::Close);
                    f.phase = 4;
                    f.entered = Instant::now();
                }
                4 if world.resource::<ClientSession>().container.is_none()
                    && !world.resource::<terra_ui::InventoryView>().open =>
                {
                    advance(f)
                }
                _ => {}
            }
        }
        19 if elapsed > Duration::from_millis(400) => {
            match f.phase {
                0 => {
                    world.write_message(terra_ui::InventoryAction::OpenWorkbench(
                        f.bench.ok_or("Missing linked workbench")?,
                    ));
                    f.phase = 1;
                    f.entered = Instant::now();
                }
                1 if world.resource::<terra_ui::InventoryView>().linked_container
                    && world.resource::<terra_ui::CraftingView>().workbench =>
                {
                    let snapshot = world
                        .resource::<ClientSession>()
                        .container
                        .as_ref()
                        .ok_or("Linked chest snapshot missing")?;
                    if snapshot.workbench != f.bench
                        || snapshot.halves.len() != 2
                        || snapshot.halves[1].inventory.count(ItemKind::Block(4)) != 12
                    {
                        return Err(
                            "Workbench did not expose the existing physical double chest".into(),
                        );
                    }
                    require_linked_container_layout(world)?;
                    capture(world, "workbench-linked-double-chest")?;
                    world.write_message(terra_ui::CraftingAction::Select(
                        terra_core::crafting::recipe_key("teramic::workbench"),
                    ));
                    f.phase = 2;
                    f.entered = Instant::now();
                }
                2 if world.resource::<terra_ui::CraftingView>().can_apply_recipe => {
                    world.write_message(terra_ui::CraftingAction::ApplyRecipe);
                    f.phase = 3;
                    f.entered = Instant::now();
                }
                3 if world
                    .resource::<terra_ui::CraftingView>()
                    .grid
                    .iter()
                    .filter(|slot| !slot.empty && slot.count == "1")
                    .count()
                    == 4 =>
                {
                    let count = world
                        .resource::<ClientSession>()
                        .inventory
                        .count(ItemKind::Block(17));
                    if count != 5 {
                        return Err(format!(
                            "Expected five remaining Birch planks before reserved transfer, got {count}"
                        ));
                    }
                    f.operation = usize::from(count);
                    f.response = world.resource::<ClientSession>().inventory_response;
                    world
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .press(KeyCode::ShiftLeft);
                    press_slot(world, "inventory-hotbar", Some("Block preview 17"))?;
                    f.phase = 4;
                    f.entered = Instant::now();
                }
                4 if world.resource::<ClientSession>().inventory_response != f.response => {
                    world
                        .resource_mut::<ButtonInput<KeyCode>>()
                        .release(KeyCode::ShiftLeft);
                    let session = world.resource::<ClientSession>();
                    let transferred: u16 = session
                        .container
                        .as_ref()
                        .ok_or("Linked transfer closed the chest")?
                        .halves
                        .iter()
                        .map(|half| half.inventory.count(ItemKind::Block(17)))
                        .sum();
                    if session.last_inventory
                        != Some(terra_network::protocol::inventory::InventoryOutcome::Applied)
                        || usize::from(session.inventory.count(ItemKind::Block(17)))
                            != f.operation - 1
                        || transferred != 1
                        || session.inventory.cursor.is_some()
                        || world
                            .resource::<terra_ui::CraftingView>()
                            .grid
                            .iter()
                            .filter(|slot| !slot.empty && slot.count == "1")
                            .count()
                            != 4
                    {
                        return Err("Linked chest transfer moved or duplicated reserved crafting ingredients".into());
                    }
                    capture(world, "workbench-linked-chest-reserved-transfer")?;
                    world.write_message(terra_ui::CraftingAction::Clear);
                    world.write_message(terra_ui::InventoryAction::Close);
                    f.phase = 5;
                    f.entered = Instant::now();
                }
                5 if world.resource::<ClientSession>().container.is_none()
                    && !world.resource::<terra_ui::InventoryView>().open =>
                {
                    advance(f)
                }
                _ => {}
            }
        }
        20 if elapsed > Duration::from_millis(900) => {
            let profile = world
                .resource::<ClientSession>()
                .uuid
                .ok_or("Missing profile")?;
            let id = world.resource::<GameConfig>().world.id;
            let host = world
                .remove_resource::<terra_server::IntegratedServer>()
                .ok_or("Missing host")?;
            host.finish()?;
            let loaded = terra_storage::load_edits(&directory().join("world"), id)?;
            let bag = loaded
                .gameplay()
                .inventories
                .get(&profile)
                .ok_or("Saved inventory missing")?;
            if bag.count(ItemKind::for_wood(29, terra_core::WoodSpecies::Birch)) != 0
                || bag.count(ItemKind::Tool(ToolKind::StonePickaxe)) != 1
                || bag.count(ItemKind::Block(4)) != 0
                || bag.cursor.is_some()
                || loaded
                    .container_at(chest_position(f, false)?)
                    .is_none_or(|inventory| inventory.count(ItemKind::Block(4)) != 0)
                || loaded
                    .container_at(chest_position(f, true)?)
                    .is_none_or(|inventory| inventory.count(ItemKind::Block(4)) != 12)
                || loaded
                    .object_state(chest_position(f, false)?)
                    .and_then(|s| s.peer)
                    != Some(chest_position(f, true)?)
                || loaded
                    .object_state(chest_position(f, false)?)
                    .is_none_or(|s| s.wood != terra_core::WoodSpecies::Birch)
                || loaded
                    .object_state(f.bench.ok_or("Missing saved bench")?)
                    .is_none_or(|s| s.wood != terra_core::WoodSpecies::Birch)
                || loaded.workbench_chest(f.bench.ok_or("Missing saved linked bench")?)
                    != Some((
                        f.bench.ok_or("Missing saved bench")?,
                        chest_position(f, false)?,
                    ))
                || bag.count(ItemKind::Block(17)) != 4
                || loaded
                    .container_at(chest_position(f, false)?)
                    .is_none_or(|inventory| inventory.count(ItemKind::Block(17)) != 1)
            {
                return Err("Crafted items did not survive binary save/reload".into());
            }
            info!(
                "Crafting native smoke passed: sidebar recipes, reserved 2x2/3x3 grids, four-plank shapeless workbench, four tools, unclipped double chest, linked chest layout and reserved TiltUI transfers, compact layout and REG5 binary save/reload"
            );
            f.stage = 255;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

fn assert_chest_contents(world: &World, f: &Fixture, double: bool) -> Result<(), String> {
    let session = world.resource::<ClientSession>();
    let view = session.container.as_ref().ok_or("Missing chest contents")?;
    if session.last_inventory != Some(terra_network::protocol::inventory::InventoryOutcome::Applied)
        || session.inventory.cursor.is_some()
        || session.inventory.count(ItemKind::Block(4)) != 0
        || view.anchor != chest_position(f, false)?
        || view.halves.len() != if double { 2 } else { 1 }
        || view.halves[0].inventory.count(ItemKind::Block(4)) != if double { 0 } else { 12 }
        || (double && view.halves[1].inventory.count(ItemKind::Block(4)) != 12)
    {
        return Err("TiltUI chest transfer did not conserve the exact canonical Sand stack".into());
    }
    Ok(())
}

fn element_frame(world: &mut World, id: &str) -> Result<(Entity, Rect), String> {
    world
        .query::<(
            Entity,
            &tilt_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .find_map(|(entity, name, node, transform)| {
            (name.0 == id).then_some((
                entity,
                Rect::from_center_size(transform.translation, node.size()),
            ))
        })
        .ok_or_else(|| format!("Missing native layout element {id}"))
}
fn slots_within(world: &mut World, id: &str, bounds: Rect, expected: usize) -> Result<(), String> {
    let (root, _) = element_frame(world, id)?;
    let mut nodes = Vec::new();
    descendants(world, root, &mut nodes);
    let mut count = 0;
    for entity in nodes {
        if world
            .get::<tilt_ui::ElementId>(entity)
            .is_none_or(|id| id.0 != "inventory-cell")
        {
            continue;
        }
        let node = world
            .get::<ComputedNode>(entity)
            .ok_or("Slot has no native bounds")?;
        let transform = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Slot has no native transform")?;
        let rect = Rect::from_center_size(transform.translation, node.size());
        if node.size().min_element() < 20.0
            || rect.min.cmplt(bounds.min - Vec2::ONE).any()
            || rect.max.cmpgt(bounds.max + Vec2::ONE).any()
        {
            return Err(format!(
                "{id} slot {count} is clipped: {rect:?} outside {bounds:?}"
            ));
        }
        count += 1;
    }
    if count != expected {
        return Err(format!("{id} materialized {count}/{expected} slots"));
    }
    Ok(())
}
fn require_unclipped_double_chest(world: &mut World) -> Result<(), String> {
    let (_, bounds) = element_frame(world, "inventory-scroll")?;
    slots_within(world, "container-grid", bounds, 36)?;
    slots_within(world, "inventory-grid", bounds, 18)?;
    slots_within(world, "inventory-hotbar", bounds, 9)
}
fn require_linked_container_layout(world: &mut World) -> Result<(), String> {
    let (_, panel) = element_frame(world, "inventory-panel")?;
    let (_, storage) = element_frame(world, "workbench-storage")?;
    let size = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| {
            Vec2::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
            )
        })
        .ok_or("Missing native window")?;
    if storage.max.x > panel.min.x - 4.0
        || storage.min.cmplt(Vec2::ZERO).any()
        || storage.max.cmpgt(size).any()
        || (panel.center().x - size.x * 0.5).abs() > 2.0
    {
        return Err(format!(
            "Linked chest is not a separate visible left panel: {storage:?}, bench {panel:?}, window {size:?}"
        ));
    }
    slots_within(world, "workbench-container-grid", storage, 36)?;
    let (backpack, _) = element_frame(world, "inventory-grid")?;
    let (linked, _) = element_frame(world, "workbench-container-grid")?;
    let slot_size = |world: &World, root| {
        let mut nodes = Vec::new();
        descendants(world, root, &mut nodes);
        nodes.into_iter().find_map(|entity| {
            if world.get::<tilt_ui::ElementId>(entity)?.0 == "inventory-cell" {
                world.get::<ComputedNode>(entity).map(ComputedNode::size)
            } else {
                None
            }
        })
    };
    let normal = slot_size(world, backpack).ok_or("Missing normal inventory slot geometry")?;
    let connected = slot_size(world, linked).ok_or("Missing connected chest slot geometry")?;
    if (normal - connected).abs().max_element() > 1.1 {
        return Err(format!(
            "Connected chest slots must match the inventory: chest={connected:?}, inventory={normal:?}"
        ));
    }
    Ok(())
}

fn chest_position(f: &Fixture, second: bool) -> Result<[i64; 3], String> {
    let bench = f.bench.ok_or("Missing bench")?;
    Ok([bench[0] + i64::from(second), bench[1], bench[2] + 1])
}
fn look_at_chest(world: &mut World, f: &Fixture, second: bool) -> Result<(), String> {
    let p = chest_position(f, second)?;
    let target = Vec3::new(p[0] as f32 + 0.5, p[1] as f32 - 0.01, p[2] as f32 + 0.5);
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(target, Vec3::Y);
    }
    Ok(())
}
fn place_chest(world: &mut World, f: &Fixture, second: bool) -> Result<(), String> {
    let hit = world
        .resource::<terra_player::TargetBlock>()
        .0
        .ok_or("No chest placement target")?;
    let (position, expected_material) = terra_world::targeting::block_placement(
        world.resource::<terra_world::streaming::ChunkStore>(),
        hit,
    )
    .ok_or("Invalid chest target")?;
    if position != chest_position(f, second)? {
        return Err(format!("Wrong chest target: {position:?}"));
    }
    world.write_message(terra_world::streaming::BlockEditRequest {
        position,
        expected_material,
        material: 29,
    });
    Ok(())
}
