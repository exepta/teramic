//! Cosmetic surface fractures: the authoritative block stays solid until removal.
use super::{BuildingStatus, BuildingWorld};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_network::protocol::session::MiningTarget;
use terra_player::interaction::MiningVisual;
use terra_voxel::MaterialId;
use terra_world::streaming::ChunkStore;

#[derive(Component)]
pub(super) struct FractureOverlay;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    position: [i64; 3],
    material: u16,
    stage: usize,
    faces: u8,
}

#[derive(Resource, Default)]
pub(super) struct FractureState {
    key: Option<Key>,
    mesh: Option<Handle<Mesh>>,
}

pub(super) fn stage(progress: f32) -> usize {
    if !progress.is_finite() || progress < 0.025 {
        0
    } else {
        (progress.clamp(0.0, 1.0) * 12.0).ceil() as usize
    }
}

pub(super) fn exposed_faces(chunks: &ChunkStore, position: [i64; 3], material: u16) -> u8 {
    (0..6).fold(0, |mask, side| {
        let mut neighbor = position;
        neighbor[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
        mask | if chunks.block_at(neighbor).is_some_and(|id| {
            id == 0
                || (id != material
                    && MaterialId::from_raw(id).is_some_and(|neighbor| !neighbor.is_opaque()))
        }) {
            1 << side
        } else {
            0
        }
    })
}

pub(super) fn animate(
    mut commands: Commands,
    world: BuildingWorld,
    mining: Res<MiningVisual>,
    mut state: ResMut<FractureState>,
    mut meshes: ResMut<Assets<Mesh>>,
    overlay: Query<Entity, With<FractureOverlay>>,
    mut status: ResMut<BuildingStatus>,
) {
    let key = mining
        .target
        .filter(|_| world.active() && world.input.captured && !world.blocked.0)
        .and_then(|target| match target {
            MiningTarget::Block { position, expected }
                if stage(mining.progress) > 0
                    && world
                        .blocks
                        .templates
                        .as_ref()
                        .and_then(|templates| templates.blocks.get(&expected))
                        .is_none_or(|template| template.model.is_empty()) =>
            {
                Some(Key {
                    position,
                    material: expected,
                    stage: stage(mining.progress),
                    faces: exposed_faces(&world.chunks, position, expected),
                })
            }
            _ => None,
        });
    if state.key == key {
        if let Some(key) = key
            && let Ok(entity) = overlay.single()
        {
            commands.entity(entity).insert(overlay_transform(
                key.position,
                status.isolated_block,
                world.time.elapsed_secs(),
                mining.progress,
            ));
        }
        return;
    }
    state.key = key;
    let Some(key) = key else {
        for entity in &overlay {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = state.mesh.take() {
            meshes.remove(mesh.id());
        }
        status.crack_segments = 0;
        return;
    };
    let (mesh, segments) = geometry(key.position, key.stage, key.faces);
    status.crack_segments = segments;
    if mesh.count_vertices() == 0 {
        // Bevy's allocator does not allocate zero-byte slabs. Never enqueue an
        // empty mesh upload during the initial stage or when every face is hidden.
        for entity in &overlay {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = state.mesh.take() {
            meshes.remove(mesh.id());
        }
        return;
    }
    let handle = if let Some(handle) = &state.mesh {
        if let Some(mut current) = meshes.get_mut(handle) {
            *current = mesh;
        }
        handle.clone()
    } else {
        let handle = meshes.add(mesh);
        state.mesh = Some(handle.clone());
        handle
    };
    let transform = overlay_transform(
        key.position,
        status.isolated_block,
        world.time.elapsed_secs(),
        mining.progress,
    );
    if let Ok(entity) = overlay.single() {
        commands.entity(entity).insert(transform);
    } else if overlay.is_empty() {
        commands.spawn((
            FractureOverlay,
            Mesh3d(handle),
            MeshMaterial3d(world.assets.fracture.clone()),
            transform,
            NotShadowCaster,
        ));
    }
}

fn overlay_transform(position: [i64; 3], isolated: bool, seconds: f32, progress: f32) -> Transform {
    let mut transform =
        Transform::from_translation(Vec3::from_array(position.map(|v| v as f32 + 0.5)));
    if isolated {
        let phase = seconds * 34.0;
        transform.translation += Vec3::new(phase.sin(), (phase * 1.3).cos(), (phase * 0.7).sin())
            * (0.002 + progress.clamp(0.0, 1.0) * 0.003);
        transform.scale = Vec3::splat(0.86);
    }
    transform
}

fn hash(position: [i64; 3], salt: usize) -> u32 {
    position.into_iter().fold(salt as u32, |value, axis| {
        let value = value ^ (axis as u32).wrapping_mul(0x9e37_79b9);
        (value ^ (value >> 16)).wrapping_mul(0x7feb_352d)
    })
}

pub(super) fn geometry(position: [i64; 3], stage: usize, faces: u8) -> (Mesh, usize) {
    let mut shape = Shape::default();
    let mut segments = 0;
    for side in 0..6 {
        if faces & (1 << side) == 0 {
            continue;
        }
        for (index, [a, b]) in fracture_edges(position, side).into_iter().enumerate() {
            let center = (a + b) * 0.5;
            let seed = hash(position, 301 + side * 101 + index * 17);
            let start = 2 + (center.length() * 8.0) as usize + (seed % 2) as usize;
            if stage < start {
                continue;
            }
            let age = (stage - start + 1) as f32;
            let growth = (age / 3.0).min(1.0);
            let amount = age / (13 - start) as f32;
            let direction = (b - a).normalize_or_zero();
            let perpendicular = Vec2::new(-direction.y, direction.x);
            let middle = a.lerp(b, 0.35 + (seed % 31) as f32 * 0.01)
                + perpendicular * (((seed >> 8) % 17) as f32 - 8.0) * 0.001;
            let width = 0.00065 + amount * amount * (0.006 + (seed % 5) as f32 * 0.0015);
            shape.fissure(side, center.lerp(a, growth), middle, width);
            shape.fissure(side, middle, center.lerp(b, growth), width * 0.72);
            segments += 2;
        }
    }
    (shape.into_mesh(), segments)
}

fn fracture_edges(position: [i64; 3], side: usize) -> Vec<[Vec2; 2]> {
    let mut sites = vec![Vec2::new(-0.025, 0.018)];
    for index in 0..128 {
        let seed = hash(position, 59 + side * 137 + index * 131);
        let point = Vec2::new((seed & 65535) as f32, (seed >> 16) as f32) / 65535.0 * 0.86
            - Vec2::splat(0.43);
        if sites
            .iter()
            .all(|site| point.distance_squared(*site) > 0.018)
        {
            sites.push(point);
            if sites.len() == 10 {
                break;
            }
        }
    }
    let mut edges = Vec::new();
    for (index, &site) in sites.iter().enumerate() {
        let mut polygon = vec![
            Vec2::new(-0.46, -0.46),
            Vec2::new(0.46, -0.46),
            Vec2::new(0.46, 0.46),
            Vec2::new(-0.46, 0.46),
        ];
        // Half-plane clipping gives irregular, connected fracture cells without
        // a grid or a radial star. Ten sites keep the single-target work bounded.
        for (other_index, &other) in sites.iter().enumerate() {
            if other_index != index {
                polygon = clip_cell(
                    polygon,
                    other - site,
                    (other.length_squared() - site.length_squared()) * 0.5,
                );
            }
        }
        for edge in 0..polygon.len() {
            let a = polygon[edge];
            let b = polygon[(edge + 1) % polygon.len()];
            if a.distance_squared(b) < 0.0001
                || (0..2).any(|axis| {
                    (a[axis].abs() - 0.46).abs() < 0.00001 && (a[axis] - b[axis]).abs() < 0.00001
                })
            {
                continue;
            }
            let midpoint = (a + b) * 0.5;
            let distance = midpoint.distance_squared(site);
            let neighbor = sites
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .min_by(|(_, a), (_, b)| {
                    (midpoint.distance_squared(**a) - distance)
                        .abs()
                        .total_cmp(&(midpoint.distance_squared(**b) - distance).abs())
                })
                .map(|(i, _)| i);
            if neighbor.is_some_and(|neighbor| neighbor > index) {
                edges.push([a, b]);
            }
        }
    }
    edges
}

fn clip_cell(polygon: Vec<Vec2>, normal: Vec2, limit: f32) -> Vec<Vec2> {
    let mut result = Vec::with_capacity(polygon.len() + 1);
    for edge in 0..polygon.len() {
        let a = polygon[edge];
        let b = polygon[(edge + 1) % polygon.len()];
        let da = a.dot(normal) - limit;
        let db = b.dot(normal) - limit;
        if da <= 0.0 {
            result.push(a);
        }
        if (da <= 0.0) != (db <= 0.0) {
            result.push(a.lerp(b, da / (da - db)));
        }
    }
    result
}

#[derive(Default)]
struct Shape {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Shape {
    fn quad(&mut self, side: usize, points: [Vec2; 4], colors: [[f32; 4]; 4], plane: f32) {
        let axis = side / 2;
        let mut normal = Vec3::ZERO;
        normal[axis] = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
        let base = self.positions.len() as u32;
        let points = points.map(|p| {
            let mut point = normal * plane;
            point[(axis + 1) % 3] = p.x;
            point[(axis + 2) % 3] = p.y;
            point
        });
        let front = (points[1] - points[0])
            .cross(points[2] - points[0])
            .dot(normal)
            >= 0.0;
        self.positions.extend(points.map(|p| p.to_array()));
        self.normals.extend([normal.to_array(); 4]);
        self.colors.extend(colors);
        self.indices.extend(
            if front {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            }
            .map(|index| base + index),
        );
    }

    fn fissure(&mut self, side: usize, a: Vec2, b: Vec2, width: f32) {
        let direction = (b - a).normalize_or_zero();
        let normal = Vec2::new(-direction.y, direction.x);
        let outer = normal * (width + 0.0008);
        let inner = normal * width;
        let dark = [0.0025, 0.003, 0.002, 0.94];
        let rim = [0.025, 0.030, 0.023, 0.60];
        self.quad(
            side,
            [a - outer, b - outer, b + outer, a + outer],
            [rim; 4],
            0.502,
        );
        self.quad(
            side,
            [a - inner, b - inner, b + inner, a + inner],
            [dark; 4],
            0.5025,
        );
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn fracture_cells_are_deterministic_irregular_and_do_not_outline_the_face() {
        for position in [[0; 3], [-2, 8, -3], [8, 32, 24]] {
            for side in 0..6 {
                let edges = fracture_edges(position, side);
                assert_eq!(edges, fracture_edges(position, side));
                assert!((8..=27).contains(&edges.len()));
                assert!(edges.iter().all(|points| {
                    points
                        .iter()
                        .all(|point| point.is_finite() && point.abs().max_element() < 0.461)
                }));
                let diagonal = edges
                    .iter()
                    .filter(|[a, b]| {
                        let direction = (*b - *a).normalize();
                        direction.x.abs() > 0.05 && direction.y.abs() > 0.05
                    })
                    .count();
                assert!(
                    diagonal * 4 >= edges.len() * 3,
                    "fractures must not become a grid"
                );
            }
        }
        assert_ne!(fracture_edges([0; 3], 3), fracture_edges([1, 0, 0], 3));
    }

    #[test]
    fn fracture_colors_are_subtle_dark_recesses_not_bright_corner_pins() {
        let (mesh, _) = geometry([8, 32, 24], 12, 63);
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("Fracture shading missing");
        };
        assert!(!colors.is_empty());
        assert!(
            colors
                .iter()
                .all(|color| color[..3].iter().all(|v| (0.0..=0.035).contains(v)))
        );
    }
}
