use super::*;
use crate::protocol::trains::{TrainAction, TrainOutcome};
use terra_core::{
    WoodSpecies, train_parts,
    trains::{TrainError, TrainPart, Vehicle, VehicleKind},
};
fn train_fixture() -> (App, Peer, u64, u16, u16) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 6.0, 6.5);
    let mut parts = Vec::new();
    for z in 0..=6 {
        parts.push(TrainPart {
            offset: [0, 1, z],
            material: if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
            facing: 0,
            wood: WoodSpecies::Birch,
        });
    }
    for (offset, material) in [
        ([0, 0, 1], train_parts::BOGIE),
        ([0, 0, 5], train_parts::BOGIE),
        ([0, 1, -1], train_parts::COUPLER),
        ([0, 1, 7], train_parts::COUPLER),
        ([0, 2, 3], train_parts::CAB),
        ([0, 2, 4], train_parts::CARGO),
    ] {
        parts.push(TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
    }
    let seat = parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let cargo = parts
        .iter()
        .position(|part| part.material == train_parts::CARGO)
        .unwrap() as u16;
    let vehicle = Vehicle {
        id: 1,
        owner: peer.uuid,
        kind: VehicleKind::Locomotive,
        anchor: [4, 5, 3],
        parts,
        cargo: std::collections::BTreeMap::from([(
            cargo,
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(3),
                    count: 42,
                }],
                ..Default::default()
            },
        )]),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None, None],
        revision: 0,
    };
    let id = app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .insert(vehicle)
        .unwrap();
    (app, peer, id, seat, cargo)
}
fn run(
    app: &mut App,
    peer: &Peer,
    action: TrainAction,
    seats: &mut crate::server::trains::SeatedPlayers,
) -> Result<TrainOutcome, TrainError> {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    crate::server::trains::apply(action, peer, &mut world, seats)
}
#[test]
fn leave_overflow_keeps_both_owned_seat_binding_and_pose_unchanged() {
    let (mut app, peer, id, seat, _) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    let previous_pose = seats.pose(peer.uuid);
    let binding = seats.binding(peer.uuid);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().revision = u64::MAX;
        journal.gameplay_mut().trains.revision = u64::MAX;
    }
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        run(&mut app, &peer, TrainAction::Leave, &mut seats),
        Err(TrainError::Overflow)
    );
    assert_eq!(seats.pose(peer.uuid), previous_pose);
    assert_eq!(seats.binding(peer.uuid), binding);
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn horse_and_train_seat_transitions_are_exclusive_without_mutating_either_owner() {
    use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
    let (mut app, mut peer, id, seat, _) = train_fixture();
    peer.held_slot = None;
    let horse = Creature {
        id: (1 << 62) + 210_719,
        home: [0, 0],
        kind: CreatureKind::Horse,
        position: [6.5, 4.0, 6.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 20.0,
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Horse),
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(horse.id, horse.clone());
        journal
            .gameplay_mut()
            .creatures
            .owners
            .insert(horse.id, peer.uuid);
    }
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    {
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert!(
            world
                .mounts
                .mount(&horse, peer.uuid, world.journal.gameplay())
        );
        let before = world.journal.gameplay().clone();
        assert_eq!(
            crate::server::trains::apply(
                TrainAction::Seat {
                    vehicle: id,
                    part: seat
                },
                &peer,
                &mut world,
                &mut seats
            ),
            Err(TrainError::Busy)
        );
        assert_eq!(world.journal.gameplay(), &before);
        assert_eq!(world.mounts.active[&peer.uuid].horse, horse);
        assert!(seats.binding(peer.uuid).is_none());
        world.mounts.release(peer.uuid);
        assert!(matches!(
            crate::server::trains::apply(
                TrainAction::Seat {
                    vehicle: id,
                    part: seat
                },
                &peer,
                &mut world,
                &mut seats
            ),
            Ok(TrainOutcome::Seated(Some(_)))
        ));
        let before = world.journal.gameplay().clone();
        let pose = seats.pose(peer.uuid);
        assert!(crate::server::trains::horse_mount_blocked(
            &peer, &world, &seats, horse.id, true
        ));
        assert_eq!(world.journal.gameplay(), &before);
        assert_eq!(seats.pose(peer.uuid), pose);
        assert!(world.mounts.active.is_empty());
        assert_eq!(
            crate::server::trains::apply(TrainAction::Leave, &peer, &mut world, &mut seats),
            Ok(TrainOutcome::Seated(None))
        );
        assert!(seats.has_exit(peer.uuid));
        let before = world.journal.gameplay().clone();
        assert!(crate::server::trains::horse_mount_blocked(
            &peer, &world, &seats, horse.id, true
        ));
        assert_eq!(world.journal.gameplay(), &before);
        assert!(world.mounts.active.is_empty());
    }
}
#[test]
fn cargo_gesture_is_owner_checked_and_stale_retry_cannot_duplicate_the_owned_stack() {
    let (mut app, mut peer, id, _, cargo) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let owner = peer.uuid;
    peer.uuid = PlayerUuid::from_u128(99);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::OpenCargo {
                vehicle: id,
                part: cargo
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
    peer.uuid = owner;
    let car_revision = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .revision;
    let player_revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&owner].revision;
    let action = TrainAction::Cargo {
        vehicle: id,
        part: cargo,
        revision: car_revision,
        player_revision,
        gesture: terra_core::containers::ContainerGesture::Wheel {
            slot: terra_core::INVENTORY_SLOTS as u8,
            to_cursor: false,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    let result = run(&mut app, &peer, action.clone(), &mut seats).unwrap();
    assert!(
        matches!(result,TrainOutcome::Cargo(snapshot) if snapshot.inventory.count(ItemKind::Block(3))==41)
    );
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(before.inventories[&owner].count(ItemKind::Block(3)), 1);
    assert!(before.inventories[&owner].cursor.is_none());
    assert_eq!(
        run(&mut app, &peer, action, &mut seats),
        Err(TrainError::Stale)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}
