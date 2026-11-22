//! A bounded server-time pose buffer. Snapshot arrival jitter is not animation time.
use bevy::math::Vec3;
use std::{collections::VecDeque, time::Instant};

#[derive(Clone)]
struct Sample {
    time: f64,
    position: Vec3,
    velocity: Vec3,
    on_tread: bool,
}

impl Sample {
    /// Hermite interpolation follows the reported takeoff, turning and braking
    /// velocities. Bound both tangents per axis to the monotone range so a
    /// stale velocity cannot send a boot below the floor or past a wall.
    fn towards(&self, next: &Self, time: f64) -> Vec3 {
        let period = next.time - self.time;
        let t = ((time - self.time) / period).clamp(0., 1.) as f32;
        if period > 0.4
            || (self.velocity.length_squared() < 1e-6 && next.velocity.length_squared() < 1e-6)
        {
            // Throttled actors can cover distance while reporting zero velocity
            // at both sample times. Keep that travel continuous.
            return self.position.lerp(next.position, t);
        }
        let span = next.position - self.position;
        let mut offset = Vec3::ZERO;
        for axis in 0..3 {
            let delta = span[axis];
            if delta.abs() < 1e-6 {
                offset[axis] = t * delta;
                continue;
            }
            let scale = period as f32 / delta;
            let start = (self.velocity[axis] * scale).clamp(0., 3.);
            let end = (next.velocity[axis] * scale).clamp(0., 3.);
            offset[axis] =
                delta * t * (t * (3. - 2. * t) + (1. - t) * ((1. - t) * start - t * end));
        }
        self.position + offset
    }
}

#[derive(Clone)]
struct Correction {
    offset: Vec3,
    start: Instant,
    duration: f32,
}

#[derive(Clone, Default)]
pub struct CreaturePlayback {
    samples: VecDeque<Sample>,
    clock: Option<(f64, Instant)>,
    arrival: Option<Instant>,
    delay: f64,
    jitter: f64,
    correction: Option<Correction>,
}

impl CreaturePlayback {
    pub(super) fn has_samples(&self) -> bool {
        !self.samples.is_empty()
    }

    pub(super) fn push(
        &mut self,
        position: Vec3,
        velocity: Vec3,
        stamp: f64,
        now: Instant,
        on_tread: bool,
    ) {
        let old = self.position_at(now);
        let period = self.samples.back().map_or(0.1, |s| stamp - s.time);
        if period <= 0.0 || !stamp.is_finite() {
            return;
        }
        let discontinuity = self.samples.back().is_some_and(|s| {
            s.position.distance(position)
                > (s.velocity.length().max(velocity.length()) * period as f32 + 0.5)
        });
        if let Some(arrival) = self.arrival {
            let variation = (now.saturating_duration_since(arrival).as_secs_f64() - period).abs();
            self.jitter += (variation.min(0.3) - self.jitter) * 0.2;
        }
        let target_delay = (period.min(0.2) + self.jitter * 2.0 + 0.015).clamp(0.1, 0.4);
        self.delay = if self.clock.is_none() {
            target_delay
        } else {
            self.delay + (target_delay - self.delay).clamp(-0.005, 0.005)
        };
        let clock = self.clock.map_or(stamp, |(time, at)| {
            let predicted = time + now.saturating_duration_since(at).as_secs_f64();
            predicted + (stamp - predicted).clamp(-0.005, 0.005)
        });
        self.clock = Some((clock, now));
        self.arrival = Some(now);
        self.samples.push_back(Sample {
            time: stamp,
            position,
            velocity,
            on_tread,
        });
        if discontinuity {
            self.samples.drain(..self.samples.len() - 1);
            self.clock = Some((stamp + self.delay, now));
        }
        while self.samples.len() > 8 {
            self.samples.pop_front();
        }
        // A late packet can contain several metres of valid movement. Bridge
        // that visual correction instead of replaying it in a 0.2 s burst.
        if let (Some(old), Some(next)) = (old, self.uncorrected(now)) {
            let distance = old.distance(next);
            if distance > 64.0 {
                self.samples.drain(..self.samples.len() - 1);
                self.clock = Some((stamp + self.delay, now));
                self.correction = None;
            } else if distance > 0.5
                && (discontinuity
                    || self.correction.as_ref().is_none_or(|c| {
                        now.saturating_duration_since(c.start).as_secs_f32() >= c.duration
                    }))
            {
                self.correction = Some(Correction {
                    offset: old - next,
                    start: now,
                    duration: (distance / velocity.length().max(2.0)).clamp(0.2, 1.5),
                });
            }
        }
    }

    pub fn position_at(&self, now: Instant) -> Option<Vec3> {
        let position = self.uncorrected(now)?;
        let Some(correction) = &self.correction else {
            return Some(position);
        };
        let t = (now
            .saturating_duration_since(correction.start)
            .as_secs_f32()
            / correction.duration)
            .clamp(0.0, 1.0);
        let ease = t * t * (3.0 - 2.0 * t);
        Some(position + correction.offset * (1.0 - ease))
    }

    fn uncorrected(&self, now: Instant) -> Option<Vec3> {
        let (clock, at) = self.clock?;
        let time = clock + now.saturating_duration_since(at).as_secs_f64() - self.delay;
        let first = self.samples.front()?;
        if time <= first.time {
            return Some(first.position);
        }
        for index in 1..self.samples.len() {
            let a = &self.samples[index - 1];
            let b = &self.samples[index];
            if time <= b.time {
                return Some(a.towards(b, time));
            }
        }
        let last = self.samples.back()?;
        let ahead = (time - last.time).clamp(0.0, 0.12) as f32;
        // A grounded dragon reports the displacement across the previous stair
        // as vertical velocity. It has already reached the new tread.
        let extrapolate = if last.on_tread {
            last.velocity.with_y(0.)
        } else {
            last.velocity
        };
        Some(last.position + (extrapolate * ahead).clamp_length_max(0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn slowing_into_a_turn_follows_both_tangents_and_stays_inside_the_sample_bounds() {
        let a = Sample {
            position: Vec3::ZERO,
            velocity: Vec3::new(10., -4., 0.),
            time: 0.,
            on_tread: false,
        };
        let b = Sample {
            position: Vec3::new(1., 0., -1.),
            velocity: Vec3::new(0., 8., -10.),
            time: 0.2,
            on_tread: false,
        };
        let mid = a.towards(&b, 0.1);
        assert!(mid.x > 0.7 && mid.z > -0.3);
        for i in 0..=100 {
            let p = a.towards(&b, f64::from(i) * 0.002);
            assert!((0.0..=1.).contains(&p.x) && (-1.0..=0.).contains(&p.z));
            assert_eq!(p.y, 0.);
        }
    }

    #[test]
    fn delayed_bursts_keep_a_bounded_buffer_and_continuous_presented_motion() {
        let start = Instant::now();
        let mut playback = CreaturePlayback::default();
        let mut previous = Vec3::ZERO;
        let mut sequence = 0;
        for frame in 0..600 {
            let t = frame as f64 / 60.0;
            let now = start + Duration::from_secs_f64(t);
            if frame % 6 == 0 && !(120..=168).contains(&frame) {
                sequence += 1;
                playback.push(Vec3::X * t as f32 * 1.5, Vec3::X * 1.5, t, now, false);
            }
            let position = playback.position_at(now).unwrap();
            assert!(position.is_finite());
            assert!(
                position.distance(previous) < 0.16,
                "frame {frame}: {position:?} from {previous:?}"
            );
            previous = position;
            assert!(playback.samples.len() <= 8);
        }
        assert!(sequence > 60 && previous.x > 14.0);
        let a = playback
            .position_at(start + Duration::from_secs(20))
            .unwrap();
        let b = playback
            .position_at(start + Duration::from_secs(21))
            .unwrap();
        assert_eq!(a, b, "missing snapshots must stop extrapolation");
    }

    #[test]
    fn grounded_stair_motion_keeps_the_last_contact_height_while_waiting_for_a_packet() {
        let start = Instant::now();
        for direction in [-1., 1.] {
            let mut playback = CreaturePlayback::default();
            playback.push(Vec3::ZERO, Vec3::X * 4., 0., start, true);
            let point = Vec3::new(0.4, direction, 0.);
            playback.push(
                point,
                Vec3::new(4., direction * 10., 0.),
                0.1,
                start + Duration::from_millis(100),
                true,
            );
            let hold = playback
                .position_at(start + Duration::from_millis(330))
                .unwrap();
            assert_eq!(hold.y, point.y);
            assert!(hold.x > point.x);
        }
    }

    #[test]
    fn eight_meter_late_correction_does_not_snap_or_restart_on_each_packet() {
        let start = Instant::now();
        let mut p = CreaturePlayback::default();
        p.push(Vec3::ZERO, Vec3::ZERO, 0.0, start, false);
        let at = start + Duration::from_millis(100);
        p.push(Vec3::X * 8.0, Vec3::ZERO, 0.1, at, false);
        let mut previous = p.position_at(at).unwrap();
        assert!(previous.x < 0.1);
        for frame in 1..=180 {
            let now = at + Duration::from_secs_f64(frame as f64 / 60.0);
            if frame % 6 == 0 {
                p.push(
                    Vec3::X * 8.0,
                    Vec3::ZERO,
                    0.1 + frame as f64 / 60.0,
                    now,
                    false,
                );
            }
            let next = p.position_at(now).unwrap();
            assert!(next.distance(previous) < 0.2);
            previous = next;
        }
        assert!((previous.x - 8.0).abs() < 0.01);
    }

    #[test]
    fn successive_large_corrections_during_recovery_do_not_reintroduce_teleports() {
        let start = Instant::now();
        let mut p = CreaturePlayback::default();
        p.push(Vec3::ZERO, Vec3::ZERO, 0.0, start, false);
        let mut previous = Vec3::ZERO;
        let mut target = 0.0;
        for frame in 1..=240 {
            let t = frame as f64 / 60.0;
            let now = start + Duration::from_secs_f64(t);
            if matches!(frame, 6 | 18 | 30) {
                target += 8.0;
            }
            if frame % 6 == 0 {
                p.push(Vec3::X * target, Vec3::ZERO, t, now, false);
            }
            let next = p.position_at(now).unwrap();
            assert!(next.x >= previous.x - 0.001);
            assert!(
                next.distance(previous) < 0.5,
                "frame {frame}: {previous:?} -> {next:?}"
            );
            previous = next;
        }
        assert!((previous.x - 24.0).abs() < 0.01);
    }
}
