//! Rounded stepped faces, readable eyes, curled hair and profession-specific headwear.
use super::*;

fn hair_color(role: u8) -> [f32; 3] {
    match role {
        1 | 4 | 5 | 13 | 18 | 20 => [0.70, 0.65, 0.55],
        3 | 9 | 17 | 22 => [0.43, 0.19, 0.075],
        8 | 14 | 19 | 21 => [0.25, 0.16, 0.09],
        _ => HAIR,
    }
}
fn tuft(b: &mut Sculpt, at: [f32; 3], size: [f32; 3], color: [f32; 3], seed: u32) {
    b.ellipsoid(
        at,
        size.map(|extent| extent * 0.58),
        tint(color, 0.92 + (seed % 5) as f32 * 0.035),
    );
    b.ellipsoid(
        [
            at[0] - size[0] * 0.10,
            at[1] + size[1] * 0.25,
            at[2] - size[2] * 0.17,
        ],
        [size[0] * 0.38, size[1] * 0.30, size[2] * 0.38],
        tint(color, 1.06 + (seed % 3) as f32 * 0.035),
    );
}
fn bun(b: &mut Sculpt, at: [f32; 3], color: [f32; 3]) {
    for x in -2_i32..=2 {
        for y in -1_i32..=2 {
            for z in -2_i32..=2 {
                if x * x + y * y + z * z > 6 {
                    continue;
                }
                tuft(
                    b,
                    [
                        at[0] + x as f32 * 0.045,
                        at[1] + y as f32 * 0.045,
                        at[2] + z as f32 * 0.045,
                    ],
                    [0.065; 3],
                    color,
                    (x * 17 + y * 31 + z * 73).unsigned_abs(),
                );
            }
        }
    }
}
fn braid(b: &mut Sculpt, side: f32, color: [f32; 3], length: u32) {
    for i in 0..length {
        let y = -0.04 - i as f32 * 0.055;
        for sign in [-1.0, 1.0] {
            tuft(
                b,
                [
                    side * (0.23 + (i as f32 * 0.7).sin() * 0.025) + sign * 0.014,
                    y + sign * 0.013,
                    -0.01,
                ],
                [0.065, 0.075, 0.075],
                color,
                i * 13 + u32::from(sign > 0.0),
            );
        }
    }
    b.block(
        [side * 0.24, -0.04 - length as f32 * 0.055, -0.01],
        [0.08, 0.035, 0.08],
        GOLD,
    );
}
fn beard(b: &mut Sculpt, role: u8, color: [f32; 3]) {
    let long = matches!(role, 5 | 8 | 9 | 18);
    // Individual overlapping curls follow the jaw; the lower rows taper to a
    // rounded point instead of making one horizontal beard slab.
    for row in 0_i32..if long { 5 } else { 2 } {
        let count = if long { 3 - (row / 2) } else { 3 - row };
        for column in -count..=count {
            let x = column as f32 * 0.055;
            let y = -0.12 - row as f32 * 0.041 - (3 - column.abs()) as f32 * 0.011;
            let z = -0.205 + column.abs() as f32 * 0.012 - (row % 2) as f32 * 0.008;
            tuft(
                b,
                [x, y, z],
                [0.078, 0.085, 0.075],
                color,
                (column * 7 + row * 19).unsigned_abs(),
            );
        }
    }
    for side in [-1.0, 1.0] {
        for curl in 0..3 {
            tuft(
                b,
                [
                    side * (0.037 + curl as f32 * 0.039),
                    -0.083 - curl as f32 * 0.009,
                    -0.252 + curl as f32 * 0.010,
                ],
                [0.073, 0.047, 0.061],
                tint(color, 1.10),
                curl,
            );
        }
    }
}
fn glasses(b: &mut Sculpt) {
    let metal = [0.30, 0.22, 0.13];
    for side in [-1.0, 1.0] {
        for i in 0..12 {
            let a = i as f32 * std::f32::consts::TAU / 12.0;
            b.block(
                [
                    side * 0.115 + a.cos() * 0.069,
                    0.005 + a.sin() * 0.064,
                    -0.283,
                ],
                [0.025, 0.025, 0.025],
                metal,
            );
        }
        b.block([side * 0.215, 0.02, -0.22], [0.13, 0.025, 0.025], metal);
    }
    b.block([0.0, 0.02, -0.285], [0.10, 0.025, 0.025], metal);
}

pub(super) fn build(role: u8, female: bool) -> Sculpt {
    let mut b = Sculpt::detailed_head();
    let hair = hair_color(role);
    // Rounded cheeks and a tapered chin keep the face human and softly carved.
    b.ellipsoid([0.0, 0.0, 0.01], [0.235, 0.225, 0.205], SKIN);
    b.ellipsoid([0.0, -0.070, -0.025], [0.215, 0.145, 0.180], LIGHT);
    b.ellipsoid([0.0, -0.160, -0.015], [0.160, 0.070, 0.150], LIGHT);
    // A short bridge and compact tip replace the protruding cubic villager nose.
    b.ellipsoid([0.0, -0.036, -0.211], [0.027, 0.038, 0.029], LIGHT);
    b.block(
        [0.0, -0.063, -0.234],
        [0.047, 0.023, 0.022],
        tint(SKIN, 0.93),
    );
    let smiling = matches!(role, 1 | 20 | 22);
    for side in [-1.0, 1.0] {
        b.ellipsoid([side * 0.235, -0.04, -0.01], [0.057, 0.075, 0.052], SKIN);
        b.ellipsoid([side * 0.255, -0.03, -0.055], [0.025, 0.040, 0.027], LIGHT);
        if matches!(role, 1 | 20) {
            b.rotated(
                [side * 0.11, 0.01, -0.207],
                [0.075, 0.025, 0.025],
                tint(HAIR, 0.6),
                Quat::from_rotation_z(side * 0.18),
            );
        } else {
            b.rounded_box(
                [side * 0.113, 0.0, -0.20],
                [0.085, 0.10, 0.025],
                0.012,
                [0.99, 0.96, 0.85],
            );
            b.block(
                [side * 0.109, -0.005, -0.214],
                [0.055, 0.08, 0.025],
                [0.31, 0.19, 0.10],
            );
            b.block(
                [side * 0.105, 0.0, -0.228],
                [0.03, 0.06, 0.025],
                [0.13, 0.09, 0.05],
            );
            b.block(
                [side * 0.11 - 0.015, 0.025, -0.244],
                [0.025; 3],
                [1.0, 0.98, 0.90],
            );
            b.block(
                [side * 0.11 + 0.013, -0.025, -0.244],
                [0.022; 3],
                [0.74, 0.56, 0.27],
            );
        }
        b.rotated(
            [side * 0.11, 0.085, -0.205],
            [0.10, if female { 0.025 } else { 0.04 }, 0.035],
            tint(hair, 0.76),
            Quat::from_rotation_z(if smiling { side * 0.16 } else { -side * 0.07 }),
        );
        b.ellipsoid(
            [side * 0.155, -0.082, -0.179],
            [0.039, 0.022, 0.018],
            [0.94, 0.59, 0.40],
        );
        if matches!(role, 1 | 4 | 5 | 13 | 18 | 20) {
            b.block(
                [side * 0.177, -0.025, -0.205],
                [0.05, 0.0125, 0.025],
                tint(SKIN, 0.84),
            );
        }
    }
    b.block(
        [0.0, -0.142, -0.195],
        [if smiling { 0.10 } else { 0.075 }, 0.024, 0.025],
        [0.50, 0.26, 0.15],
    );
    for side in [-1.0, 1.0] {
        b.block([side * 0.05, -0.13, -0.195], [0.025; 3], [0.61, 0.31, 0.18]);
    }
    if role == 22 {
        b.block([0.0, -0.13, -0.21], [0.06, 0.025, 0.025], CREAM);
    }
    // A closed ellipsoidal hair shell with sculpted tufts visible from the rear too.
    for x in -4_i32..=4 {
        for y in -2_i32..=4 {
            for z in -3_i32..=3 {
                let p = Vec3::new(x as f32 * 0.052, y as f32 * 0.055, z as f32 * 0.055);
                let ellipsoid = (p / Vec3::new(0.26, 0.23, 0.23)).length_squared();
                if !(0.45..1.2).contains(&ellipsoid)
                    || (p.z < -0.105 && p.y < 0.08)
                    || (p.x.abs() > 0.21 && p.y < -0.03 && p.z < 0.06)
                {
                    continue;
                }
                tuft(
                    &mut b,
                    [p.x, p.y + 0.06, p.z + 0.04],
                    [0.075, 0.075, 0.08],
                    hair,
                    (x * 31 + y * 17 + z * 73).unsigned_abs(),
                );
            }
        }
    }
    for x in -3_i32..=3 {
        let drop = ((x * 7 + i32::from(role)) % 3).unsigned_abs() as f32 * 0.025;
        tuft(
            &mut b,
            [x as f32 * 0.06, 0.17 - drop, -0.185],
            [0.075, 0.085, 0.075],
            hair,
            (x + 4) as u32,
        );
    }
    if !female && matches!(role, 0 | 4 | 5 | 8 | 9 | 13 | 14 | 15 | 16 | 18 | 19) {
        beard(&mut b, role, hair);
    }
    if female && matches!(role, 0 | 3 | 7 | 9 | 15 | 17) {
        bun(&mut b, [-0.045, 0.31, 0.12], hair);
    }
    if female && matches!(role, 1 | 20 | 21) {
        for side in [-1.0, 1.0] {
            braid(&mut b, side, hair, if role == 21 { 7 } else { 5 });
        }
    }
    if role == 17 || role == 13 {
        glasses(&mut b);
    }
    match role {
        1 => straw(&mut b),
        2 => helmet(&mut b),
        5 => mitre(&mut b),
        8 => goggles(&mut b),
        13 | 23 => engineer_cap(&mut b),
        14 => miner_helmet(&mut b),
        16 => headband(&mut b),
        18 => traveller_hat(&mut b),
        19 | 22 => hood(&mut b, role),
        20 => {
            for i in 0..16 {
                let a = i as f32 * std::f32::consts::TAU / 16.0;
                b.block(
                    [a.cos() * 0.26, 0.20, a.sin() * 0.22],
                    [0.09, 0.075, 0.09],
                    CREAM,
                );
            }
        }
        21 => feathers(&mut b),
        _ => {}
    }
    swamp::head(&mut b, role);
    b
}
fn straw(b: &mut Sculpt) {
    let straw = [0.85, 0.65, 0.29];
    b.oval([0.0, 0.225, 0.015], [0.76, 0.045, 0.69], tint(straw, 0.88));
    b.ellipsoid([0.0, 0.25, 0.015], [0.38, 0.055, 0.345], straw);
    b.ellipsoid([0.0, 0.31, 0.02], [0.24, 0.14, 0.22], tint(straw, 1.07));
    b.oval([0.0, 0.30, 0.02], [0.48, 0.045, 0.44], tint(LEATHER, 1.5));
    for ring in 0..3 {
        let radius = 0.27 + ring as f32 * 0.045;
        for knot in 0..24 {
            let angle = knot as f32 * std::f32::consts::TAU / 24.0;
            tuft(
                b,
                [angle.cos() * radius, 0.275, angle.sin() * radius * 0.89],
                [0.042, 0.033, 0.042],
                tint(straw, 1.13),
                knot,
            );
        }
    }
    for i in 0..5 {
        b.rotated(
            [0.25, 0.36 + i as f32 * 0.028, -0.04],
            [0.04, 0.15, 0.04],
            GREEN,
            Quat::from_rotation_z(-0.5 + i as f32 * 0.18),
        );
    }
}
fn helmet(b: &mut Sculpt) {
    for row in 0..4 {
        let width = 0.55 - row as f32 * 0.09;
        b.block(
            [0.0, 0.15 + row as f32 * 0.065, 0.025],
            [width, 0.09, 0.48 - row as f32 * 0.065],
            METAL,
        );
    }
    for side in [-1.0, 1.0] {
        b.block([side * 0.245, -0.045, 0.02], [0.075, 0.25, 0.36], METAL);
        b.block([side * 0.245, -0.045, -0.17], [0.075, 0.25, 0.025], EDGE);
    }
    b.block([0.0, 0.12, -0.22], [0.55, 0.05, 0.075], EDGE);
    b.block([0.0, 0.14, -0.26], [0.075, 0.14, 0.05], GOLD);
    for z in 0..7 {
        tuft(
            b,
            [
                0.0,
                0.40 + (3 - (z - 2_i32).abs()).max(0) as f32 * 0.025,
                -0.16 + z as f32 * 0.065,
            ],
            [0.10, 0.16, 0.09],
            RED,
            z as u32,
        );
    }
}
fn mitre(b: &mut Sculpt) {
    for row in 0..7 {
        let width = 0.49 - row as f32 * 0.045;
        b.block(
            [0.0, 0.19 + row as f32 * 0.064, 0.015],
            [width, 0.08, 0.37],
            PURPLE,
        );
        for side in [-1.0, 1.0] {
            b.block(
                [
                    side * (width * 0.5 - 0.015),
                    0.19 + row as f32 * 0.064,
                    -0.19,
                ],
                [0.03, 0.08, 0.03],
                GOLD,
            );
        }
    }
    for side in [-1.0, 1.0] {
        b.rotated(
            [side * 0.12, 0.40, -0.19],
            [0.025, 0.46, 0.025],
            GOLD,
            Quat::from_rotation_z(side * 0.32),
        );
    }
    cross(b, [0.0, 0.43, -0.2125], 0.22, GOLD);
    b.oval([0.0, 0.17, -0.02], [0.54, 0.06, 0.45], GOLD);
}
fn goggles(b: &mut Sculpt) {
    b.block([0.0, 0.16, 0.02], [0.52, 0.05, 0.43], LEATHER);
    for side in [-1.0, 1.0] {
        b.block([side * 0.12, 0.20, -0.21], [0.16, 0.13, 0.075], GOLD);
        b.block(
            [side * 0.12, 0.20, -0.26],
            [0.11, 0.08, 0.025],
            [0.20, 0.23, 0.25],
        );
        b.block(
            [side * 0.12 - 0.025, 0.225, -0.28],
            [0.04, 0.025, 0.025],
            EDGE,
        );
    }
}
fn engineer_cap(b: &mut Sculpt) {
    let navy = cloth(13);
    b.oval([0.0, 0.19, 0.01], [0.53, 0.075, 0.45], navy);
    b.ellipsoid([-0.025, 0.25, 0.035], [0.29, 0.13, 0.24], tint(navy, 1.14));
    b.oval([0.0, 0.19, -0.015], [0.54, 0.025, 0.47], GOLD);
    b.ellipsoid([0.0, 0.155, -0.22], [0.255, 0.0375, 0.17], tint(navy, 0.70));
    cross(b, [0.0, 0.28, -0.205], 0.12, GOLD);
}
fn miner_helmet(b: &mut Sculpt) {
    let brown = [0.38, 0.29, 0.20];
    b.ellipsoid([0.0, 0.17, 0.02], [0.285, 0.22, 0.245], brown);
    b.oval([0.0, 0.16, -0.015], [0.62, 0.045, 0.54], tint(brown, 1.3));
    for side in [-1.0, 1.0] {
        for row in 0..6 {
            let angle = row as f32 * std::f32::consts::PI / 10.0;
            b.block(
                [side * 0.14, 0.17 + angle.sin() * 0.18, angle.cos() * 0.21],
                [0.035, 0.045, 0.055],
                METAL,
            );
            b.block(
                [side * 0.14, 0.17 + angle.sin() * 0.18, -angle.cos() * 0.21],
                [0.035, 0.045, 0.055],
                METAL,
            );
        }
    }
    b.block([0.0, 0.27, -0.215], [0.19, 0.18, 0.075], GOLD);
    b.block([0.0, 0.27, -0.26], [0.13, 0.13, 0.025], [1.0, 0.88, 0.53]);
    b.block([0.0, 0.27, -0.28], [0.09, 0.09, 0.025], [1.0, 0.99, 0.83]);
}
fn headband(b: &mut Sculpt) {
    b.oval([0.0, 0.15, 0.02], [0.53, 0.075, 0.45], CREAM);
    b.block([0.0, 0.15, -0.22], [0.49, 0.025, 0.025], RED);
    b.block([0.25, 0.16, 0.17], [0.10, 0.10, 0.07], CREAM);
    b.rotated(
        [0.27, 0.06, 0.17],
        [0.075, 0.20, 0.05],
        CREAM,
        Quat::from_rotation_z(0.25),
    );
}
fn traveller_hat(b: &mut Sculpt) {
    let brown = [0.39, 0.23, 0.14];
    b.oval([0.0, 0.185, 0.02], [0.78, 0.04, 0.69], tint(brown, 0.85));
    b.ellipsoid([0.0, 0.21, 0.02], [0.39, 0.055, 0.345], brown);
    b.ellipsoid(
        [-0.025, 0.31, 0.035],
        [0.235, 0.21, 0.205],
        tint(brown, 1.10),
    );
    b.oval([-0.025, 0.265, 0.035], [0.47, 0.06, 0.41], PURPLE);
    b.rotated(
        [0.17, 0.44, -0.025],
        [0.08, 0.40, 0.04],
        RED,
        Quat::from_rotation_z(-0.40),
    );
    for i in 0..5 {
        b.block([-0.19 + i as f32 * 0.09, 0.25, -0.22], [0.035; 3], GOLD);
    }
}
fn hood(b: &mut Sculpt, role: u8) {
    let color = if role == 22 {
        PURPLE
    } else {
        [0.55, 0.40, 0.26]
    };
    for row in 0..5 {
        b.block(
            [0.0, 0.13 + row as f32 * 0.06, 0.03],
            [0.59 - row as f32 * 0.08, 0.08, 0.50 - row as f32 * 0.055],
            color,
        );
    }
    for side in [-1.0, 1.0] {
        for row in 0..5 {
            b.block(
                [
                    side * (0.255 + row as f32 * 0.01),
                    0.09 - row as f32 * 0.065,
                    0.025,
                ],
                [0.085, 0.09, 0.45],
                color,
            );
        }
        for row in 0..6 {
            b.block(
                [
                    side * (0.20 + 0.065 * (1.0 - row as f32 / 5.0)),
                    0.24 - row as f32 * 0.065,
                    -0.235,
                ],
                [0.075, 0.075, 0.075],
                if role == 22 { GOLD } else { CREAM },
            );
        }
    }
    if role == 22 {
        for i in -3_i32..=3 {
            b.block(
                [i as f32 * 0.07, 0.32 - (i.abs() as f32 * 0.025), -0.20],
                [0.05, 0.075, 0.035],
                GOLD,
            );
        }
    }
}
fn feathers(b: &mut Sculpt) {
    for i in 0..18 {
        let a = i as f32 * std::f32::consts::TAU / 18.0;
        b.block(
            [a.cos() * 0.255, 0.20, a.sin() * 0.22],
            [0.075, 0.05, 0.075],
            if i % 3 == 0 {
                [0.24, 0.58, 0.53]
            } else {
                LEATHER
            },
        );
    }
    for i in 0..4 {
        b.rotated(
            [0.16 + i as f32 * 0.028, 0.38 + i as f32 * 0.033, 0.08],
            [0.055, 0.32, 0.035],
            if i % 2 == 0 { CREAM } else { GOLD },
            Quat::from_rotation_z(-0.18 - i as f32 * 0.20),
        );
    }
    for side in [-1.0, 1.0] {
        b.block(
            [side * 0.27, -0.10, -0.04],
            [0.045, 0.09, 0.045],
            [0.27, 0.66, 0.62],
        );
    }
}
