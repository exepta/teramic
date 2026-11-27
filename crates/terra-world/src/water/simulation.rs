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
use std::collections::{HashMap, HashSet, VecDeque};
use terra_voxel::global_to_section;

const TICK_SECONDS: f64 = terra_core::fluid_surface::WATER_STEP_SECONDS;
const CELL_BUDGET: usize = 256;
mod flux;
mod pools;
mod river;
pub(crate) mod sources;
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
    pub flooded_torches: Vec<[i64; 3]>,
    prop_sections: Vec<terra_voxel::SectionPos>,
    lava_contacts: Vec<[i64; 3]>,
    wetted: Vec<[i64; 3]>,
}

struct Sampler<'a> {
    terrain: &'a TerraGenerator,
    chunks: &'a ChunkStore,
    columns: HashMap<[i64; 2], WaterColumns>,
    spaces: HashMap<[i64; 3], super::space::FluidSpace>,
    river_columns: HashMap<[i64; 2], crate::SurfaceColumn>,
    river_origins: HashMap<[i64; 3], river::Origin>,
}
impl Sampler<'_> {
    fn cell(&mut self, journal: &EditJournal, p: [i64; 3]) -> Option<FluidCell> {
        let block = self.chunks.block_at(p)?;
        if !super::can_hold_fluid(block) {
            return Some(FluidCell::default());
        }
        let cell = journal.fluid(p).unwrap_or_else(|| {
            let natural = self.natural(p);
            if let Some(c) = self
                .columns
                .get(&[p[0], p[2]])
                .filter(|c| c.falls.iter().any(Option::is_some))
            {
                super::falls::sampled(*c, p, journal)
            } else {
                natural
            }
        });
        Some(self.space(journal, p).clamp(cell))
    }
    fn natural(&mut self, p: [i64; 3]) -> FluidCell {
        let column = *self.columns.entry([p[0], p[2]]).or_insert_with(|| {
            self.chunks.surface_column_at(p[0], p[2]).map_or_else(
                || {
                    self.terrain
                        .water_columns(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
                },
                |surface| surface.water_columns(),
            )
        });
        from_columns(column, p[1])
    }
    fn space(&mut self, journal: &EditJournal, p: [i64; 3]) -> &super::space::FluidSpace {
        self.spaces.entry(p).or_insert_with(|| {
            super::space::FluidSpace::wet_at(
                journal,
                p,
                |q| self.chunks.block_at(q),
                |q, material| {
                    self.chunks
                        .raw_fluid_cell_at(self.terrain, journal, q, material)
                },
            )
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
        // Waterlogging fills a bank from the wet side; it is not an outlet
        // through that bank into a previously dry neighbouring cell.
        if p[1] == q[1]
            && self
                .chunks
                .block_at(p)
                .is_some_and(terra_core::construction::is_shape)
            && !self
                .chunks
                .block_at(q)
                .is_some_and(terra_core::construction::is_shape)
        {
            return false;
        }
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
    simulate_sampled_step(
        Sampler {
            terrain,
            chunks,
            columns: HashMap::new(),
            spaces: HashMap::new(),
            river_columns: HashMap::new(),
            river_origins: HashMap::new(),
        },
        journal,
        budget,
    )
}

fn simulate_sampled_step(
    mut sampler: Sampler<'_>,
    journal: &mut EditJournal,
    budget: usize,
) -> Result<FluidStep, FluidError> {
    let terrain = sampler.terrain;
    let chunks = sampler.chunks;
    let mut result = FluidStep::default();
    if !terrain.config().water {
        return Ok(result);
    }
    let budget = budget.min(4096);
    let mut visited = HashSet::new();
    let mut resting = pools::Resting::default();
    let mut sources = std::mem::take(&mut journal.fluid_sources);
    sources.begin_step(&mut sampler, journal);
    let mut work: VecDeque<_> = journal.take_fluid_work(budget).into();
    while result.visited < budget {
        let Some(p) = work.pop_front() else { break };
        if !visited.insert(p) {
            continue;
        }
        result.visited += 1;
        if resting.handled(p) {
            continue;
        }
        let Some(cell) = sampler.cell(journal, p) else {
            journal.sleep_fluid(p, global_to_section(p).0);
            continue;
        };
        if !chunks.block_at(p).is_some_and(super::can_hold_fluid) {
            continue;
        }
        if journal.fluid(p).is_none()
            && chunks.surface_column_at(p[0], p[2]).is_some_and(|c| {
                c.falls
                    .into_iter()
                    .flatten()
                    .any(|s| s.contains(p[1]) && s.live(p, journal))
            })
        {
            // Each segment is continuously supplied by its prepared source.
            // No tick or journal entries are required for the resting stream.
            continue;
        }
        if cell.amount > 0 {
            // Pending proof and recession still contain real, visible water.
            result.wetted.push(p);
        }
        if cell.amount == 0 {
            // A live inlet may already have paid the hidden part below a
            // model's first exposed film. Let its next unit open that film
            // before retiring an otherwise impossible, unfunded residue.
            match sources.can_receive(&mut sampler, journal, p) {
                sources::Support::Connected => {
                    flux::advance_supplied(&mut sampler, journal, p, &mut result)?;
                    continue;
                }
                sources::Support::Pending(waiting) => {
                    if let Some(section) = waiting {
                        journal.sleep_fluid(p, section);
                    } else {
                        journal.wake_fluid(p);
                    }
                    continue;
                }
                sources::Support::Disconnected => {}
            }
            if sampler
                .cell(journal, p)
                .is_some_and(|cell| cell.amount == 0)
                && let Some(hidden) = journal.fluid(p).filter(|cell| cell.amount > 0)
            {
                let empty = FluidCell::default();
                pools::preflight(journal, &[(p, empty)])?;
                result.drained += u64::from(hidden.amount);
                result.changed |=
                    journal.set_receded_fluid(p, empty, sampler.natural(p).amount > 0)?;
                sources.forget(p);
            }
            continue;
        }
        if cell.amount > 0 && !cell.source {
            match sources.support(&mut sampler, journal, p) {
                sources::Support::Connected => {}
                sources::Support::Pending(waiting) => {
                    if let Some(section) = waiting {
                        journal.sleep_fluid(p, section);
                    } else {
                        journal.wake_fluid(p);
                    }
                    continue;
                }
                sources::Support::Disconnected => {
                    let mut next = cell;
                    next.amount = next.amount.saturating_sub(1);
                    next.flow = [0; 2];
                    // A model's last visible film can require height5. Never
                    // leave the remaining height4 hidden inside its solid half.
                    next = sampler.space(journal, p).clamp(next);
                    pools::preflight(journal, &[(p, next)])?;
                    result.drained += u64::from(cell.amount - next.amount);
                    let pin_empty = next.amount == 0 && sampler.natural(p).amount > 0;
                    result.changed |= journal.set_receded_fluid(p, next, pin_empty)?;
                    if next.amount == 0 {
                        sources.forget(p);
                    }
                    continue;
                }
            }
        }
        // Empty members must join the same atomic pool step before ordinary
        // finite lateral supply can copy a neighbour's level into them.
        if !river::bank_overflow(&mut sampler, journal, p)
            && resting.settle(&mut sampler, journal, p, &mut result)?
        {
            continue;
        }
        if cell.source {
            flux::advance(&mut sampler, journal, p, &mut result)?;
        } else {
            flux::advance_supplied(&mut sampler, journal, p, &mut result)?;
        }
    }
    for p in work {
        if !visited.contains(&p) {
            journal.wake_fluid(p);
        }
    }
    journal.fluid_sources = sources;
    let mut torches = std::collections::BTreeSet::new();
    for &p in &result.wetted {
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let q = [p[0] + x, p[1] + y, p[2] + z];
                    if matches!(
                        chunks.block_at(q),
                        Some(
                            40 | terra_core::block_shapes::VIOLET_TORCH
                                | terra_core::lantern::LANTERN
                        )
                    ) {
                        torches.insert(q);
                    }
                }
            }
        }
    }
    result.flooded_torches = torches
        .into_iter()
        .filter(|&p| crate::torches::flooded(terrain, chunks, journal, p))
        .collect();
    retire_flooded_props(terrain, chunks, journal, &mut result)?;
    Ok(result)
}

#[cfg(test)]
mod cascade_tests;
#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod partial_tests;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod waterlogged_pipe_tests;

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
    // Retain the fractional step even when rendering or the server frame runs
    // late. Perform at most one bounded simulation step per update.
    let elapsed = time.elapsed_secs_f64() - *last;
    *last += (elapsed / TICK_SECONDS).floor() * TICK_SECONDS;
    let started = std::time::Instant::now();
    match simulate_fluid_step(
        &terrain.0,
        &chunks,
        journal.bypass_change_detection(),
        CELL_BUDGET,
    ) {
        Ok(step) => {
            for p in step.lava_contacts {
                chunks.wake_lava(p);
            }
            if !step.flooded_torches.is_empty()
                && let Err(error) =
                    crate::torches::wash_away(&mut chunks, &mut journal, &step.flooded_torches)
            {
                tracing::error!(%error, "Flooded torch removal rejected");
            }
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
        for d in crate::lava::NEIGHBORS {
            let other = std::array::from_fn(|i| p[i] + d[i]);
            if chunks.lava_at(other) {
                step.lava_contacts.push(other);
            }
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
            .filter(|p| {
                p.kind != crate::props::PropKind::AppleFruit
                    && !p.removed
                    && !p.water_tolerant()
                    && !journal.prop_removed(owner, p.id)
            })
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
