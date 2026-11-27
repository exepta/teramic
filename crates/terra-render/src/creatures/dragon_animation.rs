pub(super) mod ground;

use super::model::dragon::{Joint, scale};
use bevy::prelude::*;
use terra_core::creatures::dragons::{Lineage, missile_path};
use terra_core::creatures::{Creature, CreatureBehavior as B, NeedKind, dragon_pose::Pose};

#[derive(Component)]
pub(super) struct Presentation {
    pose: Pose,
    age: f64,
    target: Option<Vec3>,
    scale: f32,
    ground: ground::Stance,
}
impl Presentation {
    pub fn of(a: &Creature, since: f32) -> Self {
        let scale = scale(a.kind, a.variant);
        let target = a
            .needs
            .goal
            .filter(|g| matches!(g.kind, NeedKind::AttackAim | NeedKind::AirborneAttackAim))
            .map(|goal| {
                let local = Vec3::from_array(std::array::from_fn(|i| {
                    (goal.position[i] as f64 + 0.5 - a.position[i]) as f32
                }));
                Quat::from_rotation_y(-a.yaw) * local / a.growth_scale() as f32
            });
        Self {
            pose: Pose::of(a, since),
            age: a.age + f64::from(since),
            target,
            scale,
            ground: Default::default(),
        }
    }

    pub fn advance(
        &mut self,
        a: &Creature,
        since: f32,
        dt: f32,
        ground_distance: f32,
        root: &Transform,
    ) {
        let desired = a.age + f64::from(since);
        let delta = desired - self.age;
        // Late packets can repeat a fraction of the preceding wing stroke.
        // Catch up at a bounded rate; a reset (e.g. a new fixture) snaps once.
        let age = if delta.abs() < 0.45 {
            self.age + delta.clamp(0., f64::from(dt) * 1.4)
        } else {
            desired
        };
        let previous = self.pose;
        self.age = age;
        self.pose = Pose::of(a, (age - a.age) as f32);
        self.target = a
            .needs
            .goal
            .filter(|g| matches!(g.kind, NeedKind::AttackAim | NeedKind::AirborneAttackAim))
            .map(|goal| {
                let world = Vec3::from_array(goal.position.map(|v| v as f32 + 0.5));
                (root.rotation.inverse() * (world - root.translation)) / root.scale
            });
        self.pose.settle_from(previous, dt);
        self.pose.stride_from(
            previous,
            ground_distance / (self.scale * a.growth_scale() as f32),
            dt,
        );
    }

    /// Particles travel continuously from the mouth, then disperse around the
    /// warned point. All particles share an existing additive draw batch.
    pub(super) fn motes(
        &self,
        kind: super::Kind,
        root: &GlobalTransform,
        mut emit: impl FnMut(Vec3, f32, [f32; 4]),
    ) {
        let p = self.pose.progress;
        let b = self.pose.behavior;
        if b == B::DragonBurrow || b == B::DragonSurge && kind == super::Kind::DarkDragon {
            let earth = b == B::DragonBurrow;
            let immersion = self.pose.submergence();
            let radius = if earth { 5.0 } else { 6.2 };
            for i in 0..24 {
                let t = (p * if earth { 4. } else { 2.8 } + i as f32 * 0.071).fract();
                let angle = i as f32 * 2.399963 + if earth { 0. } else { 2.4 * p };
                let r = radius * (0.35 + t * 0.8);
                let y = if earth {
                    0.2 + 1.8 * (std::f32::consts::PI * t).sin()
                } else {
                    0.12 + 2.4 * (1. - t).powi(2)
                };
                let pos = root
                    .transform_point(Vec3::new(r * angle.cos(), y, r * angle.sin()) * self.scale);
                let alpha = (1. - t) * immersion;
                let color = if earth {
                    [0.60, 0.41, 0.21, alpha]
                } else if i % 4 == 0 {
                    [1.5, 0.08, 0.32, alpha]
                } else {
                    [0.42, 0.07, 0.74, alpha]
                };
                emit(
                    pos,
                    if earth {
                        0.48 + 0.48 * t
                    } else {
                        0.28 + 0.50 * t
                    },
                    color,
                );
            }
            return;
        }
        let Some(aim) = self.target else { return };
        if !matches!(b, B::Spit | B::DragonVolley | B::Tremor | B::Attack) {
            return;
        }
        let start = root
            .transform_point(Vec3::from(self.pose.bone(Joint::Breath).translation) * self.scale);
        let target = root.transform_point(aim);
        let axis = (target - start).normalize_or(Vec3::NEG_Z);
        let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = side.cross(axis);
        let (color, scatter, drift) = match kind {
            super::Kind::FireDragon => ([1.50, 0.36, 0.045], 2.25, 2.9),
            super::Kind::WaterDragon => ([0.22, 0.88, 1.28], 1.6, -0.8),
            super::Kind::EarthDragon => ([1.25, 0.47, 0.095], 1.25, -1.8),
            _ => ([0.83, 0.13, 1.20], 2.1, 2.2),
        };
        let rgba = |alpha| [color[0], color[1], color[2], alpha];
        let impact = terra_core::creatures::dragons::impact_phase(b);
        if matches!(b, B::Tremor | B::Attack) {
            if p >= impact {
                let t = (p - impact) / (1. - impact);
                for i in 0..26 {
                    let angle = i as f32 * 2.399963;
                    let radius = (5.5 + (i % 7) as f32) * t.sqrt();
                    let arc = 4.0 * t * (1.0 - t) * (0.7 + (i % 4) as f32 * 0.44);
                    emit(
                        target + Vec3::new(angle.cos() * radius, arc, angle.sin() * radius),
                        (0.28 + (i % 3) as f32 * 0.23) * (1. - t * 0.5),
                        rgba(1. - t),
                    );
                }
            }
            return;
        }
        let charge = (p / 0.29).clamp(0., 1.);
        if p < 0.36 {
            let fade = charge * ((0.36 - p) / 0.07).clamp(0., 1.);
            for i in 0..12 {
                let angle = i as f32 * 2.399963 + 4. * charge;
                let radial = side * angle.cos() + up * angle.sin();
                let pos = start + radial * ((1. - charge) * 2.4 + 0.22) - axis * (1. - charge);
                emit(pos, 0.36 + charge * 0.32, rgba(fade * 0.75));
            }
            emit(start, 1.0 + 1.3 * charge, rgba(fade));
        }
        if b == B::Spit && (0.23..0.89).contains(&p) {
            let head = ((p - 0.27) / (impact - 0.27)).clamp(0., 1.);
            let tail = ((p - 0.77) / 0.12).clamp(0., 1.);
            for i in 0..46 {
                let t = (i as f32 / 46. + p * 3.2).fract();
                if t < tail || t > head {
                    continue;
                }
                let turn = i as f32 * 2.399963 + t * 4.4;
                let radial = side * turn.cos() + up * turn.sin();
                let center = start.lerp(target, t);
                let radius = (0.2 + t) * scatter * ((i * 7 % 17) as f32 / 17.);
                let v = center + radial * radius + Vec3::Y * (drift * t * t * 0.24);
                let glow = (1. - t * 0.35) * ((p - 0.23) / 0.09).clamp(0., 1.);
                emit(v, (0.33 + (i % 5) as f32 * 0.16) * (1. + t), rgba(glow));
            }
        }
        if p >= impact {
            let age = (p - impact) / (1. - impact);
            for i in 0..22 {
                let theta = i as f32 * 2.399963;
                let spread = 1.5 + (i * 9 % 11) as f32 * 0.45;
                let radial = Vec3::new(theta.cos(), 0., theta.sin());
                let rise = (2. + (i % 5) as f32) * age * (1. - 0.72 * age);
                emit(
                    target + radial * spread * age + Vec3::Y * rise,
                    (0.44 + 1.05 * age) * (1. - 0.6 * age),
                    rgba(1. - age),
                );
            }
        }
    }

    pub fn animate(
        &self,
        joint: Joint,
        luminous: bool,
        t: &mut Transform,
        visibility: &mut Visibility,
    ) {
        let b = self.pose.behavior;
        let p = self.pose.progress;
        let show = match joint {
            Joint::Missile(_) => b == B::DragonVolley && (0.15..0.74).contains(&p),
            Joint::Ward => b == B::DragonWard && (0.08..0.95).contains(&p),
            Joint::Surge => b == B::DragonSurge && (0.23..0.96).contains(&p),
            Joint::Breath => b == B::Spit && (0.27..0.84).contains(&p),
            // Ground eruptions follow foot contact. Ranged impacts use the
            // dispersing motes at the actual aim height, including mid-air.
            Joint::Impact => {
                (b == B::DragonBurrow && (0.08..0.98).contains(&p))
                    || matches!(b, B::Tremor | B::Attack) && (0.48..0.91).contains(&p)
            }
            Joint::Rune => b == B::Ink && (0.08..0.94).contains(&p),
            _ => self.pose.submergence() < 0.985,
        };
        *visibility = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if !show {
            return;
        }
        let frame = self.ground.bone(self.pose, joint);
        t.translation = Vec3::from(frame.translation) * self.scale;
        t.rotation = frame.rotation;
        t.scale = self.pose.joint_scale(joint);
        match joint {
            Joint::Eyes => {
                let phase = self.pose.clock.rem_euclid(4.9);
                let openness = match b {
                    B::Dead | B::Sleep => 0.03,
                    _ => ((phase - 0.13).abs() / 0.13).clamp(0.03, 1.),
                };
                // The eye mesh shares the skull joint. Scale about the iris
                // centre so hatchling and adult eyes close inside their sockets.
                let offset = (8.87 - joint.pivot().y) * self.scale * t.scale.y;
                t.translation += t.rotation * Vec3::Y * offset * (1. - openness);
                t.scale.y *= openness;
            }
            Joint::Missile(i) => {
                let Some((form, aim)) = self.pose.lineage.zip(self.target) else {
                    return;
                };
                let start = Vec3::from(self.pose.bone(Joint::Breath).translation) * self.scale;
                let flight = ((p - 0.30) / 0.42).clamp(0., 1.);
                t.translation = missile_path(form, i, flight, start, aim);
                t.rotation = Quat::from_rotation_arc(
                    Vec3::NEG_Z,
                    (aim - start).try_normalize().unwrap_or(Vec3::NEG_Z),
                ) * Quat::from_rotation_z(p * 13. + f32::from(i));
                t.scale = Vec3::splat(((p - 0.15) / 0.15).clamp(0.01, 1.));
            }
            Joint::Ward | Joint::Surge => {
                let ward = joint == Joint::Ward;
                let shadow = self.pose.kind == super::Kind::DarkDragon;
                if !ward
                    && !shadow
                    && let Some(target) = self.target
                {
                    t.translation = target;
                }
                let start = if ward { 0.08 } else { 0.23 };
                let rise = ((p - start) / if ward { 0.14 } else { 0.38 }).clamp(0.02, 1.);
                let fade = ((0.96 - p) / 0.15).clamp(0.02, 1.);
                t.rotation = Quat::from_rotation_y(p * if shadow { 5.6 } else { 0.8 });
                t.scale = Vec3::new(rise * fade, (rise * rise * fade).max(0.01), rise * fade);
            }
            Joint::Breath => {
                let direction = self
                    .target
                    .map_or(frame.rotation * Vec3::NEG_Z * (24. * self.scale), |pos| {
                        pos - t.translation
                    });
                let extend = ((p - 0.27) / 0.28).clamp(0., 1.);
                let fade = ((0.84 - p) / 0.12).clamp(0., 1.);
                let width = (0.95 + 0.065 * (p * 33.).sin()) * fade;
                t.rotation = Quat::from_rotation_arc(
                    Vec3::NEG_Z,
                    direction.try_normalize().unwrap_or(Vec3::NEG_Z),
                ) * Quat::from_rotation_z(p * 8.);
                t.scale = Vec3::new(
                    width,
                    width,
                    (direction.length() / self.scale).clamp(0.01, 80.) * extend.max(0.01),
                );
            }
            Joint::Impact if b == B::DragonBurrow => {
                let amount = self.pose.submergence();
                let ring = 0.48 + 0.55 * amount;
                t.translation = Vec3::Y * 0.08;
                t.scale = Vec3::new(ring, (0.30 + 0.16 * (p * 42.).sin()) * amount, ring);
            }
            Joint::Impact => {
                let phase = (p - 0.48) / 0.43;
                let wave = 0.06 + (1. - (1. - phase).powi(2)) * 1.25;
                t.scale = Vec3::new(wave, (phase * std::f32::consts::PI).sin().max(0.01), wave);
            }
            Joint::Rune => {
                if matches!(self.pose.lineage, Some(Lineage::Shadow | Lineage::Stone))
                    && let Some(target) = self.target
                {
                    let direction = Vec3::new(target.x, 0., target.z);
                    t.rotation = Quat::from_rotation_arc(
                        Vec3::NEG_Z,
                        direction.try_normalize().unwrap_or(Vec3::NEG_Z),
                    );
                    let f = ((p - 0.08) / 0.47).clamp(0.01, 1.);
                    let fade = ((0.94 - p) / 0.17).clamp(0.01, 1.);
                    t.scale = Vec3::new(
                        fade * if luminous {
                            0.94 + 0.07 * (p * 24.).sin()
                        } else {
                            1.
                        },
                        fade,
                        (direction.length() / self.scale).max(0.02) * f,
                    );
                    return;
                }
                if let Some(target) = self.target {
                    t.translation = target + Vec3::Y * 0.08;
                }
                let phase = (p - 0.08) / 0.86;
                let radius = 0.29 + ((phase * 4.).min(1.)) * 0.76;
                t.rotation = Quat::from_rotation_y(phase * 2.4);
                t.scale = Vec3::splat(radius * ((1. - phase) * 8.).clamp(0.01, 1.));
            }
            _ => {}
        }
    }
}
