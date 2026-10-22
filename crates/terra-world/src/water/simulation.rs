use super::{
    FLUID_CAPACITY, FluidCell, FluidError, WaterColumns,
    fluid::{adjacent, from_columns},
};
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess, WorldTerrain},
};
use bevy::prelude::*;
use std::collections::HashMap;
use terra_voxel::global_to_section;

const TICK_SECONDS: f64 = 0.1;
const CELL_BUDGET: usize = 256;
mod supply;

#[derive(Resource, Debug, Default)]
pub struct FluidDiagnostics {
    pub visited: usize,
    pub elapsed_ms: f64,
}

/// Snapshot for infrequent profiling; counting saved cells traverses the journal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FluidWorkCounts {
    pub active: usize,
    pub queued: usize,
    pub sleeping: usize,
    pub sleeping_sections: usize,
    pub recorded_cells: usize,
    pub sources: usize,
    pub water_epoch_sections: usize,
}

#[derive(Debug, Default, Clone)]
pub struct FluidStep {
    pub visited: usize,
    pub transferred: u64,
    pub sourced: u64,
    /// Level replenishment from flowing cells, separately from source injection.
    pub replenished: u64,
    pub absorbed: u64,
    /// Explicit recession after a source/outlet is removed, not lost transfer mass.
    pub drained: u64,
    pub changed: bool,
    pub removed_props: usize,
    prop_sections: Vec<terra_voxel::SectionPos>,
    wetted: Vec<[i64; 3]>,
}

struct Sampler<'a> {
    terrain: &'a TerraGenerator,
    chunks: &'a ChunkStore,
    columns: HashMap<[i64; 2], WaterColumns>,
    spaces: HashMap<[i64; 3], super::space::FluidSpace>,
}
impl Sampler<'_> {
    fn cell(&mut self, journal: &EditJournal, p: [i64; 3]) -> Option<FluidCell> {
        let block = self.chunks.block_at(p)?;
        if !super::can_hold_fluid(block) {
            return Some(FluidCell::default());
        }
        let cell = journal.fluid(p).unwrap_or_else(|| {
            let column = *self.columns.entry([p[0], p[2]]).or_insert_with(|| {
                self.terrain
                    .water_columns(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
            });
            from_columns(column, p[1])
        });
        Some(self.space(journal, p).clamp(cell))
    }
    fn space(&mut self, journal: &EditJournal, p: [i64; 3]) -> &super::space::FluidSpace {
        self.spaces.entry(p).or_insert_with(|| {
            super::space::FluidSpace::at(journal, p, |q| self.chunks.block_at(q))
        })
    }
    fn connected(
        &mut self,
        journal: &EditJournal,
        p: [i64; 3],
        a: FluidCell,
        q: [i64; 3],
        b: FluidCell,
    ) -> bool {
        let left = self.space(journal, p).clone();
        left.connected(
            a,
            self.space(journal, q),
            b,
            std::array::from_fn(|i| q[i] - p[i]),
        )
    }
    fn can_drain(
        &mut self,
        journal: &EditJournal,
        p: [i64; 3],
        cell: FluidCell,
        down: [i64; 3],
    ) -> bool {
        let open = FluidCell {
            amount: FLUID_CAPACITY,
            ..Default::default()
        };
        self.connected(journal, p, cell, down, open)
    }
}

/// Execute one bounded authority step. A dry unloaded neighbor is not assumed to
/// be air: work sleeps until that section loads. Non-source levels relax toward
/// upstream supply, so disconnected pools cannot keep one another alive.
pub fn simulate_fluid_step(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    budget: usize,
) -> Result<FluidStep, FluidError> {
    let mut result = FluidStep::default();
    if !terrain.config().water {
        return Ok(result);
    }
    let work = journal.take_fluid_work(budget.min(4096));
    let mut sampler = Sampler {
        terrain,
        chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
    };
    for p in work {
        result.visited += 1;
        let Some(cell) = sampler.cell(journal, p) else {
            journal.sleep_fluid(p, global_to_section(p).0);
            continue;
        };
        if !chunks.block_at(p).is_some_and(super::can_hold_fluid) {
            continue;
        }
        if cell.amount > 0 {
            result.wetted.push(p);
        }
        if cell.source {
            continue;
        }
        let Some(down) = adjacent(p, [0, -1, 0]) else {
            continue;
        };
        let Some(below) = sampler.cell(journal, down) else {
            journal.sleep_fluid(p, global_to_section(down).0);
            continue;
        };
        let (wanted, waiting, from_source) = supply::incoming(&mut sampler, journal, p);
        if let Some(section) = waiting {
            journal.sleep_fluid(p, section);
            continue;
        }
        if below.source && wanted.amount == 0 && sampler.can_drain(journal, p, cell, down) {
            result.absorbed += u64::from(cell.amount);
            result.transferred += u64::from(cell.amount);
            result.changed |= journal.set_fluid(p, FluidCell::default())?;
            continue;
        }
        let amount = if wanted.amount < cell.amount {
            cell.amount - 1
        } else {
            wanted.amount
        };
        let next = sampler
            .space(journal, p)
            .clamp(FluidCell { amount, ..wanted });
        let amount = next.amount;
        let added = u64::from(amount.saturating_sub(cell.amount));
        if from_source {
            result.sourced += added;
        } else {
            result.replenished += added;
        }
        result.drained += u64::from(cell.amount.saturating_sub(amount));
        result.transferred += added;
        result.changed |= journal.set_fluid(p, next)?;
        if amount > 0 {
            result.wetted.push(p);
        }
        if amount > wanted.amount {
            journal.wake_fluid(p);
        }
    }
    retire_flooded_props(terrain, chunks, journal, &mut result)?;
    Ok(result)
}

#[cfg(test)]
mod cascade_tests;
#[cfg(test)]
mod partial_tests;
#[cfg(test)]
mod tests;

pub(crate) fn tick(
    time: Res<Time>,
    access: Res<WorldAccess>,
    terrain: Res<WorldTerrain>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut last: Local<f64>,
    diagnostics: Option<ResMut<FluidDiagnostics>>,
) {
    if !access.authoritative() || time.elapsed_secs_f64() - *last < TICK_SECONDS {
        return;
    }
    *last = time.elapsed_secs_f64();
    let started = std::time::Instant::now();
    match simulate_fluid_step(
        &terrain.0,
        &chunks,
        journal.bypass_change_detection(),
        CELL_BUDGET,
    ) {
        Ok(step) => {
            if let Some(mut diagnostics) = diagnostics {
                diagnostics.visited = step.visited;
                diagnostics.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            }
            for position in step.prop_sections {
                if let Some(section) = chunks.loaded.get_mut(&position) {
                    journal.apply_prop_removals(position, &mut section.props);
                }
            }
            if step.changed {
                journal.set_changed();
            }
        }
        Err(error) => {
            journal.set_changed();
            tracing::error!(%error, "Fluid simulation update rejected");
        }
    }
}

fn retire_flooded_props(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    step: &mut FluidStep,
) -> Result<(), FluidError> {
    let mut contacts = std::mem::take(&mut step.wetted);
    contacts.sort_unstable();
    contacts.dedup();
    let mut candidates: HashMap<terra_voxel::SectionPos, Vec<[[f64; 3]; 2]>> = HashMap::new();
    for p in contacts {
        let Some(block) = chunks.block_at(p) else {
            continue;
        };
        let cell = super::fluid_cell_at(terrain, journal, p, block);
        if cell.amount == 0 {
            continue;
        }
        let low = global_to_section([p[0] - 2, p[1] - 4, p[2] - 2]).0;
        let high = global_to_section([p[0] + 2, p[1], p[2] + 2]).0;
        let bounds = [
            p.map(|v| v as f64),
            [
                p[0] as f64 + 1.0,
                p[1] as f64 + cell.height(),
                p[2] as f64 + 1.0,
            ],
        ];
        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let owner = terra_voxel::SectionPos::new(x, y, z);
                    if chunks.loaded.contains_key(&owner) {
                        candidates.entry(owner).or_default().push(bounds);
                    }
                }
            }
        }
    }
    let mut owners: Vec<_> = candidates.keys().copied().collect();
    owners.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    for owner in owners {
        let Some(section) = chunks.loaded.get(&owner) else {
            continue;
        };
        let water = &candidates[&owner];
        for prop in section
            .props
            .iter()
            .filter(|p| !p.removed && !p.water_tolerant() && !journal.prop_removed(owner, p.id))
            .copied()
            .collect::<Vec<_>>()
        {
            let bounds = prop.bounds();
            if water.iter().any(|wet| {
                (0..3).all(|axis| {
                    bounds[1][axis] > wet[0][axis] + 0.001 && bounds[0][axis] < wet[1][axis] - 0.001
                })
            }) && journal.remove_prop(owner, prop.id)?
            {
                step.removed_props += 1;
                step.changed = true;
                if step.prop_sections.last() != Some(&owner) {
                    step.prop_sections.push(owner);
                }
            }
        }
    }
    Ok(())
}
