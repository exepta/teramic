use super::*;
use crate::water::material::{WaterAssets, WaterExtension, WaterKey, WaterMaterial};
use bevy::{asset::AssetEvent, pbr::MaterialExtension};
use terra_core::{GameConfig, WaterQuality, WeatherKind};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<ShaderBuffer>()
        .init_asset::<WaterMaterial>()
        .init_resource::<GameConfig>()
        .init_resource::<crate::weather::WeatherVisual>()
        .init_resource::<crate::scene::daylight::Daylight>()
        .init_resource::<crate::scene::caves::CaveState>()
        .init_resource::<crate::water::contacts::WakeField>()
        .init_resource::<crate::water::raymarch::WaterRaymarch>()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ))
        .add_systems(Startup, crate::water::material::setup_water)
        .add_systems(Update, crate::water::material::update_water_settings);
    install(&mut app);
    app.update();
    app.update();
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<WaterMaterial>>>()
        .clear();
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
        .clear();
    app
}

fn material_modifications(app: &mut App) -> usize {
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<WaterMaterial>>>()
        .drain()
        .filter(|event| matches!(event, AssetEvent::Modified { .. }))
        .count()
}

#[test]
fn weather_cave_and_live_wake_updates_do_not_modify_material_or_buffer_assets() {
    let mut app = app();
    let handle = app.world().resource::<WaterAssets>().0.clone();
    let baseline = app
        .world()
        .resource::<Assets<WaterMaterial>>()
        .get(&handle)
        .unwrap()
        .extension
        .clone();
    let mut previous_time = app.world().resource::<Time>().elapsed_secs();
    let initial_bytes = app
        .world()
        .resource::<Assets<ShaderBuffer>>()
        .get(&WATER_BUFFER_HANDLE)
        .unwrap()
        .cast_slice::<u8>()
        .unwrap()
        .to_vec();
    for step in 1..=32 {
        let fraction = step as f32 / 32.0;
        let now = app.world().resource::<Time>().elapsed_secs();
        {
            let mut visual = app
                .world_mut()
                .resource_mut::<crate::weather::WeatherVisual>();
            visual.wind = Vec2::new(-step as f32 * 0.2, fraction * 4.0);
            visual.rain = fraction;
        }
        app.world_mut()
            .resource_mut::<crate::scene::daylight::Daylight>()
            .daylight = 1.0 - fraction * 0.25;
        app.world_mut()
            .resource_mut::<crate::scene::caves::CaveState>()
            .blend = fraction;
        {
            let mut wakes = app
                .world_mut()
                .resource_mut::<crate::water::contacts::WakeField>();
            // The inactive first slot must still be compacted out.
            wakes.positions[1] = Vec4::new(step as f32, 4.0, 6.0, now);
            wakes.motions[1] = Vec4::new(1.0, 0.0, 2.0, fraction);
        }
        // Unrelated live-weather config changes must not touch static water settings.
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .weather_intensity = fraction;
        app.update();
        let data = app.world().resource::<WaterPayload>().0;
        let visual = app.world().resource::<crate::weather::WeatherVisual>();
        assert_eq!(
            data.weather,
            crate::water::material::water_weather(Some(visual))
        );
        let solar = (1.0 - fraction * 0.25) * crate::scene::caves::sky_visibility(fraction);
        assert_eq!(data.environment.x, (solar * 128.0).round() / 128.0);
        assert_eq!(data.environment.y, 1.0);
        assert_eq!(
            data.wake_positions[0],
            Vec4::new(step as f32, 4.0, 6.0, now)
        );
        assert_eq!(data.wake_motions[0], Vec4::new(1.0, 0.0, 2.0, fraction));
        assert_eq!(data.wake_positions[1], Vec4::ZERO);
        assert_eq!(data.wake_motions[1], Vec4::ZERO);
        assert!(app.world().resource::<Time>().elapsed_secs() > previous_time);
        previous_time = app.world().resource::<Time>().elapsed_secs();
        assert_eq!(material_modifications(&mut app), 0);
        assert!(
            app.world_mut()
                .resource_mut::<Messages<AssetEvent<ShaderBuffer>>>()
                .drain()
                .all(|event| !event.is_modified(WATER_BUFFER_HANDLE.id()))
        );
        let materials = app.world().resource::<Assets<WaterMaterial>>();
        assert_eq!(materials.len(), 1);
        assert_eq!(materials.get(&handle).unwrap().extension, baseline);
        let buffers = app.world().resource::<Assets<ShaderBuffer>>();
        assert_eq!(buffers.len(), 1);
        let buffer = buffers.get(&WATER_BUFFER_HANDLE).unwrap();
        assert_eq!(buffer.buffer_size(), 544);
        assert_eq!(
            buffer.buffer_usage,
            BufferUsages::STORAGE | BufferUsages::COPY_DST
        );
        assert_eq!(buffer.cast_slice::<u8>().unwrap(), initial_bytes);
        // This is the same encoding consumed by the render-world queue upload.
        let bytes = encode(&data).unwrap();
        assert_eq!(bytes.len(), 544);
        assert_eq!(
            f32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            data.environment.x
        );
        assert_eq!(
            f32::from_le_bytes(bytes[32..36].try_into().unwrap()),
            step as f32
        );
        assert_eq!(
            f32::from_le_bytes(bytes[300..304].try_into().unwrap()),
            fraction
        );
    }
}

#[test]
fn static_quality_changes_and_native_depth_key_still_update_the_shared_material() {
    let mut app = app();
    let handle = app.world().resource::<WaterAssets>().0.clone();
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .water_quality = WaterQuality::Low;
    app.update();
    app.update();
    assert_eq!(material_modifications(&mut app), 1);
    let extension = app
        .world()
        .resource::<Assets<WaterMaterial>>()
        .get(&handle)
        .unwrap()
        .extension
        .clone();
    assert_eq!(extension.effects.y, 1.0);
    assert_eq!(extension.dynamics, WATER_BUFFER_HANDLE);
    let old_key = WaterKey::from(&extension);
    app.world_mut()
        .resource_mut::<crate::water::raymarch::WaterRaymarch>()
        .ready = true;
    app.update();
    app.update();
    assert_eq!(material_modifications(&mut app), 1);
    let materials = app.world().resource::<Assets<WaterMaterial>>();
    let changed = &materials.get(&handle).unwrap().extension;
    assert_ne!(WaterKey::from(changed), old_key);
    assert!(changed.native_depth_filter);
    assert_eq!(changed.dynamics, WATER_BUFFER_HANDLE);
    assert_eq!(changed.effects, extension.effects);
    assert_eq!(materials.len(), 1);
    app.world_mut().resource_mut::<GameConfig>().client.weather = WeatherKind::Rain;
    app.update();
    app.update();
    assert_eq!(material_modifications(&mut app), 0);
}

#[test]
fn storage_layout_initialization_and_forward_bindings_match_both_shader_stages() {
    let data = WaterGpuData {
        weather: Vec4::new(1.0, 2.0, 3.0, 4.0),
        environment: Vec4::new(5.0, 6.0, 7.0, 8.0),
        wake_positions: std::array::from_fn(|index| {
            Vec4::from_array(std::array::from_fn(|channel| {
                (9 + index * 4 + channel) as f32
            }))
        }),
        wake_motions: std::array::from_fn(|index| {
            Vec4::from_array(std::array::from_fn(|channel| {
                (73 + index * 4 + channel) as f32
            }))
        }),
    };
    let bytes = encode(&data).unwrap();
    assert_eq!(bytes.len(), 544);
    assert_eq!(bytes.len() as u64, WaterGpuData::min_size().get());
    for (index, bytes) in bytes.chunks_exact(4).enumerate() {
        assert_eq!(
            f32::from_le_bytes(bytes.try_into().unwrap()),
            index as f32 + 1.0
        );
    }
    let mut headless = App::new();
    install(&mut headless);
    install(&mut headless);
    let buffers = headless.world().resource::<Assets<ShaderBuffer>>();
    assert_eq!(buffers.len(), 1);
    assert_eq!(
        buffers
            .get(&WATER_BUFFER_HANDLE)
            .unwrap()
            .cast_slice::<u8>()
            .unwrap(),
        encode(&WaterGpuData::default()).unwrap()
    );
    let shader = include_str!("../../../../assets/shaders/terra_water.wesl");
    assert_eq!(shader.matches("@binding(104)").count(), 1);
    assert!(shader.contains("@binding(104) var<storage, read> terra_water: WaterDynamics;"));
    assert!(shader.contains("weather: vec4<f32>,\n    environment: vec4<f32>,\n    wake_positions: array<vec4<f32>, 16>,\n    wake_motions: array<vec4<f32>, 16>,"));
    for old_binding in [105, 106, 107] {
        assert!(!shader.contains(&format!("@binding({old_binding})")));
    }
    assert_eq!(
        shader.matches("let weather = terra_water.weather;").count(),
        3
    );
    assert_eq!(
        shader
            .matches("let environment = terra_water.environment;")
            .count(),
        2
    );
    assert!(!WaterExtension::enable_prepass());
    assert!(!WaterExtension::enable_shadows());
}
