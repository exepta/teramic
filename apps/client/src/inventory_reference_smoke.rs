//! Native reference-layout inspection with real UI clicks and the integrated authority.
use super::*;
#[path = "inventory_reference_smoke/narrow.rs"]
mod narrow;
#[path = "inventory_reference_smoke/polish.rs"]
mod polish;
#[path = "inventory_reference_smoke/surfaces.rs"]
mod surfaces;

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    release: Option<(Entity, u8)>,
    key_release: Option<u16>,
    captured: usize,
    failed: bool,
    baseline: Option<terra_core::Inventory>,
    page: String,
    seeded: usize,
    fixture_worlds: Option<PathBuf>,
}

pub(super) fn install(app: &mut App) {
    install_mode(app, false);
}
pub(crate) fn install_menu(app: &mut App) {
    install_mode(app, true);
}
fn install_mode(app: &mut App, menu: bool) {
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        release: None,
        key_release: None,
        captured: 0,
        failed: false,
        baseline: None,
        page: String::new(),
        seeded: 0,
        fixture_worlds: None,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous());
    if menu {
        app.add_systems(PreUpdate, run_menu.after(bevy::input::InputSystems));
    } else if std::env::var("TERRA_POLISH_SMOKE").is_ok_and(|value| value == "1") {
        app.add_systems(PreUpdate, polish::run.after(bevy::input::InputSystems));
    } else if std::env::var("TERRA_SURFACES_SMOKE").is_ok_and(|value| value == "1") {
        surfaces::install(app);
    } else if std::env::var("TERRA_RECIPES_NARROW_SMOKE").is_ok_and(|value| value == "1") {
        app.add_systems(PreUpdate, narrow::run.after(bevy::input::InputSystems));
    } else {
        app.add_systems(PreUpdate, run.after(bevy::input::InputSystems));
    }
}

fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
}
fn click(world: &mut World, state: &mut Inspection, id: &str, next: u8) -> Result<(), String> {
    let target = element(world, id).ok_or_else(|| format!("Missing reference button {id}"))?;
    pointer(world, target, Some(true))?;
    state.release = Some((target, next));
    state.since = Instant::now();
    Ok(())
}
fn wide_reference_size() -> (u32, u32) {
    if std::env::var("TERRA_INVENTORY_4K_SMOKE").is_ok_and(|v| v == "1") {
        (3840, 2160)
    } else {
        (1680, 936)
    }
}
fn shot(world: &mut World, name: &str) -> Result<(), String> {
    let expected = if name.contains("narrow") {
        UVec2::new(768, 640)
    } else if name.contains("compact") {
        UVec2::new(1024, 640)
    } else {
        UVec2::from_array([wide_reference_size().0, wide_reference_size().1])
    };
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("{name}.png"))))
        .observe(
            move |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
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
                if (event.image.size().as_ivec2() - expected.as_ivec2()).abs().max_element() > 1 { error!(actual=?event.image.size(),?expected,"Reference screenshot has the wrong physical size"); }
                state.failed |= colors.len() < 16 || (event.image.size().as_ivec2() - expected.as_ivec2()).abs().max_element() > 1;
                state.captured += 1;
            },
        );
    Ok(())
}
fn resize_reference(world: &mut World, size: (u32, u32), unmaximize: bool) {
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        if window.mode != bevy::window::WindowMode::Windowed {
            window.mode = bevy::window::WindowMode::Windowed;
        }
        // A 4K picker inspection must fit on a 4K desktop without the
        // compositor subtracting its title bar from the requested viewport.
        if ["TERRA_PICKER_SMOKE", "TERRA_INVENTORY_4K_SMOKE"]
            .into_iter()
            .any(|key| std::env::var(key).is_ok_and(|value| value == "1"))
        {
            window.decorations = false;
        }
        if unmaximize {
            window.set_maximized(false);
        }
    }
    resize(world, size);
}
fn size_ready(world: &mut World, size: (u32, u32)) -> bool {
    let requested = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        // Fractional desktop scaling may round either dimension by one pixel.
        .all(|window| {
            let actual = window.physical_size();
            actual.x.abs_diff(size.0) <= 1 && actual.y.abs_diff(size.1) <= 1
        });
    if !requested {
        resize(world, size);
        return false;
    }
    // The requested Window size changes before Winit acknowledges it and before
    // the UI camera/layout updates. Measure only the resulting painted viewport.
    if let Some(backdrop) = element(world, "inventory-backdrop")
        && let Some(node) = world.get::<ComputedNode>(backdrop)
        && node.size().min_element() > 0.0
        && (node.size() - Vec2::new(size.0 as f32, size.1 as f32))
            .abs()
            .max_element()
            > 1.5
    {
        return false;
    }
    true
}

fn previews_ready(world: &mut World) -> bool {
    world
        .query::<(
            &ImageMetadata,
            &bevy::ui::widget::ImageNode,
            &ComputedNode,
            Option<&Visibility>,
        )>()
        .iter(world)
        .all(|(metadata, image, node, visibility)| {
            let preview = metadata.alt.as_deref().is_some_and(|alt| {
                alt.starts_with("Block preview ")
                    || alt.starts_with("Item preview ")
                    || alt.starts_with("Prop preview ")
            });
            !preview
                || node.size().min_element() < 1.0
                || visibility == Some(&Visibility::Hidden)
                || image.image != Handle::default()
        })
        && world
            .resource::<terra_render::RenderBlockPreviews>()
            .pending
            == 0
}
fn painted_bounds(node: &ComputedNode, transform: &bevy::ui::UiGlobalTransform) -> (Vec2, Vec2) {
    let half = node.size() * 0.5;
    [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .into_iter()
    .map(|corner| transform.transform_point2(corner))
    .fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), corner| (low.min(corner), high.max(corner)),
    )
}
fn bounds(world: &mut World, recipe: bool) -> Result<(), String> {
    let size = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No window")?
        .physical_size()
        .as_vec2();
    for id in if recipe {
        vec![
            "recipe-inspector",
            "recipe-preview-grid",
            "recipe-preview-output",
        ]
    } else {
        vec![
            "inventory-shell",
            "inventory-grid",
            "inventory-hotbar",
            "inventory-equipment",
            "item-browser",
        ]
    } {
        if id == "item-browser" && !recipe && !world.resource::<InventoryView>().browser_visible {
            continue;
        }
        let entity = element(world, id).ok_or_else(|| format!("Missing reference layout {id}"))?;
        let node = world
            .get::<ComputedNode>(entity)
            .ok_or("No computed node")?;
        let transform = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("No UI transform")?;
        let (low, high) = painted_bounds(node, transform);
        let center = (low + high) * 0.5;
        if matches!(id, "inventory-shell" | "recipe-inspector")
            && (center.x - size.x * 0.5).abs() > 5.0
        {
            let mut ancestor = Some(entity);
            while let Some(current) = ancestor {
                info!(?current, id=?world.get::<ElementId>(current), node=?world.get::<Node>(current), computed=?world.get::<ComputedNode>(current).map(ComputedNode::size), transform=?world.get::<bevy::ui::UiGlobalTransform>(current), "Reference centering diagnostic");
                ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
            }
            return Err(format!(
                "Reference frame is off center: {id} center={center:?} viewport={size:?}"
            ));
        }
        if (high - low).min_element() < 15.
            || low.min_element() < -2.
            || high.x > size.x + 2.
            || high.y > size.y + 2.
        {
            return Err(format!(
                "Reference UI outside viewport: {id} low={low:?} high={high:?} viewport={size:?}"
            ));
        }
    }
    for id in ["inventory-close", "craft-close-recipe"] {
        if element(world, id).is_some() {
            return Err(format!("Unwanted close button {id}"));
        }
    }
    if recipe {
        if !world.resource::<InventoryView>().open || element(world, "inventory-shell").is_none() {
            return Err("Recipe popup replaced its underlying inventory".into());
        }
        let content = element(world, "recipe-inspector").ok_or("Missing popup")?;
        let footer = element(world, "craft-recipe-page").ok_or("Missing centered page label")?;
        let parent = world
            .get::<ChildOf>(footer)
            .ok_or("No footer parent")?
            .parent();
        let label = world
            .get::<bevy::ui::UiGlobalTransform>(footer)
            .ok_or("No page position")?
            .translation
            .x;
        let (parent_low, parent_high) = painted_bounds(
            world.get::<ComputedNode>(parent).ok_or("No footer size")?,
            world
                .get::<bevy::ui::UiGlobalTransform>(parent)
                .ok_or("No footer position")?,
        );
        let parent_center = (parent_low.x + parent_high.x) * 0.5;
        let next = element(world, "craft-alternative-next").ok_or("Missing next button")?;
        let (_, next_high) = painted_bounds(
            world.get::<ComputedNode>(next).ok_or("No next size")?,
            world
                .get::<bevy::ui::UiGlobalTransform>(next)
                .ok_or("No next position")?,
        );
        if (next_high.x - parent_high.x).abs() > 4.0
            || label < parent_center
            || world
                .get::<ComputedNode>(content)
                .is_none_or(|node| node.size().min_element() < 100.0)
        {
            return Err("Recipe pagination is not aligned right".into());
        }
        if element(world, "item-browser").is_some() {
            return Err("Recipe popup still contains a JEI catalog".into());
        }
    }
    if !recipe && world.resource::<InventoryView>().browser_visible {
        let rail = element(world, "inventory-categories-host").ok_or("Missing category rail")?;
        let panel = element(
            world,
            if recipe {
                "recipe-content"
            } else {
                "inventory-panel"
            },
        );
        if let Some(panel) = panel {
            let (_, panel_high) = painted_bounds(
                world.get::<ComputedNode>(panel).ok_or("No panel size")?,
                world
                    .get::<bevy::ui::UiGlobalTransform>(panel)
                    .ok_or("No panel transform")?,
            );
            let (rail_low, _) = painted_bounds(
                world.get::<ComputedNode>(rail).ok_or("No rail size")?,
                world
                    .get::<bevy::ui::UiGlobalTransform>(rail)
                    .ok_or("No rail transform")?,
            );
            if panel_high.x > rail_low.x + 1.0 {
                return Err(format!(
                    "Category rail overlaps the workspace: {} > {}",
                    panel_high.x, rail_low.x
                ));
            }
        }
    }
    if recipe && world.resource::<terra_ui::CraftingView>().smelting {
        for id in ["furnace-recipe-header", "furnace-recipe-process"] {
            let root = element(world, id).ok_or("Missing furnace layout")?;
            let (low, high) = painted_bounds(
                world.get::<ComputedNode>(root).ok_or("No furnace size")?,
                world
                    .get::<bevy::ui::UiGlobalTransform>(root)
                    .ok_or("No furnace transform")?,
            );
            let low = low - Vec2::splat(3.0);
            let high = high + Vec2::splat(3.0);
            for (entity, node, transform) in world
                .query::<(Entity, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
                .iter(world)
            {
                if !node.is_empty() && descendant_of(world, entity, root) {
                    let (child_low, child_high) = painted_bounds(node, transform);
                    if child_low.cmplt(low).any() || child_high.cmpgt(high).any() {
                        let mut ancestor = Some(entity);
                        while let Some(current) = ancestor {
                            info!(?current, text=?world.get::<Text>(current), node=?world.get::<Node>(current), size=?world.get::<ComputedNode>(current).map(ComputedNode::size), transform=?world.get::<bevy::ui::UiGlobalTransform>(current), "Furnace clipping diagnostic");
                            if current == root {
                                break;
                            }
                            ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
                        }
                        return Err(format!(
                            "Furnace content escapes {id}: {entity:?} {child_low:?} {child_high:?}, parent {low:?} {high:?}"
                        ));
                    }
                }
            }
        }
    }
    if !recipe {
        let view = world.resource::<InventoryView>();
        if view.items.len() != 18 || view.hotbar.len() != 9 {
            return Err("Player inventory is not 18 + 9 slots".into());
        }
        let trash_visible = view.browser_visible && view.creative;
        let grid = element(world, "inventory-grid").ok_or("Missing backpack")?;
        let player_size = world
            .query::<(Entity, &ElementId, &ComputedNode)>()
            .iter(world)
            .find(|(entity, id, _)| id.0 == "inventory-cell" && descendant_of(world, *entity, grid))
            .map(|(_, _, node)| node.size())
            .ok_or("No player cell")?;
        if let Some(browser) = element(world, "inventory-browser-grid") {
            for (entity, id, node) in world
                .query::<(Entity, &ElementId, &ComputedNode)>()
                .iter(world)
            {
                if id.0 == "inventory-cell"
                    && descendant_of(world, entity, browser)
                    && !node.size().abs_diff_eq(player_size, 1.5)
                {
                    return Err(format!(
                        "Catalog slot differs from player slot: {:?} vs {player_size:?}",
                        node.size()
                    ));
                }
            }
        }
        for id in ["inventory-crafting", "inventory-craft-result"] {
            let root = element(world, id).ok_or("Missing crafting cells")?;
            for (entity, name, node) in world
                .query::<(Entity, &ElementId, &ComputedNode)>()
                .iter(world)
            {
                if name.0 == "inventory-cell"
                    && descendant_of(world, entity, root)
                    && !node.size().abs_diff_eq(
                        if id == "inventory-craft-result" {
                            player_size * (32.0 / 28.0)
                        } else {
                            player_size
                        },
                        1.5,
                    )
                {
                    return Err(format!(
                        "Crafting cell differs from inventory: {id} {:?} vs {player_size:?}",
                        node.size()
                    ));
                }
            }
        }
        if trash_visible {
            let trash = element(world, "inventory-trash").ok_or("No trash in category rail")?;
            let rail = element(world, "inventory-categories-host").ok_or("No category rail")?;
            if !descendant_of(world, trash, rail) {
                return Err("Trash is not in the catalog rail".into());
            }
        }
        for id in ["inventory-grid", "inventory-hotbar"] {
            let root = element(world, id).ok_or("Missing player grid")?;
            for (entity, name, node) in world
                .query::<(Entity, &ElementId, &ComputedNode)>()
                .iter(world)
            {
                if name.0 == "inventory-cell"
                    && descendant_of(world, entity, root)
                    && (node.size().x - node.size().y).abs() > 1.5
                {
                    return Err(format!("Player slot is not square: {id} {:?}", node.size()));
                }
            }
        }
    }
    if !recipe {
        let count = world
            .query::<&ElementId>()
            .iter(world)
            .filter(|id| id.0 == "equipment-slot")
            .count();
        if count != terra_core::EquipmentSlot::ALL.len() {
            return Err(format!("Reference layout lost equipment slots: {count}"));
        }
    }
    Ok(())
}
fn native_key(world: &mut World, state: &mut Inspection, code: u16) -> Result<(), String> {
    os_pointer(world, &[tilt_ui::serde_json::json!({"key":[code,1]})])?;
    state.key_release = Some(code);
    state.since = Instant::now();
    Ok(())
}

fn assert_inventory_return(world: &World) -> Result<(), String> {
    if world.resource::<terra_ui::CraftingView>().recipe_open
        || !world.resource::<InventoryView>().open
    {
        Err("Close key did not return from the popup to the retained inventory".into())
    } else {
        Ok(())
    }
}

fn click_wood(
    world: &mut World,
    state: &mut Inspection,
    index: usize,
    next: u8,
) -> Result<(), String> {
    let root = element(world, "recipe-wood-options").ok_or("No wood options")?;
    let preview = world
        .resource::<terra_ui::CraftingView>()
        .wood_options
        .get(index)
        .and_then(|wood| wood.slots.first())
        .ok_or("Missing wood preview")?
        .preview_key
        .clone();
    let target = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| {
            (id.0 == "recipe-wood"
                && descendant_of(world, entity, root)
                && subtree_image(world, entity, &preview))
            .then_some(entity)
        })
        .ok_or("Missing wood button")?;
    pointer(world, target, Some(true))?;
    state.release = Some((target, next));
    state.since = Instant::now();
    Ok(())
}
fn check_wood(world: &World, material: u16) -> Result<(), String> {
    let view = world.resource::<terra_ui::CraftingView>();
    if view
        .recipe_grid
        .iter()
        .filter(|slot| !slot.empty)
        .any(|slot| slot.item != Some(ItemKind::Block(material)))
    {
        Err("Wood preview click did not update every ingredient cell".into())
    } else {
        Ok(())
    }
}
fn open_cauldron(world: &mut World) {
    let potion = terra_core::brewing::recipes()[0].output().item;
    world.write_message(InventoryAction::Recipe(format!(
        "item-{}",
        potion.identifier().trim_start_matches("teramic::")
    )));
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == u8::MAX { return; }
        let result = (|| -> Result<bool,String> {
            if state.start.elapsed() > Duration::from_secs(210) { return Err(format!("Reference UI timeout at {}; physical sizes {:?}; previews ready {}", state.stage, world.query_filtered::<&Window, With<PrimaryWindow>>().iter(world).map(Window::physical_size).collect::<Vec<_>>(), previews_ready(world))); }
            if state.failed { return Err("Reference capture is blank or has the wrong physical size".into()); }
            if let Some((target,next)) = state.release {
                if state.since.elapsed() > Duration::from_millis(180) {
                    pointer(world,target,Some(false))?;
                    state.release = None;
                    advance(&mut state,next);
                }
                return Ok(false);
            }
            if let Some(code)=state.key_release {
                if state.since.elapsed()>Duration::from_millis(180) {
                    os_pointer(world,&[tilt_ui::serde_json::json!({"key":[code,0]})])?;
                    state.key_release=None;
                    state.since=Instant::now();
                }
                return Ok(false);
            }
            if state.stage != 0 && state.since.elapsed() < Duration::from_millis(900) { return Ok(false); }
            if state.stage > 1 && state.stage != 11 && !previews_ready(world) { return Ok(false); }
            match state.stage {
                0 => {
                    if !world.resource::<ClientSession>().ready || !world.resource::<terra_render::RenderStatus>().ready
                        || !world.resource::<terra_render::PlayerPreviewImage>().ready
                        || world.resource::<terra_render::RenderBlockPreviews>().pending != 0 { return Ok(false); }
                    if world.resource::<MenuState>().page != MenuPage::Playing {
                        world.resource_mut::<MenuState>().session_active = true;
                        world.write_message(MenuAction::Command("resume".into()));
                        return Ok(false);
                    }
                    if world.resource::<InventoryView>().prewarming { return Ok(false); }
                    resize_reference(world,wide_reference_size(),true);
                    world.write_message(InventoryAction::Toggle);
                    advance(&mut state,1);
                }
                1 => {
                    if !size_ready(world,wide_reference_size()) { return Ok(false); }
                    if !world.resource::<InventoryView>().open { return Ok(false); }
                    let materials = [3,4,16,17,18,6,7,8,74,82,90,172,163,164,165,28];
                    if state.seeded > 0 && world.resource::<ClientSession>().inventory.slot(state.seeded - 1)
                        .is_none_or(|stack| stack.item != ItemKind::Block(materials[state.seeded - 1])) { return Ok(false); }
                    if state.seeded < materials.len() {
                        let revision = world.resource::<ClientSession>().inventory.revision;
                        world.write_message(terra_network::client::InventoryRequest(
                            terra_network::protocol::inventory::InventoryCommand {
                                revision,
                                action: terra_network::protocol::inventory::InventoryAction::CreativeSet {
                                    slot: state.seeded as u8, item: ItemKind::Block(materials[state.seeded]), count: 1,
                                },
                            }));
                        state.seeded += 1;
                        return Ok(false);
                    }
                    advance(&mut state,2);
                }
                2 => {
                    if world.resource::<ClientSession>().inventory.occupied_slots() < 16 { return Ok(false); }
                    if state.since.elapsed() < Duration::from_secs(3) { return Ok(false); }
                    bounds(world,false)?;
                    state.baseline = Some(world.resource::<ClientSession>().inventory.clone());
                    shot(world,"player-inventory-reference-wide")?;
                    click(world,&mut state,"inventory-recipe-book",3)?;
                }
                3 => {
                    if world.resource::<InventoryView>().browser_visible || world.resource::<terra_ui::CraftingView>().recipe_open || element(world,"item-browser").is_some() { return Err("Book did not collapse only the item browser".into()); }
                    bounds(world,false)?;
                    shot(world,"player-inventory-catalog-collapsed")?;
                    click(world,&mut state,"inventory-recipe-book",4)?;
                }
                4 => {
                    if !world.resource::<InventoryView>().browser_visible { return Err("Book did not restore the item browser".into()); }
                    let category = world.resource::<InventoryView>().categories.iter().find(|category| !category.selected).ok_or("Missing category rail")?.clone();
                    state.page=category.id;
                    let target=category_target(world,&category.preview_key).ok_or("Missing category rail button")?;
                    pointer(world,target,Some(true))?;
                    state.release=Some((target,5));
                    state.since=Instant::now();
                }
                5 => {
                    if !world.resource::<InventoryView>().categories.iter().any(|category| category.id == state.page && category.selected) { return Err("Category rail click did not select its filter".into()); }
                    shot(world,"inventory-left-category-tabs")?;
                    click(world,&mut state,"inventory-browser-filter",6)?;
                }
                6 => {
                    if !world.resource::<InventoryView>().categories.iter().any(|category| category.id == "all" && category.selected) { return Err("Filter reset did not restore all items".into()); }
                    state.page=world.resource::<InventoryView>().browser_page.clone();
                    click(world,&mut state,"inventory-page-next",8)?;
                }
                8 => {
                    if world.resource::<InventoryView>().browser_page==state.page {return Err("Catalog next-page click did not advance".into());}
                    shot(world,"catalog-page-two")?;
                    click(world,&mut state,"inventory-page-previous",9)?;
                }
                9 => {
                    if world.resource::<InventoryView>().browser_page!=state.page {return Err("Catalog previous-page click did not return".into());}
                    click(world,&mut state,"inventory-search",10)?;
                }
                10 => { type_text(world,"glass")?; advance(&mut state,27); }
                27 => {
                    if world.resource::<InventoryView>().search!="glass" {return Err("Catalog search failed".into());}
                    shot(world,"catalog-search")?;advance(&mut state,51);
                }
                51 => {
                    world.write_message(InventoryAction::Search(String::new()));advance(&mut state,28);
                }
                28 => {click(world,&mut state,"inventory-browser-filter",39)?;}
                39 => {
                    let grid=element(world,"inventory-browser-grid").ok_or("Missing item grid")?;
                    let target=world.query::<(Entity,&ElementId)>().iter(world).find_map(|(entity,id)|
                        (id.0=="inventory-cell" && descendant_of(world,entity,grid) && subtree_image(world,entity,"Block preview 28")).then_some(entity));
                    if let Some(target) = target { pointer(world,target,None)?; advance(&mut state,25); }
                    else if world.resource::<InventoryView>().browser_next { click(world,&mut state,"inventory-page-next",39)?; }
                    else { return Err("Workbench missing from all catalog pages".into()); }
                }
                25 => {native_key(world,&mut state,19)?; advance(&mut state,7);}
                7 => {
                    bounds(world,true)?; shot(world,"crafting-recipe-reference-wide")?;advance(&mut state,47);
                }
                47 => {
                    world.write_message(InventoryAction::Recipe("block-29".into()));advance(&mut state,16);
                }
                16 => {
                    bounds(world,true)?;
                    if world.resource::<terra_ui::CraftingView>().wood_options.len()!=4 {return Err("Chest does not expose all four wood types".into());}
                    shot(world,"chest-oak")?;
                    click_wood(world,&mut state,1,29)?;
                }
                29 => { check_wood(world,18)?;shot(world,"chest-spruce")?;click_wood(world,&mut state,2,30)?; }
                30 => { check_wood(world,17)?;shot(world,"chest-birch")?;click_wood(world,&mut state,3,31)?; }
                31 => { check_wood(world,135)?;shot(world,"chest-cherry")?;advance(&mut state,53); }
                53 => {
                    let list=element(world,"recipe-alternative-list").ok_or("No alternatives")?;
                    let target=world.query::<(Entity,&ElementId)>().iter(world).find_map(|(entity,id)| (id.0=="recipe-alternative" && descendant_of(world,entity,list) && subtree_image(world,entity,"Block preview 17")).then_some(entity)).ok_or("No Birch alternative")?;
                    pointer(world,target,Some(true))?;state.release=Some((target,54));state.since=Instant::now();
                }
                54 => {check_wood(world,17)?;shot(world,"chest-birch-alternative")?;native_key(world,&mut state,1)?;advance(&mut state,17);}
                17 => {
                    assert_inventory_return(world)?;
                    world.write_message(InventoryAction::Recipe("item-iron_ingot".into()));
                    advance(&mut state,18);
                }
                18 => {
                    if !world.resource::<terra_ui::CraftingView>().smelting { return Err("Iron ingot did not open its real furnace recipe".into()); }
                    shot(world,"furnace-recipe-reference-wide")?;
                    advance(&mut state,26);
                }
                26 => {
                    bounds(world,true)?;
                    click(world,&mut state,"recipe-fuels-tab",19)?;
                }
                19 => {
                    if !world.resource::<terra_ui::CraftingView>().furnace_fuels { return Err("Furnace fuel tab did not open".into()); }
                    shot(world,"furnace-alternative-fuels")?;
                    click(world,&mut state,"recipe-fuel",20)?;
                }
                20 => {
                    if !world.resource::<terra_ui::CraftingView>().furnace_fuel_options.first().is_some_and(|fuel| fuel.selected) { return Err("Real fuel click did not update the preview".into()); }
                    shot(world,"furnace-selected-fuel")?;
                    native_key(world,&mut state,18)?;
                    advance(&mut state,21);
                }
                21 => {
                    assert_inventory_return(world)?;
                    open_cauldron(world);
                    advance(&mut state,32);
                }
                32 => {
                    bounds(world,true)?;
                    if !world.resource::<terra_ui::CraftingView>().brewing {return Err("Potion did not open its cauldron recipe".into());}
                    shot(world,"cauldron-recipe-wide")?;
                    native_key(world,&mut state,1)?;advance(&mut state,33);
                }
                33 => {assert_inventory_return(world)?;native_key(world,&mut state,18)?;advance(&mut state,34);}
                34 => {
                    if world.resource::<InventoryView>().open {return Err("Inventory failed to close before hotbar inspection".into());}
                    shot(world,"hotbar-wide")?;advance(&mut state,52);
                }
                52 => {
                    world.write_message(InventoryAction::Hotbar(3));advance(&mut state,35);
                }
                35 => {
                    if !world.resource::<InventoryView>().hotbar.get(3).is_some_and(|slot| slot.selected) {return Err("Hotbar selection did not update".into());}
                    shot(world,"hotbar-selected-wide")?;
                    native_key(world,&mut state,18)?;advance(&mut state,36);
                }
                36 => {
                    if !world.resource::<InventoryView>().open {return Err("Inventory did not reopen".into());}
                    resize_reference(world,(1024,640),false);advance(&mut state,11);
                }
                11 => {
                    if !size_ready(world,(1024,640)) { return Ok(false); }
                    advance(&mut state,14);
                }
                14 => {
                    if !size_ready(world,(1024,640)) || !previews_ready(world) { return Ok(false); }
                    bounds(world,false)?;
                    shot(world,"player-inventory-reference-compact")?;advance(&mut state,48);
                }
                48 => {
                    world.write_message(terra_ui::CraftingAction::OpenBook);advance(&mut state,12);
                }
                12 => {
                    bounds(world,true)?;
                    shot(world,"crafting-recipe-reference-compact")?;advance(&mut state,49);
                }
                49 => {
                    world.write_message(InventoryAction::Recipe("item-iron_ingot".into()));advance(&mut state,22);
                }
                22 => {
                    bounds(world,true)?;
                    shot(world,"furnace-recipe-reference-compact")?;advance(&mut state,50);
                }
                50 => {open_cauldron(world);advance(&mut state,37);
                }
                37 => {
                    bounds(world,true)?;shot(world,"cauldron-recipe-compact")?;
                    native_key(world,&mut state,1)?;advance(&mut state,23);
                }
                23 => {
                    assert_inventory_return(world)?;
                    native_key(world,&mut state,18)?;
                    advance(&mut state,24);
                }
                24 => {shot(world,"hotbar-compact")?;native_key(world,&mut state,1)?;advance(&mut state,40);}
                40 => { if world.resource::<MenuState>().page != MenuPage::Pause { return Err("ESC did not pause".into()); } pause_bounds(world)?;shot(world,"pause-menu-compact")?;click_menu(world,&mut state,"Resume Game",41)?; }
                41 => { if world.resource::<MenuState>().page != MenuPage::Playing { return Err("Resume did not return to gameplay".into()); } resize_reference(world,wide_reference_size(),false);advance(&mut state,42); }
                42 => { if !size_ready(world,wide_reference_size()) { return Ok(false); } native_key(world,&mut state,1)?;advance(&mut state,43); }
                43 => { if world.resource::<MenuState>().page != MenuPage::Pause { return Err("Wide pause failed".into()); } pause_bounds(world)?;shot(world,"pause-menu-wide")?;click_menu(world,&mut state,"Settings",44)?; }
                44 => { if world.resource::<MenuState>().page != MenuPage::Options { return Err("Pause Settings failed".into()); } shot(world,"settings-paused-wide")?;click_menu(world,&mut state,"Back",45)?; }
                45 => { if world.resource::<MenuState>().page != MenuPage::Pause { return Err("Settings Back lost pause".into()); } click_menu(world,&mut state,"Resume Game",46)?; }
                _ => {
                    if state.captured < 25 { return Ok(false); }
                    if world.resource::<InventoryView>().open || state.baseline.as_ref() != Some(&world.resource::<ClientSession>().inventory) {
                        return Err("Closing/navigation changed the authoritative inventory or did not close it".into());
                    }
                    info!(captures=state.captured, seconds=state.start.elapsed().as_secs_f64(), "Native inventory reference passed: 18+9 slots, equal crafting cells, equal catalog cells, trash in rail, pause and settings, square slots, stable catalog pages, right recipe pagination, no recipe catalog, four interactive wood previews, cauldron, furnace, painted hotbar, ESC/E and unchanged authoritative inventory");
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        match result {
            Ok(true) => { world.write_message(AppExit::Success); state.stage=u8::MAX; }
            Ok(false) => {}
            Err(error) => { error!(%error,"Native reference UI failed"); world.write_message(AppExit::error()); state.stage=u8::MAX; }
        }
    });
}

fn pause_bounds(world: &mut World) -> Result<(), String> {
    let size = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No window")?
        .physical_size()
        .as_vec2();
    let root = element(world, "pause-menu").ok_or("No pause panel")?;
    let center = world
        .get::<bevy::ui::UiGlobalTransform>(root)
        .ok_or("No pause transform")?
        .translation;
    if (center.x - size.x * 0.5).abs() > 3.0 {
        return Err("Pause panel is not centered".into());
    }
    let mut count = 0;
    for (entity, id, node, transform) in world
        .query::<(
            Entity,
            &ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
    {
        if id.0 != "menu-action" || !descendant_of(world, entity, root) {
            continue;
        }
        let (low, high) = painted_bounds(node, transform);
        if (high.x - low.x) < 200.0 || low.min_element() < 0.0 || high.x > size.x || high.y > size.y
        {
            return Err("Pause button is squeezed or outside the window".into());
        }
        count += 1;
    }
    if count != 5 {
        return Err("Pause actions are missing".into());
    }
    Ok(())
}
fn menu_frame_bounds(world: &mut World, id: &str) -> Result<(), String> {
    let root = element(world, id).ok_or("No copper frame")?;
    let (low, high) = painted_bounds(
        world.get::<ComputedNode>(root).ok_or("No frame size")?,
        world
            .get::<bevy::ui::UiGlobalTransform>(root)
            .ok_or("No frame transform")?,
    );
    let size = high - low;
    for (entity, name, node, transform) in world
        .query::<(
            Entity,
            &ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
    {
        if name.0 != "menu-action" || !descendant_of(world, entity, root) {
            continue;
        }
        let (a, b) = painted_bounds(node, transform);
        if a.x < low.x + size.x * 0.02
            || b.x > high.x - size.x * 0.02
            || b.y > high.y - size.x * 0.03
        {
            return Err(format!(
                "Menu action touches copper border in {id}: {a:?} {b:?}, frame {low:?} {high:?}"
            ));
        }
    }
    Ok(())
}
fn menu_button(world: &mut World, label: &str) -> Result<Entity, String> {
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| {
            (id.0 == "menu-action" && subtree_text(world, entity, label)).then_some(entity)
        })
        .ok_or_else(|| format!("Missing live menu button {label}"))
}
fn click_menu(
    world: &mut World,
    state: &mut Inspection,
    label: &str,
    next: u8,
) -> Result<(), String> {
    let target = menu_button(world, label)?;
    pointer(world, target, Some(true))?;
    state.release = Some((target, next));
    state.since = Instant::now();
    Ok(())
}
fn fixture_worlds(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "teramic183-ui-worlds-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let library = terra_storage::WorldLibrary { root: root.clone() };
    state.fixture_worlds = Some(root);
    let template = world.resource::<terra_core::GameConfig>().world.clone();
    for (index, title) in [
        "Riverside Valley",
        "Frostpeak Ridge",
        "Sunspire Desert",
        "Eldertree Grove",
        "Crystal Hollow",
    ]
    .iter()
    .enumerate()
    {
        library.create(
            title,
            Some(index as u64 + 123),
            if index == 2 {
                terra_core::GameMode::Creative
            } else {
                terra_core::GameMode::Survival
            },
            &template,
        )?;
    }
    world.resource_mut::<terra_ui::menu::MenuPaths>().worlds = library;
    Ok(())
}
fn run_menu(world: &mut World) {
    world.resource_scope(|world,mut state:Mut<Inspection>|{
        if state.stage==u8::MAX {return;}
        let result=(||->Result<bool,String>{
            if state.start.elapsed()>Duration::from_secs(100) {return Err(format!("Copper menu timeout at {}",state.stage));}
            if state.failed {return Err("Menu capture failed".into());}
            if let Some((target,next))=state.release {
                if state.since.elapsed()>Duration::from_millis(180) {pointer(world,target,Some(false))?;state.release=None;advance(&mut state,next);}
                return Ok(false);
            }
            if state.stage!=0 && state.since.elapsed()<Duration::from_millis(900) {return Ok(false);}
            match state.stage {
                0=>{
                    if !world.resource::<terra_ui::menu::MenuView>().main || element(world,"start-menu-logo").is_none() {return Ok(false);}
                    if world.get_resource::<terra_ui::menu::StartupProgress>().is_some_and(|progress|progress.active) {return Ok(false);}
                    fixture_worlds(world,&mut state)?;
                    resize_reference(world,wide_reference_size(),true);advance(&mut state,1);
                }
                1=>{
                    if !size_ready(world,wide_reference_size()) {return Ok(false);}
                    let logo=element(world,"start-menu-logo").ok_or("Missing painted logo")?;
                    let image=world.get::<bevy::ui::widget::ImageNode>(logo).ok_or("Logo has no image")?;
                    if world.resource::<Assets<Image>>().get(&image.image).is_none() {return Ok(false);}
                    if !world.resource::<terra_world::streaming::ChunkStore>().loaded.is_empty() || world.contains_resource::<terra_server::IntegratedServer>() {return Err("Start menu unexpectedly started gameplay".into());}
                    pointer(world,logo,None)?;advance(&mut state,10);
                }
                10=>{shot(world,"start-menu-wide")?;let button=menu_button(world,"Singleplayer")?;pointer(world,button,None)?;advance(&mut state,11);}
                11=>{let button=menu_button(world,"Singleplayer")?;if world.get::<tilt_ui::ElementState>(button).is_none_or(|state|!state.hovered) {return Err("Singleplayer is not hovered after native pointer motion".into());}shot(world,"singleplayer-button-hover")?;click_menu(world,&mut state,"Settings",2)?;}
                2=>{
                    if world.resource::<MenuState>().page!=MenuPage::Options {return Err("Real Settings click did not open options".into());}
                    menu_frame_bounds(world,"options-page")?;shot(world,"settings-wide")?;click(world,&mut state,"controls",12)?;
                }
                12=>{if !world.resource::<terra_ui::menu::OptionsView>().controls {return Err("Controls tab did not open".into());}shot(world,"settings-controls-wide")?;click(world,&mut state,"graphics",13)?;}
                13=>{click(world,&mut state,"multiplayer",34)?;}
                34=>{shot(world,"settings-multiplayer-wide")?;advance(&mut state,35);}
                35=>{click_menu(world,&mut state,"Back",3)?;}
                3=>{if !world.resource::<terra_ui::menu::MenuView>().main {return Err("Options Back did not restore main menu".into());}click_menu(world,&mut state,"Singleplayer",14)?;}
                14=>{if world.resource::<terra_ui::menu::WorldsView>().cards.len()!=5 {return Err("Singleplayer did not read five fixture world manifests".into());}click(world,&mut state,"world-card",15)?;}
                15=>{if !world.resource::<terra_ui::menu::WorldsView>().cards.iter().any(|c|c.selected) {return Err("World card selection failed".into());}menu_frame_bounds(world,"singleplayer-page")?;shot(world,"singleplayer-wide")?;click_menu(world,&mut state,"Create World",16)?;}
                16=>{if world.resource::<MenuState>().dialog!=terra_ui::menu::MenuDialog::CreateWorld {return Err("Create World dialog failed".into());}menu_frame_bounds(world,"menu-dialog")?;shot(world,"singleplayer-create-dialog")?;click_menu(world,&mut state,"Cancel",17)?;}
                17=>{click_menu(world,&mut state,"Delete World",18)?;}
                18=>{if world.resource::<MenuState>().dialog!=terra_ui::menu::MenuDialog::DeleteWorld {return Err("Delete World dialog failed".into());}menu_frame_bounds(world,"menu-dialog")?;shot(world,"singleplayer-delete-dialog")?;click_menu(world,&mut state,"Cancel",19)?;}
                19=>{resize_reference(world,(1024,640),false);advance(&mut state,20);}
                20=>{if !size_ready(world,(1024,640)) {return Ok(false);}menu_frame_bounds(world,"singleplayer-page")?;shot(world,"singleplayer-compact")?;click_menu(world,&mut state,"Back",21)?;}
                21=>{shot(world,"start-menu-compact")?;click_menu(world,&mut state,"Settings",22)?;}
                22=>{menu_frame_bounds(world,"options-page")?;shot(world,"settings-compact")?;click_menu(world,&mut state,"Back",23)?;}
                23=>{
                    world.resource_mut::<MenuState>().page=MenuPage::Loading;
                    world.resource_mut::<terra_ui::menu::LoadingProgress>().fraction=Some(0.42);
                    world.resource_mut::<terra_ui::menu::LoadingProgress>().detail="Preparing terrain sections".into();
                    world.resource_mut::<terra_ui::menu::SessionInfo>().title="Riverside Valley".into();
                    resize_reference(world,wide_reference_size(),false);advance(&mut state,24);
                }
                24=>{if !size_ready(world,wide_reference_size()) {return Ok(false);}shot(world,"world-loading-wide")?;advance(&mut state,25);}
                25=>{resize_reference(world,(1024,640),false);advance(&mut state,26);}
                26=>{if !size_ready(world,(1024,640)) {return Ok(false);}shot(world,"world-loading-compact")?;advance(&mut state,27);}
                27=>{world.resource_mut::<MenuState>().page=MenuPage::Main;advance(&mut state,28);}
                28=>{click_menu(world,&mut state,"Multiplayer",29)?;}
                29=>{if !world.resource::<terra_ui::menu::MenuView>().servers{return Err("Multiplayer did not open".into());}menu_frame_bounds(world,"multiplayer-page")?;shot(world,"multiplayer-compact")?;advance(&mut state,30);}
                30=>{resize_reference(world,wide_reference_size(),false);advance(&mut state,31);}
                31=>{if !size_ready(world,wide_reference_size()){return Ok(false);}menu_frame_bounds(world,"multiplayer-page")?;shot(world,"multiplayer-wide")?;click_menu(world,&mut state,"Add Server",32)?;}
                32=>{if world.resource::<MenuState>().dialog!=terra_ui::menu::MenuDialog::EditServer{return Err("Add Server dialog failed".into());}menu_frame_bounds(world,"menu-dialog")?;shot(world,"multiplayer-add-dialog")?;click_menu(world,&mut state,"Cancel",33)?;}
                _=>{if state.captured<16{return Ok(false);}info!(captures=state.captured,"Native copper menus passed: frame insets, dialogs, representative world loading, Singleplayer hover, Settings/Controls/Multiplayer/Back, five private worlds, selection, create/delete cancellation, Multiplayer/Add Server cancellation, wide and compact, no terrain/server");return Ok(true);}
            }
            Ok(false)
        })();
        match result {
            Ok(true)=>{if let Some(path)=state.fixture_worlds.take() {let _=std::fs::remove_dir_all(path);}world.write_message(AppExit::Success);state.stage=u8::MAX;}
            Ok(false)=>{}
            Err(error)=>{if let Some(path)=state.fixture_worlds.take() {let _=std::fs::remove_dir_all(path);}error!(%error,"Native copper menu failed");world.write_message(AppExit::error());state.stage=u8::MAX;}
        }
    });
}
