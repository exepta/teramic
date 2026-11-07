//! Open, fuelled cooking uses the existing food-smelting and fuel definitions.
use crate::{Inventory, ItemKind, ItemStack, creatures::AnimalProduct, furnace::SmeltingRecipe};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const MATERIAL: u16 = 162;
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
pub const SLOTS: usize = 3;

pub fn recipes() -> &'static [SmeltingRecipe] {
    static COOKING: OnceLock<Vec<SmeltingRecipe>> = OnceLock::new();
    COOKING.get_or_init(|| {
        crate::furnace::recipes()
            .iter()
            .filter(|recipe| {
                matches!(
                    recipe.input,
                    ItemKind::Animal(
                        AnimalProduct::Beef
                            | AnimalProduct::Pork
                            | AnimalProduct::Mutton
                            | AnimalProduct::Fish
                            | AnimalProduct::Clownfish
                    )
                )
            })
            .cloned()
            .collect()
    })
}
pub fn recipe_for(item: ItemKind) -> Option<&'static SmeltingRecipe> {
    recipes().iter().find(|recipe| recipe.input == item)
}
pub fn slot_capacity(slot: usize, item: ItemKind) -> u16 {
    if slot == INPUT && recipe_for(item).is_some()
        || slot == FUEL && crate::furnace::fuel_seconds(item).is_some()
    {
        item.max_stack()
    } else {
        0
    }
}
pub fn valid_inventory(inventory: &Inventory) -> bool {
    inventory.valid()
        && inventory.cursor.is_none()
        && inventory.slots.len() <= SLOTS
        && inventory.slots.iter().enumerate().all(|(slot, stack)| {
            *stack == ItemStack::EMPTY
                || if slot == OUTPUT {
                    recipes()
                        .iter()
                        .any(|recipe| recipe.output.item == stack.item)
                } else {
                    stack.count <= slot_capacity(slot, stack.item)
                }
        })
}
pub fn apply_gesture_reserved(
    player: &mut Inventory,
    inventory: &mut Inventory,
    gesture: crate::containers::ContainerGesture,
    reserved: [u16; crate::INVENTORY_SLOTS],
) -> bool {
    crate::containers::apply_filtered_kind(
        player,
        std::slice::from_mut(inventory),
        gesture,
        reserved,
        5,
    )
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampfireState {
    pub lit: bool,
    pub burn_remaining: f32,
    pub burn_total: f32,
    pub recipe: Option<u64>,
    pub progress_seconds: f32,
    pub completed: u64,
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachedCampfire {
    pub state: CampfireState,
    pub inventory: Inventory,
}
impl AttachedCampfire {
    pub fn valid(&self) -> bool {
        self.state.valid() && valid_inventory(&self.inventory)
    }
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampfireView {
    pub lit: bool,
    pub burn_remaining: f32,
    pub burn_total: f32,
    pub recipe: Option<u64>,
    pub progress: f32,
}
impl CampfireView {
    pub fn valid(&self) -> bool {
        self.burn_remaining.is_finite()
            && self.burn_total.is_finite()
            && (0.0..=3600.0).contains(&self.burn_total)
            && (0.0..=self.burn_total).contains(&self.burn_remaining)
            && (!self.lit || self.burn_remaining > 0.0)
            && self
                .recipe
                .is_none_or(|id| recipes().iter().any(|recipe| recipe.id == id))
            && self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
    }
}
impl CampfireState {
    pub fn valid(&self) -> bool {
        self.view().valid()
            && self.progress_seconds.is_finite()
            && (0.0..=600.0).contains(&self.progress_seconds)
            && self
                .recipe
                .and_then(|id| recipes().iter().find(|recipe| recipe.id == id))
                .is_none_or(|recipe| self.progress_seconds <= recipe.seconds)
    }
    pub fn view(&self) -> CampfireView {
        let seconds = self
            .recipe
            .and_then(|id| recipes().iter().find(|recipe| recipe.id == id))
            .map_or(1.0, |recipe| recipe.seconds);
        CampfireView {
            lit: self.lit,
            burn_remaining: self.burn_remaining,
            burn_total: self.burn_total,
            recipe: self.recipe,
            progress: (self.progress_seconds / seconds).clamp(0.0, 1.0),
        }
    }
}
fn load_fuel(state: &mut CampfireState, inventory: &mut Inventory) -> bool {
    let Some(fuel) = inventory.slot(FUEL) else {
        return false;
    };
    let Some(seconds) = crate::furnace::fuel_seconds(fuel.item) else {
        return false;
    };
    inventory.slots[FUEL].count -= 1;
    if inventory.slots[FUEL].count == 0 {
        inventory.slots[FUEL] = ItemStack::EMPTY;
    }
    state.burn_remaining = seconds;
    state.burn_total = seconds;
    true
}
pub fn set_lit(state: &mut CampfireState, inventory: &mut Inventory, lit: bool) -> bool {
    if !state.valid()
        || !valid_inventory(inventory)
        || state.lit == lit
        || inventory.revision == u64::MAX
    {
        return false;
    }
    if !lit {
        state.lit = false;
        state.burn_remaining = 0.0;
        state.burn_total = 0.0;
        return true;
    }
    if !load_fuel(state, inventory) {
        return false;
    }
    inventory.revision += 1;
    state.lit = true;
    true
}
/// Burning continues even without food or with a full output; it is also the kettle's heat source.
pub fn advance(state: &mut CampfireState, inventory: &mut Inventory, elapsed: f32) -> bool {
    if !state.valid()
        || !valid_inventory(inventory)
        || !state.lit
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || inventory.revision == u64::MAX
    {
        return false;
    }
    let before = state.clone();
    let original = inventory.slots.clone();
    inventory.slots.resize(SLOTS, ItemStack::EMPTY);
    let mut remaining = elapsed.min(1.0);
    for _ in 0..8 {
        let recipe = inventory
            .slot(INPUT)
            .and_then(|stack| recipe_for(stack.item));
        if state.recipe != recipe.map(|recipe| recipe.id) {
            state.recipe = recipe.map(|recipe| recipe.id);
            state.progress_seconds = 0.0;
        }
        let cooking = recipe.filter(|recipe| {
            state.completed < u64::MAX
                && inventory.slot(OUTPUT).is_none_or(|stack| {
                    stack.item == recipe.output.item
                        && stack.count <= stack.item.max_stack() - recipe.output.count
                })
        });
        let needed = cooking.map_or(remaining, |recipe| {
            (recipe.seconds - state.progress_seconds).max(0.00001)
        });
        let dt = remaining.min(state.burn_remaining).min(needed);
        remaining -= dt;
        state.burn_remaining = (state.burn_remaining - dt).max(0.0);
        if let Some(recipe) = cooking {
            state.progress_seconds += dt;
            if state.progress_seconds + 0.00001 >= recipe.seconds {
                inventory.slots[INPUT].count -= 1;
                if inventory.slots[INPUT].count == 0 {
                    inventory.slots[INPUT] = ItemStack::EMPTY;
                }
                let count = inventory.slot(OUTPUT).map_or(0, |stack| stack.count);
                inventory.slots[OUTPUT] = ItemStack {
                    count: count + recipe.output.count,
                    ..recipe.output
                };
                state.progress_seconds = 0.0;
                state.completed += 1;
            }
        }
        if state.burn_remaining <= 0.0 && !load_fuel(state, inventory) {
            state.lit = false;
            break;
        }
        if remaining <= 0.00001 {
            break;
        }
    }
    if original != inventory.slots {
        inventory.revision += 1;
    }
    before != *state || original != inventory.slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campfire_crafting_registers_valid_log_ingredients_and_spends_the_real_inputs() {
        use crate::crafting::{CraftDestination, CraftStation, RecipeBook, recipe_key};

        let book = RecipeBook::builtins();
        assert_eq!(book.error, None);
        let mut inventory = Inventory::default();
        for (item, count) in [
            (ItemKind::Block(6), 3),
            (ItemKind::Stick, 3),
            (ItemKind::Mineral(crate::MineralItem::Coal), 1),
        ] {
            assert_eq!(inventory.insert(item, count), 0);
        }
        book.craft(
            &mut inventory,
            recipe_key("teramic::campfire"),
            1,
            CraftStation::Workbench,
            CraftDestination::Inventory,
        )
        .unwrap();
        assert_eq!(inventory.count(ItemKind::Block(MATERIAL)), 1);
        assert_eq!(inventory.count(ItemKind::Block(6)), 0);
        assert_eq!(inventory.count(ItemKind::Stick), 0);
        assert_eq!(
            inventory.count(ItemKind::Mineral(crate::MineralItem::Coal)),
            0
        );
        assert!(inventory.valid());
    }

    #[test]
    fn empty_hearth_still_burns_real_fuel_and_extinguishing_discards_only_spent_fuel() {
        let mut state = CampfireState::default();
        let mut inventory = Inventory {
            slots: vec![
                ItemStack::EMPTY,
                ItemStack {
                    item: ItemKind::Stick,
                    count: 2,
                },
            ],
            ..Default::default()
        };
        assert!(set_lit(&mut state, &mut inventory, true));
        assert_eq!(inventory.slot(FUEL).unwrap().count, 1);
        let before = state.burn_remaining;
        assert!(advance(&mut state, &mut inventory, 0.5));
        assert!(state.burn_remaining < before);
        assert!(set_lit(&mut state, &mut inventory, false));
        assert_eq!(state.burn_remaining, 0.0);
        assert_eq!(inventory.slot(FUEL).unwrap().count, 1);
        assert!(!advance(&mut state, &mut inventory, 1.0));
    }
    #[test]
    fn food_uses_existing_recipe_and_does_not_smelt_ore_or_overflow_output() {
        let recipe = &recipes()[0];
        assert!(recipe_for(ItemKind::Mineral(crate::MineralItem::RawIron)).is_none());
        assert!(recipe_for(ItemKind::Block(4)).is_none());
        assert_eq!(slot_capacity(INPUT, ItemKind::Block(4)), 0);
        assert!(
            recipes()
                .iter()
                .all(|recipe| recipe.output.item != ItemKind::Block(15))
        );
        let mut inventory = Inventory {
            slots: vec![
                ItemStack {
                    item: recipe.input,
                    count: 2,
                },
                ItemStack {
                    item: ItemKind::Stick,
                    count: 2,
                },
                ItemStack {
                    item: recipe.output.item,
                    count: recipe.output.item.max_stack(),
                },
            ],
            ..Default::default()
        };
        let mut state = CampfireState::default();
        assert!(set_lit(&mut state, &mut inventory, true));
        for _ in 0..30 {
            advance(&mut state, &mut inventory, 1.0);
        }
        assert_eq!(inventory.slot(INPUT).unwrap().count, 2);
        inventory.slots[OUTPUT] = ItemStack::EMPTY;
        if !state.lit {
            inventory.slots[FUEL] = ItemStack {
                item: ItemKind::Mineral(crate::MineralItem::Coal),
                count: 1,
            };
            assert!(set_lit(&mut state, &mut inventory, true));
        }
        for _ in 0..100 {
            advance(&mut state, &mut inventory, 1.0);
        }
        assert_eq!(
            inventory.slot(OUTPUT).unwrap().count,
            recipe.output.count * 2
        );
        assert!(state.valid() && valid_inventory(&inventory));
    }

    #[test]
    fn exhausted_completion_counter_does_not_turn_remaining_fuel_into_infinite_heat() {
        let mut state = CampfireState {
            lit: true,
            burn_remaining: 0.5,
            burn_total: 0.5,
            completed: u64::MAX,
            ..Default::default()
        };
        let mut inventory = Inventory::default();
        assert!(advance(&mut state, &mut inventory, 1.0));
        assert!(!state.lit);
        assert_eq!(state.burn_remaining, 0.0);
        assert_eq!(state.completed, u64::MAX);
        assert!(inventory.slot(OUTPUT).is_none());
    }
}
