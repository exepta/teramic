use super::model::*;
use bevy::{
    prelude::*,
    window::{
        Monitor, MonitorSelection, OnMonitor, PrimaryWindow, VideoMode, VideoModeSelection,
        WindowMode,
    },
};
use std::time::{Duration, Instant};
use terra_core::{
    AudioConfig, Config, ControlsConfig, GameAction, GameConfig, GameplayConfig, GraphicsConfig,
    InputBinding, MapMode, MultiplayerConfig, ShadowQuality, WaterQuality, WindowModeSetting,
};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsTab {
    #[default]
    Graphics,
    Gameplay,
    Controls,
    Audio,
    Multiplayer,
}

#[derive(Message, Debug, Copy, Clone, PartialEq, Eq)]
pub enum AudioAudition {
    Master,
    Ambience,
    Animals,
    Footsteps,
    Water,
    River,
    Waterfall,
}

#[derive(Resource, Default)]
pub struct OptionsDraft {
    pub graphics: GraphicsConfig,
    pub controls: ControlsConfig,
    pub audio: AudioConfig,
    pub multiplayer: MultiplayerConfig,
    pub gameplay: GameplayConfig,
    pub tab: OptionsTab,
    pub rebinding: Option<GameAction>,
    pub consumed_escape: bool,
    pub armed: bool,
    pub display: Option<DisplayConfirmation>,
}
pub struct DisplayConfirmation {
    pub previous: GraphicsConfig,
    pub controls: ControlsConfig,
    pub audio: AudioConfig,
    pub multiplayer: MultiplayerConfig,
    pub gameplay: GameplayConfig,
    pub until: Instant,
}

pub(super) fn load_preferences(world: &mut World) {
    let paths = world.resource::<MenuPaths>().clone();
    let legacy = world.resource::<GameConfig>().client.clone();
    let fallback = GraphicsConfig::from_client(&legacy);
    let graphics_path = paths.configs.join("graphics.toml");
    let result = if graphics_path.exists() {
        GraphicsConfig::load(&graphics_path)
    } else {
        fallback.save(&graphics_path).map(|()| fallback.clone())
    };
    let graphics = result.unwrap_or_else(|e| {
        world
            .resource_mut::<MenuState>()
            .error
            .push_str(&format!("\ngraphics.toml: {e}"));
        fallback
    });
    let controls_path = paths.configs.join("controls.toml");
    let fallback = ControlsConfig {
        mouse_sensitivity: legacy.mouse_sensitivity,
        ..default()
    };
    let result = if controls_path.exists() {
        ControlsConfig::load(&controls_path)
    } else {
        fallback.save(&controls_path).map(|()| fallback.clone())
    };
    let controls = result.unwrap_or_else(|e| {
        let mut menu = world.resource_mut::<MenuState>();
        menu.error.push_str(&format!("\ncontrols.toml: {e}"));
        fallback
    });
    let audio = AudioConfig::load_or_create(paths.configs.join("audio.toml")).unwrap_or_else(|e| {
        world
            .resource_mut::<MenuState>()
            .error
            .push_str(&format!("\naudio.toml: {e}"));
        AudioConfig::default()
    });
    let multiplayer = MultiplayerConfig::load_or_create(paths.configs.join("multiplayer.toml"))
        .unwrap_or_else(|e| {
            world
                .resource_mut::<MenuState>()
                .error
                .push_str(&format!("\nmultiplayer.toml: {e}"));
            MultiplayerConfig::default()
        });
    let gameplay = GameplayConfig::load_or_create(paths.configs.join("gameplay.toml"))
        .unwrap_or_else(|e| {
            world
                .resource_mut::<MenuState>()
                .error
                .push_str(&format!("\ngameplay.toml: {e}"));
            GameplayConfig::default()
        });
    world.insert_resource(gameplay.clone());
    world.insert_resource(multiplayer.clone());
    apply_runtime(world, &graphics, &controls, &audio);
    world.insert_resource(OptionsDraft {
        graphics,
        controls,
        audio,
        multiplayer,
        gameplay,
        ..default()
    });
    super::library::refresh_servers(world);
    match terra_core::PlayerProfile::selected_path(&paths.configs)
        .and_then(terra_core::PlayerProfile::load_for_client)
    {
        Ok(profile) => {
            world.insert_resource(profile);
            world.resource_mut::<MenuState>().profile_loaded = true;
        }
        Err(error) => {
            world
                .resource_mut::<MenuState>()
                .error
                .push_str(&format!("\nprofile.toml: {error}"));
        }
    }
}
pub(super) fn open(world: &mut World) {
    let graphics = world.resource::<GraphicsConfig>().clone();
    let controls = world.resource::<ControlsConfig>().clone();
    let audio = world.resource::<AudioConfig>().clone();
    let multiplayer = world
        .get_resource::<MultiplayerConfig>()
        .cloned()
        .unwrap_or_default();
    let gameplay = world
        .get_resource::<GameplayConfig>()
        .cloned()
        .unwrap_or_default();
    world.insert_resource(OptionsDraft {
        graphics,
        controls,
        audio,
        multiplayer,
        gameplay,
        ..default()
    });
    let mut menu = world.resource_mut::<MenuState>();
    menu.options_return = menu.page;
    menu.page = MenuPage::Options;
    menu.error.clear();
    menu.status.clear();
}
pub(super) fn dirty(world: &World) -> bool {
    let draft = world.resource::<OptionsDraft>();
    draft.graphics != *world.resource::<GraphicsConfig>()
        || draft.controls != *world.resource::<ControlsConfig>()
        || draft.audio != *world.resource::<AudioConfig>()
        || draft.multiplayer
            != world
                .get_resource::<MultiplayerConfig>()
                .cloned()
                .unwrap_or_default()
        || draft.gameplay
            != world
                .get_resource::<GameplayConfig>()
                .cloned()
                .unwrap_or_default()
}
pub(super) fn back(world: &mut World) {
    world.resource_mut::<OptionsDraft>().rebinding = None;
    let mut menu = world.resource_mut::<MenuState>();
    menu.page = menu.options_return;
    menu.dialog = MenuDialog::None;
    menu.error.clear();
    menu.status.clear();
}
pub(super) fn reset(world: &mut World) {
    let mut draft = world.resource_mut::<OptionsDraft>();
    match draft.tab {
        OptionsTab::Graphics => draft.graphics = GraphicsConfig::default(),
        OptionsTab::Gameplay => draft.gameplay = GameplayConfig::default(),
        OptionsTab::Controls => draft.controls = ControlsConfig::default(),
        OptionsTab::Audio => draft.audio = AudioConfig::default(),
        OptionsTab::Multiplayer => draft.multiplayer = MultiplayerConfig::default(),
    }
    draft.rebinding = None;
    world.resource_mut::<MenuState>().status = "Defaults selected. Apply to save.".into();
}
pub(super) fn change(world: &mut World, control: &str, value: &str) -> Result<(), String> {
    if value == "custom" && matches!(control, "graphics-resolution" | "graphics-fps") {
        return Ok(());
    }
    let mut draft = world.resource_mut::<OptionsDraft>();
    let number = || {
        value
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| "Enter a finite number".to_string())
    };
    let flag = || {
        value
            .parse::<bool>()
            .map_err(|_| "Invalid switch value".to_string())
    };
    let graphics = &mut draft.graphics;
    match control {
        "graphics-resolution" => {
            if value != "desktop" || graphics.window_mode != WindowModeSetting::Borderless {
                let (w, h) = value.split_once('x').ok_or("Invalid resolution")?;
                let width = w.parse().map_err(|_| "Invalid width")?;
                let height = h.parse().map_err(|_| "Invalid height")?;
                graphics.width = width;
                graphics.height = height;
                if graphics.window_mode == WindowModeSetting::Borderless {
                    graphics.window_mode = WindowModeSetting::Windowed;
                }
            }
        }
        "graphics-window" => {
            graphics.window_mode = match value {
                "windowed" => WindowModeSetting::Windowed,
                "borderless" => WindowModeSetting::Borderless,
                "fullscreen" => WindowModeSetting::Fullscreen,
                _ => return Err("Invalid window mode".into()),
            }
        }
        "graphics-vsync" => graphics.vsync = flag()?,
        "graphics-fps" => graphics.fps_limit = value.parse().map_err(|_| "Invalid FPS limit")?,
        "graphics-ui-fps" => graphics.ui_fps = value.parse().map_err(|_| "Invalid UI FPS")?,
        "graphics-block-detail" | "graphics-leaf-detail" => {
            let quality = match value {
                "low" => terra_core::BlockDetailQuality::Low,
                "medium" => terra_core::BlockDetailQuality::Medium,
                "high" => terra_core::BlockDetailQuality::High,
                _ => return Err("Invalid detail quality".into()),
            };
            if control == "graphics-block-detail" {
                graphics.blocks.detail_quality = quality;
            } else {
                graphics.blocks.leaf_geometry_quality = quality;
            }
        }
        "graphics-micro" => graphics.blocks.micro_geometry = flag()?,
        "graphics-micro-distance" => {
            graphics.blocks.micro_geometry_distance =
                value.parse().map_err(|_| "Invalid detail distance")?
        }
        "graphics-material-detail" => graphics.blocks.material_detail = flag()?,
        "graphics-normal-detail" => graphics.blocks.normal_mapping = flag()?,
        "graphics-surface-variation" => graphics.blocks.surface_variation = flag()?,
        "graphics-distance" => {
            let distance = number()?;
            if !(2.0..=32.0).contains(&distance) || distance.fract() != 0.0 {
                return Err("Render distance must be an integer from 2 to 32".into());
            }
            graphics.render_distance = distance as u8;
        }
        "graphics-lod" => graphics.lod_distance = number()? / 100.0,
        "graphics-fog" => graphics.fog_distance = number()? / 100.0,
        "graphics-chunk-animation" => {
            graphics.chunk_animation = match value {
                "none" => terra_core::ChunkAnimation::None,
                "fade" => terra_core::ChunkAnimation::Fade,
                "fade_slow" => terra_core::ChunkAnimation::FadeSlow,
                "bounce" => terra_core::ChunkAnimation::Bounce,
                "rise" => terra_core::ChunkAnimation::Rise,
                "slide" => terra_core::ChunkAnimation::Slide,
                _ => return Err("Invalid chunk animation".into()),
            };
        }
        "graphics-tree-fall" => {
            graphics.tree_fall_animation = match value {
                "current" => terra_core::TreeFallAnimation::Current,
                "hell_fall" => terra_core::TreeFallAnimation::HellFall,
                "magic" => terra_core::TreeFallAnimation::Magic,
                _ => return Err("Invalid tree fall animation".into()),
            };
        }
        "graphics-shadow" => {
            graphics.shadow_quality = match value {
                "off" => ShadowQuality::Off,
                "low" => ShadowQuality::Low,
                "medium" => ShadowQuality::Medium,
                "high" => ShadowQuality::High,
                _ => return Err("Invalid shadow quality".into()),
            }
        }
        "graphics-water" => {
            graphics.water_quality = match value {
                "low" => WaterQuality::Low,
                "medium" => WaterQuality::Medium,
                "high" => WaterQuality::High,
                _ => return Err("Invalid water quality".into()),
            }
        }
        "graphics-caves" => {
            graphics.cave_quality = match value {
                "low" => terra_core::CaveQuality::Low,
                "medium" => terra_core::CaveQuality::Medium,
                "high" => terra_core::CaveQuality::High,
                "ultra" => terra_core::CaveQuality::Ultra,
                _ => return Err("Invalid cave quality".into()),
            };
        }
        "graphics-brightness" => graphics.brightness = number()? / 100.0,
        "graphics-scale" => {
            let percent = number()?;
            if !(75.0..=150.0).contains(&percent) {
                return Err("UI scale must be between 75% and 150%".into());
            }
            graphics.ui_scale = percent / 100.0;
        }
        "graphics-fov" => graphics.fov = number()?,
        "graphics-reflections" => graphics.water_reflections = flag()?,
        "graphics-mist" => graphics.water_volumetric_mist = flag()?,
        "graphics-waves" => graphics.water_wave_strength = number()?,
        "graphics-overlay" => graphics.debug_overlay = flag()?,
        "weather-quality" => {
            graphics.weather_quality = match value {
                "off" => terra_core::WeatherQuality::Off,
                "low" => terra_core::WeatherQuality::Low,
                "medium" => terra_core::WeatherQuality::Medium,
                "high" => terra_core::WeatherQuality::High,
                "ultra" => terra_core::WeatherQuality::Ultra,
                _ => return Err("Invalid weather quality".into()),
            }
        }
        "controls-sensitivity" => draft.controls.mouse_sensitivity = number()? / 1000.0,
        "controls-auto-climb" => draft.controls.auto_climb = flag()?,
        "heartbeat" => draft.gameplay.heartbeat = flag()?,
        "game-language" => {
            draft.gameplay.language = match value {
                "de" => terra_core::GameLanguage::German,
                "en" => terra_core::GameLanguage::English,
                _ => return Err("Unsupported language".into()),
            }
        }
        "view-bobbing" => draft.gameplay.view_bobbing = flag()?,
        "mob-health-bars" => {
            draft.gameplay.mob_health_bars = match value {
                "in_view" => terra_core::MobHealthBars::InView,
                "looking_at" => terra_core::MobHealthBars::LookingAt,
                "always" => terra_core::MobHealthBars::Always,
                "none" => terra_core::MobHealthBars::None,
                _ => return Err("Invalid mob health bar mode".into()),
            };
        }
        "mob-health-friendly-color" | "mob-health-hostile-color" | "mob-health-font-color" => {
            let color = rgb(value)?;
            match control {
                "mob-health-friendly-color" => draft.gameplay.mob_health_friendly_color = color,
                "mob-health-hostile-color" => draft.gameplay.mob_health_hostile_color = color,
                _ => draft.gameplay.mob_health_font_color = color,
            }
        }
        "minimap-enabled" => draft.gameplay.minimap.enabled = flag()?,
        "minimap-size" => {
            let size = number()?;
            if !(180.0..=360.0).contains(&size) || size.fract() != 0.0 {
                return Err("MiniMap size must be an integer from 180 to 360".into());
            }
            draft.gameplay.minimap.size = size as u16;
        }
        "minimap-zoom" => {
            let zoom = number()?;
            if !(0.0..=3.0).contains(&zoom) || zoom.fract() != 0.0 {
                return Err("MiniMap zoom must be an integer from 0 to 3".into());
            }
            draft.gameplay.minimap.zoom = zoom as u8;
        }
        "minimap-mode" => {
            draft.gameplay.minimap.mode = match value {
                "auto" => MapMode::Auto,
                "surface" => MapMode::Surface,
                "cave" => MapMode::Cave,
                _ => return Err("Invalid MiniMap mode".into()),
            };
        }
        "minimap-fps" => {
            let fps = value.parse::<u8>().map_err(|_| "Invalid MiniMap FPS")?;
            if ![24, 30, 40, 60].contains(&fps) {
                return Err("MiniMap FPS must be 24, 30, 40 or 60".into());
            }
            draft.gameplay.minimap.fps = fps;
        }
        "minimap-icon-scale" => {
            let scale = number()?;
            if !(1.0..=2.5).contains(&scale) {
                return Err("MiniMap icon scale must be between 1 and 2.5".into());
            }
            draft.gameplay.minimap.icon_scale = scale;
        }
        "minimap-rotate" => draft.gameplay.minimap.rotate = flag()?,
        "minimap-relief" => draft.gameplay.minimap.relief = flag()?,
        "minimap-mobs" => draft.gameplay.minimap.mobs = flag()?,
        "minimap-players" => draft.gameplay.minimap.players = flag()?,
        "minimap-coordinates" => draft.gameplay.minimap.coordinates = flag()?,
        "minimap-biome" => draft.gameplay.minimap.biome = flag()?,
        "minimap-clock" => draft.gameplay.minimap.clock = flag()?,
        "audio-muted" => {
            let muted = flag()?;
            if draft.audio.muted != muted {
                draft.audio.muted = muted;
                world.write_message(AudioAudition::Master);
            }
        }
        id if id.starts_with("audio-") => {
            let audio = &mut draft.audio;
            let (gain, cue) = match id {
                "audio-master" => (&mut audio.master_volume, AudioAudition::Master),
                "audio-ambience" => (&mut audio.ambience_volume, AudioAudition::Ambience),
                "audio-animals" => (&mut audio.animals_volume, AudioAudition::Animals),
                "audio-footsteps" => (&mut audio.footsteps_volume, AudioAudition::Footsteps),
                "audio-water" => (&mut audio.water_volume, AudioAudition::Water),
                "audio-river" => (&mut audio.river_volume, AudioAudition::River),
                "audio-waterfall" => (&mut audio.waterfall_volume, AudioAudition::Waterfall),
                _ => return Err("Unknown setting".into()),
            };
            let value = number()? / 100.0;
            if *gain != value {
                *gain = value;
                if (0.0..=1.0).contains(&value) {
                    world.write_message(cue);
                }
            }
        }
        "multiplayer-names" => draft.multiplayer.player_names = flag()?,
        "multiplayer-name-distance" => draft.multiplayer.player_name_distance = number()?,
        "multiplayer-cursors" => draft.multiplayer.chest_cursors = flag()?,
        "multiplayer-random-color" => {
            draft.multiplayer.cursor_color = if flag()? { None } else { Some([78, 190, 140]) };
        }
        "multiplayer-color" => {
            draft.multiplayer.cursor_color = Some(rgb(value)?);
        }
        _ => return Err("Unknown setting".into()),
    }
    world.resource_mut::<MenuState>().error.clear();
    world.resource_mut::<MenuState>().status.clear();
    Ok(())
}

fn rgb(value: &str) -> Result<[u8; 3], String> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.is_ascii() {
        return Err("Expected an RGB color in #RRGGBB format".into());
    }
    let mut rgb = [0; 3];
    for (channel, component) in rgb.iter_mut().enumerate() {
        *component = u8::from_str_radix(&value[channel * 2..channel * 2 + 2], 16)
            .map_err(|_| "Invalid RGB color".to_owned())?;
    }
    Ok(rgb)
}
pub(super) fn apply(world: &mut World) -> Result<(), String> {
    let draft = world.resource::<OptionsDraft>();
    draft.graphics.validate().map_err(|e| e.to_string())?;
    draft.controls.validate().map_err(|e| e.to_string())?;
    draft.audio.validate().map_err(|e| e.to_string())?;
    draft.multiplayer.validate().map_err(|e| e.to_string())?;
    draft.gameplay.validate().map_err(|e| e.to_string())?;
    let graphics = draft.graphics.clone();
    let controls = draft.controls.clone();
    let audio = draft.audio.clone();
    let previous = world.resource::<GraphicsConfig>().clone();
    let old_controls = world.resource::<ControlsConfig>().clone();
    let old_audio = world.resource::<AudioConfig>().clone();
    let old_multiplayer = world
        .get_resource::<MultiplayerConfig>()
        .cloned()
        .unwrap_or_default();
    let old_gameplay = world
        .get_resource::<GameplayConfig>()
        .cloned()
        .unwrap_or_default();
    if graphics.window_mode == WindowModeSetting::Fullscreen
        && fullscreen_video_mode(world, &graphics).is_none()
    {
        return Err("This monitor does not support the selected exclusive fullscreen resolution. Choose another resolution or Borderless.".into());
    }
    if graphics.display_changed(&previous) {
        apply_runtime(world, &graphics, &controls, &audio);
        world.resource_mut::<OptionsDraft>().display = Some(DisplayConfirmation {
            previous,
            controls: old_controls,
            audio: old_audio,
            multiplayer: old_multiplayer,
            gameplay: old_gameplay,
            until: Instant::now() + Duration::from_secs(15),
        });
        world.resource_mut::<MenuState>().dialog = MenuDialog::Display;
        Ok(())
    } else {
        save(world, &graphics, &controls, &audio)?;
        apply_runtime(world, &graphics, &controls, &audio);
        world.insert_resource(world.resource::<OptionsDraft>().multiplayer.clone());
        world.insert_resource(world.resource::<OptionsDraft>().gameplay.clone());
        world.resource_mut::<MenuState>().status = "Settings saved".into();
        Ok(())
    }
}
fn save(
    world: &World,
    graphics: &GraphicsConfig,
    controls: &ControlsConfig,
    audio: &AudioConfig,
) -> Result<(), String> {
    let root = &world.resource::<MenuPaths>().configs;
    let mut saved = 0;
    let result = (|| {
        graphics
            .save(root.join("graphics.toml"))
            .map_err(|e| format!("Cannot save graphics: {e}"))?;
        saved = 1;
        controls
            .save(root.join("controls.toml"))
            .map_err(|e| format!("Cannot save controls: {e}"))?;
        saved = 2;
        audio
            .save(root.join("audio.toml"))
            .map_err(|e| format!("Cannot save audio: {e}"))?;
        saved = 3;
        world
            .resource::<OptionsDraft>()
            .multiplayer
            .save(root.join("multiplayer.toml"))
            .map_err(|e| format!("Cannot save multiplayer preferences: {e}"))?;
        saved = 4;
        world
            .resource::<OptionsDraft>()
            .gameplay
            .save(root.join("gameplay.toml"))
            .map_err(|e| format!("Cannot save gameplay preferences: {e}"))?;
        saved = 5;
        crate::localization::save_profile_language(world)?;
        Ok::<(), String>(())
    })();
    if let Err(mut error) = result {
        let draft = world.resource::<OptionsDraft>();
        let previous_graphics = draft.display.as_ref().map_or_else(
            || world.resource::<GraphicsConfig>(),
            |display| &display.previous,
        );
        let previous_controls = draft.display.as_ref().map_or_else(
            || world.resource::<ControlsConfig>(),
            |display| &display.controls,
        );
        if saved >= 5 {
            let gameplay = draft.display.as_ref().map_or_else(
                || {
                    world
                        .get_resource::<GameplayConfig>()
                        .cloned()
                        .unwrap_or_default()
                },
                |display| display.gameplay.clone(),
            );
            if let Err(rollback) = gameplay.save(root.join("gameplay.toml")) {
                error.push_str(&format!(". Gameplay rollback also failed: {rollback}"));
            }
        }
        if saved >= 4 {
            let multiplayer = draft.display.as_ref().map_or_else(
                || {
                    world
                        .get_resource::<MultiplayerConfig>()
                        .cloned()
                        .unwrap_or_default()
                },
                |display| display.multiplayer.clone(),
            );
            if let Err(rollback) = multiplayer.save(root.join("multiplayer.toml")) {
                error.push_str(&format!(". Multiplayer rollback also failed: {rollback}"));
            }
        }
        if saved >= 3 {
            let audio = draft
                .display
                .as_ref()
                .map_or_else(|| world.resource::<AudioConfig>(), |display| &display.audio);
            if let Err(rollback) = audio.save(root.join("audio.toml")) {
                error.push_str(&format!(". Audio rollback also failed: {rollback}"));
            }
        }
        if saved >= 2
            && let Err(rollback) = previous_controls.save(root.join("controls.toml"))
        {
            error.push_str(&format!(". Controls file rollback also failed: {rollback}"));
        }
        if saved >= 1
            && let Err(rollback) = previous_graphics.save(root.join("graphics.toml"))
        {
            error.push_str(&format!(". Graphics file rollback also failed: {rollback}"));
        }
        return Err(error);
    }
    Ok(())
}
pub(super) fn keep_display(world: &mut World) -> Result<(), String> {
    let draft = world.resource::<OptionsDraft>();
    let graphics = draft.graphics.clone();
    let controls = draft.controls.clone();
    let audio = draft.audio.clone();
    if let Err(error) = save(world, &graphics, &controls, &audio) {
        revert_display(world);
        return Err(error);
    }
    world.resource_mut::<OptionsDraft>().display = None;
    world.insert_resource(world.resource::<OptionsDraft>().multiplayer.clone());
    world.insert_resource(world.resource::<OptionsDraft>().gameplay.clone());
    let mut menu = world.resource_mut::<MenuState>();
    menu.dialog = MenuDialog::None;
    menu.status = "Display confirmed. Settings saved.".into();
    Ok(())
}
pub(super) fn revert_display(world: &mut World) {
    if let Some(previous) = world.resource_mut::<OptionsDraft>().display.take() {
        apply_runtime(
            world,
            &previous.previous,
            &previous.controls,
            &previous.audio,
        );
        let mut draft = world.resource_mut::<OptionsDraft>();
        draft.graphics = previous.previous;
        draft.controls = previous.controls;
        draft.audio = previous.audio;
        draft.multiplayer = previous.multiplayer.clone();
        draft.gameplay = previous.gameplay.clone();
        world.insert_resource(previous.multiplayer);
        world.insert_resource(previous.gameplay);
    }
    let mut menu = world.resource_mut::<MenuState>();
    menu.dialog = MenuDialog::None;
    menu.status = "Display settings reverted".into();
}
pub(super) fn display_timeout(world: &mut World) {
    if world
        .resource::<OptionsDraft>()
        .display
        .as_ref()
        .is_some_and(|p| Instant::now() >= p.until)
    {
        revert_display(world);
    }
}
fn apply_runtime(
    world: &mut World,
    graphics: &GraphicsConfig,
    controls: &ControlsConfig,
    audio: &AudioConfig,
) {
    let fullscreen = fullscreen_video_mode(world, graphics)
        .map_or(VideoModeSelection::Current, VideoModeSelection::Specific);
    let mut config = world.resource_mut::<GameConfig>();
    graphics.apply_to(&mut config.client);
    config.client.mouse_sensitivity = controls.mouse_sensitivity;
    world.insert_resource(graphics.clone());
    world.insert_resource(controls.clone());
    world.insert_resource(audio.clone());
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    for mut window in windows.iter_mut(world) {
        window
            .resolution
            .set_physical_resolution(graphics.width, graphics.height);
        window.mode = match graphics.window_mode {
            WindowModeSetting::Windowed => WindowMode::Windowed,
            WindowModeSetting::Borderless => {
                WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            WindowModeSetting::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Current, fullscreen)
            }
        };
    }
}
fn fullscreen_video_mode(world: &mut World, graphics: &GraphicsConfig) -> Option<VideoMode> {
    let current = world
        .query_filtered::<&OnMonitor, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|monitor| monitor.0);
    let monitor = current.and_then(|entity| world.get::<Monitor>(entity))?;
    monitor
        .video_modes
        .iter()
        .filter(|mode| mode.physical_size == UVec2::new(graphics.width, graphics.height))
        .max_by_key(|mode| (mode.refresh_rate_millihertz, mode.bit_depth))
        .copied()
}
pub(super) fn sync_fullscreen(world: &mut World) {
    let graphics = world.resource::<GraphicsConfig>().clone();
    if graphics.window_mode != WindowModeSetting::Fullscreen {
        return;
    }
    let pending = world
        .query_filtered::<(&Window, &OnMonitor), With<PrimaryWindow>>()
        .iter(world)
        .any(|(window, _)| {
            matches!(
                window.mode,
                WindowMode::Fullscreen(_, VideoModeSelection::Current)
            )
        });
    if !pending {
        return;
    }
    if let Some(mode) = fullscreen_video_mode(world, &graphics) {
        for mut window in world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .iter_mut(world)
        {
            window.mode = WindowMode::Fullscreen(
                MonitorSelection::Current,
                VideoModeSelection::Specific(mode),
            );
        }
    } else {
        let mut fallback = graphics;
        fallback.window_mode = WindowModeSetting::Windowed;
        let controls = world.resource::<ControlsConfig>().clone();
        let audio = world.resource::<AudioConfig>().clone();
        apply_runtime(world, &fallback, &controls, &audio);
        world.resource_mut::<OptionsDraft>().graphics = fallback;
        world.resource_mut::<MenuState>().error =
            "Saved fullscreen resolution is unavailable on this monitor; using Windowed.".into();
    }
}
pub(super) fn apply_ui_scale(
    graphics: Res<GraphicsConfig>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<bevy::ui::UiScale>,
) {
    if let Some(window) = windows.iter().next() {
        let next = fitted_ui_scale(graphics.ui_scale, window.size());
        if scale.0 != next {
            scale.0 = next;
        }
    }
}

pub(super) fn fitted_ui_scale(requested: f32, viewport: Vec2) -> f32 {
    // Preserve room for compact menus while zooming relative to the fitted 100% layout.
    let ceiling = (viewport.x / 900.0).min(viewport.y / 640.0);
    (ui_scale_base(viewport) * requested).min(ceiling).max(0.1)
}

pub(super) fn ui_scale_base(viewport: Vec2) -> f32 {
    1.0_f32
        .min(viewport.x / 1000.0)
        .min(viewport.y / 740.0)
        .max(0.1)
}
pub(super) fn capture_binding(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut draft: ResMut<OptionsDraft>,
    mut menu: ResMut<MenuState>,
    mut pending: Local<Option<KeyCode>>,
) {
    draft.consumed_escape = false;
    let Some(action) = draft.rebinding else {
        *pending = None;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        draft.rebinding = None;
        draft.consumed_escape = true;
        draft.armed = false;
        *pending = None;
        return;
    }
    if !draft.armed {
        draft.armed = true;
        return;
    }
    let binding = captured_binding(&keys, &buttons, &mut pending, action);
    if let Some(binding) = binding {
        match draft.controls.rebind(action, binding) {
            Ok(()) => {
                *pending = None;
                draft.rebinding = None;
                draft.armed = false;
                menu.error.clear();
            }
            Err(error) => menu.error = error,
        }
    }
}

fn captured_binding(
    keys: &ButtonInput<KeyCode>,
    buttons: &ButtonInput<MouseButton>,
    pending: &mut Option<KeyCode>,
    action: GameAction,
) -> Option<InputBinding> {
    if let Some(modifier) = *pending {
        if keys.just_released(modifier) {
            *pending = None;
            return Some(InputBinding::Key(modifier));
        }
        return keys
            .get_just_pressed()
            .find(|key| **key != modifier)
            .copied()
            .map(|key| InputBinding::Chord { modifier, key });
    }
    // Modifier keys and the debug-bounds action wait for a second key or release.
    let modifier = [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::F1,
    ]
    .into_iter()
    .find(|key| keys.just_pressed(*key));
    if let Some(key) = modifier.or_else(|| keys.get_just_pressed().next().copied()) {
        if modifier.is_some() || action == GameAction::DebugBounds {
            *pending = Some(key);
            return keys
                .get_just_pressed()
                .find(|other| **other != key)
                .copied()
                .map(|other| InputBinding::Chord {
                    modifier: key,
                    key: other,
                });
        }
        return Some(InputBinding::Key(key));
    }
    buttons
        .get_just_pressed()
        .next()
        .copied()
        .map(InputBinding::Mouse)
}

#[cfg(test)]
mod chord_capture_tests {
    use super::*;

    #[test]
    fn debug_chord_capture_waits_for_second_key_or_release() {
        let mut keys = ButtonInput::default();
        let buttons = ButtonInput::default();
        let mut pending = None;
        keys.press(KeyCode::F3);
        assert_eq!(
            captured_binding(&keys, &buttons, &mut pending, GameAction::DebugBounds),
            None
        );
        keys.clear();
        keys.press(KeyCode::KeyG);
        assert_eq!(
            captured_binding(&keys, &buttons, &mut pending, GameAction::DebugBounds),
            Some(InputBinding::Chord {
                modifier: KeyCode::F3,
                key: KeyCode::KeyG
            })
        );
        pending = None;
        keys.reset_all();
        keys.press(KeyCode::F1);
        assert_eq!(
            captured_binding(&keys, &buttons, &mut pending, GameAction::DebugOverlay),
            None
        );
        keys.clear();
        keys.release(KeyCode::F1);
        assert_eq!(
            captured_binding(&keys, &buttons, &mut pending, GameAction::DebugOverlay),
            Some(InputBinding::Key(KeyCode::F1))
        );
    }
}
