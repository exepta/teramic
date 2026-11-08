//! Persistent rail motion and deterministic rigid transforms, independent of the runtime ECS.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    #[default]
    Ready,
    Braking,
    NoFuel,
    NoEngine,
    EndOfTrack,
    Unloaded,
    Obstructed,
    InvalidTrack,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RailCursor {
    pub anchor: [i64; 3],
    pub path: u8,
    pub distance: f64,
    pub forward: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrainMotion {
    pub translation: [f64; 3],
    pub pivot: [f64; 3],
    pub yaw: f64,
    pub pitch: f64,
    pub speed_mps: f64,
    pub throttle: i8,
    pub brake: bool,
    pub wheel_distance_m: f64,
    pub fuel_available: bool,
    pub stop_reason: StopReason,
    pub cursor: Option<RailCursor>,
}
impl Default for TrainMotion {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            pivot: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            speed_mps: 0.0,
            throttle: 0,
            brake: true,
            wheel_distance_m: 0.0,
            fuel_available: false,
            stop_reason: StopReason::Ready,
            cursor: None,
        }
    }
}
impl TrainMotion {
    pub fn valid(self) -> bool {
        self.translation
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_980_000.0)
            && self.pivot.iter().all(|v| v.is_finite() && v.abs() <= 128.0)
            && self.yaw.is_finite()
            && self.yaw.abs() <= std::f64::consts::TAU + 1e-6
            && self.pitch.is_finite()
            && self.pitch.abs() <= std::f64::consts::FRAC_PI_2
            && self.speed_mps.is_finite()
            && self.speed_mps.abs() <= 64.0
            && (-3..=3).contains(&self.throttle)
            && self.wheel_distance_m.is_finite()
            && self.wheel_distance_m.abs() <= 1e12
            && self.cursor.is_none_or(|cursor| {
                cursor.path < 4
                    && cursor.distance.is_finite()
                    && (0.0..=512.0).contains(&cursor.distance)
                    && cursor.anchor.iter().all(|v| v.unsigned_abs() <= 999_000)
            })
    }
    pub fn stopped(self) -> bool {
        self.speed_mps.abs() < 0.001
    }
    pub fn rotate(self, v: [f64; 3], build_yaw: f64) -> [f64; 3] {
        let v = yaw(v, -build_yaw);
        let (s, c) = self.pitch.sin_cos();
        yaw(
            [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c],
            self.yaw + build_yaw,
        )
    }
    pub fn inverse(self, v: [f64; 3], build_yaw: f64) -> [f64; 3] {
        let v = yaw(v, -self.yaw - build_yaw);
        let (s, c) = self.pitch.sin_cos();
        yaw([v[0], v[1] * c + v[2] * s, -v[1] * s + v[2] * c], build_yaw)
    }
    pub fn quaternion(self, build_yaw: f64) -> [f64; 4] {
        fn multiply(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
            [
                a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
                a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
                a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
                a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
            ]
        }
        let (a, b) = ((self.yaw + build_yaw) * 0.5).sin_cos();
        let (c, d) = (self.pitch * 0.5).sin_cos();
        let (e, f) = (-build_yaw * 0.5).sin_cos();
        multiply(
            multiply([0.0, a, 0.0, b], [c, 0.0, 0.0, d]),
            [0.0, e, 0.0, f],
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainMotionFrame {
    pub id: u64,
    pub revision: u64,
    pub motion: TrainMotion,
}

fn yaw(v: [f64; 3], angle: f64) -> [f64; 3] {
    let (s, c) = angle.sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}
/// Throttle reversal first brakes to zero. Positive limits are live rail-bed limits.
pub fn speed_step(
    speed: f64,
    throttle: i8,
    brake: bool,
    powered: bool,
    limit: f64,
    dt: f64,
) -> f64 {
    if ![speed, limit, dt].iter().all(|v| v.is_finite())
        || limit <= 0.0
        || !(0.0..=0.25).contains(&dt)
    {
        return 0.0;
    }
    let target = if powered && !brake {
        f64::from(throttle.clamp(-3, 3)) * limit / 3.0
    } else {
        0.0
    };
    let decelerating = brake || !powered || speed * target < 0.0 || target.abs() < speed.abs();
    let rate = if brake {
        6.0
    } else if decelerating {
        2.5
    } else {
        1.25
    };
    let delta = (target - speed).clamp(-rate * dt, rate * dt);
    (speed + delta).clamp(-limit, limit)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rigid_roundtrip_for_every_build_axis_and_grade() {
        for axis in 0..4 {
            let angle = f64::from(axis) * std::f64::consts::FRAC_PI_2;
            let motion = TrainMotion {
                yaw: 0.7,
                pitch: -0.3,
                ..Default::default()
            };
            let point = [2.7, -1.5, 9.1];
            let restored = motion.inverse(motion.rotate(point, angle), angle);
            assert!(
                point
                    .iter()
                    .zip(restored)
                    .all(|(a, b)| (a - b).abs() < 1e-10)
            );
        }
    }
    #[test]
    fn braking_fuel_loss_and_reversal_do_not_add_energy() {
        assert!(speed_step(8.0, 3, true, true, 12.0, 0.1) < 8.0);
        assert!(speed_step(8.0, 3, false, false, 12.0, 0.1) < 8.0);
        assert!(speed_step(8.0, -3, false, true, 12.0, 0.1) < 8.0);
        assert_eq!(speed_step(f64::NAN, 3, false, true, 12.0, 0.1), 0.0);
    }
}
