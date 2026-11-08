use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_physics::TerraPhysicsPlugin;
use terra_voxel::{BlockSection, SectionPos};
use terra_world::streaming::LoadedSection;

fn flight_toggle_app() -> App {
    let mut app = App::new();
    let mut session = ClientSession::default();
    session.ready = true;
    session.allow_flight = true;
    session.game_mode = terra_core::GameMode::Creative;
    app.init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ControllerMode>()
        .init_resource::<PlayerInputBlock>()
        .insert_resource(CameraInputState {
            captured: true,
            ..default()
        })
        .insert_resource(session)
        .add_systems(Update, toggle_flight);
    app
}

fn tap_flight_key(app: &mut App, key: KeyCode, elapsed_ms: u64) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(elapsed_ms));
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.reset_all();
    keys.press(key);
    app.update();
}

#[test]
fn creative_flight_uses_fast_jump_pairs_and_ignores_held_jump_slow_taps_and_old_f() {
    let mut app = flight_toggle_app();
    tap_flight_key(&mut app, KeyCode::KeyF, 0);
    tap_flight_key(&mut app, KeyCode::KeyF, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::Space, 0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(100));
    app.update();
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::Space, 250);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
}

#[test]
fn flight_double_tap_obeys_jump_rebinding_and_discards_menu_input() {
    let mut app = flight_toggle_app();
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(Action::Jump, terra_core::InputBinding::Key(KeyCode::KeyP))
        .unwrap();
    app.insert_resource(controls);
    tap_flight_key(&mut app, KeyCode::Space, 0);
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::KeyP, 0);
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    app.update();
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
    tap_flight_key(&mut app, KeyCode::KeyP, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::KeyP, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
}

#[test]
fn double_jump_flight_requires_ready_authorized_non_spectator_gameplay() {
    for (mode, ready, permission, captured) in [
        (terra_core::GameMode::Survival, true, false, true),
        (terra_core::GameMode::Spectator, true, true, true),
        (terra_core::GameMode::Creative, false, true, true),
        (terra_core::GameMode::Creative, true, false, true),
        (terra_core::GameMode::Creative, true, true, false),
    ] {
        let mut app = flight_toggle_app();
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.game_mode = mode;
        session.ready = ready;
        session.allow_flight = permission;
        app.world_mut().resource_mut::<CameraInputState>().captured = captured;
        tap_flight_key(&mut app, KeyCode::Space, 0);
        tap_flight_key(&mut app, KeyCode::Space, 100);
        assert_eq!(
            *app.world().resource::<ControllerMode>(),
            ControllerMode::Walk
        );
    }
}

#[test]
fn authorized_survival_can_leave_and_reenter_flight_after_a_creative_mode_change() {
    let mut app = flight_toggle_app();
    app.init_resource::<ChunkStore>()
        .init_resource::<PlayerControllerStatus>()
        .add_systems(Update, sync_controller.before(toggle_flight));
    tap_flight_key(&mut app, KeyCode::Space, 0);
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    app.world_mut().resource_mut::<ClientSession>().game_mode = terra_core::GameMode::Survival;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Walk
    );
    tap_flight_key(&mut app, KeyCode::Space, 100);
    tap_flight_key(&mut app, KeyCode::Space, 100);
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
}

#[test]
fn restored_join_applies_saved_camera_aim_and_flight_once_before_first_body_frame() {
    let location = terra_core::PlayerLocation {
        position: [-17.5, -28.08, 16.5],
        look: [-1.2, 0.44],
        flying: true,
    };
    let mut app = App::new();
    let mut session = ClientSession::default();
    session.ready = true;
    session.allow_flight = true;
    session.spawn = Vec3::from_array(location.position);
    session.initial_location = Some(location);
    app.init_resource::<ChunkStore>()
        .init_resource::<ControllerMode>()
        .init_resource::<PlayerControllerStatus>()
        .insert_resource(session)
        .add_systems(Update, sync_controller);
    let camera = app
        .world_mut()
        .spawn((TerraCamera, Transform::default()))
        .id();
    app.update();
    assert_eq!(
        *app.world().resource::<ControllerMode>(),
        ControllerMode::Fly
    );
    let transform = app.world().get::<Transform>(camera).unwrap();
    assert_eq!(
        transform.translation,
        Vec3::from_array(location.position) + Vec3::Y * PLAYER_EYE_OFFSET
    );
    let rotation = Quat::from_euler(EulerRot::YXZ, location.look[0], location.look[1], 0.0);
    assert!(
        app.world()
            .get::<CameraLook>(camera)
            .unwrap()
            .rotation()
            .abs_diff_eq(rotation, 0.00001)
    );
    assert_eq!(
        app.world().resource::<ClientSession>().initial_location,
        None
    );
    assert_eq!(app.world().resource::<ClientSession>().look, location.look);
    assert_eq!(
        app.world_mut()
            .query_filtered::<&Transform, With<PlayerBody>>()
            .single(app.world())
            .unwrap()
            .translation,
        Vec3::from_array(location.position)
    );
    app.world_mut()
        .get_mut::<Transform>(camera)
        .unwrap()
        .translation = Vec3::ONE;
    app.update();
    assert_eq!(
        app.world().get::<Transform>(camera).unwrap().translation,
        Vec3::ONE
    );
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(GameConfig::default()))
        .add_plugins(TerraPhysicsPlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .insert_resource(ChunkFocus([2.5, 5.0, 2.5]))
        .insert_resource(CameraInputState {
            captured: true,
            ..default()
        })
        .init_resource::<ControllerMode>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<PlayerControllerStatus>()
        .init_resource::<WalkIntent>()
        .add_systems(Update, sync_controller)
        .add_systems(FixedUpdate, walk_step.after(PhysicsSystems::Writeback));
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    for z in 0..16 {
        for x in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    for z in 0..16 {
        for y in 1..5 {
            data.set_block([5, y, z], 3).unwrap();
        }
    }
    for (position, data) in [
        (SectionPos::default(), data),
        (
            SectionPos::new(0, -1, 0),
            BlockSection::empty(SectionPos::new(0, -1, 0)).unwrap(),
        ),
    ] {
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
    app.world_mut()
        .spawn((TerraCamera, Transform::from_xyz(2.5, 5.0, 2.5)));
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if !app
            .world()
            .resource::<PlayerControllerStatus>()
            .waiting_for_terrain
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "controller waiting for collision data"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app
}

fn body(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PlayerBody>>()
        .single(app.world())
        .unwrap()
}

fn step_obstacle(auto_climb: bool) -> Vec3 {
    let mut app = app();
    app.insert_resource(terra_core::ControlsConfig {
        auto_climb,
        ..default()
    });
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([4, 1, 2], 3)
        .unwrap();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        section.data.set_block([4, 1, 2], 3).unwrap();
        section.mesh_epoch = 1;
    }
    for _ in 0..60 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::X;
    for _ in 0..120 {
        app.update();
    }
    app.world().get::<Transform>(body).unwrap().translation
}

#[test]
fn automatic_step_climbs_one_block_but_not_the_taller_wall() {
    let position = step_obstacle(true);
    assert!(
        position.x > 4.3 && position.x < 4.8,
        "step/wall: {position:?}"
    );
    assert!(
        position.y > 2.7 && position.y < 3.1,
        "step height: {position:?}"
    );
}

#[test]
fn default_controls_do_not_climb_a_one_block_obstacle() {
    assert!(!terra_core::ControlsConfig::default().auto_climb);
    let position = step_obstacle(false);
    assert!(
        position.x > 3.5 && position.x < 3.8 && position.y < 2.1,
        "auto-climb disabled: {position:?}"
    );
}

#[test]
fn automatic_step_respects_low_ceiling_clearance() {
    let mut app = app();
    app.insert_resource(terra_core::ControlsConfig {
        auto_climb: true,
        ..default()
    });
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    for p in [[4, 1, 2], [2, 3, 2], [3, 3, 2], [4, 3, 2]] {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(p, 3)
            .unwrap();
    }
    let epoch = app
        .world()
        .resource::<EditJournal>()
        .mesh_epoch(SectionPos::default());
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        section.data.set_block([4, 1, 2], 3).unwrap();
        for x in 2..5 {
            section.data.set_block([x, 3, 2], 3).unwrap();
        }
        section.mesh_epoch = epoch;
    }
    for _ in 0..60 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::X;
    for _ in 0..120 {
        app.update();
    }
    let position = app.world().get::<Transform>(body).unwrap().translation;
    assert!(
        position.x > 3.5 && position.x < 3.8 && position.y < 2.1,
        "ceiling: {position:?}"
    );
}

#[test]
fn swimming_holds_depth_after_descent_and_space_ascends_without_flight_permission() {
    let mut app = app();
    let body = body(&mut app);
    let mut config = app.world().resource::<GameConfig>().world.clone();
    config.biomes = false;
    config.base_height = -4.0;
    config.hill_amplitude = 0.0;
    config.mountain_amplitude = 0.0;
    config.caves = false;
    config.trees = false;
    config.sea_level = 8.0;
    app.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::TerraGenerator::new(config.clone()).unwrap(),
    ));
    app.world_mut().resource_mut::<GameConfig>().world = config;
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation
        .y = 4.0;
    for _ in 0..240 {
        app.update();
    }
    let surface = app.world().get::<Transform>(body).unwrap().translation.y;
    assert!((surface - 4.0).abs() < 0.1, "depth: {surface}");
    app.world_mut().resource_mut::<WalkIntent>().direction.y = -1.0;
    for _ in 0..60 {
        app.update();
    }
    let descended = app.world().get::<Transform>(body).unwrap().translation.y;
    assert!(
        descended < surface - 0.5,
        "descent: {surface} -> {descended}"
    );
    app.world_mut().resource_mut::<WalkIntent>().direction.y = 0.0;
    for _ in 0..120 {
        app.update();
    }
    let held = app.world().get::<Transform>(body).unwrap().translation.y;
    assert!(
        held <= descended + 0.05 && held > descended - 0.5,
        "released: {descended} -> {held}"
    );
    app.world_mut().resource_mut::<WalkIntent>().jump = true;
    for _ in 0..30 {
        app.world_mut().resource_mut::<WalkIntent>().jump = true;
        app.update();
    }
    assert!(app.world().get::<Transform>(body).unwrap().translation.y > held + 0.3);
}

#[test]
fn control_tap_latches_directional_diving_and_space_releases_it() {
    let mut app = app();
    let body = body(&mut app);
    let mut config = app.world().resource::<GameConfig>().world.clone();
    config.biomes = false;
    config.base_height = -4.0;
    config.hill_amplitude = 0.0;
    config.mountain_amplitude = 0.0;
    config.caves = false;
    config.trees = false;
    config.sea_level = 8.0;
    app.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::TerraGenerator::new(config.clone()).unwrap(),
    ));
    app.world_mut().resource_mut::<GameConfig>().world = config;
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation
        .y = 4.0;
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<TerraCamera>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<CameraLook>(camera)
        .unwrap()
        .aim(Quat::from_rotation_x(-0.5));
    app.init_resource::<ButtonInput<KeyCode>>()
        .add_systems(Update, read_walk_input.after(sync_controller));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ControlLeft);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();
    assert!(app.world().resource::<WalkIntent>().diving);
    assert!(app.world().resource::<WalkIntent>().direction.y < 0.0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ControlLeft);
    app.update();
    assert!(app.world().resource::<WalkIntent>().diving);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.update();
    assert!(!app.world().resource::<WalkIntent>().diving);
    assert!(app.world().resource::<WalkIntent>().jump);
}

#[test]
fn swimming_surface_jump_exits_onto_a_one_meter_bank() {
    let mut app = app();
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    let mut config = app.world().resource::<GameConfig>().world.clone();
    config.biomes = false;
    config.base_height = 0.0;
    config.hill_amplitude = 0.0;
    config.mountain_amplitude = 0.0;
    config.caves = false;
    config.trees = false;
    config.sea_level = 2.0;
    app.insert_resource(terra_world::streaming::WorldTerrain(
        terra_world::TerraGenerator::new(config.clone()).unwrap(),
    ));
    app.world_mut().resource_mut::<GameConfig>().world = config;
    for y in 1..=2 {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([4, y, 2], 3)
            .unwrap();
    }
    let epoch = app
        .world()
        .resource::<EditJournal>()
        .mesh_epoch(SectionPos::default());
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        for y in 1..=2 {
            section.data.set_block([4, y, 2], 3).unwrap();
        }
        section.mesh_epoch = epoch;
    }
    for _ in 0..60 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..90 {
        {
            let mut intent = app.world_mut().resource_mut::<WalkIntent>();
            intent.direction = Vec3::X;
            intent.jump = true;
        }
        app.update();
    }
    let p = app.world().get::<Transform>(body).unwrap().translation;
    assert!(p.x > 4.2 && p.x < 4.8 && p.y > 3.7, "shore exit: {p:?}");
}

#[test]
fn camera_moves_each_render_frame_between_fixed_physics_steps() {
    let mut app = app();
    for _ in 0..120 {
        app.update();
    }
    app.add_systems(Update, follow_player_camera);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 144.0,
    )));
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::X;
    let mut previous = None;
    for frame in 0..24 {
        app.update();
        let x = app
            .world_mut()
            .query_filtered::<&Transform, With<TerraCamera>>()
            .single(app.world())
            .unwrap()
            .translation
            .x;
        if frame > 4
            && let Some(previous) = previous
        {
            let delta: f32 = x - previous;
            assert!(
                delta > 0.01 && delta < 0.07,
                "quantized camera motion: {delta}"
            );
        }
        previous = Some(x);
    }
}

fn presented_walk_app() -> (App, Entity, Entity) {
    let mut app = App::new();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
    app.insert_resource(time)
        .init_resource::<Time<Fixed>>()
        .insert_resource(ChunkFocus([0.0; 3]))
        .init_resource::<ControllerMode>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<super::super::CameraPerspective>()
        .init_resource::<terra_core::GameplayConfig>()
        .insert_resource(CameraInputState {
            captured: true,
            ..default()
        })
        .insert_resource(WalkIntent {
            direction: Vec3::X,
            ..default()
        })
        .add_systems(Update, follow_player_camera);
    let body = app
        .world_mut()
        .spawn((
            PlayerBody,
            PlayerGrounded(true),
            Transform::from_xyz(-16.5, 2.0, 3.0),
            LinearVelocity(Vec3::X * terra_physics::character::WALK_SPEED),
        ))
        .id();
    let aim = Quat::from_euler(EulerRot::YXZ, 0.4, -0.25, 0.0);
    let camera = app
        .world_mut()
        .spawn((TerraCamera, Transform::from_rotation(aim)))
        .id();
    app.world_mut()
        .get_mut::<CameraLook>(camera)
        .unwrap()
        .aim(aim);
    (app, body, camera)
}

#[test]
fn walk_bob_only_changes_visual_translation_not_aim_body_or_streaming_focus() {
    let (mut app, body, camera) = presented_walk_app();
    let rotation = app.world().get::<Transform>(camera).unwrap().rotation;
    app.update();
    let mut largest_offset = 0.0_f32;
    for tick in 1..=60 {
        let position = Vec3::new(-16.5 + tick as f32 * 0.09, 2.0, 3.0);
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation = position;
        app.update();
        let view = app.world().get::<Transform>(camera).unwrap();
        let eye = position + Vec3::Y * PLAYER_EYE_OFFSET;
        largest_offset = largest_offset.max(view.translation.distance(eye));
        assert_eq!(view.rotation, rotation);
        assert_eq!(
            app.world().get::<CameraLook>(camera).unwrap().rotation(),
            rotation
        );
        assert_eq!(
            app.world().get::<Transform>(body).unwrap().translation,
            position
        );
        assert_eq!(
            app.world().resource::<ChunkFocus>().0,
            eye.to_array().map(f64::from)
        );
        assert!(view.translation.distance(eye) < 0.06);
    }
    assert!(largest_offset > 0.025, "walking has no perceptible gait");
}

#[test]
fn menus_swimming_third_person_and_motion_setting_disable_walk_bob() {
    for disabled_by in 0..4 {
        let (mut app, body, camera) = presented_walk_app();
        app.update();
        for _ in 0..20 {
            app.world_mut()
                .get_mut::<Transform>(body)
                .unwrap()
                .translation
                .x += 0.09;
            app.update();
        }
        match disabled_by {
            0 => app.world_mut().resource_mut::<PlayerInputBlock>().0 = true,
            1 => app.world_mut().resource_mut::<WalkIntent>().swimming = true,
            2 => {
                *app.world_mut()
                    .resource_mut::<super::super::CameraPerspective>() =
                    super::super::CameraPerspective::ThirdPerson;
            }
            _ => {
                app.world_mut()
                    .resource_mut::<terra_core::GameplayConfig>()
                    .view_bobbing = false;
            }
        }
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation
            .x += 0.09;
        app.update();
        let eye =
            app.world().get::<Transform>(body).unwrap().translation + Vec3::Y * PLAYER_EYE_OFFSET;
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            eye
        );
    }
}

#[test]
fn jump_has_a_short_ballistic_arc_without_hovering() {
    let mut app = app();
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    let base = app.world().get::<Transform>(body).unwrap().translation.y;
    app.world_mut().resource_mut::<WalkIntent>().jump = true;
    let mut peak = base;
    for frame in 0..55 {
        app.update();
        let y = app.world().get::<Transform>(body).unwrap().translation.y;
        peak = peak.max(y);
        if frame == 42 {
            assert!(y < base + 0.1, "still floating after 0.7 seconds: {y}");
        }
    }
    assert!(
        (1.05..1.4).contains(&(peak - base)),
        "jump height: {}",
        peak - base
    );
}

#[test]
fn collision_rebuild_does_not_consume_a_queued_jump_or_cancel_its_ascent() {
    for interrupted_in_air in [false, true] {
        let mut app = app();
        let body = body(&mut app);
        for _ in 0..120 {
            app.update();
        }
        let base = app.world().get::<Transform>(body).unwrap().translation.y;
        app.world_mut().resource_mut::<WalkIntent>().jump = true;
        if interrupted_in_air {
            for _ in 0..6 {
                app.update();
            }
        }
        let before = app.world().get::<Transform>(body).unwrap().translation;
        let velocity = app.world().get::<LinearVelocity>(body).unwrap().0;
        let grounded = app.world().get::<PlayerGrounded>(body).unwrap().0;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .mesh_epoch = 999;
        for _ in 0..6 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<PlayerControllerStatus>()
                .waiting_for_terrain
        );
        assert_eq!(
            app.world().get::<Transform>(body).unwrap().translation,
            before
        );
        assert_eq!(app.world().get::<LinearVelocity>(body).unwrap().0, velocity);
        assert_eq!(app.world().get::<PlayerGrounded>(body).unwrap().0, grounded);
        assert_eq!(
            app.world().resource::<WalkIntent>().jump,
            !interrupted_in_air
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .mesh_epoch = 0;
        let mut peak = before.y;
        for _ in 0..55 {
            app.update();
            peak = peak.max(app.world().get::<Transform>(body).unwrap().translation.y);
        }
        assert!(
            (1.05..1.4).contains(&(peak - base)),
            "jump height={}",
            peak - base
        );
        assert!(!app.world().resource::<WalkIntent>().jump);
    }
}

#[test]
fn avian_walk_falls_lands_jumps_and_cannot_walk_through_a_voxel_wall() {
    let mut app = app();
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    let ground_height = app.world().get::<Transform>(body).unwrap().translation.y;
    assert!((ground_height - 1.9).abs() < 0.08, "height={ground_height}");
    assert!(app.world().get::<PlayerGrounded>(body).unwrap().0);
    app.world_mut().resource_mut::<WalkIntent>().jump = true;
    for _ in 0..12 {
        app.update();
    }
    assert!(app.world().get::<Transform>(body).unwrap().translation.y > ground_height + 0.6);
    assert!(!app.world().get::<PlayerGrounded>(body).unwrap().0);
    for _ in 0..100 {
        app.update();
    }
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::X;
    for _ in 0..100 {
        app.update();
    }
    let position = app.world().get::<Transform>(body).unwrap().translation;
    assert!(
        position.x > 4.5 && position.x < 4.7,
        "wall collision: {position:?}"
    );
    assert!(app.world().get::<PlayerGrounded>(body).unwrap().0);
}

#[test]
fn menu_blocks_motion_and_jump_but_support_mining_still_applies_gravity() {
    let mut app = app();
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    let before = app.world().get::<Transform>(body).unwrap().translation;
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    {
        let mut intent = app.world_mut().resource_mut::<WalkIntent>();
        intent.direction = Vec3::X;
        intent.jump = true;
    }
    for _ in 0..20 {
        app.update();
    }
    let after = app.world().get::<Transform>(body).unwrap().translation;
    assert!(
        (after - before).length() < 0.05,
        "menu moved body: {before:?} -> {after:?}"
    );
    let affected = app
        .world_mut()
        .resource_mut::<EditJournal>()
        .record([2, 0, 2], 0)
        .unwrap();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        section.data.set_block([2, 0, 2], 0).unwrap();
        section.mesh_epoch = 1;
        for position in affected {
            if let Some(section) = chunks.loaded.get_mut(&position) {
                section.mesh_epoch = 1;
            }
        }
    }
    for _ in 0..100 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.world().get::<Transform>(body).unwrap().translation.y < before.y - 1.0);
}

#[test]
fn unloaded_collision_sections_hold_the_player_and_fly_mode_releases_the_body() {
    let mut app = app();
    let body = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    let before = app.world().get::<Transform>(body).unwrap().translation;
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::X;
    for _ in 0..20 {
        app.update();
    }
    assert_eq!(
        app.world().get::<Transform>(body).unwrap().translation,
        before
    );
    assert!(
        app.world()
            .resource::<PlayerControllerStatus>()
            .waiting_for_terrain
    );
    *app.world_mut().resource_mut::<ControllerMode>() = ControllerMode::Fly;
    app.update();
    assert!(app.world().get_entity(body).is_err());
}

#[test]
fn typing_chat_or_search_does_not_wake_a_sleeping_player() {
    for (blocked, captured, just_captured, expected) in [
        (true, true, false, false),
        (false, false, false, false),
        (false, true, true, false),
        (false, true, false, true),
    ] {
        let mut app = App::new();
        let mut session = ClientSession::default();
        session.ready = true;
        session.player = Some(terra_core::PlayerId(1));
        session
            .sleepers
            .push(terra_network::protocol::session::SleepingPlayer {
                player: terra_core::PlayerId(1),
                anchor: [0, 0, 0],
                facing: 0,
            });
        app.init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(CameraInputState {
                captured,
                just_captured,
                ..Default::default()
            })
            .init_resource::<ControllerMode>()
            .insert_resource(PlayerInputBlock(blocked))
            .insert_resource(session)
            .init_resource::<WalkIntent>()
            .add_systems(Update, read_walk_input);
        app.world_mut().spawn((TerraCamera, Transform::default()));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert_eq!(
            app.world().resource::<ClientSession>().wake_requested,
            expected
        );
        assert_eq!(app.world().resource::<WalkIntent>().direction, Vec3::ZERO);
    }
}

#[test]
fn x_dismounts_and_shift_sprints_only_with_captured_mounted_input() {
    for (key, blocked) in [
        (KeyCode::KeyX, false),
        (KeyCode::KeyX, true),
        (KeyCode::ShiftLeft, false),
        (KeyCode::ShiftLeft, true),
    ] {
        use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
        let horse = Creature {
            id: (1 << 62) + 400,
            home: [0, 0],
            kind: CreatureKind::Horse,
            position: [0.0, 1.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            age: 0.0,
            health: 20.0,
            timer: 0.0,
            variant: 0,
            behavior: CreatureBehavior::Rest,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        };
        let mut session = ClientSession::default();
        session.ready = true;
        session.mounted = Some(terra_network::protocol::session::MountedState::new(
            &horse, 20.0, true,
        ));
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(CameraInputState {
                captured: true,
                ..Default::default()
            })
            .insert_resource(PlayerInputBlock(blocked))
            .insert_resource(session)
            .init_resource::<ControllerMode>()
            .init_resource::<WalkIntent>()
            .add_systems(Update, read_walk_input);
        app.world_mut().spawn((TerraCamera, Transform::default()));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        assert_eq!(
            app.world().resource::<ClientSession>().dismount_requested,
            !blocked && key == KeyCode::KeyX
        );
        assert_eq!(
            app.world().resource::<WalkIntent>().sprint,
            !blocked && key == KeyCode::ShiftLeft
        );
    }
}

#[test]
fn diagonal_walk_intent_is_normalized_and_placement_rejects_body_overlap() {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(CameraInputState {
            captured: true,
            ..default()
        })
        .init_resource::<ControllerMode>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<WalkIntent>()
        .add_systems(Update, read_walk_input);
    app.world_mut().spawn((TerraCamera, Transform::default()));
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
    }
    app.update();
    assert!((app.world().resource::<WalkIntent>().direction.length() - 1.0).abs() < 1e-6);
    assert!(block_overlaps_body([0, 0, 0], Vec3::new(0.5, 0.9, 0.5)));
    assert!(!block_overlaps_body([2, 0, 0], Vec3::new(0.5, 0.9, 0.5)));
}

#[test]
fn walk_crosses_negative_section_boundaries_without_catching_or_falling() {
    let mut app = app();
    let mut data = BlockSection::empty(SectionPos::new(-1, 0, 0)).unwrap();
    for z in 0..16 {
        for x in 0..16 {
            data.set_block([x, 0, z], 3).unwrap();
        }
    }
    for (position, data) in [
        (SectionPos::new(-1, 0, 0), data),
        (
            SectionPos::new(-1, -1, 0),
            BlockSection::empty(SectionPos::new(-1, -1, 0)).unwrap(),
        ),
    ] {
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let world = app.world();
        if world.resource::<TerrainCollisionState>().ready_for_aabb(
            Vec3::new(-1.0, 0.0, 1.0),
            Vec3::new(1.0, 3.0, 3.0),
            world.resource::<ChunkStore>(),
            world.resource::<EditJournal>(),
        ) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..120 {
        app.update();
    }
    app.world_mut().resource_mut::<WalkIntent>().direction = Vec3::NEG_X;
    for _ in 0..120 {
        app.update();
    }
    let body = body(&mut app);
    let position = app.world().get::<Transform>(body).unwrap().translation;
    assert!(position.x < -4.0, "section seam blocked walk: {position:?}");
    assert!(
        (position.y - 1.9).abs() < 0.08,
        "section seam lost ground: {position:?}"
    );
    assert!(app.world().get::<PlayerGrounded>(body).unwrap().0);
}

#[test]
fn network_prediction_backpressure_keeps_the_queued_jump_and_body_until_acknowledgment() {
    let mut app = app();
    let entity = body(&mut app);
    for _ in 0..120 {
        app.update();
    }
    assert!(app.world().get::<PlayerGrounded>(entity).unwrap().0);
    let mut session = ClientSession::default();
    session.ready = true;
    session.game_mode = terra_core::GameMode::Creative;
    app.insert_resource(session);
    let mut blocked_at = None;
    for _ in 0..=terra_network::client::MAX_PREDICTION_LEAD {
        let before = app.world().resource::<ClientSession>().pending_inputs;
        app.update();
        let pending = app.world().resource::<ClientSession>().pending_inputs;
        if pending == before && pending > 0 {
            blocked_at = Some(pending);
            break;
        }
    }
    let lead = blocked_at.expect("the real prediction window must apply bounded backpressure");
    assert!(lead <= terra_network::client::MAX_PREDICTION_LEAD);
    let position = app.world().get::<Transform>(entity).unwrap().translation;
    let velocity = app.world().get::<LinearVelocity>(entity).unwrap().0;
    app.world_mut().resource_mut::<WalkIntent>().jump = true;
    for _ in 0..6 {
        app.update();
        assert!(
            app.world().resource::<WalkIntent>().jump,
            "predict(false) cannot consume a jump that the authority has not received"
        );
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            position
        );
        assert_eq!(
            app.world().get::<LinearVelocity>(entity).unwrap().0,
            velocity
        );
        let session = app.world().resource::<ClientSession>();
        assert!(session.ready && session.error.is_none());
        assert_eq!(session.pending_inputs, lead);
    }
    // ACK reopening and the eventual single jump are exercised by the network
    // flow/UDP regressions. This Player-level test verifies intent ownership at
    // the real consumed_input=false boundary without forging a network ACK.
}

#[test]
fn train_camera_uses_the_seat_without_physics_lerp_walk_bob_or_stair_smoothing() {
    use terra_core::{
        PlayerUuid,
        trains::{SeatBinding, TrainPart, VehicleKind, VehicleView},
    };
    let (mut app, body, camera) = presented_walk_app();
    let owner = PlayerUuid::from_u128(712);
    let mut session = ClientSession::default();
    session.ready = true;
    session.uuid = Some(owner);
    session.train_seat = Some(SeatBinding {
        player: owner,
        vehicle: 1,
        part: 0,
    });
    session.trains.vehicles.push(VehicleView {
        id: 1,
        owner,
        kind: VehicleKind::Locomotive,
        anchor: [0, 2, 0],
        parts: vec![TrainPart {
            offset: [0; 3],
            material: 214,
            facing: 0,
            wood: Default::default(),
        }],
        blocks: Default::default(),
        couplings: [None; 2],
        revision: 1,
        mass_kg: 2000,
        motion: Default::default(),
    });
    app.insert_resource(session);
    app.world_mut()
        .entity_mut(body)
        .insert(terra_physics::character::CharacterPresentation {
            previous: Vec3::splat(-100.0),
            correction: Vec3::splat(0.3),
        });
    for frame in 0..60 {
        app.world_mut()
            .resource_mut::<ClientSession>()
            .trains
            .vehicles[0]
            .motion
            .translation = [f64::from(frame) * 0.1, f64::from(frame) * 0.01, 0.0];
        let expected = app
            .world()
            .resource::<ClientSession>()
            .seated_state()
            .unwrap()
            .position
            + Vec3::Y * PLAYER_EYE_OFFSET;
        app.update();
        let position = app.world().get::<Transform>(camera).unwrap().translation;
        assert!(
            position.distance(expected) < 1e-5,
            "seat camera drifted at frame {frame}: {position:?} / {expected:?}"
        );
    }
}
