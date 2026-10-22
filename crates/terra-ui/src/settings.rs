use crate::SettingsView;
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    window::{PresentMode, PrimaryWindow},
};
use terra_core::{
    ClientConfig, Config, GameConfig, WaterQuality, WeatherQuality, config_directory,
};
use terra_player::{
    CaptureRequest, ControllerMode, PlayerInputBlock, PlayerSet, TargetBlock, TerraCamera,
};
use terra_world::{
    edits::EditJournal,
    environment::{SurfaceCondition, SurfaceConditionRequest},
    streaming::ChunkStore,
};

#[derive(Message, Clone, Debug)]
pub enum SettingsAction {
    Open,
    Cancel,
    Resume,
    Apply,
    Reset,
    Save,
    SurfaceWet,
    SurfaceCharred,
    SurfaceReset,
    Change { control: String, value: String },
}

#[derive(Resource)]
pub struct SettingsConfigPath(pub std::path::PathBuf);

impl Default for SettingsConfigPath {
    fn default() -> Self {
        Self(config_directory().join("client.toml"))
    }
}

#[derive(Resource, Default)]
struct SettingsSession {
    baseline: Option<ClientConfig>,
    baseline_mode: Option<ControllerMode>,
    status: String,
    surface: Option<[i64; 3]>,
}

#[derive(SystemParam)]
struct SettingsWorld<'w> {
    target: Res<'w, TargetBlock>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    surface: MessageWriter<'w, SurfaceConditionRequest>,
    save_path: Res<'w, SettingsConfigPath>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<SettingsSession>()
        .init_resource::<SettingsConfigPath>()
        .init_resource::<SettingsView>()
        .add_message::<SettingsAction>()
        .add_systems(
            Update,
            (
                escape_menu.after(crate::chat::ChatInputSet),
                handle_actions,
                apply_camera_window,
                update_view,
            )
                .chain()
                .before(PlayerSet::Movement),
        );
}

fn escape_menu(
    keys: Res<ButtonInput<KeyCode>>,
    view: Res<SettingsView>,
    mut actions: MessageWriter<SettingsAction>,
    menu: Option<Res<crate::menu::MenuState>>,
    inventory: Option<Res<crate::inventory::InventoryState>>,
    chat: Option<Res<crate::chat::ChatState>>,
) {
    if menu.is_some()
        || chat.as_ref().is_some_and(|chat| chat.blocks_input())
        || inventory
            .as_ref()
            .is_some_and(|inventory| inventory.blocks_input())
    {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        actions.write(if view.open {
            SettingsAction::Cancel
        } else {
            SettingsAction::Open
        });
    }
}

fn handle_actions(
    mut actions: MessageReader<SettingsAction>,
    mut session: ResMut<SettingsSession>,
    mut config: ResMut<GameConfig>,
    mut mode: ResMut<ControllerMode>,
    mut blocked: ResMut<PlayerInputBlock>,
    mut capture: MessageWriter<CaptureRequest>,
    mut world: SettingsWorld,
) {
    for action in actions.read() {
        if matches!(action, SettingsAction::Open) {
            if session.baseline.is_none() {
                session.baseline = Some(config.client.clone());
                session.baseline_mode = Some(*mode);
                session.surface = world
                    .target
                    .0
                    .filter(|hit| world.chunks.block_at(hit.position) == Some(1))
                    .map(|hit| hit.position);
                session.status.clear();
                blocked.0 = true;
                capture.write(CaptureRequest(false));
            }
            continue;
        }
        if session.baseline.is_none() {
            continue;
        }
        match action {
            SettingsAction::Cancel => {
                if let Some(previous) = session.baseline.take() {
                    config.client = previous;
                }
                if let Some(previous) = session.baseline_mode.take() {
                    *mode = previous;
                }
                blocked.0 = false;
                capture.write(CaptureRequest(true));
            }
            SettingsAction::Resume => {
                session.baseline = None;
                session.baseline_mode = None;
                blocked.0 = false;
                capture.write(CaptureRequest(true));
            }
            SettingsAction::Apply => {
                session.baseline = Some(config.client.clone());
                session.baseline_mode = Some(*mode);
                session.status = "Fuer diese Sitzung uebernommen".into();
            }
            SettingsAction::Save => match config.client.save(&world.save_path.0) {
                Ok(()) => {
                    session.baseline = Some(config.client.clone());
                    session.baseline_mode = Some(*mode);
                    session.status = "Client-Einstellungen gespeichert".into();
                }
                Err(error) => {
                    warn!(%error, "Client settings could not be saved");
                    session.status = match error {
                        terra_core::ConfigError::Io { source, .. } => {
                            format!("Speichern fehlgeschlagen: {source}")
                        }
                        error => format!("Speichern fehlgeschlagen: {error}"),
                    };
                }
            },
            SettingsAction::Reset => {
                reset_exposed_options(&mut config.client);
                session.status = "Standardwerte in der Vorschau".into();
            }
            SettingsAction::SurfaceWet
            | SettingsAction::SurfaceCharred
            | SettingsAction::SurfaceReset => {
                let Some(position) = session.surface else {
                    continue;
                };
                if world.chunks.block_at(position) != Some(1) {
                    session.status = "Der ausgewaehlte Grassblock ist nicht verfuegbar".into();
                    continue;
                }
                let mut condition = world.journal.surface_condition(position);
                match action {
                    SettingsAction::SurfaceWet => condition.wetness = 1.0,
                    SettingsAction::SurfaceCharred => condition.charred = 1.0,
                    _ => condition = SurfaceCondition::default(),
                }
                world.surface.write(SurfaceConditionRequest {
                    position,
                    condition,
                });
                session.status = "Bodenzustand fuer diese Weltsitzung angefordert".into();
            }
            SettingsAction::Change { control, value } => {
                if control == "settings-mode" {
                    match value.as_str() {
                        "walk" => *mode = ControllerMode::Walk,
                        "fly" => *mode = ControllerMode::Fly,
                        _ => continue,
                    }
                    session.status.clear();
                    continue;
                }
                match edited_config(&config.client, control, value) {
                    Ok(next) => {
                        if next != config.client {
                            config.client = next;
                        }
                        session.status.clear();
                    }
                    Err(error) => session.status = error,
                }
            }
            SettingsAction::Open => {}
        }
    }
}

fn reset_exposed_options(config: &mut ClientConfig) {
    let defaults = ClientConfig::default();
    config.fov_degrees = defaults.fov_degrees;
    config.view_distance = defaults.view_distance;
    config.mouse_sensitivity = defaults.mouse_sensitivity;
    config.vsync = defaults.vsync;
    config.debug_overlay = defaults.debug_overlay;
    config.water_quality = defaults.water_quality;
    config.water_wave_strength = defaults.water_wave_strength;
    config.water_reflections = defaults.water_reflections;
    config.weather_quality = defaults.weather_quality;
}

fn edited_config(
    current: &ClientConfig,
    control: &str,
    value: &str,
) -> Result<ClientConfig, String> {
    let mut next = current.clone();
    let number = || {
        value
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or("Ungueltiger Zahlenwert")
    };
    let flag = || {
        value
            .parse::<bool>()
            .map_err(|_| "Ungueltiger Schalterwert")
    };
    match control {
        "settings-fov" => next.fov_degrees = number()?,
        "settings-distance" => {
            let distance = number()?;
            if !(2.0..=32.0).contains(&distance) || distance.fract() != 0.0 {
                return Err("Sichtweite muss 2 bis 32 Chunk-Spalten betragen".into());
            }
            next.view_distance = distance as u8;
        }
        "settings-sensitivity" => next.mouse_sensitivity = number()? / 1000.0,
        "settings-vsync" => next.vsync = flag()?,
        "settings-overlay" => next.debug_overlay = flag()?,
        "settings-reflections" => next.water_reflections = flag()?,
        "settings-waves" => next.water_wave_strength = number()?,
        "settings-weather-quality" => {
            next.weather_quality = match value {
                "off" => WeatherQuality::Off,
                "low" => WeatherQuality::Low,
                "medium" => WeatherQuality::Medium,
                "high" => WeatherQuality::High,
                "ultra" => WeatherQuality::Ultra,
                _ => return Err("Unbekannte Wetterqualitaet".into()),
            }
        }
        "settings-quality" => {
            next.water_quality = match value {
                "low" => WaterQuality::Low,
                "medium" => WaterQuality::Medium,
                "high" => WaterQuality::High,
                _ => return Err("Unbekannte Wasserqualitaet".into()),
            }
        }
        _ => return Err("Unbekannte Einstellung".into()),
    }
    next.validate().map_err(|error| error.to_string())?;
    Ok(next)
}

fn apply_camera_window(
    config: Res<GameConfig>,
    mut cameras: Query<&mut Projection, With<TerraCamera>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for mut projection in &mut cameras {
        let fov = config.client.fov_degrees.to_radians();
        if matches!(&*projection, Projection::Perspective(perspective) if perspective.fov != fov)
            && let Projection::Perspective(perspective) = &mut *projection
        {
            perspective.fov = fov;
        }
    }
    let mode = if config.client.vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    for mut window in &mut windows {
        if window.present_mode != mode {
            window.present_mode = mode;
        }
    }
}

fn update_view(
    config: Res<GameConfig>,
    session: Res<SettingsSession>,
    mode: Res<ControllerMode>,
    mut view: ResMut<SettingsView>,
) {
    if !config.is_changed() && !session.is_changed() && !mode.is_changed() {
        return;
    }
    let client = &config.client;
    let next = SettingsView {
        open: session.baseline.is_some(),
        overlay: client.debug_overlay && session.baseline.is_none(),
        fov: client.fov_degrees,
        fov_label: format!("{:.0} Grad", client.fov_degrees),
        distance: client.view_distance,
        distance_label: format!(
            "{} m / {} Sections",
            u16::from(client.view_distance) * 16,
            client.view_distance
        ),
        sensitivity: client.mouse_sensitivity * 1000.0,
        sensitivity_label: format!("{:.4}", client.mouse_sensitivity),
        vsync: client.vsync,
        debug: client.debug_overlay,
        reflections: client.water_reflections,
        waves: client.water_wave_strength,
        waves_label: format!("{:.2}", client.water_wave_strength),
        quality: match client.water_quality {
            WaterQuality::Low => "low",
            WaterQuality::Medium => "medium",
            WaterQuality::High => "high",
        }
        .into(),
        weather_quality: match client.weather_quality {
            WeatherQuality::Off => "off",
            WeatherQuality::Low => "low",
            WeatherQuality::Medium => "medium",
            WeatherQuality::High => "high",
            WeatherQuality::Ultra => "ultra",
        }
        .into(),
        mode: match *mode {
            ControllerMode::Walk => "walk",
            ControllerMode::Fly => "fly",
        }
        .into(),
        status: session.status.clone(),
        surface_available: session.surface.is_some(),
        surface_label: session
            .surface
            .map(|[x, y, z]| format!("Grassblock {x}, {y}, {z}"))
            .unwrap_or_else(|| "Kein Grassblock ausgewaehlt".into()),
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_validated_without_mutating_the_current_settings() {
        let current = ClientConfig::default();
        let edited = edited_config(&current, "settings-fov", "90").unwrap();
        assert_eq!(edited.fov_degrees, 90.0);
        assert_eq!(current.fov_degrees, 45.0);
        assert_eq!(
            edited_config(&current, "settings-distance", "32")
                .unwrap()
                .view_distance,
            32
        );
        for (field, value) in [
            ("settings-fov", "nan"),
            ("settings-fov", "111"),
            ("settings-distance", "1"),
            ("settings-distance", "2.5"),
            ("settings-distance", "33"),
            ("settings-vsync", "maybe"),
        ] {
            assert!(edited_config(&current, field, value).is_err());
        }
        assert_eq!(
            edited_config(&current, "settings-sensitivity", "2.2")
                .unwrap()
                .mouse_sensitivity,
            0.0022
        );
    }

    #[test]
    fn reset_preserves_unexposed_user_configuration() {
        let mut config = ClientConfig {
            title: "Custom".into(),
            max_mesh_jobs: 3,
            fov_degrees: 90.0,
            ..default()
        };
        reset_exposed_options(&mut config);
        assert_eq!(config.title, "Custom");
        assert_eq!(config.max_mesh_jobs, 3);
        assert_eq!(config.fov_degrees, 45.0);
    }

    #[test]
    fn camera_and_present_mode_follow_live_configuration() {
        let mut app = App::new();
        app.init_resource::<GameConfig>()
            .add_systems(Update, apply_camera_window);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Projection::default()))
            .id();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        {
            let mut config = app.world_mut().resource_mut::<GameConfig>();
            config.client.fov_degrees = 90.0;
            config.client.vsync = false;
        }
        app.update();
        assert!(
            matches!(app.world().get::<Projection>(camera), Some(Projection::Perspective(p)) if p.fov == 90.0_f32.to_radians())
        );
        assert_eq!(
            app.world().get::<Window>(window).unwrap().present_mode,
            PresentMode::AutoNoVsync
        );
        app.world_mut().resource_mut::<GameConfig>().client.vsync = true;
        app.update();
        assert_eq!(
            app.world().get::<Window>(window).unwrap().present_mode,
            PresentMode::AutoVsync
        );
    }

    #[test]
    fn cancel_restores_preview_and_apply_advances_the_rollback_point() {
        let mut app = App::new();
        app.init_resource::<GameConfig>()
            .init_resource::<SettingsSession>()
            .init_resource::<SettingsConfigPath>()
            .init_resource::<ControllerMode>()
            .init_resource::<PlayerInputBlock>()
            .init_resource::<TargetBlock>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .add_message::<CaptureRequest>()
            .add_message::<SettingsAction>()
            .add_message::<SurfaceConditionRequest>()
            .add_systems(Update, handle_actions);
        let send = |app: &mut App, action| {
            app.world_mut().write_message(action);
            app.update();
        };
        send(&mut app, SettingsAction::Open);
        assert!(app.world().resource::<PlayerInputBlock>().0);
        send(
            &mut app,
            SettingsAction::Change {
                control: "settings-fov".into(),
                value: "90".into(),
            },
        );
        send(&mut app, SettingsAction::Apply);
        send(
            &mut app,
            SettingsAction::Change {
                control: "settings-fov".into(),
                value: "100".into(),
            },
        );
        send(&mut app, SettingsAction::Cancel);
        assert_eq!(
            app.world().resource::<GameConfig>().client.fov_degrees,
            90.0
        );
        assert!(!app.world().resource::<PlayerInputBlock>().0);
        send(
            &mut app,
            SettingsAction::Change {
                control: "settings-fov".into(),
                value: "110".into(),
            },
        );
        assert_eq!(
            app.world().resource::<GameConfig>().client.fov_degrees,
            90.0
        );
    }
}
