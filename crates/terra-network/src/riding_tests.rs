use super::*;
use crate::protocol::session::{ClientControl, InputChannel, MovementInput};
use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds, spawning};
use terra_world::edits::EditJournal;

#[test]
fn real_udp_horse_mount_checks_ownership_moves_jumps_and_dismounts() {
    let (mut host, mut a, mut b) = pair();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    let owner = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let start = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let id;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.creatures = Default::default();
        state.animal_bonds.clear();
        state.animal_families.clear();
        state.spider_dens.clear();
        id = spawning::next_command_id(&state.creatures).unwrap();
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                home: [0, 0],
                kind: CreatureKind::Horse,
                position: [
                    f64::from(start[0]),
                    f64::from(start[1] - terra_physics::character::PLAYER_HALF_HEIGHT),
                    f64::from(start[2] - 2.5),
                ],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 60.0,
                age: 0.0,
                variant: 3,
                needs: CreatureNeeds::full(CreatureKind::Horse),
            },
        );
        state.creatures.owners.insert(id, owner);
    }
    std::thread::sleep(Duration::from_millis(460));
    send(
        &mut b,
        ClientControl::Creature {
            id,
            feed: true,
            look: [0.0; 2],
        },
    );
    for _ in 0..12 {
        host.update();
        a.update();
        b.update();
    }
    assert!(
        b.world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .mounted
            .is_none()
    );
    send(
        &mut a,
        ClientControl::Creature {
            id,
            feed: true,
            look: [0.0; 2],
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.mounted.is_some())
    });
    let mounted = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .mounted
        .unwrap();
    assert_eq!(mounted.id, id);
    assert_eq!(mounted.variant, 3);
    let before = mounted.position;
    let world = a.world_mut();
    let mut senders = world.query_filtered::<&mut lightyear::prelude::MessageSender<MovementInput>, With<lightyear::prelude::client::Client>>();
    senders
        .single_mut(world)
        .unwrap()
        .send::<InputChannel>(MovementInput {
            generation: 0,
            sequence: 1,
            direction: [1.0, 0.0, 0.0],
            look: [0.0; 2],
            jump: true,
            sprint: true,
            auto_climb: false,
            flight_speed: None,
        });
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .and_then(|s| s.mounted)
            .is_some_and(|m| m.position[0] > before[0] + 0.2 && m.position[1] > before[1] + 0.2)
    });
    let moving = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .mounted
        .unwrap();
    assert!(moving.stamina < mounted.stamina);
    assert!(moving.valid());
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ReplicatedPlayers>()
            .players
            .values()
            .any(|p| p.current.mounted.is_some_and(|m| m.id == id))
    });
    send(&mut a, ClientControl::Dismount);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.mounted.is_none())
    });
    assert!(host.world().resource::<EditJournal>().gameplay().valid());
}
