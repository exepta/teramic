use super::BlockDetailConfig;
use super::{
    ChunkAnimation, ClientConfig, Config, ConfigError, WaterQuality, WeatherKind, WeatherQuality,
    ensure,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowModeSetting {
    #[default]
    Windowed,
    Borderless,
    Fullscreen,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    #[default]
    High,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveQuality {
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct GraphicsConfig {
    pub blocks: BlockDetailConfig,
    pub width: u32,
    pub height: u32,
    pub window_mode: WindowModeSetting,
    pub vsync: bool,
    pub fps_limit: u16,
    pub render_distance: u8,
    /// Surface haze distance relative to the streamed radius; zero disables surface fog.
    pub fog_distance: f32,
    pub chunk_animation: ChunkAnimation,
    pub lod_distance: f32,
    pub shadow_quality: ShadowQuality,
    pub cave_quality: CaveQuality,
    pub water_quality: WaterQuality,
    /// Legacy input only. Logical world props cannot be hidden through graphics settings.
    #[serde(skip_serializing)]
    pub vegetation_density: f32,
    pub brightness: f32,
    pub ui_scale: f32,
    pub ui_fps: u16,
    /// Relative position within the usable HUD area, independent of DPI/resolution.
    pub hud_position: [f32; 2],
    pub chat_position: Option<[f32; 2]>,
    pub target_position: Option<[f32; 2]>,
    pub minimap_position: Option<[f32; 2]>,
    pub fov: f32,
    pub water_reflections: bool,
    pub water_volumetric_mist: bool,
    pub water_wave_strength: f32,
    pub weather: WeatherKind,
    pub weather_intensity: f32,
    pub weather_quality: WeatherQuality,
    pub debug_overlay: bool,
}
impl Default for GraphicsConfig {
    fn default() -> Self {
        Self::from_client(&ClientConfig::default())
    }
}
impl GraphicsConfig {
    pub fn from_client(client: &ClientConfig) -> Self {
        Self {
            blocks: client.blocks,
            width: client.width.max(640),
            height: client.height.max(480),
            window_mode: WindowModeSetting::Windowed,
            vsync: client.vsync,
            fps_limit: 0,
            render_distance: client.view_distance.clamp(2, 32),
            fog_distance: client.fog_distance,
            chunk_animation: client.chunk_animation,
            lod_distance: (client.lod_near_distance / 96.0).clamp(0.75, 2.0),
            shadow_quality: ShadowQuality::High,
            cave_quality: CaveQuality::default(),
            water_quality: client.water_quality,
            vegetation_density: 1.0,
            brightness: 1.0,
            ui_scale: 1.0,
            ui_fps: 60,
            hud_position: [0.0; 2],
            chat_position: None,
            target_position: None,
            minimap_position: None,
            fov: client.fov_degrees,
            water_reflections: client.water_reflections,
            water_volumetric_mist: client.water_volumetric_mist,
            water_wave_strength: client.water_wave_strength,
            weather: client.weather,
            weather_intensity: client.weather_intensity,
            weather_quality: client.weather_quality,
            debug_overlay: client.debug_overlay,
        }
    }
    pub fn apply_to(&self, client: &mut ClientConfig) {
        client.blocks = self.blocks;
        client.width = self.width;
        client.height = self.height;
        client.vsync = self.vsync;
        client.view_distance = self.render_distance;
        client.fog_distance = self.fog_distance;
        client.chunk_animation = self.chunk_animation;
        client.lod_near_distance = 96.0 * self.lod_distance;
        client.lod_far_distance = 160.0 * self.lod_distance;
        client.lod_hysteresis = client
            .lod_hysteresis
            .min((client.lod_far_distance - client.lod_near_distance) * 0.49);
        client.water_quality = self.water_quality;
        client.vegetation_density = 1.0;
        client.fov_degrees = self.fov;
        client.water_reflections = self.water_reflections;
        client.water_volumetric_mist = self.water_volumetric_mist;
        client.water_wave_strength = self.water_wave_strength;
        client.weather = self.weather;
        client.weather_intensity = self.weather_intensity;
        client.weather_quality = self.weather_quality;
        client.debug_overlay = self.debug_overlay;
    }
    pub fn display_changed(&self, other: &Self) -> bool {
        self.width != other.width
            || self.height != other.height
            || self.window_mode != other.window_mode
    }
}
impl Config for GraphicsConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(self.blocks.valid(), "Invalid block detail settings")?;
        ensure(
            [
                self.chat_position,
                self.target_position,
                self.minimap_position,
            ]
            .into_iter()
            .flatten()
            .flatten()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v)),
            "Chat/target/map HUD positions must be finite and within 0..1",
        )?;
        ensure(
            self.hud_position
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "HUD position must be finite and within 0..1",
        )?;
        ensure(
            self.water_wave_strength.is_finite() && (0.0..=2.0).contains(&self.water_wave_strength),
            "Water wave strength must be 0..2",
        )?;
        ensure(
            self.weather_intensity.is_finite() && (0.0..=1.0).contains(&self.weather_intensity),
            "Weather intensity must be 0..1",
        )?;
        ensure(
            (640..=7680).contains(&self.width) && (480..=4320).contains(&self.height),
            "Resolution must be 640x480 through 7680x4320",
        )?;
        ensure(
            self.fps_limit == 0 || (30..=360).contains(&self.fps_limit),
            "FPS limit must be 0 (unlimited) or 30..360",
        )?;
        ensure(
            (2..=32).contains(&self.render_distance),
            "Render distance must be 2..32 sections",
        )?;
        ensure(
            self.fog_distance.is_finite() && (0.0..=1.5).contains(&self.fog_distance),
            "Fog distance must be 0 (off) or up to 1.5",
        )?;
        ensure(
            self.lod_distance.is_finite() && (0.75..=2.0).contains(&self.lod_distance),
            "LOD distance must be 0.75..2.0",
        )?;
        ensure(
            self.vegetation_density.is_finite() && (0.0..=1.0).contains(&self.vegetation_density),
            "Vegetation density must be 0..1",
        )?;
        ensure(
            self.brightness.is_finite() && (0.5..=1.5).contains(&self.brightness),
            "Brightness must be 0.5..1.5",
        )?;
        ensure(
            self.ui_scale.is_finite() && (0.75..=1.5).contains(&self.ui_scale),
            "UI scale must be 0.75..1.5",
        )?;
        ensure((24..=240).contains(&self.ui_fps), "UI FPS must be 24..240")?;
        ensure(
            self.fov.is_finite() && (45.0..=110.0).contains(&self.fov),
            "FOV must be 45..110",
        )
    }
}
