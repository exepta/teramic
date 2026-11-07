//! Atomic rail edits copy only occupied sparse sections and their epoch halo.
use super::EditJournal;
use crate::{placement::Layout, streaming::ChunkStore, water::FluidCell};
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::global_to_section;

impl EditJournal {
    pub(crate) fn commit_rail_layout(
        &mut self,
        chunks: &mut ChunkStore,
        plan: &Layout,
    ) -> Result<bool, String> {
        crate::rails::placement::validate_commit(plan, chunks, self)?;
        let rail_epoch = self
            .rail_epoch
            .checked_add(1)
            .ok_or("Rail network revision overflow")?;
        let owners: BTreeSet<_> = plan
            .edits
            .iter()
            .map(|edit| global_to_section(edit.position).0)
            .collect();
        let mut staged = Self::default();
        let mut halo = BTreeSet::new();
        for &owner in &owners {
            if let Some(edits) = self.sections.get(&owner) {
                staged.sections.insert(owner, edits.clone());
            }
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let neighbor =
                            owner.offset([x, y, z]).map_err(|error| error.to_string())?;
                        halo.insert(neighbor);
                        staged
                            .mesh_epochs
                            .insert(neighbor, self.mesh_epoch(neighbor));
                        staged
                            .water_mesh_epochs
                            .insert(neighbor, self.water_mesh_epoch(neighbor));
                    }
                }
            }
        }
        let mut blocks = BTreeMap::new();
        for edit in &plan.edits {
            if edit.material == edit.expected_material {
                continue;
            }
            let (owner, local) = global_to_section(edit.position);
            if let Some(loaded) = chunks.loaded.get(&owner) {
                let data = blocks.entry(owner).or_insert_with(|| loaded.data.clone());
                data.set_block(local, edit.material)
                    .map_err(|error| error.to_string())?;
            }
            let water = if terra_core::rails::is_rail(edit.material) {
                Some(self.fluid(edit.position).unwrap_or_else(|| {
                    if matches!(edit.expected_material, 14 | 27) {
                        return FluidCell {
                            amount: 8,
                            source: true,
                            ..Default::default()
                        };
                    }
                    chunks
                        .surface_column_at(edit.position[0], edit.position[2])
                        .map_or_else(FluidCell::default, |surface| {
                            crate::water::fluid::from_columns(
                                surface.water_columns(),
                                edit.position[1],
                            )
                        })
                }))
            } else {
                None
            };
            staged
                .record(edit.position, edit.material)
                .map_err(|error| error.to_string())?;
            if let Some(water) = water.filter(|water| water.amount > 0) {
                staged
                    .set_fluid(edit.position, water)
                    .map_err(|error| error.to_string())?;
            }
        }
        for (position, state) in &plan.states {
            staged.set_object_state(*position, *state)?;
        }
        // All fallible block, object, water and halo counter changes succeeded.
        // Live inventory ownership and tree registries have never been cloned.
        for (owner, edits) in staged.sections {
            self.sections.insert(owner, edits);
        }
        for owner in owners {
            self.refresh_object_section(owner);
        }
        for (owner, data) in blocks {
            chunks
                .loaded
                .get_mut(&owner)
                .expect("captured rail section")
                .data = data;
        }
        self.mesh_epochs.extend(staged.mesh_epochs);
        self.water_mesh_epochs.extend(staged.water_mesh_epochs);
        self.rail_epoch = rail_epoch;
        std::sync::Arc::make_mut(&mut self.rail_identity);
        for edit in &plan.edits {
            self.wake_fluid_neighbors(edit.position);
        }
        for section in halo {
            chunks.mark_dirty_if_wanted(section);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rails, streaming::LoadedSection};
    use terra_core::rails::{Piece, STRAIGHT};
    use terra_voxel::BlockSection;

    #[test]
    fn late_mesh_or_water_counter_overflow_cannot_publish_any_rail_cells() {
        for water in [false, true] {
            let piece = Piece::new([-1, 16, -1], STRAIGHT, 0).unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            let mut set = |cell, material| {
                let (owner, local) = global_to_section(cell);
                chunks
                    .loaded
                    .entry(owner)
                    .or_insert_with(|| LoadedSection {
                        data: BlockSection::empty(owner).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    })
                    .data
                    .set_block(local, material)
                    .unwrap();
            };
            for cell in rails::cells(piece) {
                set(cell, 0);
            }
            for point in piece.supports() {
                set(
                    std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
                    3,
                );
            }
            let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
            let last = plan.edits.last().unwrap().position;
            let owner = global_to_section(last).0;
            if water {
                journal.water_mesh_epochs.insert(owner, u64::MAX);
            } else {
                journal.mesh_epochs.insert(owner, u64::MAX);
            }
            let before: BTreeMap<_, _> = chunks
                .loaded
                .iter()
                .map(|(owner, section)| (*owner, section.data.clone()))
                .collect();
            assert!(rails::commit(&mut chunks, &mut journal, &plan).is_err());
            assert_eq!(journal.edited_blocks(), 0);
            assert_eq!(journal.rail_module_count(), 0);
            assert!(
                before
                    .into_iter()
                    .all(|(owner, data)| chunks.loaded[&owner].data == data)
            );
        }
    }
}
