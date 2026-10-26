use super::{Config, ConfigError, ensure};
use crate::{GENERATOR_VERSION, WorldId, WorldSeed};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldConfig {
    /// Saved surface ecology/drainage contract. Absent in old saves retains their terrain.
    #[serde(default)]
    pub ecology_revision: u8,
    /// Natural population cap within 150 m of each player; owned animals are excluded.
    pub natural_mob_limit: u16,
    /// New saved terrain contract; absent in existing worlds means no retrogen.
    #[serde(default)]
    pub bounded_world: bool,
    /// Absent in saved worlds retains their original narrow ravines.
    #[serde(default)]
    pub ravine_revision: u8,
    /// Saved river-bank erosion contract; missing in existing worlds preserves their terrain.
    #[serde(default)]
    pub river_terrain_revision: u8,
    pub time: crate::WorldTimeConfig,
    pub seasons: crate::SeasonConfig,
    pub game_rules: crate::GameRules,
    pub id: WorldId,
    pub seed: WorldSeed,
    pub generator_version: u32,
    pub base_height: f64,
    pub hill_amplitude: f64,
    pub mountain_amplitude: f64,
    pub sea_level: f64,
    pub snow_height: f64,
    pub caves: bool,
    /// Saved landscape feature; missing in older saves preserves their cave field.
    #[serde(default)]
    pub geology_revision: u8,
    /// Saved terrain contract. Missing in existing worlds preserves the legacy heightfield.
    #[serde(default)]
    pub landscape_revision: u8,
    /// Missing in saved worlds preserves their seabed and unfrozen water.
    #[serde(default)]
    pub ocean_revision: u8,
    /// Saved cave ecology/hydrology contract; absent in old saves disables retrogen.
    #[serde(default)]
    pub cave_structure_revision: u8,
    /// Saved settlement/ruin/mine contract. Fresh worlds opt in; missing saved fields disable retrogen.
    #[serde(default)]
    pub structures_revision: u8,
    pub cave_biomes: super::CaveBiomesConfig,
    pub trees: bool,
    pub water: bool,
    /// Opt-in saved feature: absent in existing version-5 worlds means unchanged terrain.
    pub rivers: bool,
    /// Relative tree density; 0.8 retains 80% of otherwise eligible candidates.
    pub tree_density: f64,
    /// Broad climate transition scale in meters, independent of chunk boundaries.
    pub biome_scale: f64,
    pub ocean_scale: f64,
    pub biomes: bool,
    pub terrace_height: f64,
    pub terrace_strength: f64,
    #[serde(default = "crate::OreGeneration::legacy")]
    pub ores: crate::OreGeneration,
}
impl WorldConfig {
    /// Prepared terrain is independent of live calendar, population and game rules.
    pub fn same_terrain(&self, other: &Self) -> bool {
        // Exhaustive destructuring makes new settings an explicit cache-identity decision.
        let Self {
            ecology_revision,
            natural_mob_limit: _,
            time: _,
            seasons: _,
            game_rules: _,
            id,
            seed,
            generator_version,
            bounded_world,
            ravine_revision,
            river_terrain_revision,
            base_height,
            hill_amplitude,
            mountain_amplitude,
            sea_level,
            snow_height,
            caves,
            geology_revision,
            landscape_revision,
            ocean_revision,
            cave_structure_revision,
            structures_revision,
            cave_biomes,
            trees,
            water,
            rivers,
            tree_density,
            biome_scale,
            ocean_scale,
            biomes,
            terrace_height,
            terrace_strength,
            ores,
        } = self;
        ecology_revision == &other.ecology_revision
            && id == &other.id
            && seed == &other.seed
            && generator_version == &other.generator_version
            && bounded_world == &other.bounded_world
            && ravine_revision == &other.ravine_revision
            && river_terrain_revision == &other.river_terrain_revision
            && base_height == &other.base_height
            && hill_amplitude == &other.hill_amplitude
            && mountain_amplitude == &other.mountain_amplitude
            && sea_level == &other.sea_level
            && snow_height == &other.snow_height
            && caves == &other.caves
            && geology_revision == &other.geology_revision
            && landscape_revision == &other.landscape_revision
            && ocean_revision == &other.ocean_revision
            && cave_structure_revision == &other.cave_structure_revision
            && structures_revision == &other.structures_revision
            && cave_biomes == &other.cave_biomes
            && trees == &other.trees
            && water == &other.water
            && rivers == &other.rivers
            && tree_density == &other.tree_density
            && biome_scale == &other.biome_scale
            && ocean_scale == &other.ocean_scale
            && biomes == &other.biomes
            && terrace_height == &other.terrace_height
            && terrace_strength == &other.terrace_strength
            && ores == &other.ores
    }
}
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            ecology_revision: 2,
            natural_mob_limit: 256,
            bounded_world: true,
            ravine_revision: 1,
            river_terrain_revision: 1,
            time: crate::WorldTimeConfig::default(),
            seasons: crate::SeasonConfig::default(),
            game_rules: crate::GameRules::default(),
            id: WorldId(1),
            seed: WorldSeed(42042),
            generator_version: GENERATOR_VERSION,
            base_height: 16.0,
            hill_amplitude: 12.0,
            mountain_amplitude: 48.0,
            sea_level: 8.0,
            snow_height: 112.0,
            caves: true,
            geology_revision: 4,
            landscape_revision: 1,
            ocean_revision: 1,
            cave_structure_revision: 8,
            structures_revision: 5,
            cave_biomes: Default::default(),
            trees: true,
            water: true,
            rivers: false,
            tree_density: 0.8,
            biome_scale: 2048.0,
            ocean_scale: 4096.0,
            biomes: true,
            terrace_height: 4.0,
            terrace_strength: 0.72,
            ores: crate::OreGeneration::default(),
        }
    }
}
impl Config for WorldConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            self.ecology_revision <= 2,
            "Unsupported world.ecology_revision",
        )?;
        ensure(
            self.structures_revision <= 5,
            "Unsupported world.structures_revision",
        )?;
        ensure(self.game_rules.valid(), "Invalid world.game_rules")?;
        ensure(
            self.cave_structure_revision <= 8,
            "Unsupported world.cave_structure_revision",
        )?;
        self.cave_biomes.validate()?;
        ensure(self.ocean_revision <= 1, "Unsupported world.ocean_revision")?;
        ensure(
            self.ravine_revision <= 1,
            "Unsupported world.ravine_revision",
        )?;
        ensure(
            self.river_terrain_revision <= 1,
            "Unsupported world.river_terrain_revision",
        )?;
        ensure(
            self.natural_mob_limit <= 4096,
            "world.natural_mob_limit must be 0..=4096",
        )?;
        ensure(
            self.landscape_revision <= 1,
            "Unsupported world.landscape_revision",
        )?;
        ensure(
            self.geology_revision <= 4,
            "Unsupported world.geology_revision",
        )?;
        ensure(
            !self.ores.enabled || self.ores.revision == 1,
            "Ore generation requires a native ore world; legacy worlds stay unchanged. Create a new world instead of enabling retrogen.",
        )?;
        ensure(
            self.ores.valid(),
            "world.ores requires valid depths, radius, frequency and cave affinity",
        )?;
        ensure(
            self.seasons.valid(),
            "world.seasons requires lengths in 1..=3650 days and a finite positive transition_days no longer than any season",
        )?;
        ensure(
            self.time.valid(),
            "world.time requires day_length_seconds in 60..=604800 and start_hour in 0..<24",
        )?;
        ensure(
            self.generator_version == GENERATOR_VERSION,
            &format!("unsupported world.generator_version (supported: {GENERATOR_VERSION})"),
        )?;
        for (name, value) in [
            ("base_height", self.base_height),
            ("hill_amplitude", self.hill_amplitude),
            ("mountain_amplitude", self.mountain_amplitude),
            ("sea_level", self.sea_level),
            ("snow_height", self.snow_height),
        ] {
            ensure(
                value.is_finite() && value.abs() <= 100_000.0,
                &format!("world.{name} must be finite and within +/-100000"),
            )?;
        }
        ensure(
            self.hill_amplitude >= 0.0 && self.mountain_amplitude >= 0.0,
            "world terrain amplitudes must be nonnegative",
        )?;
        for (name, value) in [
            ("tree_density", self.tree_density),
            ("terrace_strength", self.terrace_strength),
        ] {
            ensure(
                value.is_finite() && (0.0..=1.0).contains(&value),
                &format!("world.{name} must be finite and within 0..=1"),
            )?;
        }
        ensure(
            self.biome_scale.is_finite() && (256.0..=16384.0).contains(&self.biome_scale),
            "world.biome_scale must be finite and within 256..=16384 meters",
        )?;
        ensure(
            self.ocean_scale.is_finite() && (512.0..=32768.0).contains(&self.ocean_scale),
            "world.ocean_scale must be finite and within 512..=32768 meters",
        )?;
        ensure(
            self.terrace_height.is_finite() && (2.0..=12.0).contains(&self.terrace_height),
            "world.terrace_height must be finite and within 2..=12 meters",
        )?;
        ensure(
            self.snow_height > self.sea_level,
            "world.snow_height must exceed sea_level",
        )
    }
}
