use super::*;
use crate::protocol::inventory::{
    ContainerCommand, InventoryAction, InventoryCommand, InventoryOutcome,
};
use terra_core::{
    Inventory, ItemKind, ItemStack,
    containers::{CHEST_SLOTS, ContainerGesture},
};
use terra_world::edits::{BlockObjectState, EditJournal};

fn inventory(host: &mut App, client: &mut App, action: InventoryAction) -> InventoryOutcome {
    let session = client.world().resource::<client::ClientSession>();
    let revision = session.inventory.revision;
    let response = session.inventory_response;
    client
        .world_mut()
        .write_message(client::InventoryRequest(InventoryCommand {
            revision,
            action,
        }));
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

fn gesture(client: &App, action: ContainerGesture) -> ContainerCommand {
    let session = client.world().resource::<client::ClientSession>();
    let snapshot = session.container.as_ref().unwrap();
    ContainerCommand {
        inventory_revision: session.inventory.revision,
        anchor: snapshot.anchor,
        reserved: [0; terra_core::INVENTORY_SLOTS],
        revisions: snapshot
            .halves
            .iter()
            .map(|half| half.inventory.revision)
            .collect(),
        action,
    }
}

fn transfer(host: &mut App, client: &mut App, command: ContainerCommand) -> InventoryOutcome {
    let response = client
        .world()
        .resource::<client::ClientSession>()
        .inventory_response;
    client
        .world_mut()
        .write_message(client::ContainerRequest(command));
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

fn act(host: &mut App, client: &mut App, action: ContainerGesture) {
    let command = gesture(client, action);
    assert_eq!(transfer(host, client, command), InventoryOutcome::Applied);
}

#[test]
fn shared_chest_cursors_use_authoritative_items_and_only_reach_other_viewers() {
    use crate::protocol::social::ChestCursorInput;
    let (mut host, mut a, mut b) = pair();
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 0,
                item: ItemKind::Block(40),
                count: 1,
            }
        ),
        InventoryOutcome::Applied
    );
    let player = a
        .world()
        .resource::<client::ClientSession>()
        .player
        .unwrap();
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ReplicatedPlayers>()
            .players
            .get(&player)
            .is_some_and(|track| {
                track.current.held_torch
                    && track.current.held_item == Some(terra_core::ItemKind::Block(40))
            })
            && clients[1]
                .world()
                .resource::<client::SocialState>()
                .names
                .contains_key(&player)
    });
    assert!(
        b.world()
            .resource::<client::SocialState>()
            .names
            .contains_key(&player)
    );
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = None;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ReplicatedPlayers>()
            .players
            .get(&player)
            .is_some_and(|track| !track.current.held_torch && track.current.held_item.is_none())
    });
    let [anchor, _] = fixture(&mut host, &a, false);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    let cursor = ChestCursorInput {
        anchor,
        position: [12000, 32000],
        color: [220, 68, 145],
    };
    a.world_mut().resource_mut::<client::CursorShare>().0 = Some(cursor);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeCursor {
                item: ItemKind::Block(3),
                count: 7
            }
        ),
        InventoryOutcome::Applied
    );
    for _ in 0..30 {
        host.update();
        a.update();
        b.update();
    }
    assert!(
        b.world()
            .resource::<client::SocialState>()
            .cursors
            .is_empty()
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut b,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::SocialState>()
            .cursors
            .iter()
            .any(|c| {
                c.cursor == cursor
                    && c.item
                        .is_some_and(|s| s.item == ItemKind::Block(3) && s.count == 7)
            })
    });
    assert_eq!(
        inventory(&mut host, &mut b, InventoryAction::CloseContainer),
        InventoryOutcome::Applied
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::SocialState>()
            .cursors
            .is_empty()
    });
    // A forged anchor cannot leak a cursor into another chest session.
    a.world_mut().resource_mut::<client::CursorShare>().0 = Some(ChestCursorInput {
        anchor: [999, 1, 999],
        ..cursor
    });
    for _ in 0..30 {
        host.update();
        a.update();
        b.update();
    }
    assert!(
        b.world()
            .resource::<client::SocialState>()
            .cursors
            .is_empty()
    );
}

fn put(host: &mut App, position: [i64; 3], material: u16) {
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let properties = world
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
                .clone();
            let mut chunks = world.resource_mut::<ChunkStore>();
            let expected_material = chunks
                .block_at(position)
                .expect("loaded chest fixture corridor");
            if expected_material != material {
                terra_world::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &terra_world::streaming::BlockEditRequest {
                        position,
                        expected_material,
                        material,
                    },
                    &properties,
                )
                .unwrap();
            }
        });
}

fn fixture(host: &mut App, client: &App, double: bool) -> [[i64; 3]; 2] {
    let body = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let anchor = [
        body[0].floor() as i64,
        (body[1] + 0.5).floor() as i64,
        body[2].floor() as i64 - 2,
    ];
    let other = [anchor[0] + 1, anchor[1], anchor[2]];
    // Keep physics ground unchanged, but remove incidental eye-height terrain.
    for x in anchor[0] - 2..=anchor[0] + 3 {
        for y in anchor[1]..=anchor[1] + 3 {
            for z in anchor[2] - 1..=anchor[2] + 2 {
                put(host, [x, y, z], 0);
            }
        }
    }
    for cell in [anchor, other].into_iter().take(if double { 2 } else { 1 }) {
        put(host, [cell[0], cell[1] - 1, cell[2]], 3);
        put(host, cell, 29);
    }
    if double {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for (cell, peer, secondary) in [(anchor, other, false), (other, anchor, true)] {
            journal
                .set_object_state(
                    cell,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..default()
                    }),
                )
                .unwrap();
        }
    }
    [anchor, other]
}

fn seed(host: &mut App, position: [i64; 3], count: u16) {
    let value = Inventory {
        slots: vec![ItemStack {
            item: ItemKind::Block(3),
            count,
        }],
        ..default()
    };
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_container(position, value)
        .unwrap();
}

fn furnace_fixture(host: &mut App, client: &App) -> [[i64; 3]; 2] {
    let [anchor, _] = fixture(host, client, false);
    let upper = [anchor[0], anchor[1] + 1, anchor[2]];
    for (position, peer, secondary) in [(anchor, upper, false), (upper, anchor, true)] {
        put(host, position, 59);
        host.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )
            .unwrap();
    }
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_furnace(anchor, default())
        .unwrap();
    [anchor, upper]
}

#[test]
fn furnace_upper_half_opens_shared_state_and_autofill_invalidates_stale_transfers() {
    use terra_core::{MineralItem, furnace};
    let (mut host, mut a, mut b) = pair();
    let [anchor, upper] = furnace_fixture(&mut host, &a);
    let recipe = furnace::recipe_for(ItemKind::Mineral(MineralItem::RawIron)).unwrap();
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 0,
                item: recipe.input,
                count: 8,
            }
        ),
        InventoryOutcome::Applied
    );
    for (client, position) in [(&mut a, upper), (&mut b, anchor)] {
        assert_eq!(
            inventory(
                &mut host,
                client,
                InventoryAction::OpenContainer { position }
            ),
            InventoryOutcome::Applied
        );
        let snapshot = client
            .world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .unwrap();
        assert_eq!(snapshot.anchor, anchor);
        assert!(snapshot.furnace.is_some() && snapshot.workbench.is_none());
        assert_eq!(snapshot.halves.len(), 1);
    }
    let stale = gesture(
        &b,
        ContainerGesture::QuickMove {
            slot: terra_core::INVENTORY_SLOTS as u8,
        },
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::LoadSmeltingRecipe {
                recipe: recipe.id,
                station: anchor
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(total(&host, recipe.input), 8);
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(
        journal.container_at(anchor).unwrap().slot(0).unwrap().count,
        1
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(recipe.input),
        7
    );
    assert_eq!(
        transfer(&mut host, &mut b, stale),
        InventoryOutcome::Conflict
    );
    let stale_player = gesture(&a, ContainerGesture::QuickMove { slot: 0 });
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 5,
                item: ItemKind::Stick,
                count: 1
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        transfer(&mut host, &mut a, stale_player),
        InventoryOutcome::Conflict
    );
    assert_eq!(total(&host, recipe.input), 8);
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
                        .is_some_and(|stack| stack.count == 1)
                })
        })
    });
    act(
        &mut host,
        &mut b,
        ContainerGesture::QuickMove {
            slot: terra_core::INVENTORY_SLOTS as u8,
        },
    );
    assert_eq!(total(&host, recipe.input), 8);
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(recipe.input),
        1
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .as_ref()
                .is_some_and(|snapshot| snapshot.halves[0].inventory.slot(0).is_none())
        })
    });
    put(&mut host, upper, 0);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .is_none()
        })
    });
}

#[test]
fn furnace_slot_filters_reject_invalid_items_and_output_is_extract_only() {
    use terra_core::{MineralItem, furnace};
    let (mut host, mut a, mut b) = pair();
    let [anchor, _] = furnace_fixture(&mut host, &a);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    for (slot, item) in [
        (furnace::INPUT, ItemKind::Stick),
        (furnace::FUEL, ItemKind::Mineral(MineralItem::RawIron)),
        (furnace::OUTPUT, ItemKind::Mineral(MineralItem::IronIngot)),
        (furnace::BELLOWS, ItemKind::Block(3)),
    ] {
        assert_eq!(
            inventory(
                &mut host,
                &mut a,
                InventoryAction::CreativeCursor { item, count: 1 }
            ),
            InventoryOutcome::Applied
        );
        let before = a
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .clone();
        let command = gesture(
            &a,
            ContainerGesture::Click {
                slot: (terra_core::INVENTORY_SLOTS + slot) as u8,
                secondary: false,
            },
        );
        assert_eq!(
            transfer(&mut host, &mut a, command),
            InventoryOutcome::Invalid
        );
        assert_eq!(
            a.world().resource::<client::ClientSession>().inventory,
            before
        );
        assert!(
            host.world()
                .resource::<EditJournal>()
                .container_at(anchor)
                .is_none_or(|bag| bag.occupied_slots() == 0)
        );
        assert_eq!(
            inventory(&mut host, &mut a, InventoryAction::ReturnCursor),
            InventoryOutcome::Applied
        );
    }
    let bellows = ItemKind::Bellows(terra_core::BellowsKind::Ruby);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeCursor {
                item: bellows,
                count: 2
            }
        ),
        InventoryOutcome::Applied
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: (terra_core::INVENTORY_SLOTS + furnace::BELLOWS) as u8,
            secondary: false,
        },
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor,
        Some(ItemStack {
            item: bellows,
            count: 1
        })
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(anchor)
            .unwrap()
            .slot(furnace::BELLOWS),
        Some(ItemStack {
            item: bellows,
            count: 1
        })
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::ReturnCursor),
        InventoryOutcome::Applied
    );
    let output = ItemKind::Mineral(MineralItem::IronIngot);
    let mut contents = host
        .world()
        .resource::<EditJournal>()
        .container_at(anchor)
        .unwrap()
        .clone();
    assert!(contents.creative_set(
        furnace::OUTPUT,
        ItemStack {
            item: output,
            count: 3
        }
    ));
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_container(anchor, contents)
        .unwrap();
    assert_eq!(
        inventory(
            &mut host,
            &mut b,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.halves[0]
                    .inventory
                    .slot(furnace::OUTPUT)
                    .is_some_and(|stack| stack.count == 3)
            })
    });
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: (terra_core::INVENTORY_SLOTS + furnace::OUTPUT) as u8,
            secondary: false,
        },
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor,
        Some(ItemStack {
            item: output,
            count: 3
        })
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .is_some_and(|snapshot| snapshot.halves[0].inventory.slot(furnace::OUTPUT).is_none())
    });
}

#[test]
fn furnace_recipe_loading_requires_current_station_known_recipe_and_compatible_input() {
    use terra_core::{MineralItem, furnace};
    let (mut host, mut a, _) = pair();
    let [anchor, _] = furnace_fixture(&mut host, &a);
    let recipe = furnace::recipe_for(ItemKind::Mineral(MineralItem::RawIron)).unwrap();
    let load = InventoryAction::LoadSmeltingRecipe {
        recipe: recipe.id,
        station: anchor,
    };
    assert_eq!(
        inventory(&mut host, &mut a, load),
        InventoryOutcome::WrongStation
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::LoadSmeltingRecipe {
                recipe: u64::MAX,
                station: anchor
            }
        ),
        InventoryOutcome::UnknownRecipe
    );
    assert_eq!(
        inventory(&mut host, &mut a, load),
        InventoryOutcome::MissingIngredients
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 0,
                item: recipe.input,
                count: 3
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::LoadSmeltingRecipe {
                recipe: recipe.id,
                station: [anchor[0] + 1, anchor[1], anchor[2]]
            }
        ),
        InventoryOutcome::WrongStation
    );
    let mut contents = Inventory::default();
    assert!(contents.creative_set(
        furnace::INPUT,
        ItemStack {
            item: ItemKind::Mineral(MineralItem::RawGold),
            count: 1
        }
    ));
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_container(anchor, contents.clone())
        .unwrap();
    assert_eq!(
        inventory(&mut host, &mut a, load),
        InventoryOutcome::NoSpace
    );
    assert_eq!(
        host.world().resource::<EditJournal>().container_at(anchor),
        Some(&contents)
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(recipe.input),
        3
    );
}

fn total(host: &App, item: ItemKind) -> u32 {
    let journal = host.world().resource::<EditJournal>();
    journal
        .gameplay()
        .inventories
        .values()
        .map(|inventory| {
            u32::from(inventory.count(item))
                + inventory
                    .cursor
                    .filter(|stack| stack.item == item)
                    .map_or(0, |stack| u32::from(stack.count))
        })
        .sum::<u32>()
        + journal
            .containers()
            .map(|(_, inventory)| u32::from(inventory.count(item)))
            .sum::<u32>()
        + journal
            .gameplay()
            .drops
            .iter()
            .filter(|drop| drop.stack.item == item)
            .map(|drop| u32::from(drop.stack.count))
            .sum::<u32>()
}

#[test]
fn workbench_links_oldest_adjacent_chest_and_keeps_crafting_reservations_atomic() {
    let (mut host, mut a, mut b) = pair();
    let [bench, second] = fixture(&mut host, &a, true);
    for (position, peer, secondary) in [(bench, second, false), (second, bench, true)] {
        put(&mut host, position, 28);
        host.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )
            .unwrap();
    }
    let oldest = [bench[0] - 1, bench[1], bench[2]];
    let newer = [second[0] + 1, second[1], second[2]];
    for cell in [oldest, newer] {
        put(&mut host, [cell[0], cell[1] - 1, cell[2]], 3);
        put(&mut host, cell, 29);
    }
    seed(&mut host, newer, 77);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 0,
                item: ItemKind::Block(3),
                count: 10,
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: second }
        ),
        InventoryOutcome::Applied
    );
    let snapshot = a
        .world()
        .resource::<client::ClientSession>()
        .container
        .as_ref()
        .unwrap();
    assert_eq!(snapshot.workbench, Some(bench));
    assert_eq!(snapshot.anchor, oldest);
    assert_eq!(snapshot.halves.len(), 1);
    assert!(
        b.world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    );
    let mut command = gesture(&a, ContainerGesture::QuickMove { slot: 0 });
    command.reserved[0] = 6;
    let stale = command.clone();
    assert_eq!(
        transfer(&mut host, &mut a, command),
        InventoryOutcome::Applied
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .slot(0)
            .unwrap()
            .count,
        6
    );
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(
        journal.container_at(oldest).unwrap().slot(0).unwrap().count,
        4
    );
    assert_eq!(
        journal.container_at(newer).unwrap().slot(0).unwrap().count,
        77
    );
    assert_eq!(
        transfer(&mut host, &mut a, stale),
        InventoryOutcome::Conflict
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 87);

    // Attaching a physical second half changes the authorized topology; reopen
    // gets that pair, never an unrelated third chest next to the same bench.
    let partner = [oldest[0] - 1, oldest[1], oldest[2]];
    put(&mut host, [partner[0], partner[1] - 1, partner[2]], 3);
    put(&mut host, partner, 29);
    for (position, peer, secondary) in [(partner, oldest, false), (oldest, partner, true)] {
        host.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..default()
                }),
            )
            .unwrap();
    }
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    });
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: bench }
        ),
        InventoryOutcome::Applied
    );
    let snapshot = a
        .world()
        .resource::<client::ClientSession>()
        .container
        .as_ref()
        .unwrap();
    assert_eq!(snapshot.anchor, partner);
    assert_eq!(snapshot.workbench, Some(bench));
    assert_eq!(snapshot.halves.len(), 2);
    assert!(!snapshot.halves.iter().any(|half| half.position == newer));
    put(&mut host, bench, 0);
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    });
    assert_eq!(total(&host, ItemKind::Block(3)), 87);
}

#[test]
fn private_udp_snapshots_only_reach_openers_and_both_openers_receive_commits() {
    let (mut host, mut a, mut b) = pair();
    let [anchor, other] = fixture(&mut host, &a, true);
    seed(&mut host, anchor, 24);
    seed(&mut host, other, 12);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    assert!(
        b.world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    );
    for client in [&a, &b] {
        assert_eq!(
            client
                .world()
                .resource::<EditJournal>()
                .containers()
                .count(),
            0,
            "private contents leaked through public terrain replication"
        );
    }
    assert_eq!(
        inventory(
            &mut host,
            &mut b,
            InventoryAction::OpenContainer { position: other }
        ),
        InventoryOutcome::Applied
    );
    for client in [&a, &b] {
        let view = client
            .world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .unwrap();
        assert_eq!(view.anchor, anchor);
        assert_eq!(
            view.halves
                .iter()
                .map(|half| half.position)
                .collect::<Vec<_>>(),
            vec![anchor, other]
        );
    }
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .as_ref()
                .is_some_and(|view| view.halves[0].inventory.slot(0).is_none())
        })
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        24
    );
    assert!(
        b.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    act(
        &mut host,
        &mut b,
        ContainerGesture::QuickMove {
            slot: (terra_core::INVENTORY_SLOTS + CHEST_SLOTS) as u8,
        },
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|client| {
            client
                .world()
                .resource::<client::ClientSession>()
                .container
                .as_ref()
                .is_some_and(|view| view.halves[1].inventory.slot(0).is_none())
        })
    });
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(3)),
        12
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 36);
}

#[test]
fn double_chest_udp_click_drag_wheel_and_shift_span_both_halves_without_loss() {
    let (mut host, mut a, _) = pair();
    let [anchor, other] = fixture(&mut host, &a, true);
    let stone = ItemKind::Block(3);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .set_container(
            anchor,
            Inventory {
                slots: vec![
                    ItemStack {
                        item: stone,
                        count: terra_core::STACK_LIMIT
                    };
                    CHEST_SLOTS
                ],
                ..default()
            },
        )
        .unwrap();
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 0,
                item: stone,
                count: 20
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: other }
        ),
        InventoryOutcome::Applied
    );
    let chest = (terra_core::INVENTORY_SLOTS + CHEST_SLOTS) as u8;
    let expected = CHEST_SLOTS as u32 * u32::from(terra_core::STACK_LIMIT) + 20;
    act(&mut host, &mut a, ContainerGesture::QuickMove { slot: 0 });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(other)
            .unwrap()
            .slot(0)
            .unwrap()
            .count,
        20
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: chest,
            secondary: false,
        },
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Drag {
            slots: vec![1, chest, chest + 1],
            secondary: false,
        },
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        2
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: chest + 2,
            secondary: true,
        },
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Wheel {
            slot: chest,
            to_cursor: true,
        },
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::QuickMove { slot: chest + 1 },
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::CloseContainer),
        InventoryOutcome::Applied
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::ReturnCursor),
        InventoryOutcome::Applied
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    assert_eq!(total(&host, stone), expected);
    assert!(
        host.world()
            .resource::<EditJournal>()
            .containers()
            .all(|(_, inventory)| terra_core::containers::valid_chest(inventory))
    );
}

#[test]
fn stale_revisions_unopened_anchors_and_invalid_slots_are_rejected_atomically() {
    let (mut host, mut a, mut b) = pair();
    let [anchor, _] = fixture(&mut host, &a, false);
    seed(&mut host, anchor, 12);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    let original = gesture(
        &a,
        ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
    );
    assert_eq!(
        transfer(&mut host, &mut a, original.clone()),
        InventoryOutcome::Applied
    );
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    assert_eq!(
        transfer(&mut host, &mut a, original.clone()),
        InventoryOutcome::Conflict
    );
    let mut stale_chest = gesture(
        &a,
        ContainerGesture::Click {
            slot: 0,
            secondary: false,
        },
    );
    stale_chest.revisions = original.revisions;
    assert_eq!(
        transfer(&mut host, &mut a, stale_chest),
        InventoryOutcome::Conflict
    );
    let mut stale_player = gesture(
        &a,
        ContainerGesture::Click {
            slot: 0,
            secondary: false,
        },
    );
    stale_player.inventory_revision = original.inventory_revision;
    assert_eq!(
        transfer(&mut host, &mut a, stale_player),
        InventoryOutcome::Conflict
    );
    let mut forged = gesture(
        &a,
        ContainerGesture::Click {
            slot: 0,
            secondary: false,
        },
    );
    forged.inventory_revision = b
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    assert_eq!(
        transfer(&mut host, &mut b, forged),
        InventoryOutcome::ContainerUnavailable
    );
    let invalid = gesture(
        &a,
        ContainerGesture::Click {
            slot: u8::MAX,
            secondary: false,
        },
    );
    assert_eq!(
        transfer(&mut host, &mut a, invalid),
        InventoryOutcome::Invalid
    );
    let mut wrong_anchor = gesture(
        &a,
        ContainerGesture::Click {
            slot: 0,
            secondary: false,
        },
    );
    wrong_anchor.anchor[0] += 1;
    assert_eq!(
        transfer(&mut host, &mut a, wrong_anchor),
        InventoryOutcome::ContainerUnavailable
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        before
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 12);
}

#[test]
fn occluded_and_out_of_reach_udp_accesses_cannot_open_chests() {
    let (mut host, mut a, mut b) = pair();
    // Teleport completion waits for a real local character and its destination
    // sections. Bare transport clients in pair() intentionally have no avatar.
    a.init_resource::<Desired>()
        .add_systems(
            Update,
            avatar_driver.before(terra_world::streaming::StreamingSet::Interest),
        )
        .add_systems(
            FixedUpdate,
            prediction_driver.after(avian3d::prelude::PhysicsSystems::Writeback),
        );
    let [anchor, _] = fixture(&mut host, &a, false);
    seed(&mut host, anchor, 7);
    for x in anchor[0] - 2..=anchor[0] + 3 {
        for y in anchor[1]..=anchor[1] + 3 {
            put(&mut host, [x, y, anchor[2] + 1], 3);
        }
    }
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::ContainerUnavailable
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    );
    for x in anchor[0] - 2..=anchor[0] + 3 {
        for y in anchor[1]..=anchor[1] + 3 {
            put(&mut host, [x, y, anchor[2] + 1], 0);
        }
    }
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    a.world_mut()
        .resource_mut::<client::ClientChatCommands>()
        .0
        .push_back(client::ChatCommand::Send(format!(
            "/tp {} {} {}",
            state.position[0] + 10.0,
            state.position[1],
            state.position[2]
        )));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|next| next.generation > state.generation)
    });
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::ContainerUnavailable
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 7);
}

fn break_chest(
    host: &mut App,
    client: &mut App,
    position: [i64; 3],
) -> protocol::session::EditOutcome {
    use protocol::session::{ClientControl, EditAction};
    let owner = terra_voxel::global_to_section(position).0;
    wait(host, &mut [client], |host, clients| {
        clients[0].world().resource::<EditJournal>().revision(owner)
            == host.world().resource::<EditJournal>().revision(owner)
    });
    let session = client.world().resource::<client::ClientSession>();
    let eye = Vec3::from_array(session.confirmed_state().unwrap().position)
        + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(position.map(|v| v as f32 + 0.5)) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    let revision = client.world().resource::<EditJournal>().revision(owner);
    let command = {
        let mut session = client.world_mut().resource_mut::<client::ClientSession>();
        session.look = look;
        session.last_edit = None;
        session
            .edit_command(
                revision,
                EditAction::Block {
                    position,
                    expected: 29,
                    material: 0,
                },
            )
            .unwrap()
    };
    send(client, ClientControl::Edit(command));
    wait(host, &mut [client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some()
    });
    client
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .1
}

#[test]
fn nonempty_physical_half_blocks_removal_and_topology_change_closes_other_viewer() {
    let (mut host, mut a, mut b) = pair();
    let [anchor, other] = fixture(&mut host, &a, true);
    seed(&mut host, anchor, 12);
    seed(&mut host, other, 9);
    assert_ne!(
        break_chest(&mut host, &mut a, anchor),
        protocol::session::EditOutcome::Applied
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(anchor),
        Some(29)
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory(
            &mut host,
            &mut b,
            InventoryAction::OpenContainer { position: other }
        ),
        InventoryOutcome::Applied
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::CloseContainer),
        InventoryOutcome::Applied
    );
    assert_eq!(
        break_chest(&mut host, &mut a, anchor),
        protocol::session::EditOutcome::Applied
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[1]
            .world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    });
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(
        journal
            .container_at(other)
            .unwrap()
            .count(ItemKind::Block(3)),
        9
    );
    assert!(journal.container_at(anchor).is_none());
    let state = journal.object_state(other).unwrap();
    assert!(!state.secondary && state.peer.is_none());
    assert_eq!(
        inventory(
            &mut host,
            &mut b,
            InventoryAction::OpenContainer { position: other }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        b.world()
            .resource::<client::ClientSession>()
            .container
            .as_ref()
            .unwrap()
            .halves
            .len(),
        1
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 21);
}

#[test]
fn close_unload_disconnect_and_same_uuid_rejoin_conserve_held_chest_items() {
    let (mut host, mut a, mut b) = pair();
    let [anchor, _] = fixture(&mut host, &a, false);
    seed(&mut host, anchor, 17);
    assert_eq!(
        inventory(
            &mut host,
            &mut a,
            InventoryAction::OpenContainer { position: anchor }
        ),
        InventoryOutcome::Applied
    );
    act(
        &mut host,
        &mut a,
        ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: true,
        },
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        9
    );
    let owner = terra_voxel::global_to_section(anchor).0;
    let unloaded = host
        .world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&owner)
        .expect("loaded physical chest section");
    // Exercise one genuine unavailable-authority tick, then restore the exact
    // section. Keeping an interested spawn section forcibly missing indefinitely
    // is not a real streaming unload and can starve a subsequent player's spawn.
    host.update();
    host.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .entry(owner)
        .or_insert(unloaded);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .container
            .is_none()
    });
    assert_eq!(total(&host, ItemKind::Block(3)), 17);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(anchor)
            .unwrap()
            .count(ItemKind::Block(3)),
        8
    );
    assert_eq!(
        inventory(&mut host, &mut a, InventoryAction::CloseContainer),
        InventoryOutcome::Applied
    );
    let profile = a.world().resource::<terra_core::PlayerProfile>().clone();
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
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
        assert!(
            Instant::now() < deadline,
            "chest cursor owner did not disconnect"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut rejoined = profile_client(address, profile);
    wait(&mut host, &mut [&mut rejoined, &mut b], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready
            && u32::from(session.inventory.count(ItemKind::Block(3)))
                + session
                    .inventory
                    .cursor
                    .map_or(0, |stack| u32::from(stack.count))
                == 9
    });
    let inventory = &rejoined
        .world()
        .resource::<client::ClientSession>()
        .inventory;
    assert_eq!(
        rejoined.world().resource::<client::ClientSession>().uuid,
        Some(uuid)
    );
    assert_eq!(
        u32::from(inventory.count(ItemKind::Block(3)))
            + inventory.cursor.map_or(0, |stack| u32::from(stack.count)),
        9
    );
    assert_eq!(total(&host, ItemKind::Block(3)), 17);
}
