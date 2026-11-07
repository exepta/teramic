mod advanced;
mod biome_terrain;
pub use advanced::{TerrainDebugSample, TerrainFields};
pub mod biomes;
mod cave_biomes;
mod cave_ecology;
mod cave_structures;
mod connector_water;
mod webs;
pub use cave_biomes::{CaveBiome, CaveBiomeColumn, CaveBiomeSample};
mod caves;
pub use cave_structures::{CaveEcology, CaveStructure};
pub use caves::CaveKind;
#[cfg(test)]
mod column_tests;
mod columns;
mod density;
mod generator;
#[cfg(test)]
mod geology_tests;
mod lakes;
mod landforms;
mod landscape;
pub use landscape::LandscapeNoise;
mod landscape_regions;
pub use landscape_regions::{LandformKind, LandscapeRegionSample};
mod oases;
mod ocean;
pub use ocean::{OceanArea, OceanHabitat};
mod ores;
pub use oases::OasisSample;
mod rivers;
pub use rivers::{HydrologyDistributary, HydrologyRegion, HydrologyTributary, RiverSample};
#[cfg(test)]
mod landscape_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod reference_tests;
#[cfg(test)]
mod section_tests;
pub mod sections;
mod seed;
pub mod structures;
mod surface;
mod terrain;
#[cfg(test)]
mod tests;
mod vegetation;

pub use columns::PreparedColumn;
pub use density::{ChunkGenerator, GenerationError};
pub use generator::TerraGenerator;
pub use sections::{SectionGenerationError, SectionGenerator};
pub use surface::SurfaceColumn;
pub use terrain::{MAX_SAMPLE_COORDINATE, TerrainGenerator};
pub use vegetation::{GroundCover, TreeAppearance, TreeAppearanceSnapshot};

#[cfg(feature = "bevy")]
pub use vegetation::planted_shape;
