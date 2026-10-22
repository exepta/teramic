use super::{BiomeSample, BiomeWeights, smoothstep};
use crate::generation::seed::layer_seed;
use noise::{NoiseFn, Perlin};

#[derive(Clone)]
pub(in crate::generation) struct ClimateSampler {
    temperature: Perlin,
    humidity: Perlin,
    uplands: Perlin,
    forest: Perlin,
    clearings: Perlin,
    continents: Perlin,
    mushrooms: Perlin,
    scale: f64,
    ocean_scale: f64,
}

impl ClimateSampler {
    pub fn continentalness(&self, x: f64, z: f64) -> f64 {
        self.continents
            .get([x / self.ocean_scale + 0.23, z / self.ocean_scale + 0.67])
    }
    pub fn new(seed: u64, scale: f64, ocean_scale: f64) -> Self {
        Self {
            temperature: Perlin::new(layer_seed(seed, 6)),
            humidity: Perlin::new(layer_seed(seed, 7)),
            uplands: Perlin::new(layer_seed(seed, 8)),
            forest: Perlin::new(layer_seed(seed, 9)),
            clearings: Perlin::new(layer_seed(seed, 10)),
            continents: Perlin::new(layer_seed(seed, 13)),
            mushrooms: Perlin::new(layer_seed(seed, 14)),
            scale,
            ocean_scale,
        }
    }

    pub fn sample(&self, x: f64, z: f64) -> BiomeSample {
        let continent = 0.5
            + self
                .continents
                .get([x / self.ocean_scale + 0.23, z / self.ocean_scale + 0.67])
                * 0.95;
        let ocean = 1.0 - smoothstep(0.25, 0.65, continent);
        let x = x / self.scale;
        let z = z / self.scale;
        let temperature = (0.5 + self.temperature.get([x + 0.71, z + 0.39]) * 0.9).clamp(0.0, 1.0);
        let humidity = (0.5 + self.humidity.get([x + 1.37, z + 2.19]) * 0.9).clamp(0.0, 1.0);
        let uplands = 0.5 + self.uplands.get([x * 0.83 + 3.31, z * 0.83 + 4.73]) * 0.9;
        let highland = (1.0 - ocean) * smoothstep(0.50, 0.90, uplands) * 0.80;
        let desert = (1.0 - ocean - highland)
            * smoothstep(0.52, 0.82, temperature)
            * (1.0 - smoothstep(0.18, 0.48, humidity));
        let park = 0.5 + self.mushrooms.get([x * 1.7 + 5.29, z * 1.7 - 3.61]) * 0.9;
        let mushroom_park = (1.0 - ocean - highland - desert)
            * smoothstep(0.58, 0.82, park)
            * smoothstep(0.28, 0.58, humidity);
        let forest =
            (1.0 - ocean - highland - desert - mushroom_park) * smoothstep(0.35, 0.72, humidity);
        BiomeSample {
            temperature,
            humidity,
            weights: BiomeWeights {
                ocean,
                plains: 1.0 - ocean - highland - forest - desert - mushroom_park,
                forest,
                desert,
                highland,
                mushroom_park,
                cliffted_desert: 0.0,
            },
        }
    }

    /// Interior islands in existing hot/dry regions, never isolated desert patches.
    pub fn with_cliffs(&self, mut biome: BiomeSample, x: f64, z: f64) -> BiomeSample {
        let parent = biome.weights.desert;
        if parent > 0.7 {
            let patch = self.uplands.get([x / 1024.0 + 19.7, z / 1024.0 - 31.4]);
            let fraction = smoothstep(0.7, 0.9, parent) * smoothstep(-0.06, 0.38, patch);
            biome.weights.cliffted_desert = parent * fraction;
            biome.weights.desert -= biome.weights.cliffted_desert;
        }
        biome
    }

    pub fn ground_masks(&self, x: f64, z: f64) -> (f64, f64) {
        let patches = 0.5 + self.forest.get([x * 0.045 + 5.73, z * 0.045 + 6.17]) * 0.9;
        let path = self
            .clearings
            .get([x * 0.008 + 1.23, z * 0.008 + 4.91])
            .abs();
        (
            smoothstep(0.25, 0.70, patches),
            smoothstep(0.025, 0.08, path),
        )
    }

    pub fn forest_mask(&self, x: f64, z: f64) -> f64 {
        let group = 0.5 + self.forest.get([x * 0.006 + 0.83, z * 0.006 + 1.71]) * 0.8;
        let glade = self.clearings.get([x * 0.017 + 2.47, z * 0.017 + 3.13]);
        (0.65 + 0.55 * smoothstep(0.15, 0.72, group)) * (1.0 - smoothstep(0.30, 0.60, glade))
    }

    /// Decorative ecology only: changing these masks never moves v5 trees or terrain.
    pub fn vegetation_masks(&self, x: f64, z: f64) -> [f64; 4] {
        let clearing = self.clearings.get([x * 0.017 + 2.47, z * 0.017 + 3.13]);
        let meadow = 0.5 + self.clearings.get([x * 0.038 + 7.31, z * 0.038 + 4.17]) * 0.9;
        let shrub = 0.5 + self.forest.get([x * 0.105 + 8.71, z * 0.105 + 3.79]) * 0.95;
        let tuft = 0.5 + self.forest.get([x * 0.19 + 2.93, z * 0.19 + 8.51]) * 0.9;
        [
            1.0 - smoothstep(0.18, 0.48, clearing),
            smoothstep(0.28, 0.72, meadow),
            smoothstep(0.32, 0.72, shrub),
            smoothstep(0.15, 0.78, tuft),
        ]
    }
}
