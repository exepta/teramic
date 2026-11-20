mod audio;
mod biomes;
mod terrain;
pub use biomes::{
    BiomeClimate, BiomeDefinition, BiomeDefinitions, BiomeKind, BiomeTerrain, MarineFloorWeights,
    MarineHabitatConfig, OceanBiomeConfig,
};
pub use terrain::{LandscapeDefinitions, LandscapeProfile, TerrainKind};
mod block_details;
pub use block_details::{BlockDetailConfig, BlockDetailQuality};
mod cave_biomes;
pub use cave_biomes::{CaveBiomesConfig, CaveConnectors, CaveDistribution, CaveRegions};
mod client;
#[cfg(feature = "controls")]
pub mod controls;
mod error;
mod game;
mod gameplay;
mod graphics;
mod load;
mod multiplayer;
mod persist;
mod server;
pub mod servers;
mod world;

pub use audio::AudioConfig;
pub use client::{ChunkAnimation, ClientConfig, WaterQuality, WeatherKind, WeatherQuality};
pub use error::ConfigError;
pub use game::GameConfig;
pub use gameplay::{
    GameLanguage, GameplayConfig, LanguagePreference, MapMode, MinimapConfig, MobHealthBars,
};
pub use graphics::{
    CaveQuality, GraphicsConfig, ShadowQuality, TreeFallAnimation, WindowModeSetting,
};
pub use load::{Config, config_directory, menu_config_directory};
pub use multiplayer::MultiplayerConfig;
pub use persist::save_toml;
pub use server::{PlayerPermissions, ServerConfig};
pub use world::{StructureSettings, WorldConfig};

fn ensure(condition: bool, message: &str) -> Result<(), ConfigError> {
    if condition {
        Ok(())
    } else {
        Err(ConfigError::Validation(message.into()))
    }
}

#[cfg(test)]
mod tests;
