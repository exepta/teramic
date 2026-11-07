//! Calibrated RGB pigment batches on the shared mechanical/inventory path.
use crate::{Inventory, ItemKind, ItemStack, beds::DyeColor};
use serde::{Deserialize, Serialize};

pub const MATERIAL: u16 = 156;
pub const INPUT_SLOTS: usize = 6;
pub const OUTPUT: usize = INPUT_SLOTS;
pub const SLOTS: usize = INPUT_SLOTS + 1;
pub const SECONDS: f32 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct MixerRecipe {
    pub inputs: Vec<ItemStack>,
    pub output: ItemStack,
    pub seconds: f32,
}

fn linear(rgb: [f32; 3]) -> [f32; 3] {
    rgb.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    })
}

/// A bounded integer pigment fit in linear light, refined by single-dose exchanges.
/// The mixer calibrates concentration to the requested byte-exact RGB; no color catalog/cache grows.
pub fn recipe(target: [u8; 3]) -> MixerRecipe {
    let palette = DyeColor::ALL.map(|color| {
        let [r, g, b, _] = color.tint();
        linear([r, g, b])
    });
    let goal = linear(target.map(|v| f32::from(v) / 255.0));
    let error = |color: [f32; 3]| (0..3).map(|a| (color[a] - goal[a]).powi(2)).sum::<f32>();
    let nearest = (0..palette.len())
        .min_by(|&a, &b| error(palette[a]).total_cmp(&error(palette[b])))
        .unwrap();
    let exact = error(palette[nearest]) < 0.00002;
    let batch = if exact { 1 } else { 16 };
    let mut counts = [0u16; DyeColor::ALL.len()];
    counts[nearest] = batch;
    let mut sum = palette[nearest].map(|v| v * f32::from(batch));
    for _ in 0..96 {
        let mut best = error(sum.map(|v| v / f32::from(batch)));
        let mut exchange = None;
        for from in 0..counts.len() {
            if counts[from] == 0 {
                continue;
            }
            for to in 0..counts.len() {
                if from == to
                    || counts[to] == 0
                        && counts.iter().filter(|&&n| n > 0).count() >= INPUT_SLOTS
                        && counts[from] > 1
                {
                    continue;
                }
                let next = std::array::from_fn(|a| sum[a] - palette[from][a] + palette[to][a]);
                let cost = error(next.map(|v| v / f32::from(batch)));
                if cost + 0.00000001 < best {
                    best = cost;
                    exchange = Some((from, to, next));
                }
            }
        }
        let Some((from, to, next)) = exchange else {
            break;
        };
        counts[from] -= 1;
        counts[to] += 1;
        sum = next;
    }
    MixerRecipe {
        inputs: DyeColor::ALL
            .into_iter()
            .zip(counts)
            .filter_map(|(color, count)| {
                (count > 0).then_some(ItemStack {
                    item: ItemKind::Dye(color),
                    count,
                })
            })
            .collect(),
        output: ItemStack {
            item: ItemKind::Dye(DyeColor::Rgb(target)),
            count: batch,
        },
        seconds: SECONDS,
    }
}

pub fn slot_capacity(slot: usize, item: ItemKind) -> u16 {
    if slot < INPUT_SLOTS && matches!(item, ItemKind::Dye(color) if DyeColor::ALL.contains(&color))
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
                    matches!(stack.item, ItemKind::Dye(DyeColor::Rgb(_)))
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
        3,
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerState {
    pub target: [u8; 3],
    pub progress_seconds: f32,
    pub completed: u64,
}
impl Default for MixerState {
    fn default() -> Self {
        Self {
            target: [160, 90, 210],
            progress_seconds: 0.0,
            completed: 0,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerView {
    pub target: [u8; 3],
    pub progress: f32,
    pub working: bool,
}
impl MixerView {
    pub fn valid(&self) -> bool {
        self.progress.is_finite() && (0.0..=1.0).contains(&self.progress)
    }
}
impl MixerState {
    pub fn valid(&self) -> bool {
        self.progress_seconds.is_finite() && (0.0..=SECONDS).contains(&self.progress_seconds)
    }
    pub fn view(&self, inventory: &Inventory) -> MixerView {
        MixerView {
            target: self.target,
            progress: self.progress_seconds / SECONDS,
            working: self.can_process(inventory),
        }
    }
    pub fn can_process(&self, inventory: &Inventory) -> bool {
        (0..INPUT_SLOTS).any(|slot| inventory.slot(slot).is_some())
            && inputs_available(inventory, &recipe(self.target))
    }
}
fn inputs_available(inventory: &Inventory, recipe: &MixerRecipe) -> bool {
    valid_inventory(inventory)
        && inventory.slot(OUTPUT).is_none_or(|stack| {
            stack.item == recipe.output.item
                && stack.count <= stack.item.max_stack() - recipe.output.count
        })
        && recipe.inputs.iter().all(|needed| {
            (0..INPUT_SLOTS)
                .filter_map(|slot| inventory.slot(slot))
                .filter(|stack| stack.item == needed.item)
                .map(|stack| u32::from(stack.count))
                .sum::<u32>()
                >= u32::from(needed.count)
        })
}
pub fn advance(state: &mut MixerState, inventory: &mut Inventory, elapsed: f32) -> bool {
    if !state.valid()
        || !elapsed.is_finite()
        || elapsed <= 0.0
        || !(0..INPUT_SLOTS).any(|slot| inventory.slot(slot).is_some())
    {
        return false;
    }
    let recipe = recipe(state.target);
    if !inputs_available(inventory, &recipe) {
        return false;
    }
    let progress = state.progress_seconds + elapsed.min(1.0);
    if progress < SECONDS {
        state.progress_seconds = progress;
        return true;
    }
    if inventory.revision == u64::MAX || state.completed == u64::MAX {
        return false;
    }
    inventory.slots.resize(SLOTS, ItemStack::EMPTY);
    for needed in &recipe.inputs {
        let mut remaining = needed.count;
        for stack in &mut inventory.slots[..INPUT_SLOTS] {
            if stack.item == needed.item {
                let moved = remaining.min(stack.count);
                stack.count -= moved;
                remaining -= moved;
                if stack.count == 0 {
                    *stack = ItemStack::EMPTY;
                }
            }
        }
    }
    let count = inventory.slot(OUTPUT).map_or(0, |s| s.count) + recipe.output.count;
    inventory.slots[OUTPUT] = ItemStack {
        count,
        ..recipe.output
    };
    inventory.revision += 1;
    state.completed += 1;
    state.progress_seconds = 0.0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_backpressure_invalid_elapsed_and_overflow_never_consume_inputs() {
        let target = [50, 60, 70];
        let r = recipe(target);
        let mut inventory = Inventory::default();
        inventory.slots.resize(SLOTS, ItemStack::EMPTY);
        for (slot, s) in r.inputs.iter().enumerate() {
            inventory.slots[slot] = *s;
        }
        let mut state = MixerState {
            target,
            ..Default::default()
        };
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let before = (state.clone(), inventory.clone());
            assert!(!advance(&mut state, &mut inventory, invalid));
            assert_eq!((state.clone(), inventory.clone()), before);
        }
        inventory.slots[OUTPUT] = ItemStack {
            item: ItemKind::Dye(DyeColor::Rgb([1, 2, 3])),
            count: 1,
        };
        let before = inventory.clone();
        assert!(!advance(&mut state, &mut inventory, 1.0));
        assert_eq!(inventory, before);
        inventory.slots[OUTPUT] = ItemStack {
            count: crate::STACK_LIMIT,
            ..r.output
        };
        assert!(!advance(&mut state, &mut inventory, 1.0));
        inventory.slots[OUTPUT] = ItemStack::EMPTY;
        inventory.revision = u64::MAX;
        state.progress_seconds = SECONDS - 0.1;
        let before = inventory.clone();
        assert!(!advance(&mut state, &mut inventory, 1.0));
        assert_eq!(inventory, before);
    }
    #[test]
    fn rgb_pigments_craft_into_capes_without_enumerating_new_recipes() {
        use crate::{ArmorMaterial, CapeItem};
        let color = DyeColor::Rgb([17, 219, 84]);
        let cape = CapeItem {
            material: ArmorMaterial::Iron,
            color: DyeColor::White,
        };
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Cape(cape), 1);
        bag.insert(ItemKind::Dye(color), 1);
        let book = crate::crafting::RecipeBook::builtins();
        let cells = [Some(0), Some(1), None, None];
        let expected = ItemKind::Cape(CapeItem { color, ..cape });
        assert_eq!(
            book.output_for_grid(&bag, &cells, 2, crate::crafting::CraftStation::Hand)
                .unwrap()
                .item,
            expected
        );
        assert!(book.craft_grid(
            &mut bag,
            &cells,
            2,
            crate::crafting::CraftStation::Hand,
            1,
            crate::crafting::CraftDestination::Inventory
        ));
        assert_eq!(bag.count(expected), 1);
        assert_eq!(bag.count(ItemKind::Dye(color)), 0);
        assert!(expected.identifier().ends_with("#11DB54"));
    }
    #[test]
    fn recipes_are_bounded_deterministic_and_never_catalog_rgb() {
        for target in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 255],
            [71, 93, 184],
            [0, 255, 0],
            [21, 242, 237],
        ] {
            let a = recipe(target);
            assert_eq!(a, recipe(target));
            assert!(a.inputs.len() <= INPUT_SLOTS);
            assert_eq!(
                a.inputs.iter().map(|s| s.count).sum::<u16>(),
                a.output.count
            );
            assert_eq!(a.output.item, ItemKind::Dye(DyeColor::Rgb(target)));
            assert!(
                a.inputs
                    .iter()
                    .all(|s| matches!(s.item,ItemKind::Dye(c) if DyeColor::ALL.contains(&c)))
            );
            assert!(!ItemKind::EXTRA.contains(&a.output.item));
        }
    }
    #[test]
    fn atomic_batch_split_inputs_output_backpressure_and_restart() {
        let target = [72, 115, 203];
        let r = recipe(target);
        let mut inv = Inventory::default();
        inv.slots.resize(SLOTS, ItemStack::EMPTY);
        for (slot, stack) in r.inputs.iter().enumerate() {
            inv.slots[slot] = *stack;
        }
        let mut state = MixerState {
            target,
            ..Default::default()
        };
        let original = inv.clone();
        for _ in 0..3 {
            assert!(advance(&mut state, &mut inv, 1.0));
        }
        assert_eq!(inv, original);
        let encoded = serde_json::to_vec(&(state.clone(), inv.clone())).unwrap();
        let (mut loaded, mut loaded_inv): (MixerState, Inventory) =
            serde_json::from_slice(&encoded).unwrap();
        assert!(advance(&mut state, &mut inv, 1.0));
        assert!(advance(&mut loaded, &mut loaded_inv, 1.0));
        assert_eq!(state, loaded);
        assert_eq!(inv, loaded_inv);
        assert_eq!(inv.slot(OUTPUT), Some(r.output));
        assert!(
            inv.slots[..INPUT_SLOTS]
                .iter()
                .all(|s| *s == ItemStack::EMPTY)
        );
        assert!(!advance(&mut state, &mut inv, 1.0));
    }
}
