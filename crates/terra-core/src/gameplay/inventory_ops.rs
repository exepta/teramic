use super::{INVENTORY_SLOTS, Inventory, ItemStack};

impl Inventory {
    /// Exact, atomic transfers for multiple visual crafting cells sharing inventory slots.
    pub fn transfer_cursor(
        &mut self,
        item: super::ItemKind,
        counts: [u16; INVENTORY_SLOTS],
        to_cursor: bool,
    ) -> bool {
        let total: u32 = counts.iter().map(|&n| u32::from(n)).sum();
        if !self.valid() || total == 0 || total > u32::from(item.max_stack()) {
            return false;
        }
        let total = total as u16;
        let held = self.cursor.map_or(0, |s| s.count);
        if self.cursor.is_some_and(|s| s.item != item)
            || (to_cursor && held.saturating_add(total) > item.max_stack())
            || (!to_cursor && held < total)
        {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let mut next = self.clone();
        for (slot, count) in counts.into_iter().enumerate().filter(|(_, n)| *n > 0) {
            let stack = next.slot(slot);
            if stack.is_some_and(|s| s.item != item) {
                return false;
            }
            let present = stack.map_or(0, |s| s.count);
            let amount = if to_cursor {
                let Some(amount) = present.checked_sub(count) else {
                    return false;
                };
                amount
            } else {
                let amount = present.saturating_add(count);
                if amount > item.max_stack() {
                    return false;
                }
                amount
            };
            next.set(
                slot,
                if amount == 0 {
                    ItemStack::EMPTY
                } else {
                    ItemStack {
                        item,
                        count: amount,
                    }
                },
            );
        }
        let count = if to_cursor {
            held + total
        } else {
            held - total
        };
        next.cursor = (count > 0).then_some(ItemStack { item, count });
        next.revision = revision;
        if !next.valid() {
            return false;
        }
        *self = next;
        true
    }

    /// Apply ordinary inventory gestures to the unreserved portion atomically.
    pub fn with_reserved(
        &mut self,
        reserved: [u16; INVENTORY_SLOTS],
        operation: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        if !self.valid() {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let mut next = self.clone();
        for (slot, &count) in reserved.iter().enumerate().filter(|(_, count)| **count > 0) {
            let Some(stack) = self.slot(slot) else {
                return false;
            };
            if count > stack.count {
                return false;
            }
            next.set(
                slot,
                if count == stack.count {
                    ItemStack::EMPTY
                } else {
                    ItemStack {
                        count: stack.count - count,
                        ..stack
                    }
                },
            );
        }
        if !operation(&mut next) || !next.valid() {
            return false;
        }
        for (slot, &count) in reserved.iter().enumerate().filter(|(_, count)| **count > 0) {
            let Some(original) = self.slot(slot) else {
                return false;
            };
            let current = next.slot(slot);
            if current.is_some_and(|stack| stack.item != original.item) {
                return false;
            }
            let restored = current.map_or(0, |stack| stack.count) + count;
            if restored > original.item.max_stack() {
                return false;
            }
            next.set(
                slot,
                ItemStack {
                    item: original.item,
                    count: restored,
                },
            );
        }
        next.revision = revision;
        *self = next;
        true
    }
    /// Move an explicit amount while leaving visually reserved crafting items in place.
    pub fn take_cursor(&mut self, slot: usize, count: u16) -> bool {
        if self.cursor.is_some() || count == 0 || slot >= INVENTORY_SLOTS {
            return false;
        }
        let Some(stack) = self.slot(slot) else {
            return false;
        };
        if count > stack.count {
            return false;
        }
        self.set(
            slot,
            if count == stack.count {
                ItemStack::EMPTY
            } else {
                ItemStack {
                    count: stack.count - count,
                    ..stack
                }
            },
        );
        self.cursor = Some(ItemStack { count, ..stack });
        self.changed();
        true
    }
    pub fn craft_cursor(&mut self, cells: [Option<u8>; 4]) -> bool {
        crate::crafting::RecipeBook::builtins().craft_cells(
            self,
            cells,
            crate::crafting::CraftDestination::Cursor,
        )
    }
    pub fn return_cursor(&mut self) -> bool {
        let Some(stack) = self.cursor else {
            return false;
        };
        let revision = self.revision;
        let remaining = self.insert(stack.item, stack.count);
        if remaining == stack.count {
            return false;
        }
        self.cursor = (remaining > 0).then_some(ItemStack {
            count: remaining,
            ..stack
        });
        self.revision = revision.saturating_add(1);
        true
    }
    pub fn creative_cursor(&mut self, stack: ItemStack) -> bool {
        if !stack.valid() {
            return false;
        }
        self.cursor = Some(stack);
        self.changed();
        true
    }

    pub fn creative_quick_move(&mut self, stack: ItemStack) -> bool {
        if !stack.valid() {
            return false;
        }
        let mut remaining = stack.count;
        for range in [0..9, 9..INVENTORY_SLOTS] {
            for occupied in [false, true] {
                for slot in range.clone() {
                    let existing = self.slot(slot);
                    if existing.is_some() != occupied
                        || existing.is_some_and(|s| s.item != stack.item)
                    {
                        continue;
                    }
                    let present = existing.map_or(0, |s| s.count);
                    let amount = remaining.min(stack.item.max_stack() - present);
                    if amount == 0 {
                        continue;
                    }
                    self.set(
                        slot,
                        ItemStack {
                            item: stack.item,
                            count: present + amount,
                        },
                    );
                    remaining -= amount;
                }
            }
        }
        if remaining == stack.count {
            return false;
        }
        self.changed();
        true
    }

    /// Primary clicks move whole stacks; secondary clicks split or place one.
    pub fn click(&mut self, slot: usize, secondary: bool) -> bool {
        if slot >= INVENTORY_SLOTS {
            return false;
        }
        let target = self.slot(slot);
        match (self.cursor, target) {
            (None, None) => return false,
            (None, Some(mut stack)) => {
                let count = if secondary {
                    stack.count.div_ceil(2)
                } else {
                    stack.count
                };
                self.cursor = Some(ItemStack { count, ..stack });
                stack.count -= count;
                self.set(
                    slot,
                    if stack.count == 0 {
                        ItemStack::EMPTY
                    } else {
                        stack
                    },
                );
            }
            (Some(mut cursor), target) if target.is_none_or(|s| s.item == cursor.item) => {
                let present = target.map_or(0, |s| s.count);
                let amount = (if secondary { 1 } else { cursor.count })
                    .min(cursor.item.max_stack() - present);
                if amount == 0 {
                    return false;
                }
                self.set(
                    slot,
                    ItemStack {
                        item: cursor.item,
                        count: present + amount,
                    },
                );
                cursor.count -= amount;
                self.cursor = (cursor.count > 0).then_some(cursor);
            }
            (Some(cursor), Some(target)) if !secondary => {
                self.set(slot, cursor);
                self.cursor = Some(target);
            }
            _ => return false,
        }
        self.changed();
        true
    }

    pub fn drag(&mut self, slots: [Option<u8>; INVENTORY_SLOTS], secondary: bool) -> bool {
        let Some(mut cursor) = self.cursor else {
            return false;
        };
        let mut targets = Vec::new();
        for slot in slots.into_iter().flatten().map(usize::from) {
            if slot >= INVENTORY_SLOTS || targets.contains(&slot) {
                return false;
            }
            if self
                .slot(slot)
                .is_none_or(|s| s.item == cursor.item && s.count < cursor.item.max_stack())
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
                .min(cursor.item.max_stack() - present);
            if moved > 0 {
                self.set(
                    slot,
                    ItemStack {
                        item: cursor.item,
                        count: present + moved,
                    },
                );
                cursor.count -= moved;
            }
        }
        if cursor.count == original {
            return false;
        }
        self.cursor = (cursor.count > 0).then_some(cursor);
        self.changed();
        true
    }

    pub fn quick_move(&mut self, slot: usize, count: u16) -> bool {
        let Some(source) = self.slot(slot) else {
            return false;
        };
        if slot >= INVENTORY_SLOTS || count == 0 {
            return false;
        }
        let targets = if slot < 9 { 9..INVENTORY_SLOTS } else { 0..9 };
        let mut remaining = count.min(source.count);
        let original = remaining;
        for occupied in [true, false] {
            for target in targets.clone() {
                let existing = self.slot(target);
                if existing.is_some() != occupied || existing.is_some_and(|s| s.item != source.item)
                {
                    continue;
                }
                let present = existing.map_or(0, |s| s.count);
                let moved = remaining.min(source.item.max_stack() - present);
                if moved == 0 {
                    continue;
                }
                self.set(
                    target,
                    ItemStack {
                        item: source.item,
                        count: present + moved,
                    },
                );
                remaining -= moved;
            }
        }
        let moved = original - remaining;
        if moved == 0 {
            return false;
        }
        self.set(
            slot,
            if moved == source.count {
                ItemStack::EMPTY
            } else {
                ItemStack {
                    count: source.count - moved,
                    ..source
                }
            },
        );
        self.changed();
        true
    }

    pub fn collect(&mut self, slot: usize) -> bool {
        let Some(item) = self
            .cursor
            .map(|s| s.item)
            .or_else(|| self.slot(slot).map(|s| s.item))
        else {
            return false;
        };
        if slot >= INVENTORY_SLOTS {
            return false;
        }
        let mut cursor = self.cursor.unwrap_or(ItemStack { item, count: 0 });
        let original = cursor.count;
        // Collect partial stacks first, leaving full stacks intact where possible.
        for full in [false, true] {
            for index in 0..self.slots.len() {
                let Some(mut stack) = self.slot(index) else {
                    continue;
                };
                if stack.item != item || (stack.count == item.max_stack()) != full {
                    continue;
                }
                let amount = stack.count.min(item.max_stack() - cursor.count);
                stack.count -= amount;
                cursor.count += amount;
                self.set(
                    index,
                    if stack.count == 0 {
                        ItemStack::EMPTY
                    } else {
                        stack
                    },
                );
            }
        }
        if cursor.count == original {
            return false;
        }
        self.cursor = Some(cursor);
        self.changed();
        true
    }

    pub fn collect_from(&mut self, slot: usize) -> bool {
        if slot >= INVENTORY_SLOTS {
            return false;
        }
        let Some(mut cursor) = self.cursor else {
            return self.click(slot, false);
        };
        let Some(mut source) = self.slot(slot) else {
            return false;
        };
        if source.item != cursor.item || cursor.count >= cursor.item.max_stack() {
            return false;
        }
        let amount = source.count.min(cursor.item.max_stack() - cursor.count);
        source.count -= amount;
        cursor.count += amount;
        self.cursor = Some(cursor);
        self.set(
            slot,
            if source.count == 0 {
                ItemStack::EMPTY
            } else {
                source
            },
        );
        self.changed();
        true
    }

    pub fn wheel(&mut self, slot: usize, to_cursor: bool) -> bool {
        if slot >= INVENTORY_SLOTS {
            return false;
        }
        let Some(mut cursor) = self.cursor else {
            if !to_cursor {
                return self.quick_move(slot, 1);
            }
            let Some(target) = self.slot(slot) else {
                return false;
            };
            if target.count >= target.item.max_stack() {
                return false;
            }
            let mut source_range = if slot < 9 { 9..INVENTORY_SLOTS } else { 0..9 };
            let source = source_range
                .find(|&source| self.slot(source).is_some_and(|s| s.item == target.item));
            return source.is_some_and(|source| self.move_slot(source, slot, 1));
        };
        if !to_cursor {
            return self.click(slot, true);
        }
        let Some(mut source) = self.slot(slot) else {
            return false;
        };
        if source.item != cursor.item || cursor.count >= cursor.item.max_stack() {
            return false;
        }
        source.count -= 1;
        cursor.count += 1;
        self.cursor = Some(cursor);
        self.set(
            slot,
            if source.count == 0 {
                ItemStack::EMPTY
            } else {
                source
            },
        );
        self.changed();
        true
    }

    pub fn take_stack(&mut self, slot: Option<usize>, all: bool) -> Option<ItemStack> {
        let mut source = match slot {
            Some(index) if index < INVENTORY_SLOTS => self.slot(index)?,
            Some(_) => return None,
            None => self.cursor?,
        };
        let result = ItemStack {
            count: if all { source.count } else { 1 },
            ..source
        };
        source.count -= result.count;
        match slot {
            Some(index) => self.set(
                index,
                if source.count == 0 {
                    ItemStack::EMPTY
                } else {
                    source
                },
            ),
            None => self.cursor = (source.count > 0).then_some(source),
        }
        self.changed();
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ItemKind;
    use crate::STACK_LIMIT;

    #[test]
    fn player_capacity_is_27_and_legacy_rows_recover_without_losing_items() {
        let player = crate::PlayerUuid::from_u128(1);
        let stack = ItemStack {
            item: ItemKind::Block(3),
            count: STACK_LIMIT,
        };
        let old = Inventory {
            slots: vec![stack; 45],
            ..Default::default()
        };
        assert!(!old.valid());
        assert!(old.valid_saved_player());
        let mut state = crate::GameplayState::default();
        state.inventories.insert(player, old);
        assert!(state.recover_player_inventory(player, [12.0, 30.0, -8.0]));
        let bag = &state.inventories[&player];
        assert!(bag.valid());
        assert_eq!(bag.slots.len(), 27);
        assert_eq!(state.drops.len(), 18);
        assert!(
            state
                .drops
                .iter()
                .all(|drop| drop.position == [12.0, 30.0, -8.0])
        );
        assert_eq!(
            bag.count(stack.item) + state.drops.iter().map(|drop| drop.stack.count).sum::<u16>(),
            45 * STACK_LIMIT
        );
        assert!(
            !state
                .inventories
                .get_mut(&player)
                .unwrap()
                .creative_set(27, stack)
        );
        let before = state.clone();
        assert!(state.recover_player_inventory(player, [12.0, 30.0, -8.0]));
        assert_eq!(state, before);
    }

    #[test]
    fn recovery_merges_legacy_items_and_rejects_unrecoverable_rows_atomically() {
        let player = crate::PlayerUuid::from_u128(2);
        let item = ItemKind::Block(3);
        let mut old = Inventory {
            slots: vec![ItemStack::EMPTY; 45],
            ..Default::default()
        };
        old.slots[0] = ItemStack {
            item,
            count: STACK_LIMIT - 1,
        };
        old.slots[44] = ItemStack { item, count: 2 };
        let mut state = crate::GameplayState::default();
        state.inventories.insert(player, old);
        assert!(state.recover_player_inventory(player, [0.0; 3]));
        assert_eq!(
            state.inventories[&player].slot(0).unwrap().count,
            STACK_LIMIT
        );
        assert_eq!(state.inventories[&player].slot(1).unwrap().count, 1);
        assert!(state.drops.is_empty());
        state.inventories.get_mut(&player).unwrap().slots = vec![
            ItemStack {
                item,
                count: STACK_LIMIT
            };
            45
        ];
        for _ in 0..crate::MAX_WORLD_DROPS {
            assert!(state.drop_one([0.0; 3], item));
        }
        let before = state.clone();
        assert!(!state.recover_player_inventory(player, [0.0; 3]));
        assert_eq!(state, before);
    }

    #[test]
    fn saved_inventories_keep_their_items_and_the_last_row_supports_real_gestures() {
        let mut old = Inventory::default();
        for slot in 0..26 {
            assert!(old.creative_set(
                slot,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 1
                }
            ));
        }
        let encoded = serde_json::to_vec(&old).unwrap();
        let mut restored: Inventory = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored, old);
        assert!(restored.valid());
        assert!(restored.click(0, false));
        assert!(restored.click(26, false));
        assert_eq!(restored.slot(26).unwrap().count, 1);
        assert!(restored.quick_move(26, 1));
        assert!(restored.slot(26).is_none());
        assert_eq!(resources(&restored, ItemKind::Block(3)), 26);
    }

    fn resources(bag: &Inventory, item: ItemKind) -> u16 {
        bag.count(item) + bag.cursor.filter(|s| s.item == item).map_or(0, |s| s.count)
    }

    #[test]
    fn cursor_split_place_swap_collect_and_drag_conserve_items() {
        let stone = ItemKind::Block(3);
        let dirt = ItemKind::Block(2);
        let mut bag = Inventory::default();
        bag.insert(stone, 127);
        bag.insert(dirt, 9);
        assert!(bag.click(0, true));
        assert_eq!(bag.cursor.unwrap().count, 64);
        assert_eq!(bag.slot(0).unwrap().count, 63);
        assert!(bag.click(2, true));
        assert_eq!(bag.slot(2).unwrap().count, 1);
        let mut targets = [None; INVENTORY_SLOTS];
        targets[..3].copy_from_slice(&[Some(3), Some(4), Some(5)]);
        assert!(bag.drag(targets, false));
        assert!(bag.cursor.is_none());
        assert_eq!(bag.slot(3).unwrap().count, 21);
        assert_eq!(resources(&bag, stone), 127);
        assert!(bag.click(1, false));
        assert!(bag.click(2, false));
        assert_eq!(bag.cursor.unwrap().item, stone);
        assert_eq!(bag.slot(2).unwrap().item, dirt);
        assert!(bag.collect(0));
        assert_eq!(bag.cursor.unwrap().count, 127);
        assert!(bag.return_cursor());
        assert!(bag.cursor.is_none());
        assert_eq!(resources(&bag, stone), 127);
        assert_eq!(resources(&bag, dirt), 9);
        assert!(bag.valid());
    }

    #[test]
    fn duplicate_drag_targets_are_atomic_and_secondary_drag_places_one_per_slot() {
        let stone = ItemKind::Block(3);
        let mut bag = Inventory::default();
        bag.creative_cursor(ItemStack {
            item: stone,
            count: 7,
        });
        let before = bag.clone();
        let mut targets = [None; INVENTORY_SLOTS];
        targets[..2].copy_from_slice(&[Some(0), Some(0)]);
        assert!(!bag.drag(targets, false));
        assert_eq!(bag, before);
        targets[..3].copy_from_slice(&[Some(0), Some(1), Some(2)]);
        assert!(bag.drag(targets, true));
        assert_eq!(bag.cursor.unwrap().count, 4);
        assert!(bag.slots.iter().all(|s| s.count == 1));
        let encoded = toml::to_string(&bag).unwrap();
        assert_eq!(toml::from_str::<Inventory>(&encoded).unwrap(), bag);
    }

    #[test]
    fn quick_moves_wheel_and_drop_keep_stable_indices_and_stack_limits() {
        let stone = ItemKind::Block(3);
        let mut bag = Inventory::default();
        bag.creative_set(
            0,
            ItemStack {
                item: stone,
                count: 128,
            },
        );
        assert!(bag.quick_move(0, STACK_LIMIT));
        assert!(bag.slot(0).is_none());
        assert_eq!(bag.slot(9).unwrap().count, 128);
        assert!(bag.wheel(9, false));
        assert_eq!(bag.slot(0).unwrap().count, 1);
        assert!(bag.click(0, false));
        assert!(bag.wheel(9, true));
        assert_eq!(bag.cursor.unwrap().count, 2);
        let dropped = bag.take_stack(None, false).unwrap();
        assert_eq!(dropped.count, 1);
        assert_eq!(resources(&bag, stone) + dropped.count, 128);
        assert!(bag.valid());
    }

    #[test]
    fn crafting_to_cursor_consumes_ingredients_once_and_rejects_overflow_atomically() {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(6), 2);
        assert!(bag.craft_cursor([Some(0), None, None, None]));
        assert_eq!(bag.count(ItemKind::Block(6)), 1);
        assert_eq!(
            bag.cursor,
            Some(ItemStack {
                item: ItemKind::Block(16),
                count: 4
            })
        );
        bag.cursor.as_mut().unwrap().count = 126;
        let before = bag.clone();
        assert!(!bag.craft_cursor([Some(0), None, None, None]));
        assert_eq!(bag, before);
    }

    #[test]
    fn repeated_creative_quick_moves_choose_slots_on_the_authoritative_inventory() {
        let item = ItemKind::Block(3);
        let mut bag = Inventory::default();
        for count in 1..=10 {
            assert!(bag.creative_quick_move(ItemStack { item, count: 128 }));
            assert_eq!(bag.count(item), count * 128);
        }
        assert!(bag.slots[..9].iter().all(|s| s.count == STACK_LIMIT));
        assert_eq!(bag.slot(9).unwrap().count, 128);
    }

    #[test]
    fn creative_browser_grants_choose_empty_hotbar_slots_before_merging_or_backpack() {
        let item = ItemKind::Block(3);
        let mut bag = Inventory::default();
        bag.creative_set(0, ItemStack { item, count: 1 });
        bag.creative_set(9, ItemStack { item, count: 1 });
        assert!(bag.creative_quick_move(ItemStack { item, count: 1 }));
        assert_eq!(bag.slot(0).unwrap().count, 1);
        assert_eq!(bag.slot(1), Some(ItemStack { item, count: 1 }));
        assert_eq!(bag.slot(9).unwrap().count, 1);
    }

    #[test]
    fn explicit_cursor_take_leaves_reserved_items_and_rejects_invalid_transactions_atomically() {
        let item = ItemKind::Block(16);
        let mut bag = Inventory::default();
        bag.insert(item, 20);
        let before = bag.clone();
        for count in [0, 21, u16::MAX] {
            assert!(!bag.take_cursor(0, count));
            assert_eq!(bag, before);
        }
        assert!(bag.take_cursor(0, 18));
        assert_eq!(bag.slot(0), Some(ItemStack { item, count: 2 }));
        assert_eq!(bag.cursor, Some(ItemStack { item, count: 18 }));
        assert_eq!(bag.revision, before.revision + 1);
        let before = bag.clone();
        assert!(!bag.take_cursor(0, 2));
        assert_eq!(bag, before);
    }

    #[test]
    fn protected_outgoing_gestures_keep_every_reserved_stack_and_wood_variant() {
        let birch = ItemKind::for_wood(29, crate::WoodSpecies::Birch);
        let spruce = ItemKind::for_wood(29, crate::WoodSpecies::Spruce);
        let operations: [fn(&mut Inventory) -> bool; 4] = [
            |bag| bag.quick_move(0, STACK_LIMIT),
            |bag| bag.collect(0),
            |bag| bag.collect_from(0),
            |bag| bag.wheel(0, true),
        ];
        for operation in operations {
            let mut bag = Inventory::default();
            for (slot, item, count) in [(0, birch, 20), (1, birch, 10), (2, spruce, 12)] {
                bag.creative_set(slot, ItemStack { item, count });
            }
            bag.cursor = Some(ItemStack {
                item: birch,
                count: 1,
            });
            let revision = bag.revision;
            let mut reserved = [0; INVENTORY_SLOTS];
            reserved[0] = 4;
            reserved[1] = 3;
            reserved[2] = 5;
            assert!(bag.with_reserved(reserved, operation));
            for (slot, item, count) in [(0, birch, 4), (1, birch, 3), (2, spruce, 5)] {
                let stack = bag.slot(slot).unwrap();
                assert_eq!(stack.item, item);
                assert!(stack.count >= count);
            }
            assert_eq!(
                bag.count(birch)
                    + bag
                        .cursor
                        .filter(|stack| stack.item == birch)
                        .map_or(0, |stack| stack.count),
                31
            );
            assert_eq!(bag.count(spruce), 12);
            assert_eq!(bag.revision, revision + 1);
            assert!(bag.valid());
        }
    }

    #[test]
    fn protected_moves_reject_invalid_reservations_and_hidden_target_conflicts_atomically() {
        let birch = ItemKind::for_wood(29, crate::WoodSpecies::Birch);
        let spruce = ItemKind::for_wood(29, crate::WoodSpecies::Spruce);
        let mut bag = Inventory::default();
        bag.creative_set(
            0,
            ItemStack {
                item: birch,
                count: 4,
            },
        );
        bag.creative_set(
            9,
            ItemStack {
                item: spruce,
                count: 128,
            },
        );
        let before = bag.clone();
        let mut reserved = [0; INVENTORY_SLOTS];
        reserved[0] = 5;
        assert!(!bag.with_reserved(reserved, |bag| bag.collect(9)));
        assert_eq!(bag, before);
        reserved[0] = 4;
        reserved[26] = 1;
        assert!(!bag.with_reserved(reserved, |bag| bag.collect(9)));
        assert_eq!(bag, before);
        reserved[26] = 0;
        assert!(!bag.with_reserved(reserved, |bag| bag.quick_move(9, 128)));
        assert_eq!(bag, before);
        bag.creative_set(
            9,
            ItemStack {
                item: birch,
                count: 128,
            },
        );
        let before = bag.clone();
        assert!(!bag.with_reserved(reserved, |bag| bag.quick_move(9, 128)));
        assert_eq!(bag, before);
        assert!(!bag.with_reserved(reserved, |bag| {
            bag.cursor = Some(ItemStack {
                item: birch,
                count: 1,
            });
            false
        }));
        assert_eq!(bag, before);
        assert!(!bag.with_reserved(reserved, |bag| {
            bag.cursor = Some(ItemStack {
                item: birch,
                count: 0,
            });
            true
        }));
        assert_eq!(bag, before);
    }

    #[test]
    fn held_left_sweep_collects_only_the_visited_matching_slot() {
        let item = ItemKind::Block(3);
        let mut bag = Inventory::default();
        bag.creative_set(0, ItemStack { item, count: 100 });
        bag.creative_set(1, ItemStack { item, count: 40 });
        bag.creative_set(2, ItemStack { item, count: 30 });
        bag.creative_set(
            3,
            ItemStack {
                item: ItemKind::Block(2),
                count: 8,
            },
        );
        assert!(bag.click(0, false));
        let before = bag.clone();
        assert!(!bag.collect_from(3));
        assert_eq!(bag, before);
        assert!(bag.collect_from(1));
        assert_eq!(bag.cursor.unwrap().count, 128);
        assert_eq!(bag.slot(1).unwrap().count, 12);
        assert_eq!(bag.slot(2).unwrap().count, 30);
        assert_eq!(bag.slot(3).unwrap().count, 8);
        assert_eq!(bag.count(item) + bag.cursor.unwrap().count, 170);
    }

    #[test]
    fn exact_cursor_transfers_are_atomic_bounded_and_conserve_items() {
        let item = ItemKind::Block(16);
        let mut bag = Inventory {
            cursor: Some(ItemStack { item, count: 11 }),
            ..Default::default()
        };
        let mut counts = [0; INVENTORY_SLOTS];
        counts[0] = 4;
        counts[1] = 6;
        assert!(bag.transfer_cursor(item, counts, false));
        assert_eq!(bag.cursor.unwrap().count, 1);
        assert_eq!(bag.slot(0).unwrap().count, 4);
        assert_eq!(bag.slot(1).unwrap().count, 6);
        let before = bag.clone();
        assert!(!bag.transfer_cursor(item, counts, false));
        assert_eq!(bag, before);
        counts[1] = 7;
        assert!(!bag.transfer_cursor(item, counts, true));
        assert_eq!(
            bag, before,
            "One invalid source must roll back all preceding sources"
        );
        counts[1] = 6;
        assert!(!bag.transfer_cursor(ItemKind::Block(17), counts, true));
        assert_eq!(bag, before);
        assert!(bag.transfer_cursor(item, counts, true));
        assert_eq!(bag.cursor.unwrap().count, 11);
        assert_eq!(bag.count(item), 0);
        assert!(bag.valid());
        counts.fill(u16::MAX);
        let before = bag.clone();
        assert!(!bag.transfer_cursor(item, counts, true));
        assert!(!bag.transfer_cursor(item, [0; INVENTORY_SLOTS], false));
        assert_eq!(bag, before);
        counts.fill(0);
        counts[0] = 1;
        bag.creative_set(
            0,
            ItemStack {
                item,
                count: STACK_LIMIT,
            },
        );
        let before = bag.clone();
        assert!(!bag.transfer_cursor(item, counts, false));
        assert_eq!(bag, before);
    }
}
