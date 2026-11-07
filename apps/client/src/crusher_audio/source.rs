use bevy::{
    audio::{AudioSource, ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

const RATE: f32 = 48_000.0;
const SMOOTH: f32 = 1.0 / (RATE * 0.075);
const RELEASE: f32 = 0.5 / (RATE * 0.075);
const MAX_VOICES: usize = 12;
const PATHS: [&str; 8] = [
    "footsteps/detail/wood-3.wav",
    "footsteps/detail/wood-4.wav",
    "footsteps/detail/wood-5.wav",
    "footsteps/detail/stone-3.wav",
    "footsteps/detail/stone-5.wav",
    "footsteps/detail/gravel-2.wav",
    "footsteps/detail/gravel-4.wav",
    "footsteps/detail/gravel-7.wav",
];
pub(super) type FoleyBank = Arc<Vec<Vec<f32>>>;

/// Decode eight existing material recordings once, outside the frame/audio
/// threads. Only this bounded subset is retained, not a full second copy of
/// the creature/environment bank.
pub(super) fn load_foley() -> Result<FoleyBank, String> {
    let root = terra_core::asset_directory().join("audio");
    let mut bank = Vec::with_capacity(PATHS.len());
    for name in PATHS {
        let data = std::fs::read(root.join(name)).map_err(|error| format!("{name}: {error}"))?;
        if data.len() > 1_000_000 {
            return Err(format!("Oversized crusher Foley: {name}"));
        }
        let audio = AudioSource { bytes: data.into() };
        let decoder = <AudioSource as Decodable>::Decoder::try_from(Cursor::new(audio))
            .map_err(|error| format!("Invalid crusher Foley {name}: {error}"))?;
        if decoder.channels().get() != 1 || decoder.sample_rate().get() != 24_000 {
            return Err(format!("Crusher Foley must be mono 24000 Hz: {name}"));
        }
        let clip: Vec<f32> = decoder.take(24_001).collect();
        if clip.len() < 240
            || clip.len() > 24_000
            || clip
                .iter()
                .any(|sample| !sample.is_finite() || sample.abs() > 1.0)
        {
            return Err(format!("Invalid crusher Foley samples: {name}"));
        }
        bank.push(clip);
    }
    Ok(Arc::new(bank))
}

#[derive(Default)]
pub(super) struct MixControls {
    gains: [[AtomicU32; 2]; 2],
    rms: AtomicU32,
    voices: AtomicU32,
    rate: AtomicU32,
}

impl MixControls {
    pub(super) fn update_rate(&self, rate: f32) {
        let rate = if rate.is_finite() {
            rate.clamp(0.25, 4.0)
        } else {
            1.0
        };
        self.rate.store(rate.to_bits(), Ordering::Relaxed);
    }
    pub(super) fn update(&self, gains: [[f32; 2]; 2]) {
        for (voice, values) in self.gains.iter().zip(gains) {
            for (channel, gain) in voice.iter().zip(values) {
                channel.store(gain.clamp(0.0, 0.5).to_bits(), Ordering::Relaxed);
            }
        }
    }
    pub(super) fn rms(&self) -> f32 {
        f32::from_bits(self.rms.load(Ordering::Relaxed))
    }
    pub(super) fn voices(&self) -> usize {
        self.voices.load(Ordering::Relaxed) as usize
    }
}

#[derive(Asset, TypePath)]
pub(super) struct MachineSource {
    pub(super) controls: Arc<MixControls>,
    pub(super) bank: FoleyBank,
}

impl Decodable for MachineSource {
    type Decoder = MachineStream;
    fn decoder(&self) -> Self::Decoder {
        MachineStream {
            controls: self.controls.clone(),
            bank: self.bank.clone(),
            gains: [[0.0; 2]; 2],
            targets: [[0.0; 2]; 2],
            seed: 0x7c18_a35f,
            bass: 0.0,
            grit: 0.0,
            voices: [None; MAX_VOICES],
            next_event: [0, 240, 1_100, 480],
            metal: [0.0; 2],
            phase: [0.0; 2],
            frequency: [270.0, 645.0],
            stereo: [0.0; 2],
            right: false,
            frame: 0,
            energy: 0.0,
            clock_rate: 1.0,
            #[cfg(test)]
            events: [0; 3],
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    clip: usize,
    kind: usize,
    position: f32,
    step: f32,
    gain: f32,
    remaining: u32,
    length: u32,
    low: f32,
    bass: f32,
}

pub(super) struct MachineStream {
    controls: Arc<MixControls>,
    bank: FoleyBank,
    gains: [[f32; 2]; 2],
    targets: [[f32; 2]; 2],
    seed: u32,
    bass: f32,
    grit: f32,
    voices: [Option<Voice>; MAX_VOICES],
    next_event: [u32; 4],
    metal: [f32; 2],
    phase: [f32; 2],
    frequency: [f32; 2],
    stereo: [f32; 2],
    right: bool,
    frame: u32,
    energy: f32,
    clock_rate: f32,
    #[cfg(test)]
    events: [usize; 3],
}

impl MachineStream {
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (f64::from(self.seed) / f64::from(u32::MAX)) as f32
    }

    fn spawn(&mut self, family: usize) {
        let (first, count, seconds, level) = match family {
            0 => (0, 3, 0.34, 0.30),
            1 => (3, 2, 0.28, 0.43),
            _ => (5, 3, 0.14, 0.28),
        };
        let clip = first + (self.random() * count as f32) as usize % count;
        let Some(samples) = self.bank.get(clip) else {
            return;
        };
        let count = samples.len();
        let position = if family == 2 {
            self.random() * count as f32 * 0.18
        } else {
            0.0
        };
        let pitch = 0.76 + self.random() * 0.40;
        let step = 0.5 * pitch * self.clock_rate.sqrt().clamp(0.6, 2.0);
        let length = ((count as f32 - position) / step).min(RATE * seconds) as u32;
        let gain = level * (0.78 + self.random() * 0.38);
        let Some(index) = self.voices.iter().position(Option::is_none) else {
            return;
        };
        self.voices[index] = Some(Voice {
            clip,
            kind: usize::from(family != 0),
            position,
            step,
            gain,
            remaining: length,
            length,
            low: 0.0,
            bass: 0.0,
        });
        #[cfg(test)]
        {
            self.events[family] += 1;
        }
    }

    fn material_foley(&mut self) -> [f32; 2] {
        let mut result = [0.0; 2];
        for slot in &mut self.voices {
            let Some(voice) = slot else {
                continue;
            };
            let Some(clip) = self.bank.get(voice.clip) else {
                *slot = None;
                continue;
            };
            let index = voice.position as usize;
            if voice.remaining == 0 || index + 1 >= clip.len() {
                *slot = None;
                continue;
            }
            let sample = clip[index] + (clip[index + 1] - clip[index]) * voice.position.fract();
            voice.low += (sample - voice.low) * if voice.kind == 0 { 0.20 } else { 0.36 };
            voice.bass += (voice.low - voice.bass) * 0.012;
            let attack = ((voice.length - voice.remaining) as f32 / (RATE * 0.006)).min(1.0);
            let release = (voice.remaining as f32 / (RATE * 0.022)).min(1.0);
            result[voice.kind] += (voice.low - voice.bass * 0.68) * voice.gain * attack * release;
            voice.position += voice.step;
            voice.remaining -= 1;
        }
        result
    }

    fn frame(&mut self) {
        if self.frame.is_multiple_of(256) {
            let rate = f32::from_bits(self.controls.rate.load(Ordering::Relaxed));
            self.clock_rate = if rate.is_finite() && rate > 0.0 {
                rate.clamp(0.25, 4.0)
            } else {
                1.0
            };
            for (target, voice) in self.targets.iter_mut().zip(&self.controls.gains) {
                for (gain, control) in target.iter_mut().zip(voice) {
                    *gain = f32::from_bits(control.load(Ordering::Relaxed));
                }
            }
            self.controls
                .rms
                .store((self.energy / 512.0).sqrt().to_bits(), Ordering::Relaxed);
            self.controls.voices.store(
                self.voices.iter().filter(|voice| voice.is_some()).count() as u32,
                Ordering::Relaxed,
            );
            self.energy = 0.0;
        }
        self.frame = self.frame.wrapping_add(1);
        if self.targets.iter().flatten().all(|gain| *gain == 0.0)
            && self.gains.iter().flatten().all(|gain| *gain == 0.0)
        {
            self.voices.fill(None);
            self.metal = [0.0; 2];
            self.bass = 0.0;
            self.grit = 0.0;
            self.stereo = [0.0; 2];
            self.next_event = [0, 240, 1_100, 480];
            return;
        }
        let noise = self.random() * 2.0 - 1.0;
        self.bass += 0.009 * (noise - self.bass);
        self.grit += 0.13 * (noise - self.grit);
        for family in 0..4 {
            let kind = usize::from(family == 1 || family == 2);
            self.next_event[family] = self.next_event[family].saturating_sub(1);
            if self.next_event[family] == 0 && self.targets[kind].iter().any(|gain| *gain > 0.0) {
                let (minimum, spread) = match family {
                    0 => (0.28, 0.34),
                    1 => (0.10, 0.12),
                    2 => (0.038, 0.071),
                    _ => (0.12, 0.10),
                };
                self.next_event[family] =
                    (RATE * (minimum + self.random() * spread) / self.clock_rate) as u32;
                if family < 3 {
                    self.spawn(family);
                } else {
                    self.metal = [0.4 + self.random() * 0.5, 0.25 + self.random() * 0.4];
                    self.frequency = [220.0 + self.random() * 110.0, 540.0 + self.random() * 230.0];
                }
            }
        }
        let mut voices = self.material_foley();
        // The forged teeth add brief, damped contact modes, not the old steady
        // tonal buzz. Randomized real wood/stone/gravel transients carry the mix.
        for mode in 0..2 {
            if self.metal[mode] < 0.000_01 {
                self.metal[mode] = 0.0;
                continue;
            }
            self.phase[mode] =
                (self.phase[mode] + self.frequency[mode] * self.clock_rate / RATE).fract();
            voices[0] += (self.phase[mode] * std::f32::consts::TAU).sin()
                * self.metal[mode]
                * if mode == 0 { 0.025 } else { 0.009 };
            self.metal[mode] *= if mode == 0 { 0.999_35 } else { 0.999_0 };
        }
        voices[0] += self.bass * 0.12;
        voices[1] += self.bass * 0.13 + self.grit * 0.028;
        self.stereo = [0.0; 2];
        for (kind, voice) in voices.into_iter().enumerate() {
            for channel in 0..2 {
                self.gains[kind][channel] = if self.targets[kind][channel] == 0.0 {
                    (self.gains[kind][channel] - RELEASE).max(0.0)
                } else {
                    self.gains[kind][channel]
                        + (self.targets[kind][channel] - self.gains[kind][channel]) * SMOOTH
                };
                self.stereo[channel] += voice * self.gains[kind][channel];
            }
        }
        for sample in &mut self.stereo {
            *sample = sample.clamp(-0.8, 0.8);
            self.energy += *sample * *sample;
        }
    }
}

impl Iterator for MachineStream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if !self.right {
            self.frame();
        }
        let sample = self.stereo[usize::from(self.right)];
        self.right = !self.right;
        Some(sample)
    }
}

impl Source for MachineStream {
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
    #[test]
    fn faster_gears_increase_contact_cadence_without_unbounded_voices_or_nonfinite_samples() {
        let bank = load_foley().unwrap();
        let events = |rate| {
            let controls = Arc::new(MixControls::default());
            controls.update([[0.35; 2], [0.0; 2]]);
            controls.update_rate(rate);
            let mut stream = MachineSource {
                controls,
                bank: bank.clone(),
            }
            .decoder();
            for sample in stream.by_ref().take(192_000) {
                assert!(sample.is_finite() && sample.abs() <= 0.5);
            }
            assert!(stream.voices.iter().flatten().count() <= MAX_VOICES);
            stream.events[0]
        };
        assert!(events(2.0) > events(0.5));
    }
    #[test]
    fn shipped_material_foley_is_real_bounded_and_covers_wood_stone_and_grit() {
        let bank = load_foley().unwrap();
        assert_eq!(bank.len(), 8);
        assert!(bank.iter().map(|clip| clip.len() * 4).sum::<usize>() < 800_000);
        for clip in bank.iter() {
            assert!(clip.iter().map(|sample| sample * sample).sum::<f32>() > 0.001);
        }
    }
    #[test]
    fn mechanical_mix_has_soft_boundaries_finite_energy_and_fades_to_silence() {
        let controls = Arc::new(MixControls::default());
        controls.update([[0.35; 2]; 2]);
        let mut stream = MachineSource {
            controls: controls.clone(),
            bank: load_foley().unwrap(),
        }
        .decoder();
        let samples: Vec<_> = stream.by_ref().take(192_000).collect();
        assert!(samples[0].abs() < 0.001);
        let energy =
            samples.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / samples.len() as f64;
        assert!(
            energy > 0.000_005 && energy < 0.02,
            "unexpected crusher energy {energy}"
        );
        assert!(samples.iter().all(|v| v.is_finite() && v.abs() < 0.5));
        assert!(samples.windows(2).all(|s| (s[1] - s[0]).abs() < 0.16));
        assert!(stream.events.iter().all(|events| *events >= 3));
        assert!(stream.voices.iter().flatten().count() <= MAX_VOICES);
        assert!(controls.rms() > 0.000_1);
        controls.update([[0.0; 2]; 2]);
        for _ in 0..8_640 {
            stream.next();
        }
        assert!(stream.by_ref().take(4_000).all(|v| v == 0.0));
        assert_eq!(controls.rms(), 0.0);
        controls.update([[0.35; 2]; 2]);
        assert!(stream.take(24_000).any(|v| v.abs() > 0.001));
    }
    #[test]
    fn empty_cranking_never_schedules_mineral_crunches_and_recording_timing_is_not_a_loop() {
        let controls = Arc::new(MixControls::default());
        controls.update([[0.35; 2], [0.0; 2]]);
        let mut stream = MachineSource {
            controls,
            bank: load_foley().unwrap(),
        }
        .decoder();
        let samples: Vec<_> = stream.by_ref().take(192_000).collect();
        assert!(stream.events[0] >= 3);
        assert_eq!(&stream.events[1..], &[0, 0]);
        let first = &samples[48_000..72_000];
        let second = &samples[96_000..120_000];
        let dot = first
            .iter()
            .zip(second)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum::<f64>();
        let energy = |values: &[f32]| values.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
        assert!(dot.abs() / (energy(first) * energy(second)).sqrt() < 0.95);
    }

    #[test]
    fn idle_stream_skips_dsp_clears_voices_and_wakes_across_frame_counter_wrap() {
        let controls = Arc::new(MixControls::default());
        controls.update([[0.35; 2]; 2]);
        let mut stream = MachineSource {
            controls: controls.clone(),
            bank: load_foley().unwrap(),
        }
        .decoder();
        for _ in 0..96_000 {
            stream.next();
        }
        assert!(stream.events.iter().all(|count| *count > 0));
        controls.update([[0.0; 2]; 2]);
        for _ in 0..19_200 {
            stream.next();
        }
        assert_eq!(stream.metal, [0.0; 2]);
        assert!(stream.voices.iter().all(Option::is_none));
        assert_eq!((stream.bass, stream.grit), (0.0, 0.0));
        let seed = stream.seed;
        stream.frame = u32::MAX - 10;
        assert!(stream.by_ref().take(8_192).all(|sample| sample == 0.0));
        assert_eq!(stream.seed, seed, "silent frames must not run the RNG/DSP");
        assert_eq!(controls.rms(), 0.0);
        assert_eq!(controls.voices(), 0);
        let events = stream.events[0];
        controls.update([[0.35; 2], [0.0; 2]]);
        assert!(
            stream
                .by_ref()
                .take(24_000)
                .any(|sample| sample.abs() > 0.001)
        );
        assert!(stream.events[0] > events);
    }

    #[test]
    #[ignore = "writes the explicit six-second manual/crushing/mute listening fixture"]
    fn write_crusher_listening_preview() {
        let controls = Arc::new(MixControls::default());
        let mut stream = MachineSource {
            controls: controls.clone(),
            bank: load_foley().unwrap(),
        }
        .decoder();
        let mut samples = Vec::with_capacity(48_000 * 2 * 6);
        for (frames, gains) in [
            (48_000 * 2, [[0.35, 0.28], [0.0; 2]]),
            (48_000 * 3, [[0.35, 0.28]; 2]),
            (48_000, [[0.0; 2]; 2]),
        ] {
            controls.update(gains);
            samples.extend(stream.by_ref().take(frames * 2));
        }
        let data_size = (samples.len() * 2) as u32;
        let mut wav = Vec::with_capacity(data_size as usize + 44);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&48_000_u32.to_le_bytes());
        wav.extend_from_slice(&192_000_u32.to_le_bytes());
        wav.extend_from_slice(&4_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(
                &((sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16).to_le_bytes(),
            );
        }
        let decoded = <AudioSource as Decodable>::Decoder::try_from(Cursor::new(AudioSource {
            bytes: wav.clone().into(),
        }))
        .unwrap();
        assert_eq!(decoded.channels().get(), 2);
        assert_eq!(decoded.sample_rate().get(), 48_000);
        let output = std::env::var_os("TERRA_CRUSHER_AUDIO_PREVIEW")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/phase108/crusher-mix.wav")
            });
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(&output, wav).unwrap();
        println!(
            "Crusher audio preview: {} (0-2s empty crank, 2-5s mineral crushing, 5-6s mute)",
            output.display()
        );
    }
}
