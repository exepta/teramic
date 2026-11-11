use super::*;
use serde::Serialize;
use terra_core::crafting::{
    CraftDestination, CraftGridCell, CraftStation, Recipe, RecipeBook, RecipePattern,
};
use tilt_ui::html_shared;
#[path = "crafting_pointer.rs"]
mod grid_pointer;

pub(super) fn drag_preview(
    state: &CraftingState,
    session: &ClientSession,
) -> Option<(ItemKind, [Option<CraftGridCell>; 9], u16)> {
    grid_pointer::preview(state, session)
}

#[derive(Message, Clone, Debug)]
pub enum CraftingAction {
    OpenBook,
    FurnaceTab(bool),
    SelectFuel(usize),
    PreviewWood(usize),
    Clear,
    Select(u64),
    Search(String),
    CloseRecipe,
    ApplyRecipe,
    Alternative(i32),
    Category(String),
    SelectAlternative(usize),
    PreviewAlternative { index: usize, wood: Option<usize> },
    Craft { maximum: bool, cursor: bool },
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum RecipeKind {
    #[default]
    Craft,
    Smelting,
    Crushing,
    Brewing,
    Campfire,
}

#[derive(Resource, Default)]
pub(super) struct CraftingState {
    cells: [Option<CraftGridCell>; 9],
    selected: Option<u64>,
    selected_kind: RecipeKind,
    recipe_item: Option<ItemKind>,
    pub(super) recipe_open: bool,
    search: String,
    furnace_fuels: bool,
    selected_fuel: Option<ItemKind>,
    preview_wood: usize,
    error: String,
    context: Option<Option<[i64; 3]>>,
    furnace_context: Option<[i64; 3]>,
    crusher_context: Option<[i64; 3]>,
    alchemy_context: Option<([i64; 3], bool)>,
    pending: Option<PendingGrid>,
    pointer: grid_pointer::GridPointer,
}
struct PendingGrid {
    response: u64,
    cells: [Option<CraftGridCell>; 9],
    retries: u8,
}

#[derive(Clone, Default, Serialize, PartialEq)]
pub struct RecipeRow {
    pub key: String,
    pub name: String,
    pub preview: String,
    pub selected: bool,
    pub available: bool,
    pub station: String,
}
#[derive(Clone, Default, Serialize, PartialEq)]
pub struct IngredientView {
    pub slots: Vec<view::InventoryItemView>,
    pub name: String,
    pub amount: String,
    pub enough: bool,
}
#[derive(Clone, Default, Serialize, PartialEq)]
pub struct RecipeCategoryView {
    #[serde(skip)]
    pub item: Option<ItemKind>,
    pub id: String,
    pub name: String,
    pub preview_key: String,
    pub selected: bool,
    pub count: String,
}
#[derive(Clone, Default, Serialize, PartialEq)]
pub struct RecipeAlternativeView {
    pub id: String,
    pub wood: Option<usize>,
    pub index: usize,
    pub name: String,
    pub selected: bool,
    pub number: String,
    pub has_grid: bool,
    pub grid: Vec<view::InventoryItemView>,
    pub output: Vec<view::InventoryItemView>,
    pub summary: String,
    pub input: Vec<view::InventoryItemView>,
    pub duration: String,
}
#[derive(Clone, Default, Serialize, PartialEq)]
pub struct FurnaceFuelView {
    pub index: usize,
    pub slots: Vec<view::InventoryItemView>,
    pub name: String,
    pub burn_time: String,
    pub yield_text: String,
    pub selected: bool,
}
#[derive(Clone, Default, Serialize, PartialEq)]
pub struct WoodOptionView {
    pub index: usize,
    pub name: String,
    pub slots: Vec<view::InventoryItemView>,
    pub selected: bool,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct CraftingView {
    pub wood_options: Vec<WoodOptionView>,
    pub any_wood: bool,
    pub cauldron_model: Vec<view::InventoryItemView>,
    pub cauldron_inputs: Vec<view::InventoryItemView>,
    pub cauldron_tools: Vec<view::InventoryItemView>,
    pub cauldron_progress: String,
    pub cauldron_heat: String,
    pub furnace_fuels: bool,
    pub furnace_model: Vec<view::InventoryItemView>,
    pub furnace_fuel: Vec<view::InventoryItemView>,
    pub furnace_fuel_options: Vec<FurnaceFuelView>,
    pub furnace_time: String,
    pub item_preview: String,
    pub item_category: String,
    pub source_summary: String,
    pub uses_summary: String,
    pub workbench: bool,
    pub grid: Vec<view::InventoryItemView>,
    pub stock: Vec<view::InventoryItemView>,
    pub station: String,
    pub search: String,
    pub recipes: Vec<RecipeRow>,
    pub selected: bool,
    pub name: String,
    pub ingredients: Vec<IngredientView>,
    pub output: Vec<view::InventoryItemView>,
    pub maximum: String,
    pub enabled: bool,
    pub can_max: bool,
    pub can_cursor: bool,
    pub message: String,
    pub empty: bool,
    pub recipe_open: bool,
    pub recipe_grid: Vec<view::InventoryItemView>,
    pub recipe_output: Vec<view::InventoryItemView>,
    pub description: String,
    pub recipe_station: String,
    pub missing: bool,
    pub recipe_previous: bool,
    pub recipe_next: bool,
    pub recipe_page: String,
    pub recipe_multiple: bool,
    pub can_apply_recipe: bool,
    pub smelting: bool,
    pub crushing: bool,
    pub mixing: bool,
    pub brewing: bool,
    pub campfire: bool,
    pub recipe_duration: String,
    pub recipe_heading: String,
    pub apply_title: String,
    pub recipe_categories: Vec<RecipeCategoryView>,
    pub recipe_alternatives: Vec<RecipeAlternativeView>,
    pub recipe_layout: String,
    pub recipe_shape: String,
    pub recipe_category: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<RecipeBook>()
        .init_resource::<CraftingState>()
        .init_resource::<CraftingView>()
        .add_message::<CraftingAction>()
        .add_systems(
            Update,
            (actions, update)
                .chain()
                .after(super::actions)
                .before(super::pointer::send_requests),
        );
}
fn station(inventory: &InventoryState) -> CraftStation {
    if inventory.station.is_some() {
        CraftStation::Workbench
    } else {
        CraftStation::Hand
    }
}
fn size(inventory: &InventoryState) -> usize {
    if inventory.station.is_some() { 3 } else { 2 }
}
fn fits(recipe: &Recipe, size: usize) -> bool {
    match &recipe.pattern {
        RecipePattern::Shaped { width, height, .. } => {
            usize::from(*width) <= size && usize::from(*height) <= size
        }
        RecipePattern::Shapeless { ingredients } => ingredients.len() <= size * size,
    }
}
fn pattern(recipe: &Recipe, size: usize) -> Vec<(usize, terra_core::crafting::Ingredient)> {
    match &recipe.pattern {
        RecipePattern::Shapeless { ingredients } => {
            ingredients.iter().copied().enumerate().collect()
        }
        RecipePattern::Shaped { width, cells, .. } => cells
            .iter()
            .enumerate()
            .filter_map(|(i, ingredient)| {
                ingredient.map(|v| (i % usize::from(*width) + i / usize::from(*width) * size, v))
            })
            .collect(),
    }
}
fn arrange(
    recipe: &Recipe,
    inventory: &terra_core::Inventory,
    size: usize,
    requested: Option<ItemKind>,
) -> Option<[Option<CraftGridCell>; 9]> {
    if !fits(recipe, size) {
        return None;
    }
    let mut cells = [None; 9];
    let mut used = [0_u16; terra_core::INVENTORY_SLOTS];
    let wood = requested.and_then(|item| match item {
        ItemKind::WoodenBlock { material, wood }
            if recipe.output.item == ItemKind::Block(material) =>
        {
            Some(wood)
        }
        _ => None,
    });
    for (cell, ingredient) in pattern(recipe, size) {
        let (slot, _) = inventory.slots.iter().enumerate().find(|(i, s)| {
            *i < used.len()
                && ingredient.matches(s.item)
                && (!ingredient.wood || wood.is_none_or(|wood| s.item.wood_species() == Some(wood)))
                && s.count.saturating_sub(used[*i]) >= ingredient.count
        })?;
        cells[cell] = Some(CraftGridCell {
            slot: slot as u8,
            count: ingredient.count,
        });
        used[slot] += ingredient.count;
    }
    Some(cells)
}
fn recipe_display_output(
    recipe: &Recipe,
    requested: Option<ItemKind>,
    actual: Option<terra_core::ItemStack>,
) -> terra_core::ItemStack {
    actual.unwrap_or_else(|| {
        let mut output = recipe.output;
        if let Some(item @ ItemKind::WoodenBlock { material, .. }) = requested
            && output.item == ItemKind::Block(material)
        {
            output.item = item;
        }
        output
    })
}
#[derive(Clone, Copy)]
enum RecipeChoice<'a> {
    Craft(&'a Recipe),
    Smelt(&'a terra_core::furnace::SmeltingRecipe),
    Crush(&'a terra_core::crusher::CrusherRecipe),
    Brew(&'a terra_core::brewing::BrewingRecipe),
    Campfire(&'a terra_core::furnace::SmeltingRecipe),
}
impl<'a> RecipeChoice<'a> {
    fn category(self) -> &'static str {
        match self {
            Self::Craft(recipe) if recipe.station == CraftStation::Workbench => "workbench",
            Self::Craft(_) => "crafting",
            Self::Smelt(_) => "smelting",
            Self::Crush(_) => "crushing",
            Self::Brew(_) => "brewing",
            Self::Campfire(_) => "campfire",
        }
    }
    fn name(self) -> &'a str {
        match self {
            Self::Craft(recipe) => &recipe.name,
            Self::Smelt(recipe) | Self::Campfire(recipe) => &recipe.name,
            Self::Crush(recipe) => &recipe.name,
            Self::Brew(recipe) => &recipe.name,
        }
    }
    fn selection(self) -> (u64, RecipeKind) {
        match self {
            Self::Craft(recipe) => (recipe.key(), RecipeKind::Craft),
            Self::Smelt(recipe) => (recipe.id, RecipeKind::Smelting),
            Self::Crush(recipe) => (recipe.id, RecipeKind::Crushing),
            Self::Brew(recipe) => (u64::from(recipe.id), RecipeKind::Brewing),
            Self::Campfire(recipe) => (recipe.id, RecipeKind::Campfire),
        }
    }
}
fn alternatives(book: &RecipeBook, item: Option<ItemKind>) -> Vec<RecipeChoice<'_>> {
    let item = item.map(|item| match item {
        ItemKind::HeraldicCape { cape, .. } => ItemKind::Cape(cape.undyed()),
        _ => item,
    });
    // RGB coloring and mixing are generated for this exact item. Base-material
    // recipes must not replace its color or appear as another station tab.
    if item.is_some_and(|item| terra_core::tints::canonical_item(item).is_some())
        || matches!(
            item,
            Some(ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_)))
        )
    {
        return Vec::new();
    }
    let mut recipes: Vec<_> = book
        .iter()
        .filter(|r| {
            item.is_some_and(|item| {
                r.output.item == item
                    || (item.block_material().is_some()
                        && r.output.item.block_material() == item.block_material())
            })
        })
        .collect();
    recipes.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    recipes
        .into_iter()
        .map(RecipeChoice::Craft)
        .chain(
            terra_core::furnace::recipes()
                .iter()
                .filter(|recipe| item == Some(recipe.output.item))
                .map(RecipeChoice::Smelt),
        )
        .chain(
            terra_core::crusher::recipes()
                .iter()
                .filter(|recipe| item == Some(recipe.output.item))
                .map(RecipeChoice::Crush),
        )
        .chain(
            terra_core::brewing::recipes()
                .iter()
                .filter(|recipe| item == Some(recipe.output().item))
                .map(RecipeChoice::Brew),
        )
        .chain(
            terra_core::campfire::recipes()
                .iter()
                .filter(|recipe| item == Some(recipe.output.item))
                .map(RecipeChoice::Campfire),
        )
        .collect()
}
fn select_recipe(
    state: &mut CraftingState,
    recipe: &Recipe,
    inventory: &InventoryState,
    bag: &terra_core::Inventory,
) {
    state.selected = Some(recipe.key());
    state.selected_kind = RecipeKind::Craft;
    state.recipe_open = true;
    state.error.clear();
    if recipe.station == CraftStation::Workbench && inventory.station.is_none() {
        state.error = "Requires a workbench.".into();
    } else if arrange(recipe, bag, size(inventory), state.recipe_item).is_none() {
        state.error = "Missing ingredients.".into();
    }
}

fn furnace_anchor(session: &ClientSession) -> Option<[i64; 3]> {
    session
        .container
        .as_ref()
        .filter(|container| container.furnace.is_some())
        .map(|container| container.anchor)
}

fn crusher_anchor(session: &ClientSession) -> Option<[i64; 3]> {
    session
        .container
        .as_ref()
        .filter(|container| container.crusher.is_some())
        .map(|container| container.anchor)
}

fn alchemy_anchor(session: &ClientSession) -> Option<([i64; 3], bool)> {
    session
        .container
        .as_ref()
        .filter(|container| container.cauldron.is_some() || container.campfire.is_some())
        .map(|container| (container.anchor, container.cauldron.is_some()))
}

fn machine_open(session: &ClientSession) -> bool {
    furnace_anchor(session).is_some()
        || crusher_anchor(session).is_some()
        || session.container.as_ref().is_some_and(|container| {
            container.mixer.is_some()
                || container.filter.is_some()
                || container.distributor.is_some()
                || container.cauldron.is_some()
                || container.campfire.is_some()
        })
}

fn storage_only(session: &ClientSession) -> bool {
    session.container.as_ref().is_some_and(|container| {
        container.workbench.is_none()
            && container.furnace.is_none()
            && container.crusher.is_none()
            && container.mixer.is_none()
            && container.filter.is_none()
            && container.distributor.is_none()
            && container.cauldron.is_none()
            && container.campfire.is_none()
    })
}

fn can_load_smelting(
    recipe: &terra_core::furnace::SmeltingRecipe,
    session: &ClientSession,
) -> bool {
    let Some(container) = session
        .container
        .as_ref()
        .filter(|container| container.furnace.is_some())
    else {
        return false;
    };
    session.inventory.count(recipe.input) > 0
        && container.halves.first().is_some_and(|half| {
            half.inventory
                .slot(0)
                .is_none_or(|stack| stack.item == recipe.input && stack.count < STACK_LIMIT)
        })
}

fn select_choice(
    state: &mut CraftingState,
    recipe: RecipeChoice<'_>,
    inventory: &InventoryState,
    session: &ClientSession,
) {
    match recipe {
        RecipeChoice::Craft(recipe) => {
            select_recipe(state, recipe, inventory, &session.inventory);
            if machine_open(session) {
                state.error = "Requires a crafting grid.".into();
            }
        }
        RecipeChoice::Smelt(recipe) => {
            state.selected = Some(recipe.id);
            state.selected_kind = RecipeKind::Smelting;
            state.recipe_open = true;
            state.error = if furnace_anchor(session).is_none() {
                "Requires a furnace."
            } else if session.inventory.count(recipe.input) == 0 {
                "Missing material."
            } else if !can_load_smelting(recipe, session) {
                "The furnace material slot is occupied."
            } else {
                ""
            }
            .into();
        }
        RecipeChoice::Crush(recipe) => {
            state.selected = Some(recipe.id);
            state.selected_kind = RecipeKind::Crushing;
            state.recipe_open = true;
            state.error = if crusher_anchor(session).is_none() {
                "Requires a crusher."
            } else if session.inventory.count(recipe.input) == 0 {
                "Missing material."
            } else if !can_load_crushing(recipe, session) {
                "The crusher material slot is occupied."
            } else {
                ""
            }
            .into();
        }
        RecipeChoice::Brew(recipe) => {
            state.selected = Some(u64::from(recipe.id));
            state.selected_kind = RecipeKind::Brewing;
            state.recipe_open = true;
            state.error = if session
                .container
                .as_ref()
                .is_some_and(|container| container.cauldron.is_some())
            {
                ""
            } else {
                "Requires a cauldron over a lit campfire."
            }
            .into();
        }
        RecipeChoice::Campfire(recipe) => {
            state.selected = Some(recipe.id);
            state.selected_kind = RecipeKind::Campfire;
            state.recipe_open = true;
            state.error = if session
                .container
                .as_ref()
                .is_some_and(|container| container.campfire.is_some())
            {
                ""
            } else {
                "Requires a campfire."
            }
            .into();
        }
    }
}

fn can_load_crushing(recipe: &terra_core::crusher::CrusherRecipe, session: &ClientSession) -> bool {
    session.inventory.count(recipe.input) > 0
        && session
            .container
            .as_ref()
            .filter(|container| container.crusher.is_some())
            .and_then(|container| container.halves.first())
            .is_some_and(|half| {
                half.inventory
                    .slot(terra_core::crusher::INPUT)
                    .is_none_or(|stack| {
                        stack.item == recipe.input && stack.count < recipe.input.max_stack()
                    })
            })
}

fn recipe_matches_search(
    language: &crate::localization::SearchText,
    query: &str,
    key: &str,
    name: &str,
    output: ItemKind,
    palette: &BlockPalette,
) -> bool {
    if key.to_lowercase().contains(query) || language.matches(query, name) {
        return true;
    }
    let (name, _, description) = view::item_metadata(output, palette);
    language.matches(query, &name) || language.matches(query, &description)
}

#[allow(clippy::too_many_arguments)]
fn actions(
    mut slots: MessageReader<InventoryAction>,
    mut events: MessageReader<CraftingAction>,
    mut state: ResMut<CraftingState>,
    inventory: Res<InventoryState>,
    book: Res<RecipeBook>,
    session: Option<Res<ClientSession>>,
    palette: Res<BlockPalette>,
    keys: Res<ButtonInput<KeyCode>>,
    mut queue: ResMut<pointer::InventoryQueue>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut wheel: MessageReader<MouseWheel>,
    language: crate::localization::SearchText,
) {
    let Some(session) =
        session.filter(|s| s.ready && !s.dead() && !s.teleporting && inventory.open)
    else {
        if state.context.is_some() {
            *state = default();
        }
        events.clear();
        slots.clear();
        return;
    };
    let book = book.with_cape_heraldry(
        session
            .uuid
            .and_then(|owner| session.kingdoms.heraldry_for_member(owner)),
    );
    if inventory.container_open_response.is_some() {
        if state.context.is_some() {
            *state = default();
        }
        slots.clear();
        events.clear();
        wheel.clear();
        return;
    }
    if state.context != Some(inventory.station)
        || state.furnace_context != furnace_anchor(&session)
        || state.crusher_context != crusher_anchor(&session)
        || state.alchemy_context != alchemy_anchor(&session)
    {
        *state = CraftingState {
            context: Some(inventory.station),
            furnace_context: furnace_anchor(&session),
            crusher_context: crusher_anchor(&session),
            alchemy_context: alchemy_anchor(&session),
            ..default()
        };
    }
    if state
        .pending
        .as_ref()
        .is_some_and(|p| p.response != session.inventory_response)
    {
        let retry = session.last_inventory
            == Some(terra_network::protocol::inventory::InventoryOutcome::Conflict)
            && state
                .pending
                .as_ref()
                .is_some_and(|pending| pending.retries < 2);
        if retry {
            if let Some(pending) = &mut state.pending {
                pending.response = session.inventory_response;
                pending.retries += 1;
            }
        } else if let Some(pending) = state.pending.take()
            && session.last_inventory
                == Some(terra_network::protocol::inventory::InventoryOutcome::Applied)
        {
            state.cells = pending.cells;
            state.error.clear();
        }
    }
    // Reservations reference authoritative slots, never duplicated item storage.
    let mut used = [0_u16; terra_core::INVENTORY_SLOTS];
    let mut validated = state.cells;
    for cell in &mut validated {
        if let Some(reserved) = cell {
            let source = usize::from(reserved.slot);
            let available = session
                .inventory
                .slot(source)
                .map_or(0, |s| s.count)
                .saturating_sub(used.get(source).copied().unwrap_or(0));
            reserved.count = reserved.count.min(available);
            if source >= used.len() || reserved.count == 0 {
                *cell = None;
            } else {
                used[source] += reserved.count;
            }
        }
    }
    if state.cells != validated {
        state.cells = validated;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mut craft = None;
    for event in slots.read() {
        if !storage_only(&session)
            && !machine_open(&session)
            && grid_pointer::handle(
                &mut state,
                &inventory,
                &session,
                event,
                shift,
                time.elapsed_secs_f64(),
            )
        {
            continue;
        }
        let inspected = match event {
            InventoryAction::Recipe(item) => Some(item),
            InventoryAction::Pointer { command, item, .. }
                if command == "catalog"
                    && (session.game_mode == GameMode::Survival || state.recipe_open)
                    && !session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.filter.is_some()) =>
            {
                Some(item)
            }
            _ => None,
        };
        if let Some(item) = inspected {
            let item = inspectable_item(&session, &palette, item);
            let generated = item
                .is_some_and(|item| terra_core::tints::canonical_item(item).is_some())
                || matches!(
                    item,
                    Some(ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_)))
                );
            let recipes = alternatives(&book, item);
            // A recipe request is a no-op when the item has no registered or
            // generated process. Keep the previous popup and selection intact.
            if !generated && recipes.is_empty() {
                continue;
            }
            if state.pending.is_some() {
                state.error = "Waiting for the inventory update.".into();
                continue;
            }
            state.recipe_item = item;
            if generated {
                state.selected = None;
                state.selected_kind = RecipeKind::Craft;
                state.recipe_open = true;
                state.error.clear();
                continue;
            }
            let recipe = recipes
                .iter()
                .find(|r| match r {
                    RecipeChoice::Craft(r) => {
                        !storage_only(&session)
                            && !machine_open(&session)
                            && (r.station == CraftStation::Hand || inventory.station.is_some())
                            && arrange(r, &session.inventory, size(&inventory), item).is_some()
                    }
                    RecipeChoice::Smelt(r) => can_load_smelting(r, &session),
                    RecipeChoice::Crush(r) => can_load_crushing(r, &session),
                    RecipeChoice::Brew(_) => session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.cauldron.is_some()),
                    RecipeChoice::Campfire(_) => session
                        .container
                        .as_ref()
                        .is_some_and(|container| container.campfire.is_some()),
                })
                .copied()
                .or_else(|| recipes.first().copied());
            if let Some(recipe) = recipe {
                select_choice(&mut state, recipe, &inventory, &session);
            }
            continue;
        }
        match event {
            InventoryAction::CraftCell(index) => edit_cell(
                &mut state,
                &inventory,
                &session,
                &mut queue,
                usize::from(*index),
                false,
            ),
            InventoryAction::Pointer { command, .. }
                if matches!(command.as_str(), "bench-output" | "craft") =>
            {
                craft = Some((shift, !shift));
            }
            InventoryAction::Craft => craft = Some((false, false)),
            InventoryAction::CraftHeld => craft = Some((false, true)),
            _ => {}
        }
    }
    for event in events.read() {
        if state.pending.is_some() && !matches!(event, CraftingAction::CloseRecipe) {
            state.error = "Waiting for the inventory update.".into();
            continue;
        }
        match event {
            CraftingAction::OpenBook => {
                if let Some(recipe) = book
                    .iter()
                    .find(|recipe| recipe.output.item == ItemKind::Block(28))
                    .or_else(|| book.iter().next())
                {
                    state.recipe_item = Some(recipe.output.item);
                    state.selected = Some(recipe.key());
                    state.selected_kind = RecipeKind::Craft;
                    state.recipe_open = true;
                    state.error.clear();
                }
            }
            CraftingAction::PreviewWood(index) => {
                if *index < WOOD_PREVIEWS.len() {
                    state.preview_wood = *index;
                }
            }
            CraftingAction::FurnaceTab(fuels) => state.furnace_fuels = *fuels,
            CraftingAction::SelectFuel(index) => {
                if let Some(fuel) = terra_core::furnace::fuels().get(*index) {
                    state.selected_fuel = Some(fuel.item);
                }
            }
            CraftingAction::Clear => {
                state.cells = [None; 9];
                state.pending = None;
            }
            CraftingAction::Select(key) => {
                if let Some(recipe) = book.get(*key) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Craft(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) =
                    terra_core::furnace::recipes().iter().find(|r| r.id == *key)
                {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Smelt(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) =
                    terra_core::crusher::recipes().iter().find(|r| r.id == *key)
                {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Crush(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) = terra_core::brewing::recipes()
                    .iter()
                    .find(|r| u64::from(r.id) == *key)
                {
                    state.recipe_item = Some(recipe.output().item);
                    select_choice(&mut state, RecipeChoice::Brew(recipe), &inventory, &session);
                }
            }
            CraftingAction::Search(search) => {
                state.search = search.chars().take(96).collect();
                let query = state.search.to_lowercase();
                if let Some(recipe) = book.iter().find(|r| {
                    recipe_matches_search(
                        &language,
                        &query,
                        &r.id,
                        &r.name,
                        r.output.item,
                        &palette,
                    )
                }) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Craft(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) = terra_core::furnace::recipes().iter().find(|r| {
                    recipe_matches_search(
                        &language,
                        &query,
                        &r.key,
                        &r.name,
                        r.output.item,
                        &palette,
                    )
                }) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Smelt(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) = terra_core::crusher::recipes().iter().find(|r| {
                    recipe_matches_search(
                        &language,
                        &query,
                        &r.key,
                        &r.name,
                        r.output.item,
                        &palette,
                    )
                }) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Crush(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) = terra_core::brewing::recipes().iter().find(|r| {
                    recipe_matches_search(
                        &language,
                        &query,
                        &r.key,
                        &r.name,
                        r.output().item,
                        &palette,
                    )
                }) {
                    state.recipe_item = Some(recipe.output().item);
                    select_choice(&mut state, RecipeChoice::Brew(recipe), &inventory, &session);
                }
            }
            CraftingAction::CloseRecipe => state.recipe_open = false,
            CraftingAction::ApplyRecipe => {
                if session.inventory.cursor.is_some() || !queue.is_empty() {
                    state.error = "Empty the cursor before adding a recipe.".into();
                } else if state.selected.is_none()
                    && let Some(ItemKind::Dye(terra_core::beds::DyeColor::Rgb(target))) =
                        state.recipe_item
                {
                    if let Some(container) = session
                        .container
                        .as_ref()
                        .filter(|container| container.mixer.is_some())
                    {
                        queue.push(ServerAction::SetMixerColor {
                            station: container.anchor,
                            target,
                        });
                        state.recipe_open = false;
                        state.error.clear();
                    } else {
                        state.error = "Requires a mixer.".into();
                    }
                } else if state.selected_kind == RecipeKind::Brewing {
                    if let Some(recipe) = state
                        .selected
                        .and_then(|id| u16::try_from(id).ok())
                        .and_then(terra_core::brewing::recipe)
                        && let Some(container) = session
                            .container
                            .as_ref()
                            .filter(|container| container.cauldron.is_some())
                    {
                        queue.push(ServerAction::SetBrewingRecipe {
                            station: container.anchor,
                            recipe: recipe.id,
                        });
                        state.recipe_open = false;
                        state.error.clear();
                    } else {
                        state.error = "Requires a cauldron.".into();
                    }
                } else if state.selected_kind == RecipeKind::Campfire {
                    state.error = "Place the food in the campfire material slot.".into();
                } else if state.selected_kind == RecipeKind::Crushing {
                    if let Some(recipe) = terra_core::crusher::recipes()
                        .iter()
                        .find(|r| Some(r.id) == state.selected)
                    {
                        if let Some(station) = crusher_anchor(&session)
                            && can_load_crushing(recipe, &session)
                        {
                            queue.push(ServerAction::LoadCrushingRecipe {
                                recipe: recipe.id,
                                station,
                            });
                            state.recipe_open = false;
                            state.error.clear();
                        } else {
                            select_choice(
                                &mut state,
                                RecipeChoice::Crush(recipe),
                                &inventory,
                                &session,
                            );
                        }
                    }
                } else if state.selected_kind == RecipeKind::Smelting {
                    if let Some(recipe) = terra_core::furnace::recipes()
                        .iter()
                        .find(|r| Some(r.id) == state.selected)
                    {
                        if let Some(station) = furnace_anchor(&session)
                            && can_load_smelting(recipe, &session)
                        {
                            queue.push(ServerAction::LoadSmeltingRecipe {
                                recipe: recipe.id,
                                station,
                            });
                            state.recipe_open = false;
                            state.error.clear();
                        } else {
                            select_choice(
                                &mut state,
                                RecipeChoice::Smelt(recipe),
                                &inventory,
                                &session,
                            );
                        }
                    }
                } else if storage_only(&session) || machine_open(&session) {
                    state.error = "Requires a crafting grid.".into();
                } else if let Some(recipe) = state.selected.and_then(|key| book.get(key)) {
                    if recipe.station == CraftStation::Workbench && inventory.station.is_none() {
                        state.error = "Requires a workbench.".into();
                    } else if let Some(cells) = arrange(
                        recipe,
                        &session.inventory,
                        size(&inventory),
                        state.recipe_item,
                    ) {
                        state.cells = cells;
                        state.recipe_open = false;
                        state.error.clear();
                        state.pointer = default();
                    } else {
                        state.error = "Missing ingredients.".into();
                    }
                }
            }
            CraftingAction::Alternative(direction) => {
                let recipes = alternatives(&book, state.recipe_item);
                if let Some(selected) = recipes.iter().find(|r| {
                    Some(r.selection()) == state.selected.map(|key| (key, state.selected_kind))
                }) {
                    let category = selected.category();
                    let choices: Vec<_> = recipes
                        .iter()
                        .copied()
                        .filter(|r| r.category() == category)
                        .flat_map(|choice| {
                            alternative_woods(choice)
                                .into_iter()
                                .map(move |wood| (choice, wood))
                        })
                        .collect();
                    let current = choices
                        .iter()
                        .position(|(r, wood)| {
                            r.selection() == selected.selection()
                                && wood.is_none_or(|wood| wood == state.preview_wood)
                        })
                        .unwrap();
                    let next = (current as i32 + direction.signum())
                        .clamp(0, choices.len() as i32 - 1) as usize;
                    let (choice, wood) = choices[next];
                    if let Some(wood) = wood {
                        state.preview_wood = wood;
                    }
                    select_choice(&mut state, choice, &inventory, &session);
                }
            }
            CraftingAction::Category(category) => {
                if let Some(choice) = alternatives(&book, state.recipe_item)
                    .into_iter()
                    .find(|r| r.category() == category)
                {
                    select_choice(&mut state, choice, &inventory, &session);
                }
            }
            CraftingAction::SelectAlternative(index)
            | CraftingAction::PreviewAlternative { index, .. } => {
                if let Some(choice) = alternatives(&book, state.recipe_item).get(*index).copied() {
                    if let CraftingAction::PreviewAlternative {
                        wood: Some(wood), ..
                    } = event
                    {
                        if alternative_woods(choice).contains(&Some(*wood)) {
                            state.preview_wood = *wood;
                        } else {
                            continue;
                        }
                    }
                    select_choice(&mut state, choice, &inventory, &session);
                }
            }
            CraftingAction::Craft { maximum, cursor } => craft = Some((*maximum, *cursor)),
        }
    }
    let before_cells = state.cells;
    let before_pending = state.pending.is_some();
    // Pointer bookkeeping alone must not recompute every recipe each idle frame.
    grid_pointer::finish(
        state.bypass_change_detection(),
        &session,
        &mut queue,
        &mouse,
        &mut wheel,
        keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
    );
    if state.cells != before_cells || state.pending.is_some() != before_pending {
        state.set_changed();
    }
    if let Some((maximum, cursor)) = craft.filter(|_| {
        state.pending.is_none()
            && queue.is_empty()
            && !storage_only(&session)
            && !machine_open(&session)
    }) {
        let destination = if cursor {
            CraftDestination::Cursor
        } else {
            CraftDestination::Inventory
        };
        let grid_size = size(&inventory);
        let active = &state.cells[..grid_size * grid_size];
        let count = if maximum {
            maximum_count(
                &book,
                &session.inventory,
                active,
                grid_size,
                station(&inventory),
                destination,
            )
        } else {
            1
        };
        if book.craft_reserved_grid(
            &mut session.inventory.clone(),
            active,
            grid_size,
            station(&inventory),
            count,
            destination,
        ) {
            let mut remaining = state.cells;
            if let Some(consumption) = book.grid_consumption_reserved(
                &session.inventory,
                active,
                grid_size,
                station(&inventory),
            ) {
                for (index, amount) in consumption {
                    if let Some(reserved) = &mut remaining[index] {
                        reserved.count =
                            reserved.count.saturating_sub(amount.saturating_mul(count));
                        if reserved.count == 0 {
                            remaining[index] = None;
                        }
                    }
                }
            }
            queue.push(ServerAction::CraftReserved {
                cells: state.cells,
                size: grid_size as u8,
                station: inventory.station,
                count,
                destination,
            });
            state.pending = Some(PendingGrid {
                response: session.inventory_response,
                cells: remaining,
                retries: 0,
            });
            state.error.clear();
        } else {
            state.error = "Invalid pattern, missing ingredients or no room for the result.".into();
        }
    }
}

fn edit_cell(
    state: &mut CraftingState,
    inventory: &InventoryState,
    session: &ClientSession,
    queue: &mut pointer::InventoryQueue,
    index: usize,
    secondary: bool,
) {
    if index >= size(inventory).pow(2) || state.pending.is_some() || !queue.is_empty() {
        return;
    }
    if let Some(cursor) = session.inventory.cursor {
        let count = if secondary { 1 } else { cursor.count };
        let source = state.cells[index]
            .and_then(|c| {
                session
                    .inventory
                    .slot(usize::from(c.slot))
                    .filter(|s| {
                        s.item == cursor.item && s.count.saturating_add(count) <= STACK_LIMIT
                    })
                    .map(|_| usize::from(c.slot))
            })
            .or_else(|| {
                (0..terra_core::INVENTORY_SLOTS)
                    .find(|slot| session.inventory.slot(*slot).is_none())
            });
        if let Some(source) = source {
            let mut cells = state.cells;
            let previous = cells[index]
                .filter(|c| usize::from(c.slot) == source)
                .map_or(0, |c| c.count);
            cells[index] = Some(CraftGridCell {
                slot: source as u8,
                count: previous + count,
            });
            queue.push(ServerAction::Click {
                slot: source as u8,
                secondary,
            });
            state.pending = Some(PendingGrid {
                response: session.inventory_response,
                cells,
                retries: 0,
            });
        } else {
            state.error = "Free an inventory slot before placing this stack.".into();
        }
    } else if let Some(mut cell) = state.cells[index] {
        let count = if secondary {
            cell.count.div_ceil(2)
        } else {
            cell.count
        };
        queue.push(ServerAction::TakeToCursor {
            slot: cell.slot,
            count,
        });
        cell.count -= count;
        let mut cells = state.cells;
        cells[index] = (cell.count > 0).then_some(cell);
        state.pending = Some(PendingGrid {
            response: session.inventory_response,
            cells,
            retries: 0,
        });
    } else if let Some(source) = inventory.source.and_then(|s| {
        session
            .inventory
            .slot(usize::from(s))
            .map(|stack| (s, stack))
    }) {
        let used: u16 = state
            .cells
            .iter()
            .flatten()
            .filter(|c| c.slot == source.0)
            .map(|c| c.count)
            .sum();
        if source.1.count > used {
            state.cells[index] = Some(CraftGridCell {
                slot: source.0,
                count: 1,
            });
        }
    }
}
fn maximum_count(
    book: &RecipeBook,
    inventory: &terra_core::Inventory,
    cells: &[Option<CraftGridCell>],
    size: usize,
    station: CraftStation,
    destination: CraftDestination,
) -> u16 {
    (1..=128)
        .rev()
        .find(|count| {
            book.craft_reserved_grid(
                &mut inventory.clone(),
                cells,
                size,
                station,
                *count,
                destination,
            )
        })
        .unwrap_or(0)
}
pub(super) fn available_count(state: &CraftingState, slot: usize, count: u16) -> u16 {
    count.saturating_sub(
        state
            .cells
            .iter()
            .flatten()
            .filter(|c| usize::from(c.slot) == slot)
            .map(|c| c.count)
            .sum(),
    )
}

pub(super) fn reserved_counts(state: &CraftingState) -> [u16; terra_core::INVENTORY_SLOTS] {
    let mut counts = [0_u16; terra_core::INVENTORY_SLOTS];
    for cell in state.cells.iter().flatten() {
        if let Some(count) = counts.get_mut(usize::from(cell.slot)) {
            *count = count.saturating_add(cell.count);
        }
    }
    counts
}

pub(super) fn busy(state: &CraftingState) -> bool {
    state.pending.is_some()
}

type RecipeInventoryStamp = (
    u64,
    u64,
    u64,
    Option<([i64; 3], u64)>,
    Option<terra_core::brewing::CauldronView>,
);

#[allow(clippy::too_many_arguments)]
fn update(
    mut inventory: ResMut<InventoryState>,
    mut state: ResMut<CraftingState>,
    book: Res<RecipeBook>,
    palette: Res<BlockPalette>,
    session: Option<Res<ClientSession>>,
    chunks: Option<Res<terra_world::streaming::ChunkStore>>,
    inventory_view: Option<Res<InventoryView>>,
    mut view: ResMut<CraftingView>,
    mut stamp: Local<Option<RecipeInventoryStamp>>,
    config: Option<Res<terra_core::GameConfig>>,
) {
    let prewarming = inventory_view
        .as_ref()
        .is_some_and(|view| view.visible && view.prewarming);
    let Some(session) = session.filter(|s| s.ready && (inventory.open || prewarming)) else {
        if *view != CraftingView::default() {
            *view = default();
        }
        *stamp = None;
        return;
    };
    if let Some(position) = inventory.station {
        let loaded = chunks.as_ref().is_some_and(|c| {
            c.block_at(position) == Some(terra_voxel::MaterialId::Workbench as u16)
        });
        let near = session.confirmed_state().is_some_and(|s| {
            Vec3::from_array(position.map(|p| p as f32 + 0.5))
                .distance(Vec3::from_array(s.position))
                < 7.0
        });
        if !loaded || !near {
            inventory.station = None;
            state.cells = [None; 9];
            state.error = "Workbench no longer available.".into();
        }
    }
    let next_stamp = Some((
        session.inventory.revision,
        session.inventory_response,
        session.kingdoms.revision,
        session.container.as_ref().and_then(|container| {
            container
                .halves
                .first()
                .map(|half| (container.anchor, half.inventory.revision))
        }),
        session
            .container
            .as_ref()
            .and_then(|container| container.cauldron.clone()),
    ));
    if *stamp == next_stamp
        && !state.is_changed()
        && !book.is_changed()
        && !palette.is_changed()
        && !inventory.is_changed()
        && !config.as_ref().is_some_and(|config| config.is_changed())
    {
        return;
    }
    *stamp = next_stamp;
    let book = book.with_cape_heraldry(
        session
            .uuid
            .and_then(|owner| session.kingdoms.heraldry_for_member(owner)),
    );
    let grid_size = size(&inventory);
    let station = station(&inventory);
    let active = &state.cells[..grid_size * grid_size];
    let drag_preview = drag_preview(&state, &session);
    let displayed_cells = drag_preview
        .as_ref()
        .map_or(active, |(_, cells, _)| &cells[..grid_size * grid_size]);
    let result = book.output_for_reserved_grid(&session.inventory, active, grid_size, station);
    let maximum = maximum_count(
        &book,
        &session.inventory,
        active,
        grid_size,
        station,
        CraftDestination::Inventory,
    );
    let recipe = state
        .selected
        .filter(|_| state.selected_kind == RecipeKind::Craft)
        .and_then(|key| book.get(key));
    let grid = displayed_cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let mut slot = cell
                .and_then(|c| {
                    session
                        .inventory
                        .slot(usize::from(c.slot))
                        .map(|s| s.item)
                        .or_else(|| drag_preview.as_ref().map(|(item, _, _)| *item))
                        .map(|item| view::item_view(item, c.count, &palette))
                })
                .unwrap_or_else(|| view::InventoryItemView {
                    empty: true,
                    ..default()
                });
            slot.id = format!("bench-{index}");
            slot.index = index.to_string();
            slot.command = if grid_size == 3 {
                "bench-cell"
            } else {
                "craft-cell"
            }
            .into();
            slot
        })
        .collect();
    let output = vec![
        result
            .map(|s| {
                let mut v = view::item_view(s.item, s.count, &palette);
                v.command = if grid_size == 3 {
                    "bench-output"
                } else {
                    "craft"
                }
                .into();
                v.id = "result".into();
                v
            })
            .unwrap_or_else(|| view::InventoryItemView {
                id: "result".into(),
                command: "bench-output".into(),
                empty: true,
                ..default()
            }),
    ];
    let mut missing = false;
    let mut recipe_grid: Vec<_> = (0..9)
        .map(|i| view::InventoryItemView {
            id: format!("preview-{i}"),
            empty: true,
            command: "recipe-preview".into(),
            ..default()
        })
        .collect();
    if let Some(recipe) = recipe {
        let mut used = [0_u16; terra_core::INVENTORY_SLOTS];
        for (index, ingredient) in pattern(recipe, 3) {
            let source = session.inventory.slots.iter().enumerate().find(|(i, s)| {
                *i < used.len()
                    && ingredient.matches(s.item)
                    && s.count.saturating_sub(used[*i]) >= ingredient.count
            });
            let mut slot = view::item_view(
                if ingredient.wood {
                    wood_preview(&state)
                } else {
                    source.map_or(ingredient.item, |(_, s)| s.item)
                },
                ingredient.count,
                &palette,
            );
            describe_ingredient(&mut slot, ingredient);
            slot.id = format!("preview-{index}");
            slot.command = "recipe-preview".into();
            slot.missing = source.is_none();
            missing |= slot.missing;
            if let Some((i, _)) = source {
                used[i] += ingredient.count;
            }
            recipe_grid[index] = slot;
        }
    }
    let recipes = alternatives(&book, state.recipe_item);
    let page = recipes
        .iter()
        .position(|r| Some(r.selection()) == state.selected.map(|key| (key, state.selected_kind)))
        .unwrap_or(0);
    let mut next = CraftingView {
        workbench: grid_size == 3,
        grid,
        output,
        station: if grid_size == 3 {
            "Workbench"
        } else {
            "Crafting"
        }
        .into(),
        selected: recipe.is_some(),
        name: recipe.map_or("Recipe", |r| r.name.as_str()).into(),
        maximum: maximum.to_string(),
        enabled: result.is_some(),
        can_max: maximum > 0,
        ingredients: recipe
            .map(|r| {
                r.ingredients()
                    .into_iter()
                    .map(|ingredient| {
                        let count = ingredient.available_count(&session.inventory);
                        let slot = view::item_view(ingredient.item, ingredient.count, &palette);
                        IngredientView {
                            name: if ingredient.wood {
                                "Wood planks".into()
                            } else {
                                slot.name.clone()
                            },
                            slots: vec![slot],
                            amount: format!("{count} / {}", ingredient.count),
                            enough: count >= ingredient.count,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        can_cursor: book.craft_reserved_grid(
            &mut session.inventory.clone(),
            active,
            grid_size,
            station,
            1,
            CraftDestination::Cursor,
        ),
        message: book.error.clone().unwrap_or_else(|| state.error.clone()),
        recipe_open: state.recipe_open,
        recipe_heading: "Crafting Recipe".into(),
        apply_title: "Add recipe to crafting grid".into(),
        recipe_grid,
        missing,
        recipe_output: recipe
            .map(|r| {
                let resolved = recipe_display_output(
                    r,
                    state.recipe_item,
                    result.filter(|_| {
                        book.grid_match_reserved(&session.inventory, active, grid_size, station)
                            .is_some_and(|(key, _)| key == r.key())
                    }),
                );
                let mut slot = view::item_view(resolved.item, resolved.count, &palette);
                slot.id = "recipe-result".into();
                slot.command = "recipe-preview".into();
                vec![slot]
            })
            .unwrap_or_default(),
        description: recipe
            .map(|r| view::item_view(r.output.item, 1, &palette).description)
            .unwrap_or_default(),
        recipe_station: recipe
            .map_or("", |r| {
                if r.station == CraftStation::Workbench {
                    "Workbench / 3 x 3"
                } else {
                    "Player / 2 x 2"
                }
            })
            .into(),
        recipe_previous: recipes.len() > 1,
        recipe_next: recipes.len() > 1,
        recipe_multiple: recipes.len() > 1,
        can_apply_recipe: recipe.is_some_and(|r| {
            !storage_only(&session)
                && !machine_open(&session)
                && (r.station == CraftStation::Hand || inventory.station.is_some())
                && arrange(r, &session.inventory, grid_size, state.recipe_item).is_some()
        }) && state.pending.is_none()
            && session.inventory.cursor.is_none(),
        recipe_page: if recipes.is_empty() {
            String::new()
        } else {
            format!("{} / {}", page + 1, recipes.len())
        },
        empty: recipe.is_none(),
        ..default()
    };
    if state.selected_kind == RecipeKind::Smelting
        && let Some(recipe) = terra_core::furnace::recipes()
            .iter()
            .find(|r| Some(r.id) == state.selected)
    {
        let count = session.inventory.count(recipe.input);
        let mut input = view::item_view(recipe.input, 1, &palette);
        input.command = "recipe-preview".into();
        input.id = "smelting-input".into();
        input.missing = count == 0;
        let mut output = view::item_view(recipe.output.item, recipe.output.count, &palette);
        output.command = "recipe-preview".into();
        output.id = "smelting-result".into();
        next.smelting = true;
        next.selected = true;
        next.empty = false;
        next.name = recipe.name.clone();
        next.description = format!("Turns {} into {} in a furnace.", input.name, output.name);
        next.recipe_station = "Furnace".into();
        next.recipe_heading = "Furnace Recipe".into();
        next.furnace_fuels = state.furnace_fuels;
        next.furnace_model = vec![view::item_view(ItemKind::Block(59), 1, &palette)];
        next.furnace_time = config.as_ref().map_or_else(
            || format!("{:.1} s", recipe.seconds),
            |config| {
                format!(
                    "{:.1} s ({} ticks)",
                    recipe.seconds,
                    (recipe.seconds * f32::from(config.server.tick_rate)).round() as u32
                )
            },
        );
        let fuel = state
            .selected_fuel
            .and_then(|item| {
                terra_core::furnace::fuels()
                    .iter()
                    .find(|fuel| fuel.item == item)
            })
            .or_else(|| {
                terra_core::furnace::fuels()
                    .iter()
                    .find(|fuel| fuel.item == ItemKind::Mineral(terra_core::MineralItem::Coal))
            })
            .or_else(|| terra_core::furnace::fuels().first());
        next.furnace_fuel = fuel
            .map(|fuel| vec![view::item_view(fuel.item, 1, &palette)])
            .unwrap_or_default();
        next.furnace_fuel_options = terra_core::furnace::fuels()
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                let slot = view::item_view(candidate.item, 1, &palette);
                FurnaceFuelView {
                    index,
                    name: slot.name.clone(),
                    slots: vec![slot],
                    burn_time: format!("{:.1} s", candidate.seconds),
                    yield_text: format!(
                        "{} operations / fuel",
                        (candidate.seconds / recipe.seconds).floor() as u32
                    ),
                    selected: fuel.is_some_and(|fuel| fuel.item == candidate.item),
                }
            })
            .collect();
        next.recipe_duration = format!("{} s", recipe.seconds);
        next.apply_title = "Add material to furnace".into();
        next.missing = input.missing;
        next.ingredients = vec![IngredientView {
            slots: vec![input.clone()],
            name: input.name.clone(),
            amount: format!("{count} / 1"),
            enough: count > 0,
        }];
        next.recipe_grid = vec![input];
        next.recipe_output = vec![output];
        next.can_apply_recipe =
            can_load_smelting(recipe, &session) && session.inventory.cursor.is_none();
    }
    if state.selected_kind == RecipeKind::Crushing
        && let Some(recipe) = terra_core::crusher::recipes()
            .iter()
            .find(|r| Some(r.id) == state.selected)
    {
        let count = session.inventory.count(recipe.input);
        let mut input = view::item_view(recipe.input, 1, &palette);
        input.command = "recipe-preview".into();
        input.id = "crushing-input".into();
        input.missing = count == 0;
        let mut output = view::item_view(recipe.output.item, recipe.output.count, &palette);
        output.command = "recipe-preview".into();
        output.id = "crushing-result".into();
        next.crushing = true;
        next.selected = true;
        next.empty = false;
        next.name = recipe.name.clone();
        next.description = output.description.clone();
        next.recipe_station = "Crusher".into();
        next.recipe_heading = "Crushing Recipe".into();
        next.recipe_duration = format!("{} s", recipe.seconds);
        next.apply_title = "Add material to crusher".into();
        next.missing = input.missing;
        next.ingredients = vec![IngredientView {
            slots: vec![input.clone()],
            name: input.name.clone(),
            amount: format!("{count} / 1"),
            enough: count > 0,
        }];
        next.recipe_grid = vec![input];
        next.recipe_output = vec![output];
        next.can_apply_recipe =
            can_load_crushing(recipe, &session) && session.inventory.cursor.is_none();
    }
    if state.selected_kind == RecipeKind::Brewing
        && let Some(recipe) = state
            .selected
            .and_then(|id| u16::try_from(id).ok())
            .and_then(terra_core::brewing::recipe)
    {
        next.brewing = true;
        next.selected = true;
        next.empty = false;
        next.name = recipe.name.clone();
        next.description = alchemy::effects(recipe);
        next.recipe_station = "Cauldron / lit campfire".into();
        next.recipe_heading = "Cauldron Recipe".into();
        next.cauldron_model = vec![view::item_view(
            ItemKind::Block(terra_core::brewing::MATERIAL),
            1,
            &palette,
        )];
        let cauldron = session
            .container
            .as_ref()
            .and_then(|container| container.cauldron.as_ref())
            .filter(|cauldron| cauldron.recipe == Some(recipe.id));
        next.cauldron_progress = format!(
            "width: {}%;",
            cauldron.map_or(0.0, |cauldron| cauldron.progress.clamp(0.0, 1.0) * 100.0)
        );
        next.cauldron_heat = if cauldron.is_some_and(|cauldron| cauldron.heated) {
            "Heated — stir to brew"
        } else {
            "Requires heat and stirring"
        }
        .into();
        next.recipe_duration = format!("{} s", recipe.seconds);
        next.apply_title = "Select cauldron recipe".into();
        let requirements = recipe.inputs.iter().copied().chain([
            terra_core::ItemStack {
                item: ItemKind::WaterBottle,
                count: 1,
            },
            terra_core::ItemStack {
                item: ItemKind::Tool(terra_core::ToolKind::WoodSpoon),
                count: 1,
            },
        ]);
        next.recipe_grid.clear();
        next.ingredients.clear();
        next.missing = false;
        for (index, required) in requirements.enumerate() {
            let mut slot = view::item_view(required.item, required.count, &palette);
            slot.id = format!("brewing-input-{index}");
            slot.command = "recipe-preview".into();
            let available = if terra_core::brewing::spoon_speed(required.item).is_some() {
                session
                    .inventory
                    .slots
                    .iter()
                    .filter(|stack| terra_core::brewing::spoon_speed(stack.item).is_some())
                    .map(|stack| stack.count)
                    .sum::<u16>()
            } else {
                session.inventory.count(required.item)
            };
            slot.missing = available < required.count;
            next.missing |= slot.missing;
            next.ingredients.push(IngredientView {
                slots: vec![slot.clone()],
                name: if terra_core::brewing::spoon_speed(required.item).is_some() {
                    "Stirring spoon (reusable)".into()
                } else {
                    slot.name.clone()
                },
                amount: format!("{available} / {}", required.count),
                enough: !slot.missing,
            });
            next.recipe_grid.push(slot);
        }
        let mut output = view::item_view(recipe.output().item, 1, &palette);
        output.id = "brewing-result".into();
        output.command = "recipe-preview".into();
        next.recipe_output = vec![output];
        next.cauldron_inputs = next
            .recipe_grid
            .iter()
            .filter(|slot| {
                slot.item
                    .is_none_or(|item| terra_core::brewing::spoon_speed(item).is_none())
            })
            .cloned()
            .collect();
        next.cauldron_tools = next
            .recipe_grid
            .iter()
            .filter(|slot| {
                slot.item
                    .is_some_and(|item| terra_core::brewing::spoon_speed(item).is_some())
            })
            .cloned()
            .collect();
        next.can_apply_recipe = session
            .container
            .as_ref()
            .is_some_and(|container| container.cauldron.is_some())
            && session.inventory.cursor.is_none();
    }
    if state.selected_kind == RecipeKind::Campfire
        && let Some(recipe) = terra_core::campfire::recipes()
            .iter()
            .find(|recipe| Some(recipe.id) == state.selected)
    {
        let count = session.inventory.count(recipe.input);
        let mut input = view::item_view(recipe.input, 1, &palette);
        input.id = "campfire-input".into();
        input.command = "recipe-preview".into();
        input.missing = count == 0;
        let mut output = view::item_view(recipe.output.item, recipe.output.count, &palette);
        output.id = "campfire-result".into();
        output.command = "recipe-preview".into();
        next.campfire = true;
        next.selected = true;
        next.empty = false;
        next.name = recipe.name.clone();
        next.description = output.description.clone();
        next.recipe_station = "Campfire / fuel".into();
        next.recipe_heading = "Roasting Recipe".into();
        next.recipe_duration = format!("{} s", recipe.seconds);
        next.apply_title = "Place food in the campfire material slot".into();
        next.can_apply_recipe = false;
        next.missing = input.missing;
        next.ingredients = vec![IngredientView {
            slots: vec![input.clone()],
            name: input.name.clone(),
            amount: format!("{count} / 1"),
            enough: count > 0,
        }];
        next.recipe_grid = vec![input];
        next.recipe_output = vec![output];
    }
    if state.selected.is_none()
        && let Some(item) = state.recipe_item
        && let Some(source) = terra_core::tints::canonical_item(item)
        && let Some(rgb) = terra_core::tints::color(item)
    {
        let output = view::item_view(item, 1, &palette);
        next.selected = true;
        next.empty = false;
        next.name = output.name.clone();
        next.description = output.description.clone();
        next.recipe_station = "Crafting".into();
        next.recipe_heading = "Coloring Recipe".into();
        next.recipe_duration.clear();
        next.apply_title = "Use crafting grid".into();
        next.ingredients = [source, ItemKind::Dye(terra_core::beds::DyeColor::Rgb(rgb))]
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let count = session.inventory.count(item);
                let mut input = view::item_view(item, 1, &palette);
                input.id = format!("coloring-input-{index}");
                input.command = "recipe-preview".into();
                input.missing = count == 0;
                IngredientView {
                    name: input.name.clone(),
                    slots: vec![input],
                    amount: format!("{count} / 1"),
                    enough: count > 0,
                }
            })
            .collect();
        next.recipe_grid = next
            .ingredients
            .iter()
            .flat_map(|ingredient| ingredient.slots.clone())
            .collect();
        next.missing = next.ingredients.iter().any(|ingredient| !ingredient.enough);
        next.recipe_output = vec![view::InventoryItemView {
            id: "coloring-result".into(),
            command: "recipe-preview".into(),
            ..output
        }];
        next.recipe_previous = false;
        next.recipe_next = false;
        next.recipe_multiple = false;
        next.recipe_page = "1 / 1".into();
        next.can_apply_recipe = false;
    }
    if state.selected.is_none()
        && let Some(item @ ItemKind::Dye(terra_core::beds::DyeColor::Rgb(target))) =
            state.recipe_item
    {
        let recipe = terra_core::mixer::recipe(target);
        let output = view::item_view(item, recipe.output.count, &palette);
        next.mixing = true;
        next.selected = true;
        next.empty = false;
        next.name = output.name.clone();
        next.description = output.description.clone();
        next.recipe_station = "Mixer".into();
        next.recipe_heading = "Mixing Recipe".into();
        next.recipe_duration = format!("{} s", recipe.seconds);
        next.apply_title = "Set mixer target".into();
        next.ingredients = recipe
            .inputs
            .iter()
            .map(|required| {
                let count = session.inventory.count(required.item);
                let mut input = view::item_view(required.item, required.count, &palette);
                input.command = "recipe-preview".into();
                input.id = format!("mixing-{}", item_id(required.item));
                input.missing = count < required.count;
                IngredientView {
                    name: input.name.clone(),
                    amount: format!("{count} / {}", required.count),
                    enough: !input.missing,
                    slots: vec![input],
                }
            })
            .collect();
        next.recipe_grid = next
            .ingredients
            .iter()
            .flat_map(|ingredient| ingredient.slots.clone())
            .collect();
        next.missing = next.ingredients.iter().any(|ingredient| !ingredient.enough);
        next.recipe_output = vec![view::InventoryItemView {
            id: "mixing-result".into(),
            command: "recipe-preview".into(),
            ..output
        }];
        next.recipe_previous = false;
        next.recipe_next = false;
        next.recipe_multiple = false;
        next.recipe_page = "1 / 1".into();
        next.can_apply_recipe = session
            .container
            .as_ref()
            .is_some_and(|container| container.mixer.is_some())
            && session.inventory.cursor.is_none();
    }
    browser_view(&mut next, &recipes, recipe, &state, &palette);
    if let Some(item) = next.recipe_output.first().and_then(|slot| slot.item) {
        let details = view::item_view(item, 1, &palette);
        next.item_preview = details.preview_key;
        next.item_category = details.category;
        next.source_summary = format!(
            "Source: {} ({} recipes)",
            next.recipe_category,
            next.recipe_alternatives.len().max(1)
        );
        let used = book
            .iter()
            .filter(|recipe| {
                recipe
                    .ingredients()
                    .iter()
                    .any(|ingredient| ingredient.matches(item))
            })
            .count();
        next.uses_summary = format!("Used in: {used} crafting recipes");
    }
    if *view != next {
        *view = next;
    }
}

const WOOD_PREVIEWS: [(&str, u16); 4] =
    [("Oak", 16), ("Spruce", 18), ("Birch", 17), ("Cherry", 135)];
fn wood_preview(state: &CraftingState) -> ItemKind {
    ItemKind::Block(WOOD_PREVIEWS[state.preview_wood.min(WOOD_PREVIEWS.len() - 1)].1)
}

fn describe_ingredient(
    slot: &mut view::InventoryItemView,
    ingredient: terra_core::crafting::Ingredient,
) {
    if ingredient.wood {
        slot.name = "Any wood planks".into();
        slot.description =
            "Accepts Oak, Birch, Spruce and Cherry planks, including mixed wood types.".into();
        slot.identifier = "Wood planks (any type)".into();
    }
}

fn category_metadata(id: &str) -> (&'static str, ItemKind) {
    match id {
        "workbench" => ("Workbench", ItemKind::Block(28)),
        "smelting" => ("Smelting", ItemKind::Block(59)),
        "crushing" => ("Crushing", ItemKind::Block(99)),
        "brewing" => ("Brewing", ItemKind::Block(161)),
        "campfire" => ("Campfire", ItemKind::Block(162)),
        "mixing" => ("Mixing", ItemKind::Block(156)),
        "coloring" => ("Coloring", ItemKind::Dye(terra_core::beds::DyeColor::White)),
        _ => ("Crafting", ItemKind::Block(28)),
    }
}

fn alternative_woods(choice: RecipeChoice<'_>) -> Vec<Option<usize>> {
    if matches!(choice, RecipeChoice::Craft(recipe) if recipe.ingredients().iter().any(|ingredient| ingredient.wood))
    {
        (0..WOOD_PREVIEWS.len()).map(Some).collect()
    } else {
        vec![None]
    }
}

fn browser_view(
    next: &mut CraftingView,
    recipes: &[RecipeChoice<'_>],
    recipe: Option<&Recipe>,
    state: &CraftingState,
    palette: &BlockPalette,
) {
    let selected = state.selected.map(|key| (key, state.selected_kind));
    let active = recipes.iter().find(|r| Some(r.selection()) == selected);
    let category = active.map_or_else(
        || {
            if next.mixing {
                "mixing"
            } else if next.selected {
                "coloring"
            } else {
                "crafting"
            }
        },
        |choice| choice.category(),
    );
    next.recipe_category = category_metadata(category).0.into();
    let mut categories = Vec::new();
    for choice in recipes {
        let id = choice.category();
        if categories
            .iter()
            .any(|row: &RecipeCategoryView| row.id == id)
        {
            continue;
        }
        let (name, icon) = category_metadata(id);
        categories.push(RecipeCategoryView {
            item: Some(icon),
            id: id.into(),
            name: name.into(),
            preview_key: preview_key(icon),
            selected: id == category,
            count: recipes
                .iter()
                .filter(|r| r.category() == id)
                .count()
                .to_string(),
        });
    }
    if categories.is_empty() && next.selected {
        let (name, icon) = category_metadata(category);
        categories.push(RecipeCategoryView {
            item: Some(icon),
            id: category.into(),
            name: name.into(),
            preview_key: preview_key(icon),
            selected: true,
            count: "1".into(),
        });
    }
    next.recipe_categories = categories;
    if recipe.is_some_and(|recipe| {
        recipe
            .ingredients()
            .iter()
            .any(|ingredient| ingredient.wood)
    }) {
        next.any_wood = true;
        next.wood_options = WOOD_PREVIEWS
            .iter()
            .enumerate()
            .map(|(index, (name, material))| WoodOptionView {
                index,
                name: (*name).into(),
                selected: index == state.preview_wood,
                slots: vec![view::item_view(ItemKind::Block(*material), 1, palette)],
            })
            .collect();
    }
    next.recipe_alternatives = recipes
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.category() == category)
        .flat_map(|(index, choice)| {
            alternative_woods(*choice)
                .into_iter()
                .map(move |wood| (index, choice, wood))
        })
        .map(|(index, choice, wood)| RecipeAlternativeView {
            id: format!("recipe-{index}-{}", wood.unwrap_or(4)),
            wood,
            index,
            name: if let Some(wood) = wood {
                format!("{} · {}", choice.name(), WOOD_PREVIEWS[wood].0)
            } else {
                match choice {
                    RecipeChoice::Smelt(recipe) => view::item_view(recipe.input, 1, palette).name,
                    _ => choice.name().into(),
                }
            },
            selected: Some(choice.selection()) == selected
                && wood.is_none_or(|wood| wood == state.preview_wood),
            number: (index + 1).to_string(),
            has_grid: matches!(choice, RecipeChoice::Craft(_)),
            grid: match choice {
                RecipeChoice::Craft(recipe) => {
                    let mut grid: Vec<_> = (0..9)
                        .map(|cell| view::InventoryItemView {
                            id: format!("alternative-{index}-{cell}"),
                            empty: true,
                            ..default()
                        })
                        .collect();
                    for (cell, ingredient) in pattern(recipe, 3) {
                        let mut slot = view::item_view(
                            if ingredient.wood {
                                ItemKind::Block(WOOD_PREVIEWS[wood.unwrap_or(state.preview_wood)].1)
                            } else {
                                ingredient.item
                            },
                            ingredient.count,
                            palette,
                        );
                        describe_ingredient(&mut slot, ingredient);
                        slot.id = format!("alternative-{index}-{cell}");
                        grid[cell] = slot;
                    }
                    grid
                }
                _ => vec![],
            },
            output: match choice {
                RecipeChoice::Craft(recipe) => {
                    let output = recipe_display_output(recipe, state.recipe_item, None);
                    vec![view::item_view(output.item, output.count, palette)]
                }
                RecipeChoice::Smelt(recipe) => vec![view::item_view(
                    recipe.output.item,
                    recipe.output.count,
                    palette,
                )],
                RecipeChoice::Brew(recipe) => vec![view::item_view(
                    recipe.output().item,
                    recipe.output().count,
                    palette,
                )],
                _ => vec![],
            },
            input: match choice {
                RecipeChoice::Smelt(recipe) => vec![view::item_view(recipe.input, 1, palette)],
                RecipeChoice::Brew(recipe) => recipe
                    .inputs
                    .iter()
                    .copied()
                    .chain([terra_core::ItemStack {
                        item: ItemKind::WaterBottle,
                        count: 1,
                    }])
                    .map(|stack| view::item_view(stack.item, stack.count, palette))
                    .collect(),
                _ => vec![],
            },
            duration: match choice {
                RecipeChoice::Smelt(recipe) => format!("{:.1} s", recipe.seconds),
                RecipeChoice::Brew(recipe) => format!("{:.1} s", recipe.seconds),
                _ => String::new(),
            },
            summary: match choice {
                RecipeChoice::Craft(recipe) => recipe
                    .ingredients()
                    .iter()
                    .map(|ingredient| {
                        if ingredient.wood {
                            format!(
                                "{} × {} planks",
                                ingredient.count,
                                WOOD_PREVIEWS[wood.unwrap_or(state.preview_wood)].0
                            )
                        } else {
                            format!(
                                "{} × {}",
                                ingredient.count,
                                view::item_view(ingredient.item, ingredient.count, palette).name
                            )
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" + "),
                _ => category_metadata(choice.category()).0.into(),
            },
        })
        .collect();
    for (index, row) in next.recipe_alternatives.iter_mut().enumerate() {
        row.number = (index + 1).to_string();
    }
    let count = next
        .recipe_alternatives
        .len()
        .max(usize::from(next.selected));
    let page = next
        .recipe_alternatives
        .iter()
        .position(|r| r.selected)
        .unwrap_or(0);
    next.recipe_page = if count > 0 {
        format!("{} / {count}", page + 1)
    } else {
        String::new()
    };
    next.recipe_multiple = count > 1;
    next.recipe_previous = page > 0;
    next.recipe_next = page + 1 < count;
    next.recipe_layout = if next.smelting || next.crushing || next.campfire {
        "recipe-single-input"
    } else if recipe.is_some() {
        "recipe-grid"
    } else {
        "recipe-loose-inputs"
    }
    .into();
    next.recipe_shape = if let Some(recipe) = recipe {
        if matches!(recipe.pattern, RecipePattern::Shapeless { .. }) {
            "Shapeless"
        } else {
            "Shaped"
        }
    } else if next.mixing {
        "Rotational power"
    } else if next.brewing {
        "Heat & stir"
    } else if next.crushing {
        "Rotational power"
    } else if next.smelting || next.campfire {
        "Heat & fuel"
    } else {
        "Shapeless"
    }
    .into();
}

#[cfg(test)]
#[path = "crafting_browser_tests.rs"]
mod browser_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wood_variants_share_recipes_and_tmi_reserves_the_requested_planks() {
        use terra_core::WoodSpecies::{Birch, Cherry, Spruce};
        let book = RecipeBook::builtins();
        for material in [141, 144, 148, 149] {
            let recipe = book
                .iter()
                .find(|r| r.output.item == ItemKind::Block(material))
                .unwrap();
            for (wood, plank) in [(Birch, 17), (Spruce, 18), (Cherry, 135)] {
                let item = ItemKind::for_wood(material, wood);
                let choices = alternatives(book, Some(item));
                assert_eq!(choices.len(), if material == 148 { 2 } else { 1 });
                assert!(choices.iter().any(|choice| matches!(choice, RecipeChoice::Craft(candidate) if candidate.id == recipe.id)));
                if material == 148 {
                    assert!(choices.iter().any(|choice| matches!(choice, RecipeChoice::Craft(candidate) if candidate.id == "teramic::conveyor_dye" && candidate.station == CraftStation::Hand)));
                }
                let expected = terra_core::ItemStack {
                    item,
                    count: recipe.output.count,
                };
                assert_eq!(recipe_display_output(recipe, Some(item), None), expected);
                assert_eq!(
                    recipe_display_output(recipe, Some(item), Some(recipe.output)),
                    recipe.output
                );
                let mut bag = terra_core::Inventory::default();
                bag.insert(ItemKind::Block(16), 64);
                for ingredient in recipe.ingredients() {
                    bag.insert(
                        if ingredient.wood {
                            ItemKind::Block(plank)
                        } else {
                            ingredient.item
                        },
                        ingredient.count,
                    );
                }
                let before = bag.clone();
                let cells = arrange(recipe, &bag, 3, Some(item)).unwrap();
                assert_eq!(
                    book.output_for_reserved_grid(&bag, &cells, 3, CraftStation::Workbench),
                    Some(expected)
                );
                assert_eq!(bag, before);
                for (cell, ingredient) in pattern(recipe, 3) {
                    if ingredient.wood {
                        assert_eq!(
                            bag.slots[usize::from(cells[cell].unwrap().slot)].item,
                            ItemKind::Block(plank)
                        );
                    }
                }
                for stack in &mut bag.slots {
                    if stack.item == ItemKind::Block(plank) {
                        *stack = terra_core::ItemStack::EMPTY;
                    }
                }
                assert!(arrange(recipe, &bag, 3, Some(item)).is_none());
                assert!(arrange(recipe, &bag, 3, None).is_some());
            }
        }
    }
    #[test]
    fn too_many_items_exposes_every_kinetic_recipe_and_autofills_the_existing_grid() {
        let book = RecipeBook::builtins();
        for material in terra_core::kinetic::WOODEN_GEAR..=terra_core::kinetic::CONVEYOR {
            let choices = alternatives(book, Some(ItemKind::Block(material)));
            assert_eq!(
                choices.len(),
                if material == 148 { 2 } else { 1 },
                "kinetic material {material}"
            );
            if material == 148 {
                assert!(choices.iter().any(|choice| matches!(choice, RecipeChoice::Craft(recipe) if recipe.id == "teramic::conveyor_dye" && recipe.station == CraftStation::Hand)));
            }
            let recipe = book
                .iter()
                .find(|recipe| recipe.output.item == ItemKind::Block(material))
                .unwrap();
            let mut inventory = terra_core::Inventory::default();
            for ingredient in recipe.ingredients() {
                inventory.insert(ingredient.item, ingredient.count);
            }
            let before = inventory.clone();
            let cells = arrange(recipe, &inventory, 3, None).unwrap();
            assert_eq!(
                book.grid_match_reserved(&inventory, &cells, 3, CraftStation::Workbench)
                    .map(|value| value.0),
                Some(recipe.key())
            );
            assert_eq!(inventory, before);
        }
    }
    #[test]
    fn all_recipes_autofill_without_duplicating_inventory() {
        for recipe in RecipeBook::builtins().iter() {
            let mut inventory = terra_core::Inventory::default();
            for ingredient in recipe.ingredients() {
                inventory.insert(ingredient.item, ingredient.count * 2);
            }
            let before = inventory.clone();
            let cells = arrange(recipe, &inventory, 3, None).unwrap();
            assert_eq!(
                RecipeBook::builtins()
                    .grid_match_reserved(&inventory, &cells, 3, CraftStation::Workbench)
                    .map(|v| v.0),
                Some(recipe.key()),
                "{}",
                recipe.id
            );
            assert_eq!(inventory, before);
        }
    }
    #[test]
    fn hand_grid_rejects_workbench_patterns_and_reservations_are_bounded() {
        let book = RecipeBook::builtins();
        let chest = book
            .iter()
            .find(|r| r.output.item == ItemKind::Block(29))
            .unwrap();
        let mut bag = terra_core::Inventory::default();
        bag.insert(ItemKind::Block(16), 8);
        assert!(arrange(chest, &bag, 2, None).is_none());
        let cells = arrange(chest, &bag, 3, None).unwrap();
        let state = CraftingState { cells, ..default() };
        assert_eq!(available_count(&state, 0, 8), 0);
        assert_eq!(
            maximum_count(
                book,
                &bag,
                &cells,
                3,
                CraftStation::Workbench,
                CraftDestination::Inventory
            ),
            1
        );
    }
}
