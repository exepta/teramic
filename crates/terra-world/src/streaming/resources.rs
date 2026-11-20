use crate::{TerraGenerator, generation::sections::GeneratedSection};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::{prelude::*, tasks::Task};
use std::collections::VecDeque;
use std::sync::Arc;
use terra_voxel::meshing::BlockMesh;
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[cfg(test)]
#[path = "confirmed_delta_tests.rs"]
mod confirmed_delta_tests;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GenerationStatus {
    #[default]
    Pending,
    Ready,
    Failed(String),
}
#[derive(Resource, Default)]
pub struct WorldGeneration {
    pub status: GenerationStatus,
    pub loaded_sections: usize,
    pub pending_jobs: usize,
    pub wanted_sections: usize,
    pub prepared_columns: usize,
}
#[derive(Resource, Clone)]
pub struct WorldTerrain(pub TerraGenerator);
#[derive(Resource, Debug, Clone)]
pub struct ChunkFocus(pub [f64; 3]);
/// Server-owned simulation centers. None keeps the single-focus fixture/client path.
#[derive(Resource, Debug, Default)]
pub struct ChunkFoci(pub Option<Vec<[f64; 3]>>);

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WorldAccess {
    #[default]
    Local,
    Server,
    Replica {
        ready: bool,
    },
}
impl WorldAccess {
    pub fn ready(self) -> bool {
        !matches!(self, Self::Replica { ready: false })
    }
    pub fn authoritative(self) -> bool {
        !matches!(self, Self::Replica { .. })
    }
}

/// Drop replica state before a new session. Render/collider caches observe the
/// empty store while the new transport handshake is in progress.
pub fn reset_replica(world: &mut World) {
    world.insert_resource(WorldAccess::Replica { ready: false });
    world.insert_resource(ChunkStore::default());
    world.insert_resource(PendingSections::default());
    world.insert_resource(super::water_jobs::PendingWaterJobs::default());
    world.insert_resource(WorldGeneration::default());
    world.insert_resource(crate::edits::EditJournal::default());
    if let Some(mut messages) = world.get_resource_mut::<Messages<SectionPublished>>() {
        messages.clear();
    }
    if let Some(mut messages) = world.get_resource_mut::<Messages<SectionRemoved>>() {
        messages.clear();
    }
    if let Some(mut messages) = world.get_resource_mut::<Messages<PreparedColumnChanged>>() {
        messages.clear();
    }
    if let Some(mut messages) = world.get_resource_mut::<Messages<WaterPublished>>() {
        messages.clear();
    }
    if let Some(mut messages) = world.get_resource_mut::<Messages<SectionEditApplied>>() {
        messages.clear();
    }
}
#[derive(Resource, Clone, Copy)]
pub struct StreamingSettings {
    pub radius: i64,
    pub vertical_radius: i64,
    pub max_jobs: usize,
    pub meshes: bool,
}

/// Frontends may reveal a nearby playable world before streaming a distant view.
/// The requested graphics distance stays intact; this only limits current work.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamingRadiusLimit(pub Option<u8>);
pub struct LoadedSection {
    pub data: BlockSection,
    /// CPU output consumed by terra-render. Dedicated servers do not request it.
    pub mesh: Option<BlockMesh>,
    pub mesh_epoch: u64,
    pub water_epoch: u64,
    pub water: Option<crate::water::WaterPatch>,
    pub props: Vec<crate::props::WorldProp>,
}
/// A generated section was installed or replaced in `ChunkStore::loaded`.
/// The prepared column is available before this message. Other store changes,
/// including removals and water-only patches, do not emit it.
#[derive(Message, Debug, Clone, Copy)]
pub struct SectionPublished {
    pub position: SectionPos,
}
/// A water-only mesh replaced an existing section's visual patch. Block payloads
/// are unchanged, so consumers must not treat this as a terrain publication.
#[derive(Message, Debug, Clone, Copy)]
pub struct WaterPublished {
    pub position: SectionPos,
}
/// A confirmed replica delta changed section metadata, edits or loaded props.
/// No generated block payload was published; consumers refresh this column's
/// metadata without invalidating every loaded section. Confirmed sparse voxel
/// overrides are already visible to readers while the render mesh is pending.
#[derive(Message, Debug, Clone, Copy)]
pub struct SectionEditApplied {
    pub position: SectionPos,
}
/// A loaded section was evicted by a streaming-interest change.
#[derive(Message, Debug, Clone, Copy)]
pub struct SectionRemoved {
    pub position: SectionPos,
}
/// An immutable prepared column was installed, replaced or evicted.
/// Consumers inspect the current store to distinguish presence from removal.
#[derive(Message, Debug, Clone, Copy)]
pub struct PreparedColumnChanged {
    pub coordinate: [i64; 2],
}
#[derive(Resource, Default)]
pub struct ChunkStore {
    pub(super) support_queue: VecDeque<[i64; 3]>,
    pub loaded: HashMap<SectionPos, LoadedSection>,
    pub(super) wanted: HashSet<SectionPos>,
    pub(super) dirty: HashSet<SectionPos>,
    pub(super) water_dirty: HashSet<SectionPos>,
    pub(super) queue: super::queue::SectionQueue,
    pub(super) surface_ranges: HashMap<[i64; 2], [i64; 2]>,
    pub(super) columns: HashMap<[i64; 2], Arc<crate::PreparedColumn>>,
    pub(super) cave_views: Vec<(SectionPos, i64)>,
    /// Forgotten overrides need their generated base before collision is known.
    pub(super) unknown_voxel_bases: HashMap<SectionPos, HashSet<[usize; 3]>>,
}
impl ChunkStore {
    pub(crate) fn invalidate_sections(&mut self, positions: impl IntoIterator<Item = SectionPos>) {
        for position in positions {
            if self.is_wanted(position) {
                self.dirty.insert(position);
            }
        }
    }

    pub(crate) fn request_sections(&mut self, positions: impl IntoIterator<Item = SectionPos>) {
        for position in positions {
            if self.wanted.insert(position) && !self.loaded.contains_key(&position) {
                self.queue.push_back(position);
            }
        }
    }

    pub fn cave_structures_enabled(&self) -> bool {
        self.columns
            .values()
            .next()
            .is_some_and(|c| c.cave_structures_enabled())
    }
    pub fn surface_column_at(&self, x: i64, z: i64) -> Option<&crate::SurfaceColumn> {
        self.columns
            .get(&[x.div_euclid(16), z.div_euclid(16)])?
            .surface_at(x, z)
    }
    /// Shared immutable generation data for low-priority cartography workers.
    pub fn prepared_column(&self, coordinate: [i64; 2]) -> Option<Arc<crate::PreparedColumn>> {
        self.columns.get(&coordinate).cloned()
    }

    /// Unique callers may deduplicate by stable settlement ID. Never plans regions.
    pub fn village_settlements(
        &self,
    ) -> impl Iterator<Item = &Arc<crate::generation::structures::VillageSettlement>> {
        self.columns
            .values()
            .flat_map(|column| column.village_settlements())
    }

    /// Settlement metadata already prepared by streaming; never schedules generation.
    pub fn village_residents(
        &self,
    ) -> impl Iterator<Item = &crate::generation::structures::ResidentSite> {
        self.columns
            .values()
            .flat_map(|column| column.village_residents())
    }

    #[cfg(test)]
    pub(crate) fn set_prepared_column_for_test(
        &mut self,
        coordinate: [i64; 2],
        column: Option<Arc<crate::PreparedColumn>>,
    ) {
        if let Some(column) = column {
            self.columns.insert(coordinate, column);
        } else {
            self.columns.remove(&coordinate);
        }
    }

    /// The prepared height/water field is immutable; live fluid overrides always
    /// win. Reuses generation data instead of repeating climate/river noise in AI.
    pub fn fluid_cell_at(
        &self,
        terrain: &TerraGenerator,
        journal: &crate::edits::EditJournal,
        global: [i64; 3],
        block: u16,
    ) -> crate::water::FluidCell {
        let fluid = self.raw_fluid_cell_at(terrain, journal, global, block);
        if crate::water::space::partial(block) {
            crate::water::space::FluidSpace::wet_at(
                journal,
                global,
                |p| self.block_at(p),
                |p, material| self.raw_fluid_cell_at(terrain, journal, p, material),
            )
            .clamp(fluid)
        } else {
            fluid
        }
    }

    /// Unclipped occupancy for bounded shape-region queries. Never recurse
    /// through the neighbouring fence's own clipping query.
    pub(crate) fn raw_fluid_cell_at(
        &self,
        terrain: &TerraGenerator,
        journal: &crate::edits::EditJournal,
        global: [i64; 3],
        block: u16,
    ) -> crate::water::FluidCell {
        if !terrain.config().water || !crate::water::can_hold_fluid(block) {
            return crate::water::FluidCell::default();
        }
        if let Some(fluid) = journal.fluid(global) {
            fluid
        } else if let Some(surface) = self
            .columns
            .get(&[global[0].div_euclid(16), global[2].div_euclid(16)])
            .and_then(|column| column.surface_at(global[0], global[2]))
        {
            crate::water::fluid::from_columns(surface.water_columns(), global[1])
        } else {
            crate::water::fluid::natural_cell(terrain, global)
        }
    }
    pub fn apply_confirmed_delta(
        &mut self,
        journal: &mut crate::edits::EditJournal,
        delta: crate::edits::SectionDelta,
    ) -> Result<(), String> {
        let owner = delta.position;
        let previous_prop_epoch = journal.prop_epoch(owner);
        let accepted = delta.revision >= journal.revision(owner)
            && delta.prop_epoch >= previous_prop_epoch
            && delta.appearance_epoch >= journal.appearance_epoch(owner)
            && delta.fluid_epoch >= journal.fluid_epoch(owner);
        let previous_cells: Vec<_> = if accepted && self.loaded.contains_key(&owner) {
            journal
                .blocks_in_section(owner)
                .map(|(cell, _)| cell)
                .collect()
        } else {
            Vec::new()
        };
        let affected = journal.import_replica_delta(delta)?;
        // Publication of the visible mesh can lag behind a confirmed edit.
        // Material/collision consumers need the accepted voxel values now.
        if accepted && let Some(section) = self.loaded.get_mut(&owner) {
            let known: HashSet<_> = journal
                .blocks_in_section(owner)
                .map(|(cell, _)| cell)
                .collect();
            let unresolved = self.unknown_voxel_bases.entry(owner).or_default();
            unresolved.extend(
                previous_cells
                    .into_iter()
                    .filter(|cell| !known.contains(cell)),
            );
            unresolved.retain(|cell| !known.contains(cell));
            if unresolved.is_empty() {
                self.unknown_voxel_bases.remove(&owner);
            }
            let mut blocks: Option<Vec<u16>> = None;
            for (cell, material) in journal.blocks_in_section(owner) {
                let index = cell[0] + 16 * (cell[1] + 16 * cell[2]);
                if section.data.blocks()[index] != material {
                    blocks.get_or_insert_with(|| section.data.blocks().to_vec())[index] = material;
                }
            }
            if let Some(blocks) = blocks {
                section.data = BlockSection::from_blocks(owner, journal.revision(owner), blocks)
                    .map_err(|error| error.to_string())?;
            }
        }
        for position in affected {
            if self.is_wanted(position) {
                self.dirty.insert(position);
            }
        }
        if let Some(section) = self.loaded.get_mut(&owner)
            && (journal.prop_epoch(owner) != previous_prop_epoch
                || journal.props_need_sync(owner, &section.props))
        {
            journal.apply_prop_removals(owner, &mut section.props);
        }
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    if let Ok(p) = owner.offset([x, y, z])
                        && self
                            .loaded
                            .get(&p)
                            .is_some_and(|s| s.water_epoch != journal.water_mesh_epoch(p))
                    {
                        self.water_dirty.insert(p);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn block_at(&self, global: [i64; 3]) -> Option<u16> {
        let (p, local) = global_to_section(global);
        if self
            .unknown_voxel_bases
            .get(&p)
            .is_some_and(|cells| cells.contains(&local))
        {
            return None;
        }
        self.loaded.get(&p)?.data.block(local).ok()
    }
    /// A render remesh may be pending while confirmed voxel/collision data is ready.
    /// Removing an override instead makes its generated base explicitly unknown.
    pub fn voxel_data_known(&self, position: SectionPos) -> bool {
        self.loaded.contains_key(&position) && !self.voxel_base_pending(position)
    }
    /// A forgotten override cannot supply a known halo shape until regenerated.
    pub fn voxel_base_pending(&self, position: SectionPos) -> bool {
        self.unknown_voxel_bases
            .get(&position)
            .is_some_and(|cells| !cells.is_empty())
    }
    /// Copies confirmed voxels independently of the visible render mesh.
    /// Forgotten generated bases remain unavailable until section publication.
    pub fn confirmed_voxels(
        &self,
        position: SectionPos,
        journal: &crate::edits::EditJournal,
    ) -> Result<BlockSection, String> {
        if !self.voxel_data_known(position) {
            return Err("Confirmed voxel base is not known".into());
        }
        let section = self.loaded.get(&position).ok_or("Section is not loaded")?;
        if section.mesh_epoch > journal.mesh_epoch(position) {
            return Err("Section is newer than its confirmed edits".into());
        }
        let mut blocks = section.data.blocks().to_vec();
        for (cell, material) in journal.blocks_in_section(position) {
            blocks[cell[0] + 16 * (cell[1] + 16 * cell[2])] = material;
        }
        BlockSection::from_blocks(position, journal.revision(position), blocks)
            .map_err(|error| error.to_string())
    }
    pub fn is_wanted(&self, p: SectionPos) -> bool {
        self.wanted.contains(&p)
    }
    pub(crate) fn mark_dirty_if_wanted(&mut self, position: SectionPos) {
        if self.is_wanted(position) {
            self.dirty.insert(position);
        }
    }
    /// Current streaming interest, including sections whose jobs have not finished.
    pub fn wanted_sections(&self) -> impl Iterator<Item = SectionPos> + '_ {
        self.wanted.iter().copied()
    }
    /// Connected loaded liquid span containing this cell, for swimming and
    /// underwater effects. Solid barriers split spans even inside a natural lake.
    pub fn water_column_at(
        &self,
        terrain: &TerraGenerator,
        journal: &crate::edits::EditJournal,
        global: [i64; 3],
    ) -> Option<crate::water::WaterColumn> {
        self.water_span(terrain, journal, global.map(|v| v as f64 + 0.5), false)
    }

    /// Exact liquid span through a world-space point. Slab/stair solids split
    /// the span instead of making the entire partially wet voxel swimmable.
    pub fn water_column_at_point(
        &self,
        terrain: &TerraGenerator,
        journal: &crate::edits::EditJournal,
        position: [f64; 3],
    ) -> Option<crate::water::WaterColumn> {
        self.water_span(terrain, journal, position, true)
    }

    fn water_span(
        &self,
        terrain: &TerraGenerator,
        journal: &crate::edits::EditJournal,
        position: [f64; 3],
        exact: bool,
    ) -> Option<crate::water::WaterColumn> {
        if position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > (1_i64 << 40) as f64)
        {
            return None;
        }
        let global = position.map(|v| v.floor() as i64);
        if !terrain.config().water {
            return None;
        }
        if self.block_at(global) == Some(terra_core::plumbing::BASIN) {
            return crate::plumbing::basin_water_column(self, journal, position);
        }
        let natural = self
            .columns
            .get(&[global[0].div_euclid(16), global[2].div_euclid(16)])
            .and_then(|c| c.surface_at(global[0], global[2]))
            .map_or_else(
                || terrain.water_columns(global[0] as f64 + 0.5, global[2] as f64 + 0.5),
                |s| s.water_columns(),
            );
        // The entry face selects the connected segment in neighbouring cells:
        // -1 enters their bottom, +1 their top, 0 samples the initial point.
        let cell_at = |y, entry_face: i8| {
            let p = [global[0], y, global[2]];
            if !self.block_at(p).is_some_and(crate::water::can_hold_fluid) {
                return None;
            }
            let cell = journal
                .fluid(p)
                .unwrap_or_else(|| crate::water::fluid::from_columns(natural, y));
            let block = self.block_at(p)?;
            if cell.amount == 0 {
                return None;
            }
            if !crate::water::space::partial(block) {
                return Some((0.0, cell.height(), cell));
            }
            let space = crate::water::space::FluidSpace::wet_at(
                journal,
                p,
                |q| self.block_at(q),
                |q, material| self.raw_fluid_cell_at(terrain, journal, q, material),
            );
            let x = position[0] - global[0] as f64;
            let z = position[2] - global[2] as f64;
            let mut intervals: Vec<_> = space
                .wet_boxes(cell.height())
                .into_iter()
                .filter(|b| {
                    x >= b.minimum[0] && x < b.maximum[0] && z >= b.minimum[2] && z < b.maximum[2]
                })
                .map(|b| (b.minimum[1], b.maximum[1]))
                .collect();
            intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            let mut merged = Vec::<(f64, f64)>::with_capacity(intervals.len());
            for (lo, hi) in intervals {
                if let Some(previous) = merged.last_mut()
                    && lo <= previous.1 + 1e-8
                {
                    previous.1 = previous.1.max(hi);
                } else {
                    merged.push((lo, hi));
                }
            }
            // wet_boxes partitions the same connected liquid into many AABBs.
            // Their touching vertical boundaries are not a bed or surface;
            // retain only genuine gaps made by solid geometry at this X/Z.
            merged
                .into_iter()
                .filter(|&(lo, hi)| match entry_face {
                    -1 => lo <= 1e-8,
                    1 => hi >= 1.0 - 1e-8,
                    _ => !exact || position[1] >= y as f64 + lo && position[1] < y as f64 + hi,
                })
                .map(|(lo, hi)| (lo, hi, space.clamp(cell)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
        };
        let (mut lower, mut upper_height, mut upper) = cell_at(global[1], 0)?;
        let mut top = global[1];
        let mut bottom = top;
        for _ in 0..256 {
            if upper_height < 1.0 - 1e-8 {
                break;
            }
            let next = top.checked_add(1)?;
            let Some((lo, hi, above)) = cell_at(next, -1) else {
                break;
            };
            if lo > 1e-8 {
                break;
            }
            top = next;
            upper = above;
            upper_height = hi;
        }
        for _ in 0..256 {
            if lower > 1e-8 {
                break;
            }
            let next = bottom.checked_sub(1)?;
            let Some((lo, hi, _)) = cell_at(next, 1) else {
                break;
            };
            if hi < 1.0 - 1e-8 {
                break;
            }
            bottom = next;
            lower = lo;
        }
        let top_is_placed = self.block_at([global[0], top, global[2]])
            == Some(terra_voxel::MaterialId::FluidWater as u16);
        let level = if upper.source && !top_is_placed {
            natural.at(top).map_or(top as f64 + upper_height, |column| {
                column.level.min(top as f64 + upper_height)
            })
        } else {
            top as f64 + upper_height
        };
        Some(crate::water::WaterColumn {
            level,
            bed: bottom as f64 + lower,
            depth: level - (bottom as f64 + lower),
        })
    }
}
pub(super) struct SectionJob {
    pub positions: Vec<SectionPos>,
    pub task: Task<ColumnOutput>,
}
pub(super) struct ColumnOutput {
    pub coordinate: [i64; 2],
    pub remesh: bool,
    pub prepared: Option<Arc<crate::PreparedColumn>>,
    pub sections: Vec<(SectionPos, u64, u64, Result<GeneratedSection, String>)>,
}
#[derive(Resource, Default)]
pub(super) struct PendingSections(pub HashMap<SectionPos, SectionJob>);
impl PendingSections {
    pub fn contains(&self, p: &SectionPos) -> bool {
        self.0.values().any(|j| j.positions.contains(p))
    }
}
/// Local interaction intent. Network replicas forward it; only the authority commits it.
#[derive(Message, Debug, Clone, Copy)]
pub struct BlockEditRequest {
    pub position: [i64; 3],
    pub expected_material: u16,
    pub material: u16,
}
#[derive(Message, Debug, Clone, Copy)]
pub struct PropRemoveRequest {
    pub id: crate::props::PropId,
    pub section: SectionPos,
}
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum StreamingSet {
    Focus,
    Interest,
    Edits,
    Receive,
    Dispatch,
}

#[cfg(test)]
#[path = "water_column_tests.rs"]
mod water_column_tests;
