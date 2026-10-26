use super::*;

pub(super) fn build(role: u8) -> Sculpt {
    let mut b = Sculpt::default();
    b.block([0.0, 0.0, 0.0], [0.475, 0.425, 0.40], SKIN);
    b.block([0.0, -0.025, -0.05], [0.425, 0.375, 0.35], LIGHT);
    b.block([0.0, -0.1875, -0.04], [0.375, 0.05, 0.325], LIGHT);
    b.block([0.0, -0.055, -0.235], [0.05, 0.05, 0.05], tint(SKIN, 1.04));
    let hair = match role {
        3 => [0.73, 0.29, 0.065],
        7 | 9 => [0.50, 0.19, 0.075],
        8 => [0.28, 0.19, 0.10],
        _ => HAIR,
    };
    for side in [-1.0, 1.0] {
        b.block([side * 0.255, -0.04, -0.025], [0.075, 0.125, 0.10], SKIN);
        b.block([side * 0.265, -0.025, -0.075], [0.05, 0.075, 0.05], LIGHT);
        b.block(
            [side * 0.125, 0.0, -0.225],
            [0.075, 0.10, 0.025],
            [0.99, 0.96, 0.87],
        );
        b.block(
            [side * 0.125, -0.0125, -0.25],
            [0.05, 0.075, 0.025],
            [0.14, 0.095, 0.055],
        );
        b.block([side * 0.10, 0.025, -0.26], [0.025; 3], [0.99, 0.96, 0.90]);
        b.block([side * 0.12, 0.09, -0.2375], [0.10, 0.025, 0.025], hair);
        b.block(
            [side * 0.175, -0.10, -0.2125],
            [0.075, 0.025, 0.025],
            [0.94, 0.66, 0.49],
        );
    }
    b.block(
        [0.0, -0.1375, -0.225],
        [0.075, 0.025, 0.025],
        [0.68, 0.34, 0.23],
    );
    b.block([0.0375, -0.125, -0.23], [0.025; 3], [0.68, 0.34, 0.23]);

    // Authored tufts overlap in the sculpture, not in the final mesh. Stepped
    // strands silhouette the hair from every direction, including the back.
    b.block([0.0, 0.18, 0.025], [0.50, 0.125, 0.425], tint(hair, 0.80));
    for x in -2_i32..=2 {
        for z in -2_i32..=2 {
            let h = (x * 17 + z * 11 + i32::from(role) * 3).unsigned_abs();
            b.block(
                [
                    x as f32 * 0.095,
                    0.23 + (h % 3) as f32 * 0.025,
                    z as f32 * 0.0825,
                ],
                [0.125, 0.10 + (h % 2) as f32 * 0.05, 0.125],
                tint(hair, 0.92 + (h % 5) as f32 * 0.07),
            );
        }
        let length = 0.075 + ((x + 4) % 3) as f32 * 0.05;
        b.block(
            [x as f32 * 0.09, 0.15 - length * 0.25, -0.23],
            [0.10, length, 0.075],
            tint(hair, 0.94 + ((x + 5) % 3) as f32 * 0.09),
        );
    }
    for side in [-1.0, 1.0] {
        for i in 0..4 {
            b.block(
                [
                    side * 0.24,
                    0.08 - (i % 2) as f32 * 0.05,
                    -0.13 + i as f32 * 0.10,
                ],
                [0.075, 0.20, 0.125],
                tint(hair, 0.87 + (i % 3) as f32 * 0.10),
            );
        }
        if matches!(role, 3 | 7) {
            for i in 0..4 {
                b.block(
                    [
                        side * (0.23 + (i % 2) as f32 * 0.025),
                        -0.03 - i as f32 * 0.065,
                        0.05,
                    ],
                    [0.075, 0.10, 0.20],
                    tint(hair, 0.96 + (i % 2) as f32 * 0.12),
                );
            }
        }
    }
    for i in -2_i32..=2 {
        b.block(
            [i as f32 * 0.10, 0.015, 0.21],
            [0.125, 0.325 - (i.abs() % 2) as f32 * 0.05, 0.075],
            tint(hair, 0.90 + (i.abs() % 3) as f32 * 0.06),
        );
    }
    if matches!(role, 3 | 7) {
        b.block([0.0, 0.29, 0.11], [0.25, 0.10, 0.25], GREEN);
        for i in 0..5 {
            b.block(
                [
                    -0.10 + i as f32 * 0.05,
                    0.365 + (i % 2) as f32 * 0.025,
                    0.125,
                ],
                [0.075, 0.125, 0.20],
                tint(hair, 0.94 + (i % 3) as f32 * 0.1),
            );
        }
    }
    if matches!(role, 4 | 8 | 9) {
        for x in -2_i32..=2 {
            b.block(
                [x as f32 * 0.075, -0.18 + x.abs() as f32 * 0.025, -0.225],
                [0.10, 0.10, 0.075],
                tint(hair, 0.88 + (x.abs() % 3) as f32 * 0.12),
            );
        }
        for side in [-1.0, 1.0] {
            b.block(
                [side * 0.065, -0.09, -0.25],
                [0.10, 0.05, 0.05],
                tint(hair, 1.12),
            );
        }
    }
    match role {
        1 => hat(&mut b),
        2 | 4 => helmet(&mut b, role),
        5 => hood(&mut b),
        7 => {
            for i in 0..10 {
                let a = i as f32 * std::f32::consts::TAU / 10.0;
                let at = [a.cos() * 0.27, 0.245, a.sin() * 0.225];
                b.block(at, [0.10, 0.05, 0.10], GREEN);
                gear::flower(
                    &mut b,
                    Vec3::from_array(at) + Vec3::Y * 0.0375,
                    if i % 3 == 0 {
                        [0.87, 0.41, 0.46]
                    } else {
                        CREAM
                    },
                );
            }
        }
        _ => {}
    }
    b
}

fn hat(b: &mut Sculpt) {
    let straw = [0.85, 0.66, 0.28];
    for z in -3_i32..=3 {
        let width = if z.abs() == 3 { 0.55 } else { 0.75 };
        b.block(
            [0.0, 0.215, z as f32 * 0.10],
            [width, 0.05, 0.10],
            tint(straw, 0.95 + (z.abs() % 2) as f32 * 0.10),
        );
        for x in -3_i32..=3 {
            if z.abs() < 3 || x.abs() < 3 {
                b.block(
                    [
                        x as f32 * 0.10,
                        0.24 + ((x + z).abs() % 2) as f32 * 0.025,
                        z as f32 * 0.10,
                    ],
                    [0.075, 0.05, 0.075],
                    tint(straw, 0.95 + (x.abs() % 3) as f32 * 0.08),
                );
            }
        }
    }
    b.block([0.0, 0.315, 0.015], [0.45, 0.15, 0.425], straw);
    for i in -2_i32..=2 {
        b.block(
            [i as f32 * 0.08, 0.40, 0.015],
            [0.075, 0.05, 0.35],
            tint(straw, 0.97 + (i.abs() % 3) as f32 * 0.06),
        );
    }
    b.block([0.0, 0.30, 0.015], [0.475, 0.05, 0.45], GREEN);
}

fn helmet(b: &mut Sculpt, role: u8) {
    for row in 0..3 {
        let width = 0.55 - row as f32 * 0.10;
        let depth = 0.475 - row as f32 * 0.075;
        b.block(
            [0.0, 0.16 + row as f32 * 0.075, 0.025],
            [width, 0.10, depth],
            METAL,
        );
        for x in -2_i32..=2 {
            b.block(
                [
                    x as f32 * width / 5.0,
                    0.18 + row as f32 * 0.075,
                    0.025 - depth * 0.5,
                ],
                [width / 5.0, 0.075, 0.05],
                tint(EDGE, 0.87 + (x.abs() % 3) as f32 * 0.05),
            );
        }
    }
    for side in [-1.0, 1.0] {
        for z in 0..4 {
            b.block(
                [
                    side * 0.2625,
                    -0.005 - (z % 2) as f32 * 0.025,
                    -0.125 + z as f32 * 0.10,
                ],
                [0.075, 0.225, 0.10],
                tint(METAL, 1.0 + (z % 3) as f32 * 0.12),
            );
        }
        b.block([side * 0.25, 0.03, -0.23], [0.05, 0.075, 0.05], GOLD);
    }
    b.block([0.0, 0.10, -0.24], [0.55, 0.05, 0.075], EDGE);
    b.block([0.0, 0.14, -0.28], [0.075, 0.15, 0.05], GOLD);
    for z in 0..7 {
        let y = 0.35 + (3 - (z - 2_i32).abs()).max(0) as f32 * 0.025;
        b.block(
            [0.0, y, -0.17 + z as f32 * 0.065],
            [0.10, 0.175, 0.075],
            tint(cloth(role), 0.90 + (z % 3) as f32 * 0.10),
        );
        b.block(
            [0.025, y + 0.08, -0.17 + z as f32 * 0.065],
            [0.075, 0.05, 0.075],
            tint(cloth(role), 1.18),
        );
    }
}

fn hood(b: &mut Sculpt) {
    for row in 0..4 {
        let width = 0.56 - row as f32 * 0.08;
        b.block(
            [0.0, 0.12 + row as f32 * 0.065, 0.025],
            [width, 0.075, 0.475 - row as f32 * 0.05],
            tint(CREAM, 0.94 + (row % 3) as f32 * 0.025),
        );
        b.block(
            [0.0, 0.12 + row as f32 * 0.065, -0.2375 + row as f32 * 0.025],
            [width, 0.025, 0.025],
            GOLD,
        );
    }
    for side in [-1.0, 1.0] {
        for row in 0..4 {
            b.block(
                [
                    side * (0.25 + row as f32 * 0.0125),
                    0.04 - row as f32 * 0.075,
                    0.025,
                ],
                [0.075, 0.10, 0.45],
                tint(CREAM, 0.91 + (row % 3) as f32 * 0.04),
            );
            b.block(
                [
                    side * (0.25 + row as f32 * 0.0125),
                    0.04 - row as f32 * 0.075,
                    -0.225,
                ],
                [0.075, 0.10, 0.025],
                GOLD,
            );
        }
    }
}
