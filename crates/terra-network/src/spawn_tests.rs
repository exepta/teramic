use super::*;
use terra_world::{
    edits::EditJournal,
    spawn::{WorldSpawn, find_land_spawn},
    streaming::{BlockEditRequest, ChunkFocus, WorldTerrain},
};

fn expected_position(spawn: WorldSpawn) -> Vec3 {
    Vec3::from_array(spawn.feet().map(|value| value as f32))
        + Vec3::Y * (terra_physics::character::PLAYER_HALF_HEIGHT + 0.02)
}

#[test]
fn impossible_land_spawn_exits_without_starting_listener_or_missing_resource_panic() {
    let mut host = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    let mut config = host.world().resource::<GameConfig>().world.clone();
    config.water = true;
    config.sea_level = config.base_height + 5.0;
    host.insert_resource(WorldTerrain(
        terra_world::TerraGenerator::new(config.clone()).unwrap(),
    ));
    host.world_mut().resource_mut::<GameConfig>().world = config;
    host.insert_resource(server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    host.finish();
    host.cleanup();
    host.update();
    let runtime = host.world().resource::<server::ServerRuntime>();
    assert!(
        runtime
            .error
            .as_ref()
            .unwrap()
            .contains("safe dry-land spawn")
    );
    assert!(runtime.address.is_none());
    assert!(runtime.entity.is_none());
    assert!(!host.world().contains_resource::<WorldSpawn>());
    assert!(!host.world().resource::<Messages<AppExit>>().is_empty());
    host.update();
    assert!(!host.world().contains_resource::<WorldSpawn>());
}

#[test]
fn spawn_invalidated_after_connection_is_rejected_before_welcome() {
    let mut host = server();
    host.add_observer(
        |_: On<Add, terra_physics::character::CharacterBody>,
         spawn: Res<WorldSpawn>,
         mut journal: ResMut<EditJournal>| {
            journal.record(spawn.ground, 0).unwrap();
        },
    );
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut player = profile_client(address, terra_core::PlayerProfile::default());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        host.update();
        player.update();
        let session = player.world().resource::<client::ClientSession>();
        assert!(!session.ready);
        if let Some(error) = &session.error {
            assert!(error.contains("world spawn changed"), "{error}");
            assert!(session.player.is_none(), "Unsafe Welcome must not be sent");
            break;
        }
        assert!(Instant::now() < deadline, "Spawn rejection timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        host.world()
            .resource::<server::ConnectedProfiles>()
            .0
            .is_empty()
    );
}

#[test]
fn ocean_origin_sends_same_supported_land_spawn_to_both_udp_clients() {
    let mut host = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    let world = terra_core::WorldConfig {
        seed: terra_core::WorldSeed(1),
        ..Default::default()
    };
    let terrain = terra_world::TerraGenerator::new(world.clone()).unwrap();
    let sea = terrain.water_column(8.5, 24.5).unwrap();
    assert!(sea.level - sea.bed > 20.0);
    host.world_mut().resource_mut::<GameConfig>().world = world;
    host.insert_resource(WorldTerrain(terrain));
    host.insert_resource(server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    host.finish();
    host.cleanup();
    host.update();
    let spawn = *host.world().resource::<WorldSpawn>();
    let terrain = &host.world().resource::<WorldTerrain>().0;
    assert!(spawn.is_valid(terrain, host.world().resource::<EditJournal>()));
    assert!(
        terrain
            .water_column(spawn.feet()[0], spawn.feet()[2])
            .is_none()
    );
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut first = profile_client(address, terra_core::PlayerProfile::default());
    let mut second = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut first, &mut second], |_, players| {
        players
            .iter()
            .all(|player| player.world().resource::<client::ClientSession>().ready)
    });
    for player in [&first, &second] {
        assert_eq!(
            player.world().resource::<client::ClientSession>().spawn,
            expected_position(spawn)
        );
        assert_eq!(
            player.world().resource::<ChunkFocus>().0,
            expected_position(spawn).to_array().map(f64::from)
        );
    }
    assert!(
        host.world()
            .resource::<ChunkStore>()
            .block_at(spawn.ground)
            .and_then(terra_voxel::MaterialId::from_raw)
            .is_some_and(terra_voxel::MaterialId::is_natural_ground)
    );
}

#[test]
fn authoritative_spawn_honors_journal_injected_after_plugin_build() {
    let mut host = base(
        WorldMode::DedicatedServer,
        connection::NetworkRole::DedicatedServer,
    );
    let original = find_land_spawn(
        &host.world().resource::<WorldTerrain>().0,
        host.world().resource::<EditJournal>(),
        [8, 24],
    )
    .unwrap();
    let mut saved = EditJournal::default();
    saved.record(original.ground, 0).unwrap();
    host.insert_resource(saved);
    host.insert_resource(server::ServerEndpoint {
        bind: ([127, 0, 0, 1], 0).into(),
        advertised: None,
    });
    host.finish();
    host.cleanup();
    host.update();
    let spawn = *host.world().resource::<WorldSpawn>();
    assert_ne!(spawn, original);
    assert!(spawn.is_valid(
        &host.world().resource::<WorldTerrain>().0,
        host.world().resource::<EditJournal>(),
    ));
    assert_eq!(
        host.world().resource::<ChunkFocus>().0,
        expected_position(spawn).to_array().map(f64::from),
    );
    let mut player = profile_client(
        host.world()
            .resource::<server::ServerRuntime>()
            .address
            .unwrap(),
        terra_core::PlayerProfile::default(),
    );
    wait(&mut host, &mut [&mut player], |_, players| {
        players[0].world().resource::<client::ClientSession>().ready
    });
    let session = player.world().resource::<client::ClientSession>();
    assert_eq!(session.spawn, expected_position(spawn));
    assert!(
        host.world()
            .resource::<ChunkStore>()
            .block_at(spawn.ground)
            .is_some()
    );
}

#[test]
fn joining_after_spawn_support_was_removed_selects_new_dry_ground() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let original = *host.world().resource::<WorldSpawn>();
    let mut first = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut first], |_, players| {
        players[0].world().resource::<client::ClientSession>().ready
    });
    let material = host
        .world()
        .resource::<ChunkStore>()
        .block_at(original.ground)
        .unwrap();
    host.world_mut().write_message(BlockEditRequest {
        position: original.ground,
        expected_material: material,
        material: 0,
    });
    wait(&mut host, &mut [&mut first], |host, _| {
        host.world()
            .resource::<EditJournal>()
            .block(original.ground)
            == Some(0)
    });
    let mut second = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut first, &mut second], |_, players| {
        players[1].world().resource::<client::ClientSession>().ready
    });
    let replacement = *host.world().resource::<WorldSpawn>();
    assert_ne!(replacement, original);
    assert!(replacement.is_valid(
        &host.world().resource::<WorldTerrain>().0,
        host.world().resource::<EditJournal>(),
    ));
    assert_eq!(
        second.world().resource::<client::ClientSession>().spawn,
        expected_position(replacement)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .block(original.ground),
        Some(0)
    );
}
