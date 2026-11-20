use super::{Config, ConfigError, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkAnimation {
    #[default]
    None,
    Fade,
    FadeSlow,
    Bounce,
    Rise,
    Slide,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaterQuality {
    Low,
    Medium,
    #[default]
    High,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherKind {
    #[default]
    Clear,
    Rain,
    Snow,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherQuality {
    Off,
    Low,
    Medium,
    #[default]
    High,
    Ultra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientConfig {
    pub blocks: super::BlockDetailConfig,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fov_degrees: f32,
    pub vsync: bool,
    pub debug_overlay: bool,
    pub log_filter: String,
    pub view_distance: u8,
    pub fog_distance: f32,
    pub chunk_animation: ChunkAnimation,
    pub vertical_view_distance: u8,
    pub max_chunk_jobs: usize,
    pub max_mesh_jobs: usize,
    pub mesh_uploads_per_frame: usize,
    pub lod_near_distance: f32,
    pub lod_far_distance: f32,
    pub lod_hysteresis: f32,
    pub prop_lod_near_distance: f32,
    pub prop_lod_far_distance: f32,
    pub water_quality: WaterQuality,
    /// Legacy TOML input only; all non-removed logical props remain represented.
    #[serde(skip_serializing)]
    pub vegetation_density: f32,
    pub water_wave_strength: f32,
    pub water_reflections: bool,
    pub water_volumetric_mist: bool,
    pub weather: WeatherKind,
    pub weather_intensity: f32,
    pub weather_quality: WeatherQuality,
    pub mouse_sensitivity: f32,
    pub fly_speed: f32,
    pub fly_boost_multiplier: f32,
    pub fly_precision_multiplier: f32,
}
impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            title: "Project Terra - P1/P2".into(),
            blocks: super::BlockDetailConfig::default(),
            width: 1280,
            height: 800,
            fov_degrees: 45.0,
            vsync: true,
            debug_overlay: true,
            log_filter: "info".into(),
            view_distance: 8,
            fog_distance: 1.0,
            chunk_animation: ChunkAnimation::None,
            vertical_view_distance: 3,
            max_chunk_jobs: 4,
            max_mesh_jobs: 8,
            mesh_uploads_per_frame: 12,
            lod_near_distance: 96.0,
            lod_far_distance: 160.0,
            lod_hysteresis: 12.0,
            prop_lod_near_distance: 80.0,
            prop_lod_far_distance: 144.0,
            water_quality: WaterQuality::High,
            vegetation_density: 1.0,
            water_wave_strength: 0.65,
            water_reflections: true,
            water_volumetric_mist: false,
            weather: WeatherKind::Clear,
            weather_intensity: 1.0,
            weather_quality: WeatherQuality::High,
            mouse_sensitivity: 0.0022,
            fly_speed: 20.0,
            fly_boost_multiplier: 3.0,
            fly_precision_multiplier: 0.25,
        }
    }
}
impl Config for ClientConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        ensure(self.blocks.valid(), "Invalid client block detail settings")?;
        ensure(
            self.vegetation_density.is_finite() && (0.0..=1.0).contains(&self.vegetation_density),
            "client.vegetation_density must be 0..=1",
        )?;
        ensure(
            self.fov_degrees.is_finite() && (45.0..=110.0).contains(&self.fov_degrees),
            "client.fov_degrees must be finite and 45..=110",
        )?;
        ensure(
            self.weather_intensity.is_finite() && (0.0..=1.0).contains(&self.weather_intensity),
            "client.weather_intensity must be finite and 0..=1",
        )?;
        ensure(
            self.prop_lod_near_distance.is_finite()
                && self.prop_lod_far_distance.is_finite()
                && (32.0..=256.0).contains(&self.prop_lod_near_distance)
                && self.prop_lod_far_distance <= 512.0
                && self.prop_lod_far_distance - self.prop_lod_near_distance
                    > self.lod_hysteresis * 2.0,
            "client prop LOD distances must be finite, 32 <= near < far <= 512, separated by twice hysteresis",
        )?;
        ensure(
            self.lod_near_distance.is_finite()
                && self.lod_far_distance.is_finite()
                && (16.0..=256.0).contains(&self.lod_near_distance)
                && self.lod_far_distance > self.lod_near_distance
                && self.lod_far_distance <= 512.0,
            "client LOD distances must be finite, 16 <= near < far <= 512",
        )?;
        ensure(
            self.lod_hysteresis.is_finite()
                && (0.0..=32.0).contains(&self.lod_hysteresis)
                && self.lod_hysteresis < self.lod_near_distance
                && self.lod_hysteresis * 2.0 < self.lod_far_distance - self.lod_near_distance,
            "client.lod_hysteresis must be 0..=32, below near distance and half the LOD interval",
        )?;
        ensure(
            self.water_wave_strength.is_finite() && (0.0..=2.0).contains(&self.water_wave_strength),
            "client.water_wave_strength must be finite and 0..=2",
        )?;
        ensure(
            self.mouse_sensitivity.is_finite() && (0.0001..=0.02).contains(&self.mouse_sensitivity),
            "client.mouse_sensitivity must be finite and 0.0001..=0.02",
        )?;
        ensure(
            self.fly_speed.is_finite() && (0.1..=100.0).contains(&self.fly_speed),
            "client.fly_speed must be finite and 0.1..=100",
        )?;
        ensure(
            self.fly_boost_multiplier.is_finite()
                && (1.0..=8.0).contains(&self.fly_boost_multiplier),
            "client.fly_boost_multiplier must be finite and 1..=8",
        )?;
        ensure(
            self.fly_precision_multiplier.is_finite()
                && (0.05..=1.0).contains(&self.fly_precision_multiplier),
            "client.fly_precision_multiplier must be finite and 0.05..=1",
        )?;
        ensure(
            self.view_distance <= 32,
            "client.view_distance must be 0..=32",
        )?;
        ensure(
            self.fog_distance.is_finite() && (0.0..=1.5).contains(&self.fog_distance),
            "client.fog_distance must be 0 (off) or up to 1.5",
        )?;
        ensure(
            self.vertical_view_distance <= 4,
            "client.vertical_view_distance must be 0..=4",
        )?;
        ensure(
            (1..=8).contains(&self.max_chunk_jobs),
            "client.max_chunk_jobs must be 1..=8",
        )?;
        ensure(
            (1..=32).contains(&self.max_mesh_jobs),
            "client.max_mesh_jobs must be 1..=32",
        )?;
        ensure(
            (1..=32).contains(&self.mesh_uploads_per_frame),
            "client.mesh_uploads_per_frame must be 1..=32",
        )?;
        ensure(
            !self.title.trim().is_empty(),
            "client.title must not be empty",
        )?;
        ensure(
            (320..=7680).contains(&self.width),
            "client.width must be 320..=7680",
        )?;
        ensure(
            (240..=4320).contains(&self.height),
            "client.height must be 240..=4320",
        )?;
        ensure(
            !self.log_filter.trim().is_empty(),
            "client.log_filter must not be empty",
        )
    }
}
