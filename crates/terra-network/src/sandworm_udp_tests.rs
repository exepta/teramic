//! Real authenticated movement, server combat and anchored vertical eruptions.
use super::*;
use terra_core::{
    GameMode, PlayerLocation, PlayerProfile, PlayerStats, SurvivalVitals, creatures::*,
};
use terra_world::edits::EditJournal;

fn same_phase(host: &mut App, a: &mut App, b: &mut App, id: u64, behavior: CreatureBehavior) {
    wait(host, &mut [a, b], |_, clients| {
        clients.iter().all(|client| {
            let replicas = client.world().resource::<client::ReplicatedCreatures>();
            replicas.rejected_snapshots == 0
                && replicas.creatures.get(&id).is_some_and(|track| {
                    track.current.kind == CreatureKind::SandWorm
                        && track.current.behavior == behavior
                })
        })
    });
}
fn health(client: &App) -> f32 {
    client
        .world()
        .resource::<client::ClientSession>()
        .stats
        .survival
        .health
}
fn position(client: &App) -> [f32; 3] {
    client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
}
fn stop(client: &mut App) {
    let mut desired = client.world_mut().resource_mut::<Desired>();
    desired.0.direction = Vec3::ZERO;
    desired.0.sprint = false;
}
fn move_toward(client: &mut App, destination: [f64; 3]) {
    let start = position(client);
    let direction = Vec3::new(
        destination[0] as f32 - start[0],
        0.0,
        destination[2] as f32 - start[2],
    )
    .normalize_or_zero();
    let mut desired = client.world_mut().resource_mut::<Desired>();
    desired.0.direction = direction;
    desired.0.sprint = true;
}
fn arrived(client: &App, destination: [f64; 3]) -> bool {
    let p = position(client);
    (f64::from(p[0]) - destination[0]).hypot(f64::from(p[2]) - destination[2]) <= 0.25
}
fn arrive_both(host: &mut App, a: &mut App, pa: [f64; 3], b: &mut App, pb: [f64; 3]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Each real player stops on its own arrival. One continuously walking
        // player cannot overshoot while waiting for the longer second route.
        if arrived(a, pa) {
            stop(a);
        } else {
            move_toward(a, pa);
        }
        if arrived(b, pb) {
            stop(b);
        } else {
            move_toward(b, pb);
        }
        wait(host, &mut [&mut *a, &mut *b], |_, _| true);
        if arrived(a, pa) && arrived(b, pb) {
            stop(a);
            stop(b);
            return;
        }
        assert!(
            Instant::now() < deadline,
            "independent real-input arrivals timed out: {:?}/{pa:?}, {:?}/{pb:?}",
            position(a),
            position(b)
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn authoritative_worm(host: &App, id: u64) -> Creature {
    host.world()
        .resource::<EditJournal>()
        .gameplay()
        .creatures
        .animals[&id]
        .clone()
}
fn assert_landed(host: &App, clients: [&App; 2], id: u64, destination: [f64; 3]) {
    let worm = authoritative_worm(host, id);
    assert!(
        worm.position
            .into_iter()
            .zip(destination)
            .all(|(a, b)| (a - b).abs() < 0.001)
    );
    for client in clients {
        let replicas = client.world().resource::<client::ReplicatedCreatures>();
        let track = &replicas.creatures[&id];
        assert_eq!(track.current.position, worm.position);
        assert!(
            track
                .position()
                .distance(Vec3::from_array(destination.map(|v| v as f32)))
                < 0.01,
            "a hidden re-anchor must not slide the next breach between holes"
        );
    }
}

#[test]
fn real_udp_worm_warns_both_clients_strikes_once_ignores_creative_and_allows_a_real_dodge() {
    let mut host = server();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
        config.world.time.paused = true;
    }
    let profiles = [PlayerProfile::default(), PlayerProfile::default()];
    let body_y = 32.0 + terra_physics::character::PLAYER_HALF_HEIGHT;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        // Two vertical exit/withdrawal cycles, a six-metre dodge and eight
        // metres of acid escape fit on sand. Upright mouths need air to 40 m.
        for x in 2..=26 {
            for z in -28..=24 {
                for y in 28..=42 {
                    journal
                        .record([x, y, z], if y <= 31 { 4 } else { 0 })
                        .unwrap();
                }
            }
        }
        for (index, profile) in profiles.iter().enumerate() {
            journal
                .update_player_location(
                    profile.uuid,
                    PlayerLocation {
                        position: [8.5 + index as f32, body_y, 8.5],
                        look: [0.0; 2],
                        flying: false,
                    },
                )
                .unwrap();
            journal.update_player_stats(
                profile.uuid,
                PlayerStats {
                    game_mode: Some(if index == 0 {
                        GameMode::Survival
                    } else {
                        GameMode::Creative
                    }),
                    survival: SurvivalVitals {
                        health: if index == 0 { 80.0 } else { 100.0 },
                        hunger: 60.0,
                        ..default()
                    },
                    ..default()
                },
            );
        }
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = walking_client(browser::join(address, &profiles[0]).unwrap());
    a.insert_resource(profiles[0].clone());
    let mut b = walking_client(browser::join(address, &profiles[1]).unwrap());
    b.insert_resource(profiles[1].clone());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            let session = client.world().resource::<client::ClientSession>();
            session.ready && session.confirmed_state().is_some_and(|s| s.grounded)
        })
    });
    assert_eq!(
        a.world().resource::<client::ClientSession>().game_mode,
        GameMode::Survival
    );
    assert_eq!(
        b.world().resource::<client::ClientSession>().game_mode,
        GameMode::Creative
    );
    let anchor = [8.5, 32.0, 10.0];
    let id = creature_id([0, 0], 0).unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        population.animals.clear();
        population.explored.insert([0, 0]);
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::SandWorm,
                home: [0, 0],
                position: anchor,
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::SandWorm.health(),
                behavior: CreatureBehavior::Burrowed,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::SandWorm),
            },
        );
    }
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Burrowed);
    let before = health(&a);
    assert_eq!(before, 80.0);
    let at_rest = host.world().resource::<Time<Real>>().elapsed_secs();
    wait(&mut host, &mut [&mut a, &mut b], |host, _| {
        host.world().resource::<Time<Real>>().elapsed_secs() - at_rest >= 0.8
    });
    assert_eq!(
        authoritative_worm(&host, id).behavior,
        CreatureBehavior::Burrowed,
        "nearby stationary players cannot start periodic idle eruptions"
    );

    // Wake it with actual prediction/input packets, not a server-side teleport
    // or a manually forced phase. Stop once both observers see the warning.
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::NEG_Z;
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Tremor);
    stop(&mut a);
    let warning = authoritative_worm(&host, id);
    let landing = warning.position;
    assert_eq!(health(&a), before, "a warning cannot deal damage");
    for client in [&a, &b] {
        let replicas = client.world().resource::<client::ReplicatedCreatures>();
        let worm = &replicas.creatures[&id].current;
        assert!(sandworm::warning(worm) > 0.0);
        assert_eq!(worm.position, warning.position);
        assert_eq!(
            hitboxes::for_creature(worm).iter().count(),
            0,
            "sand waves expose neither buried armor nor phantom targets"
        );
    }
    let (sin, cos) = f64::from(warning.yaw).sin_cos();
    // Keep a full metre of motor/reconciliation margin outside the 1.45m
    // underfoot radius while remaining inside the shared closing-mouth volume.
    let victim = [landing[0] - sin * 2.6, landing[1], landing[2] - cos * 2.6];
    let observer = [victim[0] + cos * 0.8, victim[1], victim[2] - sin * 0.8];
    // The combat hole is nearby. Stop on arrival during its warning instead of
    // walking through the victim position while observing the vertical rise.
    arrive_both(&mut host, &mut a, victim, &mut b, observer);
    let arrived_position = position(&a);
    assert!(
        (f64::from(arrived_position[0]) - landing[0])
            .hypot(f64::from(arrived_position[2]) - landing[2])
            > 1.9,
        "the bite fixture must stay outside the distinct underfoot attack: {arrived_position:?}/{landing:?}"
    );
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Emerge);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .get(&id)
                .is_some_and(|track| {
                    track.current.behavior == CreatureBehavior::Emerge
                        && (0.48..0.65).contains(&sandworm::phase_progress(
                            track.current.behavior,
                            track.current.timer,
                        ))
                })
        })
    });
    for client in [&a, &b] {
        let replicas = client.world().resource::<client::ReplicatedCreatures>();
        let worm = &replicas.creatures[&id].current;
        assert_eq!(
            worm.position, warning.position,
            "the close eruption cannot leave its warned hole"
        );
        let boxes = hitboxes::for_creature(worm);
        assert!(
            boxes
                .iter()
                .any(|b| b.center[1] > warning.position[1] + 3.0),
            "the replicated worm genuinely leaves the sand"
        );
        let head = sandworm::pose(worm, sandworm::HEAD_SEGMENT);
        assert!(head.center[0].abs() < 0.0001 && head.center[2].abs() < 0.0001);
        assert!(
            head.pitch > 1.5,
            "the first visible rise points straight up"
        );
        let mouth = sandworm::mouth_position(worm);
        assert!(
            (mouth[0] - warning.position[0]).hypot(mouth[2] - warning.position[2]) < 0.0001,
            "both clients see the same vertical exit at the actual hole"
        );
    }
    assert_landed(&host, [&a, &b], id, landing);
    assert_eq!(
        health(&a),
        before,
        "warning and vertical emergence precede damage"
    );
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Attack);
    for client in [&a, &b] {
        let replicas = client.world().resource::<client::ReplicatedCreatures>();
        let worm = &replicas.creatures[&id].current;
        assert!(
            sandworm::pose(worm, sandworm::HEAD_SEGMENT).center[1] > 6.0,
            "the attack starts upright before its jaws lunge down"
        );
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        health(clients[0]) < before
    });
    assert_eq!(health(&a), before - 6.0);
    assert_eq!(
        health(&b),
        100.0,
        "a creative observer beside the same bite remains immune"
    );
    let mut biting = authoritative_worm(&host, id);
    biting.behavior = CreatureBehavior::Attack;
    biting.timer = sandworm::EXPOSED_SECONDS * (1.0 - sandworm::BITE_PHASE);
    let mouth = sandworm::mouth_position(&biting);
    let creative = position(&b);
    assert!(
        (mouth[0] - f64::from(creative[0])).hypot(mouth[2] - f64::from(creative[2])) < 1.45,
        "creative immunity is tested inside the real bite, not from afar"
    );
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Retreat);
    assert_eq!(health(&a), before - 6.0, "one strike per exposed phase");
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Burrowed);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .get(&id)
                .is_some_and(|track| {
                    track.current.behavior == CreatureBehavior::Burrowed
                        && track.current.timer <= 0.05
                })
        })
    });
    // A new authenticated footfall, after the natural rearm, wakes the next eruption.
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::NEG_Z;
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Tremor);
    stop(&mut a);
    let second_warning = authoritative_worm(&host, id);
    let second_landing = second_warning.position;
    let start = position(&a);
    let dodge = [
        second_landing[0] + 6.0,
        second_landing[1],
        second_landing[2],
    ];
    move_toward(&mut a, dodge);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        arrived(clients[0], dodge)
    });
    stop(&mut a);
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Emerge);
    let second_hole = authoritative_worm(&host, id).position;
    if let Some(goal) = second_warning.needs.goal {
        assert_eq!(goal.kind, NeedKind::AttackAim);
        let aim = goal.position.map(|v| v as f64 + 0.5);
        assert!(
            (second_hole[0] - aim[0]).hypot(second_hole[2] - aim[2]) <= 1.8,
            "a distant passive patrol can choose a warned charge; its line ends at the original aim"
        );
    } else {
        assert_eq!(
            second_hole, second_landing,
            "a close eruption retains the warned hole"
        );
    }
    assert_landed(&host, [&a, &b], id, second_hole);
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Attack);
    let end = position(&a);
    assert!(
        end[0] - start[0] > 4.0,
        "real input must dodge the warned vertical eruption: {start:?} -> {end:?}"
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ReplicatedCreatures>()
                .creatures
                .get(&id)
                .is_some_and(|track| {
                    let worm = &track.current;
                    worm.behavior == CreatureBehavior::Attack
                        && sandworm::phase_progress(worm.behavior, worm.timer)
                            >= sandworm::SPIT_PHASE - 0.10
                })
        })
    });
    // Dodge the actual fixed aim while a real authoritative projectile flies.
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::Z;
    a.world_mut().resource_mut::<Desired>().0.sprint = true;
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Spit);
    assert_eq!(health(&a), before - 6.0, "the second bite was dodged");
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        position(clients[0])[2] - end[2] >= 8.0
    });
    stop(&mut a);
    same_phase(&mut host, &mut a, &mut b, id, CreatureBehavior::Retreat);
    assert_eq!(
        health(&a),
        before - 6.0,
        "dodging the announced eruption and aimed spit avoids more damage"
    );
    assert_eq!(health(&b), 100.0);
    let saved = authoritative_worm(&host, id);
    assert_eq!(
        postcard::from_bytes::<Creature>(&postcard::to_allocvec(&saved).unwrap()).unwrap(),
        saved
    );
    assert!(saved.valid());
}

#[test]
fn real_udp_underfoot_eruption_launches_the_authoritative_motor_and_replicates_the_rise() {
    let mut host = server();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
        config.world.time.paused = true;
    }
    let profile = PlayerProfile::default();
    let body_y = 32.0 + terra_physics::character::PLAYER_HALF_HEIGHT;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in 3..=14 {
            for z in 3..=14 {
                for y in 28..=43 {
                    journal
                        .record([x, y, z], if y <= 31 { 4 } else { 0 })
                        .unwrap();
                }
            }
        }
        journal
            .update_player_location(
                profile.uuid,
                PlayerLocation {
                    position: [8.5, body_y, 8.5],
                    look: [0.0; 2],
                    flying: false,
                },
            )
            .unwrap();
        journal.update_player_stats(
            profile.uuid,
            PlayerStats {
                game_mode: Some(GameMode::Survival),
                survival: SurvivalVitals {
                    health: 80.0,
                    hunger: 60.0,
                    ..default()
                },
                ..default()
            },
        );
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut client = walking_client(browser::join(address, &profile).unwrap());
    client.insert_resource(profile);
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready && session.confirmed_state().is_some_and(|s| s.grounded)
    });
    let before = position(&client);
    let initial_health = health(&client);
    let joined_at = host.world().resource::<Time<Real>>().elapsed_secs();
    wait(&mut host, &mut [&mut client], |host, _| {
        host.world().resource::<Time<Real>>().elapsed_secs() - joined_at >= 2.5
    });
    let id = creature_id([0, 0], 0).unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let population = &mut journal.gameplay_mut().creatures;
        population.animals.clear();
        population.explored.insert([0, 0]);
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::SandWorm,
                home: [0, 0],
                position: [f64::from(before[0]), 32.0, f64::from(before[2])],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::SandWorm.health(),
                behavior: CreatureBehavior::Emerge,
                timer: sandworm::EMERGE_SECONDS,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::SandWorm),
            },
        );
    }
    assert_eq!(authoritative_worm(&host, id).health, 120.0);
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.stats.survival.health < initial_health
            && session
                .confirmed_state()
                .is_some_and(|s| !s.grounded && s.velocity[1] > 6.0)
    });
    assert_eq!(health(&client), initial_health - sandworm::KNOCKUP_DAMAGE);
    wait(&mut host, &mut [&mut client], |_, clients| {
        position(clients[0])[1] >= before[1] + 1.5
    });
    assert_eq!(
        health(&client),
        initial_health - sandworm::KNOCKUP_DAMAGE,
        "one emergence contact emits one launch"
    );
    assert!(
        client
            .world()
            .resource::<client::ReplicatedCreatures>()
            .rejected_snapshots
            == 0
    );
}
