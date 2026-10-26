//! Rooted, square-ended reef details. Shared geometry also drives precise picking.
use super::{
    PropDetail, PropMesh, PropPalette,
    geometry::{add_box, add_stalk},
};
use crate::props::PropKind;
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

type Batches = BTreeMap<usize, PropMesh>;
fn voxel(
    batches: &mut Batches,
    material: usize,
    p: [f32; 3],
    size: [f32; 3],
    yaw: f32,
) -> Result<(), String> {
    add_box(
        batches,
        material,
        Vec3::from_array(p),
        Vec3::from_array(size),
        Quat::from_rotation_y(yaw),
    )
}

fn colored_voxel(
    batches: &mut Batches,
    material: usize,
    p: [f32; 3],
    size: [f32; 3],
    color: [f32; 3],
) -> Result<(), String> {
    let start = batches
        .get(&material)
        .map_or(0, |mesh| mesh.positions.len());
    voxel(batches, material, p, size, 0.0)?;
    if let Some(mesh) = batches.get_mut(&material) {
        mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
        mesh.colors[start..].fill([color[0], color[1], color[2], 1.0]);
    }
    Ok(())
}

fn surface_variation(batches: &mut Batches, variant: usize) {
    for mesh in batches.values_mut() {
        mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
        for (index, quad) in mesh.positions.chunks_exact(4).enumerate() {
            let center = quad
                .iter()
                .fold(Vec3::ZERO, |sum, p| sum + Vec3::from_array(*p))
                * 0.25;
            let mut seed = variant as u32 ^ 0x79a1_2d63;
            for axis in 0..3 {
                seed = (seed ^ (center[axis] * 1024.0).round() as i32 as u32)
                    .wrapping_mul(0x85eb_ca6b);
            }
            let shade = 0.86 + (seed % 13) as f32 * 0.01;
            for color in &mut mesh.colors[index * 4..index * 4 + 4] {
                for channel in &mut color[..3] {
                    *channel *= shade;
                }
            }
        }
    }
}

pub(super) fn append(
    b: &mut Batches,
    kind: PropKind,
    v: usize,
    lod: PropDetail,
    p: &PropPalette,
) -> Result<(), String> {
    let fine = lod == PropDetail::Near;
    let far = lod == PropDetail::Far;
    match kind {
        PropKind::Seagrass => {
            for i in 0..if far {
                2
            } else if fine {
                5
            } else {
                4
            } {
                let yaw = i as f32 * 2.39996 + v as f32 * 0.4;
                let radius = 0.07 + (i % 3) as f32 * 0.04;
                let height = 2.15 + ((i * 7 + v * 3) % 9) as f32 * 0.15;
                let root = Vec3::new(yaw.cos() * radius, 0.0, yaw.sin() * radius);
                let path = [
                    Vec3::ZERO,
                    Vec3::new(0.04, height * 0.32, 0.0),
                    Vec3::new(-0.025, height * 0.68, 0.018),
                    Vec3::new(0.03, height, 0.02),
                ];
                let distant = [path[0], path[3]];
                add_stalk(
                    b,
                    p.foliage[(i + v) % 3],
                    root,
                    Vec2::new(if far { 0.10 } else { 0.065 }, 0.04),
                    if far { &distant } else { &path },
                    yaw,
                )?;
                if !far {
                    // Staggered blunt leaf blades, like the tall fronds in the reference.
                    for leaf in 0..if fine { 7 } else { 3 } {
                        let t = 0.17 + leaf as f32 * if fine { 0.11 } else { 0.28 };
                        let side = if (i + leaf) % 2 == 0 { -1.0 } else { 1.0 };
                        let turn = Quat::from_rotation_y(yaw);
                        let origin = root + turn * Vec3::new(0.01, height * t, 0.0);
                        let width = 0.08 + ((leaf + v) % 3) as f32 * 0.018;
                        add_stalk(
                            b,
                            p.foliage[(i + leaf + v) % 3],
                            origin,
                            Vec2::new(width, 0.035),
                            &[
                                Vec3::ZERO,
                                Vec3::new(side * 0.10, 0.12, 0.0),
                                Vec3::new(side * 0.13, 0.23, 0.01),
                            ],
                            yaw,
                        )?;
                    }
                }
            }
        }
        PropKind::SeaCucumber => {
            let body = [p.flowers[5], p.mushroom, p.stone][v % 3];
            let tint = if v % 3 == 2 {
                [0.17, 0.15, 0.14]
            } else {
                [0.95, 0.78, 0.78]
            };
            for (y, width, length) in [(0.06, 0.22, 0.64), (0.14, 0.28, 0.72), (0.21, 0.21, 0.58)] {
                if fine {
                    for segment in 0..5 {
                        let swell = [0.76, 0.93, 1.0, 0.94, 0.80][segment];
                        colored_voxel(
                            b,
                            body,
                            [0.0, y, (segment as f32 - 2.0) * length * 0.2],
                            [width * swell, 0.09, length * 0.22],
                            tint,
                        )?;
                    }
                } else {
                    colored_voxel(b, body, [0.0, y, 0.0], [width, 0.09, length], tint)?;
                }
            }
            if fine {
                for i in 0..22 {
                    let z = (i / 4) as f32 * 0.095 - 0.24;
                    let side = i % 4;
                    let x = if side < 2 {
                        [-0.065, 0.065][side]
                    } else {
                        [-0.135, 0.135][side - 2]
                    };
                    let y = if side < 2 { 0.258 } else { 0.155 };
                    let size = 0.028 + ((i * 3 + v) % 4) as f32 * 0.006;
                    colored_voxel(
                        b,
                        p.flowers[2],
                        [x, y, z],
                        [size; 3],
                        if v.is_multiple_of(3) {
                            [0.72, 0.34, 0.88]
                        } else {
                            [0.66, 0.43, 0.20]
                        },
                    )?;
                }
            }
        }
        PropKind::BranchingCoral => {
            let red = if v % 3 == 2 {
                p.flowers[4]
            } else {
                p.flowers[0]
            };
            voxel(b, red, [0.0, 0.30, 0.0], [0.13, 0.60, 0.13], 0.0)?;
            for i in 0..if far {
                2
            } else if fine {
                7
            } else {
                5
            } {
                let yaw = i as f32 * 2.39996 + v as f32 * 0.32;
                let height = 0.37 + (i % 3) as f32 * 0.17;
                let direction = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let root = Vec3::Y * (0.17 + i as f32 * 0.065);
                let end = root + direction * 0.24 + Vec3::Y * height;
                add_stalk(
                    b,
                    red,
                    root,
                    Vec2::splat(0.085),
                    &[Vec3::ZERO, direction * 0.19 + Vec3::Y * 0.13, end - root],
                    0.0,
                )?;
                if fine {
                    let fork = end - Vec3::Y * 0.16;
                    add_stalk(
                        b,
                        red,
                        fork,
                        Vec2::splat(0.055),
                        &[Vec3::ZERO, direction * 0.12 + Vec3::Y * 0.19],
                        0.0,
                    )?;
                    voxel(b, p.flowers[0], end.to_array(), [0.11, 0.065, 0.11], 0.0)?;
                    let side = Vec3::new(-direction.z, 0.0, direction.x);
                    add_stalk(
                        b,
                        red,
                        root + direction * 0.11 + Vec3::Y * 0.10,
                        Vec2::splat(0.05),
                        &[
                            Vec3::ZERO,
                            side * 0.15 + Vec3::Y * (0.19 + (i % 2) as f32 * 0.08),
                        ],
                        0.0,
                    )?;
                    for nub in 0..4 {
                        let center = root.lerp(end, 0.32 + nub as f32 * 0.18)
                            + side * if nub % 2 == 0 { 0.043 } else { -0.043 };
                        colored_voxel(
                            b,
                            red,
                            center.to_array(),
                            [0.072, 0.055, 0.065],
                            [1.0, 0.76 + (nub % 2) as f32 * 0.20, 0.74],
                        )?;
                    }
                }
            }
        }
        PropKind::TubeCoral => {
            for i in 0..if far { 1 } else { 6 } {
                let yaw = i as f32 * 2.39996;
                let radius = if i == 0 { 0.0 } else { 0.22 };
                let x = yaw.cos() * radius;
                let z = yaw.sin() * radius;
                let h = 0.38 + ((i * 3 + v) % 7) as f32 * 0.075;
                // Four thin walls leave a real open square bore, including in the far mesh.
                for side in 0..4 {
                    let a = side as f32 * std::f32::consts::FRAC_PI_2;
                    let tiers = if fine { 3 } else { 1 };
                    for tier in 0..tiers {
                        let step = h / tiers as f32;
                        let rim = 0.07 + tier as f32 * 0.006;
                        let size = if side % 2 == 0 {
                            [0.045, step, 0.18 + tier as f32 * 0.012]
                        } else {
                            [0.18 + tier as f32 * 0.012, step, 0.045]
                        };
                        colored_voxel(
                            b,
                            if tier == 2 {
                                p.flowers[2]
                            } else {
                                p.flowers[3]
                            },
                            [
                                x + a.cos() * rim,
                                step * (tier as f32 + 0.5),
                                z + a.sin() * rim,
                            ],
                            size,
                            match tier {
                                0 => [0.70, 0.79, 0.91],
                                1 => [0.93, 0.95, 1.0],
                                _ => [0.20, 0.53, 1.0],
                            },
                        )?;
                    }
                    if fine {
                        let size = if side % 2 == 0 {
                            [0.045, 0.035, 0.215]
                        } else {
                            [0.215, 0.035, 0.045]
                        };
                        colored_voxel(
                            b,
                            p.flowers[2],
                            [x + a.cos() * 0.084, h + 0.015, z + a.sin() * 0.084],
                            size,
                            [0.24, 0.60, 1.0],
                        )?;
                    }
                }
                if fine && i % 2 == 0 {
                    colored_voxel(
                        b,
                        p.flowers[3],
                        [x + 0.10, h * 0.58, z],
                        [0.035, 0.07, 0.045],
                        [0.68, 0.80, 0.94],
                    )?;
                }
            }
        }
        PropKind::PlateCoral => {
            voxel(b, p.flowers[1], [0.0, 0.36, 0.0], [0.11, 0.72, 0.11], 0.0)?;
            for i in 0..if far { 2 } else { 5 } {
                let yaw = i as f32 * 2.39996 + v as f32 * 0.2;
                let y = 0.18 + i as f32 * 0.15;
                let x = yaw.cos() * 0.14;
                let z = yaw.sin() * 0.14;
                let width = 0.37 + (i % 2) as f32 * 0.12;
                colored_voxel(
                    b,
                    p.flowers[1],
                    [x, y, z],
                    [width, 0.055, width * 0.72],
                    [0.72, 0.68, 0.42],
                )?;
                if !far {
                    colored_voxel(
                        b,
                        p.flowers[1],
                        [x, y, z],
                        [width * 0.72, 0.055, width],
                        [0.78, 0.73, 0.48],
                    )?;
                }
                if fine {
                    for edge in 0..4 {
                        let angle = edge as f32 * std::f32::consts::FRAC_PI_2;
                        let radius = width * (0.41 + ((edge + i + v) % 3) as f32 * 0.018);
                        let size = if edge % 2 == 0 {
                            [width * 0.17, 0.035, width * 0.62]
                        } else {
                            [width * 0.62, 0.035, width * 0.17]
                        };
                        colored_voxel(
                            b,
                            p.flowers[2],
                            [
                                x + angle.cos() * radius,
                                y + 0.035,
                                z + angle.sin() * radius,
                            ],
                            size,
                            [1.0, 0.65, 0.09],
                        )?;
                    }
                    for nub in 0..3 {
                        let angle = yaw + nub as f32 * 2.39996;
                        colored_voxel(
                            b,
                            p.flowers[1],
                            [
                                x + angle.cos() * width * 0.23,
                                y + 0.045,
                                z + angle.sin() * width * 0.23,
                            ],
                            [0.045, 0.04, 0.045],
                            [0.94, 0.90, 0.56],
                        )?;
                    }
                }
            }
        }
        PropKind::SeaAnemone => {
            colored_voxel(
                b,
                p.stone,
                [0.0, 0.035, 0.0],
                [0.36, 0.07, 0.34],
                [0.30, 0.30, 0.36],
            )?;
            let tissue = p.flowers[4];
            colored_voxel(
                b,
                tissue,
                [0.0, 0.115, 0.0],
                [0.31, 0.16, 0.29],
                [0.85, 0.40, 0.64],
            )?;
            colored_voxel(
                b,
                tissue,
                [0.0, 0.195, 0.0],
                [0.40, 0.045, 0.37],
                [0.80, 0.54, 0.73],
            )?;
            let count = if far {
                3
            } else if fine {
                28
            } else {
                10
            };
            for i in 0..count {
                let yaw = i as f32 * 2.39996 + v as f32 * 0.27;
                let outer = i % 3 != 0;
                let radial = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let root = radial * if outer { 0.15 } else { 0.07 } + Vec3::Y * 0.19;
                let h = 0.19 + ((i * 3 + v) % 7) as f32 * 0.021;
                let reach = if outer { 0.15 } else { 0.055 };
                let end = radial * reach + Vec3::Y * h;
                let path = [
                    Vec3::ZERO,
                    radial * reach * 0.7 + Vec3::Y * h * 0.45,
                    end - radial * 0.015 + Vec3::Y * 0.015,
                    end,
                ];
                let distant = [Vec3::ZERO, end];
                add_stalk(
                    b,
                    tissue,
                    root,
                    Vec2::splat(if fine { 0.026 } else { 0.043 }),
                    if fine { &path } else { &distant },
                    0.0,
                )?;
                if !far {
                    colored_voxel(
                        b,
                        p.flowers[2],
                        (root + end).to_array(),
                        [0.037, 0.041, 0.037],
                        [1.0, 0.69, 0.80],
                    )?;
                }
            }
        }
        PropKind::SmallStones => {
            for i in 0..if far { 2 } else { 5 } {
                let yaw = i as f32 * 2.39996 + v as f32 * 0.5;
                let radius = if i == 0 { 0.0 } else { 0.19 };
                let width = 0.17 + ((i + v) % 3) as f32 * 0.05;
                voxel(
                    b,
                    p.stone,
                    [yaw.cos() * radius, width * 0.40, yaw.sin() * radius],
                    [width, width * 0.8, width * 0.85],
                    yaw,
                )?;
            }
        }
        _ => return Err("Non-aquatic prop sent to reef geometry".into()),
    }
    surface_variation(b, v);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::{PropId, PropPose, WorldProp, raycast_props_with_pose};

    #[test]
    fn detailed_reef_colors_are_deterministic_and_tube_bores_remain_open() {
        let palette = PropPalette {
            foliage: [0, 1, 2],
            stem: 3,
            flowers: [4, 5, 6, 7, 8, 9, 14, 15],
            stone: 10,
            mushroom: 11,
            ..Default::default()
        };
        for variant in 0..8 {
            let meshes =
                super::super::template(PropKind::TubeCoral, variant, PropDetail::Near, &palette)
                    .unwrap();
            assert_eq!(
                meshes,
                super::super::template(PropKind::TubeCoral, variant, PropDetail::Near, &palette)
                    .unwrap()
            );
            let crown = meshes
                .iter()
                .find(|mesh| mesh.material == palette.flowers[2])
                .unwrap();
            assert!(
                crown
                    .colors
                    .iter()
                    .all(|color| color[2] > color[1] && color[1] > color[0])
            );
            let prop = WorldProp {
                id: PropId(78),
                kind: PropKind::TubeCoral,
                variant,
                position: [0.5, 1.0, 0.5],
                rotation: 0.0,
                scale: 1.0,
                section: Default::default(),
                removed: false,
            };
            assert!(
                raycast_props_with_pose(
                    [0.5, 2.5, 0.5],
                    [0.0, -1.0, 0.0],
                    3.0,
                    [&prop],
                    PropPose::Rest
                )
                .is_none(),
                "the central tube must not become a capped pillar"
            );
            assert!(
                raycast_props_with_pose(
                    [0.57, 2.5, 0.5],
                    [0.0, -1.0, 0.0],
                    3.0,
                    [&prop],
                    PropPose::Rest
                )
                .is_some()
            );
        }
        let anemone =
            super::super::template(PropKind::SeaAnemone, 0, PropDetail::Near, &palette).unwrap();
        assert!(
            anemone
                .iter()
                .find(|m| m.material == palette.stone)
                .unwrap()
                .colors
                .iter()
                .all(|color| color[..3].iter().all(|channel| *channel <= 0.36))
        );
        let plates =
            super::super::template(PropKind::PlateCoral, 0, PropDetail::Near, &palette).unwrap();
        assert!(
            plates
                .iter()
                .find(|m| m.material == palette.flowers[2])
                .unwrap()
                .colors
                .iter()
                .all(|color| color[0] > color[1] && color[1] > color[2])
        );
    }

    #[test]
    fn reef_templates_have_rooted_valid_geometry_and_bounded_lods() {
        for kind in PropKind::ALL.into_iter().filter(|kind| kind.aquatic()) {
            assert_eq!(
                PropKind::from(terra_core::ResourcePropKind::from(kind)),
                kind
            );
            for variant in 0..8 {
                let mut previous = usize::MAX;
                for level in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                    let meshes =
                        super::super::template(kind, variant, level, &PropPalette::default())
                            .unwrap();
                    let vertices: usize = meshes.iter().map(|mesh| mesh.positions.len()).sum();
                    let budget = match level {
                        PropDetail::Near => 2400,
                        PropDetail::Middle => 800,
                        PropDetail::Far => 160,
                    };
                    assert!(
                        vertices > 0 && vertices <= budget && vertices <= previous,
                        "{kind:?} {level:?}: {vertices}"
                    );
                    previous = vertices;
                    let minimum = meshes
                        .iter()
                        .flat_map(|mesh| &mesh.positions)
                        .map(|p| p[1])
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (-0.001..0.04).contains(&minimum),
                        "{kind:?} must touch its supporting block"
                    );
                    for mesh in meshes {
                        assert_eq!(mesh.normals.len(), mesh.positions.len());
                        assert_eq!(mesh.uvs.len(), mesh.positions.len());
                        assert_eq!(mesh.colors.len(), mesh.positions.len());
                        assert!(
                            mesh.colors
                                .iter()
                                .flatten()
                                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
                        );
                        assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
                        for triangle in mesh.indices.chunks_exact(3) {
                            assert!(
                                triangle
                                    .iter()
                                    .all(|&i| (i as usize) < mesh.positions.len())
                            );
                            let [a, b, c] = std::array::from_fn::<_, 3, _>(|i| {
                                Vec3::from_array(mesh.positions[triangle[i] as usize])
                            });
                            let n = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                            assert!((b - a).cross(c - a).dot(n) > 0.0);
                        }
                    }
                }
                let prop = WorldProp {
                    id: PropId(78),
                    kind,
                    variant,
                    position: [-16.5, -32.0, 32.5],
                    rotation: 0.0,
                    scale: 0.8,
                    section: terra_voxel::global_to_section([-17, -33, 32]).0,
                    removed: false,
                };
                assert!(prop.valid() && prop.water_tolerant());
                let center = prop.bounds();
                let origin = [prop.position[0], center[1][1] + 1.0, prop.position[2]];
                // Tubes and scattered rocks deliberately contain gaps; search their actual triangles.
                let has_hit = super::super::template(
                    kind,
                    variant,
                    PropDetail::Near,
                    &PropPalette::default(),
                )
                .unwrap()
                .iter()
                .flat_map(|mesh| &mesh.positions)
                .any(|p| {
                    raycast_props_with_pose(
                        [
                            origin[0] + f64::from(p[0]) * 0.8,
                            origin[1],
                            origin[2] + f64::from(p[2]) * 0.8,
                        ],
                        [0.0, -1.0, 0.0],
                        4.0,
                        [&prop],
                        PropPose::Rest,
                    )
                    .is_some()
                });
                assert!(has_hit, "{kind:?} must be individually pickable");
            }
        }
    }

    #[test]
    fn underwater_sway_is_rooted_and_independent_of_atmospheric_storms() {
        use crate::props::{wind_displacement, wind_parameters};
        for kind in [PropKind::Seagrass, PropKind::SeaAnemone] {
            assert_eq!(wind_parameters(kind, 0, 0.0, 1.0)[0], 0.0);
            let weights = wind_parameters(kind, 0, 0.7, 1.0);
            let calm = wind_displacement([2.0, -20.0, -17.0], 3.0, weights, [0.86, 0.51, 0.2, 1.0]);
            let storm = wind_displacement([2.0, -20.0, -17.0], 3.0, weights, [-1.0, 0.0, 4.0, 1.0]);
            assert_eq!(calm, storm);
            assert_ne!(calm, [0.0; 2]);
        }
    }
}
