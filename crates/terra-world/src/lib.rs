//! Stateless terrain generation and shared section streaming/editing.
#[cfg(all(test, feature = "bevy"))]
mod creature_tests;
#[cfg(feature = "bevy")]
pub mod creatures;
pub mod edits;
pub mod environment;
pub mod farming;
#[cfg(feature = "bevy")]
mod fire;
#[cfg(feature = "bevy")]
pub mod furnaces;
pub mod generation;
#[cfg(feature = "bevy")]
mod growth;
#[cfg(feature = "bevy")]
pub mod interaction_shapes;
pub mod objects;
pub mod ore_shapes;
#[cfg(feature = "bevy")]
pub mod placement;
pub mod props;
pub mod segments;
#[cfg(feature = "bevy")]
pub mod slabs;
pub mod spawn;
#[cfg(feature = "bevy")]
pub mod streaming;
#[cfg(feature = "bevy")]
pub mod targeting;
#[cfg(feature = "bevy")]
pub mod tree_felling;
pub mod water;
pub mod weather;
pub use generation::{
    ChunkGenerator, GenerationError, GroundCover, MAX_SAMPLE_COORDINATE, PreparedColumn,
    SectionGenerationError, SectionGenerator, SurfaceColumn, TerraGenerator, TerrainGenerator,
};
#[cfg(feature = "bevy")]
pub use streaming::{TerraWorldPlugin, WorldMode};
pub mod cartography;
pub mod enclosure;
