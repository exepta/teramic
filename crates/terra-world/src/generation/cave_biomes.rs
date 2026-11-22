//! One spatial ecology field for rooms, noise connectors and entrance passages.
use super::{TerraGenerator, biomes::smoothstep, seed::layer_seed};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CaveBiome {
    #[default]
    Normal,
    Salt,
    Lush,
    Cascade,
}
impl CaveBiome {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Salt, Self::Lush, Self::Cascade];
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Salt => "Salt",
            Self::Lush => "Lush",
            Self::Cascade => "Cascade",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaveBiomeSample {
    pub primary: CaveBiome,
    pub secondary: Option<CaveBiome>,
    /// Secondary contribution; zero in a homogeneous region.
    pub blend: f32,
    pub weights: [f32; 4],
    pub moisture: f32,
    pub region_id: u64,
    pub boundary_distance: f32,
}
impl Default for CaveBiomeSample {
    fn default() -> Self {
        Self::from_weights([1.0, 0.0, 0.0, 0.0], 0, f32::INFINITY)
    }
}
impl CaveBiomeSample {
    pub(super) fn from_weights(weights: [f32; 4], region_id: u64, boundary_distance: f32) -> Self {
        let mut order = [0, 1, 2, 3];
        order.sort_by(|&a, &b| weights[b].total_cmp(&weights[a]));
        let secondary = (weights[order[1]] > 0.001).then_some(CaveBiome::ALL[order[1]]);
        Self {
            primary: CaveBiome::ALL[order[0]],
            secondary,
            blend: weights[order[1]],
            weights,
            moisture: weights[0] * 0.24 + weights[1] * 0.40 + weights[2] * 0.88 + weights[3] * 0.96,
            region_id,
            boundary_distance,
        }
    }
}

/// Fixed-size prepared column: depth modulation needs no hashing/noise/cache locks.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CaveBiomeColumn {
    pub sample: CaveBiomeSample,
    pub surface: f64,
}
impl CaveBiomeColumn {
    pub fn weights_at(self, y: f64) -> [f32; 4] {
        let depth = self.surface - y;
        let cover = smoothstep(4.0, 12.0, depth) as f32;
        let mut w = self.sample.weights;
        w[1] *= cover * (0.35 + 0.65 * smoothstep(12.0, 40.0, depth) as f32);
        w[2] *= cover * (1.0 - 0.78 * smoothstep(48.0, 150.0, depth) as f32);
        w[3] *= cover;
        w[0] = 1.0 - w[1] - w[2] - w[3];
        w
    }
    pub fn at(self, y: f64) -> CaveBiomeSample {
        CaveBiomeSample::from_weights(
            self.weights_at(y),
            self.sample.region_id,
            self.sample.boundary_distance,
        )
    }
}

#[derive(Clone, Copy, Default)]
struct Site {
    center: [f64; 2],
    radius: f64,
    biome: usize,
    id: u64,
}
#[derive(Default)]
pub(super) struct CaveBiomeCache(Mutex<HashMap<[i64; 2], Arc<[Site; 25]>>>);

impl TerraGenerator {
    pub fn cave_biome_at(&self, p: [f64; 3]) -> CaveBiomeSample {
        if p.iter().any(|v| !v.is_finite()) {
            return CaveBiomeSample::default();
        }
        self.cave_biome_column(p[0], p[2]).at(p[1])
    }

    pub fn cave_biome_column(&self, x: f64, z: f64) -> CaveBiomeColumn {
        if self.config().cave_structure_revision < 3
            || !self.config().caves
            || !self.config().cave_biomes.enabled
            || !x.is_finite()
            || !z.is_finite()
        {
            return CaveBiomeColumn::default();
        }
        self.cave_biome_column_with_surface(x, z, self.surface_height(x, z))
    }

    pub(super) fn cave_biome_column_with_surface(
        &self,
        x: f64,
        z: f64,
        surface: f64,
    ) -> CaveBiomeColumn {
        if self.config().cave_structure_revision < 3
            || !self.config().caves
            || !self.config().cave_biomes.enabled
            || !x.is_finite()
            || !z.is_finite()
        {
            return CaveBiomeColumn::default();
        }
        let cfg = &self.config().cave_biomes;
        let ranges = [
            cfg.regions.normal_diameter,
            cfg.regions.salt_diameter,
            cfg.regions.lush_diameter,
            cfg.regions.cascade_diameter,
        ];
        let spacing = (ranges.iter().map(|r| r[1]).fold(0.0_f64, f64::max) * 0.55).max(64.0);
        // Smooth domain warp is world-based, never relative to a chunk origin.
        let px = x + (z * 0.0081 + x * 0.0037).sin() * 15.0;
        let pz = z + (x * 0.0073 - z * 0.0041).sin() * 13.0;
        let cell = [(px / spacing).floor() as i64, (pz / spacing).floor() as i64];
        let cached = self
            .cave_biomes
            .0
            .lock()
            .ok()
            .and_then(|c| c.get(&cell).cloned());
        let sites = cached.unwrap_or_else(|| {
            let sites = Arc::new(std::array::from_fn(|i| {
                let c = [cell[0] + i as i64 % 5 - 2, cell[1] + i as i64 / 5 - 2];
                let id = (c[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (c[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9)
                    ^ self.config().seed.0;
                let h = layer_seed(id, 301);
                let unit = |shift: u32| f64::from((h >> shift) & 255) / 255.0;
                let center = [
                    (c[0] as f64 + 0.5 + (unit(0) - 0.5) * 0.3) * spacing,
                    (c[1] as f64 + 0.5 + (unit(8) - 0.5) * 0.3) * spacing,
                ];
                let climate = self.climate_biome_at(center[0], center[1]);
                let mut w = cfg.distribution.normalized();
                w[1] *= 0.45
                    + (1.0 - climate.humidity) * 0.9
                    + climate.weights.desert
                    + climate.weights.cliffted_desert;
                w[2] *= 0.4
                    + climate.humidity
                    + climate.weights.forest * 0.5
                    + climate.weights.plains * 0.2;
                w[3] *= 0.4
                    + climate.humidity
                    + climate.weights.highland * 0.5
                    + climate.weights.ocean * 0.4;
                if self.config().cave_structure_revision >= 4 {
                    w[1] *= 0.6 + climate.temperature;
                    w[2] *= 1.0 - (climate.temperature - 0.55).abs() * 0.7;
                    w[3] *= 0.65 + climate.humidity * 0.7;
                }
                if self.config().cave_structure_revision >= 13 {
                    w[1] *= 0.43;
                }
                let pick = f64::from(layer_seed(id, 302))
                    / (f64::from(u32::MAX)
                        + if self.config().cave_structure_revision >= 4 {
                            1.0
                        } else {
                            0.0
                        })
                    * w.iter().sum::<f64>();
                let mut accumulated = 0.0;
                let biome = w
                    .iter()
                    .position(|weight| {
                        accumulated += weight;
                        pick < accumulated
                    })
                    .unwrap_or(3);
                let range = ranges[biome];
                Site {
                    center,
                    radius: (range[0] + (range[1] - range[0]) * unit(16)) * 0.5,
                    biome,
                    id,
                }
            }));
            if let Ok(mut cache) = self.cave_biomes.0.lock() {
                if cache.len() >= 512 {
                    cache.clear();
                }
                cache.insert(cell, sites.clone());
            }
            sites
        });
        let distances: [f64; 25] = std::array::from_fn(|i| {
            (px - sites[i].center[0]).hypot(pz - sites[i].center[1]) - sites[i].radius
        });
        let mut best = 0;
        for i in 1..25 {
            if distances[i] < distances[best]
                || distances[i] == distances[best] && sites[i].id < sites[best].id
            {
                best = i;
            }
        }
        let boundary = distances
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != best)
            .map(|(_, d)| (d - distances[best]) * 0.5)
            .fold(f64::INFINITY, f64::min);
        let mut weights = [0.0_f32; 4];
        for i in 0..25 {
            let influence = 1.0
                - smoothstep(
                    0.0,
                    cfg.regions.transition_distance * 2.0,
                    distances[i] - distances[best],
                );
            weights[sites[i].biome] += influence as f32;
        }
        let total: f32 = weights.iter().sum();
        weights = weights.map(|v| v / total);
        CaveBiomeColumn {
            sample: CaveBiomeSample::from_weights(weights, sites[best].id, boundary as f32),
            surface,
        }
    }
}
