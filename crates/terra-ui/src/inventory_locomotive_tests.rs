//! The authored locomotive catalog must reach native inventory and recipe controls.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(serde::Deserialize)]
struct CatalogBlock {
    id: u16,
    name: String,
    description: String,
    creative_category: Option<String>,
}

#[derive(serde::Deserialize)]
struct CatalogCategory {
    id: String,
    name: String,
    icon: ItemKind,
    order: i32,
}

#[derive(serde::Deserialize)]
struct CatalogItem {
    item: ItemKind,
    name: String,
    description: String,
}

fn authored_palette() -> (BlockPalette, BTreeMap<u16, String>) {
    let root = terra_core::asset_directory().join("terra");
    let mut palette = BlockPalette {
        choices: Vec::new(),
        ..default()
    };
    let mut names = BTreeMap::new();
    for entry in std::fs::read_dir(root.join("blocks")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let block: CatalogBlock =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        names.insert(block.id, block.name.clone());
        palette.item_categories.insert(
            ItemKind::Block(block.id),
            block.creative_category.unwrap_or_else(|| "terrain".into()),
        );
        palette.choices.push(BlockChoice {
            id: block.id,
            name: block.name,
            identifier: ItemKind::Block(block.id).identifier(),
            description: block.description,
        });
    }
    palette.choices.sort_by_key(|block| block.id);
    for entry in std::fs::read_dir(root.join("inventory/categories")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let category: CatalogCategory =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        palette
            .creative_categories
            .push(terra_player::interaction::CreativeCategory {
                id: category.id,
                name: category.name,
                icon: category.icon,
                order: category.order,
            });
    }
    palette
        .creative_categories
        .sort_by_key(|category| category.order);
    let mut registered = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("items")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let item: CatalogItem =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(
            registered.insert(item.item),
            "duplicate authored item metadata"
        );
        assert!(!item.name.is_empty() && !item.description.is_empty());
        palette
            .item_metadata
            .insert(item.item, (item.name, item.description));
    }
    assert_eq!(
        registered,
        ItemKind::EXTRA.into_iter().collect::<BTreeSet<_>>(),
        "the strict item metadata registry must cover the new canonical wood variants too"
    );
    (palette, names)
}

#[test]
fn authored_f54_items_page_through_tmi_with_real_names_previews_and_default_recipe_popups() {
    use terra_core::locomotive_parts::{FIRST, LAST};
    let (palette, names) = authored_palette();
    let expected: BTreeSet<_> = (FIRST..=LAST).map(ItemKind::Block).collect();
    let mut app = app(GameMode::Creative);
    app.insert_resource(palette);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    action(&mut app, InventoryAction::Toggle);
    action(&mut app, InventoryAction::Category("trains".into()));
    assert!(
        app.world()
            .resource::<InventoryView>()
            .categories
            .iter()
            .any(|category| category.id == "trains" && category.selected)
    );
    let mut discovered = BTreeMap::new();
    let pages = app.world().resource::<InventoryView>().browser_pages;
    for page in 0..pages {
        let view = app.world().resource::<InventoryView>();
        for item in &view.browser_items {
            if item.item.is_some_and(|item| expected.contains(&item)) {
                assert!(
                    discovered
                        .insert(item.item.unwrap(), item.clone())
                        .is_none(),
                    "each base item occurs once across actual catalog pages"
                );
            }
        }
        if page + 1 < pages {
            action(&mut app, InventoryAction::Page(1));
        }
    }
    assert_eq!(
        discovered.keys().copied().collect::<BTreeSet<_>>(),
        expected
    );
    for (item, entry) in &discovered {
        let material = item.block_material().unwrap();
        assert!(
            entry.name.starts_with(&names[&material]),
            "authored name missing for {material}"
        );
        assert!(!entry.description.is_empty());
        assert_eq!(entry.identifier, item.identifier());
        assert_eq!(entry.preview_key, format!("Block preview {material}"));
        assert_eq!(entry.category, "trains");
    }
    for material in [214, 215, 216, 217, 267, 268] {
        for wood in [
            terra_core::WoodSpecies::Oak,
            terra_core::WoodSpecies::Spruce,
            terra_core::WoodSpecies::Birch,
            terra_core::WoodSpecies::Cherry,
        ] {
            let item = ItemKind::for_wood(material, wood);
            assert!(
                available_items(
                    GameMode::Creative,
                    &before,
                    app.world().resource::<BlockPalette>()
                )
                .contains(&item),
                "wood variant missing: {}",
                item.identifier()
            );
            let view = view::item_view(item, 1, app.world().resource::<BlockPalette>());
            assert!(view.name.starts_with(&names[&material]));
            if wood != terra_core::WoodSpecies::Oak {
                assert!(view.name.contains(wood.slug()));
            }
            assert_eq!(view.category, "trains");
        }
    }
    for (item, entry) in discovered {
        action(&mut app, InventoryAction::Recipe(entry.item_id));
        let recipe = app.world().resource::<CraftingView>();
        assert!(
            recipe.recipe_open && recipe.selected,
            "missing default recipe for {}",
            item.identifier()
        );
        let output = &recipe.recipe_output[0];
        assert_eq!(
            output.item.and_then(ItemKind::block_material),
            item.block_material()
        );
        assert!(
            output
                .name
                .starts_with(&names[&item.block_material().unwrap()])
        );
        assert_eq!(output.preview_key, preview_key(output.item.unwrap()));
        assert!(
            recipe
                .recipe_categories
                .iter()
                .any(|category| category.id == "workbench" && category.selected)
        );
        assert!(!recipe.recipe_grid.is_empty());
    }
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert!(
        requests(&mut app).is_empty(),
        "browsing every locomotive recipe must never grant Creative items"
    );
}
