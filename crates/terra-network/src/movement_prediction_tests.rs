use super::*;
use terra_physics::character::{CharacterBody, CharacterInput};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore},
};

fn body_position(app: &mut App) -> Vec3 {
    app.world_mut()
        .query_filtered::<&Transform, With<CharacterBody>>()
        .single(app.world())
        .unwrap()
        .translation
}

fn ordinary_edit_burst(host: &mut App, current: Vec3, count: usize) -> usize {
    let properties = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    let mut changed = 0;
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for index in 0..count {
                let position = [
                    current.x.floor() as i64 + 5 + index as i64,
                    current.y.floor() as i64 + 2,
                    current.z.floor() as i64 + 5,
                ];
                let Some(expected) = chunks.block_at(position) else {
                    continue;
                };
                let material = if expected == 3 { 0 } else { 3 };
                assert!(
                    terra_world::streaming::commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &BlockEditRequest {
                            position,
                            expected_material: expected,
                            material,
                        },
                        &properties
                    )
                    .unwrap()
                );
                changed += 1;
            }
        });
    changed
}

fn ready_walker(id: u64) -> (App, App) {
    let mut host = server();
    // Creative mode alone does not grant flight to an unprivileged remote
    // profile. These high-speed fixtures explicitly request authorized flight.
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .allow_flight = true;
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(id)
        .unwrap();
    let mut client = walking_client(token);
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|state| state.grounded)
    });
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .allow_flight
    );
    for _ in 0..18 {
        host.update();
        client.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    (host, client)
}

#[test]
fn real_udp_high_speed_prediction_keeps_its_distance_through_packet_and_ordinary_edit_bursts() {
    let (mut host, mut client) = ready_walker(210_801);
    let initial = body_position(&mut client);
    let sequence = client
        .world()
        .resource::<client::ClientSession>()
        .prediction_sequence_for_test();
    client.world_mut().resource_mut::<Desired>().0 = CharacterInput {
        direction: Vec3::X,
        flight_speed: Some(40.0),
        ..Default::default()
    };
    let mut largest_error = 0.0_f32;
    let mut edits = 0;
    let deadline = Instant::now() + Duration::from_secs(15);
    while client
        .world()
        .resource::<client::ClientSession>()
        .prediction_sequence_for_test()
        - sequence
        < 72
    {
        assert!(
            Instant::now() < deadline,
            "streamed high-speed movement did not become ready"
        );
        for _ in 0..3 {
            client.update();
            let n = client
                .world()
                .resource::<client::ClientSession>()
                .prediction_sequence_for_test()
                - sequence;
            let decay = (-10.0_f64 / 60.0).exp();
            let expected = f64::from(initial.x)
                + 40.0 / 60.0 * (n as f64 - decay * (1.0 - decay.powf(n as f64)) / (1.0 - decay));
            let actual = body_position(&mut client).x;
            largest_error = largest_error.max((actual - expected as f32).abs());
            assert!(
                (actual - expected as f32).abs() < 0.08,
                "predicted steps={n}, expected={expected}, actual={actual}, max={largest_error}"
            );
            assert!(
                client
                    .world()
                    .resource::<client::ClientSession>()
                    .error
                    .is_none(),
                "server={:?}",
                server::movement_diagnostics_for_test(host.world_mut())
            );
        }
        let near = body_position(&mut client);
        edits += ordinary_edit_burst(&mut host, near, 3);
        for _ in 0..3 {
            host.update();
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert!(
        edits >= 24,
        "ordinary authoritative edit path was exercised: {edits}"
    );
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .prediction_sequence_for_test()
            - sequence
            >= 40
    );
    eprintln!(
        "movement_burst flight_speed=40 edits={edits} maximum_position_error={largest_error:.6}"
    );
}

#[test]
fn real_udp_jump_replay_has_no_systematic_vertical_drift_during_batched_delivery() {
    let (mut host, mut client) = ready_walker(210_802);
    let initial = body_position(&mut client);
    let sequence = client
        .world()
        .resource::<client::ClientSession>()
        .prediction_sequence_for_test();
    client.world_mut().resource_mut::<Desired>().0.jump = true;
    client.update();
    client.world_mut().resource_mut::<Desired>().0.jump = false;
    let mut samples = 0;
    let mut maximum_error = 0.0_f32;
    for _ in 0..9 {
        for _ in 0..3 {
            host.update();
            std::thread::sleep(Duration::from_millis(1));
        }
        for _ in 0..3 {
            client.update();
            let n = client
                .world()
                .resource::<client::ClientSession>()
                .prediction_sequence_for_test()
                - sequence;
            if n == 0 || n > 28 {
                continue;
            }
            let n = n as f32;
            let expected = initial.y + n * 7.8 / 60.0 - 24.0 / (60.0 * 60.0) * n * (n + 1.0) * 0.5;
            let actual = body_position(&mut client).y;
            maximum_error = maximum_error.max((actual - expected).abs());
            assert!(
                (actual - expected).abs() < 0.08,
                "jump steps={n}, expected={expected}, actual={actual}, max={maximum_error}"
            );
            samples += 1;
        }
    }
    assert!(samples >= 12);
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .error
            .is_none()
    );
    eprintln!("movement_burst jump_samples={samples} maximum_vertical_error={maximum_error:.6}");
}

#[test]
fn sparse_input_packets_cannot_repeatedly_renew_airborne_wait_and_suspend_gravity() {
    use lightyear::prelude::{MessageSender, client::Client};
    use protocol::session::{InputChannel, MovementInput};
    fn send_input(client: &mut App, sequence: u64, jump: bool) {
        let world = client.world_mut();
        world
            .query_filtered::<&mut MessageSender<MovementInput>, With<Client>>()
            .single_mut(world)
            .unwrap()
            .send::<InputChannel>(MovementInput {
                generation: 0,
                sequence,
                direction: [0.0; 3],
                look: [0.0; 2],
                jump,
                sprint: false,
                auto_climb: false,
                flight_speed: None,
            });
    }
    let (mut host, mut a, mut b) = pair();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|state| state.grounded)
    });
    let initial = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    send_input(&mut a, 1, true);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|state| state.ack == 1 && state.position[1] > initial.position[1] + 0.1)
    });
    let mut sequence = 1;
    for tick in 0..78 {
        if tick % 6 == 0 {
            sequence += 1;
            send_input(&mut a, sequence, false);
        }
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    let after = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    assert!(
        after.grounded && (after.position[1] - initial.position[1]).abs() < 0.05,
        "packet pulses cannot buy a fresh gravity suspension: {after:?}"
    );
    assert!(after.ack >= 2);
}

// The renderer cannot publish a future epoch. This exercises the same actual
// collision-readiness barrier as an unfinished section replacement, only on
// the authoritative app; no movement acknowledgment or body is fabricated.
fn hold_server_collision(
    host: &mut App,
    near: Vec3,
    held: &mut std::collections::BTreeMap<terra_voxel::SectionPos, u64>,
) {
    let center = terra_voxel::global_to_section(near.to_array().map(|v| v.floor() as i64)).0;
    for (&position, section) in &mut host.world_mut().resource_mut::<ChunkStore>().loaded {
        if (position.x - center.x).abs() <= 1
            && (position.y - center.y).abs() <= 1
            && (position.z - center.z).abs() <= 1
        {
            if section.mesh_epoch != u64::MAX {
                held.insert(position, section.mesh_epoch);
            }
            section.mesh_epoch = u64::MAX;
        }
    }
}

fn release_server_collision(
    host: &mut App,
    held: &mut std::collections::BTreeMap<terra_voxel::SectionPos, u64>,
) {
    let mut chunks = host.world_mut().resource_mut::<ChunkStore>();
    for (position, epoch) in std::mem::take(held) {
        if let Some(section) = chunks.loaded.get_mut(&position)
            && section.mesh_epoch == u64::MAX
        {
            section.mesh_epoch = epoch;
        }
    }
}

#[test]
fn real_udp_long_running_asymmetric_clocks_and_collision_stalls_bound_prediction_without_lost_steps()
 {
    use bevy::time::TimeUpdateStrategy;
    let (mut host, mut client) = ready_walker(211_801);
    let initial = body_position(&mut client);
    let initial_sequence = client
        .world()
        .resource::<client::ClientSession>()
        .prediction_sequence_for_test();
    let mut reference_position = f64::from(initial.x);
    let mut reference_velocity = 0.0_f64;
    let mut direction = 1.0_f32;
    let mut held = std::collections::BTreeMap::new();
    let mut maximum_error = 0.0_f32;
    let mut maximum_pending = 0;
    let mut blocked_fixed_steps = 0;
    let mut throttled_frames = 0;
    let mut edits = 0;
    let server_started = host.world().resource::<Time<Fixed>>().elapsed();
    let client_started = client
        .world()
        .resource::<Time<bevy::time::Virtual>>()
        .elapsed();
    let deadline = Instant::now() + Duration::from_secs(120);
    // These independent frame phases deliberately lose authoritative fixed
    // steps while the client keeps running. Balanced one-update-each fixtures
    // cannot reproduce the integrated server's permanent queue debt.
    for frame in 0..3600 {
        assert!(
            Instant::now() < deadline,
            "long-running movement test timed out"
        );
        if reference_position > f64::from(initial.x) + 8.0 {
            direction = -1.0;
        } else if reference_position < f64::from(initial.x) - 8.0 {
            direction = 1.0;
        }
        client.world_mut().resource_mut::<Desired>().0 = CharacterInput {
            direction: Vec3::X * direction,
            flight_speed: Some(40.0),
            ..Default::default()
        };
        client.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            [10, 23, 17][frame % 3],
        )));
        let previous = client
            .world()
            .resource::<client::ClientSession>()
            .prediction_sequence_for_test();
        client.update();
        let session = client.world().resource::<client::ClientSession>();
        assert!(
            session.ready && session.error.is_none(),
            "frame={frame}: {:?}; sequence={} pending={} server={:?}",
            session.error,
            session.prediction_sequence_for_test(),
            session.pending_inputs,
            server::movement_diagnostics_for_test(host.world_mut())
        );
        let sequence = session.prediction_sequence_for_test();
        maximum_pending = maximum_pending.max(session.pending_inputs);
        assert!(session.pending_inputs <= client::MAX_PREDICTION_LEAD);
        if sequence == previous && session.pending_inputs >= 18 {
            throttled_frames += 1;
        }
        for _ in previous..sequence {
            reference_velocity += (f64::from(direction) * 40.0 - reference_velocity)
                * (1.0 - (-10.0_f64 / 60.0).exp());
            reference_position += reference_velocity / 60.0;
        }
        let error = (body_position(&mut client).x - reference_position as f32).abs();
        maximum_error = maximum_error.max(error);
        assert!(
            error < 0.12,
            "frame={frame} max_error={maximum_error} actual={} expected={reference_position}",
            body_position(&mut client).x
        );

        let stalled = frame % 60 < 4;
        if stalled {
            let near = Vec3::from_array(
                client
                    .world()
                    .resource::<client::ClientSession>()
                    .confirmed_state()
                    .unwrap()
                    .position,
            );
            hold_server_collision(&mut host, near, &mut held);
        } else {
            release_server_collision(&mut host, &mut held);
        }
        if frame % 120 == 30 {
            let near = body_position(&mut client);
            edits += ordinary_edit_burst(&mut host, near, 3);
        }
        host.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            [25, 10, 14][frame % 3],
        )));
        let before = host.world().resource::<Time<Fixed>>().elapsed();
        host.update();
        if stalled {
            let elapsed = host.world().resource::<Time<Fixed>>().elapsed() - before;
            blocked_fixed_steps += (elapsed.as_secs_f64() * 60.0).round() as usize;
        }
        // Real UDP transport/task workers get CPU time, while simulated
        // independent clocks cover more than the reported 54-second failure.
        std::thread::sleep(Duration::from_millis(1));
    }
    release_server_collision(&mut host, &mut held);
    assert!(
        host.world().resource::<Time<Fixed>>().elapsed() - server_started > Duration::from_secs(54)
    );
    assert!(
        client
            .world()
            .resource::<Time<bevy::time::Virtual>>()
            .elapsed()
            - client_started
            > Duration::from_secs(54)
    );
    assert!(
        blocked_fixed_steps > 128,
        "blocked fixed steps={blocked_fixed_steps}"
    );
    assert!(
        throttled_frames > 0,
        "the prediction horizon was never exercised"
    );
    assert!(edits >= 60, "ordinary authoritative edits={edits}");
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .prediction_sequence_for_test()
            - initial_sequence
            > 2500
    );

    // Stop producing new fixed inputs while still receiving real snapshots.
    // Every previously simulated step must be ACKed and replay to the same
    // position; none may be silently discarded to escape the queue limit.
    client.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    host.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .pending_inputs
            == 0
    });
    let session = client.world().resource::<client::ClientSession>();
    assert!(session.ready && session.error.is_none());
    let acknowledged_position = session.confirmed_state().unwrap().position[0];
    assert!((acknowledged_position - reference_position as f32).abs() < 0.12);
    assert!((body_position(&mut client).x - reference_position as f32).abs() < 0.12);
    let drained_sequence = client
        .world()
        .resource::<client::ClientSession>()
        .prediction_sequence_for_test();
    client.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    for _ in 0..24 {
        host.update();
        client.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        client
            .world()
            .resource::<client::ClientSession>()
            .prediction_sequence_for_test()
            >= drained_sequence + 20
    );
    eprintln!(
        "movement_long_run simulated_client_seconds=60 speed=40 blocked_fixed_steps={blocked_fixed_steps} ordinary_edits={edits} maximum_pending={maximum_pending} throttled_frames={throttled_frames} maximum_position_error={maximum_error:.6}"
    );
}
