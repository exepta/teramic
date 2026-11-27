use super::*;
use std::f32::consts::TAU;

pub(super) fn build(mesh: &mut Builder, c: Coat, joint: Joint) -> Result<(), String> {
    match joint {
        Joint::Missile(i) => return spells::missile(mesh, c, i),
        Joint::Ward => return spells::ward(mesh, c),
        Joint::Surge => return spells::surge(mesh, c),
        Joint::Rune
            if matches!(
                lineage(c.kind, c.variant),
                Some(Lineage::Shadow | Lineage::Stone)
            ) =>
        {
            return spells::fissure(mesh, c, false);
        }
        Joint::Breath => breath(mesh, c, false)?,
        Joint::Impact if c.kind == Kind::WaterDragon => spells::wave(mesh, c)?,
        Joint::Impact if c.kind == Kind::DarkDragon => {
            for arc in 0..7 {
                let angle = TAU * arc as f32 / 7.;
                for strand in 0..2 {
                    let path: Vec<_> = (0..=9)
                        .map(|n| {
                            let t = n as f32 / 9.;
                            let bend = angle + 0.86 * t + 0.12 * strand as f32;
                            let r = 2.4 + 3.1 * t + 0.55 * strand as f32;
                            let width = 0.035 + 0.25 * (std::f32::consts::PI * t).sin();
                            sec(
                                v(bend.cos() * r, 0.18 + 2.6 * t * t, bend.sin() * r),
                                width,
                                width * 0.38,
                            )
                        })
                        .collect();
                    sweep(
                        mesh,
                        &path,
                        4,
                        if strand == 0 { c.shadow } else { c.growth },
                    )?;
                }
            }
        }
        Joint::Impact => {
            for i in 0..22 {
                let theta = i as f32 * TAU / 22.;
                let radial = v(theta.cos(), 0., theta.sin());
                let r = 3.5 + ((i * 7 % 5) as f32) * 0.69;
                let root = radial * r;
                let h = 1.5 + ((i * 3 % 7) as f32) * 0.58;
                horn(
                    mesh,
                    root,
                    root + radial * 0.62 + Vec3::Y * h * 0.56,
                    root + radial * 1.12 + Vec3::Y * h,
                    0.50 + 0.08 * (i % 3) as f32,
                    if i % 3 == 0 { c.horn } else { c.growth },
                )?;
            }
        }
        Joint::Rune => {
            for ring in 0..2 {
                let radius = if ring == 0 { 4.4 } else { 6.0 };
                for i in 0..48 {
                    let point = |n: i32| {
                        let theta = n as f32 * TAU / 48.;
                        let height = if c.kind == Kind::WaterDragon {
                            (theta * 3. + ring as f32).sin() * 0.52 + ring as f32 * 0.85 + 0.61
                        } else {
                            0.06
                        };
                        v(theta.cos() * radius, height, theta.sin() * radius)
                    };
                    beam(mesh, point(i), point(i + 1), 0.095, 0.088, c.light)?;
                    if ring == 1 && i % 6 == 0 {
                        let a = point(i);
                        let b = point(i + 2);
                        beam(mesh, a * 1.04, b * 1.23, 0.11, 0.09, c.growth)?;
                        beam(mesh, b * 1.23, point(i + 4) * 1.04, 0.09, 0.11, c.growth)?;
                    }
                }
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

/// Author cross-sections in XY. Only Z gets stretched to cover the shot distance;
/// no facet or blending layer can protrude backwards through the jaw.
pub(super) fn breath(mesh: &mut Builder, c: Coat, light: bool) -> Result<(), String> {
    if c.kind == Kind::EarthDragon {
        return rock_volley(mesh, c, light);
    }
    if c.kind == Kind::FireDragon {
        // Interlocking pointed tongues form a billowing cone with a hot core.
        for i in 0..29 {
            let t = i as f32 / 32.;
            let theta = i as f32 * 2.399963;
            let spread = (0.10 + t * 1.65) * if light { 0.46 } else { 1. };
            let base = v(theta.cos() * spread, theta.sin() * spread, -t);
            let r = (0.11 + 0.55 * t) * if light { 0.43 } else { 1. };
            let tip = base + v(0.38 * r * theta.cos(), 0.70 * r, -0.10);
            let color = if light {
                c.light
            } else if i % 4 == 0 {
                c.growth
            } else {
                c.wing
            };
            for j in 0..5 {
                let edge = |j: usize| {
                    let a = theta + TAU * j as f32 / 5.;
                    base + v(a.cos() * r, a.sin() * r, 0.)
                };
                mesh.triangle(
                    [edge(j), tip, edge(j + 1)],
                    tint(color, 0.84 + 0.03 * j as f32),
                )?;
            }
        }
        return Ok(());
    }
    let (arms, turns, spread) = match c.kind {
        Kind::WaterDragon => (3, 8.8, 0.87),
        _ => (2, 5.4, 1.21),
    };
    for arm in 0..arms {
        let ring = |step: usize| {
            let t = step as f32 / 20.;
            let a = TAU * arm as f32 / arms as f32 + t * turns;
            let envelope = t.sqrt() * (1. - 0.30 * t.powi(5));
            let edges = (a * 2.4 + t * 7.).sin();
            let radius = match c.kind {
                Kind::WaterDragon => 0.09 + envelope * spread,
                _ => 0.09 + envelope * spread * (0.64 + 0.36 * edges),
            };
            let width = (0.07 + 0.38 * envelope) * if light { 0.24 } else { 1. };
            let radial = v(a.cos(), a.sin(), 0.);
            let tangent = v(-a.sin(), a.cos(), 0.);
            let center = radial * (radius + if light { width + 0.06 } else { 0. }) - Vec3::Z * t;
            [
                center + radial * width * 0.55,
                center + tangent * width,
                center - radial * width * 0.55,
                center - tangent * width,
            ]
        };
        let broken = |n: usize| light && (n + 2 * arm) % 11 == 3;
        for n in 0..20 {
            if broken(n) {
                continue;
            }
            let a = ring(n);
            let b = ring(n + 1);
            let color = if light {
                c.light
            } else if c.kind == Kind::DarkDragon {
                c.shadow
            } else {
                c.wing
            };
            for k in 0..4 {
                let j = (k + 1) % 4;
                mesh.quad(
                    [a[k], b[k], b[j], a[j]],
                    tint(color, 0.78 + 0.07 * k as f32),
                )?;
            }
            if n == 0 || broken(n - 1) {
                mesh.quad(a, color)?;
            }
            if n == 19 || broken(n + 1) {
                mesh.quad([b[3], b[2], b[1], b[0]], color)?;
            }
        }
    }
    Ok(())
}

fn rock_volley(mesh: &mut Builder, c: Coat, light: bool) -> Result<(), String> {
    // Each shard has its own fractured silhouette. Longitudinal dimensions
    // are fractions of the cast, while lateral dimensions stay in metres.
    for i in 0..29 {
        let t = (0.6 + i as f32) / 30.;
        let a = i as f32 * 2.399963;
        let r = (0.1 + t.sqrt()) * (0.26 + (i * 7 % 13) as f32 * 0.145);
        let center = v(r * a.cos(), r * a.sin(), -t);
        let width = (0.15 + (i * 3 % 5) as f32 * 0.08) * (0.48 + t);
        let along = 0.009 + (i * 5 % 6) as f32 * 0.0025;
        let flank = |n: usize| {
            let f = a + TAU * n as f32 / 5.;
            center + v(f.cos() * width, f.sin() * width, 0.)
        };
        let nose = center + v(width * 0.27, 0., -along);
        let tail = center + v(-width * 0.64, 0., along * (if light { 2.1 } else { 1.5 }));
        if light {
            let edge = flank(0);
            let normal = (edge - center).normalize() * 0.012;
            let edge2 = edge.lerp(flank(1), 0.18);
            mesh.triangle([nose + normal, edge2 + normal, edge + normal], c.light)?;
            mesh.triangle([tail + normal, edge + normal, edge2 + normal], c.light)?;
        } else {
            for n in 0..5 {
                let v = flank(n);
                let w = flank((n + 1) % 5);
                let face = if (i + n) % 3 == 0 { c.horn } else { c.ridge };
                mesh.triangle([nose, w, v], face)?;
                mesh.triangle([tail, v, w], tint(face, 0.64))?;
            }
        }
    }
    Ok(())
}
