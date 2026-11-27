//! Real narrow-window recipe navigation and scrolling, using the temporary authority.
use super::*;

fn rect(world: &World, entity: Entity) -> Result<Rect, String> {
    Ok(Rect::from_center_size(
        world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Missing narrow recipe transform")?
            .translation,
        world
            .get::<ComputedNode>(entity)
            .ok_or("Missing narrow recipe layout")?
            .size(),
    ))
}

fn recipe_bounds(world: &mut World) -> Result<(), String> {
    let viewport = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing narrow recipe window")?
        .physical_size()
        .as_vec2();
    let root = element(world, "recipe-inspector").ok_or("Missing narrow recipe popup")?;
    let frame = rect(world, root)?;
    if frame.min.min_element() < -2.0
        || frame.max.x > viewport.x + 2.0
        || frame.max.y > viewport.y + 2.0
        || (frame.center() - viewport * 0.5).abs().max_element() > 3.0
    {
        return Err(format!(
            "Narrow recipe popup is outside its viewport: {frame:?}, {viewport:?}"
        ));
    }
    if !world.resource::<InventoryView>().open
        || element(world, "inventory-shell").is_none()
        || !world.resource::<terra_ui::CraftingView>().recipe_open
    {
        return Err("Narrow recipe lost the inventory beneath the popup".into());
    }
    if element(world, "item-browser").is_some()
        || element(world, "craft-close-recipe").is_some()
        || element(world, "inventory-close").is_some()
    {
        return Err("Narrow recipe unexpectedly contains a catalog or close button".into());
    }
    let scroll = element(world, "recipe-content").ok_or("Missing recipe scroll document")?;
    let scroll_rect = rect(world, scroll)?;
    let node = world
        .get::<Node>(scroll)
        .ok_or("Missing narrow recipe scroll style")?;
    if node.overflow.y != bevy::ui::OverflowAxis::Scroll {
        return Err("Narrow recipes must use automatic vertical scrolling".into());
    }
    let computed = world
        .get::<ComputedNode>(scroll)
        .ok_or("Missing narrow recipe scroll layout")?;
    let offset = world
        .get::<bevy::ui::ScrollPosition>(scroll)
        .map_or(0.0, |position| position.0.y)
        / computed.inverse_scale_factor();
    let document_top = scroll_rect.min.y - offset;
    let document_bottom = document_top + computed.content_size().y;
    if scroll_rect.min.x < frame.min.x - 2.0
        || scroll_rect.max.x > frame.max.x + 2.0
        || scroll_rect.min.y < frame.min.y - 2.0
        || scroll_rect.max.y > frame.max.y + 2.0
    {
        return Err("Narrow recipe scroll viewport escaped its popup".into());
    }
    for id in [
        "recipe-preview-grid",
        "recipe-preview-output",
        "recipe-alternative-list",
    ] {
        let child = element(world, id).ok_or_else(|| format!("Missing recipe child {id}"))?;
        let child_rect = rect(world, child)?;
        if !descendant_of(world, child, scroll)
            || child_rect.min.x < scroll_rect.min.x - 3.0
            || child_rect.max.x > scroll_rect.max.x + 3.0
            || child_rect.min.y < document_top - 3.0
            || child_rect.max.y > document_bottom + 3.0
        {
            return Err(format!(
                "Narrow recipe child {id} escaped its scroll document: {child_rect:?}, viewport={scroll_rect:?}, document_y={document_top}..{document_bottom}"
            ));
        }
    }
    Ok(())
}

fn scroll_recipe(world: &mut World, steps: i32) -> Result<(), String> {
    let content = element(world, "recipe-content").ok_or("Missing recipe scroll document")?;
    let center = rect(world, content)?.center();
    pointer_at(world, center, MouseButton::Left, None)?;
    os_pointer(
        world,
        &[tilt_ui::serde_json::json!({"wheel":steps,"wait":0.1})],
    )
}

fn require_scrolled(world: &mut World) -> Result<(), String> {
    let scroll = element(world, "recipe-content").ok_or("Missing recipe scroll document")?;
    let overflowing = world
        .get::<ComputedNode>(scroll)
        .is_some_and(|node| node.content_size().y > node.size().y + 2.0);
    if overflowing
        && !world
            .get::<bevy::ui::ScrollPosition>(scroll)
            .is_some_and(|position| position.0.y > 1.0)
    {
        return Err("Native wheel did not scroll the narrow recipe document".into());
    }
    recipe_bounds(world)
}

pub(super) fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == u8::MAX {
            return;
        }
        let result = (|| -> Result<bool, String> {
            if state.start.elapsed() > Duration::from_secs(150) {
                return Err(format!("Narrow recipe timeout at stage {}", state.stage));
            }
            if state.failed {
                return Err("Narrow recipe capture was blank or had the wrong physical size".into());
            }
            if let Some(code) = state.key_release {
                if state.since.elapsed() > Duration::from_millis(180) {
                    os_pointer(world, &[tilt_ui::serde_json::json!({"key":[code,0]})])?;
                    state.key_release = None;
                    state.since = Instant::now();
                }
                return Ok(false);
            }
            if state.stage != 0 && state.since.elapsed() < Duration::from_millis(1000) {
                return Ok(false);
            }
            if state.stage > 1 && !previews_ready(world) {
                return Ok(false);
            }
            match state.stage {
                0 => {
                    if !world.resource::<ClientSession>().ready
                        || !world.resource::<terra_render::RenderStatus>().ready
                        || !world.resource::<terra_render::PlayerPreviewImage>().ready
                        || world.resource::<terra_render::RenderBlockPreviews>().pending != 0
                    {
                        return Ok(false);
                    }
                    if world.resource::<MenuState>().page != MenuPage::Playing {
                        world.resource_mut::<MenuState>().session_active = true;
                        world.write_message(MenuAction::Command("resume".into()));
                        return Ok(false);
                    }
                    if world.resource::<InventoryView>().prewarming {
                        return Ok(false);
                    }
                    resize_reference(world, (768, 640), true);
                    world.write_message(InventoryAction::Toggle);
                    advance(&mut state, 1);
                }
                1 => {
                    if !size_ready(world, (768, 640))
                        || !world.resource::<InventoryView>().open
                    {
                        return Ok(false);
                    }
                    state.baseline = Some(world.resource::<ClientSession>().inventory.clone());
                    world.write_message(InventoryAction::Recipe("block-29".into()));
                    advance(&mut state, 2);
                }
                2 => {
                    if !size_ready(world, (768, 640)) {
                        return Ok(false);
                    }
                    let view = world.resource::<terra_ui::CraftingView>();
                    if view.smelting || view.brewing || view.wood_options.len() != 5 {
                        return Err("Narrow chest recipe lost a wood alternative".into());
                    }
                    recipe_bounds(world)?;
                    shot(world, "crafting-recipe-reference-narrow")?;
                    advance(&mut state, 3);
                }
                3 => {
                    scroll_recipe(world, -20)?;
                    advance(&mut state, 4);
                }
                4 => {
                    require_scrolled(world)?;
                    shot(world, "crafting-recipe-scrolled-narrow")?;
                    advance(&mut state, 5);
                }
                5 => {
                    native_key(world, &mut state, 1)?;
                    advance(&mut state, 6);
                }
                6 => {
                    assert_inventory_return(world)?;
                    world.write_message(InventoryAction::Recipe("item-iron_ingot".into()));
                    advance(&mut state, 7);
                }
                7 => {
                    if !world.resource::<terra_ui::CraftingView>().smelting {
                        return Err("Narrow iron ingot did not open its production furnace recipe".into());
                    }
                    recipe_bounds(world)?;
                    shot(world, "furnace-recipe-reference-narrow")?;
                    advance(&mut state, 8);
                }
                8 => {
                    scroll_recipe(world, -20)?;
                    advance(&mut state, 9);
                }
                9 => {
                    require_scrolled(world)?;
                    shot(world, "furnace-recipe-scrolled-narrow")?;
                    advance(&mut state, 10);
                }
                10 => {
                    native_key(world, &mut state, 18)?;
                    advance(&mut state, 11);
                }
                11 => {
                    assert_inventory_return(world)?;
                    native_key(world, &mut state, 18)?;
                    advance(&mut state, 12);
                }
                _ => {
                    if state.captured < 4 {
                        return Ok(false);
                    }
                    if world.resource::<InventoryView>().open
                        || state.baseline.as_ref()
                            != Some(&world.resource::<ClientSession>().inventory)
                    {
                        return Err("Narrow recipe navigation changed or failed to close the inventory".into());
                    }
                    info!(captures = state.captured, seconds = state.start.elapsed().as_secs_f64(), "Native narrow recipes passed: 768x640 physical window, bounded popup, real scroll documents and wheel, retained inventory, no catalog or close button, five plank choices, production furnace recipe, native ESC/E and unchanged authoritative inventory");
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        match result {
            Ok(true) => {
                world.write_message(AppExit::Success);
                state.stage = u8::MAX;
            }
            Ok(false) => {}
            Err(error) => {
                error!(%error, "Native narrow recipe UI failed");
                world.write_message(AppExit::error());
                state.stage = u8::MAX;
            }
        }
    });
}
