//! TiltUI catalog and game HUD integration.
mod cadence;
mod chat;
mod choice_labels;
mod debug;
mod fonts;
mod frames;
mod hud_layout;
mod inventory;
pub mod menu;
pub mod minimap;
mod multiplayer;
mod plugin;
mod process_stats;
mod settings;
mod sleep;
mod survival;
mod target_hud;
mod text_editing;
pub use sleep::SleepView;
pub use survival::{EffectCardView, StatusEffectView, SurvivalView, VitalRenderStatus};
pub use target_hud::TargetView;
tilt_ui::include_components!();
pub use __tilt_ui_generated_components::app_shell_component::MenuIntent;
pub use __tilt_ui_generated_components::debug_overlay_component::DebugOverlay;
pub use __tilt_ui_generated_components::settings_menu_component::SettingsView;
pub use chat::{ChatAction, ChatView};
pub use fonts::UiFontCacheStatus;
pub use inventory::crafting::{CraftingAction, CraftingView};
pub use inventory::{
    BlockPreviewImages, InventoryAction, InventoryView, ItemPreviewImages, PlayerPreviewImage,
    PropPreviewImages, TooltipOwner,
};
pub use plugin::TerraUiPlugin;
pub use process_stats::AppProcessStats;
pub use settings::{SettingsAction, SettingsConfigPath};
pub use tilt_ui::{
    ControlChecked, ElementId, HtmlEvent, NumericRange, OptionData, UiDocumentState, UiLoadState,
};
