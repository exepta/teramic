use super::*;
use crate::{WoodSpecies, beds::DyeColor};

#[test]
fn crafting_carpet_from_rgb_wool_does_not_erase_its_color() {
    let book = RecipeBook::builtins();
    let rgb = [71, 93, 184];
    let wool = crate::tints::recolored(ItemKind::Block(103), DyeColor::Rgb(rgb)).unwrap();
    let carpet = crate::tints::recolored(ItemKind::Block(106), DyeColor::Rgb(rgb)).unwrap();
    let mut bag = Inventory::default();
    bag.insert(wool, 2);
    assert!(book.craft_grid(
        &mut bag,
        &[Some(0), Some(0), None, None],
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Cursor
    ));
    assert_eq!(
        bag.cursor,
        Some(ItemStack {
            item: carpet,
            count: 3
        })
    );
    assert_eq!(bag.occupied_slots(), 0);
}

#[test]
fn exact_rgb_dye_crafts_each_fabric_family_and_preserves_only_the_consumed_belt_wood() {
    let book = RecipeBook::builtins();
    for rgb in [[0, 0, 0], [255, 255, 255], [73, 196, 218]] {
        let dye = ItemKind::Dye(DyeColor::Rgb(rgb));
        for input in [
            ItemKind::Block(103),
            ItemKind::Block(104),
            ItemKind::Block(105),
            ItemKind::Block(106),
            ItemKind::DyedBed {
                material: 72,
                color: DyeColor::Pink,
            },
            ItemKind::for_wood(148, WoodSpecies::Cherry),
        ] {
            let mut bag = Inventory::default();
            bag.insert(input, 2);
            bag.insert(dye, 2);
            bag.insert(ItemKind::Block(18), 128);
            let cells = [Some(0), Some(1), None, None];
            let expected = crate::tints::recolored(input, DyeColor::Rgb(rgb)).unwrap();
            assert_eq!(
                book.output_for_grid(&bag, &cells, 2, CraftStation::Hand),
                Some(ItemStack {
                    item: expected,
                    count: 1
                })
            );
            assert!(book.craft_grid(
                &mut bag,
                &cells,
                2,
                CraftStation::Hand,
                1,
                CraftDestination::Cursor
            ));
            assert_eq!(
                bag.cursor,
                Some(ItemStack {
                    item: expected,
                    count: 1
                })
            );
            assert_eq!(bag.count(input), 1);
            assert_eq!(bag.count(dye), 1);
            assert_eq!(bag.count(ItemKind::Block(18)), 128);
            assert_eq!(
                toml::from_str::<Inventory>(&toml::to_string(&bag).unwrap()).unwrap(),
                bag
            );
        }
    }
}

#[test]
fn rgb_fabrics_can_be_recolored_without_creating_catalog_entries() {
    let book = RecipeBook::builtins();
    let before = book.iter().count();
    for base in [103, 104, 105, 106, 148, 71, 72, 73] {
        let first = crate::tints::recolored(
            ItemKind::for_wood(base, WoodSpecies::Spruce),
            DyeColor::Rgb([1, 2, 3]),
        )
        .unwrap();
        let dye = ItemKind::Dye(DyeColor::Rgb([92, 134, 176]));
        let mut bag = Inventory::default();
        bag.insert(first, 1);
        bag.insert(dye, 1);
        let cells = [Some(0), None, None, Some(1)];
        assert!(book.craft_grid(
            &mut bag,
            &cells,
            2,
            CraftStation::Hand,
            1,
            CraftDestination::Cursor
        ));
        assert_eq!(
            crate::tints::color(bag.cursor.unwrap().item),
            Some([92, 134, 176])
        );
        assert_eq!(bag.occupied_slots(), 0);
    }
    assert_eq!(book.iter().count(), before);
}
