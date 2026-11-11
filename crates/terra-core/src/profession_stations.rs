//! Stable profession depots. A profession marker does not invent an automated job.
use crate::{block_shapes::ShapeBox, npc_jobs::WorkKind};
use serde::Deserialize;
use std::sync::OnceLock;

pub const LUMBERJACK: u16 = 297;
pub const SMITH: u16 = 298;
pub const MARKET: u16 = 299;
pub const ARMY: u16 = 300;
pub const RAILWAY: u16 = 301;
pub const MINE: u16 = 302;
pub const BUTCHER: u16 = 303;
pub const BISHOP: u16 = 304;
pub const FARM: u16 = 305;
pub const ALL: [u16; 9] = [
    LUMBERJACK, SMITH, MARKET, ARMY, RAILWAY, MINE, BUTCHER, BISHOP, FARM,
];

pub const fn is_station(material: u16) -> bool {
    matches!(material, LUMBERJACK..=FARM)
}

pub const fn is_depot(material: u16) -> bool {
    matches!(material, 29 | crate::train_parts::CARGO) || is_station(material)
}

pub fn supports_work(material: u16, kind: WorkKind) -> bool {
    if matches!(material, 29 | crate::train_parts::CARGO) {
        return true;
    }
    is_station(material)
        && (kind == WorkKind::Collector
            || matches!(
                (material, kind),
                (LUMBERJACK, WorkKind::Lumberjack)
                    | (MINE, WorkKind::Miner)
                    | (FARM, WorkKind::Farmer)
            ))
}

pub const fn label(material: u16) -> Option<&'static str> {
    Some(match material {
        LUMBERJACK => "Holzfaeller-Station",
        SMITH => "Schmiede-Station",
        MARKET => "Verkaufsstand",
        ARMY => "Armee-Station",
        RAILWAY => "Lok-Station",
        MINE => "Minen-Station",
        BUTCHER => "Metzger-Station",
        BISHOP => "Bischofs-Station",
        FARM => "Farmer-Station",
        _ => return None,
    })
}

pub fn boxes(material: u16) -> &'static [ShapeBox] {
    if !is_station(material) {
        return &[];
    }
    #[derive(Deserialize)]
    struct Definition {
        collision_boxes: Vec<ShapeBox>,
    }
    const SOURCES: [&str; 9] = [
        include_str!("../../../assets/terra/blocks/lumberjack_station.json"),
        include_str!("../../../assets/terra/blocks/smith_station.json"),
        include_str!("../../../assets/terra/blocks/market_station.json"),
        include_str!("../../../assets/terra/blocks/army_station.json"),
        include_str!("../../../assets/terra/blocks/railway_station.json"),
        include_str!("../../../assets/terra/blocks/mine_station.json"),
        include_str!("../../../assets/terra/blocks/butcher_station.json"),
        include_str!("../../../assets/terra/blocks/bishop_station.json"),
        include_str!("../../../assets/terra/blocks/farm_station.json"),
    ];
    static PROFILES: [OnceLock<Vec<ShapeBox>>; 9] = [const { OnceLock::new() }; 9];
    let index = usize::from(material - LUMBERJACK);
    PROFILES[index].get_or_init(|| {
        serde_json::from_str::<Definition>(SOURCES[index])
            .ok()
            .map(|definition| definition.collision_boxes)
            .filter(|boxes| {
                !boxes.is_empty() && boxes.len() <= 128 && boxes.iter().all(ShapeBox::valid)
            })
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_recipes_charge_real_materials_and_require_a_workbench() {
        use crate::crafting::{CraftDestination, CraftStation, RecipeBook};
        let recipes = RecipeBook::builtins();
        assert!(recipes.error.is_none(), "{:?}", recipes.error);
        for material in ALL {
            let recipe = recipes
                .iter()
                .find(|recipe| recipe.output.item == crate::ItemKind::Block(material))
                .unwrap();
            let mut inventory = crate::Inventory::default();
            for ingredient in recipe.ingredients() {
                assert_eq!(inventory.insert(ingredient.item, ingredient.count), 0);
            }
            let before = inventory.clone();
            assert!(
                recipes
                    .craft(
                        &mut inventory,
                        recipe.key(),
                        1,
                        CraftStation::Hand,
                        CraftDestination::Cursor
                    )
                    .is_err()
            );
            assert_eq!(inventory, before);
            recipes
                .craft(
                    &mut inventory,
                    recipe.key(),
                    1,
                    CraftStation::Workbench,
                    CraftDestination::Cursor,
                )
                .unwrap();
            assert_eq!(
                inventory.cursor,
                Some(crate::ItemStack {
                    item: crate::ItemKind::Block(material),
                    count: 1
                })
            );
            assert_eq!(inventory.occupied_slots(), 0);
        }
    }

    #[test]
    fn every_station_grid_uniquely_selects_its_recipe_among_all_builtins() {
        use crate::crafting::{CraftGridCell, CraftStation, RecipeBook, RecipePattern};
        let book = RecipeBook::builtins();
        for material in ALL {
            let recipe = book
                .iter()
                .find(|recipe| recipe.output.item == crate::ItemKind::Block(material))
                .unwrap();
            let RecipePattern::Shapeless { ingredients } = &recipe.pattern else {
                panic!("Station recipe must declare its real components");
            };
            let mut inventory = crate::Inventory::default();
            let mut cells = [None; 9];
            for (index, ingredient) in ingredients.iter().enumerate() {
                assert!(inventory.creative_set(
                    index,
                    crate::ItemStack {
                        item: ingredient.item,
                        count: ingredient.count
                    }
                ));
                cells[index] = Some(CraftGridCell {
                    slot: index as u8,
                    count: ingredient.count,
                });
            }
            assert_eq!(
                book.grid_match_reserved(&inventory, &cells, 3, CraftStation::Workbench)
                    .map(|result| result.0),
                Some(recipe.key()),
                "{}",
                recipe.id
            );
            assert_eq!(
                book.output_for_reserved_grid(&inventory, &cells, 3, CraftStation::Workbench),
                Some(recipe.output)
            );
        }
    }

    #[test]
    fn all_profession_depots_have_stable_ids_real_geometry_and_only_supported_jobs() {
        assert_eq!(ALL, [297, 298, 299, 300, 301, 302, 303, 304, 305]);
        for material in ALL {
            assert!(is_station(material) && is_depot(material));
            assert!(label(material).is_some());
            assert!(terra_voxel::MaterialId::from_raw(material).is_some());
            let shapes = boxes(material);
            assert!(shapes.len() >= 3 && shapes.iter().all(ShapeBox::valid));
            for facing in 0..4 {
                assert!(shapes.iter().all(|shape| shape.rotated_y(facing).valid()));
            }
            assert!(supports_work(material, WorkKind::Collector));
            assert_eq!(supports_work(material, WorkKind::Miner), material == MINE);
            assert_eq!(supports_work(material, WorkKind::Farmer), material == FARM);
            assert_eq!(
                supports_work(material, WorkKind::Lumberjack),
                material == LUMBERJACK
            );
        }
        assert!(supports_work(29, WorkKind::Lumberjack));
        assert!(!is_station(296) && !is_station(306));
        assert!(!supports_work(3, WorkKind::Collector));
    }
}
