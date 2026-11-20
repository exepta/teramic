use super::*;

pub(super) fn parts(variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let armored = variant == 2;
    let steel = STEEL;
    let edge = EDGE;
    let mut out = Vec::with_capacity(4);
    for side in [-1.0, 1.0] {
        let mut arm = Builder::with_resolution(32);
        shape::rounded(
            &mut arm,
            [0.0, -0.015, 0.0],
            [0.17, 0.13, 0.17],
            0.025,
            IVORY,
        )?;
        shape::segment(
            &mut arm,
            Vec3::new(0.0, -0.06, 0.0),
            Vec3::new(side * 0.035, -0.29, -0.01),
            0.07,
            0.075,
            BONE,
        )?;
        shape::rounded(
            &mut arm,
            [side * 0.035, -0.30, -0.01],
            [0.13, 0.10, 0.13],
            0.015,
            IVORY,
        )?;
        for delta in [-0.022, 0.022] {
            shape::segment(
                &mut arm,
                Vec3::new(side * 0.035 + delta, -0.35, -0.02),
                Vec3::new(side * 0.045 + delta, -0.56, -0.045),
                0.035,
                0.045,
                IVORY,
            )?;
        }
        shape::rounded(
            &mut arm,
            [side * 0.045, -0.61, -0.05],
            [0.13, 0.12, 0.11],
            0.02,
            BONE,
        )?;
        for finger in 0..3 {
            arm.voxel(
                [side * 0.045 + (finger as f32 - 1.0) * 0.038, -0.68, -0.065],
                [0.028, 0.07, 0.055],
                IVORY,
            )?;
        }
        if armored {
            shape::rounded(
                &mut arm,
                [0.0, 0.015, 0.0],
                [0.25, 0.18, 0.27],
                0.035,
                steel,
            )?;
            arm.voxel([0.0, 0.08, -0.10], [0.27, 0.045, 0.16], edge)?;
            arm.voxel([0.0, -0.065, -0.13], [0.22, 0.035, 0.045], edge)?;
            for offset in [-0.065, 0.065] {
                arm.voxel([offset, 0.015, -0.143], [0.022, 0.022, 0.022], edge)?;
            }
            shape::rounded(
                &mut arm,
                [side * 0.04, -0.46, -0.03],
                [0.17, 0.24, 0.18],
                0.023,
                steel,
            )?;
        }
        if variant > 0 && side < 0.0 {
            arm.voxel(
                [-0.05, -0.62, -0.18],
                [0.065, 0.07, 0.32],
                [0.24, 0.12, 0.06],
            )?;
            arm.voxel([-0.05, -0.62, -0.30], [0.29, 0.065, 0.06], steel)?;
            for side in [-1.0, 1.0] {
                arm.voxel(
                    [-0.05 + side * 0.13, -0.62, -0.30],
                    [0.035, 0.075, 0.09],
                    edge,
                )?;
            }
            arm.voxel([-0.05, -0.62, -0.61], [0.115, 0.045, 0.56], steel)?;
            arm.voxel([-0.05, -0.62, -0.915], [0.075, 0.045, 0.08], edge)?;
            arm.voxel([-0.05, -0.62, -0.97], [0.035, 0.045, 0.03], edge)?;
            arm.voxel([-0.05, -0.59, -0.62], [0.025, 0.018, 0.54], edge)?;
            arm.voxel(
                [-0.05, -0.643, -0.61],
                [0.018, 0.006, 0.48],
                [0.19, 0.22, 0.27],
            )?;
            arm.voxel([-0.05, -0.62, -0.045], [0.09, 0.09, 0.06], steel)?;
        }
        out.push((
            Part::VillagerArm(side),
            Vec3::new(side * 0.29, 1.37, 0.0),
            arm.finish()?,
        ));
        let mut leg = Builder::default();
        shape::segment(
            &mut leg,
            Vec3::new(0.0, -0.03, 0.0),
            Vec3::new(side * 0.025, -0.32, 0.015),
            0.085,
            0.085,
            BONE,
        )?;
        shape::rounded(
            &mut leg,
            [side * 0.025, -0.34, 0.005],
            [0.14, 0.11, 0.14],
            0.02,
            IVORY,
        )?;
        for d in [-0.024, 0.024] {
            shape::segment(
                &mut leg,
                Vec3::new(side * 0.025 + d, -0.40, 0.01),
                Vec3::new(side * 0.035 + d, -0.65, 0.0),
                0.037,
                0.045,
                IVORY,
            )?;
        }
        shape::rounded(
            &mut leg,
            [side * 0.035, -0.695, -0.055],
            [0.17, 0.09, 0.23],
            0.018,
            BONE,
        )?;
        for toe in 0..3 {
            leg.voxel(
                [side * 0.035 + (toe as f32 - 1.0) * 0.049, -0.715, -0.15],
                [0.04, 0.05, 0.08],
                IVORY,
            )?;
        }
        if armored {
            shape::rounded(
                &mut leg,
                [side * 0.025, -0.34, -0.025],
                [0.20, 0.22, 0.20],
                0.025,
                steel,
            )?;
            shape::rounded(
                &mut leg,
                [side * 0.035, -0.67, -0.06],
                [0.22, 0.14, 0.30],
                0.025,
                steel,
            )?;
            leg.voxel([side * 0.025, -0.34, -0.132], [0.14, 0.05, 0.025], edge)?;
            leg.voxel(
                [side * 0.035, -0.565, -0.025],
                [0.16, 0.045, 0.17],
                [0.23, 0.12, 0.075],
            )?;
        }
        out.push((
            Part::Leg(side),
            Vec3::new(side * 0.125, 0.74, 0.0),
            leg.finish()?,
        ));
    }
    Ok(out)
}
