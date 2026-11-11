//! Server-owned names from the same asset metadata used by inventory tooltips.
use crate::ItemKind;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Default)]
struct Names {
    blocks: BTreeMap<u16, String>,
    items: BTreeMap<ItemKind, String>,
}

static NAMES: LazyLock<Names> = LazyLock::new(|| {
    #[derive(Deserialize)]
    struct Definition {
        id: Option<u16>,
        item: Option<ItemKind>,
        name: String,
    }
    let mut names = Names::default();
    for directory in ["terra/blocks", "terra/items"] {
        let Ok(entries) = std::fs::read_dir(crate::asset_directory().join(directory)) else {
            continue;
        };
        let mut paths: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        paths.sort();
        for path in paths {
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(definition) = serde_json::from_str::<Definition>(&source) else {
                continue;
            };
            if let Some(id) = definition.id {
                names.blocks.insert(id, definition.name.clone());
            }
            if let Some(item) = definition.item {
                names.items.insert(item, definition.name);
            }
        }
    }
    names
});

pub(super) fn preload() {
    LazyLock::force(&NAMES);
}

pub(super) fn display_name(item: ItemKind) -> String {
    match item {
        ItemKind::WornTool { tool, remaining } => format!(
            "{} ({remaining}/{})",
            display_name(ItemKind::Tool(tool)),
            tool.durability()
        ),
        ItemKind::Prop(kind) => kind.label().into(),
        ItemKind::Potion(id) => crate::brewing::recipe(id)
            .map_or_else(|| item.identifier(), |recipe| recipe.name.clone()),
        ItemKind::HeraldicCape { cape, .. } => display_name(ItemKind::Cape(cape)),
        ItemKind::Cape(cape) if cape.color != crate::beds::DyeColor::White => format!(
            "{} ({})",
            display_name(ItemKind::Cape(cape.undyed())),
            cape.color.code()
        ),
        ItemKind::FilledWateringCan { tool, units } => format!(
            "{} ({units}/{})",
            display_name(ItemKind::Tool(tool)),
            tool.watering_capacity()
        ),
        ItemKind::TintedBlock { color, .. } => format!(
            "{} ({})",
            display_name(
                crate::tints::canonical_item(item).expect("Tinted item has a canonical block")
            ),
            crate::beds::DyeColor::Rgb(color).code()
        ),
        ItemKind::DyedBed { material, color } => format!(
            "{} ({})",
            display_name(ItemKind::Block(material)),
            color.code()
        ),
        ItemKind::Block(id) | ItemKind::WoodenBlock { material: id, .. } => {
            let name = NAMES
                .blocks
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("Block {id}"));
            item.wood_species()
                .filter(|wood| {
                    *wood
                        != if id == crate::plumbing::BASIN {
                            crate::WoodSpecies::Spruce
                        } else {
                            crate::WoodSpecies::Oak
                        }
                })
                .map_or_else(|| name.clone(), |wood| format!("{name} ({})", wood.slug()))
        }
        ItemKind::Dye(crate::beds::DyeColor::Rgb(color)) => {
            format!("Mixed Dye {}", crate::beds::DyeColor::Rgb(color).code())
        }
        _ => NAMES.items.get(&item).cloned().unwrap_or_else(|| {
            item.identifier()
                .trim_start_matches("teramic::")
                .replace('_', " ")
        }),
    }
}
