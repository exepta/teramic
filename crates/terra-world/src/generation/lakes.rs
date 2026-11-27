//! Closed surface basins use existing water columns, sources and meshing.
use super::{OasisSample, TerraGenerator, seed::layer_seed};
use std::{collections::HashMap, sync::Mutex};

const SPACING: f64 = 256.0;
#[derive(Default)]
pub(super) struct LakeCache(Mutex<HashMap<[i64; 2], Option<Lake>>>);
#[derive(Clone, Copy)]
pub(super) struct Lake {
    pub(super) center: [f64; 2],
    pub(super) radius: f64,
    pub(super) level: f64,
    depth: f64,
    phase: f64,
    lobes: bool,
}
impl Lake {
    pub(super) fn sample(self, x: f64, z: f64) -> OasisSample {
        if self.lobes {
            return self.regional_sample(x, z);
        }
        let dx = (x - self.center[0]) * 0.90;
        let dz = (z - self.center[1]) / 0.90;
        let angle = dz.atan2(dx);
        let outline = 1.0
            + (angle * 3.0 + self.phase).sin() * 0.08
            + (angle * 5.0 - self.phase).sin() * 0.035;
        OasisSample {
            center: self.center,
            radius: self.radius,
            distance: dx.hypot(dz) / outline,
            level: self.level,
            depth: self.depth,
            spring: None,
        }
    }
}
impl TerraGenerator {
    pub fn lake_at(&self, x: f64, z: f64) -> Option<OasisSample> {
        let config = self.config();
        if (config.landscape_revision == 0 && !self.connected_hydrology())
            || !config.water
            || !config.biomes
            || (config.hill_amplitude == 0.0 && config.mountain_amplitude == 0.0)
            || !x.is_finite()
            || !z.is_finite()
        {
            return None;
        }
        if self.connected_hydrology() {
            return self.connected_lake_at(x, z);
        }
        let cell = [(x / SPACING).floor() as i64, (z / SPACING).floor() as i64];
        let seed = layer_seed(
            config.seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            154,
        );
        if !seed.is_multiple_of(4) {
            return None;
        }
        let center = [
            cell[0] as f64 * SPACING + 104.0 + f64::from((seed >> 4) % 48),
            cell[1] as f64 * SPACING + 104.0 + f64::from((seed >> 12) % 48),
        ];
        if (x - center[0]).abs().max((z - center[1]).abs()) > 76.0 {
            return None;
        }
        let cached = self
            .lakes
            .0
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied());
        let lake = cached.unwrap_or_else(|| {
            let lake = self.build_lake(center, seed);
            if let Ok(mut cache) = self.lakes.0.lock() {
                if cache.len() >= 512 {
                    cache.clear();
                }
                *cache.entry(cell).or_insert(lake)
            } else {
                lake
            }
        })?;
        let sample = self.spirit_lake_sample(lake, x, z);
        (sample.distance < sample.radius + super::oases::BANK_REACH).then_some(sample)
    }
    fn build_lake(&self, center: [f64; 2], seed: u32) -> Option<Lake> {
        let [x, z] = center;
        let biome = self.climate_biome_at(x, z);
        let n = self.landscape_noise(x, z);
        if self.volcanic_watershed(biome)
            || biome.aridity() > 0.35
            || biome.weights.ocean > 0.25
            || biome.weights.highland > 0.35
            || n.surface_lake < 0.43
        {
            return None;
        }
        let mut lake = Lake {
            center,
            radius: 18.0 + f64::from((seed >> 20) % 12),
            level: 0.0,
            depth: 2.5 + f64::from((seed >> 25) % 8) * 0.5,
            phase: f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0,
            lobes: false,
        };
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        // The whole influence fits inside one tile. Validate the complete wet basin
        // once, including river/oasis exclusion, so no unsupported rim spills occur.
        for dz in -40..=40 {
            for dx in -40..=40 {
                let px = x + f64::from(dx);
                let pz = z + f64::from(dz);
                if lake.sample(px, pz).distance > lake.radius + 2.0 {
                    continue;
                }
                let climate = self.climate_biome_at(px, pz);
                if self.volcanic_watershed(climate) {
                    return None;
                }
                let h = self.watershed_ground(px, pz, climate);
                low = low.min(h);
                high = high.max(h);
                if self.river_at(px, pz).is_some() || self.oasis_at(px, pz).is_some() {
                    return None;
                }
            }
        }
        if low < self.config().sea_level + 4.0 || high - low > 13.0 {
            return None;
        }
        lake.level = low - 1.25;
        Some(lake)
    }
}

mod regional;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_lakes_are_deterministic_locally_level_and_have_closed_banks() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 2,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let mut found = None;
        'find: for z in -16..=16 {
            for x in -16..=16 {
                if let Some(lake) =
                    terrain.lake_at(f64::from(x) * 256.0 + 128.0, f64::from(z) * 256.0 + 128.0)
                {
                    found = Some(lake);
                    break 'find;
                }
            }
        }
        let lake = found.expect("test seed must contain a surface lake");
        let reload = TerraGenerator::new(terrain.config().clone()).unwrap();
        assert_eq!(
            terrain.lake_at(lake.center[0], lake.center[1]),
            reload.lake_at(lake.center[0], lake.center[1])
        );
        let mut wet = 0;
        for z in -50..=50 {
            for x in -50..=50 {
                let px = lake.center[0] + f64::from(x) + 0.5;
                let pz = lake.center[1] + f64::from(z) + 0.5;
                let column = terrain.surface_column(px, pz);
                let water = terrain.water_column(px, pz);
                assert_eq!(column.water_level, water.map(|w| w.level));
                if let Some(w) = water {
                    assert_eq!(w.level, lake.level);
                    wet += 1;
                    for [dx, dz] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                        if terrain.water_column(px + dx, pz + dz).is_none() {
                            assert!(
                                (terrain.surface_height(px + dx, pz + dz) - 0.5).floor() + 1.0
                                    >= w.level - 0.5
                            );
                        }
                    }
                }
            }
        }
        assert!(wet > 100);
    }
}
