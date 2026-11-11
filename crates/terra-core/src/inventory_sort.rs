//! Deterministic storage organization without changing item identity or slot ownership.
use crate::{INVENTORY_SLOTS, Inventory, ItemKind, ItemStack, containers::CHEST_SLOTS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod names;

/// Resolve the shared display-name catalog before the first user sort request.
pub fn preload_names() {
    names::preload();
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    /// Blocks, vegetation, resources, tools, equipment, then food and drinks.
    #[default]
    Category,
    /// Full stacks first; ties use the same stable category and item ordering.
    Quantity,
    /// Case-insensitive asset display names, then stable item identity.
    Alphabetical,
    /// Numeric block material IDs, then namespaced IDs for non-block items.
    Id,
}

fn category(item: ItemKind) -> u8 {
    if crate::food(item).is_some() {
        return 5;
    }
    match item {
        ItemKind::Block(_)
        | ItemKind::WoodenBlock { .. }
        | ItemKind::TintedBlock { .. }
        | ItemKind::DyedBed { .. } => 0,
        ItemKind::Prop(_) | ItemKind::Flower(_) | ItemKind::Crop(_) | ItemKind::AppleCore => 1,
        ItemKind::Stick
        | ItemKind::Mineral(_)
        | ItemKind::Dye(_)
        | ItemKind::Dust(_)
        | ItemKind::Animal(_) => 2,
        ItemKind::Tool(_)
        | ItemKind::WornTool { .. }
        | ItemKind::FilledWateringCan { .. }
        | ItemKind::Bellows(_) => 3,
        ItemKind::Armor(_)
        | ItemKind::Accessory(_)
        | ItemKind::Cape(_)
        | ItemKind::HeraldicCape { .. } => 4,
        ItemKind::Apple | ItemKind::GlassBottle | ItemKind::WaterBottle | ItemKind::Potion(_) => 5,
    }
}

/// Equal metadata is required for merging, including color, wood, heraldry and water.
fn packed(slots: &[ItemStack], order: SortOrder) -> Vec<ItemStack> {
    let mut totals = BTreeMap::<ItemKind, u32>::new();
    for stack in slots.iter().filter(|stack| stack.valid()) {
        *totals.entry(stack.item).or_default() += u32::from(stack.count);
    }
    let mut result = Vec::new();
    for (item, mut count) in totals {
        while count > 0 {
            let amount = count.min(u32::from(item.max_stack())) as u16;
            result.push(ItemStack {
                item,
                count: amount,
            });
            count -= u32::from(amount);
        }
    }
    result.sort_by_cached_key(|stack| {
        let material = stack.item.block_material();
        (
            if order == SortOrder::Quantity {
                u16::MAX - stack.count
            } else {
                0
            },
            match order {
                SortOrder::Category | SortOrder::Quantity => category(stack.item),
                SortOrder::Id => u8::from(material.is_none()),
                SortOrder::Alphabetical => 0,
            },
            if order == SortOrder::Alphabetical {
                0
            } else {
                material.unwrap_or_default()
            },
            match order {
                SortOrder::Alphabetical => names::display_name(stack.item).to_lowercase(),
                SortOrder::Id if material.is_none() => stack.item.identifier(),
                _ => String::new(),
            },
            stack.item,
            u16::MAX - stack.count,
        )
    });
    result.resize(slots.len(), ItemStack::EMPTY);
    result
}

fn reservations_valid(player: &Inventory, reserved: &[u16; INVENTORY_SLOTS]) -> bool {
    reserved
        .iter()
        .enumerate()
        .all(|(slot, &count)| count <= player.slot(slot).map_or(0, |stack| stack.count))
}

/// Sort only the backpack. Hotbar and each slot containing a crafting reservation
/// retain their complete stack so existing recipe-grid slot references stay valid.
pub fn backpack(
    inventory: &mut Inventory,
    order: SortOrder,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    if !inventory.valid() || inventory.cursor.is_some() || !reservations_valid(inventory, &reserved)
    {
        return false;
    }
    let indices: Vec<_> = (9..INVENTORY_SLOTS)
        .filter(|&slot| reserved[slot] == 0)
        .collect();
    let before: Vec<_> = indices
        .iter()
        .map(|&slot| inventory.slot(slot).unwrap_or(ItemStack::EMPTY))
        .collect();
    let sorted = packed(&before, order);
    if before == sorted {
        return true;
    }
    let Some(revision) = inventory.revision.checked_add(1) else {
        return false;
    };
    let mut slots = inventory.slots.clone();
    slots.resize(INVENTORY_SLOTS, ItemStack::EMPTY);
    for (index, stack) in indices.into_iter().zip(sorted) {
        slots[index] = stack;
    }
    inventory.slots = slots;
    inventory.revision = revision;
    true
}

/// Organize the connected storage as one view. Player items and reservations never
/// enter the storage pool; all half revisions are preflighted before any mutation.
pub fn storage(
    player: &Inventory,
    halves: &mut [Inventory],
    order: SortOrder,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    if !player.valid()
        || player.cursor.is_some()
        || !reservations_valid(player, &reserved)
        || !(1..=2).contains(&halves.len())
        || !halves.iter().all(crate::containers::valid_chest)
    {
        return false;
    }
    let mut before = vec![ItemStack::EMPTY; halves.len() * CHEST_SLOTS];
    for (index, half) in halves.iter().enumerate() {
        let start = index * CHEST_SLOTS;
        before[start..start + half.slots.len()].copy_from_slice(&half.slots);
    }
    let sorted = packed(&before, order);
    let changed: Vec<_> = (0..halves.len())
        .map(|index| {
            let start = index * CHEST_SLOTS;
            before[start..start + CHEST_SLOTS] != sorted[start..start + CHEST_SLOTS]
        })
        .collect();
    if halves
        .iter()
        .zip(&changed)
        .any(|(half, &changed)| changed && half.revision == u64::MAX)
    {
        return false;
    }
    for (index, (half, changed)) in halves.iter_mut().zip(changed).enumerate() {
        if changed {
            let start = index * CHEST_SLOTS;
            half.slots = sorted[start..start + CHEST_SLOTS].to_vec();
            half.revision += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests;
