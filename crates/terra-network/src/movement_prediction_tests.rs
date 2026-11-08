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
                    .is_none()
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
