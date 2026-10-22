use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin, block_shapes::ShapeBox};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::{ChunkFocus, LoadedSection};

#[derive(Resource, Default)]
struct Drive {
    input: CharacterInput,
    missing_collision: usize,
    vitals: Option<terra_core::SurvivalVitals>,
}

fn move_character(
    simulation: CharacterSimulation,
    mut drive: ResMut<Drive>,
    mut bodies: ControlledBodies,
) {
    for (entity, mut transform, mut velocity, collider, mut grounded, _) in &mut bodies {
        let mut state = CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        let input = drive.input;
        let ready = if let Some(vitals) = &mut drive.vitals {
            simulation.survival_step(
                entity,
                collider,
                &mut state,
                input,
                terra_core::GameMode::Survival,
                vitals,
            )
        } else {
            simulation.step(entity, collider, &mut state, input)
        };
        if ready {
            transform.translation = state.position;
            velocity.0 = state.velocity;
            grounded.0 = state.grounded;
        } else {
            drive.missing_collision += 1;
        }
    }
}

#[test]
fn survival_speed_slow_and_freeze_modify_real_capsule_movement_without_network_input_changes() {
    use terra_core::{EffectKind, SurvivalVitals};
    for kind in [
        None,
        Some(EffectKind::Speed),
        Some(EffectKind::Slow),
        Some(EffectKind::Freeze),
    ] {
        let (mut app, body) = fixture(0.5, false, None);
        let mut vitals = SurvivalVitals::default();
        if let Some(kind) = kind {
            vitals.effects.apply(kind, 10.0, 1);
        }
        let multiplier = vitals.effects.movement_multiplier(false);
        let start = app.world().get::<Transform>(body).unwrap().translation;
        {
            let mut drive = app.world_mut().resource_mut::<Drive>();
            drive.vitals = Some(vitals);
            drive.input.direction = Vec3::NEG_X;
        }
        for _ in 0..30 {
            app.update();
        }
        let after = app.world().get::<Transform>(body).unwrap().translation;
        let expected = WALK_SPEED * multiplier * 0.5;
        assert!(
            (start.x - after.x - expected).abs() < 0.12,
            "{kind:?}: start={start:?}, after={after:?}, expected={expected}"
        );
        assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    }
}

fn shape(app: &mut App, origin: Vec3, bounds: ShapeBox) {
    let low = Vec3::from_array(bounds.minimum.map(|value| value as f32));
    let high = Vec3::from_array(bounds.maximum.map(|value| value as f32));
    let size = high - low;
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
        Transform::from_translation(origin + (low + high) * 0.5),
    ));
}

fn fixture(height: f32, stairs: bool, ceiling: Option<f32>) -> (App, Entity) {
    let mut config = GameConfig::default();
    config.world.water = false;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(config))
        .add_plugins(crate::TerraPhysicsPlugin)
        .insert_resource(ChunkFocus([8.0, 2.0, 8.5]))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    for y in [-1, 0] {
        let position = SectionPos::new(0, y, 0);
        let mut data = BlockSection::empty(position).unwrap();
        if y == 0 {
            for x in 0..16 {
                for z in 0..16 {
                    data.set_block([x, 0, z], 3).unwrap();
                }
            }
        }
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    for x in 8..14 {
        if stairs && x == 8 {
            for bounds in terra_core::construction::boxes(74) {
                shape(
                    &mut app,
                    Vec3::new(8.0, 1.0, 8.0),
                    ShapeBox {
                        minimum: [bounds.minimum[2], bounds.minimum[1], bounds.minimum[0]],
                        maximum: [bounds.maximum[2], bounds.maximum[1], bounds.maximum[0]],
                        hinged: false,
                    },
                );
            }
            continue;
        }
        shape(
            &mut app,
            Vec3::new(x as f32, 1.0, 8.0),
            ShapeBox {
                minimum: [0.0; 3],
                maximum: [
                    1.0,
                    f64::from(if stairs && x > 8 { 1.0 } else { height }),
                    1.0,
                ],
                hinged: false,
            },
        );
    }
    if let Some(ceiling) = ceiling {
        for x in 5..14 {
            shape(
                &mut app,
                Vec3::new(x as f32, ceiling, 8.0),
                ShapeBox {
                    minimum: [0.0; 3],
                    maximum: [1.0; 3],
                    hinged: false,
                },
            );
        }
    }
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .resource::<TerrainCollisionState>()
            .ready_for_aabb(
                Vec3::new(4.0, -1.0, 7.0),
                Vec3::new(15.0, 5.0, 10.0),
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
            )
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "step fixture collision was not ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let body = app
        .world_mut()
        .spawn(body_bundle(Vec3::new(6.0, 1.92, 8.5)))
        .id();
    app.init_resource::<Drive>()
        .add_systems(FixedUpdate, move_character.after(PhysicsSystems::Writeback));
    for _ in 0..20 {
        app.update();
    }
    assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
    (app, body)
}

fn walk(app: &mut App, body: Entity, auto_climb: bool) -> Vec<CharacterState> {
    app.world_mut().resource_mut::<Drive>().input = CharacterInput {
        direction: Vec3::X,
        auto_climb,
        ..default()
    };
    let mut states = Vec::new();
    for _ in 0..60 {
        app.update();
        states.push(CharacterState {
            position: app.world().get::<Transform>(body).unwrap().translation,
            velocity: app.world().get::<LinearVelocity>(body).unwrap().0,
            grounded: app.world().get::<CharacterGrounded>(body).unwrap().0,
        });
    }
    app.world_mut().resource_mut::<Drive>().input = CharacterInput::default();
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    states
}

fn standing_on(app: &App, body: Entity, height: f32) {
    let position = app.world().get::<Transform>(body).unwrap().translation;
    assert!(
        position.x > 9.5,
        "capsule did not pass the step: {position:?}"
    );
    assert!(
        (position.y - height - PLAYER_HALF_HEIGHT).abs() < 0.06,
        "capsule is hovering or penetrating the step: {position:?}"
    );
    assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
    assert!(app.world().get::<LinearVelocity>(body).unwrap().y.abs() < 0.1);
}

#[test]
fn real_avian_grounded_capsule_walks_up_half_slabs_without_auto_climb_or_jump() {
    let (mut app, body) = fixture(0.5, false, None);
    let states = walk(&mut app, body, false);
    standing_on(&app, body, 1.5);
    assert!(states.iter().all(|state| state.position.y < 2.5));
}

#[test]
fn real_avian_json_stair_treads_are_walkable_without_full_block_auto_climb() {
    let (mut app, body) = fixture(0.5, true, None);
    let states = walk(&mut app, body, false);
    standing_on(&app, body, 2.0);
    assert!(
        states
            .windows(2)
            .all(|s| s[1].position.x >= s[0].position.x - 0.001)
    );
    let crest = states.iter().position(|s| s.position.x > 9.0).unwrap();
    assert!(
        crest < 48,
        "stairs must not repeatedly brake the horizontal stride: {crest}"
    );
}

#[test]
fn real_avian_full_block_wall_stays_blocked_unless_auto_climb_is_enabled() {
    for auto_climb in [false, true] {
        let (mut app, body) = fixture(1.0, false, None);
        walk(&mut app, body, auto_climb);
        if auto_climb {
            standing_on(&app, body, 2.0);
        } else {
            let position = app.world().get::<Transform>(body).unwrap().translation;
            assert!(
                position.x < 8.0,
                "ordinary step climbed a full wall: {position:?}"
            );
            assert!((position.y - 1.0 - PLAYER_HALF_HEIGHT).abs() < 0.06);
            assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
        }
    }
}

#[test]
fn real_avian_half_step_checks_capsule_headroom_even_with_auto_climb() {
    for (ceiling, passes) in [(3.1, false), (3.5, true)] {
        for auto_climb in [false, true] {
            let (mut app, body) = fixture(0.5, false, Some(ceiling));
            let states = walk(&mut app, body, auto_climb);
            assert!(
                states
                    .iter()
                    .all(|state| { state.position.y + PLAYER_HALF_HEIGHT <= ceiling + 0.02 }),
                "step intersected its ceiling: {states:?}"
            );
            if passes {
                standing_on(&app, body, 1.5);
            } else {
                let position = app.world().get::<Transform>(body).unwrap().translation;
                assert!(
                    position.x < 8.0,
                    "capsule stepped into inadequate headroom: {position:?}"
                );
                assert!(position.y + PLAYER_HALF_HEIGHT <= ceiling + 0.02);
            }
        }
    }
}
