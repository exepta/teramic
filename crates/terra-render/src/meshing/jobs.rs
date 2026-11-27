use super::{
    DetailLevel, RenderStatus, RenderedSections,
    builder::{MeshBatch, RenderOptions, compose_section_with_leaf_detail},
    lod::{distance_squared, select_level},
};
use crate::blocks::BlockAssets;
use bevy::{
    ecs::{message::MessageCursor, system::SystemParam},
    math::DVec2,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use terra_voxel::{SectionPos, meshing::BlockMesh};
use terra_world::{
    edits::EditJournal,
    streaming::{
        ChunkFocus, ChunkStore, PreparedColumnChanged, SectionEditApplied, SectionPublished,
        SectionRemoved, WaterPublished, WorldTerrain,
    },
};

pub(crate) type MeshResult = (Result<Vec<MeshBatch>, String>, f64);

pub(crate) fn refresh_growth(
    geometry: &mut Arc<BlockMesh>,
    section: SectionPos,
    terrain: &terra_world::TerraGenerator,
    edits: &terra_world::generation::sections::BlockOverrides,
) {
    if edits.is_empty() {
        return;
    }
    if !geometry.segments.is_empty() {
        let Ok(origin) = section.origin() else {
            return;
        };
        let generator = terra_world::SectionGenerator::new(terrain.clone());
        let mut columns = HashMap::new();
        for segment in &mut Arc::make_mut(geometry).segments {
            let global = std::array::from_fn(|a| origin[a] + i64::from(segment.cell[a]));
            let profile = generator.segment_profile(global, segment.material, edits, |[x, z]| {
                *columns
                    .entry([x, z])
                    .or_insert_with(|| terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5))
            });
            segment.below = profile.below;
            segment.above = profile.above;
            segment.hanging = profile.hanging;
            segment.joined = profile.joined;
            segment.fan = profile.fan;
        }
    }
    if terrain.config().cave_structure_revision > 0 {
        return;
    }
    if edits.is_empty()
        || !geometry
            .faces
            .iter()
            .any(|face| matches!(face.material, 30 | 37))
    {
        return;
    }
    let Ok(origin) = section.origin() else {
        return;
    };
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    let mut sampled = HashMap::new();
    let growth = terra_world::ore_shapes::salt_growth_snapshot(&geometry.faces, origin, |p| {
        edits.get(&p).copied().or_else(|| {
            *sampled
                .entry(p)
                .or_insert_with(|| generator.block_at(p).ok())
        })
    });
    Arc::make_mut(geometry).mineral_growth = growth;
}
pub(crate) struct RenderJob {
    pub folded_leaves: Option<super::lod::FoldedLeaves>,
    pub ready: Option<MeshResult>,
    pub micro_detail: bool,
    pub epoch: u64,
    pub appearance_epoch: u64,
    pub level: DetailLevel,
    pub task: Task<(Result<Vec<MeshBatch>, String>, f64)>,
}

impl RenderJob {
    fn release_staged(&mut self, meshes: Option<&mut Assets<Mesh>>) {
        if let (Some((Ok(batches), _)), Some(meshes)) = (&mut self.ready, meshes) {
            crate::upload_budget::release_staged(batches, meshes);
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct RenderJobs(pub HashMap<SectionPos, RenderJob>, pub Vec<SectionPos>);

impl RenderJobs {
    pub(super) fn cancel(&mut self, position: &SectionPos, meshes: Option<&mut Assets<Mesh>>) {
        if let Some(mut job) = self.0.remove(position) {
            job.release_staged(meshes);
        }
    }
}
#[derive(Default)]
pub(crate) struct MeshQueue {
    visibility_revision: u64,
    focus: Option<[f64; 3]>,
    positions: Vec<SectionPos>,
    queued: HashSet<SectionPos>,
    priority_dirty: bool,
    motion: FlightMotion,
    dirty: bool,
    selection_dirty: bool,
    initial_ready: bool,
    frame_load: f32,
    pace_workers: bool,
    arrival_scan: bool,
    arrivals: HashSet<SectionPos>,
    sweep: Option<SelectionSweep>,
}

struct SelectionSweep {
    positions: Vec<SectionPos>,
    full_selection: bool,
}

#[derive(Default)]
struct FlightMotion {
    origin: Option<[f64; 3]>,
    seconds: f64,
    heading: Option<DVec2>,
}

impl FlightMotion {
    fn update(&mut self, focus: [f64; 3], dt: f64) -> bool {
        let origin = *self.origin.get_or_insert(focus);
        let delta = DVec2::new(focus[0] - origin[0], focus[2] - origin[2]);
        self.seconds += dt;
        // Average over several ticks. A paused frame or a teleport provides
        // no reliable heading for streaming the intervening terrain.
        let discontinuity = dt <= 0. || dt > 0.30 || delta.length_squared() > 32.0_f64.powi(2);
        if !discontinuity && self.seconds < 0.12 {
            return false;
        }
        let speed = delta.length() / self.seconds.max(f64::EPSILON);
        let next = (!discontinuity && (16.0..=100.0).contains(&speed)).then(|| delta.normalize());
        self.origin = Some(focus);
        self.seconds = 0.;
        if self.heading.zip(next).is_some_and(|(a, b)| a.dot(b) > 0.98) || self.heading == next {
            return false;
        }
        self.heading = next;
        true
    }
}

fn flight_surface_rank(
    p: SectionPos,
    focus: [f64; 3],
    heading: DVec2,
    surface: impl FnOnce() -> Option<f64>,
) -> u8 {
    let delta = DVec2::new(
        p.x as f64 * 16. + 8. - focus[0],
        p.z as f64 * 16. + 8. - focus[2],
    );
    let local = delta.length_squared() <= 32.0_f64.powi(2);
    if !local
        && (!(0.0..=80.0).contains(&delta.dot(heading)) || delta.perp_dot(heading).abs() > 40.)
    {
        return 2;
    }
    if surface().is_some_and(|y| {
        ((y - 0.5).floor() as i64).div_euclid(16) == p.y
            || local && distance_squared(p, focus) <= 16.0_f64.powi(2)
    }) {
        return u8::from(!local);
    }
    2
}

impl MeshQueue {
    /// Large woodlands can fill all composition slots with multi-megabyte jobs
    /// while the render thread is still publishing their predecessors. Retain
    /// full throughput on the loading screen and whenever the frame has room.
    fn refinement_limit(&mut self, configured: usize, frame_seconds: f32, fps_limit: u16) -> usize {
        if !self.initial_ready || frame_seconds <= 0.0 {
            self.frame_load = 1.0;
            self.pace_workers = false;
            return configured;
        }
        let target = if fps_limit == 0 {
            60
        } else {
            fps_limit.min(60)
        };
        let load = (frame_seconds * f32::from(target)).min(3.0);
        self.frame_load += (load - self.frame_load) * 0.0625;
        if self.frame_load > 1.05 {
            self.pace_workers = true;
        } else if self.frame_load < 0.87 {
            self.pace_workers = false;
        }
        if self.pace_workers {
            configured.min(3)
        } else {
            configured
        }
    }

    fn uploads_consumed(chunks: &ChunkStore, mut hidden: impl FnMut(SectionPos) -> bool) -> bool {
        // Portal-culled sections deliberately retain CPU geometry for later
        // exposure. They must not keep the renderer in unbudgeted startup mode.
        chunks
            .loaded
            .iter()
            .all(|(&position, section)| section.mesh.is_none() || hidden(position))
    }

    fn enqueue(&mut self, position: SectionPos) -> bool {
        if self.queued.insert(position) {
            self.positions.push(position);
            self.priority_dirty = true;
            true
        } else {
            false
        }
    }

    fn take_incremental_candidates(&mut self) -> HashSet<SectionPos> {
        let mut candidates = std::mem::take(&mut self.arrivals);
        candidates.extend(self.positions.drain(..));
        self.queued.clear();
        candidates
    }

    fn retain_loaded(&mut self, mut loaded: impl FnMut(SectionPos) -> bool) {
        self.positions.retain(|&position| loaded(position));
        self.queued.retain(|&position| loaded(position));
        self.arrivals.retain(|&position| loaded(position));
    }

    fn remove_unloaded(
        &mut self,
        removals: impl IntoIterator<Item = SectionPos>,
        mut loaded: impl FnMut(SectionPos) -> bool,
    ) -> HashSet<SectionPos> {
        let mut removed: HashSet<_> = removals.into_iter().collect();
        // A section may have reloaded since its removal was published.
        removed.retain(|&position| !loaded(position));
        if !removed.is_empty() {
            // Flight can unload hundreds of sections together. Scan the queue
            // once for the batch instead of once for every removed section.
            self.retain_loaded(loaded);
        }
        removed
    }

    fn begin_sweep(&mut self, positions: impl FnOnce() -> Vec<SectionPos>) {
        // Changes during a sweep request one catch-up pass; restarting on each
        // moving focus would starve the last sections of the current snapshot.
        if self.sweep.is_none() && (self.selection_dirty || self.arrival_scan) {
            self.sweep = Some(SelectionSweep {
                positions: positions(),
                full_selection: std::mem::take(&mut self.selection_dirty),
            });
            self.arrival_scan = false;
        }
    }

    fn sweep_candidate(
        &mut self,
        inspected: usize,
        elapsed: Duration,
    ) -> Option<(SectionPos, bool)> {
        if inspected >= 1_024 || (inspected != 0 && elapsed >= Duration::from_micros(750)) {
            return None;
        }
        let sweep = self.sweep.as_mut()?;
        Some((sweep.positions.pop()?, sweep.full_selection))
    }

    fn finish_sweep(&mut self) {
        if self
            .sweep
            .as_ref()
            .is_some_and(|sweep| sweep.positions.is_empty())
        {
            self.sweep = None;
        }
    }

    fn has_selection_work(&self) -> bool {
        self.dirty
            || self.selection_dirty
            || self.arrival_scan
            || self.sweep.is_some()
            || !self.arrivals.is_empty()
    }

    fn update_initial_ready(&mut self, terrain_changed: bool, ready: bool) {
        // A system local survives world switches; that frame's Ready can still
        // belong to the previous world.
        if terrain_changed {
            self.initial_ready = false;
            self.motion = FlightMotion::default();
            self.sweep = None;
            self.positions.clear();
            self.queued.clear();
            self.arrivals.clear();
        } else {
            self.initial_ready |= ready;
        }
    }

    #[cfg(test)]
    fn prioritize(&mut self, focus: [f64; 3]) {
        self.positions.sort_unstable_by_key(|p| {
            std::cmp::Reverse((distance_squared(*p, focus) as u64, p.y, p.z, p.x))
        });
    }

    fn prioritize_work(
        &mut self,
        focus: [f64; 3],
        journal: &EditJournal,
        rendered: &RenderedSections,
        chunks: &ChunkStore,
        cache: &GeometryCache,
    ) {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.prioritize_queue");
        let heading = self.motion.heading;
        self.prioritize_cached(focus, |p| {
            let priority = MeshPriority::for_section(p, journal, rendered);
            let mut rank = 2;
            if priority == MeshPriority::Missing
                && let Some(heading) = heading
            {
                rank = flight_surface_rank(p, focus, heading, || {
                    let mesh = chunks
                        .loaded
                        .get(&p)?
                        .mesh
                        .as_ref()
                        .or_else(|| cache.0.get(&p).map(|c| c.geometry.as_ref()))?;
                    if mesh.faces.is_empty() && mesh.segments.is_empty() {
                        return None;
                    }
                    chunks
                        .surface_column_at(p.x * 16 + 8, p.z * 16 + 8)
                        .map(|c| c.height)
                });
            }
            (priority, rank)
        });
        self.priority_dirty = false;
    }

    fn prioritize_cached<K: Ord>(
        &mut self,
        focus: [f64; 3],
        mut priority: impl FnMut(SectionPos) -> K,
    ) {
        self.positions.sort_by_cached_key(|p| {
            std::cmp::Reverse((
                priority(*p),
                distance_squared(*p, focus) as u64,
                p.y,
                p.z,
                p.x,
            ))
        });
    }

    fn pop_for_dispatch(&mut self, inspected: usize, elapsed: Duration) -> Option<SectionPos> {
        // Even a candidate that needs no worker can spend time capturing models.
        // Resume the ordered queue next frame once the preparation budget is spent.
        // Bound a loading-screen frame containing thousands of empty snapshots too.
        if inspected >= 128
            || self.initial_ready && inspected != 0 && elapsed >= Duration::from_millis(2)
        {
            return None;
        }
        let position = self.positions.pop()?;
        self.queued.remove(&position);
        Some(position)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum MeshPriority {
    Edit,
    Missing,
    Resident,
}

impl MeshPriority {
    pub(super) fn for_section(
        position: SectionPos,
        journal: &EditJournal,
        rendered: &RenderedSections,
    ) -> Self {
        let presentation = rendered.presentations.get(&position);
        if journal.revision(position) != presentation.map_or(0, |p| p.block_revision) {
            Self::Edit
        } else if rendered.sections.contains_key(&position)
            || presentation.is_some_and(|p| {
                p.empty_mesh_epoch
                    .is_some_and(|epoch| epoch == journal.mesh_epoch(position))
            })
        {
            Self::Resident
        } else {
            // Includes nonempty terrain becoming visible again after portal culling.
            Self::Missing
        }
    }

    fn job_limit(self, configured: usize, refinement_limit: usize) -> usize {
        match self {
            Self::Edit => configured.saturating_add(1),
            Self::Missing => configured,
            Self::Resident => refinement_limit,
        }
    }
}

pub(super) fn interactive_edit(
    position: SectionPos,
    journal: &EditJournal,
    rendered: &RenderedSections,
) -> bool {
    journal.revision(position) != rendered.block_revision(position).unwrap_or(0)
}
#[derive(Default)]
pub(crate) struct AppearanceInputs {
    heraldry_revision: Option<(bool, u64)>,
    journal_dirty: bool,
    section_revisions: HashMap<SectionPos, (u64, u64)>,
    chunks_dirty: bool,
    fluid_revisions: HashMap<[i64; 2], u64>,
    columns: HashSet<[i64; 2]>,
    publications: MessageCursor<SectionPublished>,
    removals: MessageCursor<SectionRemoved>,
    prepared_columns: MessageCursor<PreparedColumnChanged>,
    water: MessageCursor<WaterPublished>,
    imported: MessageCursor<SectionEditApplied>,
}
impl AppearanceInputs {
    fn refresh_journal(
        &mut self,
        journal: &EditJournal,
        chunks: &ChunkStore,
        terrain: &terra_world::TerraGenerator,
        cache: &mut GeometryCache,
        arrivals: &mut HashSet<SectionPos>,
    ) {
        // Gameplay and clock updates share the journal with visual edits. Compare
        // the sparse source revisions, including removals when a replica is replaced.
        let before = std::mem::take(&mut self.section_revisions);
        cache.1.clear();
        let mut fluid_revisions = HashMap::<[i64; 2], u64>::new();
        for section in journal.changed_sections() {
            let column = [section.x, section.z];
            let block = journal.revision(section);
            let appearance = journal.appearance_epoch(section);
            if block != 0 || appearance != 0 {
                self.section_revisions.insert(section, (block, appearance));
            }
            if block != 0 {
                let sum = cache.1.entry(column).or_default();
                *sum = sum.wrapping_add(block);
            }
            let fluid = journal.fluid_epoch(section);
            if fluid != 0 {
                let sum = fluid_revisions.entry(column).or_default();
                *sum = sum.wrapping_add(fluid);
            }
        }

        let mut changed_columns = HashSet::new();
        if before != self.section_revisions {
            for &position in before.keys().chain(self.section_revisions.keys()) {
                let old = before.get(&position).copied().unwrap_or_default();
                let new = self
                    .section_revisions
                    .get(&position)
                    .copied()
                    .unwrap_or_default();
                if old.0 != new.0 {
                    changed_columns.insert([position.x, position.z]);
                }
                if old.1 != new.1 && chunks.loaded.contains_key(&position) {
                    arrivals.insert(position);
                }
            }
        }
        // Block edits affect the whole roof column, including salt formations
        // across vertical seams. Coastal exposure also samples adjacent columns.
        let mut affected = neighboring_columns(&changed_columns);
        if self.fluid_revisions != fluid_revisions {
            self.fluid_revisions = fluid_revisions;
            for (&column, state) in &mut cache.3 {
                let before = state.epoch;
                state.refresh(
                    chunks,
                    column,
                    terrain,
                    journal,
                    self.fluid_revisions.get(&column).copied().unwrap_or(0),
                );
                if state.epoch != before {
                    affected.insert(column);
                }
            }
        }
        for column in affected {
            cache.4.remove(&column);
            if let Some(watched) = cache.5.columns.get(&column) {
                arrivals.extend(&watched.owners);
            }
        }
    }
}

struct CachedGeometry {
    foliage: Option<u8>,
    salt: bool,
    coastal: bool,
    epoch: u64,
    geometry: Arc<BlockMesh>,
    models: Option<Arc<super::custom::ModelSnapshot>>,
}

impl CachedGeometry {
    fn folded_leaves(
        &self,
        position: SectionPos,
        world: &MeshWorld,
        previous: Option<&super::RenderedSection>,
        level: DetailLevel,
    ) -> Option<super::lod::FoldedLeaves> {
        self.foliage.map(|occupied| {
            super::lod::select_folded_leaves(
                position,
                world.focus.0,
                previous.and_then(|entry| entry.folded_leaves),
                level,
                world.config.client.blocks.leaf_geometry_quality,
                occupied,
            )
        })
    }
}

#[derive(Default)]
struct StreamExposureCache {
    columns: HashMap<[i64; 2], StreamExposureColumn>,
    changed: HashSet<SectionPos>,
    sequence: u64,
}

struct StreamExposureColumn {
    owners: HashSet<SectionPos>,
    snapshot: Option<Arc<crate::weather::surfaces::exposure::Exposure>>,
    epoch: u64,
    pending: bool,
}

impl StreamExposureCache {
    fn watch(
        &mut self,
        position: SectionPos,
        snapshot: Option<Arc<crate::weather::surfaces::exposure::Exposure>>,
    ) {
        let column = [position.x, position.z];
        if !self.columns.contains_key(&column) {
            self.sequence = self.sequence.wrapping_add(1);
            self.columns.insert(
                column,
                StreamExposureColumn {
                    owners: HashSet::new(),
                    snapshot: snapshot.clone(),
                    epoch: self.sequence,
                    pending: false,
                },
            );
        }
        let Some(state) = self.columns.get_mut(&column) else {
            return;
        };
        state.owners.insert(position);
        state.snapshot = snapshot;
    }

    fn forget(&mut self, position: SectionPos) -> bool {
        self.changed.remove(&position);
        let column = [position.x, position.z];
        let empty = self.columns.get_mut(&column).is_some_and(|state| {
            state.owners.remove(&position);
            state.owners.is_empty()
        });
        if empty {
            self.columns.remove(&column);
        }
        empty
    }

    fn mark_pending(&mut self, column: [i64; 2]) -> bool {
        let Some(state) = self.columns.get_mut(&column) else {
            return false;
        };
        if state.pending {
            return false;
        }
        state.pending = true;
        true
    }

    fn finish(
        &mut self,
        column: [i64; 2],
        snapshot: Option<Arc<crate::weather::surfaces::exposure::Exposure>>,
    ) -> bool {
        let Some(state) = self.columns.get_mut(&column) else {
            return false;
        };
        let same = match (&state.snapshot, &snapshot) {
            (None, None) => true,
            (Some(old), Some(new)) => old.same_masks(new),
            _ => false,
        };
        state.pending = false;
        if same {
            return false;
        }
        self.sequence = self.sequence.wrapping_add(1);
        state.epoch = self.sequence;
        state.snapshot = snapshot;
        self.changed.extend(state.owners.iter().copied());
        true
    }

    fn epoch(&self, position: SectionPos) -> u64 {
        self.columns
            .get(&[position.x, position.z])
            .map_or(0, |state| {
                // Pending columns reject old workers before the bounded capture runs.
                // No worker is dispatched using this temporary revision.
                state
                    .epoch
                    .wrapping_shl(32)
                    .wrapping_add(u64::from(state.pending))
            })
    }

    fn pending(&self, position: SectionPos) -> bool {
        self.columns
            .get(&[position.x, position.z])
            .is_some_and(|state| state.pending)
    }
}

#[derive(Default)]
pub(crate) struct StreamExposureInputs {
    publications: MessageCursor<SectionPublished>,
    removals: MessageCursor<SectionRemoved>,
    columns: MessageCursor<PreparedColumnChanged>,
    pending: VecDeque<[i64; 2]>,
}

/// Streaming invalidation precedes uploads, including while all workers are busy.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_stream_exposure(
    chunks: Res<ChunkStore>,
    terrain: Res<WorldTerrain>,
    journal: Res<EditJournal>,
    config: Res<terra_core::GameConfig>,
    publications: Option<Res<Messages<SectionPublished>>>,
    removals: Option<Res<Messages<SectionRemoved>>>,
    columns: Option<Res<Messages<PreparedColumnChanged>>>,
    mut cache: ResMut<GeometryCache>,
    mut inputs: Local<StreamExposureInputs>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.stream_exposure");
    if terrain.is_changed() {
        cache.5.columns.clear();
        cache.5.changed.clear();
        inputs.pending.clear();
    }
    let mut changed = HashSet::new();
    if let Some(publications) = publications {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.stream_exposure.publications");
        for message in inputs.publications.read(&publications) {
            let position = message.position;
            cache.refresh_funnel_supports(position, &chunks, &journal);
            cache.refresh_campfire_supports(position, &chunks, &journal);
            let column = [position.x, position.z];
            if cache.5.columns.get(&column).is_some_and(|state| {
                state.snapshot.as_ref().is_none_or(|snapshot| {
                    snapshot.section_affects(
                        position,
                        chunks.loaded.get(&position).map(|s| &s.data),
                        config.world.sea_level,
                    )
                })
            }) {
                changed.insert(column);
            }
        }
    }
    if let Some(removals) = removals {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.stream_exposure.removals");
        for message in inputs.removals.read(&removals) {
            let position = message.position;
            cache.refresh_funnel_supports(position, &chunks, &journal);
            cache.refresh_campfire_supports(position, &chunks, &journal);
            let column = [position.x, position.z];
            if cache.5.columns.get(&column).is_some_and(|state| {
                state.snapshot.as_ref().is_none_or(|snapshot| {
                    snapshot.section_affects(position, None, config.world.sea_level)
                })
            }) {
                changed.insert(column);
            }
            if !chunks.loaded.contains_key(&position) {
                cache.forget(position);
            }
        }
    }
    if let Some(columns) = columns {
        for message in inputs.columns.read(&columns) {
            for z in -1..=1 {
                for x in -1..=1 {
                    let column = [message.coordinate[0] + x, message.coordinate[1] + z];
                    if column == message.coordinate
                        || cache.5.columns.get(&column).is_some_and(|state| {
                            state
                                .snapshot
                                .as_ref()
                                .is_some_and(|snapshot| snapshot.coastal(config.world.sea_level))
                        })
                    {
                        changed.insert(column);
                    }
                }
            }
        }
    }
    for column in changed {
        if cache.5.mark_pending(column) {
            inputs.pending.push_back(column);
        }
    }
    inputs
        .pending
        .retain(|column| cache.5.columns.contains_key(column));
    let started = Instant::now();
    for captured in 0..2 {
        if captured != 0 && started.elapsed() >= Duration::from_micros(750) {
            break;
        }
        let Some(column) = inputs.pending.pop_front() else {
            break;
        };
        let snapshot = crate::weather::surfaces::exposure::Exposure::capture(
            &chunks,
            column,
            config.world.sea_level,
            config.world.bounded_world,
            &terrain.0,
            &journal,
        );
        if cache.5.finish(column, snapshot) {
            let snapshot = cache
                .5
                .columns
                .get(&column)
                .and_then(|state| state.snapshot.clone());
            cache.4.insert(column, snapshot);
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct GeometryCache(
    HashMap<SectionPos, CachedGeometry>,
    HashMap<[i64; 2], u64>,
    HashMap<[i64; 2], u16>,
    HashMap<[i64; 2], crate::weather::surfaces::exposure::RoofWaterState>,
    HashMap<[i64; 2], Option<Arc<crate::weather::surfaces::exposure::Exposure>>>,
    StreamExposureCache,
    u64,
    HashMap<SectionPos, u64>,
);
type GeometrySnapshot = (Arc<BlockMesh>, Option<Arc<super::custom::ModelSnapshot>>);

fn capture_models(
    section: &terra_voxel::BlockSection,
    templates: &crate::blocks::TemplateSet,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<Option<Arc<super::custom::ModelSnapshot>>, String> {
    let edits = journal
        .snapshot(section.position())
        .map_err(|error| error.to_string())?;
    let coordinate = [section.position().x, section.position().z];
    let column = chunks.prepared_column(coordinate);
    let mut models = super::custom::capture_prepared(
        section,
        templates,
        journal,
        column.as_deref(),
        &edits,
        |global| chunks.block_at(global),
    );
    if let Some(models) = &mut models {
        // Joinery can use persisted neighbor metadata; physical support cannot
        // stand in for a neighboring section which has not actually loaded.
        let domains = column.map_or_else(Vec::new, |column| {
            column
                .village_settlements()
                .map(|site| site.domain())
                .collect()
        });
        models.set_domains(domains);
        models.refresh_heraldry(section.position(), &journal.gameplay().kingdoms);
        models.refresh_funnel_supports(section.position(), chunks, journal);
        models.refresh_campfire_supports(section.position(), chunks, journal);
        models.refresh_water_plants(section.position(), chunks, journal);
    }
    Ok(models.map(Arc::new))
}

fn coastal_section(position: SectionPos, sea: f64) -> bool {
    position
        .origin()
        .is_ok_and(|origin| origin[1] as f64 <= sea + 1.0 && origin[1] as f64 + 16.0 >= sea - 1.0)
}

fn prepared_halo_mask(column: [i64; 2], mut present: impl FnMut([i64; 2]) -> bool) -> u16 {
    let mut mask = 0;
    for z in -1..=1 {
        for x in -1..=1 {
            if present([column[0] + x, column[1] + z]) {
                mask |= 1 << ((z + 1) * 3 + x + 1);
            }
        }
    }
    mask
}

fn neighboring_columns(columns: &HashSet<[i64; 2]>) -> HashSet<[i64; 2]> {
    let mut neighbors = HashSet::new();
    for &[x, z] in columns {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let (Some(x), Some(z)) = (x.checked_add(dx), z.checked_add(dz)) {
                    neighbors.insert([x, z]);
                }
            }
        }
    }
    neighbors
}

impl GeometryCache {
    fn refresh_water_contacts(&mut self, owner: SectionPos, journal: &EditJournal) {
        let Some(models) = self
            .0
            .get_mut(&owner)
            .and_then(|cached| cached.models.as_mut())
        else {
            return;
        };
        if !models.water_contacts_changed(owner, journal) {
            return;
        }
        Arc::make_mut(models).refresh_water_contacts(owner, journal);
        let revision = self.7.entry(owner).or_default();
        *revision = revision.wrapping_add(1);
        self.5.changed.insert(owner);
    }
    fn refresh_campfire_supports(
        &mut self,
        changed: SectionPos,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) {
        let Ok(owner) = changed.offset([0, -1, 0]) else {
            return;
        };
        if !chunks.loaded.contains_key(&owner) {
            return;
        }
        let Some(models) = self
            .0
            .get_mut(&owner)
            .and_then(|cached| cached.models.as_mut())
        else {
            return;
        };
        if !models.campfire_supports_changed(owner, chunks, journal) {
            return;
        }
        if Arc::make_mut(models).refresh_campfire_supports(owner, chunks, journal) {
            let revision = self.7.entry(owner).or_default();
            *revision = revision.wrapping_add(1);
            self.5.changed.insert(owner);
        }
    }

    fn refresh_funnel_supports(
        &mut self,
        changed: SectionPos,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) {
        let Ok(owner) = changed.offset([0, 1, 0]) else {
            return;
        };
        if !chunks.loaded.contains_key(&owner) {
            return;
        }
        let Some(models) = self
            .0
            .get_mut(&owner)
            .and_then(|cached| cached.models.as_mut())
        else {
            return;
        };
        if !models.funnel_supports_changed(owner, chunks, journal) {
            return;
        }
        if Arc::make_mut(models).refresh_funnel_supports(owner, chunks, journal) {
            let revision = self.7.entry(owner).or_default();
            *revision = revision.wrapping_add(1);
            self.5.changed.insert(owner);
        }
    }

    pub(crate) fn invalidate_details(&mut self) {
        self.6 = self.6.wrapping_add(1);
    }
    fn refresh_coastal_columns(
        &mut self,
        changed: &HashSet<[i64; 2]>,
        sea: f64,
        mut present: impl FnMut([i64; 2]) -> bool,
    ) -> HashSet<SectionPos> {
        let mut affected = HashSet::new();
        let center_y = (sea / 16.0).floor() as i64;
        for column in neighboring_columns(changed) {
            let positions: Vec<_> = (-1..=1)
                .filter_map(|dy| center_y.checked_add(dy))
                .map(|y| SectionPos::new(column[0], y, column[1]))
                .filter(|p| {
                    coastal_section(*p, sea)
                        && self.0.get(p).is_some_and(|geometry| geometry.coastal)
                })
                .collect();
            if positions.is_empty() {
                self.2.remove(&column);
                continue;
            }
            let mask = prepared_halo_mask(column, &mut present);
            if self.2.insert(column, mask) != Some(mask) {
                affected.extend(positions);
            }
        }
        affected
    }

    pub(super) fn forget(&mut self, position: SectionPos) {
        self.0.remove(&position);
        self.7.remove(&position);
        if self.5.forget(position) {
            let column = [position.x, position.z];
            self.2.remove(&column);
            self.3.remove(&column);
            self.4.remove(&column);
        }
    }
    pub(crate) fn appearance_epoch(&self, position: SectionPos, journal: &EditJournal) -> u64 {
        let mut epoch = journal
            .appearance_epoch(position)
            .wrapping_add(self.6)
            .wrapping_add(self.7.get(&position).copied().unwrap_or(0))
            .wrapping_add(self.1.get(&[position.x, position.z]).copied().unwrap_or(0))
            .wrapping_add(self.5.epoch(position));
        epoch = epoch.wrapping_add(
            self.3
                .get(&[position.x, position.z])
                .map_or(0, |water| water.epoch),
        );
        if self.0.get(&position).is_some_and(|cached| cached.salt) {
            for dy in [-2, -1, 1, 2] {
                if let Ok(neighbor) = position.offset([0, dy, 0]) {
                    epoch = epoch.wrapping_add(journal.revision(neighbor));
                }
            }
        }
        if self.0.get(&position).is_some_and(|cached| cached.coastal) {
            for z in -1..=1 {
                for x in -1..=1 {
                    if x != 0 || z != 0 {
                        epoch = epoch.wrapping_add(
                            self.1
                                .get(&[position.x + x, position.z + z])
                                .copied()
                                .unwrap_or(0),
                        );
                    }
                }
            }
            epoch = epoch.wrapping_add(
                u64::from(self.2.get(&[position.x, position.z]).copied().unwrap_or(0)) << 48,
            );
        }
        epoch
    }
    pub(super) fn exposure_pending(&self, position: SectionPos) -> bool {
        self.5.pending(position)
    }
    #[cfg(test)]
    pub(crate) fn insert_test(&mut self, position: SectionPos, epoch: u64, geometry: BlockMesh) {
        self.0.insert(
            position,
            CachedGeometry {
                foliage: super::lod::foliage_octants(&geometry.faces),
                coastal: false,
                salt: !geometry.segments.is_empty()
                    || geometry
                        .faces
                        .iter()
                        .any(|face| matches!(face.material, 30 | 37)),
                epoch,
                geometry: Arc::new(geometry),
                models: None,
            },
        );
    }
    #[cfg(test)]
    pub(crate) fn insert_model_test(
        &mut self,
        position: SectionPos,
        epoch: u64,
        geometry: BlockMesh,
        models: super::custom::ModelSnapshot,
    ) {
        self.0.insert(
            position,
            CachedGeometry {
                foliage: super::lod::foliage_octants(&geometry.faces),
                coastal: false,
                salt: !geometry.segments.is_empty()
                    || geometry
                        .faces
                        .iter()
                        .any(|face| matches!(face.material, 30 | 37)),
                epoch,
                geometry: Arc::new(geometry),
                models: Some(Arc::new(models)),
            },
        );
    }
    pub(crate) fn snapshot(&self, position: SectionPos, epoch: u64) -> Option<GeometrySnapshot> {
        let cached = self
            .0
            .get(&position)
            .filter(|cached| cached.epoch == epoch)?;
        Some((cached.geometry.clone(), cached.models.clone()))
    }
}

#[cfg(test)]
#[path = "model_support_tests.rs"]
mod model_support_tests;
#[cfg(test)]
impl GeometryCache {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(SystemParam)]
pub(crate) struct MeshWorld<'w, 's> {
    time: Option<Res<'w, Time<Real>>>,
    visibility: Option<Res<'w, super::visibility::TerrainVisibility>>,
    chunks: ResMut<'w, ChunkStore>,
    generation: Option<Res<'w, terra_world::streaming::WorldGeneration>>,
    publications: Option<Res<'w, Messages<SectionPublished>>>,
    removals: Option<Res<'w, Messages<SectionRemoved>>>,
    prepared_columns: Option<Res<'w, Messages<PreparedColumnChanged>>>,
    water: Option<Res<'w, Messages<WaterPublished>>>,
    imported: Option<Res<'w, Messages<SectionEditApplied>>>,
    journal: Res<'w, EditJournal>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    config: Res<'w, terra_core::GameConfig>,
    graphics: Option<Res<'w, terra_core::GraphicsConfig>>,
    focus: Res<'w, ChunkFocus>,
    terrain: Res<'w, WorldTerrain>,
    rendered: ResMut<'w, RenderedSections>,
    entering: Query<'w, 's, (), With<crate::transitions::ChunkFade>>,
    terrain_sections: Query<'w, 's, &'static super::TerrainSection>,
    entering_removed: RemovedComponents<'w, 's, crate::transitions::EnteringChunk>,
    foliage_reference: Option<Res<'w, crate::foliage::FoliageReferenceBenchmark>>,
    exact_shadows: super::shadows::ExactInputs<'w, 's>,
}

fn needs_mesh(
    position: SectionPos,
    full_selection: bool,
    world: &MeshWorld,
    jobs: &RenderJobs,
    cache: &GeometryCache,
) -> bool {
    if world
        .visibility
        .as_ref()
        .is_some_and(|v| v.hidden(position))
    {
        return false;
    }
    let Some(section) = world.chunks.loaded.get(&position) else {
        return false;
    };
    if jobs.0.contains_key(&position) || section.mesh_epoch != world.journal.mesh_epoch(position) {
        return false;
    }
    if cache.5.pending(position) {
        return false;
    }
    if section.mesh.is_some() {
        return true;
    }
    let Some(cached) = cache.0.get(&position) else {
        return false;
    };
    let previous = world.rendered.sections.get(&position);
    // A publication elsewhere cannot change this mesh's LOD. Coastal batches
    // still observe the arrival of neighboring prepared-column halos.
    if !full_selection
        && !cached.coastal
        && previous.is_some_and(|old| {
            old.mesh_epoch == cached.epoch
                && old.appearance_epoch == cache.appearance_epoch(position, &world.journal)
        })
    {
        return false;
    }
    if previous.is_some_and(|old| {
        world.entering.contains(old.entity)
            && old.mesh_epoch == cached.epoch
            && old.appearance_epoch == cache.appearance_epoch(position, &world.journal)
    }) {
        return false;
    }
    if cached.geometry.faces.is_empty()
        && cached.geometry.segments.is_empty()
        && cached.models.is_none()
        && previous.is_none()
    {
        return false;
    }
    let level = select_level(
        position,
        world.focus.0,
        previous.map(|r| r.detail_level),
        &world.config.client,
    );
    let micro_detail = super::lod::select_configured_micro_detail(
        position,
        world.focus.0,
        previous.map(|r| r.micro_detail),
        world.config.client.blocks,
    );
    let folded_leaves = cached.folded_leaves(position, world, previous, level);
    !previous.is_some_and(|r| {
        r.folded_leaves == folded_leaves
            && r.mesh_epoch == cached.epoch
            && r.micro_detail == micro_detail
            && r.detail_level == level
            && r.appearance_epoch == cache.appearance_epoch(position, &world.journal)
    })
}

#[allow(clippy::too_many_arguments)] // Independent ECS resources and local worker queues.
pub(crate) fn dispatch_meshes(
    mut world: MeshWorld,
    assets: Res<BlockAssets>,
    mut jobs: ResMut<RenderJobs>,
    mut cache: ResMut<GeometryCache>,
    mut status: ResMut<RenderStatus>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    (mut queue, mut appearance_inputs): (Local<MeshQueue>, Local<AppearanceInputs>),
    mut readiness_trace: Local<Option<Instant>>,
    mut coarse_templates: Local<
        Option<(
            Arc<crate::blocks::TemplateSet>,
            Arc<crate::blocks::TemplateSet>,
        )>,
    >,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.dispatch");
    let kingdom_session = world.session.as_ref().filter(|session| session.ready);
    let kingdoms = kingdom_session.map_or(&world.journal.gameplay().kingdoms, |session| {
        &session.kingdoms
    });
    let heraldry_revision = (kingdom_session.is_some(), kingdoms.revision);
    if appearance_inputs.heraldry_revision != Some(heraldry_revision) || world.terrain.is_changed()
    {
        let _scope = terra_core::profiling::ScopeTimer::new("terrain.refresh.heraldry");
        let changed: Vec<_> = cache
            .0
            .iter_mut()
            .filter_map(|(&position, geometry)| {
                let models = geometry.models.as_mut()?;
                if !models.heraldry_changed(position, kingdoms) {
                    return None;
                }
                Arc::make_mut(models).refresh_heraldry(position, kingdoms);
                Some(position)
            })
            .collect();
        for position in changed {
            let revision = cache.7.entry(position).or_default();
            *revision = revision.wrapping_add(1);
            queue.enqueue(position);
            queue.arrivals.insert(position);
            queue.dirty = true;
        }
        appearance_inputs.heraldry_revision = Some(heraldry_revision);
    }
    let visibility_revision = world.visibility.as_ref().map_or(0, |v| v.revision());
    let visibility_changed = queue.visibility_revision != visibility_revision;
    queue.visibility_revision = visibility_revision;
    let published: Vec<_> = world
        .publications
        .as_ref()
        .map_or_else(Vec::new, |messages| {
            appearance_inputs
                .publications
                .read(messages)
                .map(|message| message.position)
                .collect()
        });
    let removed: Vec<_> = world.removals.as_ref().map_or_else(Vec::new, |messages| {
        appearance_inputs
            .removals
            .read(messages)
            .map(|message| message.position)
            .collect()
    });
    let prepared_changes: HashSet<_> =
        world
            .prepared_columns
            .as_ref()
            .map_or_else(HashSet::new, |messages| {
                appearance_inputs
                    .prepared_columns
                    .read(messages)
                    .map(|message| message.coordinate)
                    .collect()
            });
    let mut changed_columns = prepared_changes.clone();
    changed_columns.extend(published.iter().chain(&removed).map(|p| [p.x, p.z]));
    let water_publications: Vec<_> = world.water.as_ref().map_or_else(Vec::new, |messages| {
        appearance_inputs
            .water
            .read(messages)
            .map(|message| message.position)
            .collect()
    });
    for &position in &water_publications {
        cache.refresh_water_contacts(position, &world.journal);
        for offset in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            if let Ok(neighbor) = position.offset(offset) {
                cache.refresh_water_contacts(neighbor, &world.journal);
            }
        }
    }
    let water_published = !water_publications.is_empty();
    let edits_applied = world
        .imported
        .as_ref()
        .is_some_and(|messages| appearance_inputs.imported.read(messages).count() != 0);
    let notified = !published.is_empty()
        || !removed.is_empty()
        || !changed_columns.is_empty()
        || water_published
        || edits_applied;
    // Fixtures and exceptional store mutations still use the conservative path.
    let arrival_scan = (world.chunks.is_changed() && !notified) || world.terrain.is_changed();
    // The menu can reveal a complete first presentation while LOD swaps are
    // pending. A player flying immediately may never drain the entire queue.
    let initial_ready = status.scene_revealed
        || (!queue.initial_ready
            && status.ready
            && !world.rendered.sections.is_empty()
            && world.generation.as_ref().is_none_or(|generation| {
                generation.status == terra_world::streaming::GenerationStatus::Ready
            })
            && MeshQueue::uploads_consumed(&world.chunks, |position| {
                world
                    .visibility
                    .as_ref()
                    .is_some_and(|visibility| visibility.hidden(position))
            }));
    // Clear state from the previous world before recording this world's first
    // publications. Otherwise a full worker pool can permanently lose an edit.
    queue.update_initial_ready(world.terrain.is_changed(), initial_ready);
    let configured = world.config.client.max_mesh_jobs;
    let refinement_limit = queue.refinement_limit(
        configured,
        world.time.as_ref().map_or(0., |time| time.delta_secs()),
        world
            .graphics
            .as_ref()
            .map_or(0, |graphics| graphics.fps_limit),
    );
    // A confirmed block is already solid for gameplay. Its published remesh must
    // not wait for a time-sliced full-world LOD sweep or obsolete render workers.
    for &position in &published {
        if interactive_edit(position, &world.journal, &world.rendered) {
            queue.enqueue(position);
            // A resident replacement may already be queued at its old priority.
            queue.priority_dirty = true;
        }
    }
    queue.arrivals.extend(published);
    let stale: Vec<_> = jobs
        .0
        .iter()
        .filter_map(|(&position, job)| {
            (job.epoch != world.journal.mesh_epoch(position)).then_some(position)
        })
        .collect();
    for position in stale {
        jobs.cancel(&position, meshes.as_deref_mut());
        queue.arrivals.insert(position);
    }
    queue.arrivals.extend(cache.5.changed.drain());
    queue.arrival_scan |= arrival_scan;
    appearance_inputs
        .columns
        .extend(changed_columns.iter().copied());
    let finished: Vec<_> = world
        .entering_removed
        .read()
        .filter_map(|entity| {
            if world.entering.contains(entity) {
                return None;
            }
            let section = world.terrain_sections.get(entity).ok()?;
            world
                .rendered
                .sections
                .get(&section.position)
                .filter(|rendered| rendered.entity == entity)
                .map(|_| section.position)
        })
        .collect();
    let animation_finished = !finished.is_empty();
    let focus_changed = queue
        .focus
        .is_none_or(|previous| focus_bucket(previous) != focus_bucket(world.focus.0));
    let heading_changed = queue.motion.update(
        world.focus.0,
        world.time.as_ref().map_or(0., |t| t.delta_secs_f64()),
    );
    let invalidation_flags = [
        world.chunks.is_changed(),
        world.journal.is_changed(),
        world.config.is_changed(),
        world.terrain.is_changed(),
        assets.is_changed(),
        focus_changed,
    ]
    .into_iter()
    .enumerate()
    .fold(0, |flags, (bit, changed)| {
        flags | (u8::from(changed) << bit)
    });
    for (name, changed) in [
        ("terrain.invalidate.chunks", world.chunks.is_changed()),
        ("terrain.invalidate.journal", world.journal.is_changed()),
        ("terrain.invalidate.config", world.config.is_changed()),
        ("terrain.invalidate.generator", world.terrain.is_changed()),
        ("terrain.invalidate.assets", assets.is_changed()),
        ("terrain.invalidate.animation", animation_finished),
        ("terrain.invalidate.focus", focus_changed),
    ] {
        if changed {
            drop(terra_core::profiling::ScopeTimer::new(name));
        }
    }
    if status.ready
        && jobs.0.is_empty()
        && jobs.1.is_empty()
        && queue
            .focus
            .is_some_and(|previous| focus_bucket(previous) == focus_bucket(world.focus.0))
        && !world.chunks.is_changed()
        && !world.journal.is_changed()
        && !world.config.is_changed()
        && !world.terrain.is_changed()
        && !assets.is_changed()
        && !animation_finished
        && !visibility_changed
        && !notified
        && queue.positions.is_empty()
        && !queue.has_selection_work()
    {
        return;
    }
    queue.selection_dirty |= visibility_changed
        || focus_changed
        || world.config.is_changed()
        || world.terrain.is_changed()
        || assets.is_changed();
    queue.dirty |= queue.selection_dirty || world.chunks.is_changed() || notified;
    queue.priority_dirty |= focus_changed || heading_changed;
    queue.focus = Some(world.focus.0);
    // Selection skips roots while they fade. Recheck those roots when they
    // finish without rescanning every unrelated loaded section.
    for &position in &finished {
        if world.chunks.loaded.contains_key(&position) {
            queue.enqueue(position);
        }
    }
    // Retain invalidations while all workers are occupied. Moving the camera
    // alone cannot change column revisions, submerged roofs or coastal halos.
    appearance_inputs.journal_dirty |= world.journal.is_changed();
    appearance_inputs.chunks_dirty |=
        arrival_scan || world.config.is_changed() || world.terrain.is_changed();
    // A visual LOD change does not change roofs, rain exposure or shorelines.
    // Per-column revisions below retain masks through unrelated wetness/gameplay edits.
    if arrival_scan || world.config.is_changed() || world.terrain.is_changed() {
        cache.4.clear();
    } else {
        for column in neighboring_columns(&prepared_changes) {
            if !prepared_changes.contains(&column) || !cache.5.columns.contains_key(&column) {
                cache.4.remove(&column);
            }
        }
    }
    status.error = assets.error.clone().or(status.error.take());
    if arrival_scan {
        world
            .rendered
            .presentations
            .retain(|p, _| world.chunks.loaded.contains_key(p));
        jobs.0.retain(|p, job| {
            let loaded = world.chunks.loaded.contains_key(p);
            if !loaded {
                job.release_staged(meshes.as_deref_mut());
            }
            loaded
        });
        let removed: Vec<_> = cache
            .0
            .keys()
            .copied()
            .filter(|p| !world.chunks.loaded.contains_key(p))
            .collect();
        for position in removed {
            cache.forget(position);
        }
        queue.retain_loaded(|position| world.chunks.loaded.contains_key(&position));
        cache
            .3
            .retain(|column, _| world.chunks.prepared_column(*column).is_some());
    } else {
        let removed = queue.remove_unloaded(removed, |position| {
            world.chunks.loaded.contains_key(&position)
        });
        for position in removed {
            world.rendered.presentations.remove(&position);
            jobs.cancel(&position, meshes.as_deref_mut());
            cache.forget(position);
        }
        for column in &changed_columns {
            if world.chunks.prepared_column(*column).is_none() {
                cache.3.remove(column);
            }
        }
    }
    // A full set of paced LOD jobs must still let us inspect fresh publications
    // and use the remaining configured slots for terrain that has no mesh.
    let edit_reserve = jobs.0.len() == configured
        && queue
            .positions
            .iter()
            .any(|&p| interactive_edit(p, &world.journal, &world.rendered));
    let job_limit = configured.saturating_add(usize::from(edit_reserve));
    if let Some(templates) = assets
        .templates
        .as_ref()
        .filter(|_| jobs.0.len() < job_limit)
    {
        if appearance_inputs.journal_dirty {
            let _scope = terra_core::profiling::ScopeTimer::new("terrain.refresh.appearance");
            appearance_inputs.refresh_journal(
                &world.journal,
                &world.chunks,
                &world.terrain.0,
                &mut cache,
                &mut queue.arrivals,
            );
            appearance_inputs.journal_dirty = false;
        }
        if appearance_inputs.chunks_dirty {
            let _scope = terra_core::profiling::ScopeTimer::new("terrain.refresh.coastal");
            cache.2.clear();
            let coastal_columns: std::collections::HashSet<_> = cache
                .0
                .iter()
                .filter(|(_, cached)| cached.coastal)
                .map(|(position, _)| [position.x, position.z])
                .collect();
            for column in coastal_columns {
                cache.2.insert(
                    column,
                    prepared_halo_mask(column, |p| world.chunks.prepared_column(p).is_some()),
                );
            }
            appearance_inputs.chunks_dirty = false;
            appearance_inputs.columns.clear();
        } else if !appearance_inputs.columns.is_empty() {
            let _scope =
                terra_core::profiling::ScopeTimer::new("terrain.refresh.coastal.incremental");
            let columns = std::mem::take(&mut appearance_inputs.columns);
            queue.arrivals.extend(cache.refresh_coastal_columns(
                &columns,
                world.config.world.sea_level,
                |column| world.chunks.prepared_column(column).is_some(),
            ));
        }
        if coarse_templates
            .as_ref()
            .is_none_or(|(source, _)| !Arc::ptr_eq(source, templates))
        {
            let mut coarse = templates.as_ref().clone();
            coarse.block_details.micro_geometry = false;
            for block in coarse.blocks.values_mut() {
                block.surface = None;
            }
            for (&id, block) in &templates.cube_lods {
                coarse.blocks.insert(id, block.clone());
            }
            *coarse_templates = Some((templates.clone(), Arc::new(coarse)));
        }
        let has_models = templates
            .blocks
            .values()
            .any(|block| !block.model.is_empty());
        // Failed/revision-stale uploads are local retries, never a reason to
        // reselect every section. Dispatch below validates the current epochs.
        for position in std::mem::take(&mut jobs.1) {
            if cache.5.pending(position) {
                queue.arrivals.insert(position);
            } else if needs_mesh(position, true, &world, &jobs, &cache) {
                queue.enqueue(position);
            }
        }
        if !queue.initial_ready && queue.has_selection_work() {
            let _scope = terra_core::profiling::ScopeTimer::new("terrain.refresh.queue");
            let _selection_scope =
                terra_core::profiling::ScopeTimer::new("terrain.refresh.queue.full");
            // Keep startup's full worker fill; only subsequent play is paced.
            let global_scan = queue.selection_dirty || queue.arrival_scan;
            let candidates: Vec<_> = if global_scan {
                queue.arrivals.clear();
                world.chunks.loaded.keys().copied().collect()
            } else {
                queue.take_incremental_candidates().into_iter().collect()
            };
            for position in candidates {
                if cache.5.pending(position) {
                    queue.arrivals.insert(position);
                } else if needs_mesh(position, true, &world, &jobs, &cache) {
                    queue.enqueue(position);
                }
            }
            queue.dirty = false;
            queue.selection_dirty = false;
            queue.arrival_scan = false;
            queue.sweep = None;
        } else if queue.initial_ready {
            let _scope = terra_core::profiling::ScopeTimer::new("terrain.refresh.queue");
            if !queue.arrivals.is_empty() {
                let _scope =
                    terra_core::profiling::ScopeTimer::new("terrain.refresh.queue.incremental");
                let started = Instant::now();
                let arrivals: Vec<_> = queue.arrivals.iter().copied().take(256).collect();
                for (inspected, position) in arrivals.into_iter().enumerate() {
                    if inspected != 0 && started.elapsed() >= Duration::from_micros(250) {
                        break;
                    }
                    if cache.5.pending(position) {
                        continue;
                    }
                    queue.arrivals.remove(&position);
                    if needs_mesh(position, false, &world, &jobs, &cache) {
                        queue.enqueue(position);
                    }
                }
            }
            queue.begin_sweep(|| world.chunks.loaded.keys().copied().collect());
            if queue.sweep.is_some() {
                let full_selection = queue
                    .sweep
                    .as_ref()
                    .is_some_and(|sweep| sweep.full_selection);
                let _scope = terra_core::profiling::ScopeTimer::new(if full_selection {
                    "terrain.refresh.queue.full"
                } else {
                    "terrain.refresh.queue.arrivals"
                });
                let started = Instant::now();
                for inspected in 0..1_024 {
                    let Some((position, full_selection)) =
                        queue.sweep_candidate(inspected, started.elapsed())
                    else {
                        break;
                    };
                    if cache.5.pending(position) {
                        queue.arrivals.insert(position);
                    } else if needs_mesh(position, full_selection, &world, &jobs, &cache) {
                        queue.enqueue(position);
                    }
                }
                queue.finish_sweep();
            }
            queue.dirty = false;
        }
        if queue.priority_dirty {
            queue.prioritize_work(
                world.focus.0,
                &world.journal,
                &world.rendered,
                &world.chunks,
                &cache,
            );
        }
        let _preparation_scope = terra_core::profiling::ScopeTimer::new("terrain.prepare_jobs");
        let preparation_started = Instant::now();
        let mut inspected = 0;
        let mut exact_shadow_materials = None;
        while jobs.0.len() < job_limit {
            let Some(&next) = queue.positions.last() else {
                break;
            };
            let priority = MeshPriority::for_section(next, &world.journal, &world.rendered);
            if jobs.0.len() >= priority.job_limit(configured, refinement_limit) {
                // Leave the next candidate in order until it has a slot.
                break;
            }
            let Some(position) = queue.pop_for_dispatch(inspected, preparation_started.elapsed())
            else {
                break;
            };
            inspected += 1;
            if jobs.0.contains_key(&position) {
                continue;
            }
            if world
                .visibility
                .as_ref()
                .is_some_and(|v| v.hidden(position))
            {
                continue;
            }
            if cache.5.pending(position) {
                queue.arrivals.insert(position);
                continue;
            }
            let level = select_level(
                position,
                world.focus.0,
                world
                    .rendered
                    .sections
                    .get(&position)
                    .map(|r| r.detail_level),
                &world.config.client,
            );
            let micro_detail = super::lod::select_configured_micro_detail(
                position,
                world.focus.0,
                world
                    .rendered
                    .sections
                    .get(&position)
                    .map(|r| r.micro_detail),
                world.config.client.blocks,
            );
            let Some(section) = world.chunks.loaded.get(&position) else {
                continue;
            };
            if section.mesh_epoch != world.journal.mesh_epoch(position) {
                continue;
            }
            let models = if has_models && section.mesh.is_some() && !section.data.is_empty() {
                let _scope = terra_core::profiling::ScopeTimer::new("terrain.custom_snapshot");
                match capture_models(&section.data, templates, &world.chunks, &world.journal) {
                    Ok(mut models) => {
                        if let Some(models) = &mut models {
                            Arc::make_mut(models).refresh_heraldry(position, kingdoms);
                        }
                        models
                    }
                    Err(error) => {
                        status.error = Some(error);
                        continue;
                    }
                }
            } else {
                None
            };
            let coastal = coastal_section(position, world.config.world.sea_level);
            if coastal {
                let column = [position.x, position.z];
                cache.2.entry(column).or_insert_with(|| {
                    prepared_halo_mask(column, |p| world.chunks.prepared_column(p).is_some())
                });
            }
            // Consuming a CPU render snapshot does not alter blocks, props or
            // fluids. Do not invalidate every world-dependent cache on LOD swaps.
            let Some(section) = world
                .chunks
                .bypass_change_detection()
                .loaded
                .get_mut(&position)
            else {
                continue;
            };
            if let Some(geometry) = section.mesh.take() {
                cache.0.insert(
                    position,
                    CachedGeometry {
                        foliage: super::lod::foliage_octants(&geometry.faces),
                        coastal,
                        salt: !geometry.segments.is_empty()
                            || geometry
                                .faces
                                .iter()
                                .any(|face| matches!(face.material, 30 | 37)),
                        epoch: section.mesh_epoch,
                        geometry: Arc::new(geometry),
                        models,
                    },
                );
            }
            let Some(cached) = cache.0.get(&position) else {
                continue;
            };
            if cached.epoch != section.mesh_epoch {
                continue;
            }
            let previous = world.rendered.sections.get(&position);
            let folded_leaves = cached.folded_leaves(position, &world, previous, level);
            if previous.is_some_and(|r| {
                r.folded_leaves == folded_leaves
                    && r.mesh_epoch == cached.epoch
                    && r.micro_detail == micro_detail
                    && r.detail_level == level
                    && r.appearance_epoch == cache.appearance_epoch(position, &world.journal)
            }) {
                continue;
            }
            if cached.geometry.faces.is_empty()
                && cached.geometry.segments.is_empty()
                && cached.models.is_none()
                && !world.rendered.sections.contains_key(&position)
            {
                world.rendered.record_presentation(
                    position,
                    world.journal.revision(position),
                    Some(cached.epoch),
                );
                continue;
            }
            let mut geometry = cached.geometry.clone();
            let models = cached.models.clone();
            let epoch = cached.epoch;
            let salt = cached.salt;
            let seasonal_foliage = cached.foliage.is_some();
            let exposure = cache
                .4
                .entry([position.x, position.z])
                .or_insert_with(|| {
                    crate::weather::surfaces::exposure::Exposure::capture(
                        &world.chunks,
                        [position.x, position.z],
                        world.config.world.sea_level,
                        world.config.world.bounded_world,
                        &world.terrain.0,
                        &world.journal,
                    )
                })
                .clone();
            cache.5.watch(position, exposure.clone());
            if let Some(exposure) = &exposure {
                let column = [position.x, position.z];
                let water_epoch = cache.3.get(&column).map_or(0, |water| water.epoch);
                cache.3.insert(
                    column,
                    crate::weather::surfaces::exposure::RoofWaterState::captured(
                        exposure,
                        appearance_inputs
                            .fluid_revisions
                            .get(&column)
                            .copied()
                            .unwrap_or(0),
                        water_epoch,
                    ),
                );
            }
            let mineral_edits = if salt || seasonal_foliage {
                match world.journal.snapshot(position) {
                    Ok(edits) => Some(edits),
                    Err(error) => {
                        status.error = Some(error.to_string());
                        continue;
                    }
                }
            } else {
                None
            };
            let templates = if micro_detail {
                templates.clone()
            } else {
                coarse_templates
                    .as_ref()
                    .map(|(_, coarse)| coarse.clone())
                    .unwrap_or_else(|| templates.clone())
            };
            let material_edges = if micro_detail && world.config.client.blocks.surface_variation {
                world
                    .chunks
                    .loaded
                    .get(&position)
                    .filter(|s| s.data.blocks().iter().any(|id| matches!(id, 1..=5)))
                    .map(|s| {
                        super::material_edges::Neighborhood::capture(&s.data, |global| {
                            world.chunks.block_at(global)
                        })
                    })
            } else {
                None
            };
            let lava_surface = world
                .chunks
                .loaded
                .get(&position)
                .filter(|s| super::lava::near_lava(&s.data, &world.chunks))
                .map(|s| {
                    super::lava::capture(&s.data, &world.chunks, &world.journal, |raw| {
                        templates
                            .blocks
                            .get(&raw)
                            .is_some_and(|block| !block.model.is_empty())
                    })
                })
                .unwrap_or_default();
            let appearance_epoch = cache.appearance_epoch(position, &world.journal);
            let options =
                RenderOptions {
                    water_level: world
                        .config
                        .world
                        .water
                        .then_some(world.config.world.sea_level),
                    detail_level: level,
                    terrain: Some(world.terrain.0.clone().with_planted_appearance(
                        world.journal.planted_appearance_records(position),
                    )),
                    exposure,
                    tints: Arc::new(world.journal.tint_snapshot(position)),
                    conditions: world.journal.surface_snapshot(position),
                };
            let local_crowns =
                world.config.client.chunk_animation == terra_core::ChunkAnimation::None;
            let pack_foliage =
                world.foliage_reference.is_none() && crate::foliage::packing::enabled();
            let canopy_uv_slots = world
                .exact_shadows
                .foliage
                .as_ref()
                .map(|a| a.primary_uv_only.clone());
            let pack_static = super::builder::static_frame_packing_enabled();
            let shadow_proxies = super::shadows::enabled(world.graphics.as_deref());
            let blade_shadow_slots = world
                .exact_shadows
                .foliage
                .as_ref()
                .filter(|_| {
                    shadow_proxies && pack_foliage && super::shadows::leaf_shadows_enabled()
                })
                .map(|assets| assets.opaque_blades.clone());
            // Near shells ignore foliage faces and mineral/segment animation.
            // Use their canonical source even when the color mesh splits leaves.
            let near_shadow_geometry =
                (shadow_proxies && level == DetailLevel::Near).then(|| geometry.clone());
            let exact_shadows = if shadow_proxies && level != DetailLevel::Near {
                exact_shadow_materials
                    .get_or_insert_with(|| world.exact_shadows.capture(&assets))
                    .clone()
            } else {
                None
            };
            let prepared = world.chunks.prepared_column([position.x, position.z]);
            let task = AsyncComputeTaskPool::get().spawn(async move {
                let start = std::time::Instant::now();
                if let Some(edits) = mineral_edits.as_ref()
                    && let Some(terrain) = &options.terrain
                {
                    refresh_growth(&mut geometry, position, terrain, edits);
                    if seasonal_foliage && let Ok(origin) = position.origin() {
                        let trees = terrain.tree_appearance(origin);
                        if let Some(faces) =
                            super::seasonal_foliage::split_edited_faces_with_planted(
                                &geometry.faces,
                                origin,
                                edits,
                                options.exposure.as_deref(),
                                (
                                    |global, material| trees.at(global, material),
                                    |global, material| trees.is_planted(global, material),
                                ),
                            )
                        {
                            Arc::make_mut(&mut geometry).faces = faces;
                        }
                    }
                }
                let result = match models {
                    Some(models) => super::custom::compose_with_leaf_detail(
                        &geometry,
                        &models,
                        &templates,
                        position,
                        &options,
                        folded_leaves.unwrap_or(super::lod::FoldedLeaves::FULL),
                        prepared.as_deref(),
                    ),
                    None => compose_section_with_leaf_detail(
                        &geometry.faces,
                        &templates,
                        position,
                        &options,
                        &geometry.mineral_growth,
                        &geometry.segments,
                        folded_leaves.unwrap_or(super::lod::FoldedLeaves::FULL),
                        prepared.as_deref(),
                    ),
                };
                let result = result.and_then(|mut batches| {
                    // Far replicated sections can arrive as mesh geometry before their
                    // voxel base. Their static reservoir still needs its animated material.
                    if let Some(template) = templates.blocks.get(&terra_world::lava::LAVA) {
                        let terrain = options.terrain.as_ref().zip(position.origin().ok());
                        super::lava::surface(
                            &mut batches,
                            &geometry.faces,
                            template.face_materials[3],
                            &lava_surface,
                            |p| {
                                terrain.map_or([0.; 2], |(t, origin)| {
                                    t.lava_current(
                                        origin[0] as f64 + f64::from(p[0]),
                                        origin[2] as f64 + f64::from(p[2]),
                                    )
                                })
                            },
                        );
                    }
                    if let Some(edges) = &material_edges {
                        edges.apply(&mut batches, &templates);
                    }
                    super::lighting::apply(&mut batches, &geometry.occluders, &templates);
                    if let Some(source) = &near_shadow_geometry {
                        super::shadows::append(&mut batches, &source.faces, &templates, level)?;
                    } else if let Some(materials) = &exact_shadows {
                        super::shadows::append_exact(&mut batches, materials, level)?;
                    }
                    if seasonal_foliage
                        && let (Some(terrain), Some(edits), Ok(origin)) =
                            (&options.terrain, mineral_edits.as_ref(), position.origin())
                    {
                        let trees = terrain.tree_appearance(origin);
                        let climate =
                            crate::weather::storage::snow::SnowClimate::new(terrain, origin);
                        super::seasonal_foliage::annotate_with_planted(
                            &mut batches,
                            &templates,
                            origin,
                            edits,
                            (
                                |global, material| trees.at(global, material),
                                |global, material| trees.is_planted(global, material),
                            ),
                            |position| climate.temperature(position),
                            |cell| {
                                options
                                    .exposure
                                    .as_ref()
                                    .is_some_and(|exposure| exposure.snow_exposed(cell, origin[1]))
                            },
                        );
                    }
                    for batch in &mut batches {
                        if templates.foliage_material(batch.material) {
                            if pack_foliage {
                                if canopy_uv_slots
                                    .as_ref()
                                    .is_some_and(|slots| slots.contains(&batch.material))
                                {
                                    crate::foliage::packing::omit_unused_exposure(batch);
                                }
                                crate::foliage::packing::pack(batch);
                            }
                        } else if pack_static
                            && !templates.water_material(batch.material)
                            && !templates.torch_material(batch.material)
                            && !templates.torch_palette.contains_key(&batch.material)
                        {
                            batch.pack_static_frame();
                        }
                    }
                    if let Some(slots) = &blade_shadow_slots
                        && let Some(folded) = folded_leaves
                        && templates.block_details.leaf_geometry_quality
                            == terra_core::BlockDetailQuality::High
                        && super::shadows::profile_shadows_enabled()
                    {
                        super::shadows::append_profiles(&mut batches, slots, position, folded);
                    }
                    for batch in &mut batches {
                        batch.leaf_shadow_sides = Vec::new();
                    }
                    let mut batches =
                        crate::upload_budget::split_terrain_batches(batches, local_crowns)?;
                    for batch in &mut batches {
                        batch.compact_indices();
                        if !batch.shadow_only && !templates.water_material(batch.material) {
                            super::precision::compact_batch_tints(batch);
                            super::precision::pack_surface_uvs(
                                batch,
                                templates.foliage_material(batch.material),
                            );
                        }
                    }
                    if let Some(slots) = blade_shadow_slots {
                        super::shadows::append_leaf_shadows(
                            &mut batches,
                            &slots,
                            folded_leaves.is_some_and(|folded| folded.nearby),
                        )?;
                    }
                    if let Some(source) = &near_shadow_geometry {
                        super::resident::ShadowPiece::identify(&mut batches, source, &templates);
                    }
                    super::resident::StaticPiece::identify(&mut batches, &templates);
                    Ok(batches)
                });
                (result, start.elapsed().as_secs_f64())
            });
            jobs.0.insert(
                position,
                RenderJob {
                    ready: None,
                    folded_leaves,
                    micro_detail,
                    epoch,
                    appearance_epoch,
                    level,
                    task,
                },
            );
        }
    }
    status.pending_jobs = jobs.0.len();
    static TRACE_READY: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("TERRA_READINESS_DIAGNOSTICS").is_some());
    if *TRACE_READY && readiness_trace.is_none_or(|last| last.elapsed() >= Duration::from_secs(5)) {
        tracing::info!(focus=?world.focus.0, invalidations=invalidation_flags, jobs=jobs.0.len(), retries=jobs.1.len(), queued=queue.positions.len(), arrivals=queue.arrivals.len(), selection=queue.sweep.as_ref().map_or(0, |sweep| sweep.positions.len()), dirty=queue.dirty, selection_dirty=queue.selection_dirty, arrival_scan=queue.arrival_scan, exposure_waiting=cache.5.columns.values().filter(|state| state.pending).count(), "Terrain readiness trace");
        *readiness_trace = Some(Instant::now());
    }
    status.occluded_sections = world.visibility.as_ref().map_or(0, |v| v.len());
    status.pending_columns.clear();
    status
        .pending_columns
        .extend(jobs.0.keys().map(|position| [position.x, position.z]));
    status.ready = assets.templates.is_some()
        && status.error.is_none()
        && jobs.0.is_empty()
        && jobs.1.is_empty()
        && queue.positions.is_empty()
        && !queue.has_selection_work()
        && world.chunks.loaded.iter().all(|(&position, section)| {
            status.section_ready(position, Some(section), world.visibility.as_deref())
        });
}
pub(crate) fn poll_job(job: &mut RenderJob) -> Option<(Result<Vec<MeshBatch>, String>, f64)> {
    job.ready
        .take()
        .or_else(|| future::block_on(future::poll_once(&mut job.task)))
}

pub(crate) fn focus_bucket(focus: [f64; 3]) -> [i64; 3] {
    // At most 4m per axis of delayed visual selection stays inside the 12m
    // micro-detail hysteresis band. Streaming/collisions still use raw focus.
    focus.map(|value| (value / 4.0).floor() as i64)
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    #[test]
    fn flight_heading_follows_reversals_and_clears_on_stop_walk_or_teleport() {
        let mut motion = FlightMotion::default();
        assert!(!motion.update([-8., 100., -8.], 0.12));
        assert!(motion.update([-16., 100., -8.], 0.12));
        assert_eq!(motion.heading, Some(DVec2::NEG_X));
        assert!(motion.update([-8., 100., -8.], 0.12));
        assert_eq!(motion.heading, Some(DVec2::X));
        assert!(motion.update([-8., 100., -8.], 0.12));
        assert_eq!(motion.heading, None);
        assert!(!motion.update([-7.5, 100., -8.], 0.12));
        assert!(motion.update([-15.5, 100., -8.], 0.12));
        assert!(motion.update([256., 100., -8.], 0.016));
        assert_eq!(motion.heading, None);
    }

    #[test]
    fn flight_corridor_serves_local_ground_and_visible_forward_surfaces_in_both_directions() {
        let focus = [-8., 100., -8.];
        let p = |x, y, z| SectionPos::new(x, y, z);
        let rank = |position, heading| flight_surface_rank(position, focus, heading, || Some(8.5));
        assert_eq!(rank(p(-1, 0, -1), DVec2::NEG_X), 0);
        assert_eq!(rank(p(-5, 0, -1), DVec2::NEG_X), 1);
        assert_eq!(rank(p(-5, 0, -1), DVec2::X), 2);
        assert_eq!(rank(p(3, 0, -1), DVec2::X), 1);
        assert_eq!(rank(p(-5, 0, 4), DVec2::NEG_X), 2);
        assert_eq!(rank(p(-5, 12, -1), DVec2::NEG_X), 2);
        assert_eq!(rank(p(-5, -4, -1), DVec2::NEG_X), 2);
        assert_eq!(
            flight_surface_rank(p(-5, 0, -1), focus, DVec2::NEG_X, || None),
            2
        );
    }

    #[test]
    fn empty_publications_do_not_spend_the_only_worker_slot() {
        use terra_voxel::meshing::BlockFace;
        use terra_world::streaming::LoadedSection;
        let mut config = terra_core::GameConfig::default();
        config.client.max_mesh_jobs = 1;
        let mut app = App::new();
        let mut assets = BlockAssets::default();
        assets.templates = Some(Arc::default());
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkStore>()
            .init_resource::<RenderedSections>()
            .init_resource::<GeometryCache>()
            .init_resource::<RenderJobs>()
            .init_resource::<RenderStatus>()
            .init_resource::<EditJournal>()
            .insert_resource(assets)
            .insert_resource(ChunkFocus([8.; 3]))
            .insert_resource(WorldTerrain(
                terra_world::TerraGenerator::new(Default::default()).unwrap(),
            ))
            .insert_resource(config)
            .add_systems(Update, dispatch_meshes);
        let visible = SectionPos::new(33, 0, 0);
        for x in 0..=visible.x {
            let position = SectionPos::new(x, 0, 0);
            let mut data = terra_voxel::BlockSection::empty(position).unwrap();
            let mut geometry = BlockMesh::default();
            if position == visible {
                data.set_block([0; 3], 3).unwrap();
                geometry.faces.push(BlockFace {
                    minimum: [0; 3],
                    width: 1,
                    height: 1,
                    u_axis: 0,
                    v_axis: 2,
                    side: 3,
                    material: 3,
                    edge_mask: 15,
                    occupied_neighbor: false,
                });
            }
            app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                position,
                LoadedSection {
                    data,
                    mesh: Some(geometry),
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        app.update();
        let rendered = app.world().resource::<RenderedSections>();
        let chunks = app.world().resource::<ChunkStore>();
        for x in 0..visible.x {
            let p = SectionPos::new(x, 0, 0);
            assert!(rendered.section_presented(p, chunks.loaded.get(&p), None));
        }
        assert!(
            app.world()
                .resource::<RenderJobs>()
                .0
                .contains_key(&visible)
        );
    }

    #[test]
    fn streaming_concurrency_tracks_frame_pressure_and_respects_lower_player_limits() {
        let mut queue = MeshQueue::default();
        assert_eq!(queue.refinement_limit(8, 0.10, 0), 8);
        queue.initial_ready = true;
        for _ in 0..16 {
            queue.refinement_limit(8, 0.025, 0);
        }
        assert_eq!(queue.refinement_limit(8, 1.0 / 60.0, 0), 3);
        assert_eq!(queue.refinement_limit(2, 1.0 / 60.0, 0), 2);
        for _ in 0..48 {
            queue.refinement_limit(8, 0.010, 0);
        }
        assert_eq!(queue.refinement_limit(8, 0.010, 0), 8);
        for _ in 0..48 {
            assert_eq!(queue.refinement_limit(8, 1.0 / 30.0, 30), 8);
        }
        queue.initial_ready = false;
        assert_eq!(queue.refinement_limit(8, 0.1, 60), 8);
    }

    #[test]
    fn portal_culled_cpu_meshes_allow_the_play_budget_but_visible_pending_meshes_do_not() {
        let hidden = SectionPos::new(-2, -3, -4);
        let visible = SectionPos::new(-2, 4, -4);
        let mut chunks = ChunkStore::default();
        for position in [hidden, visible] {
            chunks.loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: Some(BlockMesh::default()),
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        assert!(!MeshQueue::uploads_consumed(&chunks, |p| p == hidden));
        chunks.loaded.get_mut(&visible).unwrap().mesh = None;
        assert!(MeshQueue::uploads_consumed(&chunks, |p| p == hidden));
        assert!(!MeshQueue::uploads_consumed(&chunks, |_| false));

        let mut queue = MeshQueue::default();
        queue.update_initial_ready(false, MeshQueue::uploads_consumed(&chunks, |p| p == hidden));
        assert!(queue.initial_ready);
        queue.selection_dirty = true;
        queue.begin_sweep(|| vec![hidden; 15_000]);
        assert!(queue.sweep_candidate(0, Duration::ZERO).is_some());
        assert!(
            queue
                .sweep_candidate(1, Duration::from_micros(750))
                .is_none()
        );
        queue.finish_sweep();
        assert!(queue.sweep.is_some(), "remaining work resumes next frame");
        assert!(
            chunks.loaded[&hidden].mesh.is_some(),
            "culled geometry is retained"
        );
    }

    #[test]
    fn batched_unloads_scan_the_backlog_once_and_preserve_reloaded_sections() {
        const BACKLOG: usize = 8_192;
        const REMOVALS: usize = 1_024;
        let mut queue = MeshQueue::default();
        for x in 0..BACKLOG as i64 {
            let position = SectionPos::new(x, -2, -3);
            queue.enqueue(position);
            queue.arrivals.insert(position);
        }
        let notices: Vec<_> = (0..REMOVALS as i64)
            .flat_map(|x| [SectionPos::new(x, -2, -3); 2])
            .collect();
        let mut inspected = 0;
        let loaded = |position: SectionPos| position.x >= REMOVALS as i64 || position.x % 2 == 0;
        let removed = queue.remove_unloaded(notices, |position| {
            inspected += 1;
            loaded(position)
        });
        assert_eq!(removed.len(), REMOVALS / 2);
        assert_eq!(inspected, REMOVALS + BACKLOG * 3);
        assert!(queue.positions.iter().copied().all(loaded));
        assert_eq!(queue.positions.len(), BACKLOG - REMOVALS / 2);
        assert_eq!(queue.queued.len(), queue.positions.len());
        assert_eq!(queue.arrivals.len(), queue.positions.len());
        assert!(queue.positions.contains(&SectionPos::new(0, -2, -3)));
        assert!(!queue.positions.contains(&SectionPos::new(1, -2, -3)));

        let before = queue.positions.clone();
        let mut inspected = 0;
        assert!(
            queue
                .remove_unloaded([SectionPos::new(0, -2, -3); 2], |_| {
                    inspected += 1;
                    true
                })
                .is_empty()
        );
        assert_eq!(
            inspected, 1,
            "reloaded-only notices cannot rescan the queue"
        );
        assert_eq!(queue.positions, before);
    }

    #[test]
    fn large_backlog_priority_looks_up_each_candidate_once_without_changing_dispatch_order() {
        let mut queue = MeshQueue {
            positions: (-4_096..4_096)
                .rev()
                .map(|x| SectionPos::new(x, x.rem_euclid(9), -3))
                .collect(),
            ..Default::default()
        };
        let mut expected = queue.positions.clone();
        let focus = [24.0, 3.0, -8.0];
        let edited = |p: SectionPos| p.x.rem_euclid(79) == 0;
        expected.sort_unstable_by_key(|p| {
            std::cmp::Reverse((
                !edited(*p),
                distance_squared(*p, focus) as u64,
                p.y,
                p.z,
                p.x,
            ))
        });
        let mut lookups = 0;
        queue.prioritize_cached(focus, |p| {
            lookups += 1;
            if edited(p) {
                MeshPriority::Edit
            } else {
                MeshPriority::Resident
            }
        });
        assert_eq!(queue.positions, expected);
        assert!(
            lookups <= queue.positions.len(),
            "HashMap-backed edit keys must not be recomputed per comparison: {lookups}"
        );
    }

    #[test]
    fn incremental_candidates_keep_pending_work_and_deduplicate_publications() {
        let pending = SectionPos::new(1, 0, 0);
        let published = SectionPos::new(2, 0, 0);
        let mut queue = MeshQueue {
            positions: vec![pending],
            ..default()
        };
        queue.arrivals.extend([published, published, pending]);
        assert_eq!(
            queue.take_incremental_candidates(),
            HashSet::from([pending, published])
        );
        assert!(queue.arrivals.is_empty());
        assert!(queue.positions.is_empty());
    }

    #[test]
    fn selection_sweep_caps_candidates_and_soft_time_without_losing_the_tail() {
        let mut queue = MeshQueue {
            initial_ready: true,
            selection_dirty: true,
            ..default()
        };
        queue.begin_sweep(|| (0..13_052).map(|x| SectionPos::new(x, 0, 0)).collect());
        let mut visited = HashSet::new();
        let mut frames = 0;
        while queue.sweep.is_some() {
            frames += 1;
            let mut count = 0;
            while let Some((position, full)) = queue.sweep_candidate(count, Duration::ZERO) {
                assert!(full);
                assert!(visited.insert(position));
                count += 1;
            }
            assert!(count <= 1_024);
            queue.finish_sweep();
        }
        assert_eq!(visited.len(), 13_052);
        assert_eq!(frames, 13);
        assert!(!queue.has_selection_work());

        queue.selection_dirty = true;
        queue.begin_sweep(|| vec![SectionPos::new(-2, -3, -4); 4]);
        // Even if an indivisible snapshot exceeded the cap, make progress once.
        assert!(queue.sweep_candidate(0, Duration::from_millis(2)).is_some());
        assert!(
            queue
                .sweep_candidate(1, Duration::from_micros(750))
                .is_none()
        );
        assert_eq!(queue.sweep.as_ref().unwrap().positions.len(), 3);
    }

    #[test]
    fn moving_negative_focus_coalesces_requests_without_restarting_a_sweep() {
        let mut queue = MeshQueue {
            initial_ready: true,
            selection_dirty: true,
            ..default()
        };
        let mut snapshots = 0;
        queue.begin_sweep(|| {
            snapshots += 1;
            (-4_096..0).map(|x| SectionPos::new(x, -2, -8)).collect()
        });
        let mut visited = HashSet::new();
        for frame in 0..4 {
            let focus = [-0.5 - f64::from(frame) * 4.0, -32.0, -128.0];
            queue.selection_dirty |= queue
                .focus
                .is_none_or(|previous| focus_bucket(previous) != focus_bucket(focus));
            queue.focus = Some(focus);
            queue.begin_sweep(|| panic!("a newer focus must not reset an active snapshot"));
            for inspected in 0..1_024 {
                let (position, _) = queue.sweep_candidate(inspected, Duration::ZERO).unwrap();
                assert!(visited.insert(position));
            }
            queue.finish_sweep();
        }
        assert_eq!(snapshots, 1);
        assert_eq!(visited.len(), 4_096);
        assert!(queue.selection_dirty);
        assert!(queue.has_selection_work());
        queue.begin_sweep(|| {
            snapshots += 1;
            vec![SectionPos::new(-9, -2, -8)]
        });
        assert_eq!(snapshots, 2);
        assert!(!queue.selection_dirty);
        assert_eq!(
            queue.sweep_candidate(0, Duration::ZERO),
            Some((SectionPos::new(-9, -2, -8), true))
        );
        queue.finish_sweep();
        assert!(!queue.has_selection_work());
    }

    #[test]
    fn retries_are_deduplicated_dispatch_work_not_global_selection_requests() {
        let position = SectionPos::new(-1, 4, -2);
        let mut queue = MeshQueue {
            initial_ready: true,
            ..default()
        };
        assert!(queue.enqueue(position));
        assert!(!queue.enqueue(position));
        assert!(!queue.selection_dirty);
        assert!(!queue.has_selection_work());
        assert_eq!(queue.positions, vec![position]);
        assert_eq!(queue.pop_for_dispatch(0, Duration::ZERO), Some(position));
        assert!(queue.queued.is_empty());
        assert!(
            queue.enqueue(position),
            "a failed later revision can retry again"
        );
    }

    #[test]
    fn interactive_edits_precede_nearer_streaming_and_neighbor_epoch_only_changes() {
        let near = SectionPos::new(-1, 10, -1);
        let far = SectionPos::new(-3, 10, -1);
        let halo = far.offset([1, 0, 0]).unwrap();
        let mut journal = EditJournal::default();
        journal
            .record(far.global_block([15, 8, 8]).unwrap(), 3)
            .unwrap();
        let mut rendered = RenderedSections::default();
        assert!(interactive_edit(far, &journal, &rendered));
        assert!(!interactive_edit(halo, &journal, &rendered));
        let mut queue = MeshQueue::default();
        for position in [near, far, halo] {
            queue.enqueue(position);
        }
        queue.prioritize_work(
            [-8.0, 168.0, -8.0],
            &journal,
            &rendered,
            &ChunkStore::default(),
            &GeometryCache::default(),
        );
        assert_eq!(queue.pop_for_dispatch(0, Duration::ZERO), Some(far));
        assert_eq!(queue.pop_for_dispatch(1, Duration::ZERO), Some(near));
        assert_eq!(queue.pop_for_dispatch(2, Duration::ZERO), Some(halo));
        rendered.record_presentation(far, journal.revision(far), None);
        assert!(
            !interactive_edit(far, &journal, &rendered),
            "priority ends after the actual revision is visible"
        );
    }

    #[test]
    fn fresh_and_newly_exposed_terrain_can_use_free_slots_under_frame_pressure() {
        let resident = SectionPos::default();
        let empty = SectionPos::new(1, 0, 0);
        let exposed = SectionPos::new(2, 0, 0);
        let arrival = SectionPos::new(3, 0, 0);
        let edit = SectionPos::new(4, 0, 0);
        let mut rendered = RenderedSections::default();
        let mut journal = EditJournal::default();
        journal
            .record(edit.global_block([8; 3]).unwrap(), 3)
            .unwrap();
        rendered.sections.insert(
            resident,
            super::super::RenderedSection {
                folded_leaves: None,
                micro_detail: true,
                appearance_epoch: 0,
                entity: Entity::PLACEHOLDER,
                meshes: Vec::new(),
                mesh_epoch: 0,
                detail_level: DetailLevel::Near,
            },
        );
        rendered.record_presentation(empty, 0, Some(0));
        rendered.record_presentation(exposed, 0, None);
        let mut queue = MeshQueue::default();
        for p in [arrival, empty, edit, resident, exposed] {
            queue.enqueue(p);
        }
        queue.prioritize_work(
            [8.; 3],
            &journal,
            &rendered,
            &ChunkStore::default(),
            &GeometryCache::default(),
        );
        let mut order = Vec::new();
        while let Some(p) = queue.pop_for_dispatch(0, Duration::ZERO) {
            order.push((
                p,
                MeshPriority::for_section(p, &journal, &rendered).job_limit(8, 3),
            ));
        }
        assert_eq!(
            order,
            [
                (edit, 9),
                (exposed, 8),
                (arrival, 8),
                (resident, 3),
                (empty, 3)
            ]
        );
    }

    #[test]
    fn pending_dispatches_are_reprioritized_for_the_current_negative_focus() {
        let positions = [
            SectionPos::new(-1, -2, -1),
            SectionPos::new(-9, -2, -1),
            SectionPos::new(-5, -2, -1),
        ];
        let mut queue = MeshQueue::default();
        for position in positions {
            queue.enqueue(position);
        }
        queue.prioritize([-136.0, -24.0, -8.0]);
        assert_eq!(
            queue.pop_for_dispatch(0, Duration::ZERO),
            Some(positions[1])
        );
        assert_eq!(
            queue.pop_for_dispatch(1, Duration::ZERO),
            Some(positions[2])
        );
        assert_eq!(
            queue.pop_for_dispatch(2, Duration::ZERO),
            Some(positions[0])
        );
        assert!(queue.queued.is_empty());
    }

    #[test]
    fn owned_sweeps_survive_eviction_and_do_not_consume_immediate_arrivals() {
        let old = SectionPos::new(-4, -2, -1);
        let retained = SectionPos::new(-3, -2, -1);
        let arrival = SectionPos::new(-2, -2, -1);
        let mut loaded = HashSet::from([old, retained]);
        let mut queue = MeshQueue {
            initial_ready: true,
            selection_dirty: true,
            ..default()
        };
        queue.begin_sweep(|| loaded.iter().copied().collect());
        loaded.remove(&old);
        loaded.insert(arrival);
        queue.arrivals.insert(arrival);
        let mut selected = HashSet::new();
        for inspected in 0..1_024 {
            let Some((position, _)) = queue.sweep_candidate(inspected, Duration::ZERO) else {
                break;
            };
            // needs_mesh performs this live-store check before reading any cache.
            if loaded.contains(&position) {
                selected.insert(position);
            }
        }
        queue.finish_sweep();
        assert_eq!(selected, HashSet::from([retained]));
        assert_eq!(queue.arrivals, HashSet::from([arrival]));
        assert!(queue.has_selection_work());
    }

    #[test]
    fn missed_removal_notifications_prune_dispatch_membership_and_arrivals_together() {
        let removed = SectionPos::new(-5, -2, -1);
        let live = SectionPos::new(-4, -2, -1);
        let mut queue = MeshQueue::default();
        queue.enqueue(removed);
        queue.enqueue(live);
        queue.arrivals.extend([removed, live]);
        queue.retain_loaded(|position| position == live);
        assert_eq!(queue.positions, vec![live]);
        assert_eq!(queue.queued, HashSet::from([live]));
        assert_eq!(queue.arrivals, HashSet::from([live]));
        assert!(
            queue.enqueue(removed),
            "a reloaded position must not be stuck in membership"
        );
    }

    #[test]
    fn sparse_coastal_arrival_and_removal_touch_only_neighboring_sea_sections() {
        let mut cache = GeometryCache::default();
        let journal = EditJournal::default();
        let coast = SectionPos::new(-2, 0, -3);
        let deep = SectionPos::new(-2, -3, -3);
        let high = SectionPos::new(-2, 4, -3);
        for position in [coast, deep, high] {
            cache.insert_test(position, 0, BlockMesh::default());
            cache.0.get_mut(&position).unwrap().coastal = coastal_section(position, 8.0);
        }
        for x in 10..10_010 {
            let position = SectionPos::new(x, 0, 0);
            cache.insert_test(position, 0, BlockMesh::default());
            cache.0.get_mut(&position).unwrap().coastal = true;
            cache.2.insert([x, 0], 511);
        }
        let distant = SectionPos::new(10_000, 0, 0);
        let distant_geometry = cache.snapshot(distant, 0).unwrap().0;
        let column = [coast.x, coast.z];
        let neighbor = [-1, -3];
        let mut present = HashSet::from([column]);
        cache
            .2
            .insert(column, prepared_halo_mask(column, |p| present.contains(&p)));
        let before = [coast, deep, high, distant].map(|p| cache.appearance_epoch(p, &journal));
        present.insert(neighbor);
        let mut probes = 0;
        assert_eq!(
            cache.refresh_coastal_columns(&HashSet::from([neighbor]), 8.0, |p| {
                probes += 1;
                present.contains(&p)
            }),
            HashSet::from([coast])
        );
        assert!(
            probes <= 81,
            "unrelated cached sections must not be scanned"
        );
        let after = [coast, deep, high, distant].map(|p| cache.appearance_epoch(p, &journal));
        assert_ne!(before[0], after[0]);
        assert_eq!(before[1..], after[1..]);
        assert!(
            cache
                .refresh_coastal_columns(&HashSet::from([neighbor]), 8.0, |p| present.contains(&p))
                .is_empty()
        );
        present.remove(&neighbor);
        assert_eq!(
            cache
                .refresh_coastal_columns(&HashSet::from([neighbor]), 8.0, |p| present.contains(&p)),
            HashSet::from([coast])
        );
        assert_eq!(cache.appearance_epoch(coast, &journal), before[0]);
        cache.0.remove(&coast);
        present.remove(&column);
        cache.refresh_coastal_columns(&HashSet::from([column]), 8.0, |p| present.contains(&p));
        assert!(!cache.2.contains_key(&column));
        assert!(Arc::ptr_eq(
            &distant_geometry,
            &cache.snapshot(distant, 0).unwrap().0
        ));
    }

    #[test]
    fn a_world_switch_resets_the_cap_without_reusing_the_previous_world_ready_flag() {
        let position = SectionPos::default();
        let mut queue = MeshQueue {
            initial_ready: true,
            positions: vec![position],
            queued: HashSet::from([position]),
            arrivals: HashSet::from([position]),
            sweep: Some(SelectionSweep {
                positions: vec![position; 2_048],
                full_selection: true,
            }),
            ..default()
        };
        queue.update_initial_ready(false, false);
        assert!(
            queue.initial_ready,
            "ordinary streaming keeps the play budget"
        );
        assert_eq!(queue.pop_for_dispatch(1, Duration::from_secs(1)), None);
        queue.update_initial_ready(true, true);
        assert!(!queue.initial_ready, "the previous world's Ready is stale");
        assert!(
            queue.positions.is_empty(),
            "old-world dispatches must not delay loading"
        );
        assert!(queue.queued.is_empty());
        assert!(queue.arrivals.is_empty());
        assert!(queue.sweep.is_none());
        queue.enqueue(position);
        assert_eq!(
            queue.pop_for_dispatch(1, Duration::from_secs(1)),
            Some(position),
            "the next world's initial loading can fill every worker"
        );
        queue.update_initial_ready(false, false);
        assert!(!queue.initial_ready);
        queue.update_initial_ready(false, true);
        assert!(queue.initial_ready);
    }

    #[test]
    fn preparation_cap_preserves_priority_and_pending_candidates_between_frames() {
        let far = SectionPos::new(3, 0, 0);
        let middle = SectionPos::new(2, 0, 0);
        let near = SectionPos::new(1, 0, 0);
        let mut queue = MeshQueue {
            initial_ready: true,
            positions: vec![far, middle, near],
            ..default()
        };
        // Always inspect one. Even if it proves stale, the work spent on it
        // counts towards the deadline and the rest of the queue survives.
        assert_eq!(
            queue.pop_for_dispatch(0, Duration::from_millis(9)),
            Some(near)
        );
        assert_eq!(queue.pop_for_dispatch(1, Duration::from_millis(2)), None);
        assert_eq!(queue.positions, vec![far, middle]);
        // The next frame resumes the same ordered queue and can fill free
        // worker slots while preparation remains under the soft limit.
        assert_eq!(queue.pop_for_dispatch(0, Duration::ZERO), Some(middle));
        assert_eq!(
            queue.pop_for_dispatch(1, Duration::from_micros(1_999)),
            Some(far)
        );
        assert!(queue.positions.is_empty());
    }

    #[test]
    fn initial_preparation_fills_all_available_workers_without_a_time_cap() {
        let mut queue = MeshQueue {
            positions: (0..16).rev().map(|x| SectionPos::new(x, 0, 0)).collect(),
            ..default()
        };
        for dispatched in 0..16 {
            assert_eq!(
                queue.pop_for_dispatch(dispatched, Duration::from_secs(1)),
                Some(SectionPos::new(dispatched as i64, 0, 0))
            );
        }
        assert!(queue.positions.is_empty());
    }

    #[test]
    fn discarded_startup_candidates_do_not_drain_an_unbounded_backlog_in_one_frame() {
        let mut queue = MeshQueue::default();
        for x in (0..140).rev() {
            queue.enqueue(SectionPos::new(x, 0, 0));
        }
        for inspected in 0..128 {
            assert!(queue.pop_for_dispatch(inspected, Duration::ZERO).is_some());
        }
        assert_eq!(queue.pop_for_dispatch(128, Duration::ZERO), None);
        assert_eq!(
            queue.pop_for_dispatch(0, Duration::ZERO),
            Some(SectionPos::new(128, 0, 0))
        );
    }
}

#[cfg(test)]
mod stream_exposure_tests {
    use super::*;
    use crate::weather::surfaces::exposure::{Exposure, RoofWaterState};

    fn mask(roof: i16) -> Arc<Exposure> {
        Arc::new(Exposure {
            natural: None,
            roofs: [roof; 256],
            snow_roofs: [roof; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        })
    }

    #[test]
    fn appearance_only_journal_updates_retain_exposure_and_block_edits_invalidate_only_their_halo()
    {
        let mut inputs = AppearanceInputs::default();
        let mut cache = GeometryCache::default();
        let mut arrivals = HashSet::new();
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let local = SectionPos::new(-2, 1, -3);
        let neighbor = SectionPos::new(-1, 2, -3);
        let distant = SectionPos::new(20, 4, 30);
        let snapshot = mask(32);
        for position in [local, neighbor, distant] {
            chunks.loaded.insert(
                position,
                terra_world::streaming::LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
            cache.5.watch(position, Some(snapshot.clone()));
            cache
                .4
                .insert([position.x, position.z], Some(snapshot.clone()));
        }

        journal.record(local.origin().unwrap(), 3).unwrap();
        inputs.refresh_journal(&journal, &chunks, &terrain, &mut cache, &mut arrivals);
        assert_eq!(arrivals, HashSet::from([local, neighbor]));
        assert!(!cache.4.contains_key(&[local.x, local.z]));
        assert!(!cache.4.contains_key(&[neighbor.x, neighbor.z]));
        assert!(Arc::ptr_eq(
            cache.4[&[distant.x, distant.z]].as_ref().unwrap(),
            &snapshot
        ));

        arrivals.clear();
        for day in 1..=128 {
            journal.clock = Some(terra_core::WorldClock {
                day,
                ..Default::default()
            });
            inputs.refresh_journal(&journal, &chunks, &terrain, &mut cache, &mut arrivals);
            assert!(
                arrivals.is_empty(),
                "clock updates cannot restart terrain selection"
            );
        }
        journal
            .set_surface_condition(
                distant.origin().unwrap(),
                terra_world::environment::SurfaceCondition {
                    snow: 0.5,
                    ..Default::default()
                },
            )
            .unwrap();
        inputs.refresh_journal(&journal, &chunks, &terrain, &mut cache, &mut arrivals);
        assert_eq!(arrivals, HashSet::from([distant]));
        assert!(cache.4.contains_key(&[distant.x, distant.z]));

        arrivals.clear();
        inputs.refresh_journal(
            &EditJournal::default(),
            &chunks,
            &terrain,
            &mut cache,
            &mut arrivals,
        );
        assert_eq!(arrivals, HashSet::from([local, neighbor, distant]));
    }

    #[test]
    fn higher_streamed_roof_arrival_and_removal_change_only_watched_column_owners() {
        let mut cache = GeometryCache::default();
        let journal = EditJournal::default();
        let owner = SectionPos::new(-2, 2, -3);
        let other = SectionPos::new(-1, 2, -3);
        let column = [owner.x, owner.z];
        let before_mask = mask(32);
        for position in [owner, other] {
            cache.insert_test(position, 0, BlockMesh::default());
            cache.5.watch(position, Some(before_mask.clone()));
        }
        cache
            .3
            .insert(column, RoofWaterState::captured(&before_mask, 0, 1));
        let geometry = cache.snapshot(owner, 0).unwrap().0;
        let old_epoch = cache.appearance_epoch(owner, &journal);
        let other_epoch = cache.appearance_epoch(other, &journal);
        assert!(cache.5.mark_pending(column));
        assert!(!cache.5.mark_pending(column), "arrival bursts coalesce");
        assert_ne!(cache.appearance_epoch(owner, &journal), old_epoch);
        assert!(cache.5.pending(owner));
        assert!(cache.5.finish(column, Some(mask(80))));
        assert_eq!(cache.5.changed, HashSet::from([owner]));
        assert!(!cache.5.pending(owner));
        let roof_epoch = cache.appearance_epoch(owner, &journal);
        assert_ne!(roof_epoch, old_epoch);
        assert_eq!(
            cache.3[&column].epoch, 1,
            "stream changes preserve fluid revisions"
        );
        assert_eq!(cache.appearance_epoch(other, &journal), other_epoch);
        assert_eq!(journal.mesh_epoch(owner), 0);
        assert!(Arc::ptr_eq(&geometry, &cache.snapshot(owner, 0).unwrap().0));
        cache.5.changed.clear();
        assert!(cache.5.mark_pending(column));
        assert!(cache.5.finish(column, Some(before_mask.clone())));
        assert_eq!(cache.5.changed, HashSet::from([owner]));
        assert_ne!(cache.appearance_epoch(owner, &journal), roof_epoch);
        assert_eq!(cache.appearance_epoch(other, &journal), other_epoch);
        cache.5.changed.clear();
        let snapshot = cache.5.columns[&column].snapshot.clone().unwrap();
        let unchanged_epoch = cache.appearance_epoch(owner, &journal);
        assert!(cache.5.mark_pending(column));
        assert!(!cache.5.finish(column, Some(mask(32))));
        assert!(cache.5.changed.is_empty());
        assert_eq!(cache.appearance_epoch(owner, &journal), unchanged_epoch);
        assert!(Arc::ptr_eq(
            &snapshot,
            cache.5.columns[&column].snapshot.as_ref().unwrap()
        ));
    }

    #[test]
    fn prepared_column_only_arrival_captures_previously_missing_masks_and_reload_is_bounded() {
        let position = SectionPos::new(-2, 2, -3);
        let column = [position.x, position.z];
        let journal = EditJournal::default();
        let mut cache = GeometryCache::default();
        let mut previous = 0;
        for _ in 0..128 {
            cache.insert_test(position, 0, BlockMesh::default());
            cache.5.watch(position, None);
            assert_ne!(cache.appearance_epoch(position, &journal), previous);
            assert!(cache.5.mark_pending(column));
            assert!(cache.5.finish(column, Some(mask(80))));
            assert_eq!(cache.5.changed, HashSet::from([position]));
            previous = cache.appearance_epoch(position, &journal);
            assert_eq!(cache.5.columns.len(), 1);
            cache.4.insert(column, Some(mask(80)));
            cache.forget(position);
            assert!(cache.0.is_empty());
            assert!(cache.5.columns.is_empty());
            assert!(cache.5.changed.is_empty());
            assert!(cache.4.is_empty());
        }
    }

    #[test]
    fn pending_stream_recapture_preserves_staged_workers_until_masks_are_known() {
        use terra_world::streaming::LoadedSection;
        for changed in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<Assets<Mesh>>()
                .init_resource::<crate::blocks::BlockAssets>()
                .init_resource::<RenderedSections>()
                .init_resource::<RenderStatus>()
                .init_resource::<RenderJobs>()
                .init_resource::<GeometryCache>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .add_systems(Update, super::super::upload_sections);
            app.world_mut()
                .resource_mut::<crate::blocks::BlockAssets>()
                .materials
                .push(Handle::default());
            let position = SectionPos::new(-2, 2, -3);
            let unrelated = SectionPos::new(3, 2, -3);
            for owner in [position, unrelated] {
                app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
                    owner,
                    LoadedSection {
                        data: terra_voxel::BlockSection::empty(owner).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
            let old_epoch;
            {
                let mut cache = app.world_mut().resource_mut::<GeometryCache>();
                cache.insert_test(position, 0, BlockMesh::default());
                cache.5.watch(position, Some(mask(32)));
                old_epoch = cache.appearance_epoch(position, &EditJournal::default());
                assert!(cache.5.mark_pending([position.x, position.z]));
            }
            let handle = app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(Mesh::from(Cuboid::default()));
            let staged = super::super::staging::StagedMesh {
                handle: handle.clone(),
                liquid: None,
                bounds: None,
                rooted: false,
                damp: false,
                opaque_shadow: false,
                coastal_quads: 0,
                far: None,
            };
            for (owner, epoch, batches) in [
                (
                    position,
                    old_epoch,
                    vec![MeshBatch {
                        staged: Some(staged),
                        ..Default::default()
                    }],
                ),
                (unrelated, 0, Vec::new()),
            ] {
                app.world_mut().resource_mut::<RenderJobs>().0.insert(
                    owner,
                    RenderJob {
                        ready: Some((Ok(batches), 0.0)),
                        folded_leaves: None,
                        micro_detail: true,
                        epoch: 0,
                        appearance_epoch: epoch,
                        level: DetailLevel::Near,
                        task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
                    },
                );
            }
            app.update();
            let jobs = app.world().resource::<RenderJobs>();
            assert!(jobs.0[&position].ready.is_some());
            assert!(
                jobs.1.is_empty(),
                "pending is not evidence of stale geometry"
            );
            assert!(app.world().resource::<Assets<Mesh>>().contains(handle.id()));
            assert!(
                app.world()
                    .resource::<RenderedSections>()
                    .presentations
                    .contains_key(&unrelated),
                "one pending column cannot block unrelated uploads"
            );
            {
                let mut cache = app.world_mut().resource_mut::<GeometryCache>();
                assert_eq!(
                    cache.5.finish(
                        [position.x, position.z],
                        Some(mask(if changed { 80 } else { 32 })),
                    ),
                    changed
                );
            }
            app.update();
            assert!(app.world().resource::<RenderJobs>().0.is_empty());
            assert_eq!(
                app.world().resource::<RenderJobs>().1,
                if changed { vec![position] } else { Vec::new() }
            );
            assert_eq!(
                app.world()
                    .resource::<RenderedSections>()
                    .sections
                    .contains_key(&position),
                !changed
            );
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().contains(handle.id()),
                !changed,
                "only genuinely stale staging is released"
            );
        }
    }

    #[test]
    fn upload_rejects_worker_captured_before_streamed_roof_change() {
        use terra_world::streaming::LoadedSection;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<RenderedSections>()
            .init_resource::<RenderStatus>()
            .init_resource::<RenderJobs>()
            .init_resource::<GeometryCache>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .add_systems(Update, super::super::upload_sections);
        let position = SectionPos::new(-2, 2, -3);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let old_epoch;
        {
            let mut cache = app.world_mut().resource_mut::<GeometryCache>();
            cache.insert_test(position, 0, BlockMesh::default());
            cache.5.watch(position, Some(mask(32)));
            old_epoch = cache.appearance_epoch(position, &EditJournal::default());
            assert!(cache.5.mark_pending([position.x, position.z]));
            assert!(cache.5.finish([position.x, position.z], Some(mask(80))));
        }
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            position,
            RenderJob {
                ready: Some((Ok(Vec::new()), 0.0)),
                folded_leaves: None,
                micro_detail: true,
                epoch: 0,
                appearance_epoch: old_epoch,
                level: DetailLevel::Near,
                task: AsyncComputeTaskPool::get().spawn(async { (Ok(Vec::new()), 0.0) }),
            },
        );
        app.update();
        assert!(app.world().resource::<RenderJobs>().0.is_empty());
        assert_eq!(app.world().resource::<RenderJobs>().1, vec![position]);
        assert!(
            app.world()
                .resource::<RenderedSections>()
                .sections
                .is_empty()
        );
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    }
}

#[cfg(test)]
mod mineral_tests {
    use super::*;

    #[test]
    fn high_player_roof_edits_invalidate_lower_crown_tags_without_changing_voxel_epochs() {
        let crown = SectionPos::new(-2, 2, -3);
        let roof = SectionPos::new(-2, 5, -3);
        let other = SectionPos::new(-1, 2, -3);
        let mut cache = GeometryCache::default();
        let mut journal = EditJournal::default();
        for position in [crown, roof, other] {
            cache.insert_test(position, 0, BlockMesh::default());
        }
        let before = cache.appearance_epoch(crown, &journal);
        let other_before = cache.appearance_epoch(other, &journal);
        let rooftop = [-24, 82, -40];
        for material in [3, 0] {
            journal.record(rooftop, material).unwrap();
            let epoch = journal
                .changed_sections()
                .filter(|position| [position.x, position.z] == [crown.x, crown.z])
                .map(|position| journal.revision(position))
                .fold(0_u64, u64::wrapping_add);
            cache.1.insert([crown.x, crown.z], epoch);
            assert!(cache.appearance_epoch(crown, &journal) > before);
            assert_eq!(journal.mesh_epoch(crown), 0);
            assert_eq!(cache.appearance_epoch(other, &journal), other_before);
        }
    }

    #[test]
    fn remote_segment_cut_refreshes_taper_and_wind_without_rebuilding_unchanged_voxels() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            base_height: -60.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            water: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let generator = terra_world::SectionGenerator::new(terrain.clone());
        let owner = SectionPos::new(-1, 0, -1);
        let mut journal = EditJournal::default();
        journal.record([-1, 14, -1], 3).unwrap();
        for y in 15..=35 {
            journal.record([-1, y, -1], 51).unwrap();
        }
        let geometry = generator
            .build(owner, 0, &journal.snapshot(owner).unwrap(), true)
            .unwrap()
            .mesh
            .unwrap();
        let mut cache = GeometryCache::default();
        cache.insert_test(owner, 0, geometry.clone());
        let before = cache.appearance_epoch(owner, &journal);
        let local_epoch = journal.mesh_epoch(owner);
        assert_eq!(geometry.segments[0].above, 20);
        journal.record([-1, 35, -1], 0).unwrap();
        assert_eq!(journal.mesh_epoch(owner), local_epoch);
        assert_ne!(cache.appearance_epoch(owner, &journal), before);
        let mut geometry = Arc::new(geometry);
        refresh_growth(
            &mut geometry,
            owner,
            &terrain,
            &journal.snapshot(owner).unwrap(),
        );
        assert_eq!(geometry.segments[0].above, 19);
    }

    #[test]
    fn neighboring_block_edits_refresh_only_coastal_wash_without_changing_collision_epochs() {
        let coast = SectionPos::new(-2, 0, -3);
        let deep = SectionPos::new(-2, -3, -3);
        let mut cache = GeometryCache::default();
        let mut journal = EditJournal::default();
        for position in [coast, deep] {
            cache.insert_test(position, 0, BlockMesh::default());
            cache.0.get_mut(&position).unwrap().coastal = coastal_section(position, 8.0);
        }
        let before = [coast, deep].map(|p| cache.appearance_epoch(p, &journal));
        let neighbor = SectionPos::new(-1, 0, -3);
        journal.record([-14, 7, -45], 3).unwrap();
        assert_eq!(
            journal.mesh_epoch(coast),
            0,
            "the edited cell is beyond the ordinary one-cell meshing halo"
        );
        cache
            .1
            .insert([neighbor.x, neighbor.z], journal.revision(neighbor));
        assert_ne!(cache.appearance_epoch(coast, &journal), before[0]);
        assert_eq!(cache.appearance_epoch(deep, &journal), before[1]);
        assert_eq!(journal.mesh_epoch(coast), 0);
    }

    #[test]
    fn coastal_prepared_halo_arrival_rebuilds_only_sea_sections_once() {
        let coast = SectionPos::new(-2, 0, -3);
        let deep = SectionPos::new(-2, -3, -3);
        let high = SectionPos::new(-2, 4, -3);
        let mut cache = GeometryCache::default();
        let journal = EditJournal::default();
        for position in [coast, deep, high] {
            cache.insert_test(position, 0, BlockMesh::default());
            cache.0.get_mut(&position).unwrap().coastal = coastal_section(position, 8.0);
        }
        let column = [coast.x, coast.z];
        let first = prepared_halo_mask(column, |p| p == column);
        cache.2.insert(column, first);
        let before = [coast, deep, high].map(|position| cache.appearance_epoch(position, &journal));
        let arrived = prepared_halo_mask(column, |p| p == column || p == [-1, -3]);
        assert_ne!(first, arrived);
        cache.2.insert(column, arrived);
        let after = [coast, deep, high].map(|position| cache.appearance_epoch(position, &journal));
        assert_ne!(before[0], after[0]);
        assert_eq!(before[1..], after[1..]);
        assert_eq!(
            prepared_halo_mask(column, |p| p == column || p == [-1, -3]),
            arrived
        );
        assert_eq!(cache.appearance_epoch(coast, &journal), after[0]);
        assert_eq!(prepared_halo_mask([-20, -30], |_| true), 511);
    }

    #[test]
    fn neighboring_clearance_edits_refresh_only_salt_rendering_not_collision_epochs() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            cave_structure_revision: 0,
            base_height: -64.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            biomes: false,
            caves: false,
            trees: false,
            water: false,
            ..default()
        })
        .unwrap();
        let generator = terra_world::SectionGenerator::new(terrain.clone());
        let root = (-48..48)
            .map(|x| [x, 16, -16])
            .find(|&p| terra_world::ore_shapes::salt_growth_height(p, 2, |_| Some(0)) > 8.0)
            .unwrap();
        let (section, _) = terra_voxel::global_to_section(root);
        let mut journal = EditJournal::default();
        journal.record(root, 30).unwrap();
        let mesh = generator
            .build(section, 1, &journal.snapshot(section).unwrap(), true)
            .unwrap()
            .mesh
            .unwrap();
        assert!(!mesh.mineral_growth.is_empty());
        let mut cache = GeometryCache::default();
        let collision_epoch = journal.mesh_epoch(section);
        cache.insert_test(section, collision_epoch, mesh);
        let appearance = cache.appearance_epoch(section, &journal);
        let blocker = [root[0], 14, root[2]];
        journal.record(blocker, 3).unwrap();
        assert_eq!(journal.mesh_epoch(section), collision_epoch);
        assert!(cache.appearance_epoch(section, &journal) > appearance);
        let (mut snapshot, _) = cache.snapshot(section, collision_epoch).unwrap();
        refresh_growth(
            &mut snapshot,
            section,
            &terrain,
            &journal.snapshot(section).unwrap(),
        );
        assert!(
            !snapshot
                .mineral_growth
                .iter()
                .any(|&(_, side, _)| side == 2)
        );
        journal.record(blocker, 0).unwrap();
        refresh_growth(
            &mut snapshot,
            section,
            &terrain,
            &journal.snapshot(section).unwrap(),
        );
        assert!(
            snapshot
                .mineral_growth
                .iter()
                .any(|&(_, side, h)| side == 2 && h > 8.0)
        );
        cache.insert_test(section, collision_epoch, BlockMesh::default());
        assert_eq!(
            cache.appearance_epoch(section, &journal),
            journal.appearance_epoch(section)
        );
    }
}
