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

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct UpdateContacts;

pub(super) const WAKE_CAPACITY: usize = 16;
const WAKE_LIFETIME: f32 = 3.2;

#[derive(Resource, Default)]
pub(super) struct WakeField {
    pub positions: [Vec4; WAKE_CAPACITY],
    pub motions: [Vec4; WAKE_CAPACITY],
    cursor: usize,
}

impl WakeField {
    fn push(&mut self, event: &WaterMotion, now: f32) {
        let direction = event.velocity.xz().normalize_or_zero();
        let speed = if event.kind == Kind::Splash {
            event
                .velocity
                .xz()
                .length()
                .max((-event.velocity.y).max(0.0))
        } else {
            event.velocity.xz().length()
        }
        .min(6.0);
        if !event.position.is_finite()
            || !event.velocity.is_finite()
            || !event.strength.is_finite()
            || !direction.is_finite()
            || speed < 0.25
            || event.kind == Kind::Bubble
        {
            return;
        }
        let i = self.cursor % WAKE_CAPACITY;
        self.cursor = (i + 1) % WAKE_CAPACITY;
        self.positions[i] = Vec4::new(event.position.x, event.position.y, event.position.z, now);
        self.motions[i] = Vec4::new(
            direction.x,
            direction.y,
            speed,
            event.strength.clamp(0.0, 0.75),
        );
    }

    fn expire(&mut self, now: f32) {
        for (position, motion) in self.positions.iter_mut().zip(&mut self.motions) {
            if now - position.w >= WAKE_LIFETIME {
                *position = Vec4::ZERO;
                *motion = Vec4::ZERO;
            }
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Particles>()
        .init_resource::<WakeField>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            update
                .after(terra_core::footsteps::WaterMotionSet)
                .in_set(UpdateContacts),
        );
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
    mut wakes: ResMut<WakeField>,
) {
    let (time, access, config) = environment;
    let dt = time.delta_secs().min(0.1);
    for p in &mut state.items {
        p.age += dt;
    }
    state.items.retain(|p| p.age < p.life);
    wakes.expire(time.elapsed_secs());
    let limit = match config.client.water_quality {
        terra_core::WaterQuality::Low => 64,
        terra_core::WaterQuality::Medium => 192,
        terra_core::WaterQuality::High => 384,
    };
    for event in events.read().take(32) {
        if !access.ready() {
            continue;
        }
        if !event.position.is_finite() || !event.velocity.is_finite() || !event.strength.is_finite()
        {
            continue;
        }
        if matches!(event.kind, Kind::Wake | Kind::Splash) {
            wakes.push(event, time.elapsed_secs());
        }
        let strength = event.strength.clamp(0.0, 0.7);
        let count = match event.kind {
            Kind::Splash => (strength * 10.0) as usize + 3,
            // The water surface carries the wake; do not paint large white
            // billboard rings over it for every swimming stroke.
            Kind::Wake => 0,
            Kind::Bubble => 3,
        };
        for i in 0..count.min(40) {
            if state.items.len() >= limit {
                state.items.pop_front();
            }
            let angle = i as f32 * 2.39996 + time.elapsed_secs();
            let velocity = match event.kind {
                Kind::Splash => Vec3::new(
                    angle.cos() * strength * 0.55,
                    0.35 + ((i * 7 % 13) as f32 / 13.0) * strength * 1.5,
                    angle.sin() * strength * 0.55,
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
                } else if event.kind == Kind::Wake {
                    1.6
                } else {
                    0.38 + strength * 0.18
                },
                size: if event.kind == Kind::Wake {
                    0.16 + event.strength * 0.10
                } else {
                    0.009 + strength * 0.007
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
        *wakes = WakeField::default();
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
            Vec3::NEG_Y * 4.905 * p.age * p.age
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
    fn shader_wakes_keep_forward_motion_bound_history_and_expire_after_leaving_water() {
        let mut wakes = WakeField::default();
        for i in 0..100 {
            wakes.push(
                &WaterMotion {
                    position: Vec3::new(i as f32, 4.0, -8.0),
                    velocity: Vec3::new(3.0, 0.0, 4.0),
                    strength: 0.6,
                    kind: Kind::Wake,
                },
                i as f32 * 0.18,
            );
        }
        assert_eq!(
            wakes.motions.iter().filter(|m| m.w > 0.0).count(),
            WAKE_CAPACITY
        );
        for (position, motion) in wakes.positions.iter().zip(wakes.motions) {
            assert!(position.x >= 84.0);
            assert!((motion.x - 0.6).abs() < 0.00001);
            assert!((motion.y - 0.8).abs() < 0.00001);
            assert_eq!(motion.z, 5.0);
        }
        wakes.expire(100.0);
        assert!(wakes.motions.iter().all(|m| *m == Vec4::ZERO));
        assert!(wakes.positions.iter().all(|p| *p == Vec4::ZERO));
    }
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
        for drop in &particles.items {
            if drop.kind == Kind::Splash {
                let peak = drop.velocity.y * drop.velocity.y / (2.0 * 9.81);
                assert!(
                    peak <= 0.12,
                    "an actor must not launch metre-high water jets"
                );
                assert!(drop.size <= 0.014);
            }
        }
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
