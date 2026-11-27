use super::{Behavior as B, Motion, model::stone_golem::Joint};
use bevy::prelude::*;
fn smooth(x: f32) -> f32 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
// A partly fallen guardian can wake immediately after a hit. Hold its current
// assembly until the replicated wake clock catches up, then join that motion.
pub(super) fn rubble_amount(action: B, phase: f32, previous: f32, dt: f32) -> f32 {
    match action {
        B::Sleep | B::Rest => (previous + dt / 1.05).min(1.),
        B::Emerge => previous.min(1. - phase),
        _ => 0.,
    }
}

// Each pose places the mesh's joint origin, which can be near an end of a long
// stone. Offsets and quarter turns put a broad face of each stone on the soil.
fn fallen(j: Joint) -> (Transform, f32, f32) {
    use std::f32::consts::FRAC_PI_2;
    let (position, yaw, tip, start, end) = match j {
        Joint::Chest => (
            Vec3::new(-0.05, 0.735, -0.32),
            -0.18,
            Quat::from_rotation_x(FRAC_PI_2),
            0.13,
            0.70,
        ),
        Joint::Face => (
            Vec3::new(0.26, 0.56, -1.39),
            0.46,
            Quat::from_rotation_z(FRAC_PI_2),
            0.43,
            0.94,
        ),
        Joint::Shoulder(s) => {
            let side = f32::from(s);
            (
                Vec3::new(side * 1.10, 0.43, 0.10),
                -0.41 * side,
                Quat::from_rotation_z(side * FRAC_PI_2),
                0.29,
                0.79,
            )
        }
        Joint::Forearm(s) => (
            if s < 0 {
                Vec3::new(-1.00, 0.365, 1.30)
            } else {
                Vec3::new(1.68, 0.365, -0.40)
            },
            if s < 0 { 1.39 } else { -0.20 },
            Quat::from_rotation_x(FRAC_PI_2),
            0.35,
            0.87,
        ),
        Joint::Leg(s) => {
            let side = f32::from(s);
            (
                Vec3::new(side * 0.83, 0.46, 0.91),
                if s < 0 { 2.96 } else { 0.36 },
                Quat::from_rotation_x(FRAC_PI_2),
                0.04,
                0.50,
            )
        }
        Joint::Foot(s) => {
            let side = f32::from(s);
            (
                if s < 0 {
                    Vec3::new(-1.36, 0.60, -0.94)
                } else {
                    Vec3::new(1.09, 0.60, 1.46)
                },
                -side * 0.84,
                Quat::IDENTITY,
                0.02,
                0.37,
            )
        }
        Joint::Seal | Joint::Shock | Joint::Chips => {
            unreachable!("effect meshes have their own trajectory")
        }
    };
    let rest =
        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw) * tip);
    (rest, start, end)
}

fn assemble(m: &Motion, j: Joint, upright: Transform) -> Transform {
    if m.sleep <= 0. {
        return upright;
    }
    let (rest, start, end) = fallen(j);
    let t = smooth((1. - m.sleep - start) / (end - start));
    let arc = 4. * t * (1. - t);
    let side = rest.translation.x.signum();
    Transform {
        translation: rest.translation.lerp(upright.translation, t)
            + Vec3::new(0.13 * side, 0.43, -0.08 * side) * arc,
        rotation: Quat::from_rotation_y(side * 0.28 * arc)
            * rest.rotation.slerp(upright.rotation, t),
        scale: Vec3::ONE,
    }
}

fn effect(m: &Motion, j: Joint) -> Option<Transform> {
    use std::f32::consts::{PI, TAU};
    let p = m.attack;
    let mut t = Transform::from_translation(j.pivot());
    match m.behavior {
        B::Emerge if j != Joint::Shock && p > 0. && p < 0.98 => {
            // A broken ring gathers at the feet, turns, then contracts at the
            // chest. Loose chips spiral inward on a slightly different beat.
            let turn = smooth(p / 0.98);
            let chips = j == Joint::Chips;
            let radius =
                (2.30 - 0.30 * turn) * smooth(p / 0.095) * (1. - smooth((p - 0.62) / 0.36));
            t.translation.y += if chips { 1.85 * turn } else { 2.80 * turn };
            t.rotation = Quat::from_rotation_y(turn * TAU * if chips { -0.49 } else { 0.68 });
            t.scale = Vec3::new(radius, (p / 0.98 * PI).sin() * 1.3, radius);
        }
        B::Tremor if j != Joint::Seal => {
            let pulse = (p - terra_core::creatures::golem::SLAM_IMPACT) / 0.34;
            if !(0.0..1.).contains(&pulse) {
                return None;
            }
            let r = 0.75 + pulse * 3.65;
            t.scale = Vec3::new(r, 1.3 * (1. - pulse), r);
            if j == Joint::Chips {
                t.translation.y += (pulse * PI).sin() * 0.68;
            }
        }
        _ => return None,
    }
    Some(t)
}

pub(super) fn animate(
    m: &Motion,
    j: Joint,
    lit: bool,
    t: &mut Transform,
    visible: &mut Visibility,
) {
    if matches!(j, Joint::Seal | Joint::Shock | Joint::Chips) {
        if let Some(pose) = effect(m, j) {
            *visible = Visibility::Inherited;
            *t = pose;
        } else {
            *visible = Visibility::Hidden;
        }
        return;
    }
    // Reveal the shoulder seals, wrist seals and eyes in sequence. Each inlay
    // shares its parent stone's complete transform, including its local pivot.
    if lit {
        let ignition = match j {
            Joint::Face => 0.82,
            Joint::Forearm(_) => 0.33,
            _ => 0.15,
        };
        *visible = if 1. - m.sleep >= ignition {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if m.sleep >= 1. {
        *t = fallen(j).0;
        return;
    }
    let p = m.attack;
    let idle = (m.clock * 1.25).sin();
    let walk = (m.speed / 0.80).clamp(0., 1.);
    let gait = m.phase.sin() * walk;
    let transfer = m.phase.cos() * walk;
    let awakening = if m.behavior == B::Emerge {
        (p * std::f32::consts::PI).sin()
    } else {
        0.
    };
    let slam = m.behavior == B::Tremor;
    let attack = matches!(m.behavior, B::Tremor | B::Attack);
    let contact = if slam {
        terra_core::creatures::golem::SLAM_IMPACT
    } else {
        terra_core::creatures::definitions::MobCombat::MELEE_IMPACT_PHASE
    };
    let wind = if attack {
        smooth(p / (contact - 0.12))
    } else {
        0.
    };
    let hit = if attack {
        smooth((p - contact + 0.12) / 0.12)
    } else {
        0.
    };
    let release = if attack {
        smooth((p - contact) / (1. - contact))
    } else {
        0.
    };
    let recoil = if m.behavior == B::Shake {
        (p * std::f32::consts::PI).sin()
    } else {
        0.
    };
    let crouch = 0.32 * (hit - release) + 0.12 * recoil;
    let body = Quat::from_euler(
        EulerRot::XYZ,
        -0.16 * (hit - release) + 0.18 * recoil,
        0.12 * gait + 0.15 * awakening,
        0.078 * transfer,
    );
    let shift = Vec3::new(
        0.115 * transfer,
        -crouch + 0.066 * gait.abs() + 0.013 * idle,
        0.,
    );
    let mut rot = Quat::IDENTITY;
    let mut pos = j.pivot();
    let shoulder = |s: f32| {
        let weight = if s > 0. || slam { 1. } else { 0.20 };
        let swing = if slam {
            2.65 * wind - 2.31 * hit - 0.34 * release
        } else {
            -0.68 * wind + 2.02 * hit - 1.34 * release
        };
        Quat::from_euler(
            EulerRot::XYZ,
            gait * s * 0.54 + weight * swing + awakening * (0.16 + s * 0.23),
            s * (wind - hit) * 0.11,
            s * (0.04 + 0.16 * (wind - hit)),
        )
    };
    match j {
        Joint::Seal | Joint::Shock | Joint::Chips => unreachable!(),
        Joint::Face => {
            rot = Quat::from_euler(
                EulerRot::XYZ,
                -0.11 * (wind - hit) + 0.15 * recoil,
                0.09 * (m.clock * 0.8).sin() - 0.10 * gait - 0.24 * awakening,
                0.,
            );
        }
        Joint::Shoulder(s) => rot = shoulder(f32::from(s)),
        Joint::Forearm(s) => {
            let arm = Joint::Shoulder(s).pivot();
            let shoulder = shoulder(f32::from(s));
            pos = arm + shoulder * (pos - arm);
            rot = shoulder
                * Quat::from_rotation_x(0.11 + 0.47 * (wind - hit) + 0.13 * gait * f32::from(s));
        }
        Joint::Leg(s) | Joint::Foot(s) => {
            let side = f32::from(s);
            let swing = (m.phase.cos() * side).max(0.) * walk;
            let hip = Quat::from_rotation_x(gait * side * 0.44);
            if matches!(j, Joint::Foot(_)) {
                // The supporting sole stays flat. The other heel rolls off the
                // soil and clears the ground before its opposite number lands.
                pos.z += gait * side * 0.43;
                pos.y += 0.27 * swing;
                rot = Quat::from_rotation_x(0.24 * gait * side * swing);
            } else {
                rot = hip;
                pos.y -= crouch * 0.22;
                pos.y += swing * 0.09;
                pos.x -= transfer * 0.045;
            }
            *t = assemble(m, j, Transform::from_translation(pos).with_rotation(rot));
            return;
        }
        Joint::Chest => {}
    }
    let pivot = Joint::Chest.pivot();
    pos = pivot + body * (pos - pivot) + shift;
    rot = body * rot;
    *t = assemble(m, j, Transform::from_translation(pos).with_rotation(rot));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{Kind, model};
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn folded_stones_touch_the_floor_and_runic_assembly_clears_it() {
        let mut motion = Motion {
            kind: Kind::StoneGolem,
            behavior: B::Sleep,
            phase: 0.,
            clock: 0.,
            speed: 0.,
            sleep: 1.,
            attack: 0.,
            worm_progress: 0.,
            worm_spit: None,
            silk_length: 0.,
            youth: 0.,
            jump: Default::default(),
        };
        for variant in [0, 1] {
            let parts = model::parts_variant(Kind::StoneGolem, variant).unwrap();
            for frame in 0..=24 {
                motion.attack = frame as f32 / 24.;
                motion.behavior = if frame == 0 { B::Sleep } else { B::Emerge };
                motion.sleep = rubble_amount(motion.behavior, motion.attack, 1., 0.);
                for (part, _, mesh) in &parts {
                    let model::Part::Golem(j, lit) = *part else {
                        unreachable!()
                    };
                    let mut transform = Transform::IDENTITY;
                    let mut visible = Visibility::Inherited;
                    animate(&motion, j, lit, &mut transform, &mut visible);
                    if matches!(j, Joint::Seal | Joint::Shock | Joint::Chips) {
                        assert_eq!(
                            visible == Visibility::Hidden,
                            j == Joint::Shock || frame == 0 || frame == 24
                        );
                        continue;
                    }
                    if lit {
                        if frame == 0 || frame == 24 {
                            assert_eq!(visible == Visibility::Hidden, frame == 0);
                        }
                        let mut rock = Transform::IDENTITY;
                        animate(&motion, j, false, &mut rock, &mut visible);
                        assert_eq!(transform, rock);
                        continue;
                    }
                    let Some(VertexAttributeValues::Float32x3(vertices)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        unreachable!()
                    };
                    let mut floor = f32::INFINITY;
                    let mut top = f32::NEG_INFINITY;
                    for vertex in vertices {
                        let y = transform.transform_point(Vec3::from_array(*vertex)).y;
                        floor = floor.min(y);
                        top = top.max(y);
                    }
                    assert!(floor >= -0.002, "{j:?} phase {frame}: below ground {floor}");
                    if frame == 0 {
                        assert!(floor < 0.015, "{j:?}: hovering at {floor}");
                        assert!(top < 1.55, "{j:?}: upright in sleep at {top}");
                    }
                }
            }
        }
    }

    #[test]
    fn interrupted_collapse_resumes_from_its_actual_pose() {
        let half_fallen = rubble_amount(B::Sleep, 0., 0., 0.50);
        assert!((0.4..0.6).contains(&half_fallen));
        assert_eq!(rubble_amount(B::Emerge, 0., half_fallen, 0.01), half_fallen);
        assert!(rubble_amount(B::Emerge, 0.9, half_fallen, 0.01) < 0.11);
        assert_eq!(rubble_amount(B::Emerge, 1., half_fallen, 0.01), 0.);
    }
}
