use super::{
    growth::{self, Pose},
    shapes,
    wind::{self, PropGrowthStatus, PropMaterial, WindProp},
};
use crate::{
    appearance::{GrassTint, condition_tint},
    blocks::{BlockAssets, PropMaterials},
    meshing::{
        DetailLevel,
        builder::MeshBatch,
        lod::{distance_squared, select_prop_level},
    },
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::VisibilityRange,
    ecs::{message::MessageCursor, system::SystemParam},
    mesh::PrimitiveTopology,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use terra_core::GameConfig;
use terra_voxel::{SectionPos, global_to_section};
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    environment::SurfaceSnapshot,
    props::{PropKind, WorldProp},
    streaming::{
        ChunkFocus, ChunkStore, PreparedColumnChanged, SectionPublished, SectionRemoved,
        StreamingSet, WorldTerrain,
    },
};

#[derive(Resource, Default)]
pub struct PropRenderStatus {
    last_focus: Option<[f64; 3]>,
    pub ready: bool,
    /// Props represented by loaded render batches, before camera-frustum culling.
    pub visible_props: usize,
    pub batches: usize,
    pub pending_jobs: usize,
    pub composition_seconds: f64,
    pub completed_jobs: usize,
    pub error: Option<String>,
}

struct PropTemplate {
    geometry: MeshBatch,
    biome_tinted: Vec<bool>,
}
type Templates = HashMap<(u8, u8, DetailLevel), PropTemplate>;
type ComposedProps = Result<(Vec<MeshBatch>, usize), String>;
#[derive(Component)]
struct PropMesh;
#[derive(Component)]
struct PropSection(SectionPos);
#[derive(Resource, Default)]
struct PropTemplates {
    geometry: Option<Arc<Templates>>,
    material: Option<Handle<PropMaterial>>,
}
struct RenderedProps {
    density: f32,
    entity: Option<Entity>,
    meshes: Vec<Handle<Mesh>>,
    terrain_epoch: u64,
    prop_epoch: u64,
    appearance_epoch: u64,
    level: DetailLevel,
    count: usize,
    growth: HashMap<terra_world::props::PropId, Pose>,
}
#[derive(Resource, Default)]
struct PropBatches(HashMap<SectionPos, RenderedProps>);
struct PropJob {
    ready: Option<(ComposedProps, f64)>,
    density: f32,
    terrain_epoch: u64,
    prop_epoch: u64,
    appearance_epoch: u64,
    level: DetailLevel,
    task: Task<(ComposedProps, f64)>,
    growth: HashMap<terra_world::props::PropId, Pose>,
}
#[derive(Resource, Default)]
struct PropJobs(HashMap<SectionPos, PropJob>, HashSet<SectionPos>);

#[derive(Resource, Default)]
struct PropSnowExposure {
    revisions: HashMap<[i64; 2], u64>,
    fluids: HashMap<[i64; 2], u64>,
    water: HashMap<[i64; 2], crate::weather::surfaces::exposure::RoofWaterState>,
    sections: HashMap<[i64; 2], HashSet<SectionPos>>,
    /// Keep fluid ripples out of the global prop selection path.
    non_fluid: HashMap<SectionPos, (u64, u64, u64)>,
    non_fluid_changed: bool,
    water_changed: HashSet<[i64; 2]>,
    stream_epochs: HashMap<[i64; 2], u64>,
    stream_changed: HashSet<[i64; 2]>,
    stream_pending: HashSet<[i64; 2]>,
    stream_snapshots: HashMap<[i64; 2], Arc<crate::weather::surfaces::exposure::Exposure>>,
    stream_sequence: u64,
    #[cfg(test)]
    sampled_sections: usize,
}

impl PropSnowExposure {
    fn appearance_epoch(&self, position: SectionPos, journal: &EditJournal) -> u64 {
        let column = [position.x, position.z];
        journal
            .appearance_epoch(position)
            .wrapping_add(self.revisions.get(&column).copied().unwrap_or(0))
            .wrapping_add(self.water.get(&column).map_or(0, |water| water.epoch))
            .wrapping_add(self.stream_epochs.get(&column).copied().unwrap_or(0))
    }

    fn record_stream_changes(&mut self, changes: impl Iterator<Item = SectionPos>) {
        for changed in changes {
            let column = [changed.x, changed.z];
            if self
                .sections
                .get(&column)
                .is_some_and(|sections| sections.iter().any(|owner| owner.y < changed.y))
            {
                self.stream_pending.insert(column);
            }
        }
    }

    fn record_prepared_changes(&mut self, columns: impl Iterator<Item = [i64; 2]>) {
        for column in columns {
            if self.sections.contains_key(&column) {
                self.stream_pending.insert(column);
            }
        }
    }

    fn resolve_stream_snapshot(
        &mut self,
        column: [i64; 2],
        snapshot: Option<Arc<crate::weather::surfaces::exposure::Exposure>>,
    ) {
        let changed = match (self.stream_snapshots.get(&column), snapshot.as_ref()) {
            (Some(previous), Some(next)) => !previous.same_masks(next),
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.stream_sequence = self.stream_sequence.wrapping_add(1);
            self.stream_epochs
                .insert(column, self.stream_sequence.wrapping_shl(32));
            self.stream_changed.insert(column);
            if let Some(snapshot) = snapshot {
                if let Some(water) = self.water.get(&column) {
                    let water = crate::weather::surfaces::exposure::RoofWaterState::captured(
                        &snapshot,
                        self.fluids.get(&column).copied().unwrap_or(0),
                        water.epoch,
                    );
                    self.water.insert(column, water);
                }
                self.stream_snapshots.insert(column, snapshot);
            } else {
                self.stream_snapshots.remove(&column);
            }
        }
        self.stream_pending.remove(&column);
    }

    fn refresh(&mut self, chunks: &ChunkStore, terrain: &TerraGenerator, journal: &EditJournal) {
        self.revisions.clear();
        self.fluids.clear();
        self.water_changed.clear();
        let previous = std::mem::take(&mut self.non_fluid);
        #[cfg(test)]
        {
            self.sampled_sections = 0;
        }
        let columns: Vec<_> = self.sections.keys().copied().collect();
        let history = journal.changed_sections();
        let local_count = if terrain.config().bounded_world {
            columns.len().saturating_mul(31)
        } else {
            chunks.loaded.len()
        };
        if history.size_hint().0 < local_count {
            for column in columns {
                self.revisions.insert(column, 0);
                self.fluids.insert(column, 0);
            }
            for section in history {
                if self.sections.contains_key(&[section.x, section.z])
                    && (if terrain.config().bounded_world {
                        (-8..=22).contains(&section.y)
                    } else {
                        chunks.loaded.contains_key(&section)
                    })
                {
                    self.sample_section(journal, section);
                }
            }
        } else if terrain.config().bounded_world {
            for column in columns {
                self.sample_column(chunks, terrain, journal, column);
            }
        } else {
            for column in columns {
                self.revisions.insert(column, 0);
                self.fluids.insert(column, 0);
            }
            for &section in chunks.loaded.keys() {
                if self.sections.contains_key(&[section.x, section.z]) {
                    self.sample_section(journal, section);
                }
            }
        }
        self.non_fluid_changed = self.non_fluid != previous;
        for (&column, state) in &mut self.water {
            let before = state.epoch;
            state.refresh(
                chunks,
                column,
                terrain,
                journal,
                self.fluids.get(&column).copied().unwrap_or(0),
            );
            if state.epoch != before {
                self.water_changed.insert(column);
            }
        }
    }

    fn sample_section(&mut self, journal: &EditJournal, section: SectionPos) {
        #[cfg(test)]
        {
            self.sampled_sections += 1;
        }
        let column = [section.x, section.z];
        let revision = self.revisions.entry(column).or_default();
        *revision = revision.wrapping_add(journal.revision(section));
        let fluid = self.fluids.entry(column).or_default();
        *fluid = fluid.wrapping_add(journal.fluid_epoch(section));
        let stamp = (
            journal.revision(section),
            journal.appearance_epoch(section),
            journal.prop_epoch(section),
        );
        if stamp != (0, 0, 0) {
            self.non_fluid.insert(section, stamp);
        }
    }

    fn sample_column(
        &mut self,
        chunks: &ChunkStore,
        terrain: &TerraGenerator,
        journal: &EditJournal,
        column: [i64; 2],
    ) {
        self.revisions.insert(column, 0);
        self.fluids.insert(column, 0);
        if terrain.config().bounded_world {
            // Match bounded generation and Exposure::capture, independent of travel history.
            for y in (-128_i64).div_euclid(16)..=356_i64.div_euclid(16) {
                self.sample_section(journal, SectionPos::new(column[0], y, column[1]));
            }
        } else {
            for &section in chunks.loaded.keys().filter(|p| [p.x, p.z] == column) {
                self.sample_section(journal, section);
            }
        }
    }

    fn retain_loaded(&mut self, chunks: &ChunkStore) {
        self.sections.retain(|_, sections| {
            sections.retain(|position| chunks.loaded.contains_key(position));
            !sections.is_empty()
        });
        self.revisions
            .retain(|column, _| self.sections.contains_key(column));
        self.fluids
            .retain(|column, _| self.sections.contains_key(column));
        self.water
            .retain(|column, _| self.sections.contains_key(column));
        self.water_changed
            .retain(|column| self.sections.contains_key(column));
        self.non_fluid
            .retain(|p, _| self.sections.contains_key(&[p.x, p.z]));
        self.stream_epochs
            .retain(|column, _| self.sections.contains_key(column));
        self.stream_changed
            .retain(|column| self.sections.contains_key(column));
        self.stream_pending
            .retain(|column| self.sections.contains_key(column));
        self.stream_snapshots
            .retain(|column, _| self.sections.contains_key(column));
    }

    fn watch(
        &mut self,
        position: SectionPos,
        chunks: &ChunkStore,
        terrain: &TerraGenerator,
        journal: &EditJournal,
    ) {
        let column = [position.x, position.z];
        let first = !self.sections.contains_key(&column);
        self.sections.entry(column).or_default().insert(position);
        if first {
            self.sample_column(chunks, terrain, journal, column);
        }
    }

    fn capture(
        &mut self,
        position: SectionPos,
        exposure: &crate::weather::surfaces::exposure::Exposure,
        chunks: &ChunkStore,
        terrain: &TerraGenerator,
        journal: &EditJournal,
    ) {
        self.watch(position, chunks, terrain, journal);
        let column = [position.x, position.z];
        if self
            .stream_snapshots
            .get(&column)
            .is_none_or(|previous| !previous.same_masks(exposure))
        {
            self.stream_snapshots.insert(
                column,
                Arc::new(crate::weather::surfaces::exposure::Exposure {
                    natural: exposure.natural.clone(),
                    roofs: exposure.roofs,
                    snow_roofs: exposure.snow_roofs,
                    coast: exposure.coast,
                    coast_vertices: exposure.coast_vertices,
                    water: exposure.water,
                }),
            );
        }
        let epoch = self.water.get(&column).map_or(0, |water| water.epoch);
        self.water.insert(
            column,
            crate::weather::surfaces::exposure::RoofWaterState::captured(
                exposure,
                self.fluids.get(&column).copied().unwrap_or(0),
                epoch,
            ),
        );
    }
}

fn prop_appearance_epoch(
    state: Option<&PropSnowExposure>,
    position: SectionPos,
    journal: &EditJournal,
) -> u64 {
    state.map_or_else(
        || journal.appearance_epoch(position),
        |state| state.appearance_epoch(position, journal),
    )
}
#[derive(Default)]
struct PropQueue {
    dirty: bool,
    selection_dirty: bool,
    positions: Vec<(SectionPos, DetailLevel, u64, u64)>,
    changed: HashSet<SectionPos>,
    changes: PropChangeReader,
}

#[derive(Default)]
struct PropChangeReader {
    published: MessageCursor<SectionPublished>,
    removed: MessageCursor<SectionRemoved>,
    columns: MessageCursor<PreparedColumnChanged>,
}

#[derive(Default)]
struct PropChanges {
    published: Vec<SectionPos>,
    removed: Vec<SectionPos>,
    columns: HashSet<[i64; 2]>,
}

impl PropChanges {
    fn any(&self) -> bool {
        !self.published.is_empty() || !self.removed.is_empty() || !self.columns.is_empty()
    }
}

impl PropChangeReader {
    fn read(&mut self, world: &PropWorld) -> PropChanges {
        PropChanges {
            published: world.published.as_ref().map_or_else(Vec::new, |messages| {
                self.published
                    .read(messages)
                    .map(|event| event.position)
                    .collect()
            }),
            removed: world.removed.as_ref().map_or_else(Vec::new, |messages| {
                self.removed
                    .read(messages)
                    .map(|event| event.position)
                    .collect()
            }),
            columns: world
                .columns
                .as_ref()
                .map_or_else(HashSet::new, |messages| {
                    self.columns
                        .read(messages)
                        .map(|event| event.coordinate)
                        .collect()
                }),
        }
    }
}

impl PropQueue {
    fn prioritize(&mut self, focus: [f64; 3]) {
        self.positions.sort_unstable_by_key(|(p, _, _, _)| {
            std::cmp::Reverse((distance_squared(*p, focus) as u64, p.y, p.z, p.x))
        });
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<PropRenderStatus>()
        .init_resource::<PropGrowthStatus>()
        .init_resource::<PropTemplates>()
        .init_resource::<PropBatches>()
        .init_resource::<PropJobs>()
        .init_resource::<PropSnowExposure>()
        .add_systems(
            Update,
            (refresh_snow_exposure, update_visibility, upload, dispatch)
                .chain()
                .after(StreamingSet::Receive),
        );
}

#[derive(SystemParam)]
struct PropWorld<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    published: Option<Res<'w, Messages<SectionPublished>>>,
    removed: Option<Res<'w, Messages<SectionRemoved>>>,
    columns: Option<Res<'w, Messages<PreparedColumnChanged>>>,
    journal: Res<'w, EditJournal>,
    config: Res<'w, GameConfig>,
    focus: Res<'w, ChunkFocus>,
    terrain: Res<'w, WorldTerrain>,
    time: Option<Res<'w, Time>>,
    entering: Query<'w, 's, (), With<crate::transitions::ChunkFade>>,
    sections: Query<'w, 's, &'static PropSection>,
    queued: Query<'w, 's, (), With<crate::transitions::QueuedChunk>>,
    entering_removed: RemovedComponents<'w, 's, crate::transitions::EnteringChunk>,
}

#[derive(SystemParam)]
struct PropPalette<'w> {
    standard: Res<'w, Assets<StandardMaterial>>,
    wind: ResMut<'w, Assets<PropMaterial>>,
}

fn refresh_snow_exposure(
    world: PropWorld,
    mut state: ResMut<PropSnowExposure>,
    mut changes: Local<PropChangeReader>,
) {
    let changes = changes.read(&world);
    state.stream_changed.clear();
    if world.terrain.is_changed() {
        *state = PropSnowExposure::default();
    } else if world.chunks.is_changed() {
        state.retain_loaded(&world.chunks);
    }
    if world.journal.is_changed() || world.terrain.is_changed() {
        state.refresh(&world.chunks, &world.terrain.0, &world.journal);
    }
    state.record_stream_changes(changes.published.into_iter().chain(changes.removed));
    state.record_prepared_changes(changes.columns.into_iter());
    let mut columns: Vec<_> = state.stream_pending.iter().copied().collect();
    columns.sort_unstable_by_key(|column| {
        let x = column[0] as f64 * 16.0 + 8.0 - world.focus.0[0];
        let z = column[1] as f64 * 16.0 + 8.0 - world.focus.0[2];
        (x * x + z * z) as u64
    });
    for column in columns.into_iter().take(2) {
        let snapshot = crate::weather::surfaces::exposure::Exposure::capture(
            &world.chunks,
            column,
            world.terrain.0.config().sea_level,
            world.terrain.0.config().bounded_world,
            &world.terrain.0,
            &world.journal,
        );
        state.resolve_stream_snapshot(column, snapshot);
    }
}

fn update_visibility(
    config: Res<GameConfig>,
    mut ranges: Query<&mut VisibilityRange, With<PropMesh>>,
) {
    if !config.is_changed() {
        return;
    }
    let end = f32::from(config.client.view_distance) * 16.0;
    for mut range in &mut ranges {
        if range.end_margin != (end..end) {
            *range = crate::transitions::abrupt_prop_range(end);
        }
    }
}

fn template_wind(kind: PropKind, variant: u8, positions: &[[f32; 3]]) -> Vec<[f32; 2]> {
    positions
        .iter()
        .map(|p| wind::parameters(kind, variant, p[1], 1.0))
        .collect()
}

fn make_templates(
    palette: &PropMaterials,
    handles: &[Handle<StandardMaterial>],
    materials: &Assets<StandardMaterial>,
) -> Result<Templates, String> {
    let mut templates = HashMap::new();
    for kind in PropKind::ALL {
        for variant in (0..8).chain((128..136).filter(|_| !kind.aquatic())) {
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let mut combined = MeshBatch::default();
                let mut biome_tinted = Vec::new();
                for part in shapes::template(kind, variant, level, palette)? {
                    let color = handles
                        .get(part.material)
                        .and_then(|handle| materials.get(handle))
                        .ok_or("Missing prop palette material")?
                        .base_color
                        .to_linear();
                    let color = [color.red, color.green, color.blue, color.alpha];
                    let base = u32::try_from(combined.positions.len())
                        .map_err(|_| "Prop template overflow")?;
                    combined
                        .colors
                        .extend((0..part.positions.len()).map(|index| {
                            let local = part.colors.get(index).copied().unwrap_or([1.0; 4]);
                            std::array::from_fn(|channel| color[channel] * local[channel])
                        }));
                    biome_tinted.extend(std::iter::repeat_n(
                        kind != PropKind::Mushroom
                            && (palette.foliage.contains(&part.material)
                                || part.material == palette.stem),
                        part.positions.len(),
                    ));
                    combined.positions.extend(part.positions);
                    combined.normals.extend(part.normals);
                    combined.uvs.extend(part.uvs);
                    for index in part.indices {
                        combined.indices.push(
                            base.checked_add(index)
                                .ok_or("Prop template index overflow")?,
                        );
                    }
                }
                // The prop shader consumes rooted wind weights, not template
                // texture UVs. Compile the scale-independent part once.
                combined.uvs = template_wind(kind, variant, &combined.positions);
                templates.insert(
                    (kind as u8, variant, level),
                    PropTemplate {
                        geometry: combined,
                        biome_tinted,
                    },
                );
            }
        }
    }
    Ok(templates)
}

fn dispatch(
    mut world: PropWorld,
    assets: Res<BlockAssets>,
    mut materials: PropPalette,
    mut templates: ResMut<PropTemplates>,
    mut rendered: ResMut<PropBatches>,
    (mut jobs, mut snow_exposure): (ResMut<PropJobs>, Option<ResMut<PropSnowExposure>>),
    (mut status, mut queue): (ResMut<PropRenderStatus>, Local<PropQueue>),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("props.dispatch");
    let changes = queue.changes.read(&world);
    queue.changed.extend(jobs.1.drain());
    let untracked_change = world.chunks.is_changed() && !changes.any();
    let journal_selection_changed = world.journal.is_changed()
        && snow_exposure
            .as_deref()
            .is_none_or(|state| state.non_fluid_changed);
    if world.journal.is_changed()
        && let Some(state) = snow_exposure.as_deref()
    {
        for column in &state.water_changed {
            if let Some(sections) = state.sections.get(column) {
                queue.changed.extend(sections.iter().copied());
            }
        }
    }
    if let Some(state) = snow_exposure.as_deref() {
        for column in &state.stream_changed {
            if let Some(sections) = state.sections.get(column) {
                queue.changed.extend(sections.iter().copied());
            }
        }
    }
    queue.changed.extend(changes.published);
    if !changes.removed.is_empty() {
        let removed: HashSet<_> = changes
            .removed
            .into_iter()
            .filter(|position| !world.chunks.loaded.contains_key(position))
            .collect();
        for position in &removed {
            jobs.0.remove(position);
            queue.changed.remove(position);
        }
        queue
            .positions
            .retain(|(position, ..)| !removed.contains(position));
    }
    let finished: Vec<_> = world
        .entering_removed
        .read()
        .filter_map(|entity| {
            if world.entering.contains(entity) {
                return None;
            }
            let section = world.sections.get(entity).ok()?;
            rendered
                .0
                .get(&section.0)
                .filter(|previous| previous.entity == Some(entity))
                .map(|_| section.0)
        })
        .collect();
    let animation_finished = !finished.is_empty();
    if status.ready
        && jobs.0.is_empty()
        && jobs.1.is_empty()
        && status.last_focus.is_some_and(|previous| {
            crate::meshing::focus_bucket(previous) == crate::meshing::focus_bucket(world.focus.0)
        })
        && !world.chunks.is_changed()
        && !world.journal.is_changed()
        && !world.config.is_changed()
        && !world.terrain.is_changed()
        && !assets.is_changed()
        && !animation_finished
        && queue.changed.is_empty()
        && snow_exposure
            .as_deref()
            .is_none_or(|state| state.stream_pending.is_empty())
    {
        return;
    }
    queue.selection_dirty |= status.last_focus.is_none_or(|previous| {
        crate::meshing::focus_bucket(previous) != crate::meshing::focus_bucket(world.focus.0)
    }) || journal_selection_changed
        || world.config.is_changed()
        || world.terrain.is_changed()
        || assets.is_changed();
    queue.dirty |= queue.selection_dirty || untracked_change;
    status.last_focus = Some(world.focus.0);
    // Focus changes can skip an entering root. Only that root needs another
    // selection pass when its fade finishes.
    for &position in &finished {
        let Some(section) = world.chunks.loaded.get(&position) else {
            continue;
        };
        if jobs.0.contains_key(&position)
            || section.mesh_epoch != world.journal.mesh_epoch(position)
            || queue.positions.iter().any(|(p, ..)| *p == position)
        {
            continue;
        }
        let previous = rendered.0.get(&position);
        let level = select_prop_level(
            position,
            world.focus.0,
            previous.map(|previous| previous.level),
            &world.config.client,
        );
        let prop_epoch = world.journal.prop_epoch(position);
        let appearance_epoch =
            prop_appearance_epoch(snow_exposure.as_deref(), position, &world.journal);
        if previous.is_some_and(|previous| {
            previous.terrain_epoch == section.mesh_epoch
                && previous.prop_epoch == prop_epoch
                && previous.appearance_epoch == appearance_epoch
                && previous.level == level
                && previous.density == 1.0
        }) {
            continue;
        }
        queue
            .positions
            .push((position, level, prop_epoch, appearance_epoch));
    }
    if animation_finished && !queue.dirty {
        queue.prioritize(world.focus.0);
    }
    if status.error.is_some() {
        return;
    }
    let Some(blocks) = &assets.templates else {
        return;
    };
    if templates.geometry.is_none() {
        match make_templates(
            &blocks.prop_materials,
            &assets.materials,
            &materials.standard,
        ) {
            Ok(cache) => {
                templates.geometry = Some(Arc::new(cache));
                let material = materials.wind.add(PropMaterial {
                    base: StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: 0.92,
                        ..default()
                    },
                    ..default()
                });
                templates.material = Some(material);
            }
            Err(error) => {
                status.error = Some(error);
                return;
            }
        }
    }
    let Some(templates) = &templates.geometry else {
        return;
    };
    if untracked_change {
        jobs.0.retain(|p, _| world.chunks.loaded.contains_key(p));
    }
    let available = world
        .config
        .client
        .max_mesh_jobs
        .min(4)
        .saturating_sub(jobs.0.len());
    if available == 0 {
        status.ready = false;
        status.pending_jobs = jobs.0.len();
        return;
    }
    let candidates: Vec<_> = if queue.dirty {
        let pending: HashSet<_> = if queue.selection_dirty {
            HashSet::new()
        } else {
            queue
                .positions
                .iter()
                .map(|(p, ..)| *p)
                .chain(queue.changed.iter().copied())
                .collect()
        };
        queue.positions.clear();
        let candidates = world
            .chunks
            .loaded
            .iter()
            .filter_map(|(&position, section)| {
                let previous = rendered.0.get(&position);
                if !queue.selection_dirty
                    && !pending.contains(&position)
                    && previous.is_some_and(|p| p.terrain_epoch == section.mesh_epoch)
                {
                    return None;
                }
                Some(position)
            })
            .collect();
        queue.changed.clear();
        queue.dirty = false;
        queue.selection_dirty = false;
        candidates
    } else {
        let changed = std::mem::take(&mut queue.changed);
        queue
            .positions
            .retain(|(position, ..)| !changed.contains(position));
        changed.into_iter().collect()
    };
    if !candidates.is_empty() {
        for position in candidates {
            let Some(section) = world.chunks.loaded.get(&position) else {
                continue;
            };
            if let Some(state) = snow_exposure.as_deref_mut() {
                state.watch(position, &world.chunks, &world.terrain.0, &world.journal);
            }
            let previous = rendered.0.get(&position);
            let level = select_prop_level(
                position,
                world.focus.0,
                previous.map(|p| p.level),
                &world.config.client,
            );
            let prop_epoch = world.journal.prop_epoch(position);
            let appearance_epoch =
                prop_appearance_epoch(snow_exposure.as_deref(), position, &world.journal);
            if previous.is_some_and(|p| {
                p.entity
                    .is_some_and(|entity| world.entering.contains(entity))
                    && p.terrain_epoch == section.mesh_epoch
                    && p.prop_epoch == prop_epoch
                    && p.appearance_epoch == appearance_epoch
            }) {
                continue;
            }
            if previous.is_some_and(|p| {
                p.terrain_epoch == section.mesh_epoch
                    && p.prop_epoch == prop_epoch
                    && p.appearance_epoch == appearance_epoch
                    && p.level == level
                    && p.density == 1.0
            }) {
                continue;
            }
            if section.props.iter().all(|p| p.removed)
                && previous.is_none_or(|p| p.entity.is_none())
            {
                rendered.0.insert(
                    position,
                    RenderedProps {
                        density: 1.0,
                        entity: None,
                        meshes: Vec::new(),
                        terrain_epoch: section.mesh_epoch,
                        prop_epoch,
                        appearance_epoch,
                        level,
                        count: 0,
                        growth: HashMap::new(),
                    },
                );
                continue;
            }
            if jobs.0.contains_key(&position)
                || section.mesh_epoch != world.journal.mesh_epoch(position)
            {
                continue;
            }
            queue
                .positions
                .push((position, level, prop_epoch, appearance_epoch));
        }
        queue.prioritize(world.focus.0);
    }
    // Finishing a worker frees capacity, not a reason to scan every loaded
    // section again. Invalidated worker results explicitly request another scan.
    for _ in 0..available {
        let Some((position, level, prop_epoch, _appearance_epoch)) = queue.positions.pop() else {
            break;
        };
        let Some(section) = world.chunks.loaded.get(&position) else {
            continue;
        };
        if snow_exposure
            .as_deref()
            .is_some_and(|state| state.stream_pending.contains(&[position.x, position.z]))
        {
            queue.changed.insert(position);
            continue;
        }
        let density = 1.0;
        let props: Vec<_> = section
            .props
            .iter()
            .filter(|p| terra_world::props::visible_at_density(p, density))
            .copied()
            .collect();
        let templates = templates.clone();
        let growth = growth::snapshot(
            &props,
            rendered.0.get(&position).map(|previous| &previous.growth),
            world.time.as_deref().map_or(0.0, Time::elapsed_secs),
        );
        let task_growth = growth.clone();
        let terrain = world.terrain.0.clone();
        let conditions = world.journal.surface_snapshot(position);
        let exposure = crate::weather::surfaces::exposure::Exposure::capture(
            &world.chunks,
            [position.x, position.z],
            terrain.config().sea_level,
            terrain.config().bounded_world,
            &terrain,
            &world.journal,
        );
        if let Some(state) = snow_exposure.as_deref_mut()
            && let Some(exposure) = exposure.as_deref()
        {
            state.capture(position, exposure, &world.chunks, &terrain, &world.journal);
        }
        let appearance_epoch =
            prop_appearance_epoch(snow_exposure.as_deref(), position, &world.journal);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let start = std::time::Instant::now();
            let result = compose_with_exposure_mode(
                &props,
                position,
                level,
                &templates,
                &terrain,
                &conditions,
                &task_growth,
                exposure.as_deref(),
                level == DetailLevel::Far,
            )
            .map(|(mut batches, count)| {
                for batch in &mut batches {
                    batch.compact_indices();
                }
                (batches, count)
            });
            (result, start.elapsed().as_secs_f64())
        });
        jobs.0.insert(
            position,
            PropJob {
                ready: None,
                density,
                terrain_epoch: section.mesh_epoch,
                prop_epoch,
                appearance_epoch,
                level,
                task,
                growth,
            },
        );
    }
    status.ready = queue.positions.is_empty()
        && queue.changed.is_empty()
        && !queue.dirty
        && jobs.0.is_empty()
        && jobs.1.is_empty()
        && snow_exposure
            .as_deref()
            .is_none_or(|state| state.stream_pending.is_empty());
    status.pending_jobs = jobs.0.len();
}

#[cfg(test)]
fn compose(
    props: &[WorldProp],
    section: SectionPos,
    level: DetailLevel,
    templates: &Templates,
    terrain: &TerraGenerator,
    conditions: &SurfaceSnapshot,
) -> Result<(Vec<MeshBatch>, usize), String> {
    compose_with_growth(
        props,
        section,
        level,
        templates,
        terrain,
        conditions,
        &HashMap::new(),
    )
}

#[cfg(test)]
fn compose_with_growth(
    props: &[WorldProp],
    section: SectionPos,
    level: DetailLevel,
    templates: &Templates,
    terrain: &TerraGenerator,
    conditions: &SurfaceSnapshot,
    growth: &HashMap<terra_world::props::PropId, Pose>,
) -> Result<(Vec<MeshBatch>, usize), String> {
    compose_with_exposure(
        props, section, level, templates, terrain, conditions, growth, None,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn compose_with_exposure(
    props: &[WorldProp],
    section: SectionPos,
    level: DetailLevel,
    templates: &Templates,
    terrain: &TerraGenerator,
    conditions: &SurfaceSnapshot,
    growth: &HashMap<terra_world::props::PropId, Pose>,
    exposure: Option<&crate::weather::surfaces::exposure::Exposure>,
) -> Result<(Vec<MeshBatch>, usize), String> {
    compose_with_exposure_mode(
        props, section, level, templates, terrain, conditions, growth, exposure, false,
    )
}

#[allow(clippy::too_many_arguments)]
fn compose_with_exposure_mode(
    props: &[WorldProp],
    section: SectionPos,
    level: DetailLevel,
    templates: &Templates,
    terrain: &TerraGenerator,
    conditions: &SurfaceSnapshot,
    growth: &HashMap<terra_world::props::PropId, Pose>,
    exposure: Option<&crate::weather::surfaces::exposure::Exposure>,
    packed: bool,
) -> Result<(Vec<MeshBatch>, usize), String> {
    let origin = section.origin().map_err(|e| e.to_string())?;
    let tint = GrassTint::new(terrain, origin);
    let snow_climate =
        exposure.map(|_| crate::weather::storage::snow::SnowClimate::new(terrain, origin));
    let wind_strength = match level {
        DetailLevel::Near => 1.0,
        DetailLevel::Middle => 0.5,
        DetailLevel::Far => 0.0,
    };
    let mut batch = MeshBatch::default();
    let mut growing_batch = MeshBatch::default();
    let mut sizes = [[0; 3]; 2];
    for prop in props.iter().filter(|p| !p.removed && !p.hidden_growth()) {
        let source = &templates
            .get(&(prop.kind as u8, prop.template_variant(), level))
            .ok_or("Missing prop template")?
            .geometry;
        if !source.indices.is_empty() {
            let size = &mut sizes[usize::from(prop.growing_grass())];
            size[0] += source.positions.len();
            size[1] += source.indices.len();
            size[2] += 1;
        }
    }
    for (index, batch) in [&mut batch, &mut growing_batch].into_iter().enumerate() {
        let [vertices, indices, instances] = sizes[index];
        batch.positions.reserve_exact(vertices);
        batch.uvs.reserve_exact(vertices);
        batch.indices.reserve_exact(indices);
        if packed {
            batch.packed_normals.reserve_exact(vertices);
            batch.packed_colors.reserve_exact(vertices);
        } else {
            batch.normals.reserve_exact(vertices);
            batch.colors.reserve_exact(vertices);
        }
        if index == 1 {
            batch.rooted_wind.reserve_exact(vertices);
            batch.growth_anchors.reserve_exact(vertices);
            batch.growth_ranges.reserve_exact(instances);
        }
    }
    let mut count = 0;
    for prop in props.iter().filter(|p| !p.removed && !p.hidden_growth()) {
        let variant = prop.template_variant();
        let batch = if prop.growing_grass() {
            &mut growing_batch
        } else {
            &mut batch
        };
        let template = templates
            .get(&(prop.kind as u8, variant, level))
            .ok_or("Missing prop template")?;
        let source = &template.geometry;
        if source.indices.is_empty() {
            continue;
        }
        count += 1;
        let scale = prop.visual_scale();
        let rotation = Quat::from_rotation_y(prop.rotation);
        let offset = Vec3::from_array(std::array::from_fn(|i| {
            (prop.position[i] - origin[i] as f64) as f32
        }));
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Prop batch overflow")?;
        if prop.growing_grass() {
            batch.growth_ranges.push((
                prop.id,
                batch.positions.len()..batch.positions.len() + source.positions.len(),
            ));
            let pose = growth.get(&prop.id).copied().unwrap_or(Pose {
                scale: prop.visual_scale(),
                ratio: 1.0,
                start: 0.0,
                pending: false,
            });
            batch.growth_anchors.extend(std::iter::repeat_n(
                offset.to_array(),
                source.positions.len(),
            ));
            batch.rooted_wind.extend(std::iter::repeat_n(
                [pose.start, pose.ratio],
                source.positions.len(),
            ));
        }
        batch.positions.extend(
            source
                .positions
                .iter()
                .map(|&p| (rotation * Vec3::from_array(p) * scale + offset).to_array()),
        );
        let normals = source
            .normals
            .iter()
            .map(|&n| (rotation * Vec3::from_array(n)).to_array());
        if packed {
            batch
                .packed_normals
                .extend(normals.map(crate::foliage::packing::normal));
        } else {
            batch.normals.extend(normals);
        }
        batch
            .uvs
            .extend(source.uvs.iter().map(|&[height, flexibility]| {
                // Preserve the plant/underwater tag even when far LOD disables wind.
                [height * wind_strength, flexibility * scale]
            }));
        let biome = tint.relative_at(offset.to_array());
        // Ground props belong to their support cell, including plants above a section ceiling.
        let support = terra_world::props::placement::support(prop);
        let (owner, local) = global_to_section(support);
        let condition = if owner == section {
            conditions.get(&local).copied().unwrap_or_default()
        } else {
            Default::default()
        };
        let condition = condition_tint(condition);
        let snow_tag = exposure
            .zip(snow_climate.as_ref())
            .map(|(exposure, climate)| {
                let cell = [
                    support[0].rem_euclid(16) as u32,
                    0,
                    support[2].rem_euclid(16) as u32,
                ];
                if exposure.snow_exposed(cell, support[1]) {
                    crate::weather::storage::snow::encode_temperature(
                        climate.temperature(offset.to_array()),
                    )
                } else {
                    0.0
                }
            });
        let colors = source
            .colors
            .iter()
            .zip(&template.biome_tinted)
            .map(|(color, &green)| {
                let mut color = *color;
                for channel in 0..3 {
                    color[channel] *= condition[channel] * if green { biome[channel] } else { 1.0 };
                }
                if let Some(tag) = snow_tag {
                    color[3] = tag;
                }
                color
            });
        if packed {
            batch
                .packed_colors
                .extend(colors.map(crate::foliage::packing::color));
        } else {
            batch.colors.extend(colors);
        }
        for &index in &source.indices {
            batch
                .indices
                .push(base.checked_add(index).ok_or("Prop batch index overflow")?);
        }
    }
    Ok((
        [batch, growing_batch]
            .into_iter()
            .filter(|batch| !batch.indices.is_empty())
            .collect(),
        count,
    ))
}

fn patch_growth(batches: &mut [MeshBatch], growth: &HashMap<terra_world::props::PropId, Pose>) {
    for batch in batches {
        for (id, range) in &batch.growth_ranges {
            if let Some(pose) = growth.get(id) {
                batch.rooted_wind[range.clone()].fill([pose.start, pose.ratio]);
            }
        }
    }
}

#[derive(SystemParam)]
struct PropUpload<'w> {
    templates: Res<'w, PropTemplates>,
    rendered: ResMut<'w, PropBatches>,
    jobs: ResMut<'w, PropJobs>,
    meshes: ResMut<'w, Assets<Mesh>>,
    status: ResMut<'w, PropRenderStatus>,
    budget: Option<ResMut<'w, crate::MeshUploadStatus>>,
    growth_status: ResMut<'w, PropGrowthStatus>,
    snow_exposure: Option<Res<'w, PropSnowExposure>>,
}

fn upload(
    mut commands: Commands,
    world: PropWorld,
    upload: PropUpload,
    mut change_reader: Local<PropChangeReader>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("props.upload");
    let changes = change_reader.read(&world);
    let untracked_change = world.chunks.is_changed() && !changes.any();
    let PropUpload {
        templates,
        mut rendered,
        mut jobs,
        mut meshes,
        mut status,
        mut budget,
        mut growth_status,
        snow_exposure,
    } = upload;
    if jobs.0.is_empty() && !world.chunks.is_changed() && !rendered.is_changed() && !changes.any() {
        return;
    }
    let removed: Vec<_> = if untracked_change {
        rendered
            .0
            .keys()
            .copied()
            .filter(|position| !world.chunks.loaded.contains_key(position))
            .collect()
    } else {
        changes
            .removed
            .into_iter()
            .filter(|position| !world.chunks.loaded.contains_key(position))
            .collect()
    };
    if !removed.is_empty() {
        for position in removed {
            jobs.0.remove(&position);
            if let Some(mut old) = rendered.0.remove(&position)
                && let Some(entity) = old.entity.take()
            {
                crate::transitions::retire(
                    &mut commands,
                    entity,
                    std::mem::take(&mut old.meshes),
                    world.config.client.chunk_animation,
                );
            }
        }
    }
    let Some(material) = &templates.material else {
        return;
    };
    let mut positions: Vec<_> = jobs.0.keys().copied().collect();
    positions.sort_by_key(|p| (distance_squared(*p, world.focus.0) as u64, p.y, p.z, p.x));
    let mut uploaded = 0;
    for position in positions {
        if uploaded >= world.config.client.mesh_uploads_per_frame.min(4) {
            break;
        }
        if snow_exposure
            .as_deref()
            .is_some_and(|state| state.stream_pending.contains(&[position.x, position.z]))
        {
            continue;
        }
        let Some(job) = jobs.0.get_mut(&position) else {
            continue;
        };
        let Some((result, seconds)) = job
            .ready
            .take()
            .or_else(|| future::block_on(future::poll_once(&mut job.task)))
        else {
            continue;
        };
        let vertices = result.as_ref().map_or(0, |(batches, _)| {
            batches.iter().map(|b| b.positions.len()).sum()
        });
        if budget
            .as_mut()
            .is_some_and(|b| !b.reserve(crate::upload_budget::UploadKind::Props, vertices))
        {
            job.ready = Some((result, seconds));
            break;
        }
        status.composition_seconds += seconds;
        status.completed_jobs += 1;
        let Some(mut job) = jobs.0.remove(&position) else {
            continue;
        };
        uploaded += 1;
        let level = select_prop_level(
            position,
            world.focus.0,
            rendered.0.get(&position).map(|p| p.level),
            &world.config.client,
        );
        if level != job.level
            || job.density != 1.0
            || job.prop_epoch != world.journal.prop_epoch(position)
            || job.appearance_epoch
                != prop_appearance_epoch(snow_exposure.as_deref(), position, &world.journal)
            || job.terrain_epoch != world.journal.mesh_epoch(position)
            || world
                .chunks
                .loaded
                .get(&position)
                .is_none_or(|s| s.mesh_epoch != job.terrain_epoch)
        {
            jobs.1.insert(position);
            continue;
        }
        let (mut batches, count) = match result {
            Ok(result) => result,
            Err(error) => {
                status.error = Some(error);
                continue;
            }
        };
        batches.retain(|batch| !batch.positions.is_empty() && batch.has_indices());
        if rendered.0.get(&position).is_some_and(|old| {
            old.entity
                .is_some_and(|entity| world.entering.contains(entity))
                && old.terrain_epoch == job.terrain_epoch
                && old.prop_epoch == job.prop_epoch
                && old.appearance_epoch == job.appearance_epoch
        }) {
            continue;
        }
        let first_upload = rendered.0.get(&position).is_none_or(|old| {
            old.entity.is_none()
                || (old
                    .entity
                    .is_some_and(|entity| world.queued.contains(entity))
                    && old.terrain_epoch == job.terrain_epoch
                    && old.prop_epoch == job.prop_epoch
                    && old.appearance_epoch == job.appearance_epoch)
        });
        let now = world.time.as_deref().map_or(0.0, Time::elapsed_secs);
        let previous_growth = rendered.0.get(&position).map(|old| &old.growth);
        growth_status
            .active
            .retain(|_, pose| pose.start + growth::SECONDS > now);
        for (&id, pose) in job.growth.iter_mut().filter(|(_, pose)| pose.pending) {
            pose.begin(
                previous_growth
                    .and_then(|previous| previous.get(&id))
                    .copied(),
                now,
            );
            growth_status.transitions = growth_status.transitions.saturating_add(1);
            growth_status.active_until = growth_status.active_until.max(now + growth::SECONDS);
            growth_status.active.insert(id, *pose);
        }
        patch_growth(&mut batches, &job.growth);
        if let Some(mut old) = rendered.0.remove(&position) {
            if old.entity.is_some()
                && old.terrain_epoch == job.terrain_epoch
                && old.prop_epoch == job.prop_epoch
                && old.appearance_epoch == job.appearance_epoch
                && old.level != level
            {
                crate::transitions::record_swap(&mut commands);
            }
            remove(&mut old, &mut commands, &mut meshes);
        }
        let mut result = RenderedProps {
            density: job.density,
            entity: None,
            meshes: Vec::new(),
            terrain_epoch: job.terrain_epoch,
            prop_epoch: job.prop_epoch,
            appearance_epoch: job.appearance_epoch,
            level,
            count: if batches.is_empty() { 0 } else { count },
            growth: HashMap::new(),
        };
        result.growth = job.growth;
        if !batches.is_empty() {
            let Ok(origin) = position.origin() else {
                continue;
            };
            let root = commands
                .spawn((
                    Name::new("Section vegetation"),
                    PropSection(position),
                    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                    Visibility::default(),
                ))
                .id();
            result.entity = Some(root);
            for mut batch in batches {
                let indices = batch.take_indices();
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::RENDER_WORLD,
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
                .with_inserted_indices(indices);
                if batch.packed_normals.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals);
                } else {
                    mesh.insert_attribute(
                        bevy::mesh::MeshVertexAttribute {
                            format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                            ..Mesh::ATTRIBUTE_NORMAL
                        },
                        bevy::mesh::VertexAttributeValues::Snorm8x4(batch.packed_normals),
                    );
                }
                if batch.packed_colors.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors);
                } else {
                    mesh.insert_attribute(
                        bevy::mesh::MeshVertexAttribute {
                            format: bevy::render::render_resource::VertexFormat::Unorm8x4,
                            ..Mesh::ATTRIBUTE_COLOR
                        },
                        bevy::mesh::VertexAttributeValues::Unorm8x4(batch.packed_colors),
                    );
                }
                if !batch.growth_anchors.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, batch.rooted_wind);
                    mesh.insert_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR, batch.growth_anchors);
                }
                let bounds = wind::padded_bounds(&mesh);
                let handle = meshes.add(mesh);
                let mut entity = commands.spawn((
                    PropMesh,
                    WindProp,
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Prop,
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    ChildOf(root),
                    crate::transitions::abrupt_prop_range(
                        f32::from(world.config.client.view_distance) * 16.0,
                    ),
                ));
                if let Some(bounds) = bounds {
                    entity.insert(bounds);
                }
                entity.insert(bevy::light::NotShadowCaster);
                result.meshes.push(handle);
            }
            if first_upload {
                crate::transitions::enter(&mut commands, root, world.config.client.chunk_animation);
            }
        }
        rendered.0.insert(position, result);
    }
    if rendered.is_changed() {
        status.visible_props = rendered.0.values().map(|r| r.count).sum();
        status.batches = rendered.0.values().map(|r| r.meshes.len()).sum();
    }
}

fn remove(old: &mut RenderedProps, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
    if let Some(entity) = old.entity.take() {
        commands.entity(entity).despawn();
    }
    for handle in old.meshes.drain(..) {
        meshes.remove(handle.id());
    }
}

#[cfg(test)]
mod growth_tests {
    use super::*;

    fn snow_snapshot(roof: i16) -> Arc<crate::weather::surfaces::exposure::Exposure> {
        Arc::new(crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [roof; 256],
            snow_roofs: [roof; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        })
    }

    #[test]
    fn prepared_column_arrival_refreshes_watched_props_without_section_publication() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let owner = SectionPos::default();
        let mut state = PropSnowExposure::default();
        state.watch(owner, &chunks, &terrain, &journal);
        assert!(state.stream_snapshots.is_empty());
        let before = state.appearance_epoch(owner, &journal);
        state.record_prepared_changes([[0, 0], [0, 0], [9, 9]].into_iter());
        assert_eq!(state.stream_pending, HashSet::from([[0, 0]]));
        state.resolve_stream_snapshot([0, 0], Some(snow_snapshot(0)));
        assert!(state.appearance_epoch(owner, &journal) > before);
        let available = state.appearance_epoch(owner, &journal);
        let snapshot = state.stream_snapshots[&[0, 0]].clone();
        state.record_prepared_changes([[0, 0]].into_iter());
        state.resolve_stream_snapshot([0, 0], Some(snow_snapshot(0)));
        assert_eq!(state.appearance_epoch(owner, &journal), available);
        assert!(Arc::ptr_eq(&snapshot, &state.stream_snapshots[&[0, 0]]));
        state.capture(owner, &snapshot, &chunks, &terrain, &journal);
        journal
            .set_fluid(
                [3, 1, 3],
                terra_world::water::FluidCell {
                    amount: 4,
                    source: false,
                    flow: [0; 2],
                },
            )
            .unwrap();
        state.refresh(&chunks, &terrain, &journal);
        let water_epoch = state.water[&[0, 0]].epoch;
        assert!(water_epoch > 0);
        let submerged = state.appearance_epoch(owner, &journal);
        state.record_prepared_changes([[0, 0]].into_iter());
        state.resolve_stream_snapshot([0, 0], None);
        assert_eq!(state.water[&[0, 0]].epoch, water_epoch);
        assert!(state.appearance_epoch(owner, &journal) > submerged);
    }

    #[test]
    fn streamed_roofs_change_only_owner_column_masks_and_survive_reload_without_epoch_reuse() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let owner = SectionPos::default();
        let other = SectionPos::new(2, 0, 0);
        let upper = SectionPos::new(0, 3, 0);
        let mut state = PropSnowExposure::default();
        let ground = snow_snapshot(0);
        state.capture(owner, &ground, &chunks, &terrain, &journal);
        state.capture(other, &ground, &chunks, &terrain, &journal);
        let original = state.stream_snapshots[&[0, 0]].clone();
        let before = state.appearance_epoch(owner, &journal);
        let unrelated = state.appearance_epoch(other, &journal);
        state.record_stream_changes([upper, upper, SectionPos::new(9, 3, 0)].into_iter());
        assert_eq!(state.stream_pending, HashSet::from([[0, 0]]));
        assert_eq!(state.appearance_epoch(owner, &journal), before);
        state.resolve_stream_snapshot([0, 0], Some(snow_snapshot(0)));
        assert!(state.stream_changed.is_empty());
        assert!(Arc::ptr_eq(&original, &state.stream_snapshots[&[0, 0]]));
        state.record_stream_changes([upper, upper].into_iter());
        state.resolve_stream_snapshot([0, 0], Some(snow_snapshot(48)));
        let arrival = state.appearance_epoch(owner, &journal);
        assert_ne!(arrival, before);
        assert_eq!(state.stream_changed, HashSet::from([[0, 0]]));
        assert_eq!(state.appearance_epoch(other, &journal), unrelated);
        state.record_stream_changes([upper].into_iter());
        state.resolve_stream_snapshot([0, 0], Some(ground.clone()));
        let removal = state.appearance_epoch(owner, &journal);
        assert!(removal > arrival);
        assert_eq!(journal.mesh_epoch(owner), 0);
        state.retain_loaded(&chunks);
        assert!(
            state.stream_epochs.is_empty()
                && state.stream_pending.is_empty()
                && state.stream_snapshots.is_empty()
        );
        state.capture(owner, &ground, &chunks, &terrain, &journal);
        state.record_stream_changes([upper].into_iter());
        state.resolve_stream_snapshot([0, 0], Some(snow_snapshot(48)));
        assert!(state.appearance_epoch(owner, &journal) > removal);
        let prop = sprout(90, 0.12, 64);
        let first = growth::snapshot(&[prop], None, 10.0);
        let reloaded = growth::snapshot(&[prop], Some(&first), 30.0);
        assert_eq!(reloaded[&prop.id].scale, first[&prop.id].scale);
        assert_eq!(reloaded[&prop.id].ratio, first[&prop.id].ratio);
        assert_eq!(reloaded[&prop.id].start, first[&prop.id].start);
    }

    #[test]
    fn pending_stream_masks_hold_completed_jobs_then_reject_stale_uploads_locally() {
        let owner = SectionPos::default();
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut state = PropSnowExposure::default();
        state.capture(owner, &snow_snapshot(0), &chunks, &terrain, &journal);
        state.record_stream_changes([SectionPos::new(0, 3, 0)].into_iter());
        let mut app = App::new();
        app.insert_resource(GameConfig::default())
            .insert_resource(WorldTerrain(terrain))
            .insert_resource(ChunkFocus([8.0; 3]))
            .insert_resource(chunks)
            .insert_resource(journal)
            .insert_resource(state)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<PropTemplates>()
            .init_resource::<PropBatches>()
            .init_resource::<PropJobs>()
            .init_resource::<PropRenderStatus>()
            .init_resource::<PropGrowthStatus>()
            .init_resource::<crate::transitions::LodSwapStatus>()
            .add_systems(Update, upload);
        app.world_mut().resource_mut::<PropTemplates>().material = Some(Handle::default());
        let pool = bevy::tasks::TaskPoolBuilder::new().num_threads(1).build();
        app.world_mut().resource_mut::<PropJobs>().0.insert(
            owner,
            PropJob {
                ready: Some((Ok((Vec::new(), 1)), 0.0)),
                density: 1.0,
                terrain_epoch: 0,
                prop_epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                task: pool.spawn(std::future::pending()),
                growth: HashMap::new(),
            },
        );
        app.update();
        assert!(app.world().resource::<PropJobs>().0[&owner].ready.is_some());
        assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 0);
        app.world_mut()
            .resource_mut::<PropSnowExposure>()
            .resolve_stream_snapshot([0, 0], Some(snow_snapshot(48)));
        app.update();
        assert!(!app.world().resource::<PropJobs>().0.contains_key(&owner));
        assert_eq!(app.world().resource::<PropJobs>().1, HashSet::from([owner]));
        assert!(!app.world().resource::<PropBatches>().0.contains_key(&owner));
        assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 0);
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
    }

    #[test]
    fn watched_empty_sections_keep_prop_and_appearance_changes_without_prepared_exposure() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut chunks = ChunkStore::default();
        let position = SectionPos::default();
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
        assert!(chunks.prepared_column([0, 0]).is_none());
        let mut journal = EditJournal::default();
        let mut state = PropSnowExposure::default();
        state.watch(position, &chunks, &terrain, &journal);
        assert!(state.water.is_empty());
        assert_eq!(state.sections[&[0, 0]], HashSet::from([position]));
        journal.place_prop(sprout(9877, 1.0, 0)).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(
            state.non_fluid_changed,
            "A prop placed into a previously empty batch needs selection"
        );
        state.refresh(&chunks, &terrain, &journal);
        assert!(!state.non_fluid_changed);
        let before = state.appearance_epoch(position, &journal);
        journal
            .set_surface_condition(
                [8, 0, 8],
                terra_world::environment::SurfaceCondition::new(0.6, 0.8).unwrap(),
            )
            .unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
        assert_ne!(state.appearance_epoch(position, &journal), before);
        journal
            .remove_prop(position, terra_world::props::PropId(9877))
            .unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
        assert!(state.water.is_empty());
    }

    #[test]
    fn roof_edits_in_another_vertical_section_invalidate_only_the_prop_column() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut state = PropSnowExposure::default();
        let ground = SectionPos::new(0, 0, 0);
        let unrelated = SectionPos::new(2, 0, 0);
        state
            .sections
            .insert([ground.x, ground.z], HashSet::from([ground]));
        state.refresh(&chunks, &terrain, &journal);
        let previous = state.appearance_epoch(ground, &journal);
        let other = state.appearance_epoch(unrelated, &journal);
        let geometry_epoch = journal.mesh_epoch(ground);
        journal.record([3, 35, 3], 3).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
        assert_ne!(state.appearance_epoch(ground, &journal), previous);
        assert_eq!(state.appearance_epoch(unrelated, &journal), other);
        assert_eq!(journal.mesh_epoch(ground), geometry_epoch);
        assert_eq!(journal.appearance_epoch(ground), 0);
        journal.record([3, 35, 3], 0).unwrap();
        let roof_epoch = state.appearance_epoch(ground, &journal);
        state.refresh(&chunks, &terrain, &journal);
        assert_ne!(state.appearance_epoch(ground, &journal), roof_epoch);
    }

    #[test]
    fn fluid_snow_exposure_changes_are_column_local_and_ignore_surface_ripples() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut state = PropSnowExposure::default();
        let position = SectionPos::default();
        state.refresh(&chunks, &terrain, &journal);
        state.capture(
            position,
            &crate::weather::surfaces::exposure::Exposure {
                natural: None,
                roofs: [0; 256],
                snow_roofs: [0; 256],
                coast: [0.0; 256],
                coast_vertices: [0.0; 289],
                water: [f32::NEG_INFINITY; 256],
            },
            &chunks,
            &terrain,
            &journal,
        );
        let before = state.appearance_epoch(position, &journal);
        let geometry_epoch = journal.mesh_epoch(position);
        let mut fluid = terra_world::water::FluidCell {
            amount: 4,
            source: false,
            flow: [0; 2],
        };
        journal.set_fluid([3, 1, 3], fluid).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(
            !state.non_fluid_changed,
            "Fluid-only ticks must not request a global selection"
        );
        assert_eq!(state.water_changed, HashSet::from([[0, 0]]));
        assert_eq!(state.sections[&[0, 0]], HashSet::from([position]));
        let submerged = state.appearance_epoch(position, &journal);
        assert_ne!(
            submerged, before,
            "An in-flight dry job must now fail the upload epoch check"
        );
        assert_eq!(journal.mesh_epoch(position), geometry_epoch);
        fluid.amount = 5;
        fluid.flow = [1, 0];
        journal.set_fluid([3, 1, 3], fluid).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.water_changed.is_empty());
        assert_eq!(state.appearance_epoch(position, &journal), submerged);
        journal.set_fluid([3, 1, 3], Default::default()).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.water_changed, HashSet::from([[0, 0]]));
        assert_ne!(state.appearance_epoch(position, &journal), submerged);
    }

    #[test]
    fn snow_input_cache_ignores_travel_history_and_releases_every_map_on_unload() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let position = SectionPos::default();
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
        for x in 10..1010 {
            journal.record([x * 16 + 3, 35, 3], 3).unwrap();
            journal
                .set_fluid(
                    [x * 16 + 3, 36, 3],
                    terra_world::water::FluidCell {
                        amount: 4,
                        source: false,
                        flow: [0; 2],
                    },
                )
                .unwrap();
        }
        journal.record([3, 35, 3], 3).unwrap();
        let mut state = PropSnowExposure::default();
        state.capture(
            position,
            &crate::weather::surfaces::exposure::Exposure {
                natural: None,
                roofs: [0; 256],
                snow_roofs: [0; 256],
                coast: [0.0; 256],
                coast_vertices: [0.0; 289],
                water: [f32::NEG_INFINITY; 256],
            },
            &chunks,
            &terrain,
            &journal,
        );
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(
            state.sampled_sections, 31,
            "Only the active column's bounded height is sampled"
        );
        assert_eq!(state.revisions.len(), 1);
        assert_eq!(state.fluids.len(), 1);
        assert_eq!(state.non_fluid.len(), 1);
        assert_eq!(
            state.revisions[&[0, 0]],
            1,
            "Roofs in other Y sections still contribute"
        );
        let before = state.appearance_epoch(position, &journal);
        journal.record([20_003, 35, 3], 3).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(!state.non_fluid_changed);
        assert_eq!(state.appearance_epoch(position, &journal), before);
        assert_eq!(state.sampled_sections, 31);
        chunks.loaded.remove(&position);
        state.retain_loaded(&chunks);
        assert!(state.sections.is_empty());
        assert!(state.revisions.is_empty());
        assert!(state.fluids.is_empty());
        assert!(state.non_fluid.is_empty());
        assert!(state.water.is_empty());
        assert!(state.water_changed.is_empty());
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.sampled_sections, 0);
        assert!(
            state.revisions.is_empty() && state.fluids.is_empty() && state.non_fluid.is_empty()
        );
    }

    #[test]
    fn sparse_journal_uses_the_smaller_history_domain_for_loaded_prop_columns() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        journal.record([3, 35, 3], 3).unwrap();
        journal.record([20_003, 35, 3], 3).unwrap();
        let mut state = PropSnowExposure::default();
        for x in 0..100 {
            state
                .sections
                .insert([x, 0], HashSet::from([SectionPos::new(x, 0, 0)]));
        }
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.sampled_sections, 1);
        assert_eq!(state.non_fluid.len(), 1);
        assert_eq!(state.revisions.len(), 100);
        assert_eq!(state.revisions[&[0, 0]], 1);
        assert!(state.water_changed.is_empty());
    }

    fn sprout(id: u64, scale: f32, variant: u8) -> WorldProp {
        WorldProp {
            id: terra_world::props::PropId(id),
            kind: PropKind::Grass,
            position: [8.0, 1.0, 8.0],
            rotation: 0.0,
            scale,
            variant,
            section: SectionPos::default(),
            removed: false,
        }
    }

    fn templates() -> Templates {
        [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far]
            .into_iter()
            .map(|level| {
                (
                    (PropKind::Grass as u8, 0, level),
                    PropTemplate {
                        geometry: MeshBatch {
                            positions: vec![[0.0; 3], [0.2, 1.0, 0.0], [-0.2, 1.0, 0.0]],
                            normals: vec![[0.0, 0.0, 1.0]; 3],
                            uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 1.0]],
                            colors: vec![[1.0; 4]; 3],
                            indices: vec![0, 1, 2],
                            ..default()
                        },
                        biome_tinted: vec![false; 3],
                    },
                )
            })
            .collect()
    }

    #[test]
    fn compiled_wind_weights_match_all_variants_scales_and_keep_underwater_tags() {
        let positions = [-0.2, 0.0, 0.15, 0.5, 1.0, 3.35, 4.0].map(|height| [0.0, height, 0.0]);
        for kind in PropKind::ALL {
            for variant in 0..=u8::MAX {
                let template_variant = WorldProp {
                    kind,
                    variant,
                    ..sprout(1, 1.0, 0)
                }
                .template_variant();
                let weights = template_wind(kind, template_variant, &positions);
                for scale in [0.1, 0.35, 0.95, 1.0, 2.0] {
                    for (&position, &[height, flexibility]) in positions.iter().zip(&weights) {
                        let original = wind::parameters(kind, variant, position[1], scale);
                        assert_eq!(
                            [height, flexibility * scale],
                            original,
                            "{kind:?}/{variant}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn directly_packed_props_keep_exact_geometry_growth_weather_and_vertex_bytes() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let templates = templates();
        let section = SectionPos::new(-1, 0, -1);
        let props = [
            WorldProp {
                position: [-8.0, 1.0, -8.0],
                rotation: 0.73,
                section,
                ..sprout(1, 0.25, 65)
            },
            WorldProp {
                position: [-7.0, 1.0, -8.0],
                rotation: 1.7,
                section,
                ..sprout(2, 1.0, 0)
            },
        ];
        let conditions = HashMap::from([(
            [8, 0, 8],
            terra_world::environment::SurfaceCondition {
                wetness: 0.7,
                snow: 0.2,
                ..default()
            },
        )]);
        let growth = HashMap::from([(
            props[0].id,
            Pose {
                scale: props[0].visual_scale(),
                ratio: 0.42,
                start: 33.0,
                pending: false,
            },
        )]);
        let exposure = crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [0; 256],
            snow_roofs: [0; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let (mut original, original_count) = compose_with_exposure(
                &props,
                section,
                level,
                &templates,
                &terrain,
                &conditions,
                &growth,
                Some(&exposure),
            )
            .unwrap();
            let (packed, count) = compose_with_exposure_mode(
                &props,
                section,
                level,
                &templates,
                &terrain,
                &conditions,
                &growth,
                Some(&exposure),
                true,
            )
            .unwrap();
            assert_eq!(count, original_count);
            for batch in &mut original {
                crate::foliage::packing::pack(batch);
            }
            assert_eq!(packed, original);
            for batch in packed {
                assert_eq!(batch.normals.capacity(), 0);
                assert_eq!(batch.colors.capacity(), 0);
                assert_eq!(batch.positions.capacity(), batch.positions.len());
                assert_eq!(batch.packed_normals.capacity(), batch.positions.len());
                assert_eq!(batch.packed_colors.capacity(), batch.positions.len());
            }
        }
    }

    #[test]
    fn real_vegetation_and_reef_batches_have_exact_capacities_and_half_size_indices() {
        let mut materials = Assets::<StandardMaterial>::default();
        let handle = materials.add(StandardMaterial::default());
        let templates = make_templates(&PropMaterials::default(), &[handle], &materials).unwrap();
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let section = SectionPos::new(-1, 0, -1);
        let mut old_index_bytes = 0;
        let mut new_index_bytes = 0;
        for kind in [
            PropKind::Grass,
            PropKind::Bush,
            PropKind::Mushroom,
            PropKind::BranchingCoral,
            PropKind::TubeCoral,
            PropKind::PlateCoral,
            PropKind::Seagrass,
        ] {
            let props: Vec<_> = (0..16)
                .map(|i| WorldProp {
                    id: terra_world::props::PropId(i),
                    kind,
                    position: [
                        -15.5 + (i % 4) as f64 * 3.0,
                        1.0,
                        -15.5 + (i / 4) as f64 * 3.0,
                    ],
                    rotation: i as f32 * 0.25,
                    scale: 0.8,
                    variant: (i % 8) as u8,
                    section,
                    removed: false,
                })
                .collect();
            for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let (mut batches, count) = compose_with_exposure_mode(
                    &props,
                    section,
                    level,
                    &templates,
                    &terrain,
                    &Default::default(),
                    &HashMap::new(),
                    None,
                    level == DetailLevel::Far,
                )
                .unwrap();
                assert_eq!(count, props.len(), "{kind:?}/{level:?}");
                assert_eq!(
                    batches.len(),
                    1,
                    "never split one material into extra draw calls"
                );
                let batch = &mut batches[0];
                assert_eq!(batch.positions.capacity(), batch.positions.len());
                assert_eq!(batch.uvs.capacity(), batch.positions.len());
                let indices = batch.indices.len();
                old_index_bytes += indices * size_of::<u32>();
                batch.compact_indices();
                assert_eq!(batch.indices.capacity(), 0);
                assert_eq!(batch.compact_indices.len(), indices);
                new_index_bytes += batch.compact_indices.capacity() * size_of::<u16>();
                if level == DetailLevel::Far {
                    assert_eq!(batch.normals.capacity(), 0);
                    assert_eq!(batch.colors.capacity(), 0);
                    assert_eq!(batch.packed_normals.len(), batch.positions.len());
                    assert_eq!(batch.packed_colors.len(), batch.positions.len());
                }
            }
        }
        assert_eq!(new_index_bytes * 2, old_index_bytes);
        eprintln!(
            "O05 seven species, three LODs, 16 props each: index bytes {old_index_bytes} -> {new_index_bytes}; unchanged represented prop count and triangles"
        );
    }

    #[test]
    fn seasonal_grass_keeps_climate_and_shelter_tags_at_every_lod() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let templates = templates();
        let prop = sprout(99, 1.0, 0);
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            for (roof, snow_roof) in [(0, 0), (8, 8), (8, 0)] {
                let exposure = crate::weather::surfaces::exposure::Exposure {
                    natural: None,
                    roofs: [roof; 256],
                    snow_roofs: [snow_roof; 256],
                    coast: [0.0; 256],
                    coast_vertices: [0.0; 289],
                    water: [f32::NEG_INFINITY; 256],
                };
                let (mut batches, _) = compose_with_exposure(
                    &[prop],
                    SectionPos::default(),
                    level,
                    &templates,
                    &terrain,
                    &Default::default(),
                    &HashMap::new(),
                    Some(&exposure),
                )
                .unwrap();
                let batch = &mut batches[0];
                assert!(batch.uvs.iter().all(|uv| uv[1] > 0.0));
                assert!(batch.colors.iter().all(|color| {
                    if snow_roof == 0 {
                        color[3] > 0.0 && color[3] < 1.0
                    } else {
                        color[3] == 0.0
                    }
                }));
                if level == DetailLevel::Far {
                    assert!(batch.uvs.iter().all(|uv| uv[0] == 0.0));
                    crate::foliage::packing::pack(batch);
                    assert!(batch.packed_colors.iter().all(|color| {
                        if snow_roof == 0 {
                            color[3] > 0 && color[3] < 255
                        } else {
                            color[3] == 0
                        }
                    }));
                }
            }
        }
    }

    #[test]
    fn growth_vertex_ranges_patch_each_prop_without_changing_other_geometry_or_lod() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let templates = templates();
        let props = [
            sprout(99, 0.25, 65),
            sprout(100, 0.12, 64),
            sprout(101, 1.0, 0),
        ];
        let poses = HashMap::from([
            (
                props[0].id,
                Pose {
                    scale: props[0].visual_scale(),
                    ratio: 0.48,
                    start: 35.5,
                    pending: false,
                },
            ),
            (
                props[1].id,
                Pose {
                    scale: props[1].visual_scale(),
                    ratio: 1.0,
                    start: 0.0,
                    pending: false,
                },
            ),
        ]);
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let (mut batches, count) = compose_with_growth(
                &props,
                SectionPos::default(),
                level,
                &templates,
                &terrain,
                &Default::default(),
                &HashMap::new(),
            )
            .unwrap();
            assert_eq!(count, 3);
            assert_eq!(batches.len(), 2);
            let positions: Vec<_> = batches
                .iter()
                .map(|batch| batch.positions.clone())
                .collect();
            let uvs: Vec<_> = batches.iter().map(|batch| batch.uvs.clone()).collect();
            if level == DetailLevel::Far {
                for batch in &mut batches {
                    crate::foliage::packing::pack(batch);
                }
            }
            patch_growth(&mut batches, &poses);
            for (index, batch) in batches.iter().enumerate() {
                assert_eq!(batch.positions, positions[index]);
                assert_eq!(batch.uvs, uvs[index]);
                for (id, range) in &batch.growth_ranges {
                    let pose = poses[id];
                    assert_eq!(range.len(), 3);
                    assert!(
                        batch.rooted_wind[range.clone()]
                            .iter()
                            .all(|timing| *timing == [pose.start, pose.ratio])
                    );
                }
            }
            assert!(batches[0].growth_ranges.is_empty());
            assert!(batches[0].rooted_wind.is_empty());
            assert_eq!(
                batches[1].growth_ranges,
                vec![(props[0].id, 0..3), (props[1].id, 3..6)]
            );
        }
    }

    #[test]
    fn fifteen_second_worker_delay_keeps_upload_size_continuous_and_lod_does_not_restart_growth() {
        use bevy::tasks::TaskPoolBuilder;
        use terra_world::streaming::LoadedSection;

        let mut app = App::new();
        let mut config = GameConfig::default();
        config.client.chunk_animation = terra_core::ChunkAnimation::None;
        config.client.prop_lod_near_distance = 16.0;
        config.client.prop_lod_far_distance = 64.0;
        config.client.lod_hysteresis = 0.0;
        let terrain = TerraGenerator::new(config.world.clone()).unwrap();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(35.5));
        app.insert_resource(config)
            .insert_resource(time)
            .insert_resource(WorldTerrain(terrain.clone()))
            .insert_resource(ChunkFocus([8.0; 3]))
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<PropTemplates>()
            .init_resource::<PropBatches>()
            .init_resource::<PropJobs>()
            .init_resource::<PropRenderStatus>()
            .init_resource::<PropGrowthStatus>()
            .init_resource::<crate::transitions::LodSwapStatus>()
            .add_systems(Update, upload);
        app.world_mut().resource_mut::<PropTemplates>().material = Some(Handle::default());
        let position = SectionPos::default();
        let prop = sprout(99, 0.25, 65);
        let old = Pose {
            scale: sprout(99, 0.12, 64).visual_scale(),
            ratio: 0.2,
            start: 20.0,
            pending: false,
        };
        let mut previous = HashMap::from([(prop.id, old)]);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![prop],
            },
        );
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        app.world_mut().resource_mut::<PropBatches>().0.insert(
            position,
            RenderedProps {
                density: 1.0,
                entity: Some(root),
                meshes: Vec::new(),
                terrain_epoch: 0,
                prop_epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                count: 1,
                growth: previous.clone(),
            },
        );
        let templates = templates();
        let pool = TaskPoolBuilder::new().num_threads(1).build();
        let mut uploaded_pose = None;
        for (level, dispatch) in [(DetailLevel::Near, 20.5), (DetailLevel::Middle, 36.0)] {
            let growth = growth::snapshot(&[prop], Some(&previous), dispatch);
            let ready = compose_with_growth(
                &[prop],
                position,
                level,
                &templates,
                &terrain,
                &Default::default(),
                &growth,
            );
            app.world_mut().resource_mut::<PropJobs>().0.insert(
                position,
                PropJob {
                    ready: Some((ready, 15.0)),
                    density: 1.0,
                    terrain_epoch: 0,
                    prop_epoch: 0,
                    appearance_epoch: 0,
                    level,
                    task: pool.spawn(std::future::pending()),
                    growth,
                },
            );
            app.update();
            assert!(app.world().resource::<PropRenderStatus>().error.is_none());
            assert!(app.world().resource::<PropJobs>().0.is_empty());
            let now = app.world().resource::<Time>().elapsed_secs();
            let rendered = &app.world().resource::<PropBatches>().0[&position];
            let pose = rendered.growth[&prop.id];
            assert_eq!(rendered.level, level);
            assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 1);
            if let Some(original) = uploaded_pose {
                assert_eq!(
                    pose, original,
                    "a delayed LOD upload must preserve the growth clock"
                );
            } else {
                assert!((pose.scale * pose.factor(now) - old.scale * old.factor(now)).abs() < 1e-6);
                assert!((pose.ratio - 0.48).abs() < 1e-6);
                assert_eq!(pose.start, now);
                uploaded_pose = Some(pose);
            }
            let mesh = app
                .world()
                .resource::<Assets<Mesh>>()
                .get(&rendered.meshes[0])
                .unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(timing)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_1)
            else {
                panic!("missing growth timing")
            };
            assert!(
                timing
                    .iter()
                    .all(|value| *value == [pose.start, pose.ratio])
            );
            previous = rendered.growth.clone();
            app.world_mut().resource_mut::<ChunkFocus>().0 = [32.0, 8.0, 8.0];
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs_f32(15.5));
        }
    }
}

#[cfg(test)]
pub(super) mod testing {
    use super::*;

    pub fn spawn_retired_root(world: &mut World, position: SectionPos) -> Entity {
        world
            .spawn((PropSection(position), crate::transitions::EnteringChunk))
            .id()
    }

    pub fn changed_tick(world: &World) -> u32 {
        world
            .get_resource_ref::<PropBatches>()
            .unwrap()
            .last_changed()
            .get()
    }

    #[derive(Debug, PartialEq)]
    pub struct Snapshot {
        pub terrain_epoch: u64,
        pub prop_epoch: u64,
        pub appearance_epoch: u64,
        pub meshes: Vec<AssetId<Mesh>>,
        pub count: usize,
    }

    pub fn snapshot(world: &World, section: SectionPos) -> Option<Snapshot> {
        world
            .resource::<PropBatches>()
            .0
            .get(&section)
            .map(|batch| Snapshot {
                terrain_epoch: batch.terrain_epoch,
                prop_epoch: batch.prop_epoch,
                appearance_epoch: batch.appearance_epoch,
                meshes: batch.meshes.iter().map(Handle::id).collect(),
                count: batch.count,
            })
    }

    pub fn pending(world: &World) -> usize {
        world.resource::<PropJobs>().0.len()
    }

    pub fn enqueue(
        world: &mut World,
        section: SectionPos,
        terrain_epoch: u64,
        prop_epoch: u64,
        props: Vec<WorldProp>,
    ) {
        let templates = world.resource::<PropTemplates>().geometry.clone().unwrap();
        let terrain = world.resource::<WorldTerrain>().0.clone();
        let conditions = world.resource::<EditJournal>().surface_snapshot(section);
        let appearance_epoch = world.resource::<EditJournal>().appearance_epoch(section);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            (
                compose(
                    &props,
                    section,
                    DetailLevel::Near,
                    &templates,
                    &terrain,
                    &conditions,
                ),
                0.0,
            )
        });
        world.resource_mut::<PropJobs>().0.insert(
            section,
            PropJob {
                ready: None,
                density: 1.0,
                terrain_epoch,
                prop_epoch,
                appearance_epoch,
                level: DetailLevel::Near,
                task,
                growth: HashMap::new(),
            },
        );
    }

    pub fn enqueue_stale_lod(world: &mut World, section: SectionPos, props: Vec<WorldProp>) {
        enqueue(world, section, 0, 0, props);
        world
            .resource_mut::<PropJobs>()
            .0
            .get_mut(&section)
            .unwrap()
            .level = DetailLevel::Far;
        world
            .resource_mut::<PropBatches>()
            .0
            .get_mut(&section)
            .unwrap()
            .level = DetailLevel::Far;
    }

    pub fn enqueue_empty_result(world: &mut World, section: SectionPos) {
        enqueue(world, section, 0, 0, Vec::new());
        world
            .resource_mut::<PropJobs>()
            .0
            .get_mut(&section)
            .unwrap()
            .ready = Some((
            Ok((
                vec![
                    MeshBatch::default(),
                    MeshBatch {
                        positions: vec![[0.0; 3]; 3],
                        ..default()
                    },
                    MeshBatch {
                        indices: vec![0, 1, 2],
                        ..default()
                    },
                ],
                3,
            )),
            0.0,
        ));
    }
}
