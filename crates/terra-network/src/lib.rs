//! Lightyear integration and versioned protocol contracts; no connection is simulated.
pub mod browser;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub mod discovery;
#[cfg(feature = "server")]
mod gateway;
mod plugin;
pub mod protocol;
#[cfg(feature = "server")]
pub mod server;
pub use plugin::TerraNetworkPlugin;
pub use protocol::{
    chunk_messages, connection, player_replication, terraform_commands, world_sync,
};
#[cfg(all(test, feature = "client", feature = "server"))]
mod tests;
mod weather;
mod world_time;
