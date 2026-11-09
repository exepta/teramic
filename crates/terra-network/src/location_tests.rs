use super::*;
use terra_core::PlayerLocation;
use terra_world::edits::EditJournal;

#[test]
fn real_udp_join_restores_each_players_negative_underground_position_and_look() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profiles = [
        terra_core::PlayerProfile::default(),
        terra_core::PlayerProfile::default(),
    ];
    let locations = [
        PlayerLocation {
            position: [-32.5, -28.08, -16.5],
            look: [-1.25, 0.42],
            flying: false,
        },
        PlayerLocation {
            position: [-24.5, -28.08, -16.5],
            look: [2.1, -0.23],
            flying: false,
        },
    ];
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in -36..=-21 {
            for z in -20..=-13 {
                for y in -30..=-25 {
                    journal
                        .record([x, y, z], if y == -30 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        for (profile, location) in profiles.iter().zip(locations) {
            journal
                .update_player_location(profile.uuid, location)
                .unwrap();
        }
    }
    let mut a = profile_client(address, profiles[0].clone());
    let mut b = profile_client(address, profiles[1].clone());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        // Reliable world readiness and the 20 Hz unreliable player snapshot
        // are separate streams; wait for the actual state asserted below.
        clients.iter().all(|c| {
            let session = c.world().resource::<client::ClientSession>();
            session.ready && session.confirmed_state().is_some()
        })
    });
    for (client, location) in [&a, &b].into_iter().zip(locations) {
        let session = client.world().resource::<client::ClientSession>();
        assert_eq!(session.spawn.to_array(), location.position);
        assert_eq!(session.initial_location, Some(location));
        let state = session.confirmed_state().unwrap();
        assert!(
            Vec3::from_array(state.position).distance(Vec3::from_array(location.position)) < 0.15,
            "{state:?}"
        );
        assert_eq!(state.look, location.look);
    }
    for (profile, location) in profiles.iter().zip(locations) {
        let saved = host
            .world()
            .resource::<EditJournal>()
            .player_location(profile.uuid)
            .unwrap();
        assert!(
            Vec3::from_array(saved.position).distance(Vec3::from_array(location.position)) < 0.15
        );
        assert_eq!(saved.look, location.look);
    }
}

#[test]
fn real_udp_join_recovers_a_legacy_save_inside_a_tall_locomotive() {
    use terra_core::{
        train_blueprint,
        trains::{TrainPart, Vehicle, VehicleKind},
    };
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = terra_core::PlayerProfile::default();
    let anchor = [32, 241, 32];
    let guide =
        train_blueprint::building_template_with_width(anchor, 0, VehicleKind::Locomotive, 4, 3);
    let mut car = Vehicle {
        id: 1,
        name: String::new(),
        owner: profile.uuid,
        kind: VehicleKind::Locomotive,
        anchor,
        parts: guide
            .iter()
            .map(|p| TrainPart {
                offset: std::array::from_fn(|i| (p.cell[i] - anchor[i]) as i16),
                material: p.material,
                facing: p.facing,
                wood: Default::default(),
            })
            .collect(),
        cargo: Default::default(),
        blocks: Default::default(),
        payloads: Default::default(),
        motion: Default::default(),
        steam: Default::default(),
        burn_seconds: 0.0,
        couplings: [None; 2],
        revision: 1,
    };
    for index in car.cargo_indices().collect::<Vec<_>>() {
        car.cargo.insert(index, terra_core::Inventory::default());
    }
    let seat = car
        .parts
        .iter()
        .position(|p| p.material == terra_core::train_parts::CAB)
        .unwrap() as u16;
    let (feet, _) = car.seat_pose(usize::from(seat)).unwrap();
    let legacy = PlayerLocation {
        position: [
            feet[0] as f32,
            feet[1] as f32 + terra_physics::character::PLAYER_HALF_HEIGHT,
            feet[2] as f32,
        ],
        look: [0.0; 2],
        flying: false,
    };
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in 25..=40 {
            for z in 25..=43 {
                for y in 238..=253 {
                    journal
                        .record([x, y, z], if y == 238 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        journal.gameplay_mut().trains.insert(car).unwrap();
        journal
            .update_player_location(profile.uuid, legacy)
            .unwrap();
    }
    let mut owner = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut owner], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready
            && session
                .confirmed_state()
                .is_some_and(|state| state.grounded)
            && !session.trains.vehicles.is_empty()
    });
    let session = owner.world().resource::<client::ClientSession>();
    assert_ne!(session.initial_location.unwrap().position, legacy.position);
    assert!(session.train_seat.is_none());
    let restored = session.initial_location.unwrap();
    assert!(terra_physics::character::space_clear_with_shapes(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        host.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        Vec3::from_array(restored.position),
    ));

    // The reliable removal must reach both the owner and an unrelated observer,
    // without turning the formerly assembled parts back into terrain blocks.
    let mut observer = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut owner, &mut observer], |_, clients| {
        clients.iter().all(|app| {
            let session = app.world().resource::<client::ClientSession>();
            session.ready && session.trains.vehicles.iter().any(|car| car.id == 1)
        })
    });
    let before = guide
        .iter()
        .map(|p| {
            (
                p.cell,
                host.world().resource::<ChunkStore>().block_at(p.cell),
            )
        })
        .collect::<Vec<_>>();
    let revision = owner
        .world()
        .resource::<client::ClientSession>()
        .trains
        .vehicles[0]
        .revision;
    let request = owner
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .train_request(crate::protocol::trains::TrainAction::Destroy {
            vehicle: 1,
            revision,
        })
        .unwrap();
    let request_id = request.0.request;
    owner.world_mut().write_message(request);
    wait(&mut host, &mut [&mut owner, &mut observer], |_, clients| {
        clients.iter().all(|app| {
            app.world()
                .resource::<client::ClientSession>()
                .trains
                .vehicles
                .is_empty()
        }) && clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_train_result
            .as_ref()
            .is_some_and(|result| {
                result.request == request_id
                    && result.outcome
                        == crate::protocol::trains::TrainOutcome::Changed { vehicle: None }
            })
    });
    for app in [&owner, &observer] {
        assert_eq!(
            app.world()
                .resource::<client::ClientSession>()
                .trains
                .destroyed,
            vec![1]
        );
    }
    assert!(before.iter().all(|(cell, material)| {
        host.world().resource::<ChunkStore>().block_at(*cell) == *material
    }));
}
