//! Distinct silhouettes for the river serpent, stone-plated quadruped and winged hunters.
use super::*;

pub(super) fn serpent(m: &mut Builder, c: Coat) -> Result<(), String> {
    let path = [
        sec(v(0., 5.20, -2.75), 1.36, 1.48),
        sec(v(0., 4.85, -1.45), 1.56, 1.46),
        sec(v(0., 4.32, 0.), 1.60, 1.40),
        sec(v(0., 3.94, 1.73), 1.47, 1.28),
        sec(tail_node(0), 1.23, 1.18),
    ];
    armoured(m, &path, 10, c)?;
    for (i, r) in path.iter().enumerate() {
        let origin = r.pos + Vec3::Y * r.depth;
        forms::blade(
            m,
            origin - Vec3::Z * 0.52,
            origin + Vec3::Z * 0.70,
            origin + v(0., 1.26, 1.47),
            0.20,
            c.growth,
        )?;
        let belly = r.pos - Vec3::Y * (r.depth * 0.87);
        m.rotated(
            belly.to_array(),
            [r.width * 1.40, 0.33, 1.20],
            tint(c.belly, 0.94 + i as f32 * 0.012),
            Quat::from_rotation_x(0.24),
        )?;
    }
    Ok(())
}

pub(super) fn details(m: &mut Builder, c: Coat, j: Joint) -> Result<(), String> {
    match (c.kind, j) {
        (Kind::WaterDragon, Joint::Skull) => {
            for side in [-1., 1.] {
                // Long flexible barbels sweep out from either side of the pale muzzle.
                let path = [
                    sec(v(side * 0.48, 8.47, -8.84), 0.18, 0.16),
                    sec(v(side * 1.18, 8.10, -9.60), 0.15, 0.13),
                    sec(v(side * 1.81, 7.35, -9.90), 0.13, 0.12),
                    sec(v(side * 2.47, 7.04, -9.56), 0.11, 0.10),
                    sec(v(side * 2.92, 7.28, -9.12), 0.08, 0.07),
                    sec(v(side * 3.05, 7.65, -9.08), 0.04, 0.04),
                ];
                sweep(m, &path, 5, c.growth)?;
                let root = v(side * 0.67, 9.29, -6.34);
                let bend = root + v(side * 1.15, 1.68, 1.25);
                horn(
                    m,
                    root,
                    bend,
                    bend + v(side * 0.44, 1.05, 1.46),
                    0.41,
                    [0.83, 0.62, 0.33],
                )?;
                horn(
                    m,
                    bend,
                    bend + v(-side * 0.28, 0.78, 0.16),
                    bend + v(-side * 0.12, 1.37, 0.46),
                    0.25,
                    [0.94, 0.79, 0.53],
                )?;
            }
        }
        (Kind::WaterDragon, Joint::Tail(i)) => {
            let node = tail_node(i);
            let end = tail_node(i + 1);
            let w = 1.15 - 0.13 * f32::from(i);
            let origin = node + Vec3::Y * w;
            forms::blade(
                m,
                origin,
                end + Vec3::Y * w,
                end + v(0., 1.31 + w, 0.49),
                0.18,
                c.growth,
            )?;
            if i + 1 == TAIL_BONES {
                for side in [-1., 1.] {
                    forms::blade(
                        m,
                        node,
                        end + Vec3::Z * 1.3,
                        end + v(side * 2.25, 0.49, 2.6),
                        0.19,
                        c.wing,
                    )?;
                }
            }
        }
        (Kind::EarthDragon, Joint::Trunk) => {
            for row in 0..6 {
                let z = row as f32 * 1.16 - 2.9;
                for s in [-1., 1.] {
                    let top = v(s * 1.64, 6.0 - 0.08 * row as f32, z);
                    m.rotated(
                        top.to_array(),
                        [2.14, 1.15, 1.72],
                        tint(c.ridge, 0.81 + 0.06 * (row % 3) as f32),
                        Quat::from_rotation_z(-s * 0.39) * Quat::from_rotation_y(s * 0.23),
                    )?;
                    let moss = top + v(-s * 0.17, 0.58, 0.13);
                    m.rotated(
                        moss.to_array(),
                        [1.67, 0.17, 1.21],
                        [0.29, 0.37 + 0.023 * row as f32, 0.12],
                        Quat::from_rotation_z(-s * 0.39),
                    )?;
                    if row % 2 == 0 {
                        let root = top + v(-s * 0.39, 0.47, 0.20);
                        horn(
                            m,
                            root,
                            root + v(s * 0.26, 0.91, 0.13),
                            root + v(s * 0.48, 1.52, 0.57),
                            0.49,
                            c.light,
                        )?;
                    }
                }
            }
        }
        (Kind::FireDragon, Joint::Skull | Joint::Neck | Joint::Tail(_)) => {
            let root = match j {
                Joint::Skull => v(0., 9.48, -6.13),
                Joint::Neck => v(0., 7.86, -4.72),
                Joint::Tail(i) => tail_node(i) + Vec3::Y * (1.18 - f32::from(i) * 0.13),
                _ => unreachable!(),
            };
            for s in [-1., 1.] {
                forms::blade(
                    m,
                    root + v(s * 0.38, 0., -0.29),
                    root + v(s * 0.45, 0.22, 0.70),
                    root + v(s * 1.00, 1.14, 1.62),
                    0.21,
                    c.light,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// These markings are painted onto occupied voxels, so they stay attached
/// to the scales through every pose, including along the underside of a bank.
pub(super) fn coat_marking(c: Coat, joint: Joint, p: Vec3, base: Rgb) -> Rgb {
    let x = p.x.abs();
    let form = f32::from(c.variant);
    let marked = match joint {
        // Three oblique bars frame the socket and continue towards the gill hinge.
        Joint::Skull if x > 0.79 && p.y > 8.23 && p.y < 9.25 && (-7.65..-5.72).contains(&p.z) => {
            (p.z + 0.61 * p.y + 0.09 * form).rem_euclid(0.84) < 0.22
        }
        Joint::Neck if x > 0.60 && p.y < 7.92 => {
            (p.z + 0.78 * p.y + 0.20 * form).rem_euclid(1.42) < 0.29
        }
        Joint::Trunk if p.y > 4.26 && p.y < 6.14 && x > 1.30 => {
            let chevron = (p.z - 0.62 * x + 0.31 * p.y + 0.24 * form).rem_euclid(2.27);
            chevron < 0.39
        }
        _ => false,
    };
    if !marked {
        return base;
    }
    let pigment = match c.kind {
        Kind::WaterDragon => c.belly,
        Kind::FireDragon => c.ridge,
        Kind::EarthDragon => {
            if c.variant == 3 {
                c.ridge
            } else {
                c.growth
            }
        }
        _ => c.wing,
    };
    std::array::from_fn(|i| base[i] * 0.32 + pigment[i] * 0.68)
}
