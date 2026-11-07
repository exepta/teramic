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
    collections::{HashMap, HashSet, VecDeque},
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
        StreamingSet, WaterPublished, WorldTerrain,
    },
};

#[path = "batch_plan.rs"]
mod batch_plan;

#[cfg(test)]
#[path = "batch_plan_tests.rs"]
mod batch_plan_tests;

#[cfg(test)]
#[path = "static_layout_tests.rs"]
mod static_layout_tests;

#[cfg(test)]
#[path = "sparse_neighbors_tests.rs"]
mod sparse_neighbors_tests;

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

fn packed_prop_policy(level: DetailLevel, smoke: bool, unpacked: bool) -> bool {
    level == DetailLevel::Far || !smoke || !unpacked
}

fn packed_prop_composition(level: DetailLevel) -> bool {
    // Gameplay uses the natively validated shared packing at every distance.
    // Only smoke comparisons may restore the older near/middle layouts.
    static EXPERIMENT: std::sync::LazyLock<(bool, bool)> = std::sync::LazyLock::new(|| {
        (
            std::env::args().any(|argument| argument.starts_with("--smoke-test")),
            std::env::var("TERRA_PACKED_PROPS").is_ok_and(|value| value == "0"),
        )
    });
    packed_prop_policy(level, EXPERIMENT.0, EXPERIMENT.1)
}

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
    support_pending: bool,
}
impl RenderedProps {
    fn expired_crop_fade(&self, now: f32) -> bool {
        self.growth.values().any(|pose| {
            pose.previous_variant.is_some() && !pose.pending && now >= pose.start + growth::SECONDS
        })
    }
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
    support_pending: bool,
}
#[derive(Resource, Default)]
struct PropJobs(HashMap<SectionPos, PropJob>, HashSet<SectionPos>);

fn release_job(job: &mut PropJob, meshes: &mut Assets<Mesh>) {
    if let Some((result, _)) = &mut job.ready {
        release_result(result, meshes);
    }
}

fn release_result(result: &mut ComposedProps, meshes: &mut Assets<Mesh>) {
    if let Ok((batches, _)) = result {
        crate::upload_budget::release_staged(batches, meshes);
    }
}

fn cancel_job(jobs: &mut PropJobs, position: SectionPos, meshes: &mut Assets<Mesh>) {
    if let Some(mut job) = jobs.0.remove(&position) {
        release_job(&mut job, meshes);
    }
}

#[derive(Resource, Default)]
struct PropAppearanceState {
    sections: HashMap<[i64; 2], HashSet<SectionPos>>,
    /// Only inputs consumed by prop composition enter global selection.
    /// Roof/water exposure belongs to terrain; props use local support, burial,
    /// surface conditions and shared shader clocks instead.
    non_fluid: HashMap<SectionPos, (u64, u64, u64)>,
    non_fluid_changed: bool,
    #[cfg(test)]
    sampled_sections: usize,
}

impl PropAppearanceState {
    fn refresh(&mut self, chunks: &ChunkStore, terrain: &TerraGenerator, journal: &EditJournal) {
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
            for &section in chunks.loaded.keys() {
                if self.sections.contains_key(&[section.x, section.z]) {
                    self.sample_section(journal, section);
                }
            }
        }
        self.non_fluid_changed = self.non_fluid != previous;
    }

    fn sample_section(&mut self, journal: &EditJournal, section: SectionPos) {
        #[cfg(test)]
        {
            self.sampled_sections += 1;
        }
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
        if terrain.config().bounded_world {
            // Keep tracked journal changes independent of vertical streaming order.
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
        self.non_fluid
            .retain(|p, _| self.sections.contains_key(&[p.x, p.z]));
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
}
#[derive(Default)]
struct PropQueue {
    dirty: bool,
    selection_dirty: bool,
    positions: Vec<(SectionPos, DetailLevel, u64, u64)>,
    changed: HashSet<SectionPos>,
    changes: PropChangeReader,
    vegetated: HashSet<SectionPos>,
    web_owners: HashSet<SectionPos>,
    selection: VecDeque<SectionPos>,
    selecting: HashSet<SectionPos>,
    waiting_support: HashSet<SectionPos>,
    prop_epochs: HashMap<SectionPos, u64>,
    #[cfg(test)]
    selection_prune_visits: usize,
}

#[derive(Default)]
struct PropChangeReader {
    published: MessageCursor<SectionPublished>,
    removed: MessageCursor<SectionRemoved>,
    columns: MessageCursor<PreparedColumnChanged>,
    water: MessageCursor<WaterPublished>,
}

#[derive(Default)]
struct PropChanges {
    published: Vec<SectionPos>,
    removed: Vec<SectionPos>,
    columns: HashSet<[i64; 2]>,
    water: bool,
}

impl PropChanges {
    fn any(&self) -> bool {
        !self.published.is_empty()
            || !self.removed.is_empty()
            || !self.columns.is_empty()
            || self.water
    }
}

impl PropChangeReader {
    fn read(&mut self, world: &PropWorld) -> PropChanges {
        PropChanges {
            water: world
                .water
                .as_ref()
                .is_some_and(|messages| self.water.read(messages).count() > 0),
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

struct NeighborOwners {
    positions: HashSet<SectionPos>,
    #[cfg(test)]
    probes: usize,
}

fn section_neighbors(center: SectionPos, radius: [i64; 3]) -> impl Iterator<Item = SectionPos> {
    (-radius[2]..=radius[2]).flat_map(move |z| {
        (-radius[1]..=radius[1]).flat_map(move |y| {
            (-radius[0]..=radius[0]).filter_map(move |x| {
                Some(SectionPos::new(
                    center.x.checked_add(x)?,
                    center.y.checked_add(y)?,
                    center.z.checked_add(z)?,
                ))
            })
        })
    })
}

fn neighbor_owners(
    owners: &HashSet<SectionPos>,
    changes: impl Iterator<Item = SectionPos>,
    radius: [i64; 3],
) -> NeighborOwners {
    let mut result = NeighborOwners {
        positions: HashSet::new(),
        #[cfg(test)]
        probes: 0,
    };
    if owners.is_empty() {
        return result;
    }
    // Mass vertical unloading can publish thousands of changes beside only a
    // handful of web/support owners. Probe the smaller sparse domain instead.
    if owners.len() < changes.size_hint().0 {
        let changes: HashSet<_> = changes.collect();
        for &owner in owners {
            for position in section_neighbors(owner, radius) {
                #[cfg(test)]
                {
                    result.probes += 1;
                }
                if changes.contains(&position) {
                    result.positions.insert(owner);
                    break;
                }
            }
        }
    } else {
        for changed in changes {
            for owner in section_neighbors(changed, radius) {
                #[cfg(test)]
                {
                    result.probes += 1;
                }
                if owners.contains(&owner) {
                    result.positions.insert(owner);
                }
            }
        }
    }
    result
}

impl PropQueue {
    fn record_web_owner(&mut self, owner: SectionPos, chunks: &ChunkStore) {
        if chunks.loaded.get(&owner).is_some_and(|section| {
            section
                .props
                .iter()
                .any(|prop| prop.kind == PropKind::SpiderWeb && !prop.removed)
        }) {
            self.web_owners.insert(owner);
        } else {
            self.web_owners.remove(&owner);
        }
    }

    fn invalidate_web_neighbors(&mut self, changes: impl Iterator<Item = SectionPos>) {
        let affected = neighbor_owners(&self.web_owners, changes, [2, 1, 2]).positions;
        self.changed.extend(affected.iter().copied());
        self.waiting_support.extend(affected);
    }

    fn record_prop_edits(&mut self, chunks: &ChunkStore, journal: &EditJournal) {
        let history = journal.changed_sections();
        let mut record = |position| {
            let epoch = journal.prop_epoch(position);
            if epoch == self.prop_epochs.get(&position).copied().unwrap_or(0) {
                return;
            }
            let Some(section) = chunks.loaded.get(&position) else {
                return;
            };
            self.prop_epochs.insert(position, epoch);
            self.changed.insert(position);
            self.record_web_owner(position, chunks);
            if section.props.iter().any(|p| !p.removed) {
                self.vegetated.insert(position);
            } else {
                self.vegetated.remove(&position);
            }
        };
        // Sparse player edits also arrive alongside unrelated streaming/fluid
        // publications. They must not depend on a section already having props.
        if history.size_hint().0 < chunks.loaded.len() {
            history.for_each(record);
        } else {
            chunks.loaded.keys().copied().for_each(&mut record);
        }
    }

    fn select(&mut self, position: SectionPos, urgent: bool) {
        if self.selecting.insert(position) {
            if urgent {
                self.selection.push_front(position);
            } else {
                self.selection.push_back(position);
            }
        } else if urgent {
            self.selection.retain(|p| *p != position);
            self.selection.push_front(position);
        }
    }

    fn next_selection(&mut self) -> Option<SectionPos> {
        let position = self.selection.pop_front()?;
        self.selecting.remove(&position);
        Some(position)
    }

    fn replace_selected(
        &mut self,
        selected: &HashSet<SectionPos>,
        replacements: Vec<(SectionPos, DetailLevel, u64, u64)>,
    ) {
        #[cfg(test)]
        {
            self.selection_prune_visits = 0;
        }
        if !selected.is_empty() {
            self.positions.retain(|(position, ..)| {
                #[cfg(test)]
                {
                    self.selection_prune_visits += 1;
                }
                !selected.contains(position)
            });
        }
        self.positions.extend(replacements);
    }

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
        .init_resource::<PropAppearanceState>()
        .add_systems(
            Update,
            (refresh_prop_appearance, update_visibility, upload, dispatch)
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
    water: Option<Res<'w, Messages<WaterPublished>>>,
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

fn refresh_prop_appearance(world: PropWorld, mut state: ResMut<PropAppearanceState>) {
    if world.terrain.is_changed() {
        *state = PropAppearanceState::default();
    } else if world.chunks.is_changed() {
        state.retain_loaded(&world.chunks);
    }
    if world.journal.is_changed() || world.terrain.is_changed() {
        state.refresh(&world.chunks, &world.terrain.0, &world.journal);
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
        for variant in (0..if kind == PropKind::SpiderWeb {
            terra_world::props::webs::VARIANTS as u8
        } else {
            8
        })
            .chain((128..136).filter(|_| {
                !kind.aquatic() && kind.crop().is_none() && kind != PropKind::SpiderWeb
            }))
        {
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
                            && kind.crop().is_none()
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
    (mut jobs, mut appearance, mut meshes): (
        ResMut<PropJobs>,
        Option<ResMut<PropAppearanceState>>,
        ResMut<Assets<Mesh>>,
    ),
    (mut status, mut queue, mut growth_status): (
        ResMut<PropRenderStatus>,
        Local<PropQueue>,
        ResMut<PropGrowthStatus>,
    ),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("props.dispatch");
    let now = world.time.as_deref().map_or(0.0, Time::elapsed_secs);
    growth_status.crop_cleanup.retain(|position, deadline| {
        if !world.chunks.loaded.contains_key(position) {
            return false;
        }
        if now >= *deadline {
            queue.changed.insert(*position);
            return false;
        }
        true
    });
    let changes = queue.changes.read(&world);
    queue.changed.extend(jobs.1.drain());
    if world.journal.is_changed() {
        queue.record_prop_edits(&world.chunks, &world.journal);
    }
    let untracked_change = world.chunks.is_changed() && !changes.any();
    // Empty sky/rock sections never acquire a vegetation LOD merely because
    // the camera moved. Publications maintain the small live selection domain.
    if untracked_change || world.terrain.is_changed() {
        queue.vegetated = world
            .chunks
            .loaded
            .iter()
            .filter_map(|(&p, section)| section.props.iter().any(|prop| !prop.removed).then_some(p))
            .collect();
        queue.web_owners = queue
            .vegetated
            .iter()
            .filter(|owner| {
                world.chunks.loaded[*owner]
                    .props
                    .iter()
                    .any(|prop| prop.kind == PropKind::SpiderWeb && !prop.removed)
            })
            .copied()
            .collect();
        queue
            .selection
            .retain(|p| world.chunks.loaded.contains_key(p));
        queue
            .selecting
            .retain(|p| world.chunks.loaded.contains_key(p));
        queue
            .waiting_support
            .retain(|p| world.chunks.loaded.contains_key(p));
        queue
            .prop_epochs
            .retain(|p, _| world.chunks.loaded.contains_key(p));
    } else {
        for position in &changes.published {
            queue.record_web_owner(*position, &world.chunks);
            if world
                .chunks
                .loaded
                .get(position)
                .is_some_and(|s| s.props.iter().any(|p| !p.removed))
            {
                queue.vegetated.insert(*position);
            } else {
                queue.vegetated.remove(position);
            }
        }
    }
    queue.invalidate_web_neighbors(changes.published.iter().chain(&changes.removed).copied());
    let journal_selection_changed = world.journal.is_changed()
        && appearance
            .as_deref()
            .is_none_or(|state| state.non_fluid_changed);
    let waiting = neighbor_owners(
        &queue.waiting_support,
        changes.published.iter().copied(),
        [1, 1, 1],
    )
    .positions;
    queue.changed.extend(waiting);
    queue.changed.extend(changes.published);
    if !changes.removed.is_empty() {
        let removed: HashSet<_> = changes
            .removed
            .into_iter()
            .filter(|position| !world.chunks.loaded.contains_key(position))
            .collect();
        for position in &removed {
            cancel_job(&mut jobs, *position, &mut meshes);
            queue.changed.remove(position);
            queue.vegetated.remove(position);
            queue.web_owners.remove(position);
            queue.selecting.remove(position);
            queue.waiting_support.remove(position);
            queue.prop_epochs.remove(position);
        }
        queue.selection.retain(|p| !removed.contains(p));
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
        && queue.selection.is_empty()
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
        let appearance_epoch = world.journal.appearance_epoch(position);
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
                        // A zero cutoff preserves climate-tagged vertex alpha,
                        // but keeps the crop coverage fragment in depth-only passes.
                        alpha_mode: AlphaMode::Mask(0.0),
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
        let removed: Vec<_> = jobs
            .0
            .keys()
            .copied()
            .filter(|p| !world.chunks.loaded.contains_key(p))
            .collect();
        for position in removed {
            cancel_job(&mut jobs, position, &mut meshes);
        }
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
        let mut candidates: Vec<_> = queue.vegetated.iter().copied().collect();
        candidates.extend(pending);
        candidates.extend(queue.changed.iter().copied());
        // An empty section may still own a previous batch that must retire.
        if untracked_change {
            candidates.extend(
                rendered
                    .0
                    .iter()
                    .filter_map(|(&p, old)| old.entity.is_some().then_some(p)),
            );
        }
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
        let mut candidates = candidates;
        candidates
            .sort_unstable_by_key(|p| (distance_squared(*p, world.focus.0) as u64, p.y, p.z, p.x));
        candidates.dedup();
        for position in candidates {
            let urgent = rendered.0.get(&position).is_some_and(|old| {
                old.prop_epoch != world.journal.prop_epoch(position)
                    || old.terrain_epoch != world.journal.mesh_epoch(position)
            });
            queue.select(position, urgent);
        }
    }
    // Keep a stable deduplicated sweep when fast flight changes focus again.
    // In particular, one LOD sweep cannot monopolize a frame or lose an edit.
    {
        let started = std::time::Instant::now();
        let _scope = terra_core::profiling::ScopeTimer::new("props.select");
        let mut selected = HashSet::with_capacity(96);
        let mut replacements = Vec::with_capacity(96);
        for index in 0..96 {
            if index > 0 && started.elapsed().as_secs_f64() >= 0.0015 {
                break;
            }
            let Some(position) = queue.next_selection() else {
                break;
            };
            let Some(section) = world.chunks.loaded.get(&position) else {
                continue;
            };
            selected.insert(position);
            if section.props.iter().any(|p| !p.removed)
                && let Some(state) = appearance.as_deref_mut()
            {
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
            let appearance_epoch = world.journal.appearance_epoch(position);
            if previous.is_some_and(|p| {
                p.entity
                    .is_some_and(|entity| world.entering.contains(entity))
                    && p.terrain_epoch == section.mesh_epoch
                    && p.prop_epoch == prop_epoch
                    && p.appearance_epoch == appearance_epoch
            }) && !queue.waiting_support.contains(&position)
                && previous.is_none_or(|p| !p.support_pending)
            {
                continue;
            }
            if previous.is_some_and(|p| {
                p.terrain_epoch == section.mesh_epoch
                    && p.prop_epoch == prop_epoch
                    && p.appearance_epoch == appearance_epoch
                    && p.level == level
                    && p.density == 1.0
                    && !p.expired_crop_fade(now)
            }) && !queue.waiting_support.contains(&position)
                && previous.is_none_or(|p| !p.support_pending)
            {
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
                        support_pending: false,
                    },
                );
                continue;
            }
            if jobs.0.contains_key(&position)
                || section.mesh_epoch != world.journal.mesh_epoch(position)
            {
                if queue.waiting_support.contains(&position) {
                    queue.changed.insert(position);
                }
                continue;
            }
            replacements.push((position, level, prop_epoch, appearance_epoch));
        }
        queue.replace_selected(&selected, replacements);
        queue.prioritize(world.focus.0);
    }
    // Finishing a worker frees capacity, not a reason to scan every loaded
    // section again. Invalidated worker results explicitly request another scan.
    let _prepare_scope = terra_core::profiling::ScopeTimer::new("props.prepare");
    for _ in 0..available {
        let Some((position, level, prop_epoch, _appearance_epoch)) = queue.positions.pop() else {
            break;
        };
        let Some(section) = world.chunks.loaded.get(&position) else {
            continue;
        };
        let density = 1.0;
        let mut unknown_support = false;
        let props: Vec<_> = section
            .props
            .iter()
            .filter(|p| {
                if p.kind == terra_world::props::PropKind::AppleFruit {
                    return false;
                }
                if p.kind == terra_world::props::PropKind::SpiderWeb
                    && !terra_world::props::webs::within_world_bounds(
                        p,
                        world.terrain.0.config().bounded_world,
                    )
                {
                    return false;
                }
                let support =
                    terra_world::props::placement::supported(p, |cell| world.chunks.block_at(cell));
                unknown_support |= support.is_none();
                support == Some(true)
                    && terra_world::props::visible_at_density(p, density)
                    && !terra_world::props::seasonal::snow_buried(p, &world.chunks, &world.journal)
            })
            .copied()
            .collect();
        if unknown_support {
            queue.waiting_support.insert(position);
        } else {
            queue.waiting_support.remove(&position);
        }
        let templates = templates.clone();
        let growth = growth::snapshot(
            &props,
            rendered.0.get(&position).map(|previous| &previous.growth),
            world.time.as_deref().map_or(0.0, Time::elapsed_secs),
        );
        let task_growth = growth.clone();
        let terrain = world.terrain.0.clone();
        let conditions = world.journal.surface_snapshot(position);
        let appearance_epoch = world.journal.appearance_epoch(position);
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
                None,
                packed_prop_composition(level),
            )
            .and_then(|(batches, count)| {
                let mut batches = crate::upload_budget::split_batches(batches)?;
                for batch in &mut batches {
                    super::static_layout::strip_identity_growth(batch);
                    batch.compact_indices();
                }
                Ok((batches, count))
            });
            (result, start.elapsed().as_secs_f64())
        });
        if let Some(mut previous) = jobs.0.insert(
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
                support_pending: unknown_support,
            },
        ) {
            release_job(&mut previous, &mut meshes);
        }
    }
    status.ready = queue.positions.is_empty()
        && queue.changed.is_empty()
        && queue.selection.is_empty()
        && !queue.dirty
        && jobs.0.is_empty()
        && jobs.1.is_empty();
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
    _exposure: Option<&crate::weather::surfaces::exposure::Exposure>,
    packed: bool,
) -> Result<(Vec<MeshBatch>, usize), String> {
    let origin = section.origin().map_err(|e| e.to_string())?;
    let tint = GrassTint::new(terrain, origin);
    let wind_strength = match level {
        DetailLevel::Near => 1.0,
        DetailLevel::Middle => 0.5,
        DetailLevel::Far => 0.0,
    };
    // A flower-bearing section keeps one shared rooted layout, rather than
    // adding a draw call solely for seasonal shrinking. Static props use 1x.
    let rooted_section = props
        .iter()
        .any(|prop| !prop.removed && prop.kind == PropKind::Flower);
    // Keep the old crop stage only for the short shader crossfade, in the same
    // section batch. It is removed by the bounded cleanup queue afterwards.
    let mut staged = Vec::with_capacity(props.len());
    for prop in props.iter().filter(|p| !p.removed && !p.hidden_growth()) {
        staged.push((*prop, false));
        if let Some(variant) = growth.get(&prop.id).and_then(|pose| pose.previous_variant) {
            staged.push((WorldProp { variant, ..*prop }, true));
        }
    }
    let entries = staged
        .iter()
        .map(|(prop, _)| {
            let source = &templates
                .get(&(prop.kind as u8, prop.template_variant(), level))
                .ok_or("Missing prop template")?
                .geometry;
            Ok(batch_plan::Entry {
                vertices: source.positions.len(),
                indices: source.indices.len(),
                rooted: rooted_section || growth::animated(prop),
                pending: growth.get(&prop.id).is_some_and(|pose| pose.pending),
            })
        })
        .collect::<Result<Vec<_>, &str>>()?;
    let plan = batch_plan::plan(&entries);
    let mut batches: Vec<_> = plan
        .pieces
        .iter()
        .map(|piece| {
            let mut batch = MeshBatch::default();
            let [vertices, indices, instances] = [piece.vertices, piece.indices, piece.instances];
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
            if piece.rooted {
                batch.rooted_wind.reserve_exact(vertices);
                batch.growth_anchors.reserve_exact(vertices);
                batch.growth_ranges.reserve_exact(instances);
            }
            batch
        })
        .collect();
    let mut count = 0;
    for ((prop, fading_out), destination) in staged.iter().zip(plan.destinations) {
        let Some(destination) = destination else {
            continue;
        };
        let variant = prop.template_variant();
        let batch = &mut batches[destination];
        let template = templates
            .get(&(prop.kind as u8, variant, level))
            .ok_or("Missing prop template")?;
        let source = &template.geometry;
        if source.indices.is_empty() {
            continue;
        }
        count += usize::from(!*fading_out);
        let scale = prop.visual_scale();
        let rotation = Quat::from_rotation_y(prop.rotation);
        let offset = Vec3::from_array(std::array::from_fn(|i| {
            (prop.position[i] - origin[i] as f64) as f32
        }));
        let base = u32::try_from(batch.positions.len()).map_err(|_| "Prop batch overflow")?;
        if rooted_section || growth::animated(prop) {
            if growth::animated(prop) {
                batch.growth_ranges.push((
                    prop.id,
                    batch.positions.len()..batch.positions.len() + source.positions.len(),
                ));
            }
            let pose = growth.get(&prop.id).copied().unwrap_or(Pose {
                scale: growth::extent(prop),
                ratio: 1.0,
                start: 0.0,
                pending: false,
                variant: prop.variant,
                previous_variant: None,
            });
            batch.growth_anchors.extend(std::iter::repeat_n(
                offset.to_array(),
                source.positions.len(),
            ));
            batch.rooted_wind.extend(std::iter::repeat_n(
                [
                    pose.start,
                    if *fading_out {
                        -3.0
                    } else if prop.kind.crop().is_some()
                        && (pose.pending || pose.previous_variant.is_some() || pose.ratio < 1.0)
                    {
                        -1.0 - pose.ratio
                    } else {
                        pose.ratio
                    },
                ],
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
        let colors = source
            .colors
            .iter()
            .zip(&template.biome_tinted)
            .map(|(color, &green)| {
                let mut color = *color;
                for channel in 0..3 {
                    color[channel] *= condition[channel] * if green { biome[channel] } else { 1.0 };
                }
                color[3] = terra_world::props::seasonal::flower_rank(prop);
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
    Ok((batches, count))
}

fn patch_growth(batches: &mut [MeshBatch], growth: &HashMap<terra_world::props::PropId, Pose>) {
    for batch in batches {
        if batch.staged.is_some() {
            continue;
        }
        for (id, range) in &batch.growth_ranges {
            if let Some(pose) = growth.get(id) {
                let tag = batch.rooted_wind[range.start][1];
                let ratio = if tag < -2.5 {
                    -3.0
                } else if tag < -0.5 {
                    -1.0 - pose.ratio
                } else {
                    pose.ratio
                };
                batch.rooted_wind[range.clone()].fill([pose.start, ratio]);
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
            cancel_job(&mut jobs, position, &mut meshes);
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
    let unloaded_jobs: Vec<_> = jobs
        .0
        .keys()
        .copied()
        .filter(|position| !world.chunks.loaded.contains_key(position))
        .collect();
    for position in unloaded_jobs {
        cancel_job(&mut jobs, position, &mut meshes);
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
        let Some(job) = jobs.0.get_mut(&position) else {
            continue;
        };
        let Some((mut result, seconds)) = job
            .ready
            .take()
            .or_else(|| future::block_on(future::poll_once(&mut job.task)))
        else {
            continue;
        };
        let current_level = select_prop_level(
            position,
            world.focus.0,
            rendered.0.get(&position).map(|p| p.level),
            &world.config.client,
        );
        if current_level != job.level
            || job.density != 1.0
            || job.prop_epoch != world.journal.prop_epoch(position)
            || job.terrain_epoch != world.journal.mesh_epoch(position)
            || job.appearance_epoch != world.journal.appearance_epoch(position)
            || world
                .chunks
                .loaded
                .get(&position)
                .is_none_or(|s| s.mesh_epoch != job.terrain_epoch)
        {
            status.completed_jobs += 1;
            status.composition_seconds += seconds;
            release_result(&mut result, &mut meshes);
            jobs.0.remove(&position);
            jobs.1.insert(position);
            continue;
        }
        if rendered.0.get(&position).is_some_and(|old| {
            old.entity
                .is_some_and(|entity| world.entering.contains(entity))
                && old.terrain_epoch == job.terrain_epoch
                && old.prop_epoch == job.prop_epoch
                && old.appearance_epoch == job.appearance_epoch
                && old.level == job.level
                && old.density == job.density
                && !old.support_pending
        }) {
            status.completed_jobs += 1;
            status.composition_seconds += seconds;
            release_result(&mut result, &mut meshes);
            jobs.0.remove(&position);
            continue;
        }
        if budget
            .as_mut()
            .is_some_and(|budget| !budget.reserve_section(crate::upload_budget::UploadKind::Props))
        {
            job.ready = Some((result, seconds));
            break;
        }
        if let Ok((batches, _)) = &mut result {
            let now = world.time.as_deref().map_or(0.0, Time::elapsed_secs);
            let previous = rendered.0.get(&position).map(|r| &r.growth);
            for batch in batches.iter_mut().filter(|b| b.staged.is_none()) {
                if batch.positions.is_empty() || !batch.has_indices() {
                    continue;
                }
                if budget.as_mut().is_some_and(|budget| {
                    !budget.reserve_batch_bytes(
                        crate::upload_budget::UploadKind::Props,
                        batch.positions.len(),
                        crate::upload_budget::batch_bytes(batch),
                    )
                }) {
                    break;
                }
                // Pending poses share the final growing batch. Earlier rooted
                // pieces contain mature/running poses and must not start them.
                if !batch.growth_ranges.is_empty() {
                    for (id, _) in &batch.growth_ranges {
                        let Some(pose) = job.growth.get_mut(id).filter(|pose| pose.pending) else {
                            continue;
                        };
                        pose.begin(previous.and_then(|p| p.get(id)).copied(), now);
                        growth_status.transitions = growth_status.transitions.saturating_add(1);
                        growth_status.active_until =
                            growth_status.active_until.max(now + growth::SECONDS);
                        growth_status.active.insert(*id, *pose);
                    }
                    patch_growth(std::slice::from_mut(batch), &job.growth);
                }
                prepare_prop_batch(batch, &mut meshes);
            }
            if batches.iter().any(|batch| {
                batch.staged.is_none() && !batch.positions.is_empty() && batch.has_indices()
            }) {
                job.ready = Some((result, seconds));
                break;
            }
        }
        status.composition_seconds += seconds;
        status.completed_jobs += 1;
        let Some(job) = jobs.0.remove(&position) else {
            continue;
        };
        uploaded += 1;
        let level = current_level;
        let (mut batches, count) = match result {
            Ok(result) => result,
            Err(error) => {
                status.error = Some(error);
                continue;
            }
        };
        batches.retain(|batch| batch.staged.is_some());
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
        growth_status
            .active
            .retain(|_, pose| pose.start + growth::SECONDS > now);
        if let Some(deadline) = job
            .growth
            .values()
            .filter(|pose| pose.previous_variant.is_some() && !pose.pending)
            .map(|pose| pose.start + growth::SECONDS)
            .max_by(f32::total_cmp)
        {
            growth_status.crop_cleanup.insert(position, deadline);
        }
        let origin = if batches.is_empty() {
            None
        } else {
            match position.origin() {
                Ok(origin) => Some(origin),
                Err(_) => {
                    crate::upload_budget::release_staged(&mut batches, &mut meshes);
                    continue;
                }
            }
        };
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
            support_pending: job.support_pending,
        };
        result.growth = job.growth;
        if let Some(origin) = origin {
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
                let prepared = batch.staged.take().expect("complete prop upload is atomic");
                let bounds = prepared.bounds;
                let handle = prepared.handle;
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
                    entity.insert((bounds, bevy::camera::visibility::NoAutoAabb));
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

fn prepare_prop_batch(batch: &mut MeshBatch, meshes: &mut Assets<Mesh>) {
    let indices = batch.take_indices();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        std::mem::take(&mut batch.positions),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, std::mem::take(&mut batch.uvs))
    .with_inserted_indices(indices);
    if batch.packed_normals.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, std::mem::take(&mut batch.normals));
    } else {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Snorm8x4,
                ..Mesh::ATTRIBUTE_NORMAL
            },
            bevy::mesh::VertexAttributeValues::Snorm8x4(std::mem::take(&mut batch.packed_normals)),
        );
    }
    if batch.packed_colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut batch.colors));
    } else {
        mesh.insert_attribute(
            bevy::mesh::MeshVertexAttribute {
                format: bevy::render::render_resource::VertexFormat::Unorm8x4,
                ..Mesh::ATTRIBUTE_COLOR
            },
            bevy::mesh::VertexAttributeValues::Unorm8x4(std::mem::take(&mut batch.packed_colors)),
        );
    }
    if !batch.growth_anchors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, std::mem::take(&mut batch.rooted_wind));
        mesh.insert_attribute(
            wind::ATTRIBUTE_GROWTH_ANCHOR,
            std::mem::take(&mut batch.growth_anchors),
        );
    }
    batch.staged = Some(crate::meshing::staging::StagedMesh {
        opaque_shadow: false,
        bounds: wind::padded_bounds(&mesh),
        handle: meshes.add(mesh),
        rooted: false,
        damp: false,
        coastal_quads: 0,
        far: None,
    });
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
mod partial_upload_tests {
    use super::*;
    use bevy::tasks::{TaskPool, TaskPoolBuilder};

    fn batch(vertices: usize) -> MeshBatch {
        MeshBatch {
            positions: vec![[0.0; 3]; vertices],
            normals: vec![[0.0, 1.0, 0.0]; vertices],
            colors: vec![[1.0; 4]; vertices],
            uvs: vec![[0.0; 2]; vertices],
            indices: (0..(vertices - vertices % 3) as u32).collect(),
            ..Default::default()
        }
    }

    fn fixture() -> (App, TaskPool, Entity) {
        let mut app = App::new();
        let mut config = GameConfig::default();
        config.client.chunk_animation = terra_core::ChunkAnimation::None;
        let terrain = TerraGenerator::new(config.world.clone()).unwrap();
        app.insert_resource(config)
            .insert_resource(WorldTerrain(terrain))
            .insert_resource(ChunkFocus([8.0; 3]))
            .init_resource::<Time>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<PropTemplates>()
            .init_resource::<PropBatches>()
            .init_resource::<PropJobs>()
            .init_resource::<PropRenderStatus>()
            .init_resource::<PropGrowthStatus>()
            .init_resource::<crate::MeshUploadStatus>()
            .init_resource::<crate::transitions::LodSwapStatus>()
            .add_systems(First, crate::MeshUploadStatus::reset)
            .add_systems(Update, upload);
        app.world_mut().resource_mut::<PropTemplates>().material = Some(Handle::default());
        let position = SectionPos::default();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
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
        let root = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        let mut previous = batch(3);
        prepare_prop_batch(
            &mut previous,
            &mut app.world_mut().resource_mut::<Assets<Mesh>>(),
        );
        app.world_mut().resource_mut::<PropBatches>().0.insert(
            position,
            RenderedProps {
                density: 1.0,
                entity: Some(root),
                meshes: vec![previous.staged.take().unwrap().handle],
                terrain_epoch: 0,
                prop_epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                count: 1,
                growth: HashMap::new(),
                support_pending: false,
            },
        );
        (app, TaskPoolBuilder::new().num_threads(1).build(), root)
    }

    fn enqueue(app: &mut App, pool: &TaskPool, batches: Vec<MeshBatch>) -> usize {
        let mut batches = crate::upload_budget::split_batches(batches).unwrap();
        for batch in &mut batches {
            batch.compact_indices();
        }
        let count = batches.len();
        app.world_mut().resource_mut::<PropJobs>().0.insert(
            SectionPos::default(),
            PropJob {
                ready: Some((Ok((batches, 2)), 0.125)),
                density: 1.0,
                terrain_epoch: 0,
                prop_epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                task: pool.spawn(std::future::pending()),
                growth: HashMap::new(),
                support_pending: false,
            },
        );
        count
    }

    fn staged(world: &World) -> Vec<AssetId<Mesh>> {
        world.resource::<PropJobs>().0[&SectionPos::default()]
            .ready
            .as_ref()
            .unwrap()
            .0
            .as_ref()
            .unwrap()
            .0
            .iter()
            .filter_map(|batch| batch.staged.as_ref().map(|staged| staged.handle.id()))
            .collect()
    }

    #[test]
    fn material_batches_stage_once_and_replace_the_previous_prop_root_atomically() {
        let (mut app, pool, old) = fixture();
        enqueue(
            &mut app,
            &pool,
            vec![batch(60_000), MeshBatch::default(), batch(60_000)],
        );
        app.update();
        let first = staged(app.world());
        assert_eq!(first.len(), 1);
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            60_000
        );
        assert_eq!(
            app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
            Some(old)
        );
        assert!(app.world().get_entity(old).is_ok());
        assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 0);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            60_000
        );
        assert!(app.world().resource::<PropJobs>().0.is_empty());
        let rendered = &app.world().resource::<PropBatches>().0[&SectionPos::default()];
        assert_ne!(rendered.entity, Some(old));
        assert_eq!(rendered.meshes.len(), 2);
        assert_eq!(
            rendered.meshes[0].id(),
            first[0],
            "staged assets must not be recreated"
        );
        assert!(app.world().get_entity(old).is_err());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 1);
        assert_eq!(
            app.world()
                .resource::<PropRenderStatus>()
                .composition_seconds,
            0.125
        );
    }

    #[test]
    fn entering_prop_roots_accept_partial_lod_and_density_replacements() {
        for changed_lod in [true, false] {
            let (mut app, pool, old) = fixture();
            let position = SectionPos::default();
            let level = if changed_lod {
                app.world_mut().resource_mut::<ChunkFocus>().0 = [120.0, 8.0, 8.0];
                DetailLevel::Middle
            } else {
                app.world_mut()
                    .resource_mut::<PropBatches>()
                    .0
                    .get_mut(&position)
                    .unwrap()
                    .density = 0.5;
                DetailLevel::Near
            };
            crate::transitions::start_test_fade(app.world_mut(), old);
            enqueue(&mut app, &pool, vec![batch(60_000), batch(60_000)]);
            app.world_mut()
                .resource_mut::<PropJobs>()
                .0
                .get_mut(&position)
                .unwrap()
                .level = level;
            app.update();
            let first = staged(app.world());
            assert_eq!(first.len(), 1);
            assert_eq!(
                app.world().resource::<PropBatches>().0[&position].entity,
                Some(old)
            );
            assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 0);
            app.update();
            assert!(app.world().resource::<PropJobs>().0.is_empty());
            let rendered = &app.world().resource::<PropBatches>().0[&position];
            assert_eq!(rendered.level, level);
            assert_eq!(rendered.density, 1.0);
            assert_ne!(rendered.entity, Some(old));
            assert_eq!(rendered.meshes[0].id(), first[0]);
            assert_eq!(rendered.meshes.len(), 2);
            assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 1);
        }
    }

    #[test]
    fn oversized_prop_worker_result_progresses_as_bounded_parts_and_swaps_atomically() {
        let (mut app, pool, old) = fixture();
        let oversized = crate::upload_budget::PROP_VERTICES + 6;
        let pieces = enqueue(&mut app, &pool, vec![batch(oversized), batch(3)]);
        app.update();
        assert_eq!(staged(app.world()).len(), 1);
        let first = app
            .world()
            .resource::<crate::MeshUploadStatus>()
            .admitted_vertices;
        assert!(first <= crate::upload_budget::MAX_BATCH_VERTICES);
        assert!(first < oversized);
        assert!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_bytes
                <= crate::upload_budget::PROP_BYTES
        );
        assert_eq!(
            app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
            Some(old)
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::MeshUploadStatus>()
                .admitted_vertices,
            oversized - first + 3
        );
        assert!(app.world().resource::<PropJobs>().0.is_empty());
        assert_eq!(
            app.world().resource::<PropBatches>().0[&SectionPos::default()]
                .meshes
                .len(),
            pieces
        );
    }

    #[test]
    fn partial_prop_assets_are_released_on_epoch_lod_density_and_unload_invalidation() {
        for cause in ["terrain", "props", "appearance", "lod", "density", "unload"] {
            let (mut app, pool, old) = fixture();
            enqueue(&mut app, &pool, vec![batch(60_000), batch(60_000)]);
            app.update();
            let handles = staged(app.world());
            match cause {
                "unload" => {
                    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
                }
                cause => {
                    let mut jobs = app.world_mut().resource_mut::<PropJobs>();
                    let job = jobs.0.get_mut(&SectionPos::default()).unwrap();
                    match cause {
                        "terrain" => job.terrain_epoch += 1,
                        "props" => job.prop_epoch += 1,
                        "appearance" => job.appearance_epoch += 1,
                        "lod" => job.level = DetailLevel::Far,
                        "density" => job.density = 0.5,
                        _ => unreachable!(),
                    }
                }
            }
            app.update();
            assert!(app.world().resource::<PropJobs>().0.is_empty(), "{cause}");
            assert!(
                handles
                    .iter()
                    .all(|id| !app.world().resource::<Assets<Mesh>>().contains(*id)),
                "{cause}"
            );
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                0,
                "{cause}"
            );
            if cause != "unload" {
                assert_eq!(
                    app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
                    Some(old),
                    "{cause}"
                );
            }
        }
    }

    #[test]
    fn prepared_column_messages_keep_partial_meshes_and_finish_the_atomic_handoff() {
        let (mut app, pool, old) = fixture();
        app.add_message::<PreparedColumnChanged>();
        enqueue(&mut app, &pool, vec![batch(60_000), batch(60_000)]);
        app.update();
        let handles = staged(app.world());
        assert_eq!(handles.len(), 1);
        app.world_mut()
            .write_message(PreparedColumnChanged { coordinate: [0, 0] });
        app.update();
        assert!(app.world().resource::<PropJobs>().0.is_empty());
        assert!(app.world().resource::<PropJobs>().1.is_empty());
        assert!(
            handles
                .iter()
                .all(|id| app.world().resource::<Assets<Mesh>>().contains(*id))
        );
        let rendered = &app.world().resource::<PropBatches>().0[&SectionPos::default()];
        assert_ne!(rendered.entity, Some(old));
        assert!(rendered.meshes.iter().any(|mesh| mesh.id() == handles[0]));
        assert_eq!(app.world().resource::<PropRenderStatus>().completed_jobs, 1);
    }

    #[test]
    fn delayed_static_staging_does_not_start_new_growth_before_the_atomic_handoff() {
        let (mut app, pool, old) = fixture();
        let id = terra_world::props::PropId(919);
        let previous = Pose {
            scale: 0.25,
            ratio: 1.0,
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<PropBatches>()
            .0
            .get_mut(&SectionPos::default())
            .unwrap()
            .growth
            .insert(id, previous);
        let mut growing = batch(60_000);
        growing.rooted_wind = vec![[-1.0, 0.5]; 60_000];
        growing.growth_anchors = vec![[0.0; 3]; 60_000];
        growing.growth_ranges.push((id, 0..60_000));
        let pieces = enqueue(&mut app, &pool, vec![batch(60_000), growing]);
        app.world_mut()
            .resource_mut::<PropJobs>()
            .0
            .get_mut(&SectionPos::default())
            .unwrap()
            .growth
            .insert(
                id,
                Pose {
                    scale: 0.5,
                    ratio: 0.5,
                    start: -1.0,
                    pending: true,
                    ..Default::default()
                },
            );
        app.update();
        assert!(app.world().resource::<PropJobs>().0[&SectionPos::default()].growth[&id].pending);
        assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 0);
        assert_eq!(
            app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
            Some(old)
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(15));
        app.update();
        let rendered = &app.world().resource::<PropBatches>().0[&SectionPos::default()];
        assert_eq!(rendered.meshes.len(), pieces);
        let pose = rendered.growth[&id];
        assert_eq!(pose.start, 15.0);
        assert!(!pose.pending);
        assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 1);
        assert_eq!(
            pose.scale * pose.factor(15.0),
            previous.scale * previous.factor(15.0)
        );
        let meshes = app.world().resource::<Assets<Mesh>>();
        let mut vertices = 0;
        for handle in &rendered.meshes[1..] {
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(timing)) =
                meshes.get(handle).unwrap().attribute(Mesh::ATTRIBUTE_UV_1)
            else {
                panic!("missing growth timing");
            };
            vertices += timing.len();
            assert!(
                timing
                    .iter()
                    .all(|value| *value == [pose.start, pose.ratio])
            );
        }
        assert_eq!(vertices, 60_000);
        app.update();
        assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 1);
    }

    #[test]
    fn stable_rooted_pieces_do_not_start_pending_crossfades_before_the_final_batch() {
        let (mut app, pool, old) = fixture();
        let mature = terra_world::props::PropId(918);
        let growing = terra_world::props::PropId(919);
        let hidden = terra_world::props::PropId(920);
        let previous = Pose {
            scale: 0.25,
            ratio: 1.0,
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<PropBatches>()
            .0
            .get_mut(&SectionPos::default())
            .unwrap()
            .growth
            .insert(growing, previous);
        let rooted = |id, tag| {
            let mut batch = batch(60_000);
            batch.rooted_wind = vec![[3.0, tag]; 60_000];
            batch.growth_anchors = vec![[0.0; 3]; 60_000];
            batch.growth_ranges.push((id, 0..60_000));
            batch
        };
        let mut pending = rooted(growing, -1.5);
        pending.rooted_wind[30_000..].fill([-1.0, -3.0]);
        pending.growth_ranges = vec![(growing, 0..30_000), (growing, 30_000..60_000)];
        let pieces = enqueue(
            &mut app,
            &pool,
            vec![rooted(mature, 0.7), rooted(mature, 0.7), pending],
        );
        {
            let mut jobs = app.world_mut().resource_mut::<PropJobs>();
            let job = jobs.0.get_mut(&SectionPos::default()).unwrap();
            job.growth.insert(
                mature,
                Pose {
                    scale: 0.8,
                    ratio: 0.7,
                    start: 3.0,
                    ..Default::default()
                },
            );
            job.growth.insert(
                growing,
                Pose {
                    scale: 0.5,
                    ratio: 0.5,
                    start: -1.0,
                    pending: true,
                    previous_variant: Some(1),
                    ..Default::default()
                },
            );
            job.growth.insert(
                hidden,
                Pose {
                    scale: 0.5,
                    ratio: 0.0,
                    start: -1.0,
                    pending: true,
                    previous_variant: Some(1),
                    ..Default::default()
                },
            );
        }
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_secs(15));
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::MeshUploadStatus>()
                    .admitted_vertices,
                60_000
            );
            let jobs = app.world().resource::<PropJobs>();
            let job = &jobs.0[&SectionPos::default()];
            assert!(job.growth[&growing].pending && job.growth[&hidden].pending);
            assert_eq!(job.growth[&mature].start, 3.0);
            assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 0);
            assert_eq!(
                app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
                Some(old)
            );
        }
        let stable = staged(app.world());
        assert!(!stable.is_empty() && stable.len() < pieces);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(15));
        app.update();
        let rendered = &app.world().resource::<PropBatches>().0[&SectionPos::default()];
        let pose = rendered.growth[&growing];
        assert_eq!(pose.start, 45.0);
        assert_eq!(pose.ratio, 0.5);
        assert!(!pose.pending);
        assert!(
            rendered.growth[&hidden].pending,
            "no geometry means no animation clock yet"
        );
        assert_eq!(rendered.meshes.len(), pieces);
        assert_eq!(
            rendered.meshes[..stable.len()]
                .iter()
                .map(Handle::id)
                .collect::<Vec<_>>(),
            stable
        );
        assert!(app.world().get_entity(old).is_err());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), pieces);
        assert_eq!(app.world().resource::<PropGrowthStatus>().transitions, 1);
        let meshes = app.world().resource::<Assets<Mesh>>();
        for handle in &rendered.meshes[..stable.len()] {
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(timing)) =
                meshes.get(handle).unwrap().attribute(Mesh::ATTRIBUTE_UV_1)
            else {
                panic!("missing rooted timing")
            };
            assert!(timing.iter().all(|value| *value == [3.0, 0.7]));
        }
        let mut timing = Vec::new();
        for handle in &rendered.meshes[stable.len()..] {
            let Some(bevy::mesh::VertexAttributeValues::Float32x2(values)) =
                meshes.get(handle).unwrap().attribute(Mesh::ATTRIBUTE_UV_1)
            else {
                panic!("missing growing timing")
            };
            timing.extend_from_slice(values);
        }
        assert_eq!(timing.len(), 60_000);
        assert!(timing[..30_000].iter().all(|value| *value == [45.0, -1.5]));
        assert!(timing[30_000..].iter().all(|value| *value == [45.0, -3.0]));
    }

    #[test]
    fn cancelling_or_unloading_multiple_staged_pieces_releases_every_partial_asset() {
        for unload in [false, true] {
            let (mut app, pool, old) = fixture();
            enqueue(&mut app, &pool, (0..5).map(|_| batch(60_000)).collect());
            app.update();
            app.update();
            let handles = staged(app.world());
            assert_eq!(handles.len(), 2);
            if unload {
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .loaded
                    .remove(&SectionPos::default());
            } else {
                app.world_mut()
                    .resource_mut::<PropJobs>()
                    .0
                    .get_mut(&SectionPos::default())
                    .unwrap()
                    .terrain_epoch = 1;
            }
            app.update();
            assert!(app.world().resource::<PropJobs>().0.is_empty());
            assert!(
                handles
                    .iter()
                    .all(|id| !app.world().resource::<Assets<Mesh>>().contains(*id))
            );
            if unload {
                assert!(
                    !app.world()
                        .resource::<PropBatches>()
                        .0
                        .contains_key(&SectionPos::default())
                );
            } else {
                assert_eq!(
                    app.world().resource::<PropBatches>().0[&SectionPos::default()].entity,
                    Some(old)
                );
            }
        }
    }
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    #[test]
    fn web_support_publications_and_unloads_invalidate_only_bounded_live_owners() {
        let web = terra_world::props::webs::spanning(
            [15, 8, -1],
            [1, 0, 0],
            21,
            10,
            terra_world::props::PropId(902),
        )
        .unwrap();
        let owner = web.section;
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![web],
            },
        );
        let mut queue = PropQueue::default();
        queue.record_web_owner(owner, &chunks);
        assert_eq!(queue.web_owners.len(), 1);
        // The far anchor belongs to the second neighboring section, not the
        // ordinary one-section vegetation halo. Publication and removal share it.
        let far_anchor = global_to_section([36, 8, -1]).0;
        for _ in 0..3 {
            queue.invalidate_web_neighbors(std::iter::once(far_anchor));
            assert_eq!(queue.changed, HashSet::from([owner]));
            assert!(
                queue.waiting_support.contains(&owner),
                "bypass unchanged owner epochs and recheck support"
            );
            queue.changed.clear();
        }
        queue.invalidate_web_neighbors(std::iter::once(SectionPos::new(3, 0, -1)));
        assert!(queue.changed.is_empty());
        chunks.loaded.get_mut(&owner).unwrap().props.clear();
        queue.record_web_owner(owner, &chunks);
        assert!(queue.web_owners.is_empty());
        queue.invalidate_web_neighbors(std::iter::once(far_anchor));
        assert!(queue.changed.is_empty());
        assert_eq!(
            chunks.loaded.len(),
            1,
            "support invalidation never generates neighbors"
        );
    }

    #[test]
    fn planting_into_an_empty_section_updates_the_index_despite_other_stream_publications() {
        let owner = SectionPos::default();
        let mut chunks = ChunkStore::default();
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block([8, 0, 8], 3).unwrap();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut journal = EditJournal::default();
        let mut queue = PropQueue::default();
        queue.record_prop_edits(&chunks, &journal);
        assert!(queue.vegetated.is_empty());
        let prop = WorldProp {
            id: terra_world::props::PropId(119),
            kind: PropKind::Grass,
            position: [8.5, 1.0, 8.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: owner,
            removed: false,
        };
        assert!(
            terra_world::streaming::commit_prop_placement(&mut chunks, &mut journal, prop).unwrap()
        );
        let changes = PropChanges {
            water: true,
            published: vec![SectionPos::new(10, 0, 10)],
            ..Default::default()
        };
        assert!(changes.any(), "This is not an untracked chunk mutation");
        queue.record_prop_edits(&chunks, &journal);
        assert!(queue.vegetated.contains(&owner));
        assert!(queue.changed.remove(&owner));
        queue.record_prop_edits(&chunks, &journal);
        assert!(
            queue.changed.is_empty(),
            "Unchanged edits do not requeue work"
        );
        terra_world::streaming::commit_prop_removal(
            &mut chunks,
            &mut journal,
            &terra_world::streaming::PropRemoveRequest {
                section: owner,
                id: prop.id,
            },
        )
        .unwrap();
        queue.record_prop_edits(&chunks, &journal);
        assert!(!queue.vegetated.contains(&owner));
        assert!(queue.changed.contains(&owner));
    }

    #[test]
    fn streamed_root_space_retries_an_unchanged_prop_batch_without_tombstoning_it() {
        let owner = SectionPos::default();
        let root_section = SectionPos::new(0, 1, 0);
        let prop = WorldProp {
            id: terra_world::props::PropId(119),
            kind: PropKind::Grass,
            position: [8.5, 16.0, 8.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 64,
            section: owner,
            removed: false,
        };
        let lower = WorldProp {
            id: terra_world::props::PropId(120),
            position: [6.5, 1.0, 6.5],
            ..prop
        };
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        data.set_block([8, 15, 8], 3).unwrap();
        data.set_block([6, 0, 6], 3).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![prop, lower],
            },
        );
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(chunks)
            .init_resource::<EditJournal>()
            .init_resource::<GameConfig>()
            .insert_resource(WorldTerrain(
                TerraGenerator::new(Default::default()).unwrap(),
            ))
            .insert_resource(ChunkFocus([8.0; 3]))
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<PropMaterial>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<BlockAssets>()
            .init_resource::<PropTemplates>()
            .init_resource::<PropBatches>()
            .init_resource::<PropJobs>()
            .init_resource::<PropRenderStatus>()
            .init_resource::<PropGrowthStatus>()
            .init_resource::<crate::transitions::LodSwapStatus>()
            .add_message::<SectionPublished>()
            .add_systems(Update, dispatch);
        app.world_mut().resource_mut::<BlockAssets>().templates =
            Some(Arc::new(Default::default()));
        app.world_mut().resource_mut::<PropTemplates>().geometry = Some(Arc::new(HashMap::new()));
        app.world_mut().resource_mut::<PropTemplates>().material = Some(Handle::default());
        app.update();
        let first = app
            .world_mut()
            .resource_mut::<PropJobs>()
            .0
            .remove(&owner)
            .unwrap();
        assert_eq!(first.growth.len(), 1);
        assert!(first.growth.contains_key(&lower.id));
        let entering = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Inherited))
            .id();
        crate::transitions::start_test_fade(app.world_mut(), entering);
        app.world_mut().resource_mut::<PropBatches>().0.insert(
            owner,
            RenderedProps {
                density: 1.0,
                entity: Some(entering),
                meshes: Vec::new(),
                terrain_epoch: 0,
                prop_epoch: 0,
                appearance_epoch: 0,
                level: DetailLevel::Near,
                count: 1,
                growth: first.growth,
                support_pending: true,
            },
        );
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            root_section,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(root_section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.world_mut().write_message(SectionPublished {
            position: root_section,
        });
        app.update();
        let jobs = app.world().resource::<PropJobs>();
        assert!(jobs.0[&owner].growth.contains_key(&prop.id));
        assert!(!app.world().resource::<ChunkStore>().loaded[&owner].props[0].removed);
        assert_eq!(app.world().resource::<EditJournal>().prop_epoch(owner), 0);
        app.world_mut()
            .resource_mut::<PropJobs>()
            .0
            .get_mut(&owner)
            .unwrap()
            .ready = Some((
            Ok((
                vec![MeshBatch {
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    normals: vec![[0.0, 0.0, 1.0]; 3],
                    colors: vec![[1.0; 4]; 3],
                    uvs: vec![[0.0; 2]; 3],
                    indices: vec![0, 1, 2],
                    ..Default::default()
                }],
                2,
            )),
            0.0,
        ));
        app.add_systems(Update, upload.before(dispatch));
        app.update();
        let rendered = &app.world().resource::<PropBatches>().0[&owner];
        assert_eq!(
            rendered.count, 2,
            "An entering partial batch must accept newly supported props"
        );
        assert!(rendered.growth.contains_key(&prop.id));
        assert_ne!(rendered.entity, Some(entering));
    }

    #[test]
    fn bounded_selection_replaces_a_large_backlog_in_one_pass_without_duplicates() {
        let sections: Vec<_> = (0..48_000).map(|x| SectionPos::new(x, 0, -1)).collect();
        let mut queue = PropQueue::default();
        for &position in &sections {
            queue.positions.push((position, DetailLevel::Far, 0, 0));
            queue.select(position, false);
        }
        let urgent = *sections.last().unwrap();
        queue.select(urgent, true);
        assert_eq!(queue.selection.front(), Some(&urgent));
        let mut selected = HashSet::new();
        let mut replacements = Vec::new();
        for index in 0..96 {
            let position = queue.next_selection().unwrap();
            if index == 0 {
                assert_eq!(position, urgent);
            }
            assert!(selected.insert(position));
            if position.x % 3 != 0 {
                replacements.push((position, DetailLevel::Middle, 17, 23));
            }
        }
        let mut expected = queue.positions.clone();
        expected.retain(|(position, ..)| !selected.contains(position));
        expected.extend(replacements.iter().copied());
        let focus = [8.0; 3];
        expected.sort_unstable_by_key(|(p, _, _, _)| {
            std::cmp::Reverse((distance_squared(*p, focus) as u64, p.y, p.z, p.x))
        });
        queue.replace_selected(&selected, replacements);
        assert_eq!(queue.selection_prune_visits, sections.len());
        queue.prioritize(focus);
        assert_eq!(queue.positions, expected);
        assert_eq!(
            queue
                .positions
                .iter()
                .map(|(position, ..)| *position)
                .collect::<HashSet<_>>()
                .len(),
            queue.positions.len()
        );
        assert_eq!(queue.selection.len(), sections.len() - selected.len());
        assert_eq!(queue.selecting.len(), queue.selection.len());
        assert!(
            queue
                .selection
                .iter()
                .all(|position| !selected.contains(position))
        );
        queue.replace_selected(&HashSet::new(), Vec::new());
        assert_eq!(queue.selection_prune_visits, 0);
        assert_eq!(queue.positions, expected);
    }

    #[test]
    fn repeated_focus_sweeps_are_deduplicated_fair_and_edits_take_priority() {
        let sections: Vec<_> = (0..4096).map(|x| SectionPos::new(x, 0, -1)).collect();
        let mut queue = PropQueue::default();
        for &p in &sections {
            queue.select(p, false);
        }
        let edit = *sections.last().unwrap();
        queue.select(edit, true);
        assert_eq!(queue.next_selection(), Some(edit));
        let mut visited = HashSet::from([edit]);
        for _ in 0..43 {
            // A new camera bucket cannot restart the old sweep or duplicate it.
            for &p in sections.iter().rev() {
                queue.select(p, false);
            }
            assert_eq!(queue.selection.len(), sections.len());
            assert_eq!(queue.selecting.len(), sections.len());
            for _ in 0..96 {
                visited.insert(queue.next_selection().unwrap());
            }
        }
        assert_eq!(visited.len(), sections.len());
        while queue.next_selection().is_some() {}
        assert!(queue.selecting.is_empty());
        assert!(queue.selection.is_empty());
    }

    #[derive(Resource, Default)]
    struct WaterObservation(bool, bool);

    #[test]
    fn water_only_publications_are_not_untracked_terrain_or_full_vegetation_lod_sweeps() {
        let mut app = App::new();
        app.init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<GameConfig>()
            .insert_resource(WorldTerrain(
                TerraGenerator::new(Default::default()).unwrap(),
            ))
            .insert_resource(ChunkFocus([0.0; 3]))
            .init_resource::<WaterObservation>()
            .add_message::<WaterPublished>()
            .add_systems(
                Update,
                |world: PropWorld,
                 mut reader: Local<PropChangeReader>,
                 mut result: ResMut<WaterObservation>| {
                    let changes = reader.read(&world);
                    result.0 = changes.water;
                    result.1 = world.chunks.is_changed() && !changes.any();
                },
            );
        app.update();
        app.world_mut().resource_mut::<ChunkStore>().set_changed();
        app.world_mut().write_message(WaterPublished {
            position: SectionPos::new(-1, 0, 2),
        });
        app.update();
        let result = app.world().resource::<WaterObservation>();
        assert!(result.0);
        assert!(!result.1);
    }
}

#[cfg(test)]
mod growth_tests {
    use super::*;

    #[test]
    fn packed_props_are_default_and_only_smoke_can_restore_unpacked_near_layouts() {
        for smoke in [false, true] {
            for unpacked in [false, true] {
                assert!(packed_prop_policy(DetailLevel::Far, smoke, unpacked));
                for level in [DetailLevel::Near, DetailLevel::Middle] {
                    assert_eq!(
                        packed_prop_policy(level, smoke, unpacked),
                        !smoke || !unpacked
                    );
                }
            }
        }
    }

    #[test]
    fn roof_and_water_exposure_masks_cannot_change_prop_vertices_or_growth() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut templates = HashMap::new();
        for kind in [PropKind::Grass, PropKind::Flower, PropKind::Wheat] {
            for variant in 0..2 {
                for ((_, _, level), template) in self::templates() {
                    templates.insert((kind as u8, variant, level), template);
                }
            }
        }
        let props = [
            sprout(1, 0.65, 64),
            WorldProp {
                kind: PropKind::Flower,
                ..sprout(2, 0.8, 0)
            },
            WorldProp {
                kind: PropKind::Wheat,
                variant: 1,
                ..sprout(3, 1.0, 0)
            },
        ];
        let growth = HashMap::from([
            (
                props[0].id,
                Pose {
                    ratio: 0.45,
                    start: 12.0,
                    ..default()
                },
            ),
            (
                props[2].id,
                Pose {
                    ratio: 0.7,
                    start: 9.0,
                    previous_variant: Some(0),
                    ..default()
                },
            ),
        ]);
        let conditions = HashMap::from([(
            [8, 0, 8],
            terra_world::environment::SurfaceCondition {
                wetness: 0.6,
                charred: 0.25,
                ..default()
            },
        )]);
        let dry = crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [0; 256],
            snow_roofs: [0; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        let submerged = crate::weather::surfaces::exposure::Exposure {
            natural: None,
            roofs: [128; 256],
            snow_roofs: [180; 256],
            coast: [42.0; 256],
            coast_vertices: [42.0; 289],
            water: [200.0; 256],
        };
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            for packed in [false, true] {
                let compose = |exposure| {
                    compose_with_exposure_mode(
                        &props,
                        SectionPos::default(),
                        level,
                        &templates,
                        &terrain,
                        &conditions,
                        &growth,
                        exposure,
                        packed,
                    )
                    .unwrap()
                };
                let no_mask = compose(None);
                assert_eq!(compose(Some(&dry)), no_mask);
                assert_eq!(compose(Some(&submerged)), no_mask);
                assert_eq!(no_mask.1, props.len());
                assert!(!no_mask.0.is_empty());
                assert!(
                    no_mask
                        .0
                        .iter()
                        .any(|batch| !batch.growth_ranges.is_empty())
                );
            }
        }
    }

    #[test]
    fn watched_empty_sections_keep_direct_prop_and_surface_changes() {
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
        let mut journal = EditJournal::default();
        let mut state = PropAppearanceState::default();
        state.watch(position, &chunks, &terrain, &journal);
        assert_eq!(state.sections[&[0, 0]], HashSet::from([position]));
        journal.place_prop(sprout(9877, 1.0, 0)).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
        state.refresh(&chunks, &terrain, &journal);
        assert!(!state.non_fluid_changed);
        let before = journal.appearance_epoch(position);
        journal
            .set_surface_condition(
                [8, 0, 8],
                terra_world::environment::SurfaceCondition::new(0.6, 0.8).unwrap(),
            )
            .unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
        assert_ne!(journal.appearance_epoch(position), before);
        journal
            .remove_prop(position, terra_world::props::PropId(9877))
            .unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(state.non_fluid_changed);
    }

    #[test]
    fn distant_roofs_cannot_change_prop_surface_or_geometry_epochs() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut state = PropAppearanceState::default();
        let ground = SectionPos::default();
        state.watch(ground, &chunks, &terrain, &journal);
        state.refresh(&chunks, &terrain, &journal);
        let geometry_epoch = journal.mesh_epoch(ground);
        let appearance_epoch = journal.appearance_epoch(ground);
        for material in [3, 0] {
            journal.record([3, 35, 3], material).unwrap();
            state.refresh(&chunks, &terrain, &journal);
            assert!(state.non_fluid_changed);
            assert_eq!(journal.mesh_epoch(ground), geometry_epoch);
            assert_eq!(journal.appearance_epoch(ground), appearance_epoch);
        }
    }

    #[test]
    fn fluid_only_changes_do_not_recompose_unchanged_prop_geometry() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut state = PropAppearanceState::default();
        let position = SectionPos::default();
        state.watch(position, &chunks, &terrain, &journal);
        state.refresh(&chunks, &terrain, &journal);
        let geometry_epoch = journal.mesh_epoch(position);
        let appearance_epoch = journal.appearance_epoch(position);
        for amount in [4, 5, 0] {
            journal
                .set_fluid(
                    [3, 1, 3],
                    terra_world::water::FluidCell {
                        amount,
                        source: false,
                        flow: [0; 2],
                    },
                )
                .unwrap();
            state.refresh(&chunks, &terrain, &journal);
            assert!(!state.non_fluid_changed);
            assert_eq!(state.sections[&[0, 0]], HashSet::from([position]));
            assert_eq!(journal.mesh_epoch(position), geometry_epoch);
            assert_eq!(journal.appearance_epoch(position), appearance_epoch);
        }
    }

    #[test]
    fn prop_change_tracking_ignores_travel_history_and_releases_on_unload() {
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
        }
        journal.record([3, 35, 3], 3).unwrap();
        let mut state = PropAppearanceState::default();
        state.watch(position, &chunks, &terrain, &journal);
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.sampled_sections, 31);
        assert_eq!(state.non_fluid.len(), 1);
        journal.record([20_003, 35, 3], 3).unwrap();
        state.refresh(&chunks, &terrain, &journal);
        assert!(!state.non_fluid_changed);
        assert_eq!(state.sampled_sections, 31);
        chunks.loaded.remove(&position);
        state.retain_loaded(&chunks);
        assert!(state.sections.is_empty() && state.non_fluid.is_empty());
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.sampled_sections, 0);
        assert!(state.non_fluid.is_empty());
    }

    #[test]
    fn sparse_journal_uses_the_smaller_history_domain_for_loaded_prop_columns() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        journal.record([3, 35, 3], 3).unwrap();
        journal.record([20_003, 35, 3], 3).unwrap();
        let mut state = PropAppearanceState::default();
        for x in 0..100 {
            state
                .sections
                .insert([x, 0], HashSet::from([SectionPos::new(x, 0, 0)]));
        }
        state.refresh(&chunks, &terrain, &journal);
        assert_eq!(state.sampled_sections, 1);
        assert_eq!(state.non_fluid.len(), 1);
        assert_eq!(state.sections.len(), 100);
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
    fn crop_crossfade_batches_two_stages_then_releases_the_old_geometry_without_extra_entities() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut crop_templates = HashMap::new();
        for stage in 0..2 {
            for ((_, _, level), template) in templates() {
                crop_templates.insert((PropKind::Wheat as u8, stage, level), template);
            }
        }
        let prop = WorldProp {
            kind: PropKind::Wheat,
            variant: 1,
            ..sprout(8, 1.0, 0)
        };
        let pose = Pose {
            scale: growth::extent(&prop),
            ratio: 0.7,
            start: 10.0,
            variant: 1,
            previous_variant: Some(0),
            pending: false,
        };
        let fading = HashMap::from([(prop.id, pose)]);
        let settled = growth::snapshot(&[prop], Some(&fading), 13.0);
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let (mut batches, count) = compose_with_growth(
                &[prop],
                prop.section,
                level,
                &crop_templates,
                &terrain,
                &Default::default(),
                &fading,
            )
            .unwrap();
            assert_eq!(count, 1);
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].positions.len(), 6);
            patch_growth(&mut batches, &fading);
            assert!(
                batches[0].rooted_wind[..3]
                    .iter()
                    .all(|uv| *uv == [10.0, -1.7])
            );
            assert!(
                batches[0].rooted_wind[3..]
                    .iter()
                    .all(|uv| *uv == [10.0, -3.0])
            );
            let (batches, count) = compose_with_growth(
                &[prop],
                prop.section,
                level,
                &crop_templates,
                &terrain,
                &Default::default(),
                &settled,
            )
            .unwrap();
            assert_eq!(count, 1);
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].positions.len(), 3);
        }
        let rendered = RenderedProps {
            density: 1.0,
            entity: None,
            meshes: Vec::new(),
            terrain_epoch: 0,
            prop_epoch: 0,
            appearance_epoch: 0,
            level: DetailLevel::Near,
            count: 1,
            growth: fading,
            support_pending: false,
        };
        assert!(!rendered.expired_crop_fade(11.0));
        assert!(rendered.expired_crop_fade(13.0));
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
                ..Default::default()
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
                let vertices = batches
                    .iter()
                    .map(|batch| batch.positions.len())
                    .sum::<usize>();
                if vertices <= batch_plan::VERTEX_LIMIT {
                    assert_eq!(batches.len(), 1, "small material batches remain unsplit");
                }
                for batch in &mut batches {
                    assert!(batch.positions.len() <= batch_plan::VERTEX_LIMIT);
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
        }
        assert_eq!(new_index_bytes * 2, old_index_bytes);
        eprintln!(
            "O05 seven species, three LODs, 16 props each: index bytes {old_index_bytes} -> {new_index_bytes}; unchanged represented prop count and triangles"
        );
    }

    #[test]
    fn global_snow_reserves_never_hide_or_flatten_grass_before_local_layers_accumulate() {
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
                assert!(batch.colors.iter().all(|color| color[3] == 1.0));
                if level == DetailLevel::Far {
                    assert!(batch.uvs.iter().all(|uv| uv[0] == 0.0));
                    crate::foliage::packing::pack(batch);
                    assert!(batch.packed_colors.iter().all(|color| color[3] == 255));
                }
            }
        }
    }

    #[test]
    fn seasonal_flower_ranks_use_existing_alpha_without_geometry_or_lod_dependent_reselection() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let templates: Templates = templates()
            .into_iter()
            .map(|((_, variant, level), template)| {
                ((PropKind::Flower as u8, variant, level), template)
            })
            .collect();
        let mut flower = sprout(712, 0.65, 0);
        flower.kind = PropKind::Flower;
        let rank = terra_world::props::seasonal::flower_rank(&flower);
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let (mut batches, count) = compose_with_growth(
                &[flower],
                SectionPos::default(),
                level,
                &templates,
                &terrain,
                &Default::default(),
                &HashMap::new(),
            )
            .unwrap();
            assert_eq!(count, 1);
            assert_eq!(batches.len(), 1, "flowers retain section batching");
            for batch in &mut batches {
                assert!(batch.colors.iter().all(|color| color[3] == rank));
                assert_eq!(batch.growth_anchors.len(), batch.positions.len());
                assert!(
                    batch
                        .growth_anchors
                        .iter()
                        .all(|anchor| *anchor == flower.position.map(|v| v as f32))
                );
                let vertices = batch.positions.len();
                crate::foliage::packing::pack(batch);
                assert_eq!(batch.positions.len(), vertices);
                assert!(
                    batch
                        .packed_colors
                        .iter()
                        .all(|color| f32::from(color[3]) / 255.0 == rank)
                );
            }
        }
        let vertex = include_str!("../../../../assets/shaders/terra_props.wesl");
        let fragment = include_str!("../../../../assets/shaders/terra_prop_fragment.wesl");
        assert!(vertex.contains("terra_weather.flowers[rank / 4u][rank % 4u]"));
        assert!(vertex.contains("/ 10.0, 0.0, 1.0)"));
        assert!(vertex.contains("vec3<f32>(0.40, 0.19, 0.055)"));
        assert!(vertex.contains("smoothstep(0.0, 0.45, progress)"));
        assert!(!vertex.contains("fn prop_snow"));
        assert!(fragment.contains("threshold >= input.color.a"));
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
                    ..Default::default()
                },
            ),
            (
                props[1].id,
                Pose {
                    scale: props[1].visual_scale(),
                    ratio: 1.0,
                    start: 0.0,
                    pending: false,
                    ..Default::default()
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
            ..Default::default()
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
                support_pending: false,
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
                    support_pending: false,
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
                support_pending: false,
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
