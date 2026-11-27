//! Rolling meadow ridges and weathered limestone in the temperate Plains.
use super::{TerraGenerator, biomes::smoothstep};
use noise::NoiseFn;
use terra_voxel::MaterialId;

impl TerraGenerator {
    pub(super) fn plains_height(&self, x: f64, z: f64, height: f64, weight: f64) -> f64 {
        if self.config().ecology_revision < 13
            || weight <= 0.4
            || self.config().hill_amplitude == 0.0
            || self.config().landscape_revision < 9
        {
            return height;
        }
        // Broad grazable backs alternate with shallow, level meadow benches.
        // Rivers and lakes erode this same field afterwards.
        let folds = self.hills.get([x / 132.0 + 23.7, z / 207.0 - 41.3]);
        let valley = self.valleys.get([x / 290.0 - 63.2, z / 190.0 + 47.6]);
        let amplitude = (self.config().hill_amplitude / 20.0).min(1.0);
        let rolling = height + folds * 5.5 + valley * 3.2;
        let step = 3.5;
        let contour = rolling / step;
        let bench = (contour.floor() + smoothstep(0.33, 0.94, contour.rem_euclid(1.0))) * step;
        let relief = rolling + (bench - rolling) * smoothstep(-0.13, 0.45, valley) * 0.7;
        height + (relief - height) * smoothstep(0.4, 0.88, weight) * amplitude
    }

    pub(super) fn plains_stone(
        &self,
        point: [f64; 2],
        weight: f64,
        slope: f64,
        wet_bank: bool,
        surface: MaterialId,
    ) -> MaterialId {
        if self.config().ecology_revision < 13
            || weight <= 0.52
            || slope < 0.72
            || !matches!(
                surface,
                MaterialId::Grass | MaterialId::Stone | MaterialId::Sand
            )
        {
            return surface;
        }
        let [x, z] = point;
        let outcrop = self.valleys.get([x / 39.0 + 42.2, z / 45.0 - 13.9]);
        if outcrop > 0.08 && (wet_bank || slope > 1.65) {
            if outcrop > 0.40 && wet_bank {
                MaterialId::MossyStone
            } else {
                MaterialId::MeadowLimestone
            }
        } else {
            surface
        }
    }
}
