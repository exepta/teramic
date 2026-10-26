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
