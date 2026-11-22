//! Positional calls follow the authoritative action clock. A separate stereo
//! bus preserves the orchestral placement and the tail of the previous theme.
use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};
use terra_core::creatures::{Creature, CreatureBehavior as B, CreatureKind as K, dragons};
use terra_network::client::ReplicatedCreatures;

pub(super) const FAMILIES: [&str; 3] = ["dark", "earth", "water"];
pub(super) const CUES: [&str; 6] = ["roar", "wing", "land", "cast", "hurt", "death"];

fn family(kind: K) -> Option<usize> {
    match kind {
        K::DarkDragon => Some(0),
        K::EarthDragon => Some(1),
        K::WaterDragon => Some(2),
        _ => None,
    }
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
            Self::Cast => 0.66,
            Self::Land => 0.63,
            Self::Hurt => 0.42,
            Self::Wing => 0.24,
        }
    }
    pub(super) fn radius(self) -> f32 {
        match self {
            Self::Roar | Self::Death => 112.,
            Self::Cast => 86.,
            Self::Land => 74.,
            Self::Wing | Self::Hurt => 52.,
        }
    }
}
#[derive(Default)]
pub(super) struct Cadence {
    last: Option<(B, f32, f32, f64)>,
    next_wing: f32,
    next_hurt: f32,
}

impl Cadence {
    pub(super) fn update(&mut self, animal: &Creature, now: f32) -> Option<Sound> {
        let p = dragons::phase(animal.behavior, animal.timer);
        let (old, prior, health, age) =
            self.last
                .replace((animal.behavior, p, animal.health, animal.age))?;
        // A stale track re-entering earshot starts at its current pose.
        if animal.age < age || animal.age - age > 1.5 {
            return None;
        }
        let restarted = animal.behavior != old || p + 0.18 < prior;
        let impact = match animal.behavior {
            B::Emerge => Some((0.16, Sound::Roar)),
            B::Leap => Some((0.32, Sound::Wing)),
            B::Land => Some((0.63, Sound::Land)),
            B::Attack | B::Tremor => Some((0.49, Sound::Land)),
            B::Ink | B::Spit | B::DragonWard | B::DragonVolley | B::DragonSurge => {
                Some((0.34, Sound::Cast))
            }
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
            self.next_hurt = now + 1.1;
            return Some(Sound::Hurt);
        }
        let wing_phase = |age| terra_core::creatures::dragon_pose::wing_angle(age, animal.id);
        if animal.behavior == B::Fly
            && old == B::Fly
            && wing_phase(animal.age) < wing_phase(age)
            && terra_core::creatures::dragon_pose::stroke_strength(animal, animal.age) > 0.38
            && now >= self.next_wing
        {
            self.next_wing = now + 0.85;
            return Some(Sound::Wing);
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
    config: Option<Res<terra_core::AudioConfig>>,
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
    let gain = config
        .as_deref()
        .map_or(1., |c| if c.muted { 0. } else { c.master_volume });
    let mut found = false;
    for (entity, mut theme, sink) in &mut themes {
        let entering = desired == Some(theme.family);
        found |= entering;
        if !entering || sink.is_some() {
            theme.envelope = change_envelope(theme.envelope, entering, time.delta_secs().min(0.25));
        }
        if let Some(mut sink) = sink {
            let smooth = theme.envelope * theme.envelope * (3. - 2. * theme.envelope);
            sink.set_volume(Volume::Linear(0.48 * smooth * gain));
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
    #[test]
    fn replicated_attack_crossings_fire_once_including_death() {
        let kind = K::DarkDragon;
        let mut a = Creature {
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
        };
        let mut cadence = Cadence::default();
        assert_eq!(cadence.update(&a, 0.), None);
        a.behavior = B::Spit;
        a.timer = 2.2;
        assert_eq!(cadence.update(&a, 0.1), None);
        a.timer = 1.4;
        assert_eq!(cadence.update(&a, 0.9), Some(Sound::Cast));
        assert_eq!(cadence.update(&a, 1.), None);
        a.health = 0.;
        a.behavior = B::Dead;
        a.timer = 1.8;
        assert_eq!(cadence.update(&a, 1.1), Some(Sound::Death));
        assert_eq!(cadence.update(&a, 1.2), None);
    }
}
