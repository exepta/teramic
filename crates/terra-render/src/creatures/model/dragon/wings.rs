use super::*;
use std::f32::consts::PI;

fn membrane(
    mesh: &mut Builder,
    wrist: Vec3,
    a: Vec3,
    b: Vec3,
    color: Rgb,
    edge_bend: [Vec3; 2],
) -> Result<(), String> {
    let surface = |r: f32, t: f32| {
        // The rim rises between supporting fingers, forming the scalloped
        // silhouette. Camber vanishes along both fingers and at the wrist.
        let end = a.lerp(b, t);
        let arch = (PI * t).sin();
        let radial = r * (1. - 0.18 * arch * r);
        wrist.lerp(end, radial)
            + edge_bend[0].lerp(edge_bend[1], t) * (PI * radial).sin()
            + Vec3::Y * (arch * (r * PI).sin() * 0.49)
    };
    for ring in 0..4 {
        let r = ring as f32 * 0.25;
        let next = r + 0.25;
        for j in 0..5 {
            let u = j as f32 / 5.;
            let w = (j + 1) as f32 / 5.;
            let shade = 0.77 + next * 0.19 + 0.14 * ((u + w) * 0.5 * PI).sin();
            let a = surface(r, u);
            let b = surface(next, u);
            let c = surface(next, w);
            mesh.triangle([a, b, c], tint(color, shade))?;
            if ring > 0 {
                mesh.triangle([a, c, surface(r, w)], tint(color, shade * 1.018))?;
            }
        }
    }
    Ok(())
}

pub(super) fn wing(mesh: &mut Builder, c: Coat, joint: Joint, s: f32) -> Result<(), String> {
    let anchor = Joint::WingArm(s as i8).pivot();
    let knuckle = wrist(s);
    let elbow = v(4.75 * s, 8.50, 0.33);
    if matches!(joint, Joint::WingArm(_)) {
        // Arm and fan meet along the shoulder–wrist line, which is the axis
        // of the fan's fold. Both sides of that seam stay attached in flight.
        membrane(mesh, anchor, elbow, knuckle, c.wing, [Vec3::ZERO; 2])?;
        let path = [
            sec(anchor, 0.90, 0.89),
            sec(anchor.lerp(elbow, 0.56), 0.76, 0.72),
            sec(elbow, 0.67, 0.65),
            sec(elbow.lerp(knuckle, 0.40), 0.54, 0.54),
            sec(elbow.lerp(knuckle, 0.76), 0.49, 0.49),
            sec(knuckle, 0.45, 0.42),
        ];
        armoured(mesh, &path, 7, c)?;
        horn(
            mesh,
            knuckle,
            knuckle + v(s * 0.43, 0.60, -0.34),
            knuckle + v(s * 0.75, 0.98, -0.15),
            0.30,
            c.horn,
        )?;
        return Ok(());
    }
    let fingers = [
        v(s * 18.4, 5.0, 2.45),
        v(s * 15.95, 3.42, 6.85),
        v(s * 12.82, 3.35, 9.26),
        v(s * 8.28, 3.15, 9.53),
        v(s * 4.69, 3.48, 6.94),
        anchor,
    ];
    for (i, tip) in fingers[..5].iter().copied().enumerate() {
        let bend = v(s * 0.23, 0.65, -0.17);
        membrane(
            mesh,
            knuckle,
            tip,
            fingers[i + 1],
            tint(c.wing, 1. - i as f32 * 0.035),
            [bend, if i < 4 { bend } else { Vec3::ZERO }],
        )?;
        if c.kind == Kind::FireDragon {
            // A hotter translucent-looking fringe follows the heated outer web,
            // framed by the darker structural fingers.
            let next = fingers[i + 1];
            let middle = tip.lerp(next, 0.53).lerp(knuckle, 0.13) + Vec3::Y * 0.20;
            forms::blade(
                mesh,
                tip + Vec3::Y * 0.12,
                next + Vec3::Y * 0.12,
                middle,
                0.14,
                tint(c.light, 0.86),
            )?;
        }
        // Flesh on the leading edge is thicker; the trailing fingers taper.
        let width = if i == 0 {
            0.52
        } else {
            0.29 - i as f32 * 0.025
        };
        let mid = knuckle.lerp(tip, 0.47) + v(s * 0.23, 0.65, -0.17);
        let near = knuckle.lerp(tip, 0.86) + Vec3::Y * 0.25;
        if i == 0 {
            armoured(
                mesh,
                &[
                    sec(knuckle, width, width),
                    sec(knuckle.lerp(mid, 0.52), width * 0.91, width * 0.84),
                    sec(mid, width * 0.81, width * 0.75),
                    sec(mid.lerp(near, 0.53), width * 0.63, width * 0.60),
                    sec(near, width * 0.49, width * 0.46),
                ],
                7,
                c,
            )?;
        } else {
            sweep(
                mesh,
                &[
                    sec(knuckle, width, width),
                    sec(mid, width * 0.66, width * 0.64),
                    sec(near, width * 0.39, width * 0.37),
                ],
                6,
                c.ridge,
            )?;
        }
        horn(
            mesh,
            near,
            near.lerp(tip, 0.56),
            tip + v(s * 0.10, -0.34, 0.14),
            width * 0.46,
            c.horn,
        )?;
    }
    beam(mesh, knuckle, anchor, 0.21, 0.35, c.scale)
}

fn vein(m: &mut Builder, a: Vec3, b: Vec3, width: f32, color: Rgb) -> Result<(), String> {
    let delta = b - a;
    m.rotated(
        ((a + b) * 0.5).to_array(),
        [width, 0.10, delta.length() + 0.04],
        color,
        Quat::from_rotation_arc(Vec3::Z, delta.normalize()),
    )
}

/// The light follows the wing's scallops and tapers into the finger tips.
pub(super) fn lit_rims(mesh: &mut Builder, c: Coat, s: f32) -> Result<(), String> {
    let tips = [
        v(s * 18.4, 5., 2.45),
        v(s * 15.95, 3.42, 6.85),
        v(s * 12.82, 3.35, 9.26),
        v(s * 8.28, 3.15, 9.53),
        v(s * 4.69, 3.48, 6.94),
        Joint::WingArm(s as i8).pivot(),
    ];
    let knuckle = wrist(s);
    let bend = v(s * 0.23, 0.65, -0.17);
    for (i, pair) in tips.windows(2).enumerate() {
        let on_rim = |t: f32| {
            let radial = 1. - 0.18 * (PI * t).sin();
            let offset = bend.lerp(if i == 4 { Vec3::ZERO } else { bend }, t) * (PI * radial).sin();
            knuckle.lerp(pair[0].lerp(pair[1], t), radial) + offset
        };
        // Restrict the shine to the exposed portion between two fingers. A
        // full bright outline washed out the local colour of folded wings.
        for j in 0..3 {
            let t = j as f32 * 0.15 + 0.14;
            let upper = Vec3::Y * 0.20;
            let a = on_rim(t) + upper;
            let b = on_rim(t + 0.15) + upper;
            vein(mesh, a, b, 0.13, tint(c.light, 0.81))?;
        }
        // Fissures branch part way back into the membrane; keep the opposing
        // faces connected in silhouette instead of floating a second sail.
        let tip = pair[0];
        let lit = |t: f32| knuckle.lerp(tip, t) + bend * (PI * t).sin() + Vec3::Y * 0.09;
        vein(mesh, lit(0.79), lit(0.93), 0.13, tint(c.light, 0.76))?;
        vein(
            mesh,
            lit(0.90),
            lit(0.83) + v(s * 0.14, 0.01, 0.28),
            0.085,
            c.light,
        )?;
    }
    Ok(())
}
