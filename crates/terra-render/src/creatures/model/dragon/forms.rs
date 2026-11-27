//! Sculpted traits of the great bloodlines, in the animated bone's bind frame.
use super::*;
use terra_core::creatures::dragons::Lineage;

// A curved, ridged leaf of membrane or crystal. Both faces are closed around
// a medial rib, so the crest reads from the back as well as from the side.
pub(super) fn blade(
    m: &mut Builder,
    a: Vec3,
    b: Vec3,
    tip: Vec3,
    width: f32,
    color: Rgb,
) -> Result<(), String> {
    let ridge = (a + b) * 0.38 + tip * 0.24;
    let normal = (b - a).cross(tip - a).normalize() * width;
    for (n, shade) in [(normal, 1.), (-normal, 0.82)] {
        let points = if shade == 1. {
            [a, b, tip]
        } else {
            [tip, b, a]
        };
        for i in 0..3 {
            m.triangle(
                [points[i], points[(i + 1) % 3], ridge + n],
                tint(color, shade - i as f32 * 0.045),
            )?;
        }
    }
    Ok(())
}

pub(super) fn sculpt(m: &mut Builder, c: Coat, form: Lineage, j: Joint) -> Result<(), String> {
    match (form, j) {
        (Lineage::Shadow, Joint::Skull) => {
            // Swept forked horns project far behind the narrow swept face.
            for s in [-1., 1.] {
                let root = v(s * 0.80, 9.11, -6.15);
                let elbow = root + v(s * 0.93, 0.92, 1.78);
                horn(m, root, elbow, root + v(s * 1.84, 1.56, 3.94), 0.53, c.horn)?;
                horn(
                    m,
                    elbow,
                    elbow + v(s * 0.42, 0.64, 0.28),
                    elbow + v(s * 0.52, 1.02, 0.91),
                    0.26,
                    c.ridge,
                )?;
            }
        }
        (Lineage::Shadow, Joint::Tail(i)) => {
            let node = tail_node(i);
            let end = tail_node(i + 1);
            for s in [-1., 1.] {
                let width = 1.05 - f32::from(i) * 0.10;
                blade(
                    m,
                    node + v(s * width, 0.15, 0.),
                    end + v(s * width * 0.8, 0.16, 0.),
                    end + v(s * (width + 0.8), 1.32, 0.93),
                    0.18,
                    c.ridge,
                )?;
            }
        }
        (Lineage::Tidal, Joint::Tail(i)) => {
            let a = tail_node(i);
            let b = tail_node(i + 1);
            for s in [-1., 1.] {
                let vane = if i >= 4 { 2.55 } else { 1.12 };
                let base = a + v(s * 0.35, 0.2, 0.);
                let tip = b + v(s * vane, vane * 0.60, 1.5);
                blade(m, base, b + v(s * 0.32, 0.35, 0.), tip, 0.13, c.wing)?;
                beam(m, base, tip, 0.15, 0.026, c.growth)?;
            }
        }
        (Lineage::Stone, Joint::Trunk) => {
            for i in 0..5 {
                let z = -2.1 + i as f32 * 1.2;
                let radius = 2.4 - i as f32 * 0.13;
                for s in [-1., 1.] {
                    let p = v(s * radius * 0.64, 6.19 - i as f32 * 0.13, z);
                    block(
                        m,
                        p,
                        v(1.88, 1.22, 1.73),
                        Quat::from_rotation_z(-s * 0.35),
                        c.ridge,
                    )?;
                    horn(
                        m,
                        p,
                        p + v(s * 0.43, 0.96, 0.16),
                        p + v(s * 0.67, 1.30, 0.75),
                        0.62,
                        c.horn,
                    )?;
                    block(
                        m,
                        p + v(s * 0.25, 0.59, 0.12),
                        v(0.95, 0.18, 1.20),
                        Quat::from_rotation_z(-s * 0.35),
                        c.growth,
                    )?;
                }
            }
        }
        (Lineage::Stone, Joint::Thigh { side, hind }) => {
            let p = j.pivot() + v(f32::from(side) * 0.58, -0.20, 0.);
            block(
                m,
                p,
                v(1.48, 1.83, 1.93),
                Quat::from_rotation_x(if hind { 0.28 } else { -0.22 }),
                c.ridge,
            )?;
            horn(
                m,
                p,
                p + v(f32::from(side) * 0.66, 0.65, 0.16),
                p + v(f32::from(side) * 0.84, 0.78, 0.76),
                0.45,
                c.horn,
            )?;
        }
        (Lineage::Stone, Joint::Tail(i)) if i >= 4 => {
            let a = tail_node(i);
            let b = tail_node(i + 1);
            let r = if i == 4 { 0.94 } else { 1.31 };
            sweep(
                m,
                &[
                    sec(a, 0.57, 0.51),
                    sec(a.lerp(b, 0.5), r, r * 0.79),
                    sec(b, 0.61, 0.56),
                ],
                7,
                c.ridge,
            )?;
            for s in [-1., 1.] {
                horn(
                    m,
                    b,
                    b + v(s * r * 0.6, 0.6, 0.3),
                    b + v(s * r * 0.85, 1.08, 0.63),
                    0.40,
                    c.horn,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Raised paired runes follow the dark armour; the tidal species has a
/// luminous lateral line and the stone species exposes its mineral veins.
pub(super) fn marks(m: &mut Builder, c: Coat, form: Lineage, j: Joint) -> Result<(), String> {
    let (center, width, height, length) = match j {
        Joint::Skull => (v(0., 8.7, -7.45), 1.10, 0.72, 1.4),
        Joint::Trunk if form == Lineage::Tidal => (v(0., 4.28, -1.5), 1.42, 0.80, 3.5),
        Joint::Trunk => (v(0., 4.70, -1.5), 2.57, 1.43, 3.5),
        Joint::Tail(i) => (tail_node(i), 1.21 - f32::from(i) * 0.16, 0.48, 1.8),
        _ => unreachable!(),
    };
    for s in [-1., 1.] {
        for i in 0..3 {
            let t = i as f32 / 3.;
            let a = center + v(s * width, height * 0.46, t * length);
            let b = a + v(-s * width * 0.16, height * 0.61, length * 0.17);
            let end = b + v(s * width * 0.12, -height * 0.29, length * 0.14);
            match form {
                Lineage::Shadow => {
                    beam(m, a, b, 0.083, 0.042, c.light)?;
                    beam(m, b, end, 0.05, 0.019, c.light)?;
                }
                Lineage::Tidal => {
                    blade(m, a, b, end + Vec3::Z * 0.23, 0.054, c.growth)?;
                }
                Lineage::Stone => {
                    beam(m, a, b, 0.084, 0.052, c.light)?;
                    beam(m, b, b + v(0., 0.05, 0.21), 0.05, 0.021, c.light)?;
                }
            }
        }
    }
    Ok(())
}
