//! A rooted bark guardian with antler thorns and an exposed red heart.
//! All leaves and bark merge into eight shared articulated meshes.
use super::*;
const BARK: [f32; 3] = [0.42, 0.26, 0.17];
const RIDGE: [f32; 3] = [0.62, 0.41, 0.25];
const SHADOW: [f32; 3] = [0.27, 0.15, 0.105];
const LEAF: [f32; 3] = [0.63, 0.025, 0.085];
const LEAF_LIGHT: [f32; 3] = [0.90, 0.075, 0.13];
const TIP: [f32; 3] = [0.89, 0.73, 0.46];

fn branch(b: &mut Builder, points: &[Vec3], width: f32) -> Result<(), String> {
    let count = points.len().saturating_sub(1).max(1);
    for (i, pair) in points.windows(2).enumerate() {
        let w = width * (1.0 - i as f32 / count as f32 * 0.66);
        shape::segment(b, pair[0], pair[1], w, w * 0.88, BARK)?;
        let ridge = Vec3::new(w * 0.17, 0.0, -w * 0.38);
        shape::segment(
            b,
            pair[0] + ridge,
            pair[1] + ridge,
            w * 0.30,
            w * 0.18,
            RIDGE,
        )?;
    }
    Ok(())
}
fn thorn(b: &mut Builder, base: Vec3, tip: Vec3, radius: f32) -> Result<(), String> {
    let direction = tip - base;
    let elbow = base + direction * 0.58 + Vec3::Y * radius * 0.4;
    shape::segment(b, base, elbow, radius * 2.0, radius * 1.65, TIP)?;
    shape::segment(
        b,
        elbow,
        base + direction * 0.88,
        radius,
        radius * 0.82,
        IVORY,
    )?;
    shape::segment(
        b,
        base + direction * 0.84,
        tip,
        radius * 0.34,
        radius * 0.34,
        IVORY,
    )
}
fn foliage(b: &mut Builder, center: Vec3, seed: usize) -> Result<(), String> {
    for i in 0..6 {
        let a = i as f32 * 2.399 + seed as f32;
        let p = center + Vec3::new(a.cos() * 0.13, (i % 3) as f32 * 0.035, a.sin() * 0.11);
        b.rotated(
            p.to_array(),
            [0.22, 0.055, 0.105],
            if i % 3 == 0 { LEAF_LIGHT } else { LEAF },
            Quat::from_euler(EulerRot::YXZ, a, 0.20 + i as f32 * 0.045, a.sin() * 0.28),
        )?;
    }
    Ok(())
}

pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::with_cell_size(0.04);
    shape::rounded(
        &mut body,
        [0.0, 1.25, 0.035],
        [0.75, 0.82, 0.55],
        0.10,
        SHADOW,
    )?;
    for i in -2_i32..=2 {
        let x = i as f32 * 0.13;
        branch(
            &mut body,
            &[
                Vec3::new(x * 0.72, 0.82, 0.04),
                Vec3::new(x - 0.05, 1.03, -0.10),
                Vec3::new(x + 0.055, 1.30, -0.15),
                Vec3::new(x * 1.32, 1.56, -0.015),
            ],
            0.19,
        )?;
        branch(
            &mut body,
            &[
                Vec3::new(x * 0.70, 0.85, 0.20),
                Vec3::new(x * 1.10, 1.26, 0.25),
                Vec3::new(x * 1.20, 1.58, 0.20),
            ],
            0.15,
        )?;
    }
    body.rotated(
        [0.0, 1.35, -0.285],
        [0.28, 0.28, 0.06],
        SHADOW,
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
    )?;
    for side in [-1.0, 1.0] {
        branch(
            &mut body,
            &[
                Vec3::new(side * 0.14, 1.60, -0.04),
                Vec3::new(side * 0.43, 1.65, 0.015),
                Vec3::new(side * 0.52, 1.50, 0.08),
            ],
            0.22,
        )?;
        foliage(
            &mut body,
            Vec3::new(side * 0.36, 1.62, 0.13),
            usize::from(side > 0.0),
        )?;
    }
    let head_pivot = Vec3::new(0.0, 1.85, -0.025);
    let mut head = Builder::with_cell_size(0.035);
    shape::rounded(
        &mut head,
        [0.0, 0.0, 0.025],
        [0.48, 0.45, 0.39],
        0.07,
        SHADOW,
    )?;
    for i in -2_i32..=2 {
        let x = i as f32 * 0.091;
        branch(
            &mut head,
            &[
                Vec3::new(x * 0.82, -0.20, -0.13),
                Vec3::new(x + 0.015, 0.04, -0.17),
                Vec3::new(x * 0.94, 0.22, -0.09),
            ],
            0.095,
        )?;
    }
    for side in [-1.0, 1.0] {
        head.rotated(
            [side * 0.115, 0.025, -0.198],
            [0.145, 0.095, 0.035],
            DARK,
            Quat::from_rotation_z(side * -0.18),
        )?;
        branch(
            &mut head,
            &[
                Vec3::new(side * 0.16, 0.17, 0.02),
                Vec3::new(side * 0.30, 0.31, 0.04),
                Vec3::new(side * 0.39, 0.46, 0.015),
                Vec3::new(side * 0.35, 0.61, -0.015),
            ],
            0.115,
        )?;
        thorn(
            &mut head,
            Vec3::new(side * 0.29, 0.33, 0.035),
            Vec3::new(side * 0.48, 0.39, -0.01),
            0.045,
        )?;
        thorn(
            &mut head,
            Vec3::new(side * 0.37, 0.46, 0.02),
            Vec3::new(side * 0.47, 0.57, 0.01),
            0.035,
        )?;
        foliage(&mut head, Vec3::new(side * 0.18, 0.15, 0.06), 2)?;
    }
    head.voxel([0.0, -0.12, -0.182], [0.21, 0.055, 0.025], DARK)?;
    let mut eyes = Builder::with_cell_size(0.006);
    for side in [-1.0, 1.0] {
        eyes.rotated(
            [side * 0.115, 0.025, -0.222],
            [0.070, 0.057, 0.018],
            RUBY,
            Quat::from_rotation_z(side * -0.18),
        )?;
    }
    let mut heart = Builder::with_cell_size(0.012);
    heart.rotated(
        [0.0, 1.35, -0.323],
        [0.14, 0.17, 0.065],
        RUBY,
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
    )?;
    let mut out = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Head, head_pivot, head.finish()?),
        (Part::ThornGlow { head: true }, head_pivot, eyes.finish()?),
        (Part::ThornGlow { head: false }, Vec3::ZERO, heart.finish()?),
    ];
    for side in [-1.0, 1.0] {
        let mut arm = Builder::with_cell_size(0.04);
        branch(
            &mut arm,
            &[
                Vec3::new(0.0, 0.015, 0.02),
                Vec3::new(side * 0.07, -0.27, 0.04),
                Vec3::new(side * 0.10, -0.56, -0.045),
                Vec3::new(side * 0.07, -0.74, -0.10),
            ],
            0.32,
        )?;
        foliage(&mut arm, Vec3::new(side * 0.02, 0.045, 0.0), 3)?;
        for (base, tip, radius) in [
            (
                Vec3::new(side * 0.12, 0.02, 0.0),
                Vec3::new(side * 0.30, 0.25, -0.04),
                0.065,
            ),
            (
                Vec3::new(side * 0.16, -0.43, 0.0),
                Vec3::new(side * 0.30, -0.30, -0.07),
                0.055,
            ),
        ] {
            thorn(&mut arm, base, tip, radius)?;
        }
        for finger in -1_i32..=1 {
            let x = side * 0.07 + finger as f32 * 0.085;
            branch(
                &mut arm,
                &[
                    Vec3::new(x, -0.70, -0.10),
                    Vec3::new(x * 1.10, -0.88, -0.16),
                    Vec3::new(x, -0.95, -0.26),
                ],
                0.075,
            )?;
            thorn(
                &mut arm,
                Vec3::new(x, -0.94, -0.245),
                Vec3::new(x, -1.01, -0.32),
                0.035,
            )?;
        }
        out.push((
            Part::VillagerArm(side),
            Vec3::new(side * 0.47, 1.59, 0.0),
            arm.finish()?,
        ));
        let mut leg = Builder::with_cell_size(0.04);
        branch(
            &mut leg,
            &[
                Vec3::new(0.0, -0.02, 0.02),
                Vec3::new(side * 0.025, -0.32, 0.04),
                Vec3::new(-side * 0.035, -0.56, -0.015),
                Vec3::new(0.0, -0.68, -0.10),
            ],
            0.31,
        )?;
        shape::rounded(
            &mut leg,
            [0.0, -0.69, -0.095],
            [0.34, 0.16, 0.41],
            0.025,
            BARK,
        )?;
        for toe in -1_i32..=1 {
            branch(
                &mut leg,
                &[
                    Vec3::new(toe as f32 * 0.09, -0.69, -0.16),
                    Vec3::new(toe as f32 * 0.11, -0.735, -0.31),
                ],
                0.075,
            )?;
        }
        thorn(
            &mut leg,
            Vec3::new(side * 0.13, -0.26, -0.06),
            Vec3::new(side * 0.22, -0.13, -0.15),
            0.05,
        )?;
        foliage(&mut leg, Vec3::new(0.0, -0.33, 0.07), 4)?;
        out.push((
            Part::Leg(side),
            Vec3::new(side * 0.22, 0.77, 0.0),
            leg.finish()?,
        ));
    }
    Ok(out)
}
