use super::{CaveEcology, CaveStructure, smoothstep};

impl CaveStructure {
    pub(super) fn bedrock_chamber(self) -> bool {
        self.revision >= 11
            && matches!(
                self.kind,
                CaveEcology::Stone | CaveEcology::Lush | CaveEcology::Waterfall
            )
    }

    /// A winding bedding-plane passage crossed by a shorter joint. Both lobes
    /// stay inside the owning region, including when it borders a suppressed room.
    pub(super) fn bedrock_contour(self, u: f64, v: f64) -> f64 {
        let phase = f64::from(self.seed % 628) * 0.01;
        let cross = v - 0.14 * ((u * 3.7 + phase).sin() - phase.sin());
        let width = 0.51 + 0.12 * (u * 5.7 - phase).cos();
        let passage = ((u / 0.94).powi(2) + (cross / width).powi(2)).sqrt();
        let side = if self.seed & 2 == 0 { 1.0 } else { -1.0 };
        let joint = (((u - 0.17 - side * cross * 0.28) / 0.35).powi(2)
            + ((cross - side * 0.28) / 0.47).powi(2))
        .sqrt();
        passage.min(joint).max((u * u + v * v).sqrt() / 0.985)
    }

    pub(super) fn bedrock_section(self, x: f64, z: f64, u: f64, v: f64, r: f64) -> [f64; 2] {
        let phase = f64::from(self.seed % 628) * 0.01;
        let base = self.center[1] - self.height * 0.72;
        let floor = if self.dynamic_water {
            self.cascade_floor(x, z)
        } else {
            let channel = self.channel_mask(u, v);
            // Gently dipping beds weather into broad benches. Ease the face of
            // each step so voxel sampling keeps ramps between adjacent benches.
            let dip = v * 0.075 + (u * 0.041 + phase).sin() * 1.15;
            let bench = (dip.floor() + smoothstep(0.56, 0.94, dip - dip.floor())) * 1.65;
            base + (2.0 + bench) * (1.0 - channel) - channel * 3.1
                + smoothstep(0.73, 1.02, r) * self.height * 1.05
        };
        // Sections along strike alternate between tall vaults and low saddles;
        // the roof dips across the chamber, meeting the raised outer benches.
        let vault =
            0.66 + 0.20 * (u * 0.065 + phase).cos() + 0.12 * ((v - u * 0.25) * 0.084 - phase).sin();
        let walls = smoothstep(0.62, 1.0, r);
        let ceiling = self.center[1] + self.height * (vault * (1.0 - walls) + 0.2 - walls * 0.57);
        [floor.max(-118.5), ceiling]
    }

    pub(super) fn bedrock_junction(self, y: f64, old: f64, chamber: f64) -> f64 {
        let invert = self
            .water
            .map_or(self.floor, |water| water.level.max(self.floor));
        let reach = smoothstep(0.62, 0.85, self.radial) * smoothstep(invert + 0.5, invert + 2.5, y);
        let wall = chamber.min((1.0 - self.radial) * self.radius);
        // A smooth union rounds the re-entrant angle where both surfaces meet.
        let overlap = (2.0 - (wall - old).abs()).max(0.0);
        let union = wall.max(old) + overlap * overlap * 0.125;
        let joined = chamber + (union - chamber) * reach;
        let opening = smoothstep(3.0, 6.0, self.ceiling - self.floor);
        old + (joined - old) * opening
    }
}
