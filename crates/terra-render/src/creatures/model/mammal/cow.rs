use super::*;

const EYE: [f32; 2] = [0.166, -0.151];

const HIDE: Tint = [0.49, 0.27, 0.15];
const IVORY: Tint = [0.91, 0.84, 0.69];
const NOSE: Tint = [0.71, 0.44, 0.37];
const DARK: Tint = [0.20, 0.145, 0.115];

pub(super) fn parts() -> Result<Parts, String> {
    let mut body = Builder::default();
    for (center, radius) in [
        ([0., 1.01, 0.04], [0.397, 0.362, 0.608]),    // rib cage
        ([0., 1.055, -0.365], [0.373, 0.365, 0.299]), // shoulder and brisket
        ([0., 1.045, 0.414], [0.385, 0.338, 0.28]),   // hips
        ([0., 0.879, 0.08], [0.345, 0.259, 0.453]),   // low flank
        ([0., 1.18, -0.545], [0.248, 0.256, 0.244]),  // nape
        ([0., 0.980, -0.572], [0.157, 0.242, 0.163]), // hanging dewlap
    ] {
        mass(&mut body, center, radius, HIDE)?;
    }
    mass(
        &mut body,
        [0., 0.655, 0.29],
        [0.198, 0.15, 0.20],
        [0.76, 0.53, 0.43],
    )?;
    for x in [-0.09, 0.09] {
        for z in [0.21, 0.35] {
            body.rounded([x, 0.505, z], [0.053, 0.093, 0.061], 0.016, NOSE)?;
        }
    }
    let body = body.finish_painted(|p, tint| {
        if tint != HIDE {
            return tint;
        }
        // Irregular pinto islands wrap continuously over the stepped surface.
        let z = p.z + 0.12 * p.x + 0.045 * (9.0 * p.y + 3.0 * p.x).sin();
        let saddle = ((z - 0.13) / 0.25).powi(2) + ((p.y - 1.39) / 0.42).powi(2);
        let shoulder = ((z + 0.41) / 0.18).powi(2) + ((p.y - 1.04 - 0.24 * p.x) / 0.23).powi(2);
        if saddle < 1. || shoulder < 1. || p.y < 0.81 - 0.075 * (z * 8.).cos() {
            IVORY
        } else {
            HIDE
        }
    })?;

    let mut head = Builder::default();
    mass(&mut head, [0., 0.045, 0.], [0.202, 0.186, 0.179], HIDE)?;
    mass(&mut head, [0., -0.015, -0.124], [0.168, 0.143, 0.173], HIDE)?;
    mass(&mut head, [0., -0.142, -0.239], [0.15, 0.087, 0.115], IVORY)?;
    head.rounded([0., -0.096, -0.274], [0.337, 0.163, 0.145], 0.047, NOSE)?;
    eye_sockets(&mut head, EYE, HIDE)?;
    for side in [-1., 1.] {
        head.voxel([side * 0.095, -0.078, -0.349], [0.062, 0.042, 0.020], DARK)?;
        mass(
            &mut head,
            [side * 0.255, 0.126, 0.048],
            [0.116, 0.060, 0.069],
            HIDE,
        )?;
        head.rotated(
            [side * 0.286, 0.126, -0.014],
            [0.113, 0.052, 0.024],
            NOSE,
            Quat::from_rotation_z(side * 0.14),
        )?;
        // Thick horn base at the poll, with a narrow tip sweeping back up.
        shape::segment(
            &mut head,
            Vec3::new(side * 0.131, 0.181, 0.054),
            Vec3::new(side * 0.237, 0.239, 0.054),
            0.088,
            0.091,
            [0.59, 0.47, 0.33],
        )?;
        shape::segment(
            &mut head,
            Vec3::new(side * 0.224, 0.236, 0.054),
            Vec3::new(side * 0.265, 0.333, 0.071),
            0.056,
            0.063,
            IVORY,
        )?;
        head.voxel(
            [side * 0.264, 0.344, 0.079],
            [0.035, 0.052, 0.039],
            [0.39, 0.31, 0.24],
        )?;
    }
    let head = head.finish_painted(|p, tint| {
        let blaze_width = 0.028 + ((p.y + 0.06) * 0.21).clamp(0., 0.05);
        if tint == HIDE && p.z < -0.086 && (p.x - 0.012).abs() < blaze_width {
            IVORY
        } else {
            tint
        }
    })?;
    let mut tail = Builder::with_resolution(18);
    shape::segment(
        &mut tail,
        Vec3::ZERO,
        Vec3::new(0.014, -0.28, 0.10),
        0.075,
        0.085,
        HIDE,
    )?;
    shape::segment(
        &mut tail,
        Vec3::new(0.014, -0.25, 0.10),
        Vec3::new(0.028, -0.46, 0.12),
        0.055,
        0.06,
        IVORY,
    )?;
    mass(&mut tail, [0.03, -0.48, 0.124], [0.060, 0.10, 0.058], DARK)?;

    Rig {
        head: Vec3::new(0., 1.23, -0.67),
        eye: EYE,
        tail: Vec3::new(0., 1.22, 0.60),
        hip_height: 0.62,
        leg_x: 0.279,
        front_z: -0.407,
        rear_z: 0.415,
        thigh_width: 0.215,
        hoof_width: 0.179,
        leg_coat: HIDE,
        pastern: IVORY,
        hoof: DARK,
    }
    .assemble(body, head, HIDE, tail.finish()?)
}
