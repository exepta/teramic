//! Continuous, equal-power region transitions and long overlapping ambience loops.
use super::ambience::{LAYERS, LOOP_CLIPS};

const RATE: f32 = 48_000.0;
// A large region change takes several seconds; tiny boundary fluctuations never
// restart a fade. Smoothing power keeps two unrelated beds equally audible midway.
const RESPONSE: f32 = 1.0 / (RATE * 1.5);
const SENDS: [f32; LAYERS] = [
    0.0, 0.0, 0.32, 0.035, 0.06, 0.015, 0.025, 0.01, 0.015, 0.025, 0.04,
];

pub(super) struct Beds {
    power: [f32; LAYERS],
    target: [f32; LAYERS],
    phase: [[f32; 2]; LAYERS],
}

impl Default for Beds {
    fn default() -> Self {
        Self {
            power: [0.0; LAYERS],
            target: [0.0; LAYERS],
            phase: std::array::from_fn(|i| [i as f32 * 1973.0, i as f32 * 1973.0 + 7919.0]),
        }
    }
}

impl Beds {
    pub(super) fn retarget(&mut self, levels: [f32; LAYERS]) {
        self.target = levels.map(|gain| {
            if gain.is_finite() {
                gain.clamp(0.0, 1.0).powi(2)
            } else {
                0.0
            }
        });
    }

    pub(super) fn frame(&mut self, bank: &[Vec<f32>]) -> ([f32; 2], [f32; 2]) {
        let mut dry = [0.0; 2];
        let mut send = [0.0; 2];
        for (layer, &index) in LOOP_CLIPS.iter().enumerate() {
            self.power[layer] += (self.target[layer] - self.power[layer]) * RESPONSE;

            let Some(clip) = bank.get(index).filter(|clip| clip.len() >= 4) else {
                continue;
            };
            if self.target[layer] == 0. && self.power[layer] < 1e-16 {
                self.power[layer] = 0.;
                for phase in &mut self.phase[layer] {
                    advance(phase, clip.len() as f32);
                }
                continue;
            }
            let gain = self.power[layer].sqrt();
            for channel in 0..2 {
                let value = loop_frame(clip, &mut self.phase[layer][channel]) * gain;
                dry[channel] += value;
                send[channel] += value * SENDS[layer];
            }
        }
        (dry, send)
    }
}

fn interpolate(clip: &[f32], phase: f32) -> f32 {
    let index = phase as usize;
    let a = clip[index];
    let b = clip.get(index + 1).copied().unwrap_or(a);
    a + (b - a) * phase.fract()
}

fn loop_frame(clip: &[f32], phase: &mut f32) -> f32 {
    let length = clip.len() as f32;
    // Overlap up to two seconds of 24 kHz source audio. Start both channels at
    // separate positions, and preserve their phase even while the region is quiet.
    let overlap = (length * 0.25).min(48_000.0);
    if *phase >= length {
        *phase %= length;
    }
    let remaining = length - *phase;
    let mut value = interpolate(clip, *phase);
    if remaining < overlap {
        let mix = (1.0 - remaining / overlap).clamp(0.0, 1.0);
        value = value * (1.0 - mix).sqrt() + interpolate(clip, overlap - remaining) * mix.sqrt();
    }
    advance(phase, length);
    value
}

fn advance(phase: &mut f32, length: f32) {
    if *phase >= length {
        *phase %= length;
    }
    *phase += 0.5;
    if *phase >= length {
        *phase = (length * 0.25).min(48_000.) + (*phase - length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_swap_keeps_total_power_and_repeated_updates_do_not_restart_the_fade() {
        let mut beds = Beds::default();
        beds.power[3] = 0.01;
        let mut levels = [0.0; LAYERS];
        levels[4] = 0.1;
        let mut last = beds.power;
        for frame in 0..240_000 {
            if frame % 24_000 == 0 {
                beds.retarget(levels);
            }
            beds.frame(&[]);
            assert!(beds.power[3] <= last[3] && beds.power[4] >= last[4]);
            assert!((beds.power.iter().sum::<f32>() - 0.01).abs() < 0.00002);
            if frame == 48_000 {
                assert!(beds.power[3] > 0.004 && beds.power[4] > 0.004);
            }
            last = beds.power;
        }
        assert!(beds.power[3] < 0.0004 && beds.power[4] > 0.0095);
    }

    #[test]
    fn long_loop_overlap_has_no_seam_click_and_preserves_stereo_phase() {
        let clip: Vec<_> = (0..192_000)
            .map(|i| (i as f32 * 0.019).sin() * 0.2 + (i as f32 * 0.003).cos() * 0.1)
            .collect();
        let mut phase = clip.len() as f32 - 10.0;
        let mut previous = loop_frame(&clip, &mut phase);
        for _ in 0..200 {
            let value = loop_frame(&clip, &mut phase);
            assert!((value - previous).abs() < 0.005);
            previous = value;
        }
        assert!(phase > 48_000.0 && phase < 48_200.0);
        let mut invalid = Beds::default();
        invalid.retarget([f32::NAN; LAYERS]);
        assert_eq!(invalid.frame(&[]), ([0.0; 2], [0.0; 2]));
    }
}
