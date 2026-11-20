//! Articulation follows replicated behavior; presentation never advances authority.
use super::{Behavior, Kind, Limb, Motion, model::Part};
use bevy::prelude::*;
mod skeleton;
#[cfg(test)]
mod tests;
pub(super) fn animate(m: &Motion, part: &Limb, t: &mut Transform) {
    t.translation = part.1;
    t.rotation = Quat::IDENTITY;
    t.scale = Vec3::ONE;
    let walk = (m.speed / 1.2).clamp(0.0, 1.0);
    match m.kind {
        Kind::Raven => {
            let flying = matches!(m.behavior, Behavior::Fly | Behavior::Flee | Behavior::Land);
            match part.0 {
                Part::Wing(side) => {
                    t.rotation = Quat::from_rotation_z(
                        side * if flying {
                            0.30 + (m.clock * 10.0).sin() * 0.75
                        } else {
                            0.08
                        },
                    )
                }
                Part::Head => t.rotation = Quat::from_rotation_y((m.clock * 1.4).sin() * 0.14),
                Part::Tail => t.rotation = Quat::from_rotation_x((m.clock * 2.0).sin() * 0.09),
                _ => {}
            }
        }
        Kind::BloodSlime => {
            // Face uses the identical squash matrix, staying on the closed body.
            let squash =
                1.0 + (m.clock * if walk > 0.1 { 5.0 } else { 2.0 }).sin() * (0.025 + walk * 0.09);
            t.scale = Vec3::new(1.0 / squash.sqrt(), squash, 1.0 / squash.sqrt());
            t.translation.y += (m.clock * 5.0).sin().max(0.0) * walk * 0.10;
        }
        Kind::BloodStalker => super::animate_spider(m, part, t),
        Kind::BloodWisp => super::spirit_animation::animate(m, part, t),
        Kind::Skeleton => skeleton::animate(m, part, t),
        Kind::ThornBrute => {
            let stride = m.phase.sin();
            let run = (m.speed / 2.1).clamp(0.0, 1.0);
            t.translation.y += stride * stride * run * 0.025;
            let smooth = |p: f32| {
                let p = p.clamp(0.0, 1.0);
                p * p * (3.0 - 2.0 * p)
            };
            let windup = smooth(m.attack / 0.30);
            let hit = smooth((m.attack - 0.30) / 0.12);
            let recovery = smooth((m.attack - 0.42) / 0.58);
            let swing = 0.70 * windup - 1.65 * hit + 0.95 * recovery;
            match part.0 {
                Part::Leg(side) => t.rotation = Quat::from_rotation_x(stride * side * run * 0.66),
                Part::VillagerArm(side) => {
                    t.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        -stride * side * run * 0.46 + swing * if side < 0.0 { 1.0 } else { 0.3 },
                        swing * side * 0.10,
                        side * 0.035,
                    )
                }
                Part::Head | Part::ThornGlow { head: true } => {
                    t.rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        -(m.attack * std::f32::consts::PI).sin() * 0.11,
                        (m.clock * 0.7).sin() * 0.06,
                        0.0,
                    )
                }
                Part::ThornGlow { head: false } => {
                    let pulse = 1.0 + (m.clock * 2.8).sin() * 0.018;
                    t.scale = Vec3::splat(pulse);
                    t.translation += Vec3::new(0.0, 1.35, -0.323) * (1.0 - pulse);
                }
                _ => {}
            }
        }
        _ => {}
    }
}
