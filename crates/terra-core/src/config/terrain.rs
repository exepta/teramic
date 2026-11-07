use super::{ConfigError, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainKind {
    Plains,
    RollingPlains,
    Forest,
    MountainForest,
    FantasyMountains,
    SnowyHighlands,
    CherryGrove,
    SpiritForest,
    Desert,
    Ocean,
}

impl TerrainKind {
    pub const ALL: [Self; 10] = [
        Self::Plains,
        Self::RollingPlains,
        Self::Forest,
        Self::MountainForest,
        Self::FantasyMountains,
        Self::SnowyHighlands,
        Self::CherryGrove,
        Self::SpiritForest,
        Self::Desert,
        Self::Ocean,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Plains => "Hilly Plains",
            Self::RollingPlains => "Rolling Plains",
            Self::Forest => "Forest",
            Self::MountainForest => "Mountain Forest",
            Self::FantasyMountains => "Fantasy Mountains",
            Self::SnowyHighlands => "Snowy Highlands",
            Self::CherryGrove => "Cherry Grove",
            Self::SpiritForest => "Spirit Forest",
            Self::Desert => "Desert",
            Self::Ocean => "Ocean & Coast",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandscapeProfile {
    pub kind: TerrainKind,
    pub min_height: f64,
    pub max_height: f64,
    pub horizontal_scale: f64,
    pub roughness: f64,
    pub ridge_strength: f64,
    pub erosion: f64,
    pub overhangs: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandscapeDefinitions {
    pub warp_scale: f64,
    pub warp_strength: f64,
    pub region_scale: f64,
    pub dune_wind_angle: f64,
    pub profiles: [LandscapeProfile; 10],
}

impl Default for LandscapeDefinitions {
    fn default() -> Self {
        serde_json::from_str(include_str!(
            "../../../../assets/biome/terrain_profiles.json"
        ))
        .expect("valid built-in landscape profiles")
    }
}

impl LandscapeDefinitions {
    /// Immutable fallback for saved worlds created before the lower mountain defaults.
    pub fn legacy_v5() -> Self {
        serde_json::from_str(include_str!("terrain/legacy_v5.json"))
            .expect("valid immutable revision-five landscape profiles")
    }

    pub fn get(&self, kind: TerrainKind) -> LandscapeProfile {
        self.profiles[kind as usize]
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        for (value, min, max) in [
            (self.warp_scale, 64.0, 8192.0),
            (self.warp_strength, 0.0, 512.0),
            (self.region_scale, 128.0, 16384.0),
            (self.dune_wind_angle, -360.0, 360.0),
        ] {
            ensure(
                value.is_finite() && (min..=max).contains(&value),
                "Invalid landscape field scale, warp or wind angle",
            )?;
        }
        let mut weight = 0.0;
        for (kind, profile) in TerrainKind::ALL.into_iter().zip(self.profiles) {
            ensure(
                profile.kind == kind,
                "Landscape profiles must retain their ordered IDs",
            )?;
            ensure(
                profile.min_height.is_finite()
                    && profile.max_height.is_finite()
                    && profile.min_height >= -120.0
                    && profile.max_height <= 350.0
                    && profile.min_height < profile.max_height,
                "Invalid landscape height band",
            )?;
            for (value, min, max) in [
                (profile.horizontal_scale, 32.0, 8192.0),
                (profile.roughness, 0.0, 1.0),
                (profile.ridge_strength, 0.0, 1.0),
                (profile.erosion, 0.0, 1.0),
                (profile.overhangs, 0.0, 1.0),
                (profile.weight, 0.0, 10.0),
            ] {
                ensure(
                    value.is_finite() && (min..=max).contains(&value),
                    "Invalid landscape profile setting",
                )?;
            }
            weight += profile.weight;
        }
        ensure(
            weight > 0.0 && self.get(TerrainKind::Ocean).weight > 0.0,
            "Landscape profiles require positive weights and an ocean profile",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_keep_ordered_ids_distinct_labels_and_valid_height_bands() {
        let definitions = LandscapeDefinitions::default();
        definitions.validate().unwrap();
        for kind in TerrainKind::ALL {
            assert_eq!(definitions.get(kind).kind, kind);
            assert!(!kind.label().is_empty());
            assert_eq!(
                TerrainKind::ALL
                    .into_iter()
                    .filter(|other| other.label() == kind.label())
                    .count(),
                1
            );
        }
        for (kind, min, max) in [
            (TerrainKind::Plains, 32.0, 60.0),
            (TerrainKind::Forest, 40.0, 90.0),
            (TerrainKind::Desert, 50.0, 100.0),
            (TerrainKind::CherryGrove, 45.0, 115.0),
            (TerrainKind::Ocean, -98.0, 22.0),
        ] {
            let profile = definitions.get(kind);
            assert_eq!((profile.min_height, profile.max_height), (min, max));
        }
        let serialized = serde_json::to_string(&definitions).unwrap();
        assert_eq!(
            serde_json::from_str::<LandscapeDefinitions>(&serialized).unwrap(),
            definitions
        );
    }

    #[test]
    fn new_mountains_have_lower_caps_and_wider_shapes_without_changing_other_profiles() {
        let definitions = LandscapeDefinitions::default();
        let legacy = LandscapeDefinitions::legacy_v5();
        legacy.validate().unwrap();
        for (kind, minimum, maximum, scale) in [
            (TerrainKind::MountainForest, 50.0, 160.0, 1050.0),
            (TerrainKind::FantasyMountains, 65.0, 200.0, 1500.0),
            (TerrainKind::SnowyHighlands, 75.0, 200.0, 1300.0),
            (TerrainKind::SpiritForest, 45.0, 145.0, 850.0),
        ] {
            let profile = definitions.get(kind);
            let previous = legacy.get(kind);
            assert_eq!((profile.min_height, profile.max_height), (minimum, maximum));
            assert_eq!(profile.horizontal_scale, scale);
            assert!(profile.max_height < previous.max_height);
            assert!(profile.horizontal_scale > previous.horizontal_scale);
            assert!(profile.overhangs < previous.overhangs);
        }
        assert!(
            definitions
                .profiles
                .iter()
                .all(|profile| profile.max_height <= 200.0)
        );
        assert_eq!(definitions.get(TerrainKind::FantasyMountains).weight, 0.35);
        assert_eq!(definitions.get(TerrainKind::SnowyHighlands).weight, 0.75);
        for kind in [
            TerrainKind::Plains,
            TerrainKind::RollingPlains,
            TerrainKind::Forest,
            TerrainKind::CherryGrove,
            TerrainKind::Desert,
            TerrainKind::Ocean,
        ] {
            assert_eq!(definitions.get(kind), legacy.get(kind));
        }
        assert_eq!(definitions.warp_scale, legacy.warp_scale);
        assert_eq!(definitions.warp_strength, legacy.warp_strength);
        assert_eq!(definitions.region_scale, legacy.region_scale);
        assert_eq!(definitions.dune_wind_angle, legacy.dune_wind_angle);
        assert_eq!(legacy.get(TerrainKind::FantasyMountains).max_height, 348.0);
        assert_eq!(legacy.get(TerrainKind::MountainForest).max_height, 240.0);
        assert_eq!(legacy.get(TerrainKind::SnowyHighlands).max_height, 270.0);
        assert_eq!(legacy.get(TerrainKind::SpiritForest).max_height, 195.0);
    }

    #[test]
    fn global_fields_reject_nonfinite_and_out_of_range_values_but_accept_endpoints() {
        let defaults = LandscapeDefinitions::default();
        for (index, min, max) in [
            (0, 64.0, 8192.0),
            (1, 0.0, 512.0),
            (2, 128.0, 16384.0),
            (3, -360.0, 360.0),
        ] {
            for value in [
                min,
                max,
                min - 0.001,
                max + 0.001,
                f64::NAN,
                f64::NEG_INFINITY,
                f64::INFINITY,
            ] {
                let mut definitions = defaults.clone();
                *match index {
                    0 => &mut definitions.warp_scale,
                    1 => &mut definitions.warp_strength,
                    2 => &mut definitions.region_scale,
                    _ => &mut definitions.dune_wind_angle,
                } = value;
                assert_eq!(
                    definitions.validate().is_ok(),
                    value == min || value == max,
                    "global field {index}: {value}"
                );
            }
        }
    }

    #[test]
    fn every_profile_rejects_invalid_heights_settings_and_nonfinite_values() {
        let defaults = LandscapeDefinitions::default();
        for kind in TerrainKind::ALL {
            for (min, max) in [
                (-120.0, 350.0),
                (60.0, 32.0),
                (32.0, 32.0),
                (-120.001, 60.0),
                (32.0, 350.001),
                (f64::NAN, 60.0),
                (32.0, f64::NAN),
                (f64::NEG_INFINITY, 60.0),
                (32.0, f64::INFINITY),
            ] {
                let mut definitions = defaults.clone();
                let profile = &mut definitions.profiles[kind as usize];
                profile.min_height = min;
                profile.max_height = max;
                assert_eq!(
                    definitions.validate().is_ok(),
                    min == -120.0 && max == 350.0,
                    "{kind:?}: {min}..{max}"
                );
            }
            for (index, min, max) in [
                (0, 32.0, 8192.0),
                (1, 0.0, 1.0),
                (2, 0.0, 1.0),
                (3, 0.0, 1.0),
                (4, 0.0, 1.0),
                (5, 0.0, 10.0),
            ] {
                for value in [
                    min,
                    max,
                    min - 0.001,
                    max + 0.001,
                    f64::NAN,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                ] {
                    let mut definitions = defaults.clone();
                    let profile = &mut definitions.profiles[kind as usize];
                    *match index {
                        0 => &mut profile.horizontal_scale,
                        1 => &mut profile.roughness,
                        2 => &mut profile.ridge_strength,
                        3 => &mut profile.erosion,
                        4 => &mut profile.overhangs,
                        _ => &mut profile.weight,
                    } = value;
                    let valid_endpoint = (value == min || value == max)
                        && !(kind == TerrainKind::Ocean && index == 5 && value == 0.0);
                    assert_eq!(
                        definitions.validate().is_ok(),
                        valid_endpoint,
                        "{kind:?}, field {index}: {value}"
                    );
                }
            }
        }
    }

    #[test]
    fn profile_order_and_positive_ocean_weight_are_part_of_the_snapshot_contract() {
        let mut definitions = LandscapeDefinitions::default();
        definitions.profiles.swap(0, 1);
        assert!(definitions.validate().is_err());
        let mut definitions = LandscapeDefinitions::default();
        definitions.profiles[1].kind = TerrainKind::Plains;
        assert!(definitions.validate().is_err());
        let mut definitions = LandscapeDefinitions::default();
        for profile in &mut definitions.profiles {
            profile.weight = 0.0;
        }
        assert!(definitions.validate().is_err());
        definitions.profiles[TerrainKind::Ocean as usize].weight = 1.0;
        definitions.validate().unwrap();
        definitions.profiles[TerrainKind::Ocean as usize].weight = 0.0;
        definitions.profiles[TerrainKind::Plains as usize].weight = 1.0;
        assert!(definitions.validate().is_err());
    }
}
