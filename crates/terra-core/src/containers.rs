//! Atomic gestures over the player's inventory and one or two physical chests.
use crate::{INVENTORY_SLOTS, Inventory, ItemStack, STACK_LIMIT};
use serde::{Deserialize, Serialize};

pub const CHEST_SLOTS: usize = 18;

/// Ordinary storage chests; only material 29 supports a paired second half.
pub const fn is_chest(material: u16) -> bool {
    matches!(material, 29 | crate::grave_blocks::ORNATE_CHEST)
}

pub const CONTAINER_VIEW_SLOTS: usize = INVENTORY_SLOTS + 2 * CHEST_SLOTS;

/// Placement intent only. Each physical chest retains its own inventory.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChestMerge {
    #[default]
    Auto,
    Separate,
    Target([i64; 3]),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerGesture {
    Click {
        slot: u8,
        secondary: bool,
    },
    Drag {
        slots: Vec<u8>,
        secondary: bool,
    },
    QuickMove {
        slot: u8,
    },
    Collect {
        slot: u8,
    },
    CollectFrom {
        slot: u8,
    },
    Wheel {
        slot: u8,
        to_cursor: bool,
    },
    Sort {
        order: crate::inventory_sort::SortOrder,
    },
    SortBackpack {
        order: crate::inventory_sort::SortOrder,
    },
}

pub fn valid_chest(inventory: &Inventory) -> bool {
    inventory.valid() && inventory.cursor.is_none() && inventory.slots.len() <= CHEST_SLOTS
}

struct View {
    slots: Vec<ItemStack>,
    cursor: Option<ItemStack>,
    reservations: [Option<ItemStack>; INVENTORY_SLOTS],
    machine: u8,
}
impl View {
    fn capacity(&self, slot: usize, item: crate::ItemKind) -> u16 {
        if slot >= INVENTORY_SLOTS {
            match self.machine {
                1 => return crate::furnace::slot_capacity(slot - INVENTORY_SLOTS, item),
                2 => return crate::crusher::slot_capacity(slot - INVENTORY_SLOTS, item),
                3 => return crate::mixer::slot_capacity(slot - INVENTORY_SLOTS, item),
                4 => return crate::brewing::slot_capacity(slot - INVENTORY_SLOTS, item),
                5 => return crate::campfire::slot_capacity(slot - INVENTORY_SLOTS, item),
                _ => {}
            }
        }
        self.reservations
            .get(slot)
            .copied()
            .flatten()
            .map_or(item.max_stack(), |reserved| {
                if reserved.item == item {
                    item.max_stack().saturating_sub(reserved.count)
                } else {
                    0
                }
            })
    }
    fn slot(&self, slot: usize) -> Option<ItemStack> {
        self.slots.get(slot).copied().filter(|stack| stack.valid())
    }
    fn set(&mut self, slot: usize, stack: Option<ItemStack>) {
        self.slots[slot] = stack.unwrap_or(ItemStack::EMPTY);
    }
    fn opposite(&self, slot: usize) -> std::ops::Range<usize> {
        if slot < INVENTORY_SLOTS {
            INVENTORY_SLOTS..self.slots.len()
        } else {
            0..INVENTORY_SLOTS
        }
    }
    fn click(&mut self, slot: usize, secondary: bool) -> bool {
        match (self.cursor, self.slot(slot)) {
            (None, None) => false,
            (None, Some(mut stack)) => {
                let count = if secondary {
                    stack.count.div_ceil(2)
                } else {
                    stack.count
                };
                self.cursor = Some(ItemStack { count, ..stack });
                stack.count -= count;
                self.set(slot, (stack.count > 0).then_some(stack));
                true
            }
            (Some(mut cursor), target) if target.is_none_or(|s| s.item == cursor.item) => {
                let present = target.map_or(0, |s| s.count);
                let moved = (if secondary { 1 } else { cursor.count })
                    .min(self.capacity(slot, cursor.item).saturating_sub(present));
                if moved == 0 {
                    return false;
                }
                self.set(
                    slot,
                    Some(ItemStack {
                        item: cursor.item,
                        count: present + moved,
                    }),
                );
                cursor.count -= moved;
                self.cursor = (cursor.count > 0).then_some(cursor);
                true
            }
            (Some(cursor), Some(target))
                if !secondary && self.capacity(slot, cursor.item) >= cursor.count =>
            {
                self.set(slot, Some(cursor));
                self.cursor = Some(target);
                true
            }
            _ => false,
        }
    }
    fn quick_move(&mut self, slot: usize, count: u16) -> bool {
        let Some(mut source) = self.slot(slot) else {
            return false;
        };
        let original = source.count;
        let mut remaining = count.min(original);
        for occupied in [true, false] {
            for target in self.opposite(slot) {
                let stack = self.slot(target);
                if stack.is_some() != occupied || stack.is_some_and(|s| s.item != source.item) {
                    continue;
                }
                let present = stack.map_or(0, |s| s.count);
                let moved =
                    remaining.min(self.capacity(target, source.item).saturating_sub(present));
                if moved > 0 {
                    self.set(
                        target,
                        Some(ItemStack {
                            item: source.item,
                            count: present + moved,
                        }),
                    );
                    remaining -= moved;
                }
            }
        }
        source.count -= count.min(original) - remaining;
        if source.count == original {
            return false;
        }
        self.set(slot, (source.count > 0).then_some(source));
        true
    }
    fn collect_from(&mut self, slot: usize, limit: u16) -> bool {
        let Some(mut cursor) = self.cursor else {
            return self.click(slot, false);
        };
        let Some(mut source) = self.slot(slot) else {
            return false;
        };
        if source.item != cursor.item {
            return false;
        }
        let moved = limit
            .min(source.count)
            .min(cursor.item.max_stack() - cursor.count);
        if moved == 0 {
            return false;
        }
        source.count -= moved;
        cursor.count += moved;
        self.cursor = Some(cursor);
        self.set(slot, (source.count > 0).then_some(source));
        true
    }
    fn apply(&mut self, gesture: ContainerGesture) -> bool {
        if let ContainerGesture::Drag { slots, secondary } = gesture {
            let Some(mut cursor) = self.cursor else {
                return false;
            };
            if slots.len() > CONTAINER_VIEW_SLOTS {
                return false;
            }
            let mut seen = [false; CONTAINER_VIEW_SLOTS];
            let mut targets = Vec::new();
            for slot in slots.into_iter().map(usize::from) {
                if slot >= self.slots.len() || seen[slot] {
                    return false;
                }
                seen[slot] = true;
                if self.slot(slot).is_none_or(|s| s.item == cursor.item)
                    && self.capacity(slot, cursor.item) > self.slot(slot).map_or(0, |s| s.count)
                {
                    targets.push(slot);
                }
            }
            if targets.is_empty() {
                return false;
            }
            let amount = if secondary {
                1
            } else {
                cursor.count / targets.len() as u16
            };
            if amount == 0 {
                return false;
            }
            let original = cursor.count;
            for slot in targets {
                let present = self.slot(slot).map_or(0, |s| s.count);
                let moved = amount
                    .min(cursor.count)
                    .min(self.capacity(slot, cursor.item).saturating_sub(present));
                if moved > 0 {
                    self.set(
                        slot,
                        Some(ItemStack {
                            item: cursor.item,
                            count: present + moved,
                        }),
                    );
                    cursor.count -= moved;
                }
            }
            self.cursor = (cursor.count > 0).then_some(cursor);
            return cursor.count != original;
        }
        let slot = usize::from(match gesture {
            ContainerGesture::Click { slot, .. }
            | ContainerGesture::QuickMove { slot }
            | ContainerGesture::Collect { slot }
            | ContainerGesture::CollectFrom { slot }
            | ContainerGesture::Wheel { slot, .. } => slot,
            ContainerGesture::Drag { .. }
            | ContainerGesture::Sort { .. }
            | ContainerGesture::SortBackpack { .. } => return false,
        });
        if slot >= self.slots.len() {
            return false;
        }
        match gesture {
            ContainerGesture::Click { secondary, .. } => self.click(slot, secondary),
            ContainerGesture::QuickMove { .. } => self.quick_move(slot, STACK_LIMIT),
            ContainerGesture::CollectFrom { .. } => self.collect_from(slot, STACK_LIMIT),
            ContainerGesture::Collect { .. } => {
                let Some(item) = self.cursor.or_else(|| self.slot(slot)).map(|s| s.item) else {
                    return false;
                };
                let original = self.cursor.map_or(0, |s| s.count);
                if original == self.cursor.map_or(STACK_LIMIT, |s| s.item.max_stack()) {
                    return false;
                }
                self.cursor = Some(self.cursor.unwrap_or(ItemStack { item, count: 0 }));
                let mut changed = false;
                for full in [false, true] {
                    for index in 0..self.slots.len() {
                        if self.slot(index).is_some_and(|s| {
                            s.item == item && (s.count == item.max_stack()) == full
                        }) {
                            changed |= self.collect_from(index, STACK_LIMIT);
                        }
                    }
                }
                changed
            }
            ContainerGesture::Wheel { to_cursor, .. } => {
                if self.cursor.is_some() {
                    if to_cursor {
                        self.collect_from(slot, 1)
                    } else {
                        self.click(slot, true)
                    }
                } else if !to_cursor {
                    self.quick_move(slot, 1)
                } else {
                    let Some(mut target) = self.slot(slot) else {
                        return false;
                    };
                    if target.count >= self.capacity(slot, target.item) {
                        return false;
                    }
                    let Some(source) = self
                        .opposite(slot)
                        .find(|&index| self.slot(index).is_some_and(|s| s.item == target.item))
                    else {
                        return false;
                    };
                    let Some(mut source_stack) = self.slot(source) else {
                        return false;
                    };
                    source_stack.count -= 1;
                    target.count += 1;
                    self.set(source, (source_stack.count > 0).then_some(source_stack));
                    self.set(slot, Some(target));
                    true
                }
            }
            ContainerGesture::Drag { .. }
            | ContainerGesture::Sort { .. }
            | ContainerGesture::SortBackpack { .. } => false,
        }
    }
}

/// The caller validates reach, ownership and expected revisions before committing.
/// No mutation is visible unless every resulting inventory and revision is valid.
pub fn apply_gesture(
    player: &mut Inventory,
    halves: &mut [Inventory],
    gesture: ContainerGesture,
) -> bool {
    apply_gesture_reserved(player, halves, gesture, [0; INVENTORY_SLOTS])
}

/// Crafting reservations stay in their physical inventory slots while the
/// remaining quantities participate in the same atomic chest transaction.
pub fn apply_gesture_reserved(
    player: &mut Inventory,
    halves: &mut [Inventory],
    gesture: ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    apply_filtered(player, halves, gesture, reserved, false)
}

pub(crate) fn apply_filtered(
    player: &mut Inventory,
    halves: &mut [Inventory],
    gesture: ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
    furnace: bool,
) -> bool {
    apply_filtered_kind(player, halves, gesture, reserved, u8::from(furnace))
}

pub(crate) fn apply_crusher_filtered(
    player: &mut Inventory,
    halves: &mut [Inventory],
    gesture: ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
) -> bool {
    apply_filtered_kind(player, halves, gesture, reserved, 2)
}

pub(crate) fn apply_filtered_kind(
    player: &mut Inventory,
    halves: &mut [Inventory],
    gesture: ContainerGesture,
    reserved: [u16; INVENTORY_SLOTS],
    machine: u8,
) -> bool {
    let size = match machine {
        1 => crate::furnace::SLOTS,
        2 => crate::crusher::SLOTS,
        3 => crate::mixer::SLOTS,
        4 => crate::brewing::SLOTS,
        5 => crate::campfire::SLOTS,
        _ => CHEST_SLOTS,
    };
    if !player.valid()
        || !(1..=2).contains(&halves.len())
        || match machine {
            1 => halves.len() != 1 || !halves.iter().all(crate::furnace::valid_inventory),
            2 => halves.len() != 1 || !halves.iter().all(crate::crusher::valid_inventory),
            3 => halves.len() != 1 || !halves.iter().all(crate::mixer::valid_inventory),
            4 => halves.len() != 1 || !halves.iter().all(crate::brewing::valid_inventory),
            5 => halves.len() != 1 || !halves.iter().all(crate::campfire::valid_inventory),
            _ => !halves.iter().all(valid_chest),
        }
    {
        return false;
    }
    if let ContainerGesture::Sort { order } = gesture {
        // Machine input/output/fuel slots retain their process-specific meaning.
        return machine == 0 && crate::inventory_sort::storage(player, halves, order, reserved);
    }
    if let ContainerGesture::SortBackpack { order } = gesture {
        return crate::inventory_sort::backpack(player, order, reserved);
    }
    let mut slots = vec![ItemStack::EMPTY; INVENTORY_SLOTS + halves.len() * size];
    slots[..player.slots.len()].copy_from_slice(&player.slots);
    for (index, half) in halves.iter().enumerate() {
        let start = INVENTORY_SLOTS + index * size;
        slots[start..start + half.slots.len()].copy_from_slice(&half.slots);
    }
    let before = slots.clone();
    for (slot, count) in slots.iter_mut().zip(reserved) {
        if count > slot.count {
            return false;
        }
        slot.count -= count;
        if slot.count == 0 {
            *slot = ItemStack::EMPTY;
        }
    }
    let mut view = View {
        machine,
        slots,
        cursor: player.cursor,
        reservations: std::array::from_fn(|index| {
            (reserved[index] > 0).then_some(ItemStack {
                count: reserved[index],
                ..before[index]
            })
        }),
    };
    if !view.apply(gesture) {
        return false;
    }
    for (index, count) in reserved.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let slot = &mut view.slots[index];
        if slot.valid() && slot.item != before[index].item
            || slot.count > before[index].item.max_stack().saturating_sub(count)
        {
            return false;
        }
        slot.item = before[index].item;
        slot.count += count;
    }
    let player_changed =
        view.cursor != player.cursor || view.slots[..INVENTORY_SLOTS] != before[..INVENTORY_SLOTS];
    let changed: Vec<_> = (0..halves.len())
        .map(|index| {
            let start = INVENTORY_SLOTS + index * size;
            view.slots[start..start + size] != before[start..start + size]
        })
        .collect();
    if (player_changed && player.revision == u64::MAX)
        || halves
            .iter()
            .zip(&changed)
            .any(|(half, changed)| *changed && half.revision == u64::MAX)
        || view.cursor.is_some_and(|stack| !stack.valid())
        || view
            .slots
            .iter()
            .any(|stack| *stack != ItemStack::EMPTY && !stack.valid())
    {
        return false;
    }
    if player_changed {
        player.slots = view.slots[..INVENTORY_SLOTS].to_vec();
        player.cursor = view.cursor;
        player.revision += 1;
    }
    for (index, (half, changed)) in halves.iter_mut().zip(changed).enumerate() {
        if changed {
            let start = INVENTORY_SLOTS + index * size;
            half.slots = view.slots[start..start + size].to_vec();
            half.revision += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests;
