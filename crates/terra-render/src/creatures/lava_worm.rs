use super::{Limb, Motion, model::Part};
use bevy::prelude::*;
use terra_core::creatures::{Creature, CreatureBehavior as B, lava_worm as pose};

mod wake;
pub(super) use wake::install;

pub(super) fn aim(a: &Creature) -> Option<[f32; 3]> {
    let d = Vec3::from_array(pose::bolt_direction(a)?.map(|v| v as f32));
    Some((Quat::from_rotation_y(-a.yaw) * d).to_array())
}

pub(super) fn animate(m: &Motion, part: &Limb, t: &mut Transform, v: &mut Visibility) {
    let remaining = m.worm_progress;
    *t = Transform::IDENTITY;
    match part.0 {
        Part::LavaRing { index, .. } => {
            let (center, rotation) =
                pose::segment(m.behavior, remaining, f64::from(m.clock), index);
            t.translation = center;
            t.rotation = rotation;
            *v = if center.y + pose::segment_radius(index) <= 0. {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
        Part::LavaJaw(i) => {
            let (head, axis) = pose::segment(m.behavior, remaining, f64::from(m.clock), 0);
            let angle = f32::from(i) * std::f32::consts::TAU / 6.;
            let radial = Quat::from_rotation_z(angle);
            let gape = pose::gape(m.behavior, remaining, m.clock);
            t.translation = head + axis * (radial * Vec3::new(0., 0.42, -0.17));
            t.rotation = axis * radial * Quat::from_rotation_x(gape * 0.75);
            *v = if head.y + pose::segment_radius(0) > 0. {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        Part::WormSpit(i) => {
            let since = pose::SPIT - remaining - pose::RELEASE - f32::from(i) * 0.026;
            *v = if m.behavior == B::Spit
                && since >= 0.
                && let Some(dir) = m.worm_spit
            {
                t.translation = pose::mouth_socket(m.behavior, remaining, f64::from(m.clock))
                    + Vec3::from(dir) * since * pose::BOLT_SPEED;
                t.rotation = Quat::from_rotation_z(2.8 * since + f32::from(i));
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        _ => {}
    }
}
