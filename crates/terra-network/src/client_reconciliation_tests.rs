use super::*;

#[test]
fn incomplete_collision_replay_keeps_the_current_pose_and_pending_input_transaction() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(terra_core::TerraCorePlugin(
            terra_core::GameConfig::default(),
        ))
        .add_plugins(terra_physics::TerraPhysicsPlugin);
    let mut session = ClientSession {
        ready: true,
        sequence: 2,
        latest: Some((
            2,
            PlayerState {
                ack: 1,
                position: [2.5, 1.92, 2.5],
                grounded: true,
                ..default()
            },
        )),
        ..default()
    };
    for sequence in 1..=2 {
        session.pending.push_back(MovementInput {
            generation: 0,
            sequence,
            direction: [0.0; 3],
            look: [0.0; 2],
            jump: sequence == 2,
            sprint: false,
            auto_climb: false,
            flight_speed: None,
        });
    }
    session.pending_inputs = session.pending.len();
    let position = Vec3::new(2.5, 2.8, 2.5);
    let body = app
        .world_mut()
        .spawn(terra_physics::character::body_bundle(position))
        .id();
    app.world_mut()
        .get_mut::<avian3d::prelude::LinearVelocity>(body)
        .unwrap()
        .0 = Vec3::Y * 6.0;
    app.insert_resource(session).add_systems(Update, reconcile);
    app.finish();
    app.cleanup();
    app.update();
    let session = app.world().resource::<ClientSession>();
    assert_eq!(session.reconciled, 0);
    assert_eq!(session.pending.len(), 2);
    assert_eq!(session.pending_inputs, 2);
    assert_eq!(
        app.world().get::<Transform>(body).unwrap().translation,
        position
    );
    assert_eq!(
        app.world()
            .get::<avian3d::prelude::LinearVelocity>(body)
            .unwrap()
            .0,
        Vec3::Y * 6.0
    );
    assert_eq!(
        app.world()
            .get::<terra_physics::character::CharacterPresentation>(body)
            .unwrap()
            .correction,
        Vec3::ZERO
    );
    let confirmed = Vec3::new(2.5, 3.05, 2.5);
    app.world_mut().resource_mut::<ClientSession>().latest = Some((
        3,
        PlayerState {
            ack: 2,
            position: confirmed.to_array(),
            velocity: [0.0, 4.0, 0.0],
            ..default()
        },
    ));
    app.update();
    let session = app.world().resource::<ClientSession>();
    assert_eq!(session.reconciled, 3);
    assert!(session.pending.is_empty());
    assert_eq!(session.pending_inputs, 0);
    assert_eq!(
        app.world().get::<Transform>(body).unwrap().translation,
        confirmed
    );
}

fn queued_prediction(count: u64) -> ClientSession {
    let mut session = ClientSession {
        ready: true,
        sequence: count,
        latest: Some((1, PlayerState::default())),
        ..default()
    };
    for sequence in 1..=count {
        session.pending.push_back(MovementInput {
            generation: 0,
            sequence,
            direction: [1.0, 0.0, 0.0],
            look: [0.0; 2],
            jump: sequence == count,
            sprint: false,
            auto_climb: false,
            flight_speed: None,
        });
    }
    session.pending_inputs = session.pending.len();
    session
}

#[test]
fn prediction_horizon_scales_with_tick_rate_and_leaves_room_for_normal_snapshot_latency() {
    for tick_rate in [30_u16, 60, 120, 240] {
        let delta = 1.0 / f32::from(tick_rate);
        let limit = prediction_lead(delta);
        assert!((8..=MAX_PREDICTION_LEAD).contains(&limit));
        assert!(limit < MAX_INPUT_BACKLOG);
        // A healthy 100 ms round trip plus one 20 Hz snapshot interval
        // must fit at higher tick rates as well as the default 60 Hz.
        let delayed_inputs = (f32::from(tick_rate) * 0.15).ceil() as u64;
        let mut session = queued_prediction(delayed_inputs);
        assert!(session.prediction_capacity(delta), "tick_rate={tick_rate}");
        let mut full = queued_prediction(limit as u64);
        assert!(!full.prediction_capacity(delta));
        assert!(full.ready && full.error.is_none());
        assert_eq!(full.pending.len(), limit);
        assert!(full.pending.back().unwrap().jump);
        // Only confirmed commands free capacity, independent of whether
        // collision-dependent authoritative replay has completed yet.
        full.latest.as_mut().unwrap().1.ack = 1;
        assert!(full.prediction_capacity(delta));
        assert_eq!(full.pending.front().unwrap().sequence, 2);
        assert_eq!(full.pending.len(), limit - 1);
        assert!(full.pending.back().unwrap().jump);
        assert_eq!(full.sequence, limit as u64);
    }
}

#[test]
fn confirmed_acknowledgments_retire_only_completed_steps_even_when_replay_is_pending() {
    let mut session = queued_prediction(128);
    session.latest.as_mut().unwrap().1.ack = 124;
    assert!(session.prediction_capacity(1.0 / 60.0));
    assert_eq!(session.pending.len(), 4);
    assert_eq!(session.pending.front().unwrap().sequence, 125);
    assert_eq!(session.pending.back().unwrap().sequence, 128);
    assert_eq!(session.reconciled, 0);
    assert!(session.ready && session.error.is_none());
}

#[test]
fn prediction_backpressure_preserves_the_wall_time_acknowledgment_watchdog() {
    let mut session = queued_prediction(prediction_lead(1.0 / 60.0) as u64);
    session.acknowledged_at = Instant::now() - INPUT_ACK_TIMEOUT;
    assert!(!session.prediction_capacity(1.0 / 60.0));
    assert!(session.ready && session.error.is_none());
    session.check_acknowledgment_liveness();
    assert!(!session.ready);
    assert_eq!(
        session.error.as_deref(),
        Some("Server input acknowledgement timed out")
    );

    let mut progressing = queued_prediction(12);
    progressing.acknowledged_at = Instant::now() - INPUT_ACK_TIMEOUT;
    progressing.latest.as_mut().unwrap().1.ack = 6;
    assert!(progressing.prediction_capacity(1.0 / 60.0));
    progressing.check_acknowledgment_liveness();
    assert!(progressing.ready && progressing.error.is_none());
    assert_eq!(progressing.pending.len(), 6);
    assert!(progressing.acknowledged_at.elapsed() < Duration::from_secs(1));
}

#[test]
fn prediction_capacity_rejects_an_acknowledgment_of_unsimulated_input() {
    let mut session = queued_prediction(2);
    session.latest.as_mut().unwrap().1.ack = 3;
    assert!(!session.prediction_capacity(1.0 / 60.0));
    assert!(!session.ready);
    assert_eq!(
        session.error.as_deref(),
        Some("Invalid input acknowledgement")
    );
    assert_eq!(session.pending.len(), 2);
}
