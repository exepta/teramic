//! Presentation of the real ballistic arc; never moves the physical horse/seat.
use bevy::prelude::*;
use terra_core::creatures::{Creature, CreatureBehavior};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct JumpPose {
    pub pitch: f32,
    pub front: f32,
    pub rear: f32,
    pub blend: f32,
    landing: f32,
    airborne: bool,
}

impl JumpPose {
    pub fn advance(&mut self, horse: &Creature, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let airborne = horse.behavior == CreatureBehavior::Leap;
        if self.airborne && !airborne {
            self.landing = 1.0;
        }
        self.airborne = airborne;
        self.landing *= (-dt * 9.0).exp();
        let launch = horse
            .horse_traits()
            .map_or(7.0, |t| (24.0 * t.jump_height).sqrt());
        let velocity = (horse.velocity[1] / launch).clamp(-1.0, 1.0);
        let phase = ((1.0 - velocity) * 0.5).clamp(0.0, 1.0);
        let tuck = (phase * std::f32::consts::PI).sin().max(0.0);
        let target = if airborne {
            [
                -velocity * 0.16,
                -0.20 - tuck * 0.90 + phase.powi(3) * 0.5,
                0.28 + tuck * 0.58 - phase.powi(3) * 1.0,
                1.0,
            ]
        } else {
            [
                self.landing * 0.045,
                self.landing * 0.12,
                -self.landing * 0.10,
                0.0,
            ]
        };
        let alpha = 1.0 - (-dt * 18.0).exp();
        self.pitch += (target[0] - self.pitch) * alpha;
        self.front += (target[1] - self.front) * alpha;
        self.rear += (target[2] - self.rear) * alpha;
        self.blend += (target[3] - self.blend) * alpha;
    }

    pub fn orientation(self, yaw: f32) -> Quat {
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(self.pitch)
    }
}

pub(crate) fn seat_pivot_origin(horse: &Creature, feet: Vec3, orientation: Quat) -> Vec3 {
    let pivot = Vec3::Y * (horse.size()[1] * 0.72) as f32;
    feet + pivot - orientation * pivot
}

#[cfg(test)]
mod tests {
    use super::*;
    fn horse() -> Creature {
        let kind = terra_core::creatures::CreatureKind::Horse;
        Creature {
            id: (1 << 62) + 400,
            kind,
            home: [0, 0],
            position: [0.0, 1.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(kind),
        }
    }
    #[test]
    fn pitch_keeps_the_real_seat_fixed_for_all_yaws_and_takeoff_landing_angles() {
        let horse = horse();
        let feet = Vec3::new(-16.5, 20.0, -32.5);
        let seat = Vec3::Y * (horse.size()[1] * 0.72) as f32;
        for yaw in [0.0, 0.8, -2.0] {
            for pitch in [-0.16, 0.0, 0.16] {
                let rotation = JumpPose { pitch, ..default() }.orientation(yaw);
                assert!(
                    (seat_pivot_origin(&horse, feet, rotation) + rotation * seat)
                        .abs_diff_eq(feet + seat, 0.00001)
                );
            }
        }
    }

    #[test]
    fn real_vertical_velocity_drives_distinct_smooth_takeoff_apex_descent_and_landing() {
        let mut horse = horse();
        let launch = (24.0 * horse.horse_traits().unwrap().jump_height).sqrt();
        let mut pose = JumpPose::default();
        let mut samples = Vec::new();
        horse.behavior = CreatureBehavior::Leap;
        for speed in [launch, 0.0, -launch] {
            horse.velocity[1] = speed;
            for _ in 0..20 {
                pose.advance(&horse, 1.0 / 60.0);
            }
            samples.push(pose);
        }
        assert!(samples[0].pitch < -0.1 && samples[2].pitch > 0.1);
        assert!(samples[1].front < samples[0].front - 0.4);
        assert!(samples[1].rear > samples[2].rear + 0.4);
        horse.behavior = CreatureBehavior::Rest;
        horse.velocity = [0.0; 3];
        let previous = pose.pitch;
        pose.advance(&horse, 1.0 / 60.0);
        assert!((pose.pitch - previous).abs() < 0.06);
        for _ in 0..90 {
            pose.advance(&horse, 1.0 / 60.0);
        }
        assert!(pose.pitch.abs() < 0.001 && pose.blend < 0.001);
    }
}
