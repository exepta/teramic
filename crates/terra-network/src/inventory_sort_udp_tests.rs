//! Real UDP checks use the same inventory and container commands as the UI.
use super::*;
use terra_core::inventory_sort::SortOrder;

fn raw_inventory(host: &mut App, client: &mut App, command: InventoryCommand) -> InventoryOutcome {
    let response = client
        .world()
        .resource::<client::ClientSession>()
        .inventory_response;
    client
        .world_mut()
        .write_message(client::InventoryRequest(command));
    wait(host, &mut [client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory_response
            > response
    });
    client
        .world()
        .resource::<client::ClientSession>()
        .last_inventory
        .unwrap()
}

#[test]
fn alphabetical_and_numeric_id_orders_are_applied_authoritatively_over_udp() {
    let (mut host, mut client, _) = pair();
    let stone = ItemKind::Block(3);
    for (slot, item) in [
        (0, ItemKind::Block(13)),
        (10, stone),
        (12, ItemKind::Stick),
        (14, ItemKind::Apple),
    ] {
        assert_eq!(
            inventory(
                &mut host,
                &mut client,
                InventoryAction::CreativeSet {
                    slot,
                    item,
                    count: 1
                }
            ),
            InventoryOutcome::Applied
        );
    }
    for (order, expected) in [
        (
            SortOrder::Alphabetical,
            [ItemKind::Apple, ItemKind::Stick, stone],
        ),
        (SortOrder::Id, [stone, ItemKind::Apple, ItemKind::Stick]),
    ] {
        assert_eq!(
            inventory(
                &mut host,
                &mut client,
                InventoryAction::Sort {
                    order,
                    reserved: [0; terra_core::INVENTORY_SLOTS],
                }
            ),
            InventoryOutcome::Applied
        );
        let session = client.world().resource::<client::ClientSession>();
        assert_eq!(session.inventory.slot(0).unwrap().item, ItemKind::Block(13));
        for (offset, item) in expected.into_iter().enumerate() {
            assert_eq!(
                session.inventory.slot(9 + offset),
                Some(ItemStack { item, count: 1 })
            );
        }
    }
}

#[test]
fn backpack_sort_over_udp_preserves_hotbar_reservations_and_rejects_stale_or_cursor_commands() {
    let (mut host, mut a, _) = pair();
    let stack_limit = ItemKind::Block(3).max_stack();
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .occupied_slots(),
        0
    );
    for (slot, item, count) in [
        (0, ItemKind::Block(3), 2),
        (10, ItemKind::Block(3), stack_limit - 8),
        (12, ItemKind::Block(3), 14),
        (14, ItemKind::Block(4), 3),
    ] {
        assert_eq!(
            inventory(
                &mut host,
                &mut a,
                InventoryAction::CreativeSet { slot, item, count }
            ),
            InventoryOutcome::Applied
        );
    }
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    let mut reserved = [0; terra_core::INVENTORY_SLOTS];
    reserved[14] = 1;
    let command = InventoryCommand {
        revision: before.revision,
        action: InventoryAction::Sort {
            order: SortOrder::Category,
            reserved,
        },
    };
    assert_eq!(
        raw_inventory(&mut host, &mut a, command),
        InventoryOutcome::Applied
    );
    let sorted = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    assert_eq!(sorted.slot(0), before.slot(0));
    assert_eq!(sorted.slot(14), before.slot(14));
    assert_eq!(
        sorted.slot(9),
        Some(ItemStack {
            item: ItemKind::Block(3),
            count: stack_limit
        })
    );
    assert_eq!(
        sorted.slot(10),
        Some(ItemStack {
            item: ItemKind::Block(3),
            count: 6
        })
    );
    assert_eq!(sorted.count(ItemKind::Block(3)), stack_limit + 8);
    assert_eq!(
        raw_inventory(&mut host, &mut a, command),
        InventoryOutcome::Conflict
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        sorted
    );
    reserved[14] = 4;
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::Sort {
                order: SortOrder::Quantity,
                reserved
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::SortContainer {
                order: SortOrder::Category
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeCursor {
                item: ItemKind::Block(3),
                count: 1
            }
        ),
        InventoryOutcome::Applied
    );
    let held = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::Sort {
                order: SortOrder::Quantity,
                reserved: [0; terra_core::INVENTORY_SLOTS]
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        held
    );
}

#[test]
fn double_chest_sort_over_udp_is_shared_atomic_and_checks_revisions_and_access() {
    let (mut host, mut a, mut b) = pair();
    let [anchor, other] = fixture(&mut host, &a, true);
    let stack_limit = ItemKind::Block(3).max_stack();
    seed(&mut host, anchor, stack_limit - 8);
    seed(&mut host, other, 14);
    for client in [&mut a, &mut b] {
        assert_eq!(
            inventory(
                &mut host,
                client,
                InventoryAction::OpenContainer { position: anchor }
            ),
            InventoryOutcome::Applied
        );
        assert!(
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .as_ref()
                .unwrap()
                .sortable()
        );
    }
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    let stale = gesture(
        &b,
        ContainerGesture::Sort {
            order: SortOrder::Quantity,
        },
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Sort {
            order: SortOrder::Category,
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .as_ref()
                .is_some_and(|snapshot| {
                    snapshot.halves[0]
                        .inventory
                        .slot(0)
                        .is_some_and(|stack| stack.count == stack_limit)
                        && snapshot.halves[0]
                            .inventory
                            .slot(1)
                            .is_some_and(|stack| stack.count == 6)
                })
        })
    });
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(
        journal
            .container_at(anchor)
            .unwrap()
            .count(ItemKind::Block(3)),
        stack_limit + 6
    );
    assert_eq!(
        journal
            .container_at(other)
            .unwrap()
            .count(ItemKind::Block(3)),
        0
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        before
    );
    assert_eq!(
        transfer(&mut host, &mut b, stale),
        InventoryOutcome::Conflict
    );
    let unauthorized = gesture(
        &a,
        ContainerGesture::Sort {
            order: SortOrder::Quantity,
        },
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::CloseContainer),
        InventoryOutcome::Applied
    );
    assert_eq!(
        transfer(&mut host, &mut a, unauthorized),
        InventoryOutcome::ContainerUnavailable
    );
}

#[test]
fn machine_sort_is_rejected_by_authority_without_moving_process_slots() {
    let (mut host, mut a, _) = pair();
    let [anchor, _] = furnace_fixture(&mut host, &a);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    assert!(
        !a.world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .unwrap()
            .sortable()
    );
    let before = host
        .world()
        .resource::<EditJournal>()
        .container_at(anchor)
        .cloned();
    let command = gesture(
        &a,
        ContainerGesture::Sort {
            order: SortOrder::Category,
        },
    );
    assert_eq!(
        transfer(&mut host, &mut a, command),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(anchor)
            .cloned(),
        before
    );
}
