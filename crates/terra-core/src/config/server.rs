use super::{Config, ConfigError, ensure};
use crate::DEFAULT_TICK_RATE;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlayerPermissions {
    #[serde(default)]
    pub game_mode: bool,
    pub build: bool,
    pub flight: bool,
    pub environment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// None uses the saved world's limit, including integrated singleplayer worlds.
    pub natural_mob_limit: Option<u16>,
    pub save_directory: Option<std::path::PathBuf>,
    pub title: String,
    pub description: String,
    pub lan_discovery: bool,
    /// No online account verifier is configured yet; never silently accept public profile claims.
    pub require_verified_accounts: bool,
    pub bind_address: SocketAddr,
    pub tick_rate: u16,
    pub max_players: u16,
    pub log_filter: String,
    pub simulation_distance: u8,
    pub allow_flight: bool,
    pub game_mode: crate::GameMode,
    pub player_permissions: std::collections::BTreeMap<crate::PlayerUuid, PlayerPermissions>,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            natural_mob_limit: None,
            save_directory: None,
            title: "TERAMIC Server".into(),
            description: "A shared voxel world".into(),
            lan_discovery: true,
            require_verified_accounts: false,
            bind_address: SocketAddr::from(([127, 0, 0, 1], 5000)),
            tick_rate: DEFAULT_TICK_RATE,
            max_players: 32,
            log_filter: "info".into(),
            simulation_distance: 1,
            allow_flight: false,
            game_mode: crate::GameMode::Survival,
            player_permissions: Default::default(),
        }
    }
}
impl ServerConfig {
    pub fn permissions_for(&self, uuid: crate::PlayerUuid) -> PlayerPermissions {
        self.player_permissions
            .get(&uuid)
            .copied()
            .unwrap_or(PlayerPermissions {
                game_mode: false,
                build: true,
                flight: self.allow_flight,
                environment: self.allow_flight,
            })
    }
}
impl Config for ServerConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            self.natural_mob_limit.is_none_or(|n| n <= 4096),
            "server.natural_mob_limit must be 0..=4096",
        )?;
        ensure(
            self.player_permissions.len() <= 1024
                && self.player_permissions.keys().all(|uuid| !uuid.is_nil()),
            "Invalid player permissions: maximum 1024 non-nil UUID entries",
        )?;
        ensure(
            !self.title.trim().is_empty()
                && self.title.chars().count() <= 64
                && !self.title.chars().any(char::is_control)
                && self.description.chars().count() <= 160
                && !self.description.chars().any(char::is_control),
            "Invalid server title or description",
        )?;
        ensure(
            self.simulation_distance <= 8,
            "server.simulation_distance must be 0..=8",
        )?;
        ensure(
            (1..=240).contains(&self.tick_rate),
            "server.tick_rate must be 1..=240",
        )?;
        ensure(
            (1..=128).contains(&self.max_players),
            "server.max_players must be 1..=128",
        )?;
        ensure(
            self.bind_address.port() != 0,
            "server.bind_address port must not be zero",
        )?;
        ensure(
            !self.log_filter.trim().is_empty(),
            "server.log_filter must not be empty",
        )
    }
}
