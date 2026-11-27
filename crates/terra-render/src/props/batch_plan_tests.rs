use super::*;

fn template(variant: u8, vertices: usize) -> PropTemplate {
    assert_eq!(vertices % 4, 0);
    let mut geometry = MeshBatch::default();
    for quad in 0..vertices / 4 {
        let base = geometry.positions.len() as u32;
        let offset = (quad % 71) as f32 * 0.001 + f32::from(variant) * 0.025;
        for [x, y, z] in [
            [0.0, 0.0, 0.0],
            [0.25, 0.0, 0.0],
            [0.25, 0.5, 0.1],
            [0.0, 0.5, 0.1],
        ] {
            geometry.positions.push([x + offset, y, z]);
            geometry.normals.push([0.0, 1.0, 0.0]);
            geometry.uvs.push([y, 0.35 + offset]);
            geometry.colors.push([0.25 + offset, 0.75, 0.3, 1.0]);
        }
        geometry
            .indices
            .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    PropTemplate {
        biome_tinted: (0..vertices).map(|vertex| vertex % 3 != 0).collect(),
        geometry,
    }
}

fn prop(kind: PropKind, id: u64, section: SectionPos) -> WorldProp {
    WorldProp {
        id: terra_world::props::PropId(id),
        kind,
        position: [-15.5 + id as f64, 1.0, -15.5],
        rotation: id as f32 * 0.23,
        scale: 0.8,
        variant: 0,
        section,
        removed: false,
    }
}

// Compare complete ordered triangle vertices; splitting may reorder props but
// must never alter a vertex attribute, drop a face or change its winding.
fn triangles(batches: &[MeshBatch]) -> Vec<[[u32; 22]; 3]> {
    let vertex = |batch: &MeshBatch, index: usize| {
        let mut result = [0; 22];
        let mut offset = 1;
        for (flag, values) in [
            (1, batch.positions.get(index).map(|v| v.as_slice())),
            (2, batch.normals.get(index).map(|v| v.as_slice())),
            (4, batch.uvs.get(index).map(|v| v.as_slice())),
            (8, batch.rooted_wind.get(index).map(|v| v.as_slice())),
            (16, batch.growth_anchors.get(index).map(|v| v.as_slice())),
            (32, batch.colors.get(index).map(|v| v.as_slice())),
        ] {
            if let Some(values) = values {
                result[0] |= flag;
                for value in values {
                    result[offset] = value.to_bits();
                    offset += 1;
                }
            }
        }
        if let Some(values) = batch.packed_normals.get(index) {
            result[0] |= 64;
            result[offset] = u32::from_ne_bytes(values.map(|v| v as u8));
            offset += 1;
        }
        if let Some(values) = batch.packed_colors.get(index) {
            result[0] |= 128;
            result[offset] = u32::from_ne_bytes(*values);
        }
        result
    };
    let mut triangles = Vec::new();
    for batch in batches {
        let indices: Vec<_> = if batch.compact_indices.is_empty() {
            batch.indices.iter().map(|&index| index as usize).collect()
        } else {
            batch
                .compact_indices
                .iter()
                .map(|&index| usize::from(index))
                .collect()
        };
        assert!(indices.iter().all(|&index| index < batch.positions.len()));
        assert_eq!(indices.len() % 3, 0);
        for triangle in indices.chunks_exact(3) {
            triangles.push(std::array::from_fn(|i| vertex(batch, triangle[i])));
        }
    }
    triangles.sort_unstable();
    triangles
}

#[test]
fn worker_splits_preserve_every_attribute_triangle_growth_range_and_winding() {
    let section = SectionPos::new(-1, 0, -1);
    let terrain = TerraGenerator::new(Default::default()).unwrap();
    for kind in [PropKind::Bush, PropKind::Flower] {
        let templates = HashMap::from([
            ((kind as u8, 0, DetailLevel::Near), template(0, 24_000)),
            ((kind as u8, 1, DetailLevel::Near), template(1, 24_000)),
        ]);
        let props: Vec<_> = (1..=4).map(|id| prop(kind, id, section)).collect();
        let growth = if kind == PropKind::Flower {
            HashMap::from([
                (
                    props[0].id,
                    Pose {
                        scale: 0.8,
                        ratio: 0.4,
                        start: -1.0,
                        pending: true,
                        previous_variant: Some(1),
                        ..Default::default()
                    },
                ),
                (
                    props[1].id,
                    Pose {
                        scale: 0.8,
                        ratio: 0.7,
                        start: 13.0,
                        ..Default::default()
                    },
                ),
            ])
        } else {
            HashMap::new()
        };
        let conditions = HashMap::from([(
            [1, 0, 0],
            terra_world::environment::SurfaceCondition {
                wetness: 0.65,
                snow: 0.2,
                ..Default::default()
            },
        )]);
        for packed in [false, true] {
            let (mut batches, count) = compose_with_exposure_mode(
                &props,
                section,
                DetailLevel::Near,
                &templates,
                &terrain,
                &conditions,
                &growth,
                None,
                packed,
            )
            .unwrap();
            assert_eq!(count, props.len());
            assert!(batches.len() > 1);
            assert!(
                batches
                    .iter()
                    .all(|batch| batch.positions.len() <= crate::upload_budget::MAX_BATCH_VERTICES)
            );
            let mut expected = Vec::new();
            for prop in &props {
                let (one, count) = compose_with_exposure_mode(
                    &[*prop],
                    section,
                    DetailLevel::Near,
                    &templates,
                    &terrain,
                    &conditions,
                    &growth,
                    None,
                    packed,
                )
                .unwrap();
                assert_eq!(count, 1);
                expected.extend(one);
            }
            let reference = triangles(&expected);
            assert!(
                triangles(&batches) == reference,
                "split geometry changed: {kind:?}, packed={packed}"
            );
            let ranges: Vec<_> = batches
                .iter()
                .flat_map(|batch| {
                    batch.growth_ranges.iter().map(|(id, range)| {
                        assert!(range.start < range.end && range.end <= batch.positions.len());
                        (*id, range.len())
                    })
                })
                .collect();
            if kind == PropKind::Flower {
                assert_eq!(
                    ranges.iter().filter(|(id, _)| *id == props[0].id).count(),
                    2
                );
                let pending = batches.last().unwrap();
                assert!(
                    pending
                        .growth_ranges
                        .iter()
                        .all(|(id, _)| *id == props[0].id)
                );
                assert!(
                    batches[..batches.len() - 1].iter().all(|batch| {
                        batch.growth_ranges.iter().all(|(id, _)| *id != props[0].id)
                    })
                );
            } else {
                assert!(ranges.is_empty());
            }
            for batch in &mut batches {
                assert_eq!(batch.positions.capacity(), batch.positions.len());
                batch.compact_indices();
                assert!(batch.indices.is_empty());
                assert!(!batch.compact_indices.is_empty());
            }
            assert!(
                triangles(&batches) == reference,
                "index compaction changed geometry: {kind:?}, packed={packed}"
            );
        }
    }
}

#[test]
fn a_single_template_larger_than_the_limit_is_preserved_without_panicking() {
    const VERTICES: usize = (1 << 16) + 4;
    let section = SectionPos::new(-1, 0, -1);
    let terrain = TerraGenerator::new(Default::default()).unwrap();
    let templates = HashMap::from([(
        (PropKind::Bush as u8, 0, DetailLevel::Near),
        template(0, VERTICES),
    )]);
    let (mut batches, count) = compose(
        &[prop(PropKind::Bush, 1, section)],
        section,
        DetailLevel::Near,
        &templates,
        &terrain,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(batches.len(), 1);
    let batch = &mut batches[0];
    assert_eq!(batch.positions.len(), VERTICES);
    assert_eq!(batch.indices.len(), VERTICES / 4 * 6);
    batch.compact_indices();
    assert!(batch.compact_indices.is_empty());
    assert!(!batch.indices.is_empty());
}

#[test]
fn dense_canonical_props_fit_uploads_without_an_extra_material_mesh() {
    let section = SectionPos::new(-1, 0, -1);
    let terrain = TerraGenerator::new(Default::default()).unwrap();
    let mut materials = Assets::<StandardMaterial>::default();
    let handle = materials.add(StandardMaterial::default());
    let templates = make_templates(&PropMaterials::default(), &[handle], &materials).unwrap();
    for (kind, packed) in [(PropKind::Grass, true), (PropKind::Flower, false)] {
        let geometry = &templates[&(kind as u8, 0, DetailLevel::Near)].geometry;
        let vertices = geometry.positions.len();
        let indices = geometry.indices.len();
        let old_first_piece = (1 << 16) / vertices;
        let props: Vec<_> = (0..old_first_piece + 8)
            .map(|i| prop(kind, i as u64, section))
            .collect();
        // The previous first piece overflowed the uploader, with eight more
        // whole props already assigned to a separate material mesh.
        assert!(old_first_piece * vertices > crate::upload_budget::MAX_BATCH_VERTICES);
        let (batches, represented) = compose_with_exposure_mode(
            &props,
            section,
            DetailLevel::Near,
            &templates,
            &terrain,
            &Default::default(),
            &HashMap::new(),
            None,
            packed,
        )
        .unwrap();
        assert_eq!(represented, props.len());
        assert_eq!(batches.len(), 2);
        for batch in &batches {
            assert_eq!(batch.positions.len() % vertices, 0);
            assert_eq!(batch.indices.len() % indices, 0);
            assert!(batch.positions.len() <= crate::upload_budget::MAX_BATCH_VERTICES);
            assert!(
                crate::upload_budget::batch_bytes(batch) <= crate::upload_budget::MAX_BATCH_BYTES
            );
        }
        let first = &batches[0];
        if !packed {
            // At this point one more rooted flower still meets the vertex
            // quota, but its uncompressed color, timing and anchor cross 4 MiB.
            assert!(first.positions.len() + vertices <= crate::upload_budget::MAX_BATCH_VERTICES);
            let bytes_per_prop =
                crate::upload_budget::batch_bytes(first) / (first.positions.len() / vertices);
            assert!(
                crate::upload_budget::batch_bytes(first) + bytes_per_prop
                    > crate::upload_budget::MAX_BATCH_BYTES
            );
        }
        let final_batches = crate::upload_budget::split_batches(batches).unwrap();
        assert_eq!(final_batches.len(), 2);
        assert_eq!(
            final_batches
                .iter()
                .map(|b| b.positions.len())
                .sum::<usize>(),
            props.len() * vertices
        );
        assert_eq!(
            final_batches.iter().map(|b| b.indices.len()).sum::<usize>(),
            props.len() * indices
        );
    }
}
