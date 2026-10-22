//! Static provenance tags let the existing leaf shader shed natural crowns.
use super::builder::{MeshBatch, condition_faces};
use crate::blocks::TemplateSet;
use terra_voxel::meshing::BlockFace;
use terra_world::generation::{TreeAppearance, sections::BlockOverrides};

/// Color and shadow quads share species, player-provenance and snow boundaries.
pub(super) fn split_edited_faces(
    faces: &[BlockFace],
    origin: [i64; 3],
    edits: &BlockOverrides,
    exposure: Option<&crate::weather::surfaces::exposure::Exposure>,
    tree: impl Fn([i64; 3], u16) -> Option<TreeAppearance>,
) -> Option<Vec<BlockFace>> {
    let mut result: Option<Vec<BlockFace>> = None;
    for (face_index, &face) in faces.iter().enumerate() {
        if !matches!(face.material, 7..=9) {
            if let Some(result) = &mut result {
                result.push(face);
            }
            continue;
        }
        let mut grid = [(false, false, None); 256];
        let mut different = false;
        for v in 0..face.height {
            for u in 0..face.width {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
                let index = (u + v * face.width) as usize;
                let edited = edits.contains_key(&global);
                grid[index] = (
                    edited,
                    exposure.is_some_and(|exposure| exposure.snow_exposed(cell, origin[1])),
                    if edited {
                        None
                    } else {
                        tree(global, face.material)
                    },
                );
                different |= grid[index] != grid[0];
            }
        }
        if different {
            let result = result.get_or_insert_with(|| faces[..face_index].to_vec());
            result.extend(condition_faces::partition(face, &grid));
        } else if let Some(result) = &mut result {
            result.push(face);
        }
    }
    result
}

fn deciduous(kind: Option<TreeAppearance>, edited: bool) -> bool {
    !edited
        && matches!(
            kind,
            Some(TreeAppearance::Oak | TreeAppearance::Autumn | TreeAppearance::Birch)
        )
}

fn alpha_tag(global: [i64; 3]) -> f32 {
    let ticket = super::builder::variation(global, 0x0053_4845_4444);
    let seed = (ticket >> 40) as f32 / 16_777_216.0;
    0.125 + seed * 0.25
}

pub(super) fn annotate(
    batches: &mut [MeshBatch],
    _templates: &TemplateSet,
    origin: [i64; 3],
    edits: &BlockOverrides,
    tree: impl Fn([i64; 3], u16) -> Option<TreeAppearance>,
    temperature: impl Fn([f32; 3]) -> f32,
    snow_exposed: impl Fn([u32; 3]) -> bool,
) {
    // Crown micro-quads and shadow shells share one provenance lookup per cell.
    let mut tags = [0.0_f32; 3 * 4096];
    for batch in batches {
        if batch.foliage_owners.is_empty() {
            continue;
        }
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        for owner in &batch.foliage_owners {
            let cell = owner.cell;
            if !matches!(owner.material, 7..=9) || cell.iter().any(|&coordinate| coordinate >= 16) {
                continue;
            }
            let global = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
            let index = (owner.material as usize - 7) * 4096
                + cell[0] as usize
                + 16 * (cell[1] as usize + 16 * cell[2] as usize);
            if tags[index] == 0.0 {
                tags[index] = if edits.contains_key(&global) {
                    1.0
                } else {
                    let kind = tree(global, owner.material);
                    if deciduous(kind, false) {
                        alpha_tag(global)
                    } else if kind == Some(TreeAppearance::Spruce) && snow_exposed(cell) {
                        let center = cell.map(|coordinate| coordinate as f32 + 0.5);
                        0.5 + crate::weather::storage::snow::encode_temperature(temperature(center))
                            * 0.49
                    } else {
                        1.0
                    }
                };
            }
            if let Some(colors) = batch.colors.get_mut(owner.vertices.clone()) {
                for color in colors {
                    color[3] = tags[index];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};

    #[test]
    fn deciduous_crowns_shed_but_player_leaves_and_evergreens_are_protected() {
        for kind in [
            TreeAppearance::Oak,
            TreeAppearance::Autumn,
            TreeAppearance::Birch,
        ] {
            assert!(deciduous(Some(kind), false));
            assert!(!deciduous(Some(kind), true));
        }
        for kind in [
            None,
            Some(TreeAppearance::Spruce),
            Some(TreeAppearance::Palm),
            Some(TreeAppearance::Mushroom),
        ] {
            assert!(!deciduous(kind, false));
        }
    }

    #[test]
    fn micro_faces_keep_source_material_and_owner_beyond_cell_and_section_boundaries() {
        use super::super::builder::FoliageOwner;
        let origin = [-16, -16, -16];
        // Independent patches may cross a source-cell boundary or have an
        // interior normal. Neither their center nor the shared slot is provenance.
        let owners = [([0; 3], 7), ([15; 3], 9), ([1, 0, 0], 8), ([2, 0, 0], 7)];
        let mut batch = MeshBatch {
            material: 42,
            ..Default::default()
        };
        for (cell, material) in owners {
            let start = batch.positions.len();
            batch.positions.extend([[16.125, -0.125, 16.125]; 4]);
            batch.normals.extend([[0.0, 1.0, 0.0]; 4]);
            batch.foliage_owners.push(FoliageOwner {
                vertices: start..start + 4,
                cell,
                material,
            });
        }
        let edits = [([-15, -16, -16], 8)].into();
        let mut batches = [batch];
        let queries = std::cell::Cell::new(0);
        annotate(
            &mut batches,
            &TemplateSet::default(),
            origin,
            &edits,
            |global, material| {
                queries.set(queries.get() + 1);
                match (global, material) {
                    ([-16, -16, -16], 7) => Some(TreeAppearance::Oak),
                    ([-1, -1, -1], 9) => Some(TreeAppearance::Birch),
                    ([-14, -16, -16], 7) => Some(TreeAppearance::Spruce),
                    _ => panic!(
                        "sampled geometry instead of its original cell: {global:?}/{material}"
                    ),
                }
            },
            |_| -6.0,
            |_| true,
        );
        assert_eq!(
            queries.get(),
            3,
            "player provenance never samples natural trees"
        );
        for colors in batches[0].colors[..8].chunks_exact(4) {
            assert!(colors.iter().all(|color| color[3] == colors[0][3]));
            assert!((0.125..0.375).contains(&colors[0][3]));
            // At full winter loss every micro face, not merely its broad shell, is discarded.
            let seed = (colors[0][3] - 0.125) * 4.0;
            assert!(seed * 0.84 + 0.16 <= 1.0);
        }
        assert!(batches[0].colors[8..12].iter().all(|color| color[3] == 1.0));
        assert!(
            batches[0].colors[12..]
                .iter()
                .all(|color| (0.5..0.999).contains(&color[3]))
        );
    }

    #[test]
    fn edited_natural_leaf_is_partitioned_without_changing_geometry_coverage() {
        let face = BlockFace {
            minimum: [0, 1, 0],
            width: 3,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 7,
            occupied_neighbor: false,
            edge_mask: 15,
        };
        let faces = split_edited_faces(
            &[face],
            [-16, 0, -16],
            &[([-15, 1, -16], 7)].into(),
            None,
            |_, _| Some(TreeAppearance::Oak),
        )
        .unwrap();
        assert_eq!(faces.len(), 3);
        assert_eq!(
            faces
                .iter()
                .map(|part| part.width * part.height)
                .sum::<u32>(),
            3
        );
        assert!(faces.iter().all(|part| part.material == 7));
    }

    #[test]
    fn crown_and_shadow_tags_preserve_wood_branches_and_exclude_placed_replacements() {
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            7,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [1; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        let leaf = || MeshBatch {
            material: 1,
            foliage_owners: vec![super::super::builder::FoliageOwner {
                vertices: 0..4,
                cell: [0, 1, 0],
                material: 7,
            }],
            positions: vec![
                [0.0, 2.0, 0.0],
                [1.0, 2.0, 0.0],
                [1.0, 2.0, 1.0],
                [0.0, 2.0, 1.0],
            ],
            normals: vec![[0.0, 1.0, 0.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Default::default()
        };
        let wood = || MeshBatch {
            material: 2,
            positions: vec![[0.5, 1.5, 0.5]; 4],
            ..Default::default()
        };
        let mut batches = vec![
            leaf(),
            MeshBatch {
                shadow_only: true,
                ..leaf()
            },
            wood(),
        ];
        let lookups = std::cell::Cell::new(0);
        annotate(
            &mut batches,
            &templates,
            [-16, 0, -16],
            &Default::default(),
            |_, _| {
                lookups.set(lookups.get() + 1);
                Some(TreeAppearance::Oak)
            },
            |_| -6.0,
            |_| true,
        );
        assert_eq!(
            lookups.get(),
            1,
            "color and shadow quads share one provenance lookup"
        );
        assert!(
            batches[0]
                .colors
                .iter()
                .all(|color| (0.125..0.375).contains(&color[3]))
        );
        assert_eq!(batches[0].colors, batches[1].colors);
        assert_eq!(batches[2], wood());
        assert_eq!(batches[0].positions, leaf().positions);
        let edits = [([-16, 1, -16], 7)].into();
        annotate(
            &mut batches,
            &templates,
            [-16, 0, -16],
            &edits,
            |_, _| Some(TreeAppearance::Oak),
            |_| -6.0,
            |_| true,
        );
        assert!(batches[0].colors.iter().all(|color| color[3] == 1.0));
        assert_eq!(batches[2], wood());
        annotate(
            &mut batches,
            &templates,
            [-16, 0, -16],
            &Default::default(),
            |_, _| Some(TreeAppearance::Spruce),
            |_| -6.0,
            |_| true,
        );
        assert!(
            batches[0]
                .colors
                .iter()
                .all(|color| (0.5..0.999).contains(&color[3]))
        );
        assert_eq!(batches[0].colors, batches[1].colors);
        assert_eq!(batches[2], wood());
        annotate(
            &mut batches,
            &templates,
            [-16, 0, -16],
            &Default::default(),
            |_, _| Some(TreeAppearance::Spruce),
            |_| -6.0,
            |_| false,
        );
        assert!(batches[0].colors.iter().all(|color| color[3] == 1.0));
        assert_eq!(batches[0].colors, batches[1].colors);
        assert_eq!(batches[2], wood());
    }

    #[test]
    fn adjacent_oak_and_spruce_do_not_share_a_seasonal_shadow_quad() {
        let face = BlockFace {
            minimum: [0, 1, 0],
            width: 2,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 7,
            occupied_neighbor: false,
            edge_mask: 15,
        };
        let kind = |global: [i64; 3], _: u16| {
            Some(if global[0] == 0 {
                TreeAppearance::Oak
            } else {
                TreeAppearance::Spruce
            })
        };
        let parts = split_edited_faces(&[face], [0; 3], &Default::default(), None, kind).unwrap();
        assert_eq!(parts.len(), 2);
        assert!(
            parts
                .iter()
                .all(|part| part.width == 1 && part.material == 7)
        );
        assert_ne!(
            kind(parts[0].minimum.map(i64::from), 7),
            kind(parts[1].minimum.map(i64::from), 7)
        );
    }

    #[test]
    fn far_crowns_and_shadow_quads_keep_open_roofed_and_submerged_snow_tags_separate() {
        use crate::weather::surfaces::exposure::Exposure;
        let face = BlockFace {
            minimum: [0, 1, 0],
            width: 3,
            height: 1,
            u_axis: 0,
            v_axis: 2,
            side: 3,
            material: 7,
            occupied_neighbor: false,
            edge_mask: 15,
        };
        let mut exposure = Exposure {
            natural: None,
            roofs: [1; 256],
            snow_roofs: [1; 256],
            coast: [0.0; 256],
            coast_vertices: [0.0; 289],
            water: [f32::NEG_INFINITY; 256],
        };
        assert!(
            split_edited_faces(
                &[face],
                [0; 3],
                &Default::default(),
                Some(&exposure),
                |_, _| Some(TreeAppearance::Spruce)
            )
            .is_none(),
            "uniform exposed crowns keep the cached greedy geometry"
        );
        exposure.snow_roofs[1] = 4;
        exposure.water[2] = 3.0;
        let faces = split_edited_faces(
            &[face],
            [0; 3],
            &Default::default(),
            Some(&exposure),
            |_, _| Some(TreeAppearance::Spruce),
        )
        .unwrap();
        assert_eq!(faces.len(), 2);
        assert_eq!(
            faces
                .iter()
                .map(|part| part.width * part.height)
                .sum::<u32>(),
            3
        );
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            7,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [1; 6],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        let mut batches = Vec::new();
        for shadow in [false, true] {
            let mut material_batches = std::collections::BTreeMap::new();
            for &part in &faces {
                super::super::builder::append_greedy(
                    &mut material_batches,
                    part,
                    &templates.blocks[&7],
                )
                .unwrap();
            }
            for mut batch in material_batches.into_values() {
                batch.shadow_only = shadow;
                batches.push(batch);
            }
        }
        annotate(
            &mut batches,
            &templates,
            [0; 3],
            &Default::default(),
            |_, _| Some(TreeAppearance::Spruce),
            |_| -6.0,
            |cell| exposure.snow_exposed(cell, 0),
        );
        assert_eq!(batches[0].colors, batches[1].colors);
        assert!(
            batches[0].colors[..4]
                .iter()
                .all(|color| (0.5..0.999).contains(&color[3]))
        );
        assert!(batches[0].colors[4..].iter().all(|color| color[3] == 1.0));
        exposure.snow_roofs[1] = 1;
        exposure.water[2] = f32::NEG_INFINITY;
        assert!(
            split_edited_faces(
                &[face],
                [0; 3],
                &Default::default(),
                Some(&exposure),
                |_, _| Some(TreeAppearance::Spruce)
            )
            .is_none()
        );
        annotate(
            &mut batches,
            &templates,
            [0; 3],
            &Default::default(),
            |_, _| Some(TreeAppearance::Spruce),
            |_| -6.0,
            |cell| exposure.snow_exposed(cell, 0),
        );
        assert!(
            batches[0]
                .colors
                .iter()
                .all(|color| (0.5..0.999).contains(&color[3])),
            "removing a roof or draining flood water restores cap eligibility"
        );
    }
}
