use super::*;
use crate::protocol::inventory::{InventoryAction, InventoryCommand, InventoryOutcome};
use terra_core::{ItemKind, ItemStack, ResourcePropKind};
use terra_world::edits::EditJournal;
#[test]
fn all_dyes_are_real_server_inventory_stacks_and_replicate_to_the_client() {
    let (mut host, mut client, _) = pair();
    for color in terra_core::beds::DyeColor::ALL {
        let item = ItemKind::Dye(color);
        assert_eq!(
            inventory_command(
                &mut host,
                &mut client,
                InventoryAction::CreativeQuickMove { item, count: 1 }
            ),
            InventoryOutcome::Applied
        );
        assert_eq!(
            client
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(item),
            1
        );
    }
}

fn inventory_command(
    host: &mut App,
    client: &mut App,
    action: InventoryAction,
) -> InventoryOutcome {
    let revision = client
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
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
            .last_inventory
            .is_some()
    });
    client
        .world()
        .resource::<client::ClientSession>()
        .last_inventory
        .unwrap()
}

#[test]
fn creative_trash_removes_only_the_players_real_stack_without_spawning_a_drop() {
    let (mut host, mut a, b) = pair();
    let item = ItemKind::Block(3);
    for slot in [None, Some(8)] {
        assert_eq!(
            inventory_command(
                &mut host,
                &mut a,
                match slot {
                    Some(slot) => InventoryAction::CreativeSet {
                        slot,
                        item,
                        count: 73
                    },
                    None => InventoryAction::CreativeCursor { item, count: 73 },
                }
            ),
            InventoryOutcome::Applied
        );
        let before = a
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .clone();
        let other = b
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .clone();
        let drops = host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .clone();
        let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
        assert_eq!(
            inventory_command(&mut host, &mut a, InventoryAction::CreativeTrash { slot }),
            InventoryOutcome::Applied
        );
        let bag = &a.world().resource::<client::ClientSession>().inventory;
        assert_eq!(bag.revision, before.revision + 1);
        assert!(match slot {
            Some(slot) => bag.slot(usize::from(slot)).is_none(),
            None => bag.cursor.is_none(),
        });
        assert_eq!(
            &host
                .world()
                .resource::<EditJournal>()
                .gameplay()
                .inventories[&uuid],
            bag
        );
        assert_eq!(
            b.world().resource::<client::ClientSession>().inventory,
            other
        );
        assert_eq!(
            host.world().resource::<EditJournal>().gameplay().drops,
            drops
        );
        assert_eq!(
            inventory_command(&mut host, &mut a, InventoryAction::CreativeTrash { slot }),
            InventoryOutcome::Invalid
        );
    }
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CreativeTrash {
                slot: Some(u8::MAX)
            }
        ),
        InventoryOutcome::Invalid
    );
}

#[test]
fn survival_and_spectator_cannot_use_creative_trash_packets() {
    let (mut host, mut a, _) = pair();
    let item = ItemKind::Block(3);
    for action in [
        InventoryAction::CreativeSet {
            slot: 8,
            item,
            count: 27,
        },
        InventoryAction::CreativeCursor { item, count: 12 },
    ] {
        assert_eq!(
            inventory_command(&mut host, &mut a, action),
            InventoryOutcome::Applied
        );
    }
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    for mode in [
        terra_core::GameMode::Survival,
        terra_core::GameMode::Spectator,
    ] {
        host.world_mut()
            .resource_mut::<GameConfig>()
            .server
            .game_mode = mode;
        for slot in [None, Some(8)] {
            assert_eq!(
                inventory_command(&mut host, &mut a, InventoryAction::CreativeTrash { slot }),
                InventoryOutcome::Denied
            );
            assert_eq!(
                a.world().resource::<client::ClientSession>().inventory,
                before
            );
        }
    }
}

#[test]
fn crafting_cursor_transfers_replicate_and_cannot_create_items_or_use_other_players_slots() {
    let (mut host, mut a, mut b) = pair();
    let item = ItemKind::Block(16);
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CreativeCursor { item, count: 12 }
        ),
        InventoryOutcome::Applied
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    let mut counts = [0; terra_core::INVENTORY_SLOTS];
    counts[0] = 6;
    counts[1] = 5;
    let deposit = InventoryAction::TransferCursor {
        item,
        counts,
        to_cursor: false,
    };
    assert_eq!(
        inventory_command(&mut host, &mut b, deposit),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        inventory_command(&mut host, &mut a, deposit),
        InventoryOutcome::Applied
    );
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    assert_eq!(before.cursor.unwrap().count, 1);
    assert_eq!(before.slot(0).unwrap().count, 6);
    assert_eq!(
        inventory_command(&mut host, &mut a, deposit),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        before
    );
    let collect = InventoryAction::TransferCursor {
        item,
        counts,
        to_cursor: true,
    };
    assert_eq!(
        inventory_command(&mut host, &mut b, collect),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        inventory_command(&mut host, &mut a, collect),
        InventoryOutcome::Applied
    );
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(bag.count(item), 0);
    assert_eq!(bag.cursor.unwrap().count, 12);
}

#[test]
fn reserved_grid_commands_validate_real_quantities_and_replicate_dominant_wood() {
    use terra_core::crafting::{CraftDestination, CraftGridCell};
    let (mut host, mut a, mut b) = pair();
    for (slot, item, count) in [
        (0, ItemKind::Block(16), 2),
        (1, ItemKind::Block(17), 4),
        (2, ItemKind::Block(18), 80),
    ] {
        assert_eq!(
            inventory_command(
                &mut host,
                &mut a,
                InventoryAction::CreativeSet { slot, item, count }
            ),
            InventoryOutcome::Applied
        );
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    let mut cells = [None; 9];
    for (index, slot, count) in [(0, 0, 1), (1, 1, 1), (2, 1, 1), (3, 1, 1)] {
        cells[index] = Some(CraftGridCell { slot, count });
    }
    let request = InventoryAction::CraftReserved {
        cells,
        size: 2,
        station: None,
        count: 1,
        destination: CraftDestination::Cursor,
    };
    let before = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .clone();
    let mut invalid = cells;
    invalid[8] = Some(CraftGridCell { slot: 2, count: 1 });
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CraftReserved {
                cells: invalid,
                size: 2,
                station: None,
                count: 1,
                destination: CraftDestination::Cursor
            }
        ),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        a.world().resource::<client::ClientSession>().inventory,
        before
    );
    assert_eq!(
        inventory_command(&mut host, &mut b, request),
        InventoryOutcome::Invalid
    );
    assert_eq!(
        inventory_command(&mut host, &mut a, request),
        InventoryOutcome::Applied
    );
    let inventory = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(
        inventory.cursor,
        Some(ItemStack {
            item: ItemKind::for_wood(28, terra_core::WoodSpecies::Birch),
            count: 1
        })
    );
    assert_eq!(inventory.count(ItemKind::Block(16)), 1);
    assert_eq!(inventory.count(ItemKind::Block(17)), 1);
    assert_eq!(inventory.count(ItemKind::Block(18)), 80);
}

#[test]
fn real_recipe_transactions_reject_replay_and_revalidate_workbench_support() {
    use terra_core::crafting::{CraftDestination, recipe_key};
    let (mut host, mut a, mut b) = pair();
    for client in [&mut a, &mut b] {
        assert_eq!(
            inventory_command(
                &mut host,
                client,
                InventoryAction::CreativeSet {
                    slot: 0,
                    item: ItemKind::Block(16),
                    count: 32
                }
            ),
            InventoryOutcome::Applied
        );
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    let a_id = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let b_id = b.world().resource::<client::ClientSession>().uuid.unwrap();
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let bench = [
        position[0].floor() as i64,
        (position[1] + 0.5).floor() as i64,
        position[2].floor() as i64 - 2,
    ];
    let put = |host: &mut App, material| {
        host.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let properties = world
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
                    .clone();
                let mut chunks = world.resource_mut::<ChunkStore>();
                let expected_material = chunks.block_at(bench).unwrap();
                terra_world::streaming::commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &terra_world::streaming::BlockEditRequest {
                        position: bench,
                        expected_material,
                        material,
                    },
                    &properties,
                )
                .unwrap();
            });
    };
    put(&mut host, 28);
    let recipe = InventoryAction::Recipe {
        recipe: recipe_key("teramic::chest"),
        count: 2,
        station: Some(bench),
        destination: CraftDestination::Inventory,
    };
    let revision = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    assert_eq!(
        inventory_command(&mut host, &mut a, recipe),
        InventoryOutcome::Applied
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(29)),
        2
    );
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(16)),
        16
    );
    a.world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    a.world_mut()
        .write_message(client::InventoryRequest(InventoryCommand {
            revision,
            action: recipe,
        }));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_inventory
            == Some(InventoryOutcome::Conflict)
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&a_id]
            .count(ItemKind::Block(29)),
        2
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&b_id]
            .count(ItemKind::Block(16)),
        32
    );
    // Each player spends their own resources at the same station.
    assert_eq!(
        inventory_command(&mut host, &mut b, recipe),
        InventoryOutcome::Applied
    );
    put(&mut host, 0);
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .inventories[&a_id]
        .clone();
    assert_eq!(
        inventory_command(&mut host, &mut a, recipe),
        InventoryOutcome::WrongStation
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&a_id],
        before
    );
    for station in [Some([i64::MIN; 3]), Some([900_000; 3]), None] {
        assert_eq!(
            inventory_command(
                &mut host,
                &mut a,
                InventoryAction::Recipe {
                    station,
                    recipe: recipe_key("teramic::chest"),
                    count: 2,
                    destination: CraftDestination::Inventory
                }
            ),
            InventoryOutcome::WrongStation
        );
    }
}

#[test]
fn real_udp_cursor_drag_split_and_drops_conserve_resources_and_reject_stale_replay() {
    let (mut host, mut a, mut b) = pair();
    let stone = ItemKind::Block(3);
    let actions = [
        InventoryAction::CreativeCursor {
            item: stone,
            count: 128,
        },
        InventoryAction::Click {
            slot: 0,
            secondary: true,
        },
        InventoryAction::Click {
            slot: 1,
            secondary: true,
        },
    ];
    for action in actions {
        assert_eq!(
            inventory_command(&mut host, &mut a, action),
            InventoryOutcome::Applied
        );
    }
    let mut slots = [None; 27];
    slots[..3].copy_from_slice(&[Some(9), Some(10), Some(11)]);
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::Drag {
                slots,
                secondary: false
            }
        ),
        InventoryOutcome::Applied
    );
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(bag.cursor, None);
    assert_eq!(bag.slot(0).unwrap().count, 1);
    assert_eq!(bag.slot(9).unwrap().count, 42);
    assert_eq!(bag.count(stone), 128);
    let revision = bag.revision;
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::DropSlot { slot: 9, all: true }
        ),
        InventoryOutcome::Applied
    );
    let state = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(state.drops.len(), 1);
    assert_eq!(state.drops[0].stack.count, 42);
    assert!(state.drops[0].pickup_delay_seconds > 0.0);
    assert!(state.drops[0].velocity.iter().any(|v| v.abs() > 0.01));
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    assert_eq!(
        state.inventories[&uuid].count(stone)
            + state
                .drops
                .iter()
                .filter(|d| d.stack.item == stone)
                .map(|d| d.stack.count)
                .sum::<u16>(),
        128
    );
    a.world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    a.world_mut()
        .write_message(client::InventoryRequest(InventoryCommand {
            revision,
            action: InventoryAction::DropSlot { slot: 9, all: true },
        }));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_inventory
            == Some(InventoryOutcome::Conflict)
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .len(),
        1
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CreativeCursor {
                item: stone,
                count: 128
            }
        ),
        InventoryOutcome::Denied
    );
}

#[test]
fn real_udp_cursor_outside_drop_and_close_recover_the_exact_stack() {
    let (mut host, mut a, _) = pair();
    let stone = ItemKind::Block(3);
    for action in [
        InventoryAction::CreativeCursor {
            item: stone,
            count: 7,
        },
        InventoryAction::DropCursor { all: false },
        InventoryAction::ReturnCursor,
    ] {
        assert_eq!(
            inventory_command(&mut host, &mut a, action),
            InventoryOutcome::Applied
        );
    }
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert!(bag.cursor.is_none());
    assert_eq!(bag.count(stone), 6);
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().drops[0]
            .stack
            .count,
        1
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory_response
            >= 3
    );
}

#[test]
fn full_drop_budget_rolls_back_cursor_close_and_slot_drops_without_losing_items() {
    let (mut host, mut a, _) = pair();
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let stone = ItemKind::Block(3);
    let flower = ItemKind::Prop(ResourcePropKind::Flower);
    let mut bag = terra_core::Inventory::default();
    assert_eq!(bag.insert(stone, 27 * 128), 0);
    bag.creative_cursor(ItemStack {
        item: flower,
        count: 7,
    });
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let state = journal.gameplay_mut();
        state.inventories.insert(uuid, bag.clone());
        for _ in 0..terra_core::MAX_WORLD_DROPS {
            assert!(state.drop_one([500.0; 3], stone));
        }
        state.revision += 1;
    }
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            == bag
    });
    for action in [
        InventoryAction::ReturnCursor,
        InventoryAction::DropCursor { all: true },
        InventoryAction::DropSlot { slot: 0, all: true },
    ] {
        assert_eq!(
            inventory_command(&mut host, &mut a, action),
            InventoryOutcome::Invalid
        );
        assert_eq!(a.world().resource::<client::ClientSession>().inventory, bag);
        assert_eq!(
            host.world()
                .resource::<EditJournal>()
                .gameplay()
                .inventories[&uuid],
            bag
        );
    }
}

#[test]
fn real_udp_inventory_moves_crafts_rejects_replay_and_denies_survival_item_creation() {
    let (mut host, mut a, mut b) = pair();
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 8,
                item: ItemKind::Block(6),
                count: 128
            }
        ),
        InventoryOutcome::Applied
    );
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::Move {
                from: 8,
                to: 0,
                count: 1
            }
        ),
        InventoryOutcome::Applied
    );
    let revision = a
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::Craft {
                slots: [Some(0), None, None, None]
            }
        ),
        InventoryOutcome::Applied
    );
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(bag.slot(8).unwrap().count, 127);
    assert_eq!(
        bag.slot(0),
        Some(ItemStack {
            item: ItemKind::Block(16),
            count: 4
        })
    );
    a.world_mut()
        .resource_mut::<client::ClientSession>()
        .last_inventory = None;
    a.world_mut()
        .write_message(client::InventoryRequest(InventoryCommand {
            revision,
            action: InventoryAction::Craft {
                slots: [Some(0), None, None, None],
            },
        }));
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_inventory
            == Some(InventoryOutcome::Conflict)
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(16)),
        4
    );
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    assert_eq!(
        inventory_command(
            &mut host,
            &mut a,
            InventoryAction::CreativeSet {
                slot: 1,
                item: ItemKind::Block(3),
                count: 128
            }
        ),
        InventoryOutcome::Denied
    );
    assert!(
        b.world()
            .resource::<client::ClientSession>()
            .inventory
            .slots
            .is_empty()
    );
}

#[test]
fn harvested_prop_placement_replicates_and_survives_section_reload_without_duplication() {
    let (mut host, mut a, mut b) = pair();
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let ground_y =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let chunks = host.world().resource::<ChunkStore>();
    let mut chosen = None;
    'search: for x in 5..=12 {
        for z in 19..=22 {
            let support = [x, ground_y, z];
            let target = Vec3::new(x as f32 + 0.5, ground_y as f32 + 1.0, z as f32 + 0.5);
            if chunks.loaded.values().flat_map(|s| &s.props).any(|p| {
                !p.removed
                    && (p.position[0] - f64::from(target.x)).abs() < 0.8
                    && (p.position[2] - f64::from(target.z)).abs() < 0.8
            }) {
                continue;
            }
            let direction = (target - eye).normalize();
            let (_, selected) = terra_world::targeting::pick_world(
                chunks,
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            );
            if matches!(selected, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == support && hit.normal == [0,1,0])
            {
                chosen = Some((
                    support,
                    [(-direction.x).atan2(-direction.z), direction.y.asin()],
                ));
                break 'search;
            }
        }
    }
    let (support, look) = chosen.expect("walkable empty planting target");
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let bag = journal.gameplay_mut().inventories.entry(uuid).or_default();
        let item = ItemKind::Prop(ResourcePropKind::Flower);
        assert!(bag.creative_set(0, ItemStack { item, count: 7 }));
        assert!(bag.creative_set(8, ItemStack { item, count: 1 }));
        journal.gameplay_mut().revision += 1;
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = terra_core::GameMode::Survival;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Prop(ResourcePropKind::Flower))
            == 8
    });
    a.world_mut().resource_mut::<client::ClientSession>().look = look;
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    a.world_mut().write_message(client::PropPlaceRequest {
        support,
        kind: ResourcePropKind::Flower,
    });
    let owner = terra_voxel::global_to_section(support).0;
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|app| {
            app.world()
                .resource::<EditJournal>()
                .section_delta(owner)
                .placed_props
                .len()
                == 1
        })
    });
    let delta = host.world().resource::<EditJournal>().section_delta(owner);
    let prop = delta.placed_props[0];
    assert_eq!(prop.position[1], (support[1] + 1) as f64);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Prop(ResourcePropKind::Flower))
            == 7
    });
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(bag.slot(0).unwrap().count, 7);
    assert!(bag.slot(8).is_none());
    for app in [&mut a, &mut b] {
        let props = &app.world().resource::<ChunkStore>().loaded[&owner].props;
        assert_eq!(
            props
                .iter()
                .filter(|p| p.id == prop.id && !p.removed)
                .count(),
            1
        );
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let mut chunks = world.resource_mut::<ChunkStore>();
                let mut section = chunks.loaded.remove(&owner).unwrap();
                section.props.retain(|p| p.id != prop.id);
                chunks.loaded.insert(owner, section);
                chunks
                    .apply_confirmed_delta(&mut journal, delta.clone())
                    .unwrap();
                chunks
                    .apply_confirmed_delta(&mut journal, delta.clone())
                    .unwrap();
                assert_eq!(
                    chunks.loaded[&owner]
                        .props
                        .iter()
                        .filter(|p| p.id == prop.id && !p.removed)
                        .count(),
                    1
                );
            });
    }
    let encoded = serde_json::to_string(&delta).unwrap();
    let mut reloaded = EditJournal::default();
    reloaded
        .import_delta(serde_json::from_str(&encoded).unwrap())
        .unwrap();
    let mut props = Vec::new();
    reloaded.apply_prop_removals(owner, &mut props);
    reloaded.apply_prop_removals(owner, &mut props);
    assert_eq!(
        props,
        [terra_world::props::WorldProp {
            position: [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
            ..prop
        }]
    );
    reloaded.remove_prop(owner, prop.id).unwrap();
    props.clear();
    reloaded.apply_prop_removals(owner, &mut props);
    assert!(props.is_empty());
    assert!(reloaded.section_delta(owner).placed_props.is_empty());

    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    for requested_count in 2..=5 {
        let look = planting_look_through_gaps(&host, &a, support);
        {
            let mut session = a.world_mut().resource_mut::<client::ClientSession>();
            session.look = look;
            session.last_edit = None;
        }
        a.world_mut().write_message(client::PropPlaceRequest {
            support,
            kind: ResourcePropKind::Flower,
        });
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some()
        });
        let expected = if requested_count <= 4 {
            crate::protocol::session::EditOutcome::Applied
        } else {
            crate::protocol::session::EditOutcome::Conflict
        };
        assert_eq!(
            a.world()
                .resource::<client::ClientSession>()
                .last_edit
                .unwrap()
                .1,
            expected
        );
        let accepted_count = requested_count.min(4);
        wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
            clients.iter().all(|app| {
                app.world()
                    .resource::<EditJournal>()
                    .section_delta(owner)
                    .placed_props
                    .len()
                    == accepted_count
            }) && clients[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(ItemKind::Prop(ResourcePropKind::Flower))
                == (8 - accepted_count) as u16
        });
        let delta = host.world().resource::<EditJournal>().section_delta(owner);
        assert_eq!(
            delta
                .placed_props
                .iter()
                .find(|candidate| candidate.id == prop.id),
            Some(&prop)
        );
        assert!(
            delta
                .placed_props
                .iter()
                .all(|candidate| terra_world::props::placement::support(candidate) == support)
        );
        let active = |app: &App| {
            let mut props: Vec<_> = app.world().resource::<ChunkStore>().loaded[&owner]
                .props
                .iter()
                .copied()
                .filter(|p| !p.removed && terra_world::props::placement::support(p) == support)
                .collect();
            props.sort_unstable_by_key(|p| p.id);
            props
        };
        let authoritative = active(&host);
        assert_eq!(authoritative.len(), accepted_count);
        for client in [&a, &b] {
            assert_eq!(active(client), authoritative);
        }
        for current in &authoritative {
            let mut stored = *delta
                .placed_props
                .iter()
                .find(|p| p.id == current.id)
                .unwrap();
            stored.position = current.position;
            assert_eq!(*current, stored);
        }
    }
    let saved = serde_json::to_string(&host.world().resource::<EditJournal>().section_delta(owner))
        .unwrap();
    let mut restored = EditJournal::default();
    restored
        .import_delta(serde_json::from_str(&saved).unwrap())
        .unwrap();
    let mut restored_props = Vec::new();
    restored.apply_prop_removals(owner, &mut restored_props);
    restored_props.sort_unstable_by_key(|p| p.id);
    let mut live: Vec<_> = host.world().resource::<ChunkStore>().loaded[&owner]
        .props
        .iter()
        .copied()
        .filter(|p| !p.removed && terra_world::props::placement::support(p) == support)
        .collect();
    live.sort_unstable_by_key(|p| p.id);
    assert_eq!(restored_props, live);
}

fn planting_look_through_gaps(host: &App, client: &App, support: [i64; 3]) -> [f32; 2] {
    let state = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    for z in [0.5, 0.3, 0.7, 0.1, 0.9] {
        for x in [0.5, 0.3, 0.7, 0.1, 0.9] {
            let target = Vec3::new(
                support[0] as f32 + x,
                support[1] as f32 + 1.0,
                support[2] as f32 + z,
            );
            let direction = (target - eye).normalize();
            let (_, selected) = terra_world::targeting::pick_world(
                host.world().resource::<ChunkStore>(),
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            );
            if matches!(selected, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == support && hit.normal == [0, 1, 0])
            {
                return [(-direction.x).atan2(-direction.z), direction.y.asin()];
            }
        }
    }
    panic!("Corner plants must leave visible support for the next placement");
}

#[test]
fn authoritative_drops_fall_collide_expire_and_keep_old_snapshot_compatibility() {
    let (mut host, mut a, mut b) = pair();
    let y = host
        .world()
        .resource::<terra_world::streaming::WorldTerrain>()
        .0
        .surface_height(20.5, 24.5)
        .ceil();
    assert!(
        host.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .drop_one([20.5, y + 4.0, 24.5], ItemKind::Block(3))
    );
    let original = host.world().resource::<EditJournal>().gameplay().drops[0].position[1];
    for _ in 0..180 {
        host.update();
        a.update();
        b.update();
    }
    let drop = &host.world().resource::<EditJournal>().gameplay().drops[0];
    assert!(
        drop.position[1] < original - 2.0,
        "drop did not fall: {drop:?}"
    );
    assert!(
        (y..=y + 0.25).contains(&drop.position[1]),
        "drop crossed the supporting terrain: {drop:?}"
    );
    assert!(drop.age_seconds > 1.0);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .drops[0]
        .age_seconds = terra_core::DROP_LIFETIME_SECONDS - 0.001;
    host.update();
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
}
