//! Parked vehicle collision uses the same authored part volumes as picking.
use avian3d::prelude::*;
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use terra_core::trains::{TrainBlockView, TrainPart, VehicleView};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};

#[derive(Resource, Default)]
pub struct TrainCollisionViews {
    identity: Arc<()>,
    epoch: u64,
    vehicles: Vec<VehicleView>,
    retirements: BTreeMap<u64, Vec<(terra_voxel::SectionPos, u64)>>,
}
impl TrainCollisionViews {
    /// Called while consuming the ordered train packet, before its following
    /// section snapshots. A newer complete section proves restoration or any
    /// later coalesced edit, without requiring an intermediate material value.
    pub fn set_confirmed_views(
        &mut self,
        vehicles: &[VehicleView],
        journal: &EditJournal,
    ) -> Result<bool, String> {
        if vehicles.len() > terra_core::trains::MAX_VEHICLES
            || vehicles
                .iter()
                .any(|vehicle| vehicle.parts.len() > terra_core::trains::MAX_PARTS)
            || vehicles
                .iter()
                .map(|vehicle| vehicle.parts.len())
                .sum::<usize>()
                > terra_core::trains::MAX_TOTAL_PARTS
        {
            return Err("Train collision snapshot exceeds geometry budget".into());
        }
        let mut removed = Vec::new();
        for previous in &self.vehicles {
            if !vehicles.iter().any(|vehicle| vehicle.id == previous.id) {
                let checkpoints = checkpoints(previous.anchor, &previous.parts, journal, true)?;
                removed.push((previous.id, checkpoints));
            }
        }
        // The client drains at most eight reliable controls per update. The
        // physics consumer drains this buffer in that same update.
        if self.retirements.len().saturating_add(removed.len())
            > terra_core::trains::MAX_VEHICLES * 8
        {
            return Err("Too many unpublished train retirement checkpoints".into());
        }
        for (id, checkpoint) in removed {
            self.retirements.entry(id).or_insert(checkpoint);
        }
        Ok(self.set_views(vehicles))
    }
    /// Cargo, ownership, coupling and presentation revisions do not rebuild collision.
    fn set_views(&mut self, vehicles: &[VehicleView]) -> bool {
        if self.vehicles.len() == vehicles.len()
            && self.vehicles.iter().zip(vehicles).all(|(old, new)| {
                old.id == new.id
                    && same_geometry(
                        old.anchor,
                        &old.parts,
                        &old.blocks,
                        new.anchor,
                        &new.parts,
                        &new.blocks,
                    )
            })
        {
            return false;
        }
        self.vehicles = vehicles.to_vec();
        self.epoch = self.epoch.wrapping_add(1);
        true
    }
}

type BlockViews = BTreeMap<u16, TrainBlockView>;
const MAX_COLLIDER_JOBS: usize = 2;

fn same_geometry(
    a: [i64; 3],
    parts: &[TrainPart],
    blocks: &BlockViews,
    b: [i64; 3],
    other: &[TrainPart],
    other_blocks: &BlockViews,
) -> bool {
    a == b
        && parts.len() == other.len()
        && parts.iter().zip(other).enumerate().all(|(index, (a, b))| {
            a.offset == b.offset
                && a.material == b.material
                && a.facing == b.facing
                && blocks.get(&(index as u16)).map(|view| &view.boxes)
                    == other_blocks.get(&(index as u16)).map(|view| &view.boxes)
        })
}

#[derive(Component)]
pub struct TrainCollider(pub u64);

struct Entry {
    anchor: [i64; 3],
    parts: Vec<TrainPart>,
    blocks: BlockViews,
    bounds: [[f64; 3]; 2],
    entity: Option<Entity>,
    active: bool,
    retiring: bool,
    checkpoints: Option<Vec<(terra_voxel::SectionPos, u64)>>,
    build: Option<u64>,
    failed: bool,
}

struct ColliderJob {
    vehicle: u64,
    task: Task<Option<Collider>>,
}

fn checkpoints(
    anchor: [i64; 3],
    parts: &[TrainPart],
    journal: &EditJournal,
    after: bool,
) -> Result<Vec<(terra_voxel::SectionPos, u64)>, String> {
    let sections: BTreeSet<_> = parts
        .iter()
        .map(|part| {
            terra_voxel::global_to_section(std::array::from_fn(|axis| {
                anchor[axis] + i64::from(part.offset[axis])
            }))
            .0
        })
        .collect();
    if sections.len() > 64 {
        return Err("Train retirement exceeds section budget".into());
    }
    sections
        .into_iter()
        .map(|section| {
            let revision = journal.revision(section);
            let required = if after {
                revision
                    .checked_add(1)
                    .ok_or("Train retirement revision overflow")?
            } else {
                revision
            };
            Ok((section, required))
        })
        .collect()
}

#[derive(Resource, Default)]
pub(crate) struct TrainCollisionState {
    source: Option<(bool, u64)>,
    entries: BTreeMap<u64, Entry>,
    retired: Vec<[[f64; 3]; 2]>,
    source_identity: Option<Arc<()>>,
    next_build: u64,
    jobs: BTreeMap<u64, ColliderJob>,
}
impl TrainCollisionState {
    pub(crate) fn ready(
        &self,
        low: Vec3,
        high: Vec3,
        journal: &EditJournal,
        views: &TrainCollisionViews,
        access: Option<WorldAccess>,
    ) -> bool {
        let replica = matches!(access, Some(WorldAccess::Replica { .. }));
        let revision = if replica {
            views.epoch
        } else {
            journal.gameplay().trains.revision
        };
        let empty = if replica {
            views.vehicles.is_empty()
        } else {
            journal.gameplay().trains.vehicles.is_empty()
        };
        if self.source != Some((replica, revision))
            || replica
                && !self
                    .source_identity
                    .as_ref()
                    .is_some_and(|identity| Arc::ptr_eq(identity, &views.identity))
        {
            return empty && self.entries.is_empty() && self.retired.is_empty();
        }
        let intersects = |bounds: [[f64; 3]; 2]| {
            (0..3).all(|axis| {
                f64::from(low[axis]) <= bounds[1][axis] && f64::from(high[axis]) >= bounds[0][axis]
            })
        };
        !self
            .entries
            .values()
            .any(|entry| !entry.active && intersects(entry.bounds))
            && !self.retired.iter().any(|bounds| intersects(*bounds))
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TrainCollisionViews>()
        .init_resource::<TrainCollisionState>()
        .add_systems(
            Update,
            sync.after(terra_world::streaming::StreamingSet::Receive),
        )
        .add_systems(FixedUpdate, activate.after(PhysicsSystems::Writeback));
}

fn sync(
    mut commands: Commands,
    journal: Res<EditJournal>,
    mut views: ResMut<TrainCollisionViews>,
    access: Option<Res<WorldAccess>>,
    mut state: ResMut<TrainCollisionState>,
    chunks: Res<ChunkStore>,
    terrain: Res<crate::TerrainCollisionState>,
) {
    let retirement_marks = std::mem::take(&mut views.bypass_change_detection().retirements);
    let replica = matches!(access.as_deref(), Some(WorldAccess::Replica { .. }));
    let revision = if replica {
        views.epoch
    } else {
        journal.gameplay().trains.revision
    };
    if state.source != Some((replica, revision))
        || replica
            && !state
                .source_identity
                .as_ref()
                .is_some_and(|identity| Arc::ptr_eq(identity, &views.identity))
    {
        let vehicles: Vec<_> = if replica {
            views
                .vehicles
                .iter()
                .map(|vehicle| {
                    (
                        vehicle.id,
                        vehicle.anchor,
                        vehicle.parts.as_slice(),
                        &vehicle.blocks,
                        vehicle.model_bounds(),
                    )
                })
                .collect()
        } else {
            journal
                .gameplay()
                .trains
                .vehicles
                .values()
                .map(|vehicle| {
                    (
                        vehicle.id,
                        vehicle.anchor,
                        vehicle.parts.as_slice(),
                        &vehicle.blocks,
                        vehicle.model_bounds(),
                    )
                })
                .collect()
        };
        let removed: Vec<_> = state
            .entries
            .keys()
            .filter(|id| !vehicles.iter().any(|(current, ..)| current == *id))
            .copied()
            .collect();
        for id in removed {
            let entry = state.entries.get_mut(&id).unwrap();
            if !entry.retiring {
                entry.retiring = true;
                entry.checkpoints = if replica {
                    retirement_marks.get(&id).cloned()
                } else {
                    checkpoints(entry.anchor, &entry.parts, &journal, false).ok()
                };
            }
        }
        for (id, anchor, parts, blocks, bounds) in vehicles {
            if state.entries.get(&id).is_some_and(|old| {
                same_geometry(old.anchor, &old.parts, &old.blocks, anchor, parts, blocks)
            }) {
                state.entries.get_mut(&id).unwrap().retiring = false;
                continue;
            }
            let reusable = state
                .entries
                .iter()
                .find(|(_, old)| {
                    old.retiring
                        && old.active
                        && old.entity.is_some()
                        && same_geometry(old.anchor, &old.parts, &old.blocks, anchor, parts, blocks)
                })
                .map(|(id, _)| *id);
            if let Some(previous) = reusable {
                let mut entry = state.entries.remove(&previous).unwrap();
                entry.retiring = false;
                entry.checkpoints = None;
                commands
                    .entity(entry.entity.unwrap())
                    .insert(TrainCollider(id));
                state.entries.insert(id, entry);
                continue;
            }
            if let Some(old) = state.entries.remove(&id) {
                if let Some(entity) = old.entity {
                    commands.entity(entity).despawn();
                }
                state.retired.push(old.bounds);
            }
            state.entries.insert(
                id,
                Entry {
                    anchor,
                    parts: parts.to_vec(),
                    blocks: blocks.clone(),
                    bounds,
                    entity: None,
                    active: false,
                    retiring: false,
                    checkpoints: None,
                    build: None,
                    failed: false,
                },
            );
        }
        state.source = Some((replica, revision));
        state.source_identity = replica.then(|| views.identity.clone());
    }
    let restored: Vec<_> = state
        .entries
        .iter()
        .filter(|(_, entry)| {
            entry.retiring
                && restored(entry, &chunks, &journal, &terrain)
                && !state.entries.values().any(|other| {
                    !other.retiring
                        && !other.active
                        && (0..3).all(|axis| {
                            entry.bounds[0][axis] < other.bounds[1][axis]
                                && entry.bounds[1][axis] > other.bounds[0][axis]
                        })
                })
        })
        .map(|(id, _)| *id)
        .collect();
    for id in restored {
        let entry = state.entries.remove(&id).unwrap();
        if let Some(entity) = entry.entity {
            commands.entity(entity).despawn();
        }
        state.retired.push(entry.bounds);
    }
    // Keep obsolete jobs until their CPU work actually completes: dropping a
    // task cannot interrupt a native mesh cook already in progress. Tracking
    // those jobs prevents rapid replacements from exceeding the global limit.
    let completed: Vec<_> = state
        .jobs
        .iter_mut()
        .filter_map(|(&generation, job)| {
            future::block_on(future::poll_once(&mut job.task))
                .map(|collider| (generation, job.vehicle, collider))
        })
        .collect();
    for (generation, id, collider) in completed {
        state.jobs.remove(&generation);
        let Some(entry) = state
            .entries
            .get_mut(&id)
            .filter(|entry| entry.build == Some(generation))
        else {
            continue;
        };
        entry.build = None;
        if entry.retiring || entry.entity.is_some() {
            continue;
        }
        let Some(collider) = collider else {
            entry.failed = true;
            bevy::log::error!(vehicle = id, "Train collision geometry could not be built");
            continue;
        };
        let translation = Vec3::from_array(std::array::from_fn(|axis| {
            (entry.anchor[axis] as f64
                + if axis == 1 {
                    terra_core::trains::MODEL_Y_OFFSET
                } else {
                    0.0
                }) as f32
        }));
        entry.entity = Some(
            commands
                .spawn((
                    TrainCollider(id),
                    RigidBody::Static,
                    collider,
                    Friction::new(0.0),
                    Restitution::new(0.0),
                    Position(translation),
                    Rotation::IDENTITY,
                    Transform::from_translation(translation),
                ))
                .id(),
        );
    }
    let capacity = MAX_COLLIDER_JOBS.saturating_sub(state.jobs.len());
    let waiting: Vec<_> = state
        .entries
        .iter()
        .filter(|(_, entry)| {
            entry.entity.is_none() && entry.build.is_none() && !entry.retiring && !entry.failed
        })
        .take(capacity)
        .map(|(&id, _)| id)
        .collect();
    for id in waiting {
        loop {
            state.next_build = state.next_build.wrapping_add(1);
            if !state.jobs.contains_key(&state.next_build) {
                break;
            }
        }
        let generation = state.next_build;
        let entry = state.entries.get_mut(&id).unwrap();
        entry.build = Some(generation);
        let parts = entry.parts.clone();
        let blocks = entry.blocks.clone();
        state.jobs.insert(
            generation,
            ColliderJob {
                vehicle: id,
                task: AsyncComputeTaskPool::get().spawn(async move {
                    let _scope = terra_core::profiling::ScopeTimer::new("collision.train_cook");
                    collider(&parts, &blocks)
                }),
            },
        );
    }
}

fn restored(
    entry: &Entry,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &crate::TerrainCollisionState,
) -> bool {
    let mut sections = BTreeSet::new();
    for (index, part) in entry.parts.iter().enumerate() {
        let cell = std::array::from_fn(|axis| entry.anchor[axis] + i64::from(part.offset[axis]));
        let owner = terra_voxel::global_to_section(cell).0;
        if !chunks.loaded.contains_key(&owner) {
            continue;
        }
        if let Some(checkpoints) = &entry.checkpoints {
            if checkpoints
                .iter()
                .any(|(section, required)| *section == owner && journal.revision(owner) < *required)
            {
                return false;
            }
        } else if chunks.block_at(cell) != Some(part.material)
            || journal.object_state(cell).map_or(0, |state| state.facing) != part.facing
        {
            return false;
        }
        sections.insert(owner);
        for bounds in terra_core::trains::part_shapes(part, entry.blocks.get(&(index as u16))) {
            let low = terra_voxel::global_to_section(std::array::from_fn(|axis| {
                (cell[axis] as f64 + bounds.minimum[axis]).floor() as i64
            }))
            .0;
            let high = terra_voxel::global_to_section(std::array::from_fn(|axis| {
                (cell[axis] as f64 + bounds.maximum[axis] - 1e-8).floor() as i64
            }))
            .0;
            for x in low.x..=high.x {
                for y in low.y..=high.y {
                    for z in low.z..=high.z {
                        let neighbor = terra_voxel::SectionPos::new(x, y, z);
                        if chunks.loaded.contains_key(&neighbor) {
                            sections.insert(neighbor);
                        }
                    }
                }
            }
        }
    }
    sections.into_iter().all(|section| {
        if !terrain.in_dispatch_interest(section) {
            return true;
        }
        section.origin().is_ok_and(|origin| {
            let low = Vec3::from_array(origin.map(|value| value as f32)) + Vec3::splat(0.001);
            terrain.ready_for_aabb(low, low + Vec3::splat(15.998), chunks, journal)
        })
    })
}

fn activate(
    mut state: ResMut<TrainCollisionState>,
    installed: Query<&ColliderAabb, With<TrainCollider>>,
) {
    for entry in state.entries.values_mut() {
        if entry
            .entity
            .is_some_and(|entity| installed.contains(entity))
        {
            entry.active = true;
        }
    }
    state.retired.clear();
}

fn collider(parts: &[TrainPart], blocks: &BlockViews) -> Option<Collider> {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        for bounds in terra_core::trains::part_shapes(part, blocks.get(&(index as u16))) {
            let base = positions.len() as u32;
            for corner in 0..8 {
                positions.push(Vec3::from_array(std::array::from_fn(|axis| {
                    f32::from(part.offset[axis])
                        + if corner & (1 << axis) == 0 {
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
    }
    if positions.is_empty() {
        return None;
    }
    Collider::try_trimesh_with_config(positions, indices, TrimeshFlags::FIX_INTERNAL_EDGES).ok()
}

#[cfg(test)]
mod tests;
