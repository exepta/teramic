use super::*;
use crate::{
    ToolKind, WoodSpecies,
    containers::{ContainerGesture, apply_gesture_reserved},
};

fn stack(item: ItemKind, count: u16) -> ItemStack {
    ItemStack { item, count }
}
fn totals(inventories: &[Inventory]) -> BTreeMap<ItemKind, u32> {
    let mut counts = BTreeMap::new();
    for inventory in inventories {
        for stack in inventory.slots.iter().filter(|stack| stack.valid()) {
            *counts.entry(stack.item).or_default() += u32::from(stack.count);
        }
    }
    counts
}

#[test]
fn alphabetical_uses_display_names_while_id_uses_numeric_materials_then_item_identifiers() {
    let stone = ItemKind::Block(3);
    let original = [
        stack(stone, 5),
        stack(ItemKind::Stick, 2),
        stack(ItemKind::Apple, 4),
    ];
    let alphabetic = packed(&original, SortOrder::Alphabetical);
    assert_eq!(
        alphabetic,
        [
            stack(ItemKind::Apple, 4),
            stack(ItemKind::Stick, 2),
            stack(stone, 5)
        ]
    );
    assert_eq!(names::display_name(stone), "Weathered Stone");
    assert_eq!(
        packed(&original, SortOrder::Id),
        [
            stack(stone, 5),
            stack(ItemKind::Apple, 4),
            stack(ItemKind::Stick, 2)
        ]
    );
    let numeric = [stack(ItemKind::Block(13), 1), stack(ItemKind::Block(3), 1)];
    assert_eq!(packed(&numeric, SortOrder::Id), [numeric[1], numeric[0]]);
    for order in [SortOrder::Alphabetical, SortOrder::Id] {
        let sorted = packed(&original, order);
        assert_eq!(packed(&sorted, order), sorted);
    }
}

#[test]
fn backpack_sort_conserves_metadata_hotbar_and_entire_reserved_slots() {
    let stone = ItemKind::Block(3);
    let stack_limit = stone.max_stack();
    let red = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Oak,
        color: [210, 20, 30],
    };
    let blue = ItemKind::TintedBlock {
        material: 148,
        wood: WoodSpecies::Oak,
        color: [30, 20, 210],
    };
    let water = |units| ItemKind::FilledWateringCan {
        tool: ToolKind::IronWateringCan,
        units,
    };
    let mut bag = Inventory {
        slots: vec![ItemStack::EMPTY; INVENTORY_SLOTS],
        revision: 8,
        cursor: None,
    };
    bag.slots[0] = stack(stone, 8);
    bag.slots[9] = stack(water(3), 1);
    bag.slots[10] = stack(stone, stack_limit - 8);
    bag.slots[12] = stack(stone, 24);
    bag.slots[13] = stack(red, 3);
    bag.slots[14] = stack(blue, 2);
    bag.slots[16] = stack(water(4), 1);
    bag.slots[20] = stack(red, 7);
    let before = bag.clone();
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[20] = 2;
    assert!(backpack(&mut bag, SortOrder::Category, reserved));
    assert_eq!(&bag.slots[..9], &before.slots[..9]);
    assert_eq!(bag.slots[20], before.slots[20]);
    assert_eq!(totals(&[bag.clone()]), totals(&[before]));
    assert_eq!(bag.slot(9), Some(stack(stone, stack_limit)));
    assert_eq!(bag.slot(10), Some(stack(stone, 16)));
    assert_eq!(bag.revision, 9);
    assert!(bag.valid());
    let sorted = bag.clone();
    assert!(backpack(&mut bag, SortOrder::Category, reserved));
    assert_eq!(bag, sorted);
    assert!(backpack(&mut bag, SortOrder::Quantity, reserved));
    let amounts: Vec<_> = (9..INVENTORY_SLOTS)
        .filter(|&i| reserved[i] == 0)
        .filter_map(|i| bag.slot(i))
        .map(|s| s.count)
        .collect();
    assert!(amounts.windows(2).all(|pair| pair[0] >= pair[1]));
    assert_eq!(bag.slots[20], sorted.slots[20]);
}

#[test]
fn backpack_sort_rejects_cursor_forged_reservations_and_revision_overflow_atomically() {
    let mut bag = Inventory {
        slots: vec![ItemStack::EMPTY; 12],
        revision: 2,
        cursor: None,
    };
    bag.slots[10] = stack(ItemKind::Block(3), 5);
    for case in 0..3 {
        let mut draft = bag.clone();
        let mut reserved = [0; INVENTORY_SLOTS];
        match case {
            0 => draft.cursor = Some(stack(ItemKind::Block(3), 1)),
            1 => reserved[10] = 6,
            _ => draft.revision = u64::MAX,
        }
        let before = draft.clone();
        assert!(!backpack(&mut draft, SortOrder::Category, reserved));
        assert_eq!(draft, before);
    }
}

#[test]
fn double_storage_sort_is_atomic_conserves_variants_and_leaves_player_unchanged() {
    let mut player = Inventory {
        slots: vec![stack(ItemKind::Block(3), 10)],
        ..Default::default()
    };
    let oak = ItemKind::for_wood(29, WoodSpecies::Oak);
    let birch = ItemKind::for_wood(29, WoodSpecies::Birch);
    let stack_limit = birch.max_stack();
    let mut halves = [
        Inventory {
            slots: vec![ItemStack::EMPTY; CHEST_SLOTS],
            ..Default::default()
        },
        Inventory::default(),
    ];
    halves[0].slots[17] = stack(birch, stack_limit - 8);
    halves[1].slots = vec![stack(oak, 5), stack(birch, 14)];
    let before = halves.clone();
    let player_before = player.clone();
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[0] = 4;
    assert!(apply_gesture_reserved(
        &mut player,
        &mut halves,
        ContainerGesture::Sort {
            order: SortOrder::Category
        },
        reserved
    ));
    assert_eq!(player, player_before);
    assert_eq!(totals(&halves), totals(&before));
    assert_eq!(halves[0].slot(0), Some(stack(oak, 5)));
    assert_eq!(halves[0].slot(1), Some(stack(birch, stack_limit)));
    assert_eq!(halves[0].slot(2), Some(stack(birch, 6)));
    assert_eq!(halves[0].revision, 1);
    assert_eq!(halves[1].revision, 1);
    let mut exhausted = before;
    exhausted[1].revision = u64::MAX;
    let snapshot = exhausted.clone();
    assert!(!storage(
        &player,
        &mut exhausted,
        SortOrder::Category,
        reserved
    ));
    assert_eq!(exhausted, snapshot);
}

#[test]
fn machine_process_slots_cannot_be_sorted_but_its_player_backpack_can() {
    let mut player = Inventory {
        slots: vec![ItemStack::EMPTY; 12],
        ..Default::default()
    };
    player.slots[11] = stack(ItemKind::Block(3), 7);
    let mut machine = Inventory::default();
    let before = machine.clone();
    assert!(!crate::furnace::apply_gesture_reserved(
        &mut player,
        &mut machine,
        ContainerGesture::Sort {
            order: SortOrder::Category
        },
        [0; INVENTORY_SLOTS]
    ));
    assert_eq!(machine, before);
    assert!(crate::furnace::apply_gesture_reserved(
        &mut player,
        &mut machine,
        ContainerGesture::SortBackpack {
            order: SortOrder::Category
        },
        [0; INVENTORY_SLOTS]
    ));
    assert_eq!(machine, before);
    assert_eq!(player.slot(9), Some(stack(ItemKind::Block(3), 7)));
}
