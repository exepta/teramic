use super::{Lake, OasisSample, TerraGenerator};
use crate::generation::oases::BANK_REACH;

impl Lake {
    fn lobes(self) -> [([f64; 2], f64); 3] {
        let (sin, cos) = self.phase.sin_cos();
        [
            (self.center, self.radius),
            (
                [
                    self.center[0] + cos * self.radius * 0.82,
                    self.center[1] + sin * self.radius * 0.82,
                ],
                self.radius * 0.72,
            ),
            (
                [
                    self.center[0] - cos * self.radius * 0.65,
                    self.center[1] - sin * self.radius * 0.65,
                ],
                self.radius * 0.55,
            ),
        ]
    }

    fn outline(self, angle: f64) -> f64 {
        1.0 + (angle * 3.0 + self.phase).sin() * 0.12 + (angle * 5.0 - self.phase).sin() * 0.055
    }

    pub(super) fn regional_sample(self, x: f64, z: f64) -> OasisSample {
        let (sin, cos) = self.phase.sin_cos();
        let distance = self
            .lobes()
            .into_iter()
            .map(|(center, radius)| {
                let dx = x - center[0];
                let dz = z - center[1];
                let u = (dx * cos + dz * sin) / 1.10;
                let v = (-dx * sin + dz * cos) * 1.10;
                u.hypot(v) / self.outline(v.atan2(u)) * self.radius / radius
            })
            .fold(f64::INFINITY, f64::min);
        OasisSample {
            center: self.center,
            radius: self.radius,
            distance,
            level: self.level,
            depth: self.depth,
        }
    }
}

impl TerraGenerator {
    pub(in crate::generation) fn build_regional_lake(
        &self,
        center: [f64; 2],
        seed: u32,
    ) -> Option<Lake> {
        let biome = self.climate_biome_at(center[0], center[1]);
        let center_height = self.base_height_with_biome(center[0], center[1], biome);
        if !self.config().biomes
            || biome.aridity() > 0.40
            || biome.weights.ocean > 0.65
            || center_height < self.config().sea_level + 5.0
        {
            return None;
        }
        let radius = match seed % 10 {
            0 => 138.0 + f64::from((seed >> 8) % 23),
            1..=4 => 80.0 + f64::from((seed >> 8) % 33),
            _ => 46.0 + f64::from((seed >> 8) % 25),
        };
        let mut lake = Lake {
            center,
            radius,
            level: 0.0,
            depth: 5.0 + radius * 0.045 + f64::from((seed >> 17) % 5),
            phase: f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0,
            lobes: true,
        };
        let (sin, cos) = lake.phase.sin_cos();
        let mut spill = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        // Trace the exposed shore at roughly one-metre arc spacing. The two
        // outer dry-bank rings need fewer samples. Even the largest lake stays
        // below 5000 probes, without scanning the complete basin area.
        for (center, radius) in lake.lobes() {
            let samples = (std::f64::consts::TAU * radius * 1.4).ceil() as usize;
            for sample in 0..samples {
                let angle = sample as f64 * std::f64::consts::TAU / samples as f64;
                let (a_sin, a_cos) = angle.sin_cos();
                for bank in [0.0, BANK_REACH * 0.5, BANK_REACH] {
                    if bank > 0.0 && sample % 4 != 0 {
                        continue;
                    }
                    let reach = (radius + bank * radius / lake.radius) * lake.outline(angle);
                    let u = a_cos * reach * 1.10;
                    let v = a_sin * reach / 1.10;
                    let x = center[0] + u * cos - v * sin;
                    let z = center[1] + u * sin + v * cos;
                    if lake.sample(x, z).distance < lake.radius - 1.0 {
                        continue;
                    }
                    let height = self.base_height_with_biome(x, z, self.climate_biome_at(x, z));
                    spill = spill.min(height);
                    high = high.max(height);
                }
            }
        }
        lake.level = (spill - 3.0).min(center_height + 3.0);
        if lake.level < self.config().sea_level + 2.0
            || center_height - lake.level > 22.0
            || high - lake.level > 45.0
        {
            return None;
        }
        Some(lake)
    }
}
