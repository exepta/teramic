use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MultiplayerConfig {
    pub player_names: bool,
    pub player_name_distance: f32,
    pub chest_cursors: bool,
    /// None chooses a stable, saturated color from the authenticated profile UUID.
    pub cursor_color: Option<[u8; 3]>,
}
impl Default for MultiplayerConfig {
    fn default() -> Self {
        Self {
            player_names: true,
            player_name_distance: 40.0,
            chest_cursors: true,
            cursor_color: None,
        }
    }
}
impl MultiplayerConfig {
    pub fn color(&self, uuid: crate::PlayerUuid) -> [u8; 3] {
        self.cursor_color.unwrap_or_else(|| {
            let bytes = uuid.as_bytes();
            let hue = (u16::from(bytes[0]) * 256 + u16::from(bytes[1])) % 1536;
            let t = (hue % 256) as u8;
            match hue / 256 {
                0 => [240, 56 + (u16::from(t) * 184 / 255) as u8, 56],
                1 => [240 - (u16::from(t) * 184 / 255) as u8, 240, 56],
                2 => [56, 240, 56 + (u16::from(t) * 184 / 255) as u8],
                3 => [56, 240 - (u16::from(t) * 184 / 255) as u8, 240],
                4 => [56 + (u16::from(t) * 184 / 255) as u8, 56, 240],
                _ => [240, 56, 240 - (u16::from(t) * 184 / 255) as u8],
            }
        })
    }
}
impl Config for MultiplayerConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            self.player_name_distance.is_finite()
                && (8.0..=150.0).contains(&self.player_name_distance),
            "Player name distance must be between 8 and 150 meters",
        )?;
        ensure(
            self.cursor_color.is_none_or(|c| {
                c.iter()
                    .max()
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(c.iter().min().copied().unwrap_or(0))
                    >= 40
            }),
            "Cursor color must be chromatic, not black, white or gray",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_profile_colors_are_stable_chromatic_and_validated() {
        let config = MultiplayerConfig::from_toml("").unwrap();
        assert!(config.player_names && config.chest_cursors);
        assert_eq!(config.player_name_distance, 40.0);
        assert!(MultiplayerConfig::from_toml("player_name_distance = 200").is_err());
        assert!(MultiplayerConfig::from_toml("player_name_distance = 72").is_ok());
        assert!(config.cursor_color.is_none());
        let mut colors = std::collections::BTreeSet::new();
        for i in 0..256u128 {
            let id = crate::PlayerUuid::from_u128(i << 112);
            let color = config.color(id);
            assert_eq!(color, config.color(id));
            assert!(color.iter().max().unwrap() - color.iter().min().unwrap() >= 100);
            colors.insert(color);
        }
        assert!(colors.len() > 100);
        assert!(MultiplayerConfig::from_toml("cursor_color = [128, 128, 128]").is_err());
        assert!(MultiplayerConfig::from_toml("cursor_color = [255, 128, 32]").is_ok());
    }
}
