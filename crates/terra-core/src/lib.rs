//! Shared identities, validated configuration and optional ECS integration.
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
mod food;
#[cfg(feature = "bevy")]
pub mod footsteps;
pub mod furnace;
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
pub use map_palette::MapPalette;
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
    AudioConfig, BlockDetailConfig, BlockDetailQuality, ChunkAnimation, ClientConfig, Config,
    ConfigError, GameConfig, GameplayConfig, MapMode, MinimapConfig, MobHealthBars,
    MultiplayerConfig, PlayerPermissions, ServerConfig, WaterQuality, WeatherKind, WeatherQuality,
    WorldConfig, config_directory,
};
pub use config::{
    CaveQuality, GraphicsConfig, ShadowQuality, TreeFallAnimation, WindowModeSetting,
    menu_config_directory, save_toml,
};
pub use constants::*;
#[cfg(feature = "bevy")]
pub use debug::DebugSettings;
pub use gameplay::{
    BellowsKind, DROP_LIFETIME_SECONDS, GameMode, GameplayState, INVENTORY_SLOTS, Inventory,
    ItemKind, ItemStack, MAX_WORLD_DROPS, PlayerStats, ResourcePropKind, STACK_LIMIT, ToolKind,
    WoodSpecies, WorldDrop, crafting_preview,
};
pub use identity::{PlayerId, WorldId, WorldSeed};
pub use paths::asset_directory;
#[cfg(feature = "bevy")]
pub use plugin::TerraCorePlugin;
pub use profile::{PlayerGender, PlayerProfile};
pub use uuid::Uuid as PlayerUuid;
pub mod beds;
pub mod block_properties;
pub mod block_shapes;
pub mod construction;
pub mod crafting;
pub mod flower_pot;
pub mod textiles;
