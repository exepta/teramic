//! Client-only interpolation between paid fluid surfaces, never a second simulation.
//! A fixed history stores recent surface geometry, not one mesh for every liquid level.
use bevy::{
    mesh::{Indices, MeshVertexAttribute, VertexAttributeValues},
    prelude::*,
    render::render_resource::VertexFormat,
};
use std::collections::{HashMap, HashSet};
use terra_voxel::SectionPos;

pub(crate) const FROM: MeshVertexAttribute =
    MeshVertexAttribute::new("Liquid_From", 1_883_214_076, VertexFormat::Float32x4);
pub(crate) const DATA: MeshVertexAttribute =
    MeshVertexAttribute::new("Liquid_Motion", 1_883_214_077, VertexFormat::Float32x4);
pub(crate) const POUR_COORDS: MeshVertexAttribute = MeshVertexAttribute::new(
    "Bile_Pour_Coordinates",
    1_883_214_078,
    VertexFormat::Float32x4,
);

fn bile_direction(code: f32) -> IVec2 {
    match code.abs() as u8 {
        1 => IVec2::NEG_X,
        2 => IVec2::X,
        3 => IVec2::NEG_Y,
        4 => IVec2::Y,
        _ => IVec2::ZERO,
    }
}

const MAX_VERTICES: usize = 65_536;
const MAX_SURFACES: usize = 96;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum Kind {
    Water,
    Galle,
    Tar,
}
impl Kind {
    fn period(self) -> f32 {
        match self {
            Self::Water => 0.075,
            Self::Galle => 2. * Self::Water.period(),
            Self::Tar => 0.34,
        }
    }
    pub(crate) fn material(templates: &crate::blocks::TemplateSet, slot: usize) -> Option<Self> {
        if templates.bile_material(slot) {
            return Some(Self::Galle);
        }
        templates
            .blocks
            .get(&terra_core::roads::WET_TAR)
            .filter(|b| b.face_materials[3] == slot)
            .map(|_| Self::Tar)
    }
}
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct Key(pub SectionPos, pub Kind);
struct Surface {
    mesh: Mesh,
    at: f32,
    slot: usize,
}
#[derive(Resource, Default)]
pub struct FlowInterpolationStatus {
    surfaces: HashMap<Key, Surface>,
    vertices: usize,
    pub publications: u64,
    pub last_started: f32,
    pub last_duration: f32,
}
pub(crate) type History = FlowInterpolationStatus;
#[derive(Component)]
pub(crate) struct Retiring {
    key: Key,
    until: f32,
    mesh: Handle<Mesh>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<History>()
        .add_systems(PostUpdate, cleanup);
}
fn cleanup(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut history: ResMut<History>,
    chunks: Option<Res<terra_world::streaming::ChunkStore>>,
    retiring: Query<(Entity, &Retiring)>,
) {
    let now = time.elapsed_secs();
    for (entity, state) in &retiring {
        if now >= state.until || history.surfaces.contains_key(&state.key) {
            commands.entity(entity).despawn();
            meshes.remove(state.mesh.id());
        }
    }
    if let Some(chunks) = chunks
        && chunks.is_changed()
    {
        let removed: Vec<_> = history
            .surfaces
            .keys()
            .filter(|key| !chunks.loaded.contains_key(&key.0))
            .copied()
            .collect();
        for key in removed {
            history.remove(key);
        }
    }
}
fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p,
        _ => &[],
    }
}
fn normal(mesh: &Mesh, index: usize) -> Vec3 {
    match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => v
            .get(index)
            .copied()
            .map(Vec3::from_array)
            .unwrap_or(Vec3::Y),
        Some(VertexAttributeValues::Snorm8x4(v)) => v
            .get(index)
            .map(|v| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32).normalize_or_zero())
            .unwrap_or(Vec3::Y),
        _ => Vec3::Y,
    }
}
fn rendered(mesh: &Mesh, index: usize, age: f32) -> Vec3 {
    let target = Vec3::from_array(positions(mesh)[index]);
    let Some(VertexAttributeValues::Float32x4(from)) = mesh.attribute(FROM) else {
        return target;
    };
    let Some(VertexAttributeValues::Float32x4(data)) = mesh.attribute(DATA) else {
        return target;
    };
    let progress = (age / data[index][0].max(0.001)).clamp(0., 1.);
    Vec3::from_array([from[index][0], from[index][1], from[index][2]])
        .lerp(target, progress * progress * (3. - 2. * progress))
}
// Keep the same point of the advancing sheet attached to the preceding
// frame, even when the nearest old vertex belongs to a different cross-section.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct PourPoint {
    owner: IVec3,
    flow: IVec2,
    uv: [u32; 3],
}
impl PourPoint {
    fn read(mesh: &Mesh, i: usize, owner: IVec3) -> Option<Self> {
        if let Some(VertexAttributeValues::Float32x4(coords)) = mesh.attribute(POUR_COORDS) {
            let [r, c, h, code] = coords[i];
            return (code > 0.).then(|| Self {
                owner,
                flow: bile_direction(code),
                uv: [r, c, h].map(f32::to_bits),
            });
        }
        let VertexAttributeValues::Float32x4(colors) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)?
        else {
            return None;
        };
        let [r, c, height, mask] = colors[i];
        if mask != terra_world::water::POUR_SURFACE {
            return None;
        }
        let VertexAttributeValues::Float32x2(flow) = mesh.attribute(Mesh::ATTRIBUTE_UV_1)? else {
            return None;
        };
        Some(Self {
            owner,
            flow: Vec2::from_array(flow[i]).as_ivec2(),
            uv: [r, c, height].map(f32::to_bits),
        })
    }
    fn inlet(self) -> (usize, usize, f32, IVec3) {
        let axis = if self.flow.x != 0 { 0 } else { 2 };
        let sign = if axis == 0 { self.flow.x } else { self.flow.y };
        let mut upstream = self.owner;
        upstream[axis] -= sign;
        (
            axis,
            2 - axis,
            (self.owner[axis] + i32::from(sign < 0)) as f32,
            upstream,
        )
    }
}
fn anchors(mesh: &Mesh) -> Vec<Vec3> {
    let p = positions(mesh);
    let mut anchors: Vec<_> = p
        .iter()
        .map(|p| Vec3::from_array(*p).floor() + Vec3::splat(0.5))
        .collect();
    if let Some(indices) = mesh.indices() {
        let ids: Vec<_> = indices.iter().collect();
        for triangle in ids.chunks_exact(3) {
            let center = triangle
                .iter()
                .map(|&i| Vec3::from_array(p[i]))
                .sum::<Vec3>()
                / 3.;
            let cell = (center - normal(mesh, triangle[0]) * 0.0001).floor();
            for &i in triangle {
                anchors[i] = cell + Vec3::splat(0.5);
            }
        }
    }
    anchors
}
impl FlowInterpolationStatus {
    pub(crate) fn remove(&mut self, key: Key) {
        if let Some(old) = self.surfaces.remove(&key) {
            self.vertices -= positions(&old.mesh).len();
        }
    }
    #[cfg(test)]
    fn contains(&self, key: Key) -> bool {
        self.surfaces.contains_key(&key)
    }
    pub(crate) fn prepare(
        &mut self,
        key: Key,
        mesh: &mut Mesh,
        now: f32,
        animate: bool,
        slot: usize,
    ) {
        let mut count = positions(mesh).len();
        if count == 0 || count > MAX_VERTICES {
            self.remove(key);
            return;
        }
        let previous = self.surfaces.get(&key);
        // A visible patch can outlive its bounded CPU history. Without a
        // recorded previous shape, keep its new surface at its published height.
        let animate = animate && previous.is_some();
        let period = key.1.period();
        let duration = diagnostic_duration().unwrap_or_else(|| {
            previous.map_or(period, |s| (now - s.at).clamp(period * 0.75, period * 2.0))
        });
        self.publications = self.publications.saturating_add(1);
        self.last_started = now;
        self.last_duration = duration;
        let origin = key.0.origin().unwrap_or([0; 3]).map(|v| v as f32);
        let origin = Vec3::from_array(origin);
        let mut candidates: HashMap<[i32; 3], Vec<(Vec3, Vec3, IVec3)>> = HashMap::new();
        let mut unique = HashSet::new();
        let mut prior_pours = HashMap::new();
        if animate {
            for (old_key, surface) in &self.surfaces {
                if old_key.1 != key.1
                    || (old_key.0.x - key.0.x).abs() > 1
                    || (old_key.0.y - key.0.y).abs() > 1
                    || (old_key.0.z - key.0.z).abs() > 1
                {
                    continue;
                }
                let offset =
                    Vec3::from_array(old_key.0.origin().unwrap_or([0; 3]).map(|v| v as f32))
                        - origin;
                let old_anchors = anchors(&surface.mesh);
                #[allow(
                    clippy::needless_range_loop,
                    reason = "Mesh attributes and rendered() use the same vertex index."
                )]
                for i in 0..positions(&surface.mesh).len() {
                    let point = rendered(&surface.mesh, i, now - surface.at) + offset;
                    let n = normal(&surface.mesh, i);
                    let tag = [
                        (point.x * 1024.).round() as i32,
                        (point.y * 1024.).round() as i32,
                        (point.z * 1024.).round() as i32,
                        (n.x * 8.) as i32,
                        (n.y * 8.) as i32,
                        (n.z * 8.) as i32,
                    ];
                    let owner = (old_anchors[i] + offset).floor().as_ivec3();
                    if key.1 != Kind::Tar
                        && let Some(label) = PourPoint::read(&surface.mesh, i, owner)
                    {
                        prior_pours.insert(label, point);
                    }
                    // Coincident corners can belong to different cells. Keep
                    // each owner so both sides of a shared edge can reuse it.
                    if unique.insert((tag, owner)) {
                        candidates
                            .entry(point.floor().as_ivec3().to_array())
                            .or_default()
                            .push((point, n, owner));
                    }
                }
            }
        }
        let anchors = anchors(mesh);
        let mut from = Vec::with_capacity(count);
        let mut data = Vec::with_capacity(count);
        // Top and side faces often duplicate an edge vertex to carry different
        // normals. Pick one trajectory per physical point or those faces open
        // a slit between publications, especially as a sheet goes over a lip.
        let mut water_corners = HashMap::<[u32; 3], (u8, Vec3)>::new();
        for (i, &point) in positions(mesh).iter().enumerate() {
            let target = Vec3::from_array(point);
            let n = normal(mesh, i);
            let owner = anchors[i].floor().as_ivec3();
            let label = if animate && key.1 != Kind::Tar {
                PourPoint::read(mesh, i, owner)
            } else {
                None
            };
            let inlet = label.map(PourPoint::inlet);
            let mut inlet_heights = [None::<f32>; 2];
            let corresponding = label.and_then(|label| prior_pours.get(&label).copied());
            let mut best = corresponding;
            let mut distance = 2.26;
            if animate && corresponding.is_none() {
                let cell = target.floor().as_ivec3();
                for x in -1..=1 {
                    for y in -1..=1 {
                        for z in -1..=1 {
                            if let Some(points) =
                                candidates.get(&(cell + IVec3::new(x, y, z)).to_array())
                            {
                                for &(p, old_normal, old_cell) in points {
                                    // A nearby surface across a glass/stone cell
                                    // is a different pool. Never stretch its
                                    // vertices through that intervening block.
                                    if let Some((axis, cross, seam, upstream)) = inlet
                                        && old_cell == upstream
                                        && old_normal.y > 0.1
                                        && (p[axis] - seam).abs() < 0.001
                                    {
                                        let lateral = p[cross] - owner[cross] as f32;
                                        if (lateral - lateral.round()).abs() < 0.001
                                            && (-0.001..=1.001).contains(&lateral)
                                        {
                                            let height =
                                                &mut inlet_heights[lateral.round() as usize];
                                            *height = Some(height.map_or(p.y, |h| h.max(p.y)));
                                        }
                                    }
                                    if (old_cell - owner).abs().element_sum() > 1 {
                                        continue;
                                    }
                                    let d = p.distance_squared(target);
                                    if old_normal.dot(n) > 0.2 && d < distance {
                                        best = Some(p);
                                        distance = d;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let (Some((axis, cross, seam, _)), [Some(low), Some(high)]) = (inlet, inlet_heights)
            {
                // A new strip unfolds from the actual wet face with its full
                // transverse span. Collapsing to the nearest corner would pull
                // its entire middle sideways when entering a new cell.
                let mut edge = target;
                edge[axis] = seam;
                edge.y = edge
                    .y
                    .min(low + (high - low) * (target[cross] - owner[cross] as f32).clamp(0., 1.));
                best = Some(edge);
            }
            let start = if !animate {
                target
            } else {
                best.unwrap_or_else(|| {
                    let mut seed = anchors[i];
                    seed.y = seed.y - 0.5
                        + if key.1 == Kind::Tar {
                            terra_core::roads::PATH_HEIGHT
                        } else {
                            0.
                        };
                    seed
                })
            };
            let delta = target - start;
            let mut flow = Vec2::new(delta.x, delta.z).normalize_or_zero();
            if let Some(VertexAttributeValues::Float32x2(values)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_1)
                && key.1 == Kind::Water
            {
                let current = Vec2::from_array(values[i]);
                if current.length_squared() > 0.01 {
                    flow = current.normalize_or_zero();
                }
            }
            if let Some(VertexAttributeValues::Float32x4(coords)) = mesh.attribute(POUR_COORDS) {
                let current = bile_direction(coords[i][3]);
                if current != IVec2::ZERO {
                    flow = current.as_vec2();
                }
            }
            let base = anchors[i].y - 0.5
                + if key.1 == Kind::Tar {
                    terra_core::roads::PATH_HEIGHT
                } else {
                    0.
                };
            let depth = (target.y - base).clamp(0., 1.);
            if animate && key.1 != Kind::Tar {
                let priority = if corresponding.is_some() {
                    3
                } else if best.is_some() && n.y > 0. {
                    2
                } else {
                    u8::from(best.is_some())
                };
                let corner = water_corners
                    .entry(point.map(f32::to_bits))
                    .or_insert((priority, start));
                if priority > corner.0 {
                    *corner = (priority, start);
                }
            }
            from.push([start.x, start.y, start.z, now]);
            data.push([duration, flow.x, flow.y, depth]);
        }
        if !water_corners.is_empty() {
            for (source, point) in from.iter_mut().zip(positions(mesh)) {
                let start = water_corners[&point.map(f32::to_bits)].1;
                source[..3].copy_from_slice(&start.to_array());
            }
        }
        mesh.insert_attribute(FROM, from);
        mesh.insert_attribute(DATA, data);
        if animate
            && let Some(previous) = previous
            && let Some(retiring) = retiring_faces(
                previous,
                &anchors,
                key.1,
                now,
                duration,
                MAX_VERTICES - count,
            )
        {
            // A quality/LOD transition may also change packed attribute formats.
            // Merge into a temporary so an incompatible attribute can never
            // leave the newly published mesh with mismatched buffer lengths.
            let mut combined = mesh.clone();
            if combined.merge(&retiring).is_ok()
                && combined
                    .attributes()
                    .all(|(_, values)| values.len() == positions(&combined).len())
            {
                *mesh = combined;
                count = positions(mesh).len();
            }
        }
        self.remove(key);
        while self.surfaces.len() >= MAX_SURFACES || self.vertices + count > MAX_VERTICES {
            let Some(oldest) = self
                .surfaces
                .iter()
                .min_by(|a, b| a.1.at.total_cmp(&b.1.at))
                .map(|(k, _)| *k)
            else {
                break;
            };
            self.remove(oldest);
        }
        self.vertices += count;
        self.surfaces.insert(
            key,
            Surface {
                mesh: mesh.clone(),
                at: now,
                slot,
            },
        );
    }
    pub(crate) fn drain(&mut self, key: Key, now: f32) -> Option<(Mesh, f32, usize)> {
        let old = self.surfaces.remove(&key)?;
        self.vertices -= positions(&old.mesh).len();
        let anchors = anchors(&old.mesh);
        let mut mesh = old.mesh.clone();
        let duration = diagnostic_duration().unwrap_or_else(|| key.1.period());
        self.publications = self.publications.saturating_add(1);
        self.last_started = now;
        self.last_duration = duration;
        let mut target = Vec::with_capacity(anchors.len());
        let mut from = Vec::with_capacity(anchors.len());
        for (i, mut center) in anchors.into_iter().enumerate() {
            center.y = center.y - 0.5
                + if key.1 == Kind::Tar {
                    terra_core::roads::PATH_HEIGHT
                } else {
                    0.
                };
            target.push(center.to_array());
            let start = rendered(&old.mesh, i, now - old.at);
            from.push([start.x, start.y, start.z, now]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, target);
        mesh.insert_attribute(FROM, from);
        let count = positions(&mesh).len();
        mesh.insert_attribute(DATA, vec![[duration, 0., 0., 0.]; count]);
        Some((mesh, duration, old.slot))
    }
}
/// Retain disappearing cells in the same paid section mesh. Their targets are
/// zero-area triangles; a later publication drops completed triangles rather
/// than restarting their drain or accumulating permanent ghost geometry.
fn retiring_faces(
    old: &Surface,
    occupied: &[Vec3],
    kind: Kind,
    now: f32,
    duration: f32,
    capacity: usize,
) -> Option<Mesh> {
    if capacity < 3 {
        return None;
    }
    let occupied: HashSet<_> = occupied
        .iter()
        .map(|p| p.floor().as_ivec3().to_array())
        .collect();
    let old_positions = positions(&old.mesh);
    let old_anchors = anchors(&old.mesh);
    let old_data = match old.mesh.attribute(DATA) {
        Some(VertexAttributeValues::Float32x4(values)) => values,
        _ => return None,
    };
    let indices: Vec<_> = old.mesh.indices()?.iter().collect();
    let mut removed = Vec::new();
    let mut destinations = Vec::new();
    let mut remaining = Vec::new();
    for triangle in indices.chunks_exact(3) {
        if removed.len() + 3 > capacity {
            break;
        }
        let point = Vec3::from_array(old_positions[triangle[0]]);
        let collapsed = triangle
            .iter()
            .all(|&i| point.distance_squared(Vec3::from_array(old_positions[i])) < 0.0000001);
        let seconds = if collapsed {
            old_data[triangle[0]][0] - (now - old.at)
        } else {
            duration
        };
        if seconds <= 0.0 {
            continue;
        }
        let mut center = if collapsed {
            point.floor() + Vec3::splat(0.5)
        } else {
            old_anchors[triangle[0]]
        };
        if occupied.contains(&center.floor().as_ivec3().to_array()) {
            continue;
        }
        center.y = center.y - 0.5
            + if kind == Kind::Tar {
                terra_core::roads::PATH_HEIGHT
            } else {
                0.0
            };
        removed.extend(triangle.iter().map(|&i| i as u32));
        destinations.extend([center.to_array(); 3]);
        remaining.extend([seconds; 3]);
    }
    if removed.is_empty() {
        return None;
    }
    let mut retiring = old.mesh.clone();
    retiring.insert_indices(Indices::U32(removed));
    retiring.duplicate_vertices();
    let from: Vec<_> = (0..destinations.len())
        .map(|i| {
            let start = rendered(&retiring, i, now - old.at);
            [start.x, start.y, start.z, now]
        })
        .collect();
    let count = destinations.len();
    retiring.insert_attribute(Mesh::ATTRIBUTE_POSITION, destinations);
    retiring.insert_attribute(FROM, from);
    retiring.insert_attribute(
        DATA,
        remaining
            .into_iter()
            .map(|seconds| [seconds, 0.0, 0.0, 0.0])
            .collect::<Vec<_>>(),
    );
    retiring.insert_indices(Indices::U32((0..count as u32).collect()));
    Some(retiring)
}

pub(crate) fn bounds(mesh: &Mesh) -> Option<bevy::camera::primitives::Aabb> {
    let points = positions(mesh);
    if points.is_empty() {
        return None;
    }
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for &p in points {
        low = low.min(Vec3::from_array(p));
        high = high.max(Vec3::from_array(p));
    }
    if let Some(VertexAttributeValues::Float32x4(from)) = mesh.attribute(FROM) {
        for p in from {
            let p = Vec3::new(p[0], p[1], p[2]);
            low = low.min(p);
            high = high.max(p);
        }
    }
    Some(bevy::camera::primitives::Aabb::from_min_max(
        low - Vec3::splat(0.025),
        high + Vec3::splat(0.025),
    ))
}
fn diagnostic_duration() -> Option<f32> {
    static DURATION: std::sync::LazyLock<Option<f32>> = std::sync::LazyLock::new(|| {
        std::env::args()
            .any(|arg| arg.starts_with("--smoke-test"))
            .then(|| {
                std::env::var("TERRA_FLOW_TWEEN_SECONDS")
                    .ok()?
                    .parse::<f32>()
                    .ok()
            })
            .flatten()
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.05, 2.0))
    });
    *DURATION
}
pub(crate) fn retiring(key: Key, until: f32, mesh: Handle<Mesh>) -> Retiring {
    Retiring { key, until, mesh }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{Indices, PrimitiveTopology},
    };
    fn surface(x: f32, height: f32) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [x, height, 0.],
                [x, height, 1.],
                [x + 1., height, 1.],
                [x + 1., height, 0.],
            ],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 4])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
    }
    #[test]
    fn top_and_wall_follow_one_path_at_each_lip_vertex() {
        let key = Key(SectionPos::default(), Kind::Water);
        let mut history = History::default();
        history.prepare(key, &mut surface(0., 0.75), 1., false, 0);
        let mut next = surface(0.5, 0.5);
        let mut wall = surface(0.5, 0.5);
        wall.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[1.5, 0.5, 0.], [1.5, 0.5, 1.], [1.5, 0., 1.], [1.5, 0., 0.]],
        );
        wall.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[1., 0., 0.]; 4]);
        next.merge(&wall).unwrap();
        history.prepare(key, &mut next, 1.1, true, 0);
        for age in [0., 0.016, 0.04, 0.09] {
            assert_eq!(rendered(&next, 3, age), rendered(&next, 4, age));
            assert_eq!(rendered(&next, 2, age), rendered(&next, 5, age));
        }
    }

    #[test]
    fn poured_sheet_unfolds_from_the_wet_edge_and_keeps_its_transverse_coordinates() {
        let key = Key(SectionPos::default(), Kind::Water);
        let mut history = History::default();
        history.prepare(key, &mut surface(0., 0.75), 1., false, 0);
        let sheet = |width: f32| {
            let mut mesh = surface(1., 0.5);
            let uv = [[0., 0.25], [0., 0.75], [1., 0.75], [1., 0.25]];
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                uv.map(|[r, c]| [1. + width * r, 0.75 - 0.55 * r, c])
                    .to_vec(),
            );
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_COLOR,
                uv.map(|[r, c]| [r, c, 1., terra_world::water::POUR_SURFACE])
                    .to_vec(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[1., 0.]; 4]);
            mesh
        };
        let mut first = sheet(0.2);
        history.prepare(key, &mut first, 1.3, true, 0);
        assert_eq!(rendered(&first, 2, 0.), Vec3::new(1., 0.75 - 0.55, 0.75));
        let mut next = sheet(0.85);
        let before = rendered(&first, 2, 0.06);
        history.prepare(key, &mut next, 1.36, true, 0);
        assert!(rendered(&next, 2, 0.).distance(before) < 0.00001);
        let middle = rendered(&next, 2, 0.05);
        assert!(middle.x > before.x && middle.x < 1.85);
        assert_eq!(middle.z, 0.75);
    }

    #[test]
    fn expanding_front_has_intermediate_positions_and_resamples_a_pending_transition() {
        let key = Key(SectionPos::new(0, 0, 0), Kind::Water);
        let mut history = History::default();
        let mut first = surface(0., 0.5);
        history.prepare(key, &mut first, 10., false, 0);
        let mut next = surface(0.5, 1.);
        history.prepare(key, &mut next, 10.1, true, 0);
        let start = rendered(&next, 2, 0.);
        let middle = rendered(&next, 2, 0.05);
        let end = rendered(&next, 2, 0.2);
        assert!(start.x < middle.x && middle.x < end.x);
        assert!(start.y < middle.y && middle.y < end.y);
        let before = rendered(&next, 2, 0.05);
        let mut third = surface(0.75, 1.);
        history.prepare(key, &mut third, 10.15, true, 0);
        assert!(rendered(&third, 2, 0.).distance(before) < 0.001);
    }
    #[test]
    fn a_new_pool_across_an_intervening_glass_cell_does_not_borrow_the_old_pool_surface() {
        let key = Key(SectionPos::default(), Kind::Water);
        let mut history = History::default();
        let mut left = surface(0., 0.75);
        history.prepare(key, &mut left, 10., false, 0);
        // A full glass cell occupies x=1..2. The server has independently
        // confirmed a newly poured pool on the far side at x=2..3.
        let mut right = surface(2., 0.75);
        history.prepare(key, &mut right, 10.1, true, 0);
        for vertex in 0..4 {
            assert!(rendered(&right, vertex, 0.).x >= 2.);
            assert!(rendered(&right, vertex, 0.05).x >= 2.);
        }
    }
    #[test]
    fn draining_keeps_a_continuous_surface_then_releases_its_history() {
        let key = Key(SectionPos::new(0, 0, 0), Kind::Galle);
        let mut history = History::default();
        let mut mesh = surface(0., 0.8);
        history.prepare(key, &mut mesh, 50., false, 0);
        let (draining, duration, _) = history.drain(key, 51.).unwrap();
        let start = rendered(&draining, 0, 0.);
        let middle = rendered(&draining, 0, duration * 0.5);
        let end = rendered(&draining, 0, duration);
        assert!(start.y > middle.y && middle.y > end.y);
        assert!(history.surfaces.is_empty() && history.vertices == 0);
        assert_eq!(history.publications, 2);
        assert_eq!(history.last_started, 51.0);
    }
    #[test]
    fn removed_cell_drains_while_source_remains_without_extending_its_deadline() {
        let key = Key(SectionPos::new(0, 0, 0), Kind::Galle);
        let mut history = History::default();
        let mut both = surface(0., 0.8);
        both.merge(&surface(2., 0.8)).unwrap();
        history.prepare(key, &mut both, 10., false, 0);
        let mut source = surface(0., 0.8);
        history.prepare(key, &mut source, 11., true, 0);
        let duration = history.last_duration;
        assert_eq!(positions(&source).len(), 10);
        let start = rendered(&source, 4, 0.);
        let middle = rendered(&source, 4, duration * 0.5);
        let end = rendered(&source, 4, duration);
        assert!(start.y > middle.y && middle.y > end.y);
        assert_eq!(rendered(&source, 0, duration * 0.5).y, 0.8);
        let mut updated = surface(0., 0.8);
        history.prepare(key, &mut updated, 11. + duration * 0.5, true, 0);
        assert!(rendered(&updated, 4, 0.).distance(middle) < 0.001);
        assert!(rendered(&updated, 4, duration * 0.5).distance(end) < 0.001);
        let mut settled = surface(0., 0.8);
        history.prepare(key, &mut settled, 11. + duration + 0.01, true, 0);
        assert_eq!(positions(&settled).len(), 4);
        assert_eq!(history.vertices, 4);
    }
    #[test]
    fn publication_rejects_retirement_with_incomplete_new_vertex_attributes() {
        let key = Key(SectionPos::new(0, 0, 0), Kind::Water);
        let mut history = History::default();
        let mut both = surface(0., 0.8);
        both.merge(&surface(2., 0.8)).unwrap();
        history.prepare(key, &mut both, 10., false, 0);
        // An appearance/quality change adds a channel not present in the old
        // capture. Bevy's merge silently leaves that channel shorter than the
        // combined positions; publishing it would invalidate GPU indices.
        let mut source =
            surface(0., 0.8).with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.; 2]; 4]);
        history.prepare(key, &mut source, 11., true, 0);
        assert_eq!(positions(&source).len(), 4);
        assert!(source.attributes().all(|(_, values)| values.len() == 4));
        assert_eq!(history.vertices, 4);
    }
    #[test]
    fn render_timestamps_cross_the_global_shader_hour_without_replaying() {
        let key = Key(SectionPos::new(0, 0, 0), Kind::Water);
        let mut history = History::default();
        let mut first = surface(0., 0.5);
        history.prepare(key, &mut first, 3599.95, false, 0);
        let mut next = surface(0.5, 1.);
        history.prepare(key, &mut next, 3600.05, true, 0);
        let Some(VertexAttributeValues::Float32x4(from)) = next.attribute(FROM) else {
            panic!("missing render timestamp")
        };
        assert!(from.iter().all(|value| value[3] > 3600.));
        assert!(
            rendered(&next, 2, history.last_duration * 0.5).x
                < rendered(&next, 2, history.last_duration).x
        );
    }
    #[test]
    fn an_evicted_history_does_not_refill_a_visible_pool_from_its_bed() {
        let key = Key(SectionPos::default(), Kind::Water);
        let mut history = History::default();
        let mut pool = surface(0., 0.8);
        history.prepare(key, &mut pool, 2., false, 0);
        history.remove(key);
        let mut replacement = surface(0., 0.75);
        history.prepare(key, &mut replacement, 5., true, 0);
        for (i, &p) in positions(&replacement).iter().enumerate() {
            assert_eq!(rendered(&replacement, i, 0.), Vec3::from_array(p));
        }
    }
    #[test]
    fn a_rewetted_section_releases_its_overlapping_drain_mesh_in_the_same_frame() {
        let key = Key(SectionPos::default(), Kind::Water);
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>().init_resource::<Time>();
        install(&mut app);
        let mut pool = surface(0., 1.);
        app.world_mut()
            .resource_mut::<History>()
            .prepare(key, &mut pool, 0., false, 0);
        let (drain, seconds, _) = app
            .world_mut()
            .resource_mut::<History>()
            .drain(key, 0.)
            .unwrap();
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(drain);
        let entity = app
            .world_mut()
            .spawn(retiring(key, seconds, handle.clone()))
            .id();
        app.update();
        assert!(app.world().get_entity(entity).is_ok());
        app.world_mut()
            .resource_mut::<History>()
            .prepare(key, &mut pool, 0.02, false, 0);
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        assert!(!app.world().resource::<Assets<Mesh>>().contains(handle.id()));
    }
    #[test]
    fn history_is_bounded_and_prioritizes_recently_changed_sections() {
        let mut history = History::default();
        for x in 0..MAX_SURFACES as i64 + 8 {
            let mut mesh = surface(0., 0.5);
            history.prepare(
                Key(SectionPos::new(x, 0, 0), Kind::Water),
                &mut mesh,
                x as f32,
                false,
                0,
            );
            assert!(history.vertices <= MAX_VERTICES && history.surfaces.len() <= MAX_SURFACES);
        }
        assert!(!history.contains(Key(SectionPos::new(0, 0, 0), Kind::Water)));
        assert!(history.contains(Key(
            SectionPos::new(MAX_SURFACES as i64 + 7, 0, 0),
            Kind::Water
        )));
    }
}
