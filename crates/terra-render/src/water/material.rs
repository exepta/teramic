use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    render::storage::ShaderBuffer,
    shader::ShaderRef,
};
use terra_core::{ClientConfig, GameConfig, WaterQuality};

pub(crate) const ATTRIBUTE_WAVE_WEIGHT: bevy::mesh::MeshVertexAttribute =
    bevy::mesh::MeshVertexAttribute::new(
        "Water_WaveWeight",
        1_883_214_072,
        bevy::render::render_resource::VertexFormat::Float32,
    );

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, PartialEq)]
#[bind_group_data(WaterKey)]
pub(crate) struct WaterExtension {
    pub native_depth_filter: bool,
    pub regional_seas: bool,
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
    /// A stable binding: live weather and contacts never modify material assets.
    #[storage(104, read_only)]
    pub dynamics: Handle<ShaderBuffer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct WaterKey {
    native_depth_filter: bool,
    regional_seas: bool,
}

impl From<&WaterExtension> for WaterKey {
    fn from(extension: &WaterExtension) -> Self {
        Self {
            native_depth_filter: extension.native_depth_filter,
            regional_seas: extension.regional_seas,
        }
    }
}

impl MaterialExtension for WaterExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_water.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_water.wesl".into()
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
        if biome
            && key.bind_group_data.regional_seas
            && let Some(fragment) = &mut descriptor.fragment
        {
            fragment.shader_defs.push("WATER_REGIONAL_SEAS".into());
        }
        let waves = layout.0.contains(ATTRIBUTE_WAVE_WEIGHT);
        let medium_weights = layout.0.contains(super::biome::ATTRIBUTE_MEDIUM_WEIGHTS);
        let liquid = layout.0.contains(crate::liquid_motion::FROM);
        if biome || waves || medium_weights || liquid {
            let mut attributes = vec![
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ];
            if liquid {
                attributes.push(crate::liquid_motion::FROM.at_shader_location(11));
                attributes.push(crate::liquid_motion::DATA.at_shader_location(12));
                descriptor.vertex.shader_defs.push("LIQUID_MOTION".into());
            }
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
            if medium_weights {
                attributes.push(super::biome::ATTRIBUTE_MEDIUM_WEIGHTS.at_shader_location(10));
                descriptor
                    .vertex
                    .shader_defs
                    .push("WATER_MEDIUM_WEIGHTS".into());
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

pub(crate) fn setup_water(
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
            regional_seas: false,
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
            dynamics: super::storage::WATER_BUFFER_HANDLE,
        }
    }
}

type WaterEnvironment<'w> = (
    Option<Res<'w, crate::scene::daylight::Daylight>>,
    Option<Res<'w, crate::scene::caves::CaveState>>,
    Option<Res<'w, super::contacts::WakeField>>,
    Option<Res<'w, Time>>,
    Option<Res<'w, crate::ParticleFluidInput>>,
    Option<Res<'w, crate::ParticleFluidStatus>>,
    Option<Res<'w, terra_world::streaming::WorldTerrain>>,
);

pub(super) fn update_water_settings(
    config: Res<GameConfig>,
    assets: Res<WaterAssets>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    visual: Option<Res<crate::weather::WeatherVisual>>,
    raymarch: Option<Res<super::raymarch::WaterRaymarch>>,
    mut payload: ResMut<super::storage::WaterPayload>,
    environment: WaterEnvironment,
) {
    let weather = water_weather(visual.as_deref());
    let native_depth_filter = raymarch.as_ref().is_some_and(|state| state.ready);
    let (daylight, cave, wakes, time, _, _, terrain) = environment;
    let regional_seas = terrain.is_some_and(|terrain| terrain.0.marine_overhaul());
    let solar = daylight.as_ref().map_or(1.0, |day| day.daylight)
        * cave
            .as_ref()
            .map_or(1.0, |cave| crate::scene::caves::sky_visibility(cave.blend));
    let solar = (solar.clamp(0.0, 1.0) * 128.0).round() / 128.0;
    // Match Bevy's globals.time without changing this material just to advance
    // animation. Its existing global uniform already updates once per frame.
    let wrap = time
        .as_ref()
        .map_or(3600.0, |time| time.wrap_period().as_secs_f32());
    let mut wake_positions = [Vec4::ZERO; super::contacts::WAKE_CAPACITY];
    let mut wake_motions = [Vec4::ZERO; super::contacts::WAKE_CAPACITY];
    let mut count = 0;
    if let Some(wakes) = wakes {
        for (position, motion) in wakes.positions.iter().zip(wakes.motions) {
            if motion.w <= 0.0 {
                continue;
            }
            wake_positions[count] = Vec4::new(
                position.x,
                position.y,
                position.z,
                position.w.rem_euclid(wrap),
            );
            wake_motions[count] = motion;
            count += 1;
        }
    }
    let environment = Vec4::new(
        solar,
        count as f32,
        wrap,
        time.as_ref().map_or(0.0, |time| time.elapsed_secs()),
    );
    let data = super::storage::WaterGpuData {
        weather,
        environment,
        wake_positions,
        wake_motions,
        gallery_aperture: cave.as_ref().map_or(Vec4::ZERO, |cave| {
            let diffuse_sky = daylight.as_ref().map_or(1., |d| d.daylight)
                * visual.as_ref().map_or(1., |w| 1. - w.clouds * 0.5);
            cave.gallery_aperture(diffuse_sky)
        }),
    };
    if payload.0 != data {
        payload.0 = data;
    }
    let Some(current) = materials.get(&assets.0) else {
        return;
    };
    if config.is_changed() || current.extension.regional_seas != regional_seas {
        let mut next = WaterExtension::from_config(&config.client);
        next.native_depth_filter = native_depth_filter;
        next.regional_seas = regional_seas;
        if current.extension != next
            && let Some(mut material) = materials.get_mut(&assets.0)
        {
            material.extension = next;
        }
    } else if current.extension.native_depth_filter != native_depth_filter
        && let Some(mut material) = materials.get_mut(&assets.0)
    {
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
