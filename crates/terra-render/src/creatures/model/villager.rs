//! Ten shared, surface-meshed citizen rigs. Details never spawn ECS entities.
use super::{Builder, Part};
use bevy::prelude::*;
mod gear;
mod head;
mod sculpt;
use sculpt::Sculpt;

const SKIN: [f32; 3] = [0.90, 0.61, 0.42];
const LIGHT: [f32; 3] = [0.99, 0.76, 0.56];
const CREAM: [f32; 3] = [0.91, 0.86, 0.72];
const GREEN: [f32; 3] = [0.33, 0.44, 0.20];
const LEATHER: [f32; 3] = [0.40, 0.25, 0.12];
const HAIR: [f32; 3] = [0.27, 0.13, 0.065];
const METAL: [f32; 3] = [0.49, 0.53, 0.57];
const EDGE: [f32; 3] = [0.77, 0.79, 0.79];
const GOLD: [f32; 3] = [0.88, 0.62, 0.22];

fn tint(color: [f32; 3], amount: f32) -> [f32; 3] {
    color.map(|c| (c * amount).min(1.0))
}

fn cloth(role: u8) -> [f32; 3] {
    match role {
        2 | 9 => [0.63, 0.16, 0.10],
        4 | 5 => [0.17, 0.34, 0.60],
        6 | 8 => [0.47, 0.30, 0.15],
        _ => GREEN,
    }
}

pub(super) fn parts(role: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let role = role % terra_core::creatures::villagers::ROLE_COUNT;
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body(role).finish()?),
        (
            Part::Head,
            Vec3::new(0.0, 1.435, -0.015),
            head::build(role).finish()?,
        ),
    ];
    for side in [-1.0, 1.0] {
        parts.push((
            Part::Leg(side),
            Vec3::new(side * 0.125, 0.6025, 0.0),
            leg(role).finish()?,
        ));
        parts.push((
            Part::VillagerArm(side),
            Vec3::new(side * 0.30, 1.15, 0.0),
            arm(role, side).finish()?,
        ));
    }
    Ok(parts)
}

fn pouch(b: &mut Sculpt, x: f32, y: f32, z: f32, color: [f32; 3]) {
    b.block([x, y, z], [0.125, 0.15, 0.125], tint(color, 0.75));
    b.block([x, y + 0.025, z - 0.025], [0.15, 0.10, 0.125], color);
    b.block(
        [x, y + 0.06, z - 0.03],
        [0.15, 0.05, 0.15],
        tint(color, 1.18),
    );
    b.block([x, y + 0.01, z - 0.10], [0.025, 0.05, 0.025], GOLD);
}

fn body(role: u8) -> Sculpt {
    let mut b = Sculpt::default();
    let armored = matches!(role, 2 | 4);
    let robe = matches!(role, 5 | 7);
    let fabric = cloth(role);
    b.block(
        [0.0, 0.95, 0.0],
        [0.425, 0.45, 0.30],
        if armored { LEATHER } else { CREAM },
    );
    b.block([0.0, 1.21, 0.0], [0.175, 0.10, 0.175], SKIN);
    for i in 0..7 {
        let x = (i as f32 - 3.0) * 0.06;
        b.block(
            [x, 0.725 + (i % 2) as f32 * 0.0125, -0.005],
            [0.065, 0.15, 0.325],
            tint(CREAM, 0.93 + (i % 3) as f32 * 0.035),
        );
    }
    for y in [0.88, 0.99, 1.10] {
        b.block([0.0, y, -0.1625], [0.025, 0.025, 0.025], tint(CREAM, 0.72));
        b.block(
            [-0.0375, y + 0.025, -0.163],
            [0.05, 0.05, 0.025],
            tint(CREAM, 1.04),
        );
    }
    if matches!(role, 0 | 3 | 7) {
        for side in [-1.0, 1.0] {
            b.block([side * 0.15, 0.98, -0.025], [0.15, 0.425, 0.35], fabric);
            b.block(
                [side * 0.0875, 1.06, -0.20],
                [0.025, 0.30, 0.05],
                tint(fabric, 1.23),
            );
            for y in [0.88, 0.99, 1.10] {
                b.block([side * 0.12, y, -0.205], [0.025, 0.025, 0.025], GOLD);
            }
        }
    }
    if matches!(role, 1 | 6 | 8) {
        b.block([0.0, 0.96, -0.175], [0.30, 0.325, 0.05], fabric);
        for side in [-1.0, 1.0] {
            b.block(
                [side * 0.1375, 1.10, -0.06],
                [0.075, 0.225, 0.325],
                tint(LEATHER, 1.13),
            );
            b.block([side * 0.1375, 1.05, -0.24], [0.05, 0.05, 0.025], GOLD);
            b.block([side * 0.1375, 1.05, -0.255], [0.025, 0.025, 0.025], METAL);
        }
        b.block([0.0, 0.985, -0.225], [0.15, 0.10, 0.05], tint(fabric, 1.14));
        for i in -3_i32..=3 {
            b.block(
                [i as f32 * 0.06, 0.62 + (i.abs() % 2) as f32 * 0.015, -0.045],
                [0.065, if role == 6 { 0.40 } else { 0.18 }, 0.325],
                tint(fabric, 0.94 + (i.abs() % 3) as f32 * 0.07),
            );
        }
    }
    if robe {
        for row in 0..5 {
            let width = 0.575 - row as f32 * 0.03;
            let y = 0.225 + row as f32 * 0.10;
            b.block([0.0, y, 0.0], [width, 0.125, 0.35], CREAM);
            for i in -3_i32..=3 {
                b.block(
                    [i as f32 * width / 8.0, y, -0.185],
                    [0.05, 0.125, 0.05],
                    tint(CREAM, if i % 2 == 0 { 1.03 } else { 0.91 }),
                );
            }
            for side in [-1.0, 1.0] {
                b.block(
                    [
                        side * if role == 5 { 0.10 } else { width * 0.4 },
                        y + 0.015,
                        -0.22,
                    ],
                    [if role == 5 { 0.075 } else { 0.10 }, 0.125, 0.05],
                    fabric,
                );
                if role == 5 {
                    b.block([side * 0.15, y, -0.2375], [0.025, 0.125, 0.025], GOLD);
                }
            }
        }
        if role == 5 {
            b.block([0.0, 0.175, -0.01], [0.60, 0.05, 0.375], GOLD);
            b.block([0.0, 1.01, -0.20], [0.30, 0.35, 0.05], fabric);
            for side in [-1.0, 1.0] {
                b.rotated(
                    [side * 0.07, 1.135, -0.235],
                    [0.05, 0.20, 0.05],
                    GOLD,
                    Quat::from_rotation_z(side * 0.7),
                );
            }
            gear::gem(&mut b, [0.0, 1.025, -0.25], 0.075);
        }
    }
    if armored {
        for row in 0..4 {
            for col in -2_i32..=2 {
                b.block(
                    [
                        col as f32 * 0.075,
                        0.88 + row as f32 * 0.075,
                        -0.17 - (2 - col.abs()) as f32 * 0.0125,
                    ],
                    [0.075; 3],
                    tint(METAL, 0.95 + ((row + col.abs()) % 3) as f32 * 0.13),
                );
            }
        }
        for side in [-1.0, 1.0] {
            for i in 0..3 {
                b.block(
                    [
                        side * (0.125 + i as f32 * 0.035),
                        0.69 - i as f32 * 0.05,
                        -0.06,
                    ],
                    [0.15, 0.075, 0.325],
                    tint(METAL, 1.13 - i as f32 * 0.06),
                );
            }
        }
        for i in -3_i32..=3 {
            b.block(
                [
                    i as f32 * 0.06,
                    0.86 + (i.abs() % 2) as f32 * 0.025,
                    0.20 + (i.abs() % 2) as f32 * 0.025,
                ],
                [0.075, 0.725, 0.05],
                tint(fabric, 0.87 + (i.abs() % 3) as f32 * 0.07),
            );
        }
    }
    if role == 9 {
        b.block([0.0, 0.98, 0.0], [0.45, 0.425, 0.325], fabric);
        for y in 0..5 {
            for x in -3_i32..=3 {
                for z in [-0.175, 0.175] {
                    b.block(
                        [x as f32 * 0.065, 0.83 + y as f32 * 0.075, z],
                        [0.06, 0.075, 0.025],
                        tint(fabric, if (x + y) % 2 == 0 { 0.46 } else { 0.85 }),
                    );
                }
            }
        }
    }
    if matches!(role, 2..=5 | 7) {
        for i in 0..8 {
            let angle = i as f32 * std::f32::consts::TAU / 8.0;
            b.block(
                [
                    angle.cos() * 0.18,
                    1.17 + (i % 2) as f32 * 0.025,
                    angle.sin() * 0.16,
                ],
                [0.16, 0.10, 0.125],
                tint(fabric, 0.95 + (i % 3) as f32 * 0.06),
            );
        }
    }
    b.block([0.0, 0.785, 0.0], [0.475, 0.075, 0.375], tint(LEATHER, 0.8));
    b.block([0.0, 0.79, -0.2125], [0.10, 0.10, 0.05], GOLD);
    b.block([0.0, 0.79, -0.24], [0.05, 0.05, 0.025], LEATHER);
    for side in [-1.0, 1.0] {
        pouch(&mut b, side * 0.2375, 0.76, -0.06, LEATHER);
        b.block([side * 0.15, 0.785, -0.20], [0.025, 0.075, 0.025], GOLD);
    }
    gear::backpack(&mut b, role);
    b
}

fn leg(role: u8) -> Sculpt {
    let mut b = Sculpt::default();
    let trousers = if matches!(role, 2 | 4 | 6 | 8) {
        [0.26, 0.24, 0.20]
    } else {
        tint(GREEN, 0.8)
    };
    b.block([0.0, -0.18, 0.0], [0.225, 0.375, 0.25], trousers);
    b.block(
        [0.0, -0.30, -0.01],
        [0.25, 0.075, 0.275],
        tint(trousers, 1.2),
    );
    b.block([0.0, -0.425, 0.0], [0.20, 0.225, 0.25], LEATHER);
    b.block(
        [0.0, -0.525, -0.045],
        [0.25, 0.10, 0.35],
        tint(LEATHER, 1.06),
    );
    b.block(
        [0.0, -0.575, -0.04],
        [0.275, 0.05, 0.35],
        tint(LEATHER, 0.60),
    );
    b.block(
        [0.0, -0.51, -0.175],
        [0.225, 0.075, 0.075],
        tint(LEATHER, 1.25),
    );
    for y in [-0.45, -0.36] {
        b.block([0.0, y, -0.01], [0.225, 0.05, 0.30], tint(LEATHER, 0.78));
        b.block(
            [0.085, y, -0.17],
            [0.05, 0.05, 0.025],
            if matches!(role, 2 | 4) { EDGE } else { GOLD },
        );
    }
    for y in [-0.50, -0.475, -0.40] {
        b.block([0.0, y, -0.1375], [0.075, 0.025, 0.025], tint(LEATHER, 1.4));
    }
    if matches!(role, 2 | 4) {
        b.block([0.0, -0.32, -0.16], [0.225, 0.10, 0.075], EDGE);
        b.block([0.0, -0.41, -0.135], [0.15, 0.10, 0.05], METAL);
    }
    b
}

fn arm(role: u8, side: f32) -> Sculpt {
    let mut b = Sculpt::default();
    let armored = matches!(role, 2 | 4);
    let sleeve = if role == 9 { cloth(role) } else { CREAM };
    b.block([0.0, -0.08, 0.0], [0.20, 0.225, 0.25], sleeve);
    for i in 0..3 {
        b.block(
            [
                side * (0.025 + i as f32 * 0.025),
                -0.025 - i as f32 * 0.05,
                0.0,
            ],
            [0.20, 0.075, 0.275],
            tint(
                if armored { METAL } else { sleeve },
                0.93 + (i % 2) as f32 * 0.15,
            ),
        );
    }
    b.block([0.0, -0.225, 0.0], [0.175, 0.15, 0.20], SKIN);
    b.block(
        [0.0, -0.165, -0.015],
        [0.225, 0.075, 0.275],
        tint(sleeve, 1.04),
    );
    if matches!(role, 2..=4 | 6 | 8 | 9) {
        b.block([0.0, -0.30, -0.015], [0.20, 0.125, 0.25], LEATHER);
        for y in [-0.34, -0.265] {
            b.block(
                [0.0, y, -0.015],
                [0.225, 0.025, 0.275],
                if armored { EDGE } else { tint(LEATHER, 0.65) },
            );
        }
        b.block(
            [0.0, -0.30, -0.15],
            [0.125, 0.075, 0.025],
            if armored { METAL } else { tint(LEATHER, 1.25) },
        );
    }
    b.block([0.0, -0.395, -0.02], [0.175, 0.125, 0.20], LIGHT);
    for i in 0..4 {
        b.block(
            [
                -0.0625 + i as f32 * 0.04,
                -0.465 + (i % 3) as f32 * 0.0125,
                -0.035,
            ],
            [0.025, 0.075, 0.15],
            tint(LIGHT, 0.92 + (i % 2) as f32 * 0.05),
        );
    }
    b.block([-side * 0.10, -0.39, -0.075], [0.05, 0.10, 0.10], SKIN);
    gear::hand(&mut b, role, side);
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn ten_roles_have_distinct_bounded_shared_indexed_models_and_grounded_boots() {
        let mut roles = std::collections::BTreeSet::new();
        for role in 0..terra_core::creatures::villagers::ROLE_COUNT {
            let parts = parts(role).unwrap();
            assert_eq!(parts.len(), 6, "six shared rigid parts per resident");
            let vertices = parts
                .iter()
                .map(|(_, _, mesh)| mesh.count_vertices())
                .sum::<usize>();
            assert!(vertices < 16000, "role {role}: {vertices} vertices");
            let mut fingerprint = 14695981039346656037_u64;
            for (part, offset, mesh) in parts {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing villager positions");
                };
                assert!(!positions.is_empty());
                assert!(positions.iter().flatten().all(|value| value.is_finite()));
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|index| index < positions.len())
                );
                assert_eq!(mesh.indices().unwrap().len() % 3, 0);
                for value in positions.iter().flatten() {
                    fingerprint =
                        fingerprint.wrapping_mul(1099511628211) ^ u64::from(value.to_bits());
                }
                let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("Missing villager palette");
                };
                for value in colors.iter().flatten() {
                    fingerprint =
                        fingerprint.wrapping_mul(1099511628211) ^ u64::from(value.to_bits());
                }
                if matches!(part, Part::Leg(_)) {
                    let bottom = positions
                        .iter()
                        .map(|p| p[1] + offset.y)
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (bottom - 0.0025).abs() < 0.0001,
                        "role {role}: feet {bottom}"
                    );
                }
            }
            assert!(roles.insert(fingerprint), "role {role} duplicated");
        }
    }
}
