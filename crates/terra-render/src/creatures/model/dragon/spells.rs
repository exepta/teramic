use super::*;
use std::f32::consts::TAU;

fn circle(
    mesh: &mut Builder,
    center: Vec3,
    radius: f32,
    width: f32,
    color: Rgb,
) -> Result<(), String> {
    for n in 0..28 {
        let at = |i| {
            let (sn, cs) = (TAU * i as f32 / 28.).sin_cos();
            center + v(cs * radius, 0., sn * radius)
        };
        beam(mesh, at(n), at(n + 1), width, width, color)?;
    }
    Ok(())
}

pub(super) fn missile(m: &mut Builder, c: Coat, i: u8) -> Result<(), String> {
    let stone = c.kind == Kind::EarthDragon;
    let r = if stone { 0.82 } else { 0.60 } * (1.0 - 0.07 * f32::from(i));
    sweep(
        m,
        &[
            sec(v(0., -r, 0.), 0.22 * r, 0.19 * r),
            sec(v(0.10 * r, -r * 0.48, 0.), 0.83 * r, 0.78 * r),
            sec(v(-0.09 * r, r * 0.27, 0.), r, 0.91 * r),
            sec(v(0., r * 0.86, 0.), r * 0.60, r * 0.54),
            sec(v(0., r, 0.), r * 0.19, r * 0.17),
        ],
        if stone { 6 } else { 10 },
        if stone { c.ridge } else { c.growth },
    )?;
    if !stone {
        circle(m, Vec3::ZERO, r * 1.17, 0.049, c.light)?;
        for z in [0.8, 1.42] {
            horn(
                m,
                v(0., 0., z * r),
                v(0.09, 0.06, z * r + 0.31),
                v(0.18, 0.1, z * r + 0.65),
                0.13,
                c.light,
            )?;
        }
    }
    Ok(())
}

pub(super) fn ward(m: &mut Builder, c: Coat) -> Result<(), String> {
    let form = lineage(c.kind, c.variant).expect("ward belongs to a greater dragon");
    for i in 0..12 {
        let (sn, cs) = (i as f32 * TAU / 12.).sin_cos();
        let radial = v(cs, 0., sn);
        match form {
            Lineage::Stone => {
                let r = 4.9 + (i % 3) as f32 * 0.38;
                let h = 2.5 + (i * 7 % 5) as f32 * 0.43;
                let p = radial * r;
                sweep(
                    m,
                    &[
                        sec(p + Vec3::Y * 0.15, 0.89, 0.64),
                        sec(p + Vec3::Y * (h * 0.47), 1.14, 0.73),
                        sec(p - radial * 0.47 + Vec3::Y * h, 0.62, 0.49),
                        sec(p - radial * 0.82 + Vec3::Y * (h + 0.55), 0.18, 0.11),
                    ],
                    6,
                    c.ridge,
                )?;
                let chip = p + radial * 0.50 + Vec3::Y * (h * 0.55);
                horn(
                    m,
                    chip,
                    chip + radial * 0.37 + Vec3::Y * 0.37,
                    chip + radial * 0.41 + Vec3::Y * 0.81,
                    0.24,
                    c.light,
                )?;
            }
            Lineage::Tidal => {
                for j in 0..8 {
                    let point = |k| {
                        let b = k as f32 * std::f32::consts::FRAC_PI_2 / 8.;
                        radial * (6.7 * b.cos()) + Vec3::Y * (9.6 * b.sin())
                    };
                    beam(m, point(j), point(j + 1), 0.073, 0.055, c.light)?;
                }
                let drop = radial * (2.3 + (i % 4) as f32) + Vec3::Y * (5.7 + (i % 3) as f32);
                beam(m, drop, drop + Vec3::Y * 1.1, 0.038, 0.014, c.growth)?;
            }
            Lineage::Shadow => {
                let p = radial * 5.6 + Vec3::Y * (2. + (i % 2) as f32);
                let side = radial.cross(Vec3::Y);
                for sign in [-1., 1.] {
                    beam(
                        m,
                        p - Vec3::Y * 1.1,
                        p + side * sign * 0.66,
                        0.09,
                        0.075,
                        c.light,
                    )?;
                    beam(
                        m,
                        p + side * sign * 0.66,
                        p + Vec3::Y * 1.1,
                        0.075,
                        0.055,
                        c.light,
                    )?;
                }
            }
        }
    }
    circle(m, Vec3::Y * 0.23, 6.0, 0.09, c.growth)
}

pub(super) fn surge(m: &mut Builder, c: Coat) -> Result<(), String> {
    match lineage(c.kind, c.variant).expect("surge belongs to a greater dragon") {
        Lineage::Tidal => {
            for i in 0..6 {
                let h = 5. + (i % 3) as f32 * 1.13;
                let path: Vec<_> = (0..=10)
                    .map(|step| {
                        let t = step as f32 / 10.;
                        let angle = TAU * i as f32 / 6. + t * 3.3;
                        let radial = v(angle.cos(), 0., angle.sin());
                        let radius = 0.52 + t * 0.8 + t.powi(4) * 1.45;
                        let width = (1. - t) * 0.41 + 0.04;
                        sec(
                            radial * radius + Vec3::Y * (h * t * (1.20 - 0.20 * t)),
                            width,
                            width * 0.77,
                        )
                    })
                    .collect();
                sweep(m, &path, 6, if i % 2 == 0 { c.wing } else { c.growth })?;
            }
            circle(m, Vec3::Y * 0.18, 2.0, 0.22, c.light)
        }
        Lineage::Stone => {
            for i in 0..21 {
                let (sn, cs) = (TAU * i as f32 / 21.).sin_cos();
                let radial = v(cs, 0., sn);
                let p = radial * (1.7 + (i % 4) as f32 * 0.9)
                    + Vec3::Y * (0.45 + (i % 5) as f32 * 0.55);
                block(
                    m,
                    p,
                    v(0.51, 0.85, 0.43),
                    Quat::from_rotation_x(i as f32),
                    c.ridge,
                )?;
                beam(m, p - radial * 0.66, p, 0.13, 0.05, c.belly)?;
            }
            circle(m, Vec3::Y * 0.12, 5.1, 0.12, c.growth)
        }
        Lineage::Shadow => {
            for arm in 0..3 {
                for n in 0..18 {
                    let point = |i| {
                        let t = i as f32 / 18.;
                        let angle = TAU * arm as f32 / 3. + t * 3.7;
                        let r = 3.2 + (t * std::f32::consts::PI).sin() * 1.25;
                        v(r * angle.cos(), 0.4 + t * 6.1, r * angle.sin())
                    };
                    let x = n as f32 / 18.;
                    let w = (x * std::f32::consts::PI).sin() * 0.12 + 0.025;
                    beam(m, point(n), point(n + 1), w, w * 0.86, c.growth)?;
                }
            }
            Ok(())
        }
    }
}

pub(super) fn fissure(m: &mut Builder, c: Coat, core: bool) -> Result<(), String> {
    // The longitudinal axis is normalised, so absolutely no bevel width is
    // applied in Z. During a long cast that width would turn into metres.
    let point = |n: usize| {
        v(
            ((n * 43 % 13) as f32 - 6.) * 0.097,
            if core { 0.145 } else { 0.108 },
            -(n as f32 / 28.),
        )
    };
    let mut vein = |a: Vec3, b: Vec3, r: f32, s: f32| -> Result<(), String> {
        m.quad(
            [
                a - Vec3::X * r,
                a + Vec3::X * r,
                b + Vec3::X * s,
                b - Vec3::X * s,
            ],
            if core { c.light } else { c.shadow },
        )
    };
    for i in 0..28 {
        let (a, b) = (point(i), point(i + 1));
        let width = |n: usize| {
            if core {
                0.036 + (n * 3 % 7) as f32 * 0.009
            } else {
                0.19 + (n * 3 % 7) as f32 * 0.029
            }
        };
        // Leave cold patches between incandescent seams.
        if !core || i % 8 != 5 {
            vein(a, b, width(i), width(i + 1))?;
        }
        if i % 3 == 0 {
            let sign = if (i / 3) % 2 == 0 { 1. } else { -1. };
            let fork = a + v(sign * (0.46 + 0.10 * (i % 5) as f32), 0., -0.017);
            let tip = fork + v(sign * (0.73 + 0.16 * (i % 3) as f32), 0., -0.04);
            vein(a, fork, width(i) * 0.72, width(i) * 0.42)?;
            vein(fork, tip, width(i) * 0.42, 0.008)?;
        }
    }
    if !core {
        for i in (2..27).step_by(3) {
            let p = point(i);
            let sign = if i % 2 == 0 { 1. } else { -1. };
            let foot = p + v(sign * (0.39 + 0.07 * (i % 5) as f32), 0., -0.006);
            let tip = foot + v(sign * 0.34, 0.42 + 0.18 * (i * 3 % 5) as f32, -0.024);
            let far = foot + v(sign * 0.28, 0., -0.020);
            m.triangle([foot, tip, far], c.ridge)?;
            m.triangle([p, tip, foot], c.scale)?;
            m.triangle([p, far, tip], tint(c.growth, 0.59))?;
        }
    }
    Ok(())
}

pub(super) fn wave(m: &mut Builder, c: Coat) -> Result<(), String> {
    for i in 0..28 {
        let point = |j| {
            let (sn, cs) = (TAU * j as f32 / 28.).sin_cos();
            v(cs, 0., sn)
        };
        let a = point(i);
        let b = point(i + 1);
        let foot = a * 4.1;
        let crest = a * 6.1 + Vec3::Y * (1.85 + 0.35 * (i as f32 * 1.7).sin());
        let next = b * 6.1 + Vec3::Y * (1.85 + 0.35 * ((i + 1) as f32 * 1.7).sin());
        m.quad(
            [foot, b * 4.1, next, crest],
            tint(c.wing, 0.88 + (i % 4) as f32 * 0.035),
        )?;
        beam(m, crest, next, 0.12, 0.11, c.light)?;
    }
    Ok(())
}
