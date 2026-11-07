use super::EditJournal;
use terra_core::{Inventory, mixer::MixerState};

impl EditJournal {
    pub fn valid_mixer(&self, position: [i64; 3]) -> bool {
        self.block(position) == Some(terra_core::mixer::MATERIAL)
            && self
                .object_state(position)
                .is_some_and(|state| state.valid_for(position, terra_core::mixer::MATERIAL))
    }

    pub fn mixers(&self) -> impl Iterator<Item = ([i64; 3], &MixerState)> + '_ {
        self.mixers
            .iter()
            .map(|(&position, state)| (position, state))
    }

    pub fn mixer_at(&self, position: [i64; 3]) -> Option<&MixerState> {
        self.mixers.get(&position)
    }

    pub fn set_mixer(&mut self, position: [i64; 3], state: MixerState) -> Result<(), String> {
        let inventory = self.container_at(position).cloned().unwrap_or_default();
        self.commit_mixer(position, state, inventory).map(|_| ())
    }

    /// Pigment consumption, output and calibration share one atomic save revision.
    pub fn commit_mixer(
        &mut self,
        position: [i64; 3],
        state: MixerState,
        inventory: Inventory,
    ) -> Result<bool, String> {
        if !state.valid()
            || !self.valid_mixer(position)
            || !self.valid_container(position, &inventory)
        {
            return Err("Invalid or orphaned mixer state".into());
        }
        if self.mixers.get(&position) == Some(&state)
            && self.container_at(position) == Some(&inventory)
        {
            return Ok(false);
        }
        let revision = self
            .gameplay()
            .revision
            .checked_add(1)
            .ok_or("Mixer revision overflow")?;
        self.mixers.insert(position, state);
        self.containers.insert(position, inventory);
        self.gameplay_mut().revision = revision;
        Ok(true)
    }

    pub fn remove_mixer(&mut self, position: [i64; 3]) {
        self.mixers.remove(&position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::BlockObjectState;
    use terra_core::{ItemKind, ItemStack, beds::DyeColor, mixer};

    #[test]
    fn mixer_inventory_and_progress_commit_together_without_orphans_or_revision_wrap() {
        let mut journal = EditJournal::default();
        let position = [-1, 15, 16];
        assert!(journal.set_mixer(position, MixerState::default()).is_err());
        journal.record(position, mixer::MATERIAL).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let state = MixerState {
            target: [12, 96, 211],
            progress_seconds: 2.0,
            completed: 3,
        };
        let mut inventory = Inventory::default();
        inventory.slots.resize(mixer::SLOTS, ItemStack::EMPTY);
        inventory.slots[0] = ItemStack {
            item: ItemKind::Dye(DyeColor::ALL[0]),
            count: 4,
        };
        inventory.slots[mixer::OUTPUT] = ItemStack {
            item: ItemKind::Dye(DyeColor::Rgb(state.target)),
            count: 7,
        };
        assert!(
            journal
                .commit_mixer(position, state.clone(), inventory.clone())
                .unwrap()
        );
        assert_eq!(journal.mixers().count(), 1);
        assert_eq!(journal.mixer_at(position), Some(&state));
        assert_eq!(journal.container_at(position), Some(&inventory));
        let revision = journal.gameplay().revision;
        assert!(
            !journal
                .commit_mixer(position, state.clone(), inventory.clone())
                .unwrap()
        );
        assert_eq!(journal.gameplay().revision, revision);
        let mut invalid = inventory.clone();
        invalid.slots[0].item = ItemKind::Block(3);
        assert!(
            journal
                .commit_mixer(position, state.clone(), invalid)
                .is_err()
        );
        journal.gameplay_mut().revision = u64::MAX;
        let mut next = state.clone();
        next.completed += 1;
        assert!(
            journal
                .commit_mixer(position, next, inventory.clone())
                .is_err()
        );
        assert_eq!(journal.mixer_at(position), Some(&state));
        assert_eq!(journal.container_at(position), Some(&inventory));
        journal.remove_mixer(position);
        assert!(journal.mixer_at(position).is_none());
    }
}
