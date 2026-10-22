use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};

/// Saved generation settings. `world.caves` remains the existing boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaveBiomesConfig {
    pub enabled: bool,
    pub distribution: CaveDistribution,
    pub regions: CaveRegions,
    pub connectors: CaveConnectors,
    /// Visual surface coverage, independent of resource deposit probabilities.
    pub salt_surfaces: [f64; 4],
    pub lush_density: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaveDistribution {
    pub normal_weight: f64,
    pub salt_weight: f64,
    pub lush_weight: f64,
    pub cascade_weight: f64,
}
impl CaveDistribution {
    pub fn normalized(&self) -> [f64; 4] {
        let values = [
            self.normal_weight,
            self.salt_weight,
            self.lush_weight,
            self.cascade_weight,
        ];
        let sum: f64 = values.iter().sum();
        if !sum.is_finite() || sum <= 0.0 || values.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return [1.0, 0.0, 0.0, 0.0];
        }
        values.map(|v| v / sum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaveRegions {
    pub normal_diameter: [f64; 2],
    pub salt_diameter: [f64; 2],
    pub lush_diameter: [f64; 2],
    pub cascade_diameter: [f64; 2],
    pub transition_distance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaveConnectors {
    pub inherit_biomes: bool,
    pub blend_biomes: bool,
    pub biome_decorations: bool,
    /// Density multiplier of the existing noise tunnels, not a new tunnel engine.
    pub density: f64,
}

impl Default for CaveBiomesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            distribution: Default::default(),
            regions: Default::default(),
            connectors: Default::default(),
            salt_surfaces: [0.55, 0.30, 0.10, 0.05],
            lush_density: 0.72,
        }
    }
}
impl Default for CaveDistribution {
    fn default() -> Self {
        Self {
            normal_weight: 0.45,
            salt_weight: 0.20,
            lush_weight: 0.20,
            cascade_weight: 0.15,
        }
    }
}
impl Default for CaveRegions {
    fn default() -> Self {
        Self {
            normal_diameter: [100.0, 400.0],
            salt_diameter: [80.0, 250.0],
            lush_diameter: [100.0, 300.0],
            cascade_diameter: [120.0, 350.0],
            transition_distance: 24.0,
        }
    }
}
impl Default for CaveConnectors {
    fn default() -> Self {
        Self {
            inherit_biomes: true,
            blend_biomes: true,
            biome_decorations: true,
            density: 0.84,
        }
    }
}
impl Config for CaveBiomesConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(
            self.salt_surfaces
                .iter()
                .all(|w| w.is_finite() && (0.0..=1.0).contains(w))
                && self.salt_surfaces.iter().sum::<f64>() > 0.0
                && self.lush_density.is_finite()
                && (0.0..=1.0).contains(&self.lush_density),
            "cave surface weights and lush_density must be finite 0..=1; weights cannot all be zero",
        )?;
        let d = &self.distribution;
        let weights = [
            d.normal_weight,
            d.salt_weight,
            d.lush_weight,
            d.cascade_weight,
        ];
        ensure(
            weights
                .iter()
                .all(|v| v.is_finite() && (0.0..=1e6).contains(v))
                && weights.iter().sum::<f64>() > 0.0,
            "world.cave_biomes.distribution requires finite nonnegative weights and a positive total",
        )?;
        let r = &self.regions;
        for [min, max] in [
            r.normal_diameter,
            r.salt_diameter,
            r.lush_diameter,
            r.cascade_diameter,
        ] {
            ensure(
                min.is_finite() && max.is_finite() && min >= 64.0 && max <= 800.0 && min <= max,
                "world.cave_biomes.regions diameters require 64 <= min <= max <= 800",
            )?;
            ensure(
                r.transition_distance <= min * 0.5,
                "world.cave_biomes transition must not exceed half the smallest diameter",
            )?;
        }
        ensure(
            r.transition_distance.is_finite() && (1.0..=64.0).contains(&r.transition_distance),
            "world.cave_biomes.regions.transition_distance must be 1..=64",
        )?;
        ensure(
            self.connectors.density.is_finite() && (0.3..=1.5).contains(&self.connectors.density),
            "world.cave_biomes.connectors.density must be 0.3..=1.5",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_normalization_roundtrip_and_invalid_values() {
        let mut c = CaveBiomesConfig::default();
        assert_eq!(c.distribution.normalized(), [0.45, 0.20, 0.20, 0.15]);
        c.distribution.normal_weight = 45.0;
        c.distribution.salt_weight = 20.0;
        c.distribution.lush_weight = 20.0;
        c.distribution.cascade_weight = 15.0;
        assert_eq!(c.distribution.normalized(), [0.45, 0.20, 0.20, 0.15]);
        assert_eq!(
            CaveBiomesConfig::from_toml(&toml::to_string(&c).unwrap()).unwrap(),
            c
        );
        for value in [f64::NAN, -1.0, f64::INFINITY] {
            let mut bad = c.clone();
            bad.distribution.salt_weight = value;
            assert!(bad.validate().is_err());
        }
        c.regions.salt_diameter = [300.0, 100.0];
        assert!(c.validate().is_err());
        let zero =
            "[distribution]\nnormal_weight=0\nsalt_weight=0\nlush_weight=0\ncascade_weight=0";
        assert!(CaveBiomesConfig::from_toml(zero).is_err());
        for source in [
            "salt_surfaces=[0,0,0,0]",
            "salt_surfaces=[0.5,-0.1,0.5,0.1]",
            "lush_density=1.1",
        ] {
            assert!(CaveBiomesConfig::from_toml(source).is_err());
        }
    }
}
