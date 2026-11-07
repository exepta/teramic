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
pub mod funnels;
#[cfg(feature = "bevy")]
pub mod furnaces;
pub mod generation;
#[cfg(feature = "bevy")]
mod growth;
#[cfg(feature = "bevy")]
pub mod hearths;
#[cfg(feature = "bevy")]
pub mod interaction_shapes;
#[cfg(feature = "bevy")]
pub mod kingdom_access;
#[cfg(feature = "bevy")]
pub mod lanterns;
pub mod objects;
pub mod ore_shapes;
#[cfg(feature = "bevy")]
pub mod placement;
pub mod plumbing;
pub mod props;
pub mod rails;
pub mod segments;
#[cfg(feature = "bevy")]
pub mod slabs;
#[cfg(feature = "bevy")]
pub mod snow;
pub mod spawn;
#[cfg(feature = "bevy")]
pub mod streaming;
#[cfg(feature = "bevy")]
pub mod targeting;
#[cfg(feature = "bevy")]
pub mod torches;
#[cfg(feature = "bevy")]
pub mod tree_felling;
pub mod water;
pub mod weather;
#[cfg(feature = "bevy")]
pub mod wrench;
pub use generation::{
    ChunkGenerator, GenerationError, GroundCover, MAX_SAMPLE_COORDINATE, PreparedColumn,
    SectionGenerationError, SectionGenerator, SurfaceColumn, TerraGenerator, TerrainDebugSample,
    TerrainFields, TerrainGenerator,
};
#[cfg(feature = "bevy")]
pub use streaming::{TerraWorldPlugin, WorldMode};
pub mod cartography;
#[cfg(feature = "bevy")]
pub mod conveyors;
pub mod crushers;
pub mod enclosure;

pub mod fruit;
#[cfg(feature = "bevy")]
pub mod saplings;
