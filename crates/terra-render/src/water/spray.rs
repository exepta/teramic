//! One bounded billboard batch for nearby waterfall impacts, with no additional pass.
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::BTreeMap;
use terra_core::{GameConfig, WaterQuality};
use terra_player::TerraCamera;

const MAX_SOURCES: usize = 16;
const MAX_PARTICLES: usize = 96;
const RANGE: f32 = 48.0;

#[derive(Component, Default)]
pub(crate) struct SpraySources(pub(crate) Vec<Vec4>);

pub(super) fn sources(positions: &[[f32; 3]], effects: &[[f32; 4]]) -> SpraySources {
    let mut unique = BTreeMap::new();
    for (&position, &effect) in positions.iter().zip(effects) {
        let natural = effect[3] == 2.0 && effect[0] <= 0.05 && effect[2] >= 5.0;
        let fluid = effect[3] == 5.0 && effect[1] <= 0.05 && effect[0] >= 1.0;
        if !natural && !fluid {
            continue;
        }
        let key = position.map(|p| (p * 0.5).floor() as i32);
        unique.entry(key).or_insert_with(|| {
            Vec3::from_array(position).extend(if fluid { 5.0 } else { effect[2] })
        });
    }
    SpraySources(unique.into_values().take(MAX_SOURCES).collect())
}

#[derive(Component)]
struct SprayBatch;

#[derive(Resource, Default)]
pub(super) struct SprayMesh(Option<Handle<Mesh>>);

pub(super) fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut pixels = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let p = Vec2::new(x as f32 - 7.5, y as f32 - 7.5) / 7.5;
            let alpha = (1.0 - p.length_squared()).max(0.0).powi(2);
            pixels.extend([233, 244, 240, (alpha * 255.0) as u8]);
        }
    }
    let texture = images.add(Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(texture),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        fog_enabled: true,
        ..default()
    });
    commands.spawn((
        Name::new("Waterfall spray"),
        SprayBatch,
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        // The mesh follows nearby emitters and supplies its own distance culling.
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.init_resource::<SprayMesh>();
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct SprayScene<'w, 's> {
    sources: Query<'w, 's, (&'static SpraySources, &'static Transform), Without<SprayBatch>>,
    camera: Query<'w, 's, &'static GlobalTransform, With<TerraCamera>>,
    batch: Query<'w, 's, (Entity, &'static mut Visibility), With<SprayBatch>>,
}

pub(super) fn update(
    config: Res<GameConfig>,
    time: Res<Time>,
    mut scene: SprayScene,
    mut handle: ResMut<SprayMesh>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut status: ResMut<super::WaterStatus>,
    mut commands: Commands,
) {
    status.spray_particles = 0;
    let Ok((entity, mut visibility)) = scene.batch.single_mut() else {
        return;
    };
    let Ok(camera) = scene.camera.single() else {
        return;
    };
    let budget = match config.client.water_quality {
        WaterQuality::Low => 0,
        WaterQuality::Medium => MAX_PARTICLES / 2,
        WaterQuality::High => MAX_PARTICLES,
    };
    if !config.world.water || budget == 0 {
        *visibility = Visibility::Hidden;
        return;
    }
    let eye = camera.translation();
    let mut emitters: Vec<_> = scene
        .sources
        .iter()
        .flat_map(|(sources, transform)| {
            sources
                .0
                .iter()
                .map(move |p| transform.transform_point(p.truncate()).extend(p.w))
        })
        .filter(|p| p.truncate().distance_squared(eye) <= RANGE * RANGE)
        .collect();
    emitters.sort_by(|a, b| {
        a.truncate()
            .distance_squared(eye)
            .total_cmp(&b.truncate().distance_squared(eye))
    });
    emitters.truncate(MAX_SOURCES);
    if emitters.is_empty() {
        *visibility = Visibility::Hidden;
        return;
    }
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    let particles: Vec<_> = emitters
        .iter()
        .copied()
        .flat_map(|p| (0..if p.w < 0.0 { 1 } else { 6 }).map(move |i| (p, i)))
        .take(budget)
        .collect();
    let count = particles.len();
    status.spray_particles = count;
    let mut positions = Vec::with_capacity(count * 4);
    let mut colors = Vec::with_capacity(count * 4);
    let mut uvs = Vec::with_capacity(count * 4);
    let mut indices = Vec::with_capacity(count * 6);
    for (emitter, copy) in particles {
        let ticket = emitter.x * 0.173 + emitter.z * 0.231 + copy as f32 * 0.618;
        let age = (time.elapsed_secs() * 0.55 + ticket).rem_euclid(1.0);
        let angle = ticket * std::f32::consts::TAU;
        let strength = (emitter.w / 18.0).clamp(0.45, 1.0);
        let drip = emitter.w < 0.0;
        let age = if drip {
            (time.elapsed_secs() * 0.27 + ticket).rem_euclid(1.0) * 3.0
        } else {
            age
        };
        let center = emitter.truncate()
            + Vec3::new(
                if drip { 0.0 } else { angle.sin() * age * 0.46 },
                if drip {
                    -age * age * emitter.w.abs()
                } else {
                    0.05 + age * (0.45 + strength * 0.55)
                },
                if drip { 0.0 } else { angle.cos() * age * 0.46 },
            );
        let half = if drip { 0.015 } else { 0.055 + age * 0.15 };
        let fade = if drip && age >= 1.0 {
            0.0
        } else {
            (std::f32::consts::PI * age).sin().max(0.0)
        } * (1.0 - center.distance(eye) / RANGE).clamp(0.0, 1.0);
        let base = positions.len() as u32;
        for [x, y] in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            positions.push((center + right * x * half + up * y * half).to_array());
            uvs.push([(x + 1.0) * 0.5, (y + 1.0) * 0.5]);
            colors.push([1.0, 1.0, 1.0, fade * if drip { 0.60 } else { 0.20 }]);
        }
        indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }
    // Do not register empty meshes: Bevy's allocator skips their allocation but
    // still attempts a buffer copy. Keep the populated asset while hidden.
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![(-camera.forward()).to_array(); count * 4],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    if let Some(mut existing) = handle.0.as_ref().and_then(|handle| meshes.get_mut(handle)) {
        *existing = mesh;
    } else {
        let mesh = meshes.add(mesh);
        commands.entity(entity).insert(Mesh3d(mesh.clone()));
        handle.0 = Some(mesh);
    }
    *visibility = Visibility::Visible;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_real_impact_edges_emit_and_sources_are_bounded_and_deterministic() {
        let positions: Vec<_> = (0..128).map(|x| [x as f32, 0.0, 1.0]).collect();
        let effects = vec![[0.0, 0.0, 30.0, 2.0]; 128];
        let first = sources(&positions, &effects);
        assert_eq!(first.0.len(), MAX_SOURCES);
        assert_eq!(first.0, sources(&positions, &effects).0);
        for effect in [
            [0.0; 4],
            [0.0, 0.0, 4.0, 2.0],
            [1.0, 0.0, 30.0, 2.0],
            [0.0, 0.0, 30.0, 1.0],
        ] {
            assert!(sources(&positions, &vec![effect; 128]).0.is_empty());
        }
        assert!(
            !sources(&positions, &vec![[8.0, 0.0, 1.0, 5.0]; 128])
                .0
                .is_empty()
        );
        assert!(
            sources(&positions, &vec![[8.0, 8.0, 1.0, 5.0]; 128])
                .0
                .is_empty()
        );
    }

    #[test]
    fn spray_reuses_one_mesh_and_obeys_quality_range_and_unload() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<SprayMesh>()
            .init_resource::<GameConfig>()
            .init_resource::<Time>()
            .init_resource::<super::super::WaterStatus>()
            .add_systems(Update, update);
        app.world_mut().spawn((SprayBatch, Visibility::Hidden));
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                GlobalTransform::from_translation(Vec3::new(0.0, 2.0, 4.0)),
            ))
            .id();
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        assert!(app.world().resource::<SprayMesh>().0.is_none());
        let source = app
            .world_mut()
            .spawn((
                SpraySources(
                    (0..16)
                        .map(|i| Vec4::new(i as f32 * 0.2, 0.0, 0.0, 30.0))
                        .collect(),
                ),
                Transform::default(),
            ))
            .id();
        for (quality, expected) in [
            (WaterQuality::High, 96),
            (WaterQuality::Medium, 48),
            (WaterQuality::Low, 0),
        ] {
            app.world_mut()
                .resource_mut::<GameConfig>()
                .client
                .water_quality = quality;
            app.update();
            assert_eq!(
                app.world()
                    .resource::<super::super::WaterStatus>()
                    .spray_particles,
                expected
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
            if expected > 0 {
                let meshes = app.world().resource::<Assets<Mesh>>();
                let handle = app.world().resource::<SprayMesh>().0.as_ref().unwrap();
                let mesh = meshes.get(handle).unwrap();
                assert_eq!(mesh.count_vertices(), expected * 4);
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|index| index < expected * 4)
                );
            }
        }
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .water_quality = WaterQuality::High;
        app.world_mut()
            .entity_mut(camera)
            .insert(GlobalTransform::from_translation(Vec3::splat(100.0)));
        app.update();
        assert_eq!(
            app.world()
                .resource::<super::super::WaterStatus>()
                .spray_particles,
            0
        );
        app.world_mut().entity_mut(source).despawn();
        app.world_mut()
            .entity_mut(camera)
            .insert(GlobalTransform::IDENTITY);
        app.update();
        assert_eq!(
            app.world()
                .resource::<super::super::WaterStatus>()
                .spray_particles,
            0
        );
    }
}
