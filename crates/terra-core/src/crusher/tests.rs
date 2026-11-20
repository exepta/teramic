use super::*;
use crate::{MineralItem, beds::DyeColor, containers::ContainerGesture};

fn input(item: ItemKind, count: u16) -> Inventory {
    Inventory {
        slots: vec![ItemStack { item, count }],
        ..Default::default()
    }
}

#[test]
fn all_crushing_recipes_are_valid_unique_and_conserve_one_input_per_completion() {
    let mut ids = std::collections::HashSet::new();
    for recipe in recipes() {
        assert!(recipe.valid() && ids.insert(recipe.id));
        let mut state = CrusherState::default();
        let mut inventory = input(recipe.input, 2);
        for _ in 0..(recipe.seconds * 10.0).ceil() as usize {
            advance(&mut state, &mut inventory, 0.1);
        }
        assert_eq!(inventory.slot(INPUT).unwrap().count, 1, "{}", recipe.key);
        assert_eq!(
            inventory.slot(OUTPUT),
            Some(recipe.output),
            "{}",
            recipe.key
        );
        assert_eq!(state.completed, 1);
        assert!(valid_inventory(&inventory) && state.valid());
    }
    assert_eq!(recipes().len(), 20);
    assert_ne!(catalog_fingerprint(), 0);
}

#[test]
fn blocked_output_cannot_consume_input_and_changing_recipe_resets_progress() {
    let item = ItemKind::Mineral(MineralItem::RawIron);
    let mut state = CrusherState::default();
    let mut inventory = input(item, 2);
    advance(&mut state, &mut inventory, 1.0);
    assert_eq!(state.progress_seconds, 1.0);
    inventory.slots.push(ItemStack {
        item: ItemKind::Dust(DustKind::Gold),
        count: 1,
    });
    let before = inventory.clone();
    assert!(!advance(&mut state, &mut inventory, 1.0));
    assert_eq!(inventory, before);
    inventory.slots[OUTPUT] = ItemStack::EMPTY;
    inventory.slots[INPUT] = ItemStack {
        item: ItemKind::Flower(DyeColor::Blue),
        count: 1,
    };
    advance(&mut state, &mut inventory, 0.25);
    assert_eq!(state.progress_seconds, 0.25);
    assert_eq!(
        state.recipe,
        recipe_for(ItemKind::Flower(DyeColor::Blue)).map(|r| r.id)
    );
}

#[test]
fn invalid_elapsed_and_revision_overflow_are_atomic_and_huge_time_is_bounded() {
    let mut state = CrusherState::default();
    let mut inventory = input(ItemKind::Mineral(MineralItem::Coal), 128);
    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let before = (state.clone(), inventory.clone());
        assert!(!advance(&mut state, &mut inventory, dt));
        assert_eq!((state.clone(), inventory.clone()), before);
    }
    advance(&mut state, &mut inventory, 100_000.0);
    assert_eq!(state.progress_seconds, 1.0);
    inventory.revision = u64::MAX;
    let before = (state.clone(), inventory.clone());
    assert!(!advance(&mut state, &mut inventory, 1.0));
    assert_eq!((state, inventory), before);
}

#[test]
fn output_is_take_only_and_reserved_player_items_cannot_move() {
    let item = ItemKind::Flower(DyeColor::Green);
    let mut player = input(item, 5);
    let mut station = Inventory::default();
    let mut reserved = [0; INVENTORY_SLOTS];
    reserved[0] = 4;
    assert!(apply_gesture_reserved(
        &mut player,
        &mut station,
        ContainerGesture::QuickMove { slot: 0 },
        reserved
    ));
    assert_eq!(player.count(item), 4);
    assert_eq!(station.count(item), 1);
    player.cursor = Some(ItemStack {
        item: ItemKind::Dust(DustKind::Iron),
        count: 1,
    });
    let before = (player.clone(), station.clone());
    assert!(!apply_gesture_reserved(
        &mut player,
        &mut station,
        ContainerGesture::Click {
            slot: (INVENTORY_SLOTS + OUTPUT) as u8,
            secondary: false
        },
        [0; INVENTORY_SLOTS]
    ));
    assert_eq!((player.clone(), station.clone()), before);
    player.cursor = None;
    station.slots.resize(SLOTS, ItemStack::EMPTY);
    station.slots[OUTPUT] = ItemStack {
        item: ItemKind::Dust(DustKind::Iron),
        count: 3,
    };
    assert!(apply_gesture_reserved(
        &mut player,
        &mut station,
        ContainerGesture::Click {
            slot: (INVENTORY_SLOTS + OUTPUT) as u8,
            secondary: true
        },
        [0; INVENTORY_SLOTS]
    ));
    assert_eq!(player.cursor.unwrap().count, 2);
    assert_eq!(station.slot(OUTPUT).unwrap().count, 1);
}

#[test]
fn colored_flowers_round_trip_visual_variant_and_matching_pigment() {
    for color in DyeColor::FLOWER_COLORS {
        assert_eq!(flower_color(flower_variant(color)), color);
        let item = ItemKind::Flower(color);
        let recipe = recipe_for(item).unwrap();
        assert_eq!(recipe.output.item, ItemKind::Dye(color));
        assert!(item.valid() && ItemKind::Dye(color).valid());
    }
    for (variant, color) in [
        DyeColor::Red,
        DyeColor::Yellow,
        DyeColor::White,
        DyeColor::Blue,
        DyeColor::Pink,
        DyeColor::Purple,
        DyeColor::Orange,
        DyeColor::Green,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(flower_color(variant as u8), color);
    }
    for color in DyeColor::ALL {
        assert_eq!(
            ItemKind::Flower(color).valid(),
            DyeColor::FLOWER_COLORS.contains(&color)
        );
    }
}
