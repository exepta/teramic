//! Seeded macro relief, before the existing ocean and hydrology carving.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
    landforms::terrace,
};
use noise::NoiseFn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandformKind {
    Plains,
    Hills,
    Plateau,
    Ridges,
    Basin,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandscapeRegionSample {
    /// Smooth partition: plains, hills, plateau, ridges, basin. No chunk-local IDs.
    pub weights: [f64; 5],
    /// Rare, connected rock/mesa uplift within plateau and ridge regions.
    pub landmark: f64,
}

impl LandscapeRegionSample {
    pub fn dominant(self) -> LandformKind {
        let mut best = 0;
        for i in 1..self.weights.len() {
            if self.weights[i] > self.weights[best] {
                best = i;
            }
        }
        [
            LandformKind::Plains,
            LandformKind::Hills,
            LandformKind::Plateau,
            LandformKind::Ridges,
            LandformKind::Basin,
        ][best]
    }
}

struct RegionFields {
    region: LandscapeRegionSample,
    broad: f64,
    ridge: f64,
}

fn region_weights(selector: f64) -> [f64; 5] {
    let edges = [(-0.52, -0.34), (-0.19, -0.03), (0.16, 0.32), (0.50, 0.68)]
        .map(|(start, end)| smoothstep(start, end, selector));
    [
        1.0 - edges[0],
        edges[0] - edges[1],
        edges[1] - edges[2],
        edges[2] - edges[3],
        edges[3],
    ]
}

impl TerraGenerator {
    /// Spatial macro-relief fields used by landscape revisions 2 and later. Older revisions
    /// never apply these fields. This diagnostic query has no cache or side effects.
    pub fn landscape_region(&self, x: f64, z: f64) -> LandscapeRegionSample {
        self.region_fields(x, z).region
    }

    pub(super) fn landform_partition(&self, x: f64, z: f64) -> LandscapeRegionSample {
        let warp = self.hills.get([x * 0.00054 + 127.3, z * 0.00054 - 63.1]);
        let selector = self.valleys.get([
            x * 0.00071 + warp * 0.26 + 53.7,
            z * 0.00071 - warp * 0.19 - 91.3,
        ]) * 1.65;
        let weights = region_weights(selector);
        let outcrop = self.hills.get([x * 0.0053 - 119.7, z * 0.0053 + 43.1]);
        let landmark =
            smoothstep(0.53, 0.72, outcrop) * smoothstep(0.25, 0.70, weights[2] + weights[3]);
        LandscapeRegionSample { weights, landmark }
    }

    fn region_fields(&self, x: f64, z: f64) -> RegionFields {
        let region = self.landform_partition(x, z);
        let broad = self.mountains.get([x * 0.0017 - 73.1, z * 0.0017 + 117.9]);
        let ridge = (1.0
            - self
                .valleys
                .get([
                    x * 0.0041 + z * 0.0011 + 37.3,
                    z * 0.0033 - x * 0.0008 - 13.9,
                ])
                .abs()
                * 1.7)
            .clamp(0.0, 1.0);
        RegionFields {
            region,
            broad,
            ridge,
        }
    }

    pub(super) fn regional_landscape_relief(
        &self,
        x: f64,
        z: f64,
        biome: BiomeSample,
        legacy: f64,
    ) -> f64 {
        let config = self.config();
        let land = 1.0 - smoothstep(0.12, 0.72, biome.weights.ocean);
        if land == 0.0 {
            return legacy;
        }
        let fields = self.region_fields(x, z);
        let profile = biome.profile();
        // Keep the biome's mountain-range elevation. Flat regions in the
        // highlands become alpine shelves rather than turning mountains into sea.
        let reference = config.base_height
            + profile.elevation
            + smoothstep(0.16, 0.68, biome.weights.highland) * config.mountain_amplitude * 1.65;
        let local = legacy - reference;
        let hills = config.hill_amplitude;
        let mountains = config.mountain_amplitude * profile.mountains;
        let varied = config.landscape_revision >= 3;
        let (plains_detail, plateau_detail, ridge_detail, basin_detail, shelf_strength) = if varied
        {
            (0.42, 0.42, 0.46, 0.38, 0.58)
        } else {
            (0.10, 0.12, 0.20, 0.12, 1.0)
        };
        let plains = reference + local * plains_detail + fields.broad * hills * 0.30;
        let rolling = reference
            + fields.broad * hills * 1.80
            + local * 0.38
            + (fields.ridge - 0.5) * hills * 0.65;
        let shelf = (fields.broad + 0.5) * hills * 2.80 + mountains * 0.12;
        let stepped = terrace(shelf, config.terrace_height * 2.4, 0.0);
        let plateau = reference
            + shelf
            + (stepped - shelf) * (config.terrace_strength * shelf_strength)
            + local * plateau_detail;
        let ridges = reference
            + fields.ridge.powi(3) * (config.mountain_amplitude * 0.08 + mountains * 0.60)
            + fields.broad * hills * 1.05
            + local * ridge_detail;
        let basin = reference - hills * (1.4 - fields.broad * 0.9) + local * basin_detail;
        let basin = if varied {
            // Compress deep lowlands instead of clamping them to a constant
            // sea-level shelf. The floor retains local relief and a smooth
            // slope, including where the compression starts.
            let floor = (config.sea_level + 2.0).min(reference)
                + hills * (fields.broad * 0.10 - 0.08)
                + local * 0.12;
            let below = (basin - floor).min(0.0);
            let depth = (hills * 0.65).max(0.001);
            basin - below + below / (1.0 - below / depth)
        } else {
            basin.max((config.sea_level + 2.0).min(reference))
        };
        let target = [plains, rolling, plateau, ridges, basin]
            .into_iter()
            .zip(fields.region.weights)
            .map(|(height, weight)| height * weight)
            .sum::<f64>()
            + fields.region.landmark * (hills * 2.30 + mountains * 0.30);
        // Height-field uplift stays attached to bedrock; no detached islands or
        // competing density engine. Ocean, rivers, lakes and structures run after it.
        legacy + (target - legacy) * land
    }
}

#[cfg(test)]
#[path = "landscape_region_tests.rs"]
mod tests;
