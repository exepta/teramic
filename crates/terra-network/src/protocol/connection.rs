use bevy::prelude::*;
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectionStatus {
    #[default]
    Unconfigured,
    Connecting,
    Synchronizing,
    Connected,
    Disconnected,
    Failed,
}
#[derive(Debug, Clone, Copy)]
pub enum NetworkRole {
    Client,
    DedicatedServer,
}
