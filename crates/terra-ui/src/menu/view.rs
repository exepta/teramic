use super::{
    model::*,
    options::{OptionsDraft, OptionsTab},
};
use bevy::prelude::*;
use serde::Serialize;
use terra_core::{
    AudioConfig, GameAction, GraphicsConfig, ShadowQuality, WaterQuality, WindowModeSetting,
};
use tilt_ui::html_shared;

#[derive(Clone, Serialize, PartialEq)]
pub struct MenuButton {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub enabled: bool,
    pub primary: bool,
}
fn button(
    id: &str,
    title: &str,
    subtitle: &str,
    icon: &str,
    enabled: bool,
    primary: bool,
) -> MenuButton {
    MenuButton {
        id: id.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        icon: icon.into(),
        enabled,
        primary,
    }
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct MenuView {
    pub visible: bool,
    pub style: String,
    pub main: bool,
    pub worlds: bool,
    pub servers: bool,
    pub options: bool,
    pub pause: bool,
    pub loading: bool,
    pub title: String,
    pub error: String,
    pub status: String,
    pub version: String,
    pub buttons: Vec<MenuButton>,
    pub cancel_connect: bool,
    pub in_game: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct CardView {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub selected: bool,
    pub compatible: bool,
    pub icon: String,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct WorldsView {
    pub cards: Vec<CardView>,
    pub count: String,
    pub empty: bool,
    pub buttons: Vec<MenuButton>,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct ServersView {
    pub cards: Vec<CardView>,
    pub count: String,
    pub empty: bool,
    pub buttons: Vec<MenuButton>,
    pub discovery: String,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct SessionView {
    pub title: String,
    pub mode: String,
    pub seed: String,
    pub player: String,
    pub uuid: String,
    pub elapsed: String,
    pub progress: f32,
    pub show_progress: bool,
    pub progress_label: String,
    pub detail: String,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct SettingRow {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub value: String,
    pub number: f32,
    pub checked: bool,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub label: String,
    pub choices: Vec<Choice>,
    pub custom: bool,
    pub enabled: bool,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct OptionsView {
    pub graphics: bool,
    pub gameplay: bool,
    pub controls: bool,
    pub audio: bool,
    pub multiplayer: bool,
    pub dirty: bool,
    pub rows: Vec<SettingRow>,
    pub listening: bool,
    pub prompt: String,
    pub buttons: Vec<MenuButton>,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct DialogView {
    pub visible: bool,
    pub create: bool,
    pub natural_mob_limit: u16,
    pub server: bool,
    pub minimap: bool,
    pub mob_health: bool,
    pub rows: Vec<SettingRow>,
    pub title: String,
    pub message: String,
    pub name: String,
    pub seed: String,
    pub mode: String,
    pub address: String,
    pub port: String,
    pub error: String,
    pub buttons: Vec<MenuButton>,
}

fn slider(
    id: &str,
    title: &str,
    number: f32,
    min: f32,
    max: f32,
    step: f32,
    label: String,
) -> SettingRow {
    SettingRow {
        id: id.into(),
        title: title.into(),
        kind: "slider".into(),
        value: String::new(),
        number,
        checked: false,
        min,
        max,
        step,
        label,
        choices: Vec::new(),
        custom: false,
        enabled: true,
    }
}
fn select(id: &str, title: &str, value: String, choices: Vec<Choice>) -> SettingRow {
    let custom = !choices.iter().any(|choice| choice.value == value);
    SettingRow {
        id: id.into(),
        title: title.into(),
        kind: "select".into(),
        value,
        number: 0.0,
        checked: false,
        min: 0.0,
        max: 0.0,
        step: 1.0,
        label: String::new(),
        choices,
        custom,
        enabled: true,
    }
}
fn choices(values: &[(&str, &str)]) -> Vec<Choice> {
    values
        .iter()
        .map(|(value, label)| Choice {
            value: (*value).into(),
            label: (*label).into(),
        })
        .collect()
}
pub(super) fn resolution_choices(world: &mut World) -> Vec<Choice> {
    use bevy::window::{Monitor, OnMonitor, PrimaryMonitor, PrimaryWindow};
    let current = world
        .query_filtered::<&OnMonitor, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|on| on.0);
    let primary = world
        .query_filtered::<Entity, With<PrimaryMonitor>>()
        .iter(world)
        .next();
    let monitor = current.or(primary).and_then(|id| world.get::<Monitor>(id));
    let mut sizes = std::collections::BTreeSet::new();
    let limit = monitor.map(|m| UVec2::new(m.physical_width, m.physical_height));
    for size in [
        (1280, 720),
        (1280, 800),
        (1600, 900),
        (1920, 1080),
        (1920, 1200),
        (2560, 1440),
        (2560, 1600),
        (3440, 1440),
        (3840, 2160),
        (3840, 2400),
        (5120, 1440),
        (5120, 2880),
        (7680, 4320),
    ] {
        if limit.is_none_or(|limit| size.0 <= limit.x && size.1 <= limit.y) {
            sizes.insert(size);
        }
    }
    if let Some(monitor) = monitor {
        sizes.insert((monitor.physical_width, monitor.physical_height));
        for mode in &monitor.video_modes {
            sizes.insert((mode.physical_size.x, mode.physical_size.y));
        }
    }
    sizes
        .into_iter()
        .filter(|&(w, h)| (640..=7680).contains(&w) && (480..=4320).contains(&h))
        .map(|(w, h)| Choice {
            value: format!("{w}x{h}"),
            label: format!("{w} x {h}"),
        })
        .collect()
}

fn graphics_rows(
    g: &GraphicsConfig,
    resolutions: Vec<Choice>,
    viewport: Option<Vec2>,
) -> Vec<SettingRow> {
    let current = format!("{}x{}", g.width, g.height);
    let fitted = viewport.map_or(g.ui_scale, |size| {
        super::options::fitted_ui_scale(g.ui_scale, size) / super::options::ui_scale_base(size)
    });
    let mut rows = vec![
        select("graphics-resolution", "Resolution", current, resolutions),
        select(
            "graphics-window",
            "Window Mode",
            match g.window_mode {
                WindowModeSetting::Windowed => "windowed",
                WindowModeSetting::Borderless => "borderless",
                WindowModeSetting::Fullscreen => "fullscreen",
            }
            .into(),
            choices(&[
                ("windowed", "Windowed"),
                ("borderless", "Borderless"),
                ("fullscreen", "Fullscreen"),
            ]),
        ),
        select(
            "graphics-fps",
            "FPS Limit",
            g.fps_limit.to_string(),
            choices(&[
                ("0", "Unlimited"),
                ("30", "30"),
                ("60", "60"),
                ("90", "90"),
                ("120", "120"),
                ("144", "144"),
                ("165", "165"),
                ("240", "240"),
            ]),
        ),
        select(
            "graphics-ui-fps",
            "UI FPS",
            g.ui_fps.to_string(),
            choices(&[
                ("24", "24"),
                ("30", "30"),
                ("40", "40"),
                ("60", "60"),
                ("90", "90"),
                ("120", "120"),
                ("144", "144"),
                ("240", "240"),
            ]),
        ),
        slider(
            "graphics-distance",
            "Render Distance",
            f32::from(g.render_distance),
            2.0,
            32.0,
            1.0,
            format!("{} chunks", g.render_distance),
        ),
        slider(
            "graphics-lod",
            "LOD Distance",
            g.lod_distance * 100.0,
            75.0,
            200.0,
            5.0,
            format!("{:.0}%", g.lod_distance * 100.0),
        ),
        select(
            "graphics-shadow",
            "Shadow Quality",
            match g.shadow_quality {
                ShadowQuality::Off => "off",
                ShadowQuality::Low => "low",
                ShadowQuality::Medium => "medium",
                ShadowQuality::High => "high",
            }
            .into(),
            choices(&[
                ("off", "Off"),
                ("low", "Low"),
                ("medium", "Medium"),
                ("high", "High"),
            ]),
        ),
        select(
            "graphics-water",
            "Water Quality",
            match g.water_quality {
                WaterQuality::Low => "low",
                WaterQuality::Medium => "medium",
                WaterQuality::High => "high",
            }
            .into(),
            choices(&[("low", "Low"), ("medium", "Medium"), ("high", "High")]),
        ),
        select(
            "graphics-caves",
            "Cave Lighting",
            match g.cave_quality {
                terra_core::CaveQuality::Low => "low",
                terra_core::CaveQuality::Medium => "medium",
                terra_core::CaveQuality::High => "high",
                terra_core::CaveQuality::Ultra => "ultra",
            }
            .into(),
            choices(&[
                ("low", "Low"),
                ("medium", "Medium"),
                ("high", "High"),
                ("ultra", "Ultra"),
            ]),
        ),
        slider(
            "graphics-brightness",
            "Brightness",
            g.brightness * 100.0,
            50.0,
            150.0,
            5.0,
            format!("{:.0}%", g.brightness * 100.0),
        ),
        slider(
            "graphics-fov",
            "Field of View",
            g.fov,
            45.0,
            110.0,
            1.0,
            format!("{:.0} degrees", g.fov),
        ),
    ];
    rows.insert(
        2,
        slider(
            "graphics-scale",
            "UI Scale",
            g.ui_scale * 100.0,
            75.0,
            150.0,
            5.0,
            if fitted + 0.005 < g.ui_scale {
                format!("{:.0}% (fit {:.0}%)", g.ui_scale * 100.0, fitted * 100.0)
            } else {
                format!("{:.0}%", g.ui_scale * 100.0)
            },
        ),
    );
    for (id, label, quality) in [
        (
            "graphics-block-detail",
            "Block Detail",
            g.blocks.detail_quality,
        ),
        (
            "graphics-leaf-detail",
            "Leaf Geometry",
            g.blocks.leaf_geometry_quality,
        ),
    ] {
        rows.push(select(
            id,
            label,
            match quality {
                terra_core::BlockDetailQuality::Low => "low",
                terra_core::BlockDetailQuality::Medium => "medium",
                terra_core::BlockDetailQuality::High => "high",
            }
            .into(),
            choices(&[("low", "Low"), ("medium", "Medium"), ("high", "High")]),
        ));
    }
    rows.push(slider(
        "graphics-micro-distance",
        "Micro Geometry Distance",
        g.blocks.micro_geometry_distance,
        16.0,
        96.0,
        8.0,
        format!("{} m", g.blocks.micro_geometry_distance),
    ));
    for (id, title, checked) in [
        ("graphics-micro", "Micro Geometry", g.blocks.micro_geometry),
        (
            "graphics-material-detail",
            "Block Surface Shader",
            g.blocks.material_detail,
        ),
        (
            "graphics-normal-detail",
            "Surface Normals",
            g.blocks.normal_mapping,
        ),
        (
            "graphics-surface-variation",
            "Surface Variation",
            g.blocks.surface_variation,
        ),
    ] {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, String::new());
        row.kind = "switch".into();
        row.checked = checked;
        rows.push(row);
    }
    rows.push(slider(
        "graphics-fog",
        "Fog Distance",
        g.fog_distance * 100.0,
        0.0,
        150.0,
        5.0,
        if g.fog_distance == 0.0 {
            "Off".into()
        } else {
            format!("{:.0}%", g.fog_distance * 100.0)
        },
    ));
    rows.push(select(
        "graphics-chunk-animation",
        "Chunk Animation",
        match g.chunk_animation {
            terra_core::ChunkAnimation::None => "none",
            terra_core::ChunkAnimation::Fade => "fade",
            terra_core::ChunkAnimation::FadeSlow => "fade_slow",
            terra_core::ChunkAnimation::Bounce => "bounce",
            terra_core::ChunkAnimation::Rise => "rise",
            terra_core::ChunkAnimation::Slide => "slide",
        }
        .into(),
        choices(&[
            ("none", "Instant (Default)"),
            ("fade", "Fade In / Out"),
            ("fade_slow", "Slow Fade In / Out"),
            ("bounce", "Bounce"),
            ("rise", "Rise"),
            ("slide", "Glide"),
        ]),
    ));
    rows.push(select(
        "graphics-tree-fall",
        "Tree Fall Animation",
        match g.tree_fall_animation {
            terra_core::TreeFallAnimation::Current => "current",
            terra_core::TreeFallAnimation::HellFall => "hell_fall",
            terra_core::TreeFallAnimation::Magic => "magic",
        }
        .into(),
        choices(&[
            ("current", "Current (Default)"),
            ("hell_fall", "Hell Fall"),
            ("magic", "Magic"),
        ]),
    ));
    if g.window_mode == WindowModeSetting::Borderless {
        let resolution = &mut rows[0];
        resolution.value = "Desktop".into();
        resolution.custom = true;
        resolution.enabled = false;
    }
    let mut vsync = slider("graphics-vsync", "VSync", 0.0, 0.0, 0.0, 1.0, String::new());
    vsync.kind = "switch".into();
    vsync.checked = g.vsync;
    rows.insert(2, vsync);
    for (id, title, checked) in [
        (
            "graphics-reflections",
            "Scene Reflections",
            g.water_reflections,
        ),
        ("graphics-overlay", "Debug Overlay", g.debug_overlay),
        (
            "graphics-mist",
            "Volumetric Waterfall Mist",
            g.water_volumetric_mist,
        ),
    ] {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, String::new());
        row.kind = "switch".into();
        row.checked = checked;
        rows.push(row);
    }
    rows.push(slider(
        "graphics-waves",
        "Water Wave Strength",
        g.water_wave_strength,
        0.0,
        2.0,
        0.05,
        format!("{:.2}", g.water_wave_strength),
    ));
    rows.push(select(
        "weather-quality",
        "Weather Quality",
        match g.weather_quality {
            terra_core::WeatherQuality::Off => "off",
            terra_core::WeatherQuality::Low => "low",
            terra_core::WeatherQuality::Medium => "medium",
            terra_core::WeatherQuality::High => "high",
            terra_core::WeatherQuality::Ultra => "ultra",
        }
        .into(),
        choices(&[
            ("off", "Off"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
            ("ultra", "Ultra"),
        ]),
    ));
    rows
}

pub(super) fn gameplay_rows(config: &terra_core::GameplayConfig) -> Vec<SettingRow> {
    let mut rows: Vec<_> = [
        ("hud-edit", "HUD Layout", "Edit Layout"),
        ("minimap-settings", "MiniMap", "Customize"),
        ("mob-health-settings", "Mob Health Bars", "Edit Bars"),
    ]
    .into_iter()
    .map(|(id, title, label)| {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, label.into());
        row.kind = "command".into();
        row
    })
    .collect();
    let mut heartbeat = slider(
        "heartbeat",
        "Health Heartbeat",
        0.0,
        0.0,
        0.0,
        1.0,
        String::new(),
    );
    heartbeat.kind = "switch".into();
    heartbeat.checked = config.heartbeat;
    rows.push(heartbeat);
    let mut bobbing = slider(
        "view-bobbing",
        "Walking Camera Motion",
        0.0,
        0.0,
        0.0,
        1.0,
        String::new(),
    );
    bobbing.kind = "switch".into();
    bobbing.checked = config.view_bobbing;
    rows.push(bobbing);
    rows
}

pub(super) fn mob_health_rows(config: &terra_core::GameplayConfig) -> Vec<SettingRow> {
    let mut rows = vec![select(
        "mob-health-bars",
        "Show Bars",
        match config.mob_health_bars {
            terra_core::MobHealthBars::InView => "in_view",
            terra_core::MobHealthBars::LookingAt => "looking_at",
            terra_core::MobHealthBars::Always => "always",
            terra_core::MobHealthBars::None => "none",
        }
        .into(),
        choices(&[
            ("in_view", "View Port"),
            ("looking_at", "Looking At"),
            ("always", "All Time"),
            ("none", "None"),
        ]),
    )];
    for (id, title, rgb) in [
        (
            "mob-health-friendly-color",
            "Friendly",
            config.mob_health_friendly_color,
        ),
        (
            "mob-health-hostile-color",
            "Hostile",
            config.mob_health_hostile_color,
        ),
        (
            "mob-health-font-color",
            "Font",
            config.mob_health_font_color,
        ),
    ] {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, String::new());
        row.kind = "color".into();
        row.value = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        rows.push(row);
    }
    rows
}

pub(super) fn minimap_rows(config: &terra_core::MinimapConfig) -> Vec<SettingRow> {
    let mut rows = vec![
        slider(
            "minimap-size",
            "Map Size",
            f32::from(config.size),
            180.0,
            360.0,
            10.0,
            format!("{} px", config.size),
        ),
        slider(
            "minimap-zoom",
            "Default Zoom",
            f32::from(config.zoom),
            0.0,
            3.0,
            1.0,
            format!("{} m", [96, 192, 384, 768][usize::from(config.zoom.min(3))]),
        ),
        select(
            "minimap-fps",
            "Map FPS",
            config.fps.to_string(),
            choices(&[
                ("24", "24"),
                ("30", "30 (Default)"),
                ("40", "40"),
                ("60", "60"),
            ]),
        ),
        slider(
            "minimap-icon-scale",
            "Animal Icon Scale",
            config.icon_scale,
            1.0,
            2.5,
            0.1,
            format!("{:.1}x", config.icon_scale),
        ),
        select(
            "minimap-mode",
            "Map View",
            match config.mode {
                terra_core::MapMode::Auto => "auto",
                terra_core::MapMode::Surface => "surface",
                terra_core::MapMode::Cave => "cave",
            }
            .into(),
            choices(&[
                ("auto", "Automatic"),
                ("surface", "Surface"),
                ("cave", "Caves"),
            ]),
        ),
    ];
    for (id, title, checked) in [
        ("minimap-enabled", "Show MiniMap", config.enabled),
        ("minimap-rotate", "Rotate with Player", config.rotate),
        ("minimap-relief", "Terrain Relief", config.relief),
        ("minimap-mobs", "Show Animals", config.mobs),
        ("minimap-players", "Show Players", config.players),
        (
            "minimap-coordinates",
            "Show Coordinates",
            config.coordinates,
        ),
        ("minimap-biome", "Show Biome", config.biome),
        ("minimap-clock", "Show Clock", config.clock),
    ] {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, String::new());
        row.kind = "switch".into();
        row.checked = checked;
        if id == "minimap-enabled" {
            rows.insert(0, row);
        } else {
            rows.push(row);
        }
    }
    rows
}
pub(super) fn multiplayer_rows(config: &terra_core::MultiplayerConfig) -> Vec<SettingRow> {
    let mut rows = Vec::new();
    for (id, title, value) in [
        (
            "multiplayer-names",
            "Names Above Players",
            config.player_names,
        ),
        (
            "multiplayer-cursors",
            "Shared Chest Cursors",
            config.chest_cursors,
        ),
        (
            "multiplayer-random-color",
            "Automatic Cursor Color",
            config.cursor_color.is_none(),
        ),
    ] {
        let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, String::new());
        row.kind = "switch".into();
        row.checked = value;
        rows.push(row);
    }
    let color = config.cursor_color.unwrap_or([78, 190, 140]);
    rows.insert(
        1,
        slider(
            "multiplayer-name-distance",
            "Player Name Distance",
            config.player_name_distance,
            8.0,
            150.0,
            2.0,
            format!("{:.0} m", config.player_name_distance),
        ),
    );
    let mut row = slider(
        "multiplayer-color",
        "Cursor Color",
        0.0,
        0.0,
        0.0,
        1.0,
        String::new(),
    );
    row.kind = "color".into();
    row.value = format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]);
    // Choosing a color switches away from automatic coloring directly.
    row.enabled = true;
    rows.push(row);
    rows
}

fn audio_rows(audio: &AudioConfig) -> Vec<SettingRow> {
    let mut rows: Vec<_> = [
        ("audio-master", "Master Volume", audio.master_volume),
        ("audio-ambience", "Ambient Sounds", audio.ambience_volume),
        ("audio-animals", "Animals", audio.animals_volume),
        ("audio-footsteps", "Footsteps", audio.footsteps_volume),
        ("audio-water", "Water & Shore", audio.water_volume),
        ("audio-river", "River Current", audio.river_volume),
        ("audio-waterfall", "Waterfalls", audio.waterfall_volume),
    ]
    .into_iter()
    .map(|(id, title, gain)| {
        slider(
            id,
            title,
            gain * 100.0,
            0.0,
            100.0,
            1.0,
            format!("{:.0}%", gain * 100.0),
        )
    })
    .collect();
    let mut muted = slider(
        "audio-muted",
        "Mute All Sounds",
        0.0,
        0.0,
        0.0,
        1.0,
        String::new(),
    );
    muted.kind = "switch".into();
    muted.checked = audio.muted;
    rows.push(muted);
    rows
}

fn controls_rows(draft: &OptionsDraft) -> Vec<SettingRow> {
    let mut rows: Vec<_> = GameAction::ALL
        .into_iter()
        .enumerate()
        .map(|(index, action)| {
            let mut row = slider(
                &format!("bind-{index}"),
                action.label(),
                0.0,
                0.0,
                0.0,
                1.0,
                String::new(),
            );
            row.kind = "binding".into();
            row.label = if draft.rebinding == Some(action) {
                "Press a key ...".into()
            } else {
                draft.controls.binding(action).label()
            };
            row.enabled = draft.rebinding.is_none();
            row
        })
        .collect();
    let mut auto_climb = slider(
        "controls-auto-climb",
        "Auto-Climb",
        0.0,
        0.0,
        0.0,
        1.0,
        String::new(),
    );
    auto_climb.kind = "switch".into();
    auto_climb.checked = draft.controls.auto_climb;
    rows.insert(0, auto_climb);
    rows.push(slider(
        "controls-sensitivity",
        "Mouse Sensitivity",
        draft.controls.mouse_sensitivity * 1000.0,
        0.1,
        20.0,
        0.1,
        format!("{:.1}", draft.controls.mouse_sensitivity * 1000.0),
    ));
    for (index, title) in ["Interact", "Inventory", "Crouch (Walk)"]
        .into_iter()
        .enumerate()
    {
        let mut row = slider(
            &format!("unavailable-{index}"),
            title,
            0.0,
            0.0,
            0.0,
            1.0,
            "Not available".into(),
        );
        row.kind = "binding".into();
        row.enabled = false;
        rows.push(row);
    }
    rows
}

pub(super) fn update(world: &mut World) {
    if world.resource::<MenuState>().page == MenuPage::Playing
        && !world.resource::<MenuView>().visible
    {
        return;
    }
    if world.resource::<MenuState>().page == MenuPage::Playing
        && !world.resource::<MenuView>().visible
    {
        return;
    }
    let resolutions = resolution_choices(world);
    let viewport = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(Window::size);
    let menu = world.resource::<MenuState>();
    let page = menu.page;
    let main = page == MenuPage::Main;
    let pause = page == MenuPage::Pause;
    let view = MenuView {
        visible: !matches!(page, MenuPage::Playing | MenuPage::HudLayout),
        style: if page == MenuPage::Loading {
            format!(
                "opacity: {:.4};",
                1.0 - world.resource::<LoadingProgress>().reveal
            )
        } else {
            String::new()
        },
        main,
        worlds: page == MenuPage::Worlds,
        servers: page == MenuPage::Servers,
        options: page == MenuPage::Options,
        pause,
        loading: page == MenuPage::Loading,
        title: match page {
            MenuPage::Main => "",
            MenuPage::Worlds => "Singleplayer",
            MenuPage::Servers => "Multiplayer",
            MenuPage::Options => "Options",
            MenuPage::Pause => "Paused",
            MenuPage::Loading => "Your next adventure",
            MenuPage::Playing | MenuPage::HudLayout => "",
        }
        .into(),
        error: menu.error.clone(),
        status: menu.status.clone(),
        version: format!("Early Access / {}", env!("CARGO_PKG_VERSION")),
        cancel_connect: !menu.quitting,
        in_game: menu.session_active && page != MenuPage::Loading,
        buttons: if main {
            vec![
                button("singleplayer", "Singleplayer", "", "compass", true, true),
                button("multiplayer", "Multiplayer", "", "users-round", true, false),
                button("options", "Options", "", "settings", true, false),
                button("quit", "Quit Game", "", "log-out", true, false),
            ]
        } else if pause {
            let info = world.resource::<SessionInfo>();
            vec![
                button("resume", "Resume Game", "", "play", true, true),
                button("options", "Options", "", "settings", true, false),
                button(
                    "save-world",
                    if info.saving {
                        "Saving ..."
                    } else {
                        "Save World"
                    },
                    "",
                    "save",
                    info.local && !info.saving,
                    false,
                ),
                button(
                    "leave",
                    if info.local {
                        "Save & Main Menu"
                    } else {
                        "Disconnect & Main Menu"
                    },
                    "",
                    "home",
                    true,
                    false,
                ),
                button("quit", "Quit Game", "", "log-out", true, false),
            ]
        } else {
            Vec::new()
        },
    };
    let worlds = world.resource::<WorldMenu>();
    let selected = worlds
        .entries
        .iter()
        .find(|e| Some(e.manifest.world.id) == worlds.selected);
    let worlds_view = WorldsView {
        empty: worlds.entries.is_empty(),
        count: format!("Worlds ({})", worlds.entries.len()),
        cards: worlds
            .entries
            .iter()
            .map(|entry| CardView {
                id: format!("world-{:016x}", entry.manifest.world.id.0),
                title: entry.title.clone(),
                subtitle: format!("v{}  /  {}", entry.game_version, entry.mode.label()),
                detail: entry.compatible().err().unwrap_or_else(|| {
                    if entry.last_played.is_some() {
                        "Previously played".into()
                    } else {
                        "New world".into()
                    }
                }),
                selected: Some(entry.manifest.world.id) == worlds.selected,
                compatible: entry.compatible().is_ok(),
                icon: if entry.mode == terra_storage::GameMode::Creative {
                    "hammer"
                } else {
                    "mountain"
                }
                .into(),
            })
            .collect(),
        buttons: vec![
            button(
                "world-play",
                "Play World",
                "",
                "play",
                selected.is_some_and(|e| e.compatible().is_ok()),
                true,
            ),
            button("world-create", "Create World", "", "plus", true, false),
            button(
                "world-delete",
                "Delete World",
                "",
                "trash-2",
                selected.is_some(),
                false,
            ),
            button("back", "Back", "", "arrow-left", true, false),
        ],
    };
    let servers = world.resource::<ServerMenu>();
    let selected_server = servers
        .list
        .servers
        .iter()
        .find(|s| Some(s.id) == servers.selected);
    let browser = world.resource::<super::browser::ServerBrowser>();
    let mut server_view = ServersView {
        empty: servers.list.servers.is_empty() && browser.lan.is_empty(),
        count: format!("Servers ({})", servers.list.servers.len()),
        cards: servers
            .list
            .servers
            .iter()
            .map(|server| CardView {
                id: format!("server-{}", server.id),
                title: server.name.clone(),
                subtitle: format!(
                    "{}:{} / {}",
                    server.address,
                    server.port,
                    browser
                        .records
                        .get(&server.id)
                        .and_then(|record| record.status.as_ref())
                        .map_or("", |status| status.description.as_str())
                ),
                detail: browser.records.get(&server.id).map_or_else(
                    || "Checking server ...".into(),
                    |record| {
                        record.status.as_ref().map_or_else(
                            || format!("Offline / {}", record.error),
                            |status| {
                                format!(
                                    "{} / Players: {}/{} / Ping: {} ms / Local profiles",
                                    if status.compatible() {
                                        "Online"
                                    } else {
                                        "Incompatible protocol"
                                    },
                                    status.players,
                                    status.max_players,
                                    record.ping_ms
                                )
                            },
                        )
                    },
                ),
                selected: Some(server.id) == servers.selected,
                compatible: browser
                    .records
                    .get(&server.id)
                    .and_then(|record| record.status.as_ref())
                    .is_none_or(|status| status.compatible()),
                icon: "users".into(),
            })
            .collect(),
        buttons: vec![
            button(
                "server-join",
                "Join Server",
                "",
                "play",
                selected_server.is_some_and(|server| {
                    browser
                        .records
                        .get(&server.id)
                        .and_then(|record| record.status.as_ref())
                        .is_none_or(|status| status.compatible())
                }) || servers.lan_selected.is_some_and(|address| {
                    browser
                        .lan
                        .iter()
                        .any(|server| server.address == address && server.status.compatible())
                }),
                true,
            ),
            button("server-add", "Add Server", "", "plus", true, false),
            button(
                "server-edit",
                "Edit Server",
                "",
                "settings",
                selected_server.is_some(),
                false,
            ),
            button("back", "Back", "", "arrow-left", true, false),
        ],
        discovery: if browser.error.is_empty() {
            "Searching LAN".into()
        } else {
            format!("LAN unavailable: {}", browser.error)
        },
    };
    for server in &browser.lan {
        if browser.records.values().any(|record| {
            record
                .status
                .as_ref()
                .is_some_and(|status| status.instance == server.status.instance)
        }) {
            continue;
        }
        server_view.cards.push(CardView {
            id: format!("lan-{}", server.address),
            title: server.status.title.clone(),
            subtitle: format!("{} / {}", server.address, server.status.description),
            detail: format!(
                "LAN / {} / Players: {}/{} / Ping: {} ms / Local profiles",
                if server.status.compatible() {
                    "Online"
                } else {
                    "Incompatible protocol"
                },
                server.status.players,
                server.status.max_players,
                server.ping_ms
            ),
            selected: servers.lan_selected == Some(server.address),
            compatible: server.status.compatible(),
            icon: "users".into(),
        });
    }
    server_view.count = format!("Servers ({})", server_view.cards.len());
    let draft = world.resource::<OptionsDraft>();
    let listening = draft.rebinding.is_some();
    let mut rows = match draft.tab {
        OptionsTab::Graphics => graphics_rows(&draft.graphics, resolutions, viewport),
        OptionsTab::Gameplay => gameplay_rows(&draft.gameplay),
        OptionsTab::Controls => controls_rows(draft),
        OptionsTab::Audio => audio_rows(&draft.audio),
        OptionsTab::Multiplayer => multiplayer_rows(&draft.multiplayer),
    };
    if draft.tab == OptionsTab::Graphics && menu.session_active {
        let enabled = world
            .get_resource::<terra_network::client::ClientSession>()
            .is_some_and(|session| session.ready && session.allow_environment)
            && world
                .get_resource::<terra_player::TargetBlock>()
                .is_some_and(|target| target.0.is_some());
        for (id, title, label) in [
            ("surface-wet", "Target Surface", "Wet / Extinguish"),
            ("surface-snow", "Snow Cover", "Cover"),
            ("surface-fire", "Ignition", "Ignite"),
            ("surface-clear", "Surface State", "Reset"),
        ] {
            let mut row = slider(id, title, 0.0, 0.0, 0.0, 1.0, label.into());
            row.kind = "command".into();
            row.enabled = enabled;
            rows.push(row);
        }
    }
    let options = OptionsView {
        graphics: draft.tab == OptionsTab::Graphics,
        gameplay: draft.tab == OptionsTab::Gameplay,
        controls: draft.tab == OptionsTab::Controls,
        audio: draft.tab == OptionsTab::Audio,
        multiplayer: draft.tab == OptionsTab::Multiplayer,
        dirty: super::options::dirty(world),
        rows,
        listening,
        prompt: draft
            .rebinding
            .map(|action| {
                format!(
                    "{}: press a key or mouse button. Escape cancels.",
                    action.label()
                )
            })
            .unwrap_or_default(),
        buttons: vec![
            button(
                "apply",
                "Apply",
                "",
                "check",
                !listening && super::options::dirty(world),
                true,
            ),
            button("reset", "Reset", "", "rotate-ccw", !listening, false),
            button("back", "Back", "", "arrow-left", !listening, false),
        ],
    };
    let dialog = &menu.dialog;
    let (title, message, buttons) = match dialog {
        MenuDialog::CreateWorld => (
            "Create World".into(),
            String::new(),
            vec![
                button(
                    "world-create-confirm",
                    "Create",
                    "",
                    "plus",
                    !worlds.name.trim().is_empty(),
                    true,
                ),
                button("dialog-cancel", "Cancel", "", "arrow-left", true, false),
            ],
        ),
        MenuDialog::DeleteWorld => (
            "Delete World?".into(),
            format!(
                "Permanently delete \"{}\" and all its saved changes?",
                selected.map_or("", |e| e.title.as_str())
            ),
            vec![
                button(
                    "world-delete-confirm",
                    "Delete World",
                    "",
                    "trash-2",
                    selected.is_some(),
                    false,
                ),
                button("dialog-cancel", "Cancel", "", "arrow-left", true, true),
            ],
        ),
        MenuDialog::EditServer => (
            if servers.editing.is_some() {
                "Edit Server"
            } else {
                "Add Server"
            }
            .into(),
            String::new(),
            vec![
                button(
                    "server-save",
                    if servers.editing.is_some() {
                        "Save"
                    } else {
                        "Add"
                    },
                    "",
                    "check",
                    !servers.name.trim().is_empty() && !servers.address.trim().is_empty(),
                    true,
                ),
                button("dialog-cancel", "Cancel", "", "arrow-left", true, false),
            ],
        ),
        MenuDialog::Discard => (
            "Discard changes?".into(),
            "Your unapplied settings will be discarded.".into(),
            vec![
                button("discard-confirm", "Discard", "", "rotate-ccw", true, false),
                button(
                    "dialog-cancel",
                    "Keep Editing",
                    "",
                    "arrow-left",
                    true,
                    true,
                ),
            ],
        ),
        MenuDialog::Display => (
            "Keep display settings?".into(),
            format!(
                "Automatically reverting in {} seconds.",
                draft.display.as_ref().map_or(0, |p| p
                    .until
                    .saturating_duration_since(std::time::Instant::now())
                    .as_secs()
                    + 1)
            ),
            vec![
                button("display-keep", "Keep Settings", "", "check", true, true),
                button("display-revert", "Revert", "", "rotate-ccw", true, false),
            ],
        ),
        MenuDialog::Minimap => (
            "MiniMap".into(),
            String::new(),
            vec![
                button("minimap-reset", "Reset", "", "rotate-ccw", true, false),
                button("dialog-cancel", "Done", "", "check", true, true),
            ],
        ),
        MenuDialog::MobHealth => (
            "Mob Health Bars".into(),
            String::new(),
            vec![
                button("mob-health-reset", "Reset", "", "rotate-ccw", true, false),
                button("dialog-cancel", "Done", "", "check", true, true),
            ],
        ),
        MenuDialog::None => (String::new(), String::new(), Vec::new()),
    };
    let dialog_view = DialogView {
        visible: *dialog != MenuDialog::None,
        create: *dialog == MenuDialog::CreateWorld,
        natural_mob_limit: worlds.natural_mob_limit,
        server: *dialog == MenuDialog::EditServer,
        minimap: *dialog == MenuDialog::Minimap,
        mob_health: *dialog == MenuDialog::MobHealth,
        rows: if *dialog == MenuDialog::Minimap {
            minimap_rows(&draft.gameplay.minimap)
        } else if *dialog == MenuDialog::MobHealth {
            mob_health_rows(&draft.gameplay)
        } else {
            Vec::new()
        },
        title,
        message,
        name: if *dialog == MenuDialog::CreateWorld {
            worlds.name.clone()
        } else {
            servers.name.clone()
        },
        seed: worlds.seed.clone(),
        mode: worlds.mode.clone(),
        address: servers.address.clone(),
        port: servers.port.clone(),
        error: menu.error.clone(),
        buttons,
    };
    let info = world.resource::<SessionInfo>();
    let progress = world.resource::<LoadingProgress>();
    let profile = world.resource::<terra_core::PlayerProfile>();
    let minutes = info.started.map_or(0, |time| time.elapsed().as_secs() / 60);
    let session_view = SessionView {
        title: if info.title.is_empty() {
            "TERAMIC".into()
        } else {
            info.title.clone()
        },
        mode: world
            .get_resource::<terra_network::client::ClientSession>()
            .filter(|s| s.ready)
            .map_or_else(|| info.mode.clone(), |s| s.game_mode.label().into()),
        seed: world
            .resource::<terra_core::GameConfig>()
            .world
            .seed
            .0
            .to_string(),
        player: if menu.profile_loaded {
            profile.name.clone()
        } else {
            "Profile unavailable".into()
        },
        uuid: if menu.profile_loaded {
            profile.uuid.to_string()
        } else {
            String::new()
        },
        elapsed: format!("{:02}:{:02}", minutes / 60, minutes % 60),
        progress: progress.fraction.unwrap_or(0.0).clamp(0.0, 1.0) * 100.0,
        show_progress: progress.fraction.is_some() && !menu.quitting && menu.error.is_empty(),
        progress_label: progress.detail.clone(),
        detail: progress.detail.clone(),
    };
    set(world, session_view);
    set(world, view);
    set(world, worlds_view);
    set(world, server_view);
    set(world, options);
    set(world, dialog_view);
}
fn set<T: Resource + PartialEq>(world: &mut World, value: T) {
    if *world.resource::<T>() != value {
        world.insert_resource(value);
    }
}
