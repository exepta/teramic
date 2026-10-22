//! Independent, seeded column fields; never sampled once per voxel.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
    seed::layer_seed,
};
use noise::{NoiseFn, Perlin};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandscapeNoise {
    pub continentalness: f64,
    pub erosion: f64,
    pub peaks_valleys: f64,
    pub jaggedness: f64,
    pub surface_lake: f64,
}
#[derive(Clone)]
pub(super) struct LandscapeSampler {
    erosion: Perlin,
    peaks: Perlin,
    jagged: Perlin,
    lakes: Perlin,
}
impl LandscapeSampler {
    pub fn new(seed: u64) -> Self {
        Self {
            erosion: Perlin::new(layer_seed(seed, 150)),
            peaks: Perlin::new(layer_seed(seed, 151)),
            jagged: Perlin::new(layer_seed(seed, 152)),
            lakes: Perlin::new(layer_seed(seed, 153)),
        }
    }
}
impl TerraGenerator {
    pub fn landscape_noise(&self, x: f64, z: f64) -> LandscapeNoise {
        let ridges = self
            .landscape
            .peaks
            .get([x * 0.0016 + 8.3, z * 0.0016 - 4.7]);
        LandscapeNoise {
            continentalness: self.climate.continentalness(x, z),
            erosion: (0.5
                + self
                    .landscape
                    .erosion
                    .get([x * 0.0027 + 13.1, z * 0.0027 + 9.3])
                    * 0.95)
                .clamp(0.0, 1.0),
            // Fold the low-frequency field into broad valleys and narrow ridges.
            peaks_valleys: (1.0 - (3.0 * ridges.abs() - 2.0).abs()).clamp(-1.0, 1.0),
            jaggedness: self
                .landscape
                .jagged
                .get([x * 0.055 + 71.3, z * 0.055 - 37.9]),
            surface_lake: (0.5
                + self
                    .landscape
                    .lakes
                    .get([x * 0.0019 - 19.7, z * 0.0019 + 31.3])
                    * 0.95)
                .clamp(0.0, 1.0),
        }
    }
    pub(super) fn landscape_relief(&self, x: f64, z: f64, biome: BiomeSample, raw: f64) -> f64 {
        let config = self.config();
        if config.landscape_revision == 0
            || (config.hill_amplitude == 0.0 && config.mountain_amplitude == 0.0)
        {
            return raw;
        }
        let n = self.landscape_noise(x, z);
        let land = 1.0 - smoothstep(0.25, 0.80, biome.weights.ocean);
        let upland = smoothstep(0.05, 0.65, biome.weights.highland);
        let erosion = smoothstep(0.24, 0.78, n.erosion);
        let ridge = n.peaks_valleys * config.hill_amplitude * (0.24 + upland * 0.65);
        let jagged = n.jaggedness
            * smoothstep(0.35, 0.85, n.peaks_valleys)
            * upland
            * (1.0 - erosion)
            * config.mountain_amplitude
            * 0.13;
        // Eroded regions retain biome elevation, but have smoother local relief.
        let reference = config.base_height + biome.profile().elevation;
        let flattened = raw + (reference - raw) * erosion * 0.18;
        raw + land
            * (flattened - raw
                + ridge * (1.0 - erosion * 0.65)
                + n.continentalness * config.hill_amplitude * 0.25
                + jagged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Config, WorldConfig};
    #[test]
    fn saved_contract_defaults_old_files_to_legacy_and_new_worlds_to_one() {
        assert_eq!(WorldConfig::from_toml("").unwrap().landscape_revision, 0);
        assert_eq!(WorldConfig::default().landscape_revision, 1);
        assert!(WorldConfig::from_toml("landscape_revision = 2").is_err());
    }
    #[test]
    fn fields_are_repeatable_continuous_and_change_only_opted_in_worlds() {
        let a = TerraGenerator::new(WorldConfig::default()).unwrap();
        let b = TerraGenerator::new(a.config().clone()).unwrap();
        let old = TerraGenerator::new(WorldConfig {
            landscape_revision: 0,
            ..a.config().clone()
        })
        .unwrap();
        let mut changes = 0;
        for x in -32..32 {
            let x = f64::from(x) * 41.0 + 0.5;
            let n = a.landscape_noise(x, -123.5);
            assert_eq!(n, b.landscape_noise(x, -123.5));
            assert!((0.0..=1.0).contains(&n.erosion));
            assert!((0.0..=1.0).contains(&n.surface_lake));
            let m = a.landscape_noise(x + 0.001, -123.5);
            assert!((m.jaggedness - n.jaggedness).abs() < 0.002);
            let h = a.surface_height(x, -123.5);
            assert_eq!(h, b.surface_height(x, -123.5));
            assert!(h.is_finite());
            changes += usize::from((h - old.surface_height(x, -123.5)).abs() > 0.01);
        }
        assert!(changes > 20);
    }
}
