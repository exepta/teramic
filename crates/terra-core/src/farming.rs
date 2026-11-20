//! Shared crop rules; only the authority advances persistent plant state.
use crate::{ItemKind, ItemStack, ResourcePropKind, SeasonSample};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const FARMLAND: u16 = 101;
pub const GROWTH_STAGES: u8 = 8;
pub const DRY_MOISTURE: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FarmToolKind {
    WateringCan,
    Hoe,
    Scythe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FarmingAction {
    Till,
    Water,
    Refill,
    Harvest,
    FillBucket,
    EmptyBucket,
    MakePath,
    PourTar,
    FillBileBucket,
    EmptyBileBucket,
}

#[cfg(test)]
mod tool_tests {
    use super::*;
    use crate::ToolKind;

    #[test]
    fn every_farming_tier_has_one_catalog_entry_and_bounded_gameplay_parameters() {
        let tools: Vec<_> = ToolKind::ALL
            .into_iter()
            .filter(|tool| tool.farm_kind().is_some())
            .collect();
        assert_eq!(tools.len(), 15);
        for role in [
            FarmToolKind::WateringCan,
            FarmToolKind::Hoe,
            FarmToolKind::Scythe,
        ] {
            let tiers: Vec<_> = tools
                .iter()
                .filter(|tool| tool.farm_kind() == Some(role))
                .map(|tool| tool.farm_tier())
                .collect();
            assert_eq!(tiers, [1, 2, 3, 4, 5]);
        }
        for tool in tools {
            assert_eq!(
                ItemKind::EXTRA
                    .into_iter()
                    .filter(|item| *item == ItemKind::Tool(tool))
                    .count(),
                1
            );
            assert!((0.3..=0.7).contains(&tool.farm_cooldown()));
            assert!(tool.farm_radius() <= 2);
            assert!(tool.class().is_none());
            if tool.farm_kind() == Some(FarmToolKind::WateringCan) {
                assert_eq!(tool.watering_capacity(), 4 + tool.farm_tier() * 4);
            } else {
                assert_eq!(tool.watering_capacity(), 0);
            }
        }
        assert_eq!(ToolKind::WoodPickaxe.farm_kind(), None);
    }

    #[test]
    fn every_farming_recipe_is_valid_and_hoe_patterns_use_the_canonical_two_column_bounds() {
        let book = crate::crafting::RecipeBook::builtins();
        assert_eq!(book.error, None);
        let mut count = 0;
        for tool in ToolKind::ALL
            .into_iter()
            .filter(|tool| tool.farm_kind().is_some())
        {
            let recipes: Vec<_> = book
                .iter()
                .filter(|recipe| recipe.output.item == ItemKind::Tool(tool))
                .collect();
            assert_eq!(recipes.len(), 1, "{}", tool.slug());
            assert_eq!(recipes[0].output.count, 1);
            assert_eq!(recipes[0].station, crate::crafting::CraftStation::Workbench);
            if tool.farm_kind() == Some(FarmToolKind::Hoe) {
                assert!(
                    matches!(&recipes[0].pattern,crate::crafting::RecipePattern::Shaped { width:2,height:3,cells } if cells.len()==6)
                );
            }
            count += 1;
        }
        assert_eq!(count, 15);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropKind {
    Wheat,
    Corn,
    Lettuce,
}
impl CropKind {
    pub const ALL: [Self; 3] = [Self::Wheat, Self::Corn, Self::Lettuce];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Wheat => "wheat",
            Self::Corn => "corn",
            Self::Lettuce => "lettuce",
        }
    }
    pub const fn resource_prop(self) -> ResourcePropKind {
        match self {
            Self::Wheat => ResourcePropKind::Wheat,
            Self::Corn => ResourcePropKind::Corn,
            Self::Lettuce => ResourcePropKind::Lettuce,
        }
    }
    pub const fn from_prop(prop: ResourcePropKind) -> Option<Self> {
        match prop {
            ResourcePropKind::Wheat => Some(Self::Wheat),
            ResourcePropKind::Corn => Some(Self::Corn),
            ResourcePropKind::Lettuce => Some(Self::Lettuce),
            _ => None,
        }
    }
    pub const fn seed(self) -> CropItem {
        match self {
            Self::Wheat => CropItem::WheatSeeds,
            Self::Corn => CropItem::CornSeeds,
            Self::Lettuce => CropItem::LettuceSeeds,
        }
    }
    pub const fn produce(self) -> CropItem {
        match self {
            Self::Wheat => CropItem::Wheat,
            Self::Corn => CropItem::Corn,
            Self::Lettuce => CropItem::Lettuce,
        }
    }
    pub fn definition(self) -> &'static CropDefinition {
        crop_definition(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CropItem {
    WheatSeeds,
    CornSeeds,
    LettuceSeeds,
    Wheat,
    Corn,
    Lettuce,
    Bread,
}
impl CropItem {
    pub const ALL: [Self; 7] = [
        Self::WheatSeeds,
        Self::CornSeeds,
        Self::LettuceSeeds,
        Self::Wheat,
        Self::Corn,
        Self::Lettuce,
        Self::Bread,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::WheatSeeds => "wheat_seeds",
            Self::CornSeeds => "corn_seeds",
            Self::LettuceSeeds => "lettuce_seeds",
            Self::Wheat => "wheat",
            Self::Corn => "corn",
            Self::Lettuce => "lettuce",
            Self::Bread => "bread",
        }
    }
    pub const fn crop_kind(self) -> Option<CropKind> {
        match self {
            Self::WheatSeeds => Some(CropKind::Wheat),
            Self::CornSeeds => Some(CropKind::Corn),
            Self::LettuceSeeds => Some(CropKind::Lettuce),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropDefinition {
    pub kind: CropKind,
    pub growth_seconds: f32,
    pub moisture_evaporation: f32,
    /// Spring, summer, autumn and winter growth multipliers.
    pub seasonal_growth: [f32; 4],
    pub produce_count: u16,
    pub seed_count: u16,
    pub seed_bonus: u16,
}
impl CropDefinition {
    pub fn valid(self) -> bool {
        self.growth_seconds.is_finite()
            && (30.0..=86_400.0).contains(&self.growth_seconds)
            && self.moisture_evaporation.is_finite()
            && (0.0..=0.05).contains(&self.moisture_evaporation)
            && self
                .seasonal_growth
                .iter()
                .all(|v| v.is_finite() && (0.0..=4.0).contains(v))
            && (1..=16).contains(&self.produce_count)
            && (1..=16).contains(&self.seed_count)
            && self.seed_bonus <= 16
    }
}

const SOURCES: [(&str, &str); 3] = [
    (
        "wheat",
        include_str!("../../../assets/terra/farming/wheat.json"),
    ),
    (
        "corn",
        include_str!("../../../assets/terra/farming/corn.json"),
    ),
    (
        "lettuce",
        include_str!("../../../assets/terra/farming/lettuce.json"),
    ),
];
pub fn crop_definition(kind: CropKind) -> &'static CropDefinition {
    static DEFINITIONS: OnceLock<[CropDefinition; 3]> = OnceLock::new();
    let definitions = DEFINITIONS.get_or_init(|| {
        SOURCES.map(|(name, builtin)| {
            let fallback: CropDefinition =
                serde_json::from_str(builtin).expect("builtin crop definition");
            assert!(fallback.valid());
            std::fs::read_to_string(
                crate::asset_directory()
                    .join("terra/farming")
                    .join(format!("{name}.json")),
            )
            .ok()
            .and_then(|source| serde_json::from_str::<CropDefinition>(&source).ok())
            .filter(|definition| definition.valid() && definition.kind == fallback.kind)
            .unwrap_or(fallback)
        })
    });
    &definitions[match kind {
        CropKind::Wheat => 0,
        CropKind::Corn => 1,
        CropKind::Lettuce => 2,
    }]
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropState {
    pub kind: CropKind,
    pub prop_id: u64,
    pub progress_seconds: f32,
    pub moisture: f32,
    /// Captured when sowing so later JSON tuning cannot invalidate a saved crop or stage.
    pub growth_duration_seconds: f32,
}
impl CropState {
    pub fn new(kind: CropKind, prop_id: u64) -> Self {
        Self {
            kind,
            prop_id,
            progress_seconds: 0.0,
            moisture: 0.5,
            growth_duration_seconds: kind.definition().growth_seconds,
        }
    }
    pub fn valid(self) -> bool {
        self.prop_id != 0
            && self.progress_seconds.is_finite()
            && self.growth_duration_seconds.is_finite()
            && (30.0..=86_400.0).contains(&self.growth_duration_seconds)
            && (0.0..=self.growth_duration_seconds).contains(&self.progress_seconds)
            && self.moisture.is_finite()
            && (0.0..=1.0).contains(&self.moisture)
    }
    pub fn progress(self) -> f32 {
        (self.progress_seconds / self.growth_duration_seconds).clamp(0.0, 1.0)
    }
    pub fn stage(self) -> u8 {
        (self.progress() * f32::from(GROWTH_STAGES - 1)).floor() as u8
    }
    pub fn mature(self) -> bool {
        self.progress_seconds >= self.growth_duration_seconds
    }
    /// Returns whether the visible growth stage changed. Callers budget spatial sampling.
    pub fn advance(&mut self, seconds: f32, light: f32, water: f32, season: SeasonSample) -> bool {
        if !self.valid()
            || !seconds.is_finite()
            || seconds <= 0.0
            || !light.is_finite()
            || !water.is_finite()
        {
            return false;
        }
        let before = self.stage();
        let definition = self.kind.definition();
        let seconds = seconds.min(60.0);
        let water = water.clamp(0.0, 1.0);
        self.moisture = (self.moisture + water * seconds * 0.025
            - definition.moisture_evaporation * seconds * (1.0 - water))
            .clamp(0.0, 1.0);
        let seasonal = if season.enabled {
            season
                .weights
                .into_iter()
                .zip(definition.seasonal_growth)
                .map(|(w, rate)| w * rate)
                .sum::<f32>()
        } else {
            1.0
        };
        let sunlight = ((light.clamp(0.0, 1.0) - 0.08) / 0.92).clamp(0.0, 1.0);
        let hydration = if self.moisture <= DRY_MOISTURE {
            0.0
        } else {
            0.3 + self.moisture * 0.7
        };
        self.progress_seconds = (self.progress_seconds + seconds * sunlight * hydration * seasonal)
            .min(self.growth_duration_seconds);
        self.stage() != before
    }
    /// Early breaking returns the planted seed; only ripe crops multiply food and seed stock.
    pub fn harvest(self) -> Vec<ItemStack> {
        if !self.mature() {
            return vec![ItemStack {
                item: ItemKind::Crop(self.kind.seed()),
                count: 1,
            }];
        }
        let definition = self.kind.definition();
        let bonus = (self.prop_id.rotate_right(17) % u64::from(definition.seed_bonus + 1)) as u16;
        vec![
            ItemStack {
                item: ItemKind::Crop(self.kind.produce()),
                count: definition.produce_count,
            },
            ItemStack {
                item: ItemKind::Crop(self.kind.seed()),
                count: definition.seed_count + bonus,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Season, SeasonConfig, WorldClock};
    fn season(season: Season) -> SeasonSample {
        SeasonConfig {
            start_season: season,
            ..Default::default()
        }
        .sample(WorldClock::default())
    }
    #[test]
    fn growth_is_seeded_persistent_and_requires_light_water_and_suitable_season() {
        for kind in CropKind::ALL {
            assert!(kind.definition().valid());
            assert_eq!(CropKind::from_prop(kind.resource_prop()), Some(kind));
            assert_eq!(kind.seed().crop_kind(), Some(kind));
            let mut plant = CropState::new(kind, 123);
            let mut dark = plant;
            let mut winter = plant;
            let mut dry = plant;
            dry.moisture = 0.0;
            for _ in 0..100 {
                plant.advance(30.0, 1.0, 1.0, season(Season::Spring));
                dark.advance(30.0, 0.0, 1.0, season(Season::Spring));
                winter.advance(30.0, 1.0, 1.0, season(Season::Winter));
                dry.advance(30.0, 1.0, 0.0, season(Season::Spring));
            }
            assert!(plant.mature() && plant.valid());
            assert_eq!(plant.stage(), 7);
            assert_eq!(dark.progress_seconds, 0.0);
            assert_eq!(winter.progress_seconds, 0.0);
            assert_eq!(dry.progress_seconds, 0.0);
            assert_eq!(plant.harvest(), plant.harvest());
            assert!(plant.harvest().iter().all(|stack| stack.valid()));
            assert!(
                plant
                    .harvest()
                    .iter()
                    .any(|stack| stack.item == ItemKind::Crop(kind.seed()) && stack.count >= 2)
            );
            assert_eq!(CropState::new(kind, 123).harvest().len(), 1);
        }
    }
    #[test]
    fn malformed_and_unbounded_ticks_cannot_corrupt_crop_state() {
        let mut plant = CropState::new(CropKind::Corn, 1);
        let initial = plant;
        assert!(!plant.advance(f32::NAN, 1.0, 1.0, season(Season::Summer)));
        assert!(!plant.advance(1.0, f32::INFINITY, 1.0, season(Season::Summer)));
        assert_eq!(plant, initial);
        plant.advance(10_000.0, 1.0, 1.0, season(Season::Summer));
        assert!(plant.progress_seconds <= 60.0 * 1.1);
    }
    #[test]
    fn crop_products_replant_and_feed_without_turning_seeds_into_food() {
        let book = crate::crafting::RecipeBook::builtins();
        for kind in CropKind::ALL {
            assert!(
                book.iter()
                    .any(|recipe| recipe.output.item == ItemKind::Crop(kind.seed()))
            );
            assert!(crate::food(ItemKind::Crop(kind.seed())).is_none());
            assert!(
                CropItem::ALL
                    .iter()
                    .all(|item| ItemKind::Crop(*item).valid())
            );
            assert_eq!(kind.produce().crop_kind(), None);
        }
        assert!(
            book.iter()
                .any(|recipe| recipe.output.item == ItemKind::Crop(CropItem::Bread))
        );
        assert!(
            book.iter()
                .any(|recipe| recipe.output.item == ItemKind::Block(FARMLAND))
        );
        for item in [CropItem::Corn, CropItem::Lettuce, CropItem::Bread] {
            assert!(crate::food(ItemKind::Crop(item)).unwrap().valid());
        }
        assert!(crate::food(ItemKind::Crop(CropItem::Wheat)).is_none());
    }
    #[test]
    fn crop_definition_values_reject_nonfinite_and_excessive_yields() {
        let definition = *CropKind::Wheat.definition();
        let mut invalid = definition;
        invalid.growth_seconds = f32::NAN;
        assert!(!invalid.valid());
        invalid = definition;
        invalid.seasonal_growth[2] = -0.1;
        assert!(!invalid.valid());
        invalid = definition;
        invalid.seed_count = 128;
        assert!(!invalid.valid());
        invalid = definition;
        invalid.moisture_evaporation = f32::INFINITY;
        assert!(!invalid.valid());
    }
    #[test]
    fn saved_duration_preserves_progress_and_stage_across_growth_definition_changes() {
        for kind in CropKind::ALL {
            let mut saved = CropState::new(kind, 109);
            saved.growth_duration_seconds *= 2.0;
            saved.progress_seconds = saved.growth_duration_seconds * 0.75;
            assert!(saved.progress_seconds > kind.definition().growth_seconds);
            assert!(saved.valid());
            assert_eq!(saved.stage(), 5);
            assert_eq!(saved.progress(), 0.75);
            assert!(!saved.mature());
            saved.progress_seconds = saved.growth_duration_seconds;
            assert!(saved.valid() && saved.mature());
            assert_eq!(saved.stage(), 7);
            assert_eq!(
                CropState::new(kind, 110).growth_duration_seconds,
                kind.definition().growth_seconds
            );
        }
    }
}
