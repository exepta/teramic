//! Faulted quartz-bearing alpine massifs. Mineral provinces use an independent
//! kilometre-scale field and take their coverage only from cold or cool land.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

impl TerraGenerator {
    pub(super) fn with_crystal_highlands(
        &self,
        mut sample: BiomeSample,
        x: f64,
        z: f64,
    ) -> BiomeSample {
        let scale = self.config().biome_scale.max(480.) * 0.53;
        let province = self.valleys.get([x / scale - 226.72, z / scale + 112.39]);
        let cool = 1. - smoothstep(0.33, 0.5, sample.temperature);
        // Keep the glaciated cores of established Highlands intact. Quartz
        // uplifts the adjoining cool foothills and wooded plateaux instead.
        let foothill = 1. - smoothstep(0.08, 0.45, sample.weights.highland);
        let share = smoothstep(0.10, 0.40, province) * cool * foothill;
        let w = &mut sample.weights;
        for source in [&mut w.plains, &mut w.forest] {
            let transfer = *source * share;
            *source -= transfer;
            w.crystal_highlands += transfer;
        }
        sample
    }

    /// Glacial troughs expose gneiss ribbons below a broken chain of aiguilles.
    pub(super) fn crystal_height(&self, x: f64, z: f64, raw: f64, weight: f64) -> f64 {
        if weight < 0.015 {
            return raw;
        }
        let cirque = self.mountains.get([x / 156. + 62.1, z / 137. - 10.4]);
        let fracture = self.hills.get([x / 32. - 98.4, z / 38. + 118.6]);
        let dike = self.quartz_ridge(x, z, fracture);
        // Broad mountain shoulders carry the exposed narrow crest. Keeping the
        // fault field fixed retains the direction and position of the valleys.
        let crown = 40. * dike.powi(2) + 26. * dike.powi(5) * smoothstep(-0.18, 0.54, cirque);
        let shelf = smoothstep(-0.11, 0.035, cirque) * 12. + smoothstep(0.21, 0.30, cirque) * 13.;
        let trough = smoothstep(0.19, 0.56, -cirque) * 22.;
        let amplitude = (self.config().mountain_amplitude / 40.).clamp(0., 1.4);
        raw + (crown + shelf - trough + 11. + fracture * 4.5)
            * smoothstep(0.015, 0.86, weight)
            * amplitude
    }

    fn quartz_ridge(&self, x: f64, z: f64, fracture: f64) -> f64 {
        let warp = self.hills.get([x / 114. + 33.1, z / 129. - 56.4]);
        let ridge = self
            .valleys
            .get([(x + warp * 23.) / 103. - 15.4, z / 147. + 92.7]);
        (1.0 - (ridge + fracture * 0.14).abs()).clamp(0., 1.)
    }

    /// Lower local crowns can stand above the surrounding moor without
    /// reaching the absolute snow line. Keep both classes above the treeline.
    pub(super) fn crystal_summit(&self, x: f64, z: f64, height: f64) -> bool {
        if height >= 112. {
            return true;
        }
        if height < 86. {
            return false;
        }
        let fracture = self.hills.get([x / 32. - 98.4, z / 38. + 118.6]);
        self.quartz_ridge(x, z, fracture) > 0.895
    }

    pub(super) fn crystal_ground(&self, x: f64, z: f64, slope: f64, height: f64) -> M {
        let field = self.hills.get([x / 39. - 53.8, z / 31. + 25.2]);
        let fracture = self
            .valleys
            .get([x / 25. + field * 0.22 + 72.6, z / 37. - 24.3]);
        // Snow drifts in protected saddles well below the summit snowline.
        // Narrow leeward ledges intercept it even along otherwise sheer faces.
        if self.crystal_summit(x, z, height) {
            return if slope < 1.45 || slope < 3.2 && fracture > 0.12 {
                M::Snow
            } else {
                M::HighlandCliffStone
            };
        }
        let snowline = 88.0 - field * 31. - fracture * 21.;
        let ledge = (height / 6.8 + field * 2.3 + fracture).rem_euclid(1.);
        let lee =
            slope < 1.22 + fracture * 0.40 || (slope < 3.6 && ledge < 0.34 && fracture > -0.22);
        if lee && height > snowline {
            if height > snowline + 16. || field > 0.26 {
                M::Snow
            } else {
                M::SnowCoveredSod
            }
        } else if slope < 1.12 && field + fracture * 0.55 < -0.06 && height < snowline + 8. {
            M::Grass
        } else if slope > 0.48 && field + fracture * 0.27 > 0.43 && fracture.abs() < 0.26 {
            M::CrystalOre
        } else if slope > 0.74 && field < -0.47 && fracture > 0.15 {
            M::AuriciteOre
        } else if field < -0.30 && slope < 1.0 {
            M::CrystalStone
        } else if slope > 0.63 || fracture + field * 0.5 > 0.38 {
            M::HighlandCliffStone
        } else {
            M::CrystalGravel
        }
    }
}
