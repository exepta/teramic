use super::*;

#[test]
fn railway_modules_craft_atomically_at_a_workbench_with_every_plank_species() {
    let book = RecipeBook::builtins();
    for kind in crate::rails::Kind::ALL {
        let recipe = book
            .get(recipe_key(&format!("teramic::{}", kind.key())))
            .unwrap();
        assert_eq!(recipe.station, CraftStation::Workbench);
        assert_eq!(recipe.output.item, ItemKind::Block(kind.material()));
        assert_eq!(recipe.output.count, 1);
        for plank in [16, 17, 18, 135] {
            let mut inventory = Inventory::default();
            for ingredient in recipe.ingredients() {
                let item = if ingredient.wood {
                    ItemKind::Block(plank)
                } else {
                    ingredient.item
                };
                assert_eq!(inventory.insert(item, ingredient.count), 0);
            }
            let before = inventory.clone();
            assert!(
                book.craft(
                    &mut inventory,
                    recipe.key(),
                    1,
                    CraftStation::Hand,
                    CraftDestination::Cursor,
                )
                .is_err()
            );
            assert_eq!(inventory, before);
            book.craft(
                &mut inventory,
                recipe.key(),
                1,
                CraftStation::Workbench,
                CraftDestination::Cursor,
            )
            .unwrap();
            assert_eq!(inventory.cursor, Some(recipe.output));
            assert_eq!(inventory.occupied_slots(), 0);
            assert!(inventory.valid());
        }
    }
}

#[test]
fn all_hydraulic_parts_have_real_atomic_workbench_recipes() {
    let book = RecipeBook::builtins();
    for (slug, material) in [
        ("wood_pipe", 163),
        ("stone_pipe", 164),
        ("iron_pipe", 165),
        ("glass_pipe", 172),
        ("valve", 166),
        ("faucet", 167),
        ("pump", 168),
        ("basin", 169),
    ] {
        let recipe = book.get(recipe_key(&format!("teramic::{slug}"))).unwrap();
        assert_eq!(recipe.output.item, ItemKind::Block(material));
        let mut bag = Inventory::default();
        for ingredient in recipe.ingredients() {
            assert_eq!(bag.insert(ingredient.item, ingredient.count), 0);
        }
        book.craft(
            &mut bag,
            recipe.key(),
            1,
            recipe.station,
            CraftDestination::Cursor,
        )
        .unwrap();
        let expected = if material == 169 {
            ItemStack {
                item: ItemKind::for_wood(material, crate::WoodSpecies::Oak),
                ..recipe.output
            }
        } else {
            recipe.output
        };
        assert_eq!(bag.cursor, Some(expected));
        assert_eq!(bag.occupied_slots(), 0);
    }
}

#[test]
fn gearbox_recipes_are_registered_and_reorient_without_item_duplication() {
    let book = RecipeBook::builtins();
    assert!(
        book.get(recipe_key("teramic::gearbox_horizontal"))
            .is_some()
    );
    for (from, to) in [(154, 155), (155, 154)] {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(from), 1);
        assert!(book.craft_grid(
            &mut bag,
            &[Some(0), None, None, None],
            2,
            CraftStation::Hand,
            1,
            CraftDestination::Inventory
        ));
        assert_eq!(bag.count(ItemKind::Block(from)), 0);
        assert_eq!(bag.count(ItemKind::Block(to)), 1);
    }
}

#[test]
fn wooden_mechanic_recipes_inherit_consumed_planks_in_every_crafting_path() {
    use crate::WoodSpecies::{Birch, Cherry, Oak, Spruce};
    let book = RecipeBook::builtins();
    for (slug, material) in [
        ("wooden_gear", 141),
        ("wooden_shaft", 144),
        ("conveyor", 148),
        ("wooden_large_gear", 149),
    ] {
        let recipe = book.get(key(slug)).unwrap();
        assert_eq!(
            book.iter()
                .filter(|r| r.output.item.block_material() == Some(material))
                .count(),
            if material == 148 { 2 } else { 1 }
        );
        for (wood, plank) in [(Oak, 16), (Birch, 17), (Spruce, 18), (Cherry, 135)] {
            let component_wood = if wood == Oak { Cherry } else { Oak };
            let input = |ingredient: Ingredient| {
                if ingredient.wood {
                    ItemKind::Block(plank)
                } else if let ItemKind::Block(material) = ingredient.item
                    && ItemKind::supports_wood(material)
                {
                    ItemKind::for_wood(material, component_wood)
                } else {
                    ingredient.item
                }
            };
            let mut bag = Inventory::default();
            for ingredient in recipe.ingredients() {
                assert_eq!(bag.insert(input(ingredient), ingredient.count), 0);
            }
            let unused = ItemKind::Block(if plank == 18 { 135 } else { 18 });
            assert_eq!(bag.insert(unused, 128), 0);
            let RecipePattern::Shaped {
                width,
                cells: pattern,
                ..
            } = &recipe.pattern
            else {
                panic!("expected shaped mechanical recipe")
            };
            let mut cells = [None; 9];
            let mut reserved = [None; 9];
            for (index, ingredient) in pattern.iter().enumerate() {
                if let Some(ingredient) = ingredient {
                    let slot = bag
                        .slots
                        .iter()
                        .position(|stack| stack.item == input(*ingredient))
                        .unwrap() as u8;
                    let cell = index % usize::from(*width) + index / usize::from(*width) * 3;
                    cells[cell] = Some(slot);
                    reserved[cell] = Some(CraftGridCell {
                        slot,
                        count: ingredient.count,
                    });
                }
            }
            let expected = ItemStack {
                item: ItemKind::for_wood(material, wood),
                count: recipe.output.count,
            };
            assert_eq!(
                book.output_for_grid(&bag, &cells, 3, CraftStation::Workbench),
                Some(expected)
            );
            assert_eq!(
                book.output_for_reserved_grid(&bag, &reserved, 3, CraftStation::Workbench),
                Some(expected)
            );
            let before = bag.clone();
            let mut generic = bag.clone();
            book.craft(
                &mut generic,
                recipe.key(),
                1,
                CraftStation::Workbench,
                CraftDestination::Cursor,
            )
            .unwrap();
            let mut grid = bag.clone();
            assert!(book.craft_grid(
                &mut grid,
                &cells,
                3,
                CraftStation::Workbench,
                1,
                CraftDestination::Cursor
            ));
            let mut reserved_grid = bag.clone();
            assert!(book.craft_reserved_grid(
                &mut reserved_grid,
                &reserved,
                3,
                CraftStation::Workbench,
                1,
                CraftDestination::Cursor
            ));
            for result in [&generic, &grid, &reserved_grid] {
                assert_eq!(result.cursor, Some(expected), "{slug} {wood:?}");
                assert_eq!(result.count(unused), 128);
                assert_eq!(result.revision, before.revision + 1);
                assert_eq!(
                    toml::from_str::<Inventory>(&toml::to_string(result).unwrap()).unwrap(),
                    *result
                );
            }
            assert_eq!(bag, before);
            bag.cursor = Some(ItemStack {
                item: ItemKind::for_wood(material, component_wood),
                count: 1,
            });
            let before = bag.clone();
            assert!(!book.craft_reserved_grid(
                &mut bag,
                &reserved,
                3,
                CraftStation::Workbench,
                1,
                CraftDestination::Cursor
            ));
            assert_eq!(bag, before);
        }
    }
}

#[test]
fn wood_components_match_generic_recipes_without_overriding_plank_ties() {
    use crate::WoodSpecies::{Birch, Cherry, Oak, Spruce};
    for material in [28, 29, 141, 144, 148, 149] {
        let generic = Ingredient {
            item: ItemKind::Block(material),
            count: 1,
            wood: false,
        };
        let birch = Ingredient {
            item: ItemKind::for_wood(material, Birch),
            ..generic
        };
        for wood in [Oak, Birch, Spruce, Cherry] {
            let item = ItemKind::for_wood(material, wood);
            assert!(generic.matches(item));
            assert_eq!(birch.matches(item), wood == Birch);
        }
        assert!(!generic.matches(ItemKind::Block(3)));
        let mut inputs = WoodInputs::default();
        inputs.consume(ItemKind::Block(16), 2);
        inputs.consume(ItemKind::Block(17), 2);
        inputs.consume(ItemKind::for_wood(144, Birch), 1);
        let output = ItemStack {
            item: ItemKind::Block(material),
            count: 1,
        };
        assert_eq!(resolved_output(output, inputs), output);
        let mut inputs = WoodInputs::default();
        inputs.consume(ItemKind::for_wood(144, Cherry), 1);
        assert_eq!(
            resolved_output(output, inputs).item,
            ItemKind::for_wood(material, Cherry)
        );
    }
}

#[test]
fn every_non_oak_mechanical_catalog_item_has_matching_shipped_metadata() {
    for (slug, material) in [
        ("wooden_gear", 141),
        ("wooden_shaft", 144),
        ("conveyor", 148),
        ("wooden_large_gear", 149),
    ] {
        for wood in [
            crate::WoodSpecies::Birch,
            crate::WoodSpecies::Spruce,
            crate::WoodSpecies::Cherry,
        ] {
            let item = ItemKind::for_wood(material, wood);
            let path = crate::asset_directory()
                .join("terra/items")
                .join(format!("{}_{slug}.json", wood.slug()));
            let metadata: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(
                serde_json::from_value::<ItemKind>(metadata["item"].clone()).unwrap(),
                item
            );
            assert!(!metadata["name"].as_str().unwrap().is_empty());
            assert!(!metadata["description"].as_str().unwrap().is_empty());
        }
    }
}

#[test]
fn every_kinetic_part_has_a_workbench_recipe_and_atomic_inventory_transaction() {
    let book = RecipeBook::builtins();
    for (slug, material) in [
        ("wooden_large_gear", crate::kinetic::WOODEN_LARGE_GEAR),
        ("stone_large_gear", crate::kinetic::STONE_LARGE_GEAR),
        ("iron_large_gear", crate::kinetic::IRON_LARGE_GEAR),
        ("wooden_gear", crate::kinetic::WOODEN_GEAR),
        ("stone_gear", crate::kinetic::STONE_GEAR),
        ("iron_gear", crate::kinetic::IRON_GEAR),
        ("wooden_shaft", crate::kinetic::WOODEN_SHAFT),
        ("stone_shaft", crate::kinetic::STONE_SHAFT),
        ("iron_shaft", crate::kinetic::IRON_SHAFT),
        ("water_wheel", crate::kinetic::WATER_WHEEL),
        ("conveyor", crate::kinetic::CONVEYOR),
    ] {
        let recipe = book.get(key(slug)).unwrap();
        assert_eq!(recipe.station, CraftStation::Workbench);
        assert_eq!(recipe.output.item, ItemKind::Block(material));
        let mut inventory = Inventory::default();
        for ingredient in recipe.ingredients() {
            assert_eq!(inventory.insert(ingredient.item, ingredient.count), 0);
        }
        let before = inventory.clone();
        assert_eq!(
            book.craft(
                &mut inventory,
                recipe.key(),
                1,
                CraftStation::Hand,
                CraftDestination::Cursor
            ),
            Err(CraftError::WrongStation)
        );
        assert_eq!(inventory, before);
        book.craft(
            &mut inventory,
            recipe.key(),
            1,
            CraftStation::Workbench,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(inventory.cursor, Some(recipe.output));
        assert_eq!(inventory.occupied_slots(), 0);
    }
}

#[test]
fn village_building_blocks_are_real_obtainable_recipes_in_the_existing_book() {
    let book = RecipeBook::builtins();
    for (slug, material) in [
        ("path", 102),
        ("wool_block", 103),
        ("wool_stairs", 104),
        ("wool_slab", 105),
    ] {
        let recipe = book.get(key(slug)).unwrap();
        assert_eq!(recipe.output.item, ItemKind::Block(material));
        let mut bag = Inventory::default();
        for ingredient in recipe.ingredients() {
            assert_eq!(bag.insert(ingredient.item, ingredient.count), 0);
        }
        book.craft(
            &mut bag,
            recipe.key(),
            1,
            recipe.station,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(bag.cursor, Some(recipe.output));
        assert_eq!(bag.occupied_slots(), 0);
    }
    let catalog = crate::block_properties::BlockPropertiesCatalog::load(
        &crate::asset_directory().join("terra/blocks"),
    )
    .unwrap();
    assert_eq!(catalog.shape(102), crate::block_shapes::builtin_boxes(102));
    assert_eq!(catalog.shape(102)[0].maximum[1], 0.9375);
    assert_eq!(crate::block_shapes::builtin_boxes(104).len(), 3);
    assert_eq!(crate::block_shapes::builtin_boxes(105)[0].maximum[1], 0.5);
}

#[test]
fn all_palette_dyes_have_one_catalog_item_and_a_working_data_defined_recipe() {
    let book = RecipeBook::builtins();
    for color in crate::beds::DyeColor::ALL {
        let item = ItemKind::Dye(color);
        assert_eq!(
            ItemKind::EXTRA
                .iter()
                .filter(|candidate| **candidate == item)
                .count(),
            1
        );
        let recipe = book.get(key(&format!("{}_dye", color.slug()))).unwrap();
        assert_eq!(recipe.output.item, item);
        let mut inventory = Inventory::default();
        for ingredient in recipe.ingredients() {
            assert_eq!(inventory.insert(ingredient.item, ingredient.count), 0);
        }
        book.craft(
            &mut inventory,
            recipe.key(),
            1,
            CraftStation::Hand,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(inventory.cursor, Some(recipe.output));
        assert_eq!(inventory.occupied_slots(), 0);
        let definition: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                crate::asset_directory()
                    .join("terra/items")
                    .join(format!("{}_dye.json", color.slug())),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_value::<ItemKind>(definition["item"].clone()).unwrap(),
            item
        );
    }
}

#[test]
fn all_tools_have_unique_catalog_entries_and_valid_shaped_station_recipes() {
    let book = RecipeBook::builtins();
    let mut slugs = std::collections::HashSet::new();
    for tool in crate::ToolKind::ALL {
        let item = ItemKind::Tool(tool);
        assert!(slugs.insert(tool.slug()));
        assert_eq!(
            ItemKind::EXTRA
                .iter()
                .filter(|candidate| **candidate == item)
                .count(),
            1
        );
        if tool == crate::ToolKind::WaterBucket {
            assert!(
                book.get(key(tool.slug())).is_none(),
                "water is collected, never crafted for free"
            );
            continue;
        }
        let recipe = book.get(key(tool.slug())).unwrap();
        assert_eq!(recipe.output.item, item);
        let station = if matches!(
            tool,
            crate::ToolKind::WoodSpoon | crate::ToolKind::StoneSpoon | crate::ToolKind::GoldSpoon
        ) {
            CraftStation::Hand
        } else {
            CraftStation::Workbench
        };
        assert_eq!(recipe.station, station);
        assert!(matches!(recipe.pattern, RecipePattern::Shaped { .. }));
        let mut inventory = Inventory::default();
        for ingredient in recipe.ingredients() {
            assert_eq!(inventory.insert(ingredient.item, ingredient.count), 0);
        }
        if station == CraftStation::Workbench {
            let before = inventory.clone();
            assert_eq!(
                book.craft(
                    &mut inventory,
                    recipe.key(),
                    1,
                    CraftStation::Hand,
                    CraftDestination::Cursor
                ),
                Err(CraftError::WrongStation)
            );
            assert_eq!(inventory, before);
        }
        book.craft(
            &mut inventory,
            recipe.key(),
            1,
            station,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(inventory.cursor, Some(recipe.output));
        assert_eq!(inventory.occupied_slots(), 0);
        let definition: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                crate::asset_directory()
                    .join("terra/items")
                    .join(format!("{}.json", tool.slug())),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_value::<ItemKind>(definition["item"].clone()).unwrap(),
            item
        );
    }
}

#[test]
fn beds_are_white_by_default_and_any_existing_fabric_can_be_recolored_in_a_two_by_two_grid() {
    let book = RecipeBook::builtins();
    for material in 71..=73 {
        assert_eq!(
            crate::beds::bed_color(material, 0),
            crate::beds::DyeColor::White
        );
        for color in crate::beds::DyeColor::ALL {
            for old in [
                ItemKind::Block(material),
                ItemKind::DyedBed {
                    material,
                    color: crate::beds::DyeColor::Blue,
                },
            ] {
                let mut bag = Inventory::default();
                bag.insert(old, 1);
                bag.insert(ItemKind::Dye(color), 1);
                assert!(book.craft_grid(
                    &mut bag,
                    &[Some(0), None, None, Some(1)],
                    2,
                    CraftStation::Hand,
                    1,
                    CraftDestination::Inventory
                ));
                assert_eq!(bag.count(ItemKind::DyedBed { material, color }), 1);
                assert_eq!(bag.count(ItemKind::Dye(color)), 0);
            }
        }
    }
}

#[test]
fn flower_dyes_mix_and_beds_require_their_own_wood_and_wool() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Prop(crate::ResourcePropKind::Flower), 1);
    assert!(book.craft_grid(
        &mut bag,
        &[Some(0), None, None, None],
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag.count(ItemKind::Dye(crate::beds::DyeColor::Red)), 2);
    for (material, wood) in [(71, 16), (72, 17), (73, 18)] {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Animal(crate::creatures::AnimalProduct::Wool), 3);
        bag.insert(ItemKind::Block(wood), 3);
        let cells = [
            Some(0),
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(1),
            None,
            None,
            None,
        ];
        assert!(!book.craft_grid(
            &mut bag,
            &cells,
            3,
            CraftStation::Hand,
            1,
            CraftDestination::Inventory
        ));
        assert!(book.craft_grid(
            &mut bag,
            &cells,
            3,
            CraftStation::Workbench,
            1,
            CraftDestination::Inventory
        ));
        assert_eq!(bag.count(ItemKind::Block(material)), 1);
        assert_eq!(bag.count(ItemKind::Block(wood)), 0);
    }
}

#[test]
fn legacy_flower_and_mixed_dye_recipes_preserve_harvested_flower_route() {
    let book = RecipeBook::builtins();
    for flower in crate::beds::DyeColor::FLOWER_COLORS {
        let mut bag = Inventory::default();
        let input = ItemKind::Flower(flower);
        let grass = ItemKind::Prop(crate::ResourcePropKind::Grass);
        let small_plant = ItemKind::Prop(crate::ResourcePropKind::SmallPlant);
        bag.insert(input, 6);
        bag.insert(grass, 3);
        bag.insert(small_plant, 3);
        let revision = bag.revision;
        // The primary dyes use flowers; the three secondary dyes mix those outputs.
        for (step, dye) in crate::beds::DyeColor::BED_COLORS[..6]
            .iter()
            .copied()
            .enumerate()
        {
            let batches = if matches!(
                dye,
                crate::beds::DyeColor::Yellow | crate::beds::DyeColor::Blue
            ) {
                2
            } else {
                1
            };
            book.craft(
                &mut bag,
                key(&format!("{}_dye", dye.slug())),
                batches,
                CraftStation::Hand,
                CraftDestination::Inventory,
            )
            .unwrap();
            assert_eq!(bag.count(input), [5, 3, 1, 1, 1, 1][step]);
            assert_eq!(bag.count(ItemKind::Dye(dye)), if step < 3 { 2 } else { 1 });
            assert_eq!(bag.count(grass), if step == 0 { 3 } else { 1 });
            assert_eq!(bag.count(small_plant), if step < 2 { 3 } else { 1 });
            assert_eq!(bag.revision, revision + step as u64 + 1);
            assert!(bag.cursor.is_none());
        }
        for dye in [
            crate::beds::DyeColor::Red,
            crate::beds::DyeColor::Yellow,
            crate::beds::DyeColor::Blue,
        ] {
            assert_eq!(bag.count(ItemKind::Dye(dye)), 0);
        }
    }
    let exact_flower = Ingredient {
        item: ItemKind::Flower(crate::beds::DyeColor::Red),
        count: 1,
        wood: false,
    };
    assert!(exact_flower.matches(ItemKind::Flower(crate::beds::DyeColor::Red)));
    assert!(!exact_flower.matches(ItemKind::Flower(crate::beds::DyeColor::Blue)));
    assert!(!exact_flower.matches(ItemKind::Prop(crate::ResourcePropKind::Flower)));
}

#[test]
fn torch_is_one_stick_and_one_coal_in_the_real_hand_crafting_grid() {
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Stick, 1);
    bag.insert(ItemKind::Mineral(crate::MineralItem::Coal), 1);
    assert!(RecipeBook::builtins().craft_grid(
        &mut bag,
        &[Some(0), Some(1), None, None],
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag.count(ItemKind::Block(40)), 1);
    assert_eq!(bag.count(ItemKind::Stick), 0);
    assert_eq!(bag.count(ItemKind::Mineral(crate::MineralItem::Coal)), 0);
}

#[test]
fn spruce_planks_require_their_own_log_and_never_consume_oak_or_birch() {
    let book = RecipeBook::builtins();
    let recipe = book
        .iter()
        .find(|r| r.id == "teramic::spruce_planks")
        .unwrap();
    let ingredient = recipe.ingredients()[0];
    assert!(ingredient.matches(ItemKind::Block(36)));
    assert!(!ingredient.matches(ItemKind::Block(6)));
    assert!(!ingredient.matches(ItemKind::Block(10)));
    assert_eq!(
        recipe.output,
        ItemStack {
            item: ItemKind::Block(18),
            count: 4
        }
    );
    assert_eq!(
        ItemKind::Block(36).wood_species(),
        Some(crate::WoodSpecies::Spruce)
    );
}

#[test]
fn three_by_three_pattern_is_required_and_repeated_stacks_consume_exactly_once() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 24);
    let mut cells = [Some(0); 9];
    cells[4] = None;
    let before = bag.clone();
    let mut wrong = cells;
    wrong[0] = None;
    assert!(!book.craft_grid(
        &mut bag,
        &wrong,
        3,
        CraftStation::Workbench,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag, before);
    assert!(!book.craft_grid(
        &mut bag,
        &cells,
        3,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert!(book.craft_grid(
        &mut bag,
        &cells,
        3,
        CraftStation::Workbench,
        2,
        CraftDestination::Cursor
    ));
    assert_eq!(bag.count(ItemKind::Block(16)), 8);
    assert_eq!(bag.cursor.unwrap().count, 2);
    assert_eq!(bag.revision, before.revision + 1);
    assert!(!book.craft_grid(
        &mut bag,
        &cells,
        3,
        CraftStation::Workbench,
        2,
        CraftDestination::Inventory
    ));
}

fn key(name: &str) -> u64 {
    recipe_key(&format!("teramic::{name}"))
}

#[test]
fn chests_and_tools_are_shaped_in_the_actual_json_assets() {
    for source in [
        include_str!("../../../../assets/crafting/chest.json"),
        include_str!("../../../../assets/crafting/wood_pickaxe.json"),
        include_str!("../../../../assets/crafting/stone_pickaxe.json"),
        include_str!("../../../../assets/crafting/stone_axe.json"),
        include_str!("../../../../assets/crafting/stone_shovel.json"),
        include_str!("../../../../assets/crafting/iron_pickaxe.json"),
    ] {
        let recipe: Recipe = serde_json::from_str(source).unwrap();
        assert!(
            matches!(recipe.pattern, RecipePattern::Shaped { .. }),
            "{}",
            recipe.id
        );
        recipe.validate().unwrap();
    }
}

#[test]
fn workbench_accepts_four_shapeless_planks_and_keeps_dominant_wood() {
    let recipe: Recipe =
        serde_json::from_str(include_str!("../../../../assets/crafting/workbench.json")).unwrap();
    assert!(matches!(recipe.pattern, RecipePattern::Shapeless { .. }));
    assert_eq!(recipe.ingredients()[0].count, 4);
    let book = RecipeBook::from_recipes(vec![recipe]).unwrap();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 1);
    bag.insert(ItemKind::Block(17), 3);
    for cells in [
        vec![Some(0), Some(1), Some(1), Some(1)],
        vec![
            Some(1),
            None,
            Some(0),
            None,
            Some(1),
            None,
            None,
            None,
            Some(1),
        ],
    ] {
        let mut next = bag.clone();
        assert!(book.craft_grid(
            &mut next,
            &cells,
            if cells.len() == 4 { 2 } else { 3 },
            CraftStation::Workbench,
            1,
            CraftDestination::Cursor
        ));
        assert_eq!(
            next.cursor.unwrap().item,
            ItemKind::for_wood(28, crate::WoodSpecies::Birch)
        );
        assert_eq!(next.count(ItemKind::Block(16)), 0);
        assert_eq!(next.count(ItemKind::Block(17)), 0);
    }
}

#[test]
fn every_builtin_shaped_recipe_rejects_truncation_and_accepts_only_fitting_offsets() {
    for recipe in RecipeBook::builtins().iter() {
        let RecipePattern::Shaped {
            width,
            height,
            cells: pattern,
        } = &recipe.pattern
        else {
            continue;
        };
        let width = usize::from(*width);
        let height = usize::from(*height);
        let isolated = RecipeBook::from_recipes(vec![recipe.clone()]).unwrap();
        let mut inventory = Inventory::default();
        for (index, ingredient) in pattern.iter().enumerate() {
            if let Some(ingredient) = ingredient {
                inventory.creative_set(
                    index,
                    ItemStack {
                        item: ingredient.item,
                        count: 128,
                    },
                );
            }
        }
        for size in [2, 3] {
            for y in 0..size {
                for x in 0..size {
                    let mut grid = vec![None; size * size];
                    for (index, ingredient) in pattern.iter().enumerate() {
                        let px = x + index % width;
                        let py = y + index / width;
                        if ingredient.is_some() && px < size && py < size {
                            grid[py * size + px] = Some(index as u8);
                        }
                    }
                    let fits = x + width <= size && y + height <= size;
                    assert_eq!(
                        isolated
                            .grid_match_in(&inventory, &grid, size, CraftStation::Workbench)
                            .is_some(),
                        fits,
                        "{} at {x},{y} in {size}x{size}",
                        recipe.id
                    );
                    if !fits {
                        let mut next = inventory.clone();
                        assert!(!isolated.craft_grid(
                            &mut next,
                            &grid,
                            size,
                            CraftStation::Workbench,
                            1,
                            CraftDestination::Cursor
                        ));
                        assert_eq!(next, inventory);
                    } else {
                        for index in 0..grid.len() {
                            let mut malformed = grid.clone();
                            if malformed[index].take().is_none() {
                                malformed[index] = grid.iter().flatten().copied().next();
                            }
                            assert!(
                                isolated
                                    .grid_match_in(
                                        &inventory,
                                        &malformed,
                                        size,
                                        CraftStation::Workbench
                                    )
                                    .is_none(),
                                "{} accepted changed hole/cell {index}",
                                recipe.id
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_single_bottom_right_plank_stack_cannot_become_a_workbench() {
    let recipe = RecipeBook::builtins()
        .get(key("workbench"))
        .unwrap()
        .clone();
    let book = RecipeBook::from_recipes(vec![recipe]).unwrap();
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(16), 128);
    for size in [2, 3] {
        let mut grid = vec![None; size * size];
        grid[size * size - 1] = Some(0);
        assert!(
            book.output_for_grid(&inventory, &grid, size, CraftStation::Workbench)
                .is_none()
        );
        let mut reserved = vec![None; size * size];
        reserved[size * size - 1] = Some(CraftGridCell {
            slot: 0,
            count: 128,
        });
        assert!(
            book.output_for_reserved_grid(&inventory, &reserved, size, CraftStation::Workbench)
                .is_none()
        );
        let before = inventory.clone();
        assert!(!book.craft_reserved_grid(
            &mut inventory,
            &reserved,
            size,
            CraftStation::Workbench,
            1,
            CraftDestination::Cursor
        ));
        assert_eq!(inventory, before);
    }
}

#[test]
fn shapeless_recipes_resolve_overlapping_groups_and_quantities_in_either_order() {
    let recipe = Recipe {
        id: "example::mixed_wood".into(),
        name: "Mixed wood".into(),
        station: CraftStation::Hand,
        pattern: RecipePattern::Shapeless {
            ingredients: vec![
                Ingredient {
                    item: ItemKind::Block(16),
                    count: 1,
                    wood: true,
                },
                Ingredient {
                    item: ItemKind::Block(16),
                    count: 2,
                    wood: false,
                },
            ],
        },
        output: ItemStack {
            item: ItemKind::Stick,
            count: 1,
        },
    };
    let book = RecipeBook::from_recipes(vec![recipe]).unwrap();
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(16), 2);
    inventory.insert(ItemKind::Block(17), 1);
    for grid in [
        [Some(0), None, None, Some(1)],
        [Some(1), None, None, Some(0)],
    ] {
        let matched = book
            .grid_match_in(&inventory, &grid, 2, CraftStation::Hand)
            .unwrap();
        assert_eq!(matched.1, vec![(0, 2), (1, 1)]);
        let mut next = inventory.clone();
        assert!(book.craft_grid(
            &mut next,
            &grid,
            2,
            CraftStation::Hand,
            1,
            CraftDestination::Cursor
        ));
        assert_eq!(next.count(ItemKind::Block(16)), 0);
        assert_eq!(next.count(ItemKind::Block(17)), 0);
        assert_eq!(
            next.cursor,
            Some(ItemStack {
                item: ItemKind::Stick,
                count: 1
            })
        );
    }
    inventory.slots[1] = ItemStack::EMPTY;
    assert!(
        book.grid_match_in(
            &inventory,
            &[Some(0), Some(0), None, None],
            2,
            CraftStation::Hand
        )
        .is_none()
    );
}

#[test]
fn malformed_shapes_are_rejected_while_internal_holes_remain_supported() {
    let ingredient = Some(Ingredient {
        item: ItemKind::Block(16),
        count: 1,
        wood: false,
    });
    let template = RecipeBook::builtins()
        .get(key("workbench"))
        .unwrap()
        .clone();
    for (width, height, cells) in [
        (0, 1, vec![ingredient]),
        (4, 1, vec![ingredient; 4]),
        (2, 2, vec![ingredient; 3]),
        (2, 2, vec![None; 4]),
        (2, 2, vec![None, None, ingredient, ingredient]),
        (2, 2, vec![ingredient, ingredient, None, None]),
        (2, 2, vec![None, ingredient, None, ingredient]),
        (2, 2, vec![ingredient, None, ingredient, None]),
    ] {
        let mut recipe = template.clone();
        recipe.pattern = RecipePattern::Shaped {
            width,
            height,
            cells,
        };
        assert!(RecipeBook::from_recipes(vec![recipe]).is_err());
    }
    let mut recipe = template;
    recipe.pattern = RecipePattern::Shaped {
        width: 3,
        height: 3,
        cells: vec![
            ingredient, ingredient, ingredient, ingredient, None, ingredient, ingredient,
            ingredient, ingredient,
        ],
    };
    assert!(RecipeBook::from_recipes(vec![recipe]).is_ok());
}

#[test]
fn reserved_shapeless_crafting_commits_the_validated_assignment_not_full_stack_availability() {
    let item = ItemKind::Block(3);
    let recipe = Recipe {
        id: "example::reserved_quantities".into(),
        name: "Reserved quantities".into(),
        station: CraftStation::Hand,
        pattern: RecipePattern::Shapeless {
            ingredients: vec![
                Ingredient {
                    item,
                    count: 1,
                    wood: false,
                },
                Ingredient {
                    item,
                    count: 2,
                    wood: false,
                },
            ],
        },
        output: ItemStack {
            item: ItemKind::Stick,
            count: 1,
        },
    };
    let book = RecipeBook::from_recipes(vec![recipe]).unwrap();
    let mut inventory = Inventory {
        slots: vec![ItemStack { item, count: 10 }; 2],
        ..Default::default()
    };
    let reserved = [
        Some(CraftGridCell { slot: 0, count: 2 }),
        Some(CraftGridCell { slot: 1, count: 1 }),
        None,
        None,
    ];
    assert_eq!(
        book.grid_match_reserved(&inventory, &reserved, 2, CraftStation::Hand)
            .unwrap()
            .1,
        vec![(0, 2), (1, 1)]
    );
    assert!(book.craft_reserved_grid(
        &mut inventory,
        &reserved,
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Cursor
    ));
    assert_eq!(inventory.slot(0).unwrap().count, 8);
    assert_eq!(inventory.slot(1).unwrap().count, 9);
    assert_eq!(
        inventory.cursor,
        Some(ItemStack {
            item: ItemKind::Stick,
            count: 1
        })
    );
    assert_eq!(inventory.revision, 1);
}
#[test]
fn builtin_recipes_are_valid_and_catalog_roundtrip_preserves_keys() {
    let book = RecipeBook::builtins();
    assert_eq!(book.error, None);
    assert_eq!(
        book.iter().count(),
        86 + crate::ArmorItem::ALL.len()
            + crate::AccessoryKind::ALL.len()
            + crate::CapeItem::ALL.len()
            + crate::CapeItem::ALL_WHITE.len()
            + 2
            + 39
            + 5
            + 16
            + 4
            + 2 * crate::beds::DyeColor::ALL.len()
            + 2
            + 11
            + 1
            + 2
            + 3
            + 1 // The kinetic RGB mixer's workbench recipe.
            + 3 // Belt dye, filtered funnel, and belt merger.
            + 2 // Existing wool stairs and slabs can be recolored with dye.
            + 2 // Filtered tunnel and three-output belt distributor.
            + 6 // Bottles, three spoons, cauldron and campfire.
            + 7 // Three pipes, valve, faucet, pump and modular basin.
            + 1 // The additional glass pipe uses its own real workbench recipe.
            + 2 // Hold platform and pipe-fed fluid infuser.
            + 2 // Iron lantern and hanging chain.
            + 2 // Apple saplings from whole fruit or two cores.
            + 10 // Modular three-block-wide railway pieces.
    );
    for id in [
        "lantern",
        "chain",
        "apple_sapling",
        "apple_sapling_from_core",
        "glass_pipe",
        "filtered_tunnel",
        "belt_distributor",
        "filtered_funnel",
        "belt_merger",
        "conveyor_dye",
        "mixer",
        "item_hopper",
        "item_importer",
        "gearbox_horizontal",
        "gearbox_vertical",
        "gearbox_horizontal_from_vertical",
        "furnace",
        "chimney_straight",
        "oak_door",
        "birch_door",
        "spruce_door",
        "oak_window",
        "birch_window",
        "spruce_window",
        "stone_bellows",
        "iron_bellows",
        "gold_bellows",
        "ruby_bellows",
    ] {
        assert!(book.get(key(id)).is_some(), "Missing builtin recipe {id}");
    }
    for (base, suffix) in [
        (crate::textiles::WHITE_WOOL, "wool"),
        (crate::textiles::WHITE_CARPET, "carpet"),
    ] {
        for color in crate::beds::DyeColor::ALL {
            let id = format!("{}_{}", color.slug(), suffix);
            let recipe = book.get(key(&id)).unwrap();
            assert_eq!(recipe.station, CraftStation::Hand);
            assert!(matches!(recipe.pattern, RecipePattern::Shapeless { .. }));
            assert_eq!(
                recipe.output,
                ItemStack {
                    item: ItemKind::Block(crate::textiles::dyed(base, color).unwrap()),
                    count: 1,
                }
            );
            let ingredients = recipe.ingredients();
            assert_eq!(ingredients.len(), 2);
            for item in [ItemKind::Block(base), ItemKind::Dye(color)] {
                assert!(ingredients.contains(&Ingredient {
                    item,
                    count: 1,
                    wood: false,
                }));
            }
        }
    }
    for (id, station, source, amount, output, count) in [
        (
            "carpet",
            CraftStation::Hand,
            crate::textiles::WHITE_WOOL,
            2,
            crate::textiles::WHITE_CARPET,
            3,
        ),
        (
            "flower_pot",
            CraftStation::Workbench,
            19,
            3,
            crate::flower_pot::MATERIAL,
            1,
        ),
    ] {
        let recipe = book.get(key(id)).unwrap();
        assert_eq!(recipe.station, station);
        assert!(matches!(recipe.pattern, RecipePattern::Shaped { .. }));
        assert_eq!(
            recipe.ingredients(),
            vec![Ingredient {
                item: ItemKind::Block(source),
                count: amount,
                wood: false,
            }]
        );
        assert_eq!(
            recipe.output,
            ItemStack {
                item: ItemKind::Block(output),
                count,
            }
        );
    }
    for item in crate::ArmorItem::ALL
        .map(ItemKind::Armor)
        .into_iter()
        .chain(crate::AccessoryKind::ALL.map(ItemKind::Accessory))
    {
        assert!(book.iter().any(|recipe| recipe.output.item == item));
    }
    let copy = RecipeBook::from_json(&book.to_json().unwrap()).unwrap();
    assert_eq!(copy.snapshot(), book.snapshot());
    let mut inventory = Inventory::default();
    inventory.insert(ItemKind::Block(6), 3);
    assert!(inventory.craft([Some(0), None, None, None]));
    assert_eq!(inventory.count(ItemKind::Block(16)), 4);
    assert!(inventory.craft([Some(1), None, None, None]));
    assert_eq!(inventory.count(ItemKind::Block(16)), 2);
    assert_eq!(inventory.count(ItemKind::Stick), 4);
}

#[test]
fn filtered_tunnel_and_distributor_recipes_charge_their_real_parts() {
    let book = RecipeBook::builtins();
    for (id, material, costs) in [
        (
            "filtered_tunnel",
            crate::logistics::FILTERED_TUNNEL,
            vec![
                (ItemKind::Block(crate::logistics::TUNNEL), 1),
                (ItemKind::Mineral(crate::MineralItem::IronIngot), 1),
                (ItemKind::Dust(crate::crusher::DustKind::Diamond), 1),
            ],
        ),
        (
            "belt_distributor",
            crate::logistics::BELT_DISTRIBUTOR,
            vec![
                (ItemKind::Block(crate::logistics::FILTERED_FUNNEL), 3),
                (ItemKind::Mineral(crate::MineralItem::IronIngot), 2),
            ],
        ),
    ] {
        let recipe = book.get(key(id)).unwrap();
        assert_eq!(recipe.station, CraftStation::Workbench);
        assert!(matches!(recipe.pattern, RecipePattern::Shapeless { .. }));
        let output = ItemStack {
            item: ItemKind::Block(material),
            count: 1,
        };
        assert_eq!(recipe.output, output);
        let ingredients = recipe.ingredients();
        assert_eq!(ingredients.len(), costs.len());
        let mut bag = Inventory::default();
        for &(item, count) in &costs {
            assert!(ingredients.contains(&Ingredient {
                item,
                count,
                wood: false,
            }));
            assert_eq!(bag.insert(item, count + 1), 0);
        }
        book.craft(
            &mut bag,
            key(id),
            1,
            CraftStation::Workbench,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(bag.cursor, Some(output));
        for (item, _) in costs {
            assert_eq!(bag.count(item), 1, "{id} cost for {item:?}");
        }
    }
}

#[test]
fn batches_are_atomic_preserve_cursor_and_increment_revision_once() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 20);
    let revision = bag.revision;
    book.craft(
        &mut bag,
        key("sticks"),
        3,
        CraftStation::Hand,
        CraftDestination::Inventory,
    )
    .unwrap();
    assert_eq!(bag.count(ItemKind::Block(16)), 14);
    assert_eq!(bag.count(ItemKind::Stick), 12);
    assert_eq!(bag.revision, revision + 1);
    let before = bag.clone();
    assert_eq!(
        book.craft(
            &mut bag,
            key("sticks"),
            8,
            CraftStation::Hand,
            CraftDestination::Inventory
        ),
        Err(CraftError::MissingIngredients)
    );
    assert_eq!(bag, before);
    bag.cursor = Some(ItemStack {
        item: ItemKind::Stick,
        count: 127,
    });
    let before = bag.clone();
    assert_eq!(
        book.craft(
            &mut bag,
            key("sticks"),
            1,
            CraftStation::Hand,
            CraftDestination::Cursor
        ),
        Err(CraftError::NoSpace)
    );
    assert_eq!(bag, before);
    assert_eq!(
        book.max_crafts(
            &bag,
            key("sticks"),
            CraftStation::Hand,
            CraftDestination::Cursor
        ),
        0
    );
    assert_eq!(
        book.max_crafts(
            &bag,
            key("sticks"),
            CraftStation::Hand,
            CraftDestination::Inventory
        ),
        7
    );
}

#[test]
fn workstation_requirements_full_bags_and_quantity_bounds_cannot_lose_resources() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 100);
    for slot in 1..crate::INVENTORY_SLOTS {
        bag.creative_set(
            slot,
            ItemStack {
                item: ItemKind::Block(3),
                count: 128,
            },
        );
    }
    let before = bag.clone();
    assert_eq!(
        book.craft(
            &mut bag,
            key("chest"),
            1,
            CraftStation::Hand,
            CraftDestination::Inventory
        ),
        Err(CraftError::WrongStation)
    );
    assert_eq!(
        book.craft(
            &mut bag,
            key("chest"),
            1,
            CraftStation::Workbench,
            CraftDestination::Inventory
        ),
        Err(CraftError::NoSpace)
    );
    for quantity in [0, 129, u16::MAX] {
        assert_eq!(
            book.craft(
                &mut bag,
                key("chest"),
                quantity,
                CraftStation::Workbench,
                CraftDestination::Inventory
            ),
            Err(CraftError::InvalidQuantity)
        );
    }
    assert_eq!(bag, before);
    book.craft(
        &mut bag,
        key("chest"),
        1,
        CraftStation::Workbench,
        CraftDestination::Cursor,
    )
    .unwrap();
    assert_eq!(
        bag.cursor.unwrap(),
        ItemStack {
            item: ItemKind::Block(29),
            count: 1
        }
    );
    assert_eq!(bag.count(ItemKind::Block(16)), 92);
}

#[test]
fn mod_shapes_have_holes_and_repeated_slots_consume_exact_amounts() {
    let mut book = RecipeBook::default();
    let recipe = Recipe {
        id: "example::diagonal".into(),
        name: "Diagonal clay".into(),
        station: CraftStation::Hand,
        pattern: RecipePattern::Shaped {
            width: 2,
            height: 2,
            cells: vec![
                Some(Ingredient {
                    wood: false,
                    item: ItemKind::Block(4),
                    count: 2,
                }),
                None,
                None,
                Some(Ingredient {
                    wood: false,
                    item: ItemKind::Block(4),
                    count: 1,
                }),
            ],
        },
        output: ItemStack {
            item: ItemKind::Block(19),
            count: 1,
        },
    };
    book.extend(vec![recipe.clone()]).unwrap();
    let copy = RecipeBook::from_json(&book.to_json().unwrap()).unwrap();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(4), 3);
    assert!(
        copy.grid_match(&bag, [Some(0), None, Some(0), None])
            .is_none()
    );
    assert!(copy.craft_cells(
        &mut bag,
        [Some(0), None, None, Some(0)],
        CraftDestination::Inventory
    ));
    assert_eq!(bag.count(ItemKind::Block(4)), 0);
    assert_eq!(bag.count(ItemKind::Block(19)), 1);
    let snapshot = book.snapshot();
    assert!(book.extend(vec![recipe]).is_err());
    assert_eq!(book.snapshot(), snapshot);
}

#[test]
fn tool_classes_and_saved_item_variants_are_stable() {
    use crate::ToolKind::*;
    assert_eq!(WoodPickaxe.speed(3), 2.0);
    assert_eq!(StonePickaxe.speed(3), 4.0);
    assert_eq!(StoneAxe.speed(6), 4.0);
    assert_eq!(StoneShovel.speed(19), 4.0);
    assert_eq!(StoneAxe.speed(3), 1.0);
    assert!(!ItemKind::Block(3).can_harvest(12));
    assert!(ItemKind::Tool(WoodPickaxe).can_harvest(12));
    let mut bag = Inventory::default();
    for item in ItemKind::EXTRA {
        bag.insert(item, 1);
    }
    assert_eq!(
        toml::from_str::<Inventory>(&toml::to_string(&bag).unwrap()).unwrap(),
        bag
    );
}

#[test]
fn mixed_wood_is_one_recipe_and_dominant_consumed_species_survives_stacking() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 2);
    bag.insert(ItemKind::Block(17), 4);
    bag.insert(ItemKind::Block(18), 80);
    let cells = [Some(0), Some(1), Some(1), Some(1)];
    let expected = ItemKind::for_wood(28, crate::WoodSpecies::Birch);
    assert_eq!(
        book.output_for_grid(&bag, &cells, 2, CraftStation::Hand)
            .unwrap()
            .item,
        expected
    );
    assert!(book.craft_grid(
        &mut bag,
        &cells,
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag.count(expected), 1);
    assert_eq!(bag.count(ItemKind::Block(18)), 80);
    assert_eq!(
        book.iter()
            .filter(|r| r.output.item.block_material() == Some(28))
            .count(),
        1
    );
    assert_eq!(
        book.iter()
            .filter(|r| r.output.item.block_material() == Some(29))
            .count(),
        1
    );
    assert_eq!(
        toml::from_str::<Inventory>(&toml::to_string(&bag).unwrap()).unwrap(),
        bag
    );
}

#[test]
fn reserved_grid_limits_crafts_and_rejects_duplicate_reservation_without_mutation() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(18), 24);
    let cells = [
        Some(CraftGridCell { slot: 0, count: 2 }),
        Some(CraftGridCell { slot: 0, count: 4 }),
        Some(CraftGridCell { slot: 0, count: 4 }),
        Some(CraftGridCell { slot: 0, count: 2 }),
    ];
    assert_eq!(
        book.output_for_reserved_grid(&bag, &cells, 2, CraftStation::Hand)
            .unwrap()
            .item,
        ItemKind::for_wood(28, crate::WoodSpecies::Spruce)
    );
    let before = bag.clone();
    assert!(!book.craft_reserved_grid(
        &mut bag,
        &cells,
        2,
        CraftStation::Hand,
        3,
        CraftDestination::Inventory
    ));
    assert_eq!(bag, before);
    let invalid = [Some(CraftGridCell { slot: 0, count: 12 }); 4];
    assert!(
        book.grid_match_reserved(&bag, &invalid, 2, CraftStation::Hand)
            .is_none()
    );
    assert!(!book.craft_reserved_grid(
        &mut bag,
        &invalid,
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag, before);
    assert!(book.craft_reserved_grid(
        &mut bag,
        &cells,
        2,
        CraftStation::Hand,
        2,
        CraftDestination::Cursor
    ));
    assert_eq!(
        bag.cursor,
        Some(ItemStack {
            item: ItemKind::for_wood(28, crate::WoodSpecies::Spruce),
            count: 2
        })
    );
    assert_eq!(bag.count(ItemKind::Block(18)), 16);
    assert_eq!(bag.revision, before.revision + 1);
}

#[test]
fn wood_ties_are_deterministic_and_do_not_depend_on_other_bag_items() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(16), 3);
    bag.insert(ItemKind::Block(17), 3);
    bag.insert(ItemKind::Block(18), 128);
    assert!(book.craft_grid(
        &mut bag,
        &[Some(0), Some(1), Some(0), Some(1)],
        2,
        CraftStation::Hand,
        1,
        CraftDestination::Inventory
    ));
    assert_eq!(bag.count(ItemKind::Block(28)), 1);
    assert_eq!(bag.count(ItemKind::Block(18)), 128);
}

#[test]
fn dominant_wood_results_do_not_merge_into_a_different_wood_cursor() {
    let book = RecipeBook::builtins();
    let mut bag = Inventory::default();
    bag.insert(ItemKind::Block(18), 24);
    bag.cursor = Some(ItemStack {
        item: ItemKind::Block(28),
        count: 1,
    });
    let before = bag.clone();
    assert_eq!(
        book.max_crafts(
            &bag,
            key("workbench"),
            CraftStation::Hand,
            CraftDestination::Cursor
        ),
        0
    );
    assert_eq!(
        book.craft(
            &mut bag,
            key("workbench"),
            1,
            CraftStation::Hand,
            CraftDestination::Cursor
        ),
        Err(CraftError::NoSpace)
    );
    assert_eq!(bag, before);
    bag.cursor = Some(ItemStack {
        item: ItemKind::for_wood(28, crate::WoodSpecies::Spruce),
        count: 127,
    });
    assert_eq!(
        book.max_crafts(
            &bag,
            key("workbench"),
            CraftStation::Hand,
            CraftDestination::Cursor
        ),
        1
    );
    book.craft(
        &mut bag,
        key("workbench"),
        1,
        CraftStation::Hand,
        CraftDestination::Cursor,
    )
    .unwrap();
    assert_eq!(bag.cursor.unwrap().count, 128);
    assert_eq!(bag.count(ItemKind::Block(18)), 20);
}

#[test]
fn basin_recipe_inherits_each_consumed_plank_species() {
    let book = RecipeBook::builtins();
    let recipe = book.get(recipe_key("teramic::basin")).unwrap();
    for (wood, plank) in [
        (crate::WoodSpecies::Oak, 16),
        (crate::WoodSpecies::Birch, 17),
        (crate::WoodSpecies::Spruce, 18),
        (crate::WoodSpecies::Cherry, 135),
    ] {
        let mut inventory = Inventory::default();
        for ingredient in recipe.ingredients() {
            inventory.insert(
                if ingredient.wood {
                    ItemKind::Block(plank)
                } else {
                    ingredient.item
                },
                ingredient.count,
            );
        }
        book.craft(
            &mut inventory,
            recipe.key(),
            1,
            recipe.station,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(
            inventory.cursor.unwrap().item,
            ItemKind::for_wood(169, wood)
        );
        assert_eq!(inventory.occupied_slots(), 0);
    }
}

#[test]
fn whole_apples_and_consumed_cores_are_real_sapling_recipe_ingredients() {
    for (slug, input, count) in [
        ("apple_sapling", ItemKind::Apple, 1),
        ("apple_sapling_from_core", ItemKind::AppleCore, 2),
    ] {
        let book = RecipeBook::builtins();
        let recipe = book.get(recipe_key(&format!("teramic::{slug}"))).unwrap();
        assert_eq!(recipe.station, CraftStation::Hand);
        assert_eq!(
            recipe.output.item,
            ItemKind::Prop(crate::ResourcePropKind::AppleSapling)
        );
        let mut bag = Inventory::default();
        bag.insert(input, count);
        bag.insert(ItemKind::Block(2), 1);
        book.craft(
            &mut bag,
            recipe.key(),
            1,
            CraftStation::Hand,
            CraftDestination::Cursor,
        )
        .unwrap();
        assert_eq!(bag.cursor.unwrap().item, recipe.output.item);
        assert_eq!(bag.count(input), 0);
        assert_eq!(bag.count(ItemKind::Block(2)), 0);
    }
    let food = crate::food::food(ItemKind::Apple).unwrap();
    assert!(food.nutrition > 0. && food.saturation > 0.);
    assert_eq!(food.remainder, Some(ItemKind::AppleCore));
    assert!(crate::food::food(ItemKind::AppleCore).is_none());
}
