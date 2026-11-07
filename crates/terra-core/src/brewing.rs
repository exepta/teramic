//! Bounded brewing recipes reuse the shared effect kinds and authoritative inventory.
use crate::{Inventory, ItemKind, ItemStack, ToolKind, effects::EffectKind};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const MATERIAL: u16 = 161;
pub const INPUT_SLOTS: usize = 3;
pub const WATER: usize = 3;
pub const BOTTLES: usize = 4;
pub const SPOON: usize = 5;
pub const OUTPUT: usize = 6;
pub const SLOTS: usize = 7;
pub const MAX_WATER: u8 = 3;
pub const POTION_COUNT: usize = 12;
pub const ATTACHED_FIRE: u8 = 4;

pub const fn has_fire(facing: u8) -> bool {
    facing < 8 && facing & ATTACHED_FIRE != 0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotionEffect {
    pub kind: EffectKind,
    pub level: u8,
    pub seconds: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrewingRecipe {
    pub id: u16,
    pub key: String,
    pub name: String,
    pub inputs: Vec<ItemStack>,
    pub effects: Vec<PotionEffect>,
    pub seconds: f32,
    pub color: [u8; 3],
}
impl BrewingRecipe {
    pub fn valid(&self) -> bool {
        self.id > 0
            && self.key.starts_with("teramic::")
            && !self.name.is_empty()
            && self.name.len() <= 96
            && (1..=INPUT_SLOTS).contains(&self.inputs.len())
            && self.inputs.iter().all(|stack| {
                !matches!(stack.item, ItemKind::Potion(_))
                    && stack.valid()
                    && (1..=16).contains(&stack.count)
            })
            && self.inputs.iter().enumerate().all(|(index, stack)| {
                self.inputs[..index]
                    .iter()
                    .all(|other| other.item != stack.item)
            })
            && (1..=4).contains(&self.effects.len())
            && self.effects.iter().enumerate().all(|(index, dose)| {
                let definition = crate::effects::effect_definition(dose.kind);
                (1..=definition.max_level).contains(&dose.level)
                    && dose.seconds.is_finite()
                    && (0.1..=definition.max_duration_seconds).contains(&dose.seconds)
                    && self.effects[..index]
                        .iter()
                        .all(|other| other.kind != dose.kind)
            })
            && self.seconds.is_finite()
            && (2.0..=600.0).contains(&self.seconds)
    }
    pub fn output(&self) -> ItemStack {
        ItemStack {
            item: ItemKind::Potion(self.id),
            count: 1,
        }
    }
}

pub fn recipes() -> &'static [BrewingRecipe] {
    static BOOK: OnceLock<Vec<BrewingRecipe>> = OnceLock::new();
    BOOK.get_or_init(|| {
        const SOURCES: [(&str, &str); POTION_COUNT] = [
            (
                "regeneration",
                include_str!("../../../assets/brewing/regeneration.json"),
            ),
            (
                "strength",
                include_str!("../../../assets/brewing/strength.json"),
            ),
            ("speed", include_str!("../../../assets/brewing/speed.json")),
            (
                "nightvision",
                include_str!("../../../assets/brewing/nightvision.json"),
            ),
            (
                "fire_resistance",
                include_str!("../../../assets/brewing/fire_resistance.json"),
            ),
            (
                "regeneration_strength",
                include_str!("../../../assets/brewing/regeneration_strength.json"),
            ),
            (
                "poison",
                include_str!("../../../assets/brewing/poison.json"),
            ),
            ("slow", include_str!("../../../assets/brewing/slow.json")),
            (
                "freeze",
                include_str!("../../../assets/brewing/freeze.json"),
            ),
            (
                "bleeding",
                include_str!("../../../assets/brewing/bleeding.json"),
            ),
            (
                "rotten",
                include_str!("../../../assets/brewing/rotten.json"),
            ),
            (
                "on_fire",
                include_str!("../../../assets/brewing/on_fire.json"),
            ),
        ];
        SOURCES
            .into_iter()
            .map(|(name, source)| {
                let builtin: BrewingRecipe =
                    serde_json::from_str(source).expect("builtin brewing recipe");
                assert!(builtin.valid());
                std::fs::read_to_string(
                    crate::asset_directory()
                        .join("brewing")
                        .join(format!("{name}.json")),
                )
                .ok()
                .and_then(|source| serde_json::from_str::<BrewingRecipe>(&source).ok())
                .filter(|value| value.id == builtin.id && value.key == builtin.key && value.valid())
                .unwrap_or(builtin)
            })
            .collect()
    })
}
pub fn recipe(id: u16) -> Option<&'static BrewingRecipe> {
    recipes().iter().find(|recipe| recipe.id == id)
}
pub fn drink_seconds(item: ItemKind) -> Option<f32> {
    match item {
        ItemKind::WaterBottle => Some(1.4),
        ItemKind::Potion(id) if recipe(id).is_some() => Some(1.4),
        _ => None,
    }
}
/// Spoon tiers affect work only, never yields or independently defined effects.
pub fn spoon_speed(item: ItemKind) -> Option<f32> {
    match item {
        ItemKind::Tool(ToolKind::WoodSpoon) => Some(1.0),
        ItemKind::Tool(ToolKind::StoneSpoon) => Some(1.2),
        ItemKind::Tool(ToolKind::GoldSpoon) => Some(1.5),
        _ => None,
    }
}
pub fn slot_capacity(slot: usize, item: ItemKind) -> u16 {
    let accepted = match slot {
        0..INPUT_SLOTS => recipes()
            .iter()
            .any(|recipe| recipe.inputs.iter().any(|s| s.item == item)),
        WATER => matches!(
            item,
            ItemKind::WaterBottle
                | ItemKind::GlassBottle
                | ItemKind::Tool(ToolKind::WaterBucket | ToolKind::Bucket)
        ),
        BOTTLES => item == ItemKind::GlassBottle,
        SPOON => spoon_speed(item).is_some(),
        _ => false,
    };
    if accepted { item.max_stack() } else { 0 }
}
pub fn valid_inventory(inventory: &Inventory) -> bool {
    inventory.valid()
        && inventory.cursor.is_none()
        && inventory.slots.len() <= SLOTS
        && inventory.slots.iter().enumerate().all(|(slot, stack)| {
            *stack == ItemStack::EMPTY
                || if slot == OUTPUT {
                    matches!(stack.item, ItemKind::Potion(id) if recipe(id).is_some())
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
        4,
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CauldronState {
    pub recipe: Option<u16>,
    pub water: u8,
    pub progress_seconds: f32,
}
impl Default for CauldronState {
    fn default() -> Self {
        Self {
            recipe: Some(1),
            water: 0,
            progress_seconds: 0.0,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CauldronView {
    pub recipe: Option<u16>,
    pub water: u8,
    pub progress: f32,
    pub heated: bool,
    pub working: bool,
}
impl CauldronView {
    pub fn valid(&self) -> bool {
        self.recipe.is_none_or(|id| recipe(id).is_some())
            && self.water <= MAX_WATER
            && self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && (!self.working || self.heated && self.water > 0 && self.recipe.is_some())
    }
}
impl CauldronState {
    pub fn valid(&self) -> bool {
        self.recipe.is_none_or(|id| recipe(id).is_some())
            && self.water <= MAX_WATER
            && self.progress_seconds.is_finite()
            && (0.0..=600.0).contains(&self.progress_seconds)
            && self
                .recipe
                .and_then(recipe)
                .is_none_or(|recipe| self.progress_seconds <= recipe.seconds)
    }
    pub fn view(&self, heated: bool, inventory: &Inventory) -> CauldronView {
        let selected = self.recipe.and_then(recipe);
        CauldronView {
            recipe: self.recipe,
            water: self.water,
            progress: selected.map_or(0.0, |recipe| {
                (self.progress_seconds / recipe.seconds).clamp(0.0, 1.0)
            }),
            heated,
            working: heated
                && self.water > 0
                && selected.is_some_and(|recipe| ready(inventory, recipe)),
        }
    }
}
fn ready(inventory: &Inventory, recipe: &BrewingRecipe) -> bool {
    inventory
        .slot(BOTTLES)
        .is_some_and(|s| s.item == ItemKind::GlassBottle && s.count > 0)
        && inventory
            .slot(SPOON)
            .and_then(|s| spoon_speed(s.item))
            .is_some()
        && inventory
            .slot(OUTPUT)
            .is_none_or(|s| s.item == recipe.output().item && s.count < s.item.max_stack())
        && recipe.inputs.iter().all(|need| {
            inventory
                .slots
                .iter()
                .take(INPUT_SLOTS)
                .filter(|s| s.item == need.item)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                >= u32::from(need.count)
        })
}
fn take(slots: &mut [ItemStack], item: ItemKind, mut count: u16) {
    for slot in slots.iter_mut().filter(|s| s.item == item) {
        let taken = slot.count.min(count);
        slot.count -= taken;
        count -= taken;
        if slot.count == 0 {
            *slot = ItemStack::EMPTY;
        }
        if count == 0 {
            break;
        }
    }
}
pub fn advance(
    state: &mut CauldronState,
    inventory: &mut Inventory,
    heated: bool,
    elapsed: f32,
) -> bool {
    if !state.valid()
        || !valid_inventory(inventory)
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || inventory.revision == u64::MAX
    {
        return false;
    }
    let before = state.clone();
    let original = inventory.slots.clone();
    inventory.slots.resize(SLOTS, ItemStack::EMPTY);
    if let Some(source) = inventory.slot(WATER) {
        if state.water == 0 && source.item == ItemKind::Tool(ToolKind::WaterBucket) {
            state.water = MAX_WATER;
            inventory.slots[WATER] = ItemStack {
                item: ItemKind::Tool(ToolKind::Bucket),
                count: 1,
            };
        } else if state.water < MAX_WATER
            && source.item == ItemKind::WaterBottle
            && inventory
                .slot(BOTTLES)
                .is_none_or(|s| s.item == ItemKind::GlassBottle && s.count < s.item.max_stack())
        {
            take(
                &mut inventory.slots[WATER..=WATER],
                ItemKind::WaterBottle,
                1,
            );
            let count = inventory.slot(BOTTLES).map_or(0, |s| s.count);
            inventory.slots[BOTTLES] = ItemStack {
                item: ItemKind::GlassBottle,
                count: count + 1,
            };
            state.water += 1;
        }
    }
    if heated
        && state.water > 0
        && let Some(recipe) = state
            .recipe
            .and_then(recipe)
            .filter(|recipe| ready(inventory, recipe))
    {
        let speed = inventory
            .slot(SPOON)
            .and_then(|s| spoon_speed(s.item))
            .unwrap();
        state.progress_seconds += elapsed.min(1.0) * speed;
        if state.progress_seconds + 0.00001 >= recipe.seconds {
            for need in &recipe.inputs {
                take(&mut inventory.slots[..INPUT_SLOTS], need.item, need.count);
            }
            take(
                &mut inventory.slots[BOTTLES..=BOTTLES],
                ItemKind::GlassBottle,
                1,
            );
            let count = inventory.slot(OUTPUT).map_or(0, |s| s.count);
            inventory.slots[OUTPUT] = ItemStack {
                count: count + 1,
                ..recipe.output()
            };
            state.water -= 1;
            state.progress_seconds = 0.0;
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
    fn recipes_reuse_real_effect_limits_and_mixed_doses_stay_independent() {
        assert_eq!(recipes().len(), POTION_COUNT);
        assert!(recipes().iter().all(BrewingRecipe::valid));
        let mixed = recipe(6).unwrap();
        assert_eq!(mixed.effects.len(), 2);
        assert_ne!(mixed.effects[0].seconds, mixed.effects[1].seconds);
        assert_ne!(mixed.effects[0].level, mixed.effects[1].level);
        assert!(recipe(0).is_none() && recipe(u16::MAX).is_none());
        for (id, kind) in [
            (1, EffectKind::Regeneration),
            (2, EffectKind::Strength),
            (3, EffectKind::Speed),
            (4, EffectKind::NightVision),
            (5, EffectKind::FireResistance),
        ] {
            assert_eq!(recipe(id).unwrap().effects[0].kind, kind);
        }
        assert_eq!(
            recipes()
                .iter()
                .map(|recipe| recipe.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            POTION_COUNT
        );
    }

    #[test]
    fn negative_recipes_use_existing_debuffs_and_brew_exactly_one_with_real_ingredients() {
        for (id, kind) in [
            (7, EffectKind::Poison),
            (8, EffectKind::Slow),
            (9, EffectKind::Freeze),
            (10, EffectKind::Bleeding),
            (11, EffectKind::Rotten),
            (12, EffectKind::OnFire),
        ] {
            let recipe = recipe(id).unwrap();
            assert_eq!(recipe.effects.len(), 1);
            assert_eq!(recipe.effects[0].kind, kind);
            assert!(!crate::effects::effect_definition(kind).beneficial);
            assert!(ItemKind::EXTRA.contains(&ItemKind::Potion(id)));
            let mut state = CauldronState {
                recipe: Some(id),
                water: 1,
                ..Default::default()
            };
            let mut inventory = Inventory {
                slots: vec![ItemStack::EMPTY; SLOTS],
                ..Default::default()
            };
            for (slot, stack) in recipe.inputs.iter().rev().enumerate() {
                assert!(slot_capacity(slot, stack.item) >= stack.count);
                inventory.slots[slot] = *stack;
            }
            inventory.slots[BOTTLES] = ItemStack {
                item: ItemKind::GlassBottle,
                count: 1,
            };
            inventory.slots[SPOON] = ItemStack {
                item: ItemKind::Tool(ToolKind::WoodSpoon),
                count: 1,
            };
            let before = inventory.clone();
            assert!(!advance(&mut state, &mut inventory, false, 1.0));
            assert_eq!(inventory, before);
            for _ in 0..recipe.seconds.ceil() as usize {
                advance(&mut state, &mut inventory, true, 1.0);
            }
            assert_eq!(inventory.slot(OUTPUT), Some(recipe.output()));
            assert!(
                inventory.slots[..INPUT_SLOTS]
                    .iter()
                    .all(|stack| *stack == ItemStack::EMPTY)
            );
            assert!(inventory.slot(BOTTLES).is_none());
            assert_eq!(inventory.slot(SPOON), before.slot(SPOON));
            assert_eq!(state.water, 0);
            assert!(state.valid() && valid_inventory(&inventory));
        }
    }
    #[test]
    fn water_heat_bottles_and_spoon_commit_once_and_pause_without_duplication() {
        let recipe = recipe(6).unwrap();
        let mut state = CauldronState {
            recipe: Some(6),
            ..Default::default()
        };
        let mut inventory = Inventory {
            slots: vec![ItemStack::EMPTY; SLOTS],
            ..Default::default()
        };
        for (slot, need) in recipe.inputs.iter().enumerate() {
            inventory.slots[slot] = *need;
        }
        inventory.slots[WATER] = ItemStack {
            item: ItemKind::Tool(ToolKind::WaterBucket),
            count: 1,
        };
        inventory.slots[BOTTLES] = ItemStack {
            item: ItemKind::GlassBottle,
            count: 1,
        };
        inventory.slots[SPOON] = ItemStack {
            item: ItemKind::Tool(ToolKind::GoldSpoon),
            count: 1,
        };
        advance(&mut state, &mut inventory, false, 1.0);
        assert_eq!(state.water, 3);
        assert_eq!(state.progress_seconds, 0.0);
        assert_eq!(
            inventory.slot(WATER).unwrap().item,
            ItemKind::Tool(ToolKind::Bucket)
        );
        for _ in 0..100 {
            advance(&mut state, &mut inventory, true, 1.0);
        }
        assert_eq!(inventory.slot(OUTPUT), Some(recipe.output()));
        assert_eq!(state.water, 2);
        assert!(inventory.slot(BOTTLES).is_none());
        assert_eq!(inventory.slot(SPOON).unwrap().count, 1);
        assert!(valid_inventory(&inventory) && state.valid());
        let before = (state.clone(), inventory.clone());
        inventory.revision = u64::MAX;
        let snapshot = inventory.clone();
        assert!(!advance(&mut state, &mut inventory, true, 1.0));
        assert_eq!(state, before.0);
        assert_eq!(inventory, snapshot);
    }

    #[test]
    fn filled_bottle_return_and_full_output_preserve_all_inputs_until_space_exists() {
        let recipe = recipe(6).unwrap();
        let mut state = CauldronState {
            recipe: Some(6),
            progress_seconds: recipe.seconds - 0.1,
            ..Default::default()
        };
        let mut inventory = Inventory {
            slots: vec![ItemStack::EMPTY; SLOTS],
            ..Default::default()
        };
        for (slot, need) in recipe.inputs.iter().rev().enumerate() {
            inventory.slots[slot] = *need;
        }
        inventory.slots[WATER] = ItemStack {
            item: ItemKind::WaterBottle,
            count: 2,
        };
        inventory.slots[BOTTLES] = ItemStack {
            item: ItemKind::GlassBottle,
            count: ItemKind::GlassBottle.max_stack(),
        };
        inventory.slots[SPOON] = ItemStack {
            item: ItemKind::Tool(ToolKind::WoodSpoon),
            count: 1,
        };
        let before = inventory.clone();
        assert!(!advance(&mut state, &mut inventory, true, 1.0));
        assert_eq!(inventory, before);
        assert_eq!(state.water, 0);

        inventory.slots[BOTTLES].count -= 1;
        inventory.slots[OUTPUT] = ItemStack {
            item: recipe.output().item,
            count: recipe.output().item.max_stack(),
        };
        assert!(advance(&mut state, &mut inventory, true, 1.0));
        assert_eq!(state.water, 1);
        assert_eq!(inventory.slot(WATER).unwrap().count, 1);
        assert_eq!(
            inventory.slot(BOTTLES).unwrap().count,
            ItemKind::GlassBottle.max_stack()
        );
        assert_eq!(
            &inventory.slots[..INPUT_SLOTS],
            &before.slots[..INPUT_SLOTS]
        );
        let before = (state.clone(), inventory.clone());
        assert!(!advance(&mut state, &mut inventory, true, 1.0));
        assert_eq!((state.clone(), inventory.clone()), before);

        inventory.slots[OUTPUT] = ItemStack::EMPTY;
        assert!(advance(&mut state, &mut inventory, true, 1.0));
        assert_eq!(inventory.slot(OUTPUT), Some(recipe.output()));
        assert_eq!(state.water, 0);
        assert_eq!(inventory.slot(WATER).unwrap().count, 1);
        assert_eq!(
            inventory.slot(BOTTLES).unwrap().count,
            ItemKind::GlassBottle.max_stack() - 1
        );
        assert!(
            inventory.slots[..INPUT_SLOTS]
                .iter()
                .all(|stack| *stack == ItemStack::EMPTY)
        );
    }
}
