use bevy::prelude::Resource;

/// Session-local diagnostics. Debug tools never appear until explicitly enabled.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DebugSettings {
    pub enabled: bool,
    pub block_placement: bool,
    pub creature_inspection: bool,
    pub creature_bounds: bool,
    pub chunk_borders: bool,
}

/// Session-local passive-HUD presentation shared by UI and first-person rendering.
/// This state never changes saved widget preferences or third-person world models.
#[derive(Resource, Debug, Clone, Copy)]
pub struct HudVisibility {
    pub visible: bool,
}
impl Default for HudVisibility {
    fn default() -> Self {
        Self { visible: true }
    }
}
