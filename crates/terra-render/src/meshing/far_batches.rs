//! Spatially merge stable far terrain. Source meshes remain bounded by loaded
//! sections and allow immediate, upload-free restoration on edits or retirement.
use super::TerrainSection;
use crate::weather::surfaces::{TerrainDepthMaterial, WetMaterial};
use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use terra_voxel::SectionPos;

const CACHE_BYTES: usize = 64 * 1024 * 1024;
const MAX_VERTICES: usize = 64_000;

#[derive(Resource, Default)]
pub(crate) struct FarCache(Arc<AtomicUsize>);
#[derive(Debug)]
pub(crate) struct Geometry {
    mesh: Mesh,
    bytes: usize,
    counter: Arc<AtomicUsize>,
}
impl Drop for Geometry {
    fn drop(&mut self) {
        self.counter.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
impl FarCache {
    pub(super) fn capture(&self, mesh: &Mesh) -> Option<Arc<Geometry>> {
        let bytes =
            mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len);
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes).filter(|&sum| sum <= CACHE_BYTES)
            })
            .ok()?;
        Some(Arc::new(Geometry {
            mesh: mesh.clone(),
            bytes,
            counter: self.0.clone(),
        }))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Material {
    Standard(Handle<StandardMaterial>),
    Wet(Handle<WetMaterial>),
    Depth(Handle<TerrainDepthMaterial>),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    cell: [i64; 3],
    material: Material,
    layout: Vec<(
        bevy::mesh::MeshVertexAttributeId,
        bevy::render::render_resource::VertexFormat,
    )>,
    shadowless: bool,
}
#[derive(Component)]
pub(crate) struct Source {
    pub geometry: Arc<Geometry>,
    pub position: SectionPos,
    pub root: Entity,
    pub material: Material,
    pub shadowless: bool,
}
impl Source {
    fn key(&self) -> Key {
        Key {
            cell: [
                self.position.x.div_euclid(2),
                self.position.y.div_euclid(2),
                self.position.z.div_euclid(2),
            ],
            material: self.material.clone(),
            shadowless: self.shadowless,
            layout: self
                .geometry
                .mesh
                .attributes()
                .map(|(a, _)| (a.id, a.format))
                .collect(),
        }
    }
}
#[derive(Default)]
struct Group {
    members: HashSet<Entity>,
    render: Option<(Entity, Handle<Mesh>)>,
}
struct Job {
    task: Task<Option<Mesh>>,
    ready: Option<Option<Mesh>>,
}
#[derive(Resource, Default)]
struct Groups {
    groups: HashMap<Key, Group>,
    members: HashMap<Entity, Key>,
    dirty: HashSet<Key>,
    queue: VecDeque<Key>,
    jobs: HashMap<Key, Job>,
}
impl Groups {
    fn enqueue(&mut self, key: Key) {
        if self.dirty.insert(key.clone()) {
            self.queue.push_back(key);
        }
    }
}

pub(crate) fn install(app: &mut App) {
    // Larger aggregate bounds defeated section-level culling in the 4K village
    // comparison. Keep the experiment reproducible, never enable it in gameplay.
    if !std::env::args().any(|a| a.starts_with("--smoke-test"))
        || !std::env::var("TERRA_FAR_BATCHES").is_ok_and(|v| v == "1")
    {
        return;
    }
    install_enabled(app);
}

fn install_enabled(app: &mut App) {
    app.init_resource::<FarCache>()
        .init_resource::<Groups>()
        .add_systems(
            PostUpdate,
            update
                .before(bevy::transform::TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
}

#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
struct WorldView<'w, 's> {
    sources: Query<'w, 's, &'static Source>,
    added: Query<'w, 's, Entity, Added<Source>>,
    removed: RemovedComponents<'w, 's, Source>,
    roots: Query<
        'w,
        's,
        (),
        (
            With<TerrainSection>,
            Without<crate::transitions::ChunkFade>,
            Without<crate::transitions::QueuedChunk>,
        ),
    >,
    entering: Query<
        'w,
        's,
        &'static Children,
        Or<(
            Added<crate::transitions::ChunkFade>,
            Added<crate::transitions::QueuedChunk>,
        )>,
    >,
    rendered: Option<Res<'w, super::RenderedSections>>,
}
impl WorldView<'_, '_> {
    fn stable(&self, source: &Source) -> bool {
        self.roots.contains(source.root)
            && self.rendered.as_ref().is_none_or(|r| {
                r.sections
                    .get(&source.position)
                    .is_some_and(|s| s.entity == source.root)
            })
    }
}
fn invalidate(key: &Key, groups: &mut Groups, view: &WorldView, commands: &mut Commands) {
    groups.jobs.remove(key);
    if let Some(group) = groups.groups.get_mut(key)
        && let Some((root, mesh)) = group.render.take()
    {
        crate::transitions::retire(commands, root, vec![mesh], terra_core::ChunkAnimation::None);
        for &member in &group.members {
            if view.sources.contains(member) {
                commands.entity(member).try_insert(Visibility::Inherited);
            }
        }
    }
    groups.enqueue(key.clone());
}
fn update(
    mut commands: Commands,
    mut view: WorldView,
    mut groups: ResMut<Groups>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut budget: ResMut<crate::MeshUploadStatus>,
    cache: Res<FarCache>,
    mut status: ResMut<super::RenderStatus>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.far_batches");
    let mut invalid = HashSet::new();
    for entity in view.removed.read() {
        if let Some(key) = groups.members.remove(&entity) {
            if let Some(group) = groups.groups.get_mut(&key) {
                group.members.remove(&entity);
            }
            invalid.insert(key);
        }
    }
    for children in &view.entering {
        for child in children.iter() {
            if let Some(key) = groups.members.get(&child) {
                invalid.insert(key.clone());
            }
        }
    }
    for entity in &view.added {
        let source = view.sources.get(entity).unwrap();
        let key = source.key();
        groups
            .groups
            .entry(key.clone())
            .or_default()
            .members
            .insert(entity);
        groups.members.insert(entity, key.clone());
        invalid.insert(key);
    }
    if view.rendered.as_ref().is_some_and(|r| r.is_changed()) {
        for (key, group) in &groups.groups {
            if group.render.is_some()
                && group.members.iter().any(|&e| {
                    view.sources
                        .get(e)
                        .map_or(true, |source| !view.stable(source))
                })
            {
                invalid.insert(key.clone());
            }
        }
    }
    for key in invalid {
        invalidate(&key, &mut groups, &view, &mut commands);
    }
    let empty: Vec<_> = groups
        .groups
        .iter()
        .filter(|(_, g)| g.members.is_empty())
        .map(|(k, _)| k.clone())
        .collect();
    for key in empty {
        groups.groups.remove(&key);
        groups.dirty.remove(&key);
        groups.jobs.remove(&key);
    }
    if groups.queue.len() > groups.groups.len().saturating_mul(2) + 16 {
        let dirty = groups.dirty.clone();
        groups.queue.retain(|key| dirty.contains(key));
    }
    // Two worker jobs and a bounded dirty queue; no scan of the loaded world.
    let completed: Vec<_> = groups
        .jobs
        .iter_mut()
        .filter_map(|(key, job)| {
            job.ready
                .take()
                .or_else(|| future::block_on(future::poll_once(&mut job.task)))
                .map(|result| (key.clone(), result))
        })
        .collect();
    for (key, result) in completed {
        let Some(mesh) = result else {
            groups.jobs.remove(&key);
            continue;
        };
        if !budget.reserve(
            crate::upload_budget::UploadKind::Terrain,
            mesh.count_vertices(),
        ) {
            groups.jobs.get_mut(&key).unwrap().ready = Some(Some(mesh));
            continue;
        }
        groups.jobs.remove(&key);
        let group = groups.groups.get_mut(&key).unwrap();
        if group
            .members
            .iter()
            .any(|&e| view.sources.get(e).map_or(true, |s| !view.stable(s)))
        {
            groups.enqueue(key);
            continue;
        }
        let handle = meshes.add(mesh);
        let mut entity = commands.spawn((
            Name::new("Far terrain material group"),
            Mesh3d(handle.clone()),
            Transform::from_translation(Vec3::from_array(key.cell.map(|p| (p * 32) as f32))),
            Visibility::Inherited,
            crate::miniature::layers(),
            terra_core::miniature::MiniatureMesh::Terrain,
        ));
        match &key.material {
            Material::Standard(m) => {
                entity.insert(MeshMaterial3d(m.clone()));
            }
            Material::Wet(m) => {
                entity.insert(MeshMaterial3d(m.clone()));
            }
            Material::Depth(m) => {
                entity.insert(MeshMaterial3d(m.clone()));
            }
        }
        if key.shadowless {
            entity.insert(bevy::light::NotShadowCaster);
        }
        group.render = Some((entity.id(), handle));
        for &member in &group.members {
            commands.entity(member).try_insert(Visibility::Hidden);
        }
    }
    let candidates = groups.queue.len().min(8);
    for _ in 0..candidates {
        if groups.jobs.len() >= 2 {
            break;
        }
        let key = groups.queue.pop_front().unwrap();
        if !groups.dirty.remove(&key) {
            continue;
        }
        let Some(group) = groups.groups.get(&key) else {
            continue;
        };
        if group.members.len() < 2 {
            continue;
        }
        let mut pieces = Vec::new();
        let mut vertices = 0;
        let mut stable = true;
        for &member in &group.members {
            let Ok(source) = view.sources.get(member) else {
                stable = false;
                break;
            };
            if !view.stable(source) {
                stable = false;
                break;
            }
            vertices += source.geometry.mesh.count_vertices();
            let origin = source.position.origin().unwrap();
            pieces.push((
                member.to_bits(),
                source.geometry.clone(),
                Vec3::from_array(std::array::from_fn(|i| {
                    (origin[i] - key.cell[i] * 32) as f32
                })),
            ));
        }
        if !stable {
            groups.enqueue(key);
            continue;
        }
        if vertices > MAX_VERTICES {
            continue;
        }
        pieces.sort_unstable_by_key(|p| p.0);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let mut result = None::<Mesh>;
            for (_, source, offset) in pieces {
                let mesh = source.mesh.clone().translated_by(offset);
                if let Some(result) = &mut result {
                    if result.merge(&mesh).is_err() {
                        return None;
                    }
                } else {
                    result = Some(mesh);
                }
            }
            if let Some(mesh) = &mut result {
                mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
            }
            result
        });
        groups.jobs.insert(key, Job { task, ready: None });
    }
    status.far_groups = groups
        .groups
        .values()
        .filter(|group| group.render.is_some())
        .count();
    status.far_source_batches = groups
        .groups
        .values()
        .filter(|group| group.render.is_some())
        .map(|group| group.members.len())
        .sum();
    status.far_cache_bytes = cache.0.load(Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_gameplay_keeps_section_culling_without_an_extra_mesh_cache() {
        let mut app = App::new();
        install(&mut app);
        assert!(!app.world().contains_resource::<FarCache>());
        assert!(!app.world().contains_resource::<Groups>());
    }
    #[test]
    fn stable_sections_merge_once_and_restore_sources_immediately_on_unload() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<crate::MeshUploadStatus>()
            .init_resource::<super::super::RenderStatus>();
        install_enabled(&mut app);
        let mut roots = Vec::new();
        let mut members = Vec::new();
        for x in 0..2 {
            let position = SectionPos::new(x, 0, 0);
            let root = app
                .world_mut()
                .spawn(TerrainSection {
                    position,
                    mesh_epoch: 0,
                    detail_level: super::super::DetailLevel::Far,
                })
                .id();
            let mesh = Mesh::from(Cuboid::default());
            let geometry = app.world().resource::<FarCache>().capture(&mesh).unwrap();
            let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
            let member = app
                .world_mut()
                .spawn((
                    ChildOf(root),
                    Mesh3d(handle),
                    Visibility::Inherited,
                    Source {
                        geometry,
                        position,
                        root,
                        material: Material::Standard(Handle::default()),
                        shadowless: false,
                    },
                ))
                .id();
            roots.push(root);
            members.push(member);
        }
        for _ in 0..100 {
            app.update();
            if app
                .world()
                .resource::<Groups>()
                .groups
                .values()
                .any(|g| g.render.is_some())
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            app.world()
                .resource::<Groups>()
                .groups
                .values()
                .filter(|g| g.render.is_some())
                .count(),
            1
        );
        for &member in &members {
            assert_eq!(
                app.world().get::<Visibility>(member),
                Some(&Visibility::Hidden)
            );
        }
        app.world_mut().despawn(roots[0]);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(members[1]),
            Some(&Visibility::Inherited)
        );
        assert!(
            app.world()
                .resource::<Groups>()
                .groups
                .values()
                .all(|g| g.render.is_none())
        );
        app.world_mut().despawn(roots[1]);
        app.update();
        assert!(app.world().resource::<Groups>().groups.is_empty());
        assert_eq!(
            app.world().resource::<FarCache>().0.load(Ordering::Relaxed),
            0
        );
    }
    #[test]
    fn snapshot_budget_is_released_with_the_last_source_or_worker_reference() {
        let cache = FarCache::default();
        let mesh = Mesh::from(Cuboid::default());
        let a = cache.capture(&mesh).unwrap();
        let bytes = cache.0.load(Ordering::Relaxed);
        assert!(bytes > 0);
        let b = a.clone();
        drop(a);
        assert_eq!(cache.0.load(Ordering::Relaxed), bytes);
        drop(b);
        assert_eq!(cache.0.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn grid_is_spatial_and_correct_for_negative_sections() {
        for (x, expected) in [(-3_i64, -2), (-2, -1), (-1, -1), (0, 0), (1, 0), (2, 1)] {
            assert_eq!(x.div_euclid(2), expected);
        }
    }
}
