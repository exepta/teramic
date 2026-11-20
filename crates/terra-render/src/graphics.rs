use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::Exposure,
    light::{CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap},
    prelude::*,
};
use terra_core::{GameConfig, GraphicsConfig, ShadowQuality};

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        Update,
        apply_block_details
            .after(crate::blocks::poll_blocks)
            .before(crate::meshing::dispatch_meshes),
    );
    use bevy::render::{Render, RenderApp, RenderSystems};
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            Render,
            clear_stale_background.in_set(RenderSystems::PrepareBindGroups),
        );
    }
}

fn apply_block_details(
    config: Res<GameConfig>,
    mut assets: ResMut<crate::blocks::BlockAssets>,
    cache: Option<ResMut<crate::meshing::GeometryCache>>,
) {
    let Some(templates) = assets.templates.as_ref() else {
        return;
    };
    let settings = config.client.blocks;
    if templates.block_details == settings {
        return;
    }
    let geometry_changed = templates.block_details.detail_quality != settings.detail_quality
        || templates.block_details.leaf_geometry_quality != settings.leaf_geometry_quality
        || templates.block_details.micro_geometry != settings.micro_geometry
        || templates.block_details.micro_geometry_distance != settings.micro_geometry_distance;
    let geometry_changed =
        geometry_changed || templates.block_details.surface_variation != settings.surface_variation;
    if let Some(templates) = assets.templates.as_mut() {
        std::sync::Arc::make_mut(templates).block_details = settings;
    }
    if geometry_changed && let Some(mut cache) = cache {
        cache.invalidate_details();
    }
}

// Bevy 0.19 leaves these prepared components behind when TAA's motion prepass
// is removed. A later depth-only pass must not bind the old MRT pipeline.
fn clear_stale_background(
    mut commands: Commands,
    views: Query<
        Entity,
        (
            With<bevy::core_pipeline::prepass::BackgroundMotionVectorsPipelineId>,
            Without<bevy::core_pipeline::prepass::MotionVectorPrepass>,
        ),
    >,
) {
    for entity in &views {
        commands.entity(entity).remove::<(
            bevy::core_pipeline::prepass::BackgroundMotionVectorsPipelineId,
            bevy::core_pipeline::prepass::BackgroundMotionVectorsBindGroup,
        )>();
    }
}

pub(crate) fn reset_temporal_history(
    mut cameras: Query<(&Transform, &mut TemporalAntiAliasing), With<terra_player::TerraCamera>>,
    clock: Option<Res<terra_core::WorldClock>>,
    mut previous: Local<Option<(Transform, f64)>>,
) {
    let phase = clock.map_or(0.5, |clock| clock.phase);
    for (transform, mut taa) in &mut cameras {
        if let Some((old, old_phase)) = previous.as_ref() {
            let phase_distance = (phase - old_phase).abs();
            if old.translation.distance_squared(transform.translation) > 64.0
                || old.rotation.dot(transform.rotation).abs() < 0.7
                || phase_distance.min(1.0 - phase_distance) > 0.005
            {
                taa.reset = true;
            }
        }
        *previous = Some((*transform, phase));
    }
}

pub(crate) fn apply(
    mut commands: Commands,
    graphics: Option<Res<GraphicsConfig>>,
    config: Res<GameConfig>,
    mut cameras: Query<
        (Entity, &mut Exposure, Option<&TemporalAntiAliasing>),
        With<terra_player::TerraCamera>,
    >,
    mut sun: Query<
        (&mut DirectionalLight, &mut CascadeShadowConfig),
        Without<crate::previews::PreviewScene>,
    >,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    daylight: Option<Res<crate::scene::daylight::Daylight>>,
) {
    let Some(graphics) = graphics else {
        return;
    };
    if !graphics.is_changed() && !config.is_changed() {
        return;
    }
    for (entity, mut exposure, taa) in &mut cameras {
        exposure.ev100 = daylight
            .as_ref()
            .map_or(crate::scene::SURFACE_EXPOSURE, |d| d.exposure)
            - graphics.brightness.log2();
        let temporal = matches!(
            graphics.shadow_quality,
            ShadowQuality::Medium | ShadowQuality::High
        );
        if temporal && taa.is_none() {
            commands.entity(entity).insert((
                TemporalAntiAliasing::default(),
                Msaa::Off,
                bevy::light::ShadowFilteringMethod::Temporal,
            ));
        } else if !temporal && taa.is_some() {
            commands
                .entity(entity)
                .remove::<(
                    TemporalAntiAliasing,
                    bevy::render::camera::TemporalJitter,
                    bevy::render::camera::MipBias,
                    bevy::core_pipeline::prepass::MotionVectorPrepass,
                )>()
                .insert((Msaa::Sample4, bevy::light::ShadowFilteringMethod::Gaussian));
        }
    }
    let range = f32::from(config.client.view_distance.max(1)) * 16.0;
    let (enabled, size, cascades) = match graphics.shadow_quality {
        ShadowQuality::Off => (false, 512, 1),
        ShadowQuality::Low => (true, 512, 1),
        ShadowQuality::Medium => (true, 1024, if range <= 128.0 { 2 } else { 3 }),
        ShadowQuality::High => (true, 2048, if range <= 256.0 { 3 } else { 4 }),
    };
    if shadow_map.size != size {
        shadow_map.size = size;
    }
    for (mut light, mut cascade) in &mut sun {
        light.shadow_maps_enabled = enabled;
        *cascade = CascadeShadowConfigBuilder {
            num_cascades: cascades,
            first_cascade_far_bound: (range * 0.25).min(16.0),
            maximum_distance: shadow_distance(range, graphics.shadow_quality),
            overlap_proportion: 0.25,
            ..default()
        }
        .build();
    }
}

pub(crate) fn shadow_distance(range: f32, quality: ShadowQuality) -> f32 {
    range.min(match quality {
        ShadowQuality::Off | ShadowQuality::Low => 96.0,
        ShadowQuality::Medium => 192.0,
        ShadowQuality::High => 256.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shadow_cascades_follow_view_distance_with_a_bounded_quality_budget() {
        assert_eq!(shadow_distance(320.0, ShadowQuality::High), 256.0);
        assert_eq!(shadow_distance(512.0, ShadowQuality::High), 256.0);
        assert_eq!(shadow_distance(320.0, ShadowQuality::Medium), 192.0);
        assert_eq!(shadow_distance(320.0, ShadowQuality::Low), 96.0);
        assert_eq!(shadow_distance(64.0, ShadowQuality::High), 64.0);
    }

    #[test]
    fn disabling_motion_prepass_removes_its_stale_prepared_background_pipeline() {
        use bevy::core_pipeline::prepass::{
            BackgroundMotionVectorsPipelineId, MotionVectorPrepass,
        };
        use bevy::render::render_resource::CachedRenderPipelineId;
        let mut app = App::new();
        app.add_systems(Update, clear_stale_background);
        let stale = app
            .world_mut()
            .spawn(BackgroundMotionVectorsPipelineId(
                CachedRenderPipelineId::INVALID,
            ))
            .id();
        let active = app
            .world_mut()
            .spawn((
                BackgroundMotionVectorsPipelineId(CachedRenderPipelineId::INVALID),
                MotionVectorPrepass,
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<BackgroundMotionVectorsPipelineId>(stale)
                .is_none()
        );
        assert!(
            app.world()
                .get::<BackgroundMotionVectorsPipelineId>(active)
                .is_some()
        );
        app.world_mut()
            .entity_mut(active)
            .remove::<MotionVectorPrepass>();
        app.update();
        assert!(
            app.world()
                .get::<BackgroundMotionVectorsPipelineId>(active)
                .is_none()
        );
    }

    #[test]
    fn quality_and_brightness_update_real_camera_and_sun_without_gpu() {
        let mut app = App::new();
        app.init_resource::<GraphicsConfig>()
            .init_resource::<GameConfig>()
            .init_resource::<DirectionalLightShadowMap>()
            .add_systems(Update, apply);
        let camera = app
            .world_mut()
            .spawn((
                terra_player::TerraCamera,
                Exposure::default(),
                Msaa::Sample4,
                bevy::core_pipeline::prepass::DepthPrepass,
            ))
            .id();
        let sun = app
            .world_mut()
            .spawn((DirectionalLight::default(), CascadeShadowConfig::default()))
            .id();
        for (quality, enabled, size, count) in [
            (ShadowQuality::Off, false, 512, 1),
            (ShadowQuality::Low, true, 512, 1),
            (ShadowQuality::Medium, true, 1024, 2),
            (ShadowQuality::High, true, 2048, 3),
            (ShadowQuality::Low, true, 512, 1),
        ] {
            let mut graphics = app.world_mut().resource_mut::<GraphicsConfig>();
            graphics.shadow_quality = quality;
            graphics.brightness = 1.5;
            app.update();
            let temporal = matches!(quality, ShadowQuality::Medium | ShadowQuality::High);
            assert_eq!(
                app.world().get::<TemporalAntiAliasing>(camera).is_some(),
                temporal
            );
            if temporal {
                assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Off));
                assert!(
                    app.world()
                        .get::<bevy::core_pipeline::prepass::MotionVectorPrepass>(camera)
                        .is_some()
                );
                assert!(
                    app.world()
                        .get::<TemporalAntiAliasing>(camera)
                        .unwrap()
                        .reset
                );
                assert!(
                    app.world()
                        .get::<bevy::render::camera::TemporalJitter>(camera)
                        .is_some()
                );
                assert!(
                    app.world()
                        .get::<bevy::render::camera::MipBias>(camera)
                        .is_some()
                );
                assert_eq!(
                    app.world()
                        .get::<bevy::light::ShadowFilteringMethod>(camera),
                    Some(&bevy::light::ShadowFilteringMethod::Temporal)
                );
            } else {
                assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
                assert!(
                    app.world()
                        .get::<bevy::render::camera::TemporalJitter>(camera)
                        .is_none()
                );
                assert!(
                    app.world()
                        .get::<bevy::render::camera::MipBias>(camera)
                        .is_none()
                );
                assert!(
                    app.world()
                        .get::<bevy::core_pipeline::prepass::MotionVectorPrepass>(camera)
                        .is_none()
                );
            }
            assert!(
                app.world()
                    .get::<bevy::core_pipeline::prepass::DepthPrepass>(camera)
                    .is_some()
            );
            assert_eq!(
                app.world().resource::<DirectionalLightShadowMap>().size,
                size
            );
            assert_eq!(
                app.world()
                    .get::<DirectionalLight>(sun)
                    .unwrap()
                    .shadow_maps_enabled,
                enabled
            );
            assert_eq!(
                app.world()
                    .get::<CascadeShadowConfig>(sun)
                    .unwrap()
                    .bounds
                    .len(),
                count
            );
            assert!(
                (app.world().get::<Exposure>(camera).unwrap().ev100
                    - (crate::scene::SURFACE_EXPOSURE - 1.5_f32.log2()))
                .abs()
                    < 1e-5
            );
        }
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .shadow_quality = ShadowQuality::High;
        for (view, count) in [(16, 3), (32, 4)] {
            app.world_mut()
                .resource_mut::<GameConfig>()
                .client
                .view_distance = view;
            app.update();
            let cascades = app.world().get::<CascadeShadowConfig>(sun).unwrap();
            assert_eq!(cascades.bounds.len(), count);
            assert!((cascades.bounds.last().copied().unwrap() - 256.0).abs() < 0.001);
            assert_eq!(
                app.world().resource::<DirectionalLightShadowMap>().size,
                2048
            );
        }
        let night = crate::scene::daylight::Daylight::at(0.0);
        app.insert_resource(night);
        app.world_mut()
            .resource_mut::<GameConfig>()
            .client
            .view_distance = 2;
        app.update();
        assert!(
            (app.world().get::<Exposure>(camera).unwrap().ev100
                - (night.exposure - 1.5_f32.log2()))
            .abs()
                < 1e-5
        );
    }

    #[test]
    fn temporal_history_keeps_slow_motion_but_resets_camera_and_clock_jumps() {
        let mut app = App::new();
        app.init_resource::<terra_core::WorldClock>()
            .add_systems(Update, reset_temporal_history);
        let camera = app
            .world_mut()
            .spawn((
                terra_player::TerraCamera,
                Transform::default(),
                TemporalAntiAliasing::default(),
            ))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<TemporalAntiAliasing>(camera)
            .unwrap()
            .reset = false;
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .x = 0.1;
        app.world_mut()
            .resource_mut::<terra_core::WorldClock>()
            .phase += 0.00001;
        app.update();
        assert!(
            !app.world()
                .get::<TemporalAntiAliasing>(camera)
                .unwrap()
                .reset
        );
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation
            .x = 50.0;
        app.update();
        assert!(
            app.world()
                .get::<TemporalAntiAliasing>(camera)
                .unwrap()
                .reset
        );
        app.world_mut()
            .get_mut::<TemporalAntiAliasing>(camera)
            .unwrap()
            .reset = false;
        app.world_mut()
            .resource_mut::<terra_core::WorldClock>()
            .phase = 0.0;
        app.update();
        assert!(
            app.world()
                .get::<TemporalAntiAliasing>(camera)
                .unwrap()
                .reset
        );
    }
}
