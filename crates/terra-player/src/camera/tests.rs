use super::*;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use std::time::Duration;
use terra_core::GameConfig;
use terra_world::streaming::ChunkFocus;
fn app() -> (App, Entity) {
    let mut app = App::new();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs(1));
    app.insert_resource(time)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(AccumulatedMouseMotion::default())
        .insert_resource(GameConfig::default())
        .insert_resource(ControllerMode::Fly)
        .init_resource::<PlayerInputBlock>()
        .insert_resource(ChunkFocus([0.0; 3]))
        .insert_resource(CameraInputState {
            captured: true,
            accept_input: true,
            just_captured: false,
            initial_capture_pending: false,
            suppress_motion: false,
        })
        .add_systems(Update, move_camera);
    let entity = app
        .world_mut()
        .spawn((TerraCamera, Transform::default()))
        .id();
    (app, entity)
}

#[test]
fn f5_cycles_behind_front_and_first_person_and_respects_ui_lock() {
    let mut app = App::new();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<CameraPerspective>()
        .init_resource::<PlayerInputBlock>()
        .insert_resource(CameraInputState {
            captured: true,
            ..default()
        })
        .add_systems(Update, perspective::toggle);
    for expected in [
        CameraPerspective::ThirdPerson,
        CameraPerspective::ThirdPersonFront,
        CameraPerspective::FirstPerson,
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F5);
        app.update();
        assert_eq!(*app.world().resource::<CameraPerspective>(), expected);
    }
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    app.update();
    assert_eq!(
        *app.world().resource::<CameraPerspective>(),
        CameraPerspective::FirstPerson
    );
}

#[test]
fn front_view_does_not_feed_its_reversed_rotation_back_into_mouse_aim() {
    let (mut app, entity) = app();
    let aim = Quat::from_euler(EulerRot::YXZ, 0.6, -0.3, 0.0);
    let front = Transform::default()
        .looking_to(-(aim * Vec3::NEG_Z), Vec3::Y)
        .rotation;
    {
        let mut look = app.world_mut().get_mut::<CameraLook>(entity).unwrap();
        look.aim(aim);
        look.presented(front);
    }
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .rotation = front;
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::new(10.0, 5.0);
    app.update();
    let updated = app.world().get::<CameraLook>(entity).unwrap().rotation();
    let expected = Quat::from_euler(EulerRot::YXZ, 0.578, -0.311, 0.0);
    assert!(updated.angle_between(expected) < 0.0001);
}
#[test]
fn space_ascends_and_both_shift_keys_descend_without_sprinting() {
    for (key, expected) in [
        (KeyCode::Space, 18.00009),
        (KeyCode::ShiftLeft, -18.00009),
        (KeyCode::ShiftRight, -18.00009),
    ] {
        let (mut app, entity) = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        assert!(
            (app.world().get::<Transform>(entity).unwrap().translation.y - expected).abs() < 0.001
        );
        assert!((app.world().resource::<ChunkFocus>().0[1] - f64::from(expected)).abs() < 0.001);
    }
}
#[test]
fn mouse_look_needs_no_mouse_button_and_forward_stays_horizontal() {
    let (mut app, entity) = app();
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::new(100.0, -150.0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();
    let transform = app.world().get::<Transform>(entity).unwrap();
    assert_ne!(transform.rotation, Quat::IDENTITY);
    assert_eq!(transform.translation.y, 0.0);
    assert!((transform.translation.length() - 18.00009).abs() < 0.001);
}
#[test]
fn released_cursor_disables_movement_and_look() {
    let (mut app, entity) = app();
    app.world_mut().resource_mut::<CameraInputState>().captured = false;
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::splat(100.0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    app.update();
    assert_eq!(
        *app.world().get::<Transform>(entity).unwrap(),
        Transform::default()
    );
}

#[test]
fn menu_blocks_flight_and_mouse_look_without_relying_on_cursor_capture() {
    let (mut app, entity) = app();
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = true;
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::splat(100.0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();
    assert_eq!(
        *app.world().get::<Transform>(entity).unwrap(),
        Transform::default()
    );
}

fn capture_app(blocked: bool, focused: bool) -> (App, Entity) {
    use bevy::window::{CursorOptions, PrimaryWindow, WindowFocused};
    let mut app = App::new();
    app.insert_resource(CameraInputState::default())
        .insert_resource(PlayerInputBlock(blocked))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<WindowFocused>()
        .add_message::<CaptureRequest>()
        .add_systems(Update, update_capture);
    let entity = app
        .world_mut()
        .spawn((
            Window {
                focused,
                ..default()
            },
            PrimaryWindow,
            CursorOptions::default(),
        ))
        .id();
    (app, entity)
}

#[test]
fn construction_owns_the_cursor_despite_panel_resume_and_never_replays_stale_capture() {
    use bevy::window::{CursorGrabMode, CursorOptions, WindowFocused};
    let (mut app, window) = capture_app(true, true);
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs(1));
    app.insert_resource(time)
        .init_resource::<ConstructionCamera>()
        .init_resource::<GameConfig>()
        .insert_resource(ControllerMode::Fly)
        .insert_resource(ChunkFocus([4.0, 16.0, 22.0]))
        .init_resource::<AccumulatedMouseMotion>()
        .add_systems(Update, move_camera.after(update_capture));
    let pose = Transform::from_xyz(4.0, 16.0, 22.0).with_rotation(Quat::from_rotation_y(0.25));
    let camera = app.world_mut().spawn((TerraCamera, pose)).id();

    // Keep a real pending capture from a blocked panel before entering.
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    app.world_mut().resource_mut::<ConstructionCamera>().active = true;
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
    {
        let mut input = app.world_mut().resource_mut::<CameraInputState>();
        input.captured = true;
        input.initial_capture_pending = true;
        input.suppress_motion = false;
        assert!(
            input.accept_input,
            "exercise normal input, not benchmark bypass"
        );
    }
    {
        let mut cursor = app.world_mut().get_mut::<CursorOptions>(window).unwrap();
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::new(100.0, -75.0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Backquote);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.world_mut().write_message(WindowFocused {
        window,
        focused: true,
    });
    for frame in 0..3 {
        if frame == 1 {
            // Repair actual window state even if the logical capture is already off.
            assert!(!app.world().resource::<CameraInputState>().captured);
            let mut cursor = app.world_mut().get_mut::<CursorOptions>(window).unwrap();
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        // Model another panel's release, even after its block was cleared.
        app.world_mut().write_message(CaptureRequest(true));
        app.update();
        let input = app.world().resource::<CameraInputState>();
        assert!(input.accept_input);
        assert!(!input.captured && !input.just_captured && !input.capture_ready());
        assert!(!input.initial_capture_pending && input.suppress_motion);
        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert_eq!(cursor.grab_mode, CursorGrabMode::None);
        assert!(cursor.visible);
        assert_eq!(*app.world().get::<Transform>(camera).unwrap(), pose);
        assert_eq!(
            app.world().get::<FlightMotion>(camera).unwrap().velocity,
            Vec3::ZERO
        );
    }

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut().resource_mut::<ConstructionCamera>().active = false;
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    assert_eq!(*app.world().get::<Transform>(camera).unwrap(), pose);
    assert!(app.world().get::<CursorOptions>(window).unwrap().visible);

    // A new explicit resume still captures, preserving the one-frame input gate.
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    let input = app.world().resource::<CameraInputState>();
    assert!(input.captured && input.just_captured);
    assert!(!input.capture_ready());
    assert_eq!(*app.world().get::<Transform>(camera).unwrap(), pose);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    app.update();
    assert!(app.world().resource::<CameraInputState>().capture_ready());
}

#[test]
fn joining_a_focused_world_captures_after_loading_unblocks_without_another_click() {
    use bevy::window::{CursorGrabMode, CursorOptions};
    let (mut app, window) = capture_app(true, true);
    let mut session = terra_network::client::ClientSession::default();
    session.ready = true;
    app.insert_resource(session);

    // Loading requests capture in PostUpdate; the UI clears this blocker later.
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
    app.update();

    let input = app.world().resource::<CameraInputState>();
    assert!(input.captured);
    assert!(input.just_captured);
    assert!(!input.capture_ready());
    let cursor = app.world().get::<CursorOptions>(window).unwrap();
    assert_eq!(cursor.grab_mode, CursorGrabMode::Locked);
    assert!(!cursor.visible);
    app.update();
    assert!(app.world().resource::<CameraInputState>().capture_ready());
}

#[test]
fn pending_join_capture_waits_for_session_readiness_and_window_focus() {
    let (mut app, window) = capture_app(false, false);
    app.init_resource::<terra_network::client::ClientSession>();
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);

    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .ready = true;
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);

    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    assert!(app.world().resource::<CameraInputState>().captured);
}

#[test]
fn release_pause_network_failure_and_disabled_input_cancel_pending_capture() {
    for cancellation in 0..4 {
        let (mut app, _) = capture_app(true, true);
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        app.insert_resource(session);
        app.world_mut().write_message(CaptureRequest(true));
        app.update();
        match cancellation {
            0 => {
                app.world_mut().write_message(CaptureRequest(false));
            }
            1 => app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::Escape),
            2 => {
                app.world_mut()
                    .resource_mut::<terra_network::client::ClientSession>()
                    .error = Some("Disconnected".into());
            }
            3 => {
                app.world_mut()
                    .resource_mut::<CameraInputState>()
                    .accept_input = false;
            }
            _ => unreachable!(),
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .error = None;
        app.world_mut()
            .resource_mut::<CameraInputState>()
            .accept_input = true;
        app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
        app.update();
        assert!(
            !app.world().resource::<CameraInputState>().captured,
            "pending capture survived cancellation {cancellation}"
        );
    }
}

#[test]
fn explicit_menu_resume_captures_even_on_the_escape_toggle_frame() {
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
    let mut app = App::new();
    app.insert_resource(CameraInputState::default())
        .insert_resource(PlayerInputBlock(true))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<WindowFocused>()
        .add_message::<CaptureRequest>()
        .add_systems(Update, update_capture);
    let window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
            CursorOptions::default(),
        ))
        .id();
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    app.world_mut().resource_mut::<PlayerInputBlock>().0 = false;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.world_mut().write_message(CaptureRequest(true));
    app.update();
    assert!(app.world().resource::<CameraInputState>().captured);
    assert!(app.world().resource::<CameraInputState>().just_captured);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
}

#[test]
fn network_failure_releases_cursor_and_allows_clicking_retry() {
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
    let mut app = App::new();
    app.insert_resource(CameraInputState {
        captured: true,
        ..default()
    })
    .init_resource::<PlayerInputBlock>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<WindowFocused>()
    .add_message::<CaptureRequest>()
    .init_resource::<terra_network::client::ClientSession>()
    .add_systems(Update, update_capture);
    app.world_mut()
        .resource_mut::<terra_network::client::ClientSession>()
        .error = Some("Disconnected".into());
    let window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                visible: false,
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    let cursor = app.world().get::<CursorOptions>(window).unwrap();
    assert!(cursor.visible);
    assert_eq!(cursor.grab_mode, CursorGrabMode::None);
}

#[test]
fn startup_capture_waits_for_focus_and_focus_loss_releases_it() {
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
    let mut app = App::new();
    app.insert_resource(CameraInputState {
        initial_capture_pending: true,
        ..default()
    })
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<WindowFocused>()
    .add_message::<CaptureRequest>()
    .init_resource::<PlayerInputBlock>()
    .add_systems(Update, update_capture);
    let entity = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
            CursorOptions::default(),
        ))
        .id();
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    app.world_mut().write_message(WindowFocused {
        window: entity,
        focused: true,
    });
    app.update();
    assert!(app.world().resource::<CameraInputState>().captured);
    let cursor = app.world().get::<CursorOptions>(entity).expect("cursor");
    assert_eq!(cursor.grab_mode, CursorGrabMode::Locked);
    assert!(!cursor.visible);
    app.world_mut()
        .get_mut::<Window>(entity)
        .expect("window")
        .focused = false;
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    let cursor = app.world().get::<CursorOptions>(entity).expect("cursor");
    assert_eq!(cursor.grab_mode, CursorGrabMode::None);
    assert!(cursor.visible);
    // Automated rendering checks must stay at their fixed camera even if clicked.
    app.world_mut()
        .resource_mut::<CameraInputState>()
        .accept_input = false;
    app.world_mut()
        .get_mut::<Window>(entity)
        .expect("window")
        .focused = true;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
}

#[test]
fn ctrl_boost_alt_precision_and_diagonal_movement_have_consistent_speeds() {
    for (modifier, expected) in [
        (KeyCode::ControlLeft, 54.00027),
        (KeyCode::AltLeft, 4.500023),
    ] {
        let (mut app, entity) = app();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
        keys.press(modifier);
        app.update();
        assert!(
            (app.world()
                .get::<Transform>(entity)
                .unwrap()
                .translation
                .length()
                - expected)
                .abs()
                < 0.001
        );
    }
    let (mut app, entity) = app();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.press(KeyCode::KeyW);
    keys.press(KeyCode::ControlLeft);
    keys.press(KeyCode::AltLeft);
    app.update();
    assert!(
        (app.world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .length()
            - 4.500023)
            .abs()
            < 0.001
    );
}

#[test]
fn flight_response_is_frame_rate_independent_and_stops_without_drift() {
    let mut results = Vec::new();
    for frames in [30, 60, 144] {
        let (mut app, entity) = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..frames {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f64(1.0 / f64::from(frames)));
            app.update();
        }
        let before = app.world().get::<Transform>(entity).unwrap().translation;
        results.push(before);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            before
        );
        assert_eq!(
            app.world().get::<FlightMotion>(entity).unwrap().velocity,
            Vec3::ZERO
        );
    }
    assert!((results[0] - results[1]).length() < 0.001);
    assert!((results[0] - results[2]).length() < 0.001);
}

#[test]
fn escape_unlocks_without_exit_and_click_recaptures() {
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
    let mut app = App::new();
    app.insert_resource(CameraInputState {
        captured: true,
        ..default()
    })
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<WindowFocused>()
    .add_message::<CaptureRequest>()
    .init_resource::<PlayerInputBlock>()
    .add_message::<bevy::app::AppExit>()
    .add_systems(Update, update_capture);
    let entity = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                visible: false,
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<CameraInputState>().captured);
    assert_eq!(
        app.world().get::<CursorOptions>(entity).unwrap().grab_mode,
        CursorGrabMode::None
    );
    assert!(app.world().get::<CursorOptions>(entity).unwrap().visible);
    assert!(app.should_exit().is_none());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app.world().resource::<CameraInputState>().captured);
}
