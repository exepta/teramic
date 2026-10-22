use super::{MAX_SAMPLE_COORDINATE, TerrainGenerator};
use terra_voxel::{ChunkPos, SAMPLE_COUNT, SAMPLE_SIZE, VoxelChunk, VoxelError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error(transparent)]
    Voxel(#[from] VoxelError),
    #[error("chunk {0:?} exceeds the generator's +/-2^40 sample-coordinate precision budget")]
    PrecisionLimit(ChunkPos),
}
pub struct ChunkGenerator<G> {
    terrain: G,
}
impl<G: TerrainGenerator> ChunkGenerator<G> {
    pub fn new(terrain: G) -> Self {
        Self { terrain }
    }
    pub fn terrain(&self) -> &G {
        &self.terrain
    }
    pub fn generate(&self, position: ChunkPos) -> Result<VoxelChunk, GenerationError> {
        let origin = position.global_sample([0; 3])?;
        let end = position.global_sample([32; 3])?;
        if origin
            .into_iter()
            .chain(end)
            .any(|v| !(-MAX_SAMPLE_COORDINATE..=MAX_SAMPLE_COORDINATE).contains(&v))
        {
            return Err(GenerationError::PrecisionLimit(position));
        }
        let mut samples = Vec::with_capacity(SAMPLE_COUNT);
        for z in 0..SAMPLE_SIZE {
            for y in 0..SAMPLE_SIZE {
                for x in 0..SAMPLE_SIZE {
                    let global = [
                        origin[0] + x as i64,
                        origin[1] + y as i64,
                        origin[2] + z as i64,
                    ];
                    samples.push(self.terrain.sample(global.map(|v| v as f64)));
                }
            }
        }
        Ok(VoxelChunk::from_samples(position, samples)?)
    }
}
