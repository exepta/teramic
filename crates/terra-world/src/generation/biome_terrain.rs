//! Revision-four height bands share the existing climate, relief and drainage fields.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, BiomeWeights, smoothstep},
    landforms::terrace,
};
use noise::{NoiseFn, Perlin};
use terra_core::{BiomeDefinitions, BiomeKind};

impl TerraGenerator {
    pub(super) fn biome_definitions(&self) -> &BiomeDefinitions {
        self.config()
            .biome_definitions
            .as_ref()
            .unwrap_or_else(|| BiomeDefinitions::builtin())
    }

    pub(super) fn configured_height(&self, x: f64, z: f64, biome: BiomeSample) -> f64 {
        let profiles = self.biome_definitions();
        // Sharpen climate interiors, but retain a continuous coast/foothill transition.
        let mut weights = biome.weights.values().map(|weight| weight.powi(3));
        let total = weights.iter().sum::<f64>().max(f64::EPSILON);
        for weight in &mut weights {
            *weight /= total;
        }
        let scale = BiomeKind::ALL
            .into_iter()
            .zip(weights)
            .map(|(kind, weight)| profiles.get(kind).terrain.horizontal_scale * weight)
            .sum::<f64>();
        let warp = self.hills.get([x * 0.00054 + 127.3, z * 0.00054 - 63.1]) * 28.0;
        let broad = scaled_noise(&self.mountains, x + warp, z, scale / 0.8, [0.53, 0.89]);
        let rolling = scaled_noise(&self.hills, x + warp, z, scale / 2.1, [0.37, 0.71]);
        let detail = self.hills.get([x * 0.033 + 1.17, z * 0.033 + 2.31]);
        let ridge = 1.0 - scaled_noise(&self.valleys, x + warp, z, scale / 2.8, [0.19, 0.47]).abs();
        let region = self.landform_partition(x, z);
        let plains = broad * 0.55 + rolling * 0.25;
        let hills = broad * 0.45 + rolling * 0.75;
        let shelf = broad * 0.9 + rolling * 0.22;
        let ridges = broad * 0.4 + ridge.powi(3) * 1.6 - 0.45;
        let basin = broad * 0.65 + rolling * 0.35 - 0.4;
        let relief = [plains, hills, shelf, ridges, basin]
            .into_iter()
            .zip(region.weights)
            .map(|(value, weight)| value * weight)
            .sum::<f64>();
        let config = self.config();
        BiomeKind::ALL
            .into_iter()
            .zip(weights)
            .map(|(kind, weight)| {
                if weight == 0.0 {
                    return 0.0;
                }
                let p = profiles.get(kind).terrain;
                let amplitude =
                    ((config.hill_amplitude + config.mountain_amplitude) / 60.0).min(2.0);
                let shape = if kind == BiomeKind::Ocean {
                    0.30 + (broad * 0.42 + rolling * 0.22) * p.relief_strength
                        + detail * p.detail_strength * 0.12
                } else {
                    let raw = (relief * p.relief_strength * 1.15
                        + detail * p.detail_strength * 0.35
                        + region.landmark * 0.4)
                        * amplitude;
                    // A smooth bounded response avoids the flat shelves caused by clamping
                    // an unbounded mountain field at every biome's maximum height.
                    0.5 + raw / (1.0 + raw.abs()) * 0.5
                };
                let shape = shape.clamp(0.0, 1.0);
                let raw = p.min_height + (p.max_height - p.min_height) * shape;
                let stepped = terrace(raw, config.terrace_height, rolling * 0.4);
                let target = raw + (stepped - raw) * p.terrace_strength * config.terrace_strength;
                target.clamp(p.min_height, p.max_height) * weight
            })
            .sum()
    }

    pub(super) fn surface_ecology(&self, biome: BiomeSample, height: f64) -> BiomeSample {
        if self.config().landscape_revision >= 5 {
            return biome;
        }
        if self.config().landscape_revision < 4
            || self.config().hill_amplitude == 0.0 && self.config().mountain_amplitude == 0.0
        {
            return biome.at_height(height);
        }
        let profiles = self.biome_definitions();
        let mut weights = biome.weights.values();
        let land = 1.0 - weights[0];
        let mut bridge = [0.0; 10];
        for kind in BiomeKind::ALL.into_iter().skip(1) {
            let p = profiles.get(kind).terrain;
            let suitability = smoothstep(p.min_height, p.min_height + 5.0, height)
                * (1.0 - smoothstep(p.max_height - 5.0, p.max_height, height));
            weights[kind as usize] *= suitability;
            let affinity = match kind {
                BiomeKind::Plains => 0.4,
                BiomeKind::Forest => 0.3 + biome.humidity * 0.7,
                BiomeKind::Highland => 0.8,
                BiomeKind::Desert => biome.aridity(),
                BiomeKind::MushroomPark => biome.weights.mushroom_park,
                BiomeKind::ClifftedDesert => biome.weights.cliffted_desert,
                BiomeKind::CherryForest => biome.weights.cherry_forest,
                BiomeKind::SpiritBlossom => biome.weights.spirit_blossom,
                BiomeKind::BloodForest => biome.weights.blood_forest,
                BiomeKind::Ocean => 0.0,
            };
            bridge[kind as usize] = suitability * affinity;
        }
        // Fill the lost climate weight progressively with suitable foothill
        // ecology. A hard nearest-biome fallback would change every tree at Y60.
        let retained = weights[1..].iter().sum::<f64>();
        let bridge_total = bridge.iter().sum::<f64>();
        if bridge_total > f64::EPSILON {
            for kind in BiomeKind::ALL.into_iter().skip(1) {
                weights[kind as usize] +=
                    (land - retained).max(0.0) * bridge[kind as usize] / bridge_total;
            }
        }
        let total = weights[1..].iter().sum::<f64>();
        if total > f64::EPSILON {
            for weight in &mut weights[1..] {
                *weight *= land / total;
            }
        } else {
            // Shores and eroded river beds can lie below the terrestrial range.
            let kind = BiomeKind::ALL
                .into_iter()
                .skip(1)
                .filter(|kind| {
                    (*kind != BiomeKind::BloodForest || self.config().blood_forest_revision > 0)
                        && (*kind != BiomeKind::SpiritBlossom
                            || self.config().spirit_blossom_revision > 0)
                })
                .min_by(|a, b| {
                    let distance = |kind| {
                        let p = profiles.get(kind).terrain;
                        (p.min_height - height).max(0.0) + (height - p.max_height).max(0.0)
                            - biome.weights.values()[kind as usize] * 0.01
                    };
                    distance(*a).total_cmp(&distance(*b))
                })
                .unwrap_or(BiomeKind::Plains);
            weights[kind as usize] = land;
        }
        let [
            ocean,
            plains,
            forest,
            desert,
            highland,
            mushroom_park,
            cliffted_desert,
            cherry_forest,
            spirit_blossom,
            blood_forest,
        ] = weights;
        BiomeSample {
            weights: BiomeWeights {
                ocean,
                plains,
                forest,
                desert,
                highland,
                mushroom_park,
                cliffted_desert,
                cherry_forest,
                spirit_blossom,
                blood_forest,
            },
            ..biome
        }
    }

    pub(super) fn configured_soil_depth(&self, biome: BiomeSample) -> f64 {
        self.weighted_biome_setting(biome, |terrain| terrain.soil_depth)
    }

    pub(super) fn configured_tree_density(&self, biome: BiomeSample) -> f64 {
        self.weighted_biome_setting(biome, |terrain| terrain.tree_density)
    }

    fn weighted_biome_setting(
        &self,
        biome: BiomeSample,
        setting: impl Fn(terra_core::BiomeTerrain) -> f64,
    ) -> f64 {
        BiomeKind::ALL
            .into_iter()
            .zip(biome.weights.values())
            .map(|(kind, weight)| setting(self.biome_definitions().get(kind).terrain) * weight)
            .sum()
    }
}

/// Blend fixed-frequency fields, never divide world coordinates by a spatially
/// varying scale. That would amplify gradients with distance from the origin.
fn scaled_noise(field: &Perlin, x: f64, z: f64, scale: f64, offset: [f64; 2]) -> f64 {
    let octave = (scale / 8.0).log2();
    let low = 8.0 * 2.0_f64.powf(octave.floor());
    let blend = smoothstep(0.0, 1.0, octave.fract());
    let a = field.get([x / low + offset[0], z / low + offset[1]]);
    let b = field.get([x / (low * 2.0) + offset[0], z / (low * 2.0) + offset[1]]);
    a + (b - a) * blend
}

#[cfg(test)]
mod tests;
