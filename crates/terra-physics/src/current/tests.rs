use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{
    streaming::{ChunkFocus, LoadedSection},
    water::FluidCell,
};

fn fixture(flow: [i8; 2], water: bool) -> App {
    let mut config = GameConfig::default();
    config.world.water = water;
    config.world.rivers = false;
    config.world.sea_level = -100.0;
    let terrain = TerraGenerator::new(config.world.clone()).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(config))
        .add_plugins(crate::TerraPhysicsPlugin)
        .insert_resource(WorldTerrain(terrain))
        .insert_resource(ChunkFocus([7.0, 7.0, 7.0]))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data: BlockSection::empty(SectionPos::default()).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        for x in 1..15 {
            for z in 1..15 {
                for y in 5..11 {
                    journal
                        .set_fluid(
                            [x, y, z],
                            FluidCell {
                                amount: 8,
                                flow,
                                source: true,
                            },
                        )
                        .unwrap();
                }
            }
        }
    }
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if app
            .world()
            .resource::<crate::TerrainCollisionState>()
            .ready_for_aabb(
                Vec3::new(4.0, 5.0, 4.0),
                Vec3::new(11.0, 11.0, 11.0),
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
            )
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "empty collider section was not ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app
}

#[test]
fn forty_percent_response_preserves_still_water_and_active_counter_movement() {
    let flow = FluidMotion {
        velocity: [2.4, 0.0, 0.0],
        immersion: 1.0,
    };
    assert!((advection(flow).x - 0.96).abs() < 1e-6);
    let against = Vec3::NEG_X * crate::character::WALK_SPEED * 0.72 + advection(flow);
    assert!(against.x < -2.9);
    assert_eq!(advection(FluidMotion::default()), Vec3::ZERO);
    let initial = Vec3::new(1.0, -3.0, 2.0);
    assert_eq!(
        drift_velocity(
            initial,
            FluidMotion {
                velocity: [0.0; 3],
                immersion: 1.0
            },
            0.05
        ),
        initial
    );
    let mut velocity = Vec3::ZERO;
    for _ in 0..180 {
        velocity = drift_velocity(velocity, flow, 1.0 / 60.0);
    }
    assert!((velocity.x - 0.96).abs() < 0.001);
}

#[test]
fn real_avian_current_wakes_dynamic_bodies_and_excludes_static_and_characters() {
    let mut app = fixture([1, 0], true);
    let spawn = |app: &mut App, body: RigidBody, position: Vec3| {
        app.world_mut()
            .spawn((
                body,
                Collider::cuboid(0.3, 0.3, 0.3),
                GravityScale(0.0),
                Transform::from_translation(position),
            ))
            .id()
    };
    let moving = spawn(&mut app, RigidBody::Dynamic, Vec3::new(3.5, 7.5, 3.5));
    let fixed = spawn(&mut app, RigidBody::Static, Vec3::new(6.5, 7.5, 6.5));
    let character = spawn(&mut app, RigidBody::Dynamic, Vec3::new(9.5, 7.5, 9.5));
    app.world_mut()
        .entity_mut(character)
        .insert(crate::character::CharacterBody);
    app.update();
    app.world_mut().entity_mut(moving).insert(Sleeping);
    for _ in 0..60 {
        app.update();
    }
    assert!(app.world().get::<Position>(moving).unwrap().x > 4.1);
    assert!(!app.world().entity(moving).contains::<Sleeping>());
    assert_eq!(
        app.world().get::<Position>(fixed).unwrap().0,
        Vec3::new(6.5, 7.5, 6.5)
    );
    assert_eq!(
        app.world().get::<Position>(character).unwrap().0,
        Vec3::new(9.5, 7.5, 9.5)
    );
}

#[test]
fn disabled_water_and_still_ocean_do_not_push_rigid_bodies() {
    for (flow, water) in [([1, 0], false), ([0, 0], true)] {
        let mut app = fixture(flow, water);
        let body = app
            .world_mut()
            .spawn((
                RigidBody::Dynamic,
                Collider::sphere(0.2),
                GravityScale(0.0),
                Transform::from_xyz(5.5, 7.5, 5.5),
            ))
            .id();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Position>(body).unwrap().0,
            Vec3::new(5.5, 7.5, 5.5)
        );
    }
}

#[derive(Resource)]
struct Drive(crate::character::CharacterInput);

fn move_character(
    simulation: crate::character::CharacterSimulation,
    drive: Res<Drive>,
    mut bodies: crate::character::ControlledBodies,
) {
    for (entity, mut transform, mut velocity, collider, mut grounded, _) in &mut bodies {
        let mut state = crate::character::CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        if simulation.step(entity, collider, &mut state, drive.0) {
            transform.translation = state.position;
            velocity.0 = state.velocity;
            grounded.0 = state.grounded;
        }
    }
}

#[test]
fn production_character_simulation_adds_current_after_swimming_and_allows_countering() {
    let mut app = fixture([1, 0], true);
    app.insert_resource(Drive(crate::character::CharacterInput::default()))
        .add_systems(FixedUpdate, move_character.after(PhysicsSystems::Writeback));
    let body = app
        .world_mut()
        .spawn(crate::character::body_bundle(Vec3::new(7.5, 7.5, 7.5)))
        .id();
    app.update();
    assert!((app.world().get::<LinearVelocity>(body).unwrap().x - 0.96).abs() < 0.001);
    app.world_mut().resource_mut::<Drive>().0.direction = Vec3::NEG_X;
    app.update();
    assert!(app.world().get::<LinearVelocity>(body).unwrap().x < -2.9);
}
