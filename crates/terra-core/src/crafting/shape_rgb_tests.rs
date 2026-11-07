use super::*;
use crate::beds::DyeColor;

#[test]
fn cutting_plain_white_wool_keeps_canonical_shapes_but_explicit_colors_keep_metadata() {
    let book = RecipeBook::builtins();
    for material in [104, 105] {
        let recipe = book
            .iter()
            .find(|recipe| {
                recipe.output.item == ItemKind::Block(material)
                    && matches!(recipe.pattern, RecipePattern::Shaped { .. })
            })
            .unwrap();
        for color in [
            None,
            Some(DyeColor::Rgb([255, 255, 255])),
            Some(DyeColor::Rgb([249, 248, 243])),
            Some(DyeColor::Red),
        ] {
            let source = color.map_or(ItemKind::Block(103), |color| {
                crate::tints::recolored(ItemKind::Block(103), color).unwrap()
            });
            let expected = color.map_or(ItemKind::Block(material), |color| {
                crate::tints::recolored(ItemKind::Block(material), color).unwrap()
            });
            let mut bag = Inventory::default();
            bag.insert(source, 64);
            assert!(
                book.craft(
                    &mut bag,
                    recipe.key(),
                    1,
                    CraftStation::Workbench,
                    CraftDestination::Cursor,
                )
                .is_ok()
            );
            assert_eq!(
                bag.cursor,
                Some(ItemStack {
                    item: expected,
                    count: recipe.output.count,
                }),
                "material={material}, color={color:?}"
            );
        }
    }
}

#[test]
fn cutting_rgb_wool_into_stairs_and_slabs_preserves_color_and_real_recipe_quantities() {
    let book = RecipeBook::builtins();
    let rgb = [29, 133, 211];
    let wool = crate::tints::recolored(ItemKind::Block(103), DyeColor::Rgb(rgb)).unwrap();
    for material in [104, 105] {
        let recipe = book
            .iter()
            .find(|recipe| {
                recipe.output.item == ItemKind::Block(material)
                    && matches!(recipe.pattern, RecipePattern::Shaped { .. })
            })
            .unwrap();
        let RecipePattern::Shaped { width, cells, .. } = &recipe.pattern else {
            unreachable!()
        };
        let mut bag = Inventory::default();
        bag.insert(wool, 64);
        let mut grid = [None; 9];
        for (index, ingredient) in cells.iter().enumerate() {
            if ingredient.is_some() {
                grid[index % usize::from(*width) + index / usize::from(*width) * 3] = Some(0);
            }
        }
        let expected =
            crate::tints::recolored(ItemKind::Block(material), DyeColor::Rgb(rgb)).unwrap();
        assert_eq!(
            book.output_for_grid(&bag, &grid, 3, CraftStation::Workbench),
            Some(ItemStack {
                item: expected,
                count: recipe.output.count
            })
        );
        assert!(book.craft_grid(
            &mut bag,
            &grid,
            3,
            CraftStation::Workbench,
            1,
            CraftDestination::Cursor
        ));
        assert_eq!(
            bag.cursor,
            Some(ItemStack {
                item: expected,
                count: recipe.output.count
            })
        );
    }
}

#[test]
fn all_standard_dyes_and_rgb_dyes_recolor_existing_wool_shapes_without_changing_material_ids() {
    let book = RecipeBook::builtins();
    for material in [104, 105] {
        for color in DyeColor::ALL
            .into_iter()
            .chain([DyeColor::Rgb([11, 22, 33])])
        {
            let input =
                crate::tints::recolored(ItemKind::Block(material), DyeColor::Rgb([99, 88, 77]))
                    .unwrap();
            let mut bag = Inventory::default();
            bag.insert(input, 1);
            bag.insert(ItemKind::Dye(color), 1);
            assert!(book.craft_grid(
                &mut bag,
                &[Some(0), None, None, Some(1)],
                2,
                CraftStation::Hand,
                1,
                CraftDestination::Cursor
            ));
            assert_eq!(
                bag.cursor.unwrap().item,
                crate::tints::recolored(input, color).unwrap()
            );
            assert_eq!(bag.occupied_slots(), 0);
        }
    }
}
