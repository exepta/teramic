//! One inventory revision owns each atomic backpack/cursor/equipment transfer.
use terra_core::{EquipmentSlot, EquipmentSlots, Inventory, ItemStack};

pub(super) fn click(
    inventory: &mut Inventory,
    equipment: &mut EquipmentSlots,
    index: usize,
    secondary: bool,
) -> bool {
    if !inventory.valid() || !equipment.valid() {
        return false;
    }
    let Some(slot) = EquipmentSlot::from_index(index) else {
        return false;
    };
    let Some(revision) = inventory.revision.checked_add(1) else {
        return false;
    };
    let equipped = equipment.get(slot);
    match (inventory.cursor, equipped) {
        (None, Some(item)) => {
            equipment.set(slot, None);
            inventory.cursor = Some(ItemStack { item, count: 1 });
        }
        (Some(cursor), present)
            if cursor.valid()
                && cursor.count == 1
                && EquipmentSlot::for_item(cursor.item) == Some(slot)
                && present != Some(cursor.item)
                && (present.is_none() || !secondary) =>
        {
            if !equipment.set(slot, Some(cursor.item)) {
                return false;
            }
            inventory.cursor = present.map(|item| ItemStack { item, count: 1 });
        }
        _ => return false,
    }
    inventory.revision = revision;
    true
}

pub(super) fn equip(
    inventory: &mut Inventory,
    equipment: &mut EquipmentSlots,
    index: usize,
) -> bool {
    if !inventory.valid() || !equipment.valid() || inventory.revision == u64::MAX {
        return false;
    }
    let Some(stack) = inventory.slot(index) else {
        return false;
    };
    let Some(slot) = EquipmentSlot::for_item(stack.item) else {
        return false;
    };
    if equipment.get(slot).is_some() {
        return false;
    }
    if !inventory.take_from(index, stack.item) {
        return false;
    }
    equipment.set(slot, Some(stack.item));
    true
}

pub(super) fn unequip(
    inventory: &mut Inventory,
    equipment: &mut EquipmentSlots,
    index: usize,
) -> bool {
    if !inventory.valid() || !equipment.valid() {
        return false;
    }
    let Some(slot) = EquipmentSlot::from_index(index) else {
        return false;
    };
    let Some(item) = equipment.get(slot) else {
        return false;
    };
    let Some(revision) = inventory.revision.checked_add(1) else {
        return false;
    };
    let mut next = inventory.clone();
    if next.insert(item, 1) != 0 {
        return false;
    }
    next.revision = revision;
    equipment.set(slot, None);
    *inventory = next;
    true
}

#[cfg(test)]
#[path = "server_equipment_tests.rs"]
mod tests;
