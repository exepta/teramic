use crate::{ItemKind, ItemStack, WoodSpecies, construction};

#[test]
fn cherry_joinery_preserves_mixed_slab_codes_and_all_twenty_four_stair_poses() {
    for material in [136, 137, 138] {
        assert_eq!(construction::base_material(material), Some(135));
    }
    for slab in construction::SLAB_MATERIALS {
        for facing in [0, 4, 5, 6, 7, 8] {
            let code = construction::merged_slab(slab, facing, 137).unwrap();
            assert!((64..=69).contains(&code));
            let halves = construction::slab_halves(slab, code).unwrap();
            assert_eq!(halves[0], (slab, facing));
            assert_eq!(halves[1].0, 137);
        }
    }
    for facing in 0..24 {
        assert!(construction::valid_facing(136, facing));
        assert!(!construction::placed_boxes(136, facing, [None; 6]).is_empty());
    }
    assert_eq!(
        ItemKind::Block(134).wood_species(),
        Some(WoodSpecies::Cherry)
    );
    assert_eq!(
        ItemKind::Block(135).wood_species(),
        Some(WoodSpecies::Cherry)
    );
    for material in [139, 140] {
        assert!(!crate::block_shapes::builtin_boxes(material).is_empty());
        assert!(
            crate::block_shapes::builtin_boxes(material)
                .iter()
                .all(|b| b.valid_for(material))
        );
    }
}

#[test]
fn cherry_recipes_are_available_and_any_wood_recipes_keep_the_species() {
    let book = crate::crafting::RecipeBook::builtins();
    assert_eq!(book.error, None);
    for name in ["planks", "stairs", "slab", "fence", "door", "window"] {
        assert!(
            book.iter()
                .any(|r| r.id == format!("teramic::cherry_{name}"))
        );
    }
    let mut bag = crate::Inventory::default();
    bag.creative_set(
        0,
        ItemStack {
            item: ItemKind::Block(135),
            count: 20,
        },
    );
    assert!(
        crate::crafting::Ingredient {
            item: ItemKind::Block(16),
            count: 1,
            wood: true
        }
        .matches(ItemKind::Block(135))
    );
    assert_eq!(
        crate::furnace::fuel_seconds(ItemKind::Block(134)),
        Some(20.)
    );
    assert_eq!(
        crate::furnace::fuel_seconds(ItemKind::Block(135)),
        Some(15.)
    );
}

#[test]
fn buckets_have_real_item_identity_and_a_valid_iron_recipe() {
    for tool in [crate::ToolKind::Bucket, crate::ToolKind::WaterBucket] {
        let item = ItemKind::Tool(tool);
        assert!(item.valid());
        assert_eq!(item.max_stack(), 1);
        assert!(ItemKind::EXTRA.contains(&item));
    }
    let recipe = crate::crafting::RecipeBook::builtins()
        .iter()
        .find(|r| r.id == "teramic::bucket")
        .unwrap();
    assert_eq!(recipe.output.item, ItemKind::Tool(crate::ToolKind::Bucket));
}
