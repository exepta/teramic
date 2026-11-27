//! Revision-four substrates use the same continuous marine fields as their props.
use super::{BiomeSample, NoiseFn, OceanHabitat, TerraGenerator, smoothstep};
use terra_core::MarineFloorWeights;
use terra_voxel::MaterialId;

impl TerraGenerator {
    pub(super) fn modern_ocean_habitat(
        &self,
        x: f64,
        z: f64,
        biome: BiomeSample,
        sea: f64,
    ) -> OceanHabitat {
        let config = self.config().ocean_biome_config();
        let cold = 1.0
            - smoothstep(
                config.cold_temperature - 0.08,
                config.cold_temperature + 0.08,
                biome.temperature,
            );
        let coast = 0.20 + 0.80 * (1.0 - smoothstep(0.45, 0.95, biome.weights.ocean));
        let swamp = (1.0 - cold)
            * smoothstep(
                config.swamp_humidity - 0.08,
                config.swamp_humidity + 0.08,
                biome.humidity,
            )
            * coast;
        let warm = (1.0 - cold - swamp).max(0.0);
        let district = self.hills.get([x / 240.0 + 84.3, z / 240.0 - 61.7]);
        let reef =
            warm * smoothstep(
                config.warm_temperature - 0.08,
                config.warm_temperature + 0.08,
                biome.temperature,
            ) * smoothstep(-0.28, 0.18, district);
        let colony =
            (0.5 + self.valleys.get([x / 21.0 + 37.9, z / 21.0 - 104.1]) * 0.75).clamp(0.0, 1.0);
        let channel = self.valleys.get([x / 73.0 - 51.4, z / 73.0 + 29.3]).abs();
        let cover = smoothstep(0.035, 0.13, channel) * smoothstep(0.20, 0.65, colony);
        let rock = smoothstep(
            -0.12,
            0.38,
            self.hills.get([x / 33.0 - 14.7, z / 33.0 + 71.3]),
        ) * smoothstep(0.025, 0.10, channel);
        let profiles = [&config.warm, &config.cold, &config.swamp];
        let weights = [warm, cold, swamp];
        let weighted = |value: fn(&terra_core::MarineHabitatConfig) -> f64| {
            profiles
                .iter()
                .zip(weights)
                .map(|(profile, weight)| value(profile) * weight)
                .sum::<f64>()
        };
        let floor = MarineFloorWeights {
            sand: weighted(|profile| {
                profile.floor.sand / profile.floor.values().into_iter().sum::<f64>()
            }),
            gravel: weighted(|profile| {
                profile.floor.gravel / profile.floor.values().into_iter().sum::<f64>()
            }),
            mud: weighted(|profile| {
                profile.floor.mud / profile.floor.values().into_iter().sum::<f64>()
            }),
            dirt: weighted(|profile| {
                profile.floor.dirt / profile.floor.values().into_iter().sum::<f64>()
            }),
            stone: weighted(|profile| {
                profile.floor.stone / profile.floor.values().into_iter().sum::<f64>()
            }),
        };
        // Nearby columns share this low-frequency scalar; materials form patches,
        // not independently selected gravel/sand speckles at every voxel.
        let selector = (0.5 + self.valleys.get([x / 38.0 - 91.7, z / 38.0 + 46.3]) * 1.1)
            .clamp(0.0, 1.0 - f64::EPSILON);
        let mut substrate = select_floor(floor, selector);
        let coral_density = weighted(|profile| profile.coral_density) * reef * sea;
        if coral_density > 0.0 && rock > 0.54 && colony > 0.70 && cover > 0.35 {
            substrate = if colony > 0.94 {
                MaterialId::CoralPurple
            } else if colony > 0.87 {
                MaterialId::CoralBlue
            } else if colony > 0.80 {
                MaterialId::CoralRed
            } else {
                MaterialId::CoralYellow
            };
        }
        if self.marine_overhaul() && sea > 0.5 {
            substrate = match substrate {
                MaterialId::Sand if rock < 0.36 => MaterialId::WetSand,
                MaterialId::Stone if cold > 0.46 => MaterialId::SeaBasalt,
                MaterialId::Stone if reef > 0.35 && rock > 0.48 => MaterialId::ReefStone,
                MaterialId::Stone if swamp < 0.28 => MaterialId::MarineLimestone,
                other => other,
            };
        }
        let clay_depth = if config.clay_frequency <= 0.0 {
            0.0
        } else {
            let field = (0.5
                + self.valleys.get([
                    x / config.clay_cluster_scale + 133.7,
                    z / config.clay_cluster_scale - 57.9,
                ]) * 1.4)
                .clamp(0.0, 1.0);
            let threshold = 1.0 - config.clay_frequency;
            let deposit = if config.clay_frequency >= 1.0 {
                1.0
            } else {
                smoothstep(threshold, (threshold + 0.10).min(1.0), field)
            };
            config.clay_depth * deposit * sea
        };
        OceanHabitat {
            weights: [reef, (warm + swamp - reef).max(0.0), cold]
                .map(|weight| (weight * sea) as f32),
            colony: colony as f32,
            cover: cover as f32,
            rock: rock as f32,
            swamp: (swamp * sea) as f32,
            coral_density: coral_density as f32,
            seagrass_density: (weighted(|profile| profile.seagrass_density) * sea) as f32,
            max_vegetation_depth: weighted(|profile| profile.max_vegetation_depth) as f32,
            clay_depth: clay_depth as f32,
            substrate: (sea > 0.1).then_some(substrate),
            ice_depth: 0,
        }
    }
}

fn select_floor(floor: MarineFloorWeights, selector: f64) -> MaterialId {
    let selector = selector * floor.values().into_iter().sum::<f64>();
    let mut threshold = 0.0;
    for (weight, material) in [
        (floor.sand, MaterialId::Sand),
        (floor.gravel, MaterialId::Gravel),
        (floor.mud, MaterialId::Mud),
        (floor.dirt, MaterialId::Dirt),
        (floor.stone, MaterialId::Stone),
    ] {
        threshold += weight;
        if selector < threshold {
            return material;
        }
    }
    MaterialId::Stone
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnormalized_floor_weights_keep_their_relative_material_distribution() {
        let raw = MarineFloorWeights {
            sand: 70.0,
            gravel: 5.0,
            mud: 4.0,
            dirt: 1.0,
            stone: 20.0,
        };
        let normalized = MarineFloorWeights {
            sand: 0.70,
            gravel: 0.05,
            mud: 0.04,
            dirt: 0.01,
            stone: 0.20,
        };
        for rank in 0..1000 {
            let selector = (f64::from(rank) + 0.5) / 1000.0;
            assert_eq!(
                select_floor(raw, selector),
                select_floor(normalized, selector)
            );
        }
    }
}
