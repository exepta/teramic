//! Nearby locomotive exhaust uses one shared soft-particle mesh with fixed budgets.
use crate::furnaces::{Geometry, SmokeExtension, SmokeMaterial};
use bevy::{camera::visibility::NoFrustumCulling, light::NotShadowCaster, prelude::*};
use terra_core::trains::VehicleView;
use terra_network::client::ClientSession;
use terra_player::TerraCamera;

const MAX_PUFFS: usize = 256;
const LIFETIME: f32 = 4.0;
struct Puff {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    seed: f32,
}
#[derive(Resource, Default)]
struct Exhaust {
    puffs: Vec<Puff>,
    next_emit: f32,
    next_draw: f32,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<SmokeMaterial>>,
    entity: Option<Entity>,
    sequence: u32,
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Exhaust>()
        .add_systems(Update, update.after(super::synchronize));
}
fn display_pose(car: &VehicleView, motion: terra_core::train_motion::TrainMotion) -> Transform {
    let (origin, orientation) = car.model_transform_at(motion);
    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32)))
        .with_rotation(Quat::from_array(orientation.map(|v| v as f32)))
}
fn outlets(car: &VehicleView, pose: Transform) -> impl Iterator<Item = Vec3> + '_ {
    car.parts
        .iter()
        .filter(|part| part.material == 224)
        .take(2)
        .map(move |part| {
            pose.transform_point(Vec3::new(
                f32::from(part.offset[0]) + 0.5,
                f32::from(part.offset[1]) + 1.0,
                f32::from(part.offset[2]) + 0.5,
            ))
        })
}
#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    time: Res<Time>,
    session: Option<Res<ClientSession>>,
    camera: Query<&GlobalTransform, With<TerraCamera>>,
    mut state: ResMut<Exhaust>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SmokeMaterial>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(camera) = camera.iter().next() else {
        return;
    };
    let dt = time.delta_secs().min(0.1);
    for puff in &mut state.puffs {
        puff.age += dt;
        puff.position += puff.velocity * dt;
        puff.velocity.x *= (-0.7 * dt).exp();
        puff.velocity.z *= (-0.7 * dt).exp();
    }
    state.puffs.retain(|p| {
        p.age < LIFETIME && p.position.distance_squared(camera.translation()) < 192.0 * 192.0
    });
    let now = time.elapsed_secs();
    if now >= state.next_emit {
        state.next_emit = now + 0.12;
        if let Some(session) = session.filter(|session| session.ready) {
            let mut cars: Vec<_> = session
                .trains
                .vehicles
                .iter()
                .filter(|car| car.motion.speed_mps.abs() > 0.02 && car.motion.fuel_available)
                .map(|car| {
                    (
                        Vec3::from_array(car.world_point([0.5, 0.5, 0.5]).map(|v| v as f32))
                            .distance_squared(camera.translation()),
                        car,
                    )
                })
                .filter(|(distance, _)| *distance < 160.0 * 160.0)
                .collect();
            cars.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, car) in cars.into_iter().take(4) {
                let motion = session.displayed_train_motion(car);
                let pose = display_pose(car, motion);
                for position in outlets(car, pose) {
                    if state.puffs.len() >= MAX_PUFFS {
                        break;
                    }
                    state.sequence = state.sequence.wrapping_add(1);
                    let seed = state.sequence as f32 * 1.618_034;
                    let direction =
                        pose.rotation * Vec3::new(0.0, 0.0, motion.speed_mps as f32 * 0.06);
                    state.puffs.push(Puff {
                        position,
                        velocity: Vec3::new(
                            direction.x + seed.sin() * 0.2,
                            1.25,
                            direction.z + seed.cos() * 0.2,
                        ),
                        age: 0.0,
                        seed,
                    });
                }
            }
        }
    }
    if now < state.next_draw {
        return;
    }
    state.next_draw = now + 1.0 / 30.0;
    if state.puffs.is_empty() {
        if let Some(entity) = state.entity
            && let Ok(mut visible) = visibility.get_mut(entity)
        {
            *visible = Visibility::Hidden;
        }
        return;
    }
    if state.mesh.is_none() {
        let mesh = meshes.add(crate::furnaces::mesh());
        let material = materials.add(SmokeMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                double_sided: true,
                ..default()
            },
            extension: SmokeExtension {
                animation: Vec4::ZERO,
            },
        });
        let entity = commands
            .spawn((
                Name::new("Locomotive exhaust"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::default(),
                Visibility::Visible,
                NotShadowCaster,
                NoFrustumCulling,
            ))
            .id();
        state.mesh = Some(mesh);
        state.material = Some(material);
        state.entity = Some(entity);
    }
    let mut geometry = Geometry::default();
    // Older puffs are painted first; no per-puff entities, materials or lights.
    for puff in &state.puffs {
        let progress = puff.age / LIFETIME;
        let radius = 0.16 + puff.age * 0.27;
        let alpha = (1.0 - progress).powi(2) * 0.75;
        let grey = 0.42 + progress * 0.25 + puff.seed.sin() * 0.025;
        geometry.wisp(
            puff.position,
            radius,
            *camera.right(),
            *camera.up(),
            [grey, grey, grey, alpha],
        );
    }
    if let Some(mut mesh) = meshes.get_mut(state.mesh.as_ref().unwrap()) {
        geometry.write(&mut mesh);
    }
    if let Some(mut material) = state
        .material
        .as_ref()
        .and_then(|handle| materials.get_mut(handle))
    {
        material.extension.animation = Vec4::new(now, 0.0, 0.0, 0.0);
    }
    if let Some(entity) = state.entity
        && let Ok(mut visible) = visibility.get_mut(entity)
    {
        *visible = Visibility::Visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_starts_at_the_actual_transformed_chimney_lip() {
        let car = VehicleView {
            steam: Default::default(),
            name: String::new(),
            id: 1,
            owner: Default::default(),
            kind: Default::default(),
            anchor: [20, 10, -7],
            parts: vec![terra_core::trains::TrainPart {
                offset: [0, 4, 6],
                material: 224,
                facing: 0,
                wood: Default::default(),
            }],
            blocks: Default::default(),
            couplings: [None; 2],
            revision: 1,
            mass_kg: 0,
            motion: terra_core::train_motion::TrainMotion {
                yaw: 1.2,
                translation: [1.0, 0.0, 2.0],
                ..Default::default()
            },
        };
        let expected = Vec3::from_array(car.world_point([0.5, 5.0, 6.5]).map(|v| v as f32));
        let actual = outlets(&car, display_pose(&car, car.motion))
            .next()
            .unwrap();
        assert!(actual.distance(expected) < 1.0e-5);
        let mut displayed = car.motion;
        displayed.translation[0] += 5.0;
        let interpolated = outlets(&car, display_pose(&car, displayed)).next().unwrap();
        assert!((interpolated - actual - Vec3::X * 5.0).length() < 1.0e-5);
    }
}
