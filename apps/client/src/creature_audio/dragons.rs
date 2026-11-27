//! Positional calls follow the authoritative action clock. A separate stereo
//! bus preserves the orchestral placement and the tail of the previous theme.
use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};
use terra_core::creatures::{Creature, CreatureBehavior as B, CreatureKind as K, dragons};
use terra_network::client::ReplicatedCreatures;

pub(super) const FAMILIES: [&str; 4] = ["dark", "earth", "water", "fire"];
pub(super) const CUES: [&str; 15] = [
    "roar", "wing", "land", "cast", "hurt", "death", "bite", "tremor", "ward", "volley", "surge",
    "burrow", "step", "launch", "breath",
];

fn family(kind: K) -> Option<usize> {
    match kind {
        K::DarkDragon => Some(0),
        K::EarthDragon => Some(1),
        K::FireDragon => Some(3),
        K::WaterDragon => Some(2),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) struct Listener {
    pub(super) ear: Vec3,
    pub(super) right: Vec3,
    pub(super) cave: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Sound {
    Roar,
    Wing,
    Land,
    Cast,
    Hurt,
    Death,
    Bite,
    Tremor,
    Ward,
    Volley,
    Surge,
    Burrow,
    Step,
    Launch,
    Breath,
}
impl Sound {
    pub(super) fn clip(self, kind: K) -> usize {
        super::worm::FIRST
            + super::worm::BANK_LEN
            + family(kind).expect("dragon voice") * CUES.len()
            + self as usize
    }
    pub(super) fn level(self) -> f32 {
        match self {
            Self::Roar => 0.78,
            Self::Death => 0.68,
            Self::Cast | Self::Volley | Self::Surge => 0.72,
            Self::Tremor => 0.83,
            Self::Land | Self::Ward | Self::Burrow => 0.65,
            Self::Bite | Self::Launch => 0.58,
            Self::Step | Self::Hurt => 0.45,
            Self::Wing => 0.25,
            Self::Breath => 0.15,
        }
    }
    pub(super) fn radius(self) -> f32 {
        match self {
            Self::Roar | Self::Death | Self::Tremor => 112.,
            Self::Cast | Self::Volley | Self::Surge | Self::Ward => 87.,
            Self::Land | Self::Burrow | Self::Launch => 74.,
            Self::Wing | Self::Hurt | Self::Bite => 55.,
            Self::Step => 48.,
            Self::Breath => 26.,
        }
    }

    fn position(self, a: &Creature, origin: Vec3, side: i8) -> Vec3 {
        use terra_core::creatures::dragon_pose::{Joint, Pose, wrist};
        let pose = Pose::of(a, 0.);
        let local = match self {
            Self::Wing => pose.point(Joint::WingArm(side), wrist(f32::from(side))),
            Self::Land | Self::Step | Self::Tremor | Self::Burrow => Vec3::Y * 0.4,
            Self::Ward | Self::Death | Self::Launch | Self::Hurt => {
                pose.bone(Joint::Trunk).translation.into()
            }
            _ => pose.bone(Joint::Breath).translation.into(),
        };
        origin
            + Quat::from_rotation_y(a.yaw)
                * local
                * (dragons::model_scale(a.kind, a.variant) * a.growth_scale() as f32)
    }

    pub(super) fn emit(
        self,
        a: &Creature,
        origin: Vec3,
        listener: Listener,
        mut transmission: impl FnMut(Vec3) -> f32,
        mut enqueue: impl FnMut(super::source::Cue),
    ) -> bool {
        let mut audible = false;
        let wings = self == Self::Wing;
        let sides: &[i8] = if wings { &[-1, 1] } else { &[1] };
        for &side in sides {
            let point = self.position(a, origin, side);
            let delta = point - listener.ear;
            let distance = delta.length();
            if distance >= self.radius() {
                continue;
            }
            let direction = delta.normalize_or_zero();
            let pan = direction.dot(listener.right).clamp(-0.96, 0.96);
            let body = if matches!(self, Self::Tremor | Self::Burrow) {
                0.19
            } else {
                0.
            };
            let direct = transmission(point).clamp(0., 1.) * (1. - body) + body;
            let gain = self.level()
                * if wings { 0.56 } else { 1. }
                * (1. - distance / self.radius()).powi(2)
                * direct;
            let radial_speed = Vec3::from_array(a.velocity).dot(direction);
            let doppler = (343. / (343. + radial_speed.clamp(-35., 35.))).clamp(0.91, 1.10);
            let take = (a.id ^ (a.age * 5.) as u64).wrapping_mul(0x9e3779b97f4a7c15);
            let pitch = (0.97 + (take >> 57) as f32 * 0.00035 + (a.id % 11) as f32 * 0.005
                - f32::from(a.variant % 4) * 0.017
                + (1. - a.growth_scale() as f32) * 0.23
                + if wings { f32::from(side) * 0.026 } else { 0. })
                * doppler;
            audible = true;
            enqueue(super::source::Cue {
                category: super::source::Category::Animals,
                sound: self.clip(a.kind),
                gains: [
                    gain * ((1. - pan) * 0.5).sqrt(),
                    gain * ((1. + pan) * 0.5).sqrt(),
                ],
                pitch,
                reverb: (0.055 + listener.cave * 0.55 + distance / self.radius() * 0.09).min(0.78),
            });
        }
        audible
    }
}
#[derive(Default)]
pub(super) struct Cadence {
    last: Option<(B, f32, f32, f64)>,
    next_wing: f32,
    next_hurt: f32,
    next_breath: f32,
}

impl Cadence {
    pub(super) fn update(&mut self, animal: &Creature, now: f32) -> Option<Sound> {
        let p = dragons::phase(animal.behavior, animal.timer);
        let Some((old, prior, health, age)) =
            self.last
                .replace((animal.behavior, p, animal.health, animal.age))
        else {
            self.next_breath = now + 6. + (animal.id % 7) as f32;
            return None;
        };
        // A stale track re-entering earshot starts at its current pose.
        if animal.age < age || animal.age - age > 1.5 {
            return None;
        }
        let restarted = animal.behavior != old || p + 0.18 < prior;
        let impact = match animal.behavior {
            B::Emerge => Some((0.16, Sound::Roar)),
            B::Leap => Some((0.32, Sound::Launch)),
            B::Land => Some((0.63, Sound::Land)),
            B::Attack => Some((dragons::impact_phase(B::Attack), Sound::Bite)),
            B::Tremor => Some((dragons::impact_phase(B::Tremor), Sound::Tremor)),
            B::Spit | B::Ink => Some((dragons::impact_phase(animal.behavior), Sound::Cast)),
            B::DragonBurrow => Some((0.24, Sound::Burrow)),
            B::DragonWard => Some((0.22, Sound::Ward)),
            B::DragonVolley => Some((0.30, Sound::Volley)),
            B::DragonSurge => Some((dragons::impact_phase(B::DragonSurge), Sound::Surge)),
            B::Dead => Some((0.0, Sound::Death)),
            _ => None,
        };
        if let Some((at, sound)) = impact
            && (restarted || prior < at)
            && p >= at
        {
            return Some(sound);
        }
        if animal.health > 0. && animal.health + 0.05 < health && now >= self.next_hurt {
            self.next_hurt = now + 1.9;
            return Some(Sound::Hurt);
        }
        let wing_phase = |age| terra_core::creatures::dragon_pose::wing_angle(animal, age);
        if animal.behavior == B::Fly
            && old == B::Fly
            && wing_phase(animal.age) < wing_phase(age)
            && terra_core::creatures::dragon_pose::stroke_strength(animal, animal.age) > 0.38
            && now >= self.next_wing
        {
            self.next_wing = now + 1.15;
            return Some(Sound::Wing);
        }
        if now >= self.next_breath && matches!(animal.behavior, B::Rest | B::Sleep | B::Wander) {
            self.next_breath = now + 10. + ((animal.id + animal.age as u64) % 9) as f32 * 0.81;
            return Some(Sound::Breath);
        }
        None
    }
}

#[derive(Component)]
struct Theme {
    family: usize,
    envelope: f32,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Update, music);
}

fn change_envelope(value: f32, entering: bool, dt: f32) -> f32 {
    (value + if entering { dt / 2.3 } else { -dt / 3.1 }).clamp(0., 1.)
}
#[derive(bevy::ecs::system::SystemParam)]
struct EncounterAudio<'w> {
    tracks: Option<Res<'w, ReplicatedCreatures>>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    menu: Option<Res<'w, terra_ui::menu::MenuState>>,
}

fn music(
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Res<AssetServer>,
    encounter: EncounterAudio,
    preferences: crate::audio_preferences::AudioPreferences,
    mut themes: Query<(Entity, &mut Theme, Option<&mut AudioSink>)>,
) {
    let desired = encounter
        .tracks
        .as_deref()
        .filter(|_| {
            encounter
                .session
                .as_ref()
                .is_some_and(|s| s.ready && !s.dead())
                && encounter
                    .menu
                    .as_ref()
                    .is_none_or(|m| m.world_audio_active())
        })
        .and_then(|r| r.creatures.get(&r.boss?))
        .filter(|track| track.current.health > 0.)
        .and_then(|track| family(track.current.kind));
    let gain = preferences.master_gain();
    let mut found = false;
    for (entity, mut theme, sink) in &mut themes {
        let entering = desired == Some(theme.family);
        found |= entering;
        if !entering || sink.is_some() {
            theme.envelope = change_envelope(theme.envelope, entering, time.delta_secs().min(0.25));
        }
        if let Some(mut sink) = sink {
            let smooth = theme.envelope * theme.envelope * (3. - 2. * theme.envelope);
            sink.set_volume(Volume::Linear(0.36 * smooth * gain));
        }
        if !entering && theme.envelope <= 0. {
            commands.entity(entity).despawn();
        }
    }
    if !found && let Some(family) = desired {
        let name = FAMILIES[family];
        commands.spawn((
            Name::new(format!("{name} dragon encounter score")),
            Theme {
                family,
                envelope: 0.,
            },
            AudioPlayer::new(assets.load(format!("audio/dragons/{name}-theme.wav"))),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_scores_decode_in_stereo_and_join_at_the_same_sample_level() {
        use bevy::audio::{AudioSource, Decodable, Source};
        for family in FAMILIES {
            let path =
                terra_core::asset_directory().join(format!("audio/dragons/{family}-theme.wav"));
            let audio = AudioSource {
                bytes: std::fs::read(path).unwrap().into(),
            };
            let mut decoder = audio.decoder();
            assert_eq!(decoder.channels().get(), 2);
            assert_eq!(decoder.sample_rate().get(), 44_100);
            let first = [decoder.next().unwrap(), decoder.next().unwrap()];
            let mut last = first;
            let mut frames = 1;
            while let Some(left) = decoder.next() {
                last = [left, decoder.next().expect("complete stereo frame")];
                frames += 1;
            }
            assert!(frames > 44_100 * 60);
            for (start, end) in first.into_iter().zip(last) {
                assert!((start - end).abs() < 0.01, "{family} loop clicks");
            }
        }
    }

    #[test]
    fn fade_reverses_from_the_current_gain_and_reaches_silence() {
        let a = change_envelope(0., true, 0.7);
        let b = change_envelope(a, false, 0.21);
        assert!(0. < b && b < a && a < 1.);
        assert_eq!(change_envelope(b, false, 4.), 0.);
        assert_eq!(change_envelope(b, true, 4.), 1.);
    }
    fn adult(kind: K) -> Creature {
        Creature {
            id: 1,
            kind,
            home: [0, 0],
            position: [0.; 3],
            velocity: [0.; 3],
            yaw: 0.,
            health: kind.health(),
            behavior: B::Rest,
            timer: 0.,
            age: 60.,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(kind),
        }
    }
    #[test]
    fn all_four_dragon_banks_have_unique_voices_and_separate_action_impacts() {
        let bank = super::super::clips::load().unwrap();
        let mut signatures = std::collections::BTreeSet::new();
        for kind in [K::DarkDragon, K::EarthDragon, K::WaterDragon, K::FireDragon] {
            for (cue, name) in CUES.iter().enumerate() {
                let clip = &bank[Sound::Roar.clip(kind) + cue];
                assert!(clip.len() >= 12_000 && clip.iter().all(|v| v.is_finite()));
                let rms = clip.iter().map(|s| s * s).sum::<f32>() / clip.len() as f32;
                assert!(rms > 0.0002);
                assert!(
                    signatures.insert((clip.len(), rms.to_bits())),
                    "{kind:?} {name}"
                );
            }
            for (behavior, expected) in [
                (B::Emerge, Sound::Roar),
                (B::Leap, Sound::Launch),
                (B::Land, Sound::Land),
                (B::Attack, Sound::Bite),
                (B::Spit, Sound::Cast),
                (B::Ink, Sound::Cast),
                (B::Tremor, Sound::Tremor),
                (B::DragonWard, Sound::Ward),
                (B::DragonSurge, Sound::Surge),
                (B::DragonVolley, Sound::Volley),
                (B::DragonBurrow, Sound::Burrow),
            ] {
                let mut a = adult(kind);
                let mut timing = Cadence::default();
                timing.update(&a, 0.);
                a.behavior = behavior;
                let mut fired = Vec::new();
                for frame in 0..=40 {
                    let phase = frame as f32 / 40.;
                    a.timer = dragons::duration(behavior) * (1. - phase);
                    a.age += 0.035;
                    fired.extend(timing.update(&a, phase));
                }
                assert_eq!(fired, [expected], "{kind:?} {behavior:?}");
            }
        }
    }

    #[test]
    fn wings_pan_from_their_own_joints_and_walls_and_distance_reduce_calls() {
        let a = adult(K::FireDragon);
        let listener = Listener {
            ear: Vec3::new(0., 2., 5.),
            right: Vec3::X,
            cave: 0.,
        };
        let hear = |sound: Sound, origin, transmission| {
            let mut cues = Vec::new();
            sound.emit(&a, origin, listener, |_| transmission, |cue| cues.push(cue));
            cues
        };
        let wings = hear(Sound::Wing, Vec3::ZERO, 1.);
        assert_eq!(wings.len(), 2);
        let balance = |g: [f32; 2]| g[0] - g[1];
        assert!(balance(wings[0].gains) * balance(wings[1].gains) < 0.);
        let roar = hear(Sound::Roar, Vec3::ZERO, 1.);
        let muffled = hear(Sound::Roar, Vec3::ZERO, 0.25);
        let distant = hear(Sound::Roar, Vec3::Z * 70., 1.);
        assert!(muffled[0].gains[0] < roar[0].gains[0] * 0.3);
        assert!(distant[0].gains[0] < roar[0].gains[0] * 0.4);
        assert!(hear(Sound::Roar, Vec3::Z * 170., 1.).is_empty());
        assert!(hear(Sound::Tremor, Vec3::ZERO, 0.)[0].gains[0] > 0.);
    }

    #[test]
    fn audible_encounters_render_species_specific_stereo_through_the_game_mixer() {
        use super::super::source::{Controls, RecordedSoundscape};
        use bevy::audio::Decodable;
        let bank = super::super::clips::load().unwrap();
        let mut samples = Vec::new();
        let mut signatures = std::collections::BTreeSet::new();
        let listener = Listener {
            ear: Vec3::new(0., 2., 0.),
            right: Vec3::X,
            cave: 0.24,
        };
        for kind in [K::DarkDragon, K::EarthDragon, K::WaterDragon, K::FireDragon] {
            let controls = std::sync::Arc::new(Controls::default());
            controls.configure(&terra_core::AudioConfig::default(), true);
            let mut stream = RecordedSoundscape {
                bank: bank.clone(),
                controls: controls.clone(),
            }
            .decoder();
            let mut a = adult(kind);
            let beginning = samples.len();
            for (sound, action, x, frames) in [
                (Sound::Roar, B::Emerge, -7., 112_000),
                (Sound::Wing, B::Fly, 0., 33_600),
                (Sound::Cast, B::Spit, 5., 72_000),
                (Sound::Land, B::Land, 2., 84_000),
            ] {
                a.behavior = action;
                a.age += 0.7;
                a.timer = dragons::duration(action) * 0.4;
                assert!(sound.emit(
                    &a,
                    Vec3::new(x, 1., -7.),
                    listener,
                    |_| 1.,
                    |cue| controls.enqueue(cue)
                ));
                samples.extend(stream.by_ref().take(frames * 2));
            }
            let slice = &samples[beginning..];
            let rms = (slice.iter().map(|v| v * v).sum::<f32>() / slice.len() as f32).sqrt();
            assert!(
                rms > 0.001 && rms < 0.3 && signatures.insert(rms.to_bits()),
                "{kind:?}: {rms}"
            );
            assert!(slice.iter().all(|s| s.is_finite() && s.abs() < 0.89));
            assert!(slice.chunks_exact(2).any(|s| (s[0] - s[1]).abs() > 0.01));
        }
        if let Some(root) = std::env::var_os("TERRA_AUDIO_EXPORT_DIR") {
            super::super::source::tests::export_stereo(
                &std::path::PathBuf::from(root),
                "drachen-hoerprobe",
                &samples,
            );
        }
    }

    #[test]
    fn replicated_attack_crossings_fire_once_including_death() {
        let mut a = adult(K::DarkDragon);
        let mut cadence = Cadence::default();
        assert_eq!(cadence.update(&a, 0.), None);
        a.behavior = B::Spit;
        a.timer = 2.2;
        assert_eq!(cadence.update(&a, 0.1), None);
        a.timer = 1.0;
        assert_eq!(cadence.update(&a, 0.9), Some(Sound::Cast));
        assert_eq!(cadence.update(&a, 1.), None);
        a.health = 0.;
        a.behavior = B::Dead;
        a.timer = 1.8;
        assert_eq!(cadence.update(&a, 1.1), Some(Sound::Death));
        assert_eq!(cadence.update(&a, 1.2), None);
    }
}
