//! Reusable, solid voxel crops. All ears, leaves and kernels join one section batch.
use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use crate::props::PropKind;
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;

type Batches = BTreeMap<usize, PropMesh>;
const GREEN: [f32; 3] = [0.27, 0.60, 0.085];
const LIGHT: [f32; 3] = [0.48, 0.76, 0.13];
const DARK: [f32; 3] = [0.15, 0.39, 0.045];
const GOLD: [f32; 3] = [0.99, 0.68, 0.12];
const STRAW: [f32; 3] = [0.82, 0.55, 0.15];

fn linear_color(color: [f32; 3]) -> [f32; 4] {
    let color = color.map(|value| {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    [color[0], color[1], color[2], 1.0]
}

fn block(
    batches: &mut Batches,
    material: usize,
    center: Vec3,
    size: Vec3,
    yaw: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let start = batches
        .get(&material)
        .map_or(0, |mesh| mesh.positions.len());
    add_box(batches, material, center, size, Quat::from_rotation_y(yaw))?;
    let mesh = batches.get_mut(&material).expect("box creates its batch");
    mesh.colors.resize(mesh.positions.len(), [1.0; 4]);
    mesh.colors[start..].fill(linear_color(color));
    Ok(())
}

pub(super) fn append(
    batches: &mut Batches,
    kind: PropKind,
    stage: u8,
    detail: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    let stage = stage.min(7);
    if stage == 0 {
        for (index, [x, z]) in [[-0.16, -0.13], [0.14, 0.12], [-0.10, 0.18], [0.18, -0.17]]
            .into_iter()
            .enumerate()
        {
            block(
                batches,
                palette.crop,
                Vec3::new(x, 0.024, z),
                Vec3::new(0.055, 0.045, 0.040),
                index as f32 * 0.7,
                if kind == PropKind::Lettuce {
                    [0.75, 0.71, 0.42]
                } else {
                    GOLD
                },
            )?;
        }
        return Ok(());
    }
    match kind {
        PropKind::Wheat => wheat(batches, palette.crop, stage, detail),
        PropKind::Corn => corn(batches, palette.crop, stage, detail),
        PropKind::Lettuce => lettuce(batches, palette.crop, stage, detail),
        _ => Err("Not a crop template".into()),
    }
}

fn wheat(
    batches: &mut Batches,
    material: usize,
    stage: u8,
    detail: PropDetail,
) -> Result<(), String> {
    let count = match detail {
        PropDetail::Near => {
            if stage < 3 {
                5
            } else {
                9
            }
        }
        PropDetail::Middle => 5,
        PropDetail::Far => 3,
    };
    let progress = f32::from(stage) / 7.0;
    let height = [0.0, 0.13, 0.27, 0.42, 0.62, 0.82, 1.01, 1.22][usize::from(stage)];
    let ripe = stage == 7;
    for ordinal in 0..count {
        let index = match count {
            3 => [2, 4, 6][ordinal],
            5 => [0, 2, 4, 6, 8][ordinal],
            _ => ordinal,
        };
        let x = [-0.24, 0.0, 0.24, -0.22, 0.02, 0.23, -0.19, 0.01, 0.25][index];
        let z = [-0.22, -0.20, -0.21, 0.0, 0.02, 0.01, 0.22, 0.24, 0.22][index];
        let x = x * (0.50 + progress * 0.50);
        let z = z * (0.50 + progress * 0.50);
        let h = height * (0.84 + (index * 7 % 5) as f32 * 0.04);
        let stem = if ripe { STRAW } else { GREEN };
        block(
            batches,
            material,
            Vec3::new(x, h * 0.40, z),
            Vec3::new(0.033, h * 0.80, 0.033),
            0.0,
            stem,
        )?;
        let leaves = if detail == PropDetail::Far { 1 } else { 2 };
        for side in 0..leaves {
            let sign = if (index + side) % 2 == 0 { -1.0 } else { 1.0 };
            let reach = (0.06 + progress * 0.075) * sign;
            let y = h * (0.30 + side as f32 * 0.22);
            block(
                batches,
                material,
                Vec3::new(x + reach * 0.5, y, z),
                Vec3::new(reach.abs() + 0.025, 0.027, 0.045),
                0.0,
                if ripe { GOLD } else { LIGHT },
            )?;
            if detail == PropDetail::Near && stage >= 3 {
                block(
                    batches,
                    material,
                    Vec3::new(x + reach, y + 0.035, z),
                    Vec3::new(0.040, 0.085, 0.040),
                    0.0,
                    if ripe { STRAW } else { GREEN },
                )?;
            }
        }
        if stage >= 5 {
            let rows = if detail == PropDetail::Far { 2 } else { 4 };
            let ear = h * 0.20;
            for row in 0..rows {
                let y = h * 0.79 + ear * row as f32 / rows as f32;
                let width = if row == rows - 1 { 0.062 } else { 0.082 };
                let color = if ripe {
                    if (row + index) % 3 == 0 {
                        [1.0, 0.79, 0.24]
                    } else {
                        GOLD
                    }
                } else if stage == 6 && (row + index) % 3 == 0 {
                    GOLD
                } else {
                    LIGHT
                };
                for sign in [-1.0, 1.0] {
                    block(
                        batches,
                        material,
                        Vec3::new(x + sign * 0.038, y, z),
                        Vec3::new(width, ear / rows as f32 * 0.96, 0.060),
                        0.0,
                        color,
                    )?;
                }
                if detail == PropDetail::Near {
                    block(
                        batches,
                        material,
                        Vec3::new(x, y + 0.014, z + 0.035),
                        Vec3::new(0.052, ear / rows as f32, 0.055),
                        0.0,
                        color,
                    )?;
                }
            }
            block(
                batches,
                material,
                Vec3::new(x, h + 0.018, z),
                Vec3::new(0.033, 0.07, 0.033),
                0.0,
                if ripe { GOLD } else { LIGHT },
            )?;
        }
    }
    Ok(())
}

fn corn_leaf(
    batches: &mut Batches,
    material: usize,
    root: Vec3,
    yaw: f32,
    length: f32,
    fine: bool,
) -> Result<(), String> {
    let steps = if fine { 4 } else { 2 };
    let rotation = Quat::from_rotation_y(yaw);
    for step in 0..steps {
        let t = (step as f32 + 0.5) / steps as f32;
        let width = length * (0.20 + (1.0 - (t * 2.0 - 1.0).abs()) * 0.35);
        let position =
            root + rotation * Vec3::new(length * t, length * (0.15 + (t * 2.7).sin() * 0.23), 0.0);
        block(
            batches,
            material,
            position,
            Vec3::new(length / steps as f32 * 1.03, 0.035, width),
            yaw,
            if step % 2 == 0 { GREEN } else { LIGHT },
        )?;
        if fine && step % 2 == 0 {
            block(
                batches,
                material,
                position + Vec3::Y * 0.022,
                Vec3::new(length / steps as f32 * 1.04, 0.016, 0.023),
                yaw,
                [0.53, 0.71, 0.12],
            )?;
        }
    }
    Ok(())
}

fn cob(
    batches: &mut Batches,
    material: usize,
    root: Vec3,
    stage: u8,
    detail: PropDetail,
) -> Result<(), String> {
    let ripe = stage == 7;
    let length = if ripe { 0.32 } else { 0.24 };
    block(
        batches,
        material,
        root + Vec3::Y * length * 0.45,
        Vec3::new(if ripe { 0.13 } else { 0.15 }, length, 0.13),
        0.0,
        GREEN,
    )?;
    if stage < 6 {
        return Ok(());
    }
    let rows = if detail == PropDetail::Near { 6 } else { 3 };
    let sides = if detail == PropDetail::Far { 2 } else { 4 };
    for row in 0..rows {
        for side in 0..sides {
            let angle = side as f32 * std::f32::consts::TAU / sides as f32;
            let radius = if row == rows - 1 { 0.048 } else { 0.069 };
            block(
                batches,
                material,
                root + Vec3::new(
                    angle.cos() * radius,
                    0.04 + row as f32 * length / rows as f32,
                    angle.sin() * radius,
                ),
                Vec3::new(
                    if ripe { 0.086 } else { 0.060 },
                    length / rows as f32 * 0.88,
                    if ripe { 0.086 } else { 0.060 },
                ),
                angle,
                if ripe {
                    if (side + row) % 3 == 0 {
                        [1.0, 0.83, 0.22]
                    } else {
                        GOLD
                    }
                } else {
                    [0.72, 0.78, 0.16]
                },
            )?;
        }
    }
    for sign in [-1.0, 1.0] {
        block(
            batches,
            material,
            root + Vec3::new(sign * 0.10, 0.065, 0.0),
            Vec3::new(0.07, 0.18, 0.12),
            0.0,
            LIGHT,
        )?;
    }
    Ok(())
}

fn corn(
    batches: &mut Batches,
    material: usize,
    stage: u8,
    detail: PropDetail,
) -> Result<(), String> {
    let height = [0.0, 0.20, 0.37, 0.65, 1.01, 1.40, 1.69, 1.95][usize::from(stage)];
    let stems = if stage < 3 || detail != PropDetail::Near {
        1
    } else {
        3
    };
    for stem in 0..stems {
        let root = [
            Vec3::ZERO,
            Vec3::new(-0.20, 0.0, 0.17),
            Vec3::new(0.19, 0.0, -0.18),
        ][stem];
        let h = height * if stem == 0 { 1.0 } else { 0.84 };
        block(
            batches,
            material,
            root + Vec3::Y * h * 0.47,
            Vec3::new(0.068, h * 0.94, 0.068),
            0.0,
            GREEN,
        )?;
        let leaves = match detail {
            PropDetail::Near => (usize::from(stage) + 1).min(6),
            PropDetail::Middle => 4.min(usize::from(stage) + 1),
            PropDetail::Far => 3.min(usize::from(stage) + 1),
        };
        for leaf in 0..leaves {
            let y = h * (0.12 + leaf as f32 / leaves as f32 * 0.65);
            let yaw = leaf as f32 * 2.40 + stem as f32 * 1.2;
            let length = (0.10 + h * 0.23) * if leaf == leaves - 1 { 0.75 } else { 1.0 };
            corn_leaf(
                batches,
                material,
                root + Vec3::Y * y,
                yaw,
                length.min(0.39),
                detail == PropDetail::Near,
            )?;
        }
        if stage >= 5 {
            let y = h * 0.51;
            cob(
                batches,
                material,
                root + Vec3::new(0.09, y, 0.03),
                stage,
                detail,
            )?;
            if stem == 0 && detail == PropDetail::Near && stage >= 6 {
                cob(
                    batches,
                    material,
                    root + Vec3::new(-0.09, h * 0.36, -0.03),
                    stage,
                    detail,
                )?;
            }
            let tassel = if stage >= 6 { GOLD } else { LIGHT };
            block(
                batches,
                material,
                root + Vec3::Y * h * 0.95,
                Vec3::new(0.026, h * 0.10, 0.026),
                0.0,
                tassel,
            )?;
            for branch in 0..if detail == PropDetail::Near { 6 } else { 3 } {
                let sign = if branch % 2 == 0 { -1.0 } else { 1.0 };
                let t = branch as f32 / 6.0;
                block(
                    batches,
                    material,
                    root + Vec3::new(sign * 0.055, h * (0.90 + t * 0.08), 0.0),
                    Vec3::new(0.10 * (1.0 - t * 0.5), 0.025, 0.025),
                    0.0,
                    tassel,
                )?;
            }
        }
    }
    Ok(())
}

fn lettuce_head(batches: &mut Batches, material: usize, radius: f32) -> Result<(), String> {
    let mut cells = BTreeMap::new();
    for x in -4_i16..4 {
        for y in 0_i16..8 {
            for z in -4_i16..4 {
                let dx = (f32::from(x) + 0.5) / 3.65;
                let dy = (f32::from(y) - 3.5) / 3.85;
                let dz = (f32::from(z) + 0.5) / 3.65;
                let fold = (x.abs() + z.abs() + y * 2).rem_euclid(5);
                let edge = if fold == 0 { 0.91 } else { 1.04 };
                if dx * dx + dy * dy + dz * dz > edge {
                    continue;
                }
                let color = if fold == 0 {
                    GREEN
                } else if y >= 5 && x.abs() + z.abs() < 5 {
                    [0.72, 0.87, 0.29]
                } else if fold == 1 {
                    [0.58, 0.79, 0.17]
                } else {
                    LIGHT
                };
                cells.insert([x, y, z], linear_color(color));
            }
        }
    }
    let unit = radius * 0.1333;
    let batch = batches.entry(material).or_insert_with(|| PropMesh {
        material,
        ..Default::default()
    });
    // Only the outer leaf surface is emitted; hidden faces inside the head never reach the GPU.
    for (&cell, &color) in &cells {
        let center = Vec3::from_array(cell.map(|v| f32::from(v) + 0.5)) * unit + Vec3::Y * 0.055;
        for axis in 0..3 {
            for sign in [-1_i16, 1] {
                let mut neighbor = cell;
                neighbor[axis] += sign;
                if cells.contains_key(&neighbor) {
                    continue;
                }
                let mut normal = Vec3::ZERO;
                normal[axis] = f32::from(sign);
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let base =
                    u32::try_from(batch.positions.len()).map_err(|_| "Crop head overflow")?;
                base.checked_add(3).ok_or("Crop head overflow")?;
                for [a, b] in [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
                    let mut point = normal * unit * 0.5;
                    point[u] = a * unit;
                    point[v] = b * unit;
                    batch.positions.push((center + point).to_array());
                    batch.normals.push(normal.to_array());
                    batch.uvs.push([a + 0.5, b + 0.5]);
                    batch.colors.push(color);
                }
                let indices = if sign > 0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                };
                batch.indices.extend(indices.map(|index| base + index));
            }
        }
    }
    Ok(())
}

fn lettuce(
    batches: &mut Batches,
    material: usize,
    stage: u8,
    detail: PropDetail,
) -> Result<(), String> {
    let progress = f32::from(stage) / 7.0;
    let radius = 0.055 + progress * 0.35;
    let leaves = match detail {
        PropDetail::Near => 10,
        PropDetail::Middle => 7,
        PropDetail::Far => 4,
    };
    let steps = if detail == PropDetail::Near { 4 } else { 2 };
    block(
        batches,
        material,
        Vec3::Y * 0.045,
        Vec3::new(radius * 0.75, 0.09, radius * 0.75),
        0.0,
        DARK,
    )?;
    for leaf in 0..leaves {
        let yaw = leaf as f32 * std::f32::consts::TAU / leaves as f32;
        let rotation = Quat::from_rotation_y(yaw);
        for step in 0..steps {
            let t = (step as f32 + 0.5) / steps as f32;
            let width = radius * (0.33 + (1.0 - (t * 2.0 - 1.0).abs()) * 0.60);
            let rise = progress * (0.045 + t * t * 0.21);
            let center = rotation * Vec3::new(radius * t, rise + 0.04, 0.0);
            block(
                batches,
                material,
                center,
                Vec3::new(
                    radius / steps as f32 * 1.04,
                    0.035 + progress * 0.025,
                    width,
                ),
                yaw,
                if (leaf + step) % 3 == 0 { LIGHT } else { GREEN },
            )?;
            if detail == PropDetail::Near && stage >= 3 {
                block(
                    batches,
                    material,
                    center + Vec3::Y * 0.028,
                    Vec3::new(radius / steps as f32, 0.020, 0.024),
                    yaw,
                    [0.65, 0.82, 0.25],
                )?;
            }
        }
    }
    if stage >= 3 {
        if detail == PropDetail::Near {
            return lettuce_head(batches, material, radius);
        }
        let tiers = 3;
        for tier in 0..tiers {
            let t = tier as f32 / tiers as f32;
            let width = radius * (1.18 - t * 0.65);
            let y = 0.12 + progress * 0.35 * t;
            block(
                batches,
                material,
                Vec3::Y * y,
                Vec3::new(width, 0.055 + progress * 0.024, width),
                0.0,
                if tier % 2 == 0 {
                    LIGHT
                } else {
                    [0.68, 0.84, 0.24]
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::visual::template;

    #[test]
    fn wheat_only_looks_fully_golden_when_the_harvest_is_mature() {
        for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
            let colors = |stage| {
                template(PropKind::Wheat, stage, detail, &PropPalette::default())
                    .unwrap()
                    .into_iter()
                    .flat_map(|mesh| mesh.colors)
                    .collect::<Vec<_>>()
            };
            let ripening = colors(6);
            assert!(ripening.iter().any(|color| color[1] > color[0] * 2.0));
            assert!(ripening.iter().any(|color| color[0] > color[1] * 1.4));
            let mature = colors(7);
            assert!(mature.iter().all(|color| color[0] > color[1] * 1.4));
        }
    }

    #[test]
    fn lettuce_has_a_rounded_wrinkled_head_without_hidden_internal_cube_faces() {
        let mut batches = BTreeMap::new();
        lettuce_head(&mut batches, 0, 0.405).unwrap();
        let mesh = &batches[&0];
        assert!(mesh.positions.len() < 1600, "Cull the solid head interior");
        let top_heights: std::collections::BTreeSet<_> = mesh
            .positions
            .chunks_exact(4)
            .zip(mesh.normals.chunks_exact(4))
            .filter(|(_, normals)| normals[0][1] > 0.99)
            .map(|(positions, _)| (positions[0][1] * 10000.0).round() as i32)
            .collect();
        assert!(
            top_heights.len() >= 4,
            "The head needs individual folded leaves"
        );
        let top = mesh.positions.iter().map(|p| p[1]).fold(0.0_f32, f32::max);
        assert!(
            mesh.positions
                .iter()
                .filter(|p| p[1] > top - 0.001)
                .all(|p| p[0].hypot(p[2]) < 0.18)
        );
    }

    #[test]
    fn crop_stages_are_distinct_rooted_solid_and_bounded_shared_templates() {
        for (kind, max_height, budget) in [
            (PropKind::Wheat, 1.30, 4400),
            (PropKind::Corn, 1.96, 6200),
            (PropKind::Lettuce, 0.51, 4000),
        ] {
            let mut previous = None;
            let mut previous_height = 0.0;
            for stage in 0..8 {
                for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                    let meshes = template(kind, stage, detail, &PropPalette::default()).unwrap();
                    assert_eq!(
                        meshes,
                        template(kind, stage, detail, &PropPalette::default()).unwrap()
                    );
                    let vertices = meshes.iter().map(|m| m.positions.len()).sum::<usize>();
                    assert!(
                        vertices > 0 && vertices <= budget,
                        "{kind:?}/{stage}/{detail:?}: {vertices}"
                    );
                    let mut height = 0.0_f32;
                    for mesh in &meshes {
                        assert_eq!(mesh.colors.len(), mesh.positions.len());
                        assert_eq!(mesh.normals.len(), mesh.positions.len());
                        assert_eq!(mesh.uvs.len(), mesh.positions.len());
                        assert!(mesh.indices.len().is_multiple_of(3));
                        for position in &mesh.positions {
                            assert!(position.iter().all(|v| v.is_finite()));
                            assert!(position[0].abs() <= 0.65 && position[2].abs() <= 0.65);
                            assert!(
                                position[1] >= -0.001 && position[1] <= max_height,
                                "{kind:?}/{stage}: {position:?}"
                            );
                            height = height.max(position[1]);
                        }
                        for triangle in mesh.indices.chunks_exact(3) {
                            assert!(
                                triangle
                                    .iter()
                                    .all(|&i| (i as usize) < mesh.positions.len())
                            );
                            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                                .map(|i| Vec3::from_array(mesh.positions[i as usize]));
                            let normal = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                            assert!((b - a).cross(c - a).dot(normal) > 0.0);
                        }
                    }
                    if detail == PropDetail::Near {
                        assert!(
                            height > previous_height,
                            "{kind:?}/{stage}: growth must remain visible"
                        );
                        assert_ne!(previous.as_ref(), Some(&meshes));
                        previous_height = height;
                        previous = Some(meshes);
                    }
                }
            }
        }
    }
}
