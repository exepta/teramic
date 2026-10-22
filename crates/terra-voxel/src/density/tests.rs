use super::*;
use crate::MaterialId;
#[test]
fn euclidean_coordinates_and_roundtrip() {
    for (global, chunk, local) in [
        (0, 0, 0),
        (1, 0, 1),
        (31, 0, 31),
        (32, 1, 0),
        (33, 1, 1),
        (-1, -1, 31),
        (-32, -1, 0),
        (-33, -2, 31),
        (i64::MIN, i64::MIN / 32, 0),
    ] {
        let (p, l) = global_to_chunk([global; 3]);
        assert_eq!(p, ChunkPos::new(chunk, chunk, chunk));
        assert_eq!(l, [local; 3]);
        assert_eq!(p.global_sample(l).unwrap(), [global; 3]);
    }
    let (p, l) = global_to_chunk([i64::MAX; 3]);
    assert_eq!(p.global_sample(l).unwrap(), [i64::MAX; 3]);
    assert!(ChunkPos::new(i64::MAX, 0, 0).global_sample([0; 3]).is_err());
}
#[test]
fn indexing_is_bijective() {
    let mut seen = vec![false; SAMPLE_COUNT];
    for z in 0..SAMPLE_SIZE {
        for y in 0..SAMPLE_SIZE {
            for x in 0..SAMPLE_SIZE {
                let i = sample_index([x, y, z]).unwrap();
                assert!(!seen[i]);
                seen[i] = true;
            }
        }
    }
    assert!(seen.into_iter().all(|v| v));
    assert_eq!(sample_index([32; 3]).unwrap(), SAMPLE_COUNT - 1);
}
#[test]
fn checked_access_and_revisions() {
    let mut chunk = VoxelChunk::new(ChunkPos::default());
    let stone = VoxelSample::new(-1.0, MaterialId::Stone);
    chunk.set_sample([31; 3], stone).unwrap();
    assert_eq!(chunk.sample([31; 3]).unwrap(), stone);
    assert_eq!(chunk.revision(), 1);
    chunk.set_sample([31; 3], stone).unwrap();
    assert_eq!(chunk.revision(), 1);
    for p in [[33, 0, 0], [0, 33, 0], [0, 0, 33], [usize::MAX; 3]] {
        assert!(chunk.sample(p).is_err());
        assert!(chunk.set_sample(p, stone).is_err());
    }
    assert!(
        chunk
            .set_sample(
                [0; 3],
                VoxelSample {
                    density: f32::NAN,
                    material: 0
                }
            )
            .is_err()
    );
    assert_eq!(chunk.revision(), 1);
    chunk.set_revision_for_test(u64::MAX);
    assert!(matches!(
        chunk.set_sample([0; 3], stone),
        Err(VoxelError::RevisionOverflow)
    ));
    assert_eq!(chunk.sample([0; 3]).unwrap(), VoxelSample::AIR);
}
#[test]
fn boundary_ownership_and_sync() {
    let mut left = VoxelChunk::new(ChunkPos::new(-1, 0, 0));
    let mut right = VoxelChunk::new(ChunkPos::default());
    let value = VoxelSample::new(-2.0, MaterialId::Dirt);
    assert!(matches!(
        left.set_sample([32, 4, 5], value),
        Err(VoxelError::HaloWrite(_))
    ));
    right.set_sample([0, 4, 5], value).unwrap();
    left.sync_halo([0, 4, 5], right.sample([0, 4, 5]).unwrap())
        .unwrap();
    assert_eq!(left.sample([32, 4, 5]).unwrap(), value);
    assert!(left.sync_halo([-1, 4, 5], value).is_err());
    assert!(left.sync_halo([1, 4, 5], value).is_err());
}
#[test]
fn invalid_sample_buffers_and_stable_materials() {
    assert!(VoxelChunk::from_samples(ChunkPos::default(), vec![]).is_err());
    assert!(
        VoxelChunk::from_samples(
            ChunkPos::default(),
            vec![VoxelSample::new(f32::INFINITY, MaterialId::Air); SAMPLE_COUNT]
        )
        .is_err()
    );
    assert_eq!(
        [
            MaterialId::Air as u16,
            MaterialId::Grass as u16,
            MaterialId::Dirt as u16,
            MaterialId::Stone as u16,
            MaterialId::Sand as u16,
            MaterialId::Snow as u16
        ],
        [0, 1, 2, 3, 4, 5]
    );
}
