//! Bounded cosmetic water contacts: a shared mesh, never an entity per droplet.
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::VecDeque;
use terra_core::footsteps::{WaterMotion, WaterMotionKind as Kind};
#[derive(Clone, Copy)]
struct Particle {
    origin: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    size: f32,
    kind: Kind,
}
#[derive(Resource, Default)]
struct Particles {
    items: VecDeque<Particle>,
    mesh: Handle<Mesh>,
}
#[derive(Component)]
struct Batch;
pub(super) fn install(app: &mut App) {
    app.init_resource::<Particles>()
        .add_systems(Startup, setup)
        .add_systems(Update, update.after(terra_core::footsteps::WaterMotionSet));
}
fn setup(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    particles.mesh = meshes.add(Rectangle::new(0.01, 0.01));
    commands.spawn((
        Name::new("Actor splashes, wakes and breath bubbles"),
        Batch,
        Mesh3d(particles.mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
        bevy::camera::visibility::NoFrustumCulling,
    ));
}
fn update(
    mut events: MessageReader<WaterMotion>,
    mut state: ResMut<Particles>,
    environment: (
        Res<Time>,
        Res<terra_world::streaming::WorldAccess>,
        Res<terra_core::GameConfig>,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&GlobalTransform, With<terra_player::TerraCamera>>,
    mut batch: Query<&mut Visibility, With<Batch>>,
) {
    let (time, access, config) = environment;
    let dt = time.delta_secs().min(0.1);
    for p in &mut state.items {
        p.age += dt;
    }
    state.items.retain(|p| p.age < p.life);
    let limit = match config.client.water_quality {
        terra_core::WaterQuality::Low => 64,
        terra_core::WaterQuality::Medium => 192,
        terra_core::WaterQuality::High => 384,
    };
    for event in events.read().take(32) {
        if !access.ready() {
            continue;
        }
        let count = match event.kind {
            Kind::Splash => (event.strength * 16.0) as usize + 6,
            Kind::Wake => 2,
            Kind::Bubble => 3,
        };
        for i in 0..count.min(40) {
            if state.items.len() >= limit {
                state.items.pop_front();
            }
            let angle = i as f32 * 2.39996 + time.elapsed_secs();
            let velocity = match event.kind {
                Kind::Splash => Vec3::new(
                    angle.cos() * event.strength,
                    0.8 + ((i * 7 % 13) as f32 / 13.0) * event.strength * 2.0,
                    angle.sin() * event.strength,
                ),
                Kind::Wake => {
                    -event.velocity * 0.12 + Vec3::new(angle.cos() * 0.08, 0.0, angle.sin() * 0.08)
                }
                Kind::Bubble => Vec3::new(
                    angle.cos() * 0.04,
                    0.45 + i as f32 * 0.08,
                    angle.sin() * 0.04,
                ),
            };
            state.items.push_back(Particle {
                origin: event.position,
                velocity,
                age: 0.0,
                life: if event.kind == Kind::Bubble {
                    1.8
                } else {
                    0.8 + event.strength * 0.2
                },
                size: if event.kind == Kind::Wake {
                    0.09
                } else {
                    0.015 + event.strength * 0.012
                },
                kind: event.kind,
            });
        }
    }
    let Ok(mut visible) = batch.single_mut() else {
        return;
    };
    let Ok(camera) = camera.single() else {
        return;
    };
    if !access.ready() {
        state.items.clear();
    }
    if state.items.is_empty() {
        *visible = Visibility::Hidden;
        return;
    }
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    for p in &state.items {
        let gravity = if p.kind == Kind::Splash {
            Vec3::NEG_Y * 3.6 * p.age * p.age
        } else {
            Vec3::ZERO
        };
        let center = p.origin + p.velocity * p.age + gravity;
        let half = p.size * (1.0 + p.age * 0.8);
        let fade = (1.0 - p.age / p.life).clamp(0.0, 1.0);
        let (r, u) = if p.kind == Kind::Wake {
            (Vec3::X, Vec3::Z)
        } else {
            (right, up)
        };
        // Hollow bubbles/wakes preserve the water below instead of opaque squares.
        let segments = if p.kind == Kind::Splash { 4 } else { 10 };
        for i in 0..segments {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let inner = if p.kind == Kind::Splash { 0.0 } else { 0.78 };
            let base = positions.len() as u32;
            for (angle, scale) in [(a, inner), (a, 1.0), (b, 1.0), (b, inner)] {
                positions
                    .push((center + (r * angle.cos() + u * angle.sin()) * half * scale).to_array());
                colors.push([
                    0.81,
                    0.94,
                    0.98,
                    fade * if p.kind == Kind::Bubble { 0.48 } else { 0.65 },
                ]);
            }
            indices.extend([0, 1, 2, 0, 2, 3].map(|i| i + base));
        }
    }
    let count = positions.len();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![up.to_array(); count]);
    mesh.insert_indices(Indices::U32(indices));
    if let Some(mut target) = meshes.get_mut(&state.mesh) {
        *target = mesh;
    }
    *visible = Visibility::Visible;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_particles_reuse_one_mesh_and_entities_and_obey_vertex_budget() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Time>()
            .init_resource::<terra_world::streaming::WorldAccess>()
            .init_resource::<terra_core::GameConfig>()
            .add_message::<WaterMotion>();
        install(&mut app);
        app.world_mut().spawn((
            terra_player::TerraCamera,
            GlobalTransform::from_translation(Vec3::new(0.0, 2.0, 4.0)),
        ));
        app.update();
        let count = app.world().entities().len();
        for _ in 0..100 {
            app.world_mut().write_message(WaterMotion {
                position: Vec3::ZERO,
                velocity: Vec3::NEG_Y * 10.0,
                strength: 2.0,
                kind: Kind::Splash,
            });
        }
        app.update();
        let particles = app.world().resource::<Particles>();
        assert!(particles.items.len() <= 384);
        let mesh = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&particles.mesh)
            .unwrap();
        assert!(mesh.count_vertices() > 0 && mesh.count_vertices() <= 384 * 40);
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|i| i < mesh.count_vertices())
        );
        assert_eq!(app.world().entities().len(), count);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(10));
        // Frame clamping keeps simulation stable after a stall; repeated bounded steps expire all particles.
        for _ in 0..30 {
            app.update();
        }
        assert!(app.world().resource::<Particles>().items.is_empty());
    }
}
