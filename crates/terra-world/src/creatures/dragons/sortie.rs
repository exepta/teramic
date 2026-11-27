//! A combat pass starts with separation, turns back to breathe and ends on a surveyed tread.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum AttackRun {
    Climb { waypoint: DVec3, remaining: f64 },
    Pass,
    Touchdown,
}

impl Encounter {
    pub(super) fn steer_attack_run(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        target: DVec3,
        dt: f64,
    ) -> bool {
        if !pose::flies(a.kind) {
            return false;
        }
        let from = DVec3::from_array(a.position);
        match self.attack_run {
            Some(AttackRun::Climb {
                waypoint,
                remaining,
            }) if self.airborne => {
                // Get to one side of a close opponent before banking in. Climbing straight
                // over its head gives the mouth no horizontal direction to aim along.
                if from.distance_squared(waypoint) > 9. && remaining > 0. {
                    self.attack_run = Some(AttackRun::Climb {
                        waypoint,
                        remaining: remaining - dt,
                    });
                    self.travel(a, h, waypoint, dt, 12.);
                    return true;
                }
                self.attack_run = Some(AttackRun::Pass);
                self.steer_time = 0.;
            }
            Some(AttackRun::Climb { .. }) => {
                // A ceiling or newly placed block interrupted the launch.
                self.attack_run = None;
                self.launch_retry = 3.;
            }
            Some(AttackRun::Touchdown) if self.airborne => {
                let outward = (from - target).with_y(0.).normalize_or(DVec3::Z);
                let pad = target + outward * 8.;
                self.attack_pad = Some((pad, outward, 0.));
                self.approach_pad(a, h, pad, outward, dt);
                return true;
            }
            None if !self.airborne
                && self.launch_retry <= 0.
                && self.ground_casts
                    >= 2 + ((a.id ^ u64::from(self.attack_number / 2)) % 2) as u8 =>
            {
                self.launch_retry = 2.5;
                if on_ground(a, h) && flight::can_launch(a, h) {
                    let (s, c) = f64::from(a.yaw).sin_cos();
                    let forward = DVec3::new(-s, 0., -c);
                    let side = DVec3::new(-forward.z, 0., forward.x)
                        * if self.attack_number.is_multiple_of(2) {
                            1.
                        } else {
                            -1.
                        };
                    self.attack_run = Some(AttackRun::Climb {
                        waypoint: from + forward * 10. + side * 12. + DVec3::Y * 11.,
                        remaining: 3.6,
                    });
                    self.ground_casts = 0;
                    self.steer_time = 0.;
                    set_pose(a, B::Leap);
                    return true;
                }
            }
            _ => {}
        }
        false
    }
}
