use super::EditJournal;
use terra_core::{
    Inventory,
    brewing::CauldronState,
    campfire::{AttachedCampfire, CampfireState},
};

pub(super) const MAX_HEARTHS: usize = 4096;
impl EditJournal {
    pub fn has_attached_campfire(&self, position: [i64; 3]) -> bool {
        self.valid_cauldron(position)
            && self
                .object_state(position)
                .is_some_and(|state| terra_core::brewing::has_fire(state.facing))
    }
    pub fn attached_campfire_slot_available(&self, position: [i64; 3]) -> bool {
        self.attached_campfires.contains_key(&position)
            || self.attached_campfires.len() < MAX_HEARTHS
    }
    pub fn attached_campfires(&self) -> impl Iterator<Item = ([i64; 3], &AttachedCampfire)> + '_ {
        self.attached_campfires
            .iter()
            .map(|(&position, fire)| (position, fire))
    }
    pub fn attached_campfire_at(&self, position: [i64; 3]) -> Option<&AttachedCampfire> {
        self.attached_campfires.get(&position)
    }
    pub fn hearths(&self) -> impl Iterator<Item = ([i64; 3], &CampfireState)> + '_ {
        self.campfires().chain(
            self.attached_campfires()
                .map(|(position, fire)| (position, &fire.state)),
        )
    }
    pub fn hearth_at(&self, position: [i64; 3]) -> Option<&CampfireState> {
        self.campfire_at(position)
            .or_else(|| self.attached_campfire_at(position).map(|fire| &fire.state))
    }
    pub fn hearth_inventory_at(&self, position: [i64; 3]) -> Option<&Inventory> {
        if self.has_attached_campfire(position) {
            self.attached_campfire_at(position)
                .map(|fire| &fire.inventory)
        } else {
            self.container_at(position)
        }
    }
    pub fn remove_attached_campfire(&mut self, position: [i64; 3]) -> Option<AttachedCampfire> {
        self.attached_campfires.remove(&position)
    }
    pub fn commit_hearth(
        &mut self,
        position: [i64; 3],
        state: CampfireState,
        inventory: Inventory,
    ) -> Result<bool, String> {
        if self.has_attached_campfire(position) {
            self.commit_attached_campfire(position, AttachedCampfire { state, inventory })
        } else {
            self.commit_campfire(position, state, inventory)
        }
    }
    pub fn commit_attached_campfire(
        &mut self,
        position: [i64; 3],
        fire: AttachedCampfire,
    ) -> Result<bool, String> {
        if !self.has_attached_campfire(position)
            || !fire.valid()
            || !self.attached_campfire_slot_available(position)
        {
            return Err("Invalid, orphaned or over-capacity attached campfire".into());
        }
        if self.attached_campfire_at(position) == Some(&fire) {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Attached campfire revision overflow")?;
        self.attached_campfires.insert(position, fire);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
    pub fn commit_attached_container_transaction(
        &mut self,
        player_uuid: terra_core::PlayerUuid,
        player: Inventory,
        position: [i64; 3],
        inventory: Inventory,
    ) -> Result<(), String> {
        if player_uuid.is_nil()
            || !player.valid()
            || !terra_core::campfire::valid_inventory(&inventory)
            || !self.has_attached_campfire(position)
            || self.attached_campfire_at(position).is_none()
            || !self.gameplay().inventories.contains_key(&player_uuid)
                && self.gameplay().inventories.len() >= 4096
        {
            return Err("Invalid attached hearth transaction".into());
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Attached inventory revision overflow")?;
        self.attached_campfires
            .get_mut(&position)
            .unwrap()
            .inventory = inventory;
        let game = self.gameplay_mut();
        game.inventories.insert(player_uuid, player);
        game.revision = revision;
        Ok(())
    }
    pub fn hearth_slot_available(&self, position: [i64; 3], material: u16) -> bool {
        match material {
            terra_core::brewing::MATERIAL => {
                self.cauldrons.contains_key(&position) || self.cauldrons.len() < MAX_HEARTHS
            }
            terra_core::campfire::MATERIAL => {
                self.campfires.contains_key(&position) || self.campfires.len() < MAX_HEARTHS
            }
            _ => false,
        }
    }
    pub fn valid_cauldron(&self, position: [i64; 3]) -> bool {
        self.block(position) == Some(terra_core::brewing::MATERIAL)
            && self
                .object_state(position)
                .is_some_and(|state| state.valid_for(position, terra_core::brewing::MATERIAL))
    }
    pub fn valid_campfire(&self, position: [i64; 3]) -> bool {
        self.block(position) == Some(terra_core::campfire::MATERIAL)
            && self
                .object_state(position)
                .is_some_and(|state| state.valid_for(position, terra_core::campfire::MATERIAL))
    }
    pub fn cauldrons(&self) -> impl Iterator<Item = ([i64; 3], &CauldronState)> + '_ {
        self.cauldrons.iter().map(|(&p, state)| (p, state))
    }
    pub fn campfires(&self) -> impl Iterator<Item = ([i64; 3], &CampfireState)> + '_ {
        self.campfires.iter().map(|(&p, state)| (p, state))
    }
    pub fn cauldron_at(&self, position: [i64; 3]) -> Option<&CauldronState> {
        self.cauldrons.get(&position)
    }
    pub fn campfire_at(&self, position: [i64; 3]) -> Option<&CampfireState> {
        self.campfires.get(&position)
    }
    pub fn remove_cauldron(&mut self, position: [i64; 3]) {
        self.cauldrons.remove(&position);
    }
    pub fn remove_campfire(&mut self, position: [i64; 3]) {
        self.campfires.remove(&position);
    }
    pub fn set_cauldron(&mut self, position: [i64; 3], state: CauldronState) -> Result<(), String> {
        let inventory = self.container_at(position).cloned().unwrap_or_default();
        self.commit_cauldron(position, state, inventory).map(|_| ())
    }
    pub fn set_campfire(&mut self, position: [i64; 3], state: CampfireState) -> Result<(), String> {
        let inventory = self.container_at(position).cloned().unwrap_or_default();
        self.commit_campfire(position, state, inventory).map(|_| ())
    }
    pub fn commit_cauldron(
        &mut self,
        position: [i64; 3],
        state: CauldronState,
        inventory: Inventory,
    ) -> Result<bool, String> {
        if !state.valid()
            || !self.valid_cauldron(position)
            || !self.valid_container(position, &inventory)
            || !self.cauldrons.contains_key(&position) && self.cauldrons.len() >= MAX_HEARTHS
        {
            return Err("Invalid, orphaned or over-capacity cauldron".into());
        }
        if self.cauldrons.get(&position) == Some(&state)
            && self.container_at(position) == Some(&inventory)
        {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Cauldron revision overflow")?;
        self.cauldrons.insert(position, state);
        self.containers.insert(position, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
    pub fn commit_campfire(
        &mut self,
        position: [i64; 3],
        state: CampfireState,
        inventory: Inventory,
    ) -> Result<bool, String> {
        if !state.valid()
            || !self.valid_campfire(position)
            || !self.valid_container(position, &inventory)
            || !self.campfires.contains_key(&position) && self.campfires.len() >= MAX_HEARTHS
        {
            return Err("Invalid, orphaned or over-capacity campfire".into());
        }
        if self.campfires.get(&position) == Some(&state)
            && self.container_at(position) == Some(&inventory)
        {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Campfire revision overflow")?;
        self.campfires.insert(position, state);
        self.containers.insert(position, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{ItemKind, ItemStack};

    #[test]
    fn attached_hearth_transactions_reject_orphans_and_revision_overflow_atomically() {
        let position = [-1, -16, 15];
        let mut journal = EditJournal::default();
        assert!(
            journal
                .commit_attached_campfire(position, Default::default())
                .is_err()
        );
        journal.record(position, 161).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: terra_core::brewing::ATTACHED_FIRE,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.set_cauldron(position, Default::default()).unwrap();
        journal
            .commit_attached_campfire(position, Default::default())
            .unwrap();
        let state = journal.attached_campfire_at(position).unwrap().clone();
        let uuid = terra_core::PlayerUuid::new_v4();
        let mut player = Inventory::default();
        player.insert(ItemKind::Stick, 1);
        journal
            .gameplay_mut()
            .inventories
            .insert(uuid, player.clone());
        let kettle = journal.container_at(position).cloned();
        journal.gameplay_mut().revision = u64::MAX;
        let mut fire_inventory = Inventory {
            slots: vec![ItemStack::EMPTY; 3],
            ..Default::default()
        };
        fire_inventory.slots[1] = ItemStack {
            item: ItemKind::Stick,
            count: 1,
        };
        assert!(
            journal
                .commit_attached_container_transaction(
                    uuid,
                    Inventory::default(),
                    position,
                    fire_inventory
                )
                .is_err()
        );
        assert_eq!(journal.gameplay().inventories[&uuid], player);
        assert_eq!(journal.attached_campfire_at(position), Some(&state));
        assert_eq!(journal.container_at(position), kettle.as_ref());
        journal.set_object_state(position, None).unwrap();
        assert!(journal.attached_campfire_at(position).is_none());
        assert!(journal.container_at(position).is_none());
    }
}
