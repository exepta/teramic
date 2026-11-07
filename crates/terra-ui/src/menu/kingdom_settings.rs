//! Editable world policy; values use the same validated DTO as the authority.
use super::view::WorldRuleCard;
use terra_core::kingdom::KingdomRules;

struct Field {
    key: &'static str,
    title: &'static str,
    hint: &'static str,
    icon: &'static str,
    switch: bool,
    min: u32,
    max: u32,
}
macro_rules! number {
    ($key:literal, $title:literal, $hint:literal, $icon:literal, $min:expr, $max:expr) => {
        Field {
            key: $key,
            title: $title,
            hint: $hint,
            icon: $icon,
            switch: false,
            min: $min,
            max: $max,
        }
    };
}
macro_rules! toggle {
    ($key:literal, $title:literal, $hint:literal, $icon:literal) => {
        Field {
            key: $key,
            title: $title,
            hint: $hint,
            icon: $icon,
            switch: true,
            min: 0,
            max: 1,
        }
    };
}
const FIELDS: [Field; 20] = [
    toggle!(
        "require_adjacent_claims",
        "Adjacent Claims",
        "Build claims connect through build land to the Bergfried; territory adjoins any own land.",
        "network"
    ),
    toggle!(
        "settlements_follow_kingdom_level",
        "Use Kingdom Level",
        "Off: settlements have their own levels. On: limits follow the kingdom.",
        "arrow-up"
    ),
    number!(
        "realm_min_border_gap",
        "Kingdom / Outpost Border Gap",
        "Free chunks between separate kingdom and outpost lands.",
        "ruler",
        0,
        4096
    ),
    number!(
        "kingdom_build_base",
        "Kingdom: Level 1 Build",
        "Protected building chunks at level 1.",
        "hammer",
        1,
        65_536
    ),
    number!(
        "kingdom_build_per_level",
        "Kingdom: Build per Level",
        "Additional protected chunks for each further level.",
        "hammer",
        0,
        1040
    ),
    number!(
        "kingdom_territory_base",
        "Kingdom: Level 1 Territory",
        "Territorial chunks at level 1.",
        "map",
        1,
        65_536
    ),
    number!(
        "kingdom_territory_per_level",
        "Kingdom: Territory per Level",
        "Additional territorial chunks for each further level.",
        "map",
        0,
        1040
    ),
    number!(
        "outposts_per_level",
        "Outposts per Kingdom Level",
        "Maximum outposts equals this value times the kingdom level.",
        "flag",
        0,
        256
    ),
    number!(
        "outpost_build_base",
        "Outpost: Level 1 Build",
        "Protected building chunks at level 1.",
        "flag",
        1,
        65_536
    ),
    number!(
        "outpost_build_per_level",
        "Outpost: Build per Level",
        "Additional protected chunks for each further level.",
        "flag",
        0,
        1040
    ),
    number!(
        "outpost_territory_base",
        "Outpost: Level 1 Territory",
        "Territorial chunks at level 1.",
        "map-pin",
        1,
        65_536
    ),
    number!(
        "outpost_territory_per_level",
        "Outpost: Territory per Level",
        "Additional territorial chunks for each further level.",
        "map-pin",
        0,
        1040
    ),
    toggle!(
        "village_requires_territory",
        "Villages Require Territory",
        "Village building claims remain inside your realm's territory.",
        "home"
    ),
    number!(
        "village_build_base",
        "Village: Level 1 Build",
        "Protected chunks; villages have no territory of their own.",
        "home",
        1,
        65_536
    ),
    number!(
        "village_build_per_level",
        "Village: Build per Level",
        "Additional protected chunks for each further level.",
        "home",
        0,
        1040
    ),
    number!(
        "city_min_kingdom_level",
        "Cities Unlock at Kingdom Level",
        "Required kingdom level before founding a city.",
        "crown",
        1,
        64
    ),
    toggle!(
        "city_requires_territory",
        "Cities Require Territory",
        "City building claims remain inside your realm's territory.",
        "landmark"
    ),
    number!(
        "city_build_base",
        "City: Level 1 Build",
        "Protected chunks; cities have no territory of their own.",
        "landmark",
        1,
        65_536
    ),
    number!(
        "city_build_per_level",
        "City: Build per Level",
        "Additional protected chunks for each further level.",
        "landmark",
        0,
        1040
    ),
    number!(
        "city_min_build_gap",
        "City Build Gap",
        "Free chunks from kingdom and village building land.",
        "ruler",
        0,
        4096
    ),
];

pub(super) fn change(rules: KingdomRules, key: &str, value: &str) -> Result<KingdomRules, String> {
    let field = FIELDS
        .iter()
        .find(|field| field.key == key)
        .ok_or("Unknown kingdom setting")?;
    let mut serialized = serde_json::to_value(rules).map_err(|error| error.to_string())?;
    serialized[key] = if field.switch {
        value
            .parse::<bool>()
            .map(serde_json::Value::Bool)
            .map_err(|_| "Choose On or Off".to_string())?
    } else {
        let number = value
            .parse::<u32>()
            .ok()
            .filter(|number| (field.min..=field.max).contains(number))
            .ok_or_else(|| format!("{} must be {}..={}", field.title, field.min, field.max))?;
        serde_json::Value::from(number)
    };
    let next: KingdomRules =
        serde_json::from_value(serialized).map_err(|error| error.to_string())?;
    if !next.valid() {
        return Err(
            "Kingdom limits must fit the maximum claim budget over all settlement levels".into(),
        );
    }
    Ok(next)
}

pub(super) fn cards(rules: KingdomRules) -> Vec<WorldRuleCard> {
    let values =
        serde_json::to_value(rules).expect("integer and boolean kingdom settings serialize");
    FIELDS
        .iter()
        .map(|field| {
            let value = &values[field.key];
            WorldRuleCard {
                name: field.key.into(),
                id: format!("world-kingdom-{}", field.key),
                title: field.title.into(),
                hint: field.hint.into(),
                icon: field.icon.into(),
                kind: if field.switch { "switch" } else { "number" }.into(),
                checked: value.as_bool().unwrap_or(false),
                value: value.to_string(),
                min: field.min.to_string(),
                max: field.max.to_string(),
                step: "1".into(),
            }
        })
        .collect()
}
