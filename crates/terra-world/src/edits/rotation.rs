use super::EditJournal;
use crate::{placement::Layout, streaming::ChunkStore};
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::{SectionPos, global_to_section};

impl EditJournal {
    pub(crate) fn commit_rotation(
        &mut self,
        chunks: &mut ChunkStore,
        plan: &Layout,
    ) -> Result<bool, String> {
        let changed = plan.edits.iter().any(|edit| {
            edit.expected_material != edit.material
                || self.block(edit.position) != Some(edit.material)
        }) || plan
            .states
            .iter()
            .any(|(position, state)| self.object_state(*position) != *state)
            || plan
                .belts
                .iter()
                .any(|(position, owner)| self.belt_owner(*position) != *owner);
        if !changed {
            return Ok(false);
        }
        let tints = self.rotation_tints(plan);
        // The footprint is at most five cells wide. Include the two-cell pipe
        // and fluid halo, copying at most 27 sparse journal sections, not history.
        let mut low = [i64::MAX; 3];
        let mut high = [i64::MIN; 3];
        for edit in &plan.edits {
            for axis in 0..3 {
                low[axis] = low[axis].min(edit.position[axis] - 2);
                high[axis] = high[axis].max(edit.position[axis] + 2);
            }
        }
        let low = global_to_section(low).0;
        let high = global_to_section(high).0;
        let mut staged = Self {
            next_furniture_order: self.next_furniture_order,
            ..Default::default()
        };
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    let section = SectionPos { x, y, z };
                    if let Some(edits) = self.sections.get(&section) {
                        staged.sections.insert(section, edits.clone());
                    }
                    staged.mesh_epochs.insert(section, self.mesh_epoch(section));
                    staged
                        .water_mesh_epochs
                        .insert(section, self.water_mesh_epoch(section));
                }
            }
        }
        let mut blocks = BTreeMap::new();
        for edit in &plan.edits {
            if let Some(order) = self.furniture_order.get(&edit.position) {
                staged.furniture_order.insert(edit.position, *order);
            }
            if edit.material == edit.expected_material {
                continue;
            }
            let (owner, local) = global_to_section(edit.position);
            let data = blocks
                .entry(owner)
                .or_insert_with(|| chunks.loaded[&owner].data.clone());
            data.set_block(local, edit.material)
                .map_err(|error| error.to_string())?;
        }
        staged.rotation_records(plan, &tints)?;
        let moves = crate::wrench::relocated_furniture(plan, self);
        let private_moves = moves
            .iter()
            .any(|(from, _)| self.containers.contains_key(from));
        let next_gameplay = if private_moves {
            Some(
                self.gameplay
                    .revision
                    .checked_add(1)
                    .ok_or("Container revision overflow")?,
            )
        } else {
            None
        };
        for (from, to) in &moves {
            if let Some(inventory) = self.containers.get(from)
                && !staged.valid_container(*to, inventory)
            {
                return Err("Invalid rotated container state".into());
            }
        }
        let orders: Vec<_> = moves
            .iter()
            .filter_map(|(from, to)| self.furniture_order.get(from).map(|order| (*to, *order)))
            .collect();
        // All fallible metadata and block mutations ran on the bounded copies.
        // No mutable world access occurs between preflight and these same writes.
        let affected = self.rotation_records(plan, &tints)?;
        for edit in &plan.edits {
            if edit.material != edit.expected_material {
                let (owner, local) = global_to_section(edit.position);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, edit.material)
                    .map_err(|error| error.to_string())?;
            }
            if edit.expected_material == 99 {
                self.crusher_driven.remove(&edit.position);
            }
        }
        for (from, to) in moves {
            if let Some(inventory) = self.containers.remove(&from) {
                self.containers.insert(to, inventory);
            }
        }
        if let Some(revision) = next_gameplay {
            self.gameplay.revision = revision;
        }
        for (position, order) in orders {
            self.furniture_order.insert(position, order);
        }
        for section in affected {
            chunks.mark_dirty_if_wanted(section);
        }
        Ok(true)
    }

    fn rotation_tints(&self, plan: &Layout) -> BTreeMap<[i64; 3], [u8; 3]> {
        let mut tints = BTreeMap::new();
        for edit in &plan.edits {
            if !terra_core::beds::is_bed(edit.material) {
                continue;
            }
            let Some(state) = plan
                .states
                .iter()
                .find(|(position, _)| *position == edit.position)
                .and_then(|(_, state)| *state)
                .filter(|state| !state.secondary)
            else {
                continue;
            };
            let Some(color) = self.tint_at(edit.position) else {
                continue;
            };
            tints.insert(edit.position, color);
            if let Some(peer) = state.peer {
                tints.insert(peer, color);
            }
        }
        tints
    }

    fn rotation_records(
        &mut self,
        plan: &Layout,
        tints: &BTreeMap<[i64; 3], [u8; 3]>,
    ) -> Result<BTreeSet<SectionPos>, String> {
        let mut affected = BTreeSet::new();
        for edit in &plan.edits {
            if edit.expected_material != edit.material
                || self.block(edit.position) != Some(edit.material)
            {
                affected.extend(
                    self.record(edit.position, edit.material)
                        .map_err(|error| error.to_string())?,
                );
            }
        }
        for (position, state) in &plan.states {
            affected.extend(self.set_object_state(*position, *state)?);
        }
        for (position, owner) in &plan.belts {
            affected.extend(self.set_belt_owner(*position, *owner)?);
        }
        for (&position, &color) in tints {
            self.set_tint(position, Some(color))?;
        }
        Ok(affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edits::BlockObjectState, streaming::LoadedSection};
    use terra_voxel::BlockSection;

    #[test]
    fn a_late_water_or_mesh_epoch_overflow_does_not_publish_partial_pose_changes() {
        for water in [false, true] {
            let position = [4, 4, 4];
            let (owner, local) = global_to_section(position);
            let mut data = BlockSection::empty(owner).unwrap();
            data.set_block(local, 144).unwrap();
            let mut chunks = ChunkStore::default();
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            let mut journal = EditJournal::default();
            journal.record(position, 144).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
            if water {
                journal.water_mesh_epochs.insert(owner, u64::MAX);
            } else {
                journal.mesh_epochs.insert(owner, u64::MAX);
            }
            let before = journal.section_delta(owner);
            let mesh = journal.mesh_epoch(owner);
            let water_epoch = journal.water_mesh_epoch(owner);
            let blocks = chunks.loaded[&owner].data.revision();
            let plan = crate::wrench::propose(&chunks, &journal, position, false).unwrap();
            assert!(crate::wrench::commit(&mut chunks, &mut journal, &plan).is_err());
            assert_eq!(journal.section_delta(owner), before);
            assert_eq!(journal.mesh_epoch(owner), mesh);
            assert_eq!(journal.water_mesh_epoch(owner), water_epoch);
            assert_eq!(chunks.loaded[&owner].data.revision(), blocks);
            assert_eq!(
                journal.object_state(position),
                Some(BlockObjectState::default())
            );
        }
    }
}
