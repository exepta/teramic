use super::MeshBatch;
use bevy::mesh::Indices;

#[test]
fn compact_indices_halve_ready_and_gpu_index_bytes_without_changing_triangles() {
    let expected = vec![0, 1, 2, 0, 2, 3];
    let mut batch = MeshBatch {
        positions: vec![[0.0; 3]; 4],
        indices: expected.clone(),
        ..Default::default()
    };
    let original_bytes = batch.indices.capacity() * size_of::<u32>();
    batch.compact_indices();
    assert!(batch.indices.is_empty());
    assert_eq!(batch.indices.capacity(), 0);
    assert_eq!(
        batch.compact_indices.capacity() * size_of::<u16>(),
        original_bytes / 2
    );
    assert!(batch.has_indices());
    batch.compact_indices();
    assert!(batch.has_indices());
    let Indices::U16(indices) = batch.take_indices() else {
        panic!("small batches should upload two-byte indices");
    };
    assert_eq!(
        indices.into_iter().map(u32::from).collect::<Vec<_>>(),
        expected
    );
    assert!(!batch.has_indices());
}

#[test]
fn largest_compact_vertex_index_is_lossless_and_larger_batches_keep_u32() {
    for vertices in [65_536, 65_537] {
        let expected = vec![0, 1, vertices as u32 - 1];
        let mut batch = MeshBatch {
            positions: vec![[0.0; 3]; vertices],
            indices: expected.clone(),
            ..Default::default()
        };
        batch.compact_indices();
        let indices = batch.take_indices();
        assert_eq!(
            indices.iter().collect::<Vec<_>>(),
            expected.iter().map(|&i| i as usize).collect::<Vec<_>>()
        );
        assert_eq!(matches!(indices, Indices::U16(_)), vertices == 65_536);
    }
}

#[test]
fn malformed_index_is_never_truncated_and_empty_batches_stay_empty() {
    let mut batch = MeshBatch {
        positions: vec![[0.0; 3]; 3],
        indices: vec![0, 1, 65_536],
        ..Default::default()
    };
    batch.compact_indices();
    assert!(matches!(batch.take_indices(), Indices::U32(indices) if indices == [0, 1, 65_536]));
    let mut empty = MeshBatch::default();
    empty.compact_indices();
    assert!(!empty.has_indices());
    assert_eq!(empty.compact_indices.capacity(), 0);
}
