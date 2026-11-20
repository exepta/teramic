use super::*;
use terra_core::creatures::{CreatureKind as K, CreatureNeeds};
fn animal(behavior: B, progress: f32, now: f32) -> Creature {
    let duration = match behavior {
        B::Leap => pose::LEAP_SECONDS,
        B::Emerge => pose::EMERGE_SECONDS,
        B::Attack => pose::EXPOSED_SECONDS,
        B::Spit => pose::SPIT_SECONDS,
        B::Retreat => pose::RETREAT_SECONDS,
        B::Tremor => pose::WARNING_SECONDS,
        _ => 1.0,
    };
    Creature {
        id: 17,
        kind: K::SandWorm,
        home: [0; 2],
        position: [8., 32., 12.],
        velocity: [0.; 3],
        yaw: 0.,
        health: K::SandWorm.health(),
        behavior,
        timer: duration * (1. - progress),
        age: f64::from(now),
        variant: 0,
        needs: CreatureNeeds::full(K::SandWorm),
    }
}
#[test]
fn a_real_head_breach_roars_once_and_the_same_leap_emergence_never_roars_again() {
    let mut state = Cadence::default();
    assert!(state.update(&animal(B::Burrowed, 0., 0.), 0.).is_none());
    let mut sounds = Vec::new();
    for (phase, start, duration, steps) in [
        (B::Tremor, 0.1, pose::WARNING_SECONDS, 36),
        (B::Leap, 2.0, pose::LEAP_SECONDS, 100),
        (B::Emerge, 7.05, pose::EMERGE_SECONDS, 27),
    ] {
        for i in 0..=steps {
            let progress = i as f32 / steps as f32;
            let now = start + duration * progress;
            let a = animal(phase, progress, now);
            if let Some(event) = state.update(&a, now) {
                if event.sound == Sound::Roar {
                    assert!(visible(&a));
                    assert!(pose::mouth_position(&a)[1] > a.position[1] + 0.08);
                }
                sounds.push(event.sound);
            }
        }
    }
    assert_eq!(sounds.iter().filter(|&&s| s == Sound::Roar).count(), 1);
    assert_eq!(sounds.iter().filter(|&&s| s == Sound::Land).count(), 1);
    assert!(sounds.contains(&Sound::Rumble));
}
#[test]
fn startup_visible_baselines_and_late_packets_do_not_replay_roars_or_attacks() {
    for phase in [B::Leap, B::Emerge, B::Attack, B::Spit, B::Retreat, B::Dead] {
        let mut state = Cadence::default();
        let a = animal(phase, 0.4, 4.0);
        assert!(state.update(&a, 4.).is_none());
        for i in 1..=10 {
            let event = state.update(&a, 4. + i as f32 * 0.05);
            assert!(event.is_none_or(|event| event.sound == Sound::Breath));
        }
    }
    let mut state = Cadence::default();
    state.update(&animal(B::Tremor, 0., 0.), 0.);
    assert!(
        state.update(&animal(B::Attack, 0.3, 2.), 2.).is_none(),
        "a stale new snapshot is a baseline"
    );
}
#[test]
fn bite_spit_hurt_and_death_follow_observed_actions_without_snapshot_duplicates() {
    let mut state = Cadence::default();
    let mut sounds = Vec::new();
    state.update(&animal(B::Emerge, 0.9, 0.), 0.);
    for i in 0..=55 {
        let now = 0.05 + i as f32 * 0.05;
        let a = animal(B::Attack, i as f32 / 84., now);
        if let Some(cue) = state.update(&a, now) {
            sounds.push(cue.sound);
        }
    }
    assert_eq!(sounds.iter().filter(|&&s| s == Sound::Bite).count(), 1);
    let mut a = animal(B::Spit, 0., 2.85);
    assert_eq!(state.update(&a, 2.85).unwrap().sound, Sound::Spit);
    assert!(state.update(&a, 2.9).is_none());
    a.health -= 6.;
    a.age = 2.95;
    assert_eq!(state.update(&a, 2.95).unwrap().sound, Sound::Hurt);
    assert!(state.update(&a, 3.).is_none());
    a.health -= 2.;
    a.age = 3.05;
    assert!(
        state.update(&a, 3.05).is_none(),
        "rapid repeated hits share one short hurt window"
    );
    a.health = 0.;
    a.behavior = B::Dead;
    a.age = 3.1;
    assert_eq!(state.update(&a, 3.1).unwrap().sound, Sound::Death);
    assert!(state.update(&a, 3.15).is_none());
}
#[test]
fn exact_replicated_bite_timer_roundtrip_fires_once_and_is_silent_on_join() {
    let mut state = Cadence::default();
    state.update(&animal(B::Attack, 0.10, 0.), 0.);
    let at_bite = animal(B::Attack, pose::BITE_PHASE, 0.1);
    let decoded = pose::phase_progress(at_bite.behavior, at_bite.timer);
    assert!((decoded - pose::BITE_PHASE).abs() <= f32::EPSILON);
    assert_eq!(state.update(&at_bite, 0.1).unwrap().sound, Sound::Bite);
    for i in 1..=5 {
        assert!(state.update(&at_bite, 0.1 + i as f32 * 0.05).is_none());
    }
    let mut joined = Cadence::default();
    assert!(joined.update(&at_bite, 0.1).is_none());
    assert!(joined.update(&animal(B::Attack, 0.22, 0.2), 0.2).is_none());
}
#[test]
fn buried_motion_has_quiet_granular_foley_and_standing_is_silent() {
    let mut state = Cadence::default();
    let mut a = animal(B::Burrowed, 0., 0.);
    state.update(&a, 0.);
    for i in 1..20 {
        a.age = i as f64 * 0.05;
        assert!(state.update(&a, i as f32 * 0.05).is_none());
    }
    a.velocity = [1.2, 0., 0.];
    a.age = 1.;
    assert_eq!(state.update(&a, 1.).unwrap().sound, Sound::Dig);
    assert!(state.update(&a, 1.05).is_none());
    assert!(Sound::Roar.radius() > Sound::Dig.radius() * 3.0);
    let origin = Sound::Roar.position(&animal(B::Attack, 0., 1.), Vec3::new(8., 32., 12.));
    assert!(
        origin.y > 38.0,
        "airborne voice is attached to the actual head"
    );
}
#[test]
fn all_worm_clips_use_the_single_shared_bank_and_can_be_mixed_to_a_real_stereo_stream() {
    use super::super::source::{Category, Controls, Cue, RecordedSoundscape};
    use bevy::audio::Decodable;
    use std::sync::{Arc, atomic::Ordering};
    let bank = super::super::clips::load().unwrap();
    assert_eq!(bank.len(), FIRST + COUNT);
    let controls = Arc::new(Controls::default());
    controls.volume.store(1_f32.to_bits(), Ordering::Relaxed);
    let mut stream = RecordedSoundscape {
        bank: bank.clone(),
        controls: controls.clone(),
    }
    .decoder();
    for sound in Sound::ALL {
        let clip = &bank[sound.clip()];
        assert!(clip.iter().any(|s| s.abs() > 0.1));
        controls.enqueue(Cue {
            category: Category::Animals,
            sound: sound.clip(),
            gains: [0.32, 0.24],
            pitch: 1.,
            reverb: 0.,
        });
        let samples: Vec<_> = stream.by_ref().take((clip.len() + 300) * 4).collect();
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.));
        assert!(
            samples.iter().any(|s| s.abs() > 0.01),
            "{} must reach the shared decoder",
            sound.name()
        );
    }
    assert_eq!(controls.played.load(Ordering::Relaxed), COUNT as u32);
}
