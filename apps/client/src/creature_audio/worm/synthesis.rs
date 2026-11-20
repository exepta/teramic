//! Original monster voice and recorded sand/water Foley, prepared once at load.
const RATE: f32 = 24_000.0;
const TAU: f32 = std::f32::consts::TAU;

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn envelope(t: f32, length: f32, attack: f32, release: f32) -> f32 {
    smooth(t / attack) * smooth((length - t) / release)
}
fn at(source: &[f32], position: f32) -> f32 {
    if source.len() < 2 {
        return 0.0;
    }
    let position = position.rem_euclid((source.len() - 1) as f32);
    let index = position as usize;
    source[index] + (source[index + 1] - source[index]) * position.fract()
}
fn finish(mut sound: Vec<f32>, peak: f32) -> Vec<f32> {
    let mut dc = 0.0;
    let dc_alpha = 1.0 - (-TAU * 28.0 / RATE).exp();
    let count = sound.len();
    for (i, sample) in sound.iter_mut().enumerate() {
        dc += (*sample - dc) * dc_alpha;
        let edge = smooth(i as f32 / 160.0) * smooth((count - 1 - i) as f32 / 240.0);
        *sample = (*sample - dc) * edge;
    }
    let maximum = sound
        .iter()
        .fold(0.0_f32, |maximum, &sample| maximum.max(sample.abs()));
    if maximum > 0.0 {
        for sample in &mut sound {
            *sample *= peak / maximum;
        }
    }
    sound
}

/// Breath drives a deep, irregular glottal voice rather than a clean sine drone.
pub(super) fn roar(death: bool, hurt: bool) -> Vec<f32> {
    let length = if hurt {
        0.52
    } else if death {
        2.65
    } else {
        2.35
    };
    let mut noise = Noise(if death {
        0xdea7_4219
    } else if hurt {
        0x7a11_002b
    } else {
        0x5a1d_71e9
    });
    let mut throat = 0.0;
    let mut breath = 0.0;
    let mut phase = 0.0_f32;
    let out = (0..(length * RATE) as usize)
        .map(|i| {
            let t = i as f32 / RATE;
            let progress = t / length;
            let raw = noise.next();
            throat += (raw - throat) * 0.19;
            breath += (raw - breath) * 0.017;
            let downward = if death {
                1.0 - progress * 0.48
            } else {
                0.78 + (-t * 2.8).exp() * 0.30
            };
            let pitch = (if hurt { 109.0 } else { 72.0 }) * downward
                + (t * 17.3).sin() * 3.1
                + (t * 7.7).sin() * 2.2;
            phase += TAU * pitch / RATE;
            let folds = (phase.sin() * 2.4 + (phase * 0.502).sin() * 0.68).tanh();
            let rasp = throat * (0.28 + folds.abs() * 0.65);
            let mouth = folds * 0.31
                + (phase * 2.01).sin() * 0.08
                + (phase * 4.03 + (t * 11.0).sin() * 0.3).sin() * 0.035;
            let effort = 0.82 + (t * 21.0).sin() * 0.10 + (t * 37.7).sin() * 0.04;
            let fade = envelope(
                t,
                length,
                if hurt { 0.025 } else { 0.12 },
                if hurt { 0.26 } else { 0.82 },
            );
            (mouth + rasp * 0.75 + breath * 0.28) * effort * fade
        })
        .collect();
    finish(out, if hurt { 0.56 } else { 0.68 })
}

pub(super) fn sand(source: &[f32], landing: bool) -> Vec<f32> {
    let length = if landing { 0.86 } else { 0.68 };
    let mut noise = Noise(if landing { 0x1a9d_204b } else { 0xd166_431f });
    let mut low = 0.0;
    let mut body = 0.0;
    let out = (0..(length * RATE) as usize)
        .map(|i| {
            let t = i as f32 / RATE;
            let raw = noise.next();
            low += (raw - low) * 0.25;
            body += (raw - body) * 0.013;
            let grain = at(source, i as f32 * if landing { 0.88 } else { 0.94 });
            let spray = grain * 0.55 + (low - body) * 0.11;
            let thump = if landing {
                (TAU * (48.0 * t + 14.0 * (1.0 - (-t * 6.0).exp()))).sin() * (-t * 8.5).exp() * 0.26
            } else {
                0.0
            };
            (spray + body * 0.30 + thump)
                * envelope(t, length, 0.012, if landing { 0.58 } else { 0.30 })
        })
        .collect();
    finish(out, if landing { 0.60 } else { 0.48 })
}

pub(super) fn bite(stone: &[f32], wood: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; 10_800];
    for (start, gain) in [(0, 0.85), (960, 0.50), (2_200, 0.24)] {
        for i in 0..6_500 {
            let t = i as f32 / RATE;
            out[start + i] += (at(stone, i as f32 * 1.10) * 0.60
                + at(wood, i as f32 * 0.82) * 0.40)
                * gain
                * envelope(t, 0.27, 0.004, 0.22);
        }
    }
    finish(out, 0.58)
}

pub(super) fn spit(water: &[f32]) -> Vec<f32> {
    let length = 0.88;
    let mut noise = Noise(0x6a11_e021);
    let mut low = 0.0;
    let mut phase = 0.0_f32;
    let out = (0..(length * RATE) as usize)
        .map(|i| {
            let t = i as f32 / RATE;
            low += (noise.next() - low) * 0.09;
            phase += TAU * (138.0 - t * 72.0 + (t * 37.0).sin() * 12.0) / RATE;
            let bubble = phase.sin() * ((t * 29.0).sin() * 0.5 + 0.5).powi(3) * 0.075;
            let jet = at(water, i as f32 * 0.84 + 1_100.0) * 0.50;
            (jet + low * 0.35 + bubble) * envelope(t, length, 0.04, 0.40)
        })
        .collect();
    finish(out, 0.56)
}

pub(super) fn breath() -> Vec<f32> {
    let length = 1.05;
    let mut noise = Noise(0xb7ea_7a14);
    let mut low = 0.0;
    let mut bass = 0.0;
    let out = (0..(length * RATE) as usize)
        .map(|i| {
            let t = i as f32 / RATE;
            low += (noise.next() - low) * 0.16;
            bass += (low - bass) * 0.018;
            (low - bass) * (0.86 + (t * 15.0).sin() * 0.09) * envelope(t, length, 0.18, 0.48)
        })
        .collect();
    finish(out, 0.30)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_worm_voice_and_foley_are_deterministic_declicked_and_bounded() {
        let foley: Vec<_> = (0..30_000)
            .map(|i| ((i as f32 * 0.073).sin() + (i as f32 * 0.021).sin() * 0.35) * 0.15)
            .collect();
        let clips = [
            roar(false, false),
            sand(&foley, false),
            sand(&foley, true),
            bite(&foley, &foley),
            spit(&foley),
            roar(false, true),
            roar(true, false),
            breath(),
        ];
        let mut signatures = std::collections::BTreeSet::new();
        for clip in clips {
            assert!((7_200..=72_000).contains(&clip.len()));
            assert!(
                clip.iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 0.681)
            );
            assert_eq!(clip[0], 0.0);
            assert_eq!(*clip.last().unwrap(), 0.0);
            let rms =
                (clip.iter().map(|sample| sample * sample).sum::<f32>() / clip.len() as f32).sqrt();
            assert!((0.008..0.35).contains(&rms), "RMS {rms}");
            signatures.insert((clip.len(), clip[clip.len() / 2].to_bits()));
        }
        assert_eq!(signatures.len(), 8);
        assert_eq!(roar(false, false), roar(false, false));
    }
}
