use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub ambience_volume: f32,
    pub animals_volume: f32,
    pub footsteps_volume: f32,
    pub water_volume: f32,
    pub river_volume: f32,
    pub waterfall_volume: f32,
    pub muted: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            ambience_volume: 1.0,
            animals_volume: 1.0,
            footsteps_volume: 1.0,
            water_volume: 1.0,
            river_volume: 1.0,
            waterfall_volume: 1.0,
            muted: false,
        }
    }
}

impl Config for AudioConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        for (volume, name) in [
            (self.master_volume, "Master volume"),
            (self.ambience_volume, "Ambience volume"),
            (self.animals_volume, "Animals volume"),
            (self.footsteps_volume, "Footsteps volume"),
            (self.water_volume, "Water volume"),
            (self.river_volume, "River volume"),
            (self.waterfall_volume, "Waterfall volume"),
        ] {
            ensure(
                volume.is_finite() && (0.0..=1.0).contains(&volume),
                &format!("{name} must be between 0 and 1"),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_audio_fields_keep_defaults_and_invalid_gains_are_rejected() {
        let partial = AudioConfig::from_toml("river_volume = 0.4\nmuted = true").unwrap();
        assert_eq!(partial.river_volume, 0.4);
        assert_eq!(partial.master_volume, 1.0);
        assert_eq!(partial.animals_volume, 1.0);
        assert_eq!(partial.footsteps_volume, 1.0);
        assert!(partial.muted);
        assert!(AudioConfig::from_toml("master_volum = 0.5").is_err());
        for value in ["nan", "inf", "-0.1", "1.1"] {
            for field in [
                "master_volume",
                "ambience_volume",
                "animals_volume",
                "footsteps_volume",
                "water_volume",
                "river_volume",
                "waterfall_volume",
            ] {
                assert!(AudioConfig::from_toml(&format!("{field} = {value}")).is_err());
            }
        }
    }

    #[test]
    fn legacy_audio_keeps_existing_levels_and_new_categories_round_trip() {
        let old = AudioConfig::from_toml(
            "master_volume = 0.8\nambience_volume = 0.6\nwater_volume = 0.4\nriver_volume = 0.3\nwaterfall_volume = 0.2\nmuted = true\n",
        ).unwrap();
        assert_eq!(old.master_volume, 0.8);
        assert_eq!(old.ambience_volume, 0.6);
        assert_eq!(old.water_volume, 0.4);
        assert_eq!(old.river_volume, 0.3);
        assert_eq!(old.waterfall_volume, 0.2);
        assert!(old.muted);
        assert_eq!((old.animals_volume, old.footsteps_volume), (1.0, 1.0));
        let current = AudioConfig {
            animals_volume: 0.25,
            footsteps_volume: 0.75,
            ..old
        };
        let serialized = toml::to_string(&current).unwrap();
        assert_eq!(AudioConfig::from_toml(&serialized).unwrap(), current);
    }
}
