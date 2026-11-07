use super::*;
use crate::ItemKind;
const FIRST_CHEST: u8 = INVENTORY_SLOTS as u8;
const SECOND_CHEST: u8 = (INVENTORY_SLOTS + CHEST_SLOTS) as u8;
const VIEW_END: u8 = CONTAINER_VIEW_SLOTS as u8;

fn stack(count: u16) -> ItemStack {
    ItemStack {
        item: ItemKind::Block(3),
        count,
    }
}
fn total(player: &Inventory, halves: &[Inventory]) -> u32 {
    std::iter::once(player)
        .chain(halves.iter())
        .flat_map(|inventory| inventory.slots.iter().copied().chain(inventory.cursor))
        .filter(|stack| stack.valid())
        .map(|stack| u32::from(stack.count))
        .sum()
}

#[test]
fn linked_workbench_chest_moves_only_unreserved_items_without_duplicating_stacks() {
    let mut player = Inventory::default();
    player.insert(ItemKind::Block(3), 12);
    let mut halves = [Inventory::default()];
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[0] = 7;
    assert!(apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: 0 },
        reserved
    ));
    assert_eq!(player.slot(0), Some(stack(7)));
    assert_eq!(halves[0].slot(0), Some(stack(5)));
    assert_eq!(total(&player, &halves), 12);
    let before = (player.clone(), halves.clone());
    assert!(!apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: 0,
            secondary: false
        },
        reserved
    ));
    assert_eq!((player.clone(), halves.clone()), before);
    player.cursor = Some(ItemStack {
        item: ItemKind::Stick,
        count: 3,
    });
    let before = (player.clone(), halves.clone());
    assert!(!apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: 0,
            secondary: false
        },
        reserved
    ));
    assert_eq!((player, halves), before);
}

#[test]
fn reserved_merge_cannot_overflow_and_invalid_reservations_leave_all_state_unchanged() {
    for count in [129, 5] {
        let mut player = Inventory::default();
        player.insert(ItemKind::Block(3), 4);
        let mut halves = [Inventory::default()];
        let mut reserved = [0; INVENTORY_SLOTS];
        reserved[0] = count;
        let before = (player.clone(), halves.clone());
        assert!(!apply_gesture_reserved(
            &mut player,
            &mut halves,
            ContainerGesture::QuickMove { slot: 0 },
            reserved
        ));
        assert_eq!((player, halves), before);
    }
    let mut player = Inventory::default();
    player.insert(ItemKind::Block(3), 128);
    player.cursor = Some(stack(1));
    let mut halves = [Inventory::default()];
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[0] = 128;
    let before = (player.clone(), halves.clone());
    assert!(!apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: 0,
            secondary: false
        },
        reserved
    ));
    assert_eq!((player, halves), before);
}

#[test]
fn incoming_chest_stacks_skip_slots_reserved_for_a_different_crafting_ingredient() {
    let mut player = Inventory::default();
    player.insert(ItemKind::Stick, 4);
    let mut halves = [Inventory {
        slots: vec![stack(8)],
        ..Default::default()
    }];
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[0] = 4;
    assert!(apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: FIRST_CHEST },
        reserved
    ));
    assert_eq!(player.slot(0).unwrap().item, ItemKind::Stick);
    assert_eq!(player.slot(0).unwrap().count, 4);
    assert_eq!(player.slot(1), Some(stack(8)));
    assert_eq!(halves[0].occupied_slots(), 0);
    assert_eq!(total(&player, &halves), 12);
}

#[test]
fn cross_inventory_split_drag_collect_and_quick_move_conserve_items_and_revisions() {
    let mut player = Inventory::default();
    player.insert(ItemKind::Block(3), 127);
    let mut halves = [Inventory::default(), Inventory::default()];
    let original = total(&player, &halves);
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: 0,
            secondary: true
        }
    ));
    assert_eq!(player.cursor, Some(stack(64)));
    let revision = player.revision;
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Drag {
            slots: vec![FIRST_CHEST, SECOND_CHEST],
            secondary: false
        }
    ));
    assert_eq!(player.revision, revision + 1);
    assert_eq!(halves[0].revision, 1);
    assert_eq!(halves[1].revision, 1);
    assert_eq!(halves[0].slot(0), Some(stack(32)));
    assert_eq!(halves[1].slot(0), Some(stack(32)));
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Collect { slot: 0 }
    ));
    assert_eq!(player.cursor, Some(stack(127)));
    assert!(
        halves
            .iter()
            .all(|half| half.occupied_slots() == 0 && half.cursor.is_none())
    );
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: false
        }
    ));
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: FIRST_CHEST }
    ));
    assert_eq!(player.slot(0), Some(stack(127)));
    assert_eq!(total(&player, &halves), original);
}

#[test]
fn duplicate_or_unavailable_drag_slots_invalid_inventories_and_revision_overflow_are_atomic() {
    let player = Inventory {
        cursor: Some(stack(5)),
        ..Default::default()
    };
    let halves = [Inventory::default()];
    for slots in [
        vec![FIRST_CHEST, FIRST_CHEST],
        vec![FIRST_CHEST, SECOND_CHEST],
        vec![VIEW_END],
        vec![FIRST_CHEST; CONTAINER_VIEW_SLOTS + 1],
        Vec::new(),
    ] {
        let mut next = player.clone();
        let mut targets = halves.clone();
        assert!(!apply_gesture(
            &mut next,
            &mut targets,
            ContainerGesture::Drag {
                slots,
                secondary: false
            }
        ));
        assert_eq!(next, player);
        assert_eq!(targets, halves);
    }
    let mut next = player.clone();
    let mut targets = [Inventory {
        revision: u64::MAX,
        ..Default::default()
    }];
    let before = targets.clone();
    assert!(!apply_gesture(
        &mut next,
        &mut targets,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: false
        }
    ));
    assert_eq!(next, player);
    assert_eq!(targets, before);
    targets[0] = Inventory {
        cursor: Some(stack(1)),
        ..Default::default()
    };
    assert!(!apply_gesture(
        &mut next,
        &mut targets,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: false
        }
    ));
}

#[test]
fn opposite_pane_moves_merge_before_empty_slots_and_preserve_unlike_stacks() {
    let mut player = Inventory::default();
    player.insert(ItemKind::Block(3), 7);
    let mut halves = [Inventory {
        slots: vec![ItemStack::EMPTY, stack(125)],
        ..Default::default()
    }];
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: 0 }
    ));
    assert_eq!(halves[0].slot(1), Some(stack(128)));
    assert_eq!(halves[0].slot(0), Some(stack(4)));
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: true
        }
    ));
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Wheel {
            slot: FIRST_CHEST + 1,
            to_cursor: true
        }
    ));
    assert_eq!(player.cursor, Some(stack(3)));
    assert_eq!(halves[0].slot(1), Some(stack(127)));
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Wheel {
            slot: 1,
            to_cursor: false
        }
    ));
    assert_eq!(player.slot(1), Some(stack(1)));
    assert_eq!(total(&player, &halves), 132);
}

#[test]
fn gesture_serialization_roundtrips_variable_drag_targets() {
    let gesture = ContainerGesture::Drag {
        slots: vec![
            0,
            FIRST_CHEST - 1,
            FIRST_CHEST,
            SECOND_CHEST - 1,
            SECOND_CHEST,
            VIEW_END - 1,
        ],
        secondary: true,
    };
    let text = toml::to_string(&gesture).unwrap();
    assert_eq!(toml::from_str::<ContainerGesture>(&text).unwrap(), gesture);
}

#[test]
fn full_capacity_and_foreign_secondary_swaps_reject_without_touching_any_inventory() {
    let mut player = Inventory {
        slots: vec![stack(1)],
        cursor: Some(ItemStack {
            item: ItemKind::Stick,
            count: 9,
        }),
        revision: 7,
    };
    let mut halves = [Inventory {
        slots: vec![stack(128); CHEST_SLOTS],
        revision: 12,
        cursor: None,
    }];
    let original = (player.clone(), halves.clone());
    assert!(!apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: 0 }
    ));
    assert_eq!((player.clone(), halves.clone()), original);
    assert!(!apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: true
        }
    ));
    assert_eq!((player.clone(), halves.clone()), original);
    assert!(!apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::CollectFrom { slot: FIRST_CHEST }
    ));
    assert_eq!((player.clone(), halves.clone()), original);
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::Click {
            slot: FIRST_CHEST,
            secondary: false
        }
    ));
    assert_eq!(player.cursor, Some(stack(128)));
    assert_eq!(
        halves[0].slot(0),
        Some(ItemStack {
            item: ItemKind::Stick,
            count: 9
        })
    );
    assert_eq!(player.revision, 8);
    assert_eq!(halves[0].revision, 13);
}

#[test]
fn shift_click_with_held_cursor_can_use_second_half_without_modifying_first() {
    let mut player = Inventory {
        slots: vec![stack(7)],
        cursor: Some(ItemStack {
            item: ItemKind::Stick,
            count: 3,
        }),
        revision: 4,
    };
    let first = Inventory {
        slots: vec![stack(128); CHEST_SLOTS],
        revision: 17,
        cursor: None,
    };
    let mut halves = [first.clone(), Inventory::default()];
    let original = total(&player, &halves);
    assert!(apply_gesture(
        &mut player,
        &mut halves,
        ContainerGesture::QuickMove { slot: 0 }
    ));
    assert_eq!(
        player.cursor,
        Some(ItemStack {
            item: ItemKind::Stick,
            count: 3
        })
    );
    assert_eq!(halves[0], first);
    assert_eq!(halves[1].slot(0), Some(stack(7)));
    assert_eq!(halves[1].revision, 1);
    assert_eq!(player.revision, 5);
    assert_eq!(total(&player, &halves), original);
}
