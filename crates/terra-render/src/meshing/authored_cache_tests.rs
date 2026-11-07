use super::*;

fn uncached(
    block: &crate::blocks::BlockTemplate,
    side: usize,
    mask: u8,
    offset: Vec3,
) -> BTreeMap<usize, MeshBatch> {
    let mut batches = BTreeMap::new();
    for primitive in &block.faces[side] {
        append(
            &mut batches,
            primitive,
            offset,
            Quat::IDENTITY,
            Some((mask, block.bevel_width)),
            None,
        )
        .unwrap();
    }
    batches
}

fn equivalent(actual: &BTreeMap<usize, MeshBatch>, expected: &BTreeMap<usize, MeshBatch>) {
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    for (&material, actual) in actual {
        let expected = &expected[&material];
        assert_eq!(actual.indices.len(), expected.indices.len());
        assert_eq!(actual.positions.len(), actual.tangents.len());
        for (&a, &b) in actual.indices.iter().zip(&expected.indices) {
            let (a, b) = (a as usize, b as usize);
            assert_eq!(actual.positions[a], expected.positions[b]);
            assert_eq!(actual.uvs[a], expected.uvs[b]);
            assert!(
                (Vec3::from_array(actual.normals[a]) - Vec3::from_array(expected.normals[b]))
                    .length()
                    < 0.004
            );
            assert_eq!(actual.tangents[a][3], expected.tangents[b][3]);
            for axis in 0..3 {
                assert!((actual.tangents[a][axis] - expected.tangents[b][axis]).abs() < 0.004);
            }
        }
        assert!(
            actual
                .tangents
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
    }
}

#[test]
fn cached_authored_faces_keep_all_neighbor_masks_uvs_triangles_and_tangent_frames() {
    let templates = crate::blocks::authored_terrain_templates();
    let block = &templates.blocks[&1];
    let mut cache = AuthoredFaceCache::default();
    for side in 0..6 {
        for mask in 0..64 {
            for offset in [
                Vec3::ZERO,
                Vec3::new(15.0, 14.0, 13.0),
                Vec3::new(-8.0, -7.0, -6.0),
            ] {
                let mut actual = BTreeMap::new();
                cache
                    .append(&mut actual, 1, block, side, offset, mask)
                    .unwrap();
                equivalent(&actual, &uncached(block, side, mask, offset));
            }
        }
    }
    assert_eq!(cache.0.len(), AuthoredFaceCache::LIMIT);
}

#[test]
fn authored_face_cache_is_composition_owned_bounded_and_falls_back_for_new_keys() {
    let templates = crate::blocks::authored_terrain_templates();
    let block = &templates.blocks[&1];
    let mut cache = AuthoredFaceCache::default();
    for side in 0..6 {
        for mask in 0..64 {
            cache
                .append(&mut BTreeMap::new(), 1, block, side, Vec3::ZERO, mask)
                .unwrap();
        }
    }
    for id in 2..130 {
        let mut actual = BTreeMap::new();
        let offset = Vec3::new(3.0, 4.0, 5.0);
        cache.append(&mut actual, id, block, 0, offset, 42).unwrap();
        assert_eq!(actual, uncached(block, 0, 42, offset));
        assert_eq!(cache.0.len(), AuthoredFaceCache::LIMIT);
    }
    drop(cache);
    let next_composition = AuthoredFaceCache::default();
    assert!(next_composition.0.is_empty());
}
