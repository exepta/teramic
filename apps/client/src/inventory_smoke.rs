//! Native inventory layout, real pointer picking, and authoritative creative grants.
use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::InputFocus,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::PrimaryWindow,
};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
use terra_core::ItemKind;
use terra_network::client::ClientSession;
use terra_player::{CameraInputState, PlayerInputBlock};
use terra_ui::{
    InventoryAction, InventoryView,
    menu::{MenuAction, MenuPage, MenuState},
};
use tilt_ui::{ElementId, ImageMetadata};

#[path = "inventory_reference_smoke.rs"]
pub(crate) mod reference;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Loading,
    Open,
    LayoutCapture,
    CategoryHover,
    CategoryPress,
    CategoryRelease,
    CategorySelected,
    CategoryDropdownRelease,
    CategoryDropdownOpen,
    CategoryOptionRelease,
    CategoryOptionConfirm,
    AllPress,
    AllRelease,
    Hover,
    Press,
    Release,
    SingleGrant,
    SinglePickupPress,
    SinglePickupRelease,
    SinglePickupConfirm,
    Cursor,
    HeldTabPress,
    HeldTabRelease,
    HeldTabConfirm,
    PlacePress,
    PlaceRelease,
    Grant,
    HalfPress,
    HalfRelease,
    HalfConfirm,
    OnePress,
    OneRelease,
    OneConfirm,
    DragStart,
    DragMove,
    DragRelease,
    DragConfirm,
    SweepStart,
    SweepMove,
    SweepRelease,
    SweepConfirm,
    SweepReturnPress,
    SweepReturnRelease,
    SweepReturnConfirm,
    OutsidePickupStart,
    OutsidePickupPress,
    OutsidePickupRelease,
    OutsidePickupConfirm,
    OutsideRelease,
    OutsideConfirm,
    CraftGrid,
    Wide,
    SlotHover,
    WideResume,
    SearchFocus,
    Search,
    Compact,
    Capture,
    ScrollSetup,
    ScrollConstrained,
    ScrollDown,
    ScrollHover,
    ScrollUp,
    TrashPickupReady,
    TrashPickupRelease,
    TrashPickupConfirm,
    TrashCursorRelease,
    TrashCursorConfirm,
    TrashSlotReady,
    TrashSlotRelease,
    TrashSlotConfirm,
    TrashCapture,
    Close,
    Reopen,
    HudPause,
    HudOptions,
    HudOptionsCapture,
    HudBack,
    HudResume,
    HudHidden,
    HudHiddenInventory,
    HudRestore,
    HudRestored,
    HudCaptured,
    Done,
}
#[derive(Resource)]
struct InventorySmoke {
    started: Instant,
    entered: Instant,
    stage: Stage,
    captures: usize,
    capture_error: bool,
    target: Option<Entity>,
    creative: bool,
    grid_phase: u8,
    grid_count: u16,
    scroll_revision: u64,
    scroll_attempts: u8,
    trash_drops: usize,
    category_id: &'static str,
    category_return: Stage,
    category_scrolls: u8,
    category_all_items: Vec<String>,
    cached_backdrop: Option<Entity>,
    cached_shell: Option<Entity>,
    reopened: bool,
}

#[derive(Resource)]
struct NativeOsPointer {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Drop for NativeOsPointer {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.wait();
    }
}

fn os_pointer(world: &mut World, commands: &[tilt_ui::serde_json::Value]) -> Result<(), String> {
    if !world.contains_resource::<NativeOsPointer>() {
        let mut child = Command::new("python3")
            .arg("tools/ui/wayland_pointer.py")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| error.to_string())?;
        let input = child.stdin.take();
        let output = child
            .stdout
            .take()
            .ok_or("Native pointer output unavailable")?;
        let mut process = NativeOsPointer {
            child,
            input,
            output: BufReader::new(output),
        };
        let mut ready = String::new();
        process
            .output
            .read_line(&mut ready)
            .map_err(|error| error.to_string())?;
        if ready.trim() != "READY" {
            return Err("Native pointer did not initialize".into());
        }
        world.insert_resource(process);
    }
    let mut process = world.resource_mut::<NativeOsPointer>();
    for command in commands {
        let input = process
            .input
            .as_mut()
            .ok_or("Native pointer input unavailable")?;
        writeln!(input, "{command}").map_err(|error| error.to_string())?;
        input.flush().map_err(|error| error.to_string())?;
        let mut acknowledgement = String::new();
        process
            .output
            .read_line(&mut acknowledgement)
            .map_err(|error| error.to_string())?;
        if !acknowledgement.starts_with("OK ") {
            return Err("Native pointer command failed".into());
        }
    }
    Ok(())
}
impl InventorySmoke {
    fn advance(&mut self, stage: Stage) {
        self.stage = stage;
        self.entered = Instant::now();
    }
    fn elapsed(&self, seconds: f32) -> bool {
        self.entered.elapsed().as_secs_f32() >= seconds
    }
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-inventory") {
        return;
    }
    if std::env::var("TERRA_CLAIMS_GALLERY").as_deref() == Ok("1") {
        crate::claims_smoke::install(app);
        return;
    }
    if crate::kingdom_smoke::requested() {
        crate::kingdom_smoke::install(app);
        return;
    }
    if std::env::var("TERRA_INVENTORY_REFERENCE_SMOKE").is_ok_and(|value| value == "1") {
        reference::install(app);
        return;
    }
    let now = Instant::now();
    app.insert_resource(InventorySmoke {
        started: now,
        entered: now,
        stage: Stage::Loading,
        captures: 0,
        capture_error: false,
        target: None,
        creative: true,
        grid_phase: 0,
        grid_count: 0,
        scroll_revision: 0,
        scroll_attempts: 0,
        trash_drops: 0,
        category_id: "all",
        category_return: Stage::CategorySelected,
        category_scrolls: 0,
        category_all_items: Vec::new(),
        cached_backdrop: None,
        cached_shell: None,
        reopened: false,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, run.after(bevy::input::InputSystems))
    .add_systems(
        PostUpdate,
        tooltip_frame_guard.after(bevy::ui::UiSystems::Stack),
    );
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<InventorySmoke>| {
        if state.stage == Stage::Done {
            return;
        }
        match advance(world, &mut state) {
            Ok(true) => {
                tracing::info!(
                    captures = state.captures,
                    creative_interaction = state.creative,
                    "Native inventory mode/layout acceptance completed"
                );
                state.stage = Stage::Done;
                world.write_message(AppExit::Success);
            }
            Ok(false) => {}
            Err(error) => {
                tracing::error!(%error, stage = ?state.stage, "Native inventory smoke failed");
                state.stage = Stage::Done;
                world.write_message(AppExit::error());
            }
        }
        world.resource_mut::<CameraInputState>().captured = !world.resource::<PlayerInputBlock>().0;
    });
}

fn advance(world: &mut World, state: &mut InventorySmoke) -> Result<bool, String> {
    let timeout = if state.stage == Stage::Loading {
        300
    } else {
        150
    };
    if state.started.elapsed() > Duration::from_secs(timeout) || state.capture_error {
        return Err(format!(
            "Inventory smoke timed out or captured a blank image: stage={:?}, elapsed={:.1}s, pending_previews={}, rendered_blocks={}",
            state.stage,
            state.started.elapsed().as_secs_f32(),
            world
                .resource::<terra_render::RenderBlockPreviews>()
                .pending,
            world
                .resource::<terra_render::RenderBlockPreviews>()
                .images
                .len(),
        ));
    }
    if let Some(error) = world
        .resource::<ClientSession>()
        .error
        .as_ref()
        .or(world
            .resource::<terra_render::RenderStatus>()
            .error
            .as_ref())
        .or(world
            .resource::<terra_render::RenderBlockPreviews>()
            .error
            .as_ref())
    {
        return Err(error.clone());
    }
    match state.stage {
        Stage::Loading => {
            if !world.contains_resource::<terra_server::IntegratedServer>() {
                return Err("Inventory smoke requires a real integrated server".into());
            }
            let hud_only = std::env::var_os("TERRA_INVENTORY_HUD_ONLY").is_some();
            if state.elapsed(10.0) {
                let shell = element(world, "inventory-shell");
                let backdrop_present = element(world, "inventory-backdrop").is_some();
                info!(
                    hud_only,
                    session_ready = world.resource::<ClientSession>().ready,
                    world_ready = world.resource::<terra_render::RenderStatus>().ready,
                    player_preview_ready = world.resource::<terra_render::PlayerPreviewImage>().ready,
                    block_previews_pending = world.resource::<terra_render::RenderBlockPreviews>().pending,
                    prop_previews = world.resource::<terra_render::RenderPropPreviews>().images.len(),
                    page = ?world.resource::<MenuState>().page,
                    inventory_prewarming = world.resource::<InventoryView>().prewarming,
                    inventory_shell_present = shell.is_some(),
                    inventory_shell_parked = shell.is_some_and(|shell| world.get::<terra_ui::ParkedUi>(shell).is_some()),
                    inventory_backdrop_present = backdrop_present,
                    "Native inventory smoke loading readiness"
                );
                state.entered = Instant::now();
            }
            if !world.resource::<ClientSession>().ready
                || !world.resource::<terra_render::RenderStatus>().ready
                || !world.resource::<terra_render::PlayerPreviewImage>().ready
                || world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .pending
                    != 0
                // HUD-only checks never request the offscreen prop catalog. Its
                // lazy thumbnails are unrelated to HUD or retained inventory readiness.
                || (!hud_only
                    && world
                        .resource::<terra_render::RenderPropPreviews>()
                        .images
                        .len()
                        < 6)
            {
                return Ok(false);
            }
            if !world.resource::<MenuState>().session_active {
                world.resource_mut::<MenuState>().session_active = true;
            }
            if world.resource::<MenuState>().page != MenuPage::Playing {
                world.write_message(MenuAction::Command("resume".into()));
                return Ok(false);
            }
            state.creative =
                world.resource::<ClientSession>().game_mode == terra_core::GameMode::Creative;
            let Some(backdrop) = element(world, "inventory-backdrop") else {
                return Ok(false);
            };
            if world.resource::<InventoryView>().prewarming
                || element(world, "inventory-shell")
                    .is_none_or(|shell| world.get::<terra_ui::ParkedUi>(shell).is_none())
            {
                return Ok(false);
            }
            require_closed_inventory(world)?;
            state.cached_backdrop = Some(backdrop);
            state.cached_shell = element(world, "inventory-shell");
            if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                world.write_message(terra_world::streaming::BlockEditRequest {
                    position: [990_000, 0, 990_000],
                    expected_material: 0,
                    material: 3,
                });
            }
            resize(
                world,
                if std::env::var_os("TERRA_INVENTORY_4K").is_some() {
                    (3840, 2160)
                } else {
                    (1280, 800)
                },
            );
            if hud_only {
                state.target = element(world, "inventory-hud");
                hud_toggle_key(world)?;
                state.started = Instant::now();
                state.advance(Stage::HudHidden);
                return Ok(false);
            }
            world.write_message(InventoryAction::Toggle);
            state.started = Instant::now();
            state.advance(Stage::Open);
        }
        Stage::Open if state.elapsed(0.8) => {
            if element(world, "inventory-backdrop") != state.cached_backdrop {
                return Err("Inventory replaced its retained backdrop on opening".into());
            }
            if !world.resource::<InventoryView>().open || !world.resource::<PlayerInputBlock>().0 {
                return Err("Inventory did not block gameplay".into());
            }
            require_previews(world)?;
            require_layout(world)?;
            if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                let viewport = world
                    .query_filtered::<&Window, With<PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .ok_or("Missing window")?
                    .physical_size()
                    .as_vec2();
                let ready = world
                    .query::<(
                        Entity,
                        &tilt_ui::ToastState,
                        &ComputedNode,
                        &bevy::ui::UiGlobalTransform,
                    )>()
                    .iter(world)
                    .any(|(entity, toast, node, transform)| {
                        toast.open
                            && toast.kind == tilt_ui::ToastKind::Error
                            && !node.is_empty()
                            && transform.translation.x > viewport.x * 0.7
                            && transform.translation.y - node.size().y * 0.5 < viewport.y * 0.15
                            && world
                                .get::<Node>(entity)
                                .is_some_and(|node| node.flex_direction == FlexDirection::Column)
                            && world
                                .get::<tilt_ui::CascadedStyle>(entity)
                                .and_then(|style| style.0.background_color)
                                .is_some_and(|color| color.alpha > 0.5)
                    });
                if !ready {
                    let diagnostics = world
                        .query::<(
                            Entity,
                            &tilt_ui::ToastState,
                            &Node,
                            &ComputedNode,
                            &bevy::ui::UiGlobalTransform,
                        )>()
                        .iter(world)
                        .map(|(entity, toast, node, computed, transform)| {
                            (
                                entity,
                                toast.kind,
                                toast.open,
                                node.flex_direction,
                                computed.size(),
                                transform.translation,
                                world
                                    .get::<tilt_ui::ComponentStyleOwner>(entity)
                                    .map(|owner| owner.0),
                                world
                                    .get::<tilt_ui::CascadedStyle>(entity)
                                    .and_then(|style| style.0.background_color),
                            )
                        })
                        .collect::<Vec<_>>();
                    error!(?diagnostics, "Toast styling diagnostics");
                    return Err(
                        "The real rejected edit did not produce a top-right TiltUI error toast"
                            .into(),
                    );
                }
            }
            capture(
                world,
                if std::env::var_os("TERRA_INVENTORY_NOTICE").is_some() {
                    "network-error-toast"
                } else {
                    "inventory-open"
                },
            )?;
            if std::env::var_os("TERRA_INVENTORY_LAYOUT_ONLY").is_some() {
                if std::env::var_os("TERRA_INVENTORY_4K").is_some()
                    && world.resource::<InventoryView>().browser_page != "1 / 1"
                {
                    return Err("The current item catalog must fit on one 4K page".into());
                }
                state.advance(Stage::LayoutCapture);
            } else if state.creative {
                state.category_all_items = world
                    .resource::<InventoryView>()
                    .browser_items
                    .iter()
                    .map(|item| item.item_id.clone())
                    .collect();
                if world.resource::<InventoryView>().category_dropdown {
                    begin_category_dropdown(world, state, "tools", Stage::CategorySelected)?;
                } else {
                    let preview = world
                        .resource::<InventoryView>()
                        .categories
                        .iter()
                        .find(|category| category.id == "tools")
                        .ok_or("Tools category missing")?
                        .preview_key
                        .clone();
                    let target =
                        category_target(world, &preview).ok_or("Tools category tab missing")?;
                    pointer(world, target, None)?;
                    state.target = Some(target);
                    state.advance(Stage::CategoryHover);
                }
            } else {
                if world.resource::<InventoryView>().items.len() != 18 {
                    return Err("Survival did not display 18 backpack and 9 hotbar slots".into());
                }
                state.advance(Stage::Wide);
            }
        }
        Stage::LayoutCapture if state.captures > 0 && state.elapsed(0.3) => {
            info!(items=world.resource::<InventoryView>().browser_items.len(),page=%world.resource::<InventoryView>().browser_page,"Native layout-only capture passed; no pointer acceptance claimed");
            return Ok(true);
        }
        Stage::CategoryHover if state.elapsed(0.6) => {
            let target = state.target.ok_or("Missing category tab")?;
            require_popup(world, target, "inventory-category-tooltip", "Tools")?;
            if subtree_text(world, target, "Tools") {
                return Err("Category title is permanently rendered inside tab".into());
            }
            capture(world, "inventory-category-hover")?;
            pointer(world, target, Some(true))?;
            state.advance(Stage::CategoryPress);
        }
        Stage::CategoryPress if state.elapsed(0.08) => {
            pointer(
                world,
                state.target.ok_or("Missing category tab")?,
                Some(false),
            )?;
            state.advance(Stage::CategoryRelease);
        }
        Stage::CategoryRelease if state.elapsed(0.2) => state.advance(Stage::CategorySelected),
        Stage::CategoryDropdownRelease if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing category dropdown")?,
                Some(false),
            )?;
            state.advance(Stage::CategoryDropdownOpen);
        }
        Stage::CategoryDropdownOpen if state.elapsed(0.3) => {
            let choice = element(world, "inventory-category-select")
                .ok_or("Category dropdown disappeared")?;
            let parts = world
                .get::<tilt_ui::ChoiceBoxParts>(choice)
                .copied()
                .ok_or("Missing TiltUI category dropdown parts")?;
            if !parts.open {
                return Err("Native click did not open the category dropdown".into());
            }
            let option = world
                .query::<(Entity, &tilt_ui::OptionData)>()
                .iter(world)
                .find_map(|(entity, option)| {
                    (descendant_of(world, entity, parts.popup) && option.value == state.category_id)
                        .then_some(entity)
                })
                .ok_or_else(|| format!("Missing real category option {}", state.category_id))?;
            let bounds = |entity| -> Result<Rect, String> {
                Ok(Rect::from_center_size(
                    world
                        .get::<bevy::ui::UiGlobalTransform>(entity)
                        .ok_or("Missing category option transform")?
                        .translation,
                    world
                        .get::<ComputedNode>(entity)
                        .ok_or("Missing category option bounds")?
                        .size(),
                ))
            };
            let viewport = bounds(parts.popup)?;
            let row = bounds(option)?;
            if row.center().y < viewport.min.y + 2.0 || row.center().y > viewport.max.y - 2.0 {
                if state.category_scrolls >= 12 {
                    return Err(format!(
                        "Category option is clipped after native scrolling: {} {row:?} within {viewport:?}",
                        state.category_id
                    ));
                }
                state.category_scrolls += 1;
                let wheel = if row.center().y < viewport.center().y {
                    2
                } else {
                    -2
                };
                pointer(world, parts.popup, None)?;
                os_pointer(
                    world,
                    &[tilt_ui::serde_json::json!({"wheel":wheel,"wait":0.1})],
                )?;
                state.entered = Instant::now();
                return Ok(false);
            }
            pointer(world, option, Some(true))?;
            state.target = Some(option);
            state.advance(Stage::CategoryOptionRelease);
        }
        Stage::CategoryOptionRelease if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing selected category option")?,
                Some(false),
            )?;
            state.advance(Stage::CategoryOptionConfirm);
        }
        Stage::CategoryOptionConfirm => {
            let choice = element(world, "inventory-category-select")
                .ok_or("Category dropdown disappeared after release")?;
            let parts = world
                .get::<tilt_ui::ChoiceBoxParts>(choice)
                .copied()
                .ok_or("Missing category dropdown after release")?;
            let checked = world
                .query::<(Entity, &tilt_ui::OptionData, &tilt_ui::ControlChecked)>()
                .iter(world)
                .filter_map(|(entity, option, selected)| {
                    (selected.0 && descendant_of(world, entity, parts.popup))
                        .then_some(option.value.clone())
                })
                .collect::<Vec<_>>();
            let view = world.resource::<InventoryView>();
            let selected = view
                .categories
                .iter()
                .filter(|category| category.selected)
                .map(|category| category.id.clone())
                .collect::<Vec<_>>();
            let ids = view
                .browser_items
                .iter()
                .map(|item| item.item_id.clone())
                .collect::<Vec<_>>();
            let ready = !parts.open
                && checked == [state.category_id]
                && selected == [state.category_id]
                && !ids.is_empty()
                && (state.category_id == "all"
                    || view
                        .browser_items
                        .iter()
                        .all(|item| item.category == state.category_id));
            let page = view.browser_page.clone();
            let presentation_error = if ready {
                require_previews(world).err()
            } else {
                None
            };
            if ready && presentation_error.is_none() {
                info!(category=state.category_id, ?checked, ?selected, ?ids, %page,
                    "Native category dropdown confirmed by the actual control, inventory filter and rendered previews");
                let next = state.category_return;
                state.advance(next);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Native category dropdown did not settle: expected={}, open={}, checked={checked:?}, current_category={selected:?}, ids={ids:?}, page={page}, presentation={presentation_error:?}",
                    state.category_id, parts.open,
                ));
            }
        }
        Stage::CategorySelected => {
            let palette = world.resource::<terra_player::interaction::BlockPalette>();
            let expected = ItemKind::EXTRA
                .into_iter()
                .filter(|item| {
                    palette
                        .item_categories
                        .get(item)
                        .is_some_and(|category| category == "tools")
                })
                .map(|item| format!("Item preview {}", item.identifier()))
                .collect::<Vec<_>>();
            let view = world.resource::<InventoryView>();
            let pages = expected.len().div_ceil(view.browser_capacity.max(1));
            let expected = expected
                .into_iter()
                .take(view.browser_capacity)
                .collect::<Vec<_>>();
            if view.browser_items.iter().any(|item| !item.count.is_empty()) {
                return Err("Unlimited Creative catalog items display finite stack counts".into());
            }
            if !view
                .categories
                .iter()
                .any(|category| category.id == "tools" && category.selected)
                || view.browser_items.is_empty()
                || view.browser_page != format!("1 / {pages}")
                || view.browser_previous
                || view.browser_next != (pages > 1)
                || view
                    .browser_items
                    .iter()
                    .any(|item| item.category != "tools")
                || view
                    .browser_items
                    .iter()
                    .map(|item| item.preview_key.clone())
                    .collect::<Vec<_>>()
                    != expected
                || view
                    .browser_items
                    .iter()
                    .map(|item| item.item_id.clone())
                    .collect::<Vec<_>>()
                    == state.category_all_items
            {
                return Err(format!(
                    "Native category click did not filter actual inventory data: current_category={:?}, ids={:?}, expected_tool_previews={expected:?}, actual_previews={:?}, all_ids={:?}, page={}",
                    view.categories
                        .iter()
                        .filter(|category| category.selected)
                        .map(|category| &category.id)
                        .collect::<Vec<_>>(),
                    view.browser_items
                        .iter()
                        .map(|item| &item.item_id)
                        .collect::<Vec<_>>(),
                    view.browser_items
                        .iter()
                        .map(|item| &item.preview_key)
                        .collect::<Vec<_>>(),
                    state.category_all_items,
                    view.browser_page,
                ));
            }
            let dropdown = view.category_dropdown;
            require_previews(world)?;
            if dropdown {
                begin_category_dropdown(world, state, "all", Stage::AllRelease)?;
            } else {
                let preview = world
                    .resource::<InventoryView>()
                    .categories
                    .iter()
                    .find(|category| category.id == "all")
                    .ok_or("All category missing")?
                    .preview_key
                    .clone();
                let target = category_target(world, &preview).ok_or("All category tab missing")?;
                pointer(world, target, Some(true))?;
                state.target = Some(target);
                state.advance(Stage::AllPress);
            }
        }
        Stage::AllPress if state.elapsed(0.08) => {
            pointer(world, state.target.ok_or("Missing All tab")?, Some(false))?;
            state.advance(Stage::AllRelease);
        }
        Stage::AllRelease if state.elapsed(0.2) => {
            let view = world.resource::<InventoryView>();
            if !view
                .categories
                .iter()
                .any(|category| category.id == "all" && category.selected)
                || view
                    .browser_items
                    .iter()
                    .map(|item| item.item_id.clone())
                    .collect::<Vec<_>>()
                    != state.category_all_items
            {
                return Err("All category did not restore the original real item page".into());
            }
            require_previews(world)?;
            let target =
                item_target(world, "Block preview 3").ok_or("Stone catalog slot missing")?;
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::Hover);
        }
        Stage::Hover if state.elapsed(0.6) => {
            require_tooltip(
                world,
                state.target.ok_or("Missing hovered item")?,
                "teramic::stone",
            )?;
            pointer(
                world,
                state.target.ok_or("Missing pointer target")?,
                Some(true),
            )?;
            state.advance(Stage::Press);
        }
        Stage::Press if state.elapsed(0.08) => {
            pointer(
                world,
                state.target.ok_or("Missing pointer target")?,
                Some(false),
            )?;
            state.advance(Stage::Release);
        }
        Stage::Release if state.elapsed(0.15) => state.advance(Stage::SingleGrant),
        Stage::SingleGrant => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_none()
                && inventory
                    .slot(0)
                    .is_some_and(|stack| stack.item == ItemKind::Block(3) && stack.count == 1)
                && inventory.count(ItemKind::Block(3)) == 1
            {
                let target = hotbar_target(world, 0).ok_or("Missing granted hotbar slot")?;
                pointer(world, target, Some(true))?;
                state.target = Some(target);
                state.advance(Stage::SinglePickupPress);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Catalog pointer did not grant exactly one item into the first free hotbar slot: {inventory:?}"
                ));
            }
        }
        Stage::SinglePickupPress if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing granted hotbar target")?,
                Some(false),
            )?;
            state.advance(Stage::SinglePickupRelease);
        }
        Stage::SinglePickupRelease if state.elapsed(0.2) => {
            state.advance(Stage::SinglePickupConfirm);
        }
        Stage::SinglePickupConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.slot(0).is_none()
                && inventory
                    .cursor
                    .is_some_and(|stack| stack.item == ItemKind::Block(3) && stack.count == 1)
            {
                // The real catalog click grants one hotbar item; seed the existing
                // conservation fixture only after picking that item up through the UI.
                let revision = world.resource::<ClientSession>().inventory.revision;
                world.write_message(terra_network::client::InventoryRequest(
                    terra_network::protocol::inventory::InventoryCommand {
                        revision,
                        action:
                            terra_network::protocol::inventory::InventoryAction::CreativeCursor {
                                item: ItemKind::Block(3),
                                count: 128,
                            },
                    },
                ));
                tracing::info!(
                    "Native catalog single-item hotbar grant and pickup confirmed; seeding drag conservation stack"
                );
                state.advance(Stage::Cursor);
            } else if state.elapsed(5.0) {
                return Err(format!("Native single-item pickup failed: {inventory:?}"));
            }
        }
        Stage::Cursor => {
            if world
                .resource::<ClientSession>()
                .inventory
                .cursor
                .is_some_and(|stack| stack.item == ItemKind::Block(3) && stack.count == 128)
            {
                let Some(cursor) = element(world, "inventory-cursor") else {
                    return Ok(false);
                };
                if !subtree_image(world, cursor, "Block preview 3")
                    || !subtree_text(world, cursor, "128")
                {
                    return Ok(false);
                }
                let cursor_size = world
                    .get::<ComputedNode>(cursor)
                    .ok_or("Held cursor has no layout")?
                    .size();
                if cursor_size.min_element() < 20.0 {
                    return Ok(false);
                }
                if world
                    .resource::<ClientSession>()
                    .inventory
                    .slot(0)
                    .is_some()
                {
                    return Err("Single-item pickup left a duplicate hotbar stack".into());
                }
                if world.resource::<InventoryView>().category_dropdown {
                    begin_category_dropdown(world, state, "all", Stage::HeldTabConfirm)?;
                } else {
                    let preview = world
                        .resource::<InventoryView>()
                        .categories
                        .iter()
                        .find(|category| category.id == "all")
                        .ok_or("All category missing")?
                        .preview_key
                        .clone();
                    let target =
                        category_target(world, &preview).ok_or("Missing All browser category")?;
                    pointer(world, target, Some(true))?;
                    state.target = Some(target);
                    state.advance(Stage::HeldTabPress);
                }
            } else if state.elapsed(5.0) {
                return Err("Creative seed did not create an authoritative cursor stack".into());
            }
        }
        Stage::HeldTabPress if state.elapsed(0.08) => {
            pointer(
                world,
                state.target.ok_or("Missing held-cursor tab target")?,
                Some(false),
            )?;
            state.advance(Stage::HeldTabRelease);
        }
        Stage::HeldTabRelease if state.elapsed(0.25) => state.advance(Stage::HeldTabConfirm),
        Stage::HeldTabConfirm => {
            let session = world.resource::<ClientSession>();
            if session
                .inventory
                .cursor
                .is_none_or(|stack| stack.item != ItemKind::Block(3) || stack.count != 128)
                || session
                    .drops
                    .iter()
                    .any(|drop| drop.stack.item == ItemKind::Block(3))
            {
                return Err(
                    "Clicking browser category tabs dropped or changed the held stack".into(),
                );
            }
            let target = hotbar_target(world, 0).ok_or("Missing first hotbar slot")?;
            pointer(world, target, Some(true))?;
            state.target = Some(target);
            state.advance(Stage::PlacePress);
        }
        Stage::PlacePress if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing hotbar target")?,
                Some(false),
            )?;
            state.advance(Stage::PlaceRelease);
        }
        Stage::PlaceRelease if state.elapsed(0.2) => state.advance(Stage::Grant),
        Stage::Grant => {
            if world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .is_some_and(|stack| stack.item == ItemKind::Block(3) && stack.count == 128)
            {
                tracing::info!(
                    revision = world.resource::<ClientSession>().inventory.revision,
                    "Native inventory creative stack confirmed by server"
                );
                if world.resource::<ClientSession>().inventory.cursor.is_some() {
                    return Err("Placing a full cursor stack left ghost items".into());
                }
                let target = hotbar_target(world, 0).ok_or("Filled hotbar slot missing")?;
                pointer_button(world, target, MouseButton::Right, Some(true))?;
                state.target = Some(target);
                state.advance(Stage::HalfPress);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Real TiltUI slot click did not request an authoritative stack: active={:?}, outcome={:?}, inventory={:?}, detail={:?}",
                    world.resource::<terra_player::interaction::ActiveItem>().0,
                    world.resource::<ClientSession>().last_inventory,
                    world.resource::<ClientSession>().inventory,
                    world.resource::<InventoryView>().detail_name
                ));
            }
        }
        Stage::HalfPress if state.elapsed(0.1) => {
            pointer_button(
                world,
                state.target.ok_or("Missing split target")?,
                MouseButton::Right,
                Some(false),
            )?;
            state.advance(Stage::HalfRelease);
        }
        Stage::HalfRelease if state.elapsed(0.2) => state.advance(Stage::HalfConfirm),
        Stage::HalfConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_some_and(|stack| stack.count == 64)
                && inventory.slot(0).is_some_and(|stack| stack.count == 64)
            {
                let target = hotbar_target(world, 1).ok_or("Second hotbar slot missing")?;
                pointer_button(world, target, MouseButton::Right, Some(true))?;
                state.target = Some(target);
                state.advance(Stage::OnePress);
            } else if state.elapsed(5.0) {
                return Err("Right-click did not split the stack in half".into());
            }
        }
        Stage::OnePress if state.elapsed(0.1) => {
            pointer_button(
                world,
                state.target.ok_or("Missing single-place target")?,
                MouseButton::Right,
                Some(false),
            )?;
            state.advance(Stage::OneRelease);
        }
        Stage::OneRelease if state.elapsed(0.2) => state.advance(Stage::OneConfirm),
        Stage::OneConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_some_and(|stack| stack.count == 63)
                && inventory.slot(1).is_some_and(|stack| stack.count == 1)
            {
                let target = hotbar_target(world, 2).ok_or("Third hotbar slot missing")?;
                pointer(world, target, Some(true))?;
                state.advance(Stage::DragStart);
            } else if state.elapsed(5.0) {
                return Err("Right-click did not place exactly one item".into());
            }
        }
        Stage::DragStart if state.elapsed(0.15) => {
            let target = hotbar_target(world, 3).ok_or("Fourth hotbar slot missing")?;
            pointer(world, target, None)?;
            state.advance(Stage::DragMove);
        }
        Stage::DragMove if state.elapsed(0.15) => {
            let target = hotbar_target(world, 4).ok_or("Fifth hotbar slot missing")?;
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::DragRelease);
        }
        Stage::DragRelease if state.elapsed(0.15) => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_none_or(|stack| stack.count != 63)
                || (2..=4).any(|slot| inventory.slot(slot).is_some())
            {
                return Err("Held drag changed authoritative items before release".into());
            }
            for index in 2..=4 {
                let slot = &world.resource::<InventoryView>().hotbar[index];
                if slot.empty || slot.count != "21" {
                    return Err("Held drag did not show its live slot counts".into());
                }
                let target = hotbar_target(world, index).ok_or("Held preview slot missing")?;
                require_slot_presentation(world, target, "21")?;
            }
            capture(world, "inventory-held-drag-preview")?;
            pointer(
                world,
                state.target.ok_or("Missing drag target")?,
                Some(false),
            )?;
            state.advance(Stage::DragConfirm);
        }
        Stage::DragConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_none()
                && (2..=4).all(|slot| inventory.slot(slot).is_some_and(|stack| stack.count == 21))
            {
                tracing::info!(
                    "Native inventory cursor pickup, split, single place, and even drag confirmed by server"
                );
                let target = hotbar_target(world, 2).ok_or("Sweep source slot missing")?;
                pointer(world, target, Some(true))?;
                state.advance(Stage::SweepStart);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Left drag did not distribute 63 items evenly: {inventory:?}"
                ));
            }
        }
        Stage::SweepStart if state.elapsed(0.2) => {
            let target = hotbar_target(world, 3).ok_or("Sweep middle slot missing")?;
            pointer(world, target, None)?;
            state.advance(Stage::SweepMove);
        }
        Stage::SweepMove if state.elapsed(0.2) => {
            let target = hotbar_target(world, 4).ok_or("Sweep final slot missing")?;
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::SweepRelease);
        }
        Stage::SweepRelease if state.elapsed(0.2) => {
            pointer(
                world,
                state.target.ok_or("Missing sweep target")?,
                Some(false),
            )?;
            state.advance(Stage::SweepConfirm);
        }
        Stage::SweepConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_some_and(|stack| stack.count == 63)
                && (2..=4).all(|slot| inventory.slot(slot).is_none())
            {
                let target = hotbar_target(world, 2).ok_or("Sweep return slot missing")?;
                pointer(world, target, Some(true))?;
                state.target = Some(target);
                state.advance(Stage::SweepReturnPress);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "LMB pickup sweep did not collect exactly the three visited stacks: {inventory:?}"
                ));
            }
        }
        Stage::SweepReturnPress if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing sweep return target")?,
                Some(false),
            )?;
            state.advance(Stage::SweepReturnRelease);
        }
        Stage::SweepReturnRelease if state.elapsed(0.2) => state.advance(Stage::SweepReturnConfirm),
        Stage::SweepReturnConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory.cursor.is_none()
                && inventory.slot(2).is_some_and(|stack| stack.count == 63)
                && inventory.count(ItemKind::Block(3)) == 128
            {
                tracing::info!(
                    "Native MouseTweaks LMB pickup sweep conserved all 128 items across real server acknowledgements"
                );
                capture(world, "inventory-creative-wide")?;
                state.advance(Stage::OutsidePickupStart);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Sweep return did not preserve total stack counts: {inventory:?}"
                ));
            }
        }
        Stage::OutsidePickupStart if state.elapsed(0.4) => {
            let target = hotbar_target(world, 2).ok_or("Outside-drop source slot missing")?;
            pointer(world, target, Some(true))?;
            state.target = Some(target);
            state.advance(Stage::OutsidePickupPress);
        }
        Stage::OutsidePickupPress if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Outside-drop pickup target missing")?,
                Some(false),
            )?;
            state.advance(Stage::OutsidePickupRelease);
        }
        Stage::OutsidePickupRelease if state.elapsed(0.2) => {
            state.advance(Stage::OutsidePickupConfirm)
        }
        Stage::OutsidePickupConfirm => {
            let inventory = &world.resource::<ClientSession>().inventory;
            if inventory
                .cursor
                .is_some_and(|stack| stack.item == ItemKind::Block(3) && stack.count == 63)
                && inventory.slot(2).is_none()
            {
                pointer_at(world, Vec2::splat(4.0), MouseButton::Left, Some(true))?;
                state.advance(Stage::OutsideRelease);
            } else if state.elapsed(5.0) {
                return Err(format!("Outside-drop pickup failed: {inventory:?}"));
            }
        }
        Stage::OutsideRelease if state.elapsed(0.1) => {
            pointer_at(world, Vec2::splat(4.0), MouseButton::Left, Some(false))?;
            state.advance(Stage::OutsideConfirm);
        }
        Stage::OutsideConfirm => {
            let session = world.resource::<ClientSession>();
            if session.inventory.cursor.is_none()
                && session.inventory.count(ItemKind::Block(3)) == 65
                && session
                    .drops
                    .iter()
                    .any(|drop| drop.stack.item == ItemKind::Block(3) && drop.stack.count == 63)
            {
                tracing::info!(
                    "Native outside click dropped the complete held stack: 65 inventory + 63 replicated world drop = 128"
                );
                state.advance(Stage::CraftGrid);
            } else if state.elapsed(5.0) {
                return Err(format!(
                    "Outside click did not preserve the held stack as a real world drop: inventory={:?}, drops={:?}, outcome={:?}",
                    session.inventory, session.drops, session.last_inventory
                ));
            }
        }
        Stage::CraftGrid => {
            if !crafting_pointer_probe(world, state)? {
                return Ok(false);
            }
            state.advance(Stage::Wide);
        }
        Stage::Wide if state.captures >= 1 && state.elapsed(0.3) => {
            let target = if state.creative {
                hotbar_target(world, 0).ok_or("Missing occupied hotbar tooltip target")?
            } else {
                item_target(world, "Block preview 3").ok_or("Missing Survival browser item")?
            };
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::SlotHover);
        }
        Stage::SlotHover if state.elapsed(0.8) => {
            require_tooltip(
                world,
                state.target.ok_or("Missing occupied slot hover")?,
                "teramic::stone",
            )?;
            capture(
                world,
                if state.creative {
                    "inventory-hotbar-tooltip"
                } else {
                    "inventory-survival-browser-tooltip"
                },
            )?;
            state.advance(Stage::WideResume);
        }
        Stage::WideResume => {
            resize(world, (960, 640));
            state.advance(Stage::SearchFocus);
        }
        Stage::SearchFocus if state.elapsed(0.5) => {
            // Responsive TiltUI may recreate the editor during the resize. Focus
            // the current rendered entity, after that layout commit has settled.
            let search =
                element(world, "inventory-search").ok_or("Missing TiltUI search editor")?;
            world
                .resource_mut::<InputFocus>()
                .set(search, bevy::input_focus::FocusCause::Navigated);
            type_text(world, "mushroom")?;
            state.advance(Stage::Search);
        }
        Stage::Search if state.elapsed(0.5) => {
            let view = world.resource::<InventoryView>();
            if view.search != "mushroom"
                || view.browser_items.is_empty()
                || view.browser_items.iter().any(|slot| {
                    !slot.name.to_lowercase().contains("mushroom")
                        && !slot.identifier.contains("mushroom")
                })
            {
                return Err(format!(
                    "Native search keyboard input did not filter the catalog: query={:?}, items={}",
                    view.search,
                    view.browser_items.len()
                ));
            }
            let target =
                item_target(world, "Block preview 21").ok_or("Purple mushroom cap slot missing")?;
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::Compact);
        }
        Stage::Compact if state.elapsed(0.8) => {
            require_layout(world)?;
            require_tooltip(
                world,
                state.target.ok_or("Missing compact hover")?,
                &ItemKind::Block(21).identifier(),
            )?;
            capture(
                world,
                if state.creative {
                    "inventory-creative-compact"
                } else {
                    "inventory-survival-compact"
                },
            )?;
            state.advance(Stage::Capture);
        }
        Stage::Capture if state.captures >= 2 => {
            resize(world, (960, 360));
            state.advance(Stage::ScrollSetup);
        }
        Stage::ScrollSetup if state.elapsed(0.6) => {
            let scroll = element(world, "inventory-scroll").ok_or("Missing slot scroll area")?;
            let computed = world
                .get::<ComputedNode>(scroll)
                .copied()
                .ok_or("Missing scroll layout")?;
            tracing::info!(
                ui_scale = world.resource::<bevy::ui::UiScale>().0,
                inverse_scale = computed.inverse_scale_factor(),
                viewport = ?computed.size(), content = ?computed.content_size(),
                "Native scroll fixture before constraining its test viewport"
            );
            for (entity, id, node) in world
                .query::<(Entity, &ElementId, &ComputedNode)>()
                .iter(world)
            {
                if descendant_of(world, entity, scroll)
                    && matches!(
                        id.0.as_str(),
                        "inventory-equipment" | "inventory-grid" | "inventory-hotbar"
                    )
                {
                    tracing::info!(element = id.0, size = ?node.size(), content = ?node.content_size(), "Native scroll fixture child layout");
                }
            }
            let height = Val::Px(120.0 * computed.inverse_scale_factor());
            world
                .entity_mut(scroll)
                .insert(tilt_ui::WidgetLayoutOverride {
                    height: Some(height),
                    max_height: Some(height),
                    ..default()
                });
            if let Some(mut node) = world.get_mut::<Node>(scroll) {
                node.height = height;
                node.max_height = height;
                node.min_height = Val::Px(0.0);
            }
            state.advance(Stage::ScrollConstrained);
        }
        Stage::ScrollConstrained if state.elapsed(0.6) => {
            let scroll = element(world, "inventory-scroll").ok_or("Missing slot scroll area")?;
            let node = world
                .get::<ComputedNode>(scroll)
                .ok_or("Missing scroll layout")?;
            if node.content_size().y > node.size().y + 2.0 {
                state.scroll_revision = world.resource::<ClientSession>().inventory.revision;
                state.scroll_attempts = 1;
                scroll_inventory(world, -12)?;
                state.advance(Stage::ScrollDown);
            } else {
                return Err(format!(
                    "Short viewport must create a real inventory scroll range: content={:?}, viewport={:?}",
                    node.content_size(),
                    node.size()
                ));
            }
        }
        Stage::ScrollDown if state.elapsed(0.6) => {
            let scroll = element(world, "inventory-scroll").ok_or("Missing slot scroll area")?;
            if !world
                .get::<bevy::ui::ScrollPosition>(scroll)
                .is_some_and(|position| position.0.y > 1.0)
            {
                return Err("Real native wheel did not scroll the overflowing inventory".into());
            }
            let target = hotbar_target(world, 8).ok_or("Missing final hotbar slot")?;
            let bounds = |entity| -> Result<Rect, String> {
                Ok(Rect::from_center_size(
                    world
                        .get::<bevy::ui::UiGlobalTransform>(entity)
                        .ok_or("Missing scroll target transform")?
                        .translation,
                    world
                        .get::<ComputedNode>(entity)
                        .ok_or("Missing scroll target layout")?
                        .size(),
                ))
            };
            let viewport = bounds(scroll)?;
            let slot = bounds(target)?;
            if slot.min.y < viewport.min.y - 2.0 || slot.max.y > viewport.max.y + 2.0 {
                if state.scroll_attempts < 8 {
                    state.scroll_attempts += 1;
                    scroll_inventory(world, -12)?;
                    state.entered = Instant::now();
                    return Ok(false);
                }
                return Err(format!(
                    "Last inventory row remains clipped after scrolling: {slot:?}, viewport={viewport:?}"
                ));
            }
            if world.resource::<ClientSession>().inventory.revision != state.scroll_revision {
                return Err("Plain wheel scrolling unexpectedly changed inventory items".into());
            }
            pointer(world, target, None)?;
            state.target = Some(target);
            state.advance(Stage::ScrollHover);
        }
        Stage::ScrollHover if state.elapsed(0.4) => {
            let target = state.target.ok_or("Missing scrolled hotbar target")?;
            if !world
                .get::<tilt_ui::ElementState>(target)
                .is_some_and(|state| state.hovered)
            {
                return Err("Scrolled inventory slot is visible but cannot be picked".into());
            }
            capture(world, "inventory-scroll-reachable")?;
            state.scroll_attempts = 1;
            scroll_inventory(world, 12)?;
            state.advance(Stage::ScrollUp);
        }
        Stage::ScrollUp if state.elapsed(0.6) => {
            let scroll = element(world, "inventory-scroll").ok_or("Missing slot scroll area")?;
            if !world
                .get::<bevy::ui::ScrollPosition>(scroll)
                .is_some_and(|position| position.0.y < 1.0)
            {
                if state.scroll_attempts < 8 {
                    state.scroll_attempts += 1;
                    scroll_inventory(world, 12)?;
                    state.entered = Instant::now();
                    return Ok(false);
                }
                return Err("Native wheel did not return inventory scroll to the first row".into());
            }
            if world.resource::<ClientSession>().inventory.revision != state.scroll_revision {
                return Err("Plain wheel scrolling unexpectedly changed inventory items".into());
            }
            if state.creative {
                state.trash_drops = world.resource::<ClientSession>().drops.len();
                state.scroll_attempts = 1;
                scroll_inventory(world, -12)?;
                state.advance(Stage::TrashPickupReady);
            } else {
                world.write_message(InventoryAction::Close);
                state.advance(Stage::Close);
            }
        }
        Stage::TrashPickupReady if state.elapsed(0.6) => {
            let target = hotbar_target(world, 0).ok_or("Missing native trash cursor source")?;
            let scroll =
                element(world, "inventory-scroll").ok_or("Missing trash pickup scroll area")?;
            let bounds = |entity| -> Result<Rect, String> {
                Ok(Rect::from_center_size(
                    world
                        .get::<bevy::ui::UiGlobalTransform>(entity)
                        .ok_or("Missing trash pickup transform")?
                        .translation,
                    world
                        .get::<ComputedNode>(entity)
                        .ok_or("Missing trash pickup bounds")?
                        .size(),
                ))
            };
            let viewport = bounds(scroll)?;
            let slot = bounds(target)?;
            if slot.min.y < viewport.min.y - 2.0 || slot.max.y > viewport.max.y + 2.0 {
                if state.scroll_attempts < 8 {
                    state.scroll_attempts += 1;
                    scroll_inventory(world, -12)?;
                    state.entered = Instant::now();
                    return Ok(false);
                }
                return Err("Native trash pickup source remains clipped after scrolling".into());
            }
            state.grid_count = world
                .resource::<ClientSession>()
                .inventory
                .count(ItemKind::Block(3));
            pointer(world, target, Some(true))?;
            state.target = Some(target);
            state.advance(Stage::TrashPickupRelease);
        }
        Stage::TrashPickupRelease if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing trash cursor source")?,
                Some(false),
            )?;
            state.advance(Stage::TrashPickupConfirm);
        }
        Stage::TrashPickupConfirm => {
            let session = world.resource::<ClientSession>();
            if let Some(held) = session
                .inventory
                .cursor
                .filter(|stack| stack.item == ItemKind::Block(3))
            {
                state.grid_count = state.grid_count.saturating_sub(held.count);
                state.scroll_revision = session.inventory.revision;
                let trash =
                    element(world, "inventory-trash").ok_or("Creative trash button missing")?;
                pointer(world, trash, Some(true))?;
                state.target = Some(trash);
                state.advance(Stage::TrashCursorRelease);
            } else if state.elapsed(5.0) {
                return Err("Native trash probe did not acquire a real cursor stack".into());
            }
        }
        Stage::TrashCursorRelease if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing native trash button")?,
                Some(false),
            )?;
            state.advance(Stage::TrashCursorConfirm);
        }
        Stage::TrashCursorConfirm => {
            let session = world.resource::<ClientSession>();
            if session.inventory.cursor.is_none()
                && session.inventory.revision > state.scroll_revision
            {
                if session.inventory.count(ItemKind::Block(3)) != state.grid_count
                    || session.drops.len() != state.trash_drops
                {
                    return Err(
                        "Creative cursor trash changed inventory stock or spawned a world drop"
                            .into(),
                    );
                }
                let slot = (0..9)
                    .find(|slot| session.inventory.slot(*slot).is_some())
                    .ok_or("Native selected-slot trash needs a real inventory stack")?
                    as u8;
                state.grid_phase = slot;
                world.write_message(InventoryAction::Hotbar(slot));
                state.advance(Stage::TrashSlotReady);
            } else if state.elapsed(5.0) {
                return Err("Creative trash did not delete the authoritative cursor stack".into());
            }
        }
        Stage::TrashSlotReady if state.elapsed(0.3) => {
            state.scroll_revision = world.resource::<ClientSession>().inventory.revision;
            let trash =
                element(world, "inventory-trash").ok_or("Creative selected-stack trash missing")?;
            pointer(world, trash, Some(true))?;
            state.target = Some(trash);
            state.advance(Stage::TrashSlotRelease);
        }
        Stage::TrashSlotRelease if state.elapsed(0.1) => {
            pointer(
                world,
                state.target.ok_or("Missing selected-stack trash button")?,
                Some(false),
            )?;
            state.advance(Stage::TrashSlotConfirm);
        }
        Stage::TrashSlotConfirm => {
            let session = world.resource::<ClientSession>();
            if session
                .inventory
                .slot(usize::from(state.grid_phase))
                .is_none()
                && session.inventory.revision > state.scroll_revision
            {
                if session.drops.len() != state.trash_drops {
                    return Err("Creative selected-stack trash spawned a world drop".into());
                }
                info!(
                    "Native Creative trash deleted cursor and selected inventory stacks through real OS clicks and server acknowledgements"
                );
                capture(world, "inventory-creative-trash")?;
                state.advance(Stage::TrashCapture);
            } else if state.elapsed(5.0) {
                return Err(
                    "Creative trash did not delete the selected real inventory stack".into(),
                );
            }
        }
        Stage::TrashCapture if state.elapsed(0.4) => {
            world.write_message(InventoryAction::Close);
            state.advance(Stage::Close);
        }
        Stage::Close if state.elapsed(0.3) => {
            if world.resource::<InventoryView>().open || world.resource::<PlayerInputBlock>().0 {
                return Err("Inventory close did not return control".into());
            }
            if element(world, "inventory-backdrop") != state.cached_backdrop {
                return Err("Closing the inventory discarded its retained backdrop".into());
            }
            require_closed_inventory(world)?;
            if !state.reopened {
                // The scroll stress test shrinks to 960x360. Restore the normal
                // viewport and its injected height override on the retained tree.
                if let Some(scroll) = element(world, "inventory-scroll") {
                    world
                        .entity_mut(scroll)
                        .remove::<tilt_ui::WidgetLayoutOverride>();
                    if let Some(mut node) = world.get_mut::<Node>(scroll) {
                        node.height = Val::Auto;
                        node.max_height = Val::Auto;
                    }
                    if let Some(mut position) = world.get_mut::<bevy::ui::ScrollPosition>(scroll) {
                        position.0 = Vec2::ZERO;
                    }
                }
                resize(world, (1280, 800));
                world.write_message(InventoryAction::Toggle);
                state.advance(Stage::Reopen);
                return Ok(false);
            }
            state.target = element(world, "inventory-hud-hotbar");
            if state.target.is_none() {
                return Err("World hotbar missing before pause".into());
            }
            world.write_message(MenuAction::Command("pause".into()));
            state.advance(Stage::HudPause);
        }
        Stage::Reopen if state.elapsed(0.8) => {
            if !world.resource::<InventoryView>().open
                || element(world, "inventory-backdrop") != state.cached_backdrop
                || element(world, "inventory-shell") != state.cached_shell
            {
                return Err("Reopening the inventory replaced its cached UI".into());
            }
            require_previews(world)?;
            require_layout(world)?;
            info!(
                "Native inventory reuses the same prewarmed shell and parks hidden reactivity; backdrop and world HUD remain retained"
            );
            state.reopened = true;
            world.write_message(InventoryAction::Close);
            state.advance(Stage::Close);
        }
        Stage::HudPause if state.elapsed(0.6) => {
            if world.resource::<MenuState>().page != MenuPage::Pause
                || element(world, "inventory-hud-hotbar") != state.target
                || element(world, "mini-map").is_none()
            {
                return Err("Pause unmounted or hid the existing world HUD".into());
            }
            capture(world, "hud-pause-overlay")?;
            world.write_message(MenuAction::Command("options".into()));
            state.advance(Stage::HudOptions);
        }
        Stage::HudOptions if state.elapsed(0.6) => {
            if world.resource::<MenuState>().page != MenuPage::Options
                || element(world, "inventory-hud-hotbar") != state.target
                || element(world, "mini-map").is_none()
            {
                return Err("Options unmounted or hid the existing world HUD".into());
            }
            let ready = element(world, "graphics")
                .and_then(|entity| world.get::<ComputedNode>(entity))
                .is_some_and(|node| !node.is_empty());
            if !ready {
                if state.elapsed(5.0) {
                    return Err("Options tab did not finish native layout".into());
                }
                return Ok(false);
            }
            state.advance(Stage::HudOptionsCapture);
        }
        Stage::HudOptionsCapture if state.elapsed(0.6) => {
            capture(world, "hud-options-overlay")?;
            world.write_message(MenuAction::Command("back".into()));
            state.advance(Stage::HudBack);
        }
        Stage::HudBack if state.elapsed(0.3) => {
            world.write_message(MenuAction::Command("resume".into()));
            state.advance(Stage::HudResume);
        }
        Stage::HudResume if state.elapsed(0.3) => {
            if world.resource::<MenuState>().page != MenuPage::Playing
                || element(world, "inventory-hud-hotbar") != state.target
            {
                return Err("World HUD was rebuilt instead of retained below the overlay".into());
            }
            info!("Native pause/options retained the same hotbar entities and visible minimap");
            return Ok(true);
        }
        Stage::HudHidden if state.elapsed(0.5) => {
            require_hud_visibility(world, state.target, false)?;
            capture(world, "hud-f2-hidden")?;
            world.write_message(InventoryAction::Toggle);
            state.advance(Stage::HudHiddenInventory);
        }
        Stage::HudHiddenInventory if state.elapsed(0.8) => {
            require_hud_visibility(world, state.target, false)?;
            if !world.resource::<InventoryView>().open || !world.resource::<PlayerInputBlock>().0 {
                return Err("Hidden HUD prevented opening the inventory".into());
            }
            let viewport = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .ok_or("Missing native inventory window")?
                .physical_size()
                .as_vec2();
            for id in ["inventory-backdrop", "inventory-shell", "inventory-grid"] {
                let entity =
                    element(world, id).ok_or_else(|| format!("Missing {id} with hidden HUD"))?;
                if !world
                    .get::<InheritedVisibility>(entity)
                    .is_some_and(|visibility| visibility.get())
                {
                    return Err(format!("F2 hid the interactive inventory: {id}"));
                }
                let node = world
                    .get::<ComputedNode>(entity)
                    .ok_or("Missing inventory layout")?;
                let transform = world
                    .get::<bevy::ui::UiGlobalTransform>(entity)
                    .ok_or("Missing inventory position")?;
                let bounds = Rect::from_center_size(transform.translation, node.size());
                if !node.size().is_finite()
                    || node.size().min_element() <= 1.0
                    || bounds.min.cmplt(-Vec2::splat(2.0)).any()
                    || bounds.max.cmpgt(viewport + Vec2::splat(2.0)).any()
                {
                    return Err(format!(
                        "Hidden HUD left {id} unusable: bounds={bounds:?}, viewport={viewport:?}"
                    ));
                }
            }
            hud_toggle_key(world)?;
            capture(world, "hud-f2-inventory")?;
            state.advance(Stage::HudRestore);
        }
        Stage::HudRestore if state.elapsed(0.4) => {
            require_hud_visibility(world, state.target, false)?;
            world.write_message(InventoryAction::Close);
            state.advance(Stage::HudRestored);
        }
        Stage::HudRestored if state.elapsed(0.4) => {
            if world.resource::<PlayerInputBlock>().0 {
                return Err("Inventory did not release input".into());
            }
            hud_toggle_key(world)?;
            state.advance(Stage::HudCaptured);
        }
        Stage::HudCaptured if state.elapsed(0.5) => {
            require_hud_visibility(world, state.target, true)?;
            capture(world, "hud-f2-restored")?;
            state.advance(Stage::HudResume);
            // The existing final stage also checks that the hotbar remains retained.
            state.target = element(world, "inventory-hud-hotbar");
        }
        _ => {}
    }
    Ok(false)
}

fn hud_toggle_key(world: &mut World) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing native window")?;
    for state in [ButtonState::Pressed, ButtonState::Released] {
        world.write_message(KeyboardInput {
            key_code: KeyCode::F2,
            logical_key: Key::F2,
            state,
            text: None,
            repeat: false,
            window,
        });
    }
    Ok(())
}

fn require_hud_visibility(
    world: &mut World,
    hotbar: Option<Entity>,
    visible: bool,
) -> Result<(), String> {
    if world.resource::<terra_ui::HudVisibility>().visible != visible
        || element(world, "inventory-hud") != hotbar
    {
        return Err("F2 failed to toggle the retained HUD".into());
    }
    for id in [
        "inventory-hud",
        "mini-map",
        "survival-hud",
        "target-card",
        "debug-overlay",
    ] {
        if let Some(entity) = element(world, id) {
            let rendered = world
                .get::<InheritedVisibility>(entity)
                .is_some_and(|visibility| visibility.get());
            if !visible && rendered {
                return Err(format!("F2 left {id} rendered"));
            }
            if visible && id == "inventory-hud" && !rendered {
                return Err("F2 did not restore the hotbar".into());
            }
        }
    }
    Ok(())
}

fn require_closed_inventory(world: &mut World) -> Result<(), String> {
    let shell = element(world, "inventory-shell")
        .ok_or("Closed inventory did not retain its prewarmed shell")?;
    if world.get::<terra_ui::ParkedUi>(shell).is_none() {
        return Err("Closed inventory kept its retained shell reactive".into());
    }
    let backdrop =
        element(world, "inventory-backdrop").and_then(|entity| world.get::<Node>(entity));
    if backdrop.is_none_or(|node| node.display != Display::None) {
        return Err("Closed inventory remains in layout or pointer picking".into());
    }
    if world
        .query::<&ElementId>()
        .iter(world)
        .filter(|id| id.0 == "inventory-shell")
        .count()
        != 1
    {
        return Err("Inventory cache contains multiple shells".into());
    }
    for (entity, owner) in world
        .query::<(Entity, &terra_ui::TooltipOwner)>()
        .iter(world)
    {
        if world.get::<terra_ui::ParkedUi>(owner.0).is_some()
            && world.get::<Visibility>(entity) != Some(&Visibility::Hidden)
        {
            return Err("Closed inventory left a portalled tooltip visible".into());
        }
    }
    let orphaned = world
        .query::<&terra_ui::TooltipOwner>()
        .iter(world)
        .any(|owner| world.get_entity(owner.0).is_err());
    if orphaned {
        return Err("Closed inventory retained orphaned tooltip entities".into());
    }
    Ok(())
}

fn tooltip_frame_guard(world: &mut World) {
    if world.resource::<InventorySmoke>().capture_error {
        return;
    }
    let Some((pointer, viewport)) = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .and_then(|window| {
            Some((
                window.physical_cursor_position()?,
                window.physical_size().as_vec2(),
            ))
        })
    else {
        return;
    };
    let invalid = world
        .query::<(
            &tilt_ui::TiltElement,
            &Node,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
            &InheritedVisibility,
        )>()
        .iter(world)
        .filter(|(kind, node, size, _, visibility)| {
            kind.kind == tilt_ui::ElementKind::ToolTip
                && node.display != Display::None
                && !size.is_empty()
                && visibility.get()
        })
        .find_map(|(_, _, size, transform, _)| {
            let gap = 10.0 / size.inverse_scale_factor;
            let mut expected = pointer + Vec2::splat(gap);
            for axis in 0..2 {
                if expected[axis] + size.size()[axis] > viewport[axis] - gap {
                    expected[axis] = pointer[axis] - gap - size.size()[axis];
                }
            }
            expected = expected.clamp(Vec2::ZERO, (viewport - size.size()).max(Vec2::ZERO));
            let actual = transform.affine().transform_point2(-size.size() * 0.5);
            (actual.distance(expected) > 2.0).then_some((actual, expected))
        });
    if let Some((actual, expected)) = invalid {
        error!(
            ?actual,
            ?expected,
            "Tooltip was visible at an incorrect position, including its first frame"
        );
        world.resource_mut::<InventorySmoke>().capture_error = true;
    }
}

fn scroll_inventory(world: &mut World, steps: i32) -> Result<(), String> {
    let scroll = element(world, "inventory-scroll").ok_or("Missing slot scroll area")?;
    let center = world
        .get::<bevy::ui::UiGlobalTransform>(scroll)
        .ok_or("Missing slot scroll transform")?
        .translation;
    pointer_at(world, center, MouseButton::Left, None)?;
    os_pointer(
        world,
        &[tilt_ui::serde_json::json!({"wheel":steps,"wait":0.1})],
    )
}

fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, value)| (value.0 == id).then_some(entity))
}
fn subtree_image(world: &World, entity: Entity, alt: &str) -> bool {
    world
        .get::<ImageMetadata>(entity)
        .is_some_and(|image| image.alt.as_deref() == Some(alt))
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| subtree_image(world, child, alt))
        })
}
fn subtree_text(world: &World, entity: Entity, text: &str) -> bool {
    world
        .get::<Text>(entity)
        .is_some_and(|value| value.0 == text)
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| subtree_text(world, child, text))
        })
}
fn descendant_of(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(entity) {
        if parent.parent() == ancestor {
            return true;
        }
        entity = parent.parent();
    }
    false
}
fn item_target(world: &mut World, alt: &str) -> Option<Entity> {
    let browser = element(world, "inventory-browser-grid")?;
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| {
            (id.0 == "inventory-cell"
                && descendant_of(world, entity, browser)
                && subtree_image(world, entity, alt))
            .then_some(entity)
        })
}
fn category_target(world: &mut World, alt: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| {
            (id.0 == "inventory-category" && subtree_image(world, entity, alt)).then_some(entity)
        })
}

fn begin_category_dropdown(
    world: &mut World,
    state: &mut InventorySmoke,
    category: &'static str,
    next: Stage,
) -> Result<(), String> {
    let target =
        element(world, "inventory-category-select").ok_or("Missing native category dropdown")?;
    state.category_id = category;
    state.category_return = next;
    state.category_scrolls = 0;
    pointer(world, target, Some(true))?;
    state.target = Some(target);
    state.advance(Stage::CategoryDropdownRelease);
    Ok(())
}

fn hotbar_target(world: &mut World, index: usize) -> Option<Entity> {
    let parent = element(world, "inventory-hotbar")?;
    let mut slots: Vec<_> = world
        .query::<(Entity, &ElementId, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(entity, id, _)| {
            if id.0 != "inventory-cell" {
                return false;
            }
            let mut current = *entity;
            while let Some(ancestor) = world.get::<ChildOf>(current) {
                if ancestor.parent() == parent {
                    return true;
                }
                current = ancestor.parent();
            }
            false
        })
        .map(|(entity, _, transform)| (entity, transform.translation.x))
        .collect();
    slots.sort_by(|a, b| a.1.total_cmp(&b.1));
    slots.get(index).map(|entry| entry.0)
}

fn crafting_cell(world: &mut World, index: usize) -> Option<Entity> {
    let root = element(world, "inventory-crafting")?;
    let mut slots: Vec<_> = world
        .query::<(Entity, &ElementId, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(entity, id, _)| id.0 == "inventory-cell" && descendant_of(world, *entity, root))
        .map(|(entity, _, t)| (entity, t.translation))
        .collect();
    slots.sort_by(|a, b| {
        a.1.y
            .total_cmp(&b.1.y)
            .then_with(|| a.1.x.total_cmp(&b.1.x))
    });
    slots.get(index).map(|slot| slot.0)
}

fn crafting_pointer_probe(world: &mut World, state: &mut InventorySmoke) -> Result<bool, String> {
    let held = world
        .resource::<ClientSession>()
        .inventory
        .cursor
        .map_or(0, |s| s.count);
    let expected = state.grid_count;
    let cell = |world: &mut World, index| {
        crafting_cell(world, index).ok_or("Missing native crafting cell")
    };
    let hotbar =
        |world: &mut World| hotbar_target(world, 0).ok_or("Missing native crafting source");
    match state.grid_phase {
        0 => {
            state.grid_count = world
                .resource::<ClientSession>()
                .inventory
                .slot(0)
                .ok_or("Missing crafting probe stack")?
                .count;
            let target = hotbar(world)?;
            pointer(world, target, Some(true))?;
        }
        1 if state.elapsed(0.2) => {
            let target = hotbar(world)?;
            pointer(world, target, Some(false))?;
        }
        2 if state.elapsed(0.3) && held > 0 => {
            let session = world.resource::<ClientSession>();
            let dropped: u32 = session
                .drops
                .iter()
                .filter(|drop| drop.stack.item == ItemKind::Block(3))
                .map(|drop| u32::from(drop.stack.count))
                .sum();
            if u32::from(session.inventory.count(ItemKind::Block(3))) + u32::from(held) + dropped
                != 128
            {
                return Ok(false);
            }
            // The outside-drop probe can be picked up again between snapshots.
            // Use the acknowledged amount and park an odd remainder with real input.
            state.grid_count = held;
            if held % 2 == 1 {
                let target = hotbar_target(world, 1).ok_or("Missing crafting remainder slot")?;
                pointer_button(world, target, MouseButton::Right, Some(true))?;
                state.grid_count = held - 1;
                state.grid_phase = 20;
                state.entered = Instant::now();
                return Ok(false);
            }
            let target = cell(world, 0)?;
            pointer(world, target, Some(true))?;
        }
        20 if state.elapsed(0.2) => {
            let target = hotbar_target(world, 1).ok_or("Missing crafting remainder slot")?;
            pointer_button(world, target, MouseButton::Right, Some(false))?;
        }
        21 if state.elapsed(0.3) && held == expected => {
            state.grid_phase = 2;
            state.entered = Instant::now();
            return Ok(false);
        }
        3 if state.elapsed(0.2) => {
            let target = cell(world, 1)?;
            pointer(world, target, None)?;
        }
        4 if state.elapsed(0.2) => {
            if held != expected {
                return Err("Held crafting preview committed before release".into());
            }
            let count = (expected / 2).to_string();
            for index in 0..2 {
                let shown = &world.resource::<terra_ui::CraftingView>().grid[index];
                if shown.empty || shown.count != count {
                    return Err("Held crafting drag did not show its live slot counts".into());
                }
                let target = cell(world, index)?;
                require_slot_presentation(world, target, &count)?;
            }
            capture(world, "crafting-held-drag-preview")?;
            let target = cell(world, 1)?;
            pointer(world, target, Some(false))?;
        }
        5 if state.elapsed(0.3) => {
            let grid = &world.resource::<terra_ui::CraftingView>().grid;
            if held != 0
                || grid[..2]
                    .iter()
                    .any(|s| s.count != (expected / 2).to_string())
            {
                if state.elapsed(5.0) {
                    return Err(
                        "Native crafting drag did not distribute the held stack equally".into(),
                    );
                }
                return Ok(false);
            }
            capture(world, "crafting-mouse-drag")?;
            let target = cell(world, 0)?;
            pointer(world, target, Some(true))?;
        }
        6 if state.elapsed(0.2) => {
            let target = cell(world, 1)?;
            pointer(world, target, None)?;
        }
        7 if state.elapsed(0.2) => {
            let target = cell(world, 1)?;
            pointer(world, target, Some(false))?;
        }
        8 if state.elapsed(0.3) && held == expected => {
            if world
                .resource::<terra_ui::CraftingView>()
                .grid
                .iter()
                .any(|s| !s.empty)
            {
                return Err("Native crafting sweep left duplicated grid items".into());
            }
            let target = hotbar(world)?;
            pointer(world, target, Some(true))?;
        }
        9 if state.elapsed(0.2) => {
            let target = hotbar(world)?;
            pointer(world, target, Some(false))?;
        }
        10 if state.elapsed(0.3) && held == 0 => {
            let session = world.resource::<ClientSession>();
            let inventory = u32::from(session.inventory.count(ItemKind::Block(3)));
            let dropped: u32 = session
                .drops
                .iter()
                .filter(|d| d.stack.item == ItemKind::Block(3))
                .map(|d| u32::from(d.stack.count))
                .sum();
            if inventory + dropped != 128 {
                if state.elapsed(5.0) {
                    return Err(format!(
                        "Native crafting gestures changed the conserved total: inventory={inventory}, drops={dropped}"
                    ));
                }
                return Ok(false);
            }
            info!(
                "Native crafting-grid drag and pickup sweep passed with real OS input and authoritative transfers"
            );
            pointer_at(world, Vec2::splat(4.0), MouseButton::Left, None)?;
            return Ok(true);
        }
        _ => {
            if state.elapsed(5.0) {
                return Err(format!(
                    "Native crafting pointer probe stalled at {} with cursor={held}",
                    state.grid_phase
                ));
            }
            return Ok(false);
        }
    }
    state.grid_phase += 1;
    state.entered = Instant::now();
    Ok(false)
}
fn require_previews(world: &mut World) -> Result<(), String> {
    if !world
        .query::<(&ImageMetadata, &bevy::ui::widget::ImageNode)>()
        .iter(world)
        .any(|(meta, node)| {
            meta.alt.as_deref() == Some("Player preview")
                && node.image != Handle::default()
                && node.color == Color::WHITE
        })
    {
        return Err("Real preview handle missing: Player preview".into());
    }
    let browser = element(world, "inventory-browser-grid").ok_or("Missing item browser")?;
    let previews = world
        .query::<(
            Entity,
            &ImageMetadata,
            &bevy::ui::widget::ImageNode,
            &ComputedNode,
        )>()
        .iter(world)
        .filter(|(entity, _, _, node)| descendant_of(world, *entity, browser) && !node.is_empty())
        .map(|(_, metadata, image, _)| {
            (
                metadata.alt.clone(),
                image.image != Handle::default() && image.color == Color::WHITE,
            )
        })
        .collect::<Vec<_>>();
    let view = world.resource::<InventoryView>();
    if previews.len() != view.browser_items.len()
        || previews.iter().any(|(_, valid)| !valid)
        || view.browser_items.iter().any(|item| {
            !previews
                .iter()
                .any(|(alt, _)| alt.as_deref() == Some(item.preview_key.as_str()))
        })
    {
        return Err("Browser items do not all use their current real rendered previews".into());
    }
    if world.resource::<InventoryView>().category_dropdown {
        let choice = element(world, "inventory-category-select")
            .ok_or("Missing responsive category dropdown")?;
        let popup = world
            .get::<tilt_ui::ChoiceBoxParts>(choice)
            .ok_or("Missing real TiltUI category dropdown")?
            .popup;
        let options = world
            .query::<(Entity, &tilt_ui::OptionData)>()
            .iter(world)
            .filter_map(|(entity, option)| {
                descendant_of(world, entity, popup).then_some(option.value.clone())
            })
            .collect::<Vec<_>>();
        let categories = &world.resource::<InventoryView>().categories;
        if options.len() != categories.len()
            || categories
                .iter()
                .any(|category| !options.contains(&category.id))
        {
            return Err("Responsive dropdown does not contain every JSON-defined category".into());
        }
        return Ok(());
    }
    let categories = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .filter_map(|(entity, id)| (id.0 == "inventory-category").then_some(entity))
        .collect::<Vec<_>>();
    if categories.len() != world.resource::<InventoryView>().categories.len() {
        return Err("Not all JSON-defined category widgets were rendered".into());
    }
    for category in categories {
        if !world
            .query::<(Entity, &ImageMetadata, &bevy::ui::widget::ImageNode)>()
            .iter(world)
            .any(|(entity, _, image)| {
                descendant_of(world, entity, category)
                    && image.image != Handle::default()
                    && image.color == Color::WHITE
            })
        {
            return Err("Category tab does not use a real rendered item preview".into());
        }
    }
    Ok(())
}

fn require_slot_presentation(world: &mut World, target: Entity, count: &str) -> Result<(), String> {
    let has_image = world
        .query::<(Entity, &ImageMetadata, &bevy::ui::widget::ImageNode)>()
        .iter(world)
        .any(|(entity, metadata, image)| {
            descendant_of(world, entity, target)
                && metadata
                    .alt
                    .as_deref()
                    .is_some_and(|alt| alt.starts_with("Block preview "))
                && image.image != Handle::default()
        });
    let has_count = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .any(|(entity, text)| descendant_of(world, entity, target) && text.0 == count);
    if !has_image || !has_count {
        return Err(format!(
            "Live slot preview missing rendered image/count: image={has_image}, count={has_count}"
        ));
    }
    Ok(())
}
pub(crate) fn require_tooltip(
    world: &mut World,
    target: Entity,
    namespace: &str,
) -> Result<(), String> {
    require_popup(world, target, "inventory-tooltip", namespace)
}
fn require_popup(
    world: &mut World,
    target: Entity,
    popup_id: &str,
    namespace: &str,
) -> Result<(), String> {
    let parent = world.get::<ChildOf>(target).map(ChildOf::parent);
    let tip = world
        .query::<(Entity, &ElementId, &Node, Option<&Visibility>)>()
        .iter(world)
        .find_map(|(entity, id, node, visibility)| {
            (id.0 == popup_id
                && node.display != Display::None
                && visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
                && parent.is_some_and(|parent| {
                    descendant_of(world, entity, parent)
                        || world
                            .get::<terra_ui::TooltipOwner>(entity)
                            .is_some_and(|owner| {
                                owner.0 == parent || descendant_of(world, owner.0, parent)
                            })
                })
                && subtree_text(world, entity, namespace))
            .then_some(entity)
        });
    let Some(tip) = tip else {
        let pointer = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .iter(world)
            .next()
            .and_then(Window::physical_cursor_position);
        tracing::error!(?pointer, target=?world.get::<bevy::ui::UiGlobalTransform>(target), size=?world.get::<ComputedNode>(target).map(ComputedNode::size), interaction=?world.get::<bevy::picking::hover::Hovered>(target), "Native tooltip hover target diagnostics");
        return Err("Hover did not open the component-local tooltip".into());
    };
    let mut ancestor = world.get::<ChildOf>(tip).map(ChildOf::parent);
    while let Some(entity) = ancestor {
        if world.get::<Node>(entity).is_some_and(|node| {
            node.overflow.x == bevy::ui::OverflowAxis::Scroll
                || node.overflow.y == bevy::ui::OverflowAxis::Scroll
        }) {
            return Err("Tooltip still contributes to an ancestor's scrolling content".into());
        }
        ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    let layout = world
        .get::<ComputedNode>(tip)
        .ok_or("Tooltip has no layout")?;
    let position = world
        .get::<bevy::ui::UiGlobalTransform>(tip)
        .ok_or("Tooltip is not positioned")?;
    if layout.size().min_element() < 12.0 || !position.translation.is_finite() {
        return Err(format!("Tooltip is not visibly placed: {namespace}"));
    }
    let tip_rect = Rect::from_center_size(position.translation, layout.size());
    let tip_size = layout.size();
    let gap = 10.0 / layout.inverse_scale_factor;
    for (entity, node) in world.query::<(Entity, &Node)>().iter(world) {
        if (entity == tip || descendant_of(world, entity, tip))
            && node.overflow != Overflow::visible()
        {
            return Err("Tooltip acquired a scrollable or clipped content surface".into());
        }
    }
    for (entity, part, node) in world
        .query::<(Entity, &tilt_ui::ControlPart, &Node)>()
        .iter(world)
    {
        if descendant_of(world, entity, tip)
            && matches!(
                part.kind,
                tilt_ui::ControlPartKind::ScrollbarXTrack
                    | tilt_ui::ControlPartKind::ScrollbarYTrack
                    | tilt_ui::ControlPartKind::ScrollbarXThumb
                    | tilt_ui::ControlPartKind::ScrollbarYThumb
            )
            && node.display != Display::None
        {
            return Err("Tooltip rendered a scrollbar".into());
        }
    }
    let viewport = world
        .get::<ComputedUiRenderTargetInfo>(target)
        .ok_or("Missing UI viewport")?
        .physical_size()
        .as_vec2();
    let pointer = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .and_then(Window::physical_cursor_position)
        .ok_or("Missing tooltip pointer")?;
    let mut expected = pointer + Vec2::splat(gap);
    for axis in 0..2 {
        if expected[axis] + tip_size[axis] > viewport[axis] - gap {
            expected[axis] = pointer[axis] - gap - tip_size[axis];
        }
    }
    expected = expected.clamp(Vec2::ZERO, (viewport - tip_size).max(Vec2::ZERO));
    if tip_rect.min.distance(expected) > 2.0 {
        return Err(format!(
            "Tooltip does not follow pointer at ten UI pixels: {tip_rect:?}, pointer={pointer:?}, expected={expected:?}"
        ));
    }
    if tip_rect.min.cmplt(Vec2::splat(-1.0)).any() || tip_rect.max.cmpgt(viewport + Vec2::ONE).any()
    {
        return Err(format!(
            "Tooltip escapes viewport: {namespace} tip={tip_rect:?} viewport={viewport:?}"
        ));
    }
    Ok(())
}
fn require_layout(world: &mut World) -> Result<(), String> {
    let size = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| {
            Vec2::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
            )
        })
        .ok_or("Missing window")?;
    for (id, node, transform) in world
        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
        .iter(world)
        .filter(|(id, _, _)| id.0 == "inventory-cell")
    {
        let half = node.size() * 0.5;
        let min = transform.translation - half;
        let max = transform.translation + half;
        if half.min_element() < 12.0 || (half.x - half.y).abs() > 1.5 {
            return Err(format!(
                "Slot layout invalid: {} {min:?}..{max:?} within {size:?}",
                id.0
            ));
        }
    }
    let panel = element(world, "inventory-panel").ok_or("Missing inventory panel")?;
    let panel_rect = Rect::from_center_size(
        world
            .get::<bevy::ui::UiGlobalTransform>(panel)
            .ok_or("Missing panel transform")?
            .translation,
        world
            .get::<ComputedNode>(panel)
            .ok_or("Missing panel bounds")?
            .size(),
    );
    if panel_rect.min.cmplt(-Vec2::ONE).any() || panel_rect.max.cmpgt(size + Vec2::ONE).any() {
        return Err(format!(
            "Inventory frame escapes viewport: {panel_rect:?} {size:?}"
        ));
    }
    if (panel_rect.center().x - size.x * 0.5).abs() > 2.0 {
        return Err(format!(
            "Main inventory panel must stay centered in the viewport: {panel_rect:?}, viewport={size:?}"
        ));
    }
    let browser = element(world, "item-browser").ok_or("Missing item browser frame")?;
    let browser_rect = Rect::from_center_size(
        world
            .get::<bevy::ui::UiGlobalTransform>(browser)
            .ok_or("Missing browser transform")?
            .translation,
        world
            .get::<ComputedNode>(browser)
            .ok_or("Missing browser bounds")?
            .size(),
    );
    let (window_scale, media_width) = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map_or((1.0, size.x), |window| {
            (window.scale_factor(), window.width())
        });
    let scale = window_scale * world.resource::<bevy::ui::UiScale>().0;
    let grid = element(world, "inventory-browser-grid").ok_or("Missing browser item grid")?;
    let mut item_min = f32::INFINITY;
    let mut item_max = f32::NEG_INFINITY;
    for (entity, id, node, transform) in world
        .query::<(
            Entity,
            &ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
    {
        if id.0 == "inventory-cell" && descendant_of(world, entity, grid) && !node.is_empty() {
            item_min = item_min.min(transform.translation.x - node.size().x * 0.5);
            item_max = item_max.max(transform.translation.x + node.size().x * 0.5);
        }
    }
    if item_min.is_finite()
        && ((item_min + item_max) * 0.5 - browser_rect.center().x).abs() > 6.0 * scale
    {
        return Err(format!(
            "Item contents must be centered inside their allocated sidebar: items={item_min}..{item_max}, sidebar={browser_rect:?}"
        ));
    }
    if !world.resource::<InventoryView>().category_dropdown {
        let categories = element(world, "inventory-categories").ok_or("Missing category grid")?;
        let mut bounds = (f32::INFINITY, f32::NEG_INFINITY);
        for (entity, id, node, transform) in world
            .query::<(
                Entity,
                &ElementId,
                &ComputedNode,
                &bevy::ui::UiGlobalTransform,
            )>()
            .iter(world)
        {
            if id.0 == "inventory-category"
                && descendant_of(world, entity, categories)
                && !node.is_empty()
            {
                bounds.0 = bounds.0.min(transform.translation.x - node.size().x * 0.5);
                bounds.1 = bounds.1.max(transform.translation.x + node.size().x * 0.5);
            }
        }
        let rail_node = world
            .get::<ComputedNode>(categories)
            .ok_or("Missing category rail bounds")?;
        let rail_transform = world
            .get::<bevy::ui::UiGlobalTransform>(categories)
            .ok_or("Missing category rail transform")?;
        let rail_rect = Rect::from_center_size(rail_transform.translation, rail_node.size());
        let scrollable = world
            .get::<Node>(categories)
            .is_some_and(|node| node.overflow.y != bevy::ui::OverflowAxis::Visible);
        if !bounds.0.is_finite()
            || bounds.0 < rail_rect.min.x - 2.0
            || bounds.1 > rail_rect.max.x + 2.0
            || rail_rect.max.x > browser_rect.min.x + 1.0
            || (rail_rect.height() - browser_rect.height()).abs() > 3.0 * scale
            || !scrollable
        {
            return Err(format!(
                "Categories must form a vertical scroll rail outside the left browser border: tabs={bounds:?}, rail={rail_rect:?}, sidebar={browser_rect:?}, scrollable={scrollable}"
            ));
        }
    }
    if element(world, "inventory-cancel-move").is_some() {
        return Err("Removed inventory cancel footer is still rendered".into());
    }
    let creative = world.resource::<InventoryView>().creative;
    match element(world, "inventory-trash") {
        Some(trash) if creative => {
            let bounds = Rect::from_center_size(
                world
                    .get::<bevy::ui::UiGlobalTransform>(trash)
                    .ok_or("Missing trash transform")?
                    .translation,
                world
                    .get::<ComputedNode>(trash)
                    .ok_or("Missing trash bounds")?
                    .size(),
            );
            if bounds.size().min_element() < 20.0 * scale
                || bounds.min.cmplt(panel_rect.min - Vec2::ONE).any()
                || bounds.max.cmpgt(panel_rect.max + Vec2::ONE).any()
            {
                return Err(format!(
                    "Creative trash control must stay visible inside the inventory: {bounds:?}"
                ));
            }
        }
        None if !creative => {}
        _ => return Err("Inventory trash must render only in Creative mode".into()),
    }
    if media_width > 620.0 {
        if browser_rect.min.cmplt(-Vec2::ONE).any()
            || browser_rect.max.cmpgt(size + Vec2::ONE).any()
        {
            return Err(format!(
                "Item browser escapes viewport: {browser_rect:?} {size:?}"
            ));
        }
        if (panel_rect.center().y - size.y * 0.5).abs() > 2.0 {
            return Err(format!(
                "Main inventory panel must stay vertically centered: {panel_rect:?}, viewport={size:?}"
            ));
        }
        let right_inset = (size.x - browser_rect.max.x) / scale;
        let height = browser_rect.height() / size.y;
        if !(0.0..=17.0).contains(&right_inset) || (height - 0.95).abs() > 0.015 {
            return Err(format!(
                "Item browser must hug the right viewport edge at 95% height: inset={right_inset}, height={height}"
            ));
        }
        if media_width > 780.0
            && ((browser_rect.min.x - panel_rect.max.x) / scale - 100.0).abs() > 2.0
        {
            return Err(
                "Desktop item browser must keep 100 UI pixels between itself and the inventory"
                    .into(),
            );
        }
        let gap = if media_width > 780.0 { 100.0 } else { 68.0 };
        let expected_inset = if media_width > 1150.0 {
            16.0
        } else if media_width > 950.0 {
            12.0
        } else if media_width > 780.0 {
            10.0
        } else {
            8.0
        };
        let expected_width = (size.x - panel_rect.max.x) / scale - gap - expected_inset;
        if (browser_rect.width() / scale - expected_width).abs() > 2.0 {
            return Err(format!(
                "Item browser must fill the remaining right-side width: actual={}, expected={expected_width}",
                browser_rect.width() / scale
            ));
        }
    } else if browser_rect.min.x < -1.0
        || browser_rect.max.x > size.x + 1.0
        || browser_rect.min.y < panel_rect.max.y - 1.0
    {
        return Err(format!(
            "Compact item browser must stack below the centered inventory without horizontal clipping: {panel_rect:?} {browser_rect:?}"
        ));
    }
    let center_x = |world: &mut World, id: &str| -> Result<f32, String> {
        let entity =
            element(world, id).ok_or_else(|| format!("Missing pagination element {id}"))?;
        world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .map(|transform| transform.translation.x)
            .ok_or_else(|| format!("Missing pagination layout {id}"))
    };
    let previous = center_x(world, "inventory-page-previous")?;
    let next = center_x(world, "inventory-page-next")?;
    let page = center_x(world, "inventory-browser-page")?;
    if (page - (previous + next) * 0.5).abs() > 2.0 {
        return Err(format!(
            "Item page label is not centered between its arrows: {previous}, {page}, {next}"
        ));
    }
    for id in ["inventory-page-previous", "inventory-page-next"] {
        let entity = element(world, id).ok_or_else(|| format!("Missing pagination arrow {id}"))?;
        let center = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Missing arrow transform")?
            .translation;
        let arrow_size = world
            .get::<ComputedNode>(entity)
            .ok_or("Missing arrow bounds")?
            .size();
        let bounds = Rect::from_center_size(center, arrow_size);
        if arrow_size.min_element() < 10.0
            || bounds.min.cmplt(browser_rect.min - Vec2::ONE).any()
            || bounds.max.cmpgt(browser_rect.max + Vec2::ONE).any()
        {
            return Err(format!(
                "Pagination arrow is missing or clipped by the sidebar: {id} {bounds:?}, sidebar={browser_rect:?}"
            ));
        }
    }
    if !(previous < page && page < next) {
        return Err(format!(
            "Pagination order is invalid: {previous}, {page}, {next}"
        ));
    }
    if panel_rect.max.x > browser_rect.min.x + 1.0 && panel_rect.max.y > browser_rect.min.y + 1.0 {
        return Err(format!(
            "Shared inventory overlaps the item browser: {panel_rect:?} {browser_rect:?}"
        ));
    }
    for (id, expected) in [
        ("inventory-grid", terra_core::INVENTORY_SLOTS - 9),
        ("inventory-hotbar", 9),
    ] {
        let section = element(world, id).ok_or_else(|| format!("Missing {id}"))?;
        let slots = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .filter(|(entity, id)| {
                id.0 == "inventory-cell" && descendant_of(world, *entity, section)
            })
            .count();
        if slots != expected {
            return Err(format!(
                "Shared player inventory has {slots} slots in {id}, expected {expected}"
            ));
        }
    }
    for (entity, _) in world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .filter(|(_, id)| id.0 == "inventory-category")
    {
        if !descendant_of(world, entity, browser) {
            return Err("Item category is outside the shared browser".into());
        }
    }
    let view = world.resource::<InventoryView>();
    if !view.assembly && !view.container && !view.furnace && !view.crusher && !view.mixer {
        let equipment_count = world
            .query::<(&ElementId, &ComputedNode)>()
            .iter(world)
            .filter(|(id, _)| id.0 == "equipment-slot")
            .try_fold(0, |count, (_, node)| {
                if node.size().min_element() < 20.0 || (node.size().x - node.size().y).abs() > 1.5 {
                    return Err(format!(
                        "Equipment slot must be a stable square: {:?}",
                        node.size()
                    ));
                }
                Ok(count + 1)
            })?;
        let expected_equipment = terra_core::EquipmentSlot::ALL.len();
        if equipment_count != expected_equipment {
            return Err(format!(
                "Player inventory must show {expected_equipment} equipment positions: got {equipment_count}"
            ));
        }
        let symbols = world
            .query::<(&tilt_ui::ElementClasses, &ComputedNode)>()
            .iter(world)
            .filter(|(classes, _)| {
                classes
                    .classes
                    .iter()
                    .any(|class| class == "equipment-symbol")
            })
            .map(|(classes, node)| (classes.classes.len(), node.size()))
            .collect::<Vec<_>>();
        if symbols.len() != expected_equipment
            || symbols
                .iter()
                .any(|(classes, size)| *classes < 2 || size.min_element() < 12.0)
        {
            return Err(
                "Equipment silhouettes require a resolved piece type and visible area".into(),
            );
        }
    }
    Ok(())
}
pub(crate) fn pointer(
    world: &mut World,
    target: Entity,
    pressed: Option<bool>,
) -> Result<(), String> {
    pointer_button(world, target, MouseButton::Left, pressed)
}
fn pointer_button(
    world: &mut World,
    target: Entity,
    button: MouseButton,
    pressed: Option<bool>,
) -> Result<(), String> {
    let physical = world
        .get::<bevy::ui::UiGlobalTransform>(target)
        .ok_or("No slot layout")?
        .translation;
    pointer_at(world, physical, button, pressed)
}
fn pointer_at(
    world: &mut World,
    physical: Vec2,
    button: MouseButton,
    pressed: Option<bool>,
) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No native window")?;
    if !std::env::var("TERRA_NATIVE_OS_POINTER").is_ok_and(|value| value == "1") {
        return Err("Creative pointer acceptance requires TERRA_NATIVE_OS_POINTER=1 and the native uinput helper".into());
    }
    let viewport = world.get::<Window>(window).ok_or("No native window")?;
    let mut commands = vec![
        tilt_ui::serde_json::json!({"move":[physical.x,physical.y],"size":[viewport.physical_width(),viewport.physical_height()],"wait":0.1}),
    ];
    if let Some(pressed) = pressed {
        commands.push(if button == MouseButton::Right {
            tilt_ui::serde_json::json!({"key":[273,u8::from(pressed)],"wait":0.1})
        } else {
            tilt_ui::serde_json::json!({"button":u8::from(pressed),"wait":0.1})
        });
    }
    os_pointer(world, &commands)
}
pub(crate) fn type_text(world: &mut World, value: &str) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No native window")?;
    for c in value.chars() {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            world.write_message(KeyboardInput {
                key_code: KeyCode::KeyA,
                logical_key: Key::Character(c.to_string().into()),
                state,
                text: (state == ButtonState::Pressed).then(|| c.to_string().into()),
                repeat: false,
                window,
            });
        }
    }
    Ok(())
}
fn resize(world: &mut World, size: (u32, u32)) {
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        window.set_cursor_position(None);
        window.resolution.set_physical_resolution(size.0, size.1);
    }
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-inventory"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<InventorySmoke>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(101)
                    .take(4096)
                    .map(|rgba| [rgba[0], rgba[1], rgba[2]])
                    .collect();
                state.capture_error |= colors.len() < 16;
                state.captures += 1;
            },
        );
    Ok(())
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    const TOOL: &str = "Item preview teramic::stone_pickaxe";

    fn fixture() -> (World, Entity, Entity, Handle<Image>) {
        let mut world = World::new();
        world.init_resource::<InventoryView>();
        world.init_resource::<Assets<Image>>();
        world
            .resource_mut::<InventoryView>()
            .browser_items
            .push(default());
        world.resource_mut::<InventoryView>().browser_items[0].preview_key = TOOL.into();
        let image = world.resource_mut::<Assets<Image>>().add(Image::default());
        let browser = world.spawn(ElementId("inventory-browser-grid".into())).id();
        spawn_image(&mut world, None, "Player preview", image.clone(), true);
        let tool = spawn_image(&mut world, Some(browser), TOOL, image.clone(), true);
        (world, browser, tool, image)
    }

    fn spawn_image(
        world: &mut World,
        parent: Option<Entity>,
        alt: &str,
        image: Handle<Image>,
        visible: bool,
    ) -> Entity {
        let entity = world
            .spawn((
                ImageMetadata {
                    alt: Some(alt.into()),
                    ..default()
                },
                bevy::ui::widget::ImageNode { image, ..default() },
                ComputedNode {
                    size: if visible {
                        Vec2::splat(48.0)
                    } else {
                        Vec2::ZERO
                    },
                    ..default()
                },
            ))
            .id();
        if let Some(parent) = parent {
            world.entity_mut(entity).insert(ChildOf(parent));
        }
        entity
    }

    #[test]
    fn tools_catalog_checks_its_active_previews_without_requiring_stone_or_unrelated_images() {
        let (mut world, browser, _, _) = fixture();
        spawn_image(&mut world, None, "Block preview 3", Handle::default(), true);
        spawn_image(
            &mut world,
            Some(browser),
            "Block preview 3",
            Handle::default(),
            false,
        );
        assert!(require_previews(&mut world).is_ok());
    }

    #[test]
    fn active_catalog_images_must_have_the_expected_metadata_and_a_real_handle() {
        let (mut world, browser, tool, image) = fixture();
        spawn_image(&mut world, None, TOOL, image.clone(), true);
        world
            .get_mut::<bevy::ui::widget::ImageNode>(tool)
            .unwrap()
            .image = Handle::default();
        assert!(require_previews(&mut world).is_err());
        world
            .get_mut::<bevy::ui::widget::ImageNode>(tool)
            .unwrap()
            .image = image.clone();
        world.get_mut::<ImageMetadata>(tool).unwrap().alt = Some("Block preview 3".into());
        assert!(require_previews(&mut world).is_err());
        world.resource_mut::<InventoryView>().browser_items[0].preview_key =
            "Block preview 3".into();
        assert!(require_previews(&mut world).is_ok());
        spawn_image(&mut world, Some(browser), TOOL, Handle::default(), true);
        assert!(require_previews(&mut world).is_err());
    }
}
