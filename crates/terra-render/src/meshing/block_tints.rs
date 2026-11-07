//! Preserve crisp per-cell textile colors across otherwise greedy opaque faces.
use std::collections::HashMap;
use terra_voxel::meshing::BlockFace;

pub(super) fn split(face: BlockFace, tints: &HashMap<[usize; 3], [u8; 3]>) -> Vec<BlockFace> {
    let mut result = Vec::new();
    subdivide(face, tints, &mut result);
    result
}

fn subdivide(face: BlockFace, tints: &HashMap<[usize; 3], [u8; 3]>, result: &mut Vec<BlockFace>) {
    let color = |u: u32, v: u32| {
        let mut cell = face.minimum.map(|v| v as usize);
        cell[face.u_axis] += u as usize;
        cell[face.v_axis] += v as usize;
        tints.get(&cell).copied().unwrap_or([255; 3])
    };
    let first = color(0, 0);
    if (0..face.height).all(|v| (0..face.width).all(|u| color(u, v) == first)) {
        result.push(face);
        return;
    }
    let mut a = face;
    let mut b = face;
    if face.width >= face.height {
        a.width /= 2;
        b.minimum[face.u_axis] += a.width;
        b.width -= a.width;
    } else {
        a.height /= 2;
        b.minimum[face.v_axis] += a.height;
        b.height -= a.height;
    }
    subdivide(a, tints, result);
    subdivide(b, tints, result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wool_rgb_boundaries_keep_exact_geometry_palette_and_batches_at_every_lod_and_negative_sections()
     {
        use crate::{
            blocks::{BlockTemplate, TemplateSet, cube_faces, definitions::BlockRenderKind},
            meshing::{
                DetailLevel,
                builder::{RenderOptions, compose_section_with_options},
            },
        };
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            103,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: cube_faces(0.0, [0; 6], None),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        let face = BlockFace {
            minimum: [0; 3],
            width: 3,
            height: 1,
            side: 3,
            u_axis: 0,
            v_axis: 2,
            material: 103,
            occupied_neighbor: false,
            edge_mask: 15,
        };
        let rgb = [[71, 93, 184], [237, 92, 31], [255; 3]];
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            for section in [
                terra_voxel::SectionPos::default(),
                terra_voxel::SectionPos::new(-1, -2, -1),
            ] {
                let options = RenderOptions {
                    water_level: None,
                    detail_level: level,
                    terrain: None,
                    exposure: None,
                    tints: std::sync::Arc::new([([0, 0, 0], rgb[0]), ([1, 0, 0], rgb[1])].into()),
                    conditions: Default::default(),
                };
                let batches =
                    compose_section_with_options(&[face], &templates, section, &options).unwrap();
                assert_eq!(batches.len(), 1, "RGB boundaries must not add draws");
                let batch = &batches[0];
                let mut area = 0.0;
                for triangle in batch.indices.chunks_exact(3) {
                    let points: [bevy::math::Vec3; 3] = std::array::from_fn(|corner| {
                        bevy::math::Vec3::from_array(batch.positions[triangle[corner] as usize])
                    });
                    let center = (points[0] + points[1] + points[2]) / 3.0;
                    let expected = crate::dye_tint::rgb(rgb[center.x.floor() as usize]);
                    for &index in triangle {
                        assert_eq!(batch.colors[index as usize], expected);
                    }
                    assert!(points.iter().all(|point| point.y == 1.0
                        && (0.0..=3.0).contains(&point.x)
                        && (0.0..=1.0).contains(&point.z)));
                    area += (points[1] - points[0])
                        .cross(points[2] - points[0])
                        .length()
                        * 0.5;
                }
                assert!(
                    (area - 3.0).abs() < 1e-6,
                    "no missing or duplicated seam area"
                );
            }
        }
    }
    #[test]
    fn equal_colors_stay_greedy_and_sharp_changes_never_blend_or_open_seams() {
        for side in 0..6 {
            let face = BlockFace {
                minimum: [0; 3],
                width: 16,
                height: 16,
                u_axis: (side / 2 + 1) % 3,
                v_axis: (side / 2 + 2) % 3,
                side,
                material: 103,
                occupied_neighbor: false,
                edge_mask: 15,
            };
            let tints: HashMap<_, _> = (0..16)
                .flat_map(|u| {
                    (0..16).map(move |v| {
                        let mut cell = [0; 3];
                        cell[face.u_axis] = u;
                        cell[face.v_axis] = v;
                        (cell, if u < 8 { [64, 128, 192] } else { [240, 80, 32] })
                    })
                })
                .collect();
            let parts = split(face, &tints);
            assert_eq!(parts.iter().map(|p| p.width * p.height).sum::<u32>(), 256);
            for part in &parts {
                let first = tints[&part.minimum.map(|v| v as usize)];
                for u in 0..part.width {
                    for v in 0..part.height {
                        let mut cell = part.minimum.map(|v| v as usize);
                        cell[face.u_axis] += u as usize;
                        cell[face.v_axis] += v as usize;
                        assert_eq!(tints[&cell], first);
                    }
                }
                assert_eq!(part.side, side);
                assert_eq!(part.edge_mask, face.edge_mask);
            }
            assert!(parts.len() <= 4);
            assert_eq!(
                split(face, &tints.keys().map(|&k| (k, [64, 128, 192])).collect()).len(),
                1
            );
        }
    }
}
