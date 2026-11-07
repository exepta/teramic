#[cfg(test)]
use super::{DrainageFrame, STEP};
use super::{MIN_WATERFALL_DROP, POINTS, RiverPath};
use crate::generation::biomes::smoothstep;

impl RiverPath {
    pub(super) fn fall_erosion(&self, index: usize, t: f64, lateral: f64, width: f64) -> f64 {
        let mut erosion = 0.0_f64;
        // Local path samples bound the cost; no additional terrain or cave probes.
        for i in index.saturating_sub(7)..=(index + 2).min(POINTS - 2) {
            let drop = self.points[i][1] - self.points[i + 1][1];
            if drop < MIN_WATERFALL_DROP {
                continue;
            }
            let cover = (self.ground[i] - self.points[i][1]).max(0.0);
            let surface = 1.0 - smoothstep(12.0 + drop * 0.7, 24.0 + drop * 1.5, cover);
            if surface <= 0.0 {
                continue;
            }
            let edge = (i as f64 + 0.5) * self.step;
            let upstream = (5.0 + drop * 0.18).clamp(6.0, 13.0);
            let downstream = (18.0 + drop * 0.75).clamp(24.0, 52.0);
            let along = smoothstep(edge - upstream, edge - 0.5, t)
                * (1.0 - smoothstep(edge + downstream * 0.45, edge + downstream, t));
            if along <= 0.0 {
                continue;
            }
            let shift =
                (t * 0.045 + self.phase).sin() * 4.0 + (t * 0.11 - self.phase * 0.7).sin() * 1.25;
            let reach = width
                + (12.0 + drop * 0.5).clamp(16.0, 34.0)
                    * (1.0 + (t * 0.06 + self.phase).sin() * 0.13);
            let across =
                (lateral + shift).abs() + (lateral * 0.19 + t * 0.08 + self.phase).sin() * 0.9;
            let opening = 1.0 - smoothstep(width + 2.0, reach, across);
            erosion = erosion.max(along * opening * surface);
        }
        erosion
    }
}

#[cfg(test)]
mod tests;
