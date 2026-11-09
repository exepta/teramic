//! Rigid running gear follows authoritative travelled meters rather than wall time.
use crate::blocks::TrainAnimationRole;
use bevy::prelude::*;
use terra_core::{train_parts, trains::TrainPart};
use terra_world::{edits::EditJournal, streaming::WorldAccess};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Animation {
    pub pivot: Vec3,
    pub axis: Vec3,
    pub forward: Vec3,
    pub role: TrainAnimationRole,
    pub radius: f64,
}

#[derive(Component, Clone, Copy, Debug)]
pub(super) struct AnimatedPart {
    pub vehicle: u64,
    pub animation: Animation,
}

/// Mesh vertices already include the part offset and its authored cardinal pose.
pub(super) fn descriptor(part: &TrainPart, role: TrainAnimationRole) -> Animation {
    let rotation = Quat::from_rotation_y(f32::from(part.facing % 4) * std::f32::consts::FRAC_PI_2);
    let center = Vec3::splat(0.5);
    let local_pivot = match role {
        TrainAnimationRole::BogieWheel(index) => Vec3::new(
            if index < 2 { -0.5 } else { 1.5 },
            0.5,
            f32::from(index % 2),
        ),
        TrainAnimationRole::Throttle => {
            Vec3::from_array(terra_core::locomotive_parts::THROTTLE_PIVOT)
        }
        TrainAnimationRole::Wheel if part.material == 232 => Vec3::new(0.02, 0.90, 0.5),
        _ => center,
    };
    Animation {
        pivot: Vec3::from_array(part.offset.map(f32::from))
            + center
            + rotation * (local_pivot - center),
        axis: rotation * Vec3::X,
        forward: rotation * Vec3::Z,
        role,
        radius: terra_core::locomotive_parts::wheel_radius(part.material).unwrap_or(0.45),
    }
}

/// Distance is signed along this part's authored forward axis. Throttle is in
/// driver-facing notches; positive pushes the lever towards the cab console.
pub(super) fn pose(animation: Animation, distance: f64, throttle: i8) -> Transform {
    let distance = if distance.is_finite() { distance } else { 0.0 };
    let phase = (distance / animation.radius).rem_euclid(std::f64::consts::TAU) as f32;
    let (rotation, translation) = match animation.role {
        TrainAnimationRole::BogieWheel(_) | TrainAnimationRole::Wheel => {
            (Quat::from_axis_angle(animation.axis, phase), Vec3::ZERO)
        }
        TrainAnimationRole::Throttle => (
            Quat::from_axis_angle(animation.axis, -f32::from(throttle.clamp(-6, 6)) * 0.09),
            Vec3::ZERO,
        ),
        TrainAnimationRole::ConnectingRod | TrainAnimationRole::RodJoint => (
            Quat::IDENTITY,
            Vec3::Y * (-phase.sin() * 0.12) + animation.forward * ((phase.cos() - 1.0) * 0.12),
        ),
        TrainAnimationRole::Piston | TrainAnimationRole::Crosshead => (
            Quat::IDENTITY,
            animation.forward * ((phase.cos() - 1.0) * 0.12),
        ),
    };
    Transform {
        translation: animation.pivot - rotation * animation.pivot + translation,
        rotation,
        ..default()
    }
}

pub(super) fn animate(
    journal: Res<EditJournal>,
    session: Option<Res<terra_network::client::ClientSession>>,
    access: Option<Res<WorldAccess>>,
    mut parts: Query<(&AnimatedPart, &mut Transform)>,
) {
    let driving_radius = |parts: &[TrainPart]| {
        [232, 233, 234, train_parts::BOGIE]
            .into_iter()
            .find(|material| parts.iter().any(|part| part.material == *material))
            .and_then(terra_core::locomotive_parts::wheel_radius)
            .unwrap_or(0.45)
    };
    for (part, mut transform) in &mut parts {
        let source = if let Some(session) = session.as_deref().filter(|session| session.ready) {
            session
                .trains
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == part.vehicle)
                .map(|vehicle| {
                    (
                        session.displayed_train_motion(vehicle),
                        vehicle.build_yaw(),
                        driving_radius(&vehicle.parts),
                    )
                })
        } else if access
            .as_deref()
            .is_none_or(|access| access.authoritative())
        {
            journal
                .gameplay()
                .trains
                .vehicles
                .get(&part.vehicle)
                .map(|vehicle| {
                    (
                        vehicle.motion,
                        vehicle.build_yaw(),
                        driving_radius(&vehicle.parts),
                    )
                })
        } else {
            None
        };
        let Some((motion, build_yaw, driving_radius)) = source else {
            continue;
        };
        let direction = Quat::from_rotation_y(build_yaw as f32) * Vec3::Z;
        let alignment = if part.animation.forward.dot(direction) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let notch = if part.animation.role == TrainAnimationRole::Throttle {
            // A cab's look direction is local -Z, while chassis forward is +Z.
            (f32::from(motion.throttle) * -alignment) as i8
        } else {
            motion.throttle
        };
        let mut animation = part.animation;
        if matches!(
            animation.role,
            TrainAnimationRole::ConnectingRod
                | TrainAnimationRole::RodJoint
                | TrainAnimationRole::Piston
                | TrainAnimationRole::Crosshead
        ) {
            animation.radius = driving_radius;
        }
        *transform = pose(
            animation,
            motion.wheel_distance_m * f64::from(alignment),
            notch,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_rotation(actual: Quat, expected: Quat) {
        // acos(dot) loses precision for almost identical f32 quaternions,
        // including a quaternion compared with itself. Verify the resulting
        // orientation directly to a two-micrometer error on each unit axis.
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!((actual * axis).distance(expected * axis) < 2e-6);
        }
    }

    fn part(material: u16, facing: u8) -> TrainPart {
        TrainPart {
            material,
            facing,
            offset: [2, 3, -4],
            wood: Default::default(),
        }
    }

    #[test]
    fn one_real_wheel_circumference_returns_every_authored_pivot_to_its_start() {
        for material in [train_parts::BOGIE, 232, 233, 234] {
            for facing in 0..4 {
                let roles = if material == train_parts::BOGIE {
                    (0..4)
                        .map(TrainAnimationRole::BogieWheel)
                        .collect::<Vec<_>>()
                } else {
                    vec![TrainAnimationRole::Wheel]
                };
                for role in roles {
                    let animation = descriptor(&part(material, facing), role);
                    let full_turn = pose(animation, animation.radius * std::f64::consts::TAU, 0);
                    assert!(full_turn.rotation.angle_between(Quat::IDENTITY).abs() < 1e-5);
                    for distance in [
                        0.0,
                        animation.radius,
                        animation.radius * 2.0,
                        -animation.radius,
                    ] {
                        let transform = pose(animation, distance, 0);
                        assert!(
                            transform
                                .transform_point(animation.pivot)
                                .distance(animation.pivot)
                                < 1e-5
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn forward_and_reverse_roll_in_opposite_directions_and_stopped_wheels_do_not_drift() {
        let animation = descriptor(&part(232, 0), TrainAnimationRole::Wheel);
        let quarter = animation.radius * std::f64::consts::FRAC_PI_2;
        let forward = pose(animation, quarter, 0);
        let reverse = pose(animation, -quarter, 0);
        assert_same_rotation(forward.rotation, reverse.rotation.conjugate());
        let bottom = animation.pivot - Vec3::Y * animation.radius as f32;
        assert!(forward.transform_point(bottom).z < animation.pivot.z);
        assert_eq!(pose(animation, quarter, -3), pose(animation, quarter, 3));
    }

    #[test]
    fn lever_notches_rotate_about_the_physical_hinge_and_rods_share_the_wheel_phase() {
        let lever = descriptor(&part(train_parts::CAB, 1), TrainAnimationRole::Throttle);
        let mut last = -4.0;
        for notch in -3..=3 {
            let transform = pose(lever, 0.0, notch);
            assert!(transform.transform_point(lever.pivot).distance(lever.pivot) < 1e-5);
            let angle = -f32::from(notch) * 0.09;
            assert_same_rotation(transform.rotation, Quat::from_axis_angle(lever.axis, angle));
            assert!(f32::from(notch) > last);
            last = f32::from(notch);
        }
        for role in [
            TrainAnimationRole::ConnectingRod,
            TrainAnimationRole::RodJoint,
            TrainAnimationRole::Piston,
            TrainAnimationRole::Crosshead,
        ] {
            let animation = descriptor(&part(236, 0), role);
            let start = pose(animation, 0.0, 0);
            let end = pose(animation, animation.radius * std::f64::consts::TAU, 0);
            assert!(start.translation.distance(end.translation) < 1e-5);
            assert!(
                pose(animation, animation.radius * std::f64::consts::PI, 0)
                    .translation
                    .length()
                    > 0.1
            );
        }
    }

    #[test]
    fn each_named_wheel_instance_keeps_its_own_pivot_role() {
        for index in 0..4 {
            assert_eq!(
                TrainAnimationRole::from_material(&format!("train_bogie_wheel_{index}_tire")),
                Some(TrainAnimationRole::BogieWheel(index))
            );
        }
        assert!(TrainAnimationRole::from_material("train_bogie_wheel_4_tire").is_none());
        assert!(TrainAnimationRole::from_material("train_axle").is_none());
    }
}

#[cfg(test)]
mod crank_tests {
    use super::*;
    #[test]
    fn coupling_rod_stays_on_the_rotating_wheels_eccentric_pin() {
        let wheel = descriptor(
            &TrainPart {
                material: 232,
                offset: [0; 3],
                facing: 0,
                wood: Default::default(),
            },
            TrainAnimationRole::Wheel,
        );
        let rod = Animation {
            role: TrainAnimationRole::ConnectingRod,
            ..wheel
        };
        let pin = wheel.pivot + wheel.forward * 0.12;
        for distance in [0.0, 0.1, 0.5, 1.0, 2.0, -0.4] {
            assert!(
                pose(wheel, distance, 0)
                    .transform_point(pin)
                    .distance(pose(rod, distance, 0).transform_point(pin))
                    < 1e-5
            );
        }
    }
}
