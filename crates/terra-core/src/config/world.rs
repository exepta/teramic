use super::{Config, ConfigError, ensure};
use crate::{GENERATOR_VERSION, WorldId, WorldSeed};
use serde::{Deserialize, Serialize};

fn legacy_sea_level() -> f64 {
    8.0
}
fn default_structure_border_gap() -> u16 {
    16
}

/// Immutable generation switches for the authored structure kinds.
/// Legacy royal kinds shared `villages`; missing new flags inherit that setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StructureSettings {
    pub villages: bool,
    pub ruins: bool,
    pub mines: bool,
    pub kingdoms: bool,
    pub outposts: bool,
}
impl Default for StructureSettings {
    fn default() -> Self {
        Self {
            villages: true,
            ruins: true,
            mines: true,
            kingdoms: true,
            outposts: true,
        }
    }
}
impl<'de> Deserialize<'de> for StructureSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(default, deny_unknown_fields)]
            struct Saved {
                villages: bool,
                ruins: bool,
                mines: bool,
                kingdoms: Option<bool>,
                outposts: Option<bool>,
            }
            impl Default for Saved {
                fn default() -> Self {
                    Self {
                        villages: true,
                        ruins: true,
                        mines: true,
                        kingdoms: None,
                        outposts: None,
                    }
                }
            }
            let saved = Saved::deserialize(deserializer)?;
            Ok(Self {
                villages: saved.villages,
                ruins: saved.ruins,
                mines: saved.mines,
                kingdoms: saved.kingdoms.unwrap_or(saved.villages),
                outposts: saved.outposts.unwrap_or(saved.villages),
            })
        } else {
            // Protocol peers use the five concrete booleans written by Serialize,
            // without Option discriminants. Old wire layouts are version-rejected.
            #[derive(Deserialize)]
            struct Wire {
                villages: bool,
                ruins: bool,
                mines: bool,
                kingdoms: bool,
                outposts: bool,
            }
            let wire = Wire::deserialize(deserializer)?;
            Ok(Self {
                villages: wire.villages,
                ruins: wire.ruins,
                mines: wire.mines,
                kingdoms: wire.kingdoms,
                outposts: wire.outposts,
            })
        }
    }
}
impl StructureSettings {
    pub fn set_all(&mut self, enabled: bool) {
        self.villages = enabled;
        self.ruins = enabled;
        self.mines = enabled;
        self.kingdoms = enabled;
        self.outposts = enabled;
    }
    pub fn any_enabled(self) -> bool {
        self.villages || self.ruins || self.mines || self.kingdoms || self.outposts
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldConfig {
    /// Saved ecology: 6 regional flora/cacti, 7 rooted birches, 8 buried Spirit fog stones,
    /// 9 fantasy arches/leaning spires and curled botanical clumps,
    /// 10 monumental arches, 11 widely separated landmarks, 12 woodland giants and glades,
    /// 13 Plains meadows and farmsteads, taller mushroom groves and rarer pavilions.
    /// 14 wooded trails and desert oases, 15 denser woods, sparse mesas, ocean and highland ecology; 16 volcanic provinces, 17 savanna grasslands, 18 willow swamps, 19 petrified forests.
    /// Missing in older saves retains their original ecology.
    #[serde(default)]
    pub ecology_revision: u8,
    /// Spirit terrain contract: 1 blossom groves, 2 wetlands and grave sites.
    /// Absent in saved worlds means no retrogen.
    #[serde(default)]
    pub spirit_blossom_revision: u8,
    /// Saved Blood Forest contract; absent in old saves disables retrogen.
    #[serde(default)]
    pub blood_forest_revision: u8,
    /// Natural population cap within 150 m of each player; owned animals are excluded.
    pub natural_mob_limit: u16,
    /// Loaded days on a straw nest before a dragon egg can hatch.
    pub dragon_incubation_days: u8,
    /// New saved terrain contract; absent in existing worlds means no retrogen.
    #[serde(default)]
    pub bounded_world: bool,
    /// Revision 2 grades both ravine banks. Earlier saves retain their own cross-sections.
    #[serde(default)]
    pub ravine_revision: u8,
    /// Saved hydrology: 1 eroded banks, 2 connected rivers with distinct outlets,
    /// 3 tributaries and broader temperate brook habitats on the main terrain.
    /// 4 adds more compact temperate brook catchments without changing regional rivers.
    /// 5 bounds voxelized channels to 6–22 blocks across the current.
    /// 6 adds sweeping courses, deeper plunge pools and open sculpted gorges.
    /// 7 adds variable long catchments, distant tributaries and lake forks.
    /// Missing in existing worlds preserves their terrain.
    #[serde(default)]
    pub river_terrain_revision: u8,
    pub time: crate::WorldTimeConfig,
    pub seasons: crate::SeasonConfig,
    pub game_rules: crate::GameRules,
    #[serde(default)]
    pub kingdom_rules: crate::kingdom::KingdomRules,
    pub id: WorldId,
    pub seed: WorldSeed,
    pub generator_version: u32,
    pub base_height: f64,
    pub hill_amplitude: f64,
    pub mountain_amplitude: f64,
    #[serde(default = "legacy_sea_level")]
    pub sea_level: f64,
    pub snow_height: f64,
    pub caves: bool,
    /// Saved landscape feature; missing in older saves preserves their cave field.
    #[serde(default)]
    pub geology_revision: u8,
    /// Saved terrain: 0 legacy, 1 drainage, 2 varied relief, 3 connected hydrology,
    /// 4 JSON biome bands, 5 landscape profiles and erosion, 6 fantasy macro-relief,
    /// 7 regional mountain ranges, 8 broader thermal regions and varied lowland districts.
    /// 9 gives land biomes a regional interior and blends their borders.
    /// Missing in existing worlds preserves the legacy heightfield.
    #[serde(default)]
    pub landscape_revision: u8,
    /// Immutable JSON profile snapshot. Missing in old saves deliberately stays absent.
    #[serde(default)]
    pub biome_definitions: Option<super::BiomeDefinitions>,
    /// Missing in saved worlds preserves their seabed and unfrozen water.
    #[serde(default)]
    pub ocean_revision: u8,
    /// Saved cave ecology/hydrology contract; absent in old saves disables retrogen.
    #[serde(default)]
    pub cave_structure_revision: u8,
    /// Saved settlement/ruin/mine contract. Fresh worlds opt in; missing saved fields disable retrogen.
    #[serde(default)]
    pub structures_revision: u8,
    /// Immutable minimum royal-site border gap for generation revision 9+.
    #[serde(default = "default_structure_border_gap")]
    pub structure_border_gap: u16,
    /// Saved generation choices; default preserves all authored sites.
    pub structures: StructureSettings,
    /// Initial safe spawn preference only; never changes the generated biome field.
    /// Ocean selects a safe shore near marine water rather than an underwater spawn.
    #[serde(default)]
    pub preferred_spawn_biome: Option<super::BiomeKind>,
    pub cave_biomes: super::CaveBiomesConfig,
    pub trees: bool,
    pub water: bool,
    /// Opt-in saved feature; absent in existing worlds means unchanged terrain.
    #[serde(default)]
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
    /// Historic world manifests can omit the ocean profile.
    pub fn ocean_biome_config(&self) -> &super::OceanBiomeConfig {
        self.biome_definitions
            .as_ref()
            .and_then(|defs| defs.get(super::BiomeKind::Ocean).ocean.as_ref())
            .unwrap_or_else(|| {
                super::BiomeDefinitions::builtin()
                    .get(super::BiomeKind::Ocean)
                    .ocean
                    .as_ref()
                    .expect("validated ocean defaults")
            })
    }

    pub const CURRENT_ECOLOGY_REVISION: u8 = 22;
    pub const CURRENT_STRUCTURES_REVISION: u8 = 16;
    pub const CURRENT_CAVE_STRUCTURE_REVISION: u8 = 13;
    pub const CURRENT_LANDSCAPE_REVISION: u8 = 9;
    pub const CURRENT_RIVER_TERRAIN_REVISION: u8 = 7;
    pub const CURRENT_RAVINE_REVISION: u8 = 2;

    /// Prepared terrain is independent of live calendar, population and game rules.
    pub fn same_terrain(&self, other: &Self) -> bool {
        // Exhaustive destructuring makes new settings an explicit cache-identity decision.
        let Self {
            ecology_revision,
            spirit_blossom_revision,
            blood_forest_revision,
            natural_mob_limit: _,
            dragon_incubation_days: _,
            time: _,
            seasons: _,
            game_rules: _,
            kingdom_rules: _,
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
            biome_definitions,
            ocean_revision,
            cave_structure_revision,
            structures_revision,
            structure_border_gap,
            structures,
            preferred_spawn_biome: _,
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
        blood_forest_revision == &other.blood_forest_revision
            && spirit_blossom_revision == &other.spirit_blossom_revision
            && ecology_revision == &other.ecology_revision
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
            && biome_definitions == &other.biome_definitions
            && ocean_revision == &other.ocean_revision
            && cave_structure_revision == &other.cave_structure_revision
            && structures_revision == &other.structures_revision
            && structure_border_gap == &other.structure_border_gap
            && structures == &other.structures
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
            ecology_revision: Self::CURRENT_ECOLOGY_REVISION,
            spirit_blossom_revision: 2,
            blood_forest_revision: 1,
            natural_mob_limit: 256,
            dragon_incubation_days: 5,
            bounded_world: true,
            ravine_revision: Self::CURRENT_RAVINE_REVISION,
            river_terrain_revision: Self::CURRENT_RIVER_TERRAIN_REVISION,
            time: crate::WorldTimeConfig::default(),
            seasons: crate::SeasonConfig::default(),
            game_rules: crate::GameRules::default(),
            kingdom_rules: crate::kingdom::KingdomRules::default(),
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
            landscape_revision: Self::CURRENT_LANDSCAPE_REVISION,
            biome_definitions: None,
            ocean_revision: 1,
            cave_structure_revision: Self::CURRENT_CAVE_STRUCTURE_REVISION,
            structures_revision: Self::CURRENT_STRUCTURES_REVISION,
            structure_border_gap: default_structure_border_gap(),
            structures: StructureSettings::default(),
            preferred_spawn_biome: None,
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
            self.ecology_revision <= Self::CURRENT_ECOLOGY_REVISION,
            "Unsupported world.ecology_revision",
        )?;
        ensure(
            self.blood_forest_revision <= 1,
            "Unsupported world.blood_forest_revision",
        )?;
        ensure(
            self.structures_revision <= Self::CURRENT_STRUCTURES_REVISION,
            "Unsupported world.structures_revision",
        )?;
        ensure(self.game_rules.valid(), "Invalid world.game_rules")?;
        ensure(self.kingdom_rules.valid(), "Invalid world.kingdom_rules")?;
        ensure(
            self.structure_border_gap <= 4096,
            "Invalid world.structure_border_gap",
        )?;
        ensure(
            self.cave_structure_revision <= Self::CURRENT_CAVE_STRUCTURE_REVISION,
            "Unsupported world.cave_structure_revision",
        )?;
        ensure(
            self.spirit_blossom_revision <= 2,
            "Unsupported world.spirit_blossom_revision",
        )?;
        self.cave_biomes.validate()?;
        ensure(self.ocean_revision <= 1, "Unsupported world.ocean_revision")?;
        ensure(
            self.ravine_revision <= Self::CURRENT_RAVINE_REVISION,
            "Unsupported world.ravine_revision",
        )?;
        ensure(
            self.river_terrain_revision <= Self::CURRENT_RIVER_TERRAIN_REVISION,
            "Unsupported world.river_terrain_revision",
        )?;
        ensure(
            self.natural_mob_limit <= 4096,
            "world.natural_mob_limit must be 0..=4096",
        )?;
        ensure(
            self.landscape_revision <= Self::CURRENT_LANDSCAPE_REVISION,
            "Unsupported world.landscape_revision",
        )?;
        if let Some(definitions) = &self.biome_definitions {
            definitions.validate()?;
        }
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
            (3..=7).contains(&self.dragon_incubation_days),
            "world.dragon_incubation_days must be in 3..=7",
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
