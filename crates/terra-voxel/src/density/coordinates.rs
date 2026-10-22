use super::{CHUNK_SIZE, SAMPLE_SIZE, VoxelError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl ChunkPos {
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }
    pub fn global_sample(self, local: [usize; 3]) -> Result<[i64; 3], VoxelError> {
        sample_index(local)?;
        let mut global = [0; 3];
        for (axis, chunk) in [self.x, self.y, self.z].into_iter().enumerate() {
            global[axis] = chunk
                .checked_mul(i64::from(CHUNK_SIZE))
                .and_then(|origin| origin.checked_add(local[axis] as i64))
                .ok_or(VoxelError::CoordinateOverflow)?;
        }
        Ok(global)
    }
}

/// Cells and sample ownership use half-open chunks [0,32). Local 32 is a read-only halo.
/// Thus global -1 belongs to chunk -1, local 31. Global 0 belongs to chunk 0, local 0.
pub fn global_to_chunk(global: [i64; 3]) -> (ChunkPos, [usize; 3]) {
    let size = i64::from(CHUNK_SIZE);
    let chunk = global.map(|v| v.div_euclid(size));
    let local = global.map(|v| v.rem_euclid(size) as usize);
    (ChunkPos::new(chunk[0], chunk[1], chunk[2]), local)
}

/// X changes fastest, then Y, then Z.
pub fn sample_index(local: [usize; 3]) -> Result<usize, VoxelError> {
    if local.iter().any(|&v| v >= SAMPLE_SIZE) {
        return Err(VoxelError::OutOfBounds(local));
    }
    Ok(local[0] + SAMPLE_SIZE * (local[1] + SAMPLE_SIZE * local[2]))
}
