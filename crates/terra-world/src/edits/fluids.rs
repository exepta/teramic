use super::EditJournal;
use crate::water::{
    FluidCell, FluidError, FluidSnapshot,
    fluid::{NEIGHBORS, adjacent},
};
use terra_voxel::{SectionPos, blocks::BlockError, global_to_section};

impl EditJournal {
    /// Read-only profiling snapshot, intended for seconds-apart samples, not each frame.
    #[cfg(feature = "bevy")]
    pub fn fluid_work_counts(&self) -> crate::water::FluidWorkCounts {
        crate::water::FluidWorkCounts {
            active: self.fluid_active.len(),
            queued: self.fluid_queued.len(),
            sleeping: self.fluid_sleeping.values().map(|cells| cells.len()).sum(),
            sleeping_sections: self.fluid_sleeping.len(),
            recorded_cells: self.sections.values().map(|s| s.fluids.len()).sum(),
            sources: self.fluid_cells().filter(|(_, cell)| cell.source).count(),
            water_epoch_sections: self.water_mesh_epochs.len(),
        }
    }

    pub fn fluid_epoch(&self, section: SectionPos) -> u64 {
        self.sections.get(&section).map_or(0, |s| s.fluid_epoch)
    }
    pub fn water_mesh_epoch(&self, section: SectionPos) -> u64 {
        self.water_mesh_epochs.get(&section).copied().unwrap_or(0)
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn water_mesh_epochs(&self) -> impl Iterator<Item = (SectionPos, u64)> + '_ {
        self.water_mesh_epochs.iter().map(|(&p, &epoch)| (p, epoch))
    }
    pub fn fluid(&self, global: [i64; 3]) -> Option<FluidCell> {
        let (section, local) = global_to_section(global);
        self.sections.get(&section)?.fluids.get(&local).copied()
    }
    pub fn fluid_cells(&self) -> impl Iterator<Item = ([i64; 3], FluidCell)> + '_ {
        self.sections.iter().flat_map(|(&section, edits)| {
            edits.fluids.iter().filter_map(move |(&local, &cell)| {
                section.global_block(local).ok().map(|p| (p, cell))
            })
        })
    }
    pub fn fluid_cells_in(
        &self,
        section: SectionPos,
    ) -> impl Iterator<Item = ([i64; 3], FluidCell)> + '_ {
        self.sections
            .get(&section)
            .into_iter()
            .flat_map(move |edits| {
                edits.fluids.iter().filter_map(move |(&local, &cell)| {
                    section.global_block(local).ok().map(|p| (p, cell))
                })
            })
    }
    /// Only liquid data and water meshes change. Terrain, colliders and vegetation
    /// retain their handles and revisions during fluid simulation.
    pub fn set_fluid(&mut self, global: [i64; 3], cell: FluidCell) -> Result<bool, FluidError> {
        if !cell.validate() {
            return Err(FluidError::InvalidCell);
        }
        Ok(self.write_fluid(global, cell)?)
    }
    pub(super) fn write_fluid(
        &mut self,
        global: [i64; 3],
        cell: FluidCell,
    ) -> Result<bool, BlockError> {
        self.write_fluid_with_wake(global, cell, true)
    }
    pub(super) fn write_fluid_with_wake(
        &mut self,
        global: [i64; 3],
        cell: FluidCell,
        wake_simulation: bool,
    ) -> Result<bool, BlockError> {
        let previous = self.fluid(global).unwrap_or_default();
        if previous == cell || (previous.amount == 0 && cell.amount == 0) {
            return Ok(false);
        }
        let (owner, local) = global_to_section(global);
        let epoch = self
            .fluid_epoch(owner)
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        self.invalidate_water(global)?;
        let section = self.sections.entry(owner).or_default();
        section.fluid_epoch = epoch;
        if cell.amount == 0 {
            section.fluids.remove(&local);
        } else {
            section.fluids.insert(local, cell);
        }
        if wake_simulation {
            self.wake_fluid_neighbors(global);
        }
        Ok(true)
    }
    pub(super) fn invalidate_water(&mut self, global: [i64; 3]) -> Result<(), BlockError> {
        self.invalidate_water_halo(global, 1)
    }
    pub(super) fn invalidate_water_geometry(&mut self, global: [i64; 3]) -> Result<(), BlockError> {
        // A neighboring stair/fence shape also reads its own adjacent blocks.
        // Only solid/object edits need this wider halo; fluid ticks retain one.
        self.invalidate_water_halo(global, 2)
    }
    fn invalidate_water_halo(&mut self, global: [i64; 3], radius: usize) -> Result<(), BlockError> {
        let (owner, local) = global_to_section(global);
        let offsets = local.map(|v| {
            if v < radius {
                -1
            } else if v >= 16 - radius {
                1
            } else {
                0
            }
        });
        let mut affected = vec![owner];
        for z in [0, offsets[2]] {
            for y in [0, offsets[1]] {
                for x in [0, offsets[0]] {
                    let p = owner.offset([x, y, z])?;
                    if !affected.contains(&p) {
                        affected.push(p);
                    }
                }
            }
        }
        for &p in &affected {
            self.water_mesh_epoch(p)
                .checked_add(1)
                .ok_or(BlockError::RevisionOverflow)?;
        }
        for p in affected {
            *self.water_mesh_epochs.entry(p).or_default() += 1;
        }
        Ok(())
    }
    pub fn fluid_snapshot(&self, section: SectionPos) -> Result<FluidSnapshot, BlockError> {
        let origin = section.origin()?;
        let mut result = FluidSnapshot::new();
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    let p = section.offset([x, y, z])?;
                    if let Some(edits) = self.sections.get(&p) {
                        for (&local, &cell) in &edits.fluids {
                            let global = p.global_block(local)?;
                            if (0..3)
                                .all(|i| global[i] >= origin[i] - 1 && global[i] <= origin[i] + 16)
                            {
                                result.insert(global, cell);
                            }
                        }
                    }
                }
            }
        }
        Ok(result)
    }
    pub fn wake_fluid_neighbors(&mut self, global: [i64; 3]) {
        self.wake_fluid(global);
        for offset in NEIGHBORS {
            if let Some(p) = adjacent(global, offset) {
                self.wake_fluid(p);
            }
        }
    }
    pub(crate) fn wake_fluid(&mut self, global: [i64; 3]) {
        if self.fluid_queued.insert(global) {
            self.fluid_active.push_back(global);
        }
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn take_fluid_work(&mut self, budget: usize) -> Vec<[i64; 3]> {
        let mut result = Vec::with_capacity(budget.min(self.fluid_active.len()));
        for _ in 0..budget {
            let Some(p) = self.fluid_active.pop_front() else {
                break;
            };
            self.fluid_queued.remove(&p);
            result.push(p);
        }
        result
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn sleep_fluid(&mut self, global: [i64; 3], waiting: SectionPos) {
        self.fluid_sleeping
            .entry(waiting)
            .or_default()
            .insert(global);
    }
    #[cfg(feature = "bevy")]
    pub(crate) fn resume_fluids(&mut self, section: SectionPos) {
        let mut active: Vec<_> = self
            .fluid_sleeping
            .remove(&section)
            .into_iter()
            .flatten()
            .collect();
        if let Some(edits) = self.sections.get(&section) {
            active.extend(
                edits
                    .fluids
                    .keys()
                    .chain(edits.blocks.keys())
                    .filter_map(|&local| section.global_block(local).ok()),
            );
        }
        active.sort_unstable();
        active.dedup();
        for p in active {
            self.wake_fluid_neighbors(p);
        }
    }

    /// Streaming owns simulation interest, not persistent liquid state. Keep
    /// loaded owners waiting on an unloaded neighbor, but evict work whose own
    /// section left every player's interest. Reload rebuilds work from edits.
    #[cfg(feature = "bevy")]
    pub(crate) fn suspend_unloaded_fluids(&mut self, mut loaded: impl FnMut(SectionPos) -> bool) {
        self.fluid_active
            .retain(|p| loaded(global_to_section(*p).0));
        self.fluid_queued
            .retain(|p| loaded(global_to_section(*p).0));
        self.fluid_sleeping.retain(|_, owners| {
            owners.retain(|p| loaded(global_to_section(*p).0));
            !owners.is_empty()
        });
    }
}
