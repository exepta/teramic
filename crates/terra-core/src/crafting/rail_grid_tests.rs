use super::*;

#[test]
fn each_rail_pattern_identifies_and_crafts_its_own_module_with_all_four_plank_species() {
    let book = RecipeBook::builtins();
    let iron = ItemKind::Mineral(crate::MineralItem::IronIngot);
    for (kind, costs) in crate::rails::Kind::ALL.into_iter().zip([
        (4, 4, 0),
        (8, 6, 0),
        (12, 10, 0),
        (4, 4, 0),
        (8, 8, 0),
        (8, 8, 0),
        (12, 10, 2),
        (12, 10, 2),
        (12, 10, 0),
        (4, 4, 0),
    ]) {
        let recipe = book
            .get(recipe_key(&format!("teramic::{}", kind.key())))
            .unwrap();
        let RecipePattern::Shaped {
            width,
            height,
            cells: pattern,
        } = &recipe.pattern
        else {
            panic!("rail modules need distinguishable workbench patterns");
        };
        assert_eq!((*width, *height, pattern.len()), (3, 3, 9));
        for plank in [16, 17, 18, 135] {
            let wood = ItemKind::Block(plank);
            let mut inventory = Inventory::default();
            assert_eq!(inventory.insert(iron, costs.0 * 2), 0);
            assert_eq!(inventory.insert(wood, costs.1 * 2), 0);
            if costs.2 != 0 {
                assert_eq!(inventory.insert(ItemKind::Stick, costs.2 * 2), 0);
            }
            let cells: [Option<CraftGridCell>; 9] = std::array::from_fn(|index| {
                pattern[index].map(|ingredient| {
                    let item = if ingredient.wood {
                        wood
                    } else {
                        ingredient.item
                    };
                    let slot = inventory
                        .slots
                        .iter()
                        .position(|stack| stack.item == item)
                        .unwrap();
                    CraftGridCell {
                        slot: slot as u8,
                        count: ingredient.count,
                    }
                })
            });
            let before = inventory.clone();
            assert_eq!(
                book.grid_match_reserved(&inventory, &cells, 3, CraftStation::Workbench)
                    .map(|matched| matched.0),
                Some(recipe.key()),
                "{}/{plank}: matching must not select another module",
                recipe.id
            );
            assert_eq!(
                book.output_for_reserved_grid(&inventory, &cells, 3, CraftStation::Workbench),
                Some(ItemStack {
                    item: ItemKind::Block(kind.material()),
                    count: 1
                })
            );
            assert_eq!(
                inventory, before,
                "grid inspection cannot mutate authoritative stacks"
            );
            assert!(!book.craft_reserved_grid(
                &mut inventory,
                &cells,
                3,
                CraftStation::Hand,
                1,
                CraftDestination::Inventory
            ));
            assert_eq!(inventory, before);
            assert!(book.craft_reserved_grid(
                &mut inventory,
                &cells,
                3,
                CraftStation::Workbench,
                1,
                CraftDestination::Inventory
            ));
            assert_eq!(inventory.count(iron), costs.0);
            assert_eq!(inventory.count(wood), costs.1);
            assert_eq!(inventory.count(ItemKind::Stick), costs.2);
            for module in crate::rails::MATERIALS {
                assert_eq!(
                    inventory.count(ItemKind::Block(module)),
                    u16::from(module == kind.material()),
                    "crafting a rail pattern must create only its requested module"
                );
            }
        }
    }
}
