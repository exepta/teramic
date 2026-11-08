use super::EditJournal;
use terra_core::{Inventory, containers::valid_chest};

impl EditJournal {
    /// Real owned stacks only: logistics filters are recipes, not inventories.
    /// A compound hearth also owns the attached campfire block itself.
    pub fn block_inventory_drops(&self, position: [i64; 3]) -> Vec<terra_core::ItemStack> {
        let mut stacks: Vec<_> = self
            .container_at(position)
            .into_iter()
            .flat_map(|inventory| &inventory.slots)
            .copied()
            .filter(|stack| stack.valid())
            .collect();
        if let Some(fire) = self.attached_campfire_at(position) {
            stacks.extend(
                fire.inventory
                    .slots
                    .iter()
                    .copied()
                    .filter(|stack| stack.valid()),
            );
            stacks.push(terra_core::ItemStack {
                item: terra_core::ItemKind::Block(terra_core::campfire::MATERIAL),
                count: 1,
            });
        }
        stacks
    }

    /// Reserve the complete removal before changing any cell of a multiblock.
    /// Callers include the block/attachment drops they will publish afterwards.
    pub fn preflight_inventory_removal(
        &self,
        positions: impl IntoIterator<Item = [i64; 3]>,
        extra_drops: usize,
    ) -> Result<(), String> {
        let positions: std::collections::BTreeSet<_> = positions.into_iter().collect();
        let count = positions.iter().try_fold(extra_drops, |count, position| {
            let stacks = self.block_inventory_drops(*position);
            if !stacks.is_empty()
                && position
                    .iter()
                    .any(|value| (*value as f64 + 0.5).abs() > 1_000_000.0)
            {
                return Err("Container drops are outside world bounds");
            }
            count
                .checked_add(stacks.len())
                .ok_or("Container drop count overflow")
        })?;
        let revisions = u64::try_from(count)
            .ok()
            .and_then(|count| count.checked_add(positions.len() as u64))
            .ok_or("Container revision overflow")?;
        let game = self.gameplay();
        if game
            .drops
            .len()
            .checked_add(count)
            .is_none_or(|count| count > terra_core::MAX_WORLD_DROPS)
            || game.next_drop.checked_add(count as u64).is_none()
            || game.revision.checked_add(revisions).is_none()
        {
            return Err("No space or revision capacity for container drops".into());
        }
        Ok(())
    }

    /// Called only after the block commit succeeds and its drops were preflighted.
    /// record() may already have retired some machine state; clear every owner
    /// map so neither persistence nor an open container can resurrect its items.
    pub(crate) fn finish_inventory_removal(
        &mut self,
        position: [i64; 3],
        stacks: Vec<terra_core::ItemStack>,
    ) {
        self.containers.remove(&position);
        self.remove_attached_campfire(position);
        self.remove_furnace(position);
        self.remove_crusher(position);
        self.remove_mixer(position);
        self.remove_cauldron(position);
        self.remove_campfire(position);
        for stack in stacks {
            let accepted = self.gameplay_mut().drop_stack(
                position.map(|value| value as f64 + 0.5),
                stack,
                [0.0, 1.8, 0.0],
                0.35,
            );
            debug_assert!(accepted, "container removal drops were preflighted");
        }
        self.gameplay_mut().revision += 1;
    }

    pub fn valid_container(&self, position: [i64; 3], inventory: &Inventory) -> bool {
        match self.block(position) {
            Some(29 | terra_core::train_parts::CARGO) => valid_chest(inventory),
            Some(218) => {
                valid_chest(inventory)
                    && inventory.slots.iter().all(|stack| {
                        stack.count == 0 || terra_core::furnace::fuel_seconds(stack.item).is_some()
                    })
            }
            Some(59) => {
                self.object_state(position).is_some_and(|s| !s.secondary)
                    && terra_core::furnace::valid_inventory(inventory)
            }
            Some(99) => {
                self.object_state(position).is_some_and(|s| !s.secondary)
                    && terra_core::crusher::valid_inventory(inventory)
            }
            Some(terra_core::mixer::MATERIAL) => {
                self.valid_mixer(position) && terra_core::mixer::valid_inventory(inventory)
            }
            Some(terra_core::brewing::MATERIAL) => {
                self.valid_cauldron(position) && terra_core::brewing::valid_inventory(inventory)
            }
            Some(terra_core::campfire::MATERIAL) => {
                self.valid_campfire(position) && terra_core::campfire::valid_inventory(inventory)
            }
            _ => false,
        }
    }
    pub fn furnaces(
        &self,
    ) -> impl Iterator<Item = ([i64; 3], &terra_core::furnace::FurnaceState)> + '_ {
        self.furnaces.iter().map(|(&p, s)| (p, s))
    }
    pub fn furnace_at(&self, position: [i64; 3]) -> Option<&terra_core::furnace::FurnaceState> {
        self.furnaces.get(&position)
    }
    pub fn set_furnace(
        &mut self,
        position: [i64; 3],
        state: terra_core::furnace::FurnaceState,
    ) -> Result<(), String> {
        if !state.valid() || !self.valid_container(position, &Inventory::default()) {
            return Err("Invalid or orphaned furnace state".into());
        }
        if self.furnaces.get(&position) != Some(&state) {
            let revision = self
                .gameplay()
                .revision
                .checked_add(1)
                .ok_or("Furnace revision overflow")?;
            self.furnaces.insert(position, state);
            self.gameplay_mut().revision = revision;
        }
        Ok(())
    }
    pub fn remove_furnace(&mut self, position: [i64; 3]) {
        self.furnaces.remove(&position);
    }
    pub fn crushers(
        &self,
    ) -> impl Iterator<Item = ([i64; 3], &terra_core::crusher::CrusherState)> + '_ {
        self.crushers
            .iter()
            .map(|(&position, state)| (position, state))
    }
    pub fn crusher_at(&self, position: [i64; 3]) -> Option<&terra_core::crusher::CrusherState> {
        self.crushers.get(&position)
    }
    /// Live drive state is never written into region records or terrain deltas.
    pub fn crusher_driven(&self, position: [i64; 3]) -> bool {
        self.crusher_driven.contains(&position)
    }
    pub fn replace_crusher_drives(&mut self, driven: std::collections::HashSet<[i64; 3]>) {
        self.crusher_driven = driven;
    }
    pub fn valid_crusher(&self, position: [i64; 3]) -> bool {
        self.block(position) == Some(99)
            && self.object_state(position).is_some_and(|state| {
                !state.secondary
                    && state.valid_for(position, 99)
                    && state.peer.is_none_or(|peer| {
                        self.block(peer) == Some(99)
                            && self.object_state(peer).is_some_and(|other| {
                                other.secondary
                                    && other.peer == Some(position)
                                    && other.facing == state.facing
                                    && other.valid_for(peer, 99)
                            })
                    })
            })
    }
    pub fn crusher_primary(&self, position: [i64; 3]) -> Option<[i64; 3]> {
        let selected = self.object_state(position)?;
        let anchor = if selected.secondary {
            selected.peer?
        } else {
            position
        };
        self.valid_crusher(anchor).then_some(anchor)
    }
    pub fn crusher_crank_position(&self, anchor: [i64; 3]) -> Option<[i64; 3]> {
        self.valid_crusher(anchor).then(|| {
            let right = self.object_state(anchor).unwrap_or_default().right();
            let distance = if self.object_state(anchor).unwrap_or_default().peer.is_some() {
                2
            } else {
                1
            };
            std::array::from_fn(|axis| anchor[axis] + right[axis] * distance)
        })
    }
    pub fn crank_anchor(&self, position: [i64; 3]) -> Option<[i64; 3]> {
        let state = self.object_state(position)?;
        let anchor = state.peer?;
        (self.block(position) == Some(terra_core::crusher::CRANK_MATERIAL)
            && state.valid_for(position, terra_core::crusher::CRANK_MATERIAL)
            && (self.rotary_crank_mount(anchor, state.facing) == Some(position)
                || self.valid_crusher(anchor)
                    && self.crusher_crank_position(anchor) == Some(position)
                    && self
                        .object_state(anchor)
                        .is_some_and(|crusher| crusher.facing == state.facing)))
        .then_some(anchor)
    }
    pub fn crushers_in_section(
        &self,
        section: terra_voxel::SectionPos,
    ) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.crusher_sections
            .get(&section)
            .into_iter()
            .flatten()
            .copied()
    }
    pub fn set_crusher(
        &mut self,
        position: [i64; 3],
        state: terra_core::crusher::CrusherState,
    ) -> Result<(), String> {
        let inventory = self.container_at(position).cloned().unwrap_or_default();
        self.commit_crusher(position, state, inventory).map(|_| ())
    }
    /// A completion consumes input and publishes its output in the same save revision.
    pub fn commit_crusher(
        &mut self,
        position: [i64; 3],
        state: terra_core::crusher::CrusherState,
        inventory: Inventory,
    ) -> Result<bool, String> {
        if !state.valid()
            || !self.valid_crusher(position)
            || !self.valid_container(position, &inventory)
        {
            return Err("Invalid or orphaned crusher state".into());
        }
        if self.crushers.get(&position) == Some(&state)
            && self.container_at(position) == Some(&inventory)
        {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Crusher revision overflow")?;
        self.crushers.insert(position, state);
        self.crusher_sections
            .entry(terra_voxel::global_to_section(position).0)
            .or_default()
            .insert(position);
        self.containers.insert(position, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
    pub fn remove_crusher(&mut self, position: [i64; 3]) {
        self.crushers.remove(&position);
        self.crusher_driven.remove(&position);
        let section = terra_voxel::global_to_section(position).0;
        if let Some(positions) = self.crusher_sections.get_mut(&section) {
            positions.remove(&position);
            if positions.is_empty() {
                self.crusher_sections.remove(&section);
            }
        }
    }
    pub fn can_record_furniture(&self, count: usize) -> bool {
        u64::try_from(count)
            .ok()
            .and_then(|count| self.next_furniture_order.checked_add(count))
            .is_some()
    }
    pub fn furniture_orders(&self) -> impl Iterator<Item = ([i64; 3], u64)> + '_ {
        self.furniture_order
            .iter()
            .map(|(&position, &order)| (position, order))
    }
    pub fn import_furniture_order(&mut self, position: [i64; 3], order: u64) -> Result<(), String> {
        if order == 0
            || position
                .iter()
                .any(|value| value.unsigned_abs() > 1_000_000)
            || !self
                .block(position)
                .is_some_and(|block| matches!(block, 28..=29))
            || self.furniture_order.contains_key(&position)
        {
            return Err("Invalid or orphaned furniture placement order".into());
        }
        self.furniture_order.insert(position, order);
        self.next_furniture_order = self.next_furniture_order.max(order);
        Ok(())
    }
    /// Old saves without ordering metadata use a stable coordinate tie-breaker.
    pub fn workbench_chest(&self, position: [i64; 3]) -> Option<([i64; 3], [i64; 3])> {
        if self.block(position) != Some(28) {
            return None;
        }
        let state = self.object_state(position)?;
        let anchor = if state.secondary {
            state.peer?
        } else {
            position
        };
        let primary = self.object_state(anchor)?;
        let second = primary.peer?;
        let secondary = self.object_state(second)?;
        if primary.secondary
            || self.block(anchor) != Some(28)
            || self.block(second) != Some(28)
            || !secondary.secondary
            || secondary.peer != Some(anchor)
            || secondary.facing != primary.facing
        {
            return None;
        }
        let chest = [anchor, second]
            .into_iter()
            .flat_map(|cell| {
                [[-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, 0, 1]]
                    .map(|offset| std::array::from_fn(|axis| cell[axis] + offset[axis]))
            })
            .filter(|cell| self.block(*cell) == Some(29))
            .min_by_key(|cell| (self.furniture_order.get(cell).copied().unwrap_or(0), *cell))?;
        Some((anchor, chest))
    }
    pub fn container_at(&self, position: [i64; 3]) -> Option<&Inventory> {
        self.containers.get(&position)
    }
    pub fn containers(&self) -> impl Iterator<Item = ([i64; 3], &Inventory)> + '_ {
        self.containers
            .iter()
            .map(|(&position, inventory)| (position, inventory))
    }
    pub fn container_nonempty(&self, position: [i64; 3]) -> bool {
        self.container_at(position)
            .is_some_and(|inventory| inventory.occupied_slots() > 0)
    }
    /// Commit all already-validated gesture results together, without cloning terrain state.
    pub fn commit_container_transaction(
        &mut self,
        player_uuid: terra_core::PlayerUuid,
        player: Inventory,
        halves: Vec<([i64; 3], Inventory)>,
    ) -> Result<(), String> {
        if player_uuid.is_nil()
            || !player.valid()
            || (!self.gameplay().inventories.contains_key(&player_uuid)
                && self.gameplay().inventories.len() >= 4096)
            || !(1..=2).contains(&halves.len())
            || halves
                .iter()
                .enumerate()
                .any(|(index, (position, inventory))| {
                    position
                        .iter()
                        .any(|value| value.unsigned_abs() > 1_000_000)
                        || !self.valid_container(*position, inventory)
                        || halves[..index]
                            .iter()
                            .any(|(previous, _)| previous == position)
                })
        {
            return Err("Invalid container transaction".into());
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Container revision overflow")?;
        for (position, inventory) in halves {
            self.containers.insert(position, inventory);
        }
        let gameplay = self.gameplay_mut();
        gameplay.inventories.insert(player_uuid, player);
        gameplay.revision = revision;
        Ok(())
    }
    /// Private world state: contents never travel with public terrain section deltas.
    pub fn set_container(
        &mut self,
        position: [i64; 3],
        inventory: Inventory,
    ) -> Result<bool, String> {
        if position
            .iter()
            .any(|value| value.unsigned_abs() > 1_000_000)
            || !self.valid_container(position, &inventory)
        {
            return Err(
                "Container requires a recorded chest and at most 18 valid cursor-free slots".into(),
            );
        }
        if self.container_at(position) == Some(&inventory) {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Container revision overflow")?;
        self.containers.insert(position, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
    pub fn remove_container(&mut self, position: [i64; 3]) -> Result<Option<Inventory>, String> {
        if !self.containers.contains_key(&position) {
            return Ok(None);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Container revision overflow")?;
        let inventory = self.containers.remove(&position);
        self.gameplay_mut().revision = revision;
        Ok(inventory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::ItemKind;
    use terra_voxel::global_to_section;

    #[test]
    fn workbench_uses_first_placed_horizontal_neighbor_from_either_cell() {
        let mut journal = EditJournal::default();
        let anchor = [-1, 20, -1];
        let second = [0, 20, -1];
        for (position, peer, secondary) in [(anchor, second, false), (second, anchor, true)] {
            journal.record(position, 28).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(super::super::BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let oldest = [0, 20, 0];
        let newer = [-2, 20, -1];
        journal.record([0, 21, -1], 29).unwrap(); // Vertical proximity is not adjacency.
        journal.record(oldest, 29).unwrap();
        journal.record(newer, 29).unwrap();
        for position in [anchor, second] {
            assert_eq!(journal.workbench_chest(position), Some((anchor, oldest)));
        }
        // Recoloring or attaching another half must not make the oldest chest newer.
        journal
            .set_object_state(
                oldest,
                Some(super::super::BlockObjectState {
                    wood: terra_core::WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(journal.workbench_chest(anchor), Some((anchor, oldest)));
        journal.record(oldest, 0).unwrap();
        assert_eq!(journal.workbench_chest(anchor), Some((anchor, newer)));
        journal.record(oldest, 29).unwrap();
        assert_eq!(journal.workbench_chest(anchor), Some((anchor, newer)));
        assert!(journal.workbench_chest([4, 20, 4]).is_none());
    }

    #[test]
    fn importing_terrain_deltas_never_invents_authoritative_furniture_placement_order() {
        let mut source = EditJournal::default();
        let chest = [1, 20, 3];
        source.record(chest, 29).unwrap();
        let delta = source.section_delta(global_to_section(chest).0);
        let mut restored = EditJournal::default();
        restored.import_delta(delta).unwrap();
        assert_eq!(restored.furniture_orders().count(), 0);
        let order = source.furniture_orders().next().unwrap().1;
        restored.import_furniture_order(chest, order).unwrap();
        assert_eq!(
            restored.furniture_orders().collect::<Vec<_>>(),
            vec![(chest, order)]
        );
        let later = [2, 20, 3];
        restored.record(later, 29).unwrap();
        assert_eq!(
            restored
                .furniture_orders()
                .find(|(position, _)| *position == later)
                .unwrap()
                .1,
            order + 1
        );
    }

    #[test]
    fn private_contents_survive_section_replacement_without_meshing_or_broadcast() {
        let position = [-1, 20, -1];
        let (section, _) = global_to_section(position);
        let mut journal = EditJournal::default();
        journal.record(position, 29).unwrap();
        let delta = journal.section_delta(section);
        let epoch = journal.mesh_epoch(section);
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Block(3), 17);
        assert!(journal.set_container(position, inventory.clone()).unwrap());
        assert_eq!(journal.mesh_epoch(section), epoch);
        assert_eq!(journal.section_delta(section), delta);
        assert!(journal.container_nonempty(position));
        let revision = journal.gameplay().revision;
        assert!(!journal.set_container(position, inventory.clone()).unwrap());
        assert_eq!(journal.gameplay().revision, revision);
        journal.import_delta(delta).unwrap();
        assert_eq!(journal.container_at(position), Some(&inventory));
    }

    #[test]
    fn invalid_containers_are_rejected_atomically() {
        let mut journal = EditJournal::default();
        let position = [1, 2, 3];
        assert!(
            journal
                .set_container(position, Inventory::default())
                .is_err()
        );
        journal.record(position, 29).unwrap();
        let inventory = Inventory {
            cursor: Some(terra_core::ItemStack {
                item: ItemKind::Stick,
                count: 1,
            }),
            ..Default::default()
        };
        assert!(journal.set_container(position, inventory).is_err());
        let inventory = Inventory {
            slots: vec![terra_core::ItemStack::EMPTY; 19],
            ..Default::default()
        };
        assert!(journal.set_container(position, inventory).is_err());
        assert_eq!(journal.containers().count(), 0);
        assert_eq!(journal.gameplay().revision, 0);
    }

    #[test]
    fn a_multi_half_transaction_preflights_every_half_and_bumps_only_metadata_once() {
        let mut journal = EditJournal::default();
        let positions = [[-1, 20, -1], [0, 20, -1]];
        let player_uuid = terra_core::PlayerUuid::new_v4();
        for position in positions {
            journal.record(position, 29).unwrap();
        }
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Stick, 17);
        let sections: Vec<_> = positions
            .map(|position| global_to_section(position).0)
            .into_iter()
            .map(|section| {
                (
                    section,
                    journal.section_delta(section),
                    journal.mesh_epoch(section),
                )
            })
            .collect();
        let halves = vec![
            (positions[0], inventory.clone()),
            ([4, 20, 4], inventory.clone()),
        ];
        assert!(
            journal
                .commit_container_transaction(player_uuid, inventory.clone(), halves)
                .is_err()
        );
        assert_eq!(journal.containers().count(), 0);
        assert!(journal.gameplay().inventories.is_empty());
        assert_eq!(journal.gameplay().revision, 0);
        let duplicate = vec![
            (positions[0], inventory.clone()),
            (positions[0], inventory.clone()),
        ];
        assert!(
            journal
                .commit_container_transaction(player_uuid, inventory.clone(), duplicate)
                .is_err()
        );
        journal
            .commit_container_transaction(
                player_uuid,
                inventory.clone(),
                positions
                    .into_iter()
                    .map(|p| (p, inventory.clone()))
                    .collect(),
            )
            .unwrap();
        assert_eq!(journal.containers().count(), 2);
        assert_eq!(journal.gameplay().inventories[&player_uuid], inventory);
        assert_eq!(journal.gameplay().revision, 1);
        for (section, delta, epoch) in sections {
            assert_eq!(journal.section_delta(section), delta);
            assert_eq!(journal.mesh_epoch(section), epoch);
        }
        journal.gameplay_mut().revision = u64::MAX;
        let mut changed = inventory.clone();
        changed.slots.clear();
        assert!(
            journal
                .commit_container_transaction(
                    player_uuid,
                    changed.clone(),
                    vec![(positions[0], changed)]
                )
                .is_err()
        );
        assert_eq!(journal.container_at(positions[0]), Some(&inventory));
        assert_eq!(journal.gameplay().inventories[&player_uuid], inventory);
    }
}
