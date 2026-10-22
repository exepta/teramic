use bevy::prelude::*;
use std::path::PathBuf;
use terra_core::{WorldId, config::servers::ServerList, menu_config_directory};
use terra_storage::{WorldEntry, WorldLibrary};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MenuPage {
    #[default]
    Main,
    Worlds,
    Servers,
    Options,
    HudLayout,
    Pause,
    Loading,
    Playing,
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum MenuDialog {
    #[default]
    None,
    CreateWorld,
    DeleteWorld,
    EditServer,
    Discard,
    Display,
    Minimap,
}
#[derive(Resource, Default)]
pub struct MenuState {
    pub page: MenuPage,
    pub options_return: MenuPage,
    pub dialog: MenuDialog,
    pub status: String,
    pub error: String,
    pub session_active: bool,
    pub quitting: bool,
    pub profile_loaded: bool,
}
impl MenuState {
    pub(crate) fn world_hud_active(&self) -> bool {
        self.world_audio_active()
    }
    pub fn world_audio_active(&self) -> bool {
        self.page == MenuPage::Playing
            || (self.session_active
                && matches!(
                    self.page,
                    MenuPage::Pause | MenuPage::Options | MenuPage::HudLayout
                ))
    }
}
#[derive(Resource, Default, Clone)]
pub struct SessionInfo {
    pub title: String,
    pub mode: String,
    pub local: bool,
    pub started: Option<std::time::Instant>,
    pub saving: bool,
}
#[derive(Resource, Default)]
pub struct LoadingProgress {
    pub fraction: Option<f32>,
    pub detail: String,
    pub reveal: f32,
}
#[derive(Resource, Clone)]
pub struct MenuPaths {
    pub configs: PathBuf,
    pub worlds: WorldLibrary,
}
impl Default for MenuPaths {
    fn default() -> Self {
        Self {
            configs: menu_config_directory(),
            worlds: WorldLibrary::default(),
        }
    }
}
#[derive(Resource, Default)]
pub struct WorldMenu {
    pub natural_mob_limit: u16,
    pub entries: Vec<WorldEntry>,
    pub selected: Option<WorldId>,
    pub name: String,
    pub seed: String,
    pub mode: String,
}
#[derive(Resource, Default)]
pub struct ServerMenu {
    pub list: ServerList,
    pub selected: Option<u64>,
    pub lan_selected: Option<std::net::SocketAddr>,
    pub editing: Option<u64>,
    pub name: String,
    pub address: String,
    pub port: String,
}
#[derive(Message, Clone, Debug)]
pub enum MenuAction {
    Command(String),
    Change { control: String, value: String },
}
#[derive(Message, Clone, Debug)]
pub enum SessionRequest {
    PlayWorld(WorldId),
    JoinServer(u64),
    JoinAddress(std::net::SocketAddr),
    Save,
    Leave,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_and_menu_pages_never_enable_world_audio_even_after_network_ready() {
        for page in [
            MenuPage::Loading,
            MenuPage::Main,
            MenuPage::Worlds,
            MenuPage::Servers,
        ] {
            assert!(
                !MenuState {
                    page,
                    session_active: true,
                    ..Default::default()
                }
                .world_audio_active()
            );
        }
        for page in [
            MenuPage::Playing,
            MenuPage::Pause,
            MenuPage::Options,
            MenuPage::HudLayout,
        ] {
            assert!(
                MenuState {
                    page,
                    session_active: true,
                    ..Default::default()
                }
                .world_audio_active()
            );
        }
        assert!(
            !MenuState {
                page: MenuPage::Options,
                ..Default::default()
            }
            .world_audio_active()
        );
    }
}
