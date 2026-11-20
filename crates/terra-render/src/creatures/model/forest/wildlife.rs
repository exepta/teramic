use super::*;

pub(in crate::creatures::model) fn raven() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let feather = [0.10, 0.065, 0.16];
    let highlight = [0.24, 0.18, 0.32];
    let mut b = Builder::default();
    ellipsoid(
        &mut b,
        Vec3::new(0.0, 0.23, 0.04),
        Vec3::new(0.15, 0.18, 0.21),
        feather,
    )?;
    for side in [-1.0, 1.0] {
        shape::segment(
            &mut b,
            Vec3::new(side * 0.08, 0.16, 0.04),
            Vec3::new(side * 0.08, 0.035, 0.02),
            0.027,
            0.027,
            highlight,
        )?;
        for toe in [-1.0, 0.0, 1.0] {
            shape::segment(
                &mut b,
                Vec3::new(side * 0.08, 0.027, 0.02),
                Vec3::new(side * 0.08 + toe * 0.035, 0.025, -0.07),
                0.018,
                0.018,
                feather,
            )?;
        }
    }
    let mut head = Builder::default();
    shape::rounded(
        &mut head,
        [0.0, 0.0, 0.0],
        [0.22, 0.21, 0.22],
        0.04,
        highlight,
    )?;
    // Two tapered bill sections, never a flat painted triangle.
    head.voxel([0.0, -0.025, -0.16], [0.14, 0.075, 0.16], feather)?;
    head.voxel([0.0, -0.04, -0.25], [0.075, 0.045, 0.07], highlight)?;
    for side in [-1.0, 1.0] {
        head.voxel(
            [side * 0.112, 0.025, -0.055],
            [0.016, 0.047, 0.046],
            [0.91, 0.83, 0.86],
        )?;
        head.voxel([side * 0.123, 0.022, -0.06], [0.008, 0.022, 0.025], DARK)?;
    }
    let mut out = vec![
        (Part::Body, Vec3::ZERO, b.finish()?),
        (Part::Head, Vec3::new(0.0, 0.41, -0.12), head.finish()?),
    ];
    for side in [-1.0, 1.0] {
        let mut wing = Builder::default();
        for i in 0..6 {
            let t = i as f32 / 5.0;
            wing.rotated(
                [side * (0.06 + t * 0.18), -0.015, 0.015 + t * 0.11],
                [0.105, 0.055, 0.24 - t * 0.08],
                if i % 2 == 0 { feather } else { highlight },
                Quat::from_rotation_y(-side * 0.30),
            )?;
        }
        out.push((
            Part::Wing(side),
            Vec3::new(side * 0.12, 0.30, 0.04),
            wing.finish()?,
        ));
    }
    let mut tail = Builder::default();
    for i in -1..=1 {
        tail.rotated(
            [i as f32 * 0.05, 0.0, 0.09],
            [0.065, 0.035, 0.21],
            highlight,
            Quat::from_rotation_y(i as f32 * 0.18),
        )?;
    }
    out.push((Part::Tail, Vec3::new(0.0, 0.23, 0.22), tail.finish()?));
    Ok(out)
}

pub(in crate::creatures::model) fn slime() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut b = Builder::default();
    shape::rounded(&mut b, [0.0, 0.30, 0.0], [0.67, 0.53, 0.66], 0.10, RED)?;
    for side in [-1.0, 1.0] {
        ellipsoid(
            &mut b,
            Vec3::new(side * 0.28, 0.10, -0.12),
            Vec3::new(0.13, 0.10, 0.17),
            RUBY,
        )?;
    }
    ellipsoid(
        &mut b,
        Vec3::new(0.17, 0.63, 0.06),
        Vec3::new(0.085, 0.10, 0.085),
        RUBY,
    )?;
    b.voxel(
        [-0.17, 0.48, -0.25],
        [0.14, 0.045, 0.035],
        [1.0, 0.51, 0.57],
    )?;
    b.voxel(
        [0.23, 0.35, -0.30],
        [0.055, 0.095, 0.018],
        [0.95, 0.32, 0.43],
    )?;
    let mut face = Builder::default();
    eyes(&mut face, 0.15, 0.31, -0.341, 0.085, [1.0, 0.92, 0.85])?;
    face.voxel([0.0, 0.21, -0.345], [0.07, 0.025, 0.018], DARK)?;
    for side in [-1.0, 1.0] {
        face.voxel([side * 0.04, 0.235, -0.345], [0.025, 0.035, 0.018], DARK)?;
    }
    Ok(vec![
        (Part::Body, Vec3::ZERO, b.finish()?),
        (Part::Head, Vec3::ZERO, face.finish()?),
    ])
}
