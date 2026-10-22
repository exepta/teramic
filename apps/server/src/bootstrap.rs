//! Rendering-free dedicated server bootstrap, also exercised by integration tests.
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use std::time::Duration;
use terra_core::{GameConfig, TerraCorePlugin};
use terra_network::{TerraNetworkPlugin, connection::NetworkRole};
use terra_physics::TerraPhysicsPlugin;
use terra_world::{TerraWorldPlugin, WorldMode};

pub fn headless_app(config: GameConfig) -> App {
    build_server(config, true)
}
pub(crate) fn build_server(config: GameConfig, logging: bool) -> App {
    let mut app = App::new();
    let tick = Duration::from_secs_f64(1.0 / f64::from(config.server.tick_rate));
    let log_filter = config.server.log_filter.clone();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(tick)));
    if logging {
        app.add_plugins(LogPlugin {
            filter: log_filter,
            ..default()
        });
    }
    app.add_plugins(TransformPlugin)
        .add_plugins(TerraCorePlugin(config))
        .add_plugins((
            TerraWorldPlugin {
                mode: WorldMode::DedicatedServer,
            },
            TerraPhysicsPlugin,
            TerraNetworkPlugin(NetworkRole::DedicatedServer),
        ))
        .add_systems(Startup, log_startup);
    app
}
fn log_startup(config: Res<GameConfig>) {
    let manifest = terra_storage::WorldManifest::new(config.world.clone());
    tracing::info!(seed=config.world.seed.0,generator_version=config.world.generator_version,
        tick_rate=config.server.tick_rate,save_format_version=manifest.save_format_version,
        configured_address=%config.server.bind_address,"Authoritative server initialized");
}
