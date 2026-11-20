use super::*;
use crate::{meshing::staging::StagedMesh, props::static_layout::strip_identity_growth};
use bevy::{camera::primitives::MeshAabb, mesh::MeshVertexBufferLayouts};

fn identity_batch() -> MeshBatch {
    MeshBatch {
        positions: vec![[0.0, 0.0, 0.0], [0.25, 0.5, 0.75], [1.0, 0.0, 1.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        uvs: vec![[0.5, 0.75]; 3],
        colors: vec![[1.25, 0.5, 0.25, 1.0]; 3],
        rooted_wind: vec![[0.0, 1.0]; 3],
        growth_anchors: vec![[0.0, 0.0, 0.0]; 3],
        indices: vec![2, 1, 0],
        ..Default::default()
    }
}

#[test]
fn static_default_pose_is_identity_and_only_growth_allocations_are_released() {
    let mut batch = identity_batch();
    for (&position, &anchor) in batch.positions.iter().zip(&batch.growth_anchors) {
        for time in [0.0_f32, 1.2, 35.5, 86_400.0] {
            let factor = growth::Pose {
                ratio: 1.0,
                ..Default::default()
            }
            .factor(time);
            let transformed: [f32; 3] =
                std::array::from_fn(|axis| anchor[axis] + (position[axis] - anchor[axis]) * factor);
            assert_eq!(transformed, position);
        }
    }
    let before = crate::upload_budget::batch_bytes(&batch);
    strip_identity_growth(&mut batch);
    assert_eq!(crate::upload_budget::batch_bytes(&batch), before - 3 * 20);
    assert_eq!(batch.rooted_wind.capacity(), 0);
    assert_eq!(batch.growth_anchors.capacity(), 0);
    let mut expected = identity_batch();
    expected.rooted_wind = Vec::new();
    expected.growth_anchors = Vec::new();
    assert_eq!(batch, expected);
    strip_identity_growth(&mut batch);
    assert_eq!(batch, expected);
}

#[test]
fn unsafe_incomplete_or_animated_payloads_are_kept_unchanged() {
    let cases: [fn(&mut MeshBatch); 9] = [
        |batch| {
            batch
                .growth_ranges
                .push((terra_world::props::PropId(7), 0..3))
        },
        |batch| batch.colors[0][3] = 0.5,
        |batch| batch.rooted_wind[0] = [0.0, 0.75],
        |batch| batch.rooted_wind[0] = [1.0, 1.0],
        |batch| {
            batch.rooted_wind.pop();
        },
        |batch| {
            batch.growth_anchors.pop();
        },
        |batch| {
            batch.colors.pop();
        },
        |batch| batch.packed_colors = vec![[255; 4]; 3],
        |batch| {
            batch.staged = Some(StagedMesh {
                handle: Handle::default(),
                liquid: None,
                bounds: None,
                rooted: false,
                damp: false,
                opaque_shadow: false,
                coastal_quads: 0,
                far: None,
            })
        },
    ];
    for change in cases {
        let mut batch = identity_batch();
        let mut expected = identity_batch();
        change(&mut batch);
        change(&mut expected);
        strip_identity_growth(&mut batch);
        assert_eq!(batch, expected);
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for anchor in [false, true] {
            let mut batch = identity_batch();
            if anchor {
                batch.growth_anchors[0][0] = invalid;
            } else {
                batch.positions[0][0] = invalid;
            }
            strip_identity_growth(&mut batch);
            assert_eq!(batch.rooted_wind.len(), 3);
            assert_eq!(batch.growth_anchors.len(), 3);
        }
    }
}

#[test]
fn real_prop_preparation_removes_only_identity_growth_attributes_for_both_color_layouts() {
    for packed in [false, true] {
        let mut control = identity_batch();
        let mut candidate = identity_batch();
        if packed {
            crate::foliage::packing::pack(&mut control);
            crate::foliage::packing::pack(&mut candidate);
        }
        strip_identity_growth(&mut candidate);
        let mut meshes = Assets::<Mesh>::default();
        prepare_prop_batch(&mut control, &mut meshes);
        prepare_prop_batch(&mut candidate, &mut meshes);
        let before = meshes
            .get(&control.staged.as_ref().unwrap().handle)
            .unwrap();
        let after = meshes
            .get(&candidate.staged.as_ref().unwrap().handle)
            .unwrap();
        assert_eq!(after.count_vertices(), before.count_vertices());
        assert_eq!(after.indices(), before.indices());
        assert_eq!(after.get_aabb(), before.get_aabb());
        assert_eq!(
            candidate.staged.as_ref().unwrap().bounds,
            control.staged.as_ref().unwrap().bounds
        );
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ] {
            assert_eq!(after.attribute(attribute), before.attribute(attribute));
        }
        assert!(before.contains_attribute(Mesh::ATTRIBUTE_UV_1));
        assert!(before.contains_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR));
        assert!(!after.contains_attribute(Mesh::ATTRIBUTE_UV_1));
        assert!(!after.contains_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR));
        assert_eq!(
            after.get_vertex_buffer_size(),
            before.get_vertex_buffer_size() - 3 * 20
        );
        let layout = after.get_mesh_vertex_buffer_layout(&mut MeshVertexBufferLayouts::default());
        assert!(!layout.0.contains(wind::ATTRIBUTE_GROWTH_ANCHOR));
        assert!(
            layout
                .0
                .get_layout(&[
                    Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                    Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                    Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
                    Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
                ])
                .is_ok()
        );
    }
    for range in [false, true] {
        let mut batch = identity_batch();
        if range {
            batch
                .growth_ranges
                .push((terra_world::props::PropId(7), 0..3));
        } else {
            batch.colors[0][3] = 0.5;
        }
        strip_identity_growth(&mut batch);
        let mut meshes = Assets::<Mesh>::default();
        prepare_prop_batch(&mut batch, &mut meshes);
        let mesh = meshes.get(&batch.staged.as_ref().unwrap().handle).unwrap();
        assert!(mesh.contains_attribute(Mesh::ATTRIBUTE_UV_1));
        assert!(mesh.contains_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR));
    }
}

#[test]
fn actual_flower_crop_and_regrowing_grass_composition_retains_rooted_shader_inputs() {
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        biomes: false,
        rivers: false,
        water: false,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..Default::default()
    })
    .unwrap();
    let owner = SectionPos::new(-1, 0, -1);
    for (kind, variant) in [
        (PropKind::Flower, 0),
        (PropKind::Wheat, 7),
        (PropKind::Grass, 65),
    ] {
        let prop = WorldProp {
            id: terra_world::props::PropId(7),
            kind,
            variant,
            position: [-0.5, 1.0, -0.5],
            rotation: 0.0,
            scale: 1.0,
            section: owner,
            removed: false,
        };
        let mut geometry = identity_batch();
        geometry.rooted_wind.clear();
        geometry.growth_anchors.clear();
        let templates = HashMap::from([(
            (kind as u8, prop.template_variant(), DetailLevel::Near),
            PropTemplate {
                geometry,
                biome_tinted: vec![false; 3],
            },
        )]);
        let (mut batches, count) = compose_with_growth(
            &[prop],
            owner,
            DetailLevel::Near,
            &templates,
            &terrain,
            &Default::default(),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(batches.len(), 1);
        let batch = &mut batches[0];
        assert_eq!(batch.growth_ranges, [(prop.id, 0..3)]);
        strip_identity_growth(batch);
        let mut meshes = Assets::<Mesh>::default();
        prepare_prop_batch(batch, &mut meshes);
        let mesh = meshes.get(&batch.staged.as_ref().unwrap().handle).unwrap();
        assert!(mesh.contains_attribute(Mesh::ATTRIBUTE_UV_1));
        assert!(mesh.contains_attribute(wind::ATTRIBUTE_GROWTH_ANCHOR));
    }
}

#[test]
fn final_split_discards_only_static_piece_payload_without_regrouping_or_losing_growth_ranges() {
    let count = 66_000;
    let mut source = MeshBatch {
        positions: vec![[0.25, 0.5, 0.75]; count],
        packed_normals: vec![[0, 127, 0, 0]; count],
        uvs: vec![[0.5, 0.75]; count],
        packed_colors: vec![[128, 192, 64, 255]; count],
        rooted_wind: vec![[0.0, 1.0]; count],
        growth_anchors: vec![[0.25, 0.0, 0.75]; count],
        indices: (0..count as u32).collect(),
        growth_ranges: vec![(terra_world::props::PropId(7), count - 3..count)],
        ..Default::default()
    };
    source.packed_colors[count - 3..].fill([128, 192, 64, 64]);
    let mut pieces = crate::upload_budget::split_batches(vec![source]).unwrap();
    assert!(pieces.len() > 1);
    let before: Vec<_> = pieces
        .iter()
        .map(|piece| {
            (
                piece.positions.len(),
                piece.indices.clone(),
                piece.compact_indices.clone(),
                piece.growth_ranges.clone(),
                crate::upload_budget::batch_bytes(piece),
            )
        })
        .collect();
    let mut stripped = 0;
    for (piece, (vertices, indices, compact, ranges, bytes)) in pieces.iter_mut().zip(before) {
        strip_identity_growth(piece);
        assert_eq!(piece.positions.len(), vertices);
        assert_eq!(piece.indices, indices);
        assert_eq!(piece.compact_indices, compact);
        assert_eq!(piece.growth_ranges, ranges);
        if ranges.is_empty() {
            stripped += 1;
            assert!(piece.growth_anchors.is_empty() && piece.rooted_wind.is_empty());
            assert_eq!(
                crate::upload_budget::batch_bytes(piece),
                bytes - vertices * 20
            );
        } else {
            assert_eq!(piece.growth_anchors.len(), vertices);
            assert_eq!(piece.rooted_wind.len(), vertices);
            assert_eq!(crate::upload_budget::batch_bytes(piece), bytes);
        }
    }
    assert!(stripped > 0);
    assert_eq!(
        pieces
            .iter()
            .map(|piece| piece.positions.len())
            .sum::<usize>(),
        count
    );
    assert_eq!(
        pieces
            .iter()
            .flat_map(|piece| &piece.growth_ranges)
            .map(|(_, range)| range.len())
            .sum::<usize>(),
        3
    );
    assert!(matches!(
        pieces.last().unwrap().packed_colors.last(),
        Some([_, _, _, 64])
    ));
}
