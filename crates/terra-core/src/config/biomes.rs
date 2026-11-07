use super::{ConfigError, ensure};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiomeKind {
    Ocean,
    Plains,
    Forest,
    Desert,
    #[serde(rename = "highlands")]
    Highland,
    MushroomPark,
    ClifftedDesert,
    CherryForest,
}

impl BiomeKind {
    pub const ALL: [Self; 8] = [
        Self::Ocean,
        Self::Plains,
        Self::Forest,
        Self::Desert,
        Self::Highland,
        Self::MushroomPark,
        Self::ClifftedDesert,
        Self::CherryForest,
    ];

    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Ocean => "ocean.json",
            Self::Plains => "plains.json",
            Self::Forest => "forest.json",
            Self::Desert => "desert.json",
            Self::Highland => "highlands.json",
            Self::MushroomPark => "mushroom_park.json",
            Self::ClifftedDesert => "cliffted_desert.json",
            Self::CherryForest => "cherry_forest.json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeDefinitions {
    pub entries: [BiomeDefinition; 8],
    #[serde(default = "super::LandscapeDefinitions::legacy_v5")]
    pub landscapes: super::LandscapeDefinitions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeDefinition {
    pub biome: BiomeKind,
    /// Relative land-biome selection weight. Missing saved fields retain the
    /// original distribution; ocean extent remains controlled by continents.
    #[serde(default = "unit_selection_weight")]
    pub selection_weight: f64,
    pub terrain: BiomeTerrain,
    #[serde(default)]
    pub ocean: Option<OceanBiomeConfig>,
}

const fn unit_selection_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeTerrain {
    pub min_height: f64,
    pub max_height: f64,
    pub horizontal_scale: f64,
    pub detail_strength: f64,
    pub relief_strength: f64,
    pub soil_depth: f64,
    pub tree_density: f64,
    pub terrace_strength: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OceanBiomeConfig {
    pub warm: MarineHabitatConfig,
    pub cold: MarineHabitatConfig,
    pub swamp: MarineHabitatConfig,
    pub cold_temperature: f64,
    pub warm_temperature: f64,
    pub swamp_humidity: f64,
    pub clay_frequency: f64,
    pub clay_cluster_scale: f64,
    pub clay_depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarineHabitatConfig {
    pub floor: MarineFloorWeights,
    pub coral_density: f64,
    pub seagrass_density: f64,
    pub max_vegetation_depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarineFloorWeights {
    pub sand: f64,
    pub gravel: f64,
    pub mud: f64,
    pub dirt: f64,
    pub stone: f64,
}

impl BiomeDefinitions {
    pub fn get(&self, biome: BiomeKind) -> &BiomeDefinition {
        &self.entries[biome as usize]
    }

    /// Load only during world creation. The resulting snapshot belongs to the save.
    pub fn load_directory(directory: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let directory = directory.as_ref();
        let mut definitions = Self::default();
        for biome in BiomeKind::ALL {
            let path = directory.join(biome.file_name());
            let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::Io {
                path: path.clone(),
                source,
            })?;
            definitions.entries[biome as usize] = serde_json::from_str(&text)
                .map_err(|error| ConfigError::Validation(format!("{}: {error}", path.display())))?;
        }
        let path = directory.join("terrain_profiles.json");
        if path.exists() {
            let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::Io {
                path: path.clone(),
                source,
            })?;
            definitions.landscapes = serde_json::from_str(&text)
                .map_err(|error| ConfigError::Validation(format!("{}: {error}", path.display())))?;
        }
        definitions.normalize()?;
        Ok(definitions)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.landscapes.validate()?;
        for (kind, definition) in BiomeKind::ALL.into_iter().zip(&self.entries) {
            ensure(
                definition.biome == kind,
                "Biome snapshot order or file name does not match biome ID",
            )?;
            ensure(
                definition.selection_weight.is_finite()
                    && (0.0..=4.0).contains(&definition.selection_weight),
                "Biome selection weight must be within 0..4",
            )?;
            let t = definition.terrain;
            ensure(
                t.min_height.is_finite()
                    && t.max_height.is_finite()
                    && t.min_height >= -120.0
                    && t.max_height <= 350.0
                    && t.min_height < t.max_height,
                "Biome height range must be ordered within -120..350",
            )?;
            for (value, min, max) in [
                (t.horizontal_scale, 32.0, 8192.0),
                (t.detail_strength, 0.0, 1.0),
                (t.relief_strength, 0.0, 1.0),
                (t.soil_depth, 0.0, 32.0),
                (t.tree_density, 0.0, 4.0),
                (t.terrace_strength, 0.0, 1.0),
            ] {
                ensure(
                    value.is_finite() && (min..=max).contains(&value),
                    "Invalid biome terrain setting",
                )?;
            }
            ensure(
                (kind == BiomeKind::Ocean) == definition.ocean.is_some(),
                "Only ocean.json must contain ocean habitat settings",
            )?;
            if let Some(ocean) = definition.ocean {
                ocean.validate()?;
            }
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), ConfigError> {
        self.validate()?;
        for definition in &mut self.entries {
            if let Some(ocean) = &mut definition.ocean {
                for habitat in [&mut ocean.warm, &mut ocean.cold, &mut ocean.swamp] {
                    let sum: f64 = habitat.floor.values().into_iter().sum();
                    habitat.floor.sand /= sum;
                    habitat.floor.gravel /= sum;
                    habitat.floor.mud /= sum;
                    habitat.floor.dirt /= sum;
                    habitat.floor.stone /= sum;
                }
            }
        }
        Ok(())
    }
}

impl Default for BiomeDefinitions {
    fn default() -> Self {
        Self::builtin().clone()
    }
}

impl BiomeDefinitions {
    pub fn builtin() -> &'static Self {
        static DEFINITIONS: OnceLock<BiomeDefinitions> = OnceLock::new();
        DEFINITIONS.get_or_init(|| {
            let files = [
                include_str!("../../../../assets/biome/ocean.json"),
                include_str!("../../../../assets/biome/plains.json"),
                include_str!("../../../../assets/biome/forest.json"),
                include_str!("../../../../assets/biome/desert.json"),
                include_str!("../../../../assets/biome/highlands.json"),
                include_str!("../../../../assets/biome/mushroom_park.json"),
                include_str!("../../../../assets/biome/cliffted_desert.json"),
                include_str!("../../../../assets/biome/cherry_forest.json"),
            ];
            let mut result = Self {
                entries: files
                    .map(|text| serde_json::from_str(text).expect("valid built-in biome JSON")),
                landscapes: Default::default(),
            };
            result.normalize().expect("valid built-in biome profiles");
            result
        })
    }
}

impl MarineFloorWeights {
    pub fn values(self) -> [f64; 5] {
        [self.sand, self.gravel, self.mud, self.dirt, self.stone]
    }
}

impl OceanBiomeConfig {
    fn validate(self) -> Result<(), ConfigError> {
        for habitat in [self.warm, self.cold, self.swamp] {
            ensure(
                habitat
                    .floor
                    .values()
                    .into_iter()
                    .all(|w| w.is_finite() && w >= 0.0)
                    && habitat.floor.values().into_iter().sum::<f64>().is_finite()
                    && habitat.floor.values().into_iter().sum::<f64>() > 0.0,
                "Marine floor weights require a finite positive sum",
            )?;
            for density in [habitat.coral_density, habitat.seagrass_density] {
                ensure(
                    density.is_finite() && (0.0..=1.0).contains(&density),
                    "Marine vegetation density must be 0..1",
                )?;
            }
            ensure(
                habitat.max_vegetation_depth.is_finite()
                    && (1.0..=128.0).contains(&habitat.max_vegetation_depth),
                "Marine vegetation depth must be 1..128",
            )?;
        }
        for value in [
            self.cold_temperature,
            self.warm_temperature,
            self.swamp_humidity,
            self.clay_frequency,
        ] {
            ensure(
                value.is_finite() && (0.0..=1.0).contains(&value),
                "Ocean climate thresholds and clay frequency must be 0..1",
            )?;
        }
        ensure(
            self.cold_temperature < self.warm_temperature,
            "Cold ocean threshold must precede warm threshold",
        )?;
        ensure(
            self.clay_cluster_scale.is_finite()
                && (2.0..=128.0).contains(&self.clay_cluster_scale)
                && self.clay_depth.is_finite()
                && (0.0..=8.0).contains(&self.clay_depth),
            "Invalid ocean clay clusters",
        )
    }
}

impl Default for OceanBiomeConfig {
    fn default() -> Self {
        BiomeDefinitions::builtin()
            .get(BiomeKind::Ocean)
            .ocean
            .expect("ocean defaults")
    }
}
impl Default for MarineHabitatConfig {
    fn default() -> Self {
        OceanBiomeConfig::default().warm
    }
}
impl Default for MarineFloorWeights {
    fn default() -> Self {
        MarineHabitatConfig::default().floor
    }
}

#[cfg(test)]
mod tests;
