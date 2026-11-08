use avian3d::prelude::*;
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use terra_voxel::{
    BlockSection, SectionPos, global_to_section,
    meshing::{PADDED_VOLUME, mesh_section},
};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkFoci, ChunkFocus, ChunkStore, StreamingSet},
};

const BUILD_RADIUS: i64 = 2;
const RETAIN_RADIUS: i64 = 3;
const MAX_JOBS: usize = 2;
#[path = "terrain/belts.rs"]
mod belt_profiles;

#[derive(Default)]
struct ModelGeometry {
    boxes: Vec<terra_core::block_shapes::ShapeBox>,
    triangles: Vec<[[f32; 3]; 3]>,
}

#[derive(Component, Debug)]
pub struct TerrainCollider(pub SectionPos);

struct ReadySection {
    entity: Option<Entity>,
    epoch: u64,
    profile: Option<[u64; 4]>,
    plain: Option<PlainCollision>,
}

struct PlainCollision {
    revision: u64,
    blocks: Vec<u16>,
    properties: terra_core::block_properties::BlockPropertiesCatalog,
}

fn plain_current(
    ready: &ReadySection,
    position: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    let Some(plain) = &ready.plain else {
        return false;
    };
    if plain.revision != journal.revision(position)
        || chunks.loaded.get(&position).is_none_or(|s| {
            s.mesh_epoch > journal.mesh_epoch(position)
                || s.data.blocks() != plain.blocks.as_slice()
        })
    {
        return false;
    }
    if ready.epoch == journal.mesh_epoch(position) {
        return true;
    }
    plain_halo_current(plain, position, chunks, journal)
}

fn plain_halo_current(
    plain: &PlainCollision,
    position: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    let Ok(origin) = position.origin() else {
        return false;
    };
    // Full cubes do not depend on neighbour faces. Reuse only while no
    // authored shape can extend into this section, including pending replicas.
    for x in -1..=16 {
        for y in -1..=16 {
            for z in -1..=16 {
                if (0..16).contains(&x) && (0..16).contains(&y) && (0..16).contains(&z) {
                    continue;
                }
                let p = [origin[0] + x, origin[1] + y, origin[2] + z];
                if journal
                    .block(p)
                    .or_else(|| chunks.block_at(p))
                    .is_some_and(|m| !plain.properties.shape(m).is_empty())
                {
                    return false;
                }
            }
        }
    }
    true
}

// A remote cube edit must not pause movement throughout its entire 16³ section.
// The old collider is safe within the swept body bounds while every changed
// cell stays outside them. Authored models can extend beyond their cell, so
// they always wait for the replacement collider, including halo changes.
fn plain_region_current(
    ready: &ReadySection,
    position: SectionPos,
    minimum: Vec3,
    maximum: Vec3,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    let _scope = terra_core::profiling::ScopeTimer::new("collision.readiness.region");
    let Some(plain) = &ready.plain else {
        return false;
    };
    if ready.profile.is_some() {
        return false;
    }
    let Some(section) = chunks.loaded.get(&position) else {
        return false;
    };
    let Ok(origin) = position.origin() else {
        return false;
    };
    if section.mesh_epoch > journal.mesh_epoch(position) {
        return false;
    }
    for (index, (&old, &loaded)) in plain.blocks.iter().zip(section.data.blocks()).enumerate() {
        let cell = [
            origin[0] + (index % 16) as i64,
            origin[1] + (index / 16 % 16) as i64,
            origin[2] + (index / 256) as i64,
        ];
        let current = journal.block(cell).unwrap_or(loaded);
        if old == current {
            continue;
        }
        if !plain.properties.shape(old).is_empty() || !plain.properties.shape(current).is_empty() {
            return false;
        }
        if (0..3).all(|axis| {
            maximum[axis] + 0.05 >= cell[axis] as f32
                && minimum[axis] - 0.05 <= cell[axis] as f32 + 1.0
        }) {
            return false;
        }
    }
    plain_halo_current(plain, position, chunks, journal)
}

fn profile_snapshot(position: SectionPos, chunks: &ChunkStore, journal: &EditJournal) -> [u64; 4] {
    [-2, -1, 1, 2].map(|dy| {
        position.offset([0, dy, 0]).map_or(0, |p| {
            journal
                .revision(p)
                .wrapping_mul(2)
                .wrapping_add(u64::from(chunks.loaded.contains_key(&p)))
        })
    })
}

fn profile_current(
    profile: Option<[u64; 4]>,
    position: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    profile.is_none_or(|profile| profile == profile_snapshot(position, chunks, journal))
}

#[derive(Resource, Default)]
pub struct TerrainCollisionState {
    sections: HashMap<SectionPos, ReadySection>,
    interest: HashSet<SectionPos>,
    pub active_colliders: usize,
    pub pending_jobs: usize,
    pub completed_jobs: usize,
    pub error: Option<String>,
}

impl TerrainCollisionState {
    pub(crate) fn in_dispatch_interest(&self, section: SectionPos) -> bool {
        self.interest.contains(&section)
    }
    /// Empty sections count as ready too. Never walk into ungenerated or stale collision data.
    pub fn ready_for_aabb(
        &self,
        minimum: Vec3,
        maximum: Vec3,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) -> bool {
        if !minimum.is_finite() || !maximum.is_finite() {
            return false;
        }
        let low = global_to_section(minimum.to_array().map(|v| v.floor() as i64)).0;
        let high = global_to_section(maximum.to_array().map(|v| v.floor() as i64)).0;
        for z in low.z..=high.z {
            for y in low.y..=high.y {
                for x in low.x..=high.x {
                    let position = SectionPos::new(x, y, z);
                    let epoch = journal.mesh_epoch(position);
                    if !collision_inputs_known(position, chunks) {
                        return false;
                    }
                    if self.sections.get(&position).is_some_and(|ready| {
                        plain_current(ready, position, chunks, journal)
                            || plain_region_current(
                                ready, position, minimum, maximum, chunks, journal,
                            )
                    }) {
                        continue;
                    }
                    if self.sections.get(&position).is_none_or(|ready| {
                        ready.epoch != epoch
                            || !profile_current(ready.profile, position, chunks, journal)
                    }) || chunks
                        .loaded
                        .get(&position)
                        .is_none_or(|section| section.mesh_epoch > epoch)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

struct ColliderJob {
    data: Arc<BlockSection>,
    plain: Option<PlainCollision>,
    epoch: u64,
    profile: Option<[u64; 4]>,
    task: Task<Result<ColliderOutput, String>>,
}

struct ColliderOutput {
    collider: Option<Collider>,
    modeled: bool,
}

/// Copy only the finite voxel halo; meshes, props, fluid patches and the rest of
/// the loaded world never cross into a collider worker.
fn geometry_chunks(center: SectionPos, chunks: &ChunkStore) -> Result<ChunkStore, String> {
    let mut snapshot = ChunkStore::default();
    for x in -1..=1 {
        for y in -2..=2 {
            for z in -1..=1 {
                let position = center
                    .offset([x, y, z])
                    .map_err(|error| error.to_string())?;
                if let Some(source) = chunks.loaded.get(&position) {
                    snapshot.loaded.insert(
                        position,
                        terra_world::streaming::LoadedSection {
                            data: source.data.clone(),
                            mesh: None,
                            mesh_epoch: source.mesh_epoch,
                            water_epoch: 0,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                }
            }
        }
    }
    Ok(snapshot)
}

// Connected shapes read the one-section halo; mineral profiles also consult
// two vertical neighbors. Unknown generated bases must never become empty space.
fn collision_inputs_known(position: SectionPos, chunks: &ChunkStore) -> bool {
    if !chunks.voxel_data_known(position) {
        return false;
    }
    for x in -1..=1 {
        for y in -2..=2 {
            for z in -1..=1 {
                if position
                    .offset([x, y, z])
                    .is_ok_and(|neighbor| chunks.voxel_base_pending(neighbor))
                {
                    return false;
                }
            }
        }
    }
    true
}

fn collision_snapshot(
    position: SectionPos,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Result<Arc<BlockSection>, String> {
    if !collision_inputs_known(position, chunks) {
        return Err("Collision voxel base is not known".into());
    }
    chunks.confirmed_voxels(position, journal).map(Arc::new)
}
#[derive(Resource, Default)]
struct ColliderJobs(HashMap<SectionPos, ColliderJob>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<TerrainCollisionState>()
        .init_resource::<ColliderJobs>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .add_systems(
            RunFixedMainLoop,
            // A slow frame can contain several fixed steps. Activate completed,
            // validated jobs before that loop, rather than blocking every step
            // on a collider that would otherwise arrive in the later Update.
            (receive, ApplyDeferred)
                .chain()
                .in_set(bevy::app::RunFixedMainLoopSystems::BeforeFixedMainLoop),
        )
        .add_systems(
            Update,
            (receive, dispatch).chain().after(StreamingSet::Receive),
        );
}

fn nearby(position: SectionPos, center: SectionPos, radius: i64) -> bool {
    (position.x - center.x).abs() <= radius
        && (position.y - center.y).abs() <= radius
        && (position.z - center.z).abs() <= radius
}

fn centers(focus: &ChunkFocus, foci: Option<&ChunkFoci>) -> Vec<SectionPos> {
    let points = foci
        .and_then(|f| f.0.as_ref())
        .map_or_else(|| vec![focus.0], Clone::clone);
    points
        .into_iter()
        .filter(|p| {
            p.iter()
                .all(|v| v.is_finite() && v.abs() < (1_i64 << 40) as f64 - 32.0)
        })
        .map(|p| global_to_section(p.map(|v| v.floor() as i64)).0)
        .collect()
}

#[derive(SystemParam)]
struct CollisionFocus<'w> {
    focus: Option<Res<'w, ChunkFocus>>,
    foci: Option<Res<'w, ChunkFoci>>,
}
impl CollisionFocus<'_> {
    fn centers(&self) -> Option<Vec<SectionPos>> {
        self.focus
            .as_ref()
            .map(|focus| centers(focus, self.foci.as_deref()))
    }
}

fn dispatch(
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    focus: CollisionFocus,
    mut state: ResMut<TerrainCollisionState>,
    mut jobs: ResMut<ColliderJobs>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("collision.dispatch");
    let Some(centers) = focus.centers() else {
        state.interest.clear();
        return;
    };
    jobs.0.retain(|position, _| {
        chunks.loaded.contains_key(position)
            && centers
                .iter()
                .any(|&center| nearby(*position, center, RETAIN_RADIUS))
    });
    let positions: Vec<_> = centers
        .iter()
        .flat_map(|&center| {
            (-BUILD_RADIUS..=BUILD_RADIUS).flat_map(move |z| {
                (-BUILD_RADIUS..=BUILD_RADIUS).flat_map(move |y| {
                    (-BUILD_RADIUS..=BUILD_RADIUS)
                        .filter_map(move |x| center.offset([x, y, z]).ok())
                })
            })
        })
        .collect();
    state.interest.clear();
    state.interest.extend(positions.iter().copied());
    let mut candidates: Vec<_> = positions
        .into_iter()
        .filter_map(|position| {
            let section = chunks.loaded.get(&position)?;
            let epoch = journal.mesh_epoch(position);
            if !collision_inputs_known(position, &chunks) || section.mesh_epoch > epoch {
                return None;
            }
            if state
                .sections
                .get(&position)
                .is_some_and(|ready| plain_current(ready, position, &chunks, &journal))
            {
                state.sections.get_mut(&position).unwrap().epoch = epoch;
                return None;
            }
            (state.sections.get(&position).is_none_or(|ready| {
                ready.epoch != epoch || !profile_current(ready.profile, position, &chunks, &journal)
            }) && !jobs.0.contains_key(&position))
            .then_some(position)
        })
        .collect();
    candidates.sort_by_key(|position| {
        let distance = centers
            .iter()
            .map(|center| {
                let delta = [
                    position.x - center.x,
                    position.y - center.y,
                    position.z - center.z,
                ];
                delta.iter().map(|v| v * v).sum::<i64>()
            })
            .min()
            .unwrap_or(0);
        (distance, position.y, position.z, position.x)
    });
    candidates.dedup();
    for position in candidates {
        if jobs.0.len() >= MAX_JOBS {
            break;
        }
        let data = match collision_snapshot(position, &chunks, &journal) {
            Ok(data) => data,
            Err(error) => {
                state.error = Some(error);
                continue;
            }
        };
        let metadata = match journal.geometry_snapshot(position) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                state.error = Some(error.to_string());
                continue;
            }
        };
        let voxels = match geometry_chunks(position, &chunks) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                state.error = Some(error);
                continue;
            }
        };
        let masked: Vec<_> = properties
            .1
            .keys()
            .copied()
            .chain([
                28, 29, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 99, 100, 141, 142, 143,
                144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159,
                160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 210, 211, 212, 213, 214, 215,
                216, 217,
            ])
            .chain(terra_core::locomotive_parts::FIRST..=terra_core::locomotive_parts::LAST)
            .collect();
        let worker_properties = (*properties).clone();
        jobs.0.insert(
            position,
            ColliderJob {
                data: data.clone(),
                plain: data
                    .blocks()
                    .iter()
                    .all(|&m| properties.shape(m).is_empty())
                    .then(|| PlainCollision {
                        revision: journal.revision(position),
                        blocks: data.blocks().to_vec(),
                        properties: properties.clone(),
                    }),
                epoch: journal.mesh_epoch(position),
                profile: data
                    .blocks()
                    .iter()
                    .any(|m| terra_world::segments::is_crystal(*m))
                    .then(|| profile_snapshot(position, &chunks, &journal)),
                task: AsyncComputeTaskPool::get().spawn(async move {
                    let _scope = terra_core::profiling::ScopeTimer::new("collision.build_geometry");
                    let shapes = model_geometry(&data, &voxels, &metadata, &worker_properties)?;
                    let modeled = !shapes.boxes.is_empty() || !shapes.triangles.is_empty();
                    let collider = build_collider_with_geometry(
                        &data,
                        &shapes.boxes,
                        &shapes.triangles,
                        &masked,
                    )?;
                    Ok(ColliderOutput { collider, modeled })
                }),
            },
        );
    }
    state.pending_jobs = jobs.0.len();
}

#[cfg(test)]
fn model_shapes(
    data: &BlockSection,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<Vec<terra_core::block_shapes::ShapeBox>, String> {
    let original = model_geometry(data, chunks, journal, properties)?;
    let voxels = geometry_chunks(data.position(), chunks)?;
    let metadata = journal
        .geometry_snapshot(data.position())
        .map_err(|error| error.to_string())?;
    let worker = model_geometry(data, &voxels, &metadata, properties)?;
    assert_eq!(
        original.boxes, worker.boxes,
        "background metadata must retain authored contact volumes"
    );
    assert_eq!(
        original.triangles, worker.triangles,
        "background metadata must retain clipped belt profiles"
    );
    Ok(original.boxes)
}

fn model_geometry(
    data: &BlockSection,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<ModelGeometry, String> {
    let block_at = |position| {
        journal
            .block(position)
            .or_else(|| chunks.block_at(position))
    };
    let origin = data
        .position()
        .origin()
        .map_err(|error| error.to_string())?;
    let mut shapes = Vec::new();
    let mut triangles = Vec::new();
    let mut profiles = HashSet::new();
    let mut owners = HashSet::new();
    let mut unique = HashSet::new();
    let mut append = |global, material| {
        if properties.shape(material).is_empty()
            || terra_voxel::MaterialId::from_raw(material)
                .is_some_and(|material| !material.is_solid())
        {
            return;
        }
        let selected = journal.object_state(global).unwrap_or_default();
        // Rail geometry is already clipped per occupied cell; deduplicating by
        // its module anchor would discard every following physical slice.
        let owner = if selected.secondary && !terra_core::rails::is_rail(material) {
            selected.peer.unwrap_or(global)
        } else {
            global
        };
        if !owners.insert(owner) {
            return;
        }
        // Every physical section owns its clipped share, including a secondary
        // half whose primary collider has not arrived. A pair is visited once
        // within each section, never duplicated for its two occupied cells.
        let profile = matches!(material, 144..=146 | 148)
            .then(|| terra_world::conveyors::segment_at(journal, global))
            .flatten()
            .filter(|segment| segment.span.linked && segment.span.step[1] != 0);
        if let Some(segment) = profile
            && profiles.insert(segment.span.controller)
        {
            for triangle in terra_world::interaction_shapes::conveyor_triangles(segment.span) {
                belt_profiles::append_clipped(&mut triangles, triangle, origin);
            }
        }
        let mut visit = |mut bounds: terra_core::block_shapes::ShapeBox| {
            for (axis, value) in origin.iter().enumerate() {
                bounds.minimum[axis] = bounds.minimum[axis].max(*value as f64) - *value as f64;
                bounds.maximum[axis] =
                    bounds.maximum[axis].min(*value as f64 + 16.0) - *value as f64;
            }
            if (0..3).all(|axis| bounds.minimum[axis] < bounds.maximum[axis]) {
                let key: [u64; 6] = std::array::from_fn(|axis| {
                    if axis < 3 {
                        bounds.minimum[axis].to_bits()
                    } else {
                        bounds.maximum[axis - 3].to_bits()
                    }
                });
                if unique.insert(key) {
                    shapes.push(bounds);
                }
            }
        };
        if profile.is_some() {
            terra_world::interaction_shapes::visit_belt_attachment_boxes_with(
                journal, properties, global, block_at, &mut visit,
            );
        } else {
            terra_world::interaction_shapes::visit_collision_boxes_with(
                journal, properties, global, block_at, &mut visit,
            );
        }
    };
    for (index, &material) in data.blocks().iter().enumerate() {
        let cell = [index % 16, index / 16 % 16, index / 256];
        let global = std::array::from_fn(|axis| origin[axis] + cell[axis] as i64);
        append(global, material);
    }
    if journal.has_extended_kinetics() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Ok(section) = data.position().offset([dx, dy, dz]) {
                        for owner in journal.extended_kinetics_in_section(section) {
                            if (0..3)
                                .all(|i| owner[i] >= origin[i] - 1 && owner[i] <= origin[i] + 16)
                                && let Some(material) = block_at(owner)
                            {
                                append(owner, material);
                            }
                        }
                    }
                }
            }
        }
    }
    // Crusher hoppers extend 0.23 blocks beyond their two-cell footprint.
    // The section above owns the clipped lip just as the lower section owns its base.
    for x in 0..16 {
        for z in 0..16 {
            let below = [origin[0] + x, origin[1] - 1, origin[2] + z];
            if block_at(below) == Some(terra_core::crusher::MATERIAL) {
                append(below, terra_core::crusher::MATERIAL);
            }
        }
    }
    // Hinges and wide bogie wheels can enter a neighboring section without
    // owning a voxel there. Each section keeps its clipped physical share.
    for axis in [0, 2] {
        for boundary in [-1, 16] {
            for y in 0..16 {
                for tangent in -1..=16 {
                    let mut p = origin;
                    p[axis] += boundary;
                    p[1] += y;
                    p[2 - axis] += tangent;
                    if let Some(material) = block_at(p)
                        && (terra_world::objects::is_opening(material)
                            || material == terra_core::train_parts::BOGIE)
                    {
                        append(p, material);
                    }
                }
            }
        }
    }
    Ok(ModelGeometry {
        boxes: shapes,
        triangles,
    })
}

fn receive(
    mut commands: Commands,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    focus: CollisionFocus,
    mut state: ResMut<TerrainCollisionState>,
    mut jobs: ResMut<ColliderJobs>,
    sleeping: Query<(Entity, &ColliderAabb), With<Sleeping>>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("collision.receive");
    let Some(centers) = focus.centers() else {
        return;
    };
    state.sections.retain(|position, ready| {
        if chunks.loaded.contains_key(position)
            && centers
                .iter()
                .any(|&center| nearby(*position, center, RETAIN_RADIUS))
        {
            return true;
        }
        if let Some(entity) = ready.entity {
            commands.entity(entity).despawn();
            wake_overlapping(*position, &sleeping, &mut commands);
        }
        false
    });
    let positions: Vec<_> = jobs.0.keys().copied().collect();
    for position in positions {
        let Some(job) = jobs.0.get_mut(&position) else {
            continue;
        };
        let Some(result) = future::block_on(future::poll_once(&mut job.task)) else {
            continue;
        };
        let epoch = job.epoch;
        let expected_blocks = job.data.clone();
        let profile = job.profile;
        let plain = job.plain.take();
        jobs.0.remove(&position);
        if epoch != journal.mesh_epoch(position)
            || !profile_current(profile, position, &chunks, &journal)
            || !collision_snapshot(position, &chunks, &journal)
                .is_ok_and(|current| current.blocks() == expected_blocks.blocks())
            || !centers
                .iter()
                .any(|&center| nearby(position, center, RETAIN_RADIUS))
        {
            continue;
        }
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                state.error = Some(error);
                continue;
            }
        };
        let collider = output.collider;
        let plain = plain.filter(|_| !output.modeled);
        let Ok(origin) = position.origin() else {
            continue;
        };
        let entity = collider.map(|collider| {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let _scope = terra_core::profiling::ScopeTimer::new("collision.activate");
                world.entity_mut(entity).insert((
                    TerrainCollider(position),
                    RigidBody::Static,
                    collider,
                    Friction::new(0.0),
                    Restitution::new(0.0),
                    Position(Vec3::from_array(origin.map(|v| v as f32))),
                    Rotation::IDENTITY,
                    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                ));
            });
            entity
        });
        if let Some(old) = state.sections.insert(
            position,
            ReadySection {
                entity,
                epoch,
                profile,
                // Full voxel collision has no neighbour/mesh-light dependency.
                // Authored models still use the complete halo invalidation path.
                plain,
            },
        ) && let Some(entity) = old.entity
        {
            commands.entity(entity).despawn();
            wake_overlapping(position, &sleeping, &mut commands);
        }
        state.completed_jobs += 1;
    }
    state.active_colliders = state
        .sections
        .values()
        .filter(|ready| ready.entity.is_some())
        .count();
    state.pending_jobs = jobs.0.len();
}

fn wake_overlapping(
    section: SectionPos,
    sleeping: &Query<(Entity, &ColliderAabb), With<Sleeping>>,
    commands: &mut Commands,
) {
    let Ok(origin) = section.origin() else {
        return;
    };
    let low = Vec3::from_array(origin.map(|v| v as f32)) - Vec3::splat(0.05);
    let high = low + Vec3::splat(16.1);
    for (entity, bounds) in sleeping {
        if bounds.max.cmpge(low).all() && bounds.min.cmple(high).all() {
            commands.entity(entity).remove::<Sleeping>();
        }
    }
}

#[cfg(test)]
fn build_collider(section: &BlockSection) -> Result<Option<Collider>, String> {
    build_collider_with_models(section, &[], &[])
}

#[cfg(test)]
fn build_collider_with_models(
    section: &BlockSection,
    shapes: &[terra_core::block_shapes::ShapeBox],
    masked: &[u16],
) -> Result<Option<Collider>, String> {
    build_collider_with_geometry(section, shapes, &[], masked)
}

fn build_collider_with_geometry(
    section: &BlockSection,
    shapes: &[terra_core::block_shapes::ShapeBox],
    triangles: &[[[f32; 3]; 3]],
    masked: &[u16],
) -> Result<Option<Collider>, String> {
    if section.is_empty() && shapes.is_empty() && triangles.is_empty() {
        return Ok(None);
    }
    let mut padded = vec![0; PADDED_VOLUME];
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                // Collision only needs solid/air, not the visual material or render LOD.
                let material = section.blocks()[x + 16 * (y + 16 * z)];
                padded[x + 1 + 18 * (y + 1 + 18 * (z + 1))] = u16::from(
                    !masked.contains(&material)
                        && terra_voxel::MaterialId::from_raw(material)
                            .is_none_or(terra_voxel::MaterialId::is_solid),
                );
            }
        }
    }
    let mesh = mesh_section(&padded).map_err(|error| error.to_string())?;
    let mut positions: Vec<_> = mesh.positions.into_iter().map(Vec3::from_array).collect();
    let mut indices: Vec<_> = mesh
        .indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect();
    for bounds in shapes {
        let base = u32::try_from(positions.len()).map_err(|_| "Model collision index overflow")?;
        for corner in 0..8 {
            positions.push(Vec3::from_array(std::array::from_fn(|axis| {
                if corner & (1 << axis) == 0 {
                    bounds.minimum[axis] as f32
                } else {
                    bounds.maximum[axis] as f32
                }
            })));
        }
        for triangle in [
            [0, 2, 3],
            [0, 3, 1],
            [4, 5, 7],
            [4, 7, 6],
            [0, 4, 6],
            [0, 6, 2],
            [1, 3, 7],
            [1, 7, 5],
            [0, 1, 5],
            [0, 5, 4],
            [2, 6, 7],
            [2, 7, 3],
        ] {
            indices.push(triangle.map(|index| base + index));
        }
    }
    for triangle in triangles {
        let base = u32::try_from(positions.len()).map_err(|_| "Belt collision index overflow")?;
        positions.extend(triangle.map(Vec3::from_array));
        indices.push([base, base + 1, base + 2]);
    }
    if indices.is_empty() {
        return Ok(None);
    }
    Collider::try_trimesh_with_config(positions, indices, TrimeshFlags::FIX_INTERNAL_EDGES)
        .map(Some)
        .map_err(|error| format!("Terrain collider {:?}: {error:?}", section.position()))
}

#[cfg(test)]
mod confirmed_rail_tests;
#[cfg(test)]
mod dispatch_snapshot_tests;
#[cfg(test)]
mod fixed_receive_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod train_profile_tests;
