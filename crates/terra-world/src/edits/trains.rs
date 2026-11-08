//! Stage every touched sparse section before publishing parts, cargo and vehicle ownership.
use super::EditJournal;
use crate::{streaming::ChunkStore, trains::AssemblyPlan};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{train_parts, trains::TrainError};
use terra_voxel::global_to_section;
impl EditJournal {
    pub(crate) fn commit_train_plan(
        &mut self,
        chunks: &mut ChunkStore,
        plan: &AssemblyPlan,
    ) -> Result<u64, TrainError> {
        if self.gameplay.trains.revision != plan.registry_revision
            || self.gameplay.revision == u64::MAX
        {
            return Err(TrainError::Stale);
        }
        let fresh = if plan.assembling {
            crate::trains::plan_assembly(
                chunks,
                self,
                plan.actor,
                plan.station,
                plan.start,
                plan.vehicle.kind,
            )?
        } else {
            crate::trains::plan_dismantle(chunks, self, plan.actor, plan.station, plan.vehicle.id)?
        };
        if fresh.vehicle != plan.vehicle
            || fresh.parts.len() != plan.parts.len()
            || fresh.parts.iter().zip(&plan.parts).any(|(a, b)| {
                a.cell != b.cell
                    || a.expected != b.expected
                    || a.state != b.state
                    || a.cargo != b.cargo
            })
        {
            return Err(TrainError::Stale);
        }
        let mut registry = self.gameplay.trains.clone();
        let id = if plan.assembling {
            registry.insert(plan.vehicle.clone())?
        } else {
            registry.remove(plan.actor, plan.vehicle.id)?.id
        };
        let owners: BTreeSet<_> = plan
            .parts
            .iter()
            .map(|part| global_to_section(part.cell).0)
            .collect();
        if owners.len() > 64 {
            return Err(TrainError::Limit);
        }
        let mut staged = Self::default();
        let mut halo = BTreeSet::new();
        let mut blocks = BTreeMap::new();
        for owner in &owners {
            if let Some(section) = self.sections.get(owner) {
                staged.sections.insert(*owner, section.clone());
            }
            blocks.insert(
                *owner,
                chunks
                    .loaded
                    .get(owner)
                    .ok_or(TrainError::Incomplete)?
                    .data
                    .clone(),
            );
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let neighbor = owner.offset([x, y, z]).map_err(|_| TrainError::Overflow)?;
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
        for (index, part) in plan.parts.iter().enumerate() {
            let material = if plan.assembling {
                0
            } else {
                plan.vehicle.parts[index].material
            };
            let (owner, local) = global_to_section(part.cell);
            staged
                .record(part.cell, material)
                .map_err(|_| TrainError::Overflow)?;
            staged
                .set_object_state(part.cell, if plan.assembling { None } else { part.state })
                .map_err(|_| TrainError::Invalid)?;
            blocks
                .get_mut(&owner)
                .unwrap()
                .set_block(local, material)
                .map_err(|_| TrainError::Invalid)?;
        }
        // The registry, all block/pose/epoch writes, cargo and loaded payloads
        // have passed their fallible work. Publication cannot split ownership.
        for (owner, section) in staged.sections {
            self.sections.insert(owner, section);
        }
        for owner in &owners {
            self.refresh_object_section(*owner);
        }
        self.mesh_epochs.extend(staged.mesh_epochs);
        self.water_mesh_epochs.extend(staged.water_mesh_epochs);
        for (owner, data) in blocks {
            chunks.loaded.get_mut(&owner).unwrap().data = data;
        }
        for part in &plan.parts {
            self.containers.remove(&part.cell);
            if !plan.assembling
                && let Some(cargo) = &part.cargo
            {
                debug_assert_eq!(self.block(part.cell), Some(train_parts::CARGO));
                self.containers.insert(part.cell, cargo.clone());
            }
            self.wake_fluid_neighbors(part.cell);
        }
        self.gameplay.trains = registry;
        self.gameplay.revision += 1;
        for section in halo {
            chunks.mark_dirty_if_wanted(section);
        }
        Ok(id)
    }
}
