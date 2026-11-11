//! Profession silhouettes and layered equipment from the NPC reference sheet.
use super::*;

pub(super) fn fur(b: &mut Sculpt, at: [f32; 3], width: f32) {
    for i in 0..20 {
        let a = i as f32 * std::f32::consts::TAU / 20.0;
        let p = [
            at[0] + a.cos() * width * 0.5,
            at[1] + (i % 3) as f32 * 0.0125,
            at[2] + a.sin() * 0.18,
        ];
        b.ellipsoid(
            p,
            [0.065, 0.065, 0.065],
            tint(CREAM, 0.91 + (i % 3) as f32 * 0.045),
        );
        b.block(
            [p[0], p[1] + 0.04, p[2]],
            [0.06, 0.045, 0.07],
            tint(CREAM, 1.03),
        );
    }
}
pub(super) fn flowers(b: &mut Sculpt, at: [f32; 3], leaf: [f32; 3]) {
    for i in 0..5 {
        let x = at[0] + (i - 2) as f32 * 0.06;
        let y = at[1] + (i % 2) as f32 * 0.045;
        b.block([x, y, at[2]], [0.025, 0.13, 0.025], leaf);
        for side in [-1.0, 1.0] {
            b.rotated(
                [x + side * 0.025, y, at[2]],
                [0.065, 0.025, 0.035],
                leaf,
                Quat::from_rotation_z(side * 0.7),
            );
        }
        gear::flower(
            b,
            Vec3::new(x, y + 0.07, at[2]),
            if i % 2 == 0 {
                CREAM
            } else {
                [0.69, 0.42, 0.71]
            },
        );
    }
}
pub(super) fn scarf(b: &mut Sculpt, color: [f32; 3]) {
    for y in [1.12, 1.17, 1.22] {
        for side in [-1.0, 1.0] {
            b.block(
                [side * 0.13, y, -0.07],
                [0.18, 0.055, 0.32],
                tint(color, if y < 1.16 { 0.85 } else { 1.0 }),
            );
        }
    }
    b.rotated(
        [-0.12, 0.97, -0.21],
        [0.11, 0.30, 0.07],
        color,
        Quat::from_rotation_z(-0.15),
    );
}
pub(super) fn uniform(b: &mut Sculpt) {
    let navy = cloth(13);
    for side in [-1.0, 1.0] {
        b.rotated(
            [side * 0.10, 1.10, -0.185],
            [0.12, 0.24, 0.035],
            tint(navy, 1.3),
            Quat::from_rotation_z(-side * 0.25),
        );
        for y in [0.89, 1.0, 1.11] {
            b.block([side * 0.11, y, -0.215], [0.025; 3], GOLD);
        }
    }
    b.block([0.0, 1.13, -0.21], [0.04, 0.12, 0.035], RED);
    for i in 0..10 {
        let t = i as f32 / 9.0;
        b.block(
            [
                -0.14 + t * 0.27,
                0.91 - (t * std::f32::consts::PI).sin() * 0.10,
                -0.22,
            ],
            [0.025; 3],
            GOLD,
        );
    }
    for (x, y) in [(0.14, 1.06), (0.19, 0.99), (-0.17, 1.10)] {
        b.block([x, y, -0.224], [0.045, 0.06, 0.025], GOLD);
    }
}
fn basket(b: &mut Sculpt) {
    b.rounded_box(
        [0.10, 0.91, -0.33],
        [0.42, 0.24, 0.24],
        0.055,
        tint(LEATHER, 1.30),
    );
    for row in 0..5 {
        for col in 0..8 {
            b.block(
                [-0.10 + col as f32 * 0.057, 0.81 + row as f32 * 0.045, -0.46],
                [0.05, 0.035, 0.035],
                tint(GOLD, 0.79 + ((row + col) % 2) as f32 * 0.16),
            );
        }
    }
    b.oval([0.10, 1.045, -0.33], [0.45, 0.04, 0.27], GOLD);
    for i in 0..5 {
        let x = -0.07 + i as f32 * 0.08;
        b.block(
            [x, 1.10 + (i % 2) as f32 * 0.04, -0.32],
            [0.08, 0.15, 0.10],
            GREEN,
        );
        b.rotated(
            [x, 1.16, -0.33],
            [0.13, 0.04, 0.07],
            tint(GREEN, 1.35),
            Quat::from_rotation_z(i as f32 * 0.45),
        );
    }
    flowers(b, [0.12, 1.12, -0.35], GREEN);
}
fn trader_pack(b: &mut Sculpt, role: u8) {
    b.rounded_box([0.0, 0.95, 0.31], [0.43, 0.65, 0.24], 0.075, LEATHER);
    b.block([0.0, 1.31, 0.29], [0.44, 0.11, 0.31], tint(LEATHER, 1.26));
    for side in [-1.0, 1.0] {
        b.block(
            [side * 0.16, 0.97, 0.46],
            [0.055, 0.70, 0.05],
            tint(LEATHER, 0.70),
        );
        pouch(b, side * 0.24, 0.94, 0.24, LEATHER);
        b.block([side * 0.16, 1.26, 0.48], [0.065, 0.06, 0.025], GOLD);
    }
    // Bedroll layers and straps, not one floating brown cube.
    for row in 0..4 {
        b.block(
            [0.0, 1.44 + row as f32 * 0.025, 0.31],
            [0.42 - row as f32 * 0.04, 0.055, 0.24 - row as f32 * 0.025],
            tint(CREAM, 0.71 + row as f32 * 0.05),
        );
    }
    for side in [-1.0, 1.0] {
        b.block([side * 0.13, 1.45, 0.31], [0.045, 0.16, 0.28], LEATHER);
    }
    if role == 18 {
        b.block([0.03, 1.59, 0.25], [0.34, 0.20, 0.30], tint(LEATHER, 0.90));
        b.block([0.03, 1.70, 0.25], [0.36, 0.05, 0.32], tint(LEATHER, 1.25));
        b.block([0.03, 1.57, 0.08], [0.05, 0.09, 0.025], GOLD);
    }
}
pub(super) fn body(b: &mut Sculpt, role: u8) {
    match role {
        3 => gear::backpack(b, 3),
        6 => {
            gear::backpack(b, 3);
            gear::back_tools(b, 3);
        }

        8 => {
            for x in [-0.13, 0.04, 0.14] {
                b.block([x, 0.72, -0.23], [0.035, 0.21, 0.035], LEATHER);
                b.block([x, 0.84, -0.23], [0.10, 0.055, 0.07], METAL);
            }
        }
        9 => {
            let rotation = Quat::from_rotation_z(-0.50);
            b.rotated(
                [-0.16, 1.02, 0.27],
                [0.05, 0.92, 0.05],
                tint(LEATHER, 1.4),
                rotation,
            );
            b.rotated([-0.38, 1.42, 0.27], [0.29, 0.20, 0.10], METAL, rotation);
            b.rotated([-0.50, 1.42, 0.27], [0.055, 0.25, 0.11], EDGE, rotation);
        }
        14 => {
            b.rotated(
                [-0.21, 0.89, 0.26],
                [0.045, 0.80, 0.045],
                LEATHER,
                Quat::from_rotation_z(-0.4),
            );
            b.rotated(
                [-0.35, 1.25, 0.26],
                [0.43, 0.055, 0.075],
                METAL,
                Quat::from_rotation_z(-0.15),
            );
            for x in [-0.14, -0.02, 0.10] {
                b.block([x, 0.76, -0.23], [0.065, 0.12, 0.055], GOLD);
            }
        }
        16 => {
            b.block([0.22, 1.0, 0.27], [0.29, 0.42, 0.14], LEATHER);
            for i in 0..4 {
                let x = 0.12 + i as f32 * 0.065;
                b.block([x, 1.20, 0.29], [0.035, 0.24, 0.035], tint(LEATHER, 0.8));
                b.block(
                    [x, 0.99, 0.29],
                    [0.06, 0.24, 0.07],
                    tint(RED, 0.86 + i as f32 * 0.08),
                );
            }
            b.block([-0.26, 0.64, -0.06], [0.06, 0.25, 0.05], LEATHER);
            b.block([-0.32, 0.52, -0.06], [0.20, 0.18, 0.05], EDGE);
        }
        17 => pouch(b, 0.26, 0.73, 0.10, LEATHER),
        18..=22 => {
            trader_pack(b, role);
            if matches!(role, 20 | 22) {
                for y in [1.04, 0.88, 0.65] {
                    gear::gem(b, [0.0, y, -0.245], 0.095);
                }
                for side in [-1.0, 1.0] {
                    for i in 0..5 {
                        b.block(
                            [
                                side * (0.16 + i as f32 * 0.008),
                                1.11 - i as f32 * 0.06,
                                -0.21,
                            ],
                            [0.045; 3],
                            GOLD,
                        );
                    }
                }
            }
            if role == 21 {
                b.rotated(
                    [0.18, 0.93, -0.23],
                    [0.055, 0.65, 0.05],
                    LEATHER,
                    Quat::from_rotation_z(-0.65),
                );
                b.block([0.24, 0.62, -0.25], [0.055, 0.28, 0.05], METAL);
            }
        }
        _ => {}
    }
}

/// Role equipment shares the arm transform, so walking never leaves it suspended
/// at its old torso position. Coordinates here are authored in standing model space.
pub(super) fn hand(arm: &mut Sculpt, role: u8, side: f32) {
    if side < 0.0 {
        return;
    }
    let mut prop = Sculpt::default();
    let b = &mut prop;
    match role {
        1 => basket(b),
        5 => {
            // Crozier rises beside the bishop and remains attached to his robe-side grasp.
            b.block([0.40, 0.98, -0.05], [0.05, 1.80, 0.05], GOLD);
            for y in [0.15, 0.44, 0.97, 1.48, 1.73] {
                b.block([0.40, y, -0.05], [0.08, 0.06, 0.08], tint(GOLD, 1.15));
            }
            for i in 0..15 {
                let a = i as f32 * std::f32::consts::TAU / 17.0;
                b.block(
                    [0.40 + a.cos() * 0.12, 1.96 + a.sin() * 0.13, -0.05],
                    [0.055, 0.055, 0.06],
                    GOLD,
                );
            }
            b.block([0.40, 1.91, -0.05], [0.035, 0.10, 0.035], GOLD);
        }
        17 => {
            b.block([0.19, 0.91, -0.27], [0.25, 0.31, 0.07], LEATHER);
            b.block([0.19, 0.91, -0.315], [0.20, 0.27, 0.025], tint(CREAM, 0.91));
            b.block(
                [0.19, 0.91, -0.34],
                [0.22, 0.29, 0.025],
                tint(LEATHER, 0.85),
            );
        }
        18 => {
            let at = [0.34, 0.60, -0.11];
            b.block(at, [0.18, 0.23, 0.15], [0.95, 0.56, 0.12]);
            for side in [-1.0, 1.0] {
                b.block(
                    [at[0] + side * 0.085, at[1], at[2] - 0.075],
                    [0.025, 0.24, 0.025],
                    GOLD,
                );
            }
            for y in [at[1] - 0.13, at[1] + 0.13] {
                b.block([at[0], y, at[2]], [0.22, 0.04, 0.19], LEATHER);
            }
            b.block([at[0], at[1] + 0.20, at[2]], [0.10, 0.10, 0.025], GOLD);
        }
        _ => return,
    }
    let offset = if role == 1 {
        Vec3::new(-0.40, -1.15, 0.0)
    } else {
        Vec3::new(-0.30, -1.15, 0.0)
    };
    arm.append(prop, offset);
}
