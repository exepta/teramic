//! Authored straw puppet, shared by section batches, picking and inventory.
//! All fabric, tied straw, hat and lantern use one neutral vertex-color material.
use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;

type Batches = BTreeMap<usize, PropMesh>;
const STRAW: [f32; 3] = [0.80, 0.53, 0.17];
const GOLD: [f32; 3] = [0.96, 0.69, 0.26];
const SHADE: [f32; 3] = [0.53, 0.32, 0.10];
const CLOTH: [f32; 3] = [0.67, 0.60, 0.44];
const RED: [f32; 3] = [0.65, 0.12, 0.085];
const DARK: [f32; 3] = [0.24, 0.16, 0.09];

fn block(
    batches: &mut Batches,
    material: usize,
    center: [f32; 3],
    size: [f32; 3],
    rotation: Quat,
    color: [f32; 3],
) -> Result<(), String> {
    let start = batches
        .get(&material)
        .map_or(0, |mesh| mesh.positions.len());
    add_box(
        batches,
        material,
        Vec3::from_array(center),
        Vec3::from_array(size),
        rotation,
    )?;
    let mesh = batches
        .get_mut(&material)
        .expect("box creates a shared batch");
    mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
    let linear = color.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    mesh.colors[start..].fill([linear[0], linear[1], linear[2], 1.0]);
    Ok(())
}

pub(super) fn append(
    batches: &mut Batches,
    variant: u8,
    detail: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let material = palette.crop;
    let fine = detail == PropDetail::Near;
    let far = detail == PropDetail::Far;
    {
        let mut cube =
            |center, size, color| block(batches, material, center, size, Quat::IDENTITY, color);
        // A rooted timber stake remains visible beneath the stuffed, tied torso.
        cube([0.0, 0.50, 0.0], [0.105, 1.0, 0.105], DARK)?;
        cube(
            [-0.025, 0.35, 0.058],
            [0.024, 0.66, 0.018],
            [0.42, 0.28, 0.12],
        )?;
        cube([0.0, 1.03, 0.0], [0.43, 0.58, 0.31], CLOTH)?;
        cube([0.0, 1.07, 0.0], [0.46, 0.44, 0.34], CLOTH)?;
        cube([0.0, 1.28, 0.0], [0.48, 0.15, 0.33], SHADE)?;
        // Rigid crossbar and cloth sleeves; loose straw fingers are added below.
        cube([0.0, 1.24, 0.0], [1.37, 0.075, 0.08], DARK)?;
        for side in [-1.0, 1.0] {
            cube([side * 0.36, 1.25, 0.0], [0.32, 0.22, 0.25], CLOTH)?;
            cube([side * 0.56, 1.25, 0.0], [0.16, 0.16, 0.17], STRAW)?;
            for x in [0.48, 0.53] {
                cube([side * x, 1.25, 0.0], [0.026, 0.205, 0.211], DARK)?;
                cube([side * x, 1.25, 0.0], [0.012, 0.219, 0.223], CLOTH)?;
            }
        }
        // Square straw face with inset eyes and a small stitched mouth.
        cube([0.0, 1.56, 0.0], [0.33, 0.36, 0.30], STRAW)?;
        for x in [-0.083, 0.083] {
            cube([x, 1.59, 0.158], [0.042, 0.091, 0.016], [0.15, 0.105, 0.04])?;
        }
        cube([0.005, 1.48, 0.158], [0.12, 0.025, 0.016], SHADE)?;
        // Layered wide brim, short crown and dark hat band define the silhouette.
        for (y, width, depth) in [(1.73, 0.74, 0.62), (1.765, 0.64, 0.53), (1.80, 0.47, 0.41)] {
            cube([0.0, y, 0.0], [width, 0.040, depth], STRAW)?;
        }
        cube([0.0, 1.87, 0.0], [0.35, 0.18, 0.32], STRAW)?;
        cube([0.0, 1.97, 0.0], [0.30, 0.045, 0.27], GOLD)?;
        cube([0.0, 1.82, 0.0], [0.365, 0.067, 0.335], DARK)?;
        // A folded red scarf goes all around the neck and falls over the shirt.
        for (y, width, depth) in [(1.39, 0.43, 0.37), (1.355, 0.46, 0.40), (1.32, 0.39, 0.38)] {
            cube([0.0, y, 0.0], [width, 0.044, depth], RED)?;
        }
        for row in 0..4 {
            cube(
                [0.09, 1.29 - row as f32 * 0.061, 0.195],
                [0.115 - row as f32 * 0.016, 0.068, 0.043],
                if row % 2 == 0 {
                    RED
                } else {
                    [0.74, 0.17, 0.12]
                },
            )?;
        }
        // Rope belt is a ring of square strands rather than a colored flat stripe.
        for y in [0.79, 0.82] {
            cube([0.0, y, 0.0], [0.48, 0.024, 0.37], SHADE)?;
        }
        cube([0.14, 0.80, 0.207], [0.065, 0.067, 0.048], CLOTH)?;
    }

    // Angular straw bundles: several thick square pieces, never thin ribbons.
    let strands: usize = if fine {
        10
    } else if far {
        4
    } else {
        7
    };
    for side in [-1.0, 1.0] {
        for i in 0..strands {
            let row = i % 3;
            let z = (i / 3) as f32 * 0.049 - 0.06;
            let length = 0.22 + (i * 7 % 4) as f32 * 0.018;
            block(
                batches,
                material,
                [side * (0.61 + length * 0.32), 1.20 + row as f32 * 0.051, z],
                [length, 0.035, 0.039],
                Quat::from_rotation_z(side * (row as f32 - 1.0) * 0.16),
                if i.is_multiple_of(3) { GOLD } else { STRAW },
            )?;
        }
    }
    let skirt: usize = if fine {
        15
    } else if far {
        5
    } else {
        9
    };
    for i in 0..skirt {
        let angle = i as f32 * std::f32::consts::TAU / skirt as f32;
        let (sin, cos) = angle.sin_cos();
        let h = 0.18 + (i * 3 % 5) as f32 * 0.016;
        block(
            batches,
            material,
            [sin * 0.18, 0.73 - h * 0.5, cos * 0.13],
            [0.045, h, 0.042],
            Quat::from_rotation_y(angle) * Quat::from_rotation_x(0.24),
            if i.is_multiple_of(4) { GOLD } else { STRAW },
        )?;
    }

    if !far {
        // Raised hat straw, fabric patches and stitches stay bounded at near LOD.
        if fine {
            for side in [-1.0, 1.0] {
                for row in 0..7 {
                    for column in 0..6 {
                        let tone = match (row * 7 + column * 3) % 4 {
                            0 => [0.70, 0.63, 0.48],
                            1 => [0.64, 0.57, 0.41],
                            2 => [0.67, 0.60, 0.45],
                            _ => [0.66, 0.59, 0.43],
                        };
                        block(
                            batches,
                            material,
                            [
                                -0.1625 + column as f32 * 0.065,
                                0.90 + row as f32 * 0.058,
                                side * 0.178,
                            ],
                            [0.055, 0.049, 0.012],
                            Quat::IDENTITY,
                            tone,
                        )?;
                    }
                }
            }
        }
        let columns: usize = if fine { 7 } else { 4 };
        for side in [-1.0, 1.0] {
            for i in 0..columns {
                let x = -0.132 + i as f32 * 0.264 / (columns - 1) as f32;
                block(
                    batches,
                    material,
                    [x, 1.555, side * 0.157],
                    [0.022, 0.31, 0.018],
                    Quat::IDENTITY,
                    if i.is_multiple_of(3) { GOLD } else { STRAW },
                )?;
                // Keep eyes open rather than burying them under straw relief.
                if side > 0.0 && (x.abs() - 0.083).abs() < 0.022 {
                    block(
                        batches,
                        material,
                        [x, 1.59, 0.17],
                        [0.028, 0.083, 0.012],
                        Quat::IDENTITY,
                        [0.15, 0.105, 0.04],
                    )?;
                }
            }
            for i in 0..if fine { 11_usize } else { 6 } {
                let x = -0.33 + i as f32 * if fine { 0.066 } else { 0.132 };
                block(
                    batches,
                    material,
                    [x, 1.754 + (i % 3) as f32 * 0.005, side * 0.275],
                    [0.042, 0.021, 0.09],
                    Quat::from_rotation_y(x * 0.3),
                    if i.is_multiple_of(3) { GOLD } else { STRAW },
                )?;
            }
            block(
                batches,
                material,
                [-0.11, 1.02, side * 0.178],
                [0.115, 0.13, 0.022],
                Quat::from_rotation_z(0.12),
                [0.46, 0.36, 0.22],
            )?;
            for i in 0..4 {
                block(
                    batches,
                    material,
                    [-0.17 + i as f32 * 0.038, 1.083, side * 0.195],
                    [0.010, 0.035, 0.010],
                    Quat::IDENTITY,
                    GOLD,
                )?;
            }
            for row in 0..if fine { 7 } else { 3 } {
                let y = 0.88 + row as f32 * if fine { 0.058 } else { 0.145 };
                block(
                    batches,
                    material,
                    [-0.225, y, side * 0.183],
                    [0.014, 0.024, 0.012],
                    Quat::from_rotation_z(0.25),
                    [0.84, 0.77, 0.59],
                )?;
            }
        }
        for i in 0..if fine { 9_usize } else { 4 } {
            let x = -0.14 + i as f32 * if fine { 0.035 } else { 0.092 };
            block(
                batches,
                material,
                [x, 1.91, 0.168],
                [0.018, 0.13 + (i % 3) as f32 * 0.015, 0.018],
                Quat::IDENTITY,
                if i.is_multiple_of(3) { GOLD } else { STRAW },
            )?;
        }
        let flower_x = if variant.is_multiple_of(2) {
            -0.23
        } else {
            0.23
        };
        block(
            batches,
            material,
            [flower_x, 1.87, 0.19],
            [0.03, 0.16, 0.028],
            Quat::from_rotation_z(-0.25),
            [0.29, 0.45, 0.12],
        )?;
        for [x, y] in [[-0.043, 0.0], [0.043, 0.0], [0.0, 0.043], [0.0, -0.043]] {
            block(
                batches,
                material,
                [flower_x + x, 1.91 + y, 0.21],
                [0.047, 0.047, 0.035],
                Quat::IDENTITY,
                [0.94, 0.91, 0.78],
            )?;
        }
        block(
            batches,
            material,
            [flower_x, 1.91, 0.232],
            [0.036, 0.036, 0.03],
            Quat::IDENTITY,
            GOLD,
        )?;
    }
    lantern(batches, material, far)
}

fn lantern(batches: &mut Batches, material: usize, far: bool) -> Result<(), String> {
    let mut cube =
        |center, size, color| block(batches, material, center, size, Quat::IDENTITY, color);
    for i in 0..if far { 1 } else { 4 } {
        cube(
            [0.64, 1.12 - i as f32 * 0.054, 0.0],
            [0.032, 0.072, 0.037],
            DARK,
        )?;
    }
    cube([0.64, 0.84, 0.0], [0.18, 0.23, 0.18], [0.94, 0.49, 0.095])?;
    cube([0.64, 0.84, 0.097], [0.11, 0.17, 0.018], [1.0, 0.78, 0.28])?;
    cube([0.64, 0.84, -0.097], [0.11, 0.17, 0.018], [1.0, 0.78, 0.28])?;
    for y in [0.71, 0.96] {
        cube([0.64, y, 0.0], [0.23, 0.044, 0.23], SHADE)?;
        for x in [0.56, 0.72] {
            for z in [-0.12, 0.12] {
                cube([x, y, z], [0.025, 0.025, 0.014], CLOTH)?;
            }
        }
    }
    for x in [0.55, 0.64, 0.73] {
        for z in [-0.10, 0.10] {
            cube([x, 0.835, z], [0.018, 0.21, 0.018], DARK)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puppet_has_one_rooted_bounded_color_batch_and_progressive_detail() {
        let mut previous = usize::MAX;
        for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
            let mut batches = BTreeMap::new();
            append(&mut batches, 0, detail, &PropPalette::default()).unwrap();
            assert_eq!(batches.len(), 1);
            let mesh = batches.values().next().unwrap();
            assert!(mesh.positions.len() < previous);
            assert!(mesh.positions.len() <= 9_000);
            previous = mesh.positions.len();
            assert_eq!(mesh.positions.len(), mesh.colors.len());
            assert!(
                mesh.positions
                    .iter()
                    .all(|p| Vec3::from_array(*p).is_finite())
            );
            assert!(
                mesh.indices
                    .iter()
                    .all(|i| (*i as usize) < mesh.positions.len())
            );
            let (low, high) = mesh.positions.iter().map(|p| Vec3::from_array(*p)).fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(a, b), p| (a.min(p), b.max(p)),
            );
            assert!(low.y.abs() < 1e-6 && high.y > 1.98 && high.y < 2.05);
            assert!(high.x - low.x > 1.35 && high.x - low.x < 1.8);
            assert!(high.z - low.z > 0.60 && high.z - low.z < 0.7);
        }
    }

    #[test]
    fn puppet_picking_uses_straw_and_timber_not_its_full_two_meter_box() {
        use crate::props::{PropId, PropKind, WorldProp, raycast_props};
        use terra_voxel::SectionPos;
        let puppet = WorldProp {
            id: PropId(81),
            kind: PropKind::Scarecrow,
            position: [0.5, 1.0, 0.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 0,
            section: SectionPos::default(),
            removed: false,
        };
        assert!(raycast_props([0.5, 2.10, 2.0], [0.0, 0.0, -1.0], 3.0, [&puppet]).is_some());
        assert!(raycast_props([0.5, 1.30, 2.0], [0.0, 0.0, -1.0], 3.0, [&puppet]).is_some());
        assert!(raycast_props([0.10, 1.30, 2.0], [0.0, 0.0, -1.0], 3.0, [&puppet]).is_none());
    }
}
