use crate::meshing::builder::MeshBatch;
use bevy::prelude::*;
use terra_voxel::meshing::BlockFace;

/// Restore only faces that the opaque target originally occluded. Neighbor cells
/// keep their real material and global origin, including opposite section borders.
pub(super) fn cavity_faces(
    position: [i64; 3],
    target: u16,
    neighbors: [u16; 6],
    neighbor_tints: [Option<[u8; 3]>; 6],
    templates: &crate::blocks::TemplateSet,
    options: &crate::meshing::builder::RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    if !terra_voxel::MaterialId::from_raw(target).is_some_and(|id| id.is_opaque()) {
        return Ok(Vec::new());
    }
    let (target_section, _) = terra_voxel::blocks::global_to_section(position);
    let target_origin = target_section.origin().map_err(|error| error.to_string())?;
    let mut flat = templates.clone();
    let mut result = Vec::new();
    for (side, material) in neighbors.into_iter().enumerate() {
        if !terra_voxel::MaterialId::from_raw(material).is_some_and(|id| id.is_opaque()) {
            continue;
        }
        let Some(template) = flat.blocks.get_mut(&material) else {
            continue;
        };
        template.faces = crate::blocks::cube_faces(0.0, template.face_materials, template.side_cap);
        template.surface = None;
        template.bevel_width = 0.0;
        template.details.clear();
        let mut neighbor = position;
        neighbor[side / 2] += if side % 2 == 0 { -1 } else { 1 };
        let (section, local) = terra_voxel::blocks::global_to_section(neighbor);
        let origin = section.origin().map_err(|error| error.to_string())?;
        let delta = Vec3::from_array(std::array::from_fn(|axis| {
            (origin[axis] - target_origin[axis]) as f32
        }));
        let face = full_faces(local.map(|value| value as u32), material)[side ^ 1];
        // The neighbor may live in a different section; target-local metadata
        // must never color an unrelated cell at the same wrapped coordinates.
        let mut neighbor_options = options.clone();
        neighbor_options.tints = std::sync::Arc::new(
            neighbor_tints[side]
                .map(|rgb| (local, rgb))
                .into_iter()
                .collect(),
        );
        let batches = crate::meshing::builder::compose_section_with_options(
            &[face],
            &flat,
            section,
            &neighbor_options,
        )?;
        for mut batch in batches {
            for point in &mut batch.positions {
                *point = (Vec3::from_array(*point) + delta).to_array();
            }
            result.push(batch);
        }
    }
    Ok(result)
}

/// Subtract exactly one cell from a greedy rectangle, retaining at most four rectangles.
pub(super) fn without_cell(faces: &[BlockFace], cell: [u32; 3]) -> Vec<BlockFace> {
    let mut result = Vec::with_capacity(faces.len() + 24);
    for &face in faces {
        let u = face.u_axis;
        let v = face.v_axis;
        if cell[face.side / 2] != face.minimum[face.side / 2]
            || cell[u] < face.minimum[u]
            || cell[u] >= face.minimum[u] + face.width
            || cell[v] < face.minimum[v]
            || cell[v] >= face.minimum[v] + face.height
        {
            result.push(face);
            continue;
        }
        let du = cell[u] - face.minimum[u];
        let dv = cell[v] - face.minimum[v];
        for (x, y, width, height) in [
            (0, 0, du, face.height),
            (du + 1, 0, face.width - du - 1, face.height),
            (du, 0, 1, dv),
            (du, dv + 1, 1, face.height - dv - 1),
        ] {
            if width == 0 || height == 0 {
                continue;
            }
            let mut part = face;
            part.minimum[u] += x;
            part.minimum[v] += y;
            part.width = width;
            part.height = height;
            result.push(part);
        }
    }
    result
}

pub(super) fn full_faces(cell: [u32; 3], material: u16) -> Vec<BlockFace> {
    (0..6)
        .map(|side| BlockFace {
            minimum: cell,
            width: 1,
            height: 1,
            u_axis: (side / 2 + 1) % 3,
            v_axis: (side / 2 + 2) % 3,
            side,
            material,
            occupied_neighbor: false,
            edge_mask: 15,
        })
        .collect()
}

/// Fixed edge beams overlap the closed core, sealing the cell while its chips move.
pub(super) fn rim(batches: &[MeshBatch]) -> Vec<MeshBatch> {
    batches
        .iter()
        .map(|source| {
            let mut batch = MeshBatch {
                material: source.material,
                ..default()
            };
            for axis in 0..3 {
                for a in [-1.0, 1.0] {
                    for b in [-1.0, 1.0] {
                        let mut scale = Vec3::splat(0.12);
                        scale[axis] = 1.0;
                        let mut offset = Vec3::ZERO;
                        offset[(axis + 1) % 3] = a * 0.44;
                        offset[(axis + 2) % 3] = b * 0.44;
                        let base = batch.positions.len() as u32;
                        batch.positions.extend(
                            source
                                .positions
                                .iter()
                                .map(|p| (Vec3::from_array(*p) * scale + offset).to_array()),
                        );
                        batch.normals.extend(source.normals.iter().map(|n| {
                            (Vec3::from_array(*n) / scale)
                                .normalize_or_zero()
                                .to_array()
                        }));
                        batch
                            .uvs
                            .extend(source.positions.iter().zip(&source.normals).map(
                                |(point, normal)| {
                                    let point = Vec3::from_array(*point) * scale
                                        + offset
                                        + Vec3::splat(0.5);
                                    let normal = Vec3::from_array(*normal).abs();
                                    if normal.x > 0.5 {
                                        [point.z, point.y]
                                    } else if normal.y > 0.5 {
                                        [point.x, point.z]
                                    } else {
                                        [point.x, point.y]
                                    }
                                },
                            ));
                        batch.colors.extend_from_slice(&source.colors);
                        batch
                            .indices
                            .extend(source.indices.iter().map(|i| base + i));
                    }
                }
            }
            batch
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rim_keeps_cell_texture_coordinates_instead_of_stretching_full_tiles() {
        let mut source = std::collections::BTreeMap::new();
        crate::props::add_box(&mut source, 0, Vec3::ZERO, Vec3::ONE, Quat::IDENTITY).unwrap();
        let batches = rim(&source.into_values().collect::<Vec<_>>());
        for batch in batches {
            for ((point, normal), uv) in batch.positions.iter().zip(&batch.normals).zip(&batch.uvs)
            {
                if normal[1].abs() > 0.5 {
                    assert!((uv[0] - point[0] - 0.5).abs() < 1e-6);
                    assert!((uv[1] - point[2] - 0.5).abs() < 1e-6);
                }
            }
        }
    }
    #[test]
    fn greedy_cell_subtraction_preserves_every_other_face_without_overlap() {
        for side in 0..6 {
            for x in 0..16 {
                for y in 0..16 {
                    let u = (side / 2 + 1) % 3;
                    let v = (side / 2 + 2) % 3;
                    let mut face = full_faces([0; 3], 1)[side];
                    face.width = 16;
                    face.height = 16;
                    let mut cell = [0; 3];
                    cell[u] = x;
                    cell[v] = y;
                    let parts = without_cell(&[face], cell);
                    assert!(parts.len() <= 4);
                    let mut coverage = [[0; 16]; 16];
                    for part in parts {
                        for a in part.minimum[u]..part.minimum[u] + part.width {
                            for b in part.minimum[v]..part.minimum[v] + part.height {
                                coverage[a as usize][b as usize] += 1;
                            }
                        }
                    }
                    for (a, row) in coverage.iter().enumerate() {
                        for (b, &count) in row.iter().enumerate() {
                            assert_eq!(count, i32::from(a != x as usize || b != y as usize));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cavity_faces_seal_center_and_grazing_rays_across_negative_section_borders() {
        let mut templates = crate::blocks::TemplateSet::default();
        templates.blocks.insert(
            3,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                bevel_width: 0.04,
                faces: crate::blocks::cube_faces(0.04, [0, 1, 2, 3, 4, 5], None),
                face_materials: [0, 1, 2, 3, 4, 5],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        let options = crate::meshing::builder::RenderOptions {
            water_level: None,
            detail_level: crate::meshing::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: default(),
        };
        for position in [[-1, -16, 15], [0; 3], [15; 3]] {
            let batches =
                cavity_faces(position, 3, [3; 6], [None; 6], &templates, &options).unwrap();
            assert_eq!(batches.len(), 6);
            let center = Vec3::from_array(position.map(|v| v.rem_euclid(16) as f32 + 0.5));
            for side in 0..6 {
                let axis = side / 2;
                let mut direction = Vec3::ZERO;
                direction[axis] = if side % 2 == 0 { -1.0 } else { 1.0 };
                let batch = batches
                    .iter()
                    .find(|batch| batch.material == (side ^ 1))
                    .unwrap();
                for u in [-0.499, 0.0, 0.499] {
                    for v in [-0.499, 0.0, 0.499] {
                        let mut hit = center + direction * 0.5;
                        hit[(axis + 1) % 3] += u;
                        hit[(axis + 2) % 3] += v;
                        let covered = batch.indices.chunks_exact(3).any(|tri| {
                            let [a, b, c] = [tri[0], tri[1], tri[2]]
                                .map(|index| Vec3::from_array(batch.positions[index as usize]));
                            let normal = (b - a).cross(c - a).normalize();
                            normal.dot(direction) < -0.999
                                && (hit - a).dot(normal).abs() < 1e-5
                                && [(a, b), (b, c), (c, a)]
                                    .iter()
                                    .all(|(p, q)| (q - p).cross(hit - p).dot(normal) >= -1e-6)
                        });
                        assert!(covered, "unsealed {position:?} side {side} sample {u}/{v}");
                    }
                }
            }
        }
        assert!(
            cavity_faces(
                [0; 3],
                terra_voxel::MaterialId::Glass as u16,
                [3; 6],
                [None; 6],
                &templates,
                &options
            )
            .unwrap()
            .is_empty()
        );
        let mut wool = templates.blocks[&3].clone();
        wool.face_materials = [0; 6];
        wool.faces = crate::blocks::cube_faces(0.0, [0; 6], None);
        templates.blocks.insert(103, wool);
        let mut colored = options.clone();
        colored.tints = std::sync::Arc::new([([0; 3], [255, 0, 0])].into());
        let neighbor_tints = [
            Some([71, 93, 184]),
            None,
            Some([237, 92, 31]),
            None,
            Some([20, 200, 80]),
            None,
        ];
        for position in [[-1, -16, 15], [0; 3], [15; 3]] {
            let batches = cavity_faces(
                position,
                103,
                [103; 6],
                neighbor_tints,
                &templates,
                &colored,
            )
            .unwrap();
            assert_eq!(batches.len(), 6);
            for (side, batch) in batches.iter().enumerate() {
                let expected = neighbor_tints[side].map_or([1.0; 4], crate::dye_tint::rgb);
                for index in 0..batch.positions.len() {
                    assert_eq!(
                        batch.colors.get(index).copied().unwrap_or([1.0; 4]),
                        expected,
                        "neighbor RGB must use its own section coordinates"
                    );
                }
            }
        }
    }
}
