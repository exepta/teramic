use super::{
    editing::{apply_block_edits, apply_prop_removals},
    interest::refresh_interest,
    jobs::{dispatch_jobs, receive_jobs},
    resources::*,
    water_jobs::{PendingWaterJobs, dispatch_water_jobs, receive_water_jobs},
};
use crate::environment::{SurfaceConditionRequest, apply_surface_conditions};
use crate::{TerraGenerator, edits::EditJournal};
use bevy::prelude::*;
use terra_core::GameConfig;

#[derive(Debug, Clone, Copy)]
pub enum WorldMode {
    Client,
    DedicatedServer,
}
pub struct TerraWorldPlugin {
    pub mode: WorldMode,
}
impl Plugin for TerraWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldGeneration>()
            .init_resource::<WorldAccess>()
            .init_resource::<ChunkFoci>()
            .init_resource::<ChunkStore>()
            .init_resource::<PendingSections>()
            .init_resource::<PendingWaterJobs>()
            .init_resource::<EditJournal>()
            .init_resource::<crate::water::FluidDiagnostics>()
            .add_message::<BlockEditRequest>()
            .add_message::<PropRemoveRequest>()
            .add_message::<SectionPublished>()
            .add_message::<WaterPublished>()
            .add_message::<SectionEditApplied>()
            .add_message::<SectionRemoved>()
            .add_message::<PreparedColumnChanged>()
            .add_message::<SurfaceConditionRequest>();
        let config = app.world().resource::<GameConfig>();
        let settings = match self.mode {
            WorldMode::Client => StreamingSettings {
                radius: i64::from(config.client.view_distance),
                vertical_radius: i64::from(config.client.vertical_view_distance),
                max_jobs: config.client.max_chunk_jobs,
                meshes: true,
            },
            WorldMode::DedicatedServer => StreamingSettings {
                radius: i64::from(config.server.simulation_distance.max(1)),
                vertical_radius: 1,
                max_jobs: 4,
                meshes: false,
            },
        };
        let generator = match TerraGenerator::new(config.world.clone()) {
            Ok(generator) => generator,
            Err(error) => {
                app.world_mut().resource_mut::<WorldGeneration>().status =
                    GenerationStatus::Failed(error.to_string());
                return;
            }
        };
        let height = generator.surface_height(8.0, 24.0);
        let height = if generator.config().water {
            height.max(generator.config().sea_level)
        } else {
            height
        };
        let focus = [8.0, height + 2.0, 24.0];
        app.insert_resource(WorldTerrain(generator))
            .insert_resource(ChunkFocus(focus))
            .insert_resource(settings)
            .configure_sets(
                Update,
                (
                    StreamingSet::Focus,
                    StreamingSet::Interest,
                    StreamingSet::Edits,
                    StreamingSet::Receive,
                    StreamingSet::Dispatch,
                )
                    .chain()
                    .run_if(|access: Res<WorldAccess>| access.ready()),
            )
            .add_systems(Update, refresh_interest.in_set(StreamingSet::Interest))
            .add_systems(
                Update,
                (
                    crate::props::seasonal::tick,
                    apply_block_edits,
                    super::support::tick,
                    apply_prop_removals,
                    apply_surface_conditions,
                    crate::weather::tick,
                    crate::fire::tick,
                    crate::growth::tick,
                    crate::farming::tick,
                    crate::water::tick_fluids,
                )
                    .chain()
                    .in_set(StreamingSet::Edits),
            )
            .add_systems(
                Update,
                (receive_jobs, receive_water_jobs)
                    .chain()
                    .in_set(StreamingSet::Receive),
            )
            .add_systems(
                Update,
                (dispatch_jobs, dispatch_water_jobs)
                    .chain()
                    .in_set(StreamingSet::Dispatch),
            );
    }
}
