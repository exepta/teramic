use crate::connection;
use bevy::prelude::*;
use std::time::Duration;
use terra_core::GameConfig;

pub struct TerraNetworkPlugin(pub connection::NetworkRole);
impl Plugin for TerraNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.set_error_handler(bevy::ecs::error::warn);
        let tick_duration = Duration::from_secs_f64(
            1.0 / f64::from(app.world().resource::<GameConfig>().server.tick_rate),
        );
        match self.0 {
            connection::NetworkRole::Client => {
                #[cfg(feature = "client")]
                app.add_plugins(lightyear::prelude::client::ClientPlugins { tick_duration });
            }
            connection::NetworkRole::DedicatedServer => {
                #[cfg(feature = "server")]
                app.add_plugins(lightyear::prelude::server::ServerPlugins { tick_duration });
            }
        }
        let _ = tick_duration;
        app.init_resource::<connection::ConnectionStatus>();
        crate::protocol::session::register(app);
        if matches!(self.0, connection::NetworkRole::DedicatedServer) {
            #[cfg(feature = "server")]
            crate::server::install(app);
        }
        if matches!(self.0, connection::NetworkRole::Client) {
            #[cfg(feature = "client")]
            crate::client::install(app);
        }
    }
}
