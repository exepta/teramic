use super::EditJournal;
use terra_core::{ItemStack, containers::CHEST_SLOTS, logistics};

type Cell = [i64; 3];

fn orient_export(journal: &mut EditJournal) {
    if let Some(drop) = journal.gameplay_mut().drops.last_mut() {
        drop.grounded = true;
        drop.rotation = [0.0, 0.0, 0.0, 1.0];
        drop.angular_velocity = [0.0; 3];
    }
}

fn chest_targets(journal: &EditJournal, chest: Cell) -> Vec<Cell> {
    let mut targets = vec![chest];
    if journal.block(chest) == Some(29)
        && let Some(state) = journal.object_state(chest)
        && let Some(peer) = state.peer
        && journal.block(peer) == Some(29)
        && journal.object_state(peer).is_some_and(|other| {
            other.peer == Some(chest)
                && other.facing == state.facing
                && other.secondary != state.secondary
        })
    {
        targets.push(peer);
    }
    targets
}

fn extraction_roll(sequence: u64, source: Cell, count: usize) -> usize {
    // Saved drop IDs make retries/reloads stable without an unbounded per-port RNG.
    let mut value = source.into_iter().fold(sequence, |value, coordinate| {
        value.rotate_left(21) ^ (coordinate as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
    });
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    ((u128::from(value) * count as u128) >> 64) as usize
}

impl EditJournal {
    pub fn has_logistics(&self) -> bool {
        !self.logistics_sections.is_empty()
    }

    pub fn logistics_in_section(
        &self,
        section: terra_voxel::SectionPos,
    ) -> impl Iterator<Item = Cell> + '_ {
        self.logistics_sections
            .get(&section)
            .into_iter()
            .flatten()
            .copied()
    }

    /// Resume bounded sparse discovery without rescanning the section's earlier objects.
    pub fn logistics_in_section_after(
        &self,
        section: terra_voxel::SectionPos,
        after: Option<Cell>,
    ) -> impl Iterator<Item = Cell> + '_ {
        use std::ops::Bound::{Excluded, Unbounded};
        self.logistics_sections
            .get(&section)
            .into_iter()
            .flat_map(move |positions| {
                positions.range((after.map_or(Unbounded, Excluded), Unbounded))
            })
            .copied()
    }

    /// Machine outputs and their new visible drop are committed together. No
    /// private machine cursor or input/fuel slot is ever exposed to a port.
    pub fn eject_machine_output(
        &mut self,
        source: Cell,
        position: [f64; 3],
    ) -> Result<bool, String> {
        self.eject_machine_output_filtered(source, position, logistics::FilterConfig::default())
    }

    fn eject_machine_output_filtered(
        &mut self,
        source: Cell,
        position: [f64; 3],
        filter: logistics::FilterConfig,
    ) -> Result<bool, String> {
        if !self
            .gameplay()
            .kingdoms
            .can_transport(source, position.map(|v| v.floor() as i64))
        {
            return Ok(false);
        }
        let material = self.block(source).ok_or("Machine output disappeared")?;
        let slot = logistics::source_output(material).ok_or("Not a machine output")?;
        if !self
            .object_state(source)
            .is_some_and(|state| !state.secondary)
            || !self.gameplay().can_drop()
        {
            return Ok(false);
        }
        let Some(mut inventory) = self.container_at(source).cloned() else {
            return Ok(false);
        };
        let Some(stack) = inventory.slot(slot) else {
            return Ok(false);
        };
        if !filter.accepts(stack.item) {
            return Ok(false);
        }
        let moved = stack.count.min(logistics::ITEMS_PER_TRANSFER);
        let inventory_revision = inventory
            .revision
            .checked_add(1)
            .ok_or("Output revision overflow")?;
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Automation revision overflow")?;
        inventory.slots[slot] = if moved == stack.count {
            ItemStack::EMPTY
        } else {
            ItemStack {
                count: stack.count - moved,
                ..stack
            }
        };
        inventory.revision = inventory_revision;
        if !self.valid_container(source, &inventory) {
            return Err("Invalid machine output inventory".into());
        }
        if !self.gameplay_mut().drop_stack(
            position,
            ItemStack {
                count: moved,
                ..stack
            },
            [0.0; 3],
            0.0,
        ) {
            return Ok(false);
        }
        orient_export(self);
        self.containers.insert(source, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }

    pub fn eject_inventory_output(
        &mut self,
        source: Cell,
        position: [f64; 3],
    ) -> Result<bool, String> {
        self.eject_inventory_output_filtered(source, position, logistics::FilterConfig::default())
    }

    pub fn eject_inventory_output_filtered(
        &mut self,
        source: Cell,
        position: [f64; 3],
        filter: logistics::FilterConfig,
    ) -> Result<bool, String> {
        self.eject_inventory_output_selected(source, position, filter, false)
    }

    /// Choose uniformly among eligible occupied chest slots, across both halves.
    /// Machine extraction remains output-only, independent of the selection mode.
    pub fn eject_inventory_output_selected(
        &mut self,
        source: Cell,
        position: [f64; 3],
        filter: logistics::FilterConfig,
        random: bool,
    ) -> Result<bool, String> {
        if !filter.valid() {
            return Err("Invalid automation filter".into());
        }
        if !self
            .block(source)
            .is_some_and(terra_core::containers::is_chest)
        {
            return self.eject_machine_output_filtered(source, position, filter);
        }
        if !self.gameplay().can_drop() {
            return Ok(false);
        }
        let targets: Vec<_> = chest_targets(self, source)
            .into_iter()
            .filter(|p| {
                self.gameplay()
                    .kingdoms
                    .can_transport(*p, position.map(|v| v.floor() as i64))
            })
            .collect();
        for target in &targets {
            if self
                .container_at(*target)
                .is_some_and(|inventory| !terra_core::containers::valid_chest(inventory))
            {
                return Err("Invalid export chest".into());
            }
        }
        let candidates = || {
            targets.iter().flat_map(|target| {
                let inventory = self.container_at(*target);
                (0..CHEST_SLOTS).filter_map(move |slot| {
                    inventory
                        .and_then(|inventory| inventory.slot(slot))
                        .filter(|stack| filter.accepts(stack.item))
                        .map(|stack| (*target, slot, stack))
                })
            })
        };
        let count = candidates().count();
        if count == 0 {
            return Ok(false);
        }
        let selected = if random {
            extraction_roll(self.gameplay().next_drop, source, count)
        } else {
            0
        };
        if let Some((target, slot, stack)) = candidates().nth(selected) {
            let mut inventory = self
                .container_at(target)
                .cloned()
                .ok_or("Export chest disappeared")?;
            let moved = stack.count.min(logistics::ITEMS_PER_TRANSFER);
            inventory.revision = inventory
                .revision
                .checked_add(1)
                .ok_or("Chest revision overflow")?;
            let revision = self
                .gameplay()
                .revision
                .checked_add(1)
                .ok_or("Automation revision overflow")?;
            inventory.slots[slot] = if moved == stack.count {
                ItemStack::EMPTY
            } else {
                ItemStack {
                    count: stack.count - moved,
                    ..stack
                }
            };
            if !self.gameplay_mut().drop_stack(
                position,
                ItemStack {
                    count: moved,
                    ..stack
                },
                [0.0; 3],
                0.0,
            ) {
                return Ok(false);
            }
            orient_export(self);
            self.containers.insert(target, inventory);
            self.gameplay_mut().revision = revision;
            return Ok(true);
        }
        Ok(false)
    }

    /// Only machine input/fuel slots accept transport. Outputs and cursors remain private.
    pub fn import_drop_to_inventory(&mut self, target: Cell, drop_id: u64) -> Result<bool, String> {
        self.import_drop_to_inventory_count(target, drop_id, u16::MAX)
    }

    pub fn import_drop_to_inventory_count(
        &mut self,
        target: Cell,
        drop_id: u64,
        limit: u16,
    ) -> Result<bool, String> {
        if limit == 0 {
            return Ok(false);
        }
        let material = self.block(target).unwrap_or(0);
        if terra_core::containers::is_chest(material) {
            return self.import_drop_to_chest_count(target, drop_id, limit);
        }
        if logistics::input_slots(material) == 0
            || self.object_state(target).is_none_or(|s| s.secondary)
        {
            return Ok(false);
        }
        let Some(index) = self.gameplay().drops.iter().position(|d| d.id == drop_id) else {
            return Ok(false);
        };
        if !self.gameplay().kingdoms.can_transport(
            self.gameplay().drops[index]
                .position
                .map(|v| v.floor() as i64),
            target,
        ) {
            return Ok(false);
        }
        let stack = self.gameplay().drops[index].stack;
        if !stack.valid() {
            return Ok(false);
        }
        let mut inventory = self.container_at(target).cloned().unwrap_or_default();
        if !self.valid_container(target, &inventory) {
            return Err("Invalid import machine".into());
        }
        let budget = stack.count.min(limit);
        let mut remaining = budget;
        for slot in 0..logistics::input_slots(material) {
            let capacity = logistics::input_capacity(material, slot, stack.item);
            let present = inventory.slot(slot);
            if present.is_some_and(|s| s.item != stack.item) {
                continue;
            }
            let count = present.map_or(0, |s| s.count);
            let moved = remaining.min(capacity.saturating_sub(count));
            if moved == 0 {
                continue;
            }
            inventory
                .slots
                .resize(inventory.slots.len().max(slot + 1), ItemStack::EMPTY);
            inventory.slots[slot] = ItemStack {
                item: stack.item,
                count: count + moved,
            };
            remaining -= moved;
        }
        if remaining == budget {
            return Ok(false);
        }
        inventory.revision = inventory
            .revision
            .checked_add(1)
            .ok_or("Machine revision overflow")?;
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Automation revision overflow")?;
        if !self.valid_container(target, &inventory) {
            return Err("Invalid imported machine state".into());
        }
        self.containers.insert(target, inventory);
        let left = stack.count - (budget - remaining);
        if left == 0 {
            self.gameplay_mut().drops.remove(index);
        } else {
            self.gameplay_mut().drops[index].stack.count = left;
        }
        self.gameplay_mut().revision = revision;
        Ok(true)
    }

    /// Insert into the selected physical chest, then its valid partner. Uses
    /// the real 18-slot capacity instead of the player's larger Inventory API.
    pub fn import_drop_to_chest(&mut self, chest: Cell, drop_id: u64) -> Result<bool, String> {
        self.import_drop_to_chest_count(chest, drop_id, u16::MAX)
    }

    fn import_drop_to_chest_count(
        &mut self,
        chest: Cell,
        drop_id: u64,
        limit: u16,
    ) -> Result<bool, String> {
        if !self
            .block(chest)
            .is_some_and(terra_core::containers::is_chest)
        {
            return Ok(false);
        }
        let Some(index) = self.gameplay().drops.iter().position(|d| d.id == drop_id) else {
            return Ok(false);
        };
        let origin = self.gameplay().drops[index]
            .position
            .map(|v| v.floor() as i64);
        let stack = self.gameplay().drops[index].stack;
        if !stack.valid() {
            return Ok(false);
        }
        let budget = stack.count.min(limit);
        let mut remaining = budget;
        let mut changes = Vec::new();
        for target in chest_targets(self, chest) {
            if !self.gameplay().kingdoms.can_transport(origin, target) {
                continue;
            }
            let mut inventory = self.container_at(target).cloned().unwrap_or_default();
            if !terra_core::containers::valid_chest(&inventory) {
                return Err("Invalid import chest".into());
            }
            let previous = remaining;
            for occupied in [true, false] {
                for slot in 0..CHEST_SLOTS {
                    let present = inventory.slot(slot);
                    if present.is_some() != occupied
                        || present.is_some_and(|s| s.item != stack.item)
                    {
                        continue;
                    }
                    let count = present.map_or(0, |s| s.count);
                    let moved = remaining.min(stack.item.max_stack() - count);
                    if moved == 0 {
                        continue;
                    }
                    inventory
                        .slots
                        .resize(inventory.slots.len().max(slot + 1), ItemStack::EMPTY);
                    inventory.slots[slot] = ItemStack {
                        item: stack.item,
                        count: count + moved,
                    };
                    remaining -= moved;
                }
            }
            if remaining != previous {
                inventory.revision = inventory
                    .revision
                    .checked_add(1)
                    .ok_or("Chest revision overflow")?;
                changes.push((target, inventory));
            }
        }
        if remaining == budget {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Automation revision overflow")?;
        for (target, inventory) in changes {
            self.containers.insert(target, inventory);
        }
        let left = stack.count - (budget - remaining);
        if left == 0 {
            self.gameplay_mut().drops.remove(index);
        } else {
            self.gameplay_mut().drops[index].stack.count = left;
        }
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{Inventory, ItemKind, MineralItem};

    fn furnace(journal: &mut EditJournal, source: Cell, count: u16) {
        let peer = [source[0], source[1] + 1, source[2]];
        for (position, other, secondary) in [(source, peer, false), (peer, source, true)] {
            journal.record(position, 59).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(other),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let mut inventory = Inventory {
            slots: vec![ItemStack::EMPTY; 3],
            ..Default::default()
        };
        inventory.slots[terra_core::furnace::OUTPUT] = ItemStack {
            item: ItemKind::Mineral(MineralItem::IronIngot),
            count,
        };
        journal.set_container(source, inventory).unwrap();
    }
    #[test]
    fn ornate_chest_automation_round_trip_is_lossless_and_stays_single_cell() {
        let mut journal = EditJournal::default();
        let chest = [-1, 2, -1];
        let neighbor = [0, 2, -1];
        let item = ItemKind::Block(3);
        for (position, material, count) in [
            (chest, terra_core::grave_blocks::ORNATE_CHEST, 8),
            (neighbor, 29, 11),
        ] {
            journal.record(position, material).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
            let mut contents = Inventory::default();
            contents.insert(item, count);
            journal.set_container(position, contents).unwrap();
        }
        assert!(
            journal
                .eject_inventory_output(chest, [-0.5, 2.0, 0.5])
                .unwrap()
        );
        assert_eq!(journal.container_at(chest).unwrap().count(item), 7);
        assert_eq!(journal.container_at(neighbor).unwrap().count(item), 11);
        let drop = journal.gameplay().drops[0].id;
        assert!(journal.import_drop_to_inventory(chest, drop).unwrap());
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(journal.container_at(chest).unwrap().count(item), 8);
        assert_eq!(journal.object_state(chest).unwrap().peer, None);
        assert_eq!(journal.container_at(neighbor).unwrap().count(item), 11);
    }

    #[test]
    fn output_to_belt_to_chest_preserves_items_and_survives_reload() {
        let mut journal = EditJournal::default();
        let source = [-16, 2, 15];
        furnace(&mut journal, source, 9);
        let chest = [-16, 2, 20];
        journal.record(chest, 29).unwrap();
        assert!(
            journal
                .eject_machine_output(source, [-15.5, 1.75, 17.5])
                .unwrap()
        );
        let drop = journal.gameplay().drops[0].clone();
        assert_eq!(drop.stack.count, 1);
        assert_eq!(
            journal
                .container_at(source)
                .unwrap()
                .slot(terra_core::furnace::OUTPUT)
                .unwrap()
                .count,
            8
        );
        assert!(journal.import_drop_to_chest(chest, drop.id).unwrap());
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(
            journal.container_at(chest).unwrap().slot(0).unwrap().count,
            1
        );
        let delta = journal
            .changed_sections()
            .map(|s| journal.section_delta(s))
            .collect::<Vec<_>>();
        let mut restored = EditJournal::default();
        for delta in delta {
            restored.import_delta(delta).unwrap();
        }
        for (p, inventory) in journal.containers() {
            restored.set_container(p, inventory.clone()).unwrap();
        }
        assert_eq!(restored.container_at(chest), journal.container_at(chest));
    }
    #[test]
    fn full_chest_and_revision_overflow_are_lossless() {
        let mut journal = EditJournal::default();
        let source = [0, 2, 0];
        furnace(&mut journal, source, 4);
        let chest = [0, 2, 3];
        journal.record(chest, 29).unwrap();
        journal
            .set_container(
                chest,
                Inventory {
                    slots: vec![
                        ItemStack {
                            item: ItemKind::Block(3),
                            count: 128
                        };
                        CHEST_SLOTS
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
        journal
            .eject_machine_output(source, [0.5, 1.75, 1.5])
            .unwrap();
        let before = journal.gameplay().clone();
        assert!(
            !journal
                .import_drop_to_chest(chest, before.drops[0].id)
                .unwrap()
        );
        assert_eq!(journal.gameplay(), &before);
        journal.set_container(chest, Inventory::default()).unwrap();
        journal.gameplay_mut().revision = u64::MAX;
        assert!(
            journal
                .import_drop_to_chest(chest, before.drops[0].id)
                .is_err()
        );
        assert!(journal.container_at(chest).unwrap().slots.is_empty());
        assert_eq!(journal.gameplay().drops, before.drops);
    }
    #[test]
    fn ports_rebuild_their_bounded_section_index_and_leave_it_on_removal() {
        let mut journal = EditJournal::default();
        let p = [-17, 3, 16];
        journal.record(p, logistics::HOPPER).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some([-16, 3, 16]),
                    ..Default::default()
                }),
            )
            .unwrap();
        let section = terra_voxel::global_to_section(p).0;
        let mut restored = EditJournal::default();
        restored
            .import_delta(journal.section_delta(section))
            .unwrap();
        assert_eq!(
            restored.logistics_in_section(section).collect::<Vec<_>>(),
            [p]
        );
        restored.record(p, 0).unwrap();
        assert_eq!(restored.logistics_in_section(section).count(), 0);
    }

    #[test]
    fn invalid_drop_position_and_exhausted_id_leave_the_output_untouched() {
        let mut journal = EditJournal::default();
        let source = [1, 2, 3];
        furnace(&mut journal, source, 8);
        let before = journal.container_at(source).unwrap().clone();
        assert!(
            !journal
                .eject_machine_output(source, [f64::NAN, 0.0, 0.0])
                .unwrap()
        );
        assert_eq!(journal.container_at(source), Some(&before));
        assert!(journal.gameplay().drops.is_empty());
        journal.gameplay_mut().next_drop = u64::MAX;
        assert!(
            !journal
                .eject_machine_output(source, [1.5, 1.75, 4.5])
                .unwrap()
        );
        assert_eq!(journal.container_at(source), Some(&before));
        assert!(journal.gameplay().drops.is_empty());
    }

    #[test]
    fn paired_chest_fills_the_free_partner_without_using_player_slot_capacity() {
        let mut journal = EditJournal::default();
        let a = [1, 2, 3];
        let b = [2, 2, 3];
        for (position, peer, secondary) in [(a, b, false), (b, a, true)] {
            journal.record(position, 29).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        journal
            .set_container(
                a,
                Inventory {
                    slots: vec![
                        ItemStack {
                            item: ItemKind::Block(3),
                            count: 128
                        };
                        CHEST_SLOTS
                    ],
                    ..Default::default()
                },
            )
            .unwrap();
        journal.gameplay_mut().drop_stack(
            [1.5, 1.75, 4.5],
            ItemStack {
                item: ItemKind::Block(2),
                count: 17,
            },
            [0.0; 3],
            0.0,
        );
        let id = journal.gameplay().next_drop;
        assert!(journal.import_drop_to_chest(a, id).unwrap());
        assert_eq!(journal.container_at(a).unwrap().slots.len(), CHEST_SLOTS);
        assert_eq!(journal.container_at(b).unwrap().slot(0).unwrap().count, 17);
        assert!(journal.gameplay().drops.is_empty());
    }
}

#[cfg(test)]
mod kingdom_access_tests {
    use super::*;
    use terra_core::{
        Inventory, ItemKind, PlayerUuid,
        kingdom::{DiplomaticRelation, DiplomaticTarget},
    };
    #[test]
    fn automatic_chest_exports_and_imports_recheck_relations_at_each_commit() {
        let mut journal = EditJournal::default();
        let a = PlayerUuid::from_u128(1);
        let b = PlayerUuid::from_u128(2);
        let source = [15, 20, 0];
        let destination = [16, 20, 0];
        journal.record(source, 29).unwrap();
        journal.record(destination, 29).unwrap();
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Block(3), 4);
        journal.set_container(source, inventory).unwrap();
        let first = journal
            .gameplay_mut()
            .kingdoms
            .found(a, [0, 20, 0], "First")
            .unwrap();
        let second = journal
            .gameplay_mut()
            .kingdoms
            .found(b, [16, 20, 0], "Second")
            .unwrap();
        assert!(
            !journal
                .eject_inventory_output(source, [16.5, 20.5, 0.5])
                .unwrap()
        );
        assert_eq!(
            journal
                .container_at(source)
                .unwrap()
                .count(ItemKind::Block(3)),
            4
        );
        journal
            .gameplay_mut()
            .kingdoms
            .set_relation(
                a,
                first,
                DiplomaticTarget::Player(second),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        assert!(
            !journal
                .eject_inventory_output(source, [16.5, 20.5, 0.5])
                .unwrap()
        );
        journal
            .gameplay_mut()
            .kingdoms
            .set_relation(
                b,
                second,
                DiplomaticTarget::Player(first),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        assert!(
            journal
                .eject_inventory_output(source, [16.5, 20.5, 0.5])
                .unwrap()
        );
        let drop = journal.gameplay().drops.last().unwrap().id;
        // Revoke after emission, before a later port's actual import.
        journal
            .gameplay_mut()
            .kingdoms
            .set_relation(
                b,
                second,
                DiplomaticTarget::Player(first),
                DiplomaticRelation::Hostile,
            )
            .unwrap();
        assert!(!journal.import_drop_to_chest(source, drop).unwrap());
        assert!(journal.gameplay().drops.iter().any(|d| d.id == drop));
        assert!(journal.import_drop_to_chest(destination, drop).unwrap());
        assert!(
            !journal
                .eject_inventory_output(source, [16.5, 20.5, 0.5])
                .unwrap()
        );
        journal.gameplay_mut().kingdoms.dissolve(a, first).unwrap();
        journal.gameplay_mut().kingdoms.dissolve(b, second).unwrap();
        assert!(
            journal
                .eject_inventory_output(destination, [15.5, 20.5, 0.5])
                .unwrap()
        );
    }
}
