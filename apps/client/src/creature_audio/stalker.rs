//! Dry recorded airway Foley; scratch and scream retain their original cues.
pub(super) fn breath(source: &[f32], fast: bool) -> Vec<f32> {
    let duration = if fast { 1.55 } else { 2.8 };
    let mut output = vec![0.0; (duration * 24_000.0) as usize];
    if source.len() < 2 {
        return output;
    }
    let phases = if fast {
        [(0.05, 0.42, true), (0.57, 0.66, false)]
    } else {
        [(0.10, 0.85, true), (1.12, 1.30, false)]
    };
    for (start, length, inhale) in phases {
        let mut low = 0.0;
        let mut bass = 0.0;
        let low_alpha =
            1.0 - (-std::f32::consts::TAU * if inhale { 4500.0 } else { 3200.0 } / 24_000.0).exp();
        let bass_alpha = 1.0 - (-std::f32::consts::TAU * 160.0 / 24_000.0).exp();
        let count = (length * 24_000.0) as usize;
        for i in 0..count {
            let progress = i as f32 / count as f32;
            let at = ((if inhale { 0.0 } else { 0.08 })
                + progress * if inhale { 0.90 } else { 0.92 })
                * (source.len() - 1) as f32;
            let index = at as usize;
            let sample = source[index]
                + (source[(index + 1).min(source.len() - 1)] - source[index]) * at.fract();
            low += (sample - low) * low_alpha;
            bass += (low - bass) * bass_alpha;
            let smooth = |v: f32| {
                let v = v.clamp(0.0, 1.0);
                v * v * (3.0 - 2.0 * v)
            };
            let attack = smooth(progress / if inhale { 0.24 } else { 0.12 });
            let release = smooth((1.0 - progress) / if inhale { 0.28 } else { 0.40 });
            let flow = if inhale {
                0.72 + progress * 0.12
            } else {
                0.92 - progress * 0.20
            };
            output[(start * 24_000.0) as usize + i] = (low - bass) * attack * release * flow * 2.0;
        }
    }
    output
}
pub(super) fn scratch(wood: &[f32], leaves: &[f32]) -> Vec<f32> {
    (0..26_400)
        .map(|i| {
            let t = i as f32 / 24_000.0;
            let pulse = ((t * 22.0).sin() * 0.5 + 0.5).powi(3);
            let envelope = (t / 0.07).min(1.0) * ((1.1 - t) / 0.2).clamp(0.0, 1.0);
            (wood.get(i * 2).copied().unwrap_or(0.0) * 0.45
                + leaves.get(i).copied().unwrap_or(0.0) * 0.25)
                * pulse
                * envelope
        })
        .collect()
}
pub(super) fn scream() -> Vec<f32> {
    let mut seed = 0xdead_7931_u32;
    let mut low = 0.0;
    let mut phase = 0.0_f32;
    (0..36_000)
        .map(|i| {
            let t = i as f32 / 24_000.0;
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
            low += (noise - low) * 0.30;
            phase += std::f32::consts::TAU
                * (110.0 + 240.0 * (-t * 2.0).exp() + (t * 23.0).sin() * 8.0)
                / 24_000.0;
            let tone = phase.sin() * 0.22 + (phase * 2.8).sin() * 0.10 + (phase * 4.9).sin() * 0.06;
            (tone + low * 0.22) * (t / 0.12).min(1.0) * ((1.5 - t) / 0.6).clamp(0.0, 1.0)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalker_breath_and_scream_have_distinct_finite_energy_and_silent_edges() {
        let bank = super::super::clips::load().unwrap();
        let source = &bank[super::super::clips::STALKER_AIRWAY_CLIP];
        let clips = [breath(source, false), breath(source, true), scream()];
        for clip in &clips {
            assert!(clip.len() <= 72_000);
            assert!(clip.iter().all(|v| v.is_finite() && v.abs() < 0.95));
            assert!(clip.iter().map(|v| v * v).sum::<f32>() > 1.0);
            assert!(clip[0].abs() < 0.001 && clip.last().unwrap().abs() < 0.001);
        }
        assert_ne!(clips[0], clips[1]);
    }
    #[test]
    fn recorded_breath_has_separate_asymmetric_inhale_and_exhale_without_synthesized_water_or_tones()
     {
        let bank = super::super::clips::load().unwrap();
        let source = &bank[super::super::clips::STALKER_AIRWAY_CLIP];
        let slow = breath(source, false);
        let energy = |start: f32, end: f32| {
            slow[(start * 24_000.0) as usize..(end * 24_000.0) as usize]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
        };
        assert!(energy(0.2, 0.8) > 0.01);
        assert!(energy(1.3, 2.2) > 0.01);
        assert_eq!(energy(0.96, 1.11), 0.0);
        assert!(
            breath(&vec![0.0; source.len()], false)
                .iter()
                .all(|&v| v == 0.0),
            "no oscillator/noise/water layer may replace the recorded airway source"
        );
        assert_eq!(slow, breath(source, false));
    }
}
