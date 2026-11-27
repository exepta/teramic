pub(crate) mod builder;
mod cactus_details;
mod connectivity;
pub(crate) mod custom;
mod glass;
mod jobs;
pub(crate) mod lava;
mod lighting;
pub(crate) mod lod;
mod material_edges;
mod mushroom_details;
pub(crate) mod ore_details;
mod pots;
mod precision;
mod published_children;
mod resident;
mod seasonal_foliage;
pub(crate) mod segments;
pub(crate) mod shadows;
mod tint_faces;
mod tree_appearance;
mod tree_details;
pub use lod::DetailLevel;

/// Inspect the shared terrain material without exposing its extension type.
/// Native lighting fixtures use this to verify the actual caster entity path.
pub fn uses_shared_surface_material(
    world: &bevy::prelude::World,
    mesh: bevy::prelude::Entity,
) -> bool {
    world
        .get::<bevy::prelude::MeshMaterial3d<crate::weather::surfaces::TerrainDepthMaterial>>(mesh)
        .is_some()
}
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
    world.resource_scope(|world, mut jobs: Mut<RenderJobs>| {
        let mut meshes = world.get_resource_mut::<Assets<Mesh>>();
        for position in positions {
            jobs.cancel(position, meshes.as_deref_mut());
        }
    });
    for &position in positions {
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
    presentations: HashMap<SectionPos, SectionPresentation>,
}

struct SectionPresentation {
    block_revision: u64,
    /// A successful composition can produce no render entity.
    empty_mesh_epoch: Option<u64>,
}

impl RenderedSections {
    pub(crate) fn block_revision(&self, position: SectionPos) -> Option<u64> {
        self.presentations.get(&position).map(|p| p.block_revision)
    }

    pub(crate) fn record_presentation(
        &mut self,
        position: SectionPos,
        block_revision: u64,
        empty_mesh_epoch: Option<u64>,
    ) -> bool {
        self.presentations
            .insert(
                position,
                SectionPresentation {
                    block_revision,
                    empty_mesh_epoch,
                },
            )
            .is_some_and(|previous| previous.block_revision != block_revision)
    }

    /// A replacement LOD can be prepared while the current terrain is on screen.
    /// Missing data, a retired visible mesh, or a first upload still in flight must wait.
    pub fn section_presented(
        &self,
        position: SectionPos,
        section: Option<&terra_world::streaming::LoadedSection>,
        visibility: Option<&visibility::TerrainVisibility>,
    ) -> bool {
        section.is_some_and(|section| {
            visibility.is_some_and(|visibility| visibility.hidden(position))
                || self
                    .sections
                    .get(&position)
                    .is_some_and(|drawn| drawn.mesh_epoch == section.mesh_epoch)
                || self
                    .presentations
                    .get(&position)
                    .is_some_and(|p| p.empty_mesh_epoch == Some(section.mesh_epoch))
        })
    }
}

pub struct RenderedSection {
    pub(crate) folded_leaves: Option<lod::FoldedLeaves>,
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
    /// The session has revealed the terrain and enabled player movement.
    pub scene_revealed: bool,
    pub pending_jobs: usize,
    /// Chunk columns still awaiting terrain composition or upload.
    pub pending_columns: HashSet<[i64; 2]>,
    pub error: Option<String>,
    pub completed_mesh_jobs: usize,
    pub reused_shadow_meshes: u64,
    pub reused_shadow_bytes: u64,
    pub reused_edit_meshes: u64,
    pub reused_edit_bytes: u64,
    pub reused_streaming_meshes: u64,
    pub reused_streaming_bytes: u64,
    /// Wind-animated meshes included in the edit/streaming reuse totals.
    pub reused_wind_meshes: u64,
    pub reused_wind_bytes: u64,
    /// Peak matched CPU stream bytes released for jobs waiting across a frame boundary.
    pub peak_deferred_resident_bytes: usize,
    pub retained_draw_entities: u64,
    /// Sum of worker elapsed composition time, excluding voxel generation and GPU upload.
    pub composition_seconds: f64,
    pub max_composition_seconds: f64,
    pub far_groups: usize,
    pub far_source_batches: usize,
    pub far_cache_bytes: usize,
}

impl RenderStatus {
    /// Shared loading contract: portal-hidden meshes need no upload, but missing
    /// section data or pending composition/upload work still blocks readiness.
    pub fn section_ready(
        &self,
        position: SectionPos,
        section: Option<&terra_world::streaming::LoadedSection>,
        visibility: Option<&visibility::TerrainVisibility>,
    ) -> bool {
        section.is_some_and(|section| {
            (section.mesh.is_none() || visibility.is_some_and(|v| v.hidden(position)))
                && !self.pending_columns.contains(&[position.x, position.z])
        })
    }
}

/// Exposed surf quads in an active water batch, useful for native acceptance.
#[derive(Component)]
pub struct CoastalWashQuads(pub usize);
