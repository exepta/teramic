mod chunk;
mod coordinates;
mod error;
mod sample;
#[cfg(test)]
mod tests;

pub use chunk::VoxelChunk;
pub use coordinates::{ChunkPos, global_to_chunk, sample_index};
pub use error::VoxelError;
pub use sample::VoxelSample;
pub const CHUNK_SIZE: i32 = 32;
pub const SAMPLE_SIZE: usize = 33;
pub const SAMPLE_COUNT: usize = SAMPLE_SIZE * SAMPLE_SIZE * SAMPLE_SIZE;
