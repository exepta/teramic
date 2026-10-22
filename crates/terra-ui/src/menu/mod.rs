mod browser;
mod library;
mod model;
mod navigation;
mod options;
#[cfg(test)]
mod tests;
mod view;
use bevy::prelude::*;
pub use model::{
    LoadingProgress, MenuAction, MenuDialog, MenuPage, MenuPaths, MenuState, ServerMenu,
    SessionInfo, SessionRequest, WorldMenu,
};
pub use options::{OptionsDraft, OptionsTab};
pub use view::{DialogView, MenuView, OptionsView, ServersView, WorldsView};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MenuInputSet;

pub fn install(app: &mut App) {
    app.init_resource::<MenuState>()
        .init_resource::<MenuPaths>()
        .init_resource::<WorldMenu>()
        .init_resource::<ServerMenu>()
        .init_resource::<OptionsDraft>()
        .init_resource::<SessionInfo>()
        .init_resource::<LoadingProgress>()
        .init_resource::<terra_core::PlayerProfile>()
        .init_resource::<view::SessionView>()
        .init_resource::<MenuView>()
        .init_resource::<WorldsView>()
        .init_resource::<ServersView>()
        .init_resource::<OptionsView>()
        .init_resource::<DialogView>()
        .init_resource::<browser::ServerBrowser>()
        .add_message::<MenuAction>()
        .add_message::<SessionRequest>()
        .add_systems(Startup, options::load_preferences)
        .add_systems(
            Update,
            (
                options::capture_binding,
                navigation::keyboard_navigation,
                navigation::actions,
                options::display_timeout,
                options::sync_fullscreen,
                options::apply_ui_scale,
                browser::update,
                view::update.run_if(crate::cadence::ready),
            )
                .chain()
                .in_set(MenuInputSet)
                .before(terra_player::PlayerSet::Movement),
        );
}
