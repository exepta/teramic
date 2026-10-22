use super::*;
use crate::{MineralItem, creatures::AnimalProduct};
fn stack(item: ItemKind, count: u16) -> ItemStack {
    ItemStack { item, count }
}
fn bag(bellows: Option<BellowsKind>) -> Inventory {
    Inventory {
        slots: vec![
            stack(ItemKind::Animal(AnimalProduct::Fish), 128),
            stack(ItemKind::Mineral(MineralItem::Coal), 128),
            ItemStack::EMPTY,
            bellows.map_or(ItemStack::EMPTY, |b| stack(ItemKind::Bellows(b), 1)),
        ],
        ..Default::default()
    }
}
#[test]
fn smelting_assets_are_valid_unique_and_fuels_are_wood_not_snow_or_clay() {
    assert_eq!(recipes().len(), 8);
    for recipe in recipes() {
        assert!(recipe.valid());
    }
    for id in [6, 10, 36, 16, 17, 18] {
        assert!(fuel_seconds(ItemKind::Block(id)).is_some());
    }
    for id in [3, 5, 9, 19, 39] {
        assert!(fuel_seconds(ItemKind::Block(id)).is_none());
    }
}
#[test]
fn cooks_exactly_one_and_preserves_fuel_and_progress_across_serialization() {
    let mut inventory = bag(None);
    let mut state = FurnaceState::default();
    for _ in 0..35 {
        advance(&mut state, &mut inventory, 0.1, 7);
    }
    let bytes = serde_json::to_string(&(state.clone(), inventory.clone())).unwrap();
    let (mut restored, mut contents): (FurnaceState, Inventory) =
        serde_json::from_str(&bytes).unwrap();
    for _ in 0..25 {
        advance(&mut state, &mut inventory, 0.1, 7);
        advance(&mut restored, &mut contents, 0.1, 7);
    }
    assert_eq!(restored, state);
    assert_eq!(contents, inventory);
    assert_eq!(inventory.slot(OUTPUT).unwrap().count, 1);
    assert_eq!(inventory.slot(INPUT).unwrap().count, 127);
    assert_eq!(inventory.slot(FUEL).unwrap().count, 127);
}
#[test]
fn all_mouse_gestures_reject_output_insertion_and_enforce_bellows_capacity() {
    let output = ItemKind::Animal(AnimalProduct::CookedFish);
    for gesture in [
        ContainerGesture::Click {
            slot: (INVENTORY_SLOTS + OUTPUT) as u8,
            secondary: false,
        },
        ContainerGesture::Drag {
            slots: vec![(INVENTORY_SLOTS + OUTPUT) as u8],
            secondary: false,
        },
        ContainerGesture::Wheel {
            slot: (INVENTORY_SLOTS + OUTPUT) as u8,
            to_cursor: false,
        },
    ] {
        let mut player = Inventory {
            cursor: Some(stack(output, 4)),
            ..Default::default()
        };
        let mut contents = Inventory::default();
        assert!(!apply_gesture(&mut player, &mut contents, gesture));
        assert_eq!(player.cursor.unwrap().count, 4);
    }
    let mut player = Inventory {
        cursor: Some(stack(ItemKind::Bellows(BellowsKind::Iron), 4)),
        ..Default::default()
    };
    let mut contents = Inventory::default();
    assert!(apply_gesture(
        &mut player,
        &mut contents,
        ContainerGesture::Click {
            slot: 30,
            secondary: false
        }
    ));
    assert_eq!(contents.slot(BELLOWS).unwrap().count, 1);
    assert_eq!(player.cursor.unwrap().count, 3);
    assert!(valid_inventory(&contents));
}
#[test]
fn full_or_wrong_output_never_consumes_input_or_fresh_fuel() {
    let mut inventory = bag(Some(BellowsKind::Ruby));
    inventory.slots[OUTPUT] = stack(ItemKind::Animal(AnimalProduct::CookedFish), 128);
    let mut state = FurnaceState::default();
    for _ in 0..100 {
        advance(&mut state, &mut inventory, 1.0, 9);
    }
    assert_eq!(inventory.slot(INPUT).unwrap().count, 128);
    assert_eq!(inventory.slot(FUEL).unwrap().count, 128);
    assert_eq!(state.completed, 0);
}
#[test]
fn bellows_speed_and_bonus_are_deterministic_and_do_not_overflow_stacks() {
    for (kind, expected) in [
        (None, 0),
        (Some(BellowsKind::Stone), 1),
        (Some(BellowsKind::Iron), 1),
        (Some(BellowsKind::Gold), 1),
        (Some(BellowsKind::Ruby), 1),
    ] {
        let mut inventory = bag(kind);
        let mut state = FurnaceState::default();
        for _ in 0..55 {
            advance(&mut state, &mut inventory, 0.1, 17);
        }
        assert_eq!(state.completed, expected);
    }
    let recipe = recipe_for(ItemKind::Animal(AnimalProduct::Fish)).unwrap();
    for kind in [BellowsKind::Gold, BellowsKind::Ruby] {
        let count = (0..10_000)
            .filter(|i| {
                result_count(
                    &FurnaceState {
                        completed: *i,
                        ..Default::default()
                    },
                    recipe,
                    Some(kind),
                    7,
                ) == 2
            })
            .count();
        assert!((count as f32 / 10_000.0 - kind.double_chance()).abs() < 0.015);
    }
}
#[test]
fn changing_recipe_resets_progress_and_exhausted_revisions_are_atomic() {
    let mut inventory = bag(None);
    let mut state = FurnaceState::default();
    advance(&mut state, &mut inventory, 1.0, 7);
    inventory.slots[INPUT] = stack(ItemKind::Mineral(MineralItem::RawIron), 1);
    advance(&mut state, &mut inventory, 0.1, 7);
    assert!((state.progress_seconds - 0.1).abs() < 0.00001);
    inventory.revision = u64::MAX;
    let before = (state.clone(), inventory.clone());
    assert!(!advance(&mut state, &mut inventory, 1.0, 7));
    assert_eq!((state, inventory), before);
}

#[test]
fn shortest_valid_recipe_and_fuel_keep_full_time_across_segment_boundaries() {
    let mut recipe = recipe_for(ItemKind::Animal(AnimalProduct::Fish))
        .unwrap()
        .clone();
    recipe.seconds = 0.5;
    assert!(recipe.valid());
    for (progress_seconds, burn_remaining) in [(0.0, 0.0), (0.499, 0.01)] {
        let initial = FurnaceState {
            recipe: Some(recipe.id),
            progress_seconds,
            burn_remaining,
            burn_total: 0.5,
            ..Default::default()
        };
        let mut whole = initial.clone();
        let mut segmented = initial;
        let mut whole_inventory = bag(Some(BellowsKind::Iron));
        let mut segmented_inventory = whole_inventory.clone();
        for _ in 0..10 {
            advance_with(
                &mut whole,
                &mut whole_inventory,
                1.0,
                7,
                Some(&recipe),
                Some(0.5),
            );
            for _ in 0..10 {
                advance_with(
                    &mut segmented,
                    &mut segmented_inventory,
                    0.1,
                    7,
                    Some(&recipe),
                    Some(0.5),
                );
            }
            assert_eq!(whole.completed, segmented.completed);
            assert_eq!(whole_inventory.slots, segmented_inventory.slots);
            assert!(
                (whole.progress_seconds - segmented.progress_seconds).abs() < 0.0001,
                "whole={whole:?}, segmented={segmented:?}"
            );
            assert!(
                (whole.burn_remaining - segmented.burn_remaining).abs() < 0.0001,
                "whole={whole:?}, segmented={segmented:?}"
            );
        }
    }
}
