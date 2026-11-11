use super::*;
use bevy::{prelude::*, window::PrimaryWindow};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use terra_core::{
    AudioConfig, Config, ControlsConfig, GameAction, GameConfig, GraphicsConfig, InputBinding,
};
fn app() -> App {
    let root = std::env::temp_dir().join(format!(
        "teramic-menu-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut app = App::new();
    app.init_resource::<GameConfig>()
        .init_resource::<crate::hud_layout::HudLayout>()
        .init_resource::<MenuState>()
        .init_resource::<WorldMenu>()
        .init_resource::<ServerMenu>()
        .init_resource::<OptionsDraft>()
        .init_resource::<terra_player::ControllerMode>()
        .init_resource::<terra_player::PlayerInputBlock>()
        .insert_resource(MenuPaths {
            configs: root.join("configs"),
            worlds: terra_storage::WorldLibrary {
                root: root.join("worlds"),
            },
        })
        .add_message::<MenuAction>()
        .add_message::<SessionRequest>()
        .add_message::<terra_player::CaptureRequest>()
        .add_systems(Update, navigation::actions);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    options::load_preferences(app.world_mut());
    app
}

#[test]
fn language_changes_stage_apply_and_discard_through_the_real_settings_commands() {
    use terra_core::{GameLanguage, GameplayConfig, LanguagePreference, PlayerProfile};
    let mut app = app();
    let profile = PlayerProfile::default();
    let configs = app.world().resource::<MenuPaths>().configs.clone();
    let gameplay_path = configs.join("gameplay.toml");
    let language_path = configs
        .join("profile-preferences")
        .join(profile.uuid.simple().to_string())
        .join("language.toml");
    app.insert_resource(profile);
    command(&mut app, "options");
    command(&mut app, "gameplay-options");
    let rows = view::gameplay_rows(app.world().resource::<GameplayConfig>());
    let language = rows.iter().find(|row| row.id == "game-language").unwrap();
    assert_eq!(language.value, "de");
    assert_eq!(
        language
            .choices
            .iter()
            .map(|choice| choice.value.as_str())
            .collect::<Vec<_>>(),
        ["de", "en"]
    );
    change(&mut app, "game-language", "en");
    assert_eq!(
        app.world().resource::<OptionsDraft>().gameplay.language,
        GameLanguage::English
    );
    assert_eq!(
        app.world().resource::<GameplayConfig>().language,
        GameLanguage::German
    );
    assert_eq!(
        GameplayConfig::load(&gameplay_path).unwrap().language,
        GameLanguage::German
    );
    assert!(!language_path.exists());
    change(&mut app, "game-language", "fr");
    assert_eq!(
        app.world().resource::<MenuState>().error,
        "Unsupported language"
    );
    assert_eq!(
        app.world().resource::<OptionsDraft>().gameplay.language,
        GameLanguage::English
    );
    change(&mut app, "game-language", "en");
    command(&mut app, "apply");
    assert!(app.world().resource::<MenuState>().error.is_empty());
    assert_eq!(
        app.world().resource::<GameplayConfig>().language,
        GameLanguage::English
    );
    assert_eq!(
        GameplayConfig::load(&gameplay_path).unwrap().language,
        GameLanguage::English
    );
    assert_eq!(
        LanguagePreference::load(&language_path).unwrap().language,
        GameLanguage::English
    );
    change(&mut app, "game-language", "de");
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Discard
    );
    command(&mut app, "discard-confirm");
    assert_eq!(
        app.world().resource::<GameplayConfig>().language,
        GameLanguage::English
    );
    assert_eq!(
        GameplayConfig::load(&gameplay_path).unwrap().language,
        GameLanguage::English
    );
    assert_eq!(
        LanguagePreference::load(&language_path).unwrap().language,
        GameLanguage::English
    );
    command(&mut app, "options");
    assert_eq!(
        app.world().resource::<OptionsDraft>().gameplay.language,
        GameLanguage::English
    );
    cleanup(app);
}

#[test]
fn four_k_monitor_offers_native_resolution_even_when_wayland_reports_only_one_mode() {
    use bevy::window::{Monitor, OnMonitor};
    let mut app = app();
    let monitor = app
        .world_mut()
        .spawn(Monitor {
            name: Some("4K at 150% scale".into()),
            physical_width: 3840,
            physical_height: 2160,
            physical_position: IVec2::ZERO,
            refresh_rate_millihertz: Some(60_000),
            scale_factor: 1.5,
            video_modes: Vec::new(),
        })
        .id();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(window)
        .insert(OnMonitor(monitor));
    let choices = view::resolution_choices(app.world_mut());
    assert!(choices.iter().any(|choice| choice.value == "3840x2160"));
    assert!(choices.iter().any(|choice| choice.value == "2560x1440"));
    assert!(!choices.iter().any(|choice| choice.value == "7680x4320"));
    cleanup(app);
}

#[test]
fn view_fog_and_animation_apply_are_persisted_and_unsaved_changes_are_discardable() {
    let mut app = app();
    command(&mut app, "options");
    change(&mut app, "graphics-distance", "32");
    change(&mut app, "graphics-fog", "75");
    change(&mut app, "graphics-chunk-animation", "fade");
    change(&mut app, "graphics-mist", "true");
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<GameConfig>().client.view_distance,
        32
    );
    assert_eq!(
        app.world().resource::<GameConfig>().client.fog_distance,
        0.75
    );
    assert!(
        app.world()
            .resource::<GameConfig>()
            .client
            .water_volumetric_mist
    );
    let file = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    assert_eq!(
        GraphicsConfig::load(file).unwrap().chunk_animation,
        terra_core::ChunkAnimation::Fade
    );
    change(&mut app, "graphics-chunk-animation", "none");
    command(&mut app, "back");
    command(&mut app, "discard-confirm");
    assert_eq!(
        app.world().resource::<GameConfig>().client.chunk_animation,
        terra_core::ChunkAnimation::Fade
    );
    cleanup(app);
}

#[test]
fn tree_fall_modes_are_persisted_and_default_to_current() {
    let mut app = app();
    assert_eq!(
        app.world().resource::<GraphicsConfig>().tree_fall_animation,
        terra_core::TreeFallAnimation::Current
    );
    command(&mut app, "options");
    for (value, mode) in [
        ("hell_fall", terra_core::TreeFallAnimation::HellFall),
        ("magic", terra_core::TreeFallAnimation::Magic),
        ("current", terra_core::TreeFallAnimation::Current),
    ] {
        change(&mut app, "graphics-tree-fall", value);
        command(&mut app, "apply");
        let file = app
            .world()
            .resource::<MenuPaths>()
            .configs
            .join("graphics.toml");
        assert_eq!(
            GraphicsConfig::load(file).unwrap().tree_fall_animation,
            mode
        );
        assert_eq!(
            app.world().resource::<GraphicsConfig>().tree_fall_animation,
            mode
        );
    }
    cleanup(app);
}

#[test]
fn graphics_cave_and_tree_controls_have_visible_options_and_persist() {
    let template =
        include_str!("../../../../src-ui/components/settings-row/settings-row.component.html");
    let mut app = app();
    command(&mut app, "options");
    for (control, values) in [
        ("graphics-caves", &["low", "medium", "high", "ultra"][..]),
        ("graphics-tree-fall", &["current", "hell_fall", "magic"][..]),
    ] {
        let marker = format!("@if (row.id == '{control}')");
        let start = template.find(&marker).expect("Visible graphics dropdown");
        let markup = template[start..].lines().next().unwrap();
        assert!(markup.contains("<choice-box"));
        assert!(markup.contains("menu_change"));
        for value in values {
            assert!(markup.contains(&format!("<option value=\"{value}\"")));
        }
    }
    for (value, mode) in [
        ("low", terra_core::CaveQuality::Low),
        ("medium", terra_core::CaveQuality::Medium),
        ("high", terra_core::CaveQuality::High),
        ("ultra", terra_core::CaveQuality::Ultra),
    ] {
        change(&mut app, "graphics-caves", value);
        command(&mut app, "apply");
        let file = app
            .world()
            .resource::<MenuPaths>()
            .configs
            .join("graphics.toml");
        assert_eq!(GraphicsConfig::load(file).unwrap().cave_quality, mode);
        assert_eq!(app.world().resource::<GraphicsConfig>().cave_quality, mode);
    }
    cleanup(app);
}
fn command(app: &mut App, id: &str) {
    app.world_mut()
        .write_message(MenuAction::Command(id.into()));
    app.update();
}

#[test]
fn multiplayer_options_apply_reset_and_reload_without_previewing_unsaved_changes() {
    use terra_core::MultiplayerConfig;
    let mut app = app();
    let defaults = app.world().resource::<MultiplayerConfig>().clone();
    assert!(
        view::multiplayer_rows(&defaults)
            .iter()
            .find(|row| row.id == "multiplayer-color")
            .unwrap()
            .enabled
    );
    command(&mut app, "options");
    command(&mut app, "multiplayer-options");
    assert_eq!(
        app.world().resource::<OptionsDraft>().tab,
        OptionsTab::Multiplayer
    );
    change(&mut app, "multiplayer-names", "false");
    change(&mut app, "multiplayer-name-distance", "72");
    change(&mut app, "multiplayer-cursors", "false");
    change(&mut app, "multiplayer-color", "#da5835");
    assert_eq!(*app.world().resource::<MultiplayerConfig>(), defaults);
    command(&mut app, "apply");
    let config = app.world().resource::<MultiplayerConfig>();
    assert!(!config.player_names && !config.chest_cursors);
    assert_eq!(config.player_name_distance, 72.0);
    assert_eq!(config.cursor_color, Some([218, 88, 53]));
    let file = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("multiplayer.toml");
    assert_eq!(MultiplayerConfig::load(file).unwrap(), *config);
    command(&mut app, "reset");
    assert_eq!(app.world().resource::<OptionsDraft>().multiplayer, defaults);
    assert!(!app.world().resource::<MultiplayerConfig>().player_names);
    command(&mut app, "apply");
    assert_eq!(*app.world().resource::<MultiplayerConfig>(), defaults);
    cleanup(app);
}

#[test]
fn gameplay_minimap_popup_stages_apply_reset_and_discard_with_real_config_reload() {
    use terra_core::{GameplayConfig, MapMode};
    let mut app = app();
    let defaults = app.world().resource::<GameplayConfig>().clone();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("gameplay.toml");
    assert_eq!(GameplayConfig::load(&path).unwrap(), defaults);
    command(&mut app, "options");
    command(&mut app, "gameplay-options");
    assert_eq!(
        app.world().resource::<OptionsDraft>().tab,
        OptionsTab::Gameplay
    );
    let rows = view::gameplay_rows(&defaults);
    assert!(rows.iter().any(|row| row.id == "hud-edit"));
    assert!(rows.iter().any(|row| row.id == "minimap-settings"));
    assert!(rows.iter().any(|row| row.id == "heartbeat" && row.checked));
    assert!(
        rows.iter()
            .any(|row| row.id == "view-bobbing" && row.checked)
    );
    change(&mut app, "heartbeat", "false");
    change(&mut app, "view-bobbing", "false");
    change(&mut app, "mob-health-bars", "in_view");
    assert!(app.world().resource::<GameplayConfig>().heartbeat);
    assert!(app.world().resource::<GameplayConfig>().view_bobbing);
    command(&mut app, "minimap-settings");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Minimap
    );
    change(&mut app, "minimap-mode", "cave");
    change(&mut app, "minimap-size", "320");
    change(&mut app, "minimap-fps", "40");
    change(&mut app, "minimap-icon-scale", "2.3");
    change(&mut app, "minimap-rotate", "true");
    change(&mut app, "minimap-mobs", "false");
    assert_eq!(*app.world().resource::<GameplayConfig>(), defaults);
    assert_eq!(GameplayConfig::load(&path).unwrap(), defaults);
    command(&mut app, "dialog-cancel");
    command(&mut app, "apply");
    let applied = app.world().resource::<GameplayConfig>().clone();
    assert_eq!(applied.minimap.mode, MapMode::Cave);
    assert_eq!(applied.minimap.size, 320);
    assert_eq!(applied.minimap.fps, 40);
    assert_eq!(applied.minimap.icon_scale, 2.3);
    assert!(!applied.heartbeat);
    assert!(!applied.view_bobbing);
    assert_eq!(applied.mob_health_bars, terra_core::MobHealthBars::InView);
    assert!(applied.minimap.rotate && !applied.minimap.mobs);
    assert_eq!(GameplayConfig::load(&path).unwrap(), applied);
    command(&mut app, "minimap-settings");
    command(&mut app, "minimap-reset");
    assert_eq!(
        app.world().resource::<OptionsDraft>().gameplay.minimap,
        defaults.minimap
    );
    assert!(!app.world().resource::<OptionsDraft>().gameplay.heartbeat);
    assert!(!app.world().resource::<OptionsDraft>().gameplay.view_bobbing);
    assert_eq!(*app.world().resource::<GameplayConfig>(), applied);
    assert_eq!(GameplayConfig::load(&path).unwrap(), applied);
    command(&mut app, "dialog-cancel");
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Discard
    );
    command(&mut app, "discard-confirm");
    command(&mut app, "options");
    assert_eq!(app.world().resource::<OptionsDraft>().gameplay, applied);
    command(&mut app, "gameplay-options");
    command(&mut app, "reset");
    command(&mut app, "apply");
    assert_eq!(*app.world().resource::<GameplayConfig>(), defaults);
    assert_eq!(GameplayConfig::load(path).unwrap(), defaults);
    cleanup(app);
}

#[test]
fn mob_health_editor_stages_real_color_controls_and_preserves_apply_reset_discard() {
    use terra_core::{GameplayConfig, MobHealthBars};
    let mut app = app();
    let defaults = GameplayConfig::default();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("gameplay.toml");
    command(&mut app, "options");
    command(&mut app, "gameplay-options");
    let rows = view::gameplay_rows(&defaults);
    assert!(rows.iter().any(|row| row.id == "mob-health-settings"
        && row.kind == "command"
        && row.label == "Edit Bars"));
    assert!(!rows.iter().any(|row| row.id == "mob-health-bars"));
    command(&mut app, "mob-health-settings");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::MobHealth
    );
    let rows = view::mob_health_rows(&defaults);
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[0]
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>(),
        ["View Port", "Looking At", "All Time", "None"]
    );
    assert!(
        rows[1..]
            .iter()
            .all(|row| row.kind == "color" && row.enabled)
    );
    change(&mut app, "mob-health-bars", "in_view");
    change(&mut app, "mob-health-friendly-color", "#123456");
    change(&mut app, "mob-health-hostile-color", "#DC6E1E");
    change(&mut app, "mob-health-font-color", "#eee0d0");
    let staged = app.world().resource::<OptionsDraft>().gameplay.clone();
    assert_eq!(staged.mob_health_bars, MobHealthBars::InView);
    assert_eq!(staged.mob_health_friendly_color, [18, 52, 86]);
    assert_eq!(staged.mob_health_hostile_color, [220, 110, 30]);
    assert_eq!(staged.mob_health_font_color, [238, 224, 208]);
    assert_eq!(*app.world().resource::<GameplayConfig>(), defaults);
    assert_eq!(GameplayConfig::load(&path).unwrap(), defaults);
    command(&mut app, "apply");
    assert_eq!(
        *app.world().resource::<GameplayConfig>(),
        defaults,
        "modal blocks underlying Apply"
    );
    command(&mut app, "dialog-cancel");
    command(&mut app, "apply");
    assert_eq!(*app.world().resource::<GameplayConfig>(), staged);
    assert_eq!(GameplayConfig::load(&path).unwrap(), staged);
    command(&mut app, "mob-health-settings");
    change(&mut app, "heartbeat", "false");
    command(&mut app, "mob-health-reset");
    let reset = &app.world().resource::<OptionsDraft>().gameplay;
    assert_eq!(reset.mob_health_bars, defaults.mob_health_bars);
    assert_eq!(
        reset.mob_health_friendly_color,
        defaults.mob_health_friendly_color
    );
    assert_eq!(
        reset.mob_health_hostile_color,
        defaults.mob_health_hostile_color
    );
    assert_eq!(reset.mob_health_font_color, defaults.mob_health_font_color);
    assert!(
        !reset.heartbeat,
        "bar-only Reset leaves other Gameplay preferences alone"
    );
    assert_eq!(*app.world().resource::<GameplayConfig>(), staged);
    command(&mut app, "dialog-cancel");
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Discard
    );
    command(&mut app, "discard-confirm");
    command(&mut app, "options");
    assert_eq!(app.world().resource::<OptionsDraft>().gameplay, staged);
    for invalid in ["#12", "#11223344", "#xxff00", "#Ä12345", "##112233"] {
        assert!(options::change(app.world_mut(), "mob-health-font-color", invalid).is_err());
        assert_eq!(app.world().resource::<OptionsDraft>().gameplay, staged);
    }
    assert!(
        include_str!("../../../../src-ui/components/settings-row/settings-row.component.html")
            .contains("row.id == 'mob-health-bars'")
    );
    cleanup(app);
}

#[test]
fn minimap_fps_and_icon_scale_reject_invalid_events_without_changing_the_draft() {
    let mut app = app();
    let original = app.world().resource::<OptionsDraft>().gameplay.clone();
    for (control, value) in [
        ("minimap-fps", "25"),
        ("minimap-fps", "30.5"),
        ("minimap-icon-scale", "0.9"),
        ("minimap-icon-scale", "2.6"),
        ("minimap-icon-scale", "NaN"),
    ] {
        assert!(options::change(app.world_mut(), control, value).is_err());
        assert_eq!(app.world().resource::<OptionsDraft>().gameplay, original);
    }
    let rows = view::minimap_rows(&original.minimap);
    let fps = rows.iter().find(|row| row.id == "minimap-fps").unwrap();
    assert_eq!(fps.choices.len(), 4);
    let scale = rows
        .iter()
        .find(|row| row.id == "minimap-icon-scale")
        .unwrap();
    assert_eq!((scale.min, scale.max, scale.step), (1.0, 2.5, 0.1));
    cleanup(app);
}

#[test]
fn minimap_changes_follow_display_confirmation_and_failed_save_rolls_back_other_files() {
    use terra_core::{GameplayConfig, MultiplayerConfig};
    let mut app = app();
    let original = app.world().resource::<GameplayConfig>().clone();
    command(&mut app, "options");
    change(&mut app, "minimap-enabled", "false");
    change(&mut app, "graphics-resolution", "1600x900");
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Display
    );
    assert_eq!(*app.world().resource::<GameplayConfig>(), original);
    command(&mut app, "display-revert");
    assert_eq!(app.world().resource::<OptionsDraft>().gameplay, original);
    change(&mut app, "minimap-enabled", "false");
    change(&mut app, "graphics-resolution", "1600x900");
    command(&mut app, "apply");
    command(&mut app, "display-keep");
    assert!(!app.world().resource::<GameplayConfig>().minimap.enabled);
    let root = app.world().resource::<MenuPaths>().configs.clone();
    let old_graphics = app.world().resource::<GraphicsConfig>().clone();
    let old_multiplayer = app.world().resource::<MultiplayerConfig>().clone();
    std::fs::remove_file(root.join("gameplay.toml")).unwrap();
    std::fs::create_dir(root.join("gameplay.toml")).unwrap();
    change(&mut app, "graphics-fov", "99");
    change(&mut app, "multiplayer-names", "false");
    change(&mut app, "minimap-enabled", "true");
    command(&mut app, "apply");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    assert!(!app.world().resource::<GameplayConfig>().minimap.enabled);
    assert_eq!(*app.world().resource::<GraphicsConfig>(), old_graphics);
    assert_eq!(
        GraphicsConfig::load(root.join("graphics.toml")).unwrap(),
        old_graphics
    );
    assert_eq!(
        MultiplayerConfig::load(root.join("multiplayer.toml")).unwrap(),
        old_multiplayer
    );
    cleanup(app);
}

fn change(app: &mut App, id: &str, value: &str) {
    app.world_mut().write_message(MenuAction::Change {
        control: id.into(),
        value: value.into(),
    });
    app.update();
}
fn cleanup(app: App) {
    let root = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .parent()
        .unwrap()
        .to_path_buf();
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn escape_in_inventory_does_not_also_open_pause_menu() {
    let mut app = app();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::inventory::InventoryState>()
        .add_systems(
            Update,
            navigation::keyboard_navigation.before(navigation::actions),
        );
    app.world_mut().resource_mut::<MenuState>().page = MenuPage::Playing;
    app.world_mut()
        .resource_mut::<crate::inventory::InventoryState>()
        .open = true;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Playing);
    assert!(app.world().resource::<terra_player::PlayerInputBlock>().0);
    cleanup(app);
}

#[test]
fn discovered_server_selects_and_joins_without_creating_a_saved_entry() {
    let mut app = app();
    app.init_resource::<browser::ServerBrowser>();
    command(&mut app, "multiplayer");
    let address = "127.0.0.1:52771".parse().unwrap();
    app.world_mut()
        .resource_mut::<browser::ServerBrowser>()
        .lan
        .push(terra_network::discovery::LanServer {
            address,
            status: terra_network::browser::ServerStatus {
                instance: terra_core::PlayerUuid::new_v4(),
                protocol: terra_network::protocol::session::PROTOCOL_VERSION,
                title: "Local test".into(),
                description: String::new(),
                players: 0,
                max_players: 8,
                port: address.port(),
                identity_mode: "local_profile".into(),
            },
            seen: Instant::now(),
            ping_ms: 1,
        });
    command(&mut app, &format!("lan-{address}"));
    assert_eq!(
        app.world().resource::<ServerMenu>().lan_selected,
        Some(address)
    );
    command(&mut app, "server-add");
    assert_eq!(app.world().resource::<ServerMenu>().address, "127.0.0.1");
    assert_eq!(
        app.world().resource::<ServerMenu>().port,
        address.port().to_string()
    );
    command(&mut app, "dialog-cancel");
    command(&mut app, "server-join");
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Loading);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<SessionRequest>>()
            .drain()
            .any(
                |request| matches!(request, SessionRequest::JoinAddress(found) if found == address)
            )
    );
    assert!(app.world().resource::<ServerMenu>().list.servers.is_empty());
    cleanup(app);
}

#[test]
fn malformed_preferences_are_reported_without_silently_overwriting_user_files() {
    let mut app = app();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    let invalid = "width = 'not a number'\n";
    std::fs::write(&path, invalid).unwrap();
    options::load_preferences(app.world_mut());
    assert!(
        app.world()
            .resource::<MenuState>()
            .error
            .contains("graphics.toml")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
    assert!(app.world().resource::<GraphicsConfig>().validate().is_ok());
    cleanup(app);
}

#[test]
fn failed_apply_preserves_runtime_and_rolls_back_graphics_file() {
    let mut app = app();
    let old_graphics = app.world().resource::<GraphicsConfig>().clone();
    let root = app.world().resource::<MenuPaths>().configs.clone();
    let controls = root.join("controls.toml");
    std::fs::remove_file(&controls).unwrap();
    std::fs::create_dir(&controls).unwrap();
    command(&mut app, "options");
    change(&mut app, "graphics-fov", "90");
    command(&mut app, "apply");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    assert_eq!(*app.world().resource::<GraphicsConfig>(), old_graphics);
    assert_eq!(
        GraphicsConfig::load(root.join("graphics.toml")).unwrap(),
        old_graphics
    );
    cleanup(app);
}

#[test]
fn audio_drafts_apply_discard_reset_and_reload_use_the_same_preferences() {
    let mut app = app();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("audio.toml");
    let defaults = AudioConfig::default();
    assert_eq!(AudioConfig::load(&path).unwrap(), defaults);
    command(&mut app, "options");
    command(&mut app, "audio");
    assert_eq!(
        app.world().resource::<OptionsDraft>().tab,
        OptionsTab::Audio
    );
    change(&mut app, "audio-master", "40");
    change(&mut app, "audio-animals", "20");
    change(&mut app, "audio-footsteps", "45");
    assert_eq!(*app.world().resource::<AudioConfig>(), defaults);
    assert_eq!(AudioConfig::load(&path).unwrap(), defaults);
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Discard
    );
    command(&mut app, "discard-confirm");
    command(&mut app, "options");
    command(&mut app, "audio");
    assert_eq!(app.world().resource::<OptionsDraft>().audio, defaults);
    for (id, value) in [
        ("audio-master", "40"),
        ("audio-ambience", "60"),
        ("audio-animals", "20"),
        ("audio-footsteps", "45"),
        ("audio-water", "0"),
        ("audio-river", "35"),
        ("audio-waterfall", "80"),
        ("audio-muted", "true"),
    ] {
        change(&mut app, id, value);
    }
    let expected = AudioConfig {
        master_volume: 0.4,
        ambience_volume: 0.6,
        animals_volume: 0.2,
        footsteps_volume: 0.45,
        water_volume: 0.0,
        river_volume: 0.35,
        waterfall_volume: 0.8,
        muted: true,
    };
    command(&mut app, "apply");
    assert_eq!(*app.world().resource::<AudioConfig>(), expected);
    assert_eq!(AudioConfig::load(&path).unwrap(), expected);
    options::load_preferences(app.world_mut());
    assert_eq!(*app.world().resource::<AudioConfig>(), expected);
    command(&mut app, "audio");
    command(&mut app, "reset");
    assert_eq!(app.world().resource::<OptionsDraft>().audio, defaults);
    assert_eq!(*app.world().resource::<AudioConfig>(), expected);
    assert_eq!(AudioConfig::load(&path).unwrap(), expected);
    command(&mut app, "apply");
    assert_eq!(AudioConfig::load(&path).unwrap(), defaults);
    cleanup(app);
}

#[test]
fn malformed_audio_is_reported_and_failed_audio_save_rolls_back_all_preferences() {
    let mut app = app();
    let root = app.world().resource::<MenuPaths>().configs.clone();
    let path = root.join("audio.toml");
    let invalid = "master_volume = 'loud'\n";
    std::fs::write(&path, invalid).unwrap();
    options::load_preferences(app.world_mut());
    assert!(
        app.world()
            .resource::<MenuState>()
            .error
            .contains("audio.toml")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
    assert_eq!(
        *app.world().resource::<AudioConfig>(),
        AudioConfig::default()
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let old_graphics = app.world().resource::<GraphicsConfig>().clone();
    let old_controls = app.world().resource::<ControlsConfig>().clone();
    command(&mut app, "options");
    change(&mut app, "graphics-fov", "90");
    change(&mut app, "controls-auto-climb", "true");
    change(&mut app, "audio-master", "20");
    change(&mut app, "audio-animals", "25");
    change(&mut app, "audio-footsteps", "75");
    command(&mut app, "apply");
    assert!(
        app.world()
            .resource::<MenuState>()
            .error
            .contains("Cannot save audio")
    );
    assert_eq!(*app.world().resource::<GraphicsConfig>(), old_graphics);
    assert_eq!(*app.world().resource::<ControlsConfig>(), old_controls);
    assert_eq!(
        *app.world().resource::<AudioConfig>(),
        AudioConfig::default()
    );
    assert_eq!(
        GraphicsConfig::load(root.join("graphics.toml")).unwrap(),
        old_graphics
    );
    assert_eq!(
        ControlsConfig::load(root.join("controls.toml")).unwrap(),
        old_controls
    );
    cleanup(app);
}

#[test]
fn display_confirmation_reverts_or_persists_audio_with_the_other_drafts() {
    let mut app = app();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("audio.toml");
    command(&mut app, "options");
    change(&mut app, "graphics-resolution", "1600x900");
    change(&mut app, "audio-master", "30");
    change(&mut app, "audio-animals", "35");
    change(&mut app, "audio-footsteps", "65");
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Display
    );
    assert_eq!(app.world().resource::<AudioConfig>().master_volume, 0.3);
    assert_eq!(app.world().resource::<AudioConfig>().animals_volume, 0.35);
    assert_eq!(app.world().resource::<AudioConfig>().footsteps_volume, 0.65);
    assert_eq!(AudioConfig::load(&path).unwrap().master_volume, 1.0);
    command(&mut app, "display-revert");
    assert_eq!(app.world().resource::<AudioConfig>().master_volume, 1.0);
    assert_eq!(app.world().resource::<AudioConfig>().animals_volume, 1.0);
    assert_eq!(app.world().resource::<AudioConfig>().footsteps_volume, 1.0);
    assert_eq!(
        app.world().resource::<OptionsDraft>().audio.master_volume,
        1.0
    );
    change(&mut app, "graphics-resolution", "1600x900");
    change(&mut app, "audio-master", "30");
    change(&mut app, "audio-animals", "35");
    change(&mut app, "audio-footsteps", "65");
    command(&mut app, "apply");
    command(&mut app, "display-keep");
    assert_eq!(AudioConfig::load(&path).unwrap().master_volume, 0.3);
    assert_eq!(AudioConfig::load(&path).unwrap().animals_volume, 0.35);
    assert_eq!(AudioConfig::load(&path).unwrap().footsteps_volume, 0.65);
    change(&mut app, "audio-master", "101");
    command(&mut app, "apply");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    assert_eq!(AudioConfig::load(&path).unwrap().master_volume, 0.3);
    assert_eq!(app.world().resource::<AudioConfig>().master_volume, 0.3);
    cleanup(app);
}

#[test]
fn invalid_new_audio_category_gains_are_not_applied_or_saved() {
    let mut app = app();
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("audio.toml");
    command(&mut app, "options");
    command(&mut app, "audio");
    for control in ["audio-animals", "audio-footsteps"] {
        for value in ["-1", "101", "nan", "inf"] {
            command(&mut app, "reset");
            change(&mut app, control, value);
            command(&mut app, "apply");
            assert_eq!(
                *app.world().resource::<AudioConfig>(),
                AudioConfig::default()
            );
            assert_eq!(AudioConfig::load(&path).unwrap(), AudioConfig::default());
        }
    }
    cleanup(app);
}

#[test]
fn custom_display_values_and_invalid_slider_values_are_not_coerced() {
    let mut app = app();
    command(&mut app, "options");
    change(&mut app, "graphics-resolution", "960x640");
    change(&mut app, "graphics-fps", "75");
    change(&mut app, "graphics-resolution", "custom");
    change(&mut app, "graphics-fps", "custom");
    let draft = app.world().resource::<OptionsDraft>();
    assert_eq!((draft.graphics.width, draft.graphics.height), (960, 640));
    assert_eq!(draft.graphics.fps_limit, 75);
    let distance = draft.graphics.render_distance;
    change(&mut app, "graphics-distance", "2.5");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    assert_eq!(
        app.world()
            .resource::<OptionsDraft>()
            .graphics
            .render_distance,
        distance
    );
    cleanup(app);
}

#[test]
fn borderless_resolution_can_return_to_windowed_and_display_revert_remains_safe() {
    use terra_core::WindowModeSetting;
    let mut app = app();
    command(&mut app, "options");
    change(&mut app, "graphics-window", "borderless");
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Display
    );
    command(&mut app, "display-keep");
    let borderless = app.world().resource::<GraphicsConfig>().clone();
    assert_eq!(borderless.window_mode, WindowModeSetting::Borderless);
    change(&mut app, "graphics-resolution", "desktop");
    assert_eq!(app.world().resource::<OptionsDraft>().graphics, borderless);

    change(&mut app, "graphics-resolution", "1600x900");
    let draft = &app.world().resource::<OptionsDraft>().graphics;
    assert_eq!(draft.window_mode, WindowModeSetting::Windowed);
    assert_eq!((draft.width, draft.height), (1600, 900));
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Display
    );
    let mut windows = app
        .world_mut()
        .query_filtered::<&Window, With<PrimaryWindow>>();
    assert_eq!(
        windows.single(app.world()).unwrap().mode,
        bevy::window::WindowMode::Windowed
    );
    command(&mut app, "display-revert");
    assert_eq!(*app.world().resource::<GraphicsConfig>(), borderless);
    assert_eq!(app.world().resource::<OptionsDraft>().graphics, borderless);
    assert!(matches!(
        windows.single(app.world()).unwrap().mode,
        bevy::window::WindowMode::BorderlessFullscreen(_)
    ));

    // Other controls still apply after returning to Borderless.
    change(&mut app, "graphics-fps", "144");
    command(&mut app, "apply");
    assert_eq!(app.world().resource::<GraphicsConfig>().fps_limit, 144);
    assert_eq!(app.world().resource::<MenuState>().dialog, MenuDialog::None);
    let saved = GraphicsConfig::load(
        app.world()
            .resource::<MenuPaths>()
            .configs
            .join("graphics.toml"),
    )
    .unwrap();
    assert_eq!(saved.fps_limit, 144);
    assert_eq!(saved.window_mode, WindowModeSetting::Borderless);
    cleanup(app);
}

#[test]
fn navigation_real_world_create_delete_and_confirmation() {
    let mut app = app();
    command(&mut app, "singleplayer");
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Worlds);
    assert!(app.world().resource::<WorldMenu>().entries.is_empty());
    command(&mut app, "world-create");
    change(&mut app, "world-name", "Cedar Valley");
    change(&mut app, "world-seed", "42");
    command(&mut app, "world-create-confirm");
    let menu = app.world().resource::<WorldMenu>();
    let id = menu.selected.unwrap();
    assert_eq!(menu.entries.len(), 1);
    assert_eq!(
        app.world()
            .resource::<MenuPaths>()
            .worlds
            .load(id)
            .unwrap()
            .manifest
            .world
            .seed
            .0,
        42
    );
    command(&mut app, "world-delete");
    assert!(app.world().resource::<MenuPaths>().worlds.load(id).is_ok());
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::DeleteWorld
    );
    command(&mut app, "dialog-cancel");
    command(&mut app, "world-delete");
    command(&mut app, "world-delete-confirm");
    assert!(app.world().resource::<WorldMenu>().entries.is_empty());
    cleanup(app);
}
#[test]
fn biome_picker_selects_a_picture_and_blocks_the_underlying_form() {
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    let cards = view::world_biome_choices(app.world().resource::<WorldMenu>());
    assert_eq!(cards.len(), super::create_world::BIOMES.len());
    assert_eq!(
        cards
            .iter()
            .filter(|card| card.class_name.ends_with(" selected"))
            .count(),
        1
    );
    for card in &cards {
        let asset = card.image.strip_prefix("tilt-ui://").unwrap();
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../src-ui")
                .join(asset)
                .is_file()
        );
    }
    command(&mut app, "world-biome-picker-open");
    command(&mut app, "world-mode-creative");
    change(&mut app, "world-name", "Hidden form edit");
    command(&mut app, "world-create-confirm");
    assert_eq!(app.world().resource::<WorldMenu>().mode, "survival");
    assert!(app.world().resource::<WorldMenu>().name.is_empty());
    assert!(app.world().resource::<WorldMenu>().entries.is_empty());
    assert!(app.world().resource::<WorldMenu>().biome_picker_open);
    command(&mut app, "world-biome-invalid");
    assert!(app.world().resource::<WorldMenu>().biome_picker_open);
    command(&mut app, "world-biome-cherry-forest");
    let form = app.world().resource::<WorldMenu>();
    assert!(!form.biome_picker_open);
    assert_eq!(
        form.preferred_spawn_biome,
        Some(terra_core::BiomeKind::CherryForest)
    );
    let selected = view::world_biome_choices(form)
        .into_iter()
        .find(|card| card.class_name.ends_with(" selected"))
        .unwrap();
    assert_eq!(selected.title, "Cherry Forest");
    assert!(selected.image.ends_with("biome-choice-cherry-forest.png"));
    command(&mut app, "world-biome-picker-open");
    command(&mut app, "dialog-cancel");
    assert!(!app.world().resource::<WorldMenu>().biome_picker_open);
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::CreateWorld
    );
    command(&mut app, "dialog-cancel");
    command(&mut app, "world-create");
    assert!(!app.world().resource::<WorldMenu>().biome_picker_open);
    assert_eq!(
        app.world().resource::<WorldMenu>().preferred_spawn_biome,
        None
    );
    cleanup(app);
}

#[test]
fn biome_picker_traps_tab_focus_and_returns_to_its_trigger() {
    use bevy::input_focus::{FocusCause, InputFocus, tab_navigation::TabGroup};
    let mut app = App::new();
    app.init_resource::<InputFocus>()
        .add_systems(Update, super::create_world::focus_biome_picker);
    let trigger = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(trigger, FocusCause::Navigated);
    let picker = app
        .world_mut()
        .spawn(tilt_ui::ElementId("world-biome-picker".into()))
        .id();
    let done = app
        .world_mut()
        .spawn((
            tilt_ui::ElementId("world-biome-picker-close".into()),
            ChildOf(picker),
        ))
        .id();
    app.update();
    assert!(app.world().get::<TabGroup>(picker).unwrap().modal);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(done));
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(trigger, FocusCause::Navigated);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(done));
    app.world_mut().despawn(picker);
    app.update();
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(trigger));
}

#[test]
fn escape_closes_biome_picker_before_create_world() {
    let mut app = app();
    app.init_resource::<ButtonInput<KeyCode>>().add_systems(
        Update,
        navigation::keyboard_navigation.before(navigation::actions),
    );
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    command(&mut app, "world-biome-picker-open");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<WorldMenu>().biome_picker_open);
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::CreateWorld
    );
    cleanup(app);
}

#[test]
fn create_world_draft_resets_rules_biomes_and_structure_groups_on_reopen() {
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    {
        let form = app.world().resource::<WorldMenu>();
        assert_eq!(form.mode, "survival");
        assert!(!form.game_rules.keep_inventory && !form.game_rules.pvp);
        assert!(form.game_rules.daylight_cycle && form.game_rules.weather_cycle);
        assert!(form.game_rules.mob_griefing && form.game_rules.fire_spread);
        assert_eq!(form.preferred_spawn_biome, None);
        assert!(
            form.structures.villages
                && form.structures.ruins
                && form.structures.mines
                && form.structures.kingdoms
                && form.structures.outposts
        );
        let cards = view::world_rule_cards(form);
        let mut actual = cards
            .iter()
            .map(|card| card.name.as_str())
            .collect::<Vec<_>>();
        let mut registered = terra_core::GameRule::ALL
            .iter()
            .map(|rule| rule.name())
            .collect::<Vec<_>>();
        actual.sort_unstable();
        registered.sort_unstable();
        assert_eq!(
            actual, registered,
            "Every registered gamerule must have a card"
        );
        assert_eq!(cards.iter().filter(|card| card.kind == "switch").count(), 6);
        assert_eq!(cards.iter().filter(|card| card.kind == "number").count(), 3);
        for (name, value) in [
            ("sleep_percentage", "100"),
            ("game_time_speed", "1"),
            ("season_swap_interval", "7"),
        ] {
            let card = cards.iter().find(|card| card.name == name).unwrap();
            assert_eq!(card.kind, "number");
            assert_eq!(card.value, value);
        }
    }
    change(&mut app, "world-structure-villages", "false");
    change(&mut app, "world-structure-mines", "false");
    {
        let selected = app.world().resource::<WorldMenu>().structures;
        assert!(!selected.villages && selected.ruins && !selected.mines);
    }
    change(&mut app, "world-structures-enabled", "false");
    {
        let selected = app.world().resource::<WorldMenu>().structures;
        assert!(!selected.villages && !selected.ruins && !selected.mines);
    }
    change(&mut app, "world-structure-mines", "true");
    assert!(app.world().resource::<WorldMenu>().structures.mines);
    change(&mut app, "world-structures-enabled", "false");
    assert!(!app.world().resource::<WorldMenu>().structures.mines);
    change(&mut app, "world-structures-enabled", "true");
    {
        let selected = app.world().resource::<WorldMenu>().structures;
        assert!(
            selected.villages
                && selected.ruins
                && selected.mines
                && selected.kingdoms
                && selected.outposts
        );
    }
    change(&mut app, "world-structures-enabled", "false");
    change(&mut app, "world-structures-enabled", "true");
    {
        let selected = app.world().resource::<WorldMenu>().structures;
        assert!(
            selected.villages
                && selected.ruins
                && selected.mines
                && selected.kingdoms
                && selected.outposts
        );
    }
    command(&mut app, "world-mode-spectator");
    command(&mut app, "world-biome-cherry-forest");
    change(&mut app, "world-rule-keep-inventory", "true");
    change(&mut app, "world-rule-daylight-cycle", "false");
    change(&mut app, "world-sleep-percentage", "25");
    change(&mut app, "world-time-speed", "2");
    change(&mut app, "world-season-days", "24");
    command(&mut app, "dialog-cancel");
    assert!(app.world().resource::<WorldMenu>().entries.is_empty());
    command(&mut app, "world-create");
    let form = app.world().resource::<WorldMenu>();
    assert_eq!(form.mode, "survival");
    assert_eq!(form.preferred_spawn_biome, None);
    assert!(!form.game_rules.keep_inventory && form.game_rules.daylight_cycle);
    assert_eq!(form.game_rules.sleep_percentage, 100.0);
    assert_eq!(form.game_rules.game_time_speed, 1.0);
    assert!(
        form.structures.villages
            && form.structures.ruins
            && form.structures.mines
            && form.structures.kingdoms
            && form.structures.outposts
    );
    cleanup(app);
}

#[test]
fn create_world_persists_spectator_rules_biome_and_individual_structures() {
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    change(&mut app, "world-name", "Spectator Cherry Coast");
    change(&mut app, "world-seed", "987654321");
    command(&mut app, "world-mode-spectator");
    command(&mut app, "world-biome-ocean");
    assert_eq!(
        app.world().resource::<WorldMenu>().preferred_spawn_biome,
        Some(terra_core::BiomeKind::Ocean)
    );
    command(&mut app, "world-biome-random");
    assert_eq!(
        app.world().resource::<WorldMenu>().preferred_spawn_biome,
        None
    );
    command(&mut app, "world-biome-cherry-forest");
    for (control, value) in [
        ("world-rule-keep-inventory", "true"),
        ("world-rule-daylight-cycle", "false"),
        ("world-rule-weather-cycle", "false"),
        ("world-rule-mob-griefing", "false"),
        ("world-rule-fire-spread", "false"),
        ("world-rule-pvp", "true"),
        ("world-sleep-percentage", "35"),
        ("world-time-speed", "1.5"),
        ("world-season-days", "24"),
        ("world-mob-limit", "512"),
    ] {
        change(&mut app, control, value);
    }
    change(&mut app, "world-structures-enabled", "false");
    change(&mut app, "world-structure-ruins", "true");
    command(&mut app, "world-create-confirm");
    assert_eq!(app.world().resource::<MenuState>().dialog, MenuDialog::None);
    let id = app
        .world()
        .resource::<WorldMenu>()
        .selected
        .expect("Created world selected");
    let library = app.world().resource::<MenuPaths>().worlds.clone();
    let saved = library.load(id).unwrap();
    assert_eq!(saved.mode, terra_core::GameMode::Spectator);
    let settings = &saved.manifest.world;
    assert_eq!(settings.seed.0, 987654321);
    assert_eq!(settings.natural_mob_limit, 512);
    assert_eq!(
        settings.preferred_spawn_biome,
        Some(terra_core::BiomeKind::CherryForest)
    );
    assert!(settings.game_rules.keep_inventory && settings.game_rules.pvp);
    assert!(!settings.game_rules.daylight_cycle && !settings.game_rules.weather_cycle);
    assert!(!settings.game_rules.mob_griefing && !settings.game_rules.fire_spread);
    assert_eq!(settings.game_rules.sleep_percentage, 35.0);
    assert_eq!(settings.game_rules.game_time_speed, 1.5);
    assert_eq!(
        [
            settings.seasons.spring_days,
            settings.seasons.summer_days,
            settings.seasons.autumn_days,
            settings.seasons.winter_days
        ],
        [24; 4]
    );
    assert!(
        !settings.structures.villages
            && settings.structures.ruins
            && !settings.structures.mines
            && !settings.structures.kingdoms
            && !settings.structures.outposts
    );
    let reopened = terra_storage::WorldLibrary {
        root: library.root.clone(),
    }
    .load(id)
    .unwrap();
    assert_eq!(reopened.manifest.world, saved.manifest.world);
    assert_eq!(reopened.mode, terra_core::GameMode::Spectator);
    cleanup(app);
}

#[test]
fn saved_server_add_edit_reload_and_validation() {
    let mut app = app();
    command(&mut app, "multiplayer");
    command(&mut app, "server-add");
    change(&mut app, "server-name", "Local Terra");
    change(&mut app, "server-address", "http://bad");
    command(&mut app, "server-save");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    change(&mut app, "server-address", "127.0.0.1");
    change(&mut app, "server-port", "5000");
    command(&mut app, "server-save");
    assert_eq!(app.world().resource::<ServerMenu>().list.servers.len(), 1);
    command(&mut app, "server-edit");
    change(&mut app, "server-name", "Friends");
    command(&mut app, "server-save");
    let root = &app.world().resource::<MenuPaths>().configs;
    let saved = terra_core::config::servers::ServerList::load(root.join("servers.toml")).unwrap();
    assert_eq!(saved.servers[0].name, "Friends");
    command(&mut app, "server-join");
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Loading);
    assert!(app.world().resource::<MenuState>().error.is_empty());
    cleanup(app);
}
#[test]
fn options_apply_reset_back_discard_and_persistent_reload() {
    let mut app = app();
    command(&mut app, "options");
    change(&mut app, "graphics-fov", "87");
    assert_ne!(
        app.world().resource::<GameConfig>().client.fov_degrees,
        87.0
    );
    command(&mut app, "back");
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Discard
    );
    command(&mut app, "dialog-cancel");
    command(&mut app, "apply");
    assert_eq!(
        app.world().resource::<GameConfig>().client.fov_degrees,
        87.0
    );
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    assert_eq!(GraphicsConfig::load(&path).unwrap().fov, 87.0);
    command(&mut app, "reset");
    assert!(options::dirty(app.world()));
    command(&mut app, "back");
    command(&mut app, "discard-confirm");
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Main);
    assert_eq!(GraphicsConfig::load(path).unwrap().fov, 87.0);
    cleanup(app);
}

#[test]
fn ui_scale_changes_only_on_apply_and_preserves_discard_reset_and_window_fit() {
    let mut app = app();
    app.init_resource::<bevy::ui::UiScale>()
        .add_systems(Update, options::apply_ui_scale.after(navigation::actions));
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_physical_resolution(1920, 1200);
    {
        let mut graphics = app.world_mut().resource_mut::<GraphicsConfig>();
        graphics.width = 1920;
        graphics.height = 1200;
    }
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    command(&mut app, "options");
    change(&mut app, "graphics-scale", "120");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.0);
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 1.0);
    assert_eq!(GraphicsConfig::load(&path).unwrap().ui_scale, 1.0);
    command(&mut app, "back");
    command(&mut app, "discard-confirm");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.0);

    command(&mut app, "options");
    change(&mut app, "graphics-scale", "150");
    command(&mut app, "apply");
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 1.5);
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.5);
    assert_eq!(GraphicsConfig::load(&path).unwrap().ui_scale, 1.5);
    change(&mut app, "graphics-scale", "250");
    assert_eq!(
        app.world().resource::<OptionsDraft>().graphics.ui_scale,
        1.5
    );
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    command(&mut app, "reset");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.5);
    assert_eq!(GraphicsConfig::load(&path).unwrap().ui_scale, 1.5);
    command(&mut app, "back");
    command(&mut app, "discard-confirm");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.5);
    options::load_preferences(app.world_mut());
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 1.5);

    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_physical_resolution(1280, 800);
    app.update();
    assert!(app.world().resource::<bevy::ui::UiScale>().0 < 1.5);
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 1.5);
    assert_eq!(GraphicsConfig::load(&path).unwrap().ui_scale, 1.5);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set_physical_resolution(1920, 1200);
    app.update();
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, 1.5);

    {
        let mut window = app.world_mut().get_mut::<Window>(window).unwrap();
        window.resolution.set_physical_resolution(1280, 800);
        window.resolution.set_scale_factor_override(Some(1.5));
    }
    command(&mut app, "options");
    change(&mut app, "graphics-scale", "75");
    command(&mut app, "apply");
    let small = app.world().resource::<bevy::ui::UiScale>().0;
    change(&mut app, "graphics-scale", "90");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, small);
    command(&mut app, "apply");
    let medium = app.world().resource::<bevy::ui::UiScale>().0;
    change(&mut app, "graphics-scale", "100");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, medium);
    command(&mut app, "apply");
    let base = app.world().resource::<bevy::ui::UiScale>().0;
    assert!(small < medium && medium < base);
    assert!((small / base - 0.75).abs() < 0.001);
    assert!((medium / base - 0.9).abs() < 0.001);
    change(&mut app, "graphics-scale", "150");
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, base);
    command(&mut app, "apply");
    let large = app.world().resource::<bevy::ui::UiScale>().0;
    assert!(large > base);
    let viewport = app.world().get::<Window>(window).unwrap().size() / large;
    assert!(viewport.x >= 899.9 && viewport.y >= 639.9);
    assert_eq!(GraphicsConfig::load(&path).unwrap().ui_scale, 1.5);
    cleanup(app);
}

#[test]
fn ui_fps_and_material_controls_apply_reload_and_discard_in_the_existing_graphics_file() {
    let mut app = app();
    command(&mut app, "options");
    change(&mut app, "graphics-ui-fps", "120");
    change(&mut app, "graphics-block-detail", "medium");
    change(&mut app, "graphics-leaf-detail", "low");
    change(&mut app, "graphics-micro-distance", "64");
    change(&mut app, "graphics-normal-detail", "false");
    change(&mut app, "graphics-material-detail", "false");
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_fps, 60);
    command(&mut app, "apply");
    let file = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    let saved = GraphicsConfig::load(&file).unwrap();
    assert_eq!(saved.ui_fps, 120);
    assert_eq!(
        saved.blocks.detail_quality,
        terra_core::BlockDetailQuality::Medium
    );
    assert_eq!(
        saved.blocks.leaf_geometry_quality,
        terra_core::BlockDetailQuality::Low
    );
    assert_eq!(saved.blocks.micro_geometry_distance, 64.0);
    assert!(!saved.blocks.normal_mapping);
    assert!(!saved.blocks.material_detail);
    assert_eq!(
        app.world().resource::<GameConfig>().client.blocks,
        saved.blocks
    );
    change(&mut app, "graphics-ui-fps", "30");
    command(&mut app, "back");
    command(&mut app, "discard-confirm");
    assert_eq!(GraphicsConfig::load(file).unwrap(), saved);
    cleanup(app);
}

#[test]
fn failed_apply_does_not_resize_the_ui_or_overwrite_the_saved_scale() {
    let mut app = app();
    app.init_resource::<bevy::ui::UiScale>()
        .add_systems(Update, options::apply_ui_scale.after(navigation::actions));
    command(&mut app, "options");
    let before = app.world().resource::<bevy::ui::UiScale>().0;
    let configs = app.world().resource::<MenuPaths>().configs.clone();
    let blocked = configs.join("not-a-directory");
    std::fs::write(&blocked, b"fixture").unwrap();
    app.world_mut().resource_mut::<MenuPaths>().configs = blocked;
    change(&mut app, "graphics-scale", "125");
    command(&mut app, "apply");
    assert!(!app.world().resource::<MenuState>().error.is_empty());
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 1.0);
    assert_eq!(app.world().resource::<bevy::ui::UiScale>().0, before);
    assert_eq!(
        GraphicsConfig::load(configs.join("graphics.toml"))
            .unwrap()
            .ui_scale,
        1.0
    );
    app.world_mut().resource_mut::<MenuPaths>().configs = configs;
    cleanup(app);
}

#[test]
fn display_preview_has_automatic_rollback_and_only_persists_after_confirmation() {
    let mut app = app();
    app.init_resource::<bevy::ui::UiScale>()
        .add_systems(Update, options::apply_ui_scale.after(navigation::actions));
    let previous = app.world().resource::<GraphicsConfig>().clone();
    command(&mut app, "options");
    change(&mut app, "graphics-resolution", "1600x900");
    change(&mut app, "graphics-scale", "90");
    let original_scale = app.world().resource::<bevy::ui::UiScale>().0;
    command(&mut app, "apply");
    assert_eq!(app.world().resource::<GraphicsConfig>().ui_scale, 0.9);
    assert_eq!(
        app.world().resource::<MenuState>().dialog,
        MenuDialog::Display
    );
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("graphics.toml");
    assert_eq!(GraphicsConfig::load(&path).unwrap(), previous);
    app.world_mut()
        .resource_mut::<OptionsDraft>()
        .display
        .as_mut()
        .unwrap()
        .until = Instant::now() - Duration::from_secs(1);
    options::display_timeout(app.world_mut());
    app.update();
    assert_eq!(*app.world().resource::<GraphicsConfig>(), previous);
    assert_eq!(
        app.world().resource::<bevy::ui::UiScale>().0,
        original_scale
    );
    change(&mut app, "graphics-resolution", "1600x900");
    command(&mut app, "apply");
    command(&mut app, "display-keep");
    assert_eq!(GraphicsConfig::load(path).unwrap().width, 1600);
    cleanup(app);
}
#[test]
fn controls_conflicts_rebinding_and_reload_use_the_same_config() {
    let mut app = app();
    let binding = InputBinding::Key(KeyCode::ArrowUp);
    let mut controls = app.world().resource::<ControlsConfig>().clone();
    controls.rebind(GameAction::Forward, binding).unwrap();
    assert!(controls.rebind(GameAction::Backward, binding).is_err());
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("controls.toml");
    controls.save(&path).unwrap();
    assert_eq!(ControlsConfig::load(path).unwrap(), controls);
    command(&mut app, "options");
    command(&mut app, "bind-0");
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Update, options::capture_binding.before(navigation::actions));
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(app.world().resource::<OptionsDraft>().rebinding.is_none());
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Options);
    cleanup(app);
}

#[test]
fn auto_climb_is_a_controls_draft_until_apply_and_reset_disables_it() {
    let mut app = app();
    assert!(!app.world().resource::<ControlsConfig>().auto_climb);
    command(&mut app, "options");
    command(&mut app, "controls");
    change(&mut app, "controls-auto-climb", "true");
    assert!(app.world().resource::<OptionsDraft>().controls.auto_climb);
    assert!(!app.world().resource::<ControlsConfig>().auto_climb);
    command(&mut app, "apply");
    assert!(app.world().resource::<ControlsConfig>().auto_climb);
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("controls.toml");
    assert!(ControlsConfig::load(&path).unwrap().auto_climb);
    command(&mut app, "reset");
    assert!(!app.world().resource::<OptionsDraft>().controls.auto_climb);
    assert!(app.world().resource::<ControlsConfig>().auto_climb);
    command(&mut app, "apply");
    assert!(!ControlsConfig::load(path).unwrap().auto_climb);
    cleanup(app);
}

#[test]
fn mouse_rebinding_ignores_opening_click_and_reloads_typed_mouse_binding() {
    let mut app = app();
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_systems(Update, options::capture_binding);
    command(&mut app, "options");
    command(&mut app, "controls");
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    command(&mut app, "bind-9");
    assert_eq!(
        app.world().resource::<OptionsDraft>().rebinding,
        Some(GameAction::Place)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    assert_eq!(
        app.world()
            .resource::<OptionsDraft>()
            .controls
            .binding(GameAction::Place),
        InputBinding::Mouse(MouseButton::Right)
    );
    command(&mut app, "apply");
    let path = app
        .world()
        .resource::<MenuPaths>()
        .configs
        .join("controls.toml");
    assert_eq!(
        ControlsConfig::load(path)
            .unwrap()
            .binding(GameAction::Place),
        InputBinding::Mouse(MouseButton::Right)
    );
    cleanup(app);
}

#[test]
fn exclusive_fullscreen_selects_a_real_monitor_mode_and_rejects_unsupported_modes() {
    use bevy::window::{Monitor, OnMonitor, VideoMode, VideoModeSelection, WindowMode};
    let mut app = app();
    let mode = VideoMode {
        physical_size: UVec2::new(1600, 900),
        bit_depth: 32,
        refresh_rate_millihertz: 60_000,
    };
    let monitor = app
        .world_mut()
        .spawn(Monitor {
            name: Some("Test monitor".into()),
            physical_height: 900,
            physical_width: 1600,
            physical_position: IVec2::ZERO,
            refresh_rate_millihertz: Some(60_000),
            scale_factor: 1.0,
            video_modes: vec![mode],
        })
        .id();
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(window)
        .insert(OnMonitor(monitor));
    command(&mut app, "options");
    change(&mut app, "graphics-window", "fullscreen");
    command(&mut app, "apply");
    assert!(
        app.world()
            .resource::<MenuState>()
            .error
            .contains("does not support")
    );
    assert_eq!(
        app.world().get::<Window>(window).unwrap().mode,
        WindowMode::Windowed
    );
    change(&mut app, "graphics-resolution", "1600x900");
    command(&mut app, "apply");
    assert!(
        matches!(app.world().get::<Window>(window).unwrap().mode, WindowMode::Fullscreen(_, VideoModeSelection::Specific(selected)) if selected == mode)
    );
    command(&mut app, "display-revert");
    assert_eq!(
        app.world().get::<Window>(window).unwrap().mode,
        WindowMode::Windowed
    );
    cleanup(app);
}

#[test]
fn startup_screen_blocks_menu_navigation_setting_changes_and_quit() {
    let mut app = app();
    app.insert_resource(StartupProgress {
        active: true,
        ..Default::default()
    });
    app.world_mut()
        .write_message(MenuAction::Command("singleplayer".into()));
    app.world_mut()
        .write_message(MenuAction::Command("options".into()));
    app.world_mut().write_message(MenuAction::Change {
        control: "world-name".into(),
        value: "Premature edit".into(),
    });
    app.update();
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Main);
    assert!(!app.world().resource::<MenuState>().quitting);
    assert!(app.world().resource::<WorldMenu>().name.is_empty());
    assert!(app.world().resource::<terra_player::PlayerInputBlock>().0);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<SessionRequest>>()
            .drain()
            .count(),
        0
    );
    app.world_mut()
        .write_message(MenuAction::Command("quit".into()));
    app.update();
    assert!(!app.world().resource::<MenuState>().quitting);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<SessionRequest>>()
            .drain()
            .count(),
        0
    );
    cleanup(app);
}

#[test]
fn startup_crossfade_is_smooth_and_menu_commands_stay_blocked_until_it_finishes() {
    let mut progress = StartupProgress {
        active: true,
        fading: true,
        reveal: 0.5,
        ..Default::default()
    };
    assert_eq!(progress.blend(), 0.5);
    progress.reveal = 0.01;
    assert!(progress.blend() < 0.001);
    progress.reveal = 0.99;
    assert!(progress.blend() > 0.999);
    let mut app = app();
    app.insert_resource(progress);
    app.world_mut()
        .write_message(MenuAction::Command("singleplayer".into()));
    app.update();
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Main);
    app.world_mut().resource_mut::<StartupProgress>().active = false;
    app.world_mut()
        .write_message(MenuAction::Command("singleplayer".into()));
    app.update();
    assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Worlds);
    cleanup(app);
}

#[test]
fn world_create_keeps_kingdom_and_outpost_switches_independent_and_persists_off() {
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    change(&mut app, "world-name", "Outposts without kingdoms");
    change(&mut app, "world-structures-enabled", "false");
    change(&mut app, "world-structure-kingdoms", "true");
    {
        let settings = app.world().resource::<WorldMenu>().structures;
        assert!(settings.kingdoms);
        assert!(!settings.villages && !settings.outposts && !settings.ruins && !settings.mines);
    }
    change(&mut app, "world-structure-outposts", "true");
    change(&mut app, "world-structure-kingdoms", "false");
    {
        let settings = app.world().resource::<WorldMenu>().structures;
        assert!(settings.outposts);
        assert!(!settings.villages && !settings.kingdoms);
    }
    command(&mut app, "world-create-confirm");
    let id = app.world().resource::<WorldMenu>().selected.unwrap();
    let library = app.world().resource::<MenuPaths>().worlds.clone();
    let saved = library.load(id).unwrap();
    assert_eq!(saved.manifest.world.structures_revision, 9);
    assert!(saved.manifest.world.structures.outposts);
    assert!(!saved.manifest.world.structures.kingdoms && !saved.manifest.world.structures.villages);
    cleanup(app);
}

#[test]
fn world_creation_tabs_preserve_and_persist_every_kingdom_policy_field() {
    use terra_core::kingdom::KingdomRules;
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    assert_eq!(
        app.world().resource::<WorldMenu>().create_tab,
        WorldCreateTab::General
    );
    change(&mut app, "world-name", "Realm Policy Tabs");
    change(&mut app, "world-seed", "202602");
    command(&mut app, "world-create-tab-biomes");
    command(&mut app, "world-biome-cherry-forest");
    change(&mut app, "world-structure-mines", "false");
    command(&mut app, "world-create-tab-kingdom");
    let edits = [
        ("require_adjacent_claims", "false"),
        ("settlements_follow_kingdom_level", "true"),
        ("realm_min_border_gap", "24"),
        ("kingdom_build_base", "40"),
        ("kingdom_build_per_level", "8"),
        ("kingdom_territory_base", "80"),
        ("kingdom_territory_per_level", "20"),
        ("outposts_per_level", "2"),
        ("outpost_build_base", "12"),
        ("outpost_build_per_level", "3"),
        ("outpost_territory_base", "28"),
        ("outpost_territory_per_level", "8"),
        ("village_requires_territory", "false"),
        ("village_build_base", "12"),
        ("village_build_per_level", "3"),
        ("city_min_kingdom_level", "4"),
        ("city_requires_territory", "false"),
        ("city_build_base", "24"),
        ("city_build_per_level", "5"),
        ("city_min_build_gap", "12"),
    ];
    for (key, value) in edits {
        change(&mut app, &format!("world-kingdom-{key}"), value);
        assert!(app.world().resource::<MenuState>().error.is_empty());
    }
    let form = app.world().resource::<WorldMenu>();
    let rules = form.kingdom_rules;
    let cards = super::kingdom_settings::cards(rules);
    let serialized = serde_json::to_value(rules).unwrap();
    assert_eq!(cards.len(), serialized.as_object().unwrap().len());
    for (key, value) in edits {
        let card = cards.iter().find(|card| card.name == key).unwrap();
        assert_eq!(card.value, value);
    }
    assert_ne!(rules, KingdomRules::default());
    command(&mut app, "world-create-tab-general");
    let form = app.world().resource::<WorldMenu>();
    assert_eq!(form.name, "Realm Policy Tabs");
    assert_eq!(form.seed, "202602");
    assert_eq!(form.kingdom_rules, rules);
    assert_eq!(
        form.preferred_spawn_biome,
        Some(terra_core::BiomeKind::CherryForest)
    );
    assert!(!form.structures.mines);
    command(&mut app, "world-create-confirm");
    let id = app.world().resource::<WorldMenu>().selected.unwrap();
    let library = app.world().resource::<MenuPaths>().worlds.clone();
    let saved = library.load(id).unwrap();
    assert_eq!(saved.manifest.world.kingdom_rules, rules);
    assert_eq!(saved.manifest.world.structure_border_gap, 24);
    assert_eq!(saved.manifest.world.structures_revision, 9);
    let reopened = terra_storage::WorldLibrary { root: library.root }
        .load(id)
        .unwrap();
    assert_eq!(reopened.manifest.world, saved.manifest.world);
    command(&mut app, "world-create");
    let form = app.world().resource::<WorldMenu>();
    assert_eq!(form.create_tab, WorldCreateTab::General);
    assert_eq!(form.kingdom_rules, KingdomRules::default());
    cleanup(app);
}

#[test]
fn invalid_world_kingdom_values_leave_the_entire_draft_unchanged() {
    let mut app = app();
    command(&mut app, "singleplayer");
    command(&mut app, "world-create");
    command(&mut app, "world-create-tab-kingdom");
    let rules = app.world().resource::<WorldMenu>().kingdom_rules;
    for (key, value) in [
        ("realm_min_border_gap", "4097"),
        ("outposts_per_level", "-1"),
        ("village_requires_territory", "yes"),
        ("city_min_kingdom_level", "0"),
        ("outpost_build_base", "65536"),
        ("unregistered", "1"),
    ] {
        change(&mut app, &format!("world-kingdom-{key}"), value);
        assert!(!app.world().resource::<MenuState>().error.is_empty());
        assert_eq!(app.world().resource::<WorldMenu>().kingdom_rules, rules);
    }
    command(&mut app, "world-create-tab-biomes");
    command(&mut app, "world-biome-picker-open");
    change(&mut app, "world-kingdom-realm_min_border_gap", "5");
    assert_eq!(app.world().resource::<WorldMenu>().kingdom_rules, rules);
    cleanup(app);
}
