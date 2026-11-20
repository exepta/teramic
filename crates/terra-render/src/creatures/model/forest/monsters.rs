use super::*;

pub(in crate::creatures::model) fn stalker() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let shell = [0.22, 0.035, 0.085];
    let edge = [0.40, 0.07, 0.13];
    let mut b = Builder::default();
    shape::rounded(&mut b, [0.0, 0.50, 0.22], [0.61, 0.42, 0.72], 0.09, shell)?;
    for z in 0..4 {
        for side in [-1.0, 1.0] {
            crystal(
                &mut b,
                Vec3::new(side * 0.19, 0.65, 0.02 + z as f32 * 0.13),
                0.055,
                0.15 + (z % 2) as f32 * 0.07,
                RUBY,
            )?;
        }
    }
    let mut h = Builder::default();
    shape::rounded(&mut h, [0.0, 0.0, 0.0], [0.48, 0.30, 0.32], 0.05, edge)?;
    for side in [-1.0, 1.0] {
        shape::segment(
            &mut h,
            Vec3::new(side * 0.15, -0.05, -0.13),
            Vec3::new(side * 0.17, -0.24, -0.20),
            0.065,
            0.07,
            shell,
        )?;
        shape::segment(
            &mut h,
            Vec3::new(side * 0.17, -0.24, -0.20),
            Vec3::new(side * 0.09, -0.27, -0.28),
            0.04,
            0.04,
            IVORY,
        )?;
    }
    let mut glow = Builder::default();
    eyes(&mut glow, 0.12, 0.045, -0.175, 0.09, RUBY)?;
    let origin = Vec3::new(0.0, 0.50, -0.25);
    let mut out = vec![
        (Part::Body, Vec3::ZERO, b.finish()?),
        (Part::Head, origin, h.finish()?),
        (Part::SpiderEyes, origin, glow.finish()?),
    ];
    for side in [-1.0, 1.0] {
        for index in 0..4 {
            let z = -0.23 + index as f32 * 0.19;
            let mut leg = Builder::with_resolution(16);
            let knee = Vec3::new(side * 0.36, 0.02, (index as f32 - 1.5) * 0.13);
            let ankle = Vec3::new(side * 0.49, -0.22, (index as f32 - 1.5) * 0.17);
            let toe = Vec3::new(side * 0.51, -0.47, (index as f32 - 1.5) * 0.20);
            shape::segment(&mut leg, Vec3::ZERO, knee, 0.09, 0.10, edge)?;
            shape::segment(&mut leg, knee, ankle, 0.085, 0.075, shell)?;
            shape::segment(&mut leg, ankle, toe, 0.045, 0.055, edge)?;
            crystal(&mut leg, knee, 0.045, 0.12, RUBY)?;
            out.push((
                Part::SpiderLeg { side, index },
                Vec3::new(side * 0.23, 0.48, z),
                leg.finish()?,
            ));
        }
    }
    Ok(out)
}

pub(in crate::creatures::model) fn brute() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    super::thorn::parts()
}

pub(in crate::creatures::model) fn wisp() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    super::super::spirit::flame_spirit([0.63, 0.035, 0.16], [1.0, 0.27, 0.37], true)
}
