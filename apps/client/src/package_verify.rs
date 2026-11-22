//! Exercise the installed client, transport and world replica without a graphics device.
use bevy::prelude::*;
use std::time::{Duration, Instant};
use terra_core::{Config, GameConfig, PlayerProfile, TerraCorePlugin};
use terra_network::{
    TerraNetworkPlugin,
    client::{ClientLaunch, ClientSession, disconnect_session},
    connection::NetworkRole,
};
use terra_world::{TerraWorldPlugin, WorldMode, streaming::ChunkStore};

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let address = crate::bootstrap::connect_address()?
        .ok_or("--verify-package requires --connect IP:PORT")?;
    let directory = terra_core::config_directory();
    let mut config = GameConfig {
        client: Config::load(directory.join("client.toml"))?,
        server: Config::load(directory.join("server.toml"))?,
        world: Config::load(directory.join("world.toml"))?,
    };
    config.client.view_distance = 1;
    config.client.vertical_view_distance = 1;
    let profile = PlayerProfile::load_for_client(PlayerProfile::selected_path(
        &terra_core::menu_config_directory(),
    )?)?;
    let token = terra_network::browser::join(address, &profile)?;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(config))
        .add_plugins((
            TerraWorldPlugin {
                mode: WorldMode::Client,
            },
            terra_physics::TerraPhysicsPlugin,
            TerraNetworkPlugin(NetworkRole::Client),
        ))
        .insert_resource(profile)
        .insert_resource(ClientLaunch { token });
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        app.update();
        let session = app.world().resource::<ClientSession>();
        if let Some(error) = &session.error {
            return Err(error.clone().into());
        }
        let chunks = app.world().resource::<ChunkStore>();
        if session.ready
            && let Some(state) = session.confirmed_state()
            && session.player == Some(state.player)
            && chunks
                .block_at(state.position.map(|v| v.floor() as i64))
                .is_some()
        {
            let section_count = chunks.loaded.len();
            disconnect_session(app.world_mut());
            app.update();
            println!(
                "Package client verified ({address}, player {}, {} loaded sections)",
                state.player.0, section_count
            );
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out verifying {address}: ready={}, confirmed={}, loaded sections={}",
                session.ready,
                session.confirmed_state().is_some(),
                chunks.loaded.len()
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}
