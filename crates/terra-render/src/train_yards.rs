//! Assembly-track markings follow the same authored path as real train wheels.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};
use std::collections::BTreeMap;
use terra_world::{edits::EditJournal, streaming::ChunkStore, train_yard::YardTrack};
#[derive(Resource, Default)]
struct Yards {
    elapsed: f32,
    views: BTreeMap<[i64; 3], (Entity, YardTrack)>,
    material: Handle<StandardMaterial>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Yards>().add_systems(Update, sync);
}
fn sync(
    mut commands: Commands,
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    mut yards: ResMut<Yards>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    yards.elapsed += time.delta_secs();
    if yards.elapsed < 0.5 {
        return;
    }
    yards.elapsed = 0.0;
    let wanted = terra_world::train_services::docks(&chunks, &journal)
        .into_iter()
        .filter_map(|station| {
            terra_world::train_yard::track(&chunks, &journal, station).map(|track| (station, track))
        })
        .take(8)
        .collect::<BTreeMap<_, _>>();
    let remove = yards
        .views
        .iter()
        .filter_map(|(station, (_, old))| (wanted.get(station) != Some(old)).then_some(*station))
        .collect::<Vec<_>>();
    for station in remove {
        if let Some((entity, _)) = yards.views.remove(&station) {
            commands.entity(entity).try_despawn();
        }
    }
    if yards.material == Handle::default() {
        yards.material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            metallic: 0.45,
            perceptual_roughness: 0.7,
            ..default()
        });
    }
    for (station, track) in wanted {
        if yards.views.contains_key(&station) {
            continue;
        }
        let mesh = meshes.add(markings(track, station));
        let entity = commands
            .spawn((
                Name::new("Assembly track service markings"),
                Mesh3d(mesh),
                MeshMaterial3d(yards.material.clone()),
                Transform::from_translation(Vec3::from_array(station.map(|v| v as f32))),
            ))
            .id();
        yards.views.insert(station, (entity, track));
    }
}
fn markings(track: YardTrack, origin: [i64; 3]) -> Mesh {
    let cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
    let Some(VertexAttributeValues::Float32x3(points)) = cube.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        unreachable!()
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = cube.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        unreachable!()
    };
    let indices = cube.indices().unwrap().iter().collect::<Vec<_>>();
    let mut positions = Vec::new();
    let mut out_normals = Vec::new();
    let mut colors = Vec::new();
    let mut triangles = Vec::new();
    let mut solid = |center: Vec3, size: Vec3, rotation: Quat, color: Color| {
        let base = positions.len() as u32;
        let color = color.to_linear().to_f32_array();
        for (point, normal) in points.iter().zip(normals) {
            positions.push((center + rotation * (Vec3::from_array(*point) * size)).to_array());
            out_normals.push((rotation * Vec3::from_array(*normal)).to_array());
            colors.push(color);
        }
        triangles.extend(indices.iter().map(|index| base + *index as u32));
    };
    let count = ((track.end - track.start) / 0.8).ceil() as usize;
    for step in 0..count {
        let distance = track.start + (step as f64 + 0.5) * 0.8;
        let sample = track.path.sample_distance(distance.min(track.end));
        if sample.tangent[1].abs() > 0.02 {
            continue;
        }
        let center = Vec3::from_array(std::array::from_fn(|i| {
            (sample.position[i] - origin[i] as f64) as f32
        }));
        let forward = Vec3::from_array(sample.tangent.map(|v| v as f32));
        let right = Vec3::new(forward.z, 0.0, -forward.x).normalize();
        let rotation = Quat::from_rotation_y(forward.x.atan2(forward.z));
        for side in [-1.0, 1.0] {
            let pad = center + right * side * 1.35 + Vec3::Y * 0.035;
            solid(
                pad,
                Vec3::new(0.48, 0.06, 0.76),
                rotation,
                Color::srgb(0.26, 0.25, 0.22),
            );
            solid(
                pad + Vec3::Y * 0.033,
                Vec3::new(0.46, 0.007, 0.12),
                rotation * Quat::from_rotation_y(0.45),
                Color::srgb(0.95, 0.63, 0.09),
            );
            if step % 5 == 0 {
                let clamp = center + right * side * 1.18 + Vec3::Y * 0.17;
                solid(
                    clamp,
                    Vec3::new(0.16, 0.28, 0.27),
                    rotation,
                    Color::srgb(0.31, 0.32, 0.31),
                );
                solid(
                    clamp - right * side * 0.15 + Vec3::Y * 0.12,
                    Vec3::new(0.42, 0.08, 0.15),
                    rotation,
                    Color::srgb(0.91, 0.61, 0.12),
                );
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, out_normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    let count = mesh.count_vertices();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count]);
    mesh.insert_indices(Indices::U32(triangles));
    mesh
}
