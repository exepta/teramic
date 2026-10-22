//! Optional real raymarched fog, using Bevy's existing volumetric pipeline.
use bevy::{
    asset::RenderAssetUsages,
    light::{FogVolume, VolumetricFog, VolumetricLight},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use terra_core::{GameConfig, WaterQuality};
use terra_player::TerraCamera;

const MAX_VOLUMES: usize = 4;
const RANGE: f32 = 40.0;
#[derive(Component)]
struct Mist;
#[derive(Component)]
pub(crate) struct MistCamera;
#[derive(Component)]
pub(crate) struct MistSun;
#[derive(Resource, Default)]
pub(crate) struct MistAssets(Option<Handle<Image>>);

fn density_texture() -> Image {
    let mut pixels = Vec::with_capacity(16 * 16 * 16);
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                let p = (Vec3::new(x as f32, y as f32, z as f32) - Vec3::splat(7.5)) / 7.5;
                let edge = (1.0 - p.length_squared()).max(0.0).powi(2);
                let noise = 0.78 + 0.22 * (p.x * 5.1 + p.y * 3.7 - p.z * 4.3).sin();
                pixels.push((edge * noise * 255.0).round() as u8);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 16,
        },
        TextureDimension::D3,
        pixels,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::linear();
    image
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Scene<'w, 's> {
    sources: Query<
        'w,
        's,
        (
            &'static super::spray::SpraySources,
            &'static GlobalTransform,
        ),
    >,
    camera: MistCameras<'w, 's>,
    sun: MistLights<'w, 's>,
    volumes: Query<'w, 's, (Entity, &'static mut Transform, &'static mut FogVolume), With<Mist>>,
}
type MistCameras<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        Has<MistCamera>,
        Has<crate::scene::caves::quality::CaveCamera>,
        Option<&'static VolumetricFog>,
    ),
    With<TerraCamera>,
>;
type MistLights<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Has<VolumetricLight>,
        Has<MistSun>,
        Has<crate::scene::caves::quality::CaveSun>,
    ),
    SunFilter,
>;
type SunFilter = (
    With<DirectionalLight>,
    Without<crate::previews::PreviewScene>,
);

pub(crate) fn update(
    mut commands: Commands,
    config: Res<GameConfig>,
    time: Res<Time>,
    mut scene: Scene,
    mut assets: ResMut<MistAssets>,
    mut images: ResMut<Assets<Image>>,
    mut status: ResMut<super::WaterStatus>,
) {
    let enabled = config.world.water
        && config.client.water_volumetric_mist
        && config.client.water_quality == WaterQuality::High;
    let eye = scene
        .camera
        .iter()
        .next()
        .map(|(_, transform, ..)| transform.translation());
    let mut emitters: Vec<_> = if enabled {
        scene
            .sources
            .iter()
            .flat_map(|(sources, transform)| {
                sources
                    .0
                    .iter()
                    .map(move |p| transform.transform_point(p.truncate()).extend(p.w))
            })
            .filter(|p| {
                p.w > 0.0
                    && eye.is_some_and(|eye| p.truncate().distance_squared(eye) < RANGE * RANGE)
            })
            .collect()
    } else {
        Vec::new()
    };
    if let Some(eye) = eye {
        emitters.sort_by(|a, b| {
            a.truncate()
                .distance_squared(eye)
                .total_cmp(&b.truncate().distance_squared(eye))
        });
    }
    let mut selected: Vec<Vec4> = Vec::with_capacity(MAX_VOLUMES);
    for emitter in emitters {
        if selected
            .iter()
            .all(|p| p.truncate().distance_squared(emitter.truncate()) > 9.0)
        {
            selected.push(emitter);
            if selected.len() == MAX_VOLUMES {
                break;
            }
        }
    }
    status.mist_volumes = selected.len();
    let active = !selected.is_empty();
    for (camera, _, owned, cave, fog) in &scene.camera {
        if active && !owned {
            commands.entity(camera).insert(MistCamera);
        }
        // Restore mist parameters when a simultaneous cave claim ends.
        if active && !cave && (!owned || fog.is_none_or(|fog| fog.step_count != 24)) {
            commands.entity(camera).insert(VolumetricFog {
                step_count: 24,
                ambient_color: Color::srgb(0.82, 0.91, 1.0),
                ambient_intensity: 0.12,
                ..default()
            });
        } else if !active && owned {
            commands.entity(camera).remove::<MistCamera>();
            if !cave {
                commands.entity(camera).remove::<VolumetricFog>();
            }
        }
    }
    for (sun, volumetric, owned, cave) in &scene.sun {
        if active && !owned {
            commands.entity(sun).insert(MistSun);
        }
        if active && !volumetric {
            commands.entity(sun).insert(VolumetricLight);
        } else if !active && owned {
            commands.entity(sun).remove::<MistSun>();
            if !cave {
                commands.entity(sun).remove::<VolumetricLight>();
            }
        }
    }
    let texture = if active {
        Some(
            assets
                .0
                .get_or_insert_with(|| images.add(density_texture()))
                .clone(),
        )
    } else {
        None
    };
    let mut existing = scene.volumes.iter_mut();
    for (index, emitter) in selected.into_iter().enumerate() {
        let strength = (emitter.w / 30.0).clamp(0.4, 1.0);
        let width = 2.0 + strength * 2.8;
        let height = 1.1 + strength * 2.0;
        let transform = Transform::from_translation(emitter.truncate() + Vec3::Y * height * 0.35)
            .with_scale(Vec3::new(width, height, width));
        let volume = FogVolume {
            density_factor: 0.16 * (0.95 + 0.05 * (time.elapsed_secs() * 0.7 + index as f32).sin()),
            density_texture: texture.clone(),
            fog_color: Color::srgb(0.83, 0.93, 0.95),
            scattering: 0.28,
            absorption: 0.06,
            scattering_asymmetry: 0.2,
            light_intensity: 0.85,
            ..default()
        };
        if let Some((_, mut old_transform, mut old_volume)) = existing.next() {
            *old_transform = transform;
            *old_volume = volume;
        } else {
            commands.spawn((Name::new("Waterfall mist volume"), Mist, transform, volume));
        }
    }
    for (entity, _, _) in existing {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_density_is_soft_bounded_and_not_a_billboard_or_opaque_cube() {
        let image = density_texture();
        assert_eq!(image.texture_descriptor.dimension, TextureDimension::D3);
        let data = image.data.unwrap();
        assert_eq!(data.len(), 4096);
        assert_eq!(data[0], 0);
        assert_eq!(data[15], 0);
        assert_eq!(data[4095], 0);
        assert!(data[8 + 8 * 16 + 8 * 256] > 128);
        assert!(data.iter().filter(|&&v| v > 0).count() < 4096);
    }
}
