use super::*;
use crate::kingdom::{KingdomHeraldry, KingdomIcon};

fn fixture() -> (RecipeBook, Inventory, ItemKind, u64) {
    let cape = ItemKind::Cape(crate::CapeItem::ALL_WHITE[0]);
    let heraldry = KingdomHeraldry {
        color: [26, 111, 188],
        icon: KingdomIcon::Anchor,
    };
    let recipe = Recipe {
        id: "test::cape".into(),
        name: "Cape".into(),
        station: CraftStation::Hand,
        pattern: RecipePattern::Shapeless {
            ingredients: vec![Ingredient {
                item: ItemKind::Stick,
                count: 1,
                wood: false,
            }],
        },
        output: ItemStack {
            item: cape,
            count: 1,
        },
    };
    let key = recipe.key();
    let book = RecipeBook::from_recipes(vec![recipe])
        .unwrap()
        .with_cape_heraldry(Some(heraldry));
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Stick, 1);
    (
        book,
        inventory,
        cape.with_default_heraldry(Some(heraldry)),
        key,
    )
}

#[test]
fn every_crafting_entrypoint_resolves_a_concrete_heraldic_cape() {
    for path in 0..4 {
        let (book, mut inventory, expected, key) = fixture();
        let cells = [Some(0), None, None, None];
        assert_eq!(
            book.output_for_grid(&inventory, &cells, 2, CraftStation::Hand)
                .unwrap()
                .item,
            expected
        );
        let applied = match path {
            0 => book
                .craft(
                    &mut inventory,
                    key,
                    1,
                    CraftStation::Hand,
                    CraftDestination::Inventory,
                )
                .is_ok(),
            1 => book.craft_cells(&mut inventory, cells, CraftDestination::Inventory),
            2 => book.craft_grid(
                &mut inventory,
                &cells,
                2,
                CraftStation::Hand,
                1,
                CraftDestination::Inventory,
            ),
            _ => book.craft_reserved_grid(
                &mut inventory,
                &[Some(CraftGridCell { slot: 0, count: 1 }), None, None, None],
                2,
                CraftStation::Hand,
                1,
                CraftDestination::Inventory,
            ),
        };
        assert!(applied);
        assert_eq!(inventory.count(expected), 1);
        assert_eq!(
            crate::EquipmentSlot::for_item(expected),
            Some(crate::EquipmentSlot::Cape)
        );
        assert!(expected.valid());
        assert_eq!(expected.max_stack(), 1);
    }
}

#[test]
fn explicit_dye_including_white_keeps_manual_color_and_removes_the_emblem() {
    let heraldry = KingdomHeraldry {
        color: [44, 90, 130],
        icon: KingdomIcon::Crown,
    };
    let book = RecipeBook::builtins().with_cape_heraldry(Some(heraldry));
    let plain = ItemKind::Cape(crate::CapeItem::ALL_WHITE[0]);
    for color in [
        crate::beds::DyeColor::White,
        crate::beds::DyeColor::Purple,
        crate::beds::DyeColor::Rgb([3, 42, 118]),
    ] {
        let mut inventory = Inventory::default();
        inventory.insert(plain.with_default_heraldry(Some(heraldry)), 1);
        inventory.insert(ItemKind::Dye(color), 1);
        assert!(book.craft_grid(
            &mut inventory,
            &[Some(0), Some(1), None, None],
            2,
            CraftStation::Hand,
            1,
            CraftDestination::Inventory
        ));
        let expected = ItemKind::Cape(crate::CapeItem {
            color,
            ..crate::CapeItem::ALL_WHITE[0]
        });
        assert_eq!(inventory.count(expected), 1);
    }
}
