//! TiltUI catalog and game HUD integration.
mod biome_previews;
mod cadence;
mod chat;
mod choice_labels;
mod debug;
mod fonts;
mod frames;
mod hud_layout;
mod hud_visibility;
mod inventory;
mod kingdom;
pub mod menu;
pub mod minimap;
mod mob_health;
mod multiplayer;
mod pets;
mod plugin;
mod presentation;
mod process_stats;
mod rail_hint;
mod realm_relations;
mod realm_title;
mod settings;
pub mod settlement_map;
mod sleep;
mod survival;
mod target_hud;
mod trains;
pub use trains::{TrainUiAction, TrainView};
mod text_editing;
pub use mob_health::MobHealthHud;
pub use pets::{PetMenuAction, PetMenuView, PetPortrait};
pub use rail_hint::RailBuildView;
pub use sleep::SleepView;
pub use survival::{EffectCardView, StatusEffectView, SurvivalView, VitalRenderStatus};
pub use target_hud::TargetView;
tilt_ui::include_components!();
pub use __tilt_ui_generated_components::app_shell_component::MenuIntent;
pub use __tilt_ui_generated_components::debug_overlay_component::DebugOverlay;
pub use __tilt_ui_generated_components::settings_menu_component::SettingsView;
pub use chat::{ChatAction, ChatView};
pub use fonts::UiFontCacheStatus;
pub use hud_visibility::HudVisibility;
pub use inventory::crafting::{CraftingAction, CraftingView};
pub use inventory::{
    BlockPreviewImages, InventoryAction, InventoryView, ItemPreviewImages, PlayerPreviewImage,
    PropPreviewImages, TooltipOwner,
};
pub use kingdom::{KingdomInputSet, KingdomMemberView, KingdomMenuAction, KingdomView};
pub use plugin::TerraUiPlugin;
pub use presentation::{PresentationOverride, preferred_present_mode};
pub use process_stats::AppProcessStats;
pub use realm_title::RealmTitleView;
pub use settings::{SettingsAction, SettingsConfigPath};
pub use tilt_ui::ParkedUi;
pub use tilt_ui::{
    ControlChecked, ElementId, HtmlEvent, NumericRange, OptionData, UiDocumentState, UiLoadState,
};
