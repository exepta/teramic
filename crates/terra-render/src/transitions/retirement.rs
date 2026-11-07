//! Hide retired geometry immediately; release its ECS/assets in bounded batches.
use super::{
    ChunkAnimationStatus, ChunkFade, EnteringChunk, FadedHandle, QueuedChunk, TransitionDraws,
};
use bevy::prelude::*;
use std::{
    collections::{HashMap, VecDeque},
    sync::LazyLock,
    time::{Duration, Instant},
};

const MAX_PENDING_ROOTS: usize = 4096;
const MAX_PENDING_MESHES: usize = 32_768;
const MAX_PENDING_MATERIALS: usize = 32_768;
const ROOTS_PER_FRAME: usize = 64;
const WORK_PER_FRAME: usize = 256;
const FRAME_BUDGET: Duration = Duration::from_millis(2);

struct Pending {
    root: Entity,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<FadedHandle>,
    descendants: usize,
}

impl Pending {
    fn add_meshes(&mut self, meshes: impl IntoIterator<Item = Handle<Mesh>>) {
        for mesh in meshes {
            if !self.meshes.iter().any(|old| old.id() == mesh.id()) {
                self.meshes.push(mesh);
            }
        }
    }

    fn work(&self) -> usize {
        1 + self.descendants + self.meshes.len() + self.materials.len()
    }
}

#[derive(Resource, Default)]
struct Retirements {
    order: VecDeque<Entity>,
    pending: HashMap<Entity, Pending>,
    meshes: usize,
    materials: usize,
    overflows: usize,
}

impl Retirements {
    fn insert(&mut self, mut entry: Pending) {
        if let Some(previous) = self.pending.get_mut(&entry.root) {
            self.meshes -= previous.meshes.len();
            previous.add_meshes(entry.meshes);
            self.meshes += previous.meshes.len();
            self.materials += entry.materials.len();
            previous.materials.append(&mut entry.materials);
            previous.descendants = previous.descendants.max(entry.descendants);
        } else {
            self.meshes += entry.meshes.len();
            self.materials += entry.materials.len();
            self.order.push_back(entry.root);
            self.pending.insert(entry.root, entry);
        }
    }

    fn pop(&mut self) -> Option<Pending> {
        let root = self.order.pop_front()?;
        let entry = self.pending.remove(&root)?;
        self.meshes -= entry.meshes.len();
        self.materials -= entry.materials.len();
        Some(entry)
    }

    fn over_limit(&self) -> bool {
        self.pending.len() > MAX_PENDING_ROOTS
            || self.meshes > MAX_PENDING_MESHES
            || self.materials > MAX_PENDING_MATERIALS
    }

    fn update_status(&self, world: &mut World) {
        if let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
            status.retiring_roots = self.pending.len();
            status.retiring_meshes = self.meshes;
            status.retiring_materials = self.materials;
            status.retirement_overflows = self.overflows;
        }
    }
}

fn legacy_control(smoke: bool, value: Option<&str>) -> bool {
    smoke && value == Some("1")
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Retirements>();
}

pub(super) fn enabled(world: &World) -> bool {
    static LEGACY: LazyLock<bool> = LazyLock::new(|| {
        legacy_control(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_BENCH_LEGACY_RETIRE").ok().as_deref(),
        )
    });
    // Standalone callers without the transition plugin have no scheduled drain.
    world.contains_resource::<Retirements>() && !*LEGACY
}

pub(super) fn contains(world: &World, root: Entity) -> bool {
    world
        .get_resource::<Retirements>()
        .is_some_and(|queue| queue.pending.contains_key(&root))
}

pub(super) fn enqueue(world: &mut World, root: Entity, meshes: Vec<Handle<Mesh>>) {
    let mut entry = Pending {
        root,
        meshes,
        materials: Vec::new(),
        descendants: 0,
    };
    entry.meshes.sort_unstable_by_key(Handle::id);
    entry.meshes.dedup_by_key(|mesh| mesh.id());
    if world.get_entity(root).is_ok() && !contains(world, root) {
        // Explicitly visible shadow children ignore a hidden parent. Hide every
        // mesh as well, before this frame's visibility propagation/extraction.
        let children = super::mesh_entities(world, root);
        entry.descendants = children.len();
        if let Some(mut visibility) = world.get_mut::<Visibility>(root) {
            visibility.set_if_neq(Visibility::Hidden);
        }
        for entity in children {
            if world.get::<Visibility>(entity) == Some(&Visibility::Visible)
                && let Some(mut visibility) = world.get_mut::<Visibility>(entity)
            {
                *visibility = Visibility::Hidden;
            }
        }
        let mut entity = world.entity_mut(root);
        let queued = entity.take::<QueuedChunk>();
        let fade = entity.take::<ChunkFade>();
        entity.remove::<EnteringChunk>();
        if let Some(queued) = queued {
            entry.add_meshes(queued.meshes);
        }
        if let Some(fade) = fade {
            entry.add_meshes(fade.retired_meshes);
            entry.materials = fade
                .materials
                .into_iter()
                .map(|material| material.handle)
                .collect();
            if let Some(mut feedback) = world.get_resource_mut::<TransitionDraws>() {
                for material in &entry.materials {
                    feedback.pending.remove(&material.entity());
                }
            }
            if let Some(mut status) = world.get_resource_mut::<ChunkAnimationStatus>() {
                status.completed += 1;
            }
        }
    }
    world.init_resource::<Retirements>();
    world.resource_scope(|world, mut queue: Mut<Retirements>| {
        queue.insert(entry);
        // A warp/reset can retire more than one whole view at once. Preserve a
        // hard resident bound by synchronously reclaiming oldest entries only
        // on overflow; ordinary layer changes fit without this fallback.
        let mut overflowed = false;
        while queue.over_limit() {
            let Some(entry) = queue.pop() else { break };
            release(world, entry);
            queue.overflows += 1;
            overflowed = true;
        }
        if overflowed {
            queue.update_status(world);
        }
    });
}

fn release(world: &mut World, entry: Pending) {
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.retirement.release");
    if world.entities().contains(entry.root) {
        world.despawn(entry.root);
    }
    if let Some(mut assets) = world.get_resource_mut::<Assets<Mesh>>() {
        for mesh in entry.meshes {
            assets.remove(mesh.id());
        }
    }
    for material in entry.materials {
        match material {
            FadedHandle::Standard(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
            FadedHandle::Leaf(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
            FadedHandle::Prop(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
            FadedHandle::Water(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
            FadedHandle::Wet(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
            FadedHandle::Depth(entity, original, temporary) => {
                super::restore(world, entity, original, temporary)
            }
        }
    }
}

pub(super) fn drain(world: &mut World) {
    drain_with(world, ROOTS_PER_FRAME, WORK_PER_FRAME, FRAME_BUDGET);
}

fn drain_with(world: &mut World, roots: usize, work: usize, budget: Duration) {
    if world
        .get_resource::<Retirements>()
        .is_none_or(|queue| queue.pending.is_empty())
    {
        return;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("transitions.retirement.drain");
    let started = Instant::now();
    world.resource_scope(|world, mut queue: Mut<Retirements>| {
        let mut completed = 0;
        let mut spent = 0;
        loop {
            if completed != 0 {
                if completed >= roots || started.elapsed() >= budget {
                    break;
                }
                let Some(next) = queue.order.front().and_then(|root| queue.pending.get(root))
                else {
                    break;
                };
                if next.work() > work.saturating_sub(spent) {
                    break;
                }
            }
            let Some(entry) = queue.pop() else { break };
            spent += entry.work();
            release(world, entry);
            completed += 1;
        }
        queue.update_status(world);
    });
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
