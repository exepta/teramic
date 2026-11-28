use super::{
    Behavior, Kind, Limb, Motion,
    model::{Part, poultry},
};
use bevy::prelude::*;

// Two quick contacts, a slow lift, then a pause to inspect the next patch.
fn peck(clock: f32) -> f32 {
    let time = clock.rem_euclid(1.56);
    let strike = |center: f32| {
        let distance = time - center;
        let t = (1. - distance.abs() / if distance < 0. { 0.17 } else { 0.26 }).clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    strike(0.25).max(strike(0.76))
}

pub(super) fn animate(m: &Motion, limb: &Limb, pose: &mut Transform) {
    let hip = poultry::hip(m.kind);
    let stride = (m.speed * 0.83).min(1.) * (1. - m.sleep);
    let step = m.phase.sin();
    let dip = match m.behavior {
        Behavior::Graze => peck(m.clock),
        Behavior::Drink => (0.5 - 0.5 * (m.clock * 3.5).cos()).powi(2),
        _ => 0.,
    };
    let strike = if m.kind == Kind::Rooster && m.behavior == Behavior::Attack {
        let impact = terra_core::creatures::definitions::MobCombat::MELEE_IMPACT_PHASE;
        (m.attack / impact).clamp(0., 1.) * (1. - (m.attack - impact).max(0.) / 0.33).clamp(0., 1.)
    } else {
        0.
    };
    let rock = Quat::from_euler(
        EulerRot::XZY,
        -0.45 * dip - 0.10 * strike,
        step * 0.032 * stride,
        0.,
    );
    let squat = if m.kind == Kind::Rooster {
        0.043
    } else {
        0.004
    };
    let bob = Vec3::Y * (step.abs() * 0.013 * stride - dip * squat - m.sleep * hip.y * 0.45);
    pose.translation = hip + bob + rock * (limb.1 - hip);
    pose.rotation = rock;
    match limb.0 {
        Part::Head => {
            pose.translation.y += (pose.scale.y - 1.) * dip * 0.23;
            pose.translation.z += step * stride * 0.026 - strike * 0.074;
            pose.rotation = Quat::from_euler(
                EulerRot::YXZ,
                (m.clock * 1.55).sin() * 0.10 * (1. - dip) * (1. - m.sleep) + 1.29 * m.sleep,
                -1.82 * dip - 0.48 * strike - 0.075 * step * stride - 0.32 * m.sleep,
                -0.08 * m.sleep,
            );
        }
        Part::Wing(side) => {
            let flap = if m.behavior == Behavior::Flee {
                0.14 + 0.22 * (m.phase * 1.7).sin().abs()
            } else {
                0.
            };
            pose.rotation *= Quat::from_rotation_y(side * (flap + 0.48 * strike));
        }
        Part::Tail => {
            // Counterbalance the forward jab with the fan, independently of the hip.
            pose.rotation *= Quat::from_euler(
                EulerRot::YXZ,
                step * stride * 0.06,
                -0.16 * dip - 0.20 * strike,
                0.,
            );
        }
        Part::Leg(side) => {
            let phase = side * step;
            let tuck = m.sleep * 0.98;
            pose.translation = limb.1;
            pose.translation.y += phase.max(0.) * stride * 0.042 - (1. - tuck.cos()) * hip.y;
            pose.rotation = Quat::from_rotation_x(phase * stride * 0.57 - tuck);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_beaks_reach_the_floor_on_each_grazing_beat_and_lift_in_between() {
        for kind in [Kind::Hen, Kind::Rooster] {
            let mut m = Motion {
                kind,
                behavior: Behavior::Graze,
                phase: 0.,
                clock: 0.,
                speed: 0.,
                sleep: 0.,
                attack: 0.,
                worm_progress: 0.,
                worm_spit: None,
                silk_length: 0.,
                youth: 0.,
                jump: Default::default(),
            };
            let parts = crate::creatures::model::parts(kind).unwrap();
            let (_, pivot, mesh) = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::Head))
                .unwrap();
            let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!("head vertex positions")
            };
            let end = positions.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
            let bill: Vec<_> = positions
                .iter()
                .filter(|p| p[2] <= end + 0.001)
                .map(|&p| Vec3::from_array(p))
                .collect();
            for (clock, touching) in [(0.25, true), (0.52, false), (0.76, true), (1.38, false)] {
                m.clock = clock;
                let mut head = Transform::default();
                animate(&m, &Limb(Part::Head, *pivot), &mut head);
                let height = bill
                    .iter()
                    .map(|p| head.transform_point(*p).y)
                    .fold(f32::INFINITY, f32::min);
                if touching {
                    assert!(
                        (-0.025..0.036).contains(&height),
                        "{kind:?} beak at {height}"
                    );
                } else {
                    assert!(height > 0.28, "{kind:?} beak still at {height}");
                }
            }
        }
    }
}
