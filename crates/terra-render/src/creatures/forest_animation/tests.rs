use super::*;

fn motion(kind: Kind) -> Motion {
    Motion {
        kind,
        behavior: Behavior::Wander,
        phase: 0.0,
        clock: 0.0,
        speed: 1.2,
        sleep: 0.0,
        attack: 0.0,
        worm_progress: 0.0,
        worm_spit: None,
        silk_length: 0.0,
        youth: 0.0,
        jump: Default::default(),
    }
}

fn pose(motion: &Motion, part: Part, pivot: Vec3) -> Transform {
    let mut pose = Transform::IDENTITY;
    animate(motion, &Limb(part, pivot), &mut pose);
    pose
}

#[test]
fn thorn_glowing_eyes_follow_the_mask_and_heart_pulses_in_its_socket() {
    let mut motion = motion(Kind::ThornBrute);
    let head_pivot = Vec3::new(0.0, 1.85, -0.025);
    let heart_center = Vec3::new(0.0, 1.35, -0.323);
    for frame in 0..120 {
        motion.clock = frame as f32 / 30.0;
        motion.phase = frame as f32 * 0.1;
        motion.attack = frame as f32 / 120.0;
        let head = pose(&motion, Part::Head, head_pivot);
        let eyes = pose(&motion, Part::ThornGlow { head: true }, head_pivot);
        assert_eq!(head, eyes);
        let heart = pose(&motion, Part::ThornGlow { head: false }, Vec3::ZERO);
        let body = pose(&motion, Part::Body, Vec3::ZERO);
        assert!(
            heart
                .transform_point(heart_center)
                .abs_diff_eq(heart_center + body.translation, 1e-6)
        );
    }
}

#[test]
fn separate_skeleton_pupils_follow_the_skull_through_walk_and_attack() {
    let mut motion = motion(Kind::Skeleton);
    let pivot = Vec3::new(0.0, 1.64, 0.0);
    for step in 0..=120 {
        motion.phase = step as f32 * std::f32::consts::TAU / 120.0;
        motion.attack = step as f32 / 120.0;
        motion.clock = step as f32 / 30.0;
        let head = pose(&motion, Part::Head, pivot);
        let eyes = pose(&motion, Part::SkeletonEyes, pivot);
        assert_eq!(head.translation, eyes.translation);
        assert_eq!(head.rotation, eyes.rotation);
        assert_eq!(head.scale, eyes.scale);
        assert!(head.rotation.is_finite() && head.translation.is_finite());
    }
}

#[test]
fn skeleton_footfall_and_attack_stages_have_continuous_motion() {
    let mut motion = motion(Kind::Skeleton);
    let arm = Part::VillagerArm(-1.0);
    let shoulder = Vec3::new(-0.29, 1.37, 0.0);
    for boundary in [0.0, 0.30, 0.72, 1.0] {
        motion.attack = boundary - 0.0001;
        let before = pose(&motion, arm, shoulder);
        motion.attack = boundary + 0.0001;
        let after = pose(&motion, arm, shoulder);
        assert!(before.rotation.angle_between(after.rotation) < 0.002);
        assert!(before.translation.distance(after.translation) < 0.001);
    }
    motion.attack = 0.0;
    let neutral = pose(&motion, arm, shoulder);
    motion.attack = 1.0;
    let recovered = pose(&motion, arm, shoulder);
    assert!(neutral.rotation.angle_between(recovered.rotation) < 0.001);
    assert!(neutral.translation.distance(recovered.translation) < 1e-6);
    motion.attack = 0.30;
    let raised = pose(&motion, arm, shoulder);
    assert!(neutral.rotation.angle_between(raised.rotation) > 0.8);
    motion.attack = 0.0;
    for phase in [0.0, std::f32::consts::PI] {
        motion.phase = phase - 0.001;
        let before = pose(&motion, Part::Leg(-1.0), Vec3::new(-0.125, 0.74, 0.0));
        motion.phase = phase + 0.001;
        let after = pose(&motion, Part::Leg(-1.0), Vec3::new(-0.125, 0.74, 0.0));
        assert!((before.translation.y - 0.74).abs() < 1e-6);
        assert!((after.translation.y - 0.74).abs() < 1e-6);
    }
}

#[test]
fn opaque_slime_face_stays_attached_to_the_blended_body_during_squash() {
    let mut motion = motion(Kind::BloodSlime);
    for step in 0..=100 {
        motion.clock = step as f32 * 0.07;
        let body = pose(&motion, Part::Body, Vec3::ZERO);
        let face = pose(&motion, Part::Head, Vec3::ZERO);
        assert_eq!(body.translation, face.translation);
        assert_eq!(body.rotation, face.rotation);
        assert_eq!(body.scale, face.scale);
        assert!((body.scale.element_product() - 1.0).abs() < 1e-5);
    }
}
