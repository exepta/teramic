//! Grounded cat gaits, a sculling crocodile tail and a continuous travelling body wave.
use super::{
    Behavior, Kind, Limb, Motion,
    model::{
        Part,
        wild::{self, snake},
    },
};
use bevy::prelude::*;

fn strike(m: &Motion) -> f32 {
    if m.behavior != Behavior::Attack {
        return 0.;
    }
    let impact = terra_core::creatures::definitions::MobCombat::MELEE_IMPACT_PHASE;
    (m.attack / impact).clamp(0., 1.) * (1. - (m.attack - impact).max(0.) / 0.35).clamp(0., 1.)
}

pub(super) fn animate(m: &Motion, limb: &Limb, pose: &mut Transform, visible: &mut Visibility) {
    let contact = strike(m);
    if m.kind == Kind::Snake {
        serpent(m, limb, pose, visible, contact);
    } else if m.kind == Kind::Crocodile {
        crocodile(m, limb, pose, contact);
    } else {
        cat(m, limb, pose, contact);
    }
}

fn cat(m: &Motion, limb: &Limb, pose: &mut Transform, contact: f32) {
    let scale = wild::cat_scale(m.kind);
    let head = wild::CAT_HEAD * scale;
    let stalk = f32::from(m.behavior == Behavior::Hunt);
    let stride = (m.speed * 0.29).min(0.83) * (1. - m.sleep);
    let crouch = (0.08 * stalk + 0.29 * m.sleep) * scale.y;
    let shift = Vec3::new(
        0.,
        -crouch + stride * m.phase.sin().abs() * 0.025,
        -contact * 0.11,
    );
    let drink = f32::from(m.behavior == Behavior::Drink);
    let neck = Quat::from_euler(
        EulerRot::YXZ,
        (m.clock * 0.66).sin() * 0.058,
        -0.62 * drink - 0.15 * stalk - 0.23 * contact - 0.27 * m.sleep,
        0.,
    );
    pose.translation = limb.1 + shift;
    pose.rotation = Quat::IDENTITY;
    match limb.0 {
        Part::Head | Part::Jaw => {
            pose.scale = Vec3::splat(1. + m.youth * 0.12);
            pose.translation = head + shift + neck * ((limb.1 - head) * pose.scale);
            pose.rotation = neck;
            if matches!(limb.0, Part::Jaw) {
                let opening = if m.behavior == Behavior::Attack {
                    (m.attack * std::f32::consts::PI).sin().max(0.)
                } else {
                    0.
                };
                pose.rotation *= Quat::from_rotation_x(-0.40 * opening * (1. - contact * 0.72));
            }
        }
        Part::Leg(sign) => {
            let step = sign * m.phase.sin();
            let tuck = if limb.1.z < 0. { -1.04 } else { 1.04 };
            let angle = step * stride + tuck * (0.40 * stalk + m.sleep);
            pose.rotation = Quat::from_rotation_x(angle);
            pose.translation.y = limb.1.y * angle.cos() + step.max(0.) * stride * 0.058;
        }
        Part::Tail => {
            pose.rotation = Quat::from_euler(
                EulerRot::YXZ,
                (m.phase * 0.61).sin() * (0.11 + stride * 0.23) + m.sleep * 0.55,
                0.07 + contact * 0.19,
                0.,
            )
        }
        _ => {}
    }
}

fn crocodile(m: &Motion, limb: &Limb, pose: &mut Transform, contact: f32) {
    let swim = m.behavior == Behavior::Swim;
    let stride = (m.speed * 0.37).min(0.65) * (1. - m.sleep);
    let twist = (m.phase * 0.63).sin() * (0.04 + stride * 0.10);
    let neck = wild::CROCODILE_NECK;
    let neck_rot = Quat::from_euler(EulerRot::YXZ, -twist * 0.85, -contact * 0.17, 0.);
    pose.rotation = Quat::IDENTITY;
    pose.translation = limb.1;
    match limb.0 {
        Part::Head | Part::Jaw => {
            pose.scale = Vec3::splat(1. + m.youth * 0.12);
            pose.translation =
                neck + neck_rot * ((limb.1 - neck) * pose.scale) - Vec3::Z * contact * 0.15;
            pose.rotation = neck_rot;
            if matches!(limb.0, Part::Jaw) {
                let opening = if m.behavior == Behavior::Attack {
                    (m.attack / 0.27).clamp(0., 1.)
                        * (1. - (m.attack - 0.27).max(0.) / 0.15).clamp(0., 1.)
                } else {
                    0.
                };
                pose.rotation *= Quat::from_rotation_x(-0.22 * opening);
            }
        }
        Part::Body => {
            pose.rotation = Quat::from_rotation_y(twist * 0.27);
        }
        Part::Leg(sign) => {
            let side = limb.1.x.signum();
            pose.rotation = if swim {
                Quat::from_euler(EulerRot::YXZ, -side * 0.76, 0., side * 0.2)
            } else {
                Quat::from_rotation_y(side * sign * m.phase.sin() * stride)
            };
            pose.translation.y +=
                (sign * m.phase.cos()).max(0.) * stride * 0.045 * f32::from(!swim);
        }
        Part::ReptileJoint(i) => {
            let point = |i: u8| {
                let n = f32::from(i);
                Vec3::new(
                    n * (0.019 + stride * 0.068 + f32::from(swim) * 0.065)
                        * (m.phase * 0.69 - n * 0.72).sin(),
                    0.26 - n * 0.04,
                    wild::CROCODILE_TAIL + n * wild::CROCODILE_TAIL_STEP,
                )
            };
            let a = point(i);
            let b = point(i + 1);
            pose.translation = a;
            pose.rotation = Quat::from_rotation_arc(Vec3::Z, (b - a).normalize());
            pose.scale.z = (b - a).length() / wild::CROCODILE_TAIL_STEP;
        }
        _ => {}
    }
}

fn snake_point(m: &Motion, i: u8, contact: f32) -> Vec3 {
    let n = f32::from(i);
    let t = n / f32::from(snake::COUNT);
    let phase = m.phase * 0.68 + m.clock * 0.13;
    let width = 0.152 + (m.speed * 0.026).min(0.064);
    let lift = (1. - t * 3.1).max(0.);
    Vec3::new(
        width * ((phase - t * 6.5).sin() * (1. - m.sleep) + (0.94 - t * 7.).sin() * m.sleep),
        0.091 + lift * (0.053 + 0.092 * contact),
        snake::START + n * snake::STEP - lift * contact * 0.235,
    )
}

fn serpent(m: &Motion, limb: &Limb, pose: &mut Transform, visible: &mut Visibility, contact: f32) {
    match limb.0 {
        Part::ReptileJoint(i) => {
            let a = snake_point(m, i, contact);
            let b = snake_point(m, i + 1, contact);
            pose.translation = (a + b) * 0.5;
            pose.translation.y += limb.1.y - 0.083;
            pose.rotation = Quat::from_rotation_arc(Vec3::Z, (b - a).normalize());
            pose.scale.z = (b - a).length() / snake::STEP;
        }
        Part::Head | Part::SnakeTongue => {
            let a = snake_point(m, 0, contact);
            let b = snake_point(m, 1, contact);
            pose.scale = Vec3::splat(1. + m.youth * 0.12);
            pose.translation = a - Vec3::Y * 0.025;
            pose.rotation = Quat::from_rotation_arc(Vec3::Z, (b - a).normalize());
            if matches!(limb.0, Part::SnakeTongue) {
                let t = m.clock.rem_euclid(2.3);
                *visible = if t < 0.34 && m.sleep < 0.5 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                let slide = (t / 0.34 * std::f32::consts::PI).sin().clamp(0., 1.);
                pose.translation += pose.rotation * (Vec3::Z * (1. - slide) * 0.09 * pose.scale.z);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use std::collections::HashSet;

    // Each greedy axis-aligned quad owns its four vertices. Count outward-ray
    // intersections in mesh-local space, including the actual stepped corners.
    fn contains(mesh: &Mesh, pose: Transform, world: Vec3) -> bool {
        let local = pose.compute_affine().inverse().transform_point3(world);
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let crosses = vertices
            .chunks_exact(4)
            .filter(|q| {
                if q[0][0] <= local.x || q.iter().any(|p| p[0] != q[0][0]) {
                    return false;
                }
                [1, 2].into_iter().all(|axis| {
                    let low = q.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
                    let high = q.iter().map(|p| p[axis]).fold(f32::NEG_INFINITY, f32::max);
                    local[axis] >= low && local[axis] < high
                })
            })
            .count();
        crosses % 2 == 1
    }

    #[test]
    fn every_snake_skin_has_distinct_colors_and_connected_moving_joints() {
        let mut colors = HashSet::new();
        for variant in 0..Kind::Snake.variant_count() {
            let parts = crate::creatures::model::parts_variant(Kind::Snake, variant).unwrap();
            let links: Vec<_> = parts
                .iter()
                .filter(|(p, _, _)| matches!(p, Part::ReptileJoint(_)))
                .collect();
            assert_eq!(links.len(), usize::from(snake::COUNT));
            let mut shade = 0u64;
            for (_, _, mesh) in &links {
                let Some(VertexAttributeValues::Float32x4(tints)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("colors");
                };
                for color in tints {
                    for v in color {
                        shade = shade.wrapping_mul(1099511628211) ^ u64::from(v.to_bits());
                    }
                }
            }
            assert!(colors.insert(shade), "duplicate skin {variant}");
            for phase in [0.2, 1.6, 3.8] {
                let m = Motion {
                    kind: Kind::Snake,
                    behavior: Behavior::Attack,
                    phase,
                    clock: phase,
                    speed: 2.1,
                    attack: 0.42,
                    sleep: 0.,
                    worm_progress: 0.,
                    worm_spit: None,
                    silk_length: 0.,
                    youth: 0.,
                    jump: Default::default(),
                };
                let pose = |(part, pivot, _): &(Part, Vec3, Mesh)| {
                    let mut transform = Transform::from_translation(*pivot);
                    animate(
                        &m,
                        &Limb(*part, *pivot),
                        &mut transform,
                        &mut Visibility::Inherited,
                    );
                    transform
                };
                for pair in links.windows(2) {
                    let a = pose(pair[0]);
                    let b = pose(pair[1]);
                    let joint = (a.transform_point(Vec3::Z * snake::STEP * 0.5)
                        + b.transform_point(-Vec3::Z * snake::STEP * 0.5))
                        * 0.5;
                    assert!(
                        contains(&pair[0].2, a, joint) && contains(&pair[1].2, b, joint),
                        "skin {variant} phase {phase} lost contact at {:?}",
                        pair[0].0
                    );
                }
            }
        }
    }

    #[test]
    fn enlarged_elephant_uses_the_existing_rig_scale() {
        assert!(
            crate::creatures::model_scale(Kind::Elephant).abs_diff_eq(Vec3::splat(1.55), 0.0001)
        );
    }
}
