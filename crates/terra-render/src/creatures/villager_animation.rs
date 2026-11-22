//! A planted boot, bent knee and swinging arm share their anatomical pivots.
use super::{Limb, Motion, model::Part};
use bevy::prelude::*;

const ELBOW: Vec3 = Vec3::new(0., -0.17, 0.);
const KNEE: Vec3 = Vec3::new(0., -0.31, 0.);
const WAIST: Vec3 = Vec3::new(0., 0.65, 0.);

pub(super) fn villager_action(m: &Motion) -> f32 {
    // Long backswing, short power stroke, then release. The grip follows the wrist.
    let phase = (m.clock * 0.88).fract();
    let t = if phase < 0.43 {
        phase / 0.43
    } else {
        1. - ((phase - 0.43) / 0.31).min(1.)
    };
    t * t * (3. - 2. * t) * m.attack * (1. - m.sleep)
}

pub(super) fn animate_villager(m: &Motion, part: &Limb, t: &mut Transform) {
    let awake = 1. - m.sleep;
    let walking = (m.speed / 1.35).clamp(0., 1.) * awake;
    let stride = (m.speed * 0.36).min(0.66) * awake;
    let (step, swing) = m.phase.sin_cos();
    let breath = (m.clock * 1.75).sin() * awake;
    let action = villager_action(m);
    let torso = Quat::from_euler(
        EulerRot::YXZ,
        -step * 0.073 * walking + action * 0.17,
        -0.074 * walking - 0.14 * action + 0.006 * breath,
        0.037 * swing * walking,
    );
    let shift = Vec3::new(
        0.024 * swing * walking,
        0.016 * step * step * walking + breath * 0.003,
        0.,
    );
    t.translation = part.1 + shift + (torso * (part.1 - WAIST) - (part.1 - WAIST));
    t.rotation = torso;
    match part.0 {
        Part::Leg(side) | Part::VillagerThigh(side) => {
            let lift = (swing * side).max(0.).powi(2) * walking;
            let thigh = step * side * stride;
            let knee = -lift * (0.35 + 0.28 * stride);
            let hip = Quat::from_rotation_x(thigh);
            let shin = Quat::from_rotation_x(thigh + knee);
            let offset = hip * KNEE - shin * KNEE;
            let (sin, cos) = (thigh + knee).sin_cos();
            let sole = part.1.y + offset.y
                - 0.60 * cos
                - if sin > 0. { 0.125 * sin } else { -0.225 * sin };
            t.translation = part.1 + Vec3::Y * ((0.0025 - sole) * awake + 0.043 * lift);
            if matches!(part.0, Part::Leg(_)) {
                t.translation += offset;
                t.rotation = shin;
            } else {
                t.rotation = hip;
            }
        }
        Part::VillagerSleeve(side) | Part::VillagerArm(side) | Part::MilitaryWeapon(side) => {
            let right = if side > 0. { 1. } else { 0. };
            let shoulder = torso
                * Quat::from_euler(
                    EulerRot::XYZ,
                    -step * side * stride * 0.72 - 0.06 * awake - 0.18 * m.sleep
                        + right * action * 1.10
                        - (1. - right) * action * 0.20,
                    -side * action * 0.16,
                    side * (0.045 * awake + 0.22 * m.sleep + action * 0.12),
                );
            t.rotation = shoulder;
            if !matches!(part.0, Part::VillagerSleeve(_)) {
                let flex = (0.06
                    + (0.5 - 0.5 * step * side) * stride * 0.48
                    + action * (0.30 + right * 0.68))
                    * awake;
                t.rotation *= Quat::from_rotation_x(flex);
                t.translation += shoulder * ELBOW - t.rotation * ELBOW;
            }
        }
        Part::Head => {
            t.translation.z -= 0.085 * m.sleep;
            t.rotation = Quat::from_euler(
                EulerRot::YXZ,
                (m.clock * 0.41).sin() * 0.10 * awake + step * walking * 0.018,
                breath * 0.014 - 0.04 * m.sleep - 0.12 * action,
                0.009 * swing * walking,
            );
        }
        Part::Body | Part::MilitaryTabard => t.scale.y = 1. + breath * 0.003,
        _ => {}
    }
}
