use super::*;

const EYE: [f32; 2] = [0.172, -0.158];

const FLEECE: Tint = [0.91, 0.865, 0.745];
const WOOL_SHADE: Tint = [0.81, 0.75, 0.63];
const FACE: Tint = [0.33, 0.27, 0.225];
const MUZZLE: Tint = [0.47, 0.385, 0.316];

pub(super) fn parts() -> Result<Parts, String> {
    let mut body = Builder::default();
    mass(&mut body, [0., 0.796, 0.025], [0.382, 0.338, 0.533], FLEECE)?;
    mass(&mut body, [0., 0.782, 0.299], [0.351, 0.305, 0.262], FLEECE)?;
    mass(
        &mut body,
        [0., 0.861, -0.327],
        [0.319, 0.290, 0.266],
        FLEECE,
    )?;
    mass(
        &mut body,
        [0., 0.976, -0.411],
        [0.223, 0.211, 0.194],
        FLEECE,
    )?;
    // Locks follow the contour of the pelt. The longest curls hang below the
    // flank, where their separate ends are visible against the legs.
    for side in [-1., 1.] {
        for (x, y, z, rx, ry, rz) in [
            (0.21, 0.708, -0.393, 0.119, 0.143, 0.137),
            (0.303, 0.652, -0.234, 0.105, 0.145, 0.132),
            (0.325, 0.607, -0.04, 0.098, 0.137, 0.137),
            (0.326, 0.639, 0.135, 0.113, 0.139, 0.143),
            (0.266, 0.681, 0.32, 0.128, 0.149, 0.143),
            (0.176, 0.73, 0.451, 0.115, 0.134, 0.111),
        ] {
            mass(&mut body, [side * x, y, z], [rx, ry, rz], WOOL_SHADE)?;
            mass(
                &mut body,
                [side * (x + 0.008), y + 0.039, z - 0.019],
                [rx, ry * 0.87, rz],
                FLEECE,
            )?;
        }
        for (x, y, z) in [
            (0.23, 0.944, -0.33),
            (0.269, 0.957, -0.10),
            (0.26, 0.968, 0.14),
            (0.208, 0.958, 0.353),
        ] {
            mass(&mut body, [side * x, y, z], [0.14, 0.139, 0.16], FLEECE)?;
        }
    }
    for (y, z) in [(1.038, -0.33), (1.054, -0.12), (1.059, 0.12), (1.022, 0.33)] {
        mass(&mut body, [0., y, z], [0.187, 0.112, 0.159], FLEECE)?;
    }
    let mut head = Builder::default();
    mass(&mut head, [0., 0.03, 0.], [0.165, 0.156, 0.174], FACE)?;
    mass(&mut head, [0., -0.023, -0.118], [0.139, 0.125, 0.156], FACE)?;
    mass(
        &mut head,
        [0., -0.112, -0.212],
        [0.133, 0.085, 0.101],
        MUZZLE,
    )?;
    head.rounded(
        [0., -0.092, -0.287],
        [0.167, 0.072, 0.069],
        0.018,
        [0.244, 0.20, 0.18],
    )?;
    eye_sockets(&mut head, EYE, FACE)?;
    for side in [-1., 1.] {
        shape::segment(
            &mut head,
            Vec3::new(side * 0.122, 0.125, 0.039),
            Vec3::new(side * 0.298, 0.067, 0.047),
            0.083,
            0.112,
            FACE,
        )?;
        mass(
            &mut head,
            [side * 0.281, 0.07, 0.040],
            [0.065, 0.047, 0.063],
            MUZZLE,
        )?;
        head.voxel(
            [side * 0.269, 0.078, -0.017],
            [0.070, 0.031, 0.022],
            [0.58, 0.415, 0.355],
        )?;
        mass(
            &mut head,
            [side * 0.119, 0.159, 0.015],
            [0.093, 0.10, 0.141],
            FLEECE,
        )?;
    }
    for (x, y, z, r) in [
        (-0.078, 0.192, -0.094, 0.078),
        (0.013, 0.208, -0.115, 0.093),
        (0.098, 0.184, -0.086, 0.078),
        (0., 0.213, 0.018, 0.118),
    ] {
        mass(&mut head, [x, y, z], [r, r * 0.80, r], FLEECE)?;
    }
    let mut tail = Builder::with_resolution(12);
    mass(
        &mut tail,
        [0., -0.059, 0.038],
        [0.088, 0.128, 0.091],
        FLEECE,
    )?;
    mass(
        &mut tail,
        [0., -0.134, 0.062],
        [0.064, 0.087, 0.070],
        WOOL_SHADE,
    )?;
    Rig {
        head: Vec3::new(0., 1.05, -0.48),
        eye: EYE,
        tail: Vec3::new(0., 0.915, 0.453),
        hip_height: 0.42,
        leg_x: 0.246,
        front_z: -0.327,
        rear_z: 0.319,
        thigh_width: 0.178,
        hoof_width: 0.125,
        leg_coat: WOOL_SHADE,
        pastern: MUZZLE,
        hoof: [0.22, 0.18, 0.151],
    }
    .assemble(body.finish()?, head.finish()?, FACE, tail.finish()?)
}
