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
    Clear,
    Select(u64),
    Search(String),
    CloseRecipe,
    ApplyRecipe,
    Alternative(i32),
    Craft { maximum: bool, cursor: bool },
}

#[derive(Resource, Default)]
pub(super) struct CraftingState {
    cells: [Option<CraftGridCell>; 9],
    selected: Option<u64>,
    selected_smelting: bool,
    recipe_item: Option<ItemKind>,
    recipe_open: bool,
    search: String,
    error: String,
    context: Option<Option<[i64; 3]>>,
    furnace_context: Option<[i64; 3]>,
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
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct CraftingView {
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
    pub recipe_duration: String,
    pub recipe_heading: String,
    pub apply_title: String,
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
) -> Option<[Option<CraftGridCell>; 9]> {
    if !fits(recipe, size) {
        return None;
    }
    let mut cells = [None; 9];
    let mut used = [0_u16; terra_core::INVENTORY_SLOTS];
    for (cell, ingredient) in pattern(recipe, size) {
        let (slot, _) = inventory.slots.iter().enumerate().find(|(i, s)| {
            *i < used.len()
                && ingredient.matches(s.item)
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
#[derive(Clone, Copy)]
enum RecipeChoice<'a> {
    Craft(&'a Recipe),
    Smelt(&'a terra_core::furnace::SmeltingRecipe),
}
impl RecipeChoice<'_> {
    fn selection(self) -> (u64, bool) {
        match self {
            Self::Craft(recipe) => (recipe.key(), false),
            Self::Smelt(recipe) => (recipe.id, true),
        }
    }
}
fn alternatives(book: &RecipeBook, item: Option<ItemKind>) -> Vec<RecipeChoice<'_>> {
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
        .collect()
}
fn select_recipe(
    state: &mut CraftingState,
    recipe: &Recipe,
    inventory: &InventoryState,
    bag: &terra_core::Inventory,
) {
    state.selected = Some(recipe.key());
    state.selected_smelting = false;
    state.recipe_open = true;
    state.error.clear();
    if recipe.station == CraftStation::Workbench && inventory.station.is_none() {
        state.error = "Requires a workbench.".into();
    } else if arrange(recipe, bag, size(inventory)).is_none() {
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

fn storage_only(session: &ClientSession) -> bool {
    session
        .container
        .as_ref()
        .is_some_and(|container| container.workbench.is_none() && container.furnace.is_none())
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
            if furnace_anchor(session).is_some() {
                state.error = "Requires a crafting grid.".into();
            }
        }
        RecipeChoice::Smelt(recipe) => {
            state.selected = Some(recipe.id);
            state.selected_smelting = true;
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
    }
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
    if inventory.container_open_response.is_some() {
        if state.context.is_some() {
            *state = default();
        }
        slots.clear();
        events.clear();
        wheel.clear();
        return;
    }
    if state.context != Some(inventory.station) || state.furnace_context != furnace_anchor(&session)
    {
        *state = CraftingState {
            context: Some(inventory.station),
            furnace_context: furnace_anchor(&session),
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
            && furnace_anchor(&session).is_none()
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
                if command == "catalog" && session.game_mode == GameMode::Survival =>
            {
                Some(item)
            }
            _ => None,
        };
        if let Some(item) = inspected {
            if state.pending.is_some() {
                state.error = "Waiting for the inventory update.".into();
                continue;
            }
            let item = available_items(GameMode::Creative, &session.inventory, &palette)
                .into_iter()
                .find(|i| item_id(*i) == *item);
            state.recipe_item = item;
            let recipes = alternatives(&book, item);
            let recipe = recipes
                .iter()
                .find(|r| match r {
                    RecipeChoice::Craft(r) => {
                        !storage_only(&session)
                            && furnace_anchor(&session).is_none()
                            && (r.station == CraftStation::Hand || inventory.station.is_some())
                            && arrange(r, &session.inventory, size(&inventory)).is_some()
                    }
                    RecipeChoice::Smelt(r) => can_load_smelting(r, &session),
                })
                .copied()
                .or_else(|| recipes.first().copied());
            if let Some(recipe) = recipe {
                select_choice(&mut state, recipe, &inventory, &session);
            } else {
                state.selected = None;
                state.recipe_open = true;
                state.selected_smelting = false;
                state.error = "No recipe is registered for this item.".into();
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
                }
            }
            CraftingAction::Search(search) => {
                state.search = search.chars().take(96).collect();
                if let Some(recipe) = book.iter().find(|r| {
                    r.id.contains(&state.search)
                        || r.name.to_lowercase().contains(&state.search.to_lowercase())
                }) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Craft(recipe),
                        &inventory,
                        &session,
                    );
                } else if let Some(recipe) = terra_core::furnace::recipes().iter().find(|r| {
                    r.key.contains(&state.search)
                        || r.name.to_lowercase().contains(&state.search.to_lowercase())
                }) {
                    state.recipe_item = Some(recipe.output.item);
                    select_choice(
                        &mut state,
                        RecipeChoice::Smelt(recipe),
                        &inventory,
                        &session,
                    );
                }
            }
            CraftingAction::CloseRecipe => state.recipe_open = false,
            CraftingAction::ApplyRecipe => {
                if session.inventory.cursor.is_some() || !queue.is_empty() {
                    state.error = "Empty the cursor before adding a recipe.".into();
                } else if state.selected_smelting {
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
                } else if storage_only(&session) || furnace_anchor(&session).is_some() {
                    state.error = "Requires a crafting grid.".into();
                } else if let Some(recipe) = state.selected.and_then(|key| book.get(key)) {
                    if recipe.station == CraftStation::Workbench && inventory.station.is_none() {
                        state.error = "Requires a workbench.".into();
                    } else if let Some(cells) =
                        arrange(recipe, &session.inventory, size(&inventory))
                    {
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
                if !recipes.is_empty() {
                    let current = recipes
                        .iter()
                        .position(|r| {
                            Some(r.selection())
                                == state.selected.map(|key| (key, state.selected_smelting))
                        })
                        .unwrap_or(0);
                    let next = (current as i32 + direction.signum())
                        .rem_euclid(recipes.len() as i32) as usize;
                    select_choice(&mut state, recipes[next], &inventory, &session);
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
            && furnace_anchor(&session).is_none()
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

type RecipeInventoryStamp = (u64, u64, Option<([i64; 3], u64)>);

#[allow(clippy::too_many_arguments)]
fn update(
    mut inventory: ResMut<InventoryState>,
    mut state: ResMut<CraftingState>,
    book: Res<RecipeBook>,
    palette: Res<BlockPalette>,
    session: Option<Res<ClientSession>>,
    chunks: Option<Res<terra_world::streaming::ChunkStore>>,
    mut view: ResMut<CraftingView>,
    mut stamp: Local<Option<RecipeInventoryStamp>>,
) {
    let Some(session) = session.filter(|s| s.ready && inventory.open) else {
        if *view != CraftingView::default() {
            *view = default();
        }
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
        session.container.as_ref().and_then(|container| {
            container
                .halves
                .first()
                .map(|half| (container.anchor, half.inventory.revision))
        }),
    ));
    if *stamp == next_stamp
        && !state.is_changed()
        && !book.is_changed()
        && !palette.is_changed()
        && !inventory.is_changed()
    {
        return;
    }
    *stamp = next_stamp;
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
        .filter(|_| !state.selected_smelting)
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
                source.map_or(ingredient.item, |(_, s)| s.item),
                ingredient.count,
                &palette,
            );
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
        .position(|r| {
            Some(r.selection()) == state.selected.map(|key| (key, state.selected_smelting))
        })
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
                let resolved = result
                    .filter(|_| {
                        book.grid_match_reserved(&session.inventory, active, grid_size, station)
                            .is_some_and(|(key, _)| key == r.key())
                    })
                    .unwrap_or(r.output);
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
                && furnace_anchor(&session).is_none()
                && (r.station == CraftStation::Hand || inventory.station.is_some())
                && arrange(r, &session.inventory, grid_size).is_some()
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
    if state.selected_smelting
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
        next.description = output.description.clone();
        next.recipe_station = "Furnace".into();
        next.recipe_heading = "Smelting Recipe".into();
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
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_recipes_autofill_without_duplicating_inventory() {
        for recipe in RecipeBook::builtins().iter() {
            let mut inventory = terra_core::Inventory::default();
            for ingredient in recipe.ingredients() {
                inventory.insert(ingredient.item, ingredient.count * 2);
            }
            let before = inventory.clone();
            let cells = arrange(recipe, &inventory, 3).unwrap();
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
        assert!(arrange(chest, &bag, 2).is_none());
        let cells = arrange(chest, &bag, 3).unwrap();
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
