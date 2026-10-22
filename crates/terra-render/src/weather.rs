use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_player::TerraCamera;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

mod clouds;
mod lightning;
mod precipitation;
pub(crate) mod storage;
pub(crate) mod surfaces;
pub use precipitation::{WeatherPreview, WeatherRenderStatus};
pub(crate) use precipitation::{WeatherSet, WeatherVisual};

pub(crate) fn foliage_wind(weather: Option<&WeatherVisual>) -> Vec4 {
    let Some(weather) = weather else {
        return Vec4::new(0.86, 0.51, 1.0, 1.0);
    };
    Vec4::from_array(terra_world::props::weather_wind_frame(
        weather.wind.to_array(),
    ))
}

pub(crate) fn apply_environment(
    weather: &WeatherVisual,
    water: f32,
    cave: f32,
    fog: &mut DistanceFog,
    grading: &mut bevy::render::view::ColorGrading,
    ambient: &mut AmbientLight,
) {
    let flash = weather.flash * (1.0 - water) * (1.0 - cave);
    if flash > 0.0 {
        ambient.brightness *= 1.0 + flash * 2.0;
        let color = fog.color.to_srgba();
        fog.color = Color::srgb(
            color.red + flash * 0.20,
            color.green + flash * 0.22,
            color.blue + flash * 0.27,
        );
    }
    let cloud = ((weather.clouds - 0.30) / 0.70).clamp(0.0, 1.0) * (1.0 - water) * (1.0 - cave);
    if cloud <= 0.0 {
        return;
    }
    let color = fog.color.to_srgba();
    let luminance = color.red * 0.24 + color.green * 0.54 + color.blue * 0.22;
    fog.color = Color::srgb(
        color.red + (luminance * 0.82 - color.red) * cloud * 0.62,
        color.green + (luminance * 0.92 - color.green) * cloud * 0.62,
        color.blue + (luminance - color.blue) * cloud * 0.62,
    );
    if let FogFalloff::Linear { start, end } = &mut fog.falloff {
        let precipitation = weather.rain.max(weather.snow) * (1.0 - water) * (1.0 - cave);
        *start *= 1.0 - precipitation * 0.55;
        *end *= 1.0 - precipitation * 0.22;
    }
    ambient.brightness *= 1.0 - cloud * 0.12;
    grading.global.post_saturation *= 1.0 - cloud * 0.07;
}
#[derive(Resource, Default)]
struct Flames {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
    last: f32,
}

pub(crate) fn install(app: &mut App) {
    storage::install(app);
    precipitation::install(app);
    clouds::install(app);
    lightning::install(app);
    surfaces::install(app);
    app.init_resource::<Flames>().add_systems(
        Update,
        fire_visuals.after(terra_player::PlayerSet::Movement),
    );
}
#[derive(bevy::ecs::system::SystemParam)]
struct WeatherWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    time: Res<'w, Time>,
}
fn fire_visuals(
    mut commands: Commands,
    world: WeatherWorld,
    cameras: Query<&Transform, With<TerraCamera>>,
    mut state: ResMut<Flames>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if world.time.elapsed_secs() - state.last < 0.1 {
        return;
    }
    state.last = world.time.elapsed_secs();
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let mut fires: Vec<_> = world
        .journal
        .active_fires()
        .filter(|(p, _)| {
            world.chunks.block_at(*p).is_some()
                && Vec3::from_array(p.map(|v| v as f32)).distance_squared(camera.translation)
                    < 48.0 * 48.0
        })
        .collect();
    fires.sort_by_key(|(p, _)| *p);
    fires.truncate(64);
    if fires.is_empty() {
        if let Some(entity) = state.entity.take() {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = state.mesh.take() {
            meshes.remove(mesh.id());
        }
        return;
    }
    let mut batches = std::collections::BTreeMap::new();
    for (position, condition) in fires {
        let base = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 1.0, 0.5);
        for i in 0..3 {
            let h = 0.18
                + condition.fire * 0.25
                + (world.time.elapsed_secs() * 7.0 + i as f32).sin().abs() * 0.12;
            if let Err(error) = crate::props::add_box(
                &mut batches,
                0,
                base + Vec3::new((i as f32 - 1.0) * 0.18, h * 0.5, 0.0),
                Vec3::new(0.12, h, 0.12),
                Quat::IDENTITY,
            ) {
                tracing::error!(%error, "Fire batch geometry failed");
                return;
            }
        }
    }
    let Some(batch) = batches.remove(&0) else {
        return;
    };
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals)
    .with_inserted_indices(Indices::U32(batch.indices));
    if state.material.is_none() {
        state.material = Some(materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::srgb(1.0, 0.36, 0.025),
            emissive: LinearRgba::new(1.5, 0.35, 0.01, 1.0),
            ..default()
        }));
    }
    if let Some(handle) = &state.mesh {
        if let Some(mut existing) = meshes.get_mut(handle) {
            *existing = mesh;
        }
        if let Some(entity) = state.entity {
            commands
                .entity(entity)
                .remove::<bevy::camera::primitives::Aabb>();
        }
    } else {
        let handle = meshes.add(mesh);
        if let Some(material) = &state.material {
            state.entity = Some(
                commands
                    .spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::default(),
                        NotShadowCaster,
                    ))
                    .id(),
            );
        }
        state.mesh = Some(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storms_have_stronger_wind_inside_expanded_mesh_bounds() {
        let calm = WeatherVisual {
            wind: Vec2::new(0.35, 0.15),
            ..default()
        };
        let storm = WeatherVisual {
            wind: Vec2::new(-4.0, 3.5),
            ..default()
        };
        let a = foliage_wind(Some(&calm));
        let b = foliage_wind(Some(&storm));
        assert!(a.z < 0.3 && b.z > 1.9 && b.z <= terra_world::props::MAX_WEATHER_WIND);
        assert!(b.x < 0.0 && b.y > 0.0);
        assert!((Vec2::new(b.x, b.y).length() - 1.0).abs() < 0.01);
        assert_eq!(
            foliage_wind(Some(&WeatherVisual::default())).z,
            23.0 / 128.0
        );
    }
    #[test]
    fn precipitation_atmosphere_leaves_sheltered_and_underwater_views_untouched() {
        let weather = WeatherVisual {
            rain: 1.0,
            clouds: 1.0,
            ..default()
        };
        for (water, cave) in [(1.0, 0.0), (0.0, 1.0)] {
            let mut fog = DistanceFog::default();
            let old_color = fog.color;
            let mut grading = crate::scene::surface_grading();
            let old_saturation = grading.global.post_saturation;
            let mut ambient = AmbientLight::default();
            let old_brightness = ambient.brightness;
            apply_environment(&weather, water, cave, &mut fog, &mut grading, &mut ambient);
            assert_eq!(fog.color, old_color);
            assert_eq!(grading.global.post_saturation, old_saturation);
            assert_eq!(ambient.brightness, old_brightness);
        }
    }
}
