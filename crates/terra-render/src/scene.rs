use bevy::camera::{Exposure, Hdr};
use bevy::prelude::*;
use bevy::render::occlusion_culling::OcclusionCulling;
use bevy::{
    light::{Atmosphere, AtmosphereEnvironmentMapLight, atmosphere::ScatteringMedium},
    pbr::AtmosphereSettings,
};
use terra_core::GameConfig;
use terra_player::TerraCamera;
use terra_world::streaming::ChunkFocus;
pub(crate) mod caves;
pub(crate) mod daylight;
pub(crate) mod horizon;

pub(crate) const SURFACE_FOG: Vec3 = Vec3::new(0.58, 0.73, 0.87);
pub(crate) const SURFACE_AMBIENT: f32 = 760.0;
pub(crate) const SURFACE_EXPOSURE: f32 = 13.7;
// Keep sunlit turf below the washed-out ACES shoulder without reducing the
// camera's night/cave exposure or the indirect fill in terrain shadows.
// Startup and the live celestial cycle must use the same key-light energy.
pub(crate) const SURFACE_SUN_ILLUMINANCE: f32 = 65_000.0;
pub(crate) const SURFACE_TEMPERATURE: f32 = 0.006;
pub(crate) const SURFACE_SATURATION: f32 = 1.035;

pub(crate) fn surface_fog_range(range: f32, distance: f32) -> (f32, f32) {
    if distance == 0.0 {
        // Keep finite coefficients for smooth underwater transitions, outside the far plane.
        (range * 64.0, range * 128.0)
    } else {
        // The last visible cells must meet fully fogged background before the
        // finite streaming radius, even when the preference exceeds 100%.
        let start = if distance <= 1.0 {
            0.65 * distance
        } else {
            0.65 + (distance - 1.0) * 0.46
        };
        (range * start, range * 0.93 * distance.min(1.0))
    }
}

pub(crate) fn surface_grading() -> bevy::render::view::ColorGrading {
    use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
    ColorGrading {
        global: ColorGradingGlobal {
            temperature: SURFACE_TEMPERATURE,
            post_saturation: SURFACE_SATURATION,
            ..default()
        },
        shadows: ColorGradingSection {
            gain: 1.2,
            contrast: 1.025,
            saturation: 1.015,
            ..default()
        },
        midtones: ColorGradingSection {
            contrast: 1.035,
            saturation: 1.055,
            ..default()
        },
        highlights: ColorGradingSection {
            saturation: 0.99,
            ..default()
        },
    }
}

pub(crate) fn setup_scene(
    mut commands: Commands,
    config: Res<GameConfig>,
    focus: Res<ChunkFocus>,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
) {
    let earth = mediums.add(ScatteringMedium::earth(256, 256));
    commands.spawn(Atmosphere {
        // Finite chunk streaming exposes the atmosphere's planetary ground
        // below the horizon. Match the cool distance haze, not terrain green.
        ground_albedo: Vec3::new(0.13, 0.27, 0.46),
        ..Atmosphere::earth(earth)
    });
    let position = Vec3::from_array(focus.0.map(|v| v as f32));
    let range = f32::from(config.client.view_distance.max(1)) * 16.0;
    let (fog_start, fog_end) = surface_fog_range(range, config.client.fog_distance);
    let camera = commands
        .spawn((
            Camera3d::default(),
            (Hdr, surface_grading()),
            bevy::core_pipeline::prepass::DepthPrepass,
            bevy::pbr::ScreenSpaceTransmission {
                steps: 1,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                far: range + 16.0,
                fov: config.client.fov_degrees.to_radians(),
                ..default()
            }),
            AtmosphereSettings::default(),
            AtmosphereEnvironmentMapLight {
                intensity: 1.05,
                ..default()
            },
            AmbientLight {
                color: Color::srgb(0.86, 0.92, 1.0),
                brightness: SURFACE_AMBIENT,
                ..default()
            },
            bevy::light::ShadowFilteringMethod::Gaussian,
            Exposure {
                ev100: SURFACE_EXPOSURE,
            },
            bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
            bevy::ui::IsDefaultUiCamera,
            TerraCamera,
            Transform::from_translation(position)
                .looking_at(position + Vec3::new(-4.0, -3.5, -12.0), Vec3::Y),
            DistanceFog {
                color: Color::srgb(SURFACE_FOG.x, SURFACE_FOG.y, SURFACE_FOG.z),
                falloff: FogFalloff::Linear {
                    start: fog_start,
                    end: fog_end,
                },
                ..default()
            },
        ))
        .id();
    if use_occlusion_culling(config.client.view_distance) {
        commands.entity(camera).insert(OcclusionCulling);
    }
    commands.spawn((
        daylight::Sun,
        bevy::light::SunDisk::OFF,
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.84),
            illuminance: SURFACE_SUN_ILLUMINANCE,
            shadow_maps_enabled: true,
            ..default()
        },
        bevy::camera::visibility::RenderLayers::default()
            .with(crate::meshing::shadows::LAYER)
            .with(crate::transitions::QUEUE_SHADOW_LAYER),
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: (range * 0.25).min(24.0),
            maximum_distance: crate::graphics::shadow_distance(
                range,
                terra_core::ShadowQuality::High,
            ),
            overlap_proportion: 0.2,
            ..default()
        }
        .build(),
        Transform::from_xyz(40.0, 80.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    daylight::moon(&mut commands, range);
}

pub(crate) fn update_view_distance(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut cameras: Query<
        (
            Entity,
            &mut Projection,
            &mut DistanceFog,
            Has<OcclusionCulling>,
        ),
        With<TerraCamera>,
    >,
) {
    if !config.is_changed() {
        return;
    }
    let range = f32::from(config.client.view_distance.max(1)) * 16.0;
    let (fog_start, fog_end) = surface_fog_range(range, config.client.fog_distance);
    let occlusion = use_occlusion_culling(config.client.view_distance);
    for (entity, mut projection, mut fog, has_occlusion) in &mut cameras {
        if occlusion != has_occlusion {
            if occlusion {
                commands.entity(entity).insert(OcclusionCulling);
            } else {
                commands.entity(entity).remove::<OcclusionCulling>();
            }
        }
        if let Projection::Perspective(p) = &mut *projection {
            p.far = range + 16.0;
        }
        fog.falloff = FogFalloff::Linear {
            start: fog_start,
            end: fog_end,
        };
    }
}

fn use_occlusion_culling(view_distance: u8) -> bool {
    // Small views are CPU-bound and gain too little rejection to amortize the
    // second preprocessing phase. Re-evaluate only when settings change.
    view_distance >= 12
}

#[cfg(test)]
mod tests {
    #[test]
    fn occlusion_budget_keeps_small_views_on_the_single_phase_path() {
        assert!(!super::use_occlusion_culling(8));
        assert!(!super::use_occlusion_culling(11));
        assert!(super::use_occlusion_culling(12));
        assert!(super::use_occlusion_culling(32));
    }
    #[test]
    fn changing_view_distance_updates_the_existing_camera_culling_path() {
        use super::*;
        let mut app = App::new();
        let mut config = GameConfig::default();
        config.client.view_distance = 8;
        app.insert_resource(config)
            .add_systems(Update, update_view_distance);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Projection::default(), DistanceFog::default()))
            .id();
        app.update();
        assert!(app.world().get::<OcclusionCulling>(camera).is_none());
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 32;
        app.update();
        assert!(app.world().get::<OcclusionCulling>(camera).is_some());
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 8;
        app.update();
        assert!(app.world().get::<OcclusionCulling>(camera).is_none());
    }
    #[test]
    fn surface_fog_finishes_before_the_finite_streamed_edge_at_every_preference() {
        for range in [32.0, 192.0, 512.0] {
            for distance in [0.1, 0.5, 1.0, 1.5] {
                let (start, end) = super::surface_fog_range(range, distance);
                assert!(start >= 0.0 && end > start && end < range);
            }
            let (start, end) = super::surface_fog_range(range, 0.0);
            assert!(start > range && end > start);
        }
        assert!(super::surface_fog_range(192.0, 1.4).0 < super::surface_fog_range(192.0, 1.5).0);
    }
    #[test]
    fn shadow_lift_is_twenty_percent_without_global_exposure_change() {
        let grading = super::surface_grading();
        assert_eq!(grading.shadows.gain, 1.2);
        assert_eq!(grading.midtones.gain, 1.0);
        assert_eq!(grading.highlights.gain, 1.0);
        assert_eq!(grading.global.exposure, 0.0);
    }
}
