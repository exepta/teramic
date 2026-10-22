use terra_voxel::VoxelSample;

pub trait TerrainGenerator: Send + Sync {
    /// Position must be finite and within MAX_SAMPLE_COORDINATE on all axes.
    fn sample(&self, position: [f64; 3]) -> VoxelSample;
    fn version(&self) -> u32;
}

/// Conservative precision budget for f64 noise. Chunk positions are still represented by i64.
pub const MAX_SAMPLE_COORDINATE: i64 = 1_i64 << 40;
