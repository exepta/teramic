use crate::{__tilt_ui_generated_components, tilt_ui_component_catalog};
use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*};
use tilt_ui::prelude::*;
pub struct TerraUiPlugin;
impl Plugin for TerraUiPlugin {
    fn build(&self, app: &mut App) {
        crate::cadence::install(app);
        crate::fonts::install(app);
        crate::text_editing::install(app);
        crate::settings::install(app);
        crate::frames::install(app);
        crate::chat::install(app);
        crate::sleep::install(app);
        crate::inventory::install(app);
        crate::multiplayer::install(app);
        crate::survival::install(app);
        crate::debug::install(app);
        crate::target_hud::install(app);
        crate::mob_health::install(app);
        crate::pets::install(app);
        crate::minimap::install(app);
        let benchmark_ui = std::env::var("TERRA_BENCH_UI").is_ok_and(|value| value == "1")
            && std::env::args().any(|argument| argument == "--smoke-test-benchmark")
            && std::env::var("TERRA_BENCH_MODE").is_ok_and(|mode| mode == "integrated");
        if benchmark_ui
            || !std::env::args().any(|a| {
                a.starts_with("--smoke-test")
                    && !a.starts_with("--smoke-test-menu")
                    && a != "--smoke-test-loading"
                    && a != "--smoke-test-chat"
                    && a != "--smoke-test-minimap"
                    && a != "--smoke-test-calendar-hud"
                    && a != "--smoke-test-inventory"
                    && a != "--smoke-test-camera"
                    && a != "--smoke-test-drops"
                    && a != "--smoke-test-survival"
                    && a != "--smoke-test-effects"
                    && a != "--smoke-test-taming"
                    && a != "--smoke-test-pet-commands"
                    && a != "--smoke-test-breeding"
                    && a != "--smoke-test-riding"
                    && a != "--smoke-test-mob-health"
                    && a != "--smoke-test-equipment"
                    && a != "--smoke-test-crusher"
                    && a != "--smoke-test-farming"
                    && a != "--smoke-test-food"
                    && a != "--smoke-test-furnace-gameplay"
            })
        {
            crate::menu::install(app);
        }
        crate::process_stats::install(app);
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<__tilt_ui_generated_components::debug_overlay_component::DebugOverlay>(
            )
            .init_resource::<__tilt_ui_generated_components::network_notice_component::NetworkNotice>()
            .init_resource::<__tilt_ui_generated_components::network_notice_component::PendingNotices>()
            .add_message::<terra_network::client::ReconnectRequest>()
            .add_plugins(
                TiltUiPlugin::new(tilt_ui_component_catalog())
                    .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-ui"))
                    .without_camera(),
            )
            .add_systems(
                Update,
                __tilt_ui_generated_components::debug_overlay_component::update_debug_overlay
                    .after(terra_player::PlayerSet::Targeting),
            )
            .add_systems(
                PostUpdate,
                (crate::choice_labels::single_line_labels, crate::choice_labels::refresh_popup_layout, crate::choice_labels::refresh_bound_labels).chain().before(bevy::ui::UiSystems::Prepare),
            );
        app.add_systems(
            Update,
            __tilt_ui_generated_components::network_notice_component::update_notice,
        );
        app.add_systems(
            PostUpdate,
            __tilt_ui_generated_components::network_notice_component::present_toasts
                .before(bevy::ui::UiSystems::Prepare),
        );
    }
}
