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
            .init_resource::<StreamingRadiusLimit>()
            .init_resource::<EditJournal>()
            .init_resource::<crate::fruit::FruitProps>()
            .init_resource::<crate::water::FluidDiagnostics>()
            .init_resource::<crate::weather::ice::IceStatus>()
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
            .add_systems(Update, sync_client_settings.before(StreamingSet::Interest))
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
                    crate::weather::ice::tick,
                    crate::fire::tick,
                    crate::growth::tick,
                    crate::saplings::tick,
                    crate::fruit::tick,
                    crate::farming::tick,
                    crate::roads::tick,
                    crate::bile::tick,
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

/// The camera and network view already follow live graphics settings. Keep the
/// actual terrain interest in step as well, including shrinking without moving.
fn sync_client_settings(
    config: Res<GameConfig>,
    limit: Res<StreamingRadiusLimit>,
    mut settings: ResMut<StreamingSettings>,
) {
    if (!config.is_changed() && !limit.is_changed()) || !settings.meshes {
        return;
    }
    let radius = i64::from(limit.0.map_or(config.client.view_distance, |limit| {
        config.client.view_distance.min(limit)
    }));
    let vertical = i64::from(config.client.vertical_view_distance);
    let jobs = config.client.max_chunk_jobs;
    if settings.radius != radius
        || settings.vertical_radius != vertical
        || settings.max_jobs != jobs
    {
        settings.radius = radius;
        settings.vertical_radius = vertical;
        settings.max_jobs = jobs;
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn startup_limit_caps_real_columns_and_releases_the_full_requested_view_in_background() {
        let mut config = GameConfig::default();
        config.client.view_distance = 32;
        let mut app = App::new();
        app.insert_resource(config)
            .insert_resource(StreamingRadiusLimit(Some(15)))
            .insert_resource(StreamingSettings {
                radius: 32,
                vertical_radius: 3,
                max_jobs: 4,
                meshes: true,
            })
            .add_systems(Update, sync_client_settings);
        app.update();
        let radius = app.world().resource::<StreamingSettings>().radius;
        let wanted = super::super::interest::wanted_sections(Default::default(), radius, 3);
        let columns: std::collections::HashSet<_> = wanted.iter().map(|p| (p.x, p.z)).collect();
        assert_eq!(columns.len(), 961);
        app.world_mut().resource_mut::<StreamingRadiusLimit>().0 = Some(16);
        app.update();
        assert_eq!(app.world().resource::<StreamingSettings>().radius, 16);
        app.world_mut().resource_mut::<StreamingRadiusLimit>().0 = None;
        app.update();
        assert_eq!(app.world().resource::<StreamingSettings>().radius, 32);
        assert_eq!(
            app.world().resource::<GameConfig>().client.view_distance,
            32
        );
    }

    #[test]
    fn live_view_change_updates_real_interest_and_preserves_the_server_budget() {
        let mut app = App::new();
        app.init_resource::<GameConfig>()
            .init_resource::<StreamingRadiusLimit>()
            .insert_resource(StreamingSettings {
                radius: 8,
                vertical_radius: 3,
                max_jobs: 4,
                meshes: true,
            })
            .add_systems(Update, sync_client_settings);
        app.update();
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 32;
        app.update();
        assert_eq!(app.world().resource::<StreamingSettings>().radius, 32);
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 4;
        app.update();
        assert_eq!(app.world().resource::<StreamingSettings>().radius, 4);
        {
            let mut settings = app.world_mut().resource_mut::<StreamingSettings>();
            settings.meshes = false;
            settings.radius = 10;
        }
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 32;
        app.update();
        assert_eq!(app.world().resource::<StreamingSettings>().radius, 10);
    }
}
