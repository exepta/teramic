pub(crate) mod builder;
mod connectivity;
pub(crate) mod custom;
mod glass;
mod jobs;
mod lighting;
pub(crate) mod lod;
mod material_edges;
mod mushroom_details;
pub(crate) mod ore_details;
mod pots;
mod seasonal_foliage;
pub(crate) mod segments;
pub(crate) mod shadows;
mod tint_faces;
mod tree_appearance;
mod tree_details;
pub use lod::DetailLevel;
mod far_batches;
#[cfg(test)]
mod seam_tests;
pub(crate) mod staging;
#[cfg(test)]
mod tests;
mod upload;
pub(crate) mod visibility;
use bevy::prelude::*;
pub(crate) use far_batches::FarCache;
pub(crate) use far_batches::install as install_far_batches;
pub(crate) use jobs::focus_bucket;
pub(crate) use jobs::refresh_growth;
pub(crate) use jobs::{GeometryCache, RenderJobs, dispatch_meshes, refresh_stream_exposure};
use std::collections::{HashMap, HashSet};
use terra_voxel::SectionPos;
pub(crate) use upload::upload_sections;
/// Offline fixtures replace voxel data without a gameplay edit epoch. Cancel
/// old worker snapshots before installing their replacement section data.
pub fn forget_section_snapshots(world: &mut World, positions: &[SectionPos]) {
    visibility::invalidate(world, positions);
    for &position in positions {
        world.resource_mut::<RenderJobs>().0.remove(&position);
        world.resource_mut::<GeometryCache>().forget(position);
    }
}
#[derive(Component, Debug)]
pub struct TerrainSection {
    pub position: SectionPos,
    pub mesh_epoch: u64,
    pub detail_level: DetailLevel,
}
#[derive(Resource, Default)]
pub struct RenderedSections {
    pub sections: HashMap<SectionPos, RenderedSection>,
    /// Includes composed empty sections so their first building edit is not a new entrance.
    pub(crate) block_revisions: HashMap<SectionPos, u64>,
}

pub struct RenderedSection {
    pub micro_detail: bool,
    pub appearance_epoch: u64,
    pub entity: Entity,
    pub meshes: Vec<Handle<Mesh>>,
    pub mesh_epoch: u64,
    pub detail_level: DetailLevel,
}

#[derive(Resource, Default)]
pub struct RenderStatus {
    pub occluded_sections: usize,
    pub ready: bool,
    pub pending_jobs: usize,
    /// Chunk columns still awaiting terrain composition or upload.
    pub pending_columns: HashSet<[i64; 2]>,
    pub error: Option<String>,
    pub completed_mesh_jobs: usize,
    /// Sum of worker elapsed composition time, excluding voxel generation and GPU upload.
    pub composition_seconds: f64,
    pub max_composition_seconds: f64,
    pub far_groups: usize,
    pub far_source_batches: usize,
    pub far_cache_bytes: usize,
}

/// Exposed surf quads in an active water batch, useful for native acceptance.
#[derive(Component)]
pub struct CoastalWashQuads(pub usize);
