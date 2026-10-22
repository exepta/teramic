use super::{ClientConfig, Config, ConfigError, ServerConfig, WorldConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct GameConfig {
    pub client: ClientConfig,
    pub server: ServerConfig,
    pub world: WorldConfig,
}
impl Config for GameConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.client.validate()?;
        self.server.validate()?;
        self.world.validate()
    }
}
