use super::*;
use broadleaves::{RELIEF, template};

#[test]
fn individual_blades_cull_hidden_sides_keep_complete_motif_cells_and_fixed_budgets() {
    for index in 0..1536 {
        let triangles = template(index);
        let exposure = (index / 4) % 64;
        let middle = !(index / 256).is_multiple_of(2);
        let triangles_per_group = if middle { 2 } else { 6 };
        assert_eq!(
            triangles.len(),
            exposure.count_ones() as usize * 14 * triangles_per_group
        );
        assert!(triangles.len() <= if middle { 168 } else { 504 });
        for t in triangles {
            assert_ne!(exposure & (1 << t.side), 0);
            let cross = (t.points[1] - t.points[0]).cross(t.points[2] - t.points[0]);
            assert!(cross.dot(t.normal) > 1e-7);
            assert!(t.normal.is_finite());
            for (point, uv) in t.points.iter().zip(t.uvs) {
                assert!(point.is_finite());
                assert!(point.cmpge(Vec3::splat(-RELIEF - 1e-6)).all());
                assert!(point.cmple(Vec3::splat(1.0 + RELIEF + 1e-6)).all());
                assert!((0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1]));
            }
        }
        for (group_index, group) in triangles.chunks_exact(triangles_per_group).enumerate() {
            for axis in 0..2 {
                let lo = group
                    .iter()
                    .flat_map(|t| t.uvs)
                    .map(|uv| uv[axis])
                    .fold(f32::INFINITY, f32::min);
                let hi = group
                    .iter()
                    .flat_map(|t| t.uvs)
                    .map(|uv| uv[axis])
                    .fold(f32::NEG_INFINITY, f32::max);
                assert_eq!(hi - lo, 0.5, "one complete botanical motif cell");
                assert!(lo == 0.0 || lo == 0.5, "cell borders never cut a blade");
                if axis == 1 {
                    assert_eq!(
                        lo,
                        if group_index % 14 >= 12 { 0.5 } else { 0.0 },
                        "raised blossoms and fruits have separate motif cells from blades"
                    );
                }
            }
        }
        assert!(std::ptr::eq(triangles, template(index)));
    }
}

#[test]
fn individually_folded_blades_and_raised_blossoms_have_real_depth_without_a_cube_shell() {
    for family in [0, 512, 1024] {
        for variant in 0..4 {
            for side in 0_usize..6 {
                let near = template(family + (1 << side) * 4 + variant);
                let middle = template(family + 256 + (1 << side) * 4 + variant);
                assert_eq!(near.len(), 84);
                assert_eq!(middle.len(), 28);
                // Twelve blades and two raised motifs, no cube/backing faces.
                let mut normals = std::collections::BTreeSet::new();
                let mut min_depth = f32::INFINITY;
                let mut max_depth = f32::NEG_INFINITY;
                let mut overhang = 0.0_f32;
                for t in near {
                    normals.insert(t.normal.to_array().map(f32::to_bits));
                    for p in t.points {
                        let depth = if side.is_multiple_of(2) {
                            -p[side / 2]
                        } else {
                            p[side / 2] - 1.0
                        };
                        min_depth = min_depth.min(depth);
                        max_depth = max_depth.max(depth);
                        for axis in (0..3).filter(|axis| *axis != side / 2) {
                            overhang = overhang.max((-p[axis]).max(p[axis] - 1.0));
                        }
                    }
                }
                assert!(
                    min_depth > -0.25 && min_depth < 0.0 && max_depth > 0.19,
                    "folded blades return into the crown and the separate blossom rises above them"
                );
                assert!(
                    max_depth - min_depth > 0.20,
                    "the groups must occupy depth, not a single face plane"
                );
                assert!(
                    overhang > 0.12,
                    "groups project past neighboring face edges"
                );
                assert!(normals.len() >= 12, "independent folds and inclinations");
                // Both LODs keep all fourteen whole motifs and their tile
                // identity. The middle card drops the convex midrib geometry.
                for (near_group, middle_group) in near.chunks_exact(6).zip(middle.chunks_exact(2)) {
                    let uv_centre = |group: &[broadleaves::Triangle]| {
                        let uvs: Vec<_> = group.iter().flat_map(|t| t.uvs).collect();
                        Vec2::from_array(
                            uvs.iter()
                                .copied()
                                .fold([f32::INFINITY; 2], |a, b| [a[0].min(b[0]), a[1].min(b[1])]),
                        ) + Vec2::splat(0.25)
                    };
                    assert_eq!(uv_centre(near_group), uv_centre(middle_group));
                }
            }
        }
    }
}

#[test]
fn actual_section_mesher_keeps_leaf_owner_ranges_for_side_and_top_slots() {
    use super::super::builder::{RenderOptions, compose_section_with_options};
    use crate::blocks::{TemplateSet, definitions::BlockRenderKind};
    use terra_voxel::meshing::BlockFace;
    for material in [7, 8, 9, 132, 133, 195, 310, 311, 312, 406] {
        let block = BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [70, 70, 70, 71, 70, 70],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        let templates = TemplateSet {
            blocks: [(material, block)].into(),
            ..default()
        };
        let faces: Vec<_> = (0..6)
            .map(|side| BlockFace {
                edge_mask: 15,
                occupied_neighbor: false,
                minimum: [15; 3],
                width: 1,
                height: 1,
                u_axis: (side / 2 + 1) % 3,
                v_axis: (side / 2 + 2) % 3,
                side,
                material,
            })
            .collect();
        for level in [DetailLevel::Near, DetailLevel::Middle] {
            let options = RenderOptions {
                water_level: None,
                detail_level: level,
                terrain: None,
                exposure: None,
                tints: default(),
                conditions: default(),
            };
            let batches = compose_section_with_options(
                &faces,
                &templates,
                terra_voxel::SectionPos::new(-1, 0, -1),
                &options,
            )
            .unwrap();
            assert_eq!(batches.len(), 2);
            for batch in batches {
                assert_eq!(batch.foliage_owners.len(), 1);
                let owner = &batch.foliage_owners[0];
                assert_eq!(owner.cell, [15; 3]);
                assert_eq!(owner.material, material);
                assert_eq!(owner.vertices, 0..batch.positions.len());
                assert_eq!(batch.positions.len(), batch.normals.len());
                assert_eq!(batch.positions.len(), batch.uvs.len());
                assert!(
                    batch
                        .positions
                        .iter()
                        .flatten()
                        .all(|v| (14.89..=16.11).contains(v))
                );
                assert!(batch.colors.iter().all(|c| *c == [1.; 4]));
                for triangle in batch.indices.chunks_exact(3) {
                    let [a, b, c] = std::array::from_fn(|i| {
                        Vec3::from_array(batch.positions[triangle[i] as usize])
                    });
                    assert!(
                        (b - a)
                            .cross(c - a)
                            .dot(Vec3::from_array(batch.normals[triangle[0] as usize]))
                            > 0.
                    );
                }
            }
        }
    }
}

#[test]
fn cluster_batches_preserve_cell_ownership_wind_bounds_and_far_fallback() {
    use super::super::builder::{RenderOptions, compose_section_with_options};
    use crate::blocks::{FoliageClusterMaterials, TemplateSet, definitions::BlockRenderKind};
    use terra_voxel::meshing::BlockFace;
    let mut templates = TemplateSet::default();
    let face_materials = [70, 70, 70, 71, 70, 70];
    templates.blocks.insert(
        7,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::Cube,
            bevel_width: 0.0,
            faces: crate::blocks::cube_faces(0.0, face_materials, None),
            face_materials,
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        },
    );
    templates.foliage_clusters.insert(
        7,
        FoliageClusterMaterials {
            leaves: 72,
            needles: Some(73),
        },
    );
    let faces: Vec<_> = (0..6)
        .map(|side| BlockFace {
            edge_mask: 15,
            occupied_neighbor: false,
            minimum: [15; 3],
            width: 1,
            height: 1,
            u_axis: (side / 2 + 1) % 3,
            v_axis: (side / 2 + 2) % 3,
            side,
            material: 7,
        })
        .collect();
    for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        for quality in [
            terra_core::BlockDetailQuality::High,
            terra_core::BlockDetailQuality::Low,
        ] {
            templates.block_details.leaf_geometry_quality = quality;
            let options = RenderOptions {
                water_level: None,
                detail_level: level,
                terrain: None,
                exposure: None,
                tints: default(),
                conditions: default(),
            };
            let batches = compose_section_with_options(
                &faces,
                &templates,
                terra_voxel::SectionPos::new(-1, 0, -1),
                &options,
            )
            .unwrap();
            let detailed =
                level != DetailLevel::Far && quality != terra_core::BlockDetailQuality::Low;
            assert_eq!(batches.iter().any(|b| b.material == 72), detailed);
            if !detailed {
                continue;
            }
            assert_eq!(
                batches.len(),
                1,
                "no full cube faces behind the new group holes"
            );
            let batch = &batches[0];
            assert_eq!(
                batch.indices.len() / 3,
                if level == DetailLevel::Near { 504 } else { 168 }
            );
            assert_eq!(
                batch.positions.len(),
                if level == DetailLevel::Near { 588 } else { 336 },
                "folded blades share a midrib and six rim vertices; middle cards use four"
            );
            assert_eq!(batch.foliage_owners.len(), 1);
            let owner = &batch.foliage_owners[0];
            assert_eq!((owner.cell, owner.material), ([15; 3], 7));
            assert_eq!(owner.vertices, 0..batch.positions.len());
            let lo = batch
                .positions
                .iter()
                .map(|p| Vec3::from_array(*p))
                .fold(Vec3::splat(f32::INFINITY), Vec3::min);
            let hi = batch
                .positions
                .iter()
                .map(|p| Vec3::from_array(*p))
                .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
            assert!(lo.cmplt(Vec3::splat(15.0)).all() && hi.cmpgt(Vec3::splat(16.0)).all());
            assert!(
                lo.cmpge(Vec3::splat(15.0 - RELIEF)).all()
                    && hi.cmple(Vec3::splat(16.0 + RELIEF)).all()
            );
            // Upload computes bounds from these real vertices before adding
            // WIND_BOUNDS; neither source-cell bounds nor flat proxy are valid.
            assert!((hi - lo).min_element() > 1.1);
            assert_eq!(batch.positions.len(), batch.normals.len());
            assert_eq!(batch.positions.len(), batch.uvs.len());
            assert!(batch.colors.iter().all(|color| *color == [1.0; 4]));
            for triangle in batch.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| {
                    Vec3::from_array(batch.positions[triangle[i] as usize])
                });
                assert!(
                    (b - a)
                        .cross(c - a)
                        .dot(Vec3::from_array(batch.normals[triangle[0] as usize]))
                        > 0.0
                );
            }
        }
    }
    let block = &templates.blocks[&7];
    let mut batches = BTreeMap::new();
    append_crown(
        &mut batches,
        block,
        [-1; 3],
        [15; 3],
        63,
        DetailLevel::Near,
        CrownStyle {
            material: 7,
            species: Some(TreeAppearance::Spruce),
            cluster: templates.foliage_clusters.get(&7).copied(),
        },
    )
    .unwrap();
    assert_eq!(batches.len(), 1);
    assert!(
        batches.contains_key(&73),
        "spruce uses the needle artwork instead of the oak sheet/core"
    );
}
