use crate::meshing::{
    builder::{MeshBatch, RenderOptions, compose_section_with_options},
    custom::ModelSnapshot,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use terra_voxel::{
    SectionPos,
    meshing::{BlockFace, BlockMesh},
};
use terra_world::tree_felling::{Collapse, CollapseSection};

pub(super) struct Snapshot {
    pub position: SectionPos,
    pub geometry: Arc<BlockMesh>,
    pub models: Option<Arc<ModelSnapshot>>,
    pub options: RenderOptions,
    pub protected: Box<[bool; 4096]>,
}

pub(super) struct Geometry {
    pub terrain: Vec<(SectionPos, Vec<MeshBatch>)>,
    pub pieces: Vec<(usize, bool, Vec<MeshBatch>)>,
    pub band_height: f32,
    pub hell: Vec<super::hell::Batch>,
}

pub(super) fn prepare(
    event: &Collapse,
    snapshots: &[Snapshot],
    templates: &crate::blocks::TemplateSet,
    include_pieces: bool,
) -> Result<Geometry, String> {
    if !event.valid() {
        return Err("Invalid falling tree snapshot".into());
    }
    let pivot_origin = event.visual_origin();
    let mut cells = HashMap::new();
    for section in &event.sections {
        for (index, wood) in section.cells() {
            if let Some(position) = event.cell_position(section.position, index) {
                cells.insert(position, wood);
            }
        }
    }
    let height = cells
        .keys()
        .map(|p| p[1] - pivot_origin[1] + 1)
        .max()
        .unwrap_or(1) as f32;
    let band_height = (height / 5.0).max(1.0);
    let mut grouped = BTreeMap::<(usize, bool, usize), MeshBatch>::new();
    let mut terrain = Vec::new();
    for section in &event.sections {
        let mut mask = mask(section);
        if let Some(snapshot) = snapshots.iter().find(|s| s.position == section.position) {
            for (cell, protect) in mask.iter_mut().zip(snapshot.protected.iter()) {
                *cell &= !protect;
            }
            let geometry = BlockMesh {
                faces: snapshot
                    .geometry
                    .faces
                    .iter()
                    .flat_map(|face| {
                        if face.material == event.wood_material
                            || face.material == event.leaves_material
                        {
                            without_cells(std::slice::from_ref(face), &mask)
                        } else {
                            vec![*face]
                        }
                    })
                    .collect(),
                mineral_growth: snapshot.geometry.mineral_growth.clone(),
                segments: snapshot.geometry.segments.clone(),
                ..Default::default()
            };
            let batches = if let Some(models) = &snapshot.models {
                crate::meshing::custom::compose_without_cells(
                    &geometry,
                    models,
                    templates,
                    section.position,
                    &snapshot.options,
                    &mask,
                )?
            } else {
                crate::meshing::builder::compose_section_with_segments(
                    &geometry.faces,
                    templates,
                    section.position,
                    &snapshot.options,
                    &geometry.mineral_growth,
                    &geometry.segments,
                )?
            };
            terrain.push((section.position, batches));
        }
        let origin = section.position.origin().map_err(|e| e.to_string())?;
        // Five height bands, each with one wood and one foliage group. Geometry
        // is batched by the existing material slots, never one entity per voxel.
        for band in (0..5).filter(|_| include_pieces) {
            for wood in [true, false] {
                let material = if wood {
                    event.wood_material
                } else {
                    event.leaves_material
                };
                let mut faces = Vec::new();
                for (index, is_wood) in section.cells().filter(|(_, w)| *w == wood) {
                    let position = event
                        .cell_position(section.position, index)
                        .ok_or("Invalid tree cell")?;
                    if ((position[1] - pivot_origin[1]) as f32 / band_height)
                        .floor()
                        .clamp(0.0, 4.0) as usize
                        != band
                    {
                        continue;
                    }
                    let cell = [
                        u32::from(index % 16),
                        u32::from(index / 16 % 16),
                        u32::from(index / 256),
                    ];
                    for mut face in super::super::mining_geometry::full_faces(cell, material) {
                        let mut neighbor = position;
                        neighbor[face.side / 2] += if face.side % 2 == 0 { -1 } else { 1 };
                        // Every independently moving band remains closed at its
                        // fracture planes, including wood/foliage interfaces.
                        if cells.get(&neighbor).is_some_and(|&w| {
                            w == is_wood
                                && ((neighbor[1] - pivot_origin[1]) as f32 / band_height)
                                    .floor()
                                    .clamp(0.0, 4.0) as usize
                                    == band
                        }) {
                            continue;
                        }
                        face.occupied_neighbor = false;
                        faces.push(face);
                    }
                }
                if faces.is_empty() {
                    continue;
                }
                let options =
                    snapshots
                        .first()
                        .map(|s| s.options.clone())
                        .unwrap_or(RenderOptions {
                            water_level: None,
                            detail_level: crate::meshing::DetailLevel::Middle,
                            terrain: None,
                            exposure: None,
                            tints: Default::default(),
                            conditions: Default::default(),
                        });
                let mut batches =
                    compose_section_with_options(&faces, templates, section.position, &options)?;
                let pivot_y = (band as f32 + 0.5) * band_height;
                for batch in &mut batches {
                    for p in &mut batch.positions {
                        for axis in 0..3 {
                            p[axis] += (origin[axis] - pivot_origin[axis]) as f32
                                - if axis == 1 { pivot_y } else { 0.5 };
                        }
                    }
                }
                for batch in batches {
                    let target = grouped
                        .entry((band, wood, batch.material))
                        .or_insert_with(|| MeshBatch {
                            material: batch.material,
                            ..Default::default()
                        });
                    let offset = target.positions.len() as u32;
                    target.positions.extend(batch.positions);
                    target.normals.extend(batch.normals);
                    target.append_tangents(offset as usize, batch.tangents.into_iter());
                    target.uvs.extend(batch.uvs);
                    target.colors.extend(batch.colors);
                    target
                        .indices
                        .extend(batch.indices.into_iter().map(|i| offset + i));
                }
            }
        }
    }
    let mut pieces = BTreeMap::<(usize, bool), Vec<MeshBatch>>::new();
    for ((band, wood, _), batch) in grouped {
        pieces.entry((band, wood)).or_default().push(batch);
    }
    Ok(Geometry {
        terrain,
        pieces: pieces.into_iter().map(|((b, w), m)| (b, w, m)).collect(),
        band_height,
        hell: Vec::new(),
    })
}

pub(super) fn mask(section: &CollapseSection) -> [bool; 4096] {
    let mut result = [false; 4096];
    for (index, _) in section.cells().filter(|(i, _)| *i < 4096) {
        result[usize::from(index)] = true;
    }
    result
}

/// Subtract a set from greedy rectangles in one linear pass, keeping row runs.
pub(super) fn without_cells(faces: &[BlockFace], mask: &[bool; 4096]) -> Vec<BlockFace> {
    let mut output = Vec::new();
    for &face in faces {
        let affected = (0..face.height).any(|v| {
            (0..face.width).any(|u| {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let i = (cell[0] + 16 * (cell[1] + 16 * cell[2])) as usize;
                i < 4096 && mask[i]
            })
        });
        if !affected {
            output.push(face);
            continue;
        }
        for v in 0..face.height {
            let mut start = None;
            for u in 0..=face.width {
                let mut cell = face.minimum;
                cell[face.u_axis] += u.min(face.width.saturating_sub(1));
                cell[face.v_axis] += v;
                let index = (cell[0] + 16 * (cell[1] + 16 * cell[2])) as usize;
                let keep = u < face.width && index < 4096 && !mask[index];
                if keep && start.is_none() {
                    start = Some(u);
                }
                if !keep && let Some(first) = start.take() {
                    let mut part = face;
                    part.minimum[face.u_axis] += first;
                    part.minimum[face.v_axis] += v;
                    part.width = u - first;
                    part.height = 1;
                    output.push(part);
                }
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Collapse, crate::blocks::TemplateSet) {
        let event = Collapse {
            foot: [-1, 0, -1],
            direction: [1.0, 0.0],
            wood_material: 6,
            leaves_material: 7,
            sections: vec![CollapseSection {
                position: SectionPos::new(-1, 0, -1),
                wood: (0..5).map(|y| 15 + 16 * y + 256 * 15).collect(),
                leaves: vec![],
            }],
        };
        let mut templates = crate::blocks::TemplateSet::default();
        templates.blocks.insert(
            6,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [0; 6], None),
                face_materials: [0; 6],
                side_cap: None,
                details: vec![],
                model: vec![],
            },
        );
        (event, templates)
    }

    #[test]
    fn separated_height_bands_keep_both_fracture_caps_and_valid_batched_indices() {
        let (event, templates) = fixture();
        let result = prepare(&event, &[], &templates, true).unwrap();
        assert_eq!(result.pieces.len(), 5);
        for (_, wood, batches) in result.pieces {
            assert!(wood);
            assert_eq!(batches.len(), 1);
            let batch = &batches[0];
            for sign in [-1.0_f32, 1.0] {
                assert!(
                    batch.normals.iter().any(|n| n[1] * sign > 0.99),
                    "every moving fragment must stay closed"
                );
            }
            assert!(batch.positions.iter().flatten().all(|p| p.is_finite()));
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|i| (*i as usize) < batch.positions.len())
            );
        }
    }

    #[test]
    fn refreshing_a_tree_mask_keeps_later_player_replacements_even_of_the_same_wood() {
        let (event, templates) = fixture();
        let mut snapshots = vec![Snapshot {
            position: event.sections[0].position,
            geometry: Arc::new(BlockMesh {
                faces: super::super::super::mining_geometry::full_faces([15, 0, 15], 6),
                ..Default::default()
            }),
            models: None,
            options: RenderOptions {
                water_level: None,
                detail_level: crate::meshing::DetailLevel::Middle,
                terrain: None,
                exposure: None,
                tints: Default::default(),
                conditions: Default::default(),
            },
            protected: Box::new([false; 4096]),
        }];
        let removed = prepare(&event, &snapshots, &templates, false).unwrap();
        assert!(removed.terrain[0].1.is_empty());
        snapshots[0].protected[15 + 256 * 15] = true;
        let preserved = prepare(&event, &snapshots, &templates, false).unwrap();
        assert!(
            preserved.terrain[0]
                .1
                .iter()
                .map(|b| b.indices.len())
                .sum::<usize>()
                >= 36
        );
        assert!(
            preserved.pieces.is_empty(),
            "terrain refresh never restarts the fall animation"
        );
    }
    #[test]
    fn greedy_mask_only_removes_selected_tree_cells_not_player_neighbors() {
        let mut face = super::super::super::mining_geometry::full_faces([0, 0, 0], 6)[3];
        face.width = 16;
        face.height = 16;
        let mut mask = [false; 4096];
        mask[1] = true;
        mask[15 + 256 * 15] = true;
        let faces = without_cells(&[face], &mask);
        let area: u32 = faces.iter().map(|f| f.width * f.height).sum();
        assert_eq!(area, 254);
        assert!(
            faces
                .iter()
                .all(|f| f.material == 6 && f.width > 0 && f.height == 1)
        );
    }
}
