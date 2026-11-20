//! Smooth presentation around the existing pelvis, shoulder and skull pivots.
use super::*;

fn ease(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

pub(super) fn animate(m: &Motion, part: &Limb, t: &mut Transform) {
    let walk = (m.speed / 1.5).clamp(0.0, 1.0);
    let running = ((m.speed - 1.5) / 1.5).clamp(0.0, 1.0);
    let stride = m.phase.sin();
    // sin² has a continuous velocity at footfalls, unlike abs(sin). Each
    // articulated part shares this lift, so sockets cannot separate at impact.
    let lift = stride * stride * walk * 0.018;
    let attack = m.attack.clamp(0.0, 1.0);
    let windup = ease(attack / 0.30);
    let strike = ease((attack - 0.30) / 0.12);
    let recover = ease((attack - 0.42) / 0.58);
    let swing = 0.95 * windup - 1.30 * strike + 0.35 * recover;
    let twist = -0.10 * windup + 0.20 * strike - 0.10 * recover;
    let lean = -(0.035 + running * 0.09) * walk - (attack * std::f32::consts::PI).sin() * 0.075;
    let torso = Quat::from_euler(EulerRot::YXZ, stride * walk * 0.045 + twist, lean, 0.0);
    let hip = Vec3::new(0.0, 0.74, 0.0);
    let upper_body = matches!(
        part.0,
        Part::Body | Part::Head | Part::SkeletonEyes | Part::VillagerArm(_)
    );
    t.translation = if upper_body {
        hip + torso * (part.1 - hip)
    } else {
        part.1
    } + Vec3::Y * lift;
    t.rotation = if upper_body { torso } else { Quat::IDENTITY };
    match part.0 {
        Part::Leg(side) => {
            t.rotation = Quat::from_rotation_x(stride * side * walk * (0.46 + running * 0.22));
        }
        Part::VillagerArm(side) => {
            let wielding = side < 0.0;
            t.rotation *= Quat::from_euler(
                EulerRot::XYZ,
                -stride * side * walk * (0.36 + running * 0.18)
                    + swing * if wielding { 1.0 } else { 0.22 },
                -twist * if wielding { 2.0 } else { 0.5 },
                side * 0.045,
            );
        }
        Part::Head | Part::SkeletonEyes => {
            // Identical transform for the separate red pupils. Counter-rotate
            // the neck slightly to keep the gaze stable during shoulder sway.
            t.rotation *= Quat::from_euler(
                EulerRot::YXZ,
                (m.clock * 0.7).sin() * 0.035 - stride * walk * 0.025,
                -lean * 0.55,
                0.0,
            );
        }
        _ => {}
    }
}
