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
mod desert;
mod generator;
#[cfg(test)]
mod geology_tests;
mod highlands;
mod lakes;
mod landforms;
mod landscape;
mod misty_coast;
pub mod natural_features;
pub use landscape::LandscapeNoise;
mod fantasy_relief;
mod landscape_regions;
pub use landscape_regions::{LandformKind, LandscapeRegionSample};
mod oases;
mod ocean;
mod plains;
pub use ocean::{OceanArea, OceanHabitat};
mod ores;
pub use oases::OasisSample;
mod rivers;
pub use rivers::{HydrologyDistributary, HydrologyRegion, HydrologyTributary, RiverSample};
mod cherry_grove;
#[cfg(test)]
mod landscape_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod reference_tests;
#[cfg(test)]
mod section_tests;
pub mod sections;
pub(crate) mod seed;
mod spirit_blossom;
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
pub use vegetation::{GroundCover, PalmSpan, TreeAppearance, TreeAppearanceSnapshot};

#[cfg(feature = "bevy")]
pub use vegetation::{planted_shape, planted_shape_with_ecology};

mod ashlands;
mod blood_forest;
mod crystal_highlands;
mod petrified_forest;
pub use natural_features::{
    CoastalLandmarkKind, CoastalLandmarkSite, CrystalLandmarkKind, CrystalLandmarkSite,
};
mod savanna;
mod swamp;
