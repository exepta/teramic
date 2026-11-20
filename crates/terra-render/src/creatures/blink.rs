//! Eyelids close from their fixed upper rim, sharing the head's rigid pose.
use super::{Limb, Motion, model::Part};
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

pub(super) fn closure(clock: f32, sleep: f32) -> f32 {
    if sleep > 0.5 {
        return 1.0;
    }
    let phase = clock.rem_euclid(6.7);
    if phase < 0.055 {
        phase / 0.055
    } else if phase < 0.085 {
        1.0
    } else {
        ((0.16 - phase) / 0.075).clamp(0.0, 1.0)
    }
}

pub(super) fn apply(motion: &Motion, part: &Limb, transform: &mut Transform) {
    if !matches!(part.0, Part::Eyelids) {
        return;
    }
    let top = match motion.kind {
        Kind::Sheep | Kind::Cow | Kind::Pig => 0.105 + 0.05,
        Kind::Bear => 0.11 + 0.05,
        Kind::Horse => 0.095 + 0.0525,
        _ => return,
    };
    let amount = closure(motion.clock, motion.sleep);
    let scale = transform.scale.y;
    transform.scale.y *= amount.max(0.0001);
    transform.translation += transform.rotation * Vec3::Y * (top * (1.0 - amount) * scale);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lid_closes_holds_and_reopens_without_an_instant_full_eye_flash() {
        assert_eq!(closure(0.0, 0.0), 0.0);
        assert!((closure(0.0275, 0.0) - 0.5).abs() < 1e-6);
        assert_eq!(closure(0.07, 0.0), 1.0);
        assert!((closure(0.1225, 0.0) - 0.5).abs() < 1e-6);
        assert_eq!(closure(0.17, 0.0), 0.0);
        assert_eq!(closure(1.0, 1.0), 1.0);
    }

    #[test]
    fn half_blink_keeps_the_upper_rim_fixed_under_head_rotation_and_youth_scale() {
        for (kind, top) in [
            (Kind::Sheep, 0.155),
            (Kind::Cow, 0.155),
            (Kind::Pig, 0.155),
            (Kind::Bear, 0.16),
            (Kind::Horse, 0.1475),
        ] {
            let motion = Motion {
                kind,
                behavior: super::super::Behavior::Rest,
                phase: 0.0,
                clock: 0.0275,
                speed: 0.0,
                sleep: 0.0,
                attack: 0.0,
                worm_progress: 0.0,
                worm_spit: None,
                silk_length: 0.0,
                youth: 1.0,
                jump: Default::default(),
            };
            let part = Limb(Part::Eyelids, Vec3::new(0.0, 1.2, -0.5));
            let head = Transform::from_translation(part.1)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, -0.65, 0.4, 0.1))
                .with_scale(Vec3::splat(1.12));
            let mut lid = head;
            apply(&motion, &part, &mut lid);
            assert!(
                lid.transform_point(Vec3::Y * top)
                    .abs_diff_eq(head.transform_point(Vec3::Y * top), 1e-6)
            );
            assert!((lid.scale.y / head.scale.y - 0.5).abs() < 1e-6);
            assert_eq!(lid.rotation, head.rotation);
        }
    }
}
