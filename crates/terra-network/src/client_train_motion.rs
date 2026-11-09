//! Presentation-only smoothing; confirmed collision and transaction state stay authoritative.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use terra_core::train_motion::TrainMotion;
const PRESENTATION_DELAY: Duration = Duration::from_millis(60);
const MAX_SAMPLES: usize = 32;
#[derive(Clone)]
pub(super) struct Track {
    samples: VecDeque<(Instant, TrainMotion)>,
    received: Instant,
}
impl Track {
    pub fn new(motion: TrainMotion, now: Instant) -> Self {
        Self {
            samples: VecDeque::from([(now, motion)]),
            received: now,
        }
    }
    pub fn push(&mut self, motion: TrainMotion, now: Instant) {
        let &(previous_time, previous) = self.samples.back().expect("train track has a sample");
        if previous == motion {
            return;
        }
        let elapsed = now.saturating_duration_since(self.received).as_secs_f64();
        let jump = (0..3)
            .map(|axis| (motion.translation[axis] - previous.translation[axis]).powi(2))
            .sum::<f64>()
            > 16.0;
        if jump || previous.pivot != motion.pivot || elapsed > 0.25 {
            *self = Self::new(motion, now);
            return;
        }
        // Reliable packets can arrive together. Travel and speed recover the
        // simulation interval, so a burst does not become a visible speed spike.
        let speed = (previous.speed_mps.abs() + motion.speed_mps.abs()) * 0.5;
        let travel = (motion.wheel_distance_m - previous.wheel_distance_m).abs();
        let duration = if speed > 0.1 && travel > 1e-6 {
            (travel / speed).clamp(1.0 / 240.0, 0.1)
        } else {
            elapsed.clamp(1.0 / 240.0, 0.1)
        };
        let estimated = previous_time + Duration::from_secs_f64(duration);
        let time = if estimated
            .checked_duration_since(now)
            .unwrap_or_else(|| now.duration_since(estimated))
            <= Duration::from_millis(100)
        {
            estimated
        } else {
            now.max(previous_time + Duration::from_micros(1))
        };
        self.samples.push_back((time, motion));
        self.received = now;
        while self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
    }
    pub fn at(&self, now: Instant) -> TrainMotion {
        let time = now.checked_sub(PRESENTATION_DELAY).unwrap_or(now);
        let &(first_time, first) = self.samples.front().expect("train track has a sample");
        if time <= first_time {
            return first;
        }
        let mut previous = (first_time, first);
        for &(next_time, next) in self.samples.iter().skip(1) {
            if time <= next_time {
                let duration = next_time
                    .saturating_duration_since(previous.0)
                    .as_secs_f64()
                    .max(1e-6);
                let t = (time.saturating_duration_since(previous.0).as_secs_f64() / duration)
                    .clamp(0.0, 1.0);
                return interpolate(previous.1, next, t);
            }
            previous = (next_time, next);
        }
        // Never extrapolate passengers or a body beyond confirmed track geometry.
        previous.1
    }
}
fn interpolate(from: TrainMotion, to: TrainMotion, t: f64) -> TrainMotion {
    let mut result = to;
    let mix = |a: f64, b: f64| a + (b - a) * t;
    result.translation =
        std::array::from_fn(|axis| mix(from.translation[axis], to.translation[axis]));
    let angle = (to.yaw - from.yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    result.yaw = (from.yaw + angle * t + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI;
    result.pitch = mix(from.pitch, to.pitch);
    result.wheel_distance_m = mix(from.wheel_distance_m, to.wheel_distance_m);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_consumer_uses_the_same_frame_sample() {
        let now = Instant::now();
        let mut track = Track::new(TrainMotion::default(), now);
        let received = now + std::time::Duration::from_millis(20);
        let motion = TrainMotion {
            translation: [0.2, 0.0, 0.0],
            ..Default::default()
        };
        track.push(motion, received);
        let mut session = super::super::ClientSession::default();
        session.train_motion_tracks.insert(1, track);
        session.train_display_time =
            now + PRESENTATION_DELAY + std::time::Duration::from_millis(10);
        let vehicle = terra_core::trains::VehicleView {
            steam: Default::default(),
            name: String::new(),
            id: 1,
            owner: terra_core::PlayerUuid::nil(),
            kind: Default::default(),
            anchor: [0; 3],
            parts: Vec::new(),
            blocks: Default::default(),
            couplings: [None; 2],
            revision: 1,
            mass_kg: 0,
            motion,
        };
        for _ in 0..100 {
            assert!((session.displayed_train_motion(&vehicle).translation[0] - 0.1).abs() < 1e-9);
        }
        session.train_display_time += std::time::Duration::from_millis(5);
        assert!((session.displayed_train_motion(&vehicle).translation[0] - 0.15).abs() < 1e-9);
    }
    #[test]
    fn motion_is_continuous_between_packets_and_snaps_teleports() {
        let now = Instant::now();
        let mut track = Track::new(TrainMotion::default(), now);
        let later = now + std::time::Duration::from_millis(20);
        let motion = TrainMotion {
            translation: [0.2, 0.0, 0.0],
            wheel_distance_m: 0.2,
            ..Default::default()
        };
        track.push(motion, later);
        assert_eq!(track.at(later).translation, [0.0; 3]);
        let middle = track.at(now + PRESENTATION_DELAY + std::time::Duration::from_millis(10));
        assert!((middle.translation[0] - 0.1).abs() < 1e-9);
        assert!((middle.wheel_distance_m - 0.1).abs() < 1e-9);
        let jump = TrainMotion {
            translation: [100.0; 3],
            ..motion
        };
        track.push(jump, later + std::time::Duration::from_millis(20));
        assert_eq!(
            track
                .at(later + std::time::Duration::from_millis(20))
                .translation,
            jump.translation
        );
    }
}

#[cfg(test)]
mod angle_tests {
    use super::*;
    #[test]
    fn presentation_crosses_the_yaw_seam_without_a_full_turn() {
        let now = Instant::now();
        let before = TrainMotion {
            yaw: std::f64::consts::PI - 0.01,
            ..Default::default()
        };
        let after = TrainMotion {
            yaw: -std::f64::consts::PI + 0.01,
            ..before
        };
        let mut track = Track::new(before, now);
        let received = now + std::time::Duration::from_millis(20);
        track.push(after, received);
        let middle = track.at(now + PRESENTATION_DELAY + std::time::Duration::from_millis(10));
        assert!((middle.yaw.abs() - std::f64::consts::PI).abs() < 1e-8);
        assert!(middle.valid());
    }
}

#[cfg(test)]
mod jitter_tests {
    use super::*;
    #[test]
    fn bursty_packets_keep_constant_presented_speed_and_never_extrapolate() {
        let start = Instant::now();
        let mut motion = TrainMotion {
            speed_mps: 10.0,
            brake: false,
            ..Default::default()
        };
        let mut track = Track::new(motion, start);
        let arrivals = [20_u64, 41, 42, 68, 81, 104, 105, 141];
        let mut next = 0;
        let mut previous: Option<f64> = None;
        for millisecond in 0_u64..=190 {
            while next < arrivals.len() && arrivals[next] <= millisecond {
                motion.translation[2] = (next + 1) as f64 * 0.2;
                motion.wheel_distance_m = motion.translation[2];
                track.push(motion, start + Duration::from_millis(arrivals[next]));
                next += 1;
            }
            let pose = track.at(start + Duration::from_millis(millisecond));
            if millisecond >= 80 {
                let expected = (millisecond - 60) as f64 * 0.01;
                assert!(
                    (pose.translation[2] - expected).abs() < 1e-5,
                    "jitter at {millisecond}: {:?}",
                    pose.translation
                );
                if let Some(previous) = previous {
                    assert!((pose.translation[2] - previous - 0.01_f64).abs() < 1e-5);
                }
                previous = Some(pose.translation[2]);
            }
        }
        assert_eq!(
            track.at(start + Duration::from_secs(1)).translation,
            motion.translation
        );
    }
}
