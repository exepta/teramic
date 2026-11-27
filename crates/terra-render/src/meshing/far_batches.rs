//! Spatially merge stable far terrain. Source meshes remain bounded by loaded
//! sections and allow immediate, upload-free restoration on edits or retirement.
use super::TerrainSection;
use crate::weather::surfaces::{TerrainDepthMaterial, WetMaterial};
use bevy::{
    asset::{RenderAssetUsages, UntypedAssetId},
    camera::primitives::{Aabb, MeshAabb},
    ecs::system::SystemParam,
    pbr::PreparedMaterial,
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        erased_render_asset::ErasedRenderAssets,
        mesh::{RenderMesh, allocator::MeshAllocator},
        render_asset::RenderAssets,
    },
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use terra_voxel::SectionPos;

const CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_VERTICES: usize = 512;

// Prefer a cube that straddles each nearby section boundary. A neighbouring
// section calculates the same world origin for that cube. A full-width surface
// stays aligned on that axis, so the aggregate never exceeds one 16-block section.
fn cell_offset(bounds: Aabb) -> Option<[i64; 3]> {
    let lo = bounds.min().to_array();
    let hi = bounds.max().to_array();
    let mut offset = [0; 3];
    for axis in 0..3 {
        let (a, b) = (lo[axis], hi[axis]);
        offset[axis] = if a >= -8.0 && b <= 8.0 {
            -8
        } else if a >= 8.0 && b <= 24.0 {
            8
        } else if a >= 0.0 && b <= 16.0 {
            0
        } else {
            return None;
        };
    }
    Some(offset)
}

#[derive(Resource, Default)]
pub(crate) struct FarCache(Arc<AtomicUsize>);
#[derive(Debug)]
pub(crate) struct Geometry {
    mesh: Mesh,
    cell_offset: [i64; 3],
    bytes: usize,
    counter: Arc<AtomicUsize>,
}
impl Drop for Geometry {
    fn drop(&mut self) {
        self.counter.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
impl FarCache {
    pub(super) fn capture(&self, mesh: &Mesh, bounds: Option<Aabb>) -> Option<Arc<Geometry>> {
        if mesh.count_vertices() > MAX_SOURCE_VERTICES
            || !matches!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION),
                Some(bevy::mesh::VertexAttributeValues::Float32x3(_))
            )
            || mesh.final_uv_ranges.iter().any(Option::is_some)
        {
            return None;
        }
        let cell_offset = cell_offset(bounds?)?;
        // No other section can contribute to a cell aligned on every axis.
        if cell_offset == [0; 3] {
            return None;
        }
        let bytes =
            mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len);
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes).filter(|&sum| sum <= CACHE_BYTES)
            })
            .ok()?;
        Some(Arc::new(Geometry {
            mesh: mesh.clone(),
            cell_offset,
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
impl Material {
    fn id(&self) -> UntypedAssetId {
        match self {
            Self::Standard(m) => m.id().untyped(),
            Self::Wet(m) => m.id().untyped(),
            Self::Depth(m) => m.id().untyped(),
        }
    }

    fn attach(&self, entity: &mut bevy::ecs::system::EntityCommands) {
        match self {
            Self::Standard(m) => {
                entity.try_insert(MeshMaterial3d(m.clone()));
            }
            Self::Wet(m) => {
                entity.try_insert(MeshMaterial3d(m.clone()));
            }
            Self::Depth(m) => {
                entity.try_insert(MeshMaterial3d(m.clone()));
            }
        }
    }

    fn detach(&self, entity: &mut bevy::ecs::system::EntityCommands) {
        match self {
            Self::Standard(_) => {
                entity.try_remove::<MeshMaterial3d<StandardMaterial>>();
            }
            Self::Wet(_) => {
                entity.try_remove::<MeshMaterial3d<WetMaterial>>();
            }
            Self::Depth(_) => {
                entity.try_remove::<MeshMaterial3d<TerrainDepthMaterial>>();
            }
        }
    }
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
    fn key(&self) -> Option<Key> {
        let origin = self.position.origin().ok()?;
        // Keep both integer origins exactly representable by the GPU transform.
        if origin.iter().any(|p| p.unsigned_abs() >= 1 << 23) {
            return None;
        }
        Some(Key {
            cell: std::array::from_fn(|i| origin[i] + self.geometry.cell_offset[i]),
            material: self.material.clone(),
            shadowless: self.shadowless,
            layout: self
                .geometry
                .mesh
                .attributes()
                .map(|(a, _)| (a.id, a.format))
                .collect(),
        })
    }
}
#[derive(Default)]
struct Group {
    members: HashSet<Entity>,
    render: Option<(Entity, Handle<Mesh>, Vec<Entity>)>,
}
struct Job {
    task: Task<Option<Mesh>>,
    ready: Option<Option<Mesh>>,
}
struct PendingMesh {
    mesh: Handle<Mesh>,
    bounds: Aabb,
    ready: AtomicBool,
}

#[derive(Resource, Default)]
struct Groups {
    pending: HashMap<Key, PendingMesh>,
    rendered_groups: usize,
    rendered_sources: usize,
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

fn requested(smoke: bool, setting: Option<&str>) -> bool {
    smoke && setting == Some("1")
}

pub(crate) fn install(app: &mut App) {
    // The additional source cache and aggregate bounds cost time in the 4K
    // rain scene. Keep this cross-section experiment available for comparisons.
    if !requested(
        std::env::args().any(|a| a.starts_with("--smoke-test")),
        std::env::var("TERRA_FAR_BATCHES").ok().as_deref(),
    ) {
        return;
    }
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render.add_systems(ExtractSchedule, confirm_uploads);
    install_enabled(app);
}

fn install_enabled(app: &mut App) {
    app.init_resource::<FarCache>()
        .init_resource::<Groups>()
        .add_systems(
            PostUpdate,
            update
                .before(crate::transitions::animate)
                .before(bevy::transform::TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
}

// At extraction the previous render frame's assets and allocator slices are
// available. Only publish in the main world after the renderer has both buffers
// and the shared material. This also covers a saturated Bevy upload budget.
fn confirm_uploads(
    groups: Extract<Res<Groups>>,
    meshes: Res<RenderAssets<RenderMesh>>,
    allocator: Res<MeshAllocator>,
    materials: Res<ErasedRenderAssets<PreparedMaterial>>,
) {
    for (key, pending) in &groups.pending {
        let id = pending.mesh.id();
        if meshes.get(id).is_some()
            && allocator.mesh_vertex_slice(&id).is_some()
            && allocator.mesh_index_slice(&id).is_some()
            && materials.get(key.material.id()).is_some()
        {
            pending.ready.store(true, Ordering::Relaxed);
        }
    }
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
        Option<&'static Visibility>,
        (
            With<TerrainSection>,
            Without<crate::transitions::ChunkFade>,
            Without<crate::transitions::QueuedChunk>,
        ),
    >,
    changed_roots: Query<
        'w,
        's,
        &'static Children,
        (
            With<TerrainSection>,
            Or<(
                Changed<Visibility>,
                Added<crate::transitions::ChunkFade>,
                Added<crate::transitions::QueuedChunk>,
            )>,
        ),
    >,
    children: Query<'w, 's, &'static Children, With<TerrainSection>>,
    finished: RemovedComponents<'w, 's, crate::transitions::ChunkFade>,
    dequeued: RemovedComponents<'w, 's, crate::transitions::QueuedChunk>,
    restored_casters: RemovedComponents<'w, 's, bevy::light::NotShadowCaster>,
    rendered: Option<Res<'w, super::RenderedSections>>,
}
impl WorldView<'_, '_> {
    fn stable(&self, source: &Source) -> bool {
        self.roots
            .get(source.root)
            .is_ok_and(|v| v != Some(&Visibility::Hidden))
            && self.rendered.as_ref().is_none_or(|r| {
                r.sections
                    .get(&source.position)
                    .is_some_and(|s| s.entity == source.root)
            })
    }
}
fn invalidate(
    key: &Key,
    groups: &mut Groups,
    view: &WorldView,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
) {
    if let Some(pending) = groups.pending.remove(key) {
        meshes.remove(pending.mesh.id());
    }
    groups.jobs.remove(key);
    if let Some(group) = groups.groups.get_mut(key)
        && let Some((root, mesh, drawn)) = group.render.take()
    {
        groups.rendered_groups -= 1;
        groups.rendered_sources -= drawn.len();
        crate::transitions::retire(commands, root, vec![mesh], terra_core::ChunkAnimation::None);
        for member in drawn {
            if let Ok(source) = view.sources.get(member) {
                let mut entity = commands.entity(member);
                entity.try_insert(Visibility::Inherited);
                source.material.attach(&mut entity);
            }
        }
    }
    if groups.groups.get(key).is_some_and(|g| g.members.is_empty()) {
        groups.groups.remove(key);
        groups.dirty.remove(key);
    } else if groups.groups.get(key).is_some_and(|g| g.members.len() >= 2) {
        groups.enqueue(key.clone());
    }
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
    let settled = view.finished.read().chain(view.dequeued.read());
    for children in view
        .changed_roots
        .iter()
        .chain(settled.filter_map(|root| view.children.get(root).ok()))
    {
        for child in children.iter() {
            if let Some(key) = groups.members.get(&child) {
                invalid.insert(key.clone());
            }
        }
    }
    for entity in &view.added {
        let source = view.sources.get(entity).unwrap();
        let Some(key) = source.key() else {
            continue;
        };
        groups
            .groups
            .entry(key.clone())
            .or_default()
            .members
            .insert(entity);
        groups.members.insert(entity, key.clone());
        invalid.insert(key);
    }
    // Switching shadows off releases replacement proxies and restores the
    // original casters. Leave affected sources independent until recomposed.
    for entity in view.restored_casters.read() {
        if let Some(key) = groups.members.get(&entity)
            && key.shadowless
        {
            commands.entity(entity).try_remove::<Source>();
            invalid.insert(key.clone());
        }
    }
    for key in invalid {
        invalidate(&key, &mut groups, &view, &mut commands, &mut meshes);
    }
    if groups.queue.len() > groups.groups.len().saturating_mul(2) + 16 {
        let dirty = groups.dirty.clone();
        groups.queue.retain(|key| dirty.contains(key));
    }
    // Only changed cells enter the bounded worker pool.
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
        if !budget.reserve_batch_bytes(
            crate::upload_budget::UploadKind::Terrain,
            mesh.count_vertices(),
            mesh.get_vertex_buffer_size() + mesh.get_index_buffer_bytes().map_or(0, <[u8]>::len),
        ) {
            groups.jobs.get_mut(&key).unwrap().ready = Some(Some(mesh));
            continue;
        }
        groups.jobs.remove(&key);
        let bounds = mesh.get_aabb().expect("group members have finite bounds");
        groups.pending.insert(
            key,
            PendingMesh {
                mesh: meshes.add(mesh),
                bounds,
                ready: AtomicBool::new(false),
            },
        );
    }
    let ready: Vec<_> = groups
        .pending
        .iter()
        .filter(|(_, pending)| pending.ready.load(Ordering::Relaxed))
        .map(|(key, _)| key.clone())
        .collect();
    for key in ready {
        let PendingMesh {
            mesh: handle,
            bounds,
            ..
        } = groups.pending.remove(&key).unwrap();
        let group = groups.groups.get_mut(&key).unwrap();
        if group
            .members
            .iter()
            .any(|&e| view.sources.get(e).map_or(true, |s| !view.stable(s)))
        {
            meshes.remove(handle.id());
            continue;
        }
        let mut entity = commands.spawn((
            Name::new("Far terrain material group"),
            Mesh3d(handle.clone()),
            Transform::from_translation(Vec3::from_array(key.cell.map(|p| p as f32))),
            bounds,
            bevy::camera::visibility::NoAutoAabb,
            Visibility::Inherited,
            crate::miniature::layers(),
            terra_core::miniature::MiniatureMesh::Terrain,
        ));
        key.material.attach(&mut entity);
        if key.shadowless {
            entity.insert(bevy::light::NotShadowCaster);
        }
        let count = group.members.len();
        group.render = Some((entity.id(), handle, group.members.iter().copied().collect()));
        for &member in &group.members {
            let mut entity = commands.entity(member);
            entity.try_insert(Visibility::Hidden);
            key.material.detach(&mut entity);
        }
        groups.rendered_groups += 1;
        groups.rendered_sources += count;
    }
    let candidates = groups.queue.len().min(64);
    for _ in 0..candidates {
        if groups.jobs.len() >= 4 || groups.pending.len() + groups.jobs.len() >= 8 {
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
                Vec3::from_array(std::array::from_fn(|i| (origin[i] - key.cell[i]) as f32)),
            ));
        }
        if !stable {
            // Finishing the fade or changing the section will enqueue this cell.
            continue;
        }
        if vertices > crate::upload_budget::MAX_BATCH_VERTICES {
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
                // A previously cached local bound belongs to the first piece.
                mesh.final_aabb = None;
                mesh.asset_usage = RenderAssetUsages::RENDER_WORLD;
            }
            result
        });
        groups.jobs.insert(key, Job { task, ready: None });
    }
    status.far_groups = groups.rendered_groups;
    status.far_source_batches = groups.rendered_sources;
    status.far_cache_bytes = cache.0.load(Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_app_keeps_no_far_geometry_or_upload_queue() {
        let mut app = App::new();
        install(&mut app);
        assert!(!app.world().contains_resource::<FarCache>());
        assert!(!app.world().contains_resource::<Groups>());
    }

    #[test]
    fn cross_section_meshes_are_only_enabled_by_the_explicit_smoke_option() {
        for setting in [None, Some("1"), Some("0"), Some("true")] {
            assert!(!requested(false, setting));
            assert_eq!(requested(true, setting), setting == Some("1"));
        }
    }
    fn await_upload(app: &mut App) -> Handle<Mesh> {
        for _ in 0..100 {
            app.update();
            if let Some(staged) = app.world().resource::<Groups>().pending.values().next() {
                return staged.mesh.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("far mesh did not reach the upload queue");
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
            let mesh = Mesh::from(Cuboid::default())
                .translated_by(Vec3::X * if x == 0 { 15.0 } else { 1.0 });
            let geometry = app
                .world()
                .resource::<FarCache>()
                .capture(&mesh, mesh.get_aabb())
                .unwrap();
            let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
            let member = app
                .world_mut()
                .spawn((
                    ChildOf(root),
                    Mesh3d(handle),
                    MeshMaterial3d::<StandardMaterial>(Handle::default()),
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
        let grouped_mesh = await_upload(&mut app);
        app.update();
        for &source in &members {
            assert_eq!(
                app.world().get::<Visibility>(source),
                Some(&Visibility::Inherited)
            );
            assert!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(source)
                    .is_some()
            );
        }
        assert_eq!(app.world().resource::<Groups>().rendered_groups, 0);
        app.world()
            .resource::<Groups>()
            .pending
            .values()
            .next()
            .unwrap()
            .ready
            .store(true, Ordering::Relaxed);
        app.update();
        assert_eq!(app.world().resource::<Groups>().rendered_groups, 1);
        assert_eq!(
            app.world()
                .resource::<super::super::RenderStatus>()
                .far_source_batches,
            2
        );
        for &member in &members {
            assert!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(member)
                    .is_none()
            );
            assert_eq!(
                app.world().get::<Visibility>(member),
                Some(&Visibility::Hidden)
            );
        }
        let grouped_entity = app
            .world()
            .resource::<Groups>()
            .groups
            .values()
            .find_map(|g| g.render.as_ref().map(|(entity, _, _)| *entity))
            .unwrap();
        let assets = app.world().resource::<Assets<Mesh>>();
        let mesh = assets.get(&grouped_mesh).unwrap();
        assert_eq!(mesh.count_vertices(), 48);
        let group_origin = app
            .world()
            .get::<Transform>(grouped_entity)
            .unwrap()
            .translation;
        let bounds = mesh.get_aabb().unwrap();
        assert_eq!(
            Vec3::from(bounds.min()) + group_origin,
            Vec3::new(14.5, -0.5, -0.5)
        );
        assert_eq!(
            Vec3::from(bounds.max()) + group_origin,
            Vec3::new(17.5, 0.5, 0.5)
        );
        // Mining isolation and deferred retirement first hide the section root.
        app.world_mut()
            .entity_mut(roots[0])
            .insert(Visibility::Hidden);
        app.update();
        assert_eq!(app.world().resource::<Groups>().rendered_groups, 0);
        assert!(
            !app.world()
                .resource::<Assets<Mesh>>()
                .contains(grouped_mesh.id())
        );
        assert_eq!(
            app.world().get::<Visibility>(members[0]),
            Some(&Visibility::Inherited)
        );
        app.world_mut()
            .entity_mut(roots[0])
            .insert(Visibility::Inherited);
        let cancelled = await_upload(&mut app);
        app.world_mut().despawn(roots[0]);
        app.update();
        assert!(
            !app.world()
                .resource::<Assets<Mesh>>()
                .contains(cancelled.id())
        );
        assert!(app.world().resource::<Groups>().pending.is_empty());
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(members[1])
                .is_some()
        );
        assert_eq!(
            app.world()
                .resource::<super::super::RenderStatus>()
                .far_source_batches,
            0
        );
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
        let a = cache.capture(&mesh, mesh.get_aabb()).unwrap();
        let bytes = cache.0.load(Ordering::Relaxed);
        assert!(bytes > 0);
        let b = a.clone();
        drop(a);
        assert_eq!(cache.0.load(Ordering::Relaxed), bytes);
        drop(b);
        assert_eq!(cache.0.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn compact_cells_match_across_negative_section_edges_and_reject_oversized_bounds() {
        let boundary = |x: f32| {
            let mesh = Mesh::from(Cuboid::default()).translated_by(Vec3::X * x);
            cell_offset(mesh.get_aabb().unwrap()).unwrap()
        };
        assert_eq!(-16 + boundary(15.0)[0], boundary(1.0)[0]);
        let bounds = Aabb::from_min_max(Vec3::ZERO, Vec3::splat(16.0));
        assert_eq!(cell_offset(bounds), Some([0; 3]));
        assert!(
            FarCache::default()
                .capture(&Mesh::from(Cuboid::default()), Some(bounds))
                .is_none()
        );
        assert_eq!(
            cell_offset(Aabb::from_min_max(Vec3::ZERO, Vec3::splat(17.0))),
            None
        );
    }
}
