use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use terra_core::{ClientConfig, GameConfig, WaterQuality};

pub(super) const ATTRIBUTE_WAVE_WEIGHT: bevy::mesh::MeshVertexAttribute =
    bevy::mesh::MeshVertexAttribute::new(
        "Water_WaveWeight",
        1_883_214_072,
        bevy::render::render_resource::VertexFormat::Float32,
    );

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(WaterKey)]
pub(crate) struct WaterExtension {
    pub native_depth_filter: bool,
    #[uniform(100)]
    pub tint: Vec4,
    #[uniform(101)]
    pub settings: Vec4,
    /// Wave amplitude, wave count, caustic strength, foam strength.
    #[uniform(102)]
    pub effects: Vec4,
    /// Screen-space reflection steps, refinements, range and surface thickness.
    #[uniform(103)]
    pub reflections: Vec4,
    /// Wind direction, storm energy and wind speed. Shared by every water batch.
    #[uniform(104)]
    pub weather: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WaterKey {
    native_depth_filter: bool,
}

impl From<&WaterExtension> for WaterKey {
    fn from(extension: &WaterExtension) -> Self {
        Self {
            native_depth_filter: extension.native_depth_filter,
        }
    }
}

impl MaterialExtension for WaterExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_water.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_water.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if super::raymarch::enabled(key.bind_group_data.native_depth_filter, key.mesh_key)
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment
                .shader_defs
                .push("WATER_NATIVE_DEPTH_FILTER".into());
        }
        let biome = layout.0.contains(super::biome::ATTRIBUTE_BIOME_TINT);
        let waves = layout.0.contains(ATTRIBUTE_WAVE_WEIGHT);
        if biome || waves {
            let mut attributes = vec![
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ];
            if biome {
                attributes.push(super::biome::ATTRIBUTE_BIOME_TINT.at_shader_location(8));
                descriptor
                    .vertex
                    .shader_defs
                    .push("WATER_BIOME_TINT".into());
            }
            if waves {
                attributes.push(ATTRIBUTE_WAVE_WEIGHT.at_shader_location(9));
                descriptor
                    .vertex
                    .shader_defs
                    .push("WATER_WAVE_WEIGHT".into());
            }
            for (attribute, location) in [
                (Mesh::ATTRIBUTE_UV_1, 3),
                (Mesh::ATTRIBUTE_TANGENT, 4),
                (Mesh::ATTRIBUTE_COLOR, 5),
            ] {
                if layout.0.contains(attribute) {
                    attributes.push(attribute.at_shader_location(location));
                }
            }
            descriptor.vertex.buffers = vec![layout.0.get_layout(&attributes)?];
        }
        Ok(())
    }
}
pub(crate) type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

#[derive(Resource)]
pub(crate) struct WaterAssets(pub Handle<WaterMaterial>);

pub(super) fn setup_water(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    commands.insert_resource(WaterAssets(materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgb(0.06, 0.32, 0.32),
            perceptual_roughness: 0.16,
            reflectance: 0.35,
            specular_transmission: 1.0,
            ior: 1.333,
            double_sided: true,
            cull_mode: None,
            depth_bias: 1.0,
            ..default()
        },
        extension: WaterExtension::from_config(&config.client),
    })));
}

impl WaterExtension {
    pub(super) fn from_config(config: &ClientConfig) -> Self {
        let (waves, caustics, refraction, reflection_steps, refinements) =
            match config.water_quality {
                WaterQuality::Low => (1.0, 0.0, 0.0, 0.0, 0.0),
                WaterQuality::Medium => (2.0, 0.0, 0.004, 16.0, 3.0),
                WaterQuality::High => (3.0, 0.08, 0.006, 28.0, 5.0),
            };
        Self {
            native_depth_filter: false,
            tint: Vec3::from_array(super::biome::DEFAULT_TINT).extend(1.0),
            // Speed, absorption density, refraction, shallow foam width in meters.
            settings: Vec4::new(0.6, 0.62, refraction, 0.65),
            effects: Vec4::new(config.water_wave_strength, waves, caustics, 0.38),
            reflections: Vec4::new(
                if config.water_reflections {
                    reflection_steps
                } else {
                    0.0
                },
                refinements,
                80.0,
                0.55,
            ),
            weather: Vec4::new(0.86, 0.51, 0.0, 0.0),
        }
    }
}

pub(super) fn update_water_settings(
    config: Res<GameConfig>,
    assets: Res<WaterAssets>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    visual: Option<Res<crate::weather::WeatherVisual>>,
    raymarch: Option<Res<super::raymarch::WaterRaymarch>>,
) {
    let weather = water_weather(visual.as_deref());
    let native_depth_filter = raymarch.as_ref().is_some_and(|state| state.ready);
    let Some(current) = materials.get(&assets.0) else {
        return;
    };
    if (config.is_changed()
        || current.extension.weather != weather
        || current.extension.native_depth_filter != native_depth_filter)
        && let Some(mut material) = materials.get_mut(&assets.0)
    {
        if config.is_changed() {
            material.extension = WaterExtension::from_config(&config.client);
        }
        material.extension.weather = weather;
        material.extension.native_depth_filter = native_depth_filter;
    }
}

pub(super) fn water_weather(visual: Option<&crate::weather::WeatherVisual>) -> Vec4 {
    let wind = visual.map_or(Vec2::ZERO, |weather| weather.wind);
    let speed = wind.length();
    let direction = wind.try_normalize().unwrap_or(Vec2::new(0.86, 0.51));
    // Ordinary weather retains the calm lake/river style. Violent fronts carry
    // enough energy for visibly crossing waves instead of only faster ripples.
    let storm = ((speed - 2.2) / 4.4).clamp(0.0, 1.0);
    Vec4::new(direction.x, direction.y, storm, speed.min(12.0))
        .map(|value| (value * 128.0).round() / 128.0)
}
