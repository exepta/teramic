use super::*;
use crate::{
    client::{ClientSession, ReplicatedCreatures, SelectedInventorySlot},
    protocol::session::StalkerGrip,
};
use terra_core::{
    GameMode, ItemKind, PlayerProfile,
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds, spawning},
};
use terra_world::{creatures::stalker::Pull, edits::EditJournal};

fn inject(host: &mut App, uuid: terra_core::PlayerUuid, creature: u64, destination: Vec3) {
    server::inject_stalker_grip_for_test(
        host.world_mut(),
        Pull {
            player: uuid,
            creature,
            destination: destination.to_array().map(f64::from),
            remaining_meters: 40.0,
        },
    );
}

fn source(host: &mut App, position: Vec3) -> u64 {
    let mut journal = host.world_mut().resource_mut::<EditJournal>();
    let population = &mut journal.gameplay_mut().creatures;
    let id = spawning::next_command_id(population).unwrap();
    population.animals.insert(
        id,
        Creature {
            id,
            kind: CreatureKind::Stalker,
            home: [0, 0],
            position: (position + Vec3::NEG_X * 3.0 - Vec3::Y * 0.9)
                .to_array()
                .map(f64::from),
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Stalker.health(),
            behavior: CreatureBehavior::Rest,
            timer: 60.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Stalker),
        },
    );
    id
}

#[test]
fn real_udp_grab_only_reaches_its_recipient_and_uses_terrain_collision_then_releases_for_torch() {
    let mut host = server();
    {
        let mut config = host.world_mut().resource_mut::<GameConfig>();
        config.server.game_mode = GameMode::Survival;
        config.server.natural_mob_limit = Some(0);
    }
    let profile = PlayerProfile::default();
    let uuid = profile.uuid;
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .entry(uuid)
        .or_default()
        .insert(ItemKind::Block(40), 1);
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, profile);
    let mut b = profile_client(address, PlayerProfile::default());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|c| {
            let session = c.world().resource::<ClientSession>();
            session.ready && session.confirmed_state().is_some_and(|s| s.grounded)
        })
    });
    let start = Vec3::from_array(
        a.world()
            .resource::<ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    );
    let wall = start.x.floor() as i64 + 2;
    let floor = (start.y - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64;
    let z = start.z.floor() as i64;
    let properties = host
        .world()
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    host.world_mut()
        .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let mut journal = world.resource_mut::<EditJournal>();
            for y in floor..=floor + 6 {
                for dz in -3..=3 {
                    let position = [wall, y, z + dz];
                    let expected_material =
                        chunks.block_at(position).expect("loaded fixture wall cell");
                    if expected_material != 3 {
                        assert!(
                            terra_world::streaming::commit_block_edit(
                                &mut chunks,
                                &mut journal,
                                &terra_world::streaming::BlockEditRequest {
                                    position,
                                    expected_material,
                                    material: 3,
                                },
                                &properties,
                            )
                            .unwrap()
                        );
                    }
                }
            }
        });
    wait(&mut host, &mut [&mut a, &mut b], |host, _| {
        host.world()
            .resource::<terra_physics::TerrainCollisionState>()
            .ready_for_aabb(
                start - Vec3::new(0.5, 1.0, 0.5),
                start + Vec3::new(4.0, 3.0, 0.5),
                host.world().resource::<ChunkStore>(),
                host.world().resource::<EditJournal>(),
            )
    });
    let creature = source(&mut host, start);
    let destination = start + Vec3::X * 6.0;
    let mut received = false;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut frames = 0;
    while frames < 150 || !received {
        assert!(
            Instant::now() < deadline,
            "recipient-local grab snapshot did not arrive"
        );
        inject(&mut host, uuid, creature, destination);
        host.update();
        a.update();
        b.update();
        let a_session = a.world().resource::<ClientSession>();
        received |= a_session
            .active_stalker_grip()
            .is_some_and(|g| g.creature == creature);
        assert!(b.world().resource::<ClientSession>().stalker_grip.is_none());
        assert!(
            !a_session.allow_flight,
            "grab cannot grant flight permission"
        );
        let position = Vec3::from_array(a_session.confirmed_state().unwrap().position);
        assert!(
            position.x < wall as f32 - 0.15,
            "grab crossed a solid wall: {position:?}"
        );
        assert!(
            (position.y - start.y).abs() < 0.15,
            "flat pull must not launch the player"
        );
        frames += 1;
        std::thread::sleep(Duration::from_millis(1));
    }
    let after = a
        .world()
        .resource::<ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    assert!(
        after[0] > start.x + 0.2,
        "the authority must actually drag through physics"
    );
    assert!(
        a.world()
            .resource::<ReplicatedCreatures>()
            .creatures
            .contains_key(&creature)
    );
    a.world_mut().resource_mut::<SelectedInventorySlot>().0 = Some(0);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut consecutive_released = 0;
    while consecutive_released < 30 {
        assert!(
            Instant::now() < deadline,
            "held torch did not release authoritative grab"
        );
        inject(&mut host, uuid, creature, destination);
        host.update();
        a.update();
        b.update();
        if a.world().resource::<ClientSession>().stalker_grip.is_none() {
            consecutive_released += 1;
        } else {
            consecutive_released = 0;
        }
        assert!(b.world().resource::<ClientSession>().stalker_grip.is_none());
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn client_grab_control_expires_and_never_survives_death_or_invulnerable_modes() {
    let grip = StalkerGrip {
        creature: 7,
        destination: [-20.5, 3.9, -15.5],
        remaining_meters: 40.0,
    };
    let mut session = ClientSession::default();
    session.ready = true;
    session.game_mode = GameMode::Survival;
    session.stalker_grip = Some(grip);
    session.stalker_grip_at = Some(Instant::now());
    assert_eq!(session.active_stalker_grip(), Some(grip));
    for mode in [GameMode::Creative, GameMode::Spectator] {
        session.game_mode = mode;
        assert!(session.active_stalker_grip().is_none());
    }
    session.game_mode = GameMode::Survival;
    session.teleporting = true;
    assert!(session.active_stalker_grip().is_none());
    session.teleporting = false;
    session.error = Some("disconnected".into());
    assert!(session.active_stalker_grip().is_none());
    session.error = None;
    session.stats.survival.health = 0.0;
    assert!(session.active_stalker_grip().is_none());
    session.stats.survival.health = 100.0;
    session.stalker_grip_at = Some(Instant::now() - Duration::from_millis(351));
    assert!(session.active_stalker_grip().is_none());
    session.stalker_grip_at = Some(Instant::now());
    session.ready = false;
    assert!(session.active_stalker_grip().is_none());
}
