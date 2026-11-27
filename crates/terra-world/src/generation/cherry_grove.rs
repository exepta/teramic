//! Offset shelves create flower meadows above natural streams. Hydrology cuts
//! the finished relief, so water and the banks share the same terrain height.
use super::{TerraGenerator, biomes::smoothstep};
use noise::NoiseFn;

impl TerraGenerator {
    pub(super) fn cherry_terraces(&self, x: f64, z: f64, base: f64, weight: f64) -> f64 {
        if self.config().ecology_revision < 15 || weight <= 0.15 {
            return base;
        }
        let amplitude = (self.config().hill_amplitude / 20.0).clamp(0.0, 1.25);
        let hill = self.hills.get([x / 155.0 + 23.1, z / 170.0 - 16.8]);
        let warp = self.valleys.get([x / 83.0 - 14.1, z / 90.0 + 27.6]);
        let ridge = smoothstep(-0.06, 0.46, hill);
        let crest = base + ridge * (10.0 + 3.5 * warp);
        let band = 5.0 + 0.8 * warp;
        let phase = (crest / band).rem_euclid(1.0);
        let shelf = (smoothstep(0.36, 0.88, phase) - phase) * band;
        base + smoothstep(0.15, 0.88, weight)
            * amplitude
            * (crest - base + shelf * (0.55 + 0.40 * ridge))
    }
}
