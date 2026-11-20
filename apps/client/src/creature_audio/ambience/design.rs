//! Recorded nature preparation on the loading worker. Outdoor ambience keeps
//! the source recordings at their original pitch; there are no oscillator beds.
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

const RATE: f32 = 24_000.0;

/// Butterworth low-pass retains body without the old stair-stepped resampling.
pub(super) struct LowPass {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}
impl LowPass {
    pub fn new(cutoff: f32) -> Self {
        let omega = TAU * cutoff.clamp(25.0, 10_000.0) / RATE;
        let (sin, cos) = omega.sin_cos();
        let alpha = sin / (2.0 * FRAC_1_SQRT_2);
        let inverse = 1.0 / (1.0 + alpha);
        Self {
            b: [
                (1.0 - cos) * 0.5 * inverse,
                (1.0 - cos) * inverse,
                (1.0 - cos) * 0.5 * inverse,
            ],
            a: [-2.0 * cos * inverse, (1.0 - alpha) * inverse],
            z: [0.0; 2],
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let value = self.b[0] * sample + self.z[0];
        self.z[0] = self.b[1] * sample - self.a[0] * value + self.z[1];
        self.z[1] = self.b[2] * sample - self.a[1] * value;
        value
    }
}

pub(super) fn edge(index: usize, length: usize, samples: usize) -> f32 {
    let distance = index.min(length.saturating_sub(index + 1));
    let x = (distance as f32 / samples as f32).min(1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Scatter existing leaf recordings at their original pitch and speed. Quiet
/// gaps keep the undergrowth from becoming a continuous wind/noise generator.
pub(super) fn rustles(a: &[f32], b: &[f32], seconds: f32, mut seed: u32) -> Vec<f32> {
    let mut output = vec![0.0; (seconds * RATE) as usize];
    let mut random = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed & 0xffff) as f32 / 65535.0
    };
    let mut onset = 1.0 + random() * 3.0;
    while onset < seconds - 1.0 {
        let source = if random() < 0.5 { a } else { b };
        let gain = 0.25 + random() * 0.20;
        let begin = (onset * RATE) as usize;
        for (index, (&sample, out)) in source.iter().zip(output[begin..].iter_mut()).enumerate() {
            *out += sample * gain * edge(index, source.len(), 480);
        }
        onset += 5.0 + random() * 8.0;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorded_rustles_keep_pitch_leave_quiet_gaps_and_do_not_create_synthetic_sound() {
        let a: Vec<_> = (0..12_000)
            .map(|i| (i as f32 * 0.052).sin() * 0.2)
            .collect();
        let field = rustles(&a, &a, 32.0, 0xa947_31d5);
        assert_eq!(field.len(), 32 * 24_000);
        assert_eq!(field, rustles(&a, &a, 32.0, 0xa947_31d5));
        assert!(field.iter().all(|s| s.is_finite() && s.abs() <= 0.14));
        let quiet = field.iter().filter(|&&s| s == 0.0).count();
        assert!(
            quiet > field.len() * 85 / 100,
            "leave space between foliage movements"
        );
        let first = field.iter().position(|&s| s != 0.0).unwrap() - 1;
        let reference = 1500;
        let scale = field[first + reference] / a[reference];
        for index in 1000..11_000 {
            assert!(
                (field[first + index] - a[index] * scale).abs() < 0.00001,
                "a recorded leaf must keep its waveform and pitch"
            );
        }
        let silent = rustles(&[0.0; 24_000], &[], 10.0, 0xa947_31d5);
        assert!(silent.iter().all(|&sample| sample == 0.0));
    }
}
