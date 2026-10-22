//! Food properties live beside the existing item metadata and are shared by all authorities.
use crate::{ItemKind, MAX_VITAL, ResourcePropKind};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodDefinition {
    pub nutrition: f32,
    pub saturation: f32,
    pub eat_seconds: f32,
    #[serde(default)]
    pub regeneration: Option<FoodRegeneration>,
    #[serde(default)]
    pub remainder: Option<ItemKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoodRegeneration {
    pub health_per_second: f32,
    pub duration_seconds: f32,
}

impl FoodRegeneration {
    pub fn valid(self) -> bool {
        self.health_per_second.is_finite()
            && (0.01..=5.0).contains(&self.health_per_second)
            && self.duration_seconds.is_finite()
            && (0.1..=60.0).contains(&self.duration_seconds)
    }
}

impl FoodDefinition {
    pub fn valid(self) -> bool {
        self.nutrition.is_finite()
            && (0.1..=MAX_VITAL).contains(&self.nutrition)
            && self.saturation.is_finite()
            && (0.0..=MAX_VITAL).contains(&self.saturation)
            && self.eat_seconds.is_finite()
            && (0.25..=10.0).contains(&self.eat_seconds)
            && self.regeneration.is_none_or(FoodRegeneration::valid)
            && self.remainder.is_none_or(ItemKind::valid)
    }
}

#[derive(Deserialize)]
struct ItemFood {
    item: Option<ItemKind>,
    kind: Option<ResourcePropKind>,
    food: Option<FoodDefinition>,
}

impl ItemFood {
    fn identity(&self) -> Option<ItemKind> {
        match (self.item, self.kind) {
            (Some(item), None) => Some(item),
            (None, Some(kind)) => Some(ItemKind::Prop(kind)),
            _ => None,
        }
    }
}

const SOURCES: [(&str, &str); 12] = [
    (
        "items/raw_mutton.json",
        include_str!("../../../assets/terra/items/raw_mutton.json"),
    ),
    (
        "items/raw_beef.json",
        include_str!("../../../assets/terra/items/raw_beef.json"),
    ),
    (
        "items/raw_pork.json",
        include_str!("../../../assets/terra/items/raw_pork.json"),
    ),
    (
        "items/raw_fish.json",
        include_str!("../../../assets/terra/items/raw_fish.json"),
    ),
    (
        "items/raw_clownfish.json",
        include_str!("../../../assets/terra/items/raw_clownfish.json"),
    ),
    (
        "items/cooked_mutton.json",
        include_str!("../../../assets/terra/items/cooked_mutton.json"),
    ),
    (
        "items/cooked_beef.json",
        include_str!("../../../assets/terra/items/cooked_beef.json"),
    ),
    (
        "items/cooked_pork.json",
        include_str!("../../../assets/terra/items/cooked_pork.json"),
    ),
    (
        "items/cooked_fish.json",
        include_str!("../../../assets/terra/items/cooked_fish.json"),
    ),
    (
        "items/cooked_clownfish.json",
        include_str!("../../../assets/terra/items/cooked_clownfish.json"),
    ),
    (
        "items/honey.json",
        include_str!("../../../assets/terra/items/honey.json"),
    ),
    (
        "props/mushroom.json",
        include_str!("../../../assets/terra/props/mushroom.json"),
    ),
];

fn parse_definition(source: &str, expected: ItemKind) -> Result<Option<FoodDefinition>, String> {
    let data: ItemFood = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if data.identity() != Some(expected)
        || data
            .food
            .is_some_and(|food| !food.valid() || food.remainder == Some(expected))
    {
        return Err("Invalid food identity or values".into());
    }
    Ok(data.food)
}

pub fn food(item: ItemKind) -> Option<&'static FoodDefinition> {
    static DEFINITIONS: OnceLock<BTreeMap<ItemKind, FoodDefinition>> = OnceLock::new();
    DEFINITIONS
        .get_or_init(|| {
            let root = crate::asset_directory().join("terra");
            SOURCES
                .into_iter()
                .filter_map(|(path, source)| {
                    let builtin: ItemFood = match serde_json::from_str(source) {
                        Ok(data) => data,
                        Err(error) => {
                            eprintln!("Invalid builtin food JSON {path}: {error}; skipping food");
                            return None;
                        }
                    };
                    let Some(item) = builtin.identity() else {
                        eprintln!("Missing builtin food identity {path}; skipping food");
                        return None;
                    };
                    let fallback = match parse_definition(source, item) {
                        Ok(food) => food,
                        Err(error) => {
                            eprintln!("Invalid builtin food values {path}: {error}; skipping food");
                            return None;
                        }
                    };
                    let path = root.join(path);
                    let properties = match std::fs::read_to_string(&path) {
                        Ok(source) => match parse_definition(&source, item) {
                            Ok(food) => food,
                            Err(error) => {
                                eprintln!(
                                    "Invalid food definition {}: {error}; using builtin values",
                                    path.display()
                                );
                                fallback
                            }
                        },
                        Err(_) => fallback,
                    };
                    properties.map(|food| (item, food))
                })
                .collect()
        })
        .get(&item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::AnimalProduct;

    #[test]
    fn builtin_foods_use_existing_items_and_cooked_meals_are_more_nourishing() {
        let mut seen = std::collections::HashSet::new();
        for (_, source) in SOURCES {
            let data: ItemFood = serde_json::from_str(source).unwrap();
            let item = data.identity().unwrap();
            assert!(item.valid() && seen.insert(item));
            assert!(parse_definition(source, item).unwrap().unwrap().valid());
        }
        for (raw, cooked) in [
            (AnimalProduct::Mutton, AnimalProduct::CookedMutton),
            (AnimalProduct::Beef, AnimalProduct::CookedBeef),
            (AnimalProduct::Pork, AnimalProduct::CookedPork),
            (AnimalProduct::Fish, AnimalProduct::CookedFish),
            (AnimalProduct::Clownfish, AnimalProduct::CookedClownfish),
        ] {
            let raw = food(ItemKind::Animal(raw)).unwrap();
            let cooked = food(ItemKind::Animal(cooked)).unwrap();
            assert!(cooked.nutrition > raw.nutrition);
            assert!(cooked.saturation > raw.saturation);
        }
        assert!(food(ItemKind::Animal(AnimalProduct::Wool)).is_none());
        assert!(food(ItemKind::Animal(AnimalProduct::Leather)).is_none());
        assert!(food(ItemKind::Block(3)).is_none());
    }

    #[test]
    fn definitions_reject_invalid_values_identity_and_self_remainders() {
        let item = ItemKind::Prop(ResourcePropKind::Mushroom);
        for source in [
            r#"{"kind":"mushroom","food":{"nutrition":0,"saturation":1,"eat_seconds":1}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":-1,"eat_seconds":1}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":1,"eat_seconds":0}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":1,"eat_seconds":1,"unknown":2}}"#,
            r#"{"kind":"flower","food":{"nutrition":10,"saturation":1,"eat_seconds":1}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":1,"eat_seconds":1,"remainder":{"prop":"mushroom"}}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":1,"eat_seconds":1,"remainder":{"block":0}}}"#,
            r#"{"kind":"mushroom","food":{"nutrition":10,"saturation":1,"eat_seconds":1,"regeneration":{"health_per_second":1,"duration_seconds":100}}}"#,
        ] {
            assert!(parse_definition(source, item).is_err(), "{source}");
        }
        assert_eq!(parse_definition(r#"{"kind":"mushroom"}"#, item), Ok(None));
        let mut definition = *food(item).unwrap();
        definition.saturation = f32::NAN;
        assert!(!definition.valid());
        definition.saturation = 1.0;
        definition.nutrition = f32::INFINITY;
        assert!(!definition.valid());
    }
}
