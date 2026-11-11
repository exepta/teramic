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
    pub startup: bool,
    pub startup_fading: bool,
    pub content_style: String,
    pub startup_style: String,
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
    pub version: String,
    pub mode: String,
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
    pub icon: String,
    pub hint: String,
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
#[derive(Clone, Serialize, PartialEq)]
pub struct WorldToggle {
    pub id: String,
    pub title: String,
    pub hint: String,
    pub icon: String,
    pub checked: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct WorldRuleCard {
    pub name: String,
    pub id: String,
    pub title: String,
    pub hint: String,
    pub icon: String,
    pub kind: String,
    pub checked: bool,
    pub value: String,
    pub min: String,
    pub max: String,
    pub step: String,
}

pub(super) fn world_rule_cards(form: &WorldMenu) -> Vec<WorldRuleCard> {
    use terra_core::GameRule;
    let mut cards: Vec<_> = GameRule::ALL
        .into_iter()
        .map(|rule| {
            let (id, title, icon, kind, value, min, max, step) = match rule {
                GameRule::SleepPercentage => (
                    "world-sleep-percentage",
                    "Sleep Percentage",
                    "users-round",
                    "number",
                    form.game_rules.sleep_percentage,
                    "0",
                    "100",
                    "1",
                ),
                GameRule::GameTimeSpeed => (
                    "world-time-speed",
                    "Game Time Speed",
                    "clock",
                    "number",
                    form.game_rules.game_time_speed,
                    "0",
                    "1000",
                    "0.0001",
                ),
                GameRule::SeasonSwapInterval => (
                    "world-season-days",
                    "Days per Season",
                    "calendar",
                    "number",
                    f64::from(form.season_days),
                    "1",
                    "3650",
                    "1",
                ),
                GameRule::KeepInventory => (
                    "world-rule-keep-inventory",
                    "Keep Inventory",
                    "backpack",
                    "switch",
                    f64::from(u8::from(form.game_rules.keep_inventory)),
                    "",
                    "",
                    "",
                ),
                GameRule::DaylightCycle => (
                    "world-rule-daylight-cycle",
                    "Daylight Cycle",
                    "sun",
                    "switch",
                    f64::from(u8::from(form.game_rules.daylight_cycle)),
                    "",
                    "",
                    "",
                ),
                GameRule::WeatherCycle => (
                    "world-rule-weather-cycle",
                    "Weather Cycle",
                    "cloud-rain",
                    "switch",
                    f64::from(u8::from(form.game_rules.weather_cycle)),
                    "",
                    "",
                    "",
                ),
                GameRule::MobGriefing => (
                    "world-rule-mob-griefing",
                    "Mob Griefing",
                    "paw-print",
                    "switch",
                    f64::from(u8::from(form.game_rules.mob_griefing)),
                    "",
                    "",
                    "",
                ),
                GameRule::FireSpread => (
                    "world-rule-fire-spread",
                    "Fire Spread",
                    "flame",
                    "switch",
                    f64::from(u8::from(form.game_rules.fire_spread)),
                    "",
                    "",
                    "",
                ),
                GameRule::Pvp => (
                    "world-rule-pvp",
                    "PvP",
                    "swords",
                    "switch",
                    f64::from(u8::from(form.game_rules.pvp)),
                    "",
                    "",
                    "",
                ),
            };
            let hint = rule.description().replace(" (0 off, 1 on)", "");
            WorldRuleCard {
                name: rule.name().into(),
                id: id.into(),
                title: title.into(),
                hint,
                icon: icon.into(),
                kind: kind.into(),
                checked: value == 1.0,
                value: value.to_string(),
                min: min.into(),
                max: max.into(),
                step: step.into(),
            }
        })
        .collect();
    // Keep the common switches together while deriving every card from the registry.
    cards.sort_by_key(|card| card.kind != "switch");
    cards
}

#[derive(Clone, Default, Serialize, PartialEq)]
pub struct WorldBiomeChoice {
    pub command: String,
    pub title: String,
    pub icon: String,
    pub image: String,
    pub class_name: String,
}

pub(super) fn world_biome_choices(worlds: &WorldMenu) -> Vec<WorldBiomeChoice> {
    super::create_world::BIOMES
        .iter()
        .map(|(kind, key, title, icon)| {
            let image = match *key {
                "random" | "plains" => "plains",
                key => key,
            };
            WorldBiomeChoice {
                command: format!("world-biome-{key}"),
                title: (*title).into(),
                icon: (*icon).into(),
                image: format!("tilt-ui://assets/biome-choice-{image}.png"),
                class_name: format!(
                    "world-biome-tile biome-{key}{}",
                    if worlds.preferred_spawn_biome == *kind {
                        " selected"
                    } else {
                        ""
                    }
                ),
            }
        })
        .collect()
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct DialogView {
    pub visible: bool,
    pub create: bool,
    pub create_tab: String,
    pub kingdom_rules: Vec<WorldRuleCard>,
    pub world_rules: Vec<WorldRuleCard>,
    pub structure_choices: Vec<WorldToggle>,
    pub biome_choices: Vec<WorldBiomeChoice>,
    pub selected_biome: WorldBiomeChoice,
    pub biome_picker_open: bool,
    pub structures_enabled: bool,
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
        icon: setting_decoration(id).0.into(),
        hint: setting_decoration(id).1.into(),
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
        icon: setting_decoration(id).0.into(),
        hint: setting_decoration(id).1.into(),
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

fn setting_decoration(id: &str) -> (&'static str, &'static str) {
    match id {
        "graphics-distance" => ("eye", "How far the world is visible."),
        "graphics-shadow" => ("sun-dim", "Choose the detail of dynamic shadows."),
        "graphics-water" => ("droplet", "Visual quality of water and waves."),
        "graphics-window" => ("app-window", "Choose windowed or fullscreen mode."),
        "graphics-resolution" => ("monitor", "Set the display resolution."),
        "graphics-vsync" => ("refresh", "Synchronize frames with your display."),
        "graphics-brightness" => ("sun", "Adjust the overall game brightness."),
        "graphics-scale" => ("maximize", "Change the size of the game interface."),
        "graphics-block-detail" => ("archive", "Choose the detail of block geometry."),
        "graphics-leaf-detail" => ("leaf", "Choose the detail of leaf geometry."),
        "graphics-fov" => ("scan", "Adjust the camera field of view."),
        "graphics-reflections" => ("scan-line", "Reflect the surrounding world on water."),
        "graphics-fps" => ("timer", "Limit world frames per second."),
        "graphics-ui-fps" => ("clock", "Limit interface updates per second."),
        "graphics-caves" => ("mountain", "Choose cave geometry detail."),
        "graphics-chunk-animation" => ("move", "Choose how newly loaded terrain appears."),
        "graphics-tree-fall" => ("trees", "Choose the falling-tree animation."),
        "graphics-fog" => ("cloud-fog", "Choose the distance of atmospheric fog."),
        "graphics-lod" => ("layers", "Choose where distant geometry becomes simpler."),
        "graphics-material-detail" => ("brush", "Choose the detail of painted materials."),
        "graphics-micro" => ("flower", "Show small plants and surface details."),
        "graphics-micro-distance" => ("ruler", "Choose the visibility range of small details."),
        "graphics-mist" => ("cloud-drizzle", "Show mist around waterfalls."),
        "graphics-normal-detail" => ("grid", "Choose surface normal detail."),
        "graphics-overlay" => ("chart-line", "Show world and performance diagnostics."),
        "graphics-surface-variation" => ("palette", "Vary the appearance of natural surfaces."),
        "graphics-waves" => ("waves-horizontal", "Adjust the strength of water waves."),
        "weather-quality" => ("cloud-rain", "Choose rain, snow and weather effects."),
        "hud-edit" => ("layout-template", "Move the heads-up display elements."),
        "minimap-settings" => ("map", "Choose map display and marker options."),
        "mob-health-settings" => ("heart-pulse", "Choose the display of creature health."),
        "heartbeat" => ("heart", "Play heartbeat feedback at low health."),
        "view-bobbing" => ("route", "Move the camera gently while walking."),
        "multiplayer-names" => ("tag", "Show player names in the world."),
        "multiplayer-name-distance" => ("ruler", "Choose how far player names remain visible."),
        "multiplayer-cursors" => ("archive", "Show other players using the same chest."),
        "multiplayer-random-color" => ("dices", "Assign a cursor color automatically."),
        "multiplayer-color" => ("palette", "Choose your shared-container cursor color."),
        "audio-master" => ("volume", "Adjust the overall sound volume."),
        "audio-ambience" => ("music", "Adjust environmental background sounds."),
        "audio-animals" => ("paw", "Adjust animal sounds."),
        "audio-footsteps" => ("route", "Adjust footstep sounds."),
        "audio-water" => ("droplets", "Adjust water and shore sounds."),
        "audio-river" => ("route", "Adjust flowing river sounds."),
        "audio-waterfall" => ("waves-ladder", "Adjust waterfall sounds."),
        "audio-muted" => ("volume-x", "Mute all game audio."),
        "minimap-size" => ("maximize", "Choose the map size."),
        "minimap-zoom" => ("search", "Choose the map zoom."),
        "minimap-fps" => ("timer", "Choose the map update frequency."),
        "minimap-icon-scale" => ("flag", "Choose the marker size."),
        "minimap-mode" => ("layers", "Choose the map layer."),
        "minimap-enabled" => ("map", "Show the minimap."),
        "minimap-rotate" => ("rotate-ccw", "Rotate the map with your facing direction."),
        "minimap-relief" => ("mountain", "Show terrain height shading."),
        "minimap-mobs" => ("paw", "Show creature markers."),
        "minimap-players" => ("users-round", "Show player markers."),
        "minimap-coordinates" => ("map-pin", "Show world coordinates."),
        "minimap-biome" => ("trees", "Show the current biome."),
        "minimap-clock" => ("clock", "Show the world time."),
        "mob-health-bars" => ("heart-pulse", "Choose when creature health is visible."),
        "mob-health-friendly-color" => {
            ("heart-handshake", "Choose friendly creature health color.")
        }
        "mob-health-hostile-color" => ("swords", "Choose hostile creature health color."),
        "mob-health-font-color" => ("book-text", "Choose health label text color."),
        "surface-wet" => ("droplets", "Wet the targeted surface."),
        "surface-snow" => ("snowflake", "Cover the targeted surface with snow."),
        "surface-fire" => ("flame", "Ignite the targeted surface."),
        "surface-clear" => ("eraser", "Clear the targeted surface state."),
        _ if id.starts_with("bind-") => ("keyboard", "Click to choose a key or mouse button."),
        _ => ("settings-adjust", ""),
    }
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
        resolution.value = "desktop".into();
        resolution.custom = false;
        resolution.choices.insert(
            0,
            Choice {
                value: "desktop".into(),
                label: "Desktop (Borderless)".into(),
            },
        );
        resolution.hint = "Choosing a resolution switches to Windowed.".into();
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
    let startup = world
        .get_resource::<StartupProgress>()
        .filter(|progress| progress.active);
    // The startup gate is a full loading page, not a badge over clickable menus.
    let page = if startup.is_some_and(|progress| !progress.fading) {
        MenuPage::Loading
    } else {
        menu.page
    };
    let main = page == MenuPage::Main;
    let pause = page == MenuPage::Pause;
    let view = MenuView {
        visible: !matches!(page, MenuPage::Playing | MenuPage::HudLayout),
        style: if page == MenuPage::Loading && startup.is_none() {
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
        loading: page == MenuPage::Loading || startup.is_some(),
        startup: startup.is_some(),
        startup_fading: startup.is_some_and(|progress| progress.fading),
        content_style: startup.map_or_else(String::new, |progress| {
            format!("opacity: {:.4}; pointer-events: none;", progress.blend())
        }),
        startup_style: startup.map_or_else(String::new, |progress| {
            format!(
                "opacity: {:.4}; pointer-events: none;",
                1.0 - progress.blend()
            )
        }),
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
        error: startup
            .filter(|startup| !startup.error.is_empty())
            .map_or_else(
                || menu.error.clone(),
                |startup| {
                    if menu.error.is_empty() {
                        startup.error.clone()
                    } else {
                        format!("{}\n{}", menu.error, startup.error)
                    }
                },
            ),
        status: if startup.is_some_and(|startup| !startup.error.is_empty()) {
            "Asset loading failed. Please restart the game.".into()
        } else if startup.is_some() {
            String::new()
        } else {
            menu.status.clone()
        },
        version: format!("v{} (Early Access)", env!("CARGO_PKG_VERSION")),
        cancel_connect: !menu.quitting && startup.is_none(),
        in_game: menu.session_active && page != MenuPage::Loading,
        buttons: if main {
            vec![
                button("singleplayer", "Singleplayer", "", "leaf", true, true),
                button("multiplayer", "Multiplayer", "", "users-round", true, false),
                button("options", "Settings", "", "settings", true, false),
                button("quit", "Quit", "", "power", true, false),
            ]
        } else if pause {
            let info = world.resource::<SessionInfo>();
            vec![
                button("resume", "Resume Game", "", "play", true, true),
                button("options", "Settings", "", "settings", true, false),
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
                        "Main Menu"
                    } else {
                        "Disconnect"
                    },
                    "",
                    "home",
                    true,
                    false,
                ),
                button("quit", "Quit", "", "power", true, false),
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
                version: format!("v{} (Early Access)", entry.game_version),
                mode: entry.mode.label().into(),
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
                icon: match entry.mode {
                    terra_storage::GameMode::Creative => "hammer",
                    terra_storage::GameMode::Survival => "mountain",
                    terra_storage::GameMode::Spectator => "eye",
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
                version: String::new(),
                mode: String::new(),
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
                "server-character",
                "Charakter",
                "",
                "user",
                selected_server.is_some() || servers.lan_selected.is_some(),
                false,
            ),
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
            version: String::new(),
            mode: String::new(),
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
    if draft.tab == OptionsTab::Graphics {
        let order = [
            "graphics-distance",
            "graphics-shadow",
            "graphics-water",
            "graphics-window",
            "graphics-vsync",
            "graphics-brightness",
            "graphics-leaf-detail",
            "graphics-scale",
        ];
        rows.sort_by_key(|row| {
            order
                .iter()
                .position(|id| *id == row.id)
                .unwrap_or(order.len())
        });
    }
    for row in &mut rows {
        let (icon, hint) = setting_decoration(&row.id);
        row.icon = icon.into();
        if !(row.id == "graphics-resolution"
            && draft.graphics.window_mode == WindowModeSetting::Borderless)
        {
            row.hint = hint.into();
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
        MenuDialog::None | MenuDialog::Character => (String::new(), String::new(), Vec::new()),
    };
    let biome_choices = if *dialog == MenuDialog::CreateWorld {
        world_biome_choices(worlds)
    } else {
        Vec::new()
    };
    let selected_biome = biome_choices
        .iter()
        .find(|biome| biome.class_name.ends_with(" selected"))
        .cloned()
        .unwrap_or_default();
    let dialog_view = DialogView {
        visible: !matches!(dialog, MenuDialog::None | MenuDialog::Character),
        create: *dialog == MenuDialog::CreateWorld,
        create_tab: worlds.create_tab.key().into(),
        kingdom_rules: if *dialog == MenuDialog::CreateWorld {
            super::kingdom_settings::cards(worlds.kingdom_rules)
        } else {
            Vec::new()
        },
        world_rules: if *dialog == MenuDialog::CreateWorld {
            world_rule_cards(worlds)
        } else {
            Vec::new()
        },
        structure_choices: if *dialog == MenuDialog::CreateWorld {
            [
                ("villages", "Villages", "home", worlds.structures.villages),
                ("kingdoms", "Kingdoms", "crown", worlds.structures.kingdoms),
                ("outposts", "Outposts", "flag", worlds.structures.outposts),
                ("ruins", "Ruins", "landmark", worlds.structures.ruins),
                ("mines", "Mines", "tool", worlds.structures.mines),
            ]
            .map(|(id, title, icon, checked)| WorldToggle {
                id: format!("world-structure-{id}"),
                title: title.into(),
                hint: String::new(),
                icon: icon.into(),
                checked,
            })
            .to_vec()
        } else {
            Vec::new()
        },
        biome_choices,
        selected_biome,
        biome_picker_open: *dialog == MenuDialog::CreateWorld && worlds.biome_picker_open,
        structures_enabled: worlds.structures.any_enabled(),
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
        title: if startup.is_some() {
            "Laden".into()
        } else if info.title.is_empty() {
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
        progress: startup
            .map_or(progress.fraction, |startup| startup.fraction)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
            * 100.0,
        show_progress: (startup.is_some()
            || startup
                .map_or(progress.fraction, |startup| startup.fraction)
                .is_some())
            && !menu.quitting
            && menu.error.is_empty()
            && startup.is_none_or(|startup| startup.error.is_empty()),
        progress_label: startup.map_or_else(
            || {
                progress
                    .fraction
                    .map(|fraction| format!("{:.0}%", fraction.clamp(0.0, 1.0) * 100.0))
                    .unwrap_or_default()
            },
            |startup| startup.label.clone(),
        ),
        detail: startup
            .map_or(&progress.detail, |startup| &startup.detail)
            .clone(),
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

#[cfg(test)]
mod display_rows_tests {
    use super::*;

    #[test]
    fn unlimited_fps_and_borderless_resolution_have_selectable_semantic_options() {
        let mut graphics = GraphicsConfig {
            fps_limit: 0,
            ..Default::default()
        };
        let resolutions = vec![Choice {
            value: "1600x900".into(),
            label: "1600 x 900".into(),
        }];
        let rows = graphics_rows(&graphics, resolutions.clone(), None);
        let fps = rows.iter().find(|row| row.id == "graphics-fps").unwrap();
        assert!(!fps.custom);
        assert_eq!(fps.value, "0");
        assert_eq!(
            fps.choices
                .iter()
                .filter(|choice| choice.value == "0")
                .count(),
            1
        );
        assert_eq!(
            fps.choices
                .iter()
                .find(|choice| choice.value == "0")
                .unwrap()
                .label,
            "Unlimited"
        );

        graphics.fps_limit = 75;
        graphics.window_mode = WindowModeSetting::Borderless;
        let rows = graphics_rows(&graphics, resolutions, None);
        assert!(
            rows.iter()
                .find(|row| row.id == "graphics-fps")
                .unwrap()
                .custom
        );
        let resolution = rows
            .iter()
            .find(|row| row.id == "graphics-resolution")
            .unwrap();
        assert!(resolution.enabled);
        assert!(!resolution.custom);
        assert_eq!(resolution.value, "desktop");
        assert!(
            resolution
                .choices
                .iter()
                .any(|choice| choice.value == "desktop")
        );
        assert!(
            resolution
                .choices
                .iter()
                .any(|choice| choice.value == "1600x900")
        );
    }
}
