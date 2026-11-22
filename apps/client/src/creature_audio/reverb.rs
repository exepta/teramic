//! Four damped delay lines per channel. Allocates once, not per voice or sample.
pub struct Reverb {
    lines: Vec<Delay>,
    tail_frames: u32,
}
struct Delay {
    samples: Vec<f32>,
    cursor: usize,
    low: f32,
}
impl Default for Reverb {
    fn default() -> Self {
        Self {
            tail_frames: 0,
            lines: [2411, 3221, 4073, 4987, 2531, 3371, 4211, 5113]
                .into_iter()
                .map(|n| Delay {
                    samples: vec![0.0; n],
                    cursor: 0,
                    low: 0.0,
                })
                .collect(),
        }
    }
}
impl Reverb {
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        // At 48 kHz the longest line has repeated more than fifty times
        // after six seconds (feedback 0.73). Silent categories need no DSP.
        if input.iter().any(|sample| sample.abs() > 1e-9) {
            self.tail_frames = 6 * 48_000;
        } else if self.tail_frames == 0 {
            return [0.; 2];
        } else {
            self.tail_frames -= 1;
        }
        let mut output = [0.0; 2];
        for (i, line) in self.lines.iter_mut().enumerate() {
            let delayed = line.samples[line.cursor];
            line.low += (delayed - line.low) * 0.32;
            line.samples[line.cursor] = input[i / 4] * 0.45 + line.low * 0.73;
            line.cursor += 1;
            if line.cursor == line.samples.len() {
                line.cursor = 0;
            }
            output[i / 4] += delayed * 0.22;
        }
        output
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impulse_has_predelay_audible_tail_and_stable_decay() {
        let mut reverb = Reverb::default();
        let mut early = 0.0_f32;
        let mut tail = 0.0_f32;
        let mut late = 0.0_f32;
        for frame in 0..240_000 {
            let out = reverb.process(if frame == 0 { [1.0; 2] } else { [0.0; 2] });
            assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            if frame < 2400 {
                early += out[0].abs();
            }
            if (2400..96_000).contains(&frame) {
                tail += out[0].abs();
            }
            if frame > 192_000 {
                late += out[0].abs();
            }
        }
        assert_eq!(early, 0.0);
        assert!(tail > 0.1 && late < tail * 0.001);
    }
}
