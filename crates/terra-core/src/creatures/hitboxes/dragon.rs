//! Anatomical picking uses the same animated joints as the cached render mesh.
//! A wing is several shallow bands across its fan, leaving the air around it free.
use super::*;
use crate::creatures::{
    dragon_pose::{Joint, Pose, TAIL_BONES, tail_node, wrist},
    dragons,
};
use bevy_math::Vec3;

pub(super) fn add(boxes: &mut Hitboxes, dragon: &Creature) {
    let pose = Pose::of(dragon, 0.);
    let scale = dragons::model_scale(dragon.kind, dragon.variant);
    let point = |joint, p| (pose.point(joint, p) * scale).to_array().map(f64::from);
    let seg = |boxes: &mut Hitboxes, joint: Joint, a: Vec3, b: Vec3, w: f32| {
        boxes.segment(
            point(joint, a),
            point(joint, b),
            f64::from(w * scale * pose.joint_scale(joint).x),
        );
    };
    let v = Vec3::new;
    for (joint, a, b, w) in [
        (Joint::Trunk, v(0., 4.70, -2.54), v(0., 4.16, 3.33), 4.25),
        (Joint::Neck, v(0., 5.24, -2.63), v(0., 7.19, -4.04), 2.65),
        (Joint::Neck, v(0., 7.19, -4.04), v(0., 8.36, -5.70), 1.98),
        (Joint::Skull, v(0., 8.61, -5.78), v(0., 8.60, -7.26), 2.17),
        (Joint::Skull, v(0., 8.55, -7.31), v(0., 8.48, -9.08), 1.15),
        (Joint::Jaw, v(0., 7.90, -6.61), v(0., 7.75, -8.97), 0.78),
    ] {
        seg(boxes, joint, a, b, w);
    }
    for i in 0..TAIL_BONES {
        let width = (1. - f32::from(i) / f32::from(TAIL_BONES)) * 2.21 + 0.22;
        seg(boxes, Joint::Tail(i), tail_node(i), tail_node(i + 1), width);
    }
    for s in [-1i8, 1] {
        let side = f32::from(s);
        if dragon.kind == crate::creatures::CreatureKind::WaterDragon {
            continue;
        }
        for hind in [false, true] {
            let upper = Joint::Thigh { side: s, hind };
            let foot = Joint::Foot { side: s, hind };
            let paw = Joint::Paw { side: s, hind };
            let ankle = paw.pivot();
            seg(boxes, upper, upper.pivot(), foot.pivot(), 1.40);
            seg(boxes, foot, foot.pivot(), ankle, 1.18);
            seg(boxes, paw, ankle, ankle - v(0., 0.38, 1.72), 1.13);
        }
        if !dragons::flies(dragon.kind) {
            continue;
        }
        let arm = Joint::WingArm(s);
        let wing_factor = dragons::wing_scale(dragon.kind, dragon.variant);
        let wing_point = |p| arm.pivot().lerp(p, wing_factor);
        let elbow = wing_point(v(side * 4.75, 8.5, 0.33));
        seg(boxes, arm, arm.pivot(), elbow, 1.15 * wing_factor);
        seg(
            boxes,
            arm,
            elbow,
            wing_point(wrist(side)),
            0.86 * wing_factor,
        );
        let sail = Joint::Sail(s);
        let wrist = wing_point(wrist(side));
        let fingers = [
            v(side * 18.4, 5., 2.45),
            v(side * 15.95, 3.42, 6.85),
            v(side * 12.82, 3.35, 9.26),
            v(side * 8.28, 3.15, 9.53),
            v(side * 4.69, 3.48, 6.94),
            sail.pivot(),
        ]
        .map(wing_point);
        for pair in fingers.windows(2) {
            let end = pair[0].lerp(pair[1], 0.5);
            let delta = end - wrist;
            let frame = pose.bone(sail);
            let length = delta.length();
            let long = delta.as_dvec3().normalize();
            let normal = long.cross((pair[1] - pair[0]).as_dvec3()).normalize();
            let width = normal.cross(long);
            // Volumes are expressed in the creature's local frame then transformed
            // into world space exactly once, including juvenile scaling and yaw.
            let rotation = frame.rotation.as_dquat().normalize();
            let local_axes = [width, normal, long].map(|axis| (rotation * axis).to_array());
            for (t, span) in [(0.30, 0.45), (0.70, 0.40)] {
                let center = point(sail, wrist + delta * t + Vec3::Y * 0.24);
                let sz = [
                    (pair[1] - pair[0]).as_dvec3().dot(width).abs() as f32 * t,
                    0.68 * wing_factor,
                    length * span,
                ];
                let index = boxes.count;
                let local = std::array::from_fn(|i| center[i] * boxes.scale[i]);
                let offset = transform(boxes.axes, local);
                boxes.boxes[index] = Hitbox {
                    center: std::array::from_fn(|i| boxes.origin[i] + offset[i]),
                    size: std::array::from_fn(|i| {
                        f64::from(sz[i] * scale * pose.joint_scale(sail)[i]) * boxes.scale[i]
                    }),
                    axes: local_axes.map(|a| transform(boxes.axes, a)),
                };
                boxes.count += 1;
            }
        }
    }
}
