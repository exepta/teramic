//! One persistent proxy volume; cloud shape, drift and lighting live in the shader.
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, Face},
    shader::ShaderRef,
};
use terra_core::{GameConfig, WeatherQuality};
use terra_player::TerraCamera;
use terra_world::streaming::ChunkStore;

use super::precipitation::{WeatherRenderStatus, WeatherVisual};
mod noise;

const MIN_HEIGHT: f32 = 200.0;
const MAX_HEIGHT: f32 = 312.0;
const EXTENT: f32 = 9_000.0;
const WIND_PERIOD: f32 = 8_192.0;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct CloudExtension {
    /// Cover, daylight, above-water visibility, global lightning illumination.
    #[uniform(100)]
    weather: Vec4,
    /// Observer position and horizontal cloud extent.
    #[uniform(100)]
    observer: Vec4,
    #[uniform(100)]
    haze: Vec4,
    /// Authoritative strike position and local flash strength.
    #[uniform(100)]
    lightning: Vec4,
    #[uniform(100)]
    lightning_tint: Vec4,
    #[uniform(100)]
    sun: Vec4,
    /// Bottom, top, bounded ray sample count, storm intensity.
    #[uniform(100)]
    volume: Vec4,
    #[uniform(100)]
    drift: Vec4,
    #[texture(108, dimension = "3d")]
    #[sampler(109)]
    noise: Handle<Image>,
}

impl MaterialExtension for CloudExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_clouds.wesl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_clouds.wesl".into()
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
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        if let Some(depth) = &mut descriptor.depth_stencil {
            // Sorting bias belongs to the sky ordering, never the physical depth.
            depth.bias = default();
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}
type CloudMaterial = ExtendedMaterial<StandardMaterial, CloudExtension>;

#[derive(Component)]
struct CloudVolume;

#[derive(Resource, Default)]
struct Sky {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<CloudMaterial>>,
    drift: Vec2,
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
        .init_resource::<Sky>()
        .add_systems(
            Update,
            update
                .after(super::precipitation::WeatherSet::Particles)
                .after(crate::water::environment::update),
        );
}

#[derive(bevy::ecs::system::SystemParam)]
struct CloudWorld<'w> {
    time: Res<'w, Time>,
    config: Res<'w, GameConfig>,
    weather: Res<'w, WeatherVisual>,
    daylight: Res<'w, crate::scene::daylight::Daylight>,
    underwater: Res<'w, crate::UnderwaterState>,
    chunks: Res<'w, ChunkStore>,
    authority: Option<Res<'w, terra_core::WorldWeather>>,
    budget: Option<ResMut<'w, crate::upload_budget::MeshUploadStatus>>,
}

type CloudCamera = (With<TerraCamera>, Without<CloudVolume>);
type CloudCameraData = (
    &'static Transform,
    Option<&'static DistanceFog>,
    Option<&'static Projection>,
);

#[derive(bevy::ecs::system::SystemParam)]
struct CloudAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<CloudMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

fn update(
    mut commands: Commands,
    mut world: CloudWorld,
    cameras: Query<CloudCameraData, CloudCamera>,
    mut volumes: Query<(&mut Transform, &mut Visibility), With<CloudVolume>>,
    mut sky: ResMut<Sky>,
    mut status: ResMut<WeatherRenderStatus>,
    mut assets: CloudAssets,
) {
    // These compatibility counters deliberately remain zero: shafts no longer exist.
    status.cloud_shaft_batches = 0;
    status.cloud_shaft_vertices = 0;
    status.cloud_faces = 0;
    status.cloud_batches = 0;
    let camera = cameras.iter().next();
    let enabled = camera.is_some()
        && world.config.client.weather_quality != WeatherQuality::Off
        && !world.chunks.loaded.is_empty();
    if !enabled {
        if let Some(entity) = sky.entity
            && let Ok((_, mut visibility)) = volumes.get_mut(entity)
            && *visibility != Visibility::Hidden
        {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let (camera, fog, projection) = camera.unwrap();
    let quality = world.config.client.weather_quality;
    sky.drift += world.weather.wind * world.time.delta_secs() * 0.65;
    sky.drift = sky.drift.rem_euclid(Vec2::splat(WIND_PERIOD));
    if sky.material.is_none() {
        let noise = assets.images.add(noise::texture());
        sky.material = Some(assets.materials.add(ExtendedMaterial {
            base: StandardMaterial {
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                // Only the exit surface is a ray carrier, including inside views.
                cull_mode: Some(Face::Front),
                fog_enabled: false,
                ..default()
            },
            extension: CloudExtension {
                weather: Vec4::ZERO,
                observer: Vec4::ZERO,
                haze: Vec4::ZERO,
                lightning: Vec4::ZERO,
                lightning_tint: Vec4::ONE,
                sun: Vec4::ZERO,
                volume: Vec4::ZERO,
                drift: Vec4::ZERO,
                noise,
            },
        }));
    }
    let transform = Transform::from_translation(Vec3::new(
        camera.translation.x,
        (MIN_HEIGHT + MAX_HEIGHT) * 0.5,
        camera.translation.z,
    ));
    if let Some(entity) = sky.entity
        && let Ok((mut old_transform, mut visibility)) = volumes.get_mut(entity)
    {
        if *old_transform != transform {
            *old_transform = transform;
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    } else {
        if sky.mesh.is_none() {
            let mesh = proxy_mesh();
            if world.budget.as_deref_mut().is_some_and(|budget| {
                !budget.reserve(
                    crate::upload_budget::UploadKind::Clouds,
                    mesh.count_vertices(),
                )
            }) {
                return;
            }
            sky.mesh = Some(assets.meshes.add(mesh));
            status.cloud_uploads += 1;
        }
        sky.entity = Some(
            commands
                .spawn((
                    Name::new("Shader cloud volume"),
                    CloudVolume,
                    Mesh3d(sky.mesh.as_ref().unwrap().clone()),
                    MeshMaterial3d(sky.material.as_ref().unwrap().clone()),
                    transform,
                    Visibility::Inherited,
                    NotShadowCaster,
                    NotShadowReceiver,
                    NoFrustumCulling,
                ))
                .id(),
        );
    }
    let Some(mut material) = sky
        .material
        .as_ref()
        .and_then(|handle| assets.materials.get_mut(handle))
    else {
        return;
    };
    let haze = fog
        .map_or(Color::srgb(0.53, 0.63, 0.70), |fog| fog.color)
        .to_linear();
    let event = world
        .authority
        .as_ref()
        .and_then(|weather| weather.latest_lightning);
    let extension = &mut material.extension;
    extension.weather = Vec4::new(
        world.weather.clouds,
        world.daylight.daylight,
        1.0 - world.underwater.blend,
        world.weather.flash,
    );
    extension.observer = camera.translation.extend(EXTENT);
    extension.haze = Vec4::new(haze.red, haze.green, haze.blue, 1.0);
    extension.lightning = event.map_or(Vec4::ZERO, |event| {
        Vec3::from_array(event.position.map(|value| value as f32)).extend(world.weather.flash)
    });
    extension.lightning_tint = event.map_or(Vec4::ONE, |event| {
        super::lightning::lightning_tint(event.id).extend(1.0)
    });
    extension.sun = world.daylight.sun.extend(0.0);
    extension.volume = Vec4::new(
        MIN_HEIGHT,
        MAX_HEIGHT,
        sample_count(quality) as f32,
        world.weather.rain.max(world.weather.snow),
    );
    extension.drift = Vec4::new(sky.drift.x, sky.drift.y, 0.0, 0.0);
    // Horizon has a fixed background order; clouds precede local transparency.
    material.base.depth_bias = -projection.map_or(1_000.0, |projection| match projection {
        Projection::Perspective(projection) => projection.far,
        Projection::Orthographic(projection) => projection.far,
        _ => 1_000.0,
    });
    status.cloud_faces = 6;
    status.cloud_batches = 1;
}

fn sample_count(quality: WeatherQuality) -> u32 {
    match quality {
        WeatherQuality::Off => 0,
        WeatherQuality::Low => 10,
        WeatherQuality::Medium => 14,
        WeatherQuality::High => 20,
        WeatherQuality::Ultra => 28,
    }
}

fn proxy_mesh() -> Mesh {
    Mesh::from(Cuboid::new(
        EXTENT * 2.0,
        MAX_HEIGHT - MIN_HEIGHT,
        EXTENT * 2.0,
    ))
}

#[cfg(test)]
#[path = "clouds/tests.rs"]
mod tests;
