//! Surface-meshed reference citizens: garments, faces and tools are real voxel geometry.
use super::Part;
use bevy::prelude::*;
#[path = "villager/gear.rs"]
mod gear;
#[path = "villager/head.rs"]
mod head;
#[path = "villager/reference_gear.rs"]
mod reference_gear;
#[path = "villager/sculpt.rs"]
mod sculpt;
use sculpt::Sculpt;

const SKIN: [f32; 3] = [0.99, 0.70, 0.48];
const LIGHT: [f32; 3] = [1.0, 0.77, 0.55];
const CREAM: [f32; 3] = [0.92, 0.86, 0.73];
const GREEN: [f32; 3] = [0.31, 0.40, 0.17];
const LEATHER: [f32; 3] = [0.39, 0.24, 0.12];
const HAIR: [f32; 3] = [0.34, 0.17, 0.075];
const METAL: [f32; 3] = [0.52, 0.54, 0.56];
const EDGE: [f32; 3] = [0.78, 0.79, 0.77];
const GOLD: [f32; 3] = [0.92, 0.65, 0.22];
const PURPLE: [f32; 3] = [0.40, 0.20, 0.39];
const RED: [f32; 3] = [0.62, 0.18, 0.11];
fn tint(color: [f32; 3], amount: f32) -> [f32; 3] {
    color.map(|c| (c * amount).min(1.0))
}
fn cloth(role: u8) -> [f32; 3] {
    match role {
        2 | 13 => [0.19, 0.19, 0.29],
        4 => [0.20, 0.29, 0.48],
        5 | 18 | 22 => PURPLE,
        9 | 20 => RED,
        23 => [0.44, 0.31, 0.19],
        7 | 17 => [0.31, 0.27, 0.34],
        14 => [0.30, 0.28, 0.33],
        0 | 15 | 19 => [0.50, 0.36, 0.23],
        _ => GREEN,
    }
}
pub(super) fn female(role: u8) -> bool {
    matches!(role, 1 | 3 | 7 | 17 | 20 | 21 | 22)
}
fn armored(role: u8) -> bool {
    matches!(role, 2 | 4)
}

pub(super) fn parts(role: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let role = role % terra_core::creatures::villagers::ROLE_COUNT;
    parts_for_sex(role, female(role))
}

pub(super) fn parts_for_sex(role: u8, female: bool) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body(role, female).finish()?),
        (
            Part::Head,
            Vec3::new(0.0, 1.435, -0.015),
            head::build(role, female)
                .finish()?
                .scaled_by(Vec3::new(1.08, 1.04, 1.04)),
        ),
    ];
    for side in [-1.0, 1.0] {
        let hip = Vec3::new(side * 0.125, 0.6025, 0.0);
        let shoulder = Vec3::new(side * 0.30, 1.15, 0.0);
        let (thigh, boot) = leg(role);
        let (sleeve, hand) = arm(role, side);
        parts.extend([
            (Part::VillagerThigh(side), hip, thigh.finish()?),
            (Part::Leg(side), hip, boot.finish()?),
            (Part::VillagerSleeve(side), shoulder, sleeve.finish()?),
            (Part::VillagerArm(side), shoulder, hand.finish()?),
        ]);
    }
    Ok(parts)
}
fn pouch(b: &mut Sculpt, x: f32, y: f32, z: f32, color: [f32; 3]) {
    b.block([x, y, z], [0.125, 0.15, 0.125], tint(color, 0.75));
    b.block([x, y + 0.04, z - 0.025], [0.15, 0.09, 0.15], color);
    b.block([x, y + 0.015, z - 0.11], [0.025, 0.05, 0.025], GOLD);
}
fn cross(b: &mut Sculpt, at: [f32; 3], size: f32, color: [f32; 3]) {
    b.block(at, [size * 0.26, size, 0.025], color);
    b.block(
        [at[0], at[1] + size * 0.15, at[2]],
        [size * 0.78, size * 0.24, 0.025],
        color,
    );
}
fn robe(b: &mut Sculpt, role: u8, color: [f32; 3], bottom: f32) {
    for row in 0..8 {
        let y = bottom + row as f32 * 0.08;
        let width = 0.58 - row as f32 * 0.017;
        for col in -4_i32..=4 {
            let x = col as f32 * width / 9.0;
            let outline = (1.0 - (x / (width * 0.58)).powi(2)).sqrt();
            let depth = 0.40 * outline + 0.012 * (col.abs() % 2) as f32;
            b.block(
                [x, y, 0.0],
                [width / 9.0 + 0.014, 0.10, depth],
                tint(color, 0.92 + (col.abs() % 3) as f32 * 0.055),
            );
        }
    }
    if matches!(role, 5 | 18 | 20 | 22) {
        for side in [-1.0, 1.0] {
            b.block(
                [side * 0.11, bottom + 0.28, -0.21],
                [0.09, 0.65, 0.05],
                if role == 5 { CREAM } else { color },
            );
            b.block(
                [side * 0.16, bottom + 0.28, -0.24],
                [0.025, 0.65, 0.025],
                GOLD,
            );
            for y in [bottom + 0.12, bottom + 0.36, bottom + 0.58] {
                cross(b, [side * 0.11, y, -0.248], 0.105, GOLD);
            }
        }
        b.block([0.0, bottom - 0.035, 0.0], [0.59, 0.05, 0.39], GOLD);
    }
}
fn cape(b: &mut Sculpt, role: u8, color: [f32; 3], length: f32) {
    for row in 0..10 {
        let t = row as f32 / 9.0;
        let width = 0.56 + 0.12 * t;
        for col in -4_i32..=4 {
            b.block(
                [
                    col as f32 * width / 9.0,
                    1.18 - t * length,
                    0.19 + 0.035 * t + (col.abs() % 2) as f32 * 0.012,
                ],
                [width / 9.0 + 0.012, 0.12, 0.065],
                tint(color, 0.86 + (col.abs() % 3) as f32 * 0.07),
            );
        }
    }
    if matches!(role, 3 | 4 | 19 | 20 | 21) {
        reference_gear::fur(b, [0.0, 1.16, 0.02], 0.50);
    }
}
fn body(role: u8, female: bool) -> Sculpt {
    let mut b = Sculpt::default();
    let fabric = cloth(role);
    let width = if female {
        0.40
    } else if role == 8 {
        0.48
    } else {
        0.44
    };
    let base = match role {
        0 | 15 => fabric,
        9 => RED,
        13 => fabric,
        14 => [0.38, 0.29, 0.20],
        8 => SKIN,
        _ => CREAM,
    };
    for (y, w, d) in [
        (0.78, width * 0.86, 0.29),
        (0.88, width * 0.93, 0.31),
        (0.99, width, 0.32),
        (1.10, width, 0.30),
    ] {
        b.rounded_box(
            [0.0, y, 0.0],
            [w, 0.175, d],
            0.065,
            tint(base, 1.0 + (y * 40.0) as i32 as f32 % 3.0 * 0.012),
        );
    }
    // Sloping shoulder joins overlap the arm pivots; the shirt has no square top ledge.
    for side in [-1.0, 1.0] {
        b.ellipsoid(
            [side * width * 0.33, 1.12, 0.005],
            [width * 0.31, 0.115, 0.145],
            base,
        );
    }
    // Pelvis and neck overlap the articulated limbs, including the exposed
    // gaps between an apron, tunic hem and the tops of the trousers.
    let pants = match role {
        13 => cloth(role),
        14 => [0.27, 0.27, 0.31],
        0 | 15 => cloth(role),
        _ => [0.35, 0.28, 0.20],
    };
    b.ellipsoid([0.0, 0.685, 0.0], [width * 0.49, 0.145, 0.15], pants);
    b.ellipsoid([0.0, 1.215, 0.0], [0.10, 0.095, 0.10], SKIN);
    if matches!(role, 1 | 5 | 7 | 17 | 20 | 22) {
        robe(&mut b, role, if role == 1 { GREEN } else { fabric }, 0.20);
    }
    if matches!(role, 3 | 4 | 18 | 19 | 20 | 21 | 22) {
        cape(&mut b, role, fabric, if role == 3 { 0.65 } else { 0.95 });
    }
    // Open waistcoats expose the shirt and define a shaped, layered chest.
    if matches!(role, 3 | 6 | 7 | 17 | 18 | 19 | 21) {
        for side in [-1.0, 1.0] {
            b.rounded_box([side * 0.15, 0.99, -0.02], [0.13, 0.40, 0.35], 0.05, fabric);
            b.rotated(
                [side * 0.11, 1.09, -0.192],
                [0.05, 0.20, 0.045],
                tint(fabric, 1.22),
                Quat::from_rotation_z(side * 0.23),
            );
        }
    }
    if matches!(role, 1 | 8 | 14 | 16 | 17 | 23) {
        let apron = if matches!(role, 1 | 16 | 17) {
            CREAM
        } else {
            LEATHER
        };
        b.rounded_box([0.0, 0.99, -0.18], [0.31, 0.31, 0.06], 0.03, apron);
        b.block(
            [0.0, 0.65, -0.115],
            [0.40, if role == 8 { 0.35 } else { 0.43 }, 0.21],
            apron,
        );
        for side in [-1.0, 1.0] {
            b.block(
                [side * 0.135, 1.12, -0.02],
                [0.055, 0.18, 0.36],
                tint(LEATHER, 1.2),
            );
            b.block([side * 0.135, 1.075, -0.217], [0.055, 0.05, 0.025], GOLD);
        }
        b.block([0.0, 0.96, -0.22], [0.15, 0.11, 0.035], tint(apron, 0.85));
        if role == 1 {
            reference_gear::flowers(&mut b, [0.0, 0.53, -0.23], GREEN);
        }
        if role == 16 {
            for (x, y) in [(-0.09, 0.58), (0.06, 0.69), (0.13, 0.45)] {
                cross(&mut b, [x, y, -0.23], 0.09, RED);
            }
        }
    }
    if role == 9 {
        for y in 0..5 {
            for x in -3_i32..=3 {
                for z in [-0.17, 0.17] {
                    b.block(
                        [x as f32 * 0.06, 0.82 + y as f32 * 0.075, z],
                        [0.055, 0.072, 0.025],
                        tint(RED, if (x + y) % 2 == 0 { 0.50 } else { 0.91 }),
                    );
                }
            }
        }
        b.block([-0.13, 1.02, -0.04], [0.05, 0.43, 0.37], LEATHER);
        b.block([0.14, 1.02, -0.04], [0.05, 0.43, 0.37], LEATHER);
    }
    if armored(role) {
        for row in 0..5 {
            for col in -2_i32..=2 {
                b.block(
                    [
                        col as f32 * 0.08,
                        0.85 + row as f32 * 0.066,
                        -0.18 - (2 - col.abs()) as f32 * 0.012,
                    ],
                    [0.085, 0.075, 0.075],
                    tint(METAL, 0.97 + ((row + col.abs()) % 3) as f32 * 0.1),
                );
            }
        }
        for side in [-1.0, 1.0] {
            for i in 0..3 {
                b.block(
                    [
                        side * (0.13 + i as f32 * 0.036),
                        0.72 - i as f32 * 0.046,
                        -0.02,
                    ],
                    [0.15, 0.075, 0.34],
                    tint(METAL, 1.15 - i as f32 * 0.07),
                );
            }
        }
        if role == 4 {
            reference_gear::fur(&mut b, [0.0, 1.17, 0.0], 0.52);
        }
    }
    if role == 13 {
        reference_gear::uniform(&mut b);
    }
    if matches!(role, 0 | 15) {
        for (x, y) in [(-0.14, 1.02), (0.10, 0.88), (-0.05, 0.67)] {
            b.block([x, y, -0.185], [0.09, 0.11, 0.025], tint(LEATHER, 0.67));
            for d in [-0.04, 0.04] {
                b.block([x + d, y, -0.203], [0.025, 0.10, 0.025], CREAM);
            }
        }
        for i in -3_i32..=3 {
            b.block(
                [i as f32 * 0.064, 0.63 + (i.abs() % 3) as f32 * 0.037, -0.01],
                [0.065, 0.20, 0.32],
                tint(fabric, 0.90),
            );
        }
    }
    if role == 5 {
        b.block([0.0, 1.02, -0.18], [0.39, 0.33, 0.07], PURPLE);
        cross(&mut b, [0.0, 1.01, -0.226], 0.17, GOLD);
    }
    if matches!(role, 14 | 19) {
        reference_gear::scarf(&mut b, if role == 14 { PURPLE } else { CREAM });
    }
    if role != 15 && role != 0 {
        b.oval(
            [0.0, 0.785, 0.0],
            [width + 0.03, 0.075, 0.36],
            tint(LEATHER, 0.78),
        );
        b.block([0.0, 0.785, -0.20], [0.10, 0.09, 0.05], GOLD);
        b.block([0.0, 0.785, -0.23], [0.05, 0.04, 0.025], LEATHER);
        pouch(&mut b, -0.24, 0.72, -0.06, LEATHER);
    }
    reference_gear::body(&mut b, role);
    b
}
fn leg(role: u8) -> (Sculpt, Sculpt) {
    let mut thigh = Sculpt::default();
    let pants = match role {
        13 => cloth(role),
        14 => [0.27, 0.27, 0.31],
        0 | 15 => cloth(role),
        _ => [0.35, 0.28, 0.20],
    };
    thigh.rounded_box([0.0, -0.15, 0.0], [0.22, 0.33, 0.25], 0.055, pants);
    let mut b = Sculpt::default();
    b.block([0.0, -0.31, 0.0], [0.24, 0.075, 0.27], tint(pants, 1.22));
    let bare = matches!(role, 0 | 15);
    b.block(
        [0.0, -0.42, 0.0],
        [0.19, 0.21, 0.23],
        if bare { SKIN } else { LEATHER },
    );
    b.rounded_box(
        [0.0, -0.525, -0.045],
        [0.24, 0.10, 0.34],
        0.04,
        if bare { LIGHT } else { tint(LEATHER, 1.07) },
    );
    b.block(
        [0.0, -0.575, -0.04],
        [0.275, 0.05, 0.35],
        tint(LEATHER, 0.60),
    );
    for y in [-0.46, -0.37] {
        b.block([0.0, y, -0.01], [0.22, 0.04, 0.28], tint(LEATHER, 0.75));
    }
    if !bare {
        for y in [-0.50, -0.46, -0.39] {
            b.block([0.0, y, -0.145], [0.075, 0.025, 0.025], tint(LEATHER, 1.5));
        }
    }
    if armored(role) {
        b.block([0.0, -0.33, -0.16], [0.23, 0.11, 0.075], EDGE);
        b.block([0.0, -0.43, -0.13], [0.17, 0.16, 0.075], METAL);
        b.block([0.0, -0.52, -0.13], [0.24, 0.08, 0.19], EDGE);
    }
    (thigh, b)
}
fn arm(role: u8, side: f32) -> (Sculpt, Sculpt) {
    let mut b = Sculpt::default();
    let sleeve = match role {
        8 => SKIN,
        9 => RED,
        13 => cloth(role),
        0 | 15 => cloth(role),
        5 | 18 | 20 | 22 => cloth(role),
        _ => CREAM,
    };
    b.ellipsoid([side * 0.005, -0.045, 0.0], [0.125, 0.135, 0.14], sleeve);
    b.ellipsoid([0.0, -0.14, 0.0], [0.105, 0.125, 0.12], sleeve);

    b.oval(
        [0.0, -0.165, -0.005],
        [0.23, 0.055, 0.275],
        tint(sleeve, 1.10),
    );
    if armored(role) {
        for i in 0..3 {
            b.block(
                [
                    side * (0.025 + i as f32 * 0.024),
                    0.015 - i as f32 * 0.045,
                    0.0,
                ],
                [0.21, 0.075, 0.28],
                tint(METAL, 1.06 + i as f32 * 0.04),
            );
        }
    }
    let mut forearm = Sculpt::default();
    forearm.ellipsoid([0.0, -0.24, 0.0], [0.095, 0.135, 0.11], SKIN);
    if matches!(role, 2 | 3 | 4 | 6 | 8 | 9 | 14 | 19 | 21) {
        forearm.block(
            [0.0, -0.30, -0.015],
            [0.20, 0.13, 0.25],
            if armored(role) { METAL } else { LEATHER },
        );
        for y in [-0.34, -0.265] {
            forearm.block(
                [0.0, y, -0.015],
                [0.225, 0.025, 0.275],
                if armored(role) {
                    EDGE
                } else {
                    tint(LEATHER, 0.65)
                },
            );
        }
    }
    forearm.ellipsoid([0.0, -0.395, -0.02], [0.095, 0.085, 0.105], LIGHT);
    for i in 0..4 {
        forearm.block(
            [
                -0.0625 + i as f32 * 0.04,
                -0.465 + (i % 3) as f32 * 0.0125,
                -0.035,
            ],
            [0.025, 0.075, 0.15],
            tint(LIGHT, 0.92 + (i % 2) as f32 * 0.05),
        );
    }
    forearm.block([-side * 0.10, -0.39, -0.075], [0.05, 0.10, 0.10], SKIN);
    let carrying = matches!(role, 1 | 16 | 17);
    let bend = if carrying {
        1.22
    } else if role == 13 && side < 0.0 {
        0.45
    } else {
        0.12
    };
    let inward = if carrying { -side * 0.35 } else { 0.0 };
    let sleeve = b;
    let mut b = Sculpt::default();
    b.articulated(
        forearm,
        Vec3::new(0.0, -0.17, 0.0),
        Quat::from_rotation_x(bend) * Quat::from_rotation_z(inward),
    );
    if side < 0.0 && role == 2 {
        gear::shield(&mut b);
    }
    if side > 0.0 && role == 8 {
        // The smith's compact forging hammer is a profession prop, not an
        // oversized generic pickaxe. Its handle crosses the closed palm.
        b.block([0.0, -0.43, -0.025], [0.045, 0.29, 0.045], LEATHER);
        for y in [-0.37, -0.42, -0.47] {
            b.block([0.0, y, -0.025], [0.055, 0.025, 0.055], tint(LEATHER, 0.72));
        }
        b.block([0.0, -0.56, -0.025], [0.22, 0.10, 0.115], METAL);
        for side in [-1.0, 1.0] {
            b.block([side * 0.1125, -0.56, -0.025], [0.025, 0.115, 0.13], EDGE);
        }
        b.block([0.0, -0.50, -0.025], [0.07, 0.035, 0.07], GOLD);
    }
    if side > 0.0 && role == terra_core::creatures::villagers::CRAFTSMAN {
        // A compact builder's mallet; the ordinary voxel placement remains authoritative.
        b.block([0.0, -0.42, -0.025], [0.045, 0.23, 0.045], LEATHER);
        b.block([0.0, -0.49, -0.025], [0.20, 0.10, 0.11], [0.52, 0.36, 0.19]);
    }
    reference_gear::hand(&mut b, role, side);
    (sleeve, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn all_roles_have_distinct_bounded_shared_indexed_models_and_grounded_boots() {
        let mut roles = std::collections::BTreeSet::new();
        for role in 0..terra_core::creatures::villagers::ROLE_COUNT {
            if (10..=12).contains(&role) {
                continue;
            }
            let parts = parts(role).unwrap();
            assert_eq!(parts.len(), 10, "four two-joint limbs, head and torso");
            let vertices = parts
                .iter()
                .map(|(_, _, mesh)| mesh.count_vertices())
                .sum::<usize>();
            assert!(vertices < 50000, "role {role}: {vertices} vertices");
            let mut fingerprint = 14695981039346656037_u64;
            for (part, offset, mesh) in parts {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing villager positions");
                };
                assert!(!positions.is_empty());
                assert!(positions.iter().flatten().all(|value| value.is_finite()));
                if matches!(part, Part::VillagerArm(side) if side > 0.0) {
                    let (minimum_y, maximum_y, maximum_x, maximum_z) = match role {
                        1 => (-0.55, 0.15, 0.56, 0.50),
                        5 => (-1.10, 1.0, 0.26, 0.20),
                        8 => (-0.64, 0.15, 0.20, 0.20),
                        16 | 17 => (-0.55, 0.15, 0.26, 0.40),
                        18 => (-0.72, 0.15, 0.20, 0.25),
                        _ => (-0.55, 0.15, 0.20, 0.20),
                    };
                    assert!(
                        positions.iter().all(|p| {
                            (minimum_y..=maximum_y).contains(&p[1])
                                && p[0].abs() <= maximum_x
                                && p[2].abs() <= maximum_z
                        }),
                        "right arm and authored equipment stay within the role silhouette: role {role}"
                    );
                }
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
