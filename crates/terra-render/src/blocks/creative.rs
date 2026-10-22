use super::{BlockLibrary, valid_category_id};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};
use terra_core::{ItemKind, ResourcePropKind};
use terra_player::interaction::CreativeCategory;

pub(crate) fn item_category(item: ItemKind, declared: Option<&str>) -> String {
    if let Some(category) = declared {
        match category {
            "terrain" | "wood" | "building" => return "building".into(),
            "nature" | "vegetation" | "decoration" => return "decoration".into(),
            "minerals" | "materials" => return "materials".into(),
            "animal_products" => {}
            _ => return category.into(),
        }
    }
    if terra_core::food(item).is_some() {
        return "food".into();
    }
    match item {
        ItemKind::Block(28 | 29 | 40 | 59..=73) | ItemKind::Prop(_) | ItemKind::DyedBed { .. } => {
            "decoration"
        }
        ItemKind::Block(_) | ItemKind::WoodenBlock { .. } => "building",
        ItemKind::Stick | ItemKind::Mineral(_) | ItemKind::Dye(_) => "materials",
        ItemKind::Animal(_) => "animal_materials",
        ItemKind::Tool(_) | ItemKind::Bellows(_) => "tools",
    }
    .into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryLibrary {
    schema_version: u32,
    categories: Vec<CategoryDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CategoryDefinition {
    id: String,
    name: String,
    icon: ItemKind,
    #[serde(default)]
    order: i32,
}

pub(crate) fn load_creative_categories(
    path: &Path,
    blocks: &BlockLibrary,
    item_categories: &HashMap<ItemKind, String>,
) -> Result<Vec<CreativeCategory>, String> {
    let definitions = if path.is_dir() {
        super::read_definitions::<CategoryDefinition>(path)?
    } else {
        match std::fs::read_to_string(path) {
            Ok(source) => {
                let library: CategoryLibrary = serde_json::from_str(&source)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                if library.schema_version != 1 || library.categories.len() > 128 {
                    return Err(format!(
                        "{}: unsupported category schema or too many categories",
                        path.display()
                    ));
                }
                library.categories
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    };
    if definitions.len() > 128 {
        return Err("Too many creative categories".into());
    }
    let mut seen = HashSet::new();
    let mut categories = Vec::new();
    for definition in definitions {
        let available_icon = match definition.icon {
            ItemKind::Block(id)
            | ItemKind::WoodenBlock { material: id, .. }
            | ItemKind::DyedBed { material: id, .. } => {
                blocks.blocks.iter().any(|block| block.id == id)
            }
            ItemKind::Prop(_)
            | ItemKind::Tool(_)
            | ItemKind::Stick
            | ItemKind::Mineral(_)
            | ItemKind::Bellows(_)
            | ItemKind::Dye(_)
            | ItemKind::Animal(_) => item_categories.contains_key(&definition.icon),
        };
        if !valid_category_id(&definition.id)
            || !seen.insert(definition.id.clone())
            || definition.name.trim().is_empty()
            || definition.name.len() > 128
            || definition.name.chars().any(char::is_control)
            || !available_icon
        {
            return Err(format!(
                "{}: invalid, duplicated, or unresolved creative category {}",
                path.display(),
                definition.id
            ));
        }
        categories.push(CreativeCategory {
            id: definition.id,
            name: definition.name,
            icon: definition.icon,
            order: definition.order,
        });
    }
    let mut inferred = BTreeMap::new();
    for item in blocks
        .blocks
        .iter()
        .map(|block| ItemKind::Block(block.id))
        .chain(
            [
                ResourcePropKind::Grass,
                ResourcePropKind::Flower,
                ResourcePropKind::Bush,
                ResourcePropKind::Fern,
                ResourcePropKind::Mushroom,
                ResourcePropKind::SmallPlant,
            ]
            .map(ItemKind::Prop),
        )
        .chain(ItemKind::EXTRA)
    {
        if let Some(id) = item_categories.get(&item) {
            inferred.entry(id.clone()).or_insert(item);
        }
    }
    for (id, icon) in inferred {
        if seen.insert(id.clone()) {
            let name = id
                .rsplit("::")
                .next()
                .unwrap_or(&id)
                .split(['_', '-'])
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    chars
                        .next()
                        .map(|first| format!("{}{}", first.to_ascii_uppercase(), chars.as_str()))
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
            categories.push(CreativeCategory {
                id,
                name,
                icon,
                order: 1000,
            });
        }
    }
    categories.sort_by(|left, right| (left.order, &left.id).cmp(&(right.order, &right.id)));
    if let Some(index) = categories.iter().position(|category| category.id == "all") {
        let all = categories.remove(index);
        categories.insert(0, all);
    }
    Ok(categories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(source: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "terra-categories-{}-{}.json",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&path, source).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn blocks() -> BlockLibrary {
        BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap()
    }

    #[test]
    fn shipped_categories_resolve_real_icons_and_every_registered_item() {
        let directory = terra_core::asset_directory().join("terra");
        let blocks = blocks();
        let mut items = super::super::load_prop_metadata(&directory.join("props"))
            .unwrap()
            .categories;
        for (item, category) in &mut items {
            *category = item_category(*item, Some(category));
        }
        items.extend(blocks.blocks.iter().map(|block| {
            (
                ItemKind::Block(block.id),
                item_category(
                    ItemKind::Block(block.id),
                    block.creative_category.as_deref(),
                ),
            )
        }));
        items.extend(
            super::super::load_tool_metadata(&directory.join("items"))
                .unwrap()
                .into_iter()
                .map(|(item, _, _)| (item, item_category(item, None))),
        );
        let categories =
            load_creative_categories(&directory.join("inventory/categories"), &blocks, &items)
                .unwrap();
        assert_eq!(
            categories
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            [
                "building",
                "decoration",
                "materials",
                "animal_materials",
                "food",
                "tools"
            ]
        );
        assert!(
            items
                .values()
                .all(|id| categories.iter().any(|category| &category.id == id))
        );
        assert!(
            categories
                .iter()
                .any(|category| matches!(category.icon, ItemKind::Prop(_)))
        );
    }

    #[test]
    fn namespaced_custom_categories_support_resource_pack_names_and_inferred_fallbacks() {
        let blocks = blocks();
        let items = HashMap::from([(ItemKind::Block(3), "example::ancient_ruins".into())]);
        let custom = Fixture::new(
            r#"{"schema_version":1,"categories":[{"id":"example::ancient_ruins","name":"Ancient Ruins","icon":{"block":3},"order":-10}]}"#,
        );
        let categories = load_creative_categories(&custom.0, &blocks, &items).unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].name, "Ancient Ruins");
        assert_eq!(categories[0].icon, ItemKind::Block(3));
        let empty = Fixture::new(r#"{"schema_version":1,"categories":[]}"#);
        let fallback = load_creative_categories(&empty.0, &blocks, &items).unwrap();
        assert_eq!(fallback[0].name, "Ancient Ruins");
        assert_eq!(fallback[0].icon, ItemKind::Block(3));
    }

    #[test]
    fn categories_keep_food_and_materials_separate() {
        use terra_core::{MineralItem, ToolKind, creatures::AnimalProduct};
        assert_eq!(
            item_category(ItemKind::Animal(AnimalProduct::Leather), None),
            "animal_materials"
        );
        assert_eq!(
            item_category(ItemKind::Animal(AnimalProduct::CookedBeef), None),
            "food"
        );
        assert_eq!(
            item_category(ItemKind::Mineral(MineralItem::IronIngot), None),
            "materials"
        );
        assert_eq!(item_category(ItemKind::Stick, None), "materials");
        assert_eq!(
            item_category(ItemKind::Tool(ToolKind::StonePickaxe), None),
            "tools"
        );
        assert_eq!(item_category(ItemKind::Block(16), Some("wood")), "building");
        assert_eq!(
            item_category(ItemKind::Block(16), Some("custom::wood")),
            "custom::wood"
        );
    }

    #[test]
    fn malformed_duplicate_and_unresolved_categories_fail_instead_of_fake_icons() {
        let blocks = blocks();
        for source in [
            r#"{"schema_version":1,"categories":[{"id":"bad name","name":"Bad","icon":{"block":3}}]}"#,
            r#"{"schema_version":1,"categories":[{"id":"custom","name":"Missing","icon":{"block":65535}}]}"#,
            r#"{"schema_version":1,"categories":[{"id":"custom","name":"A","icon":{"block":3}},{"id":"custom","name":"B","icon":{"block":2}}]}"#,
        ] {
            let fixture = Fixture::new(source);
            assert!(
                load_creative_categories(&fixture.0, &blocks, &HashMap::new()).is_err(),
                "{source}"
            );
        }
    }
}
