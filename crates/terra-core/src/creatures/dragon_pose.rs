//! Shared articulated dragon skeleton in neutral model metres, with -Z forward.
use super::{Creature, CreatureBehavior as B, dragons};
use bevy_math::{Isometry3d, Quat, Vec3};
use std::f32::consts::PI;
const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joint {
    Trunk,
    Neck,
    Skull,
    Eyes,
    Frill(i8),
    Jaw,
    Tail(u8),
    Thigh { side: i8, hind: bool },
    Foot { side: i8, hind: bool },
    Paw { side: i8, hind: bool },
    WingArm(i8),
    Sail(i8),
    Breath,
    Impact,
    Rune,
    Missile(u8),
    Ward,
    Surge,
}

impl Joint {
    pub fn pivot(self) -> Vec3 {
        match self {
            Self::Trunk => v(0., 4.2, 0.),
            Self::Neck => v(0., 5.2, -2.5),
            Self::Skull | Self::Eyes => v(0., 8.15, -5.65),
            Self::Frill(s) => v(f32::from(s) * 1.08, 8.58, -6.48),
            Self::Jaw => v(0., 8.02, -6.45),
            Self::Tail(i) => tail_node(i),
            Self::WingArm(s) | Self::Sail(s) => v(f32::from(s) * 1.82, 5.65, -1.85),
            Self::Thigh { side, hind } => v(
                f32::from(side) * if hind { 1.85 } else { 1.9 },
                if hind { 4.05 } else { 4.75 },
                if hind { 2.65 } else { -2.15 },
            ),
            Self::Foot { side, hind } => v(
                f32::from(side) * if hind { 2.55 } else { 2.44 },
                if hind { 2.1 } else { 2.35 },
                if hind { 1.85 } else { -1.64 },
            ),
            Self::Paw { side, hind } => v(
                f32::from(side) * 2.65,
                0.67,
                if hind { 2.72 } else { -2.93 },
            ),
            Self::Breath => v(0., 8.48, -9.0),
            Self::Impact | Self::Rune | Self::Missile(_) | Self::Ward | Self::Surge => {
                Vec3::Y * 0.11
            }
        }
    }

    pub fn is_effect(self) -> bool {
        matches!(
            self,
            Self::Breath | Self::Impact | Self::Rune | Self::Missile(_) | Self::Ward | Self::Surge
        )
    }
}

pub const TAIL_BONES: u8 = 6;

pub fn tail_node(i: u8) -> Vec3 {
    let t = f32::from(i) / f32::from(TAIL_BONES);
    v(
        (t * 2.8).sin() * 1.55,
        3.67 - (t * 1.9).sin() * 2.55 + t.powi(3) * 0.62,
        3.72 + t * 14.65,
    )
}

pub fn wrist(side: f32) -> Vec3 {
    v(side * 9.3, 10.65, -3.2)
}

fn ease(x: f32) -> f32 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn pulse(p: f32, middle: f32, width: f32) -> f32 {
    ease(1. - ((p - middle) / width).abs())
}

/// Both picking and rendering evaluate this pose from replicated values. The client
/// may extrapolate its clock by a fraction of the most recent snapshot interval.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub behavior: B,
    pub kind: super::CreatureKind,
    pub progress: f32,
    pub lineage: Option<dragons::Lineage>,
    pub enraged: bool,
    pub clock: f32,
    airborne: f32,
    asleep: f32,
    walking: f32,
    breath: f32,
    smash: f32,
    roar: f32,
    swim: f32,
    collapse: f32,
    wingbeat: f32,
    wristbeat: f32,
    recovery: f32,
    stroke: f32,
    bank: f32,
    pitch: f32,
    aim_yaw: f32,
    aim_pitch: f32,
    gait_phase: f32,
    youth: f32,
    feeding: f32,
}
fn clock(age: f64, id: u64) -> f32 {
    // All angular frequencies use hundredths of a radian per second.
    // Reducing by their common period preserves every joint's phase.
    (age + (id % 23) as f64 * 0.37).rem_euclid(std::f64::consts::TAU * 100.) as f32
}

/// One full downstroke per cycle, shared with the positional wing Foley.
pub fn wing_angle(a: &Creature, age: f64) -> f32 {
    let rate = match a.kind {
        super::CreatureKind::EarthDragon => 3.16,
        super::CreatureKind::WaterDragon => 3.62,
        _ => 3.32,
    };
    (clock(age, a.id) * rate).rem_euclid(2. * PI)
}

// Start the audible beat during the loaded part of the stroke. The long
// recovery has time for the elbow to sweep back and the sail to feather.
fn wing_stroke(angle: f32) -> (f32, f32) {
    let cycle = (angle / (2. * PI) + 0.16).rem_euclid(1.);
    if cycle < 0.38 {
        ((PI * cycle / 0.38).cos(), 0.)
    } else {
        let recovery = PI * (cycle - 0.38) / 0.62;
        (-recovery.cos(), recovery.sin().powi(2))
    }
}

/// Climbing and hovering need lift. Fast level flight alternates a short set of
/// strokes with a glide; a descending dragon can keep its wings spread.
pub fn stroke_strength(a: &Creature, age: f64) -> f32 {
    flight_effort(a, age, a.velocity)
}

fn flight_effort(a: &Creature, age: f64, velocity: [f32; 3]) -> f32 {
    let speed = velocity[0].hypot(velocity[2]);
    let lift = (velocity[1] / 5.).clamp(0., 1.);
    let hover = (1. - speed / 7.).clamp(0., 1.);
    let rate = match a.kind {
        super::CreatureKind::EarthDragon => 0.77,
        super::CreatureKind::WaterDragon => 1.08,
        _ => 0.89,
    };
    let burst = ease(((clock(age, a.id) * rate).sin() + 0.38) / 0.85);
    let dive = 1. - 0.89 * ease(-velocity[1] / 4.);
    (burst * dive).max(lift).max(hover * 0.86)
}

/// Change in flight velocity during one stroke. The loaded downstroke peaks
/// just after midstroke. Each impulse balances across a cycle at constant effort.
pub fn wing_impulse(a: &Creature, velocity: [f32; 3]) -> [f32; 2] {
    let phase = wing_angle(a, a.age);
    let load = flight_effort(a, a.age, velocity);
    [
        load * (phase - 0.70).cos() * 1.45,
        load * (phase - 0.94).cos() * 0.74,
    ]
}

impl Pose {
    pub fn of(a: &Creature, since: f32) -> Self {
        let b = a.behavior;
        let lineage = dragons::lineage(a.kind, a.variant);
        let p = dragons::phase(b, a.timer - since);
        // Keep precision on long-lived entities while retaining an independent breath cycle.
        let age = a.age + f64::from(since);
        let clock = clock(age, a.id);
        let ground_speed = a.velocity[0].hypot(a.velocity[2]);
        let walking = if matches!(b, B::Wander | B::Hunt) {
            (ground_speed / 4.2).clamp(0., 1.)
        } else {
            0.
        };
        let airborne = match b {
            B::Fly => 1.,
            _ if dragons::casting(b)
                && a.needs
                    .goal
                    .is_some_and(|g| g.kind == super::NeedKind::AirborneAttackAim) =>
            {
                1.
            }
            B::Leap => ease((p - 0.15) / 0.65),
            B::Land => 1. - ease(p / 0.82),
            _ => 0.,
        };
        let angle = wing_angle(a, age);
        // A loaded downstroke is quicker than recovery. The wrist trails the shoulder.
        let (mut beat, recovery) = wing_stroke(angle);
        let (wristbeat, _) = wing_stroke(angle - 0.62);
        if b == B::Leap {
            beat = beat * ease(p) + (p * 2. * PI - 0.9).sin() * (1. - ease(p));
        }
        let collapse = if b == B::Dead { ease(p / 0.85) } else { 0. };
        let (sin, cos) = a.yaw.sin_cos();
        let lateral = a.velocity[0] * cos - a.velocity[2] * sin;
        let bank = (lateral / (ground_speed + 1.3)).clamp(-0.82, 0.82) * airborne;
        let pitch = a.velocity[1]
            .atan2(ground_speed.max(4.2))
            .clamp(-0.54, 0.60)
            * airborne;
        let aim = a.needs.goal.filter(|g| {
            matches!(
                g.kind,
                super::NeedKind::AttackAim | super::NeedKind::AirborneAttackAim
            ) && matches!(b, B::Spit | B::DragonVolley)
        });
        let (aim_yaw, aim_pitch) = aim.map_or((0., 0.), |goal| {
            let head = Joint::Skull.pivot()
                * dragons::model_scale(a.kind, a.variant)
                * a.growth_scale() as f32;
            let delta = Vec3::from_array(std::array::from_fn(|i| {
                (goal.position[i] as f64 + 0.5 - a.position[i]) as f32
            }));
            let local = Quat::from_rotation_y(-a.yaw) * delta - head;
            (
                (-local.x).atan2(-local.z).clamp(-0.65, 0.65),
                local.y.atan2(local.x.hypot(local.z)).clamp(-0.80, 0.55),
            )
        });
        Self {
            behavior: b,
            kind: a.kind,
            youth: (1. - a.growth()).powi(2),
            feeding: if matches!(b, B::Drink | B::Graze) {
                1.
            } else {
                0.
            },
            progress: p,
            lineage,
            enraged: dragons::enraged(a),
            clock,
            airborne,
            asleep: if b == B::Sleep { 1. } else { collapse * 0.79 },
            walking,
            breath: if matches!(b, B::Spit | B::DragonVolley) {
                ease(p / 0.27) * (1. - ease((p - 0.76) / 0.24))
            } else {
                0.
            },
            smash: if b == B::DragonSurge {
                pulse(p, 0.35, 0.28) * 1.25 - pulse(p, 0.66, 0.18)
            } else if b == B::DragonWard {
                -pulse(p, 0.52, 0.49) * 1.2
            } else if matches!(b, B::Attack | B::Tremor) {
                pulse(p, 0.27, 0.27) - 0.64 * pulse(p, 0.60, 0.19)
            } else {
                0.
            },
            roar: if b == B::Emerge {
                pulse(p, 0.50, 0.48)
            } else {
                0.
            },
            swim: if a.kind == super::CreatureKind::WaterDragon {
                if b == B::Swim { 1. } else { 0.40 }
            } else {
                0.
            },
            collapse,
            wingbeat: beat,
            wristbeat,
            recovery,
            stroke: if matches!(b, B::Leap | B::Land) {
                1.
            } else {
                stroke_strength(a, age)
            },
            bank,
            pitch,
            aim_yaw,
            aim_pitch,
            gait_phase: clock
                * (1. + 0.38 * (1. - a.growth()))
                * match a.kind {
                    super::CreatureKind::EarthDragon => 3.75,
                    super::CreatureKind::DarkDragon => 4.0,
                    _ => 4.4,
                },
        }
    }

    /// Smooth changes of load and posture without delaying the strike, breath
    /// or wingbeat clock. Blending the controls keeps the joints attached.
    pub fn settle_from(&mut self, last: Self, dt: f32) {
        let load = 1. - (-dt * 11.).exp();
        let posture = 1. - (-dt * 6.).exp();
        let blend = |from: f32, to: f32, weight: f32| from + (to - from) * weight;
        self.airborne = blend(last.airborne, self.airborne, load);
        self.stroke = blend(last.stroke, self.stroke, load);
        self.bank = blend(last.bank, self.bank, load);
        self.pitch = blend(last.pitch, self.pitch, load);
        self.walking = blend(last.walking, self.walking, load);
        self.swim = blend(last.swim, self.swim, posture);
        self.feeding = blend(last.feeding, self.feeding, posture);
        self.asleep = blend(last.asleep, self.asleep, posture);
    }

    /// The collider advances in ticks; the visible footfall follows the distance
    /// already interpolated on screen. Waiting for another packet cannot restart a step.
    pub fn stride_from(&mut self, last: Self, model_metres: f32, dt: f32) {
        let distance = if matches!(self.behavior, B::Wander | B::Hunt) {
            model_metres.clamp(0., 12. * dt)
        } else {
            0.
        };
        self.gait_phase = (last.gait_phase + distance * 1.42).rem_euclid(2. * PI);
        let load = (distance / (4.4 * dt.max(0.001))).clamp(0., 1.);
        self.walking = last.walking + (load - last.walking) * (1. - (-9. * dt).exp());
    }

    fn child(parent: Isometry3d, parent_joint: Joint, joint: Joint, rotation: Quat) -> Isometry3d {
        parent * Isometry3d::new(joint.pivot() - parent_joint.pivot(), rotation)
    }

    /// Height of the lifted claw during the recovery half of a grounded stride.
    pub fn paw_lift(self, side: i8, hind: bool) -> f32 {
        let phase =
            self.gait_phase + if side < 0 { PI } else { 0. } + if hind { 0.68 * PI } else { 0. };
        phase.cos().max(0.).powi(2) * self.walking * 0.72
    }

    /// Anchor skull, jaw and young wings at the same pivots for graphics and picking.
    pub fn joint_scale(self, joint: Joint) -> Vec3 {
        match joint {
            // The short muzzle and broad cranium give a hatchling its own profile.
            Joint::Skull | Joint::Jaw | Joint::Eyes | Joint::Frill(_) => {
                Vec3::ONE + self.youth * v(0.65, 0.46, -0.24)
            }
            Joint::WingArm(_) | Joint::Sail(_) => {
                // Finger spars draw together along the flank as the shoulder sweeps aft.
                v(0.83, 0.34, 0.32).lerp(Vec3::ONE, self.airborne) * (1. - 0.48 * self.youth)
            }
            Joint::Tail(_) => Vec3::splat(1. - 0.36 * self.youth),
            _ => Vec3::ONE,
        }
    }

    pub fn bone(self, joint: Joint) -> Isometry3d {
        let mut frame = self.rest_bone(joint);
        if self.youth == 0. {
            return frame;
        }
        let anchor = match joint {
            Joint::Tail(i) if i > 0 => Joint::Tail(0),
            Joint::Jaw | Joint::Breath | Joint::Frill(_) => Joint::Skull,
            _ => return frame,
        };
        let attachment = self.rest_bone(anchor);
        let local = Vec3::from(attachment.inverse_transform_point(frame.translation));
        frame.translation = attachment.transform_point(local * self.joint_scale(anchor));
        frame
    }

    /// Normalized immersion of the whole articulated body in its tunnel or shadow.
    pub fn submergence(self) -> f32 {
        let p = self.progress;
        if self.behavior == B::DragonBurrow {
            ease((p - 0.04) / 0.24) * (1. - ease((p - 0.70) / 0.27))
        } else if self.kind == super::CreatureKind::DarkDragon && self.behavior == B::DragonSurge {
            ease((p - 0.13) / 0.27) * (1. - ease((p - 0.64) / 0.25))
        } else {
            0.
        }
    }

    fn rest_bone(self, joint: Joint) -> Isometry3d {
        let p = self.progress;
        let t = self.clock;
        let step = self.gait_phase;
        let land = if self.behavior == B::Land {
            pulse(p, 0.70, 0.25)
        } else {
            0.
        };
        let spring = if self.behavior == B::Leap {
            pulse(p, 0.17, 0.17)
        } else {
            0.
        };
        let breathing = (t * 1.35).sin();
        let coil = match (self.lineage, self.behavior) {
            (Some(dragons::Lineage::Shadow), B::DragonSurge) => {
                0.32 * pulse(p, 0.47, 0.31) - 0.24 * pulse(p, 0.72, 0.16)
            }
            (Some(dragons::Lineage::Tidal), B::Swim) => 0.12 * (t * 2.24).sin(),
            (Some(dragons::Lineage::Stone), B::DragonVolley) => {
                0.18 * pulse(p, 0.22, 0.22) - 0.21 * pulse(p, 0.39, 0.15)
            }
            _ => 0.,
        };
        let body_rot = Quat::from_rotation_x(
            self.pitch - 0.055 * self.airborne * self.stroke * self.wristbeat + 0.17 * self.smash
                - 0.10 * self.airborne
                - 0.11 * self.asleep
                + 0.15 * land
                - 0.09 * spring,
        ) * Quat::from_rotation_z(
            self.bank * 0.83 + step.sin() * 0.049 * self.walking + self.collapse * 0.37,
        );
        let swipe = if self.behavior == B::Attack {
            -0.34 * pulse(p, 0.28, 0.27) + 0.46 * pulse(p, 0.57, 0.18)
        } else {
            0.
        };
        let body_rot =
            body_rot * Quat::from_rotation_y(coil + swipe + self.swim * 0.13 * (t * 2.24).sin());
        let trunk = Isometry3d::new(
            Joint::Trunk.pivot()
                + v(
                    step.sin() * self.walking * 0.085,
                    -self.submergence() * 16.0
                        + 0.035 * breathing
                        + 0.14 * step.cos().powi(2) * self.walking
                        - 1.98 * self.asleep
                        - 0.40 * land
                        - 0.54 * spring
                        - 1.36 * self.swim
                        - 0.42 * self.airborne * self.stroke * self.wristbeat,
                    0.,
                ),
            body_rot,
        );
        let neck_carriage = match self.lineage.or(match self.kind {
            super::CreatureKind::EarthDragon => Some(dragons::Lineage::Stone),
            super::CreatureKind::WaterDragon => Some(dragons::Lineage::Tidal),
            _ => None,
        }) {
            Some(dragons::Lineage::Stone) => {
                -0.48 * (1. - self.roar * 0.7) * (1. - self.asleep * 0.6)
            }
            Some(dragons::Lineage::Tidal) => 0.065 * (t * 1.07).sin() - self.swim * 0.28,
            Some(dragons::Lineage::Shadow) if self.enraged => 0.12 + 0.08 * (t * 4.66).sin(),
            _ => 0.,
        };
        let neck = || {
            Self::child(
                trunk,
                Joint::Trunk,
                Joint::Neck,
                Quat::from_rotation_y(
                    (t * 0.42).sin() * 0.045
                        + 0.37 * self.asleep
                        + self.bank * 0.27
                        + self.breath * self.aim_yaw * 0.60,
                ) * Quat::from_rotation_x(
                    neck_carriage - self.feeding * (0.79 + 0.12 * (t * 3.).sin())
                        + self.aim_pitch * self.breath * 0.65
                        - self.pitch * 0.32
                        + 0.085 * self.airborne * self.stroke * self.wristbeat
                        - 0.77 * self.asleep
                        - 0.45 * self.airborne
                        - 0.36 * self.swim
                        + 0.34 * self.roar
                        - 0.24 * self.breath
                        + 0.30 * self.smash
                        + breathing * 0.014,
                ),
            )
        };
        let skull = || {
            Self::child(
                neck(),
                Joint::Neck,
                Joint::Skull,
                Quat::from_rotation_y(
                    (t * 0.56).sin() * 0.05
                        + 0.19 * self.asleep
                        + self.aim_yaw * self.breath * 0.40,
                ) * Quat::from_rotation_x(
                    self.aim_pitch * self.breath * 0.35
                        + 0.24 * self.asleep
                        + 0.10 * self.airborne
                        + 0.11 * self.swim
                        + 0.23 * self.roar
                        + 0.09 * self.breath
                        - 0.25 * self.smash,
                ),
            )
        };
        match joint {
            Joint::Trunk => trunk,
            Joint::Neck => neck(),
            Joint::Skull | Joint::Eyes => skull(),
            Joint::Frill(side) => {
                use super::CreatureKind as K;
                let side = f32::from(side);
                let threat = self.roar.max(self.breath).max(self.smash.max(0.));
                // The tidal gill fans pump even at rest; a throat fan opens before
                // a flame jet, while the ore plates can only spread a little.
                let motion = match self.kind {
                    K::WaterDragon => {
                        0.10 + (0.13 + 0.07 * self.swim) * (t * 1.84 - 0.32 * side).sin()
                            + 0.35 * threat
                    }
                    K::EarthDragon => 0.025 * breathing + 0.14 * threat,
                    K::DarkDragon => 0.035 * breathing + 0.48 * threat + 0.09 * self.bank * side,
                    _ => 0.055 * breathing + 0.57 * threat,
                } * (1. - self.collapse)
                    - self.asleep * 0.30;
                Self::child(
                    skull(),
                    Joint::Skull,
                    joint,
                    Quat::from_rotation_y(side * motion) * Quat::from_rotation_x(-motion * 0.22),
                )
            }
            Joint::Jaw => {
                let snap = if self.behavior == B::Attack {
                    pulse(p, 0.38, 0.27)
                } else {
                    0.
                };
                let open =
                    0.065 + self.feeding * (0.10 + 0.09 * (t * 6.).sin()) + 0.018 * breathing
                        - 0.05 * self.asleep
                        + 0.63 * self.roar
                        + 0.57 * self.breath
                        + 0.68 * snap
                        + if self.enraged {
                            0.18 + 0.05 * (t * 4.66).sin()
                        } else {
                            0.
                        };
                Self::child(skull(), Joint::Skull, joint, Quat::from_rotation_x(-open))
            }
            Joint::Tail(last) => {
                let mut pose = trunk;
                let mut parent = Joint::Trunk;
                for i in 0..=last {
                    let lag = f32::from(i) * 0.44;
                    let wave = (t * 1.12 - lag).sin() * (1. - self.swim)
                        + (t * 2.24 - lag).sin() * self.swim;
                    let side = wave
                        * (0.055
                            + self.swim
                                * if self.kind == super::CreatureKind::WaterDragon {
                                    0.30
                                } else {
                                    0.16
                                }
                            + self.walking * 0.064)
                        - coil * 0.23
                        + self.asleep * 0.40
                        - self.bank * (0.16 - f32::from(i) * 0.013);
                    let rot = Quat::from_rotation_y(side)
                        * Quat::from_rotation_x(
                            (t * 0.82 - lag).sin() * 0.027
                                + self.airborne
                                    * (0.032 + self.stroke * (t * 3.4 - lag - 0.9).sin() * 0.047)
                                - if i == 0 { self.asleep * 0.18 } else { 0. },
                        );
                    let this = Joint::Tail(i);
                    pose = Self::child(pose, parent, this, rot);
                    parent = this;
                }
                pose
            }
            Joint::WingArm(side) | Joint::Sail(side) => {
                let s = f32::from(side);
                let tucked = 1. - self.airborne;
                let work = self.airborne * self.stroke;
                let recovery = work * self.recovery;
                let lift = -0.10 * tucked
                    + self.airborne
                        * (0.12 + self.wingbeat * 0.71 * self.stroke - s * self.bank * 0.22
                            + 0.24 * land)
                    + 0.012 * breathing;
                let shoulder = Quat::from_rotation_y(
                    -s * (tucked * 1.34 + self.airborne * 0.04 + recovery * 0.51 - work * 0.12),
                ) * Quat::from_rotation_x(0.97 * tucked)
                    * Quat::from_rotation_z(s * lift);
                // Rotate the forearm about the shoulder–elbow line. The elbow
                // stays seated as the wrist sweeps aft during recovery.
                let humerus = (v(s * 4.75, 8.50, 0.33) - joint.pivot()).normalize();
                let shoulder = shoulder * Quat::from_axis_angle(humerus, -s * recovery * 0.77);
                let mut wing = Self::child(trunk, Joint::Trunk, joint, shoulder);
                if matches!(joint, Joint::Sail(_)) {
                    let axis = ((wrist(s) - joint.pivot()) * self.joint_scale(joint)).normalize();
                    let wrist_fold = tucked * 0.23
                        + self.airborne
                            * (0.06 + self.stroke * (0.42 * self.wristbeat + 0.52 * self.recovery)
                                - self.pitch * 0.18);
                    wing.rotation *= Quat::from_axis_angle(axis, -s * wrist_fold);
                }
                wing
            }
            Joint::Thigh { side, hind } | Joint::Foot { side, hind } => {
                let footfall =
                    step + if side < 0 { PI } else { 0. } + if hind { 0.68 * PI } else { 0. };
                let gait = footfall.sin();
                let tuck = self.airborne * 0.84
                    + self.asleep * 0.76
                    + self.swim * 0.85
                    + land * 0.21
                    + spring * 0.24;
                let phi = -gait * self.walking * 0.40
                    + if hind {
                        -0.78 * tuck
                    } else {
                        0.69 * tuck - self.smash * 0.45 - self.roar * 0.12
                    };
                // In cruise the claws stream aft against the belly. As the wings
                // flare, the forelegs swing down first, ready to catch the landing.
                let stream = self.airborne * (1. - land) * (1. - spring);
                let phi = phi - stream * if hind { 0.39 } else { 1.65 };
                let thigh = Joint::Thigh { side, hind };
                let upper = Self::child(trunk, Joint::Trunk, thigh, Quat::from_rotation_x(phi));
                if matches!(joint, Joint::Thigh { .. }) {
                    return upper;
                }
                Self::child(
                    upper,
                    thigh,
                    joint,
                    Quat::from_rotation_x(
                        -phi * 0.52
                            + footfall.cos().max(0.).powi(2) * self.walking * 0.54
                            + if hind { tuck * 1.18 } else { -tuck * 1.04 }
                            - stream * if hind { 0.54 } else { 0.20 },
                    ),
                )
            }
            Joint::Paw { side, hind } => {
                let knee = Joint::Foot { side, hind };
                // Talons fold under the ankle on the upstroke and reach forward
                // as weight returns to the toes; the planted rest pose stays flat.
                let curl = self.airborne * (0.46 + 0.10 * self.wristbeat) + self.asleep * 0.14
                    - spring * 0.14
                    - self.paw_lift(side, hind) * 0.25;
                Self::child(self.bone(knee), knee, joint, Quat::from_rotation_x(-curl))
            }
            Joint::Breath => Self::child(skull(), Joint::Skull, joint, Quat::IDENTITY),
            Joint::Rune | Joint::Impact | Joint::Missile(_) | Joint::Ward | Joint::Surge => {
                Isometry3d::from_translation(joint.pivot())
            }
        }
    }

    pub fn point(self, joint: Joint, rest: Vec3) -> Vec3 {
        self.bone(joint)
            .transform_point((rest - joint.pivot()) * self.joint_scale(joint))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureKind as K, CreatureNeeds, breeding, hitboxes};
    fn dragon(kind: K, b: B) -> Creature {
        Creature {
            id: 23,
            kind,
            home: [0; 2],
            position: [11., 70., 42.],
            velocity: [0., 0., -4.],
            yaw: 0.,
            health: kind.health(),
            behavior: b,
            timer: dragons::duration(b) * 0.42,
            age: breeding::GROWTH_SECONDS + 1.,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }

    #[test]
    fn dragon_skin_stays_attached_through_takeoff_landing_attacks_sleep_and_death() {
        let mut distinct = std::collections::BTreeSet::new();
        for b in [
            B::Rest,
            B::Wander,
            B::Leap,
            B::Fly,
            B::Land,
            B::Swim,
            B::Emerge,
            B::Spit,
            B::Tremor,
            B::Attack,
            B::Ink,
            B::Sleep,
            B::Dead,
        ] {
            let state = Pose::of(&dragon(K::WaterDragon, b), 0.);
            for side in [-1, 1] {
                let arm = Joint::WingArm(side);
                let sail = Joint::Sail(side);
                for p in [arm.pivot(), wrist(f32::from(side))] {
                    assert!(
                        state
                            .point(arm, p)
                            .abs_diff_eq(state.point(sail, p), 0.0001),
                        "{b:?} wing seam"
                    );
                }
                for hind in [true, false] {
                    let knee = Joint::Foot { side, hind };
                    assert!(state.point(knee, knee.pivot()).abs_diff_eq(
                        state.point(Joint::Thigh { side, hind }, knee.pivot()),
                        0.0001
                    ));
                }
            }
            for i in 1..TAIL_BONES {
                let joint = Joint::Tail(i);
                assert!(
                    state
                        .point(joint, joint.pivot())
                        .abs_diff_eq(state.point(Joint::Tail(i - 1), joint.pivot()), 0.0001)
                );
            }
            let key = [Joint::Jaw, Joint::WingArm(1), Joint::Tail(5)]
                .map(|j| (state.bone(j).rotation.x * 10000.) as i32);
            distinct.insert(key);
        }
        assert!(distinct.len() >= 9);
    }

    #[test]
    fn changing_hatchling_proportions_keep_the_muzzle_hinge_wings_and_tail_attached() {
        let mut a = dragon(K::WaterDragon, B::Graze);
        a.id = breeding::next_birth_id(&Default::default(), 0).unwrap();
        for months in [0., 1., 3., 6., 12.] {
            a.needs.dragon = Some(crate::creatures::dragon_life::DragonLife {
                months,
                ..Default::default()
            });
            let pose = Pose::of(&a, 0.);
            for (parent, part) in [(Joint::Skull, Joint::Jaw), (Joint::Skull, Joint::Breath)] {
                assert!(
                    pose.point(parent, part.pivot())
                        .abs_diff_eq(pose.point(part, part.pivot()), 0.0001)
                );
            }
            for i in 1..TAIL_BONES {
                let joint = Joint::Tail(i);
                assert!(
                    pose.point(joint, joint.pivot())
                        .abs_diff_eq(pose.point(Joint::Tail(i - 1), joint.pivot()), 0.0001)
                );
            }
            for side in [-1, 1] {
                let point = wrist(side.into());
                assert!(
                    pose.point(Joint::WingArm(side), point)
                        .abs_diff_eq(pose.point(Joint::Sail(side), point), 0.0001)
                );
            }
        }
    }

    #[test]
    fn long_lived_dragons_keep_their_wing_and_tail_phases_across_clock_rollover() {
        let mut a = dragon(K::EarthDragon, B::Fly);
        let wrap = std::f64::consts::TAU * 100. * 400. - (a.id % 23) as f64 * 0.37;
        a.age = wrap - 0.001;
        let before = Pose::of(&a, 0.);
        a.age = wrap + 0.001;
        let after = Pose::of(&a, 0.);
        for joint in [Joint::Sail(1), Joint::Skull, Joint::Tail(5)] {
            let tip = joint.pivot() + Vec3::Z;
            assert!(before.point(joint, tip).distance(after.point(joint, tip)) < 0.04);
        }
    }

    #[test]
    fn walking_feet_follow_the_presented_distance_even_when_snapshots_repeat_or_jump() {
        let mut a = dragon(K::DarkDragon, B::Wander);
        let initial = Pose::of(&a, 0.);
        for fps in [30, 60, 144] {
            let dt = 1. / fps as f32;
            let mut last = initial;
            for frame in 0..2 * fps {
                a.age = if frame % 7 == 0 { 1248. } else { 3. };
                let mut now = Pose::of(&a, 0.);
                let travel = if frame < fps { 3. * dt } else { 0. };
                now.stride_from(last, travel, dt);
                let expected = (last.gait_phase + travel * 1.42).rem_euclid(2. * PI);
                assert!((now.gait_phase - expected).abs() < 0.0001);
                last = now;
            }
            assert!(last.walking < 0.001);
        }
    }

    #[test]
    fn sleep_and_bank_converge_continuously_at_different_render_rates() {
        let rest = Pose::of(&dragon(K::DarkDragon, B::Rest), 0.);
        let sleeping = Pose::of(&dragon(K::DarkDragon, B::Sleep), 0.);
        let mut first = sleeping;
        first.settle_from(rest, 1. / 60.);
        assert!(first.asleep > 0. && first.asleep < 0.15);
        assert!(
            first
                .point(Joint::Skull, Joint::Skull.pivot())
                .distance(rest.point(Joint::Skull, Joint::Skull.pivot()))
                < 0.75
        );
        for rate in [30, 60, 144] {
            let mut pose = rest;
            for _ in 0..rate {
                let mut next = sleeping;
                next.settle_from(pose, 1. / rate as f32);
                pose = next;
            }
            assert!((pose.asleep - (1. - (-6f32).exp())).abs() < 2e-5);
        }
    }

    #[test]
    fn aerial_casts_keep_the_hind_legs_folded_through_hover_and_charge() {
        let mut a = dragon(K::FireDragon, B::DragonVolley);
        a.velocity = [0.; 3];
        a.needs.goal = Some(super::super::NeedGoal {
            kind: super::super::NeedKind::AirborneAttackAim,
            position: [4, 12, 0],
        });
        let flight = Pose::of(&a, 0.);
        a.needs.goal.as_mut().unwrap().kind = super::super::NeedKind::AttackAim;
        let ground = Pose::of(&a, 0.);
        assert!(flight.airborne > 0.99 && ground.airborne < 0.01);
        let hip = Joint::Thigh {
            side: 1,
            hind: true,
        };
        let knee = Joint::Foot {
            side: 1,
            hind: true,
        };
        let bend = |p: Pose| p.bone(hip).rotation.inverse() * p.bone(knee).rotation;
        assert!(bend(flight).angle_between(bend(ground)) > 0.4);
    }

    #[test]
    fn grounded_dragon_wing_tips_follow_the_flanks_in_every_ground_action() {
        for kind in [K::FireDragon, K::DarkDragon] {
            for behavior in [B::Rest, B::Hunt, B::Attack, B::Tremor, B::Sleep] {
                let pose = Pose::of(&dragon(kind, behavior), 0.);
                for side in [-1, 1] {
                    let s = f32::from(side);
                    let tip = pose.point(Joint::Sail(side), v(s * 18.4, 5., 2.45));
                    let shoulder = pose.point(Joint::WingArm(side), Joint::WingArm(side).pivot());
                    let span = pose.bone(Joint::Trunk).rotation.inverse() * (tip - shoulder);
                    assert!(
                        span.z > 8. && span.y < 1. && span.x.abs() < 3.4,
                        "{kind:?} {behavior:?}: {tip:?}, {shoulder:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn power_stroke_opens_the_span_and_drives_lift_before_the_long_folded_recovery() {
        for kind in [K::FireDragon, K::DarkDragon] {
            let mut a = dragon(kind, B::Fly);
            a.velocity = [0., 6., -7.];
            let shape = |a: &Creature| {
                let pose = Pose::of(a, 0.);
                let right = pose.point(Joint::Sail(1), v(18.4, 5., 2.45));
                let left = pose.point(Joint::Sail(-1), v(-18.4, 5., 2.45));
                (right.x - left.x, right.y)
            };
            let mut loaded = (0., 0.);
            let mut recovered = (0., 0.);
            let mut lift = [0.; 2];
            for sample in 0..400 {
                a.age = sample as f64 * 0.01;
                let angle = wing_angle(&a, a.age);
                if (angle - 0.70).abs() < 0.025 {
                    loaded = shape(&a);
                    lift[0] = wing_impulse(&a, a.velocity)[0];
                }
                if (angle - 3.32).abs() < 0.025 {
                    recovered = shape(&a);
                    lift[1] = wing_impulse(&a, a.velocity)[0];
                }
            }
            assert!(
                loaded.0 > recovered.0 + 2.,
                "{kind:?} span {loaded:?}/{recovered:?}"
            );
            assert!(lift[0] > 1. && lift[1] < -0.8);
        }
    }

    #[test]
    fn bank_follows_the_turn_and_descent_releases_the_wings_into_a_glide() {
        let mut animal = dragon(K::FireDragon, B::Fly);
        animal.velocity = [5., 1., -8.];
        let right = Pose::of(&animal, 0.);
        animal.velocity[0] *= -1.;
        let left = Pose::of(&animal, 0.);
        let roll = |p: Pose| (p.bone(Joint::Trunk).rotation * Vec3::X).y;
        assert!(roll(right) - roll(left) > 0.7);
        animal.velocity = [0., -5., -12.];
        for tick in 0..100 {
            assert!(stroke_strength(&animal, animal.age + f64::from(tick) * 0.1) < 0.15);
        }
        animal.velocity = [0., 6., -7.];
        let climb = Pose::of(&animal, 0.);
        assert!(climb.stroke > 0.95 && climb.pitch > 0.5);
    }

    #[test]
    fn adult_and_hatchling_heads_and_tails_remain_targetable_as_they_move() {
        for kind in [K::FireDragon, K::DarkDragon, K::EarthDragon, K::WaterDragon] {
            for b in [B::Rest, B::Fly, B::Sleep, B::Attack, B::Spit, B::Dead] {
                let mut a = dragon(kind, b);
                for age in [0., breeding::GROWTH_SECONDS] {
                    a.age = age;
                    a.yaw = 0.71;
                    let poses = Pose::of(&a, 0.);
                    let boxes = hitboxes::for_creature(&a);
                    assert_eq!(
                        boxes.iter().count(),
                        match kind {
                            K::WaterDragon => 12,
                            K::EarthDragon => 24,
                            _ => 48,
                        }
                    );
                    for (bone, center) in [
                        (Joint::Skull, v(0., 8.63, -6.46)),
                        (Joint::Tail(4), tail_node(4).lerp(tail_node(5), 0.5)),
                    ] {
                        let p = Quat::from_rotation_y(a.yaw)
                            * poses.point(bone, center)
                            * dragons::model_scale(kind, a.variant)
                            * a.growth_scale() as f32;
                        let origin = std::array::from_fn(|i| a.position[i] + f64::from(p[i]));
                        assert!(
                            boxes.ray_distance(origin, [1., 0., 0.], 0.001).is_some(),
                            "{kind:?} {b:?} {bone:?}"
                        );
                    }
                }
            }
        }
    }
}
