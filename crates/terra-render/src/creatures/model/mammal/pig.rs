use super::*;

const EYE: [f32; 2] = [0.159, -0.137];

const SKIN: Tint = [0.85, 0.536, 0.461];
const PALE: Tint = [0.91, 0.63, 0.534];
const SNOUT: Tint = [0.772, 0.415, 0.372];
const HOOF: Tint = [0.37, 0.259, 0.228];

pub(super) fn parts() -> Result<Parts, String> {
    let mut body = Builder::default();
    for (center, radius) in [
        ([0., 0.607, 0.025], [0.339, 0.258, 0.48]),
        ([0., 0.633, 0.260], [0.341, 0.26, 0.254]), // high rounded hams
        ([0., 0.617, -0.297], [0.283, 0.240, 0.232]),
        ([0., 0.533, -0.017], [0.305, 0.214, 0.349]),
    ] {
        mass(&mut body, center, radius, SKIN)?;
    }
    // A short line of bristles follows the shoulders, tapering into the back.
    for (y, z) in [
        (0.832, -0.32),
        (0.858, -0.22),
        (0.87, -0.12),
        (0.877, -0.02),
    ] {
        body.rotated(
            [0., y, z],
            [0.045, 0.073, 0.056],
            [0.735, 0.451, 0.389],
            Quat::from_rotation_x(0.36),
        )?;
    }
    let body = body.finish_painted(|p, tint| {
        if tint == SKIN && p.y < 0.466 + 0.04 * (p.z * 7.).cos() {
            PALE
        } else {
            tint
        }
    })?;
    let mut head = Builder::default();
    mass(&mut head, [0., 0.006, 0.018], [0.202, 0.159, 0.182], SKIN)?;
    mass(&mut head, [0., -0.014, -0.113], [0.156, 0.123, 0.149], SKIN)?;
    mass(&mut head, [0., -0.071, -0.211], [0.119, 0.088, 0.11], PALE)?;
    head.rounded([0., -0.065, -0.305], [0.259, 0.139, 0.099], 0.038, SNOUT)?;
    eye_sockets(&mut head, EYE, SKIN)?;
    for side in [-1., 1.] {
        head.voxel(
            [side * 0.062, -0.060, -0.355],
            [0.044, 0.055, 0.018],
            [0.359, 0.207, 0.196],
        )?;
        mass(
            &mut head,
            [side * 0.136, -0.058, -0.068],
            [0.079, 0.098, 0.103],
            PALE,
        )?;
        // Broad at the root and folded forward at the tips.
        let angle = Quat::from_rotation_z(-side * 0.34);
        head.rotated(
            [side * 0.156, 0.177, 0.049],
            [0.147, 0.156, 0.074],
            SKIN,
            angle,
        )?;
        head.rotated(
            [side * 0.197, 0.241, 0.029],
            [0.103, 0.106, 0.064],
            SKIN,
            angle,
        )?;
        head.rotated(
            [side * 0.216, 0.268, 0.001],
            [0.065, 0.046, 0.085],
            SNOUT,
            angle,
        )?;
        head.rotated(
            [side * 0.165, 0.193, 0.008],
            [0.085, 0.092, 0.016],
            SNOUT,
            angle,
        )?;
    }
    let mut tail = Builder::with_resolution(13);
    let tail_point = |t: f32| Vec3::new(0.059 * t.sin(), 0.059 * (1. - t.cos()), t * 0.012);
    let mut start = Vec3::ZERO;
    for i in 1..=8 {
        let t = i as f32 * 0.79;
        let end = tail_point(t);
        let diameter = 0.044 - i as f32 * 0.0018;
        shape::segment(&mut tail, start, end, diameter, diameter, SNOUT)?;
        start = end;
    }
    Rig {
        head: Vec3::new(0., 0.74, -0.48),
        eye: EYE,
        tail: Vec3::new(0., 0.731, 0.451),
        hip_height: 0.32,
        leg_x: 0.225,
        front_z: -0.291,
        rear_z: 0.304,
        thigh_width: 0.185,
        hoof_width: 0.127,
        leg_coat: SKIN,
        pastern: PALE,
        hoof: HOOF,
    }
    .assemble(body, head.finish()?, SKIN, tail.finish()?)
}
