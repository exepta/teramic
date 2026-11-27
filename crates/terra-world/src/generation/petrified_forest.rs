//! Eroded shelves around a dry prehistoric woodland. The province borrows
//! only temperate land coverage, preserving sea and volcanic catchments.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

impl TerraGenerator {
    pub(super) fn with_petrified_forest(
        &self,
        mut sample: BiomeSample,
        x: f64,
        z: f64,
    ) -> BiomeSample {
        let scale = self.config().biome_scale.max(480.) * 0.54;
        let province = self.hills.get([x / scale + 153.42, z / scale - 114.7]);
        let dry = 1. - smoothstep(0.48, 0.72, sample.humidity);
        let mild = smoothstep(0.26, 0.45, sample.temperature);
        let share = smoothstep(0.13, 0.44, province) * dry * mild;
        let w = &mut sample.weights;
        for source in [&mut w.plains, &mut w.forest, &mut w.savanna] {
            let fossil = *source * share;
            *source -= fossil;
            w.petrified_forest += fossil;
        }
        sample
    }

    pub(super) fn petrified_height(&self, x: f64, z: f64, raw: f64, weight: f64) -> f64 {
        if weight < 0.02 {
            return raw;
        }
        let ridge = self.valleys.get([x / 127. + 64.2, z / 152. - 29.1]);
        let talus = self.hills.get([x / 39. + 41.6, z / 51. - 35.1]);
        let edge = ridge + talus * 0.20;
        let steps = smoothstep(-0.12, 0.06, edge) * 5.0
            + smoothstep(0.16, 0.25, edge) * 10.0
            + smoothstep(0.34, 0.42, edge) * 14.0;
        let amplitude = (self.config().mountain_amplitude / 40.).clamp(0., 1.3);
        let steps = if self.config().ecology_revision >= 20 {
            // Narrow shelves separate a low gravel wash from its eroding banks.
            let bench = self.valleys.get([x / 48. + 19.8, z / 57. - 13.2]);
            steps + smoothstep(0.18, 0.34, bench) * 2.3 - smoothstep(0.25, 0.48, -bench) * 2.0
        } else {
            steps
        };
        raw + (steps - 3.) * smoothstep(0.02, 0.9, weight) * amplitude
    }

    // A shallow fossil gallery runs under the ledge. Its ceiling meets the
    // surface at occasional roof falls, so the chamber is reachable on foot.
    pub(super) fn fossil_gallery(
        &self,
        x: f64,
        z: f64,
        surface: f64,
        weight: f64,
    ) -> Option<[f64; 2]> {
        if weight <= 0.55 || !self.config().caves || surface < self.config().sea_level + 19. {
            return None;
        }
        let seam = self.valleys.get([x / 71. + 47.8, z / 62. - 65.2]);
        let belt = self.hills.get([x / 153. - 64.5, z / 163. + 45.9]);
        let opening = smoothstep(0.15, 0.38, belt)
            * smoothstep(0.55, 0.94, weight)
            * (1. - smoothstep(0.06, 0.25, seam.abs()));
        if opening <= 0.02 {
            return None;
        }
        let collapse = smoothstep(0.22, 0.52, self.hills.get([x / 28. + 16.7, z / 25. - 81.2]));
        let center = surface - 8. + opening * collapse * 5.5;
        let half = opening * 5.8;
        let ceiling = center + half;
        // A collapsed gallery opens to daylight. A sub-voxel soil cap on a
        // sloping cave lip can otherwise leave individual blocks hanging in air.
        let ceiling = if surface - ceiling < 3.0 {
            surface + 1.0
        } else {
            ceiling
        };
        Some([center - half, ceiling])
    }

    pub(super) fn petrified_ground(&self, x: f64, z: f64, slope: f64) -> M {
        let seam = self.hills.get([x / 24. - 58.3, z / 19. + 43.]);
        if self.config().ecology_revision >= 20 && slope < 0.7 {
            // Thin silt drifts interrupt exposed limestone pavement. Branching
            // mineral seams follow the wash for several metres at a time.
            let bedding = self
                .valleys
                .get([x / 19. + 7.2 + seam * 0.36, z / 23. - 21.8]);
            let mantle = seam * 0.56 + bedding * 0.68;
            if mantle < -0.18 {
                return M::CrackedStoneSoil;
            }
            if mantle > 0.46 && bedding < 0.48 {
                return M::TurquoiseVeinedStone;
            }
            if mantle > 0.32 && bedding.abs() < 0.08 {
                return M::FossiliferousStone;
            }
        }
        if self.config().ecology_revision >= 20 && slope > 0.58 && seam < -0.44 {
            M::FossiliferousStone
        } else if self.config().ecology_revision >= 20 && slope > 0.47 && seam < -0.28 {
            M::RootBoundStone
        } else if self.config().ecology_revision >= 20 && slope > 0.52 && seam > 0.24 {
            M::TurquoiseVeinedStone
        } else if slope > 0.54 && seam > 0.42 {
            M::MineralEncrustedStone
        } else if slope > 0.7 || seam < -0.19 {
            M::CrackedStoneSoil
        } else {
            M::DustyGround
        }
    }
}
