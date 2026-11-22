use crate::menu::{MenuPage, MenuState};
use bevy::{
    ecs::system::SystemParam, input::mouse::MouseWheel, input_focus::InputFocus, prelude::*,
};
use std::collections::HashMap;
use terra_core::{GameAction, GameMode, GameplayInput, ItemKind, ResourcePropKind, STACK_LIMIT};
use terra_network::{
    client::{ClientSession, ContainerRequest, InventoryRequest, SelectedInventorySlot},
    protocol::inventory::{ContainerView, InventoryAction as ServerAction},
};
use terra_player::{
    CaptureRequest, PlayerInputBlock, PlayerSet,
    interaction::{ActiveItem, BlockPalette},
};
use tilt_ui::ElementId;

#[path = "inventory_view.rs"]
mod view;
pub use view::InventoryView;
#[path = "inventory_alchemy.rs"]
mod alchemy;
#[path = "inventory_cache.rs"]
mod cache;
#[path = "crafting.rs"]
pub(crate) mod crafting;
#[path = "inventory_filter.rs"]
mod filter;
#[path = "inventory_input_trace.rs"]
mod input_trace;
#[path = "inventory_mixer.rs"]
mod mixer;
#[path = "inventory_pointer.rs"]
mod pointer;
#[path = "inventory_tooltip.rs"]
mod tooltip;
pub use tooltip::TooltipOwner;

#[cfg(test)]
#[path = "inventory_preview_tests.rs"]
mod preview_tests;
#[cfg(test)]
#[path = "inventory_slot_style_tests.rs"]
mod slot_style_tests;
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
    container_view: Option<ContainerView>,
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
    sort_menu: Option<bool>,
    sort_player_order: Option<terra_core::inventory_sort::SortOrder>,
    sort_storage_order: Option<(
        [i64; 3],
        ContainerView,
        terra_core::inventory_sort::SortOrder,
    )>,
    browser_collapsed: bool,
    filter_selecting: bool,
    filter_output: u8,
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
    ToggleCategories,
    ToggleBrowser,
    MixerColor(String),
    FilterAllowed(bool),
    FilterClear,
    DistributorAllowed(u8, bool),
    DistributorClear(u8),
    TunnelRandom(bool),
    BrewingRecipe(u16),
    CampfireLit(bool),
    HearthView(bool),
    Page(i32),
    Slot(u8),
    Hotbar(u8),
    CraftCell(u8),
    Craft,
    CraftHeld,
    CancelMove,
    Trash,
    ToggleSortMenu {
        storage: bool,
    },
    CloseSortMenu,
    Sort {
        storage: bool,
        order: terra_core::inventory_sort::SortOrder,
    },
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
    PickItem(ItemKind),
}

#[derive(SystemParam)]
struct InventoryContext<'w, 's> {
    menu: Option<Res<'w, MenuState>>,
    session: Option<Res<'w, ClientSession>>,
    settings: Res<'w, crate::SettingsView>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    target: Option<Res<'w, terra_player::TargetWorld>>,
    creature: Option<Res<'w, terra_player::TargetCreature>>,
    chunks: Option<Res<'w, terra_world::streaming::ChunkStore>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    camera: Option<Res<'w, terra_player::CameraInputState>>,
    fullmap: Option<Res<'w, crate::minimap::fullmap::FullMapState>>,
    pets: Option<Res<'w, crate::pets::PetMenuState>>,
    kingdom: Option<Res<'w, crate::kingdom::KingdomMenuState>>,
    npc_work: Option<Res<'w, crate::npc_work::WorkUiState>>,
    trains: Option<Res<'w, crate::trains::TrainUiState>>,
    view: Query<
        'w,
        's,
        (&'static Transform, &'static terra_player::CameraLook),
        With<terra_player::TerraCamera>,
    >,
    bodies: Query<'w, 's, &'static Transform, With<terra_player::PlayerBody>>,
    perspective: Option<Res<'w, terra_player::CameraPerspective>>,
}
impl InventoryContext<'_, '_> {
    fn available(&self) -> bool {
        !self.settings.open
            && !self
                .npc_work
                .as_ref()
                .is_some_and(|work| work.blocks_input())
            && !self
                .trains
                .as_ref()
                .is_some_and(|trains| trains.blocks_input())
            && !self.kingdom.as_ref().is_some_and(|k| k.blocks_input())
            && !self.pets.as_ref().is_some_and(|p| p.blocks_input())
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
    input_trace::install(app);
    crafting::install(app);
    cache::install(app);
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
            block_opening_frame
                .after(bevy::input::InputSystems)
                .after(crate::npc_work::NpcWorkInputSet),
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
                drive_crusher,
                held_consumption,
                pointer::finish_gesture,
                pointer::send_requests,
            )
                .chain()
                .after(crate::menu::MenuInputSet)
                .before(crate::survival::SurvivalUiSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(Update, hotbar_wheel.in_set(PlayerSet::WheelSelection))
        .add_systems(
            Update,
            (
                view::update_view.run_if(crate::cadence::ready),
                pointer::cursor_view,
            )
                .chain()
                .after(PlayerSet::Retargeting)
                .before(PlayerSet::Interaction),
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

pub(crate) fn reserved_for_transport(world: &World) -> [u16; terra_core::INVENTORY_SLOTS] {
    world
        .get_resource::<crafting::CraftingState>()
        .map_or([0; terra_core::INVENTORY_SLOTS], crafting::reserved_counts)
}

fn browser_layout(
    nodes: Query<(
        Entity,
        &ElementId,
        &ComputedNode,
        &Node,
        Option<&crate::frames::CatalogGridCapacity>,
    )>,
    parents: Query<&ChildOf>,
    mut state: ResMut<InventoryState>,
    view: Option<Res<InventoryView>>,
) {
    if !state.open && !view.as_ref().is_some_and(|view| view.prewarming) {
        return;
    }
    let Some((grid, _, computed, node, fixed)) = nodes
        .iter()
        .find(|(_, id, c, _, _)| id.0 == "inventory-browser-grid" && !c.is_empty())
    else {
        return;
    };
    let cell = nodes.iter().find(|(entity, id, c, _, _)| {
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
    let Some((_, _, cell, _, _)) = cell else {
        return;
    };
    let gap = |value| match value {
        Val::Px(px) => px / computed.inverse_scale_factor(),
        _ => 0.0,
    };
    let inset = computed.content_inset();
    let space = computed.size() - inset.min_inset - inset.max_inset;
    let gaps = Vec2::new(gap(node.column_gap), gap(node.row_gap));
    let capacity = if let Some(fixed) = fixed {
        fixed.0.clamp(1, 1024)
    } else if !node.grid_template_rows.is_empty() && !node.grid_template_columns.is_empty() {
        // Explicit fractional tracks round to physical pixels; floor can lose an
        // entire row/column when a cell differs by a fraction of a pixel.
        let count = ((space + gaps) / (cell.size() + gaps).max(Vec2::ONE))
            .round()
            .max(Vec2::ONE);
        (count.x as usize * count.y as usize).clamp(1, 1024)
    } else {
        browser_capacity(space, cell.size(), gaps)
    };
    if state.page_capacity != capacity {
        let first = state.page.saturating_mul(state.page_capacity);
        state.page_capacity = capacity;
        state.page = first / capacity;
    }
}

fn browser_capacity(space: Vec2, cell: Vec2, gap: Vec2) -> usize {
    let count = ((space + gap + Vec2::splat(0.5)) / (cell + gap).max(Vec2::ONE))
        .floor()
        .max(Vec2::ONE);
    (count.x as usize * count.y as usize).clamp(1, 1024)
}

type InventoryLayoutSettlement = Option<(Entity, Vec2, usize, bool, u8)>;

fn reveal_layout(
    nodes: Query<(Entity, &tilt_ui::ElementId, &ComputedNode)>,
    parents: Query<&ChildOf>,
    mut view: ResMut<InventoryView>,
    state: Option<Res<InventoryState>>,
    mut settled: Local<InventoryLayoutSettlement>,
) {
    if !view.open && !view.prewarming {
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
    if view.open && view.layout_style == "opacity: 1; pointer-events: auto;" {
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
    let (capacity, dropdown) = state.as_ref().map_or((0, false), |state| {
        (state.page_capacity, state.category_dropdown)
    });
    if view.prewarming
        && (capacity == 0
            || capacity != view.browser_capacity
            || dropdown != view.category_dropdown)
    {
        *settled = None;
        return;
    }
    let frames = settled
        .as_ref()
        .filter(|(old, extent, old_capacity, old_dropdown, _)| {
            *old == entity
                && extent.abs_diff_eq(size, 0.1)
                && *old_capacity == capacity
                && *old_dropdown == dropdown
        })
        .map_or(0, |(_, _, _, _, frames)| frames.saturating_add(1));
    *settled = Some((entity, size, capacity, dropdown, frames));
    if frames >= 2 {
        if view.open {
            view.layout_style = "opacity: 1; pointer-events: auto;".into();
        } else {
            view.prewarming = false;
        }
    }
}

pub(crate) fn item_label(item: ItemKind, palette: &BlockPalette) -> String {
    view::item_metadata(item, palette).0
}

pub(crate) fn editor_item_metadata(
    item: ItemKind,
    palette: &BlockPalette,
) -> (String, String, String) {
    view::item_metadata(item, palette)
}

pub(crate) fn preview_key(item: ItemKind) -> String {
    match item {
        ItemKind::FilledWateringCan { tool, .. } | ItemKind::WornTool { tool, .. } => {
            preview_key(ItemKind::Tool(tool))
        }
        ItemKind::Block(id) => format!("{PREVIEW_ALT_PREFIX}{id}"),
        ItemKind::Prop(kind) => format!("{PROP_ALT_PREFIX}{}", prop_slug(kind)),
        ItemKind::Tool(_)
        | ItemKind::Stick
        | ItemKind::Mineral(_)
        | ItemKind::Animal(_)
        | ItemKind::Bellows(_)
        | ItemKind::Armor(_)
        | ItemKind::Accessory(_)
        | ItemKind::Cape(_)
        | ItemKind::HeraldicCape { .. }
        | ItemKind::Dust(_)
        | ItemKind::Crop(_)
        | ItemKind::Flower(_)
        | ItemKind::Dye(_)
        | ItemKind::DyedBed { .. }
        | ItemKind::TintedBlock { .. }
        | ItemKind::WoodenBlock { .. }
        | ItemKind::GlassBottle
        | ItemKind::WaterBottle
        | ItemKind::Potion(_)
        | ItemKind::Apple
        | ItemKind::AppleCore
        | ItemKind::BloodBerry
        | ItemKind::CrimsonMushroom
        | ItemKind::ThornTwig
        | ItemKind::BloodCrystal
        | ItemKind::DarkSap
        | ItemKind::BloodLeaf
        | ItemKind::BloodSpore
        | ItemKind::BloodSlimeGel
        | ItemKind::Bone
        | ItemKind::Bitumen
        | ItemKind::GloomEssence
        | ItemKind::DragonLoot(_)
        | ItemKind::LightSplitter
        | ItemKind::RoadAggregate => {
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

#[derive(Component)]
struct PreviewTint([f32; 3]);

type PreviewImageData = (
    Entity,
    &'static tilt_ui::ImageMetadata,
    &'static mut bevy::ui::widget::ImageNode,
    Option<&'static Visibility>,
    Option<&'static PreviewTint>,
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
        for (entity, metadata, image, visibility, tint) in &mut images.p0() {
            bind_preview_image(
                &mut commands,
                &source,
                entity,
                metadata,
                image,
                visibility,
                tint,
            );
        }
        return;
    }
    if portrait_changed {
        for (entity, metadata, image, visibility, tint) in &mut images.p0() {
            if metadata.alt.as_deref() == Some("Player preview") {
                bind_preview_image(
                    &mut commands,
                    &source,
                    entity,
                    metadata,
                    image,
                    visibility,
                    tint,
                );
            }
        }
    }
    // Runtime-owned resets and newly materialized/rebound nodes still need repair.
    for (entity, metadata, image, visibility, tint) in &mut images.p1() {
        if portrait_changed && metadata.alt.as_deref() == Some("Player preview") {
            continue;
        }
        bind_preview_image(
            &mut commands,
            &source,
            entity,
            metadata,
            image,
            visibility,
            tint,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_preview_image(
    commands: &mut Commands,
    source: &PreviewSourceIndex,
    entity: Entity,
    metadata: &tilt_ui::ImageMetadata,
    mut image: Mut<bevy::ui::widget::ImageNode>,
    visibility: Option<&Visibility>,
    previous_tint: Option<&PreviewTint>,
) {
    let Some(alt) = metadata.alt.as_deref() else {
        return;
    };
    let mixed = alt
        .strip_prefix("Item preview teramic::mixed_dye")
        .and_then(mixer::parse_target);
    let preview = if mixed.is_some() {
        source.images.get(&preview_key(ItemKind::Dye(
            terra_core::beds::DyeColor::White,
        )))
    } else if alt == "Player preview" {
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
    if let Some(rgb) = mixed {
        let current = image.color.to_srgba();
        if previous_tint.is_none() {
            commands
                .entity(entity)
                .insert(PreviewTint([current.red, current.green, current.blue]));
        }
        let color = Color::srgba_u8(rgb[0], rgb[1], rgb[2], 255).with_alpha(current.alpha);
        if image.color != color {
            image.color = color;
        }
    } else if let Some(PreviewTint([r, g, b])) = previous_tint {
        let color = Color::srgba(*r, *g, *b, image.color.alpha());
        if image.color != color {
            image.color = color;
        }
        commands.entity(entity).remove::<PreviewTint>();
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
    active: Option<Res<ActiveItem>>,
    mut blocked: ResMut<PlayerInputBlock>,
) {
    if context.available()
        && (input.just_pressed(GameAction::Inventory)
            || station_target(
                &input,
                &context,
                held_action_priority(&input, &context, active.as_deref()),
            )
            .and_then(|p| context.chunks.as_ref()?.block_at(p))
            .is_some_and(|material| {
                matches!(material, 28 | 29 | 59 | 99 | 152 | 156..=157 | 159..=162 | 216)
                    || terra_core::profession_stations::is_depot(material)
            }))
    {
        blocked.0 = true;
    }
}

fn faucet_control_target(
    input: &GameplayInput,
    context: &InventoryContext,
    held_priority: bool,
) -> Option<([i64; 3], bool)> {
    if input.keys.pressed(KeyCode::ShiftLeft)
        || input.keys.pressed(KeyCode::ShiftRight)
        || context
            .camera
            .as_ref()
            .is_some_and(|camera| !camera.capture_ready())
        || context
            .session
            .as_ref()
            .is_none_or(|session| !session.allow_build || session.game_mode == GameMode::Spectator)
    {
        return None;
    }
    let closing = input.just_pressed(GameAction::Break);
    if !closing
        && (!(input.just_pressed(GameAction::Place) || input.just_pressed(GameAction::Use))
            || held_priority)
    {
        return None;
    }
    let terra_player::WorldTarget::Block(hit) = context.target.as_ref()?.0? else {
        return None;
    };
    (context.chunks.as_ref()?.block_at(hit.position) == Some(terra_core::plumbing::FAUCET))
        .then_some((hit.position, closing))
}

fn station_target(
    input: &GameplayInput,
    context: &InventoryContext,
    held_priority: bool,
) -> Option<[i64; 3]> {
    if !(input.just_pressed(GameAction::Use) || input.just_pressed(GameAction::Place))
        || creature_place_target(input, context)
        || input.keys.pressed(KeyCode::ShiftLeft)
        || input.keys.pressed(KeyCode::ShiftRight)
        || held_priority
        || carried_item_priority(context)
    {
        return None;
    }
    let terra_player::WorldTarget::Block(hit) = context.target.as_ref()?.0? else {
        return None;
    };
    let material = context.chunks.as_ref()?.block_at(hit.position)?;
    (terra_core::profession_stations::is_depot(material)
        || matches!(
            material,
                28
                | 29
                | 59
                | 65..=73
                | 99
                | 131
                | 139..=140
                | 152
                | 156..=157
                | 159..=162
                | 166..=167
                | 209
                | 216
        ))
    .then_some(hit.position)
}

fn carried_item_priority(context: &InventoryContext) -> bool {
    let Some(session) = context.session.as_deref() else {
        return false;
    };
    let Some(journal) = context.journal.as_deref() else {
        return false;
    };
    let Some((camera, look)) = context.view.iter().next() else {
        return false;
    };
    let origin = if context
        .perspective
        .as_deref()
        .is_some_and(|p| p.is_third_person())
    {
        context
            .bodies
            .iter()
            .next()
            .map_or(camera.translation, |body| {
                body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET
            })
    } else {
        camera.translation
    };
    let limit = context
        .target
        .as_deref()
        .and_then(|target| target.0)
        .map_or(6.0, |hit| match hit {
            terra_player::WorldTarget::Block(hit) => hit.distance.min(6.0),
            terra_player::WorldTarget::Prop(hit) => hit.distance.min(6.0),
        });
    terra_world::conveyors::carried_drop_hit(
        &session.drops,
        journal,
        origin.to_array().map(f64::from),
        (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
        limit,
    )
    .is_some()
}

fn creature_place_target(input: &GameplayInput, context: &InventoryContext) -> bool {
    input.pressed(GameAction::Place)
        && !input.pressed(GameAction::Use)
        && context
            .creature
            .as_ref()
            .is_some_and(|target| target.0.is_some())
}

fn mounted_crank_target(
    input: &GameplayInput,
    context: &InventoryContext,
    held_priority: bool,
) -> Option<[i64; 3]> {
    if creature_place_target(input, context) || held_priority {
        return None;
    }
    let terra_player::WorldTarget::Block(hit) = context.target.as_ref()?.0? else {
        return None;
    };
    let journal = context.journal.as_ref()?;
    (context.chunks.as_ref()?.block_at(hit.position) == Some(terra_core::crusher::CRANK_MATERIAL)
        && journal.crank_anchor(hit.position).is_some())
    .then_some(hit.position)
}

/// Holding Use with a rail owns mode selection, independent of the looked-at object.
/// Place retains the existing station and object interaction policy.
pub(crate) fn rail_mode_priority(input: &GameplayInput, active: Option<&ActiveItem>) -> bool {
    input.pressed(GameAction::Use)
        && active
            .and_then(|active| active.0)
            .and_then(ItemKind::block_material)
            .is_some_and(terra_core::rails::is_rail)
}

fn held_action_priority(
    input: &GameplayInput,
    context: &InventoryContext,
    active: Option<&ActiveItem>,
) -> bool {
    let Some(item) = active.and_then(|active| active.0) else {
        return false;
    };
    if rail_mode_priority(input, active) || item.tool_kind() == Some(terra_core::ToolKind::Wrench) {
        return true;
    }
    if context
        .target
        .as_ref()
        .and_then(|target| target.0)
        .is_some_and(|target| {
            let terra_player::WorldTarget::Block(hit) = target else {
                return false;
            };
            context
                .chunks
                .as_ref()
                .and_then(|chunks| chunks.block_at(hit.position))
                .is_some_and(|target| {
                    (terra_core::rails::is_rail(target)
                        && item
                            .block_material()
                            .is_some_and(terra_core::rails::is_rail))
                        || (terra_core::plumbing::is_part(target)
                            && item
                                .block_material()
                                .is_some_and(terra_core::plumbing::is_part))
                        || target == 167
                            && (item == ItemKind::GlassBottle
                                || item.tool_kind().is_some_and(|tool| {
                                    tool == terra_core::ToolKind::Bucket
                                        || tool.watering_capacity() > 0
                                }))
                })
        })
    {
        return true;
    }
    matches!(item.block_material(), Some(100 | 141..=162))
        && context
            .target
            .as_ref()
            .and_then(|target| target.0)
            .is_some_and(|target| {
                let terra_player::WorldTarget::Block(hit) = target else {
                    return false;
                };
                context
                    .chunks
                    .as_ref()
                    .and_then(|chunks| chunks.block_at(hit.position))
                    .is_some_and(|target| match (item.block_material(), target) {
                        (Some(161), 162) | (Some(162), 161) => context
                            .journal
                            .as_ref()
                            .is_some_and(|journal| !journal.has_attached_campfire(hit.position)),
                        (Some(152 | 153 | 157 | 159), _) => {
                            matches!(target, 29 | 59 | 99 | 100 | 156)
                        }
                        _ => terra_core::kinetic::is_part(target),
                    })
            })
}

fn drive_crusher(
    input: GameplayInput,
    state: Res<InventoryState>,
    context: InventoryContext,
    active: Option<Res<ActiveItem>>,
    crank: Option<ResMut<terra_network::client::CrankInput>>,
) {
    let Some(mut crank) = crank else { return };
    let held = (input.pressed(GameAction::Place) || input.pressed(GameAction::Use))
        && !input.pressed(GameAction::Break)
        && !input.keys.pressed(KeyCode::ShiftLeft)
        && !input.keys.pressed(KeyCode::ShiftRight);
    let enabled = held
        && context.available()
        && !state.blocks_input()
        && context.camera.as_ref().is_none_or(|camera| camera.captured)
        && context.session.as_ref().is_some_and(|session| {
            session.ready
                && session.game_mode != GameMode::Spectator
                && !session.stats.survival.dead()
                && session.sleeping().is_none()
        });
    let target = enabled
        .then(|| {
            mounted_crank_target(
                &input,
                &context,
                held_action_priority(&input, &context, active.as_deref()),
            )
        })
        .flatten();
    if crank.0 != target {
        crank.0 = target;
    }
}

#[derive(Default)]
struct HeldConsumption {
    slot: Option<u8>,
    next_pulse: f64,
    opening_until_release: bool,
}

#[allow(clippy::too_many_arguments)]
fn held_consumption(
    input: GameplayInput,
    state: Res<InventoryState>,
    context: InventoryContext,
    active: Option<Res<ActiveItem>>,
    creature: Option<Res<terra_player::TargetCreature>>,
    time: Res<Time>,
    mut held: Local<HeldConsumption>,
    mut requests: MessageWriter<terra_network::client::ConsumptionRequest>,
) {
    if !input.pressed(GameAction::Use) && !input.pressed(GameAction::Place) {
        held.opening_until_release = false;
    } else if context.available()
        && !state.blocks_input()
        && station_target(
            &input,
            &context,
            held_action_priority(&input, &context, active.as_deref()),
        )
        .and_then(|position| context.chunks.as_ref()?.block_at(position))
        .is_some_and(|material| matches!(material, 65..=73 | 139..=140 | 166..=167 | 209))
    {
        // Doors stay in gameplay; holding the interaction must not start a bite.
        held.opening_until_release = true;
    }
    let slot = (context.available()
        && !state.blocks_input()
        && context.camera.as_ref().is_none_or(|camera| camera.captured)
        && !held.opening_until_release
        && input.consume_pressed()
        && (input.pressed(GameAction::FlyDown)
            || !context.target.as_ref().is_some_and(|target| matches!(target.0,
                Some(terra_player::WorldTarget::Prop(hit)) if hit.prop.kind == terra_world::props::PropKind::BeeNest)))
        && mounted_crank_target(&input, &context, held_action_priority(&input, &context, active.as_deref())).is_none()
        && station_target(&input, &context, held_action_priority(&input, &context, active.as_deref())).is_none()
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
                    .is_some_and(|stack| terra_core::food(stack.item).is_some()
                        || terra_core::brewing::drink_seconds(stack.item).is_some())
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
    mut commands: Commands,
    input: GameplayInput,
    state: Res<InventoryState>,
    context: InventoryContext,
    active: Option<Res<ActiveItem>>,
    palette: Res<BlockPalette>,
    focus: Option<Res<InputFocus>>,
    elements: Query<&ElementId>,
    pickers: Query<(Entity, &tilt_ui::ColorPickerState)>,
    mut actions: MessageWriter<InventoryAction>,
    crafting: Res<crafting::CraftingView>,
    mut crafting_actions: MessageWriter<crafting::CraftingAction>,
    pointer: Res<pointer::InventoryPointer>,
    mut objects: MessageWriter<terra_network::client::ObjectInteractionRequest>,
) {
    if !context.available() {
        return;
    }
    if state.open
        && let Some((picker, _)) = pickers.iter().find(|(_, picker)| picker.open)
    {
        if input.just_pressed(GameAction::Pause) {
            commands.queue(move |world: &mut World| {
                tilt_ui::set_color_picker_open(world, picker, false);
            });
        }
        return;
    }
    let typing = focus
        .as_ref()
        .and_then(|focus| focus.get())
        .and_then(|entity| elements.get(entity).ok())
        .is_some_and(|id| matches!(id.0.as_str(), "inventory-search" | "craft-search"));
    if state.open && input.just_pressed(GameAction::Pause) {
        if state.sort_menu.is_some() {
            actions.write(InventoryAction::CloseSortMenu);
        } else if crafting.recipe_open {
            crafting_actions.write(crafting::CraftingAction::CloseRecipe);
        } else {
            actions.write(InventoryAction::Close);
        }
    } else if !typing && input.just_pressed(GameAction::Inventory) {
        if state.open && crafting.recipe_open {
            crafting_actions.write(crafting::CraftingAction::CloseRecipe);
        } else {
            actions.write(InventoryAction::Toggle);
        }
    } else if !typing {
        if state.open
            && input.keys.just_pressed(KeyCode::KeyR)
            && let Some(item) = pointer.recipe_item()
            && (pointer.recipe_is_preview()
                || (!crafting.recipe_open
                    && ((pointer.recipe_is_catalog()
                        && context
                            .session
                            .as_ref()
                            .is_some_and(|session| session.game_mode == GameMode::Creative))
                        || context.session.as_ref().is_some_and(|session| {
                            inspectable_item(session, &palette, item)
                                .is_some_and(|item| terra_core::tints::color(item).is_some())
                        }))))
        {
            actions.write(InventoryAction::Recipe(item.into()));
        }
        let faucet = (!state.open)
            .then(|| {
                faucet_control_target(
                    &input,
                    &context,
                    held_action_priority(&input, &context, active.as_deref()),
                )
            })
            .flatten();
        if let Some((position, closing)) = faucet {
            objects.write(terra_network::client::ObjectInteractionRequest {
                position,
                expected: terra_core::plumbing::FAUCET,
                facing: context
                    .journal
                    .as_ref()
                    .and_then(|journal| journal.object_state(position))
                    .unwrap_or_default()
                    .facing,
                closing,
            });
        }
        let workbench = (!state.open && faucet.is_none())
            .then(|| {
                station_target(
                    &input,
                    &context,
                    held_action_priority(&input, &context, active.as_deref()),
                )
            })
            .flatten();
        if let Some(position) = workbench {
            match context.chunks.as_ref().and_then(|c| c.block_at(position)) {
                Some(material @ (65..=73 | 131 | 139..=140 | 166..=167 | 209)) => {
                    objects.write(terra_network::client::ObjectInteractionRequest {
                        position,
                        expected: material,
                        facing: context
                            .journal
                            .as_ref()
                            .and_then(|journal| journal.object_state(position))
                            .unwrap_or_default()
                            .facing,
                        closing: false,
                    });
                }
                Some(material) if terra_core::profession_stations::is_depot(material) => {
                    actions.write(InventoryAction::OpenChest(position));
                }
                Some(29 | 59 | 99 | 152 | 156..=157 | 159..=162 | 216) => {
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
            let item = context
                .chunks
                .as_deref()
                .zip(context.journal.as_deref())
                .and_then(|(chunks, journal)| {
                    if let Some((camera, look)) = context.view.iter().next() {
                        let origin = if context
                            .perspective
                            .as_deref()
                            .is_some_and(|perspective| perspective.is_third_person())
                        {
                            context
                                .bodies
                                .iter()
                                .next()
                                .map_or(camera.translation, |body| {
                                    body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET
                                })
                        } else {
                            camera.translation
                        };
                        terra_world::placement::picked_item_for_ray(
                            chunks,
                            journal,
                            hit.position,
                            (
                                origin.to_array().map(f64::from),
                                (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
                            ),
                        )
                    } else {
                        terra_world::placement::picked_item(chunks, journal, hit.position)
                    }
                })
                .unwrap_or(ItemKind::Block(id));
            actions.write(InventoryAction::PickItem(item));
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
    fn hotbar(&mut self, state: &mut InventoryState, session: &ClientSession, index: u8) {
        state.hotbar = index;
        state.active_slot = Some(index);
        self.slot.0 = Some(index);
        if let Some(from) = state.source.take()
            && state.open
            && from != index
            && let Some(stack) = session.inventory.slot(usize::from(from))
        {
            self.request(
                session,
                ServerAction::Move {
                    from,
                    to: index,
                    count: stack.count,
                },
            );
        }
        let item = session
            .inventory
            .slot(usize::from(index))
            .map(|stack| stack.item);
        state.focused = item;
        self.select(item);
    }

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
                | ItemKind::TintedBlock { material: id, .. }
                | ItemKind::DyedBed { material: id, .. } => self
                    .palette
                    .choices
                    .iter()
                    .position(|choice| choice.id == id),
                ItemKind::Prop(_)
                | ItemKind::Tool(_)
                | ItemKind::WornTool { .. }
                | ItemKind::FilledWateringCan { .. }
                | ItemKind::Stick
                | ItemKind::Mineral(_)
                | ItemKind::Animal(_)
                | ItemKind::Bellows(_)
                | ItemKind::Armor(_)
                | ItemKind::Accessory(_)
                | ItemKind::Cape(_)
                | ItemKind::HeraldicCape { .. }
                | ItemKind::Dust(_)
                | ItemKind::Crop(_)
                | ItemKind::Flower(_)
                | ItemKind::Dye(_)
                | ItemKind::GlassBottle
                | ItemKind::WaterBottle
                | ItemKind::Potion(_)
                | ItemKind::Apple
                | ItemKind::AppleCore
                | ItemKind::BloodBerry
                | ItemKind::CrimsonMushroom
                | ItemKind::ThornTwig
                | ItemKind::BloodCrystal
                | ItemKind::DarkSap
                | ItemKind::BloodLeaf
                | ItemKind::BloodSpore
                | ItemKind::BloodSlimeGel
                | ItemKind::Bone
                | ItemKind::Bitumen
                | ItemKind::GloomEssence
                | ItemKind::DragonLoot(_)
                | ItemKind::LightSplitter
                | ItemKind::RoadAggregate => None,
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

fn hotbar_wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut state: ResMut<InventoryState>,
    context: InventoryContext,
    mut selection: InventorySelection,
    mut rail: Option<ResMut<terra_player::interaction::RailBuildContext>>,
) {
    let delta: f32 = wheel.read().map(|event| event.y).sum();
    if !context.available()
        || state.open
        || rail.as_ref().is_some_and(|rail| rail.captures_wheel)
        || !delta.is_finite()
        || delta.abs() <= f32::EPSILON
    {
        return;
    }
    let Some(session) = context.session.as_deref() else {
        return;
    };
    let index = (i32::from(state.hotbar) - delta.signum() as i32).rem_euclid(9) as u8;
    selection.hotbar(&mut state, session, index);
    if let Some(rail) = rail.as_mut() {
        rail.mark_hotbar_changed();
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
    view: Option<Res<InventoryView>>,
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
                // A teleport may interrupt closing before the input/capture handoff.
                next.resume_pending = state.blocks_input()
                    && context
                        .menu
                        .as_ref()
                        .is_none_or(|menu| menu.page == MenuPage::Playing)
                    && context
                        .session
                        .as_ref()
                        .is_some_and(|session| !session.dead());
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
    selection.queue.owner_view = session.container.as_ref().map(|s| s.view);
    if session.inventory.cursor.is_some()
        || !selection.queue.is_empty()
        || crafting::busy(&selection.reservations)
    {
        state.sort_menu = None;
    }
    let container_view = session.container.as_ref().map(|container| container.view);
    if state.container_view != container_view {
        pointer.cancel_gesture();
        state.sort_menu = None;
        state.container_view = container_view;
    }
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
            || state
                .container_open_response
                .is_some_and(|response| session.inventory_response != response)
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
        if std::env::var_os("TERRA_INVENTORY_PORTRAIT_SMOKE").is_some()
            && matches!(
                action,
                InventoryAction::ToggleSortMenu { .. } | InventoryAction::CloseSortMenu
            )
        {
            info!(?action, open = state.open, cursor_empty = session.inventory.cursor.is_none(), queue_empty = selection.queue.is_empty(), crafting_busy = crafting::busy(&selection.reservations), opening = ?state.container_open_response, sortable = session.container.as_ref().is_some_and(|container| container.sortable()), "Native inventory sorting intent");
        }
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
                    | InventoryAction::CloseSortMenu
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
            InventoryAction::CloseSortMenu => state.sort_menu = None,
            InventoryAction::ToggleSortMenu { storage }
                if state.open
                    && session.inventory.cursor.is_none()
                    && selection.queue.is_empty() =>
            {
                if !*storage
                    || session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.sortable())
                {
                    state.sort_menu = if state.sort_menu == Some(*storage) {
                        None
                    } else {
                        Some(*storage)
                    };
                }
            }
            InventoryAction::Sort { storage, order }
                if state.open
                    && session.inventory.cursor.is_none()
                    && selection.queue.is_empty() =>
            {
                if *storage
                    && !session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.sortable())
                {
                    continue;
                }
                pointer.cancel_gesture();
                state.sort_menu = None;
                state.source = None;
                let request = if *storage {
                    if let Some(container) = &session.container {
                        state.sort_storage_order = Some((container.anchor, container.view, *order));
                    }
                    ServerAction::SortContainer { order: *order }
                } else {
                    state.sort_player_order = Some(*order);
                    ServerAction::Sort {
                        order: *order,
                        reserved: crafting::reserved_counts(&selection.reservations),
                    }
                };
                selection.queue.push_empty(request);
            }
            InventoryAction::TunnelRandom(random) if state.open => {
                filter::set_random(&mut selection, session, *random);
            }
            InventoryAction::BrewingRecipe(recipe) if state.open => {
                alchemy::select_recipe(&mut selection, session, *recipe);
            }
            InventoryAction::CampfireLit(lit) if state.open => {
                alchemy::set_lit(&mut selection, session, *lit);
            }
            InventoryAction::HearthView(fire)
                if state.open
                    && session.inventory.cursor.is_none()
                    && selection.queue.is_empty() =>
            {
                if let Some(container) = session.container.as_ref().filter(|container| {
                    container.attached_campfire
                        && if *fire {
                            container.cauldron.is_some() && container.view == ContainerView::Primary
                        } else {
                            container.campfire.is_some()
                                && container.view == ContainerView::AttachedCampfire
                        }
                }) {
                    pointer.cancel_gesture();
                    state.container_open_response = Some(session.inventory_response);
                    selection.queue.push(if *fire {
                        ServerAction::OpenCampfire {
                            position: container.anchor,
                        }
                    } else {
                        ServerAction::OpenContainer {
                            position: container.anchor,
                        }
                    });
                }
            }
            InventoryAction::FilterAllowed(allowed)
                if state.open
                    && session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.filter.is_some()) =>
            {
                filter::set_allowed(&mut selection, session, state.filter_output, *allowed);
            }
            InventoryAction::FilterClear
                if state.open
                    && session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.filter.is_some()) =>
            {
                state.filter_selecting = false;
                filter::set_item(&mut selection, session, state.filter_output, None);
            }
            InventoryAction::DistributorAllowed(output, allowed)
                if state.open
                    && *output < 3
                    && session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.distributor.is_some()) =>
            {
                state.filter_output = *output;
                filter::set_allowed(&mut selection, session, *output, *allowed);
            }
            InventoryAction::DistributorClear(output)
                if state.open
                    && *output < 3
                    && session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.distributor.is_some()) =>
            {
                state.filter_output = *output;
                state.filter_selecting = false;
                filter::set_item(&mut selection, session, *output, None);
            }
            InventoryAction::MixerColor(value) if state.open => {
                if let Some(target) = mixer::parse_target(value)
                    && let Some(container) =
                        session.container.as_ref().filter(|c| c.mixer.is_some())
                    && selection
                        .queue
                        .mixer_target(container.anchor)
                        .or_else(|| container.mixer.as_ref().map(|m| m.target))
                        != Some(target)
                {
                    selection.request(
                        session,
                        ServerAction::SetMixerColor {
                            station: container.anchor,
                            target,
                        },
                    );
                }
            }
            InventoryAction::OpenChest(position) if !state.open => {
                state.filter_selecting = false;
                state.filter_output = 0;
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
                state.sort_menu = None;
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
                state.filter_selecting = false;
                *pointer = default();
                if let Some(focus) = selection.focus.as_mut() {
                    focus.clear();
                }
            }
            InventoryAction::Hotbar(index) if *index < 9 => {
                selection.hotbar(&mut state, session, *index);
            }
            InventoryAction::Select(id) if state.open => {
                if filter::select(&mut state, &mut selection, session, id) {
                    continue;
                }
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
                            item.max_stack()
                        } else {
                            1
                        },
                    );
                }
            }
            InventoryAction::Focus(id) if state.open => {
                state.focused = inspectable_item(session, &selection.palette, id);
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
                if input.keys.pressed(KeyCode::ControlLeft)
                    || input.keys.pressed(KeyCode::ControlRight)
                {
                    state.source = None;
                    selection.request(session, ServerAction::CreativeClear);
                } else if let Some(stack) = session.inventory.cursor {
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
                if command == "catalog" && selection.reservations.recipe_open {
                    pointer.cancel_gesture();
                    continue;
                }
                if filter::press(
                    &mut state,
                    &mut selection,
                    session,
                    pointer::Press {
                        command,
                        index: *index,
                        item,
                        secondary: *secondary,
                        shift: false,
                    },
                ) {
                    pointer.cancel_gesture();
                    continue;
                }
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
                state.focused = inspectable_item(session, &selection.palette, item);
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
            InventoryAction::PickBlock(id) => pick_item(
                &mut state,
                &mut selection,
                session,
                ItemKind::Block(terra_core::castle::item_material(*id)),
            ),
            InventoryAction::PickItem(item) => {
                pick_item(&mut state, &mut selection, session, *item)
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
                state.category_dropdown = false;
                state.page = 0;
            }
            InventoryAction::ToggleCategories if state.open => {
                state.category_dropdown = !state.category_dropdown;
            }
            InventoryAction::ToggleBrowser if state.open => {
                state.browser_collapsed = !state.browser_collapsed;
            }
            InventoryAction::Page(direction) if state.open => {
                state.page = if *direction < 0 {
                    state.page.saturating_sub(1)
                } else {
                    state.page.saturating_add(1).min(
                        view.as_ref()
                            .map_or(1024, |view| view.browser_pages.saturating_sub(1)),
                    )
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
                            items[index].max_stack()
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

fn prop_kinds() -> [ResourcePropKind; ResourcePropKind::ALL.len()] {
    ResourcePropKind::ALL
}

fn pick_item(
    state: &mut InventoryState,
    selection: &mut InventorySelection,
    session: &ClientSession,
    item: ItemKind,
) {
    if state.blocks_input()
        || session.game_mode != GameMode::Creative
        || !item.valid()
        || !item.block_material().is_some_and(|material| {
            selection
                .palette
                .choices
                .iter()
                .any(|choice| choice.id == material)
        })
    {
        return;
    }
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
                count: 1,
            },
        );
    }
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
        ItemKind::WornTool { tool, remaining } => {
            format!("item-{}-uses-{remaining}", tool.slug())
        }
        ItemKind::FilledWateringCan { tool, units } => {
            format!("item-{}-water-{units}", tool.slug())
        }
        ItemKind::Stick => "item-stick".into(),
        ItemKind::Mineral(mineral) => format!("item-{}", mineral.slug()),
        ItemKind::Animal(product) => format!("item-{}", product.slug()),
        ItemKind::Bellows(kind) => format!("item-{}", kind.slug()),
        ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_)) => {
            format!("item-{}", item.identifier().trim_start_matches("teramic::"))
        }
        ItemKind::Dye(color) => format!("item-{}_dye", color.slug()),
        ItemKind::DyedBed {
            color: terra_core::beds::DyeColor::Rgb(_),
            ..
        }
        | ItemKind::TintedBlock { .. } => {
            format!("item-{}", item.identifier().trim_start_matches("teramic::"))
        }
        ItemKind::DyedBed { material, color } => format!("block-{material}-{}", color.slug()),
        ItemKind::Armor(_)
        | ItemKind::Accessory(_)
        | ItemKind::Cape(_)
        | ItemKind::HeraldicCape { .. }
        | ItemKind::Dust(_)
        | ItemKind::Crop(_)
        | ItemKind::Flower(_)
        | ItemKind::GlassBottle
        | ItemKind::WaterBottle
        | ItemKind::Potion(_)
        | ItemKind::Apple
        | ItemKind::AppleCore
        | ItemKind::BloodBerry
        | ItemKind::CrimsonMushroom
        | ItemKind::ThornTwig
        | ItemKind::BloodCrystal
        | ItemKind::DarkSap
        | ItemKind::BloodLeaf
        | ItemKind::BloodSpore
        | ItemKind::BloodSlimeGel
        | ItemKind::Bone
        | ItemKind::Bitumen
        | ItemKind::GloomEssence
        | ItemKind::DragonLoot(_)
        | ItemKind::LightSplitter
        | ItemKind::RoadAggregate => {
            format!("item-{}", item.identifier().trim_start_matches("teramic::"))
        }
    }
}
pub(crate) fn available_items(
    mode: GameMode,
    inventory: &terra_core::Inventory,
    palette: &BlockPalette,
) -> Vec<ItemKind> {
    if mode == GameMode::Creative {
        return palette
            .choices
            .iter()
            .filter(|choice| {
                !terra_core::castle::retired_item(choice.id)
                    && terra_core::construction::obtainable(choice.id)
                    && !matches!(choice.id, 197..=201 | 204..=205)
            })
            .map(|choice| ItemKind::Block(choice.id))
            .chain(
                prop_kinds()
                    .into_iter()
                    .filter(|kind| {
                        terra_core::farming::CropKind::from_prop(*kind).is_none()
                            && (*kind != ResourcePropKind::Seagrass
                                || !palette.choices.iter().any(|block| block.id == 50))
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

fn inspectable_item(session: &ClientSession, palette: &BlockPalette, id: &str) -> Option<ItemKind> {
    available_items(GameMode::Creative, &session.inventory, palette)
        .into_iter()
        .chain(
            session
                .inventory
                .slots
                .iter()
                .filter(|stack| stack.count > 0)
                .map(|stack| stack.item),
        )
        .chain(session.inventory.cursor.map(|stack| stack.item))
        .chain(
            session
                .container
                .iter()
                .flat_map(|container| container.halves.iter())
                .flat_map(|half| half.inventory.slots.iter())
                .filter(|stack| stack.count > 0)
                .map(|stack| stack.item),
        )
        .find(|item| item_id(*item) == id)
}

fn catalog_item(item: ItemKind) -> bool {
    // Keep legacy metadata and carried stacks available, but expose only the adaptive pipe.
    !matches!(item.block_material(), Some(61..=64 | 95))
        && item.block_material().is_none_or(|material| {
            !terra_core::textiles::is_dyed(material)
                && terra_core::construction::obtainable(material)
                && !terra_core::castle::retired_item(material)
                && !matches!(material, 197..=201 | 204..=205)
        })
        && !matches!(item, ItemKind::DyedBed { .. })
        && !matches!(item, ItemKind::TintedBlock { .. })
        && !matches!(item, ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_)))
}
