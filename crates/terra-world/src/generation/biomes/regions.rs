//! A land biome owns the area around a seeded site. Local hills, streams and
//! vegetation still vary inside the region; only actual sea water overrides it.
use super::{BiomeSample, BiomeWeights, smoothstep};
use crate::generation::{TerraGenerator, seed::layer_seed};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use terra_core::{BiomeClimate, BiomeKind};

#[derive(Clone, Copy)]
struct Site {
    center: [f64; 2],
    kind: BiomeKind,
}

#[derive(Default)]
pub(in crate::generation) struct RegionCache(Mutex<Neighborhoods>);
#[derive(Default)]
struct Neighborhoods {
    entries: HashMap<[i64; 2], Arc<[Site; 9]>>,
    order: VecDeque<[i64; 2]>,
}

fn neighbors([x, z]: [i64; 2]) -> [[i64; 2]; 9] {
    std::array::from_fn(|i| [x + (i % 3) as i64 - 1, z + (i / 3) as i64 - 1])
}

fn spacing(config: &terra_core::WorldConfig) -> f64 {
    (config.biome_scale * 0.48).max(960.0)
}

// A staggered triangular lattice lets each site meet six natural neighbours.
// Bounded jitter leaves at least (1 - 0.2 * sqrt(2)) spacings between sites.
const ROW_HEIGHT: f64 = 0.8660254037844386;

fn center(seed: u64, [x, z]: [i64; 2], spacing: f64) -> [f64; 2] {
    let id = seed
        ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z as u64).wrapping_mul(0x94d049bb133111eb);
    let hash = layer_seed(id, 913);
    let offset = |bits| 0.40 + f64::from(bits) * (0.20 / 65535.0);
    [
        (x as f64 + offset(hash & 65535) + if z & 1 == 0 { -0.25 } else { 0.25 }) * spacing,
        (z as f64 + offset(hash >> 16)) * (ROW_HEIGHT * spacing),
    ]
}

/// Two area-preserving shears curve the borders without compressing a site
/// out of existence. Reversing the shears gives the exact world-space centre.
struct Bend {
    spacing: f64,
    phase: [f64; 2],
}

impl Bend {
    fn new(spacing: f64, seed: u64) -> Self {
        let h = layer_seed(seed, 914);
        let angle = |n| f64::from(n) * (std::f64::consts::TAU / 65536.0);
        Self {
            spacing,
            phase: [angle(h & 65535), angle(h >> 16)],
        }
    }
    fn along_x(&self, z: f64) -> f64 {
        let t = z / self.spacing;
        self.spacing
            * (0.16 * (2.4 * t + self.phase[0]).sin() + 0.035 * (5.5 * t + self.phase[1]).sin())
    }
    fn along_z(&self, x: f64) -> f64 {
        let t = x / self.spacing;
        self.spacing
            * (0.15 * (2.8 * t + self.phase[1]).sin() + 0.028 * (5.7 * t - self.phase[0]).sin())
    }
    fn lattice(&self, x: f64, z: f64) -> [f64; 2] {
        let px = x + self.along_x(z);
        [px, z + self.along_z(px)]
    }
    fn world(&self, [px, pz]: [f64; 2]) -> [f64; 2] {
        let z = pz - self.along_z(px);
        [px - self.along_x(z), z]
    }
}

impl TerraGenerator {
    fn region_site(&self, cell: [i64; 2], bend: &Bend) -> (Site, BiomeKind) {
        let center = center(self.config().seed.0, cell, bend.spacing);
        let [x, z] = bend.world(center);
        let sample = self.apply_biome_overlays(x, z, self.biome_at_region_center(x, z));
        let mut weights = sample.weights;
        weights.ocean = 0.0;
        let land_kind = |weights: BiomeWeights| match weights.dominant() {
            BiomeKind::Ocean => BiomeKind::Plains,
            kind => kind,
        };
        let site = Site {
            center,
            kind: land_kind(weights),
        };
        weights.highland = 0.0;
        weights.desert = 0.0;
        weights.cliffted_desert = 0.0;
        weights.ashlands = 0.0;
        (site, land_kind(weights))
    }

    fn region_neighborhood(&self, cell: [i64; 2], bend: &Bend) -> Arc<[Site; 9]> {
        let cache = &self.biome_regions.0;
        if let Some(sites) = cache.lock().unwrap().entries.get(&cell).cloned() {
            return sites;
        }
        // Evaluate on a cache miss, outside the lock. Sites two lattice cells
        // apart cannot share a border with the bounded jitter used here.
        let sites = Arc::new(neighbors(cell).map(|key| {
            let (mut site, temperate) = self.region_site(key, bend);
            if site.kind.climate() == BiomeClimate::Cold
                && neighbors(key).into_iter().any(|next| {
                    next != key
                        && self.region_site(next, bend).0.kind.climate() == BiomeClimate::Hot
                })
            {
                site.kind = temperate;
            }
            site
        }));
        let mut cache = cache.lock().unwrap();
        if let Some(existing) = cache.entries.get(&cell) {
            return existing.clone();
        }
        if cache.order.len() == 256
            && let Some(old) = cache.order.pop_front()
        {
            cache.entries.remove(&old);
        }
        cache.order.push_back(cell);
        cache.entries.insert(cell, sites.clone());
        sites
    }

    pub(in crate::generation) fn regional_biome(
        &self,
        x: f64,
        z: f64,
        mut local: BiomeSample,
    ) -> BiomeSample {
        let ocean = local.weights.ocean;
        if ocean >= 1.0 {
            return local;
        }
        let spacing = spacing(self.config());
        let bend = Bend::new(spacing, self.config().seed.0);
        let [px, pz] = bend.lattice(x, z);
        let cell = [
            (px / spacing).floor() as i64,
            (pz / (ROW_HEIGHT * spacing)).floor() as i64,
        ];
        let sites = self.region_neighborhood(cell, &bend);
        let distances = sites.map(|site| {
            let dx = px - site.center[0];
            let dz = pz - site.center[1];
            (dx * dx + dz * dz).sqrt()
        });
        let closest = distances.into_iter().fold(f64::INFINITY, f64::min);
        let mut weights = [0.0_f64; 16];
        for (site, distance) in sites.iter().zip(distances) {
            let influence = 1.0 - smoothstep(0.0, spacing * 0.18, distance - closest);
            let weight = &mut weights[site.kind as usize];
            // Taking the strongest site for each biome keeps its nearest site
            // dominant even at three-way borders with repeated labels.
            *weight = weight.max(influence);
        }
        let land_factor = (1.0 - ocean) / weights.iter().sum::<f64>();
        for w in &mut weights[1..] {
            *w *= land_factor;
        }
        weights[0] = ocean;
        local.weights = BiomeWeights::from_values(weights);
        local
    }
}

#[cfg(test)]
#[path = "regions_tests.rs"]
mod tests;
