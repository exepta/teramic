//! Presentation of server-authored events. Clients never roll their own strikes.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use terra_core::{GameConfig, WeatherQuality, WorldWeather};

use super::precipitation::{WeatherRenderStatus, WeatherSet, WeatherVisual};

#[derive(Resource, Default)]
struct Lightning {
    observer: terra_core::LightningObserver,
    event: Option<(u64, u64)>,
    started: f64,
    initial_age: f32,
    bolt: Option<Entity>,
    light: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<StandardMaterial>>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Lightning>().add_systems(
        Update,
        update
            .after(WeatherSet::Local)
            .before(WeatherSet::Particles)
            .before(crate::water::environment::update),
    );
}

pub(super) fn flash_strength(age: f32) -> f32 {
    if !(0.0..0.65).contains(&age) {
        return 0.0;
    }
    let first = (1.0 - age / 0.11).clamp(0.0, 1.0);
    let second = (1.0 - ((age - 0.18) / 0.10).abs()).clamp(0.0, 1.0) * 0.65;
    let afterglow = (1.0 - age / 0.65).max(0.0) * 0.13;
    first.max(second).max(afterglow)
}

#[derive(bevy::ecs::system::SystemParam)]
struct LightningWorld<'w, 's> {
    time: Res<'w, Time<Real>>,
    config: Res<'w, GameConfig>,
    weather: Option<Res<'w, WorldWeather>>,
    clock: Option<Res<'w, terra_core::WorldClock>>,
    access: Res<'w, terra_world::streaming::WorldAccess>,
    bodies: Query<'w, 's, &'static Transform, With<terra_player::PlayerBody>>,
    cameras: Query<'w, 's, &'static Transform, With<terra_player::TerraCamera>>,
}

fn flash_falloff(distance: f32) -> f32 {
    let t = ((distance - 384.0) / (1600.0 - 384.0)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

fn update(
    mut commands: Commands,
    world: LightningWorld,
    mut state: ResMut<Lightning>,
    mut visual: ResMut<WeatherVisual>,
    mut status: ResMut<WeatherRenderStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let event = world
        .weather
        .as_ref()
        .and_then(|weather| weather.latest_lightning);
    let listener = world
        .bodies
        .iter()
        .next()
        .or_else(|| world.cameras.iter().next());
    let active = world.access.ready() && world.weather.is_some() && listener.is_some();
    let baseline = active && !state.observer.active();
    let live = state.observer.observe(active, event);
    if event.is_none() || !active || baseline {
        state.event = None;
        state.initial_age = 2.0;
    }
    let falloff = event.zip(listener).map_or(0.0, |(event, listener)| {
        flash_falloff(
            listener
                .translation
                .as_dvec3()
                .distance(bevy::math::DVec3::from_array(event.position)) as f32,
        )
    });
    if let Some(event) = live {
        state.event = Some((event.id, event.at.to_bits()));
        state.started = world.time.elapsed_secs_f64();
        state.initial_age = world
            .weather
            .as_ref()
            .and_then(|weather| {
                event.real_age(
                    weather.elapsed_total,
                    world
                        .clock
                        .as_ref()
                        .map_or(1440.0, |clock| clock.day_length_seconds),
                )
            })
            .unwrap_or(2.0);
        if state.initial_age >= 0.0
            && state.initial_age < 0.65
            && falloff > 0.0
            && world.config.client.weather_quality != WeatherQuality::Off
        {
            if let Some(entity) = state.bolt.take() {
                commands.entity(entity).despawn();
            }
            if let Some(entity) = state.light.take() {
                commands.entity(entity).despawn();
            }
            let end = Vec3::from_array(event.position.map(|v| v as f32));
            let mesh = bolt_mesh(event.id, end, event.ground_strike);
            let handle = if let Some(handle) = &state.mesh {
                if let Some(mut asset) = meshes.get_mut(handle) {
                    *asset = mesh;
                }
                handle.clone()
            } else {
                let handle = meshes.add(mesh);
                state.mesh = Some(handle.clone());
                handle
            };
            let material = state
                .material
                .get_or_insert_with(|| {
                    materials.add(StandardMaterial {
                        unlit: true,
                        fog_enabled: false,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    })
                })
                .clone();
            state.bolt = Some(
                commands
                    .spawn((
                        Name::new("Authoritative lightning bolt"),
                        Mesh3d(handle),
                        MeshMaterial3d(material),
                        Transform::default(),
                        NotShadowCaster,
                        NotShadowReceiver,
                        NoFrustumCulling,
                    ))
                    .id(),
            );
            state.light = Some(
                commands
                    .spawn((
                        Name::new("Lightning flash"),
                        PointLight {
                            color: Color::srgb_from_array(lightning_tint(event.id).to_array()),
                            intensity: 0.0,
                            range: 220.0,
                            radius: 12.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(if event.ground_strike {
                            end + Vec3::Y * 14.0
                        } else {
                            Vec3::new(end.x, 402.0, end.z)
                        }),
                    ))
                    .id(),
            );
        }
    }
    let age = state.initial_age + (world.time.elapsed_secs_f64() - state.started) as f32;
    let flash = if state.event.is_some()
        && event.is_some()
        && world.config.client.weather_quality != WeatherQuality::Off
    {
        flash_strength(age) * falloff
    } else {
        0.0
    };
    visual.flash = flash;
    status.lightning_flash = flash;
    if let Some(entity) = state.light {
        commands.entity(entity).insert(PointLight {
            color: Color::srgb_from_array(
                lightning_tint(event.map_or(0, |event| event.id)).to_array(),
            ),
            intensity: flash * 4_000_000.0,
            range: 220.0,
            radius: 12.0,
            shadow_maps_enabled: false,
            ..default()
        });
    }
    if flash <= 0.0 {
        if let Some(entity) = state.bolt.take() {
            commands.entity(entity).despawn();
        }
        if let Some(entity) = state.light.take() {
            commands.entity(entity).despawn();
        }
    }
    if let Some(mut material) = state
        .material
        .as_ref()
        .and_then(|handle| materials.get_mut(handle))
    {
        // Unlit PBR ignores emissive and would fog this distant sky mesh into
        // a dark wire. A local HDR core matches the direct-output cloud shader.
        material.base_color = Color::LinearRgba(LinearRgba::new(4.0, 4.0, 4.0, flash));
    }
    status.lightning_bolts = usize::from(state.bolt.is_some());
}

fn noise(seed: u64, index: u64) -> f32 {
    let mut v = seed ^ index.wrapping_mul(0x9e3779b97f4a7c15);
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    ((v >> 40) as f32 / 16_777_216.0) * 2.0 - 1.0
}

pub(super) fn lightning_tint(seed: u64) -> Vec3 {
    if noise(seed, 139) > 0.25 {
        Vec3::new(0.83, 0.72, 1.0)
    } else {
        Vec3::new(0.93, 0.95, 1.0)
    }
}

fn bolt_path(seed: u64, ground: Vec3, ground_strike: bool) -> Vec<Vec3> {
    let (start, end) = if ground_strike {
        (
            Vec3::new(
                ground.x + noise(seed, 0) * 25.0,
                ground.y.max(355.0) + 90.0,
                ground.z + noise(seed, 1) * 25.0,
            ),
            ground,
        )
    } else {
        let angle = noise(seed, 0) * std::f32::consts::PI;
        let across = Vec3::new(angle.cos(), 0.0, angle.sin());
        let center = Vec3::new(ground.x, 392.0, ground.z);
        (
            center - across * (95.0 + noise(seed, 1) * 35.0),
            center + across * (115.0 + noise(seed, 2) * 45.0) + Vec3::Y * noise(seed, 3) * 18.0,
        )
    };
    (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            start.lerp(end, t)
                + Vec3::new(
                    noise(seed, i + 4) * 12.0,
                    if ground_strike {
                        0.0
                    } else {
                        noise(seed, i + 45) * 8.0
                    },
                    noise(seed, i + 24) * 12.0,
                ) * (std::f32::consts::PI * t).sin()
        })
        .collect()
}

fn bolt_mesh(seed: u64, ground: Vec3, ground_strike: bool) -> Mesh {
    let points = bolt_path(seed, ground, ground_strike);
    let mut segments: Vec<_> = points.windows(2).map(|p| (p[0], p[1], 1.0)).collect();
    for branch in 0..3_u64 {
        let origin = points[5 + branch as usize * 4];
        let branch_end = origin
            + Vec3::new(
                noise(seed, branch + 60) * 42.0,
                if ground_strike {
                    -25.0 - branch as f32 * 12.0
                } else {
                    noise(seed, branch + 67) * 15.0
                },
                noise(seed, branch + 65) * 42.0,
            );
        let mut previous = origin;
        for i in 1..=5_u64 {
            let p = origin.lerp(branch_end, i as f32 / 5.0)
                + Vec3::new(
                    noise(seed, branch * 8 + i + 70) * 4.0,
                    0.0,
                    noise(seed, branch * 8 + i + 100) * 4.0,
                );
            segments.push((previous, p, 0.45));
            previous = p;
        }
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    let widths = if ground_strike {
        [(0.65, 0.18), (0.17, 1.0)]
    } else {
        [(1.2, 0.18), (0.35, 1.0)]
    };
    for (width, alpha) in widths {
        for &(a, b, taper) in &segments {
            let axis = (b - a).normalize_or_zero();
            let side = axis.cross(Vec3::X).normalize_or_zero();
            for normal in [side, axis.cross(side)] {
                let first = positions.len() as u32;
                for p in [
                    a - normal * width * taper,
                    b - normal * width * taper,
                    b + normal * width * taper,
                    a + normal * width * taper,
                ] {
                    positions.push(p.to_array());
                    normals.push(Vec3::Y.to_array());
                    let tint = if alpha < 1.0 {
                        lightning_tint(seed)
                    } else {
                        Vec3::new(0.98, 0.97, 1.0)
                    };
                    colors.push([tint.x, tint.y, tint.z, alpha]);
                }
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_is_local_and_fades_smoothly_before_distant_players() {
        assert_eq!(flash_falloff(0.0), 1.0);
        assert_eq!(flash_falloff(384.0), 1.0);
        assert!(flash_falloff(800.0) < 1.0 && flash_falloff(800.0) > flash_falloff(1200.0));
        assert_eq!(flash_falloff(1600.0), 0.0);
        assert_eq!(flash_falloff(100_000.0), 0.0);
    }
    #[test]
    fn bolt_geometry_is_bounded_valid_and_ends_on_authoritative_surface() {
        let mesh = bolt_mesh(77, Vec3::new(-20.0, 75.0, 10.0), true);
        assert_eq!(mesh.count_vertices(), 35 * 2 * 2 * 4);
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|index| index < mesh.count_vertices())
        );
        assert_eq!(flash_strength(-1.0), 0.0);
        assert_eq!(flash_strength(0.0), 1.0);
        assert!(flash_strength(0.18) > 0.6);
        assert_eq!(flash_strength(0.66), 0.0);
    }

    #[test]
    fn cloud_lightning_spreads_horizontally_and_ground_strikes_keep_their_endpoint() {
        let ground = Vec3::new(-20.0, 75.0, 10.0);
        for seed in 0..32 {
            let cloud = bolt_path(seed, ground, false);
            let span = cloud[20] - cloud[0];
            assert!(Vec2::new(span.x, span.z).length() > 120.0);
            assert!(cloud.iter().all(|p| (365.0..435.0).contains(&p.y)));
            assert!(bolt_path(seed, ground, true)[20].distance(ground) < 0.001);
            let mesh = bolt_mesh(seed, ground, false);
            assert_eq!(mesh.count_vertices(), 35 * 2 * 2 * 4);
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
        assert!((0..32).any(|seed| lightning_tint(seed).y < 0.8));
        assert!((0..32).any(|seed| lightning_tint(seed).y > 0.9));
    }
}
