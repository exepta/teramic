use super::*;

pub(super) fn trunk(mesh: &mut Builder, c: Coat) -> Result<(), String> {
    let path = [
        sec(v(0., 5.10, -3.58), 1.35, 1.61),
        sec(v(0., 4.89, -2.62), 2.24, 2.17),
        sec(v(0., 4.71, -1.37), 2.63, 2.28),
        sec(v(0., 4.53, 0.05), 2.69, 2.08),
        sec(v(0., 4.39, 1.45), 2.38, 1.88),
        sec(v(0., 4.28, 2.64), 2.29, 1.81),
        sec(v(0., 4.04, 3.55), 1.91, 1.54),
        sec(v(0., 3.78, 4.24), 1.32, 1.15),
    ];
    armoured(mesh, &path, 12, c)?;
    for (i, r) in path[1..7].iter().enumerate() {
        let base = r.pos + Vec3::Y * r.depth * 0.95;
        let h = if c.kind == Kind::EarthDragon {
            0.71
        } else {
            1.19
        };
        horn(
            mesh,
            base,
            base + v(0., h * 0.75, 0.19),
            base + v(0., h, 0.63),
            0.40,
            c.horn,
        )?;
        for s in [-1., 1.] {
            let pos = r.pos + v(s * r.width * 0.72, r.depth * 0.78, 0.10);
            if c.kind == Kind::EarthDragon && i % 3 != 0 {
                for j in 0..3 {
                    mesh.rotated(
                        (pos + v(s * j as f32 * 0.13, -j as f32 * 0.10, 0.23 * j as f32))
                            .to_array(),
                        [0.34, 0.13, 0.56],
                        tint(c.growth, 1. - j as f32 * 0.08),
                        Quat::from_rotation_z(-s * 0.78),
                    )?;
                }
            } else if (i + 1) % 2 == 0 {
                horn(
                    mesh,
                    pos,
                    pos + v(s * 0.19, 0.41, 0.17),
                    pos + v(s * 0.39, 0.59, 0.58),
                    0.26,
                    c.growth,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn neck(mesh: &mut Builder, c: Coat) -> Result<(), String> {
    let path = [
        sec(v(0., 5.13, -2.52), 1.72, 1.58),
        sec(v(0., 5.84, -3.33), 1.57, 1.46),
        sec(v(0., 6.61, -3.81), 1.32, 1.39),
        sec(v(0., 7.35, -4.13), 1.12, 1.24),
        sec(v(0., 7.94, -4.64), 0.99, 1.10),
        sec(v(0., 8.30, -5.27), 0.91, 0.98),
        sec(v(0., 8.46, -5.84), 1.00, 0.88),
    ];
    armoured(mesh, &path, 9, c)?;
    // Broad overlapping scutes follow the front of the throat from the breastbone
    // to the chin, where the reference exposes the pale underside of the armour.
    for pair in path.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        let tangent = (b.pos - a.pos).normalize();
        let ventral = tangent.cross(Vec3::X).normalize();
        let frame = Quat::from_mat3(&Mat3::from_cols(Vec3::X, tangent, -ventral));
        let center = a.pos.lerp(b.pos, 0.45) + ventral * ((a.depth + b.depth) * 0.50);
        block(
            mesh,
            center,
            v(
                (a.width + b.width) * 0.72,
                (a.pos.distance(b.pos) + 0.12) * 0.94,
                0.29,
            ),
            frame,
            c.belly,
        )?;
    }

    for r in &path[1..6] {
        let base = r.pos + v(0., 0.73 * r.depth, 0.68 * r.depth);
        horn(
            mesh,
            base,
            base + v(0., 0.65, 0.46),
            base + v(0., 0.97, 0.94),
            0.32,
            c.horn,
        )?;
    }
    Ok(())
}

/// A cut amethyst on the breastbone. Its outer facets and four reflected seams
/// share the neck bone, so they stay visible and attached as the throat rises.
pub(super) fn heart(mesh: &mut Builder, c: Coat) -> Result<(), String> {
    let center = v(0., 4.89, -4.55);
    let slope = Quat::from_rotation_x(-0.62);
    let peak = center + slope * v(0., 0., -0.51);
    let vertices = [
        v(0., 0.88, 0.),
        v(0.77, 0.13, -0.07),
        v(0.43, -0.47, 0.),
        v(0., -0.97, 0.),
        v(-0.43, -0.47, 0.),
        v(-0.77, 0.13, -0.07),
    ];
    for (i, point) in vertices.iter().enumerate() {
        mesh.triangle(
            [
                center + slope * *point,
                center + slope * vertices[(i + 1) % vertices.len()],
                peak,
            ],
            tint(c.light, 0.72 + i as f32 * 0.046),
        )?;
    }
    for side in [-1., 1.] {
        let a = center + slope * v(side * 0.52, 0.03, -0.13);
        let b = center + slope * v(side * 0.98, 0.30, 0.18);
        beam(mesh, a, b, 0.067, 0.045, c.light)?;
        beam(
            mesh,
            b,
            b + v(side * 0.22, 0.40, 0.20),
            0.046,
            0.012,
            c.light,
        )?;
    }
    Ok(())
}

fn eye_position(side: f32) -> Vec3 {
    v(side * 1.19, 8.87, -7.14)
}

pub(super) fn head(mesh: &mut Builder, c: Coat, joint: Joint) -> Result<(), String> {
    if joint == Joint::Eyes {
        for s in [-1., 1.] {
            let pos = eye_position(s);
            let tilt = Quat::from_rotation_x(-0.14);
            block(mesh, pos, v(0.22, 0.33, 0.51), tilt, c.light)?;
            mesh.rotated(
                (pos + v(s * 0.12, 0., -0.025)).to_array(),
                [0.035, 0.20, 0.07],
                tint(c.shadow, 0.60),
                tilt,
            )?;
        }
        return Ok(());
    }
    if joint == Joint::Jaw {
        let p = [
            sec(v(0., 8.03, -6.25), 0.90, 0.48),
            sec(v(0., 7.70, -7.18), 0.99, 0.42),
            sec(v(0., 7.63, -8.22), 0.72, 0.32),
            sec(v(0., 7.70, -9.12), 0.54, 0.28),
        ];
        sweep(mesh, &p, 8, c.belly)?;
        block(
            mesh,
            v(0., 7.90, -8.04),
            v(1.22, 0.17, 1.75),
            Quat::IDENTITY,
            tint(c.shadow, 0.85),
        )?;
        for s in [-1., 1.] {
            for j in 0..4 {
                let t = j as f32 / 3.;
                let root = v(s * (0.76 - 0.27 * t), 7.93, -7.23 - 1.65 * t);
                horn(
                    mesh,
                    root,
                    root + v(0., 0.15, 0.025),
                    root + v(-s * 0.05, 0.26, 0.10),
                    0.125,
                    c.horn,
                )?;
            }
        }
        return Ok(());
    }
    let path = [
        sec(v(0., 8.55, -5.56), 0.93, 0.88),
        sec(v(0., 8.65, -6.29), 1.32, 1.02),
        sec(v(0., 8.71, -7.02), 1.21, 0.87),
        sec(v(0., 8.48, -7.69), 1.03, 0.57),
        sec(v(0., 8.49, -8.54), 0.86, 0.50),
        sec(v(0., 8.42, -9.34), 0.63, 0.43),
    ];
    armoured(mesh, &path, 8, c)?;
    for s in [-1., 1.] {
        let pos = eye_position(s);
        block(
            mesh,
            pos - v(s * 0.095, 0., -0.02),
            v(0.30, 0.56, 0.85),
            Quat::from_rotation_x(-0.18),
            tint(c.shadow, 0.64),
        )?;
        block(
            mesh,
            pos + v(-s * 0.04, 0.31, 0.03),
            v(0.54, 0.31, 1.20),
            Quat::from_rotation_x(-0.18),
            c.ridge,
        )?;
        block(
            mesh,
            v(s * 0.62, 8.69, -9.06),
            v(0.14, 0.15, 0.31),
            Quat::from_rotation_x(-0.2),
            c.shadow,
        )?;
        let crown = v(s * 0.83, 9.28, -6.25);
        let reach = if c.kind == Kind::DarkDragon && c.variant != 0 {
            1.48
        } else {
            1.0
        };
        horn(
            mesh,
            crown,
            crown + v(s * 0.52, 1.07, 1.05) * reach,
            crown + v(s * 1.01, 1.65, 2.12) * reach,
            0.50,
            c.horn,
        )?;
        for i in 0..3 {
            let r = v(
                s * (1.03 + i as f32 * 0.025),
                9.00 - i as f32 * 0.34,
                -6.42 + i as f32 * 0.19,
            );
            horn(
                mesh,
                r,
                r + v(s * 0.63, 0.22, 0.69),
                r + v(s * (1.22 - i as f32 * 0.16), 0.47 - i as f32 * 0.21, 1.61),
                0.32 - i as f32 * 0.038,
                if c.kind == Kind::WaterDragon && i % 2 == 0 {
                    c.growth
                } else {
                    c.horn
                },
            )?;
        }
        for i in 0..5 {
            let t = i as f32 / 4.;
            let r = v(s * (0.88 - 0.32 * t), 8.24 - 0.10 * t, -7.33 - 1.75 * t);
            let h = if i == 1 || i == 4 { 0.57 } else { 0.34 };
            horn(
                mesh,
                r,
                r + v(s * 0.02, -h * 0.65, 0.01),
                r + v(-s * 0.05, -h, 0.15),
                0.135,
                c.horn,
            )?;
        }
    }
    if c.kind == Kind::WaterDragon || c.variant >= 2 {
        for i in 0..3 {
            let r = v(
                0.,
                9.31 + (i as f32 * 0.5).sin() * 0.28,
                -7.03 + 0.51 * i as f32,
            );
            horn(
                mesh,
                r,
                r + v(0., 0.58, 0.15),
                r + v(0., 0.87 + 0.20 * i as f32, 0.54),
                0.27,
                c.growth,
            )?;
        }
    }
    Ok(())
}

pub(super) fn leg(
    mesh: &mut Builder,
    c: Coat,
    joint: Joint,
    s: f32,
    hind: bool,
) -> Result<(), String> {
    let side = s as i8;
    let top = Joint::Thigh { side, hind }.pivot();
    let knee = Joint::Foot { side, hind }.pivot();
    let ankle = Joint::Paw { side, hind }.pivot();
    if matches!(joint, Joint::Thigh { .. }) {
        let path = [
            sec(top, if hind { 1.16 } else { 0.92 }, 1.12),
            sec(
                top.lerp(knee, 0.36) + v(s * 0.13, 0., if hind { -0.32 } else { 0.19 }),
                1.02,
                1.05,
            ),
            sec(top.lerp(knee, 0.69), 0.85, 0.89),
            sec(knee, 0.65, 0.70),
        ];
        armoured(mesh, &path, 8, c)?;
    } else if matches!(joint, Joint::Foot { .. }) {
        let path = [
            sec(knee, 0.68, 0.74),
            sec(knee.lerp(ankle, 0.46) + v(s * 0.03, 0., 0.21), 0.71, 0.70),
            sec(ankle, 0.59, 0.63),
        ];
        armoured(mesh, &path, 7, c)?;
        let dew = knee + v(s * 0.47, -0.08, 0.28);
        horn(
            mesh,
            dew,
            dew + v(s * 0.18, 0.10, 0.32),
            dew + v(s * 0.23, 0.01, 0.63),
            0.20,
            c.horn,
        )?;
    } else {
        // A tapered palm carries three splayed, jointed digits. The heel and
        // talon tips share a sole, so the whole paw can plant on one tread.
        let front = ankle + v(0., -0.34, -0.68);
        sweep(
            mesh,
            &[
                sec(ankle + v(0., -0.10, 0.32), 0.37, 0.34),
                sec(ankle + v(0., -0.17, -0.10), 0.66, 0.42),
                sec(front, 0.72, 0.27),
                sec(front + v(0., -0.03, -0.22), 0.53, 0.22),
            ],
            8,
            c.scale,
        )?;
        for j in [-1_f32, 0., 1.] {
            let root = front + v(j * 0.42, 0.045, j.abs() * 0.15);
            let knuckle = root + v(j * 0.16, -0.028, -0.40);
            let nail = knuckle + v(j * 0.13, -0.10, -0.36);
            sweep(
                mesh,
                &[
                    sec(root, 0.27, 0.23),
                    sec(knuckle, 0.23, 0.21),
                    sec(nail, 0.16, 0.135),
                ],
                7,
                c.ridge,
            )?;
            horn(
                mesh,
                nail,
                nail + v(j * 0.06, 0.045, -0.25),
                nail + v(j * 0.12, -0.19, -0.49),
                0.17,
                c.horn,
            )?;
        }
        let heel = ankle + v(-s * 0.18, -0.26, 0.16);
        horn(
            mesh,
            heel,
            heel + v(-s * 0.28, -0.04, 0.21),
            heel + v(-s * 0.42, -0.29, 0.34),
            0.20,
            c.horn,
        )?;
    }
    Ok(())
}

pub(super) fn tail(mesh: &mut Builder, c: Coat, index: u8) -> Result<(), String> {
    let a = tail_node(index);
    let b = tail_node(index + 1);
    let t = f32::from(index) / f32::from(TAIL_BONES);
    let r = (1.0 - t) * 1.23 + 0.21;
    let axis = (b - a).normalize();
    let path = [
        sec(a - axis * (0.26 * r), r, r * 0.84),
        sec(a.lerp(b, 0.44), r * 0.92, r * 0.81),
        sec(
            b + axis * 0.12,
            (r - 0.195).max(0.07),
            (r * 0.84 - 0.16).max(0.07),
        ),
    ];
    armoured(mesh, &path, 8, c)?;
    for (j, f) in [0.19, 0.72].into_iter().enumerate() {
        let base = a.lerp(b, f) + Vec3::Y * r * 0.75;
        let h = 0.37 + (1. - t) * 0.63;
        horn(
            mesh,
            base,
            base + v(0., h * 0.65, 0.17),
            base + v(0., h, 0.55),
            0.14 + r * 0.16,
            if c.kind == Kind::WaterDragon && j == 0 {
                c.growth
            } else {
                c.horn
            },
        )?;
    }
    if index + 1 == TAIL_BONES {
        horn(
            mesh,
            b,
            b + axis * 0.46,
            b + axis * 1.26 + v(0., 0.21, 0.),
            0.28,
            c.ridge,
        )?;
        if c.kind == Kind::WaterDragon {
            for s in [-1., 1.] {
                horn(
                    mesh,
                    b - axis * 0.89,
                    b + v(s * 0.60, 0.30, 0.60),
                    b + v(s * 1.11, 0.68, 1.36),
                    0.33,
                    c.growth,
                )?;
                horn(
                    mesh,
                    b - axis * 0.46,
                    b + v(s * 0.34, 0.21, 0.76),
                    b + v(s * 0.61, 0.47, 1.77),
                    0.23,
                    c.horn,
                )?;
            }
        }
    }
    Ok(())
}
