use super::{Behavior, Kind, Limb, Motion, model::Part};
use bevy::prelude::*;

pub(super) fn animate(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let clock = motion.clock;
    transform.translation = part.1;
    transform.rotation = Quat::IDENTITY;
    if motion.kind == Kind::Butterfly {
        let perched = motion.behavior == Behavior::Nectar;
        transform.translation.y += if perched {
            0.0
        } else {
            (clock * 3.1).sin() * 0.018
        };
        if let Part::Wing(side) = part.0 {
            // Butterflies beat visibly, at a much slower rate than bee wings.
            let angle = if perched {
                0.9 + (clock * 1.7).sin() * 0.12
            } else {
                0.30 + (clock * 13.0).sin() * 0.95
            };
            transform.rotation = Quat::from_rotation_z(side * angle);
        }
    } else if motion.kind == Kind::SpiritGloom {
        animate_gloom(motion, part, transform);
    } else if motion.kind == Kind::BloodWisp {
        transform.translation.y += (clock * 1.8).sin() * 0.055;
        // Face and flower share the same breathing offset, keeping eyes attached.
        let breath = 1.0 + (clock * 1.8).sin() * 0.025;
        transform.scale = Vec3::new(1.0 / breath, breath, 1.0 / breath);
        if let Part::SpiritArm(side) = part.0 {
            transform.rotation = Quat::from_rotation_z(side * (clock * 1.6).sin() * 0.19);
        }
        if let Part::SpiritOrbit(side) = part.0 {
            let angle = clock * 0.85 + side * std::f32::consts::FRAC_PI_2;
            transform.translation = Vec3::new(
                angle.cos() * 0.39,
                0.55 + (clock * 1.7 + side).sin() * 0.12,
                angle.sin() * 0.28,
            );
            transform.rotation = Quat::from_rotation_y(-angle);
        }
        if matches!(part.0, Part::SpiritGlow) {
            transform.rotation = Quat::from_rotation_z((clock * 1.3).sin() * 0.08);
        }
    } else {
        let moving = (motion.speed / 1.5).clamp(0.0, 1.0);
        transform.translation.y += (motion.phase * 2.0).sin().abs() * 0.018 * moving;
        transform.rotation = match part.0 {
            Part::Leg(side) => Quat::from_rotation_x(motion.phase.sin() * side * moving * 0.55),
            Part::Tail => Quat::from_euler(
                EulerRot::XYZ,
                (clock * 1.3).sin() * 0.10,
                (clock * 1.1).sin() * 0.24,
                0.0,
            ),
            Part::Head => Quat::from_euler(
                EulerRot::XYZ,
                (clock * 1.4).sin() * 0.025,
                (clock * 0.7).sin() * 0.06,
                0.0,
            ),
            _ => Quat::IDENTITY,
        };
    }
}

fn animate_gloom(motion: &Motion, part: &Limb, transform: &mut Transform) {
    let clock = motion.clock;
    let moving = (motion.speed / 0.7).clamp(0.0, 1.0);
    let calm = matches!(
        motion.behavior,
        Behavior::Rest | Behavior::Sleep | Behavior::Nectar
    );
    let startled = motion.behavior == Behavior::Flee;
    let bank = (clock * 0.9).sin() * (if startled { 0.13 } else { 0.055 }) * moving;
    let lean = -0.10 * moving + (clock * 1.2).sin() * 0.025;
    let root = Quat::from_euler(EulerRot::XYZ, lean, (clock * 0.65).sin() * 0.035, bank);
    let center = Vec3::new(0.0, 0.45, 0.0);
    let bob = (clock * if calm { 1.1 } else { 1.8 }).sin() * if calm { 0.025 } else { 0.045 };
    transform.translation = center + root * (part.1 - center) + Vec3::Y * bob;
    transform.rotation = root;
    transform.scale = Vec3::ONE;
    match part.0 {
        Part::SpiritArm(side) => {
            let flutter = (clock
                * (if startled {
                    11.0
                } else if calm {
                    2.0
                } else {
                    4.0 + moving * 3.0
                })
                + side * 0.12)
                .sin();
            let opening = if calm { 0.055 } else { 0.22 };
            transform.rotation *= Quat::from_euler(
                EulerRot::YXZ,
                side * (0.10 + flutter * opening),
                flutter * 0.07,
                side * (0.06 + flutter * 0.12),
            );
        }
        Part::SpiritGlow => {
            transform.rotation *= Quat::from_euler(
                EulerRot::XYZ,
                (clock * 1.7).sin() * 0.08,
                (clock * 1.2).sin() * 0.12,
                (clock * 1.5).sin() * 0.15,
            );
        }
        Part::SpiritOrbit(_) => {
            // Short asymmetric intervals avoid a stare while both eyes close
            // together. The center stays attached to the rigid dark face.
            let blink_phase = clock.rem_euclid(5.7);
            let blink = if blink_phase < 0.16 {
                (blink_phase / 0.16 * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            transform.scale.y = 1.0 - blink * 0.92;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motion() -> Motion {
        Motion {
            kind: Kind::SpiritGloom,
            behavior: Behavior::Fly,
            phase: 0.0,
            clock: 0.0,
            speed: 0.65,
            sleep: 0.0,
            attack: 0.0,
            worm_progress: 0.0,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
        }
    }

    #[test]
    fn gloom_flight_rest_and_startled_reactions_have_distinct_finite_poses() {
        let mut motion = motion();
        motion.clock = 1.37;
        let mut rotations = Vec::new();
        for behavior in [Behavior::Fly, Behavior::Rest, Behavior::Flee] {
            motion.behavior = behavior;
            let mut pose = Transform::IDENTITY;
            animate(
                &motion,
                &Limb(Part::SpiritArm(1.0), Vec3::new(0.205, 0.465, 0.045)),
                &mut pose,
            );
            assert!(pose.translation.is_finite() && pose.rotation.is_finite());
            rotations.push(pose.rotation);
        }
        assert!(rotations[0].angle_between(rotations[1]) > 0.02);
        assert!(rotations[0].angle_between(rotations[2]) > 0.02);
    }
    #[test]
    fn gloom_eyes_stay_attached_to_the_face_while_wings_tail_and_blinks_move() {
        let mut motion = motion();
        let mut previous_wing = None;
        let mut wing_moves = false;
        let mut saw_blink = false;
        for n in 0..240 {
            motion.clock = n as f32 * 0.03;
            let mut head = Transform::IDENTITY;
            animate(&motion, &Limb(Part::Head, Vec3::ZERO), &mut head);
            let eye_center = Vec3::new(0.083, 0.46, -0.272);
            let mut eye = Transform::IDENTITY;
            animate(&motion, &Limb(Part::SpiritOrbit(1.0), eye_center), &mut eye);
            assert!(
                eye.translation
                    .abs_diff_eq(head.transform_point(eye_center), 1e-6)
            );
            assert!(eye.rotation.abs_diff_eq(head.rotation, 1e-6));
            saw_blink |= eye.scale.y < 0.25;
            let mut wing = Transform::IDENTITY;
            animate(
                &motion,
                &Limb(Part::SpiritArm(1.0), Vec3::new(0.205, 0.465, 0.045)),
                &mut wing,
            );
            if let Some(previous) = previous_wing {
                wing_moves |= wing.rotation.angle_between(previous) > 0.01;
                assert!(wing.rotation.angle_between(previous) < 0.20);
            }
            previous_wing = Some(wing.rotation);
            let mut tail = Transform::IDENTITY;
            animate(
                &motion,
                &Limb(Part::SpiritGlow, Vec3::new(0.0, 0.218, -0.225)),
                &mut tail,
            );
            assert!(tail.rotation.is_finite() && tail.translation.is_finite());
        }
        assert!(wing_moves && saw_blink);
    }
}
