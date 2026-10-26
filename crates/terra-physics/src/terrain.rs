use avian3d::prelude::*;
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::collections::{HashMap, HashSet};
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

#[derive(Component, Debug)]
pub struct TerrainCollider(pub SectionPos);

struct ReadySection {
    entity: Option<Entity>,
    epoch: u64,
    profile: Option<[u64; 4]>,
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
    pub active_colliders: usize,
    pub pending_jobs: usize,
    pub completed_jobs: usize,
    pub error: Option<String>,
}

impl TerrainCollisionState {
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
                    if self.sections.get(&position).is_none_or(|ready| {
                        ready.epoch != epoch
                            || !profile_current(ready.profile, position, chunks, journal)
                    }) || chunks
                        .loaded
                        .get(&position)
                        .is_none_or(|section| section.mesh_epoch != epoch)
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
    epoch: u64,
    profile: Option<[u64; 4]>,
    task: Task<Result<Option<Collider>, String>>,
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
        return;
    };
    jobs.0.retain(|position, _| {
        chunks.loaded.contains_key(position)
            && centers
                .iter()
                .any(|&center| nearby(*position, center, RETAIN_RADIUS))
    });
    let mut candidates: Vec<_> = centers
        .iter()
        .flat_map(|&center| {
            (-BUILD_RADIUS..=BUILD_RADIUS).flat_map(move |z| {
                (-BUILD_RADIUS..=BUILD_RADIUS).flat_map(move |y| {
                    (-BUILD_RADIUS..=BUILD_RADIUS)
                        .filter_map(move |x| center.offset([x, y, z]).ok())
                })
            })
        })
        .filter_map(|position| {
            let section = chunks.loaded.get(&position)?;
            let epoch = journal.mesh_epoch(position);
            (section.mesh_epoch == epoch
                && state.sections.get(&position).is_none_or(|ready| {
                    ready.epoch != epoch
                        || !profile_current(ready.profile, position, &chunks, &journal)
                })
                && !jobs.0.contains_key(&position))
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
        let section = &chunks.loaded[&position];
        let data = section.data.clone();
        let shapes = match model_shapes(&data, &chunks, &journal, &properties) {
            Ok(shapes) => shapes,
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
                28, 29, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 99, 100,
            ])
            .collect();
        jobs.0.insert(
            position,
            ColliderJob {
                epoch: section.mesh_epoch,
                profile: data
                    .blocks()
                    .iter()
                    .any(|m| terra_world::segments::is_crystal(*m))
                    .then(|| profile_snapshot(position, &chunks, &journal)),
                task: AsyncComputeTaskPool::get()
                    .spawn(async move { build_collider_with_models(&data, &shapes, &masked) }),
            },
        );
    }
    state.pending_jobs = jobs.0.len();
}

fn model_shapes(
    data: &BlockSection,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<Vec<terra_core::block_shapes::ShapeBox>, String> {
    let origin = data
        .position()
        .origin()
        .map_err(|error| error.to_string())?;
    let mut shapes = Vec::new();
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
        let owner = if selected.secondary {
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
        terra_world::interaction_shapes::visit_boxes(
            chunks,
            journal,
            properties,
            global,
            |mut bounds| {
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
            },
        );
    };
    for (index, &material) in data.blocks().iter().enumerate() {
        let cell = [index % 16, index / 16 % 16, index / 256];
        let global = std::array::from_fn(|axis| origin[axis] + cell[axis] as i64);
        append(global, material);
    }
    // Crusher hoppers extend 0.23 blocks beyond their two-cell footprint.
    // The section above owns the clipped lip just as the lower section owns its base.
    for x in 0..16 {
        for z in 0..16 {
            let below = [origin[0] + x, origin[1] - 1, origin[2] + z];
            if chunks.block_at(below) == Some(terra_core::crusher::MATERIAL) {
                append(below, terra_core::crusher::MATERIAL);
            }
        }
    }
    // A neighboring hinge may swing into this section without occupying a voxel here.
    for axis in [0, 2] {
        for boundary in [-1, 16] {
            for y in 0..16 {
                for tangent in -1..=16 {
                    let mut p = origin;
                    p[axis] += boundary;
                    p[1] += y;
                    p[2 - axis] += tangent;
                    if let Some(material) = chunks.block_at(p)
                        && terra_world::objects::is_opening(material)
                    {
                        append(p, material);
                    }
                }
            }
        }
    }
    Ok(shapes)
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
        let profile = job.profile;
        jobs.0.remove(&position);
        if epoch != journal.mesh_epoch(position)
            || !profile_current(profile, position, &chunks, &journal)
            || chunks
                .loaded
                .get(&position)
                .is_none_or(|section| section.mesh_epoch != epoch)
            || !centers
                .iter()
                .any(|&center| nearby(position, center, RETAIN_RADIUS))
        {
            continue;
        }
        let collider = match result {
            Ok(collider) => collider,
            Err(error) => {
                state.error = Some(error);
                continue;
            }
        };
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

fn build_collider_with_models(
    section: &BlockSection,
    shapes: &[terra_core::block_shapes::ShapeBox],
    masked: &[u16],
) -> Result<Option<Collider>, String> {
    if section.is_empty() && shapes.is_empty() {
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
    if indices.is_empty() {
        return Ok(None);
    }
    Collider::try_trimesh_with_config(positions, indices, TrimeshFlags::FIX_INTERNAL_EDGES)
        .map(Some)
        .map_err(|error| format!("Terrain collider {:?}: {error:?}", section.position()))
}

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tests;
