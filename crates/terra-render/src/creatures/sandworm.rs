//! Cached articulated presentation follows the same phase spine as server hitboxes.
mod ground;
mod trail;
use super::{Behavior as B, Limb, Motion, model::Part};
use bevy::prelude::*;
pub(super) use ground::WaveGround;
use terra_core::creatures::sandworm::{self as pose};

pub(super) fn install(app: &mut App) {
    ground::install(app);
    trail::install(app);
}

pub(super) fn presentation_progress(
    previous: B,
    current: f32,
    behavior: B,
    remaining: f32,
    dt: f32,
) -> f32 {
    let target = pose::phase_progress(behavior, remaining);
    if previous != behavior || (target - current).abs() > 0.20 {
        return target;
    }
    // Dropping one or two packets must not replay the same part of the lunge.
    let Some(duration) = pose::phase_duration(behavior) else {
        return target;
    };
    current + (target - current).clamp(0., 1.4 * dt / duration)
}

pub(super) fn animate(m: &Motion, part: &Limb, t: &mut Transform, visibility: &mut Visibility) {
    t.translation = part.1;
    t.rotation = Quat::IDENTITY;
    t.scale = Vec3::ONE;
    let p = m.worm_progress;
    if let Part::WormSpit(index) = part.0 {
        let time = (m.behavior == B::Spit && m.worm_spit.is_some())
            .then_some(p * pose::SPIT_SECONDS - f32::from(index) * 0.035);
        if m.behavior == B::Spit
            && let Some(time) = time.filter(|t| *t >= 0.0)
        {
            let launch = pose::spine_pose(B::Spit, 0.0, pose::HEAD_SEGMENT);
            t.translation = Vec3::from_array(pose::mouth_offset(launch))
                + Vec3::from_array(m.worm_spit.unwrap_or([0.0; 3])) * time
                - Vec3::Y * (0.5 * pose::SPIT_GRAVITY * time * time);
            t.translation.x += (f32::from(index) - 1.0) * 0.035;
            t.rotation = Quat::from_rotation_x(-time * 1.8);
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    if let Part::WormRipple(index) = part.0 {
        ripple(m, index, t, visibility);
        return;
    }
    if matches!(part.0, Part::WormDust) {
        let (visible, burst) = match m.behavior {
            B::Emerge => (true, (p * std::f32::consts::PI).sin().max(0.0)),
            B::Leap => {
                let at_lip = if p < 0.44 {
                    (p - 0.12) / 0.14
                } else {
                    (p - 0.79) / 0.21
                };
                let energy = (1. - at_lip * at_lip).clamp(0., 1.);
                (energy > 0., energy * energy)
            }
            B::Attack => (p < 0.26, (1.0 - p / 0.26).max(0.0) * 0.65),
            B::Retreat => (true, (p * std::f32::consts::PI).sin().max(0.0) * 0.65),
            B::Burrowed | B::Tremor => (false, 0.0),
            _ => (false, 0.0),
        };
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if m.behavior == B::Leap && p > 0.45 {
            t.translation.z -= pose::LEAP_DISTANCE;
        }
        t.translation.y += burst * 1.55;
        t.rotation = Quat::from_rotation_y(m.clock * 0.20);
        t.scale = Vec3::new(1.0 + burst * 0.9, 1.0 + burst * 1.3, 1.0 + burst * 0.9);
        return;
    }
    if matches!(part.0, Part::WormMound) {
        if matches!(m.behavior, B::Burrowed | B::Hunt) && m.speed > 0.03 || m.behavior == B::Tremor
        {
            // This is displaced sand, not exposed worm anatomy. A continuous
            // low advancing crest remains visible between the travelling rings.
            let charge = m.behavior == B::Hunt;
            let pulse = (m.clock * if charge { 9.0 } else { 5.2 }).sin() * 0.025;
            t.translation.y = if charge { 0.24 } else { 0.18 } + pulse;
            t.translation.z = -0.20;
            t.scale = if charge {
                Vec3::new(1.18, 2.1, 1.8)
            } else {
                Vec3::new(1.12, 1.8, 1.55)
            };
            *visibility = Visibility::Inherited;
            return;
        }
        if matches!(m.behavior, B::Burrowed | B::Hunt | B::Leap) {
            *visibility = Visibility::Hidden;
            return;
        }
        let warning = if m.behavior == B::Tremor {
            (m.clock * 22.0).sin() * 0.025
        } else {
            0.0
        };
        t.translation.y += warning;
        t.scale.y = if matches!(m.behavior, B::Emerge | B::Attack | B::Retreat) {
            0.65
        } else {
            1.0
        };
        *visibility = Visibility::Inherited;
        return;
    }
    let visible = matches!(
        m.behavior,
        B::Leap | B::Emerge | B::Attack | B::Spit | B::Retreat | B::Dead
    );
    *visibility = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let index = match part.0 {
        Part::WormSegment(index) => usize::from(index),
        _ => pose::HEAD_SEGMENT,
    };
    let spine = pose::spine_pose_at(m.behavior, p, m.phase, index);
    if !pose::anatomy_visible(m.behavior, spine, index) {
        *visibility = Visibility::Hidden;
    }
    t.translation = Vec3::from_array(spine.center);
    t.rotation = Quat::from_rotation_x(spine.pitch);
    if let Part::WormSegment(index) = part.0 {
        let pulse = (m.phase * 2.7 - f32::from(index) * 0.62).cos();
        let width = 1.0 + pulse * 0.064;
        t.scale = Vec3::new(width, width, width.recip().powi(2));
    }
    if !matches!(part.0, Part::WormMandible(_) | Part::Jaw) {
        return;
    }
    let spit_gape = match m.behavior {
        // The ranged opener joins Attack after the bite. Wind the throat up
        // through that same telegraph and relax if the shot is cancelled.
        B::Attack => {
            let t = if p <= pose::SPIT_PHASE {
                (p - 0.36) / (pose::SPIT_PHASE - 0.36)
            } else {
                1.0 - (p - pose::SPIT_PHASE) / 0.18
            }
            .clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
        B::Spit => (1.0 - p).powi(3),
        _ => 0.0,
    };
    let bite = if m.behavior == B::Attack {
        pose::bite_closure(p)
    } else {
        0.0
    };
    if let Part::WormMandible(index) = part.0 {
        // The shell, gum and teeth swing around one root. The pre-bite gape
        // and bile recoil read differently even when seen in silhouette.
        let angle = match m.behavior {
            B::Attack => {
                let ready = (p / pose::BITE_PHASE).clamp(0.0, 1.0);
                0.92 * bite - 0.19 * (ready * std::f32::consts::PI).sin() * (1.0 - bite)
            }
            B::Emerge => 0.25 * (1.0 - p).powi(2),
            B::Retreat => 0.33 * p * p,
            B::Leap => 0.18,
            _ => 0.0,
        } - 0.20 * spit_gape;
        let (hinge, axis) = super::model::sandworm::mandible_hinge(index);
        let swing = Quat::from_axis_angle(axis, angle);
        t.translation += t.rotation * (hinge - swing * hinge);
        t.rotation *= swing;
    } else {
        let width = 1.0 - 0.34 * bite + 0.14 * spit_gape;
        t.scale = Vec3::new(width, width, 1.0);
        t.translation += t.rotation * (Vec3::NEG_Z * (0.08 * bite + 0.13 * spit_gape));
    }
}

/// Fixed-size surface ridges travel across the dune in staggered annuli. Only
/// transforms change; all three meshes stay cached for every worm instance.
fn ripple(m: &Motion, patch: u8, t: &mut Transform, visibility: &mut Visibility) {
    let index = patch / 8;
    let sector = f32::from(patch % 8) * std::f32::consts::FRAC_PI_4;
    let (elapsed, intensity, repeating) = match m.behavior {
        B::Tremor => (m.worm_progress * pose::WARNING_SECONDS, 1.0, true),
        B::Leap => (m.worm_progress * pose::LEAP_SECONDS, 0.85, true),
        B::Emerge => (m.worm_progress * pose::EMERGE_SECONDS, 1.0, false),
        B::Attack if m.worm_progress < 0.45 => {
            (m.worm_progress * pose::EXPOSED_SECONDS + 0.25, 0.65, false)
        }
        B::Retreat => (m.worm_progress * pose::RETREAT_SECONDS, 1.25, false),
        B::Burrowed if m.speed > 0.03 => (m.clock, 1.35, true),
        B::Hunt if m.speed > 0.03 => (m.clock * 1.3, 1.65, true),
        _ => {
            *visibility = Visibility::Hidden;
            return;
        }
    };
    let time = elapsed - f32::from(index) * 0.26;
    let wave = if repeating {
        time.rem_euclid(1.20) / 1.20
    } else {
        time / 1.35
    };
    *visibility = if (0.0..=1.0).contains(&wave) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let wave = wave.clamp(0.0, 1.0);
    let envelope = (wave * std::f32::consts::PI).sin().max(0.0);
    t.translation.y = -0.025;
    t.translation.z = if m.behavior == B::Leap && patch % 2 == 1 {
        -pose::LEAP_DISTANCE
    } else {
        0.0
    };
    let spread = 0.72 + wave * 2.35;
    t.scale = Vec3::new(spread, intensity * envelope, spread);
    t.rotation = Quat::from_rotation_y(sector);
    t.translation.x += sector.sin() * 1.20 * spread;
    t.translation.z += sector.cos() * 1.20 * spread;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn motion(behavior: B, worm_progress: f32) -> Motion {
        Motion {
            kind: super::super::Kind::SandWorm,
            behavior,
            phase: 0.0,
            clock: 0.0,
            speed: 0.0,
            sleep: 0.0,
            attack: 1.0,
            worm_progress,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
        }
    }

    #[test]
    fn airborne_and_diving_meshes_use_the_same_spine_as_hit_targets() {
        for behavior in [B::Leap, B::Emerge, B::Attack, B::Retreat] {
            for p in [0.0, 0.2, 0.5, 0.8, 1.0] {
                let m = motion(behavior, p);
                for (part, index) in [
                    (Part::Head, pose::HEAD_SEGMENT),
                    (Part::WormSegment(0), 0),
                    (Part::WormSegment(4), 4),
                ] {
                    let mut t = Transform::IDENTITY;
                    let mut v = Visibility::Inherited;
                    animate(&m, &Limb(part, Vec3::ZERO), &mut t, &mut v);
                    let expected = pose::spine_pose_at(behavior, p, m.phase, index);
                    assert!((t.translation - Vec3::from_array(expected.center)).length() < 1e-5);
                    assert!(
                        t.rotation
                            .abs_diff_eq(Quat::from_rotation_x(expected.pitch), 1e-5)
                    );
                }
            }
        }
    }

    #[test]
    fn circular_fang_fans_close_at_the_bite_then_reopen_before_the_bile_arc() {
        let part = Limb(Part::WormMandible(0), Vec3::ZERO);
        let mut open = Transform::IDENTITY;
        let mut closed = Transform::IDENTITY;
        let mut visibility = Visibility::Inherited;
        animate(&motion(B::Attack, 0.0), &part, &mut open, &mut visibility);
        animate(
            &motion(B::Attack, pose::BITE_PHASE),
            &part,
            &mut closed,
            &mut visibility,
        );
        assert!(open.translation.distance(closed.translation) > 0.3);
        assert!(!open.rotation.abs_diff_eq(closed.rotation, 0.01));
        let mut projectile = Transform::IDENTITY;
        let spit = Limb(Part::WormSpit(0), Vec3::ZERO);
        animate(
            &motion(B::Attack, pose::SPIT_PHASE - 0.01),
            &spit,
            &mut projectile,
            &mut visibility,
        );
        assert_eq!(visibility, Visibility::Hidden);
        let elapsed = 0.40;
        let mut aimed = motion(B::Spit, elapsed / pose::SPIT_SECONDS);
        aimed.worm_spit = Some([0.0, pose::SPIT_UPWARD, -pose::SPIT_SPEED]);
        animate(&aimed, &spit, &mut projectile, &mut visibility);
        let launch = pose::spine_pose(B::Spit, 0.0, pose::HEAD_SEGMENT);
        let expected = Vec3::from_array(pose::mouth_offset(launch))
            + Vec3::from_array(pose::spit_offset(elapsed))
            - Vec3::X * 0.035;
        assert_eq!(visibility, Visibility::Inherited);
        assert!(projectile.translation.distance(expected) < 1e-4);
    }

    #[test]
    fn fangs_pivot_at_the_gumline_and_meet_in_the_center_at_contact() {
        for index in 0..3 {
            let part = Limb(Part::WormMandible(index), Vec3::ZERO);
            let (hinge, _) = super::super::model::sandworm::mandible_hinge(index);
            let tip = hinge.truncate().normalize().extend(0.0) * 0.39 - Vec3::Z * 0.78;
            for (behavior, progress) in [
                (B::Attack, 0.0),
                (B::Attack, pose::BITE_PHASE),
                (B::Attack, pose::SPIT_PHASE),
                (B::Spit, 0.0),
                (B::Emerge, 0.5),
                (B::Retreat, 0.5),
                (B::Leap, 0.5),
            ] {
                let spine = pose::spine_pose(behavior, progress, pose::HEAD_SEGMENT);
                let head = Transform::from_translation(Vec3::from_array(spine.center))
                    .with_rotation(Quat::from_rotation_x(spine.pitch));
                let mut jaw = Transform::IDENTITY;
                let mut visibility = Visibility::Inherited;
                animate(
                    &motion(behavior, progress),
                    &part,
                    &mut jaw,
                    &mut visibility,
                );
                assert!(
                    head.transform_point(hinge)
                        .distance(jaw.transform_point(hinge))
                        < 1e-5
                );
                let in_mouth =
                    head.rotation.inverse() * (jaw.transform_point(tip) - head.translation);
                let clearance = in_mouth.truncate().length();
                if behavior == B::Attack && progress == pose::BITE_PHASE {
                    assert!(clearance < 0.08);
                } else if behavior == B::Spit
                    || (behavior == B::Attack && progress == pose::SPIT_PHASE)
                {
                    assert!(
                        clearance > 0.50,
                        "the mouth must visibly gape before a ranged shot"
                    );
                } else if behavior == B::Attack {
                    assert!(clearance >= 0.38);
                }
            }
        }
    }

    #[test]
    fn ground_ridges_expand_then_settle_and_are_hidden_for_a_resting_worm() {
        let mut t = Transform::IDENTITY;
        let mut v = Visibility::Inherited;
        ripple(&motion(B::Emerge, 0.2), 0, &mut t, &mut v);
        let before = t.scale.x;
        assert_eq!(v, Visibility::Inherited);
        assert!(t.scale.y > 0.0);
        ripple(&motion(B::Emerge, 0.8), 0, &mut t, &mut v);
        assert!(t.scale.x > before + 1.0);
        ripple(&motion(B::Burrowed, 0.0), 0, &mut t, &mut v);
        assert_eq!(v, Visibility::Hidden);
    }

    #[test]
    fn a_moving_burrow_always_has_a_raised_sand_crest_between_wave_pulses() {
        for (behavior, clock) in [B::Burrowed, B::Hunt]
            .into_iter()
            .flat_map(|b| [0.0, 0.1, 0.7, 1.2, 1.9, 4.7].map(|clock| (b, clock)))
        {
            let mut m = motion(behavior, 0.0);
            m.speed = 0.25;
            m.attack = 0.0;
            m.clock = clock;
            let mut transform = Transform::IDENTITY;
            let mut visible = Visibility::Hidden;
            animate(
                &m,
                &Limb(Part::WormMound, Vec3::ZERO),
                &mut transform,
                &mut visible,
            );
            assert_eq!(visible, Visibility::Inherited);
            // The cached mound's peak is6cm above its origin. Its advancing
            // crest must rise clearly above sand, rather than becoming a speck.
            assert!(transform.translation.y + transform.scale.y * 0.06 > 0.24);
            for part in [Part::Head, Part::WormSegment(18)] {
                animate(&m, &Limb(part, Vec3::ZERO), &mut transform, &mut visible);
                assert_eq!(visible, Visibility::Hidden);
            }
        }
    }

    #[test]
    fn ring_pose_bends_the_neck_and_keeps_all_phase_transforms_finite() {
        let mut motion = Motion {
            kind: super::super::Kind::SandWorm,
            behavior: B::Attack,
            phase: 0.0,
            clock: 0.0,
            speed: 0.0,
            sleep: 0.0,
            attack: 1.0,
            worm_progress: 1.0,
            worm_spit: None,
            silk_length: 0.0,
            youth: 0.0,
            jump: Default::default(),
        };
        let mut angles = Vec::new();
        for i in 0..pose::BODY_SEGMENTS as u8 {
            let mut t = Transform::IDENTITY;
            let mut v = Visibility::Inherited;
            animate(
                &motion,
                &Limb(Part::WormSegment(i), Vec3::ZERO),
                &mut t,
                &mut v,
            );
            angles.push(t.rotation.to_euler(EulerRot::XYZ).0);
        }
        assert!(angles[18] > 0.8 && (0.42..0.70).contains(&angles[23]));
        for behavior in [
            B::Burrowed,
            B::Tremor,
            B::Leap,
            B::Emerge,
            B::Attack,
            B::Retreat,
        ] {
            motion.behavior = behavior;
            for part in [
                Part::Head,
                Part::Jaw,
                Part::WormSegment(3),
                Part::WormMandible(1),
                Part::WormSpit(0),
                Part::WormDust,
                Part::WormMound,
            ] {
                let mut t = Transform::IDENTITY;
                let mut v = Visibility::Inherited;
                animate(&motion, &Limb(part, Vec3::ZERO), &mut t, &mut v);
                assert!(t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite());
                if !matches!(part, Part::WormRipple(_) | Part::WormMound)
                    && matches!(behavior, B::Burrowed | B::Hunt | B::Tremor)
                {
                    assert_eq!(v, Visibility::Hidden);
                }
            }
        }
    }
}

#[cfg(test)]
mod hidden_anatomy_tests {
    use super::*;
    #[test]
    fn no_anatomical_mesh_is_exposed_while_the_sand_crest_moves() {
        for behavior in [B::Burrowed, B::Hunt, B::Tremor] {
            let m = Motion {
                kind: super::super::Kind::SandWorm,
                behavior,
                phase: 1.0,
                clock: 2.0,
                speed: 2.0,
                sleep: 0.0,
                attack: 0.0,
                worm_progress: 0.5,
                worm_spit: None,
                silk_length: 0.0,
                youth: 0.0,
                jump: Default::default(),
            };
            for part in (0..pose::BODY_SEGMENTS as u8)
                .map(Part::WormSegment)
                .chain([
                    Part::Head,
                    Part::Jaw,
                    Part::WormDust,
                    Part::WormMandible(0),
                    Part::WormSpit(0),
                ])
            {
                let mut t = Transform::IDENTITY;
                let mut visible = Visibility::Inherited;
                animate(&m, &Limb(part, Vec3::ZERO), &mut t, &mut visible);
                assert_eq!(visible, Visibility::Hidden, "{behavior:?} leaked {part:?}");
            }
        }
    }
}
