//! Dunes, wind-cut mesas and stratified escarpments within the same desert.
use super::{TerraGenerator, biomes::smoothstep};
use noise::NoiseFn;

impl TerraGenerator {
    pub fn desert_overhaul(&self) -> bool {
        self.config().ecology_revision >= 14 && self.config().landscape_revision >= 9
    }

    /// Continuous iron-rich sediment field shared by block placement and the
    /// surface renderer where red dunes meet pale windblown sand.
    pub fn dune_sediment(&self, x: f64, z: f64) -> f64 {
        self.hills.get([x / 84.0 + 39.1, z / 91.0 - 21.6])
    }

    pub(super) fn desert_height(&self, x: f64, z: f64, height: f64, weight: f64) -> f64 {
        if !self.desert_overhaul() || weight <= 0.35 {
            return height;
        }
        let blend = smoothstep(0.35, 0.86, weight);
        let dunes = self.hills.get([x / 120.0 + 18.0, z / 185.0 - 43.0]);
        let ripple = ((x + z * 0.46) / 13.0 + dunes * 5.0).cos();
        let dune_height =
            (dunes * 3.2 + ripple * 1.7) * (self.config().hill_amplitude / 20.0).clamp(0.0, 1.5);
        let massif = self.valleys.get([x / 219.0 - 7.9, z / 261.0 + 51.3]);
        let fingers = self.hills.get([x / 49.0 + 42.0, z / 60.0 + 13.0]);
        // Only the upper tail of the ridge field rises above the dunes in new worlds.
        let ridge = massif + fingers * 0.17
            - if self.config().ecology_revision >= 15 {
                0.34
            } else {
                0.0
            };
        let ledges = smoothstep(0.01, 0.36, ridge) * 13.0
            + smoothstep(0.27, 0.39, ridge) * 18.0
            + smoothstep(0.43, 0.50, ridge) * 23.0;
        height
            + blend
                * (dune_height + ledges * (self.config().mountain_amplitude / 40.0).clamp(0.0, 1.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::WorldConfig;

    #[test]
    fn sparse_mesas_leave_dunes_between_the_remaining_desert_ranges() {
        let mesas = |revision| {
            let terrain = TerraGenerator::new(WorldConfig {
                ecology_revision: revision,
                ..Default::default()
            })
            .unwrap();
            let mut raised = 0;
            for z in (-4096..4096).step_by(64) {
                for x in (-4096..4096).step_by(64) {
                    let relief = terrain.desert_height(f64::from(x), f64::from(z), 0., 1.);
                    raised += usize::from(relief > 12.);
                }
            }
            raised
        };
        let before = mesas(14);
        let after = mesas(15);
        eprintln!("desert samples with >12 m additional relief {before} -> {after} / 16384");
        assert!(after > 0 && after < before / 2);
    }
}
