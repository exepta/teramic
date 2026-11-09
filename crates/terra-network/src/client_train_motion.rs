//! Presentation-only smoothing; confirmed collision and transaction state stay authoritative.
use std::time::Instant;
use terra_core::train_motion::TrainMotion;
#[derive(Clone)]
pub(super) struct Track {
    from: TrainMotion,
    to: TrainMotion,
    received: Instant,
    interval: f64,
}
impl Track {
    pub fn new(motion: TrainMotion, now: Instant) -> Self {
        Self {
            from: motion,
            to: motion,
            received: now,
            interval: 1.0 / 60.0,
        }
    }
    pub fn push(&mut self, motion: TrainMotion, now: Instant) {
        if self.to == motion {
            return;
        }
        let elapsed = now.duration_since(self.received).as_secs_f64();
        let jump = (0..3)
            .map(|axis| (motion.translation[axis] - self.to.translation[axis]).powi(2))
            .sum::<f64>()
            > 16.0;
        self.from = if jump || self.to.pivot != motion.pivot || elapsed > 0.25 {
            motion
        } else {
            self.at(now)
        };
        self.to = motion;
        self.received = now;
        self.interval = elapsed.clamp(1.0 / 120.0, 0.1);
    }
    pub fn at(&self, now: Instant) -> TrainMotion {
        let t = (now.duration_since(self.received).as_secs_f64() / self.interval).clamp(0.0, 1.0);
        let mut result = self.to;
        let mix = |a: f64, b: f64| a + (b - a) * t;
        result.translation =
            std::array::from_fn(|axis| mix(self.from.translation[axis], self.to.translation[axis]));
        let angle = (self.to.yaw - self.from.yaw + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        result.yaw = (self.from.yaw + angle * t + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        result.pitch = mix(self.from.pitch, self.to.pitch);
        result.wheel_distance_m = mix(self.from.wheel_distance_m, self.to.wheel_distance_m);
        result
    }
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
        session.train_display_time = received + std::time::Duration::from_millis(10);
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
        let middle = track.at(later + std::time::Duration::from_millis(10));
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
        let middle = track.at(received + std::time::Duration::from_millis(10));
        assert!((middle.yaw.abs() - std::f64::consts::PI).abs() < 1e-8);
        assert!(middle.valid());
    }
}
