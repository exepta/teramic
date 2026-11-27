use bevy::{
    light::{FogVolume, VolumetricFog, VolumetricLight},
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
    prelude::*,
};
use terra_core::{CaveQuality, GraphicsConfig};
use terra_player::TerraCamera;

#[derive(Component)]
struct CaveAir;
#[derive(Component)]
pub(crate) struct CaveCamera;
#[derive(Component)]
pub(crate) struct CaveSun;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ApplyCaveQuality;

type Cameras<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Msaa,
        Option<&'static ScreenSpaceAmbientOcclusion>,
        Has<VolumetricFog>,
        Has<CaveCamera>,
        Has<crate::water::mist::MistCamera>,
    ),
    With<TerraCamera>,
>;
type Air<'w, 's> = Query<
    'w,
    's,
    (&'static mut FogVolume, &'static mut Transform),
    (With<CaveAir>, Without<TerraCamera>),
>;
type EnvironmentEntities<'w, 's> = (
    Query<'w, 's, Entity, With<CaveAir>>,
    Query<
        'w,
        's,
        (
            Entity,
            Has<VolumetricLight>,
            Has<CaveSun>,
            Has<crate::water::mist::MistSun>,
        ),
        With<super::super::daylight::Sun>,
    >,
    Query<'w, 's, (), With<bevy::post_process::bloom::Bloom>>,
);

pub(super) fn install(app: &mut App) {
    app.add_systems(
        Update,
        update
            .after(super::probe)
            .after(crate::graphics::apply)
            .in_set(ApplyCaveQuality),
    );
}

fn update(
    mut commands: Commands,
    cave: Res<super::CaveState>,
    graphics: Option<Res<GraphicsConfig>>,
    underwater: Res<crate::water::UnderwaterState>,
    cameras: Cameras,
    mut air: Air,
    (volumes, sun, bloom): EnvironmentEntities,
) {
    let quality = graphics
        .as_ref()
        .map_or(CaveQuality::Medium, |g| g.cave_quality);
    let active = cave.blend > 0.02;
    let mut volume = None;
    for (entity, transform, msaa, ssao, fog, owned, mist) in &cameras {
        let use_bloom = active && matches!(quality, CaveQuality::High | CaveQuality::Ultra);
        if use_bloom && !bloom.contains(entity) {
            commands
                .entity(entity)
                .insert(bevy::post_process::bloom::Bloom {
                    intensity: 0.04,
                    low_frequency_boost: 0.0,
                    max_mip_dimension: 512,
                    ..default()
                });
        } else if !use_bloom && bloom.contains(entity) {
            commands
                .entity(entity)
                .remove::<bevy::post_process::bloom::Bloom>();
        }
        let desired = if active && *msaa == Msaa::Off {
            match quality {
                CaveQuality::High => Some(ScreenSpaceAmbientOcclusionQualityLevel::Medium),
                CaveQuality::Ultra => Some(ScreenSpaceAmbientOcclusionQualityLevel::High),
                _ => None,
            }
        } else {
            None
        };
        if desired != ssao.map(|s| s.quality_level) {
            if let Some(quality_level) = desired {
                commands.entity(entity).insert(ScreenSpaceAmbientOcclusion {
                    quality_level,
                    constant_object_thickness: 0.35,
                    ..default()
                });
            } else {
                commands.entity(entity).remove::<(
                    ScreenSpaceAmbientOcclusion,
                    bevy::core_pipeline::prepass::NormalPrepass,
                )>();
            }
        }
        let volumetric = active
            && cave.roof_coverage > 0.02
            && !cave.inside_lair
            && cave.gallery_air() > 0.001
            && underwater.blend < 0.01
            && graphics
                .as_deref()
                .is_none_or(|g| g.shadow_quality != terra_core::ShadowQuality::Off)
            && cave.beam_depths.max_element() > 0.0
            && cave.ceiling >= 8.0
            && quality == CaveQuality::Ultra;
        // Waterfall mist shares these Bevy components, but owns a separate claim.
        if volumetric && (!owned || !fog) {
            commands.entity(entity).insert((
                CaveCamera,
                VolumetricFog {
                    step_count: 32,
                    ambient_intensity: 0.0,
                    jitter: 0.0,
                    ..default()
                },
            ));
        } else if !volumetric && owned {
            commands.entity(entity).remove::<CaveCamera>();
            if !mist {
                commands.entity(entity).remove::<VolumetricFog>();
            }
        }
        if volumetric {
            volume = Some(transform.translation);
        }
    }
    if let Some(position) = volume {
        for (entity, volumetric, owned, _) in &sun {
            if !owned {
                commands.entity(entity).insert(CaveSun);
            }
            if !volumetric {
                commands.entity(entity).insert(VolumetricLight);
            }
        }
        let density = 0.006 * cave.blend * cave.salt_skylight * cave.gallery_air();
        if let Some(entity) = volumes.iter().next() {
            if let Ok((mut fog, mut transform)) = air.get_mut(entity) {
                fog.density_factor = density;
                transform.translation = position;
            }
        } else {
            commands.spawn((
                CaveAir,
                Name::new("Bounded cavern air"),
                FogVolume {
                    density_factor: density,
                    scattering: 0.15,
                    absorption: 0.1,
                    light_intensity: 0.4,
                    ..default()
                },
                Transform::from_translation(position).with_scale(Vec3::new(96.0, 48.0, 96.0)),
            ));
        }
    } else {
        for (entity, volumetric, owned, mist) in &sun {
            if owned {
                commands.entity(entity).remove::<CaveSun>();
                if volumetric && !mist {
                    commands.entity(entity).remove::<VolumetricLight>();
                }
            }
        }
        for entity in &volumes {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_switches_real_components_and_cleans_them_on_exit() {
        let mut app = App::new();
        app.insert_resource(super::super::CaveState {
            roof_coverage: 1.,
            blend: 1.0,
            ceiling: 12.0,
            beam_depths: Vec4::splat(12.),
            salt_skylight: 1.,
            ..default()
        })
        .init_resource::<GraphicsConfig>()
        .init_resource::<crate::water::UnderwaterState>()
        .add_systems(Update, update);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Transform::default(), Msaa::Off))
            .id();
        let sun = app
            .world_mut()
            .spawn(super::super::super::daylight::Sun)
            .id();
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .cave_quality = CaveQuality::High;
        app.update();
        assert!(
            app.world()
                .get::<ScreenSpaceAmbientOcclusion>(camera)
                .is_some()
        );
        assert!(
            app.world()
                .get::<bevy::post_process::bloom::Bloom>(camera)
                .is_some()
        );
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .cave_quality = CaveQuality::Ultra;
        app.update();
        assert!(app.world().get::<VolumetricFog>(camera).is_some());
        assert!(app.world().get::<VolumetricLight>(sun).is_some());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<CaveAir>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut()
            .resource_mut::<super::super::CaveState>()
            .lair_blend = 1.;
        app.update();
        assert!(app.world().get::<VolumetricFog>(camera).is_none());
        assert!(app.world().get::<VolumetricLight>(sun).is_none());
        assert!(
            app.world_mut()
                .query::<&CaveAir>()
                .iter(app.world())
                .next()
                .is_none()
        );
        app.world_mut()
            .resource_mut::<super::super::CaveState>()
            .lair_blend = 0.;
        app.update();
        assert!(app.world().get::<VolumetricFog>(camera).is_some());
        app.world_mut()
            .resource_mut::<super::super::CaveState>()
            .beam_depths = Vec4::ZERO;
        app.update();
        assert!(app.world().get::<VolumetricLight>(sun).is_none());
        assert!(app.world().get::<VolumetricFog>(camera).is_none());
        assert!(
            app.world()
                .get::<ScreenSpaceAmbientOcclusion>(camera)
                .is_some()
        );
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .cave_quality = CaveQuality::Low;
        app.update();
        assert!(
            app.world()
                .get::<ScreenSpaceAmbientOcclusion>(camera)
                .is_none()
        );
        assert!(
            app.world()
                .get::<bevy::core_pipeline::prepass::NormalPrepass>(camera)
                .is_none()
        );
        assert!(app.world().get::<VolumetricFog>(camera).is_none());
        assert!(app.world().get::<VolumetricLight>(sun).is_none());
        assert!(
            app.world()
                .get::<bevy::post_process::bloom::Bloom>(camera)
                .is_none()
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<CaveAir>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn underwater_and_msaa_transitions_release_only_incompatible_cave_passes() {
        let mut app = App::new();
        app.insert_resource(super::super::CaveState {
            roof_coverage: 1.,
            blend: 1.0,
            ceiling: 12.0,
            beam_depths: Vec4::splat(12.),
            salt_skylight: 1.,
            ..default()
        })
        .insert_resource(GraphicsConfig {
            cave_quality: CaveQuality::Ultra,
            ..default()
        })
        .init_resource::<crate::water::UnderwaterState>()
        .add_systems(Update, update);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Transform::default(), Msaa::Off))
            .id();
        let sun = app
            .world_mut()
            .spawn(super::super::super::daylight::Sun)
            .id();
        for (blend, msaa, underwater) in [
            (1.0, Msaa::Off, 0.0),
            (1.0, Msaa::Sample4, 0.0),
            (1.0, Msaa::Off, 1.0),
            (1.0, Msaa::Off, 0.0),
            (0.0, Msaa::Off, 0.0),
            (1.0, Msaa::Off, 0.0),
        ] {
            app.world_mut()
                .resource_mut::<super::super::CaveState>()
                .blend = blend;
            app.world_mut()
                .resource_mut::<crate::water::UnderwaterState>()
                .blend = underwater;
            app.world_mut().entity_mut(camera).insert(msaa);
            app.update();
            let volume = blend > 0.02 && underwater < 0.01;
            assert_eq!(app.world().get::<VolumetricFog>(camera).is_some(), volume);
            assert_eq!(app.world().get::<VolumetricLight>(sun).is_some(), volume);
            assert_eq!(
                app.world_mut()
                    .query_filtered::<Entity, With<CaveAir>>()
                    .iter(app.world())
                    .count(),
                usize::from(volume)
            );
            let ssao = blend > 0.02 && msaa == Msaa::Off;
            assert_eq!(
                app.world()
                    .get::<ScreenSpaceAmbientOcclusion>(camera)
                    .is_some(),
                ssao
            );
            assert_eq!(
                app.world()
                    .get::<bevy::core_pipeline::prepass::NormalPrepass>(camera)
                    .is_some(),
                ssao
            );
        }
    }

    #[test]
    fn cave_and_waterfall_mist_share_view_and_sun_without_stealing_each_others_claims() {
        use crate::water::mist::{MistAssets, MistCamera, MistSun};
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<crate::water::WaterStatus>()
            .init_resource::<crate::water::UnderwaterState>()
            .init_resource::<MistAssets>()
            .init_resource::<Assets<Image>>()
            .insert_resource(GraphicsConfig {
                cave_quality: CaveQuality::Ultra,
                ..default()
            })
            .insert_resource(super::super::CaveState {
                roof_coverage: 1.,
                ceiling: 12.0,
                beam_depths: Vec4::splat(12.),
                salt_skylight: 1.,
                ..default()
            })
            // Register the downstream writer first; the explicit set must
            // flush cave claims before mist reads them in the same frame.
            .add_systems(Update, crate::water::mist::update.after(ApplyCaveQuality))
            .add_systems(Update, update.in_set(ApplyCaveQuality));
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                Transform::default(),
                GlobalTransform::default(),
                Msaa::Off,
            ))
            .id();
        let sun = app
            .world_mut()
            .spawn((
                super::super::super::daylight::Sun,
                DirectionalLight::default(),
            ))
            .id();
        app.world_mut().spawn((
            crate::water::spray::SpraySources(vec![Vec4::new(1.0, 0.0, 0.0, 12.0)]),
            GlobalTransform::default(),
        ));
        for (cave, mist) in [
            (false, false),
            (false, true),
            (true, true),
            (false, true),
            (true, true),
            (true, false),
            (true, true),
            (false, false),
            (true, false),
            (false, true),
            (false, false),
        ] {
            app.world_mut()
                .resource_mut::<super::super::CaveState>()
                .blend = if cave { 1.0 } else { 0.0 };
            app.world_mut()
                .resource_mut::<terra_core::GameConfig>()
                .client
                .water_volumetric_mist = mist;
            let mut previous_ticks = None;
            for _ in 0..3 {
                app.update();
                assert_eq!(app.world().get::<CaveCamera>(camera).is_some(), cave);
                assert_eq!(app.world().get::<MistCamera>(camera).is_some(), mist);
                assert_eq!(app.world().get::<CaveSun>(sun).is_some(), cave);
                assert_eq!(app.world().get::<MistSun>(sun).is_some(), mist);
                assert_eq!(
                    app.world().get::<VolumetricLight>(sun).is_some(),
                    cave || mist
                );
                let fog = app.world().get::<VolumetricFog>(camera);
                assert_eq!(fog.is_some(), cave || mist);
                if let Some(fog) = fog {
                    assert_eq!(fog.step_count, if cave { 32 } else { 24 });
                    assert_eq!(fog.ambient_intensity, if cave { 0.0 } else { 0.12 });
                }
                assert_eq!(
                    app.world_mut()
                        .query::<&FogVolume>()
                        .iter(app.world())
                        .count(),
                    usize::from(cave) + usize::from(mist)
                );
                assert!(app.world().resource::<Assets<Image>>().len() <= 1);
                let ticks = (
                    app.world_mut()
                        .query::<Ref<VolumetricFog>>()
                        .get(app.world(), camera)
                        .ok()
                        .map(|value| value.last_changed()),
                    app.world_mut()
                        .query::<Ref<VolumetricLight>>()
                        .get(app.world(), sun)
                        .ok()
                        .map(|value| value.last_changed()),
                );
                if let Some(previous) = previous_ticks {
                    assert_eq!(
                        ticks, previous,
                        "unchanged claims must not rewrite shared components"
                    );
                }
                previous_ticks = Some(ticks);
            }
        }
    }
}
