//! Conservative section portals. Unknown space and partial blocks are open;
//! disconnected face openings may overdraw, but must never hide an entrance.
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};
use terra_voxel::{SectionPos, global_to_section};
use terra_world::streaming::{
    ChunkFocus, ChunkStore, SectionEditApplied, SectionPublished, SectionRemoved, StreamingSet,
    WorldTerrain,
};

const SIDES: [[i64; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

#[derive(Resource, Default)]
pub(crate) struct TerrainVisibility {
    hidden: HashSet<SectionPos>,
    pub revision: u64,
}
impl TerrainVisibility {
    pub(crate) fn len(&self) -> usize {
        self.hidden.len()
    }
    pub(crate) fn hidden(&self, p: SectionPos) -> bool {
        self.hidden.contains(&p)
    }
    fn replace(&mut self, hidden: HashSet<SectionPos>) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

pub(super) fn invalidate(world: &mut World, positions: &[SectionPos]) {
    if let Some(mut cache) = world.get_resource_mut::<PortalCache>() {
        for p in positions {
            cache.faces.remove(p);
            cache.pending.insert(*p);
        }
        cache.task = None;
        cache.opening_epoch = cache.opening_epoch.wrapping_add(1);
        cache.dirty = true;
    }
    if let Some(mut visibility) = world.get_resource_mut::<TerrainVisibility>() {
        visibility.replace(HashSet::new());
    }
}

#[derive(Resource, Default)]
struct PortalCache {
    faces: HashMap<SectionPos, (u64, u8)>,
    pending: HashSet<SectionPos>,
    opaque: Vec<bool>,
    focus: Option<SectionPos>,
    // Closing portals preserves older conservative results; opening one does not.
    opening_epoch: u64,
    dirty: bool,
    task: Option<(u64, Task<HashSet<SectionPos>>)>,
    last_analysis: Option<Instant>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TerrainVisibility>()
        .init_resource::<PortalCache>()
        .add_systems(
            Update,
            update
                .after(StreamingSet::Receive)
                .after(crate::blocks::poll_blocks)
                .before(super::upload_sections)
                .before(super::dispatch_meshes),
        );
}

#[allow(clippy::too_many_arguments)] // ECS resources and independent message cursors.
fn update(
    chunks: Res<ChunkStore>,
    terrain: Res<WorldTerrain>,
    focus: Res<ChunkFocus>,
    assets: Res<crate::blocks::BlockAssets>,
    mut publications: MessageReader<SectionPublished>,
    mut removals: MessageReader<SectionRemoved>,
    mut edits: MessageReader<SectionEditApplied>,
    journal: Res<terra_world::edits::EditJournal>,
    mut cache: ResMut<PortalCache>,
    mut visible: ResMut<TerrainVisibility>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("terrain.visibility");
    let Some(templates) = &assets.templates else {
        return;
    };
    static REFERENCE: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::args().any(|arg| arg.starts_with("--smoke-test"))
            && std::env::var("TERRA_BENCH_NO_PORTALS").is_ok_and(|v| v == "1")
    });
    if *REFERENCE {
        return;
    }
    let mut invalidate = false;
    if terrain.is_changed() || assets.is_changed() || cache.opaque.is_empty() {
        *cache = PortalCache::default();
        cache.opaque =
            vec![false; templates.blocks.keys().copied().max().unwrap_or(0) as usize + 1];
        for (&id, block) in &templates.blocks {
            cache.opaque[id as usize] = block.render_kind
                == crate::blocks::definitions::BlockRenderKind::Cube
                && block.model.is_empty()
                && terra_voxel::MaterialId::from_raw(id)
                    .is_some_and(terra_voxel::MaterialId::is_opaque);
        }
        cache.pending.extend(chunks.loaded.keys().copied());
        invalidate = true;
    }
    for event in removals.read() {
        cache.faces.remove(&event.position);
        cache.pending.remove(&event.position);
        invalidate = true;
    }
    for event in publications.read() {
        let Some(section) = chunks.loaded.get(&event.position) else {
            continue;
        };
        if cache
            .faces
            .get(&event.position)
            .is_none_or(|(epoch, _)| *epoch != section.mesh_epoch)
        {
            // Edited data can open a portal before its summary is budgeted.
            invalidate |= cache.faces.remove(&event.position).is_some();
            cache.pending.insert(event.position);
        }
    }
    for event in edits.read() {
        if cache
            .faces
            .get(&event.position)
            .is_some_and(|(epoch, _)| *epoch != journal.mesh_epoch(event.position))
        {
            cache.faces.remove(&event.position);
            cache.pending.insert(event.position);
            invalidate = true;
        }
    }
    if journal.is_changed() {
        let stale: Vec<_> = cache
            .faces
            .iter()
            .filter_map(|(&p, &(epoch, _))| (epoch != journal.mesh_epoch(p)).then_some(p))
            .collect();
        for p in stale {
            cache.faces.remove(&p);
            cache.pending.insert(p);
            invalidate = true;
        }
    }
    let eye = global_to_section(focus.0.map(|v| v.floor() as i64)).0;
    if cache.focus != Some(eye) {
        cache.focus = Some(eye);
        invalidate = true;
    }
    if invalidate {
        cache.opening_epoch = cache.opening_epoch.wrapping_add(1);
        cache.task = None;
        cache.dirty = true;
        if !visible.hidden.is_empty() {
            visible.replace(HashSet::new());
        }
    }
    let started = Instant::now();
    let pending: Vec<_> = cache.pending.iter().copied().take(64).collect();
    for (index, p) in pending.into_iter().enumerate() {
        if index > 0 && started.elapsed() >= Duration::from_micros(400) {
            break;
        }
        cache.pending.remove(&p);
        let Some(section) = chunks.loaded.get(&p) else {
            continue;
        };
        if section.mesh_epoch != journal.mesh_epoch(p) {
            cache.pending.insert(p);
            continue;
        }
        let mask = face_mask(section.data.blocks(), &cache.opaque);
        cache.faces.insert(p, (section.mesh_epoch, mask));
        cache.dirty = true;
    }
    if let Some((epoch, task)) = &mut cache.task
        && let Some(hidden) = future::block_on(future::poll_once(task))
    {
        let epoch = *epoch;
        cache.task = None;
        if epoch == cache.opening_epoch && visible.hidden != hidden {
            visible.replace(hidden);
        }
    }
    // Streaming can publish new boundaries every frame. Opening still reveals
    // immediately; only the optional removal of hidden geometry is coalesced.
    if cache.task.is_none()
        && cache.dirty
        && cache.pending.is_empty()
        && cache
            .last_analysis
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(250))
    {
        cache.last_analysis = Some(Instant::now());
        let faces = cache
            .faces
            .iter()
            .map(|(&p, &(_, mask))| (p, mask))
            .collect();
        cache.task = Some((
            cache.opening_epoch,
            AsyncComputeTaskPool::get().spawn(async move {
                let _scope = terra_core::profiling::ScopeTimer::new("terrain.visibility.worker");
                hidden_sections(&faces, eye)
            }),
        ));
        cache.dirty = false;
    }
}

fn face_mask(blocks: &[u16], opaque: &[bool]) -> u8 {
    let mut mask = 0;
    for side in 0..6 {
        let axis = side / 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        'face: for a in 0..16 {
            for b in 0..16 {
                let mut p = [0; 3];
                p[axis] = if side % 2 == 0 { 0 } else { 15 };
                p[u] = a;
                p[v] = b;
                let id = blocks[p[0] + 16 * (p[1] + 16 * p[2])] as usize;
                if !opaque.get(id).copied().unwrap_or(false) {
                    mask |= 1 << side;
                    break 'face;
                }
            }
        }
    }
    mask
}

fn hidden_sections(faces: &HashMap<SectionPos, u8>, eye: SectionPos) -> HashSet<SectionPos> {
    let mut reached = HashSet::new();
    let mut queue = VecDeque::new();
    for &p in faces.keys() {
        let near = (p.x - eye.x).abs() <= 1 && (p.y - eye.y).abs() <= 1 && (p.z - eye.z).abs() <= 1;
        if near
            || SIDES
                .iter()
                .any(|&d| p.offset(d).map_or(true, |n| !faces.contains_key(&n)))
        {
            reached.insert(p);
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        for (side, &d) in SIDES.iter().enumerate() {
            let Ok(n) = p.offset(d) else {
                continue;
            };
            if faces[&p] & (1 << side) != 0
                && faces
                    .get(&n)
                    .is_some_and(|mask| mask & (1 << (side ^ 1)) != 0)
                && reached.insert(n)
            {
                queue.push_back(n);
            }
        }
    }
    // Keep the walls around reachable air, including solid sections and shadow
    // casters. Never cull just because the player's own section is solid.
    let mut visible = reached.clone();
    for p in reached {
        for d in SIDES {
            if let Ok(n) = p.offset(d) {
                visible.insert(n);
            }
        }
    }
    faces
        .keys()
        .filter(|p| !visible.contains(p))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rock() -> HashMap<SectionPos, u8> {
        (-4..=4)
            .flat_map(|x| {
                (-4..=4).flat_map(move |y| (-4..=4).map(move |z| (SectionPos::new(x, y, z), 0)))
            })
            .collect()
    }

    #[test]
    fn sealed_cave_skips_geometry_but_its_exterior_walls_remain() {
        let faces = rock();
        let hidden = hidden_sections(&faces, SectionPos::new(20, 20, 20));
        assert!(hidden.contains(&SectionPos::new(0, 0, 0)));
        assert!(!hidden.contains(&SectionPos::new(4, 0, 0)));
        assert!(!hidden.contains(&SectionPos::new(3, 0, 0)));
        let inside = hidden_sections(&faces, SectionPos::new(0, 0, 0));
        assert!(!inside.contains(&SectionPos::new(0, 0, 0)));
        assert!(!inside.contains(&SectionPos::new(2, 0, 0)));
    }

    #[test]
    fn entrance_across_negative_sections_and_unknown_neighbors_never_disappears() {
        let mut faces = rock();
        let eye = SectionPos::new(20, 20, 20);
        for x in -4..=0 {
            faces.insert(SectionPos::new(x, 0, 0), 0b11);
        }
        let hidden = hidden_sections(&faces, eye);
        for x in -4..=0 {
            assert!(!hidden.contains(&SectionPos::new(x, 0, 0)));
            assert!(!hidden.contains(&SectionPos::new(x, 1, 0)));
        }
        faces.insert(SectionPos::new(-2, 0, 0), 0);
        assert!(hidden_sections(&faces, eye).contains(&SectionPos::new(0, 0, 0)));
        faces.remove(&SectionPos::new(-1, 0, 0));
        assert!(!hidden_sections(&faces, eye).contains(&SectionPos::new(0, 0, 0)));
    }

    #[test]
    fn partial_transparent_and_unknown_materials_leave_portals_open() {
        let mut blocks = vec![1; 4096];
        let opaque = [false, true, false];
        assert_eq!(face_mask(&blocks, &opaque), 0);
        for material in [0, 2, 999] {
            blocks[16 * (8 + 16 * 8)] = material;
            assert_eq!(face_mask(&blocks, &opaque), 1);
        }
        blocks.fill(0);
        assert_eq!(face_mask(&blocks, &opaque), 63);
    }

    #[test]
    fn closing_and_loading_can_only_hide_more_but_opening_must_invalidate() {
        let mut faces = rock();
        let eye = SectionPos::new(20, 20, 20);
        for x in -4..=0 {
            faces.insert(SectionPos::new(x, 0, 0), 3);
        }
        let open = hidden_sections(&faces, eye);
        faces.insert(SectionPos::new(-2, 0, 0), 0);
        let closed = hidden_sections(&faces, eye);
        assert!(open.is_subset(&closed));
        let removed = faces.remove(&SectionPos::new(-1, 0, 0)).unwrap();
        let unknown = hidden_sections(&faces, eye);
        faces.insert(SectionPos::new(-1, 0, 0), removed);
        assert!(unknown.is_subset(&hidden_sections(&faces, eye)));
    }

    #[test]
    fn hidden_sections_release_gpu_meshes_and_cancel_pending_uploads() {
        use super::super::{
            RenderStatus, RenderedSection, RenderedSections,
            jobs::{RenderJob, RenderJobs},
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<crate::blocks::BlockAssets>()
            .init_resource::<ChunkStore>()
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<RenderedSections>()
            .init_resource::<RenderStatus>()
            .init_resource::<RenderJobs>()
            .init_resource::<TerrainVisibility>()
            .add_systems(Update, super::super::upload_sections);
        let p = SectionPos::new(-3, -2, 5);
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            p,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(p).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        let entity = app.world_mut().spawn(Mesh3d(mesh.clone())).id();
        app.world_mut()
            .resource_mut::<RenderedSections>()
            .sections
            .insert(
                p,
                RenderedSection {
                    entity,
                    meshes: vec![mesh],
                    mesh_epoch: 0,
                    appearance_epoch: 0,
                    micro_detail: false,
                    detail_level: super::super::DetailLevel::Near,
                },
            );
        app.world_mut().resource_mut::<RenderJobs>().0.insert(
            p,
            RenderJob {
                epoch: 0,
                appearance_epoch: 0,
                micro_detail: false,
                level: super::super::DetailLevel::Near,
                ready: None,
                task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
        app.world_mut()
            .resource_mut::<TerrainVisibility>()
            .replace(HashSet::from([p]));
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        assert!(app.world().resource::<Assets<Mesh>>().is_empty());
        assert!(
            app.world()
                .resource::<RenderedSections>()
                .sections
                .is_empty()
        );
        assert!(app.world().resource::<RenderJobs>().0.is_empty());
        assert!(app.world().resource::<ChunkStore>().loaded.contains_key(&p));
        let revision = app.world().resource::<TerrainVisibility>().revision;
        invalidate(app.world_mut(), &[p]);
        let visible = app.world().resource::<TerrainVisibility>();
        assert!(!visible.hidden(p));
        assert!(visible.revision > revision);
    }
}
