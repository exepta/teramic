//! One stable weather binding shared by vegetation and wet terrain materials.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{BufferId, BufferUsages, ShaderType, encase},
        renderer::RenderQueue,
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};
use terra_core::{GameConfig, WeatherKind};

#[path = "snow.rs"]
pub(crate) mod snow;

pub(crate) const WEATHER_BUFFER_HANDLE: Handle<ShaderBuffer> =
    uuid_handle!("8dd87881-7ca7-4a50-9c64-5d9249f71f0e");

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub(crate) struct WeatherGpuData {
    pub wind: Vec4,
    /// Wetness, rain, snow, unwrapped plant growth clock.
    pub surface: Vec4,
    /// Weather RGB multiplier and smoothed seasonal temperature offset.
    pub tint: Vec4,
    pub leaves: Vec4,
    /// Grass RGB multiplier and continuous snow reserve.
    pub grass: Vec4,
    /// Flower dormancy, explicit local-gallery cosmetic snow, real flower clock, reserved.
    pub vegetation: Vec4,
    /// Fixed 256-rank flower death timestamps, packed into 64 storage vectors.
    pub flowers: [Vec4; 64],
}

impl Default for WeatherGpuData {
    fn default() -> Self {
        Self {
            wind: super::foliage_wind(None),
            surface: Vec4::ZERO,
            tint: Vec3::ONE.extend(0.0),
            leaves: Vec3::ONE.extend(0.0),
            grass: Vec3::ONE.extend(0.0),
            vegetation: Vec4::ZERO,
            flowers: [Vec4::splat(-1.0); 64],
        }
    }
}

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub(crate) struct WeatherPayload(pub WeatherGpuData);

type SnowContext<'w> = (
    Option<Res<'w, terra_core::WorldWeather>>,
    Option<Res<'w, terra_world::streaming::WorldAccess>>,
    Option<Res<'w, super::CosmeticSnowPreview>>,
    Option<Res<'w, terra_world::edits::EditJournal>>,
    Option<Res<'w, Time<Real>>>,
);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Assets<ShaderBuffer>>()
        .init_resource::<WeatherPayload>();
    let mut buffers = app.world_mut().resource_mut::<Assets<ShaderBuffer>>();
    if !buffers.contains(WEATHER_BUFFER_HANDLE.id()) {
        let mut buffer = ShaderBuffer::with_size(
            WeatherGpuData::min_size().get() as usize,
            RenderAssetUsages::RENDER_WORLD,
        );
        buffer.buffer_description.label = Some("terra shared weather");
        buffer.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        if let Err(error) = buffers.insert(WEATHER_BUFFER_HANDLE.id(), buffer) {
            tracing::error!(%error, "Shared weather buffer initialization failed");
        }
    }
    app.add_systems(PostUpdate, update);
    if app.get_sub_app(RenderApp).is_some() {
        app.add_plugins(ExtractResourcePlugin::<WeatherPayload>::default());
        app.sub_app_mut(RenderApp).add_systems(
            Render,
            upload
                .in_set(RenderSystems::PrepareResources)
                .after(prepare_assets::<GpuShaderBuffer>),
        );
    }
}

pub(crate) fn update(
    config: Option<Res<GameConfig>>,
    visual: Option<Res<super::WeatherVisual>>,
    growth: Option<Res<crate::props::PropGrowthStatus>>,
    clock: Option<Res<terra_core::WorldClock>>,
    time: Option<Res<Time>>,
    (authority, access, preview, journal, real): SnowContext,
    (mut payload, mut snow, mut flowers): (
        ResMut<WeatherPayload>,
        Local<snow::SnowVisual>,
        Local<terra_world::props::seasonal::FlowerLifecycle>,
    ),
) {
    let mut data = snapshot(
        config.as_deref(),
        visual.as_deref(),
        growth.as_deref().map_or(0.0, |growth| growth.shader_time),
    );
    let season = config.as_deref().map(|config| {
        clock
            .as_deref()
            .copied()
            .unwrap_or_default()
            .season(config.world.seasons)
    });
    snow.advance(
        season,
        data.surface.z
            * authority
                .as_deref()
                .copied()
                .map_or(1.0, terra_core::WorldWeather::snow_accumulation_multiplier),
        time.as_deref().map_or(0.0, Time::delta_secs),
        authority
            .as_deref()
            .map(|weather| weather.elapsed_total)
            .or_else(|| clock.as_deref().filter(|clock| clock.paused).map(|_| 0.0)),
    );
    data.tint.w = snow.temperature_offset;
    data.grass.w = snow.reserve;
    data.vegetation.y = if preview.is_some()
        && access
            .as_deref()
            .is_none_or(|access| matches!(access, terra_world::streaming::WorldAccess::Local))
    {
        1.0
    } else {
        0.0
    };
    if let Some(season) = season {
        data.leaves =
            Vec3::from_array(season.leaf_tint()).extend(season.leaf_loss().max(snow.reserve));
        data.grass = Vec3::from_array(season.grass_tint()).extend(snow.reserve);
        data.vegetation.x = season.flower_loss();
    }
    // Isolated galleries have no world plugin. Real worlds share their exact
    // lifecycle with authority picking and bee ecology through the journal.
    let lifecycle = journal
        .as_deref()
        .map(|journal| &journal.flowers)
        .filter(|flowers| flowers.initialized())
        .unwrap_or_else(|| {
            flowers.advance(
                data.vegetation.x,
                real.as_deref().map_or(0.0, Time::delta_secs),
            );
            &flowers
        });
    data.vegetation.z = lifecycle.shader_time();
    data.flowers = std::array::from_fn(|index| {
        Vec4::from_array(std::array::from_fn(|channel| {
            lifecycle.shader_times()[index * 4 + channel]
        }))
    });
    if payload.0 != data {
        payload.0 = data;
    }
}

fn snapshot(
    config: Option<&GameConfig>,
    visual: Option<&super::WeatherVisual>,
    growth: f32,
) -> WeatherGpuData {
    let (wetness, rain, snow) = visual.map_or_else(
        || {
            config.map_or((0.0, 0.0, 0.0), |config| {
                let intensity = config.client.weather_intensity;
                match config.client.weather {
                    WeatherKind::Rain => (intensity, intensity, 0.0),
                    WeatherKind::Snow => (0.0, 0.0, intensity),
                    _ => (0.0, 0.0, 0.0),
                }
            })
        },
        |visual| (visual.wetness, visual.rain, visual.snow),
    );
    let tint = visual.map_or_else(
        || {
            config.map_or([1.0; 3], |config| {
                crate::appearance::weather_tint(
                    config.client.weather,
                    config.client.weather_intensity,
                )
            })
        },
        crate::appearance::visual_weather_tint,
    );
    WeatherGpuData {
        wind: super::foliage_wind(visual),
        surface: Vec4::new(wetness, rain, snow, growth),
        tint: Vec3::from_array(tint).extend(0.0),
        leaves: Vec3::ONE.extend(0.0),
        grass: Vec3::ONE.extend(0.0),
        vegetation: Vec4::ZERO,
        flowers: [Vec4::splat(-1.0); 64],
    }
}

fn encode(data: &WeatherGpuData) -> Option<Vec<u8>> {
    let mut buffer = encase::StorageBuffer::new(Vec::with_capacity(1120));
    if let Err(error) = buffer.write(data) {
        tracing::error!(%error, "Shared weather payload encoding failed");
        return None;
    }
    Some(buffer.into_inner())
}

fn upload(
    payload: Option<Res<WeatherPayload>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
    mut previous: Local<Option<(BufferId, WeatherGpuData)>>,
) {
    let Some(payload) = payload else {
        return;
    };
    let Some(buffer) = buffers.get(WEATHER_BUFFER_HANDLE.id()) else {
        return;
    };
    let current = (buffer.buffer.id(), payload.0);
    if previous.as_ref() == Some(&current) {
        return;
    }
    let Some(bytes) = encode(&payload.0) else {
        return;
    };
    queue.write_buffer(&buffer.buffer, 0, &bytes);
    *previous = Some(current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosmetic_snow_requires_an_explicit_gallery_and_is_impossible_for_replicas_or_servers() {
        use terra_world::streaming::WorldAccess;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>()
            .init_resource::<GameConfig>()
            .insert_resource(terra_core::WorldClock {
                day: 21,
                ..Default::default()
            });
        install(&mut app);
        app.update();
        assert_eq!(app.world().resource::<WeatherPayload>().0.vegetation.y, 0.0);
        for access in [
            WorldAccess::Local,
            WorldAccess::Server,
            WorldAccess::Replica { ready: false },
            WorldAccess::Replica { ready: true },
        ] {
            app.insert_resource(access);
            app.update();
            assert_eq!(app.world().resource::<WeatherPayload>().0.vegetation.y, 0.0);
            app.insert_resource(super::super::CosmeticSnowPreview);
            app.update();
            assert_eq!(
                app.world().resource::<WeatherPayload>().0.vegetation.y,
                if access == WorldAccess::Local {
                    1.0
                } else {
                    0.0
                }
            );
            app.world_mut()
                .remove_resource::<super::super::CosmeticSnowPreview>();
            app.update();
            assert_eq!(app.world().resource::<WeatherPayload>().0.vegetation.y, 0.0);
        }
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 1);
        let shader = include_str!("../../../../assets/shaders/terra_wet_surface.wgsl");
        assert!(shader.contains("let preview_snow = terra_weather.grass.w * frost * snow_sky * clamp(terra_weather.vegetation.y, 0.0, 1.0);"));
        assert!(
            shader
                .contains("let snow_amount = max(saved_snow, preview_snow) * (1.0 - sheds_snow);")
        );
        let foliage = include_str!("../../../../assets/shaders/terra_foliage.wgsl");
        let global_snow_lines: Vec<_> = foliage
            .lines()
            .filter(|line| line.contains("let snow ="))
            .collect();
        assert_eq!(global_snow_lines.len(), 2);
        assert!(
            global_snow_lines
                .iter()
                .all(|line| line.contains("clamp(terra_weather.vegetation.y, 0.0, 1.0)"))
        );
    }

    #[test]
    fn season_changes_update_only_the_shared_payload_and_keep_loaded_assets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>()
            .init_resource::<GameConfig>()
            .init_resource::<terra_core::WorldClock>();
        install(&mut app);
        app.update();
        let spring = app.world().resource::<WeatherPayload>().0;
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
            .clear();
        app.world_mut().resource_mut::<terra_core::WorldClock>().day = 14;
        app.update();
        let autumn = app.world().resource::<WeatherPayload>().0;
        assert_ne!(spring.leaves, autumn.leaves);
        assert_ne!(spring.grass, autumn.grass);
        assert_eq!(spring.wind, autumn.wind);
        assert_eq!(spring.surface, autumn.surface);
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 1);
        assert!(
            !app.world_mut()
                .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
                .drain()
                .any(|event| event.is_modified(WEATHER_BUFFER_HANDLE.id()))
        );
    }

    #[test]
    fn storage_layout_matches_six_vectors_and_fixed_256_rank_times_in_every_shader() {
        let data = WeatherGpuData {
            wind: Vec4::new(1.0, 2.0, 3.0, 4.0),
            surface: Vec4::new(5.0, 6.0, 7.0, 8.0),
            tint: Vec4::new(9.0, 10.0, 11.0, 12.0),
            leaves: Vec4::new(13.0, 14.0, 15.0, 16.0),
            grass: Vec4::new(17.0, 18.0, 19.0, 20.0),
            vegetation: Vec4::new(21.0, 22.0, 23.0, 24.0),
            flowers: std::array::from_fn(|index| {
                Vec4::from_array(std::array::from_fn(|channel| {
                    (25 + index * 4 + channel) as f32
                }))
            }),
        };
        let bytes = encode(&data).unwrap();
        assert_eq!(bytes.len(), 1120);
        for (index, bytes) in bytes.chunks_exact(4).enumerate() {
            assert_eq!(
                f32::from_le_bytes(bytes.try_into().unwrap()),
                index as f32 + 1.0
            );
        }
        for shader in [
            include_str!("../../../../assets/shaders/terra_props.wgsl"),
            include_str!("../../../../assets/shaders/terra_prop_fragment.wgsl"),
            include_str!("../../../../assets/shaders/terra_foliage.wgsl"),
            include_str!("../../../../assets/shaders/terra_wet_surface.wgsl"),
        ] {
            assert!(shader.contains("vegetation: vec4<f32>,\n    flowers: array<vec4<f32>, 64>,"));
        }
    }

    #[test]
    fn flower_payload_uses_the_exact_picking_clock_without_mutating_meshes_or_materials() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>();
        let mut journal = terra_world::edits::EditJournal::default();
        journal.flowers.advance(0.0, 0.0);
        journal.flowers.advance(1.0, 0.0);
        journal.flowers.advance(1.0, 3.0);
        let times = *journal.flowers.shader_times();
        app.insert_resource(journal);
        install(&mut app);
        app.update();
        let data = app.world().resource::<WeatherPayload>().0;
        assert_eq!(data.vegetation.z, 3.0);
        for (rank, timestamp) in times.into_iter().enumerate() {
            assert_eq!(data.flowers[rank / 4][rank % 4], timestamp);
        }
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 1);
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
            .clear();
        app.update();
        assert_eq!(app.world().resource::<WeatherPayload>().0, data);
        assert!(
            app.world_mut()
                .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
                .drain()
                .all(|event| !event.is_modified(WEATHER_BUFFER_HANDLE.id()))
        );
    }

    #[test]
    fn snapshot_keeps_live_weather_precision_and_the_unwrapped_growth_clock() {
        let visual = super::super::WeatherVisual {
            wetness: 0.234_567,
            rain: 0.876_543,
            snow: 0.345_678,
            wind: Vec2::new(6.0, 2.0),
            ..default()
        };
        let data = snapshot(None, Some(&visual), 9_876.5);
        assert_eq!(data.wind, super::super::foliage_wind(Some(&visual)));
        assert_eq!(
            data.surface,
            Vec4::new(visual.wetness, visual.rain, visual.snow, 9_876.5)
        );
        assert_eq!(
            data.tint,
            Vec3::from_array(crate::appearance::visual_weather_tint(&visual)).extend(0.0)
        );
        let mut config = GameConfig::default();
        config.client.weather = WeatherKind::Rain;
        config.client.weather_intensity = 0.375;
        let fallback = snapshot(Some(&config), None, 0.0);
        assert_eq!(fallback.surface, Vec4::new(0.375, 0.375, 0.0, 0.0));
        assert_eq!(
            fallback.tint,
            Vec3::from_array(crate::appearance::weather_tint(WeatherKind::Rain, 0.375)).extend(0.0)
        );
    }

    #[test]
    fn animated_weather_updates_payload_without_modifying_the_shared_asset() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<ShaderBuffer>();
        install(&mut app);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
            .clear();
        let initial = app
            .world()
            .resource::<Assets<ShaderBuffer>>()
            .get(&WEATHER_BUFFER_HANDLE)
            .unwrap()
            .buffer_description
            .clone();
        for step in 1..=32 {
            app.insert_resource(super::super::WeatherVisual {
                wetness: step as f32 / 32.0,
                rain: step as f32 / 64.0,
                wind: Vec2::new(step as f32, 2.0),
                ..default()
            });
            app.update();
            assert_eq!(
                app.world().resource::<WeatherPayload>().0.surface.x,
                step as f32 / 32.0
            );
            assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 1);
            assert!(
                !app.world_mut()
                    .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
                    .drain()
                    .any(|event| event.is_modified(WEATHER_BUFFER_HANDLE.id()))
            );
        }
        let buffer = app
            .world()
            .resource::<Assets<ShaderBuffer>>()
            .get(&WEATHER_BUFFER_HANDLE)
            .unwrap();
        assert_eq!(
            buffer.buffer_description.usage,
            BufferUsages::STORAGE | BufferUsages::COPY_DST
        );
        assert_eq!(buffer.buffer_description.size, initial.size);
        assert_eq!(buffer.buffer_description.size, 1120);
    }
}
