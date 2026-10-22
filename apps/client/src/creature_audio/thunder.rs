//! Distance-delayed thunder shares the existing bounded ambience mixer, volume
//! controls, underwater filter, and cave reverberation.
use super::{
    clips,
    scene::Soundscape,
    source::{Category, Cue},
};
use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct ThunderPlayback {
    observer: terra_core::LightningObserver,
    pending: VecDeque<(f64, Cue)>,
}

fn schedule(pending: &mut VecDeque<(f64, Cue)>, at: f64, cue: Cue) {
    let index = pending
        .iter()
        .position(|(due, _)| *due > at)
        .unwrap_or(pending.len());
    pending.insert(index, (at, cue));
}

pub(super) fn update(
    time: Res<Time<Real>>,
    weather: Option<Res<terra_core::WorldWeather>>,
    clock: Option<Res<terra_core::WorldClock>>,
    access: Res<terra_world::streaming::WorldAccess>,
    bodies: Query<&Transform, With<terra_player::PlayerBody>>,
    sounds: Res<Soundscape>,
    mut state: Local<ThunderPlayback>,
) {
    let event = weather
        .as_ref()
        .and_then(|weather| weather.latest_lightning);
    let live = state.observer.observe(
        access.ready() && weather.is_some() && sounds.entity.is_some() && !bodies.is_empty(),
        event,
    );
    if !state.observer.active() {
        state.pending.clear();
        return;
    }
    let Some(body) = bodies.iter().next() else {
        return;
    };
    if let Some(weather) = weather
        && let Some(event) = live
    {
        let age = event
            .real_age(
                weather.elapsed_total,
                clock
                    .as_ref()
                    .map_or(1440.0, |clock| clock.day_length_seconds),
            )
            .unwrap_or(f32::INFINITY);
        let delta = Vec3::from_array(event.position.map(|v| v as f32)) - body.translation;
        let distance = delta.length();
        if age < 3.0 && distance < 1800.0 && state.pending.len() < 8 {
            let gain = (0.62 / (1.0 + distance / 170.0)).clamp(0.035, 0.62);
            let right = body.rotation * Vec3::X;
            let pan = delta.normalize_or_zero().dot(right).clamp(-0.85, 0.85);
            let cue = Cue {
                category: Category::Environment,
                sound: clips::THUNDER_CLIP,
                gains: [gain * (1.0 - pan * 0.45), gain * (1.0 + pan * 0.45)],
                pitch: 0.9 + (event.id % 17) as f32 / 80.0,
                reverb: sounds.room.cave * 0.3,
            };
            schedule(
                &mut state.pending,
                time.elapsed_secs_f64() + f64::from((distance / 343.0 - age).max(0.0)),
                cue,
            );
        }
    }
    while state
        .pending
        .front()
        .is_some_and(|(at, _)| *at <= time.elapsed_secs_f64())
    {
        if let Some((_, cue)) = state.pending.pop_front() {
            sounds.controls.enqueue(cue);
        }
    }
}

/// Broadband crack and three filtered rolling bands, generated once on the
/// existing asynchronous sample-bank worker rather than synthesized each frame.
pub(super) fn synthesize() -> Vec<f32> {
    let mut state = 0x73a9_f006_u32;
    let mut low = 0.0_f32;
    let mut body = 0.0_f32;
    let mut deep = 0.0_f32;
    (0..24_000 * 8)
        .map(|frame| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let noise = (state as f64 / f64::from(u32::MAX) * 2.0 - 1.0) as f32;
            low += (noise - low) * 0.09;
            body += (noise - body) * 0.025;
            deep += (noise - deep) * 0.006;
            let t = frame as f32 / 24_000.0;
            let onset = (t / 0.014).min(1.0);
            let crack = (noise - low) * (-t * 19.0).exp() * 0.14;
            let rolling = (low * 0.34 + body * 2.0 + deep * 3.2)
                * (-t * 0.62).exp()
                * (0.72 + 0.18 * (t * 8.7).sin() + 0.1 * (t * 3.3).sin());
            let end = ((8.0 - t) / 0.5).clamp(0.0, 1.0);
            ((crack + rolling) * onset * end).tanh() * 0.85
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn nearby_thunder_is_not_delayed_behind_a_previous_distant_strike() {
        let cue = super::Cue {
            category: super::Category::Environment,
            sound: 0,
            gains: [0.5; 2],
            pitch: 1.0,
            reverb: 0.0,
        };
        let mut queue = std::collections::VecDeque::new();
        super::schedule(&mut queue, 5.0, cue);
        super::schedule(&mut queue, 1.2, cue);
        super::schedule(&mut queue, 3.0, cue);
        assert_eq!(
            queue.iter().map(|(at, _)| *at).collect::<Vec<_>>(),
            [1.2, 3.0, 5.0]
        );
    }

    #[test]
    fn thunder_is_finite_deterministic_bounded_and_decays() {
        let a = super::synthesize();
        assert_eq!(a, super::synthesize());
        assert_eq!(a.len(), 24_000 * 8);
        assert!(
            a.iter()
                .all(|value| value.is_finite() && value.abs() < 0.86)
        );
        let rms = |range: std::ops::Range<usize>| a[range].iter().map(|v| v * v).sum::<f32>();
        assert!(rms(0..24_000) > rms(24_000 * 6..24_000 * 7) * 20.0);
        assert_eq!(a[0], 0.0);
        assert!(a[a.len() - 1].abs() < 0.0001);
    }
}
