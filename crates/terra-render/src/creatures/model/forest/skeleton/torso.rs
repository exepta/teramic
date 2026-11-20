use super::*;

pub(super) fn mesh(variant: u8) -> Result<Mesh, String> {
    let armored = variant == 2;
    let steel = STEEL;
    let edge = EDGE;
    // 14.5 mm preserves the open rib spaces without spending thousands of faces
    // on tiny staircase changes along the curved bones. Head/eyes use their
    // independent finer grids, so this cannot erase recessed pupil detail.
    let mut body = Builder::with_cell_size(0.0145);
    // Exposed spine, separate curved ribs and an open pelvic ring.
    for n in 0..7 {
        shape::rounded(
            &mut body,
            [0.0, 0.81 + n as f32 * 0.085, 0.055],
            [0.10, 0.065, 0.11],
            0.012,
            BONE,
        )?;
    }
    for row in -1_i32..3 {
        let y = 1.11 + row as f32 * 0.105;
        let span = 0.19 + row as f32 * 0.022;
        for side in [-1.0, 1.0] {
            for (a, b) in [
                ([0.0, y, 0.07], [side * span, y + 0.012, 0.035]),
                (
                    [side * span, y + 0.012, 0.035],
                    [side * (span - 0.02), y - 0.005, -0.115],
                ),
                (
                    [side * (span - 0.02), y - 0.005, -0.115],
                    [side * 0.055, y - 0.020, -0.14],
                ),
            ] {
                shape::segment(
                    &mut body,
                    Vec3::from_array(a),
                    Vec3::from_array(b),
                    0.036,
                    0.036,
                    IVORY,
                )?;
            }
        }
    }
    body.voxel([0.0, 1.17, -0.14], [0.065, 0.38, 0.055], BONE)?;
    body.voxel([0.0, 0.77, 0.045], [0.38, 0.09, 0.12], IVORY)?;
    for side in [-1.0, 1.0] {
        shape::rounded(
            &mut body,
            [side * 0.15, 0.85, 0.0],
            [0.14, 0.16, 0.19],
            0.025,
            BONE,
        )?;
        // Paired pubic bones leave a real opening below the pelvic ridge.
        shape::segment(
            &mut body,
            Vec3::new(side * 0.15, 0.79, -0.045),
            Vec3::new(side * 0.08, 0.68, -0.08),
            0.045,
            0.05,
            IVORY,
        )?;
        shape::segment(
            &mut body,
            Vec3::new(side * 0.08, 0.68, -0.08),
            Vec3::new(0.0, 0.70, -0.09),
            0.035,
            0.045,
            BONE,
        )?;
    }
    // Collar bones and the neck join the shoulder pivots to the skull. They
    // remain real bone volumes with open space around the upper ribs.
    for side in [-1.0, 1.0] {
        shape::segment(
            &mut body,
            Vec3::new(0.0, 1.39, -0.07),
            Vec3::new(side * 0.255, 1.37, 0.0),
            0.045,
            0.045,
            IVORY,
        )?;
        body.voxel([side * 0.12, 1.31, 0.13], [0.10, 0.13, 0.035], BONE)?;
    }
    for n in 0..3 {
        body.voxel(
            [0.0, 1.40 + n as f32 * 0.044, 0.035],
            [0.075, 0.029, 0.08],
            IVORY,
        )?;
    }
    if variant == 1 {
        body.voxel([0.0, 0.91, 0.015], [0.34, 0.075, 0.20], [0.23, 0.12, 0.08])?;
        body.voxel([0.0, 0.91, -0.096], [0.065, 0.065, 0.025], steel)?;
        shape::segment(
            &mut body,
            Vec3::new(-0.19, 1.36, -0.105),
            Vec3::new(0.15, 0.96, -0.13),
            0.055,
            0.035,
            [0.25, 0.12, 0.075],
        )?;
        body.voxel([-0.085, 1.225, -0.152], [0.055, 0.065, 0.025], edge)?;
    }
    if armored {
        shape::rounded(
            &mut body,
            [0.0, 1.20, -0.035],
            [0.49, 0.40, 0.34],
            0.055,
            steel,
        )?;
        body.voxel([0.0, 1.21, -0.213], [0.12, 0.32, 0.025], edge)?;
        body.voxel([0.0, 0.91, -0.015], [0.41, 0.12, 0.31], [0.22, 0.12, 0.07])?;
        body.voxel([0.0, 0.91, -0.18], [0.11, 0.10, 0.04], edge)?;
        // Layered plate rims/rivets are painted into the shared occupancy,
        // never coplanar decorative tiles.
        for side in [-1.0, 1.0] {
            body.voxel([side * 0.19, 1.32, -0.195], [0.035, 0.035, 0.045], edge)?;
            body.voxel([side * 0.16, 1.055, -0.195], [0.13, 0.045, 0.055], edge)?;
            body.voxel([side * 0.17, 0.80, -0.12], [0.14, 0.20, 0.06], steel)?;
            body.voxel([side * 0.13, 1.08, -0.205], [0.022, 0.022, 0.025], edge)?;
            body.voxel([side * 0.17, 1.36, -0.17], [0.06, 0.035, 0.06], edge)?;
        }
        // Split crimson cloth hangs from the belt without replacing the legs.
        for side in [-1.0, 1.0] {
            body.rotated(
                [side * 0.075, 0.685, -0.095],
                [0.12, 0.30, 0.035],
                [0.40, 0.025, 0.055],
                Quat::from_rotation_z(side * -0.10),
            )?;
            body.voxel(
                [side * 0.07, 0.57, -0.12],
                [0.11, 0.045, 0.025],
                [0.23, 0.015, 0.025],
            )?;
        }
    }
    if variant > 0 {
        shape::rounded(
            &mut body,
            [0.0, 1.425, -0.01],
            [0.21, 0.10, 0.19],
            0.025,
            [0.40, 0.025, 0.055],
        )?;
        body.voxel(
            [0.11, 1.30, 0.155],
            [0.075, 0.25, 0.025],
            [0.31, 0.02, 0.035],
        )?;
    }
    body.finish()
}
