use super::*;
use terra_core::{containers::ContainerGesture, inventory_sort::SortOrder};

#[test]
fn train_storage_and_backpack_sort_use_existing_owner_revision_and_atomic_cargo_path() {
    let (mut app, peer, id, _, cargo) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let stone = ItemKind::Block(3);
    let dirt = ItemKind::Block(4);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        let bag = game.inventories.entry(peer.uuid).or_default();
        bag.slots = vec![ItemStack::EMPTY; terra_core::INVENTORY_SLOTS];
        bag.slots[0] = ItemStack {
            item: stone,
            count: 1,
        };
        bag.slots[12] = ItemStack {
            item: dirt,
            count: 5,
        };
        bag.slots[14] = ItemStack {
            item: stone,
            count: 4,
        };
        bag.cursor = None;
        game.trains
            .vehicles
            .get_mut(&id)
            .unwrap()
            .cargo
            .get_mut(&cargo)
            .unwrap()
            .slots = vec![
            ItemStack {
                item: dirt,
                count: 2,
            },
            ItemStack {
                item: stone,
                count: 5,
            },
            ItemStack {
                item: dirt,
                count: 4,
            },
        ];
    }
    let before = app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].clone();
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let command = TrainAction::Cargo {
        vehicle: id,
        part: cargo,
        revision: car.revision,
        player_revision: before.revision,
        gesture: ContainerGesture::Sort {
            order: SortOrder::Category,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    assert!(matches!(
        run(&mut app, &peer, command.clone(), &mut seats),
        Ok(TrainOutcome::Cargo(_))
    ));
    let journal = app.world().resource::<EditJournal>();
    let stored = &journal.gameplay().trains.vehicles[&id].cargo[&cargo];
    assert_eq!(
        stored.slot(0),
        Some(ItemStack {
            item: stone,
            count: 5
        })
    );
    assert_eq!(
        stored.slot(1),
        Some(ItemStack {
            item: dirt,
            count: 6
        })
    );
    assert_eq!(journal.gameplay().inventories[&peer.uuid], before);
    let sorted = stored.clone();
    assert_eq!(
        run(&mut app, &peer, command, &mut seats),
        Err(TrainError::Stale)
    );

    let revision = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .revision;
    let mut reserved = [0; terra_core::INVENTORY_SLOTS];
    reserved[12] = 1;
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Cargo {
                vehicle: id,
                part: cargo,
                revision,
                player_revision: before.revision,
                gesture: ContainerGesture::SortBackpack {
                    order: SortOrder::Category
                },
                reserved,
            },
            &mut seats
        ),
        Ok(TrainOutcome::Cargo(_))
    ));
    let journal = app.world().resource::<EditJournal>();
    let bag = &journal.gameplay().inventories[&peer.uuid];
    assert_eq!(bag.slot(0), before.slot(0));
    assert_eq!(bag.slot(12), before.slot(12));
    assert_eq!(
        bag.slot(9),
        Some(ItemStack {
            item: stone,
            count: 4
        })
    );
    assert_eq!(
        journal.gameplay().trains.vehicles[&id].cargo[&cargo],
        sorted
    );

    let player_revision = bag.revision;
    let revision = journal.gameplay().trains.vehicles[&id].revision;
    let mut intruder = crate::server::survival::tests::resting_peer(100.0);
    intruder.body = peer.body;
    assert_eq!(
        run(
            &mut app,
            &intruder,
            TrainAction::Cargo {
                vehicle: id,
                part: cargo,
                revision,
                player_revision,
                gesture: ContainerGesture::Sort {
                    order: SortOrder::Quantity
                },
                reserved,
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
}
