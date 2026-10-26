use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapMode {
    #[default]
    Auto,
    Surface,
    Cave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobHealthBars {
    InView,
    #[default]
    LookingAt,
    Always,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MinimapConfig {
    pub enabled: bool,
    pub size: u16,
    pub zoom: u8,
    pub fps: u8,
    pub icon_scale: f32,
    pub mode: MapMode,
    pub rotate: bool,
    pub relief: bool,
    pub mobs: bool,
    pub players: bool,
    pub coordinates: bool,
    pub biome: bool,
    pub clock: bool,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size: 260,
            zoom: 1,
            fps: 30,
            icon_scale: 1.0,
            mode: MapMode::Auto,
            rotate: false,
            relief: true,
            mobs: true,
            players: true,
            coordinates: true,
            biome: true,
            clock: true,
        }
    }
}

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameplayConfig {
    pub minimap: MinimapConfig,
    pub heartbeat: bool,
    pub view_bobbing: bool,
    pub mob_health_bars: MobHealthBars,
    #[serde(deserialize_with = "deserialize_rgb")]
    pub mob_health_friendly_color: [u8; 3],
    #[serde(deserialize_with = "deserialize_rgb")]
    pub mob_health_hostile_color: [u8; 3],
    #[serde(deserialize_with = "deserialize_rgb")]
    pub mob_health_font_color: [u8; 3],
}

fn deserialize_rgb<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<[u8; 3], D::Error> {
    // Deserialize a sequence explicitly: TOML's fixed-array tuple path may
    // accept a valid prefix while leaving additional components unread.
    Vec::<u8>::deserialize(deserializer)?
        .try_into()
        .map_err(|_: Vec<u8>| {
            serde::de::Error::custom("RGB colors require exactly three components")
        })
}

impl Default for GameplayConfig {
    fn default() -> Self {
        Self {
            minimap: MinimapConfig::default(),
            heartbeat: true,
            view_bobbing: true,
            mob_health_bars: MobHealthBars::default(),
            mob_health_friendly_color: [105, 203, 134],
            mob_health_hostile_color: [223, 98, 93],
            mob_health_font_color: [255; 3],
        }
    }
}

impl Config for GameplayConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            (180..=360).contains(&self.minimap.size),
            "MiniMap size must be between 180 and 360 pixels",
        )?;
        ensure(
            self.minimap.zoom <= 3,
            "MiniMap zoom must be between 0 and 3",
        )?;
        ensure(
            [24, 30, 40, 60].contains(&self.minimap.fps),
            "MiniMap FPS must be 24, 30, 40 or 60",
        )?;
        ensure(
            self.minimap.icon_scale.is_finite() && (1.0..=2.5).contains(&self.minimap.icon_scale),
            "MiniMap icon scale must be between 1 and 2.5",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_defaults_partial_configs_and_bounds_are_validated() {
        let default = GameplayConfig::from_toml("").unwrap();
        assert_eq!(default, GameplayConfig::default());
        assert!(default.heartbeat);
        assert!(default.view_bobbing);
        assert_eq!(default.mob_health_bars, MobHealthBars::LookingAt);
        for (value, expected) in [
            ("in_view", MobHealthBars::InView),
            ("looking_at", MobHealthBars::LookingAt),
            ("always", MobHealthBars::Always),
            ("none", MobHealthBars::None),
        ] {
            let config =
                GameplayConfig::from_toml(&format!("mob_health_bars = '{value}'")).unwrap();
            assert_eq!(config.mob_health_bars, expected);
            assert_eq!(
                GameplayConfig::from_toml(&toml::to_string(&config).unwrap()).unwrap(),
                config
            );
        }
        assert!(GameplayConfig::from_toml("mob_health_bars = 'invalid'").is_err());
        for invalid in [
            "mob_health_friendly_color = [256, 0, 0]",
            "mob_health_hostile_color = [-1, 0, 0]",
            "mob_health_font_color = [255, 255]",
            "mob_health_font_color = [1, 2, 3, 4]",
            "mob_health_friendly_color = '#69cb86'",
        ] {
            assert!(GameplayConfig::from_toml(invalid).is_err(), "{invalid}");
        }
        for field in [
            "mob_health_friendly_color",
            "mob_health_hostile_color",
            "mob_health_font_color",
        ] {
            for value in ["[]", "[1]", "[1, 2]", "[1, 2, 3, 4]", "[1, 2, 3, 4, 5]"] {
                let invalid = format!("{field} = {value}");
                assert!(GameplayConfig::from_toml(&invalid).is_err(), "{invalid}");
            }
            let valid = format!("{field} = [0, 128, 255]");
            assert!(GameplayConfig::from_toml(&valid).is_ok(), "{valid}");
        }
        let colored = GameplayConfig::from_toml("mob_health_friendly_color = [12, 34, 56]\nmob_health_hostile_color = [210, 110, 30]\nmob_health_font_color = [240, 230, 220]").unwrap();
        assert_eq!(colored.mob_health_friendly_color, [12, 34, 56]);
        assert_eq!(colored.mob_health_hostile_color, [210, 110, 30]);
        assert_eq!(colored.mob_health_font_color, [240, 230, 220]);
        assert_eq!(
            GameplayConfig::from_toml(&toml::to_string(&colored).unwrap()).unwrap(),
            colored
        );
        assert!(
            !GameplayConfig::from_toml("view_bobbing = false")
                .unwrap()
                .view_bobbing
        );
        assert!(
            !GameplayConfig::from_toml("heartbeat = false")
                .unwrap()
                .heartbeat
        );
        let custom = GameplayConfig::from_toml(
            "[minimap]\nmode = 'cave'\nsize = 360\nzoom = 3\nplayers = false\nfps = 60\nicon_scale = 2.5",
        )
        .unwrap();
        assert_eq!(custom.minimap.mode, MapMode::Cave);
        assert_eq!(custom.minimap.size, 360);
        assert_eq!(default.minimap.fps, 30);
        assert_eq!(default.minimap.icon_scale, 1.0);
        assert_eq!(custom.minimap.fps, 60);
        assert_eq!(custom.minimap.icon_scale, 2.5);
        assert!(custom.minimap.mobs && !custom.minimap.players);
        for input in [
            "[minimap]\nsize = 179",
            "[minimap]\nsize = 361",
            "[minimap]\nzoom = 4",
            "[minimap]\nfps = 25",
            "[minimap]\nfps = 0",
            "[minimap]\nicon_scale = 0.9",
            "[minimap]\nicon_scale = 2.6",
            "[minimap]\nicon_scale = nan",
            "[minimap]\nicon_scale = inf",
            "[minimap]\nmode = 'fake'",
            "[minimap]\nunrecognized = true",
        ] {
            assert!(GameplayConfig::from_toml(input).is_err(), "{input}");
        }
        let toml = toml::to_string(&custom).unwrap();
        assert_eq!(GameplayConfig::from_toml(&toml).unwrap(), custom);
    }
}
