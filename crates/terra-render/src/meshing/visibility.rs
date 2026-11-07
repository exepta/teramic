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
pub struct TerrainVisibility {
    hidden: HashSet<SectionPos>,
    revision: u64,
}
impl TerrainVisibility {
    pub(crate) fn len(&self) -> usize {
        self.hidden.len()
    }
    /// Whether conservative portal analysis intentionally excludes this section.
    pub fn hidden(&self, p: SectionPos) -> bool {
        self.hidden.contains(&p)
    }
    pub(super) fn revision(&self) -> u64 {
        self.revision
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
        cache.graph_epoch = cache.graph_epoch.wrapping_add(1);
        cache.invalidate();
    }
    if let Some(mut visibility) = world.get_resource_mut::<TerrainVisibility>() {
        visibility.replace(HashSet::new());
    }
}

#[derive(Resource, Default)]
struct PortalCache {
    faces: HashMap<SectionPos, (u64, Portals)>,
    pending: HashSet<SectionPos>,
    opaque: Vec<bool>,
    focus: Option<SectionPos>,
    // Closing portals preserves older conservative results; opening one does not.
    opening_epoch: u64,
    graph_epoch: u64,
    reached_epoch: Option<u64>,
    reached: HashMap<SectionPos, u8>,
    dirty: bool,
    task: Option<(u64, u64, SectionPos, Task<PortalAnalysis>)>,
    last_analysis: Option<Instant>,
}

impl PortalCache {
    fn invalidate(&mut self) {
        self.task = None;
        self.opening_epoch = self.opening_epoch.wrapping_add(1);
        self.reached.clear();
        self.reached_epoch = None;
        self.dirty = true;
    }

    fn queue(&mut self, position: SectionPos) -> bool {
        let removed = self.faces.remove(&position).is_some();
        if self.pending.insert(position) || removed {
            self.graph_epoch = self.graph_epoch.wrapping_add(1);
        }
        removed
    }

    fn move_focus(&mut self, eye: SectionPos, legacy: bool) -> bool {
        if self.focus == Some(eye) {
            return false;
        }
        self.focus = Some(eye);
        legacy
            || self.reached_epoch != Some(self.graph_epoch)
            || !focus_reached(&self.faces, &self.reached, eye)
    }

    fn accepts_analysis(
        &self,
        opening_epoch: u64,
        graph_epoch: u64,
        task_eye: SectionPos,
        analysis: &PortalAnalysis,
    ) -> bool {
        opening_epoch == self.opening_epoch
            && graph_epoch == self.graph_epoch
            && self.focus.is_some_and(|eye| {
                eye == task_eye || focus_reached(&self.faces, &analysis.reached, eye)
            })
    }
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

fn smoke_legacy_control(smoke_test: bool, setting: Option<&str>) -> bool {
    smoke_test && setting == Some("1")
}

fn opaque_materials(templates: &crate::blocks::TemplateSet, legacy: bool) -> Vec<bool> {
    use crate::blocks::definitions::BlockRenderKind;

    let mut opaque = vec![false; templates.blocks.keys().copied().max().unwrap_or(0) as usize + 1];
    for (&id, block) in &templates.blocks {
        // The loader only admits opaque, complete tiled cube fallbacks here.
        // Whole models and semantically partial/transparent blocks stay open.
        let authored_cube = !legacy
            && block.render_kind == BlockRenderKind::CustomMesh
            && templates.opaque_cube_lods.contains(&id)
            && templates.cube_lods.get(&id).is_some_and(|coarse| {
                coarse.render_kind == BlockRenderKind::Cube && coarse.model.is_empty()
            });
        opaque[id as usize] = (block.render_kind == BlockRenderKind::Cube || authored_cube)
            && block.model.is_empty()
            && terra_voxel::MaterialId::from_raw(id)
                .is_some_and(terra_voxel::MaterialId::is_opaque);
    }
    opaque
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
    static LEGACY_OPACITY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        smoke_legacy_control(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_LEGACY_PORTAL_OPACITY").ok().as_deref(),
        )
    });
    static LEGACY_CONNECTIVITY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        smoke_legacy_control(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_LEGACY_PORTAL_CONNECTIVITY")
                .ok()
                .as_deref(),
        )
    });
    static LEGACY_FOCUS: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        smoke_legacy_control(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_LEGACY_PORTAL_FOCUS").ok().as_deref(),
        )
    });
    let mut invalidate = false;
    if terrain.is_changed() || assets.is_changed() || cache.opaque.is_empty() {
        *cache = PortalCache::default();
        cache.opaque = opaque_materials(templates, *LEGACY_OPACITY);
        cache.pending.extend(chunks.loaded.keys().copied());
        invalidate = true;
    }
    for event in removals.read() {
        cache.faces.remove(&event.position);
        cache.pending.remove(&event.position);
        cache.graph_epoch = cache.graph_epoch.wrapping_add(1);
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
            invalidate |= cache.queue(event.position);
        }
    }
    for event in edits.read() {
        if cache
            .faces
            .get(&event.position)
            .is_some_and(|(epoch, _)| *epoch != journal.mesh_epoch(event.position))
        {
            cache.queue(event.position);
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
            cache.queue(p);
            invalidate = true;
        }
    }
    let eye = global_to_section(focus.0.map(|v| v.floor() as i64)).0;
    invalidate |= cache.move_focus(eye, *LEGACY_FOCUS);
    if invalidate {
        cache.invalidate();
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
        let portals = if *LEGACY_CONNECTIVITY {
            Portals::legacy(face_mask(section.data.blocks(), &cache.opaque))
        } else {
            section_portals(section.data.blocks(), &cache.opaque)
        };
        cache.faces.insert(p, (section.mesh_epoch, portals));
        cache.graph_epoch = cache.graph_epoch.wrapping_add(1);
        cache.dirty = true;
    }
    if let Some((epoch, graph_epoch, task_eye, task)) = &mut cache.task
        && let Some(analysis) = future::block_on(future::poll_once(task))
    {
        let (epoch, graph_epoch, task_eye) = (*epoch, *graph_epoch, *task_eye);
        cache.task = None;
        if cache.accepts_analysis(epoch, graph_epoch, task_eye, &analysis) {
            cache.reached = analysis.reached;
            cache.reached_epoch = Some(graph_epoch);
            visible.replace(analysis.hidden);
        } else {
            cache.dirty = true;
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
            .map(|(&p, &(_, portals))| (p, portals))
            .collect();
        cache.task = Some((
            cache.opening_epoch,
            cache.graph_epoch,
            eye,
            AsyncComputeTaskPool::get().spawn(async move {
                let _scope = terra_core::profiling::ScopeTimer::new("terrain.visibility.worker");
                analyze_portals_with_boundary(&faces, eye, *LEGACY_CONNECTIVITY)
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Portals {
    open: u8,
    links: [u8; 6],
}

impl Portals {
    fn legacy(open: u8) -> Self {
        Self {
            open,
            links: std::array::from_fn(|side| if open & (1 << side) != 0 { open } else { 0 }),
        }
    }
}

fn section_portals(blocks: &[u16], opaque: &[bool]) -> Portals {
    let mut air = [0_u64; 64];
    let mut count = 0;
    for (index, &id) in blocks.iter().enumerate() {
        if !opaque.get(id as usize).copied().unwrap_or(false) {
            air[index / 64] |= 1 << (index % 64);
            count += 1;
        }
    }
    if count == 0 {
        return Portals::default();
    }
    if count == 4096 {
        return Portals::legacy(63);
    }
    let mut portals = Portals::default();
    let mut queue = [0_u16; 4096];
    for start in 0..4096 {
        if air[start / 64] & (1 << (start % 64)) == 0 {
            continue;
        }
        air[start / 64] &= !(1 << (start % 64));
        queue[0] = start as u16;
        let (mut head, mut tail, mut boundary) = (0, 1, 0_u8);
        while head < tail {
            let index = queue[head] as usize;
            head += 1;
            let [x, y, z] = [index % 16, (index / 16) % 16, index / 256];
            for (side, coordinate) in [x, y, z].into_iter().enumerate() {
                if coordinate == 0 {
                    boundary |= 1 << (side * 2);
                }
                if coordinate == 15 {
                    boundary |= 1 << (side * 2 + 1);
                }
            }
            for neighbor in [
                (x > 0).then(|| index - 1),
                (x < 15).then(|| index + 1),
                (y > 0).then(|| index - 16),
                (y < 15).then(|| index + 16),
                (z > 0).then(|| index - 256),
                (z < 15).then(|| index + 256),
            ]
            .into_iter()
            .flatten()
            {
                if air[neighbor / 64] & (1 << (neighbor % 64)) != 0 {
                    air[neighbor / 64] &= !(1 << (neighbor % 64));
                    queue[tail] = neighbor as u16;
                    tail += 1;
                }
            }
        }
        portals.open |= boundary;
        for (side, links) in portals.links.iter_mut().enumerate() {
            if boundary & (1 << side) != 0 {
                *links |= boundary;
            }
        }
    }
    portals
}

#[derive(Default)]
struct PortalAnalysis {
    hidden: HashSet<SectionPos>,
    // Reachable ports exclude the additional visible wall/shadow ring.
    reached: HashMap<SectionPos, u8>,
}

fn focus_reached(
    faces: &HashMap<SectionPos, (u64, Portals)>,
    reached: &HashMap<SectionPos, u8>,
    eye: SectionPos,
) -> bool {
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let Ok(p) = eye.offset([x, y, z]) else {
                    return false;
                };
                let Some((_, portals)) = faces.get(&p) else {
                    return false;
                };
                if reached
                    .get(&p)
                    .is_none_or(|mask| mask & portals.open != portals.open)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn analyze_portals_with_boundary(
    faces: &HashMap<SectionPos, Portals>,
    eye: SectionPos,
    legacy_boundary: bool,
) -> PortalAnalysis {
    let mut reached = HashMap::new();
    let mut queue = VecDeque::new();
    for (&p, portals) in faces {
        let near = p.x.abs_diff(eye.x) <= 1 && p.y.abs_diff(eye.y) <= 1 && p.z.abs_diff(eye.z) <= 1;
        let mut boundary = false;
        let mut entries = 0;
        for (side, &d) in SIDES.iter().enumerate() {
            if p.offset(d).map_or(true, |n| !faces.contains_key(&n)) {
                boundary = true;
                entries |= portals.links[side];
            }
        }
        if near || boundary {
            let exits = if near || legacy_boundary {
                portals.open
            } else {
                entries
            };
            // Closed boundary walls still retain their complete shadow ring;
            // unrelated inner air pockets are not exterior entry points.
            reached.insert(p, exits);
            queue.push_back((p, exits));
        }
    }
    while let Some((p, exits)) = queue.pop_front() {
        for (side, &d) in SIDES.iter().enumerate() {
            if exits & (1 << side) == 0 {
                continue;
            }
            let Ok(n) = p.offset(d) else {
                continue;
            };
            let Some(portals) = faces.get(&n) else {
                continue;
            };
            let connected = portals.links[side ^ 1];
            if connected != 0 {
                let visited = reached.entry(n).or_insert(0);
                let additional = connected & !*visited;
                if additional != 0 {
                    *visited |= additional;
                    queue.push_back((n, additional));
                }
            }
        }
    }
    let hidden = hidden_with_wall_ring(faces, &reached);
    PortalAnalysis { hidden, reached }
}

fn hidden_with_wall_ring(
    faces: &HashMap<SectionPos, Portals>,
    reached: &HashMap<SectionPos, u8>,
) -> HashSet<SectionPos> {
    if reached.len() == faces.len() {
        return HashSet::new();
    }
    // Outdoor graphs reach most sections. Test only their smaller complement
    // instead of allocating and hashing six mostly duplicate walls per entry.
    if reached.len() > faces.len() / 2 {
        return faces
            .keys()
            .filter(|p| {
                !reached.contains_key(p)
                    && !SIDES.iter().any(|&d| {
                        p.offset(d)
                            .is_ok_and(|neighbor| reached.contains_key(&neighbor))
                    })
            })
            .copied()
            .collect();
    }
    hidden_from_reached_ring(faces, reached)
}

fn hidden_from_reached_ring(
    faces: &HashMap<SectionPos, Portals>,
    reached: &HashMap<SectionPos, u8>,
) -> HashSet<SectionPos> {
    // Keep the walls around reachable air, including solid sections and shadow
    // casters. Zero-port boundary seeds also belong to this ring.
    let mut visible: HashSet<_> = reached.keys().copied().collect();
    for p in reached.keys() {
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
fn analyze_portals(faces: &HashMap<SectionPos, Portals>, eye: SectionPos) -> PortalAnalysis {
    analyze_portals_with_boundary(faces, eye, false)
}

#[cfg(test)]
fn hidden_sections(faces: &HashMap<SectionPos, u8>, eye: SectionPos) -> HashSet<SectionPos> {
    analyze_portals(
        &faces
            .iter()
            .map(|(&p, &open)| (p, Portals::legacy(open)))
            .collect(),
        eye,
    )
    .hidden
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_section(position: SectionPos) -> terra_world::streaming::LoadedSection {
        terra_world::streaming::LoadedSection {
            data: terra_voxel::BlockSection::empty(position).unwrap(),
            mesh: Some(terra_voxel::meshing::BlockMesh::default()),
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        }
    }

    #[test]
    fn section_readiness_requires_loaded_data_even_when_hidden() {
        let position = SectionPos::new(-3, -2, 5);
        let mut visibility = TerrainVisibility::default();
        visibility.replace(HashSet::from([position]));
        assert!(!super::super::RenderStatus::default().section_ready(
            position,
            None,
            Some(&visibility),
        ));
    }

    #[test]
    fn section_readiness_waits_for_visible_unconsumed_cpu_meshes() {
        let position = SectionPos::new(-3, -2, 5);
        let section = pending_section(position);
        assert!(!super::super::RenderStatus::default().section_ready(
            position,
            Some(&section),
            Some(&TerrainVisibility::default()),
        ));
    }

    #[test]
    fn section_readiness_accepts_hidden_cpu_meshes_until_the_portal_opens() {
        let position = SectionPos::new(-3, -2, 5);
        let section = pending_section(position);
        let status = super::super::RenderStatus::default();
        let mut visibility = TerrainVisibility::default();
        visibility.replace(HashSet::from([position]));
        assert!(status.section_ready(position, Some(&section), Some(&visibility)));
        visibility.replace(HashSet::new());
        assert!(!status.section_ready(position, Some(&section), Some(&visibility)));
    }

    #[test]
    fn section_readiness_keeps_pending_columns_blocked_even_when_hidden_or_consumed() {
        let position = SectionPos::new(-3, -2, 5);
        let mut section = pending_section(position);
        let mut status = super::super::RenderStatus::default();
        status.pending_columns.insert([position.x, position.z]);
        let mut visibility = TerrainVisibility::default();
        visibility.replace(HashSet::from([position]));
        assert!(!status.section_ready(position, Some(&section), Some(&visibility)));
        section.mesh = None;
        assert!(!status.section_ready(position, Some(&section), Some(&visibility)));
        status.pending_columns.clear();
        assert!(status.section_ready(position, Some(&section), Some(&visibility)));
    }

    #[test]
    fn section_readiness_accepts_consumed_visible_meshes() {
        let position = SectionPos::new(-3, -2, 5);
        let mut section = pending_section(position);
        section.mesh = None;
        let status = super::super::RenderStatus::default();
        assert!(status.section_ready(
            position,
            Some(&section),
            Some(&TerrainVisibility::default()),
        ));
        assert!(status.section_ready(position, Some(&section), None));
    }

    #[test]
    fn section_readiness_without_visibility_is_conservative() {
        let position = SectionPos::new(-3, -2, 5);
        let section = pending_section(position);
        assert!(!super::super::RenderStatus::default().section_ready(
            position,
            Some(&section),
            None,
        ));
    }

    fn rock() -> HashMap<SectionPos, u8> {
        (-4..=4)
            .flat_map(|x| {
                (-4..=4).flat_map(move |y| (-4..=4).map(move |z| (SectionPos::new(x, y, z), 0)))
            })
            .collect()
    }

    fn solid_graph(radius: i64) -> HashMap<SectionPos, Portals> {
        (-radius..=radius)
            .flat_map(|x| {
                (-radius..=radius).flat_map(move |y| {
                    (-radius..=radius).map(move |z| (SectionPos::new(x, y, z), Portals::default()))
                })
            })
            .collect()
    }

    fn cached_graph(
        faces: &HashMap<SectionPos, Portals>,
        eye: SectionPos,
    ) -> (PortalCache, TerrainVisibility) {
        let analysis = analyze_portals(faces, eye);
        let mut visible = TerrainVisibility::default();
        visible.replace(analysis.hidden);
        (
            PortalCache {
                faces: faces.iter().map(|(&p, &v)| (p, (0, v))).collect(),
                focus: Some(eye),
                graph_epoch: 7,
                reached_epoch: Some(7),
                reached: analysis.reached,
                ..default()
            },
            visible,
        )
    }

    fn connected_focus_graph() -> HashMap<SectionPos, Portals> {
        let mut faces = solid_graph(5);
        for x in -1..=2 {
            for y in -1..=1 {
                for z in -1..=1 {
                    faces.insert(SectionPos::new(x, y, z), Portals::legacy(63));
                }
            }
        }
        faces
    }

    #[test]
    fn dense_and_sparse_wall_rings_match_the_original_visibility_at_negative_borders() {
        let mut faces = solid_graph(6);
        // Irregular holes are unknown space, not implicitly visible portals.
        faces.retain(|p, _| (p.x * 7 + p.y * 11 + p.z * 13).rem_euclid(17) != 0);
        for density in [0, 1, 3, 5, 7, 9, 10] {
            let reached = faces
                .keys()
                .filter(|p| (p.x * 5 + p.y * 3 + p.z * 7).rem_euclid(10) < density)
                .map(|&p| (p, 0))
                .collect();
            assert_eq!(
                hidden_with_wall_ring(&faces, &reached),
                hidden_from_reached_ring(&faces, &reached),
                "density {density}: zero-port seeds still retain their walls"
            );
        }
        let faces = HashMap::from([
            (SectionPos::new(i64::MAX, 0, 0), Portals::default()),
            (SectionPos::new(i64::MAX - 1, 0, 0), Portals::default()),
            (SectionPos::new(i64::MIN, 0, 0), Portals::default()),
        ]);
        for reached in [
            HashMap::new(),
            HashMap::from([(SectionPos::new(i64::MAX, 0, 0), 0)]),
            HashMap::from([
                (SectionPos::new(i64::MAX - 1, 0, 0), 0),
                (SectionPos::new(i64::MIN, 0, 0), 0),
            ]),
        ] {
            assert_eq!(
                hidden_with_wall_ring(&faces, &reached),
                hidden_from_reached_ring(&faces, &reached)
            );
        }
    }

    #[test]
    #[ignore = "explicit CPU microbenchmark; no native GPU or timing threshold"]
    fn benchmark_dense_portal_wall_ring_without_changing_its_result() {
        let faces: HashMap<_, _> = (-20..=20)
            .flat_map(|x| {
                (-5..=5).flat_map(move |y| {
                    (-20..=20).map(move |z| (SectionPos::new(x, y, z), Portals::default()))
                })
            })
            .collect();
        let reached = faces
            .keys()
            .filter(|p| (p.x * 3 + p.y + p.z * 7).rem_euclid(10) != 0)
            .map(|&p| (p, 0))
            .collect();
        let expected = hidden_from_reached_ring(&faces, &reached);
        assert_eq!(hidden_with_wall_ring(&faces, &reached), expected);
        for optimized in [false, true] {
            let started = Instant::now();
            for _ in 0..64 {
                let result = if optimized {
                    hidden_with_wall_ring(&faces, &reached)
                } else {
                    hidden_from_reached_ring(&faces, &reached)
                };
                std::hint::black_box(result);
            }
            eprintln!(
                "portal_wall_ring optimized={optimized} sections={} reached={} iterations=64 total_ms={:.3}",
                faces.len(),
                reached.len(),
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    #[test]
    fn bounded_flood_keeps_disconnected_boundary_pockets_separate() {
        let opaque = [false, true];
        let mut blocks = vec![1; 4096];
        for x in 0..4 {
            blocks[x + 16 * (4 + 16 * 4)] = 0;
        }
        for x in 12..16 {
            blocks[x + 16 * (11 + 16 * 11)] = 0;
        }
        let barrier = section_portals(&blocks, &opaque);
        assert_eq!(barrier.open, 3);
        assert_eq!(barrier.links, [1, 2, 0, 0, 0, 0]);

        let mut faces = solid_graph(4);
        for x in -4..=0 {
            faces.insert(SectionPos::new(x, 0, 0), Portals::legacy(3));
        }
        faces.insert(SectionPos::new(-2, 0, 0), barrier);
        let eye = SectionPos::new(20, 20, 20);
        let analysis = analyze_portals(&faces, eye);
        assert!(analysis.hidden.contains(&SectionPos::new(0, 0, 0)));
        assert!(!analysis.hidden.contains(&SectionPos::new(-1, 0, 0)));
        assert_eq!(analysis.reached[&SectionPos::new(-2, 0, 0)], 1);
        faces.insert(SectionPos::new(-2, 0, 0), Portals::legacy(barrier.open));
        assert!(
            !analyze_portals(&faces, eye)
                .hidden
                .contains(&SectionPos::new(0, 0, 0)),
            "the old face-only graph falsely joins the two pockets"
        );

        // Join the pockets with a bent real passage; all six-neighbor turns count.
        for x in 3..=12 {
            blocks[x + 16 * (4 + 16 * 4)] = 0;
        }
        for y in 4..=11 {
            blocks[12 + 16 * (y + 16 * 4)] = 0;
        }
        for z in 4..=11 {
            blocks[12 + 16 * (11 + 16 * z)] = 0;
        }
        assert_eq!(section_portals(&blocks, &opaque), Portals::legacy(3));
        faces.insert(SectionPos::new(-2, 0, 0), section_portals(&blocks, &opaque));
        assert!(
            !analyze_portals(&faces, eye)
                .hidden
                .contains(&SectionPos::new(0, 0, 0))
        );
    }

    #[test]
    fn unknown_boundary_only_enters_air_connected_to_its_actual_missing_face() {
        let opaque = [false, true];
        let mut blocks = vec![1; 4096];
        for x in 12..16 {
            blocks[x + 16 * (11 + 16 * 11)] = 0;
        }
        let boundary = SectionPos::new(-4, 0, 0);
        let eye = SectionPos::new(20, 20, 20);
        let cave = SectionPos::new(0, 0, 0);
        let mut faces = solid_graph(4);
        for x in -3..=0 {
            faces.insert(SectionPos::new(x, 0, 0), Portals::legacy(3));
        }
        faces.insert(boundary, section_portals(&blocks, &opaque));
        let analysis = analyze_portals(&faces, eye);
        assert_eq!(analysis.reached[&boundary], 0);
        assert!(analysis.hidden.contains(&cave));
        for wall in [boundary, SectionPos::new(-3, 0, 0)] {
            assert!(
                !analysis.hidden.contains(&wall),
                "retain cliff walls and their ring"
            );
        }
        assert!(
            !analyze_portals_with_boundary(&faces, eye, true)
                .hidden
                .contains(&cave),
            "legacy boundary seeds incorrectly enter the east pocket through a closed west face"
        );

        // An unrelated west opening must not activate the separate east pocket.
        for x in 0..4 {
            blocks[x + 16 * (4 + 16 * 4)] = 0;
        }
        faces.insert(boundary, section_portals(&blocks, &opaque));
        let separated = analyze_portals(&faces, eye);
        assert_eq!(separated.reached[&boundary], 1);
        assert!(separated.hidden.contains(&cave));

        // A real bent connection from the unknown-facing west entrance restores it.
        for x in 3..=12 {
            blocks[x + 16 * (4 + 16 * 4)] = 0;
        }
        for y in 4..=11 {
            blocks[12 + 16 * (y + 16 * 4)] = 0;
        }
        for z in 4..=11 {
            blocks[12 + 16 * (11 + 16 * z)] = 0;
        }
        faces.insert(boundary, section_portals(&blocks, &opaque));
        let entrance = analyze_portals(&faces, eye);
        assert_eq!(entrance.reached[&boundary], 3);
        assert!(entrance.reached.contains_key(&cave));
        assert!(!entrance.hidden.contains(&cave));
    }

    #[test]
    fn boundary_seeds_union_all_unknown_faces_and_keep_full_near_field_seeds() {
        let opaque = [false, true];
        let mut blocks = vec![1; 4096];
        for axis in 0..16 {
            blocks[axis + 16 * (4 + 16 * 4)] = 0;
            blocks[11 + 16 * (11 + 16 * axis)] = 0;
        }
        let corner = SectionPos::new(-4, 0, -4);
        let portals = section_portals(&blocks, &opaque);
        assert_eq!(portals.links, [3, 3, 0, 0, 48, 48]);
        let mut faces = solid_graph(4);
        faces.insert(corner, portals);
        let analysis = analyze_portals(&faces, SectionPos::new(20, 20, 20));
        assert_eq!(analysis.reached[&corner], 51);

        let boundary = SectionPos::new(-4, 0, 0);
        faces.insert(boundary, Portals::legacy(2));
        assert_eq!(
            analyze_portals(&faces, SectionPos::new(20, 20, 20)).reached[&boundary],
            0
        );
        assert_eq!(
            analyze_portals(&faces, boundary).reached[&boundary],
            2,
            "the existing 27 focus sections still seed every air component"
        );
    }

    #[test]
    fn turns_and_loops_allow_a_second_entry_into_an_already_reached_section() {
        let mut faces = solid_graph(5);
        for x in -5..0 {
            faces.insert(SectionPos::new(x, 0, 0), Portals::legacy(3));
        }
        let center = SectionPos::new(0, 0, 0);
        faces.insert(
            center,
            Portals {
                open: 0b11_0011,
                links: [3, 3, 0, 0, 0b11_0000, 0b11_0000],
            },
        );
        for z in 1..=2 {
            faces.insert(SectionPos::new(0, 0, z), Portals::legacy(0b11_0000));
        }
        let eye = SectionPos::new(20, 20, 20);
        assert!(
            analyze_portals(&faces, eye)
                .hidden
                .contains(&SectionPos::new(0, 0, 2))
        );
        for (p, mask) in [
            (SectionPos::new(1, 0, 0), 0b01_0001),
            (SectionPos::new(1, 0, -1), 0b10_0001),
            (SectionPos::new(0, 0, -1), 0b10_0010),
        ] {
            faces.insert(p, Portals::legacy(mask));
        }
        let analysis = analyze_portals(&faces, eye);
        assert_eq!(analysis.reached[&center], 0b11_0011);
        assert!(analysis.reached.contains_key(&SectionPos::new(0, 0, 2)));
        assert!(!analysis.hidden.contains(&SectionPos::new(0, 0, 2)));
    }

    #[test]
    fn flood_fast_paths_and_unknown_partial_cells_are_conservative() {
        let opaque = [false, true, false];
        assert_eq!(section_portals(&vec![1; 4096], &opaque), Portals::default());
        assert_eq!(
            section_portals(&vec![0; 4096], &opaque),
            Portals::legacy(63)
        );
        for material in [0, 2, 999] {
            let mut blocks = vec![1; 4096];
            for x in 0..16 {
                blocks[x + 16 * (8 + 16 * 8)] = material;
            }
            assert_eq!(section_portals(&blocks, &opaque), Portals::legacy(3));
        }
        let mut faces = solid_graph(4);
        let unknown = SectionPos::new(-1, 0, 0);
        faces.remove(&unknown);
        faces.insert(SectionPos::new(0, 0, 0), Portals::legacy(1));
        assert!(
            analyze_portals(&faces, SectionPos::new(20, 20, 20))
                .reached
                .contains_key(&SectionPos::new(0, 0, 0)),
            "a missing summary remains an outside seed"
        );
    }

    #[test]
    fn moving_focus_with_all_ports_reached_keeps_hidden_revision_and_work() {
        let faces = connected_focus_graph();
        let eye = SectionPos::new(0, 0, 0);
        let (mut cache, mut visible) = cached_graph(&faces, eye);
        assert!(!visible.hidden.is_empty());
        let revision = visible.revision;
        let epoch = cache.opening_epoch;
        let task = AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new)
            .spawn(std::future::pending());
        cache.task = Some((epoch, cache.graph_epoch, eye, task));
        if cache.move_focus(SectionPos::new(1, 0, 0), false) {
            cache.invalidate();
            visible.replace(HashSet::new());
        }
        assert_eq!(
            visible.revision, revision,
            "no terrain release/rebuild signal"
        );
        assert_eq!(cache.opening_epoch, epoch);
        assert!(
            cache.task.is_some(),
            "a compatible running analysis is retained"
        );
        assert!(!cache.move_focus(SectionPos::new(1, 0, 0), false));
        assert!(
            cache.move_focus(eye, true),
            "legacy control invalidates focus moves"
        );
    }

    #[test]
    fn focus_reuse_never_uses_the_wall_ring_or_only_the_center_section() {
        let faces = solid_graph(5);
        let eye = SectionPos::new(0, 0, 0);
        let (mut cache, visible) = cached_graph(&faces, eye);
        let wall = SectionPos::new(2, 0, 0);
        assert!(!visible.hidden(wall));
        assert!(!cache.reached.contains_key(&wall));
        assert!(cache.reached.contains_key(&SectionPos::new(1, 0, 0)));
        assert!(cache.move_focus(SectionPos::new(1, 0, 0), false));

        let faces = connected_focus_graph();
        let (mut cache, _) = cached_graph(&faces, eye);
        // Section membership alone is insufficient for a newly seeded air pocket.
        cache.reached.insert(SectionPos::new(2, 0, 0), 1);
        assert!(cache.move_focus(SectionPos::new(1, 0, 0), false));
    }

    #[test]
    fn warps_graph_changes_and_stale_workers_cannot_publish_hidden_geometry() {
        let faces = connected_focus_graph();
        let eye = SectionPos::new(0, 0, 0);
        let analysis = analyze_portals(&faces, eye);
        let (mut cache, _) = cached_graph(&faces, eye);
        assert!(cache.accepts_analysis(0, 7, eye, &analysis));
        assert!(!cache.move_focus(SectionPos::new(1, 0, 0), false));
        assert!(cache.accepts_analysis(0, 7, eye, &analysis));
        assert!(cache.move_focus(SectionPos::new(4, 4, 4), false));
        assert!(!cache.accepts_analysis(0, 7, eye, &analysis));
        cache.invalidate();
        assert!(cache.reached.is_empty());
        assert_eq!(cache.reached_epoch, None);
        assert!(!cache.accepts_analysis(0, 7, eye, &analysis));

        let (mut cache, _) = cached_graph(&faces, eye);
        cache.queue(SectionPos::new(0, 0, 0));
        assert!(cache.move_focus(SectionPos::new(1, 0, 0), false));
        assert!(!cache.accepts_analysis(0, 7, eye, &analysis));
        assert!(cache.faces.len() < faces.len());
        assert_eq!(cache.pending.len(), 1);
        let changed = analyze_portals(
            &cache.faces.iter().map(|(&p, &(_, v))| (p, v)).collect(),
            eye,
        );
        assert!(changed.reached.len() <= cache.faces.len());
        assert!(changed.reached.keys().all(|p| cache.faces.contains_key(p)));
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
    fn legacy_portal_controls_are_only_explicit_smoke_settings() {
        for setting in [None, Some("0"), Some("1"), Some("true")] {
            assert!(!smoke_legacy_control(false, setting));
            assert_eq!(smoke_legacy_control(true, setting), setting == Some("1"));
        }
    }

    #[test]
    fn actual_authored_grass_and_proven_custom_cubes_close_but_open_shapes_do_not() {
        use crate::blocks::definitions::{BlockDefinition, BlockRenderKind};

        let mut templates = crate::blocks::authored_terrain_templates();
        assert_eq!(
            templates.blocks[&1].render_kind,
            BlockRenderKind::CustomMesh
        );
        assert!(templates.opaque_cube_lods.contains(&1));
        assert!(templates.blocks[&1].model.is_empty());
        let grass = templates.blocks[&1].clone();
        let coarse = templates.cube_lods[&1].clone();
        let stone: BlockDefinition =
            serde_json::from_str(include_str!("../../../../assets/terra/blocks/stone.json"))
                .unwrap();
        assert_eq!(stone.id, 3);
        let mut ordinary_stone = templates.blocks[&2].clone();
        ordinary_stone.render_kind = stone.render_kind;
        templates.blocks.insert(stone.id, ordinary_stone);
        for legacy in [false, true] {
            let opaque = opaque_materials(&templates, legacy);
            assert!(opaque[2] && opaque[3], "ordinary full cubes stay opaque");
            assert_eq!(opaque[1], !legacy);
        }

        // Exercise another opaque material with the same loader-proven six-face contract.
        templates.blocks.insert(3, grass.clone());
        templates.cube_lods.insert(3, coarse.clone());
        templates.opaque_cube_lods.insert(3);
        assert!(opaque_materials(&templates, false)[3]);
        assert!(!opaque_materials(&templates, true)[3]);

        // Even a cube proof cannot turn leaves, glass, an open door or a partial
        // construction material into a solid optical barrier.
        for id in [7, 15, 65, 78] {
            templates.blocks.insert(id, grass.clone());
            templates.cube_lods.insert(id, coarse.clone());
            templates.opaque_cube_lods.insert(id);
        }
        let opaque = opaque_materials(&templates, false);
        for id in [7, 15, 65, 78] {
            assert!(!opaque[id], "material {id} must keep the portal open");
        }
        let mut boundary = vec![3; 4096];
        for id in [7, 15, 65, 78, 999] {
            boundary[16 * (8 + 16 * 8)] = id;
            assert_eq!(face_mask(&boundary, &opaque), 1);
        }

        // Masked/blended authored faces never receive this loader proof.
        templates.opaque_cube_lods.remove(&1);
        assert!(!opaque_materials(&templates, false)[1]);
        templates.opaque_cube_lods.insert(1);
        templates.cube_lods.remove(&1);
        assert!(!opaque_materials(&templates, false)[1]);
        let primitive = templates.blocks[&3].faces[0][0].clone();
        templates.blocks.get_mut(&3).unwrap().model.push(primitive);
        assert!(templates.opaque_cube_lods.contains(&3));
        assert!(
            !opaque_materials(&templates, false)[3],
            "a whole custom model can have holes even with an unrelated cube fallback"
        );
    }

    #[test]
    fn grass_sealed_caves_keep_carved_entrances_negative_edges_and_cliff_walls() {
        let opaque = [false, true, true, true];
        let sealed = vec![1; 4096];
        let closed = face_mask(&sealed, &opaque);
        assert_eq!(closed, 0);
        let mut tunnel = sealed.clone();
        for x in 0..16 {
            tunnel[x + 16 * (8 + 16 * 8)] = 0;
        }
        let open = face_mask(&tunnel, &opaque);
        assert_eq!(open, 3);
        let mut faces = rock();
        let eye = SectionPos::new(20, 20, 20);
        for x in -4..=0 {
            faces.insert(SectionPos::new(x, 0, 0), open);
        }
        let visible_entrance = hidden_sections(&faces, eye);
        for x in -4..=0 {
            assert!(!visible_entrance.contains(&SectionPos::new(x, 0, 0)));
        }
        faces.insert(SectionPos::new(-2, 0, 0), closed);
        let hidden = hidden_sections(&faces, eye);
        assert!(hidden.contains(&SectionPos::new(0, 0, 0)));
        for cliff in [SectionPos::new(4, 0, 0), SectionPos::new(3, 0, 0)] {
            assert!(
                !hidden.contains(&cliff),
                "exterior cliff walls stay visible"
            );
        }
        faces.remove(&SectionPos::new(-1, 0, 0));
        assert!(!hidden_sections(&faces, eye).contains(&SectionPos::new(0, 0, 0)));
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
