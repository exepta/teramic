use super::*;

#[test]
fn common_track_pattern_identifies_and_crafts_the_actual_plank_species() {
    let book = RecipeBook::builtins();
    let iron = ItemKind::Mineral(crate::MineralItem::IronIngot);
    let iron_cost = 4;
    let wood_cost = 4;
    let recipe = book.get(recipe_key("teramic::rail_straight")).unwrap();
    let RecipePattern::Shaped {
        width,
        height,
        cells: pattern,
    } = &recipe.pattern
    else {
        panic!("track crafting needs its workbench pattern");
    };
    assert_eq!((*width, *height, pattern.len()), (3, 3, 9));
    for plank in [16, 17, 18, 135] {
        let wood = ItemKind::Block(plank);
        let mut inventory = Inventory::default();
        assert_eq!(inventory.insert(iron, iron_cost * 2), 0);
        assert_eq!(inventory.insert(wood, wood_cost * 2), 0);
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
            "{}/{plank}: matching must select the common track",
            recipe.id
        );
        assert_eq!(
            book.output_for_reserved_grid(&inventory, &cells, 3, CraftStation::Workbench),
            Some(ItemStack {
                item: ItemKind::for_wood(196, wood.wood_species().unwrap()),
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
        assert_eq!(inventory.count(iron), iron_cost);
        assert_eq!(inventory.count(wood), wood_cost);
        assert_eq!(inventory.count(ItemKind::Stick), 0);
        for species in [
            crate::WoodSpecies::Oak,
            crate::WoodSpecies::Birch,
            crate::WoodSpecies::Spruce,
            crate::WoodSpecies::Cherry,
        ] {
            assert_eq!(
                inventory.count(ItemKind::for_wood(196, species)),
                u16::from(species == wood.wood_species().unwrap())
            );
        }
        for module in crate::rails::MATERIALS.into_iter().skip(1) {
            assert_eq!(inventory.count(ItemKind::Block(module)), 0);
        }
    }
}
