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
    mode: Option<terra_core::GameMode>,
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
        let ready = if drive.mode.is_some() || drive.vitals.is_some() {
            let mode = drive.mode.unwrap_or(terra_core::GameMode::Survival);
            let vitals = drive.vitals.get_or_insert_with(Default::default);
            simulation.survival_step(entity, collider, &mut state, input, mode, vitals)
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

fn active_belt_fixture(
    start: [i64; 3],
    end: [i64; 3],
    rpm: f32,
) -> (App, Entity, terra_world::conveyors::Span) {
    use terra_core::{ItemKind, kinetic::KineticState};
    use terra_world::edits::BlockObjectState;
    let (mut app, body) = fixture(0.5, false, None);
    for p in [start, end] {
        let (section, local) = terra_voxel::global_to_section(p);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 144)
            .unwrap();
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(p, 144).unwrap();
        journal
            .set_object_state(p, Some(BlockObjectState::default()))
            .unwrap();
    }
    let plan = terra_world::conveyors::layout(
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        start,
        end,
        ItemKind::Block(148),
    )
    .unwrap();
    let span = terra_world::conveyors::proposed_span(&plan).unwrap();
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            terra_world::conveyors::commit(
                &mut world.resource_mut::<ChunkStore>(),
                &mut journal,
                &plan,
            )
            .unwrap();
            journal.replace_kinetic_states(
                span.cells()
                    .chain([span.start, span.end])
                    .map(|p| {
                        (
                            p,
                            KineticState {
                                rpm,
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            );
        });
    // This fixture owns no streaming mesher. Publish its prepared section epochs,
    // then wait for the actual asynchronous collision rebuild before moving.
    app.world_mut()
        .resource_scope(|world, journal: Mut<EditJournal>| {
            for (position, section) in &mut world.resource_mut::<ChunkStore>().loaded {
                section.mesh_epoch = journal.mesh_epoch(*position);
            }
        });
    let deadline = Instant::now() + Duration::from_secs(10);
    let start = Vec3::from_array(start.map(|v| v as f32));
    let end = Vec3::from_array(end.map(|v| v as f32));
    loop {
        app.update();
        if app
            .world()
            .resource::<TerrainCollisionState>()
            .ready_for_aabb(
                start.min(end) - Vec3::new(1.0, 1.0, 1.0),
                start.max(end) + Vec3::new(2.0, 5.0, 1.0),
                app.world().resource(),
                app.world().resource(),
            )
        {
            break;
        }
        assert!(Instant::now() < deadline, "Belt collision was not ready");
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut().resource_mut::<Drive>().missing_collision = 0;
    (app, body, span)
}

#[test]
fn passive_player_belt_motion_uses_the_capsule_motor_and_stops_at_a_real_wall() {
    let (mut app, body, _) = active_belt_fixture([6, 1, 7], [6, 1, 14], 30.0);
    let segment = terra_world::conveyors::segment_at(app.world().resource(), [6, 1, 8]).unwrap();
    let height = segment.surface_height([6.5, 0.0, 8.5]).unwrap() as f32;
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation = Vec3::new(6.5, height + PLAYER_HALF_HEIGHT + 0.02, 8.5);
    for _ in 0..90 {
        app.update();
    }
    let after = app.world().get::<Transform>(body).unwrap().translation;
    assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    assert!(
        (after.z - 8.5).abs() > 0.6,
        "Belt did not move idle player: {after:?}"
    );
    let sign = (after.z - 8.5).signum();
    let carried_velocity = app.world().get::<LinearVelocity>(body).unwrap().0;
    assert!(
        carried_velocity.z * sign > 0.6 && carried_velocity.x.abs() < 0.01,
        "Carried player must publish its net world velocity: {carried_velocity:?}"
    );
    let wall_z = after.z.floor() + sign * 2.0;
    for x in 5..=7 {
        shape(
            &mut app,
            Vec3::new(x as f32, 1.0, wall_z),
            ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0, 4.0, 1.0],
                hinged: false,
            },
        );
    }
    for _ in 0..180 {
        app.update();
    }
    let blocked = app.world().get::<Transform>(body).unwrap().translation;
    let near_face = wall_z + if sign < 0.0 { 1.0 } else { 0.0 };
    assert!(
        (blocked.z - after.z) * sign > 0.5,
        "Player stopped before reaching the wall: {after:?} -> {blocked:?}"
    );
    assert!(
        (blocked.z - near_face) * sign <= -PLAYER_RADIUS + 0.03,
        "Conveyor bypassed wall: {blocked:?}"
    );
    let blocked_velocity = app.world().get::<LinearVelocity>(body).unwrap().0;
    assert!(
        Vec2::new(blocked_velocity.x, blocked_velocity.z).length() < 0.01,
        "Blocked player must not publish fictitious counter-motion: {blocked_velocity:?}"
    );
    shape(
        &mut app,
        Vec3::new(
            5.8,
            height,
            after.z + sign * 0.8 - if sign < 0.0 { 0.25 } else { 0.0 },
        ),
        ShapeBox {
            minimum: [0.0; 3],
            maximum: [1.4, 0.18, 0.25],
            hinged: false,
        },
    );
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation = after;
    app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::ZERO;
    app.world_mut().resource_mut::<Drive>().input.direction = Vec3::Z * sign;
    let mut climbed = false;
    for _ in 0..60 {
        let previous = app.world().get::<Transform>(body).unwrap().translation;
        app.update();
        let position = app.world().get::<Transform>(body).unwrap().translation;
        if position.y > after.y + 0.08 {
            let velocity = app.world().get::<LinearVelocity>(body).unwrap().0;
            let actual = (position - previous) / app.world().resource::<Time<Fixed>>().delta_secs();
            assert!(
                Vec2::new(velocity.x - actual.x, velocity.z - actual.z).length() < 0.1,
                "Step-up must publish swept world XZ, not relative input: {velocity:?} / {actual:?}"
            );
            assert_eq!(velocity.y, 0.0);
            climbed = true;
            break;
        }
    }
    assert!(
        climbed,
        "The real capsule did not climb the small belt obstacle"
    );
    assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
}

fn standing_on_inclined_belt(rpm: f32) -> (App, Entity, Vec3) {
    let (mut app, body, _) = active_belt_fixture([6, 1, 4], [6, 10, 13], rpm);
    let segment = terra_world::conveyors::segment_at(app.world().resource(), [6, 5, 8]).unwrap();
    let height = segment.surface_height([6.5, 0.0, 8.5]).unwrap() as f32;
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation = Vec3::new(6.5, height + PLAYER_HALF_HEIGHT + 0.16, 8.5);
    app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::ZERO;
    for _ in 0..30 {
        app.update();
    }
    assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
    let position = app.world().get::<Transform>(body).unwrap().translation;
    let belt = Vec3::from_array(terra_world::conveyors::surface_velocity(
        app.world().resource(),
        app.world().resource(),
        (position - Vec3::Y * PLAYER_HALF_HEIGHT)
            .as_dvec3()
            .to_array(),
        f64::from(PLAYER_RADIUS),
    ));
    assert!(
        belt.y.abs() > 1.0,
        "Inclined belt did not contact player: {position:?}"
    );
    (app, body, belt)
}

#[test]
fn inclined_belts_carry_idle_players_up_and_down_without_turning_advection_into_a_jump() {
    for rpm in [-60.0, 60.0] {
        let (mut app, body, belt) = standing_on_inclined_belt(rpm);
        let start = app.world().get::<Transform>(body).unwrap().translation;
        let mut previous = start;
        for _ in 0..90 {
            app.update();
            let position = app.world().get::<Transform>(body).unwrap().translation;
            assert!(
                (position - previous).dot(belt.normalize()) > 0.01,
                "Inclined carry stalled or reversed: {previous:?} -> {position:?}"
            );
            assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
            assert!(app.world().get::<LinearVelocity>(body).unwrap().0.y.abs() < 0.001);
            previous = position;
        }
        let traveled = previous - start;
        assert!(traveled.dot(belt.normalize()) > 2.0, "{traveled:?}");
        assert!(traveled.y * belt.y.signum() > 1.0, "{traveled:?}");
        assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    }
}

#[test]
fn counterwalking_on_inclined_belts_cancels_both_horizontal_carry_and_ramp_height() {
    for rpm in [-60.0, 60.0] {
        let (mut app, body, belt) = standing_on_inclined_belt(rpm);
        let start = app.world().get::<Transform>(body).unwrap().translation;
        app.world_mut().resource_mut::<Drive>().input.direction =
            -Vec3::new(belt.x, 0.0, belt.z) / WALK_SPEED;
        for _ in 0..90 {
            app.update();
            let position = app.world().get::<Transform>(body).unwrap().translation;
            assert!(
                position.distance(start) < 0.04,
                "Counterwalking moved player off the ramp: {start:?} -> {position:?}"
            );
            assert!(app.world().get::<CharacterGrounded>(body).unwrap().0);
            assert!(app.world().get::<LinearVelocity>(body).unwrap().0.y.abs() < 0.001);
        }
        assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    }
}

#[test]
fn jumping_from_an_inclined_belt_disconnects_carry_until_the_player_lands() {
    for rpm in [-60.0, 60.0] {
        let (mut app, body, _) = standing_on_inclined_belt(rpm);
        let start = app.world().get::<Transform>(body).unwrap().translation;
        app.world_mut().resource_mut::<Drive>().input.jump = true;
        app.update();
        app.world_mut().resource_mut::<Drive>().input.jump = false;
        assert!(!app.world().get::<CharacterGrounded>(body).unwrap().0);
        let velocity = app.world().get::<LinearVelocity>(body).unwrap().0;
        assert!(velocity.y > 7.0 && velocity.y < JUMP_SPEED, "{velocity:?}");
        assert!(velocity.x.abs() < 0.001 && velocity.z.abs() < 0.001);
        let mut apex = start.y;
        let mut landed = false;
        for _ in 0..90 {
            app.update();
            let position = app.world().get::<Transform>(body).unwrap().translation;
            apex = apex.max(position.y);
            if app.world().get::<CharacterGrounded>(body).unwrap().0 {
                landed = true;
                break;
            }
            assert!(
                Vec2::new(position.x - start.x, position.z - start.z).length() < 0.02,
                "Airborne player still received belt carry: {start:?} -> {position:?}"
            );
        }
        assert!(apex - start.y > 1.0, "Belt prevented the real jump");
        assert!(landed, "Player did not land back on the inclined belt");
        assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
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

#[test]
fn creative_flight_collides_with_walls_floor_and_ceiling_while_spectator_passes_through() {
    for direction in [Vec3::X, Vec3::Y, Vec3::NEG_Y] {
        for mode in [
            terra_core::GameMode::Creative,
            terra_core::GameMode::Spectator,
        ] {
            let (mut app, body) = fixture(2.0, false, Some(4.5));
            {
                let mut drive = app.world_mut().resource_mut::<Drive>();
                drive.mode = Some(mode);
                drive.input = CharacterInput {
                    direction,
                    flight_speed: Some(if direction == Vec3::X { 12.0 } else { 6.0 }),
                    ..default()
                };
            }
            for _ in 0..40 {
                app.update();
            }
            let position = app.world().get::<Transform>(body).unwrap().translation;
            if mode == terra_core::GameMode::Creative {
                assert!(position.x + PLAYER_RADIUS <= 8.03, "wall: {position:?}");
                assert!(
                    position.y - PLAYER_HALF_HEIGHT >= 0.98,
                    "floor: {position:?}"
                );
                assert!(
                    position.y + PLAYER_HALF_HEIGHT <= 4.52,
                    "ceiling: {position:?}"
                );
                assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
            } else if direction == Vec3::X {
                assert!(position.x > 12.0, "spectator wall: {position:?}");
            } else if direction == Vec3::Y {
                assert!(position.y > 5.0, "spectator ceiling: {position:?}");
            } else {
                assert!(position.y < 0.0, "spectator floor: {position:?}");
            }
        }
    }
}

#[test]
fn creative_flight_accelerates_smoothly_and_pauses_atomically_for_missing_collision() {
    let (mut app, body) = fixture(0.5, false, None);
    {
        let mut drive = app.world_mut().resource_mut::<Drive>();
        drive.mode = Some(terra_core::GameMode::Creative);
        drive.input = CharacterInput {
            direction: Vec3::Y,
            flight_speed: Some(6.0),
            ..default()
        };
    }
    app.update();
    let first = app.world().get::<LinearVelocity>(body).unwrap().y;
    assert!(first > 0.0 && first < 6.0);
    app.update();
    let second = app.world().get::<LinearVelocity>(body).unwrap().y;
    assert!(second > first && second < 6.0);
    app.world_mut().resource_mut::<Drive>().input.direction = Vec3::NEG_Y;
    app.update();
    let reversed = app.world().get::<LinearVelocity>(body).unwrap().y;
    assert!(reversed < second && reversed > -6.0);
    let position = app.world().get::<Transform>(body).unwrap().translation;
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.update();
    assert_eq!(
        app.world().get::<Transform>(body).unwrap().translation,
        position
    );
    assert_eq!(app.world().get::<LinearVelocity>(body).unwrap().y, reversed);
    assert!(app.world().resource::<Drive>().missing_collision > 0);
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

#[test]
fn stale_collision_rejects_a_tick_without_destroying_jump_or_fall_velocity() {
    for (height, stairs) in [(0.5, false), (0.5, true), (1.0, false)] {
        let (mut app, body) = fixture(height, stairs, None);
        // Invalidate the swept body region; distant edits may now keep moving.
        let changed = app
            .world()
            .get::<Transform>(body)
            .unwrap()
            .translation
            .to_array()
            .map(|v| v.floor() as i64);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(changed, 3)
            .unwrap();
        for (vertical_speed, jump, grounded) in
            [(0.0, true, true), (6.0, false, false), (-4.0, false, false)]
        {
            let before = CharacterState {
                position: app.world().get::<Transform>(body).unwrap().translation,
                velocity: Vec3::new(1.0, vertical_speed, 0.0),
                grounded,
            };
            let mut state = before;
            let collider = app.world().get::<Collider>(body).unwrap().clone();
            let mut params =
                bevy::ecs::system::SystemState::<CharacterSimulation>::new(app.world_mut());
            let simulation = params.get_mut(app.world_mut()).unwrap();
            assert!(!simulation.step(
                body,
                &collider,
                &mut state,
                CharacterInput {
                    direction: Vec3::X,
                    jump,
                    ..default()
                },
            ));
            assert_eq!(state.position, before.position);
            assert_eq!(state.velocity, before.velocity);
            assert_eq!(state.grounded, before.grounded);
        }
    }
}

#[test]
fn jumps_from_slabs_stair_treads_and_blocks_keep_their_ballistic_ascent() {
    for (height, stairs, auto_climb) in
        [(0.5, false, false), (0.5, true, false), (1.0, false, true)]
    {
        let (mut app, body) = fixture(height, stairs, None);
        walk(&mut app, body, auto_climb);
        let base = app.world().get::<Transform>(body).unwrap().translation.y;
        app.world_mut().resource_mut::<Drive>().input.jump = true;
        app.update();
        app.world_mut().resource_mut::<Drive>().input.jump = false;
        let mut peak = base;
        for _ in 0..55 {
            app.update();
            peak = peak.max(app.world().get::<Transform>(body).unwrap().translation.y);
        }
        assert!(
            (1.05..1.4).contains(&(peak - base)),
            "jump height={}",
            peak - base
        );
        assert_eq!(app.world().resource::<Drive>().missing_collision, 0);
    }
}

#[test]
fn remote_placement_burst_does_not_pause_the_character_motor() {
    let (mut app, body) = fixture(0.5, false, None);
    let collider = app.world().get::<Collider>(body).unwrap().clone();
    let mut state = CharacterState {
        position: app.world().get::<Transform>(body).unwrap().translation,
        velocity: Vec3::ZERO,
        grounded: true,
    };
    for x in 1..6 {
        // Confirm edits while the old collider is still installed. No app.update
        // runs here, so the asynchronous replacement cannot mask a motor pause.
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([x, 2, 2], 3)
            .unwrap();
        let before = state.position;
        let mut params =
            bevy::ecs::system::SystemState::<CharacterSimulation>::new(app.world_mut());
        let simulation = params.get_mut(app.world_mut()).unwrap();
        assert!(simulation.step(
            body,
            &collider,
            &mut state,
            CharacterInput {
                direction: Vec3::X,
                ..default()
            }
        ));
        assert!(state.position.x > before.x);
    }
}
