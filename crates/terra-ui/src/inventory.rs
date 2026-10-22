use crate::menu::{MenuPage, MenuState};
use bevy::{
    ecs::system::SystemParam, input::mouse::MouseWheel, input_focus::InputFocus, prelude::*,
};
use std::collections::HashMap;
use terra_core::{GameAction, GameMode, GameplayInput, ItemKind, ResourcePropKind, STACK_LIMIT};
use terra_network::{
    client::{ClientSession, ContainerRequest, InventoryRequest, SelectedInventorySlot},
    protocol::inventory::InventoryAction as ServerAction,
};
use terra_player::{
    CaptureRequest, PlayerInputBlock, PlayerSet,
    interaction::{ActiveItem, BlockPalette},
};
use tilt_ui::ElementId;

#[path = "inventory_view.rs"]
mod view;
pub use view::InventoryView;
#[path = "crafting.rs"]
pub(crate) mod crafting;
#[path = "inventory_pointer.rs"]
mod pointer;
#[path = "inventory_tooltip.rs"]
mod tooltip;
pub use tooltip::TooltipOwner;

#[cfg(test)]
#[path = "inventory_preview_tests.rs"]
mod preview_tests;
#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;

#[derive(Resource, Default)]
pub struct BlockPreviewImages(pub HashMap<u16, Handle<Image>>);
#[derive(Resource, Default)]
pub struct PropPreviewImages(pub HashMap<ResourcePropKind, Handle<Image>>);
#[derive(Resource, Default)]
pub struct ItemPreviewImages(pub HashMap<ItemKind, Handle<Image>>);
#[derive(Resource, Default)]
pub struct PlayerPreviewImage(pub Handle<Image>);

const PREVIEW_ALT_PREFIX: &str = "Block preview ";
const PROP_ALT_PREFIX: &str = "Prop preview ";

#[derive(Resource, Default, PartialEq, Eq)]
pub(crate) struct InventoryState {
    container_active: bool,
    container_open_response: Option<u64>,
    pub(crate) assembly: bool,
    pub(crate) station: Option<[i64; 3]>,
    pub open: bool,
    resume_pending: bool,
    focused: Option<ItemKind>,
    source: Option<u8>,
    hotbar: u8,
    search: String,
    category: String,
    page: usize,
    page_capacity: usize,
    category_dropdown: bool,
    inventory_revision: Option<u64>,
    active_slot: Option<u8>,
}
impl InventoryState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.open || self.resume_pending
    }
}

#[derive(Message, Clone, Debug)]
pub enum InventoryAction {
    OpenChest([i64; 3]),
    OpenWorkbench([i64; 3]),
    Assembly(bool),
    Toggle,
    Close,
    Select(String),
    Recipe(String),
    Focus(String),
    Cycle(i32),
    Search(String),
    Category(String),
    Page(i32),
    Slot(u8),
    Hotbar(u8),
    CraftCell(u8),
    Craft,
    CraftHeld,
    CancelMove,
    Trash,
    Pointer {
        command: String,
        index: Option<u8>,
        item: String,
        secondary: bool,
    },
    Hover {
        command: String,
        index: Option<u8>,
        item: String,
    },
    Leave {
        command: String,
        index: Option<u8>,
        item: String,
    },
    Outside(bool),
    Drop(bool),
    PickBlock(u16),
}

#[derive(SystemParam)]
struct InventoryContext<'w> {
    menu: Option<Res<'w, MenuState>>,
    session: Option<Res<'w, ClientSession>>,
    settings: Res<'w, crate::SettingsView>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    target: Option<Res<'w, terra_player::TargetWorld>>,
    chunks: Option<Res<'w, terra_world::streaming::ChunkStore>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    camera: Option<Res<'w, terra_player::CameraInputState>>,
    fullmap: Option<Res<'w, crate::minimap::fullmap::FullMapState>>,
}
impl InventoryContext<'_> {
    fn available(&self) -> bool {
        !self.settings.open
            && !self.fullmap.as_ref().is_some_and(|map| map.blocks_input())
            && !self.chat.as_ref().is_some_and(|chat| chat.blocks_input())
            && self
                .menu
                .as_ref()
                .is_none_or(|menu| menu.page == MenuPage::Playing)
            && self
                .session
                .as_ref()
                .is_some_and(|session| session.ready && !session.teleporting && !session.dead())
    }
}

pub(crate) fn install(app: &mut App) {
    crafting::install(app);
    app.init_resource::<InventoryState>()
        .init_resource::<InventoryView>()
        .init_resource::<BlockPreviewImages>()
        .init_resource::<PropPreviewImages>()
        .init_resource::<ItemPreviewImages>()
        .init_resource::<PlayerPreviewImage>()
        .init_resource::<ActiveItem>()
        .init_resource::<SelectedInventorySlot>()
        .init_resource::<pointer::InventoryPointer>()
        .init_resource::<pointer::InventoryQueue>()
        .init_resource::<pointer::InventoryCursor>()
        .add_message::<InventoryAction>()
        .add_message::<InventoryRequest>()
        .add_message::<terra_network::client::ConsumptionRequest>()
        .add_message::<ContainerRequest>()
        .add_message::<terra_network::client::ObjectInteractionRequest>()
        .add_systems(
            PreUpdate,
            block_opening_frame.after(bevy::input::InputSystems),
        )
        .add_systems(
            PreUpdate,
            pointer::suppress_transfer_scroll.after(bevy::picking::PickingSystems::Last),
        )
        .add_systems(
            Update,
            (
                keyboard,
                actions,
                held_consumption,
                pointer::finish_gesture,
                pointer::send_requests,
                view::update_view.run_if(crate::cadence::ready),
                pointer::cursor_view,
            )
                .chain()
                .after(crate::menu::MenuInputSet)
                .before(crate::survival::SurvivalUiSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            (
                sync_preview_images,
                unclip_slot_tooltips,
                tooltip::fit_contents,
            )
                .before(bevy::ui::UiSystems::Prepare),
        )
        .add_systems(
            PostUpdate,
            (
                tooltip::position,
                tooltip::fit_identifier,
                browser_layout,
                reveal_layout,
            )
                .chain()
                .after(bevy::ui::UiSystems::PostLayout)
                .before(bevy::ui::UiSystems::Stack),
        );
}

fn browser_layout(
    nodes: Query<(Entity, &ElementId, &ComputedNode, &Node)>,
    parents: Query<&ChildOf>,
    palette: Res<BlockPalette>,
    mut state: ResMut<InventoryState>,
) {
    if !state.open {
        return;
    }
    if let Some((_, _, browser, _)) = nodes
        .iter()
        .find(|(_, id, c, _)| id.0 == "inventory-categories-host" && !c.is_empty())
    {
        let count = 1 + palette
            .creative_categories
            .iter()
            .filter(|category| category.id != "all")
            .count();
        let available = browser.size().x * browser.inverse_scale_factor;
        let dropdown = available + 0.5 < count as f32 * 86.0 - 6.0;
        if state.category_dropdown != dropdown {
            state.category_dropdown = dropdown;
        }
    }
    let Some((grid, _, computed, node)) = nodes
        .iter()
        .find(|(_, id, c, _)| id.0 == "inventory-browser-grid" && !c.is_empty())
    else {
        return;
    };
    let cell = nodes.iter().find(|(entity, id, c, _)| {
        if id.0 != "inventory-cell" || c.size().min_element() < 10.0 {
            return false;
        }
        let mut entity = *entity;
        for _ in 0..16 {
            let Ok(parent) = parents.get(entity) else {
                break;
            };
            entity = parent.parent();
            if entity == grid {
                return true;
            }
        }
        false
    });
    let Some((_, _, cell, _)) = cell else {
        return;
    };
    let gap = |value| match value {
        Val::Px(px) => px / computed.inverse_scale_factor(),
        _ => 0.0,
    };
    let inset = computed.content_inset();
    let space = computed.size() - inset.min_inset - inset.max_inset;
    let capacity = browser_capacity(
        space,
        cell.size(),
        Vec2::new(gap(node.column_gap), gap(node.row_gap)),
    );
    if state.page_capacity != capacity {
        state.page_capacity = capacity;
        state.page = 0;
    }
}

fn browser_capacity(space: Vec2, cell: Vec2, gap: Vec2) -> usize {
    let count = ((space + gap + Vec2::splat(0.5)) / (cell + gap).max(Vec2::ONE))
        .floor()
        .max(Vec2::ONE);
    (count.x as usize * count.y as usize).clamp(1, 1024)
}

fn reveal_layout(
    nodes: Query<(Entity, &tilt_ui::ElementId, &ComputedNode)>,
    parents: Query<&ChildOf>,
    mut view: ResMut<InventoryView>,
    state: Option<Res<InventoryState>>,
    mut settled: Local<Option<(Entity, Vec2, u8)>>,
) {
    if !view.open {
        *settled = None;
        return;
    }
    // A chest/workbench first needs its authoritative snapshot. Do not flash
    // the ordinary player-inventory layout during that network round trip.
    if state
        .as_ref()
        .is_some_and(|s| s.container_open_response.is_some())
    {
        *settled = None;
        return;
    }
    if view.layout_style == "opacity: 1; pointer-events: auto;" {
        return;
    }
    let Some((entity, _, panel)) = nodes
        .iter()
        .find(|(_, id, node)| id.0 == "inventory-panel" && !node.is_empty())
    else {
        return;
    };
    let size = panel.size();
    let required = view.items.len() + view.hotbar.len();
    let materialized = nodes
        .iter()
        .filter(|(slot, id, node)| {
            if id.0 != "inventory-cell" || node.size().min_element() < 10.0 {
                return false;
            }
            let mut current = *slot;
            for _ in 0..32 {
                if current == entity {
                    return true;
                }
                let Ok(parent) = parents.get(current) else {
                    break;
                };
                current = parent.parent();
            }
            false
        })
        .take(required)
        .count();
    if materialized < required {
        *settled = None;
        return;
    }
    let frames = settled
        .as_ref()
        .filter(|(old, extent, _)| *old == entity && extent.abs_diff_eq(size, 0.1))
        .map_or(0, |(_, _, frames)| frames.saturating_add(1));
    *settled = Some((entity, size, frames));
    if frames >= 2 && view.layout_style != "opacity: 1; pointer-events: auto;" {
        view.layout_style = "opacity: 1; pointer-events: auto;".into();
    }
}

pub(crate) fn preview_key(item: ItemKind) -> String {
    match item {
        ItemKind::Block(id) => format!("{PREVIEW_ALT_PREFIX}{id}"),
        ItemKind::Prop(kind) => format!("{PROP_ALT_PREFIX}{}", prop_slug(kind)),
        ItemKind::Tool(_)
        | ItemKind::Stick
        | ItemKind::Mineral(_)
        | ItemKind::Animal(_)
        | ItemKind::Bellows(_)
        | ItemKind::Dye(_)
        | ItemKind::DyedBed { .. }
        | ItemKind::WoodenBlock { .. } => {
            format!("Item preview {}", item.identifier())
        }
    }
}

fn unclip_slot_tooltips(
    mut commands: Commands,
    added: Query<(Entity, &ElementId), Added<ElementId>>,
    tooltips: Query<(Entity, &tilt_ui::TiltElement), Added<tilt_ui::TiltElement>>,
) {
    for (entity, id) in &added {
        if matches!(
            id.0.as_str(),
            "inventory-tooltip" | "inventory-category-tooltip" | "inventory-source-tooltip"
        ) {
            // TiltUI owns the popup; this renderer hint lets it escape the catalog's scroll clip.
            commands.entity(entity).insert(bevy::ui::OverrideClip);
        }
    }
    for (entity, element) in &tooltips {
        if element.kind == tilt_ui::ElementKind::ToolTip {
            commands.entity(entity).insert(bevy::ui::OverrideClip);
        }
    }
}

#[derive(Default)]
struct PreviewSourceIndex {
    images: HashMap<String, Handle<Image>>,
    initialized: bool,
    props_present: bool,
    items_present: bool,
    portrait: Handle<Image>,
}

type PreviewImageData = (
    Entity,
    &'static tilt_ui::ImageMetadata,
    &'static mut bevy::ui::widget::ImageNode,
    Option<&'static Visibility>,
);
type AllPreviewImages<'w, 's> = Query<'w, 's, PreviewImageData>;
type ChangedPreviewImages<'w, 's> = Query<
    'w,
    's,
    PreviewImageData,
    Or<(
        Changed<tilt_ui::ImageMetadata>,
        Changed<bevy::ui::widget::ImageNode>,
        Added<bevy::ui::widget::ImageNode>,
    )>,
>;

fn sync_preview_images(
    mut commands: Commands,
    previews: Res<BlockPreviewImages>,
    props: Option<Res<PropPreviewImages>>,
    items: Option<Res<ItemPreviewImages>>,
    player: Option<Res<PlayerPreviewImage>>,
    mut source: Local<PreviewSourceIndex>,
    mut images: ParamSet<(AllPreviewImages, ChangedPreviewImages)>,
) {
    let sources_changed = !source.initialized
        || previews.is_changed()
        || props.as_ref().is_some_and(|props| props.is_changed())
        || items.as_ref().is_some_and(|items| items.is_changed())
        || source.props_present != props.is_some()
        || source.items_present != items.is_some();
    if sources_changed {
        source.images.clear();
        for (id, handle) in &previews.0 {
            source
                .images
                .insert(format!("{PREVIEW_ALT_PREFIX}{id}"), handle.clone());
        }
        if let Some(props) = &props {
            for (kind, handle) in &props.0 {
                source.images.insert(
                    format!("{PROP_ALT_PREFIX}{}", prop_slug(*kind)),
                    handle.clone(),
                );
            }
        }
        if let Some(items) = &items {
            for (item, handle) in &items.0 {
                source.images.insert(
                    format!("Item preview {}", item.identifier()),
                    handle.clone(),
                );
            }
        }
        source.initialized = true;
        source.props_present = props.is_some();
        source.items_present = items.is_some();
    }
    let portrait = player
        .as_ref()
        .map(|player| &player.0)
        .cloned()
        .unwrap_or_default();
    let portrait_changed = source.portrait != portrait;
    source.portrait = portrait;
    if sources_changed {
        for (entity, metadata, image, visibility) in &mut images.p0() {
            bind_preview_image(&mut commands, &source, entity, metadata, image, visibility);
        }
        return;
    }
    if portrait_changed {
        for (entity, metadata, image, visibility) in &mut images.p0() {
            if metadata.alt.as_deref() == Some("Player preview") {
                bind_preview_image(&mut commands, &source, entity, metadata, image, visibility);
            }
        }
    }
    // Runtime-owned resets and newly materialized/rebound nodes still need repair.
    for (entity, metadata, image, visibility) in &mut images.p1() {
        if portrait_changed && metadata.alt.as_deref() == Some("Player preview") {
            continue;
        }
        bind_preview_image(&mut commands, &source, entity, metadata, image, visibility);
    }
}

fn bind_preview_image(
    commands: &mut Commands,
    source: &PreviewSourceIndex,
    entity: Entity,
    metadata: &tilt_ui::ImageMetadata,
    mut image: Mut<bevy::ui::widget::ImageNode>,
    visibility: Option<&Visibility>,
) {
    let Some(alt) = metadata.alt.as_deref() else {
        return;
    };
    let preview = if alt == "Player preview" {
        (source.portrait != Handle::default()).then_some(&source.portrait)
    } else if alt.starts_with(PREVIEW_ALT_PREFIX)
        || alt.starts_with(PROP_ALT_PREFIX)
        || alt.starts_with("Item preview ")
    {
        source.images.get(alt)
    } else {
        return;
    };
    let handle = preview.cloned().unwrap_or_default();
    if image.image != handle {
        image.image = handle;
    }
    // TiltUI owns inherited opacity, including the inventory's opening gate.
    let desired = if preview.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if visibility != Some(&desired) {
        commands.entity(entity).insert(desired);
    }
}

fn block_opening_frame(
    input: GameplayInput,
    context: InventoryContext,
    mut blocked: ResMut<PlayerInputBlock>,
) {
    if context.available()
        && (input.just_pressed(GameAction::Inventory)
            || station_target(&input, &context)
                .and_then(|p| context.chunks.as_ref()?.block_at(p))
                .is_some_and(|material| matches!(material, 28 | 29 | 59)))
    {
        blocked.0 = true;
    }
}

fn station_target(input: &GameplayInput, context: &InventoryContext) -> Option<[i64; 3]> {
    if !(input.just_pressed(GameAction::Use) || input.just_pressed(GameAction::Place))
        || input.keys.pressed(KeyCode::ShiftLeft)
        || input.keys.pressed(KeyCode::ShiftRight)
    {
        return None;
    }
    let terra_player::WorldTarget::Block(hit) = context.target.as_ref()?.0? else {
        return None;
    };
    matches!(
        context.chunks.as_ref()?.block_at(hit.position),
        Some(28 | 29 | 59 | 65..=73)
    )
    .then_some(hit.position)
}

#[derive(Default)]
struct HeldConsumption {
    slot: Option<u8>,
    next_pulse: f64,
    opening_until_release: bool,
}

fn held_consumption(
    input: GameplayInput,
    state: Res<InventoryState>,
    context: InventoryContext,
    creature: Option<Res<terra_player::TargetCreature>>,
    time: Res<Time>,
    mut held: Local<HeldConsumption>,
    mut requests: MessageWriter<terra_network::client::ConsumptionRequest>,
) {
    if !input.pressed(GameAction::Use) && !input.pressed(GameAction::Place) {
        held.opening_until_release = false;
    } else if context.available()
        && !state.blocks_input()
        && station_target(&input, &context)
            .and_then(|position| context.chunks.as_ref()?.block_at(position))
            .is_some_and(|material| (65..=73).contains(&material))
    {
        // Doors stay in gameplay; holding the interaction must not start a bite.
        held.opening_until_release = true;
    }
    let slot = (context.available()
        && !state.blocks_input()
        && context.camera.as_ref().is_none_or(|camera| camera.captured)
        && !held.opening_until_release
        && input.consume_pressed()
        && station_target(&input, &context).is_none()
        && (input.pressed(GameAction::Use)
            || creature.as_ref().is_none_or(|target| target.0.is_none()))
        && !input.pressed(GameAction::Break))
    .then_some(state.active_slot)
    .flatten()
    .filter(|slot| {
        context.session.as_ref().is_some_and(|session| {
            session.game_mode != GameMode::Spectator
                && session
                    .inventory
                    .slot(usize::from(*slot))
                    .is_some_and(|stack| terra_core::food(stack.item).is_some())
        })
    });
    let now = time.elapsed_secs_f64();
    if slot != held.slot || slot.is_some() && now >= held.next_pulse {
        requests.write(terra_network::client::ConsumptionRequest(slot));
        held.slot = slot;
        held.next_pulse = now + 0.1;
    }
}

#[allow(clippy::too_many_arguments)]
fn keyboard(
    input: GameplayInput,
    state: Res<InventoryState>,
    context: InventoryContext,
    focus: Option<Res<InputFocus>>,
    elements: Query<&ElementId>,
    mut wheel: MessageReader<MouseWheel>,
    mut actions: MessageWriter<InventoryAction>,
    crafting: Res<crafting::CraftingView>,
    mut crafting_actions: MessageWriter<crafting::CraftingAction>,
    pointer: Res<pointer::InventoryPointer>,
    mut objects: MessageWriter<terra_network::client::ObjectInteractionRequest>,
) {
    if !context.available() {
        wheel.clear();
        return;
    }
    if state.open {
        wheel.clear();
    } else {
        let delta: f32 = wheel.read().map(|event| event.y).sum();
        if delta.is_finite() && delta.abs() > f32::EPSILON {
            let slot = (i32::from(state.hotbar) - delta.signum() as i32).rem_euclid(9) as u8;
            actions.write(InventoryAction::Hotbar(slot));
        }
    }
    let typing = focus
        .as_ref()
        .and_then(|focus| focus.get())
        .and_then(|entity| elements.get(entity).ok())
        .is_some_and(|id| matches!(id.0.as_str(), "inventory-search" | "craft-search"));
    if state.open && input.just_pressed(GameAction::Pause) {
        if crafting.recipe_open {
            crafting_actions.write(crafting::CraftingAction::CloseRecipe);
        } else {
            actions.write(InventoryAction::Close);
        }
    } else if !typing && input.just_pressed(GameAction::Inventory) {
        actions.write(InventoryAction::Toggle);
    } else if !typing {
        if state.open
            && !crafting.recipe_open
            && input.keys.just_pressed(KeyCode::KeyR)
            && context
                .session
                .as_ref()
                .is_some_and(|session| session.game_mode == GameMode::Creative)
            && let Some(item) = pointer.catalog_item()
        {
            actions.write(InventoryAction::Recipe(item.into()));
        }
        let workbench = (!state.open)
            .then(|| station_target(&input, &context))
            .flatten();
        if let Some(position) = workbench {
            match context.chunks.as_ref().and_then(|c| c.block_at(position)) {
                Some(material @ 65..=73) => {
                    objects.write(terra_network::client::ObjectInteractionRequest {
                        position,
                        expected: material,
                        facing: context
                            .journal
                            .as_ref()
                            .and_then(|journal| journal.object_state(position))
                            .unwrap_or_default()
                            .facing,
                    });
                }
                Some(29 | 59) => {
                    actions.write(InventoryAction::OpenChest(position));
                }
                _ => {
                    actions.write(InventoryAction::OpenWorkbench(position));
                }
            }
        }
        if !state.blocks_input()
            && context.camera.as_ref().is_none_or(|camera| camera.captured)
            && context
                .session
                .as_ref()
                .is_some_and(|session| session.game_mode == GameMode::Creative)
            && input.just_pressed(GameAction::PickBlock)
            && let Some(terra_player::WorldTarget::Block(hit)) =
                context.target.as_ref().and_then(|target| target.0)
            && let Some(id) = context
                .chunks
                .as_ref()
                .and_then(|chunks| chunks.block_at(hit.position))
        {
            actions.write(InventoryAction::PickBlock(id));
        }
        if input.just_pressed(GameAction::DropStack)
            || (input.just_pressed(GameAction::Drop)
                && (input.keys.pressed(KeyCode::ShiftLeft)
                    || input.keys.pressed(KeyCode::ShiftRight)))
        {
            actions.write(InventoryAction::Drop(true));
        } else if input.just_pressed(GameAction::Drop) {
            actions.write(InventoryAction::Drop(false));
        }
        for (index, key) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .into_iter()
        .enumerate()
        {
            if input.keys.just_pressed(key) {
                actions.write(InventoryAction::Hotbar(index as u8));
            }
        }
        if state.open
            && (input.keys.just_pressed(KeyCode::ArrowRight)
                || input.keys.just_pressed(KeyCode::ArrowDown))
        {
            actions.write(InventoryAction::Cycle(1));
        } else if state.open
            && (input.keys.just_pressed(KeyCode::ArrowLeft)
                || input.keys.just_pressed(KeyCode::ArrowUp))
        {
            actions.write(InventoryAction::Cycle(-1));
        }
    }
}

#[derive(SystemParam)]
struct InventorySelection<'w> {
    palette: ResMut<'w, BlockPalette>,
    active: ResMut<'w, ActiveItem>,
    queue: ResMut<'w, pointer::InventoryQueue>,
    slot: ResMut<'w, SelectedInventorySlot>,
    focus: Option<ResMut<'w, InputFocus>>,
    reservations: Res<'w, crafting::CraftingState>,
}
impl InventorySelection<'_> {
    fn choose(
        &mut self,
        state: &mut InventoryState,
        session: &ClientSession,
        item: ItemKind,
        count: u16,
    ) {
        state.focused = Some(item);
        self.select(Some(item));
        state.active_slot = if session.game_mode == GameMode::Creative {
            if !session
                .inventory
                .slot(usize::from(state.hotbar))
                .is_some_and(|stack| stack.item == item && stack.count >= count)
            {
                self.request(
                    session,
                    ServerAction::CreativeSet {
                        slot: state.hotbar,
                        item,
                        count,
                    },
                );
            }
            Some(state.hotbar)
        } else {
            session
                .inventory
                .slots
                .iter()
                .position(|stack| stack.item == item && stack.count > 0)
                .map(|index| index as u8)
        };
        self.slot.0 = state.active_slot;
    }
    fn select(&mut self, item: Option<ItemKind>) {
        if self.active.0 != item {
            self.active.0 = item;
        }
        let selected = item
            .and_then(|item| match item {
                ItemKind::Block(id)
                | ItemKind::WoodenBlock { material: id, .. }
                | ItemKind::DyedBed { material: id, .. } => self
                    .palette
                    .choices
                    .iter()
                    .position(|choice| choice.id == id),
                ItemKind::Prop(_)
                | ItemKind::Tool(_)
                | ItemKind::Stick
                | ItemKind::Mineral(_)
                | ItemKind::Animal(_)
                | ItemKind::Bellows(_)
                | ItemKind::Dye(_) => None,
            })
            .unwrap_or(self.palette.choices.len());
        if self.palette.selected != selected {
            self.palette.selected = selected;
        }
    }
    fn request(&mut self, _session: &ClientSession, action: ServerAction) {
        let action = pointer::protect(action, &self.reservations);
        if !matches!(action, ServerAction::Move { count: 0, .. }) {
            self.queue.push(action);
        }
    }
}

#[derive(SystemParam)]
struct InventoryRuntime<'w> {
    blocked: ResMut<'w, PlayerInputBlock>,
    capture: MessageWriter<'w, CaptureRequest>,
    pointer: ResMut<'w, pointer::InventoryPointer>,
    time: Res<'w, Time>,
}

fn actions(
    mut events: MessageReader<InventoryAction>,
    input: GameplayInput,
    mut state: ResMut<InventoryState>,
    context: InventoryContext,
    mut selection: InventorySelection,
    runtime: InventoryRuntime,
) {
    let InventoryRuntime {
        mut blocked,
        mut capture,
        mut pointer,
        time,
    } = runtime;
    if !context.available() {
        if selection.slot.0.is_some() {
            selection.slot.0 = None;
        }
        if context
            .session
            .as_ref()
            .is_none_or(|session| !session.ready)
        {
            if *state != InventoryState::default() {
                *state = InventoryState::default();
            }
            selection.select(None);
        } else if state.blocks_input() || state.focused.is_some() {
            let mut next = InventoryState::default();
            if context
                .session
                .as_ref()
                .is_some_and(|session| session.ready)
            {
                next.hotbar = state.hotbar;
                next.active_slot = state.active_slot;
                next.inventory_revision = state.inventory_revision;
            }
            *state = next;
        }
        events.clear();
        *pointer = default();
        selection.queue.clear();
        return;
    }
    let Some(session) = context.session.as_ref() else {
        return;
    };
    selection.queue.owner = session.container.as_ref().map(|s| s.anchor);
    let failed_open = state.container_open_response.is_some_and(|response| {
        session.container.is_none()
            && session.inventory_response != response
            && !selection.queue.has_open_container()
    });
    if state.open && session.container.is_none() && (state.container_active || failed_open) {
        if session.inventory.cursor.is_some() || !selection.queue.is_empty() {
            selection.queue.push(ServerAction::ReturnCursor);
        }
        state.open = state.station.is_some();
        state.container_active = false;
        state.container_open_response = None;
        state.resume_pending = !state.open;
        *pointer = default();
    } else if state.open
        && session.container.is_some()
        && (!state.container_active
            || state.container_open_response.is_some()
            || state.station
                != session
                    .container
                    .as_ref()
                    .and_then(|container| container.workbench)
            || state.assembly
                != session
                    .container
                    .as_ref()
                    .is_some_and(|container| container.workbench.is_some()))
    {
        state.container_active = true;
        state.container_open_response = None;
        state.station = session
            .container
            .as_ref()
            .and_then(|container| container.workbench);
        state.assembly = state.station.is_some();
    }
    if let Some(slot) = state.active_slot
        && session
            .inventory
            .slot(usize::from(slot))
            .map(|stack| stack.item)
            == selection.active.0
    {
        selection.slot.0 = Some(slot);
    }
    if state.inventory_revision != Some(session.inventory.revision) {
        if state.inventory_revision.is_none() {
            state.active_slot = Some(state.hotbar);
        }
        state.inventory_revision = Some(session.inventory.revision);
        if let Some(slot) = state.active_slot {
            selection.slot.0 = Some(slot);
            selection.select(
                session
                    .inventory
                    .slot(usize::from(slot))
                    .map(|stack| stack.item),
            );
        }
        if state
            .source
            .is_some_and(|slot| session.inventory.slot(usize::from(slot)).is_none())
        {
            state.source = None;
        }
    }
    if state.resume_pending
        && !input.pressed(GameAction::Break)
        && !input.pressed(GameAction::Place)
        && !input.just_pressed(GameAction::Inventory)
        && !input.just_pressed(GameAction::Pause)
    {
        state.resume_pending = false;
        blocked.0 = false;
        capture.write(CaptureRequest(true));
    }
    for action in events.read() {
        if !state.open
            && selection.queue.has_container_lifecycle()
            && matches!(
                action,
                InventoryAction::Toggle
                    | InventoryAction::OpenChest(_)
                    | InventoryAction::OpenWorkbench(_)
            )
        {
            continue;
        }
        if (crafting::busy(&selection.reservations) || state.container_open_response.is_some())
            && !matches!(
                action,
                InventoryAction::Close
                    | InventoryAction::Toggle
                    | InventoryAction::Focus(_)
                    | InventoryAction::Search(_)
                    | InventoryAction::Category(_)
                    | InventoryAction::Page(_)
                    | InventoryAction::Leave { .. }
            )
        {
            continue;
        }
        match action {
            InventoryAction::OpenChest(position) if !state.open => {
                state.open = true;
                state.resume_pending = false;
                state.station = None;
                state.assembly = false;
                state.container_active = false;
                state.container_open_response = Some(session.inventory_response);
                selection.queue.push(ServerAction::OpenContainer {
                    position: *position,
                });
                capture.write(CaptureRequest(false));
            }
            InventoryAction::OpenWorkbench(position) if !state.open => {
                state.open = true;
                state.resume_pending = false;
                state.station = Some(*position);
                state.assembly = true;
                state.container_open_response = Some(session.inventory_response);
                selection.queue.push(ServerAction::OpenContainer {
                    position: *position,
                });
                capture.write(CaptureRequest(false));
            }
            InventoryAction::Assembly(open) if state.open => {
                state.assembly = *open;
                if !open {
                    state.station = None;
                }
            }
            InventoryAction::Toggle if !state.open => {
                state.open = true;
                state.resume_pending = false;
                state.focused = selection.active.0.or_else(|| {
                    selection
                        .palette
                        .selected()
                        .map(|choice| ItemKind::Block(choice.id))
                });
                capture.write(CaptureRequest(false));
            }
            InventoryAction::Close if !state.open => {}
            InventoryAction::Close | InventoryAction::Toggle => {
                if session.inventory.cursor.is_some() || !selection.queue.is_empty() {
                    selection.request(session, ServerAction::ReturnCursor);
                }
                if session.container.is_some()
                    || state.container_open_response.is_some()
                    || selection.queue.has_container_lifecycle()
                {
                    // Also close an opening request whose response has not arrived yet.
                    selection.queue.push(ServerAction::CloseContainer);
                }
                state.container_active = false;
                state.container_open_response = None;
                state.open = false;
                state.assembly = false;
                state.station = None;
                state.resume_pending = true;
                state.source = None;
                *pointer = default();
                if let Some(focus) = selection.focus.as_mut() {
                    focus.clear();
                }
            }
            InventoryAction::Hotbar(index) if *index < 9 => {
                state.hotbar = *index;
                state.active_slot = Some(*index);
                selection.slot.0 = Some(*index);
                if let Some(from) = state.source.take()
                    && state.open
                    && from != *index
                    && let Some(stack) = session.inventory.slot(usize::from(from))
                {
                    selection.request(
                        session,
                        ServerAction::Move {
                            from,
                            to: *index,
                            count: stack.count,
                        },
                    );
                }
                let item = session
                    .inventory
                    .slot(usize::from(*index))
                    .map(|stack| stack.item);
                state.focused = item;
                selection.select(item);
            }
            InventoryAction::Select(id) if state.open => {
                if let Some(item) =
                    available_items(session.game_mode, &session.inventory, &selection.palette)
                        .into_iter()
                        .find(|item| item_id(*item) == *id)
                {
                    selection.choose(
                        &mut state,
                        session,
                        item,
                        if input.keys.pressed(KeyCode::ShiftLeft)
                            || input.keys.pressed(KeyCode::ShiftRight)
                        {
                            STACK_LIMIT
                        } else {
                            1
                        },
                    );
                }
            }
            InventoryAction::Focus(id) if state.open => {
                state.focused =
                    available_items(GameMode::Creative, &session.inventory, &selection.palette)
                        .into_iter()
                        .find(|item| item_id(*item) == *id);
            }
            InventoryAction::Slot(index)
                if state.open && usize::from(*index) < terra_core::INVENTORY_SLOTS =>
            {
                if let Some(from) = state.source.take() {
                    if from != *index
                        && let Some(stack) = session.inventory.slot(usize::from(from))
                    {
                        selection.request(
                            session,
                            ServerAction::Move {
                                from,
                                to: *index,
                                count: stack.count,
                            },
                        );
                    }
                } else if let Some(stack) = session.inventory.slot(usize::from(*index)) {
                    state.source = Some(*index);
                    state.active_slot = Some(*index);
                    selection.slot.0 = Some(*index);
                    state.focused = Some(stack.item);
                    selection.select(Some(stack.item));
                }
            }
            InventoryAction::CancelMove => {
                state.source = None;
                selection.request(session, ServerAction::ReturnCursor);
            }
            InventoryAction::Trash if state.open && session.game_mode == GameMode::Creative => {
                pointer.cancel_gesture();
                if let Some(stack) = session.inventory.cursor {
                    selection.queue.push_queued(pointer::QueuedAction::held(
                        ServerAction::CreativeTrash { slot: None },
                        stack.item,
                    ));
                } else {
                    let slot = state.source.or(state.active_slot).unwrap_or(state.hotbar);
                    if session.inventory.slot(usize::from(slot)).is_some() {
                        state.source = None;
                        selection
                            .request(session, ServerAction::CreativeTrash { slot: Some(slot) });
                    }
                }
            }
            InventoryAction::Pointer {
                command,
                index,
                item,
                secondary,
            } if state.open => {
                pointer::press(
                    &mut pointer,
                    &mut state,
                    &mut selection,
                    session,
                    time.elapsed_secs_f64(),
                    pointer::Press {
                        command,
                        index: *index,
                        item,
                        secondary: *secondary,
                        shift: input.keys.pressed(KeyCode::ShiftLeft)
                            || input.keys.pressed(KeyCode::ShiftRight),
                    },
                );
            }
            InventoryAction::Hover {
                command,
                index,
                item,
            } if state.open => {
                if let Some(action) = pointer.hover(
                    command,
                    *index,
                    item,
                    input.keys.pressed(KeyCode::ShiftLeft)
                        || input.keys.pressed(KeyCode::ShiftRight),
                ) {
                    selection
                        .queue
                        .push_queued(action.protected(&selection.reservations));
                }
                state.focused =
                    available_items(GameMode::Creative, &session.inventory, &selection.palette)
                        .into_iter()
                        .find(|entry| item_id(*entry) == *item);
            }
            InventoryAction::Leave {
                command,
                index,
                item,
            } => pointer.leave(command, *index, item),
            InventoryAction::Outside(secondary)
                if state.open && session.inventory.cursor.is_some() =>
            {
                selection.request(session, ServerAction::DropCursor { all: !secondary });
            }
            InventoryAction::Drop(all) => {
                if state.open && session.inventory.cursor.is_some() {
                    selection.request(session, ServerAction::DropCursor { all: *all });
                } else {
                    if state.open
                        && let Some(stack) = session.inventory.slot(usize::from(state.hotbar))
                    {
                        let available = crafting::available_count(
                            &selection.reservations,
                            usize::from(state.hotbar),
                            stack.count,
                        );
                        if available == 0 {
                            continue;
                        }
                        if *all && available < stack.count {
                            selection.request(
                                session,
                                ServerAction::TakeToCursor {
                                    slot: state.hotbar,
                                    count: available,
                                },
                            );
                            selection.queue.push_queued(pointer::QueuedAction::held(
                                ServerAction::DropCursor { all: true },
                                stack.item,
                            ));
                            continue;
                        }
                    }
                    selection.request(
                        session,
                        ServerAction::DropSlot {
                            slot: state.hotbar,
                            all: *all,
                        },
                    );
                }
            }
            InventoryAction::PickBlock(id)
                if !state.blocks_input()
                    && session.game_mode == GameMode::Creative
                    && selection
                        .palette
                        .choices
                        .iter()
                        .any(|choice| choice.id == *id) =>
            {
                let item = ItemKind::Block(*id);
                let slot = (0..9)
                    .find(|index| {
                        session
                            .inventory
                            .slot(*index)
                            .is_some_and(|stack| stack.item == item)
                    })
                    .unwrap_or(usize::from(state.hotbar)) as u8;
                state.hotbar = slot;
                state.active_slot = Some(slot);
                state.focused = Some(item);
                selection.slot.0 = Some(slot);
                if session
                    .inventory
                    .slot(usize::from(slot))
                    .is_some_and(|stack| stack.item == item)
                {
                    selection.select(Some(item));
                } else {
                    selection.request(
                        session,
                        ServerAction::CreativeSet {
                            slot,
                            item,
                            count: STACK_LIMIT,
                        },
                    );
                }
            }
            InventoryAction::Search(value) if state.open => {
                state.search = value.chars().take(64).collect();
                state.page = 0;
            }
            InventoryAction::Category(value)
                if state.open
                    && (value == "all"
                        || selection
                            .palette
                            .creative_categories
                            .iter()
                            .any(|category| category.id == *value)) =>
            {
                state.category.clone_from(value);
                state.page = 0;
            }
            InventoryAction::Page(direction) if state.open => {
                state.page = if *direction < 0 {
                    state.page.saturating_sub(1)
                } else {
                    state.page.saturating_add(1).min(1024)
                };
            }
            InventoryAction::Cycle(direction) if state.open => {
                let items: Vec<_> =
                    available_items(session.game_mode, &session.inventory, &selection.palette)
                        .into_iter()
                        .filter(|item| {
                            session.game_mode != GameMode::Creative || catalog_item(*item)
                        })
                        .collect();
                if !items.is_empty() {
                    let current = selection.active.0.or_else(|| {
                        selection
                            .palette
                            .selected()
                            .map(|choice| ItemKind::Block(choice.id))
                    });
                    let index = items
                        .iter()
                        .position(|item| Some(*item) == current)
                        .map_or(0, |i| {
                            (i as i32 + direction.signum()).rem_euclid(items.len() as i32) as usize
                        });
                    selection.choose(
                        &mut state,
                        session,
                        items[index],
                        if input.keys.pressed(KeyCode::ShiftLeft)
                            || input.keys.pressed(KeyCode::ShiftRight)
                        {
                            STACK_LIMIT
                        } else {
                            1
                        },
                    );
                }
            }
            _ => {}
        }
    }
    if state.blocks_input() {
        blocked.0 = true;
    }
}

fn prop_kinds() -> [ResourcePropKind; 14] {
    ResourcePropKind::ALL
}
fn prop_slug(kind: ResourcePropKind) -> &'static str {
    kind.slug()
}
fn item_id(item: ItemKind) -> String {
    match item {
        ItemKind::Block(id) => format!("block-{id}"),
        ItemKind::WoodenBlock { material, wood } => format!("block-{material}-{}", wood.slug()),
        ItemKind::Prop(kind) => format!("prop-{}", prop_slug(kind)),
        ItemKind::Tool(tool) => format!("item-{}", tool.slug()),
        ItemKind::Stick => "item-stick".into(),
        ItemKind::Mineral(mineral) => format!("item-{}", mineral.slug()),
        ItemKind::Animal(product) => format!("item-{}", product.slug()),
        ItemKind::Bellows(kind) => format!("item-{}", kind.slug()),
        ItemKind::Dye(color) => format!("item-{}_dye", color.slug()),
        ItemKind::DyedBed { material, color } => format!("block-{material}-{}", color.slug()),
    }
}
fn available_items(
    mode: GameMode,
    inventory: &terra_core::Inventory,
    palette: &BlockPalette,
) -> Vec<ItemKind> {
    if mode == GameMode::Creative {
        return palette
            .choices
            .iter()
            .map(|choice| ItemKind::Block(choice.id))
            .chain(
                prop_kinds()
                    .into_iter()
                    .filter(|kind| {
                        *kind != ResourcePropKind::Seagrass
                            || !palette.choices.iter().any(|block| block.id == 50)
                    })
                    .map(ItemKind::Prop),
            )
            .chain(ItemKind::EXTRA)
            .collect();
    }
    let mut items = Vec::new();
    for slot in inventory.slots.iter().filter(|slot| slot.count > 0) {
        if !items.contains(&slot.item) {
            items.push(slot.item);
        }
    }
    items
}

fn catalog_item(item: ItemKind) -> bool {
    // Keep legacy metadata and carried stacks available, but expose only the adaptive pipe.
    !matches!(item.block_material(), Some(61..=64 | 95))
        && !matches!(item, ItemKind::DyedBed { .. })
}
