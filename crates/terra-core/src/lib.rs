//! Shared identities, validated configuration and optional ECS integration.
pub mod block_loot;
pub mod brewing;
pub mod building_jobs;
pub mod campfire;
pub mod castle;
#[cfg(test)]
mod cherry_tests;
pub mod config;
mod constants;
pub mod containers;
pub mod creatures;
pub mod crusher;
#[cfg(feature = "bevy")]
mod debug;
pub mod effects;
pub mod equipment;
pub mod farming;
pub mod fog_blocks;
mod food;
#[cfg(feature = "bevy")]
pub mod footsteps;
pub mod furnace;
pub mod grave_blocks;
pub mod infusion;
pub mod inventory_sort;
pub mod kinetic;
pub mod lantern;
pub mod locomotive_parts;
pub mod logistics;
pub mod mixer;
pub mod npc_jobs;
pub mod plumbing;
pub mod profession_stations;
pub mod rails;
pub mod roads;
pub mod street;
pub mod train_blueprint;
pub mod train_motion;
pub mod train_parts;
pub mod trains;
pub use crusher::DustKind;
pub use effects::{
    ActiveEffect, EffectDefinition, EffectKind, EffectStacking, StatusEffects,
    effect_catalog_fingerprint, effect_definition,
};
pub use equipment::{
    AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, CapeItem, EQUIPMENT_SLOTS,
    EquipmentBonuses, EquipmentDefinition, EquipmentSlot, EquipmentSlots, MAX_ARMOR_POINTS,
    equipment_catalog_fingerprint, equipment_definition,
};
pub use farming::{CropItem, CropKind, CropState};
pub use food::{FoodDefinition, FoodRegeneration, food};
mod gameplay;
mod identity;
mod map_palette;
pub use map_palette::{MapPalette, relative_grass_tint};
pub mod minerals;
#[cfg(feature = "bevy")]
pub mod miniature;
pub mod profiling;
pub use minerals::{MineralItem, OreGeneration, OreKind, OreVeinConfig};
mod game_rules;
mod paths;
mod player_location;
#[cfg(feature = "bevy")]
mod plugin;
mod profile;
mod server_characters;
pub use player_location::PlayerLocation;
mod seasons;
mod survival;
mod world_time;
pub use game_rules::{GameRule, GameRules, WorldRuleSettings};
pub use seasons::{Season, SeasonConfig, SeasonSample};
pub use survival::{
    DamageCause, MAX_VITAL, SurvivalEffort, SurvivalLifeOutcome, SurvivalVitals, nutrition,
};
pub use world_time::{WorldClock, WorldTimeConfig};
mod weather;
pub use weather::{LightningEvent, LightningObserver, SnowProfile, WeatherPattern, WorldWeather};

#[cfg(feature = "controls")]
pub use config::controls::{ControlsConfig, GameAction, GameplayInput, InputBinding};
pub use config::{
    AudioConfig, BiomeClimate, BiomeDefinition, BiomeDefinitions, BiomeKind, BiomeTerrain,
    BlockDetailConfig, BlockDetailQuality, ChunkAnimation, ClientConfig, Config, ConfigError,
    GameConfig, GameLanguage, GameplayConfig, LandscapeDefinitions, LandscapeProfile,
    LanguagePreference, MapMode, MarineFloorWeights, MarineHabitatConfig, MinimapConfig,
    MobHealthBars, MultiplayerConfig, OceanBiomeConfig, PlayerPermissions, ServerConfig,
    StructureSettings, TerrainKind, WaterQuality, WeatherKind, WeatherQuality, WorldConfig,
    config_directory,
};
pub use config::{
    CaveQuality, GraphicsConfig, ShadowQuality, TreeFallAnimation, WindowModeSetting,
    menu_config_directory, save_toml,
};
pub use constants::*;
#[cfg(feature = "bevy")]
pub use debug::{DebugSettings, HudVisibility};
pub use gameplay::{
    BellowsKind, DROP_LIFETIME_SECONDS, GameMode, GameplayState, INVENTORY_SLOTS, Inventory,
    ItemKind, ItemStack, MAX_WORLD_DROPS, PlayerStats, ResourcePropKind, STACK_LIMIT, ToolKind,
    WoodSpecies, WorldDrop, crafting_preview,
};
pub use identity::{PlayerId, WorldId, WorldSeed};
pub use paths::{asset_directory, installation_root};
#[cfg(feature = "bevy")]
pub use plugin::TerraCorePlugin;
pub use profile::{
    CharacterAppearance, CharacterSelection, ClothingMaterial, PlayerGender, PlayerProfile,
};
pub use server_characters::ServerCharacterProfiles;
pub use uuid::Uuid as PlayerUuid;
pub mod beds;
pub mod block_properties;
pub mod block_shapes;
pub mod construction;
pub mod crafting;
pub mod flower_pot;
pub mod kingdom;
pub mod textiles;
pub mod tints;

pub mod trees;
pub use trees::{FruitPhase, FruitState, TreeRecord, TreeRegistry, TreeSpecies, TreeStage};

pub mod train_boiler;

#[cfg(feature = "bevy")]
mod initial_ticks;

pub mod train_power;

#[cfg(feature = "bevy")]
pub mod work_areas;

pub mod bile;
pub mod biome_flora;
pub mod blood_blocks;
pub mod fluid_surface;
