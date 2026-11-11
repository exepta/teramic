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
    Buffer,
    /// Unsafe curvature: remain on the last validated pose with the brake applied.
    Derailed,
    BuilderMaterials,
    BuilderConsist,
    BuilderBlocked,
    BuilderHeight,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RailCursor {
    pub anchor: [i64; 3],
    pub path: u8,
    pub distance: f64,
    pub forward: bool,
}
/// Persisted excavation dimensions. A zero height follows the locomotive envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RailClearance {
    pub width: u8,
    pub height: u8,
}
impl RailClearance {
    pub const MIN_WIDTH: u8 = 3;
    pub const MAX_WIDTH: u8 = 15;
    pub const MIN_HEIGHT: u8 = 3;
    pub const MAX_HEIGHT: u8 = 32;
    pub fn valid(self) -> bool {
        (Self::MIN_WIDTH..=Self::MAX_WIDTH).contains(&self.width)
            && self.width % 2 == 1
            && (self.height == 0 || (Self::MIN_HEIGHT..=Self::MAX_HEIGHT).contains(&self.height))
    }
}
impl Default for RailClearance {
    fn default() -> Self {
        Self {
            width: 5,
            height: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrainMotion {
    /// Persisted work mode; the authority validates the fitted attachment and consist.
    pub rail_builder_active: bool,
    /// Excavated resources are routed into coupled freight storage before dropping overflow.
    pub rail_builder_autoload: bool,
    pub rail_clearance: RailClearance,
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
            rail_builder_active: false,
            rail_builder_autoload: false,
            rail_clearance: RailClearance::default(),
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
        self.rail_clearance.valid()
            && self
                .translation
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_980_000.0)
            && self.pivot.iter().all(|v| v.is_finite() && v.abs() <= 128.0)
            && self.yaw.is_finite()
            && self.yaw.abs() <= std::f64::consts::TAU + 1e-6
            && self.pitch.is_finite()
            && self.pitch.abs() <= std::f64::consts::FRAC_PI_2
            && self.speed_mps.is_finite()
            && self.speed_mps.abs() <= 96.0
            && (-6..=6).contains(&self.throttle)
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
    pub steam: crate::train_power::SteamState,
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
        f64::from(throttle.signum()) * limit
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
    (speed + delta).clamp(-96.0, 96.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clearance_rejects_even_or_unbounded_width_and_invalid_manual_height() {
        assert!(RailClearance::default().valid());
        for (width, height) in [(3, 3), (15, 32), (5, 0)] {
            assert!(RailClearance { width, height }.valid());
        }
        for (width, height) in [(0, 9), (4, 9), (17, 9), (5, 1), (5, 33)] {
            let size = RailClearance { width, height };
            assert!(!size.valid());
            assert!(
                !TrainMotion {
                    rail_clearance: size,
                    ..Default::default()
                }
                .valid()
            );
        }
    }
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
    #[test]
    fn discrete_gears_accelerate_smoothly_and_downshifts_never_snap_speed() {
        for gear in 1..=6 {
            let target = crate::train_power::gear_speed(gear, 6);
            let mut speed = 0.0;
            for _ in 0..2000 {
                speed = speed_step(speed, gear, false, true, target, 0.1);
            }
            assert!((speed - target).abs() < 1e-9);
        }
        let reduced = speed_step(50.0, 1, false, true, 4.0, 0.1);
        assert!(reduced > 49.0 && reduced < 50.0);
        assert!(speed_step(10.0, -2, false, true, 10.0, 0.1) > 0.0);
    }
}
