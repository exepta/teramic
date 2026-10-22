//! Shared slot rules and deterministic, fixed-budget smelting. Authorities own elapsed time.
pub use crate::BellowsKind;
use crate::{
    INVENTORY_SLOTS, Inventory, ItemKind, ItemStack, STACK_LIMIT, containers::ContainerGesture,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const SLOTS: usize = 4;
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
pub const BELLOWS: usize = 3;
pub const MAX_SMOKE_CELLS: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmeltingRecipe {
    pub id: u64,
    pub key: String,
    pub name: String,
    pub input: ItemKind,
    pub output: ItemStack,
    pub seconds: f32,
}
impl SmeltingRecipe {
    pub fn valid(&self) -> bool {
        self.id > 0
            && self.key.starts_with("teramic::")
            && !self.name.is_empty()
            && self.input.valid()
            && self.output.valid()
            && self.input != self.output.item
            && self.output.count <= STACK_LIMIT / 2
            && self.seconds.is_finite()
            && (0.5..=600.0).contains(&self.seconds)
    }
}
const RECIPES: [(&str, &str); 8] = [
    ("beef", include_str!("../../../assets/smelting/beef.json")),
    ("pork", include_str!("../../../assets/smelting/pork.json")),
    (
        "mutton",
        include_str!("../../../assets/smelting/mutton.json"),
    ),
    ("fish", include_str!("../../../assets/smelting/fish.json")),
    (
        "clownfish",
        include_str!("../../../assets/smelting/clownfish.json"),
    ),
    ("iron", include_str!("../../../assets/smelting/iron.json")),
    ("gold", include_str!("../../../assets/smelting/gold.json")),
    (
        "smooth_stone",
        include_str!("../../../assets/smelting/smooth_stone.json"),
    ),
];
fn load<T: serde::de::DeserializeOwned>(folder: &str, name: &str, fallback: &str) -> Option<T> {
    let path = crate::asset_directory()
        .join(folder)
        .join(format!("{name}.json"));
    match std::fs::read_to_string(&path) {
        Ok(source) => match serde_json::from_str(&source) {
            Ok(value) => Some(value),
            Err(error) => {
                eprintln!(
                    "Invalid {}: {error}; using builtin definition",
                    path.display()
                );
                serde_json::from_str(fallback).ok()
            }
        },
        Err(_) => serde_json::from_str(fallback).ok(),
    }
}
pub fn recipes() -> &'static [SmeltingRecipe] {
    static BOOK: OnceLock<Vec<SmeltingRecipe>> = OnceLock::new();
    BOOK.get_or_init(|| {
        let mut recipes = Vec::new();
        for (name, source) in RECIPES {
            let Some(builtin) = serde_json::from_str::<SmeltingRecipe>(source)
                .ok()
                .filter(SmeltingRecipe::valid)
            else {
                continue;
            };
            let value = load::<SmeltingRecipe>("smelting", name, source)
                .filter(|r| r.valid() && r.id == builtin.id && r.key == builtin.key)
                .unwrap_or(builtin);
            if !recipes
                .iter()
                .any(|r: &SmeltingRecipe| r.id == value.id || r.input == value.input)
            {
                recipes.push(value);
            }
        }
        recipes
    })
}
pub fn recipe_for(item: ItemKind) -> Option<&'static SmeltingRecipe> {
    recipes().iter().find(|r| r.input == item)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuelDefinition {
    pub item: ItemKind,
    pub seconds: f32,
}
pub fn fuel_seconds(item: ItemKind) -> Option<f32> {
    static FUELS: OnceLock<Vec<FuelDefinition>> = OnceLock::new();
    FUELS
        .get_or_init(|| {
            const SOURCES: [(&str, &str); 8] = [
                ("coal", include_str!("../../../assets/fuels/coal.json")),
                ("stick", include_str!("../../../assets/fuels/stick.json")),
                (
                    "oak_log",
                    include_str!("../../../assets/fuels/oak_log.json"),
                ),
                (
                    "birch_log",
                    include_str!("../../../assets/fuels/birch_log.json"),
                ),
                (
                    "spruce_log",
                    include_str!("../../../assets/fuels/spruce_log.json"),
                ),
                (
                    "oak_planks",
                    include_str!("../../../assets/fuels/oak_planks.json"),
                ),
                (
                    "birch_planks",
                    include_str!("../../../assets/fuels/birch_planks.json"),
                ),
                (
                    "spruce_planks",
                    include_str!("../../../assets/fuels/spruce_planks.json"),
                ),
            ];
            SOURCES
                .into_iter()
                .filter_map(|(name, source)| {
                    let builtin = serde_json::from_str::<FuelDefinition>(source).ok()?;
                    Some(
                        load::<FuelDefinition>("fuels", name, source)
                            .filter(|f| {
                                f.item == builtin.item
                                    && f.item.valid()
                                    && f.seconds.is_finite()
                                    && (0.5..=3600.0).contains(&f.seconds)
                            })
                            .unwrap_or(builtin),
                    )
                })
                .collect()
        })
        .iter()
        .find(|f| f.item == item)
        .map(|f| f.seconds)
}
pub fn slot_capacity(slot: usize, item: ItemKind) -> u16 {
    match slot {
        INPUT if recipe_for(item).is_some() => STACK_LIMIT,
        FUEL if fuel_seconds(item).is_some() => STACK_LIMIT,
        BELLOWS if matches!(item, ItemKind::Bellows(_)) => 1,
        _ => 0,
    }
}
pub fn valid_inventory(inventory: &Inventory) -> bool {
    inventory.valid()
        && inventory.cursor.is_none()
        && inventory.slots.len() <= SLOTS
        && inventory.slots.iter().enumerate().all(|(slot, stack)| {
            *stack == ItemStack::EMPTY
                || if slot == OUTPUT {
                    recipes().iter().any(|r| r.output.item == stack.item)
                } else {
                    stack.count <= slot_capacity(slot, stack.item)
                }
        })
}
pub fn apply_gesture(
    player: &mut Inventory,
    inventory: &mut Inventory,
    gesture: ContainerGesture,
) -> bool {
    apply_gesture_reserved(player, inventory, gesture, [0; INVENTORY_SLOTS])
}
pub fn apply_gesture_reserved(
    player: &mut Inventory,
    inventory: &mut Inventory,
    gesture: ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    crate::containers::apply_filtered(
        player,
        std::slice::from_mut(inventory),
        gesture,
        reserved,
        true,
    )
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct FurnaceView {
    pub progress: f32,
    pub burn_remaining: f32,
    pub burn_total: f32,
    pub recipe: Option<u64>,
    pub smoke_cells: u16,
    pub vented: bool,
}
impl FurnaceView {
    pub fn valid(&self) -> bool {
        self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && self.burn_remaining.is_finite()
            && self.burn_total.is_finite()
            && (0.0..=3600.0).contains(&self.burn_total)
            && (0.0..=self.burn_total).contains(&self.burn_remaining)
            && self.smoke_cells <= MAX_SMOKE_CELLS as u16
            && self
                .recipe
                .is_none_or(|id| recipes().iter().any(|r| r.id == id))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct FurnaceState {
    pub progress_seconds: f32,
    pub recipe: Option<u64>,
    pub burn_remaining: f32,
    pub burn_total: f32,
    /// Persisted ordinal prevents save/reload from rerolling bonus results.
    pub completed: u64,
    /// Signed fractional smoke-cell balance: generation minus ventilation.
    /// Reuses the original serialized field so existing furnace saves remain valid.
    pub smoke_credit: f32,
    pub outlet: [i64; 3],
    pub vented: bool,
    pub smoke_cells: Vec<[i64; 3]>,
}
impl FurnaceState {
    pub fn valid(&self) -> bool {
        self.view().valid()
            && self.progress_seconds.is_finite()
            && (0.0..=600.0).contains(&self.progress_seconds)
            && self.smoke_credit.is_finite()
            && (-2.0..=2.0).contains(&self.smoke_credit)
            && self.smoke_cells.len() <= MAX_SMOKE_CELLS
            && self
                .smoke_cells
                .iter()
                .chain(std::iter::once(&self.outlet))
                .all(|p| p.iter().all(|v| v.unsigned_abs() <= 1_000_000))
            && self
                .smoke_cells
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.smoke_cells.len()
    }
    pub fn view(&self) -> FurnaceView {
        let seconds = self
            .recipe
            .and_then(|id| recipes().iter().find(|r| r.id == id))
            .map_or(1.0, |r| r.seconds);
        FurnaceView {
            progress: (self.progress_seconds / seconds).clamp(0.0, 1.0),
            burn_remaining: self.burn_remaining,
            burn_total: self.burn_total,
            recipe: self.recipe,
            smoke_cells: self.smoke_cells.len() as u16,
            vented: self.vented,
        }
    }
}
fn mix(mut n: u64) -> u64 {
    n ^= n >> 30;
    n = n.wrapping_mul(0xbf58476d1ce4e5b9);
    n ^= n >> 27;
    n = n.wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn result_count(
    state: &FurnaceState,
    recipe: &SmeltingRecipe,
    bellows: Option<BellowsKind>,
    seed: u64,
) -> u16 {
    let roll =
        (mix(seed ^ state.completed.wrapping_mul(0x9e3779b97f4a7c15)) >> 40) as f32 / 16_777_216.0;
    recipe.output.count
        * if roll < bellows.map_or(0.0, BellowsKind::double_chance) {
            2
        } else {
            1
        }
}
/// Advances at most one second and three completions per call. Unloaded stations do not advance.
pub fn advance(
    state: &mut FurnaceState,
    inventory: &mut Inventory,
    elapsed: f32,
    seed: u64,
) -> bool {
    let recipe = inventory.slot(INPUT).and_then(|s| recipe_for(s.item));
    let fuel = inventory.slot(FUEL).and_then(|s| fuel_seconds(s.item));
    advance_with(state, inventory, elapsed, seed, recipe, fuel)
}

fn advance_with(
    state: &mut FurnaceState,
    inventory: &mut Inventory,
    elapsed: f32,
    seed: u64,
    recipe: Option<&SmeltingRecipe>,
    fuel: Option<f32>,
) -> bool {
    if !state.valid()
        || !valid_inventory(inventory)
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || inventory.revision == u64::MAX
        || state.completed == u64::MAX
    {
        return false;
    }
    let before_state = state.clone();
    let before_slots = inventory.slots.clone();
    inventory.slots.resize(SLOTS, ItemStack::EMPTY);
    let bellows = inventory.slot(BELLOWS).and_then(|s| {
        if let ItemKind::Bellows(kind) = s.item {
            Some(kind)
        } else {
            None
        }
    });
    let speed = bellows.map_or(1.0, BellowsKind::speed_multiplier);
    let mut remaining = elapsed.min(1.0);
    // A second can cross three recipe completions, two fuel boundaries, and a final partial step.
    for _ in 0..8 {
        let recipe = inventory.slot(INPUT).and(recipe);
        if state.recipe != recipe.map(|r| r.id) {
            state.recipe = recipe.map(|r| r.id);
            state.progress_seconds = 0.0;
        }
        let can_cook = recipe.is_some_and(|r| {
            inventory.slot(OUTPUT).is_none_or(|s| {
                s.item == r.output.item
                    && s.count <= STACK_LIMIT - result_count(state, r, bellows, seed)
            })
        });
        if state.burn_remaining <= 0.0
            && can_cook
            && let Some(seconds) = inventory.slot(FUEL).and(fuel)
        {
            let fuel = &mut inventory.slots[FUEL];
            fuel.count -= 1;
            if fuel.count == 0 {
                *fuel = ItemStack::EMPTY;
            }
            state.burn_remaining = seconds;
            state.burn_total = seconds;
        }
        let Some(recipe) = recipe.filter(|_| can_cook && state.burn_remaining > 0.0) else {
            state.burn_remaining = (state.burn_remaining - remaining).max(0.0);
            break;
        };
        let needed = ((recipe.seconds - state.progress_seconds).max(0.0) / speed).max(0.00001);
        let dt = remaining.min(state.burn_remaining).min(needed);
        state.burn_remaining = (state.burn_remaining - dt).max(0.0);
        state.progress_seconds += dt * speed;
        remaining -= dt;
        if state.progress_seconds + 0.0001 >= recipe.seconds {
            let count = result_count(state, recipe, bellows, seed);
            let input = &mut inventory.slots[INPUT];
            input.count -= 1;
            if input.count == 0 {
                *input = ItemStack::EMPTY;
            }
            let existing = inventory.slot(OUTPUT).map_or(0, |s| s.count);
            inventory.slots[OUTPUT] = ItemStack {
                item: recipe.output.item,
                count: existing + count,
            };
            state.completed = state.completed.saturating_add(1);
            state.progress_seconds = 0.0;
        }
        if remaining <= 0.00001 || state.completed == u64::MAX {
            break;
        }
    }
    if inventory.slots != before_slots {
        inventory.revision += 1;
    }
    *state != before_state || inventory.slots != before_slots
}

#[cfg(test)]
mod tests;
