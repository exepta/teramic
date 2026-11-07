use super::*;

pub(super) fn gem(b: &mut Sculpt, at: [f32; 3], size: f32) {
    b.block(at, [size, size, 0.05], GOLD);
    b.block(
        [at[0], at[1], at[2] - 0.025],
        [size * 0.65, size * 0.65, 0.05],
        [0.24, 0.66, 0.86],
    );
    b.block(
        [at[0] - 0.0125, at[1] + 0.0125, at[2] - 0.05],
        [0.025; 3],
        [0.64, 0.89, 0.98],
    );
}

pub(super) fn flower(b: &mut Sculpt, at: Vec3, color: [f32; 3]) {
    for p in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
        b.block((at + p * 0.0375).to_array(), [0.05, 0.025, 0.05], color);
    }
    b.block((at + Vec3::Y * 0.0125).to_array(), [0.025; 3], GOLD);
}

pub(super) fn backpack(b: &mut Sculpt, role: u8) {
    if matches!(role, 1 | 3 | 6 | 7) {
        b.rotated(
            [0.03, 0.99, -0.2375],
            [0.05, 0.55, 0.05],
            LEATHER,
            Quat::from_rotation_z(-0.78),
        );
        for i in 0..5 {
            b.block(
                [-0.12 + i as f32 * 0.07, 1.14 - i as f32 * 0.07, -0.275],
                [0.025; 3],
                GOLD,
            );
        }
    }
    if matches!(role, 3 | 6) {
        b.rotated(
            [-0.17, 1.02, 0.225],
            [0.175, 0.45, 0.175],
            LEATHER,
            Quat::from_rotation_z(-0.25),
        );
        for i in 0..5 {
            let x = -0.25 + (i % 3) as f32 * 0.075;
            let z = 0.225 + (i / 3) as f32 * 0.075;
            b.block(
                [x, 1.35 + (i % 2) as f32 * 0.035, z],
                [0.025, 0.40, 0.025],
                tint(LEATHER, 1.3),
            );
            b.block(
                [x, 1.52 + (i % 2) as f32 * 0.035, z],
                [0.075, 0.10, 0.025],
                CREAM,
            );
            b.block(
                [x, 1.52 + (i % 2) as f32 * 0.035, z],
                [0.025, 0.10, 0.075],
                tint(CREAM, 0.85),
            );
        }
        b.block(
            [-0.13, 1.20, 0.225],
            [0.225, 0.05, 0.225],
            tint(LEATHER, 1.3),
        );
    }
    if role == 1 {
        b.block([0.25, 0.86, 0.16], [0.25, 0.35, 0.25], tint(LEATHER, 0.8));
        for row in 0..5 {
            let y = 0.71 + row as f32 * 0.06;
            for i in 0..4 {
                b.block(
                    [0.15 + i as f32 * 0.065, y, 0.30],
                    [0.05, 0.05, 0.025],
                    tint(LEATHER, 1.1 + ((row + i) % 2) as f32 * 0.18),
                );
                b.block(
                    [0.39, y, 0.06 + i as f32 * 0.065],
                    [0.025, 0.05, 0.05],
                    tint(LEATHER, 1.15),
                );
            }
        }
        for i in 0..6 {
            let x = 0.16 + (i % 3) as f32 * 0.07;
            let z = 0.10 + (i / 3) as f32 * 0.07;
            let y = 1.18 + (i % 3) as f32 * 0.05;
            b.block([x, y - 0.14, z], [0.025, 0.45, 0.025], GOLD);
            for j in 0..4 {
                b.block(
                    [
                        x + if j % 2 == 0 { -0.025 } else { 0.025 },
                        y + j as f32 * 0.04,
                        z,
                    ],
                    [0.05, 0.05, 0.05],
                    tint(GOLD, 1.08 + (j % 2) as f32 * 0.08),
                );
            }
        }
    }
    if role == 7 {
        b.block([0.275, 0.71, -0.15], [0.30, 0.275, 0.20], LEATHER);
        b.block(
            [0.275, 0.825, -0.18],
            [0.325, 0.075, 0.225],
            tint(LEATHER, 1.25),
        );
        b.block([0.275, 0.71, -0.2625], [0.20, 0.20, 0.025], CREAM);
        b.block(
            [0.275, 0.71, -0.2875],
            [0.125, 0.05, 0.025],
            [0.67, 0.16, 0.12],
        );
        b.block(
            [0.275, 0.71, -0.2875],
            [0.05, 0.125, 0.025],
            [0.67, 0.16, 0.12],
        );
        b.block([0.38, 0.77, -0.275], [0.05, 0.075, 0.025], GOLD);
    }
    if matches!(role, 6 | 8 | 9) {
        for i in 0..3 {
            let x = -0.19 + i as f32 * 0.07;
            b.block(
                [x, 0.74, -0.23],
                [0.025, 0.25 + i as f32 * 0.025, 0.025],
                tint(LEATHER, 1.35),
            );
            b.block(
                [x, 0.86 + i as f32 * 0.015, -0.23],
                [0.05, 0.075, 0.05],
                EDGE,
            );
        }
    }
}

pub(super) fn back_tools(b: &mut Sculpt, role: u8) {
    // These profession props have no corresponding equippable player item.
    // Keep them stowed; the right hand uses the shared held-item renderer.
    let rot = Quat::from_rotation_z(if role == 3 { -0.35 } else { 0.18 });
    let origin = Vec3::new(0.10, 0.82, 0.34);
    let mut piece =
        |p: Vec3, size, color| b.rotated((origin + rot * p).to_array(), size, color, rot);
    if role == 3 {
        for i in 0..11 {
            let t = i as f32 / 10.0;
            piece(
                Vec3::new(
                    0.0,
                    -0.30 + t * 1.05,
                    (t * std::f32::consts::PI).sin() * 0.225,
                ),
                [0.05, 0.125, 0.075],
                tint(LEATHER, 1.0 + (i % 3) as f32 * 0.1),
            );
        }
        piece(Vec3::Y * 0.225, [0.025, 1.05, 0.025], CREAM);
        for y in [-0.25, 0.70, 0.225] {
            piece(Vec3::new(0.0, y, 0.0), [0.10, 0.075, 0.125], GOLD);
        }
    } else if role == 5 {
        piece(Vec3::Y * 0.13, [0.05, 1.10, 0.05], LEATHER);
        for y in [-0.3, 0.0, 0.48, 0.60] {
            piece(Vec3::Y * y, [0.075, 0.05, 0.075], GOLD);
        }
        for (x, y, w, h) in [
            (-0.125, 0.75, 0.05, 0.20),
            (0.125, 0.75, 0.05, 0.20),
            (0.0, 0.875, 0.20, 0.05),
            (0.0, 0.625, 0.20, 0.05),
        ] {
            piece(Vec3::new(x, y, 0.0), [w, h, 0.075], GOLD);
        }
    }
}

pub(super) fn shield(b: &mut Sculpt) {
    for row in 0..7 {
        let width = if row < 3 {
            0.15 + row as f32 * 0.10
        } else {
            0.45
        };
        let y = -0.65 + row as f32 * 0.10;
        b.block([-0.075, y, -0.18], [width, 0.10, 0.10], GOLD);
        b.block(
            [-0.075, y + 0.0125, -0.24],
            [width - 0.075, 0.075, 0.05],
            cloth(4),
        );
    }
    for (x, y, w, h) in [
        (-0.075, -0.24, 0.05, 0.35),
        (-0.075, -0.24, 0.225, 0.05),
        (-0.075, -0.425, 0.125, 0.05),
    ] {
        b.block([x, y, -0.275], [w, h, 0.025], GOLD);
    }
}
