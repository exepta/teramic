//! Explicit observed-snapshot fixture through the actual shared audio device.
use super::super::{scene::Soundscape, source::RecordedSoundscape};
use super::*;
use bevy::{app::AppExit, audio::AudioSink};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use terra_core::creatures::{CreatureKind as K, CreatureNeeds};
use terra_network::client::{CreatureTrack, ReplicatedCreatures};
#[derive(Resource)]
struct Inspection {
    started: Instant,
    step: usize,
    since: Instant,
    baseline: Option<[u32; COUNT]>,
    root: Vec3,
}
const ID: u64 = 91_731;
const STEPS: [(B, f32, f32, f32, &str); 12] = [
    (B::Burrowed, 0., 0., 0.4, "baseline"),
    (B::Burrowed, 0., 1., 0.9, "dig"),
    (B::Tremor, 0.2, 0., 0.9, "warning"),
    (B::Leap, 0.015, 0., 0.25, "hidden-head"),
    (B::Leap, 0.10, 0., 2.6, "breach-roar"),
    (B::Leap, 0.88, 0., 0.9, "entry"),
    (B::Emerge, 0.95, 0., 0.45, "upright-no-second-roar"),
    (B::Attack, 0.10, 0., 0.4, "prepare-bite"),
    (B::Attack, pose::BITE_PHASE, 0., 0.65, "bite"),
    (B::Spit, 0., 0., 1.1, "spit"),
    (B::Attack, 0.45, 0., 0.9, "hurt"),
    (B::Dead, 0., 0., 3.0, "death"),
];
pub(super) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-steps")
        || std::env::var_os("TERRA_WORM_AUDIO_INSPECT").is_none()
    {
        return;
    }
    app.init_resource::<ReplicatedCreatures>()
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .before(super::super::scene::update),
        );
}
fn setup(mut commands: Commands, terrain: Res<terra_world::streaming::WorldTerrain>) {
    let y = terrain.0.surface_height(8., 8.) as f32 + 20.;
    commands.insert_resource(Inspection {
        started: Instant::now(),
        step: 0,
        since: Instant::now(),
        baseline: None,
        root: Vec3::new(8., y, 3.),
    });
}
#[derive(bevy::ecs::system::SystemParam)]
struct View<'w, 's> {
    render: Res<'w, terra_render::RenderStatus>,
    sinks: Query<'w, 's, &'static AudioSink, With<AudioPlayer<RecordedSoundscape>>>,
    bodies: Query<'w, 's, &'static mut Transform, With<terra_player::PlayerBody>>,
    focus: ResMut<'w, terra_world::streaming::ChunkFocus>,
}
fn run(
    mut state: ResMut<Inspection>,
    mut animals: ResMut<ReplicatedCreatures>,
    sounds: Res<Soundscape>,
    mut view: View,
    mut exit: MessageWriter<AppExit>,
) {
    let listener = state.root + Vec3::Z * 5.;
    for mut body in &mut view.bodies {
        body.translation = listener;
    }
    view.focus.0 = listener.to_array().map(f64::from);
    if state.started.elapsed() > Duration::from_secs(60) {
        error!("Worm audio fixture timed out");
        exit.write(AppExit::error());
        return;
    }
    if sounds
        .entity
        .is_none_or(|entity| view.sinks.get(entity).is_err())
    {
        return;
    }
    let played = sounds
        .controls
        .worm_played
        .each_ref()
        .map(|count| count.load(Ordering::Relaxed));
    if state.baseline.is_none() && !view.render.ready {
        return;
    }
    if state.baseline.is_none() {
        state.baseline = Some(played);
        state.since = Instant::now();
    }
    if state.step == STEPS.len() {
        let consumed: [u32; COUNT] =
            std::array::from_fn(|i| played[i].wrapping_sub(state.baseline.unwrap()[i]));
        if consumed[..7].contains(&0) {
            error!(?consumed, "Worm sink missed an actual phase sound");
            exit.write(AppExit::error());
            return;
        }
        assert_eq!(
            consumed[Sound::Roar as usize],
            1,
            "one eruption may roar only once"
        );
        info!(
            ?consumed,
            clips = FIRST + COUNT,
            "Sandworm audio passed: observed breach roar, granular digging, landing, bite, spit, hurt and death through the real shared sink"
        );
        exit.write(AppExit::Success);
        return;
    }
    let (phase, progress, speed, duration, label) = STEPS[state.step];
    let length = match phase {
        B::Leap => pose::LEAP_SECONDS,
        B::Emerge => pose::EMERGE_SECONDS,
        B::Attack => pose::EXPOSED_SECONDS,
        B::Spit => pose::SPIT_SECONDS,
        _ => 1.,
    };
    let a = Creature {
        id: ID,
        kind: K::SandWorm,
        home: [0; 2],
        position: state.root.to_array().map(f64::from),
        velocity: [speed, 0., 0.],
        yaw: 0.,
        health: if phase == B::Dead {
            0.
        } else if state.step >= 10 {
            K::SandWorm.health() - 6.
        } else {
            K::SandWorm.health()
        },
        behavior: phase,
        timer: length * (1. - progress),
        age: state.started.elapsed().as_secs_f64(),
        variant: 0,
        needs: CreatureNeeds::full(K::SandWorm),
    };
    animals.creatures.insert(
        ID,
        CreatureTrack {
            playback: Default::default(),
            previous: a.clone(),
            current: a,
            received: Instant::now(),
            interval: 0.,
        },
    );
    if state.since.elapsed() >= Duration::from_secs_f32(duration) {
        info!(stage = state.step, label, "Sandworm observed audio fixture");
        state.step += 1;
        state.since = Instant::now();
    }
}
