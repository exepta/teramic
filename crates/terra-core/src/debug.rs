use bevy::prelude::Resource;

/// Session-local diagnostics. Debug tools never appear until explicitly enabled.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DebugSettings {
    pub enabled: bool,
    pub block_placement: bool,
    pub creature_inspection: bool,
}
