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
impl GameConfig {
    /// Both dedicated and integrated authorities use the same live server policy.
    pub fn pvp_enabled(&self) -> bool {
        self.server.pvp.unwrap_or(self.world.game_rules.pvp)
    }

    /// Publish exactly the rules enforced by the authority, including server overrides.
    pub fn effective_world_rules(&self) -> crate::WorldRuleSettings {
        let mut rules = crate::WorldRuleSettings::from_world(&self.world);
        rules.game_rules.pvp = self.pvp_enabled();
        rules
    }
}
impl Config for GameConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.client.validate()?;
        self.server.validate()?;
        self.world.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_pvp_policy_preserves_old_worlds_and_explicit_server_policy_is_authoritative() {
        for saved_rule in [false, true] {
            let mut game = GameConfig::from_toml("").unwrap();
            game.world.game_rules.pvp = saved_rule;
            assert_eq!(game.server.pvp, None);
            assert_eq!(game.pvp_enabled(), saved_rule);
            for policy in [None, Some(false), Some(true)] {
                game.server.pvp = policy;
                let expected = policy.unwrap_or(saved_rule);
                assert_eq!(game.pvp_enabled(), expected);
                assert_eq!(game.effective_world_rules().game_rules.pvp, expected);
                let roundtrip = GameConfig::from_toml(&toml::to_string(&game).unwrap()).unwrap();
                assert_eq!(roundtrip.server.pvp, policy);
                assert_eq!(roundtrip.pvp_enabled(), expected);
                assert_eq!(
                    game.world.game_rules.pvp, saved_rule,
                    "policy never rewrites the saved rule"
                );
            }
        }
        assert!(ServerConfig::from_toml("pvp = 'yes'").is_err());
        assert!(ServerConfig::from_toml("pvp = 1").is_err());
    }
}
