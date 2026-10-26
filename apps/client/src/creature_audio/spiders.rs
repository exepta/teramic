//! Small Foley layers and breath-shaped noise; no external recordings or decoders.
pub(super) fn step(stone: &[f32], leaves: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; 7_200];
    for (tap, gain) in [(0, 0.58), (720, 0.33), (1_490, 0.21), (2_300, 0.13)] {
        for i in 0..2_600 {
            let t = i as f32 / 2_600.0;
            let envelope = (t / 0.045).min(1.0) * (1.0 - t).powi(3);
            out[tap + i] += (stone.get(i * 2).copied().unwrap_or(0.0) * 0.65
                + leaves.get(i).copied().unwrap_or(0.0) * 0.16)
                * envelope
                * gain;
        }
    }
    out
}

pub(super) fn idle(leaves: &[f32]) -> Vec<f32> {
    let mut low = 0.0;
    (0..18_000)
        .map(|i| {
            let t = i as f32 / 24_000.0;
            let raw = leaves.get(i % leaves.len().max(1)).copied().unwrap_or(0.0);
            low += (raw - low) * 0.18;
            let pulse = ((t * 12.0).sin() * 0.5 + 0.5).powi(3);
            low * pulse * (t / 0.08).min(1.0) * ((0.75 - t) / 0.2).clamp(0.0, 1.0) * 0.7
        })
        .collect()
}

pub(super) fn hiss() -> Vec<f32> {
    let mut seed = 0x7370_6964_u32;
    let mut low = 0.0;
    let mut bass = 0.0;
    (0..31_200)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            low += (noise - low) * 0.42;
            bass += (low - bass) * 0.07;
            let t = i as f32 / 24_000.0;
            let envelope = (t / 0.11).min(1.0) * ((1.3 - t) / 0.45).clamp(0.0, 1.0);
            let breath = 0.72 + (t * 19.0).sin() * 0.13 + (t * 37.0).sin() * 0.05;
            (low - bass) * envelope * breath * 0.48
        })
        .collect()
}

/// A clean forward water stroke: body, surface swish and a late soft bubble tail.
pub(super) fn swim(source: &[f32], bubble: &[f32], variant: usize) -> Vec<f32> {
    let length = 20_400 + variant * 960;
    let mut low = 0.0;
    let mut bass = 0.0;
    let shift = variant * 731;
    (0..length)
        .map(|i| {
            let t = i as f32 / length as f32;
            let sample = source
                .get((i + shift) % source.len().max(1))
                .copied()
                .unwrap_or(0.0);
            low += (sample - low) * 0.24;
            bass += (low - bass) * 0.012;
            let tail_index = i.saturating_sub(length / 3);
            let bubbles = if i > length / 3 {
                bubble
                    .get(tail_index + variant * 103)
                    .copied()
                    .unwrap_or(0.0)
                    * 0.11
            } else {
                0.0
            };
            let attack = (t / 0.065).min(1.0);
            let release = ((1.0 - t) / 0.35).clamp(0.0, 1.0);
            let stroke = 0.75 + (t * std::f32::consts::PI).sin() * 0.25;
            ((low - bass * 0.55 + bubbles) * attack * release * stroke * 1.2).clamp(-0.62, 0.62)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spider_and_water_foley_are_bounded_declicked_and_have_distinct_envelopes() {
        let source: Vec<_> = (0..24_000)
            .map(|i| (i as f32 * 0.035).sin() * 0.24)
            .collect();
        for clip in [
            step(&source, &source),
            idle(&source),
            hiss(),
            swim(&source, &source, 0),
            swim(&source, &source, 3),
        ] {
            assert!(clip.iter().all(|v| v.is_finite() && v.abs() <= 0.65));
            assert!(clip.first().unwrap().abs() < 0.001);
            assert!(clip.last().unwrap().abs() < 0.001);
            assert!(clip.iter().any(|v| v.abs() > 0.01));
        }
        assert_eq!(hiss(), hiss());
        assert_ne!(swim(&source, &source, 0), swim(&source, &source, 1));
    }
}
