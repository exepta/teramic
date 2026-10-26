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
