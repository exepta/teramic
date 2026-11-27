//! Maritime ecology follows the actual submerged continental shelf. A lake or
//! river at sea level by itself cannot create a coastal region.
use super::{
    TerraGenerator,
    biomes::{BiomeKind, BiomeSample, BiomeWeights, smoothstep},
};
use noise::NoiseFn;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use terra_voxel::MaterialId as M;

type MarineSamples = Arc<Vec<[f64; 2]>>;
type ShoreEntries = (HashMap<[i64; 2], MarineSamples>, VecDeque<[i64; 2]>);
#[derive(Default)]
pub(super) struct ShoreCache(Mutex<ShoreEntries>);

impl BiomeWeights {
    pub fn coastal_land_fraction(self) -> f64 {
        (self.misty_coast / (1. - self.ocean).max(0.0001)).clamp(0., 1.)
    }
}

impl TerraGenerator {
    fn open_water_samples(&self, tile: [i64; 2]) -> MarineSamples {
        if let Some(samples) = self.shoreline.0.lock().unwrap().0.get(&tile).cloned() {
            return samples;
        }
        let mut points = Vec::new();
        for dz in -3..=4 {
            for dx in -3..=4 {
                let [x, z] = [
                    (tile[0] + dx) as f64 * 48. + 0.5,
                    (tile[1] + dz) as f64 * 48. + 0.5,
                ];
                let node = self.advanced.sample(x, z);
                if node.weights[9] > 0.52 && node.height < self.config().sea_level - 3. {
                    let climate = self.biome_from_node(node);
                    if self.marine_height(x, z, climate, node.height)
                        < self.config().sea_level - 2.5
                    {
                        points.push([x, z]);
                    }
                }
            }
        }
        let mut entries = self.shoreline.0.lock().unwrap();
        if let Some(existing) = entries.0.get(&tile) {
            return existing.clone();
        }
        if entries.1.len() >= 1024
            && let Some(first) = entries.1.pop_front()
        {
            entries.0.remove(&first);
        }
        let samples = Arc::new(points);
        entries.1.push_back(tile);
        entries.0.insert(tile, samples.clone());
        samples
    }

    pub(super) fn with_misty_coast(
        &self,
        x: f64,
        z: f64,
        mut biome: BiomeSample,
        terrain_height: f64,
    ) -> BiomeSample {
        let config = self.config();
        if config.ecology_revision < 22
            || !self.marine_overhaul()
            || config.mountain_amplitude + config.hill_amplitude <= 0.
            || biome.weights.ocean >= 0.9999
        {
            return biome;
        }
        let eligible = biome.weights.forest
            + biome.weights.plains
            + biome.weights.cherry_forest
            + biome.weights.mushroom_park;
        if eligible < 0.08 * (1. - biome.weights.ocean) {
            return biome;
        }
        let district = self.valleys.get([x / 540. + 19.2, z / 540. - 53.6]);
        if district <= -0.40 {
            return biome;
        }
        let level = terrain_height - config.sea_level;
        if !(-18. ..58.).contains(&level) {
            return biome;
        }
        let key = [x, z].map(|v| (v / 48.).floor() as i64);
        let dist2 = self
            .open_water_samples(key)
            .iter()
            .map(|p| (x - p[0]).powi(2) + (z - p[1]).powi(2))
            .fold(f64::INFINITY, f64::min);
        if dist2 >= 144_f64.powi(2) {
            return biome;
        }
        let fraction = smoothstep(-0.40, -0.02, district)
            * (1. - smoothstep(82., 144., dist2.sqrt()))
            * (1. - smoothstep(33., 58., level))
            * smoothstep(-18., -5., level);
        let bias = self
            .biome_definitions()
            .get(BiomeKind::MistyCoast)
            .selection_weight;
        let share = (fraction * bias / (1. + fraction * (bias - 1.)).max(0.000_001)).clamp(0., 1.);
        let w = &mut biome.weights;
        w.misty_coast = eligible * share;
        for source in [
            &mut w.plains,
            &mut w.forest,
            &mut w.mushroom_park,
            &mut w.cherry_forest,
        ] {
            *source *= 1. - share;
        }
        biome
    }

    pub(super) fn coastal_relief(&self, x: f64, z: f64, height: f64, w: BiomeWeights) -> f64 {
        let coast = w.coastal_land_fraction();
        if coast <= 0. {
            return height;
        }
        let above = height - self.config().sea_level;
        let promontory = smoothstep(-0.16, 0.46, self.hills.get([x / 89. - 6.2, z / 82. + 39.]));
        let shoulder = smoothstep(-2., 5.8, above) * (1. - smoothstep(21., 43., above));
        let notch = smoothstep(0.6, 2.1, above) * (1. - smoothstep(2.4, 5.8, above));
        height + coast * (promontory * 15.0 * shoulder - notch * (2.1 - promontory))
    }

    pub(super) fn coastal_ground(&self, x: f64, z: f64, height: f64, slope: f64) -> M {
        let above = height - self.config().sea_level;
        let s = self.hills.get([x / 17. + 24., z / 19. - 36.]);
        let exposure = self.valleys.get([x / 54. - 13., z / 49. + 74.]);
        if above < 1.8 && (slope > 0.48 || exposure + s * 0.33 > 0.28) {
            M::TidewornRock
        } else if above < 3.6 && slope < 0.65 {
            if exposure < -0.22 {
                M::CoarseShoreSand
            } else {
                M::BeachShingle
            }
        } else if slope > 0.75 && above < 10. && s > 0.08 {
            M::SaltStainedStone
        } else if slope < 0.63 && above > 4.6 && exposure + 0.45 * s < 0.08 {
            M::Grass
        } else if slope < 0.80 && above > 4. && s > -0.40 {
            M::MossyCoastalStone
        } else if s + exposure * 0.5 > 0.23 {
            M::LichenCoastalStone
        } else {
            M::CoastalGranite
        }
    }
}
