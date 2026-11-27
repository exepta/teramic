use super::clips::{Bank, RAIN_CLIP, RAIN_DETAIL_CLIP};
#[path = "reverb.rs"]
mod reverb;
use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

pub const MAX_VOICES: usize = 24;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Category {
    Environment,
    Animals,
    Footsteps,
}
const CATEGORIES: usize = 3;
#[derive(Clone, Copy)]
pub struct Cue {
    pub category: Category,
    pub sound: usize,
    pub gains: [f32; 2],
    pub pitch: f32,
    pub reverb: f32,
}
pub struct Controls {
    pub queue: Mutex<VecDeque<Cue>>,
    pub played: AtomicU32,
    pub worm_played: [AtomicU32; super::worm::COUNT],
    pub ambient: [AtomicU32; super::ambience::LAYERS],
    pub fire: [AtomicU32; 2],
    pub furnace: [AtomicU32; 2],
    pub eruption: [AtomicU32; 2],
    pub lava: [AtomicU32; 2],
    pub faucet: [AtomicU32; 2],
    pub faucet_rms: AtomicU32,
    pub rain: [AtomicU32; 2],
    pub volume: AtomicU32,
    pub categories: [AtomicU32; CATEGORIES],
    pub submerged: AtomicU32,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            played: AtomicU32::new(0),
            worm_played: Default::default(),
            ambient: Default::default(),
            fire: Default::default(),
            furnace: Default::default(),
            eruption: Default::default(),
            lava: Default::default(),
            faucet: Default::default(),
            faucet_rms: AtomicU32::new(0),
            rain: Default::default(),
            volume: AtomicU32::new(0),
            categories: std::array::from_fn(|_| AtomicU32::new(1.0_f32.to_bits())),
            submerged: AtomicU32::new(0),
        }
    }
}
impl Controls {
    pub fn configure(&self, config: &terra_core::AudioConfig, active: bool) {
        self.volume.store(
            (if active && !config.muted {
                config.master_volume
            } else {
                0.0
            })
            .to_bits(),
            Ordering::Relaxed,
        );
        for (target, gain) in self.categories.iter().zip([
            config.ambience_volume,
            config.animals_volume,
            config.footsteps_volume,
        ]) {
            target.store(gain.to_bits(), Ordering::Relaxed);
        }
    }

    pub fn enqueue(&self, cue: Cue) {
        if let Ok(mut queue) = self.queue.lock()
            && queue.len() < MAX_VOICES
        {
            queue.push_back(cue);
        }
    }
}
#[derive(Asset, TypePath)]
pub struct RecordedSoundscape {
    pub bank: Bank,
    pub controls: Arc<Controls>,
}
impl Decodable for RecordedSoundscape {
    type Decoder = Stream;
    fn decoder(&self) -> Stream {
        Stream {
            bank: self.bank.clone(),
            controls: self.controls.clone(),
            voices: Vec::with_capacity(MAX_VOICES),
            frame: 0,
            right: false,
            pair: [0.0; 2],
            low: [0.0; 2],
            beds: super::beds::Beds::default(),
            fire: [0.0; 2],
            fire_phase: 0.0,
            eruption: [0.; 2],
            eruption_phase: 0.,
            lava: [0.; 2],
            lava_phase: 0.,
            furnace: [0.0; 2],
            furnace_phase: 0.0,
            faucet: [0.0; 2],
            faucet_phase: 6_170.0,
            faucet_energy: 0.0,
            rain_phase: [0.0, 3_700.0, 181_000.0],
            rain_gain: 0.0,
            rain_roof: 0.0,
            rain_low: [0.0; 2],
            volume: 0.0,
            categories: self
                .controls
                .categories
                .each_ref()
                .map(|gain| f32::from_bits(gain.load(Ordering::Relaxed))),
            wet: 0.0,
            reverb: std::array::from_fn(|_| reverb::Reverb::default()),
            atmosphere_reverb: reverb::Reverb::default(),
        }
    }
}
struct Voice {
    cue: Cue,
    position: f32,
}
pub struct Stream {
    bank: Bank,
    controls: Arc<Controls>,
    voices: Vec<Voice>,
    frame: u64,
    right: bool,
    pair: [f32; 2],
    low: [f32; 2],
    beds: super::beds::Beds,
    fire: [f32; 2],
    fire_phase: f32,
    furnace: [f32; 2],
    furnace_phase: f32,
    eruption: [f32; 2],
    eruption_phase: f32,
    lava: [f32; 2],
    lava_phase: f32,
    faucet: [f32; 2],
    faucet_phase: f32,
    faucet_energy: f32,
    rain_phase: [f32; 3],
    rain_gain: f32,
    rain_roof: f32,
    rain_low: [f32; 2],
    volume: f32,
    categories: [f32; CATEGORIES],
    wet: f32,
    reverb: [reverb::Reverb; CATEGORIES],
    atmosphere_reverb: reverb::Reverb,
}
fn sample(clip: &[f32], position: f32) -> f32 {
    let index = position as usize;
    let a = clip.get(index).copied().unwrap_or(0.0);
    let b = clip.get(index + 1).copied().unwrap_or(a);
    a + (b - a) * position.fract()
}
fn loop_sample(clip: &[f32], phase: &mut f32) -> f32 {
    let cross = (clip.len() / 4).min(7200) as f32;
    let remaining = clip.len() as f32 - *phase;
    let mut result = sample(clip, *phase);
    if remaining < cross {
        let mix = (1.0 - remaining / cross).clamp(0.0, 1.0);
        result = result * (1.0 - mix) + sample(clip, cross - remaining) * mix;
    }
    *phase += 0.5;
    if *phase >= clip.len() as f32 {
        *phase = cross;
    }
    result
}
impl Iterator for Stream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.right {
            self.right = false;
            return Some(self.pair[1]);
        }
        self.right = true;
        if self.frame.is_multiple_of(256) {
            self.beds.retarget(
                self.controls
                    .ambient
                    .each_ref()
                    .map(|gain| f32::from_bits(gain.load(Ordering::Relaxed))),
            );
            self.controls.faucet_rms.store(
                (self.faucet_energy / 512.0).sqrt().to_bits(),
                Ordering::Relaxed,
            );
            self.faucet_energy = 0.0;
        }
        if self.frame.is_multiple_of(256)
            // A stalled game thread must never stall the device callback. Pending
            // cues remain in the queue until the next 256-frame boundary.
            && let Ok(mut queue) = self.controls.queue.try_lock()
        {
            while self.voices.len() < MAX_VOICES {
                let Some(cue) = queue.pop_front() else { break };
                if cue.sound < self.bank.len()
                    && cue.reverb.is_finite()
                    && (0.0..=1.0).contains(&cue.reverb)
                    && cue.pitch.is_finite()
                    && (0.8..=1.2).contains(&cue.pitch)
                    && cue
                        .gains
                        .iter()
                        .all(|g| g.is_finite() && (0.0..=1.0).contains(g))
                {
                    self.voices.push(Voice { cue, position: 0.0 });
                    self.controls.played.fetch_add(1, Ordering::Relaxed);
                    if let Some(index) = cue
                        .sound
                        .checked_sub(super::worm::FIRST)
                        .filter(|&index| index < super::worm::BANK_LEN)
                        .map(|index| index % super::worm::COUNT)
                    {
                        self.controls.worm_played[index].fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        self.frame = self.frame.wrapping_add(1);
        self.pair = [0.0; 2];
        for (gain, target) in self.categories.iter_mut().zip(&self.controls.categories) {
            *gain += (f32::from_bits(target.load(Ordering::Relaxed)) - *gain) * 0.001;
        }
        let mut voices = [[0.0; 2]; CATEGORIES];
        let mut reverberant = [[0.0; 2]; CATEGORIES];
        for voice in &mut self.voices {
            let value = sample(&self.bank[voice.cue.sound], voice.position);
            let category = voice.cue.category as usize;
            for (channel, send) in reverberant[category].iter_mut().enumerate() {
                voices[category][channel] += value * voice.cue.gains[channel];
                *send += value * voice.cue.gains[channel] * voice.cue.reverb;
            }
            voice.position += 0.5 * voice.cue.pitch;
        }
        self.voices
            .retain(|v| (v.position as usize) < self.bank[v.cue.sound].len());
        // Keep category tails separate so live gain changes also affect existing reverb.
        for (category, input) in reverberant.into_iter().enumerate() {
            let echo = self.reverb[category].process(input);
            for (channel, wet) in echo.into_iter().enumerate() {
                self.pair[channel] += (voices[category][channel] + wet) * self.categories[category];
            }
        }
        let environment = self.categories[Category::Environment as usize];
        let (atmosphere, atmosphere_send) = self.beds.frame(&self.bank);
        for (channel, value) in atmosphere.into_iter().enumerate() {
            self.pair[channel] += value * environment;
        }
        // Regional beds share the same bounded environment tail as distant details.
        // Category gain remains outside the effect, so muting kills its tail too.
        let echo = self.atmosphere_reverb.process(atmosphere_send);
        for (channel, value) in echo.into_iter().enumerate() {
            self.pair[channel] += value * environment;
        }
        if let Some(clip) = self.bank.get(32) {
            let value = loop_sample(clip, &mut self.fire_phase);
            for channel in 0..2 {
                let target = f32::from_bits(self.controls.fire[channel].load(Ordering::Relaxed));
                self.fire[channel] += (target - self.fire[channel]) * 0.0008;
                self.pair[channel] += value * self.fire[channel] * environment;
            }
        }
        if let Some(clip) = self.bank.get(super::clips::FURNACE_CLIP) {
            let value = loop_sample(clip, &mut self.furnace_phase);
            for channel in 0..2 {
                let target = f32::from_bits(self.controls.furnace[channel].load(Ordering::Relaxed));
                self.furnace[channel] += (target - self.furnace[channel]) * 0.00015;
                self.pair[channel] += value * self.furnace[channel] * environment;
            }
        }
        if let Some(clip) = self.bank.get(super::volcanic::RUMBLE) {
            let value = loop_sample(clip, &mut self.eruption_phase);
            for channel in 0..2 {
                let target =
                    f32::from_bits(self.controls.eruption[channel].load(Ordering::Relaxed));
                self.eruption[channel] += (target - self.eruption[channel]) * 0.00015;
                self.pair[channel] += value * self.eruption[channel] * environment;
            }
        }
        if let Some(clip) = self.bank.get(super::volcanic::FLOW) {
            let value = loop_sample(clip, &mut self.lava_phase);
            for channel in 0..2 {
                let target = f32::from_bits(self.controls.lava[channel].load(Ordering::Relaxed));
                self.lava[channel] += (target - self.lava[channel]) * 0.00012;
                self.pair[channel] += value * self.lava[channel] * environment;
            }
        }
        // The existing close, detailed water recording forms one crossfaded loop.
        // All taps share this bounded stereo layer; changing sources only changes gains.
        if let Some(clip) = self.bank.get(RAIN_DETAIL_CLIP) {
            let value = loop_sample(clip, &mut self.faucet_phase);
            for channel in 0..2 {
                let target = f32::from_bits(self.controls.faucet[channel].load(Ordering::Relaxed));
                self.faucet[channel] += (target - self.faucet[channel]) * 0.0002;
                let sample = value * self.faucet[channel] * environment;
                self.pair[channel] += sample;
                self.faucet_energy += (sample * self.volume).powi(2);
            }
        }
        if let Some(clip) = self.bank.get(RAIN_CLIP) {
            let target = f32::from_bits(self.controls.rain[0].load(Ordering::Relaxed));
            let roof = f32::from_bits(self.controls.rain[1].load(Ordering::Relaxed));
            self.rain_gain += (target - self.rain_gain) * 0.000025;
            self.rain_roof += (roof - self.rain_roof) * 0.000025;
            let bed = loop_sample(clip, &mut self.rain_phase[0]);
            for channel in 0..2 {
                let detail = self.bank.get(RAIN_DETAIL_CLIP).map_or(bed, |clip| {
                    loop_sample(clip, &mut self.rain_phase[channel + 1])
                });
                let rain = bed * 0.72 + detail * 0.40;
                self.rain_low[channel] += (rain - self.rain_low[channel]) * 0.06;
                let filtered = rain + (self.rain_low[channel] - rain) * self.rain_roof;
                self.pair[channel] += filtered * self.rain_gain * environment;
            }
        }
        let volume = f32::from_bits(self.controls.volume.load(Ordering::Relaxed));
        self.volume += (volume - self.volume) * 0.001;
        let wet = self.controls.submerged.load(Ordering::Relaxed) as f32;
        self.wet += (wet - self.wet) * 0.00015;
        for channel in 0..2 {
            let dry = self.pair[channel] * self.volume;
            self.low[channel] += (dry - self.low[channel]) * 0.035;
            self.pair[channel] =
                (dry * (1.0 - self.wet) + self.low[channel] * self.wet * 0.45).clamp(-0.9, 0.9);
        }
        Some(self.pair[0])
    }
}
impl Source for Stream {
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
pub(super) mod tests {
    use super::*;

    #[test]
    fn nearby_molten_rock_has_a_stereo_bed_between_discrete_breaches() {
        let controls = Arc::new(Controls::default());
        controls.configure(&terra_core::AudioConfig::default(), true);
        for (channel, gain) in controls.lava.iter().zip([0.28_f32, 0.12]) {
            channel.store(gain.to_bits(), Ordering::Relaxed);
        }
        let mut stream = RecordedSoundscape {
            bank: super::super::clips::load().unwrap(),
            controls: controls.clone(),
        }
        .decoder();
        let mut energies = [0.; 2];
        for frame in 0..24_000 {
            for energy in &mut energies {
                let value = stream.next().unwrap();
                assert!(value.is_finite() && value.abs() < 0.9);
                if frame >= 12_000 {
                    *energy += value * value;
                }
            }
        }
        assert!(
            energies[1] > 0.01 && energies[0] > 2. * energies[1],
            "{energies:?}"
        );
        for target in &controls.lava {
            target.store(0, Ordering::Relaxed);
        }
        let tail: Vec<_> = stream.skip(192_000).take(256).collect();
        assert!(tail.iter().all(|sample| sample.abs() < 0.0001));
    }

    #[test]
    fn actual_biome_beds_mix_in_stereo_crossfade_and_respect_live_mute() {
        let bank = super::super::clips::load().unwrap();
        let export = std::env::var_os("TERRA_AUDIO_EXPORT_DIR").map(std::path::PathBuf::from);
        let mut signatures = std::collections::BTreeSet::new();
        for (name, biome, cave) in [
            ("blood-forest", 9, 0.0),
            ("spirit-forest", 8, 0.0),
            ("desert", 3, 0.0),
            ("ocean", 0, 0.0),
            ("highlands", 4, 0.0),
            ("woodland", 2, 0.0),
            ("cherry-forest", 7, 0.0),
            ("mushroom-park", 5, 0.0),
            ("caves", 2, 1.0),
        ] {
            let controls = Arc::new(Controls::default());
            controls.configure(&terra_core::AudioConfig::default(), true);
            let mut weights = [0.0; 16];
            weights[biome] = 1.0;
            let p = super::super::ambience::profile(weights, cave, 0.5, 1.5, 0.0);
            for (gain, target) in p.levels.into_iter().zip(&controls.ambient) {
                target.store(gain.to_bits(), Ordering::Relaxed);
            }
            let mut cadence = super::super::ambience::Cadence::default();
            cadence.update(0.0, p, &controls);
            cadence.update(30.0, p, &controls);
            let mut stream = RecordedSoundscape {
                bank: bank.clone(),
                controls: controls.clone(),
            }
            .decoder();
            let samples: Vec<_> = stream.by_ref().take(48_000 * 2 * 8).collect();
            assert!(samples.iter().all(|s| s.is_finite() && s.abs() < 0.89));
            let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
            assert!(rms > 0.00005 && rms < 0.2, "{name}: {rms}");
            assert!(
                samples
                    .chunks_exact(2)
                    .any(|s| (s[0] - s[1]).abs() > 0.0001),
                "{name}: stereo"
            );
            assert!(signatures.insert(rms.to_bits()), "{name}: duplicated bed");
            // Re-select another region while using the same decoder. Smoothed
            // controls avoid a hard jump or another audio device/entity.
            for gain in &controls.ambient {
                gain.store(0, Ordering::Relaxed);
            }
            controls.ambient[2].store(0.13_f32.to_bits(), Ordering::Relaxed);
            let before = stream.next().unwrap();
            let after = stream.next().unwrap();
            assert!((after - before).abs() < 0.10);
            controls.configure(
                &terra_core::AudioConfig {
                    ambience_volume: 0.0,
                    ..Default::default()
                },
                true,
            );
            for _ in 0..48_000 {
                stream.next();
            }
            assert!(
                stream.by_ref().take(12_000).all(|s| s.abs() < 0.00001),
                "{name}: mute tail"
            );
            if let Some(root) = &export {
                export_stereo(root, name, &samples);
            }
        }
    }

    #[test]
    fn continuous_biome_journey_uses_one_decoder_and_preserves_live_volume_controls() {
        let controls = Arc::new(Controls::default());
        controls.configure(&terra_core::AudioConfig::default(), true);
        let mut stream = RecordedSoundscape {
            bank: super::super::clips::load().unwrap(),
            controls: controls.clone(),
        }
        .decoder();
        let mut samples = Vec::new();
        let mut cadence = super::super::ambience::Cadence::default();
        for (region, biome) in [2, 7, 8, 5, 9, 3, 0].into_iter().enumerate() {
            let mut weights = [0.0; 16];
            weights[biome] = 1.0;
            let profile = super::super::ambience::profile(weights, 0.0, 0.5, 2.0, 0.0);
            for (target, gain) in controls.ambient.iter().zip(profile.levels) {
                target.store(gain.to_bits(), Ordering::Relaxed);
            }
            for second in 0..6 {
                cadence.update((region * 6 + second) as f32, profile, &controls);
                samples.extend(stream.by_ref().take(48_000 * 2));
            }
        }
        assert!(
            samples
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() < 0.89)
        );
        // Check actual consecutive frames of each channel, including each abrupt
        // region replacement, rather than comparing left against right samples.
        assert!(
            samples
                .windows(3)
                .all(|pair| (pair[2] - pair[0]).abs() < 0.15)
        );
        controls.configure(
            &terra_core::AudioConfig {
                muted: true,
                ..default()
            },
            true,
        );
        stream.by_ref().take(48_000).for_each(drop);
        assert!(stream.take(12_000).all(|sample| sample.abs() < 0.00001));
        if let Some(root) = std::env::var_os("TERRA_AUDIO_EXPORT_DIR") {
            export_stereo(&std::path::PathBuf::from(root), "biome-journey", &samples);
        }
    }

    pub(in crate::creature_audio) fn export_stereo(
        path: &std::path::Path,
        name: &str,
        samples: &[f32],
    ) {
        std::fs::create_dir_all(path).unwrap();
        let size = (samples.len() * 2) as u32;
        let mut wav = Vec::with_capacity(size as usize + 44);
        wav.extend(b"RIFF");
        wav.extend((size + 36).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16_u32.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(2_u16.to_le_bytes());
        wav.extend(48_000_u32.to_le_bytes());
        wav.extend(192_000_u32.to_le_bytes());
        wav.extend(4_u16.to_le_bytes());
        wav.extend(16_u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(size.to_le_bytes());
        for sample in samples {
            wav.extend(((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(path.join(format!("{name}.wav")), wav).unwrap();
    }

    fn queued_constant_voice(
        category: Category,
        config: &terra_core::AudioConfig,
    ) -> (Arc<Controls>, Stream) {
        let controls = Arc::new(Controls::default());
        controls.configure(config, true);
        controls.enqueue(Cue {
            category,
            sound: 0,
            gains: [1.0; 2],
            pitch: 1.0,
            reverb: 0.0,
        });
        let mut clips = vec![vec![0.0; 24_000]; 14];
        clips[0] = vec![0.1; 240_000];
        let stream = RecordedSoundscape {
            bank: Arc::new(clips),
            controls: controls.clone(),
        }
        .decoder();
        (controls, stream)
    }

    #[test]
    fn contended_cue_queue_defers_playback_and_keeps_the_audio_clock_running() {
        let (controls, mut stream) =
            queued_constant_voice(Category::Environment, &Default::default());
        {
            let queue = controls.queue.lock().unwrap();
            for _ in 0..512 {
                assert!(stream.next().is_some());
            }
            assert_eq!(queue.len(), 1);
            assert_eq!(controls.played.load(Ordering::Relaxed), 0);
        }
        stream.next();
        assert_eq!(controls.played.load(Ordering::Relaxed), 1);
    }

    fn constant_voice(
        category: Category,
        config: &terra_core::AudioConfig,
    ) -> (Arc<Controls>, Stream) {
        let (controls, mut stream) = queued_constant_voice(category, config);
        for _ in 0..32_000 {
            stream.next();
        }
        (controls, stream)
    }

    #[test]
    fn previously_muted_categories_are_silent_from_the_first_immediate_cue_sample() {
        for muted in [
            Category::Environment,
            Category::Animals,
            Category::Footsteps,
        ] {
            let mut config = terra_core::AudioConfig::default();
            match muted {
                Category::Environment => config.ambience_volume = 0.0,
                Category::Animals => config.animals_volume = 0.0,
                Category::Footsteps => config.footsteps_volume = 0.0,
            }
            for category in [
                Category::Environment,
                Category::Animals,
                Category::Footsteps,
            ] {
                let (controls, mut stream) = queued_constant_voice(category, &config);
                let samples: Vec<_> = stream.by_ref().take(2048).collect();
                assert_eq!(controls.played.load(Ordering::Relaxed), 1);
                if category == muted {
                    assert!(samples.iter().all(|sample| *sample == 0.0), "{category:?}");
                } else {
                    assert!(samples[0] > 0.0);
                    assert!(samples.last().copied().unwrap() > 0.05, "{category:?}");
                }
            }
        }
    }

    fn energy(stream: &mut Stream) -> f32 {
        (0..2048)
            .map(|_| stream.next().unwrap().powi(2))
            .sum::<f32>()
            / 2048.0
    }

    #[test]
    fn category_gains_reach_decoder_without_affecting_other_sources() {
        let config = terra_core::AudioConfig {
            master_volume: 0.5,
            ambience_volume: 0.6,
            animals_volume: 0.4,
            footsteps_volume: 0.2,
            water_volume: 0.0,
            river_volume: 0.0,
            waterfall_volume: 0.0,
            ..default()
        };
        for (category, gain) in [
            (Category::Environment, 0.6),
            (Category::Animals, 0.4),
            (Category::Footsteps, 0.2),
        ] {
            let (_, mut stream) = constant_voice(category, &config);
            let expected = (0.1_f32 * 0.5 * gain).powi(2);
            assert!((energy(&mut stream) - expected).abs() < 0.000001);
        }
        for (muted, config) in [
            (
                Category::Animals,
                terra_core::AudioConfig {
                    animals_volume: 0.0,
                    ..default()
                },
            ),
            (
                Category::Footsteps,
                terra_core::AudioConfig {
                    footsteps_volume: 0.0,
                    ..default()
                },
            ),
            (
                Category::Environment,
                terra_core::AudioConfig {
                    ambience_volume: 0.0,
                    ..default()
                },
            ),
        ] {
            for category in [
                Category::Environment,
                Category::Animals,
                Category::Footsteps,
            ] {
                let (_, mut stream) = constant_voice(category, &config);
                if category == muted {
                    assert!(energy(&mut stream) < 0.000000001);
                } else {
                    assert!(energy(&mut stream) > 0.009);
                }
            }
        }
    }

    #[test]
    fn existing_voices_follow_live_category_changes_global_mute_and_revert_without_restarting() {
        for category in [
            Category::Environment,
            Category::Animals,
            Category::Footsteps,
        ] {
            let (controls, mut stream) =
                constant_voice(category, &terra_core::AudioConfig::default());
            let audible = energy(&mut stream);
            let mut config = terra_core::AudioConfig::default();
            match category {
                Category::Environment => config.ambience_volume = 0.0,
                Category::Animals => config.animals_volume = 0.0,
                Category::Footsteps => config.footsteps_volume = 0.0,
            }
            controls.configure(&config, true);
            for _ in 0..32_000 {
                stream.next();
            }
            assert!(energy(&mut stream) < 0.000000001);
            controls.configure(&terra_core::AudioConfig::default(), true);
            for _ in 0..32_000 {
                stream.next();
            }
            assert!((energy(&mut stream) / audible - 1.0).abs() < 0.0001);
            controls.configure(
                &terra_core::AudioConfig {
                    muted: true,
                    ..default()
                },
                true,
            );
            for _ in 0..32_000 {
                stream.next();
            }
            assert!(energy(&mut stream) < 0.000000001);
            assert_eq!(stream.voices.len(), 1);
            assert_eq!(controls.played.load(Ordering::Relaxed), 1);
            assert!(controls.queue.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn category_changes_also_mute_existing_reverb_tails() {
        let config = terra_core::AudioConfig {
            animals_volume: 0.0,
            footsteps_volume: 0.0,
            ..default()
        };
        let controls = Arc::new(Controls::default());
        controls.configure(&config, true);
        let mut stream = RecordedSoundscape {
            bank: Arc::new(vec![vec![0.0; 24_000]; 14]),
            controls,
        }
        .decoder();
        stream.reverb[Category::Animals as usize].process([1.0; 2]);
        stream.reverb[Category::Footsteps as usize].process([1.0; 2]);
        for _ in 0..32_000 {
            stream.next();
        }
        assert!(energy(&mut stream) < 0.000000001);
        stream.reverb[Category::Environment as usize].process([1.0; 2]);
        let tail = stream
            .by_ref()
            .take(16_000)
            .map(|sample| sample * sample)
            .sum::<f32>();
        assert!(
            tail > 0.001,
            "muted animal/step tails do not mute environment reverb"
        );
    }

    #[test]
    fn recorded_rain_has_stereo_detail_without_creating_cues_or_clipping() {
        let controls = Arc::new(Controls::default());
        controls.volume.store(1.0_f32.to_bits(), Ordering::Relaxed);
        controls.rain[0].store(0.52_f32.to_bits(), Ordering::Relaxed);
        let mut clips = vec![vec![0.0; 24_000]; RAIN_DETAIL_CLIP + 1];
        for (i, sample) in clips[RAIN_CLIP].iter_mut().enumerate() {
            *sample = (i as f32 * 0.051).sin() * 0.12;
        }
        for (i, sample) in clips[RAIN_DETAIL_CLIP].iter_mut().enumerate() {
            *sample = (i as f32 * 0.073).sin() * 0.16;
        }
        let mut stream = RecordedSoundscape {
            bank: Arc::new(clips),
            controls,
        }
        .decoder();
        let mut stereo_difference = 0.0;
        let mut energy = 0.0;
        for _ in 0..48_000 {
            let left = stream.next().unwrap();
            let right = stream.next().unwrap();
            assert!(left.abs().max(right.abs()) < 0.25);
            stereo_difference += (left - right).abs();
            energy += left * left + right * right;
        }
        assert!(energy > 1.0);
        assert!(stereo_difference > 1.0);
        assert!(stream.voices.is_empty());
    }
    #[test]
    fn positional_fire_stops_after_the_last_placed_source_disappears() {
        let controls = Arc::new(Controls::default());
        controls.volume.store(1.0_f32.to_bits(), Ordering::Relaxed);
        for gain in &controls.fire {
            gain.store(0.2_f32.to_bits(), Ordering::Relaxed);
        }
        let mut stream = RecordedSoundscape {
            bank: Arc::new(vec![vec![0.1; 24_000]; 33]),
            controls: controls.clone(),
        }
        .decoder();
        for _ in 0..24_000 {
            stream.next();
        }
        assert!(stream.next().unwrap() > 0.01);
        for gain in &controls.fire {
            gain.store(0, Ordering::Relaxed);
        }
        for _ in 0..24_000 {
            stream.next();
        }
        assert!(stream.next().unwrap().abs() < 0.00001);
        assert!(stream.voices.is_empty());
    }
    #[test]
    fn faucet_loop_reaches_existing_stereo_decoder_and_fades_after_shutoff_or_mute() {
        let controls = Arc::new(Controls::default());
        controls.configure(&terra_core::AudioConfig::default(), true);
        let mut clips = vec![vec![0.0; 24_000]; RAIN_DETAIL_CLIP + 1];
        clips[RAIN_DETAIL_CLIP] = (0..24_000)
            .map(|index| (index as f32 * 0.037).sin() * 0.2)
            .collect();
        let mut stream = RecordedSoundscape {
            bank: Arc::new(clips),
            controls: controls.clone(),
        }
        .decoder();
        let energy = |stream: &mut Stream| {
            stream
                .take(24_000)
                .map(|sample| f64::from(sample).powi(2))
                .sum::<f64>()
        };
        assert_eq!(energy(&mut stream), 0.0);
        controls.faucet[0].store(0.1_f32.to_bits(), Ordering::Relaxed);
        controls.faucet[1].store(0.3_f32.to_bits(), Ordering::Relaxed);
        assert!(energy(&mut stream) > 0.05);
        assert!(stream.faucet[1] > stream.faucet[0]);
        assert!(f32::from_bits(controls.faucet_rms.load(Ordering::Relaxed)) > 0.00001);
        for gain in &controls.faucet {
            gain.store(0, Ordering::Relaxed);
        }
        for _ in 0..192_000 {
            stream.next();
        }
        assert!(energy(&mut stream) < 1e-10);
        for gain in &controls.faucet {
            gain.store(0.3_f32.to_bits(), Ordering::Relaxed);
        }
        assert!(energy(&mut stream) > 0.05);
        controls.configure(
            &terra_core::AudioConfig {
                muted: true,
                ..Default::default()
            },
            true,
        );
        for _ in 0..48_000 {
            stream.next();
        }
        assert!(energy(&mut stream) < 1e-10);
    }

    #[test]
    fn recorded_mix_is_bounded_filtered_and_live_muting_clears_existing_voices() {
        let controls = Arc::new(Controls::default());
        controls.volume.store(1.0_f32.to_bits(), Ordering::Relaxed);
        for _ in 0..64 {
            controls.enqueue(Cue {
                category: Category::Animals,
                sound: 0,
                gains: [1.0; 2],
                pitch: 1.0,
                reverb: 0.0,
            });
        }
        assert_eq!(controls.queue.lock().unwrap().len(), MAX_VOICES);
        let bank = Arc::new(vec![vec![0.1; 24_000]; 14]);
        let mut stream = RecordedSoundscape {
            bank,
            controls: controls.clone(),
        }
        .decoder();
        for _ in 0..24_000 {
            assert!(stream.next().unwrap().abs() <= 0.9);
        }
        assert_eq!(stream.voices.len(), MAX_VOICES);
        controls.volume.store(0, Ordering::Relaxed);
        controls.submerged.store(1, Ordering::Relaxed);
        for _ in 0..24_000 {
            stream.next();
        }
        assert!(stream.next().unwrap().abs() < 0.00001);
    }
}
