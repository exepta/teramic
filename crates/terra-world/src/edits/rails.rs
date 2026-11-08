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
        let cleared = if plan.edits.iter().any(|edit| {
            terra_core::rails::is_rail(edit.material)
                && (edit.material != edit.expected_material
                    || plan
                        .states
                        .iter()
                        .find(|(cell, _)| *cell == edit.position)
                        .and_then(|(_, state)| *state)
                        .is_some_and(|state| {
                            Some(state.rail_curve)
                                != self.object_state(edit.position).map(|old| old.rail_curve)
                        }))
        }) {
            crate::rails::clearance(plan, chunks, self)?
        } else {
            Vec::new()
        };
        let mut retired_trees = BTreeMap::new();
        for cleared_prop in &cleared {
            if let Some(tree) = self.gameplay.trees.get(cleared_prop.prop.id.0)
                && tree.stage == terra_core::TreeStage::Sapling
            {
                let mut retired = tree.clone();
                retired.stage = terra_core::TreeStage::Felled;
                retired.fruit = None;
                retired.revision = retired
                    .revision
                    .checked_add(1)
                    .ok_or("Sapling revision overflow")?;
                retired_trees.insert(retired.id, retired);
            }
        }
        self.gameplay
            .trees
            .revision
            .checked_add(retired_trees.len() as u64)
            .ok_or("Sapling registry revision overflow")?;
        let rail_epoch = self
            .rail_epoch
            .checked_add(1)
            .ok_or("Rail network revision overflow")?;
        let owners: BTreeSet<_> = plan
            .edits
            .iter()
            .map(|edit| global_to_section(edit.position).0)
            .chain(cleared.iter().map(|cleared_prop| cleared_prop.prop.section))
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
        for cleared_prop in &cleared {
            staged
                .remove_prop(cleared_prop.prop.section, cleared_prop.prop.id)
                .map_err(|error| error.to_string())?;
            let section = staged
                .sections
                .get(&cleared_prop.prop.section)
                .ok_or("Missing prop clearance section")?;
            if section.removed_props.len() > 4096
                || section.placed_props.len() + section.removed_props.len() > 4096
            {
                return Err("Rail clearance exceeds persistent prop capacity".into());
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
        for (id, retired) in retired_trees {
            *self
                .gameplay
                .trees
                .get_mut(id)
                .expect("captured cleared sapling") = retired;
            self.gameplay.trees.changed();
        }
        let prop_sections: BTreeSet<_> = cleared
            .iter()
            .map(|cleared_prop| cleared_prop.prop.section)
            .collect();
        for cleared_prop in &cleared {
            if let Some(support) = self.crop_props.get(&cleared_prop.prop.id).copied() {
                self.remove_crop(support);
            }
        }
        for owner in prop_sections {
            if let Some(section) = chunks.loaded.get_mut(&owner) {
                self.apply_prop_removals(owner, &mut section.props);
            }
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
    fn a_late_prop_epoch_overflow_restores_every_prop_and_rail_cell() {
        let piece = Piece::new([15, 16, 1], STRAIGHT, 0).unwrap();
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
        for point in piece.supports().iter() {
            set(
                std::array::from_fn(|axis| piece.anchor[axis] + i64::from(point.support[axis])),
                3,
            );
        }
        let mut props = Vec::new();
        for (id, x) in [(20511, 15.5), (20512, 16.5)] {
            let prop = crate::props::WorldProp {
                id: crate::props::PropId(id),
                kind: crate::props::PropKind::Grass,
                position: [x, 16.0, 1.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: global_to_section([x as i64, 15, 1]).0,
                removed: false,
            };
            chunks
                .loaded
                .get_mut(&prop.section)
                .unwrap()
                .props
                .push(prop);
            props.push(prop);
        }
        let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
        assert_eq!(rails::clearance(&plan, &chunks, &journal).unwrap().len(), 2);
        journal
            .sections
            .entry(props[1].section)
            .or_default()
            .prop_epoch = u64::MAX;
        assert!(rails::commit(&mut chunks, &mut journal, &plan).is_err());
        assert!(
            props
                .iter()
                .all(|prop| !journal.prop_removed(prop.section, prop.id))
        );
        assert!(props.iter().all(|prop| {
            chunks.loaded[&prop.section]
                .props
                .iter()
                .any(|live| live.id == prop.id && !live.removed)
        }));
        assert!(
            rails::cells(piece)
                .into_iter()
                .all(|cell| chunks.block_at(cell) == Some(0))
        );
        assert_eq!(journal.rail_module_count(), 0);
    }

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
            for point in piece.supports().iter() {
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

    #[test]
    fn a_mixed_ramp_replacement_clears_props_atomically_and_rolls_back_prop_counter_overflow() {
        use terra_core::{WoodSpecies, rails::BuildChoice};
        for overflow in [false, true] {
            let old = Piece::new([15, 16, 0], STRAIGHT, 0).unwrap();
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
            for x in 14..=16 {
                for z in 0..=4 {
                    set([x, 15, z], 3);
                    if z == 4 {
                        set([x, 16, z], 3);
                    }
                }
            }
            let placed = rails::plan(&chunks, &journal, old.anchor, old.material, 0).unwrap();
            assert!(rails::commit(&mut chunks, &mut journal, &placed).unwrap());
            let prop = crate::props::WorldProp {
                id: crate::props::PropId(20701),
                kind: crate::props::PropKind::Grass,
                position: [15.5, 17.0, 3.5],
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: global_to_section([15, 16, 3]).0,
                removed: false,
            };
            assert!(journal.place_prop(prop).unwrap());
            let eye = [15.5, 18.125, 2.0];
            let target = [15.5, 16.75, 4.0];
            let delta: [f64; 3] = std::array::from_fn(|axis| target[axis] - eye[axis]);
            let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
            let direction = delta.map(|value| value / length);
            let hit = crate::interaction_shapes::raycast_solid(
                &chunks,
                &journal,
                &Default::default(),
                eye,
                direction,
                4.5,
            )
            .unwrap();
            let choice = rails::context_choice(
                &chunks,
                &journal,
                hit,
                (eye, direction),
                [0.0, -0.5],
                WoodSpecies::Oak,
                BuildChoice::Straight,
            );
            assert!(choice.valid, "{:?}", choice.error);
            let layout = choice.layout.unwrap();
            assert!(
                rails::clearance(&layout, &chunks, &journal)
                    .unwrap()
                    .iter()
                    .any(|entry| entry.prop.id == prop.id)
            );
            let before: BTreeMap<_, _> = chunks
                .loaded
                .iter()
                .map(|(owner, section)| (*owner, section.data.clone()))
                .collect();
            let rail_epoch = journal.rail_epoch();
            if overflow {
                journal.sections.get_mut(&prop.section).unwrap().prop_epoch = u64::MAX;
            }
            let result = rails::commit(&mut chunks, &mut journal, &layout);
            if overflow {
                assert!(result.is_err());
                assert!(rails::complete(&journal, old));
                assert!(!journal.prop_removed(prop.section, prop.id));
                assert_eq!(journal.rail_epoch(), rail_epoch);
                assert!(
                    before
                        .into_iter()
                        .all(|(owner, data)| chunks.loaded[&owner].data == data)
                );
            } else {
                assert_eq!(result, Ok(true));
                assert!(journal.prop_removed(prop.section, prop.id));
                assert_eq!(journal.rail_module_count(), 1);
                assert!(
                    journal.gameplay().drops.is_empty(),
                    "the authority reserves and emits prop drops separately"
                );
            }
        }
    }
}
