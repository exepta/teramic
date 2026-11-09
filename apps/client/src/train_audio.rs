//! Four nearby trains share one stereo mix with speed-driven exhaust and rail rhythm.
use bevy::{
    audio::{AddAudioSource, AudioSource, ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use terra_network::client::ClientSession;
const RATE: f32 = 24_000.0;
const VOICES: usize = 4;
#[derive(Default)]
struct Controls {
    gains: [[[AtomicU32; 2]; 3]; VOICES],
    speeds: [AtomicU32; VOICES],
}
type Bank = Arc<[Vec<f32>; 3]>;
#[derive(Asset, TypePath)]
struct TrainSource {
    controls: Arc<Controls>,
    bank: Bank,
}
struct Stream {
    controls: Arc<Controls>,
    bank: Bank,
    gains: [[[f32; 2]; 3]; VOICES],
    phases: [[f32; 3]; VOICES],
    pair: [f32; 2],
    chuff: [f32; VOICES],
    right: bool,
}
impl Decodable for TrainSource {
    type Decoder = Stream;
    fn decoder(&self) -> Stream {
        Stream {
            controls: self.controls.clone(),
            bank: self.bank.clone(),
            gains: [[[0.0; 2]; 3]; VOICES],
            phases: [[0.0; 3]; VOICES],
            pair: [0.0; 2],
            chuff: [0.0; VOICES],
            right: false,
        }
    }
}
impl Iterator for Stream {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.right {
            self.right = false;
            return Some(self.pair[1]);
        }
        self.pair = [0.0; 2];
        for voice in 0..VOICES {
            let speed = f32::from_bits(self.controls.speeds[voice].load(Ordering::Relaxed))
                .clamp(0.0, 32.0);
            self.chuff[voice] = (self.chuff[voice] + (0.6 + speed * 1.4) / RATE).fract();
            for kind in 0..3 {
                let phase = &mut self.phases[voice][kind];
                let rate = if kind == 2 {
                    (speed / 8.0).clamp(0.1, 4.0)
                } else {
                    1.0
                };
                let clip = &self.bank[kind];
                let index = *phase as usize % clip.len();
                let next = (index + 1) % clip.len();
                let sample = clip[index] + (clip[next] - clip[index]) * phase.fract();
                *phase = (*phase + rate).rem_euclid(clip.len() as f32);
                let chuff = if kind == 0 {
                    0.06 + (-self.chuff[voice] * 9.0).exp()
                } else {
                    1.0
                };
                for channel in 0..2 {
                    let target = f32::from_bits(
                        self.controls.gains[voice][kind][channel].load(Ordering::Relaxed),
                    );
                    self.gains[voice][kind][channel] +=
                        (target - self.gains[voice][kind][channel]) / (RATE * 0.06);
                    self.pair[channel] += sample * chuff * self.gains[voice][kind][channel];
                }
            }
        }
        self.pair = self.pair.map(|sample| sample.clamp(-0.9, 0.9));
        self.right = true;
        Some(self.pair[0])
    }
}
impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE as u32).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
fn load_bank() -> Result<Bank, String> {
    let mut result: [Vec<f32>; 3] = Default::default();
    for (index, path) in [
        "trains/locomotive-steam.wav",
        "objects/furnace-fire.wav",
        "trains/rail-rolling.wav",
    ]
    .into_iter()
    .enumerate()
    {
        let data = std::fs::read(terra_core::asset_directory().join("audio").join(path))
            .map_err(|e| e.to_string())?;
        let source = AudioSource { bytes: data.into() };
        let decoder = <AudioSource as Decodable>::Decoder::try_from(std::io::Cursor::new(source))
            .map_err(|e| e.to_string())?;
        let channels = usize::from(decoder.channels().get());
        let source_rate = decoder.sample_rate().get();
        let decoded: Vec<f32> = decoder.take(source_rate as usize * channels * 12).collect();
        let mono: Vec<f32> = decoded
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        if mono.is_empty() {
            return Err(format!("Empty train recording: {path}"));
        }
        let length = mono.len() * RATE as usize / source_rate as usize;
        result[index] = (0..length)
            .map(|sample| mono[(sample * source_rate as usize / RATE as usize).min(mono.len() - 1)])
            .collect();
        if result[index].is_empty() || result[index].iter().any(|v| !v.is_finite()) {
            return Err(format!("Invalid train recording: {path}"));
        }
    }
    Ok(Arc::new(result))
}
#[derive(Resource, Default)]
struct Sounds {
    controls: Arc<Controls>,
    loading: Option<Task<Result<Bank, String>>>,
    bank: Option<Bank>,
    entity: Option<Entity>,
    source: Option<Handle<TrainSource>>,
}
pub(crate) fn install(app: &mut App) {
    app.add_audio_source::<TrainSource>()
        .init_resource::<Sounds>()
        .add_systems(Startup, |mut sounds: ResMut<Sounds>| {
            sounds.loading = Some(AsyncComputeTaskPool::get().spawn(async { load_bank() }));
        })
        .add_systems(Update, update.after(terra_player::PlayerSet::Movement));
}
fn levels(speed: f32, powered: bool, engine: bool) -> [f32; 3] {
    [
        if powered && engine { 0.24 } else { 0.0 },
        if powered && engine { 0.22 } else { 0.0 },
        (speed / 6.0).sqrt().clamp(0.0, 1.0) * 0.32,
    ]
}
fn update(
    mut commands: Commands,
    session: Option<Res<ClientSession>>,
    config: Option<Res<terra_core::AudioConfig>>,
    cameras: Query<&Transform, With<terra_player::TerraCamera>>,
    mut sounds: ResMut<Sounds>,
    mut sources: ResMut<Assets<TrainSource>>,
) {
    if let Some(task) = sounds.loading.as_mut()
        && let Some(result) = future::block_on(future::poll_once(task))
    {
        sounds.loading = None;
        match result {
            Ok(bank) => sounds.bank = Some(bank),
            Err(error) => warn!(%error,"Cannot load train audio"),
        }
    }
    let listener = cameras.iter().next();
    let master = config.as_ref().map_or(1.0, |c| {
        if c.muted {
            0.0
        } else {
            c.master_volume * c.ambience_volume
        }
    });
    let mut nearest = Vec::new();
    if let Some(session) = session.as_ref().filter(|session| session.ready)
        && let Some(listener) = listener
    {
        for car in &session.trains.vehicles {
            let motion = session.displayed_train_motion(car);
            let (origin, _) = car.model_transform_at(motion);
            let delta = Vec3::from_array(origin.map(|v| v as f32)) - listener.translation;
            let distance = delta.length();
            if distance >= 56.0 {
                continue;
            }
            let speed = car.motion.speed_mps.abs() as f32;
            let engine = car.parts.iter().any(|part| part.material == 218);
            let powered = car.motion.fuel_available
                && (speed > 0.01 || car.motion.throttle != 0 && !car.motion.brake);
            let levels = levels(speed, powered, engine);
            if levels.iter().all(|gain| *gain == 0.0) {
                continue;
            }
            let pan = if distance > 0.2 {
                delta.dot(*listener.right()) / distance
            } else {
                0.0
            };
            nearest.push((distance, car.id, speed, levels, pan.clamp(-0.9, 0.9)));
        }
    }
    nearest.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    nearest.truncate(VOICES);
    for voice in 0..VOICES {
        let mut gains = [[0.0; 2]; 3];
        let mut speed = 0.0;
        if let Some(&(distance, _, velocity, levels, pan)) = nearest.get(voice) {
            speed = velocity;
            let attenuation = (1.0 - distance / 56.0).powi(2) * master;
            for kind in 0..3 {
                gains[kind] = [
                    levels[kind] * attenuation * ((1.0 - pan) * 0.5).sqrt(),
                    levels[kind] * attenuation * ((1.0 + pan) * 0.5).sqrt(),
                ];
            }
        }
        sounds.controls.speeds[voice].store(speed.to_bits(), Ordering::Relaxed);
        for (kind, channels) in gains.into_iter().enumerate() {
            for (channel, gain) in channels.into_iter().enumerate() {
                sounds.controls.gains[voice][kind][channel]
                    .store(gain.to_bits(), Ordering::Relaxed);
            }
        }
    }
    if sounds.entity.is_none()
        && !nearest.is_empty()
        && let Some(bank) = sounds.bank.clone()
    {
        let source = sources.add(TrainSource {
            controls: sounds.controls.clone(),
            bank,
        });
        sounds.entity = Some(
            commands
                .spawn((
                    Name::new("Locomotive, fire and railway soundscape"),
                    AudioPlayer(source.clone()),
                ))
                .id(),
        );
        sounds.source = Some(source);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stopped_unpowered_trains_are_silent_and_unpowered_coasting_keeps_the_rail_sound() {
        assert_eq!(levels(0.0, false, true), [0.0; 3]);
        assert_eq!(levels(0.0, true, false), [0.0; 3]);
        let coast = levels(8.0, false, false);
        assert_eq!(coast[0], 0.0);
        assert_eq!(coast[1], 0.0);
        assert!(coast[2] > 0.0);
        let drive = levels(8.0, true, true);
        assert!(drive.iter().all(|gain| *gain > 0.0));
    }
    #[test]
    fn recordings_decode_and_the_bounded_stereo_mix_has_audible_finite_output() {
        let bank = load_bank().unwrap();
        let controls = Arc::new(Controls::default());
        for kind in 0..3 {
            for channel in 0..2 {
                controls.gains[0][kind][channel].store(0.25f32.to_bits(), Ordering::Relaxed);
            }
        }
        controls.speeds[0].store(8.0f32.to_bits(), Ordering::Relaxed);
        let samples: Vec<_> = TrainSource { controls, bank }
            .decoder()
            .take(48_000)
            .collect();
        assert!(samples.iter().all(|v| v.is_finite() && v.abs() <= 0.9));
        assert!(samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32 > 0.0001);
    }
}
