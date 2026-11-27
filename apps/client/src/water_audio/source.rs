use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

const RATE: f32 = 48_000.0;
const SMOOTH: f32 = 1.0 / (RATE * 0.14);

/// Audio-thread controls are read once per 256 frames, without locks or allocations.
#[derive(Default)]
pub(super) struct MixControls {
    gains: [[AtomicU32; 2]; 3],
    submerged: AtomicU32,
    time: AtomicU32,
    filter_blend: AtomicU32,
    output_rms: AtomicU32,
}

impl MixControls {
    pub(super) fn output_rms(&self) -> f32 {
        f32::from_bits(self.output_rms.load(Ordering::Relaxed))
    }

    pub(super) fn filter_blend(&self) -> f32 {
        f32::from_bits(self.filter_blend.load(Ordering::Relaxed))
    }

    pub(super) fn update(&self, gains: [[f32; 2]; 3], submerged: bool, time: f32) {
        for (voice, target) in self.gains.iter().zip(gains) {
            for (channel, gain) in voice.iter().zip(target) {
                channel.store(gain.clamp(0.0, 0.35).to_bits(), Ordering::Relaxed);
            }
        }
        self.submerged
            .store(u32::from(submerged), Ordering::Relaxed);
        self.time.store(time.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Asset, TypePath)]
pub(crate) struct WaterAudioSource(pub(super) Arc<MixControls>);

impl WaterAudioSource {
    pub(crate) fn isolated(gains: [f32; 3]) -> Self {
        let controls = Arc::new(MixControls::default());
        controls.update(gains.map(|gain| [gain; 2]), false, 0.0);
        Self(controls)
    }
}

impl Decodable for WaterAudioSource {
    type Decoder = WaterStream;

    fn decoder(&self) -> Self::Decoder {
        WaterStream {
            controls: self.0.clone(),
            voices: [
                Noise::new(0x3a7c19e1),
                Noise::new(0x85ba310d),
                Noise::new(0xd2e1a83f),
            ],
            gains: [[0.0; 2]; 3],
            targets: [[0.0; 2]; 3],
            wet: 0.0,
            target_wet: 0.0,
            low_pass: [[0.0; 2]; 2],
            stereo: [0.0; 2],
            right_channel: false,
            frame: 0,
            time: 0.0,
            energy: 0.0,
        }
    }
}

pub(crate) struct WaterStream {
    controls: Arc<MixControls>,
    voices: [Noise; 3],
    gains: [[f32; 2]; 3],
    targets: [[f32; 2]; 3],
    wet: f32,
    target_wet: f32,
    low_pass: [[f32; 2]; 2],
    stereo: [f32; 2],
    right_channel: bool,
    frame: u32,
    time: f32,
    energy: f32,
}

struct Noise {
    seed: u32,
    bass: f32,
    mid: f32,
    fizz: f32,
    brook: f32,
    bubble_phase: f32,
    bubble_amplitude: f32,
    bubble_frequency: f32,
    next_bubble: u32,
}

impl Noise {
    fn new(seed: u32) -> Self {
        Self {
            seed,
            bass: 0.0,
            mid: 0.0,
            fizz: 0.0,
            brook: 0.0,
            bubble_phase: 0.0,
            bubble_amplitude: 0.0,
            bubble_frequency: 0.0,
            next_bubble: 4_000,
        }
    }

    fn next(&mut self, kind: usize, time: f32) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        let white = (self.seed as f64 / f64::from(u32::MAX) * 2.0 - 1.0) as f32;
        self.bass += 0.008 * (white - self.bass);
        self.mid += 0.095 * (white - self.mid);
        self.fizz += 0.44 * (white - self.fizz);
        match kind {
            // The slow swell follows the water shader's 0.6 rad/s wave clock.
            0 => {
                self.brook += 0.009 * (white - self.brook);
                (self.brook * 1.1 + self.bass * 0.45) * (0.68 + 0.14 * (time * 0.6).sin())
            }
            1 => {
                self.brook += 0.018 * (white - self.brook);
                let ripple = 0.75 + 0.08 * (time * 1.7).sin() + 0.05 * (time * 3.1).sin();
                (self.brook * 1.9 + self.mid * 0.12 + self.bubble() * 0.7) * ripple
            }
            _ => self.bass * 3.2 + self.mid * 1.5 + self.fizz * 0.65,
        }
    }

    fn bubble(&mut self) -> f32 {
        if self.next_bubble == 0 {
            self.next_bubble = 6_000 + self.seed % 18_000;
            self.bubble_frequency = 240.0 + (self.seed >> 12) as f32 % 420.0;
            self.bubble_amplitude = 0.06 + (self.seed & 255) as f32 / 255.0 * 0.08;
            self.bubble_phase = 0.0;
        }
        self.next_bubble -= 1;
        self.bubble_phase = (self.bubble_phase + self.bubble_frequency / RATE).fract();
        self.bubble_amplitude *= 0.9992;
        self.bubble_frequency *= 0.99997;
        (self.bubble_phase * std::f32::consts::TAU).sin() * self.bubble_amplitude
    }
}

impl WaterStream {
    fn next_frame(&mut self) {
        if self.frame.is_multiple_of(256) {
            for (target, voice) in self.targets.iter_mut().zip(&self.controls.gains) {
                for (gain, control) in target.iter_mut().zip(voice) {
                    *gain = f32::from_bits(control.load(Ordering::Relaxed));
                }
            }
            self.target_wet = self.controls.submerged.load(Ordering::Relaxed) as f32;
            self.time = f32::from_bits(self.controls.time.load(Ordering::Relaxed));
            self.controls
                .filter_blend
                .store(self.wet.to_bits(), Ordering::Relaxed);
            self.controls
                .output_rms
                .store((self.energy / 512.0).sqrt().to_bits(), Ordering::Relaxed);
            self.energy = 0.0;
        }
        self.frame = self.frame.wrapping_add(1);
        self.time += 1.0 / RATE;
        self.wet += (self.target_wet - self.wet) * SMOOTH;
        let mut mixed = [0.0; 2];
        for (kind, voice) in self.voices.iter_mut().enumerate() {
            let noise = voice.next(kind, self.time);
            for (channel, sample) in mixed.iter_mut().enumerate() {
                self.gains[kind][channel] +=
                    (self.targets[kind][channel] - self.gains[kind][channel]) * SMOOTH;
                *sample += noise * self.gains[kind][channel];
            }
        }
        // Two real low-pass poles at 650 Hz underwater, smoothly blended from 12 kHz.
        let cutoff = 12_000.0 + (650.0 - 12_000.0) * self.wet;
        let alpha = cutoff / (cutoff + RATE / std::f32::consts::TAU);
        for (channel, sample) in mixed.into_iter().enumerate() {
            self.low_pass[0][channel] += alpha * (sample - self.low_pass[0][channel]);
            self.low_pass[1][channel] +=
                alpha * (self.low_pass[0][channel] - self.low_pass[1][channel]);
            self.stereo[channel] =
                (self.low_pass[1][channel] * (1.0 - self.wet * 0.35)).clamp(-0.95, 0.95);
            self.energy += self.stereo[channel] * self.stereo[channel];
        }
    }
}

impl Iterator for WaterStream {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.right_channel {
            self.next_frame();
        }
        let sample = self.stereo[usize::from(self.right_channel)];
        self.right_channel = !self.right_channel;
        Some(sample)
    }
}

impl Source for WaterStream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN.saturating_add(1)
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::MIN.saturating_add(47_999)
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn energy(wet: bool) -> (f64, f64, f32) {
        let controls = Arc::new(MixControls::default());
        controls.update([[0.28; 2]; 3], wet, 3.0);
        let mut stream = WaterAudioSource(controls).decoder();
        for _ in 0..96_000 {
            stream.next();
        }
        let mut square = 0.0;
        let mut high = 0.0;
        let mut peak: f32 = 0.0;
        let mut previous = 0.0;
        for _ in 0..48_000 {
            let sample = stream.next().unwrap();
            stream.next();
            assert!(sample.is_finite());
            square += f64::from(sample * sample);
            high += f64::from((sample - previous).powi(2));
            peak = peak.max(sample.abs());
            previous = sample;
        }
        (square, high, peak)
    }

    #[test]
    fn underwater_reduces_high_frequencies_and_peak_is_bounded() {
        let dry = energy(false);
        let wet = energy(true);
        assert!(dry.0 > 1.0 && wet.0 > 0.1);
        assert!(wet.1 / wet.0 < dry.1 / dry.0 * 0.15);
        assert!(wet.0 < dry.0);
        assert!(dry.2 < 0.95 && wet.2 < 0.95);
    }

    #[test]
    fn disabling_soundscape_fades_to_silence() {
        let controls = Arc::new(MixControls::default());
        controls.update([[0.3; 2]; 3], false, 0.0);
        let mut stream = WaterAudioSource(controls.clone()).decoder();
        for _ in 0..48_000 {
            stream.next();
        }
        controls.update([[0.0; 2]; 3], false, 1.0);
        for _ in 0..192_000 {
            stream.next();
        }
        assert!(stream.take(1024).all(|sample| sample.abs() < 0.00001));
    }

    fn configured_energy(gains: [[f32; 2]; 3], config: &terra_core::AudioConfig) -> f64 {
        let controls = Arc::new(MixControls::default());
        controls.update(
            super::super::scene::apply_audio_settings(gains, config),
            false,
            0.0,
        );
        let mut stream = WaterAudioSource(controls).decoder();
        for _ in 0..48_000 {
            stream.next();
        }
        stream
            .take(16_384)
            .map(|sample| f64::from(sample).powi(2))
            .sum()
    }

    #[test]
    fn audio_settings_reach_the_decoder_and_silence_each_category() {
        use terra_core::AudioConfig;
        let audible = configured_energy([[0.2; 2]; 3], &AudioConfig::default());
        assert!(audible > 0.01);
        assert_eq!(
            configured_energy(
                [[0.2; 2]; 3],
                &AudioConfig {
                    animals_volume: 0.0,
                    footsteps_volume: 0.0,
                    ..default()
                }
            ),
            audible,
            "animal and footstep sliders do not change any water layer",
        );
        for config in [
            AudioConfig {
                master_volume: 0.0,
                ..default()
            },
            AudioConfig {
                ambience_volume: 0.0,
                ..default()
            },
            AudioConfig {
                muted: true,
                ..default()
            },
        ] {
            assert_eq!(configured_energy([[0.2; 2]; 3], &config), 0.0);
        }
        for kind in 0..3 {
            let mut gains = [[0.0; 2]; 3];
            gains[kind] = [0.2; 2];
            let mut config = AudioConfig::default();
            match kind {
                0 => config.water_volume = 0.0,
                1 => config.river_volume = 0.0,
                _ => config.waterfall_volume = 0.0,
            }
            assert_eq!(configured_energy(gains, &config), 0.0);
            gains[(kind + 1) % 3] = [0.2; 2];
            assert!(configured_energy(gains, &config) > 0.001);
        }
        let half = configured_energy(
            [[0.2; 2]; 3],
            &AudioConfig {
                master_volume: 0.5,
                ..default()
            },
        );
        assert!((half / audible - 0.25).abs() < 0.0001);
    }

    #[test]
    fn brook_has_less_high_frequency_energy_than_waterfall_roar() {
        let profile = |kind| {
            let mut noise = Noise::new(0x85ba310d);
            let mut square = 0.0;
            let mut high = 0.0;
            let mut previous = 0.0;
            for frame in 0..48_000 {
                let sample = noise.next(kind, frame as f32 / RATE);
                square += f64::from(sample * sample);
                high += f64::from((sample - previous).powi(2));
                previous = sample;
            }
            (square, high / square)
        };
        let brook = profile(1);
        let fall = profile(2);
        let still = profile(0);
        assert!(still.0 < brook.0 * 0.8);
        assert!(still.1 < fall.1 * 0.2);
        assert!(brook.0 > 1.0 && brook.0 < fall.0 * 0.5);
        assert!(brook.1 < fall.1 * 0.2);
    }
}
