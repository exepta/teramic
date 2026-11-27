//! Chunk mesh upload, material presentation and lighting. Generation stays in terra-world.
mod appearance;
mod apple_fruit;
mod blood_drips;
pub use blood_drips::BloodDripStatus;
mod armor;
pub use armor::ArmorRenderStatus;
mod building;
mod crushers;
pub use building::{BuildingStatus, PlacementOutline, PlacementOutlineMaterial, TreeFallStatus};
pub use crushers::CrusherRenderStatus;
pub mod blocks;
mod diagnostics;
mod draw_calls;
mod dye_tint;
pub use draw_calls::{DrawCallSnapshot, DrawCallStatus, GpuSubdrawSnapshot};
mod chests;
mod settlement_banners;
mod train_blueprint;
mod train_yards;
mod trains;
pub use settlement_banners::VillageHeraldryStatus;
pub use trains::workshop_preview::WorkshopPreviewImage;
mod chunk_borders;
pub use chunk_borders::ChunkBorderGizmos;
mod foliage;
mod footprints;
mod furnaces;
mod volcanoes;
pub use furnaces::molten::{VolcanicSound, VolcanicSources};
mod openings;
mod plumbing;
pub use footprints::FootprintStatus;
pub use plumbing::glass::{GlassPipeNodeRenderStatus, GlassPipeRenderStatus};
pub use plumbing::infusion::InfusionRenderStatus;
pub use plumbing::intake::IntakeRenderStatus;
pub use plumbing::jet::InfuserJetRenderStatus;
mod graphics;
mod map_palette;
mod miniature;
pub use foliage::{FoliageReferenceBenchmark, FoliageStatus, WindFibre, WindFoliage};
pub mod creatures;
pub mod meshing;
mod player_preview;
mod players;
pub use player_preview::{
    PlayerPreviewImage, RenderCharacterPreviewImage, RenderCharacterPreviewSelection,
    RenderInventoryPortraitImage, RenderInventoryPortraitSelection,
};
mod plugin;
mod previews;
pub use previews::{
    RenderBlockPreviews, RenderItemPreviewRequests, RenderItemPreviews, RenderPreviewStartup,
    RenderPropPreviews,
};
mod weather;
pub use players::{AvatarBlink, RemoteAvatar};
pub use weather::{CosmeticSnowPreview, WeatherPreview, WeatherRenderStatus};
mod props;
mod scene;
pub use scene::caves::CaveState;
pub use scene::caves::lights::{LightDiscoveryStatus, TorchLightingStatus, TorchSources};
mod selection;
mod transitions;
mod upload_budget;
mod work_areas;
pub use upload_budget::MeshUploadStatus;
pub use work_areas::{FogBlockStatus, SpiritMistStatus, WorkAreaOverlayStatus};
mod liquid_motion;
pub mod particle_fluids;
pub use particle_fluids::{
    FluidCollider, FluidParticleSeed, FluidPath, ParticleFluidInput, ParticleFluidReadback,
    ParticleFluidStatus,
};
mod water;
pub use diagnostics::TerraRenderDiagnosticsPlugin;
pub use liquid_motion::FlowInterpolationStatus;
pub use meshing::visibility::TerrainVisibility;
pub use meshing::{CoastalWashQuads, RenderStatus, RenderedSections, TerrainSection};
pub use plugin::TerraRenderPlugin;
pub use props::{PropGrowthStatus, PropRenderStatus, WindProp};
pub use transitions::{ChunkAnimationStatus, LodSwapStatus};
pub use water::{InkExposure, UnderwaterState, WaterStatus};
mod grouped_point_shadows;
mod material_work;
mod mesh_buffers;
mod preprocess_buffers;
mod preprocess_cache;
mod render_schedule;

#[cfg(test)]
mod shader_migration_tests;
