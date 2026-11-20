use super::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_world::{
    TerraWorldPlugin, WorldMode,
    streaming::{ChunkStore, WorldAccess},
};

#[path = "bee_tests.rs"]
mod bee_tests;
#[path = "building_templates_udp_tests.rs"]
mod building_templates_udp_tests;
#[path = "chest_placement_udp_tests.rs"]
mod chest_placement_udp_tests;
#[path = "kingdom_tests.rs"]
mod kingdom_tests;
#[path = "location_tests.rs"]
mod location_tests;
#[path = "movement_prediction_tests.rs"]
mod movement_prediction_tests;
#[path = "npc_work_udp_tests.rs"]
mod npc_work_udp_tests;
#[path = "pet_command_tests.rs"]
mod pet_command_tests;
#[path = "pvp_udp_tests.rs"]
mod pvp_udp_tests;
#[path = "riding_tests.rs"]
mod riding_tests;
#[path = "tree_felling_tests.rs"]
mod tree_felling_tests;

fn base(mode: WorldMode, role: connection::NetworkRole) -> App {
    let mut config = GameConfig::default();
    config.server.game_mode = terra_core::GameMode::Creative;
    config.client.view_distance = 1;
    config.client.vertical_view_distance = 1;
    config.world.landscape_revision = 3;
    config.world.sea_level = 8.0;
    config.world.biomes = false;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.caves = false;
    config.world.trees = false;
    config.world.water = false;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(config))
        .add_plugins((
            TerraWorldPlugin { mode },
            terra_physics::TerraPhysicsPlugin,
            TerraNetworkPlugin(role),
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    app
}
fn server() -> App {
    server_bound(([127, 0, 0, 1], 0).into())
}
fn server_bound(bind: std::net::SocketAddr) -> App {
    let mut app = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    app.insert_resource(server::ServerEndpoint {
        bind,
        advertised: None,
    });
    app.finish();
    app.cleanup();
    app.update();
    let error = &app.world().resource::<server::ServerRuntime>().error;
    assert!(error.is_none(), "server startup failed: {error:?}");
    app
}
fn client(token: Vec<u8>) -> App {
    let mut app = base(WorldMode::Client, connection::NetworkRole::Client);
    app.insert_resource(client::ClientLaunch { token });
    app.finish();
    app.cleanup();
    app
}

fn profile_client(address: std::net::SocketAddr, profile: terra_core::PlayerProfile) -> App {
    let token = browser::join(address, &profile).unwrap();
    let mut app = client(token);
    app.insert_resource(profile);
    app
}

#[test]
fn world_clock_late_join_pause_resume_and_midnight_are_authoritative() {
    use terra_core::WorldClock;
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .day_length_seconds = 60.0;
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    *host.world_mut().resource_mut::<WorldClock>() = WorldClock {
        day: 42,
        phase: 0.99,
        day_length_seconds: 60.0,
        paused: true,
    };
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    let saved = *host.world().resource::<WorldClock>();
    assert_eq!(*a.world().resource::<WorldClock>(), saved);
    let mut b = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1].world().resource::<client::ClientSession>().ready
    });
    assert_eq!(*b.world().resource::<WorldClock>(), saved);
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = false;
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        host.world().resource::<WorldClock>().day == 43
            && clients
                .iter()
                .all(|c| c.world().resource::<WorldClock>().day == 43)
    });
    for c in [&a, &b] {
        let actual = c.world().resource::<WorldClock>();
        let expected = host.world().resource::<WorldClock>();
        assert!((actual.phase - expected.phase).abs() < 0.01);
        assert!(!actual.paused);
    }
    assert_eq!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .clock,
        Some(*host.world().resource::<WorldClock>())
    );
}

#[test]
fn weather_is_authoritative_for_two_clients_late_join_and_graphics_settings() {
    use terra_core::WorldWeather;
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = true;
    let mut rain = WorldWeather::new(42042);
    rain.from_intensity = 0.72;
    rain.target_intensity = 0.72;
    rain.wetness = 0.61;
    rain.elapsed_seconds = 30.0;
    rain.elapsed_total = 30.0;
    *host.world_mut().resource_mut::<WorldWeather>() = rain;
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    assert_eq!(*a.world().resource::<WorldWeather>(), rain);
    a.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .weather_intensity = 0.0;
    let mut b = profile_client(address, terra_core::PlayerProfile::default());
    b.world_mut().resource_mut::<GameConfig>().client.weather = terra_core::WeatherKind::Snow;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1].world().resource::<client::ClientSession>().ready
    });
    assert_eq!(*b.world().resource::<WorldWeather>(), rain);
    assert_eq!(*a.world().resource::<WorldWeather>(), rain);
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .paused = false;
    host.world_mut()
        .resource_mut::<GameConfig>()
        .world
        .time
        .day_length_seconds = 60.0;
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        let expected = host.world().resource::<WorldWeather>();
        expected.elapsed_total > 55.0
            && clients.iter().all(|client| {
                let actual = client.world().resource::<WorldWeather>();
                (actual.elapsed_total - expected.elapsed_total).abs() < 1.5
            })
    });
    assert_eq!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .weather,
        Some(*host.world().resource::<WorldWeather>())
    );
    assert!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .surface_condition_count()
            <= 65_536
    );
}

#[test]
fn unavailable_bootstrap_or_account_verifier_exits_without_missing_resource_panics() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    for (address, verified, expected) in [
        (occupied.local_addr().unwrap(), false, "Cannot bind"),
        ("127.0.0.1:0".parse().unwrap(), true, "account verifier"),
    ] {
        let mut app = base(
            WorldMode::DedicatedServer,
            connection::NetworkRole::DedicatedServer,
        );
        app.world_mut()
            .resource_mut::<GameConfig>()
            .server
            .require_verified_accounts = verified;
        app.insert_resource(server::ServerEndpoint {
            bind: address,
            advertised: None,
        });
        app.finish();
        app.cleanup();
        app.update();
        assert!(
            app.world()
                .resource::<server::ServerRuntime>()
                .error
                .as_ref()
                .unwrap()
                .contains(expected)
        );
        assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
    }
}

#[test]
fn address_join_preserves_profile_uuid_and_reports_real_population() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    assert_eq!(browser::query_status(address).unwrap().players, 0);
    let profile = terra_core::PlayerProfile {
        gender: terra_core::PlayerGender::Female,
        appearance: terra_core::CharacterAppearance {
            hairstyle: 9,
            hair_color: [230, 190, 60],
            shirt_color: [30, 150, 85],
            pants_color: [40, 55, 90],
            ..Default::default()
        },
        ..default()
    };
    let mut a = profile_client(address, profile.clone());
    let mut b = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|app| app.world().resource::<client::ClientSession>().ready)
    });
    assert_eq!(
        a.world().resource::<client::ClientSession>().uuid,
        Some(profile.uuid)
    );
    assert_ne!(
        a.world().resource::<client::ClientSession>().uuid,
        b.world().resource::<client::ClientSession>().uuid
    );
    assert_eq!(browser::query_status(address).unwrap().players, 2);
    let female = a
        .world()
        .resource::<client::ClientSession>()
        .player
        .unwrap();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ReplicatedPlayers>()
            .players
            .get(&female)
            .is_some_and(|p| {
                p.current.gender == profile.gender && p.current.appearance == profile.appearance
            })
    });
    let old_player = a.world().resource::<client::ClientSession>().player;
    let link = a
        .world()
        .resource::<client::ClientSession>()
        .connection
        .unwrap();
    a.world_mut()
        .trigger(lightyear::prelude::Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while host.world().resource::<server::ConnectedProfiles>().0.len() != 1 {
        host.update();
        a.update();
        b.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(browser::query_status(address).unwrap().players, 1);
    let mut rejoined = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut rejoined, &mut b], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    let session = rejoined.world().resource::<client::ClientSession>();
    assert_eq!(session.uuid, Some(profile.uuid));
    assert_ne!(session.player, old_player);
    assert_eq!(browser::query_status(address).unwrap().players, 2);
    let returned = session.player.unwrap();
    wait(&mut host, &mut [&mut rejoined, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ReplicatedPlayers>()
            .players
            .get(&returned)
            .is_some_and(|p| p.current.appearance == profile.appearance)
    });
}

#[test]
fn address_join_rejects_duplicate_and_substituted_profile_uuid() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = terra_core::PlayerProfile::default();
    let mut good = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut good], |_, clients| {
        clients[0].world().resource::<client::ClientSession>().ready
    });
    for (claimed, expected) in [
        (profile.clone(), "already connected"),
        (terra_core::PlayerProfile::default(), "does not match"),
    ] {
        let token = browser::join(address, &profile).unwrap();
        let mut bad = client(token);
        bad.insert_resource(claimed);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            host.update();
            good.update();
            bad.update();
            if let Some(error) = &bad.world().resource::<client::ClientSession>().error {
                assert!(error.contains(expected), "{error}");
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(good.world().resource::<client::ClientSession>().ready);
        assert_eq!(
            host.world().resource::<server::ConnectedProfiles>().0.len(),
            1
        );
    }
}

#[test]
fn two_lan_browsers_discover_multiple_servers_without_duplicate_entries() {
    let host = server();
    let other = server_bound(([127, 0, 0, 2], 0).into());
    let addresses = [&host, &other].map(|app| {
        app.world()
            .resource::<server::ServerRuntime>()
            .address
            .unwrap()
    });
    let mut a = discovery::LanScanner::new().unwrap();
    let mut b = discovery::LanScanner::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        a.poll().unwrap();
        b.poll().unwrap();
        if [&a, &b].iter().all(|scanner| {
            addresses
                .iter()
                .all(|address| scanner.servers.contains_key(address))
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "LAN results: {:?} / {:?}, wanted {addresses:?}",
            a.servers.keys(),
            b.servers.keys()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    for scanner in [&a, &b] {
        for address in addresses {
            let record = &scanner.servers[&address];
            assert_eq!(record.status.players, 0);
            assert_eq!(
                browser::query_status(record.address).unwrap().instance,
                record.status.instance
            );
            assert_eq!(
                scanner
                    .servers
                    .values()
                    .filter(|server| server.status.instance == record.status.instance)
                    .count(),
                1
            );
        }
    }
}

#[derive(Resource, Default)]
struct Desired(terra_physics::character::CharacterInput);
fn prediction_driver(
    mut session: ResMut<client::ClientSession>,
    simulation: terra_physics::character::CharacterSimulation,
    desired: Res<Desired>,
    mut bodies: Query<
        (
            Entity,
            &mut Transform,
            &mut avian3d::prelude::LinearVelocity,
            &avian3d::prelude::Collider,
            &mut terra_physics::character::CharacterGrounded,
        ),
        With<terra_physics::character::CharacterBody>,
    >,
) {
    for (entity, mut transform, mut velocity, collider, mut grounded) in &mut bodies {
        let mut state = terra_physics::character::CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        session.predict(entity, collider, &mut state, desired.0, &simulation);
        transform.translation = state.position;
        velocity.0 = state.velocity;
        grounded.0 = state.grounded;
    }
}
fn avatar_driver(
    mut commands: Commands,
    session: Res<client::ClientSession>,
    bodies: Query<&Transform, With<terra_physics::character::CharacterBody>>,
    mut focus: ResMut<terra_world::streaming::ChunkFocus>,
) {
    if !session.ready {
        return;
    }
    if let Some(body) = bodies.iter().next() {
        focus.0 = body.translation.to_array().map(f64::from);
    } else if let Some(state) = session.confirmed_state() {
        commands.spawn(terra_physics::character::body_bundle(Vec3::from_array(
            state.position,
        )));
    }
}
fn walking_client(token: Vec<u8>) -> App {
    let mut app = base(WorldMode::Client, connection::NetworkRole::Client);
    app.insert_resource(client::ClientLaunch { token })
        .init_resource::<Desired>()
        .add_systems(
            Update,
            avatar_driver.before(terra_world::streaming::StreamingSet::Interest),
        )
        .add_systems(
            FixedUpdate,
            prediction_driver.after(avian3d::prelude::PhysicsSystems::Writeback),
        );
    app.finish();
    app.cleanup();
    app
}
fn wait(server: &mut App, clients: &mut [&mut App], condition: impl Fn(&App, &[&mut App]) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        server.update();
        for client in clients.iter_mut() {
            client.update();
            assert!(
                client
                    .world()
                    .resource::<client::ClientSession>()
                    .error
                    .is_none(),
                "{:?}",
                client.world().resource::<client::ClientSession>().error
            );
        }
        if condition(server, clients) {
            return;
        }
        assert!(Instant::now() < deadline, "network condition timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn udp_welcome_preserves_custom_biome_snapshots_and_absent_legacy_snapshots() {
    for landscape_revision in [3, 4] {
        let mut host = server();
        let mut expected = host.world().resource::<GameConfig>().world.clone();
        expected.landscape_revision = landscape_revision;
        if landscape_revision == 3 {
            expected.sea_level = 8.0;
            expected.biome_definitions = None;
        } else {
            expected.sea_level = 22.0;
            expected.biome_definitions = Some(terra_core::BiomeDefinitions::default());
            let definitions = expected.biome_definitions.as_mut().unwrap();
            definitions.entries[terra_core::BiomeKind::Plains as usize]
                .terrain
                .max_height = 58.0;
            definitions.entries[terra_core::BiomeKind::Forest as usize]
                .terrain
                .tree_density = 0.44;
            definitions.entries[terra_core::BiomeKind::Ocean as usize]
                .ocean
                .as_mut()
                .unwrap()
                .warm
                .coral_density = 0.61;
        }
        host.world_mut().resource_mut::<GameConfig>().world = expected.clone();
        host.world_mut()
            .resource_mut::<terra_world::streaming::WorldTerrain>()
            .0 = terra_world::TerraGenerator::new(expected.clone()).unwrap();
        let token = host
            .world()
            .resource::<server::ServerRuntime>()
            .issue_token(301)
            .unwrap();
        let mut a = client(token);
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0].world().resource::<client::ClientSession>().ready
        });
        let token = host
            .world()
            .resource::<server::ServerRuntime>()
            .issue_token(302)
            .unwrap();
        let mut b = client(token);
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients
                .iter()
                .all(|client| client.world().resource::<client::ClientSession>().ready)
        });
        for replica in [&a, &b] {
            let received = &replica.world().resource::<GameConfig>().world;
            assert_eq!(received, &expected);
            assert!(received.same_terrain(&expected));
            assert_eq!(received.biome_definitions, expected.biome_definitions);
            assert_eq!(received.sea_level, expected.sea_level);
            assert_eq!(
                replica
                    .world()
                    .resource::<terra_world::streaming::WorldTerrain>()
                    .0
                    .config(),
                &expected
            );
        }
    }
}

#[test]
fn udp_handshake_two_clients_and_confirmed_deltas_use_one_authority() {
    let mut server = server();
    let runtime = server.world().resource::<server::ServerRuntime>();
    let mut a = client(runtime.issue_token(101).unwrap());
    let mut b = client(runtime.issue_token(102).unwrap());
    // A client's local world file must not select a different multiplayer world.
    let mut local = a.world().resource::<GameConfig>().world.clone();
    local.seed.0 = 73;
    local.hill_amplitude = 28.0;
    a.world_mut().resource_mut::<GameConfig>().world = local.clone();
    a.world_mut()
        .resource_mut::<terra_world::streaming::WorldTerrain>()
        .0 = terra_world::TerraGenerator::new(local).unwrap();
    wait(&mut server, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|c| c.world().resource::<client::ClientSession>().ready)
    });
    assert_ne!(
        a.world().resource::<client::ClientSession>().player,
        b.world().resource::<client::ClientSession>().player
    );
    assert_eq!(
        a.world().resource::<GameConfig>().world,
        server.world().resource::<GameConfig>().world
    );
    assert!(matches!(
        *a.world().resource::<WorldAccess>(),
        WorldAccess::Replica { ready: true }
    ));
    wait(&mut server, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|c| !c.world().resource::<ChunkStore>().loaded.is_empty())
    });
    let y = (0..32)
        .rev()
        .find(|&y| {
            server
                .world()
                .resource::<ChunkStore>()
                .block_at([8, y, 24])
                .is_some_and(|id| id != 0)
        })
        .unwrap();
    let position = [8, y, 24];
    let expected = server
        .world()
        .resource::<ChunkStore>()
        .block_at(position)
        .unwrap();
    assert_ne!(expected, 0);
    server.world_mut().resource_scope(
        |world, mut journal: Mut<terra_world::edits::EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            assert!(
                terra_world::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &terra_world::streaming::BlockEditRequest {
                        position,
                        expected_material: expected,
                        material: 0,
                    },
                    &Default::default(),
                )
                .unwrap()
            );
        },
    );
    wait(&mut server, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|c| c.world().resource::<ChunkStore>().block_at(position) == Some(0))
    });
    assert_eq!(
        a.world()
            .resource::<terra_world::edits::EditJournal>()
            .edited_blocks(),
        1
    );
    assert_eq!(
        b.world()
            .resource::<terra_world::edits::EditJournal>()
            .edited_blocks(),
        1
    );
    let mut late = client(
        server
            .world()
            .resource::<server::ServerRuntime>()
            .issue_token(103)
            .unwrap(),
    );
    wait(
        &mut server,
        &mut [&mut a, &mut b, &mut late],
        |_, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && clients[2]
                    .world()
                    .resource::<ChunkStore>()
                    .block_at(position)
                    == Some(0)
        },
    );
}

#[test]
fn fluid_only_revisions_replicate_to_two_players_and_late_join_without_terrain_rebuilds() {
    use terra_world::{edits::EditJournal, water::FluidCell};
    let (mut host, mut a, mut b) = pair();
    let position = [8, 17, 24];
    let owner = terra_voxel::global_to_section(position).0;
    let old_mesh = a.world().resource::<EditJournal>().mesh_epoch(owner);
    let first = FluidCell {
        amount: 6,
        flow: [1, 0],
        source: false,
    };
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(position, first)
        .unwrap();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|client| client.world().resource::<EditJournal>().fluid(position) == Some(first))
    });
    let second = FluidCell {
        amount: 3,
        flow: [0, -1],
        source: false,
    };
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(position, second)
        .unwrap();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients
            .iter()
            .all(|client| client.world().resource::<EditJournal>().fluid(position) == Some(second))
    });
    assert_eq!(
        a.world().resource::<EditJournal>().mesh_epoch(owner),
        old_mesh
    );
    let mut late = client(
        host.world()
            .resource::<server::ServerRuntime>()
            .issue_token(203)
            .unwrap(),
    );
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients[2].world().resource::<client::ClientSession>().ready
            && clients[2].world().resource::<EditJournal>().fluid(position) == Some(second)
    });
    assert_eq!(late.world().resource::<EditJournal>().fluid_epoch(owner), 2);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(position, FluidCell::default())
        .unwrap();
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients.iter().all(|client| {
            let journal = client.world().resource::<EditJournal>();
            journal.fluid_epoch(owner) == 3 && journal.fluid(position).is_none()
        })
    });
}

fn send(app: &mut App, message: protocol::session::ClientControl) {
    use lightyear::prelude::*;
    let world = app.world_mut();
    let mut query=world.query_filtered::<&mut MessageSender<protocol::session::ClientControl>,With<client::Client>>();
    query
        .single_mut(world)
        .unwrap()
        .send::<protocol::session::ControlChannel>(message);
}

fn pair() -> (App, App, App) {
    let mut host = server();
    let runtime = host.world().resource::<server::ServerRuntime>();
    let mut a = client(runtime.issue_token(201).unwrap());
    let mut b = client(runtime.issue_token(202).unwrap());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world().resource::<client::ClientSession>().ready
                && c.world()
                    .resource::<client::ClientSession>()
                    .confirmed_state()
                    .is_some_and(|state| state.grounded)
        })
    });
    (host, a, b)
}

#[path = "chat_tests.rs"]
mod chat_tests;
#[path = "consumption_tests.rs"]
mod consumption_tests;
#[path = "container_tests.rs"]
mod container_tests;
#[path = "creature_tests.rs"]
mod creature_tests;
#[path = "effect_tests.rs"]
mod effect_tests;
#[path = "equipment_tests.rs"]
mod equipment_tests;
#[path = "gameplay_tests.rs"]
mod gameplay_tests;
#[path = "inventory_tests.rs"]
mod inventory_tests;
#[path = "sandworm_udp_tests.rs"]
mod sandworm_udp_tests;
#[path = "spawn_tests.rs"]
mod spawn_tests;
#[path = "stalker_bridge_tests.rs"]
mod stalker_bridge_tests;
#[path = "survival_tests.rs"]
mod survival_tests;

#[test]
fn competing_commands_commit_once_and_invalid_edits_never_mutate_replicas() {
    use protocol::session::*;
    use terra_world::streaming::{BlockEditRequest, commit_block_edit};
    let (mut host, mut a, mut b) = pair();
    let confirmed = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye = Vec3::from_array(confirmed.position)
        + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let y = (eye.y.floor() as i64) - 2;
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<terra_world::edits::EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for z in 21..=26 {
                for x in 5..=10 {
                    let position = [x, y, z];
                    let expected = chunks.block_at(position).unwrap();
                    if expected != 3 {
                        commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &BlockEditRequest {
                                position,
                                expected_material: expected,
                                material: 3,
                            },
                            &Default::default(),
                        )
                        .unwrap();
                    }
                }
            }
        });
    wait(&mut host, &mut [&mut a, &mut b], |host, clients| {
        clients.iter().all(|c| {
            c.world().resource::<ChunkStore>().block_at([8, y, 23])
                == host.world().resource::<ChunkStore>().block_at([8, y, 23])
                && c.world()
                    .resource::<terra_world::edits::EditJournal>()
                    .edited_blocks()
                    == 36
        })
    });
    let look = [0.0, -1.5];
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (block, target) = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    assert!(matches!(
        target,
        Some(terra_world::targeting::WorldTarget::Block(_))
    ));
    let position = block.unwrap().position;
    let owner = terra_voxel::global_to_section(position).0;
    let revision = a
        .world()
        .resource::<terra_world::edits::EditJournal>()
        .revision(owner);
    for client in [&mut a, &mut b] {
        let command = {
            let mut session = client.world_mut().resource_mut::<client::ClientSession>();
            session.look = look;
            session
                .edit_command(
                    revision,
                    EditAction::Block {
                        position,
                        expected: 3,
                        material: 0,
                    },
                )
                .unwrap()
        };
        send(client, ClientControl::Edit(command));
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some()
        })
    });
    let results = [
        a.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        b.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
    ];
    assert!(
        results.contains(&EditOutcome::Applied) && results.contains(&EditOutcome::Conflict),
        "{results:?}"
    );
    assert_eq!(
        host.world()
            .resource::<terra_world::edits::EditJournal>()
            .revision(owner),
        revision + 1
    );
    let command = a
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .edit_command(
            0,
            EditAction::Block {
                position: [5000, 15, 5000],
                expected: 1,
                material: 0,
            },
        )
        .unwrap();
    send(&mut a, ClientControl::Edit(command));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some_and(|(id, _)| id == 2)
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::OutOfReach
    );
    let position = [15, y, 31];
    let before = a
        .world()
        .resource::<ChunkStore>()
        .block_at(position)
        .unwrap();
    a.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: before,
        material: 0,
    });
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some_and(|(id, _)| id == 3)
    });
    assert_eq!(
        a.world().resource::<ChunkStore>().block_at(position),
        Some(before)
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(before)
    );
}

#[test]
fn authoritative_walk_and_prediction_keep_two_distant_streaming_regions_alive() {
    let mut host = server();
    let runtime = host.world().resource::<server::ServerRuntime>();
    let mut a = walking_client(runtime.issue_token(301).unwrap());
    let mut b = walking_client(runtime.issue_token(302).unwrap());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ClientSession>()
                .confirmed_state()
                .is_some_and(|s| s.grounded)
        })
    });
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::NEG_X;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.position[0] < -80.0)
    });
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::ZERO;
    for _ in 0..60 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    for c in [&a, &b] {
        let state = c
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap();
        assert!(
            state.grounded && state.position[1] > 16.7 && state.position[1] < 17.1,
            "{state:?}"
        );
        let owner = terra_voxel::global_to_section(state.position.map(|v| v.floor() as i64)).0;
        assert!(host.world().resource::<ChunkStore>().is_wanted(owner));
        assert!(
            host.world()
                .resource::<ChunkStore>()
                .loaded
                .contains_key(&owner)
        );
    }
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let body = a
        .world_mut()
        .query_filtered::<&Transform, With<terra_physics::character::CharacterBody>>()
        .single(a.world())
        .unwrap()
        .translation;
    assert!(
        body.distance(Vec3::from_array(state.position)) < 0.25,
        "prediction diverged: {body:?}, {state:?}"
    );
    assert!(a.world().resource::<client::ClientSession>().pending_inputs < 16);
    assert_eq!(
        host.world()
            .resource::<server::ServerRuntime>()
            .connected_players,
        2
    );
}

#[test]
fn one_prop_removal_reaches_both_clients_and_late_join_without_removing_neighbors() {
    use protocol::session::*;
    let (mut host, mut a, mut b) = pair();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<terra_world::streaming::WorldGeneration>()
                .status
                == terra_world::streaming::GenerationStatus::Ready
        })
    });
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let mut candidates: Vec<_> = host
        .world()
        .resource::<ChunkStore>()
        .loaded
        .values()
        .flat_map(|s| s.props.iter())
        .filter(|p| !p.removed)
        .copied()
        .collect();
    candidates.sort_by(|a, b| {
        let distance = |p: &terra_world::props::WorldProp| {
            Vec3::from_array(p.position.map(|v| v as f32)).distance_squared(eye)
        };
        distance(a).total_cmp(&distance(b))
    });
    let (prop,look)=candidates.into_iter().find_map(|prop| {
        let target=Vec3::from_array(prop.position.map(|v|v as f32))+Vec3::Y*0.2;
        let rotation=Transform::from_translation(eye).looking_at(target,Vec3::Y).rotation;
        let direction=rotation*Vec3::NEG_Z;
        let (_,hit)=terra_world::targeting::pick_world(host.world().resource::<ChunkStore>(),eye.to_array().map(f64::from),direction.to_array().map(f64::from));
        let (yaw,pitch,_)=rotation.to_euler(EulerRot::YXZ);
        matches!(hit,Some(terra_world::targeting::WorldTarget::Prop(hit)) if hit.prop.id==prop.id).then_some((prop,[yaw,pitch]))
    }).expect("reachable natural prop");
    let alive = host.world().resource::<ChunkStore>().loaded[&prop.section]
        .props
        .iter()
        .filter(|p| !p.removed)
        .count();
    let revision = a
        .world()
        .resource::<terra_world::edits::EditJournal>()
        .prop_epoch(prop.section);
    let command = {
        let mut session = a.world_mut().resource_mut::<client::ClientSession>();
        session.look = look;
        session
            .edit_command(
                revision,
                EditAction::Prop {
                    section: prop.section,
                    id: prop.id,
                },
            )
            .unwrap()
    };
    send(&mut a, ClientControl::Edit(command));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<terra_world::edits::EditJournal>()
                .prop_removed(prop.section, prop.id)
        })
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().loaded[&prop.section]
            .props
            .iter()
            .filter(|p| !p.removed)
            .count(),
        alive - 1
    );
    let mut late = client(
        host.world()
            .resource::<server::ServerRuntime>()
            .issue_token(401)
            .unwrap(),
    );
    wait(&mut host, &mut [&mut a, &mut b, &mut late], |_, clients| {
        clients[2].world().resource::<client::ClientSession>().ready
            && clients[2]
                .world()
                .resource::<terra_world::edits::EditJournal>()
                .prop_removed(prop.section, prop.id)
    });
    assert_eq!(
        late.world()
            .resource::<terra_world::edits::EditJournal>()
            .removed_props(),
        1
    );
}

#[test]
fn delayed_input_burst_is_simulated_once_per_fixed_tick_before_each_acknowledgment() {
    use lightyear::prelude::MessageSender;
    use protocol::session::*;
    let (mut host, mut a, mut b) = pair();
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let tick = host.world().resource::<server::ServerRuntime>().tick;
    let world = a.world_mut();
    let mut senders=world.query_filtered::<&mut MessageSender<MovementInput>,With<lightyear::prelude::client::Client>>();
    let mut sender = senders.single_mut(world).unwrap();
    for sequence in 1..=100 {
        sender.send::<InputChannel>(MovementInput {
            generation: 0,
            sequence,
            direction: [1.0, 0.0, 0.0],
            look: [0.0; 2],
            jump: false,
            sprint: false,
            auto_climb: false,
            flight_speed: None,
        });
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.ack == 100)
    });
    let after = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let ticks = host.world().resource::<server::ServerRuntime>().tick - tick;
    assert!(
        ticks >= 100,
        "acknowledging 100 inputs requires 100 actual steps: {ticks}"
    );
    assert!(
        after.position[0] - before.position[0]
            <= ticks as f32 * terra_physics::character::WALK_SPEED / 60.0 + 0.01
    );
}

#[test]
fn real_udp_server_keeps_a_jump_unacknowledged_until_collision_is_ready() {
    use lightyear::prelude::MessageSender;
    use protocol::session::*;
    use terra_world::streaming::ChunkStore;
    let (mut host, mut a, mut b) = pair();
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|state| state.grounded)
    });
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let position = terra_voxel::global_to_section(before.position.map(|v| v.floor() as i64)).0;
    let epoch = host.world().resource::<ChunkStore>().loaded[&position].mesh_epoch;
    host.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&position)
        .unwrap()
        .mesh_epoch = epoch.wrapping_add(1000);
    let world = a.world_mut();
    world
        .query_filtered::<&mut MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>()
        .single_mut(world)
        .unwrap()
        .send::<InputChannel>(MovementInput {
            generation: 0,
            sequence: 1,
            direction: [0.0; 3],
            look: [0.0; 2],
            jump: true,
            sprint: false,
            auto_climb: false,
            flight_speed: None,
        });
    for _ in 0..12 {
        host.update();
        a.update();
        b.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    let blocked = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    assert_eq!(
        blocked.ack, before.ack,
        "an unsimulated jump must not be acknowledged"
    );
    assert!((blocked.position[1] - before.position[1]).abs() < 0.01);
    host.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&position)
        .unwrap()
        .mesh_epoch = epoch;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|state| state.ack == 1 && state.position[1] > before.position[1] + 0.1)
    });
}

#[test]
fn reconnect_discards_old_replica_and_reimports_the_authoritative_journal() {
    use lightyear::prelude::{Connected, Disconnect};
    use terra_world::edits::EditJournal;
    let (mut host, mut a, mut b) = pair();
    let previous = a.world().resource::<client::ClientSession>().player;
    let link = a
        .world()
        .resource::<client::ClientSession>()
        .connection
        .unwrap();
    a.world_mut().trigger(Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while a.world().get::<Connected>(link).is_some()
        || a.world()
            .resource::<client::ClientSession>()
            .error
            .is_none()
    {
        host.update();
        a.update();
        b.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    a.world_mut()
        .resource_mut::<EditJournal>()
        .record([-1000, 0, -1000], 3)
        .unwrap();
    host.world_mut()
        .resource_mut::<EditJournal>()
        .record([8, 15, 24], 0)
        .unwrap();
    a.world_mut().write_message(client::ReconnectRequest);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready && session.player != previous
    });
    let journal = a.world().resource::<EditJournal>();
    assert_eq!(journal.edited_blocks(), 1);
    assert_eq!(
        journal
            .snapshot(terra_voxel::SectionPos::new(0, 0, 1))
            .unwrap()
            .get(&[8, 15, 24]),
        Some(&0)
    );
    assert_eq!(
        journal.revision(terra_voxel::SectionPos::new(-63, 0, -63)),
        0
    );
}

#[test]
fn reliable_authority_recovers_from_latency_jitter_loss_and_client_pause() {
    use lightyear::prelude::{Link, LinkConditionerConfig, RecvLinkConditioner};
    let mut host = server();
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(601)
        .unwrap();
    let mut a = walking_client(token);
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.grounded)
    });
    let conditioner = LinkConditionerConfig::default()
        .with_incoming_latency(Duration::from_millis(40))
        .with_incoming_jitter(Duration::from_millis(8))
        .with_fixed_loss(0.05);
    for mut link in host
        .world_mut()
        .query::<&mut Link>()
        .iter_mut(host.world_mut())
    {
        link.recv.conditioner = Some(RecvLinkConditioner::new(conditioner.clone()));
    }
    for mut link in a.world_mut().query::<&mut Link>().iter_mut(a.world_mut()) {
        link.recv.conditioner = Some(RecvLinkConditioner::new(conditioner.clone()));
    }
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::X;
    for _ in 0..120 {
        host.update();
        a.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    // The client window stops updating, but the dedicated authority keeps ticking.
    for _ in 0..60 {
        host.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    a.world_mut().resource_mut::<Desired>().0.direction = Vec3::ZERO;
    wait(&mut host, &mut [&mut a], |_, clients| {
        let s = clients[0].world().resource::<client::ClientSession>();
        s.confirmed_state()
            .is_some_and(|p| p.ack > 180 && p.grounded)
            && s.pending_inputs < 50
    });
}

#[test]
fn invalid_movement_is_disconnected_without_affecting_the_other_player() {
    use lightyear::prelude::MessageSender;
    use protocol::session::*;
    let (mut host, mut a, mut b) = pair();
    let world = a.world_mut();
    world.query_filtered::<&mut MessageSender<MovementInput>,With<lightyear::prelude::client::Client>>()
        .single_mut(world).unwrap().send::<InputChannel>(MovementInput {
            generation:0,sequence:1,direction:[f32::NAN,0.0,0.0],look:[0.0;2],jump:false,sprint:false,auto_climb:false,flight_speed:None,
        });
    let deadline = Instant::now() + Duration::from_secs(5);
    while a
        .world()
        .resource::<client::ClientSession>()
        .error
        .is_none()
    {
        host.update();
        a.update();
        b.update();
        assert!(
            Instant::now() < deadline,
            "invalid input must disconnect promptly"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    wait(&mut host, &mut [&mut b], |host, _| {
        host.world()
            .resource::<server::ServerRuntime>()
            .connected_players
            == 1
    });
    assert!(b.world().resource::<client::ClientSession>().ready);
}

#[test]
fn movement_packets_preserve_auto_climb_for_authority_and_prediction_replay() {
    for auto_climb in [false, true] {
        let packet = protocol::session::MovementInput {
            generation: 0,
            sequence: 1,
            direction: [1.0, 0.0, 0.0],
            look: [0.0; 2],
            jump: false,
            sprint: false,
            auto_climb,
            flight_speed: None,
        };
        assert!(packet.valid());
        assert_eq!(packet.character().auto_climb, auto_climb);
    }
}

#[test]
fn environment_tools_require_server_permission_and_replicate_confirmed_conditions() {
    use protocol::session::EditOutcome;
    use terra_world::{
        edits::EditJournal,
        environment::{SurfaceCondition, SurfaceConditionRequest},
    };
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let granted = terra_core::PlayerProfile::default();
    let restricted = terra_core::PlayerProfile::default();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.player_permissions.insert(
            granted.uuid,
            terra_core::PlayerPermissions {
                game_mode: false,
                build: true,
                flight: false,
                environment: true,
            },
        );
        config.server.player_permissions.insert(
            restricted.uuid,
            terra_core::PlayerPermissions {
                game_mode: false,
                build: false,
                flight: false,
                environment: false,
            },
        );
    }
    let mut a = profile_client(address, granted);
    let mut b = profile_client(address, restricted);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<client::ClientSession>()
                .confirmed_state()
                .is_some_and(|state| state.grounded)
        })
    });
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .allow_environment
    );
    assert!(!a.world().resource::<client::ClientSession>().allow_flight);
    assert!(!b.world().resource::<client::ClientSession>().allow_build);
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let look = [0.0, -1.4];
    let rotation = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0);
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let (block, _) = terra_world::targeting::pick_world(
        host.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        (rotation * Vec3::NEG_Z).to_array().map(f64::from),
    );
    let position = block.unwrap().position;
    let request = SurfaceConditionRequest {
        position,
        condition: SurfaceCondition::new(0.9, 0.2).unwrap(),
    };
    b.world_mut().resource_mut::<client::ClientSession>().look = look;
    b.world_mut().write_message(request);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some()
    });
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .surface_condition(position),
        SurfaceCondition::default()
    );
    a.world_mut().resource_mut::<client::ClientSession>().look = look;
    a.world_mut().write_message(request);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            c.world()
                .resource::<EditJournal>()
                .surface_condition(position)
                == request.condition
        })
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied
    );
    let section = terra_voxel::global_to_section(position).0;
    assert_eq!(a.world().resource::<EditJournal>().mesh_epoch(section), 0);
    b.world_mut()
        .resource_mut::<client::ClientSession>()
        .last_edit = None;
    b.world_mut()
        .write_message(terra_world::streaming::BlockEditRequest {
            position,
            expected_material: host
                .world()
                .resource::<ChunkStore>()
                .block_at(position)
                .unwrap(),
            material: 0,
        });
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some()
    });
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Invalid
    );
    assert_ne!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(0)
    );
}

#[test]
#[ignore = "180-second real UDP soak; run explicitly with --ignored --nocapture"]
fn sustained_two_profile_udp_session_keeps_identity_and_replication() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = terra_core::PlayerProfile::default();
    let mut a = profile_client(address, profile.clone());
    let mut b = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            let session = c.world().resource::<client::ClientSession>();
            session.ready && session.confirmed_state().is_some()
        })
    });
    let started = Instant::now();
    let mut ticks = 0_u64;
    let mut snapshots = [0_u64; 2];
    let mut last_ticks = [0; 2];
    let mut last_progress = [Instant::now(); 2];
    while started.elapsed() < Duration::from_secs(180) {
        host.update();
        for (index, app) in [&mut a, &mut b].into_iter().enumerate() {
            app.update();
            let session = app.world().resource::<client::ClientSession>();
            assert!(
                session.ready && session.error.is_none(),
                "{:?}",
                session.error
            );
            assert!(session.confirmed_state().is_some());
            let tick = app.world().resource::<client::ReplicatedPlayers>().tick;
            if tick > last_ticks[index] {
                snapshots[index] += 1;
                last_ticks[index] = tick;
                last_progress[index] = Instant::now();
            }
            assert!(
                last_progress[index].elapsed() < Duration::from_secs(5),
                "Snapshot stream stalled"
            );
        }
        assert_eq!(
            host.world().resource::<server::ConnectedProfiles>().0.len(),
            2
        );
        ticks += 1;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        a.world().resource::<client::ClientSession>().uuid,
        Some(profile.uuid)
    );
    assert_eq!(browser::query_status(address).unwrap().players, 2);
    println!(
        "UDP soak: elapsed={:?}, ticks={ticks}, client observations={snapshots:?}",
        started.elapsed()
    );
}

#[path = "railway_connection_tests.rs"]
mod railway_connection_tests;
#[path = "railway_tests.rs"]
mod railway_tests;
#[path = "sapling_tests.rs"]
mod sapling_tests;
