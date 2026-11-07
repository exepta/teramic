use super::*;

#[test]
fn actual_grid_crafts_each_ballast_form_once_without_recipe_aliasing_or_item_loss() {
    let book = RecipeBook::builtins();
    for (name, material, output, ingredient, consumed) in [
        ("ballast", 206, 4, 42, 4),
        ("ballast_stairs", 207, 4, 206, 6),
        ("ballast_slab", 208, 6, 206, 3),
    ] {
        let recipe = book.get(recipe_key(&format!("teramic::{name}"))).unwrap();
        let RecipePattern::Shaped {
            width,
            height,
            cells,
        } = &recipe.pattern
        else {
            panic!("ballast workbench pattern");
        };
        let mut inventory = Inventory::default();
        assert_eq!(
            inventory.insert(ItemKind::Block(ingredient), consumed * 2),
            0
        );
        let slot = inventory
            .slots
            .iter()
            .position(|stack| stack.item == ItemKind::Block(ingredient))
            .unwrap() as u8;
        let grid: [Option<CraftGridCell>; 9] = std::array::from_fn(|index| {
            let (x, y) = (index % 3, index / 3);
            (x < usize::from(*width) && y < usize::from(*height))
                .then(|| cells[y * usize::from(*width) + x])
                .flatten()
                .map(|item| CraftGridCell {
                    slot,
                    count: item.count,
                })
        });
        let before = inventory.clone();
        assert_eq!(
            book.grid_match_reserved(&inventory, &grid, 3, CraftStation::Workbench)
                .map(|value| value.0),
            Some(recipe.key())
        );
        assert_eq!(
            book.output_for_reserved_grid(&inventory, &grid, 3, CraftStation::Workbench),
            Some(ItemStack {
                item: ItemKind::Block(material),
                count: output
            })
        );
        assert!(!book.craft_reserved_grid(
            &mut inventory,
            &grid,
            3,
            CraftStation::Hand,
            1,
            CraftDestination::Inventory
        ));
        assert_eq!(inventory, before);
        assert!(book.craft_reserved_grid(
            &mut inventory,
            &grid,
            3,
            CraftStation::Workbench,
            1,
            CraftDestination::Inventory
        ));
        assert_eq!(inventory.count(ItemKind::Block(ingredient)), consumed);
        assert_eq!(inventory.count(ItemKind::Block(material)), output);
        assert!(inventory.valid());
    }
}
