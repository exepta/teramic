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
    blockers: std::collections::BTreeMap<u64, SmokeBlockers>,
}
struct SmokeBlockers {
    key: super::GeometryKey,
    cells: std::collections::BTreeMap<[i32; 3], Vec<(Vec3, Vec3)>>,
}
impl SmokeBlockers {
    fn new(car: &VehicleView) -> Self {
        let mut cells = std::collections::BTreeMap::<_, Vec<_>>::new();
        for (index, part) in car.parts.iter().enumerate() {
            let offset = Vec3::from_array(part.offset.map(f32::from));
            for shape in car.part_shapes(index) {
                let low = offset + Vec3::from_array(shape.minimum.map(|v| v as f32));
                let high = offset + Vec3::from_array(shape.maximum.map(|v| v as f32));
                for x in low.x.floor() as i32..=high.x.floor() as i32 {
                    for y in low.y.floor() as i32..=high.y.floor() as i32 {
                        for z in low.z.floor() as i32..=high.z.floor() as i32 {
                            cells.entry([x, y, z]).or_default().push((low, high));
                        }
                    }
                }
            }
        }
        Self {
            key: super::GeometryKey {
                anchor: car.anchor,
                parts: car.parts.clone(),
                blocks: car.blocks.clone(),
            },
            cells,
        }
    }
    fn intersects(&self, start: Vec3, end: Vec3, radius: f32) -> bool {
        let low = start.min(end) - Vec3::splat(radius);
        let high = start.max(end) + Vec3::splat(radius);
        for x in low.x.floor() as i32..=high.x.floor() as i32 {
            for y in low.y.floor() as i32..=high.y.floor() as i32 {
                for z in low.z.floor() as i32..=high.z.floor() as i32 {
                    if self.cells.get(&[x, y, z]).is_some_and(|shapes| {
                        shapes.iter().any(|&(a, b)| {
                            swept_hit(start, end, a - Vec3::splat(radius), b + Vec3::splat(radius))
                        })
                    }) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
/// Swept particle bounds prevent thin glass from being skipped between frames.
fn swept_hit(start: Vec3, end: Vec3, low: Vec3, high: Vec3) -> bool {
    let direction = end - start;
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-6 {
            if start[axis] < low[axis] || start[axis] > high[axis] {
                return false;
            }
        } else {
            let a = (low[axis] - start[axis]) / direction[axis];
            let b = (high[axis] - start[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
            if enter > exit {
                return false;
            }
        }
    }
    true
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
                f32::from(part.offset[1]) + 1.20,
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
    // Geometry is cached independently of moving poses and cargo revisions.
    if let Some(session) = session.as_ref().filter(|session| session.ready) {
        state.blockers.retain(|id, _| {
            session.trains.vehicles.iter().any(|car| {
                car.id == *id
                    && Vec3::from_array(car.world_point([0.5; 3]).map(|v| v as f32))
                        .distance_squared(camera.translation())
                        <= 192.0 * 192.0
            })
        });
        for car in &session.trains.vehicles {
            if Vec3::from_array(car.world_point([0.5; 3]).map(|v| v as f32))
                .distance_squared(camera.translation())
                > 192.0 * 192.0
            {
                continue;
            }
            if state
                .blockers
                .get(&car.id)
                .is_none_or(|cached| !cached.key.matches(car.anchor, &car.parts, &car.blocks))
            {
                state.blockers.insert(car.id, SmokeBlockers::new(car));
            }
        }
    } else {
        state.puffs.clear();
        state.blockers.clear();
    }
    let poses: Vec<_> = session
        .as_ref()
        .map(|session| {
            session
                .trains
                .vehicles
                .iter()
                .filter(|car| state.blockers.contains_key(&car.id))
                .map(|car| {
                    (
                        car.id,
                        display_pose(car, session.displayed_train_motion(car))
                            .to_matrix()
                            .inverse(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let state = &mut *state;
    for puff in &mut state.puffs {
        puff.age += dt;
        let next = puff.position + puff.velocity * dt;
        let radius = 0.16 + puff.age * 0.27;
        if poses.iter().any(|(id, inverse)| {
            state.blockers[id].intersects(
                inverse.transform_point3(puff.position),
                inverse.transform_point3(next),
                radius,
            )
        }) {
            puff.age = LIFETIME;
        }
        puff.position = next;
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
    fn swept_smoke_stops_at_thin_glass_and_does_not_block_open_air() {
        let low = Vec3::new(-1.0, 0.0, 0.49);
        let high = Vec3::new(1.0, 2.0, 0.51);
        assert!(swept_hit(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0),
            low,
            high
        ));
        assert!(!swept_hit(
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(2.0, 1.0, 1.0),
            low,
            high
        ));
        assert!(!swept_hit(
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 1.0),
            low,
            high
        ));
    }
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
        let expected = Vec3::from_array(car.world_point([0.5, 5.20, 6.5]).map(|v| v as f32));
        let actual = outlets(&car, display_pose(&car, car.motion))
            .next()
            .unwrap();
        assert!(actual.distance(expected) < 1.0e-5);
        let mut displayed = car.motion;
        displayed.translation[0] += 5.0;
        let interpolated = outlets(&car, display_pose(&car, displayed)).next().unwrap();
        assert!((interpolated - actual - Vec3::X * 5.0).length() < 1.0e-5);
    }
    #[test]
    fn real_chimney_emits_clear_of_its_lip_and_frozen_glass_stops_the_puff() {
        use terra_core::{
            block_shapes::ShapeBox,
            trains::{TrainBlockView, TrainPart},
        };
        let mut car = VehicleView {
            steam: default(),
            name: String::new(),
            id: 1,
            owner: default(),
            kind: default(),
            anchor: [0; 3],
            parts: vec![TrainPart {
                offset: [0; 3],
                material: 224,
                facing: 0,
                wood: default(),
            }],
            blocks: default(),
            couplings: [None; 2],
            revision: 0,
            mass_kg: 0,
            motion: default(),
        };
        let clear = SmokeBlockers::new(&car);
        assert!(!clear.intersects(Vec3::new(0.5, 1.20, 0.5), Vec3::new(0.5, 1.24, 0.52), 0.17));
        car.parts.push(TrainPart {
            offset: [0, 1, 1],
            material: 15,
            facing: 0,
            wood: default(),
        });
        car.blocks.insert(
            1,
            TrainBlockView {
                boxes: vec![ShapeBox {
                    minimum: [0.0, 0.0, 0.45],
                    maximum: [1.0, 1.0, 0.50],
                    hinged: false,
                }],
                ..default()
            },
        );
        let glass = SmokeBlockers::new(&car);
        assert!(glass.intersects(Vec3::new(0.5, 1.4, 0.8), Vec3::new(0.5, 1.4, 1.8), 0.17));
        assert!(!glass.intersects(Vec3::new(1.5, 1.4, 0.8), Vec3::new(1.5, 1.4, 1.8), 0.17));
        car.blocks.get_mut(&1).unwrap().boxes[0].maximum[0] = 0.1;
        assert!(!SmokeBlockers::new(&car).intersects(
            Vec3::new(0.5, 1.4, 0.8),
            Vec3::new(0.5, 1.4, 1.8),
            0.17
        ));
    }
}
