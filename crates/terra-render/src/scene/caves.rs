//! Cave readability: local environment blending plus a depth-aware, cave-only pass.
pub(crate) mod lights;
pub(crate) mod quality;
pub(crate) mod shader;
use bevy::{
    camera::{CameraProjection, Exposure},
    prelude::*,
    render::view::ColorGrading,
};
use terra_core::GraphicsConfig;
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

// Retain Phase 111's darker caves, with 12.5% more background readability.
const CAVE_EXPOSURE: f32 = 6.451_563;

pub(crate) fn sky_visibility(blend: f32) -> f32 {
    let open = 1.0 - blend.clamp(0.0, 1.0);
    open * open * (3.0 - 2.0 * open)
}

#[derive(Resource, Default, Debug)]
pub struct CaveState {
    pub blend: f32,
    target: f32,
    next_probe: f32,
    ceiling: f32,
}

pub(crate) fn install(app: &mut App) {
    shader::install(app);
    app.init_resource::<CaveState>()
        .register_required_components::<TerraCamera, shader::CaveSettings>()
        .add_systems(
            Update,
            probe
                .after(terra_player::PlayerSet::Movement)
                .before(crate::water::environment::update),
        )
        .add_systems(PostUpdate, exposure);
    quality::install(app);
    lights::install(app);
}

fn probe(
    time: Res<Time>,
    terrain: Res<WorldTerrain>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    (underwater, graphics): (
        Res<crate::water::UnderwaterState>,
        Option<Res<GraphicsConfig>>,
    ),
    mut state: ResMut<CaveState>,
    mut cameras: Query<(&Transform, &Projection, &mut shader::CaveSettings), With<TerraCamera>>,
) {
    for (transform, projection, mut settings) in &mut cameras {
        if time.elapsed_secs() >= state.next_probe {
            state.next_probe = time.elapsed_secs() + 0.25;
            let p = transform.translation.to_array().map(f64::from);
            let height = terrain.0.surface_height(p[0], p[2]);
            let enclosure = terra_world::enclosure::sample(p, height, |p| {
                chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b))
            });
            // Missing roof chunks are not an opening to the sky.
            if enclosure.complete || enclosure.cave >= state.target {
                state.target = enclosure.cave;
                state.ceiling = enclosure.ceiling;
            }
        }
        let blend =
            state.blend + (state.target - state.blend) * (1.0 - (-3.5 * time.delta_secs()).exp());
        let blend = if (blend - state.target).abs() < 0.001 {
            state.target
        } else {
            blend
        };
        if blend != state.blend {
            state.blend = blend;
        }
        let matrix = match projection {
            Projection::Perspective(p) => p.get_clip_from_view().inverse(),
            Projection::Orthographic(p) => p.get_clip_from_view().inverse(),
            Projection::Custom(_) => Mat4::IDENTITY,
        };
        let haze = if graphics
            .as_ref()
            .is_some_and(|g| g.cave_quality == terra_core::CaveQuality::Low)
        {
            0.0
        } else {
            ((state.ceiling - 6.0) / 18.0).clamp(0.0, 1.0) * 0.004
        };
        let parameters = Vec4::new(
            if haze > 0.0 {
                state.blend * (1.0 - underwater.blend)
            } else {
                0.0
            },
            haze,
            state.blend.powi(4) * (1.0 - underwater.blend),
            0.0,
        );
        if settings.view_from_clip != matrix || settings.parameters != parameters {
            settings.view_from_clip = matrix;
            settings.parameters = parameters;
        }
    }
}
type LitCameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Exposure,
        &'static mut bevy::light::AtmosphereEnvironmentMapLight,
        Option<&'static mut bevy::light::GeneratedEnvironmentMapLight>,
        Option<&'static mut bevy::light::EnvironmentMapLight>,
        Option<&'static mut bevy::pbr::AtmosphereSettings>,
    ),
    With<TerraCamera>,
>;
type CelestialLights<'w, 's> = Query<
    'w,
    's,
    (&'static mut DirectionalLight, Has<super::daylight::Sun>),
    Or<(With<super::daylight::Sun>, With<super::daylight::Moon>)>,
>;

fn exposure(
    day: Res<super::daylight::Daylight>,
    graphics: Option<Res<GraphicsConfig>>,
    weather: Option<Res<crate::weather::WeatherVisual>>,
    cave: Res<CaveState>,
    (session, mut ambient): (
        Option<Res<terra_network::client::ClientSession>>,
        Query<&mut AmbientLight, With<TerraCamera>>,
    ),
    mut cameras: LitCameras,
    mut celestial: CelestialLights,
) {
    if session
        .as_ref()
        .is_some_and(|s| s.game_mode == terra_core::GameMode::Spectator)
    {
        for mut fill in &mut ambient {
            fill.brightness += (100.0_f32.max(fill.brightness) - fill.brightness) * cave.blend;
            fill.color = fill.color.mix(&Color::srgb(0.94, 0.95, 1.0), cave.blend);
        }
    }
    let cave_exposure = day.exposure + (CAVE_EXPOSURE - day.exposure) * cave.blend;
    let solar_compensation = 2.0_f32.powf(cave_exposure - day.exposure);
    // Keep sunlit entrances at their outdoor luminance as eyes adapt to the
    // room; local lamps retain the increased cave readability. No sunlight
    // change outside, and no accumulating per-frame multiplication.
    for (mut light, sun) in &mut celestial {
        let cloud_attenuation = weather.as_ref().map_or(1.0, |weather| {
            1.0 - ((weather.clouds - 0.30) / 0.70).clamp(0.0, 1.0) * 0.62
        });
        // Cascaded outdoor shadows cover a finite range; they cannot be the
        // enclosure test for a deep room or its underwater surfaces.
        let illuminance = day.illuminance(sun)
            * solar_compensation
            * cloud_attenuation
            * sky_visibility(cave.blend);
        if light.illuminance != illuminance {
            light.illuminance = illuminance;
        }
        let shadows = illuminance > 0.0
            && graphics
                .as_ref()
                .is_none_or(|g| g.shadow_quality != terra_core::ShadowQuality::Off);
        if light.shadow_maps_enabled != shadows {
            light.shadow_maps_enabled = shadows;
        }
    }
    for (mut exposure, mut environment, generated, filtered, atmosphere) in &mut cameras {
        let brightness = graphics.as_ref().map_or(1.0, |g| g.brightness).log2();
        let target = cave_exposure - brightness;
        if exposure.ev100 != target {
            exposure.ev100 = target;
        }
        // Compensate eye adaptation: otherwise lowering EV while linearly
        // fading the sky briefly makes the entrance brighter/bluer than outside.
        let intensity = 1.05 * sky_visibility(cave.blend) * solar_compensation;
        if environment.intensity != intensity {
            environment.intensity = intensity;
        }
        // Bevy 0.19 copies intensity only when creating these components. The
        // generated/filtered light is what actually reaches the PBR view uniform.
        if let Some(mut light) = generated
            && light.intensity != intensity
        {
            light.intensity = intensity;
        }
        if let Some(mut light) = filtered
            && light.intensity != intensity
        {
            light.intensity = intensity;
        }
        if let Some(mut atmosphere) = atmosphere {
            // The outdoor aerial LUT assumes every metre sees the sun. Limit
            // its integration in enclosed rooms, retaining the actual sky at
            // entrances and the shadow-aware cavern volume on Ultra.
            let distance = 0.01 + (32_000.0 - 0.01) * (1.0 - cave.blend).powi(8);
            if atmosphere.aerial_view_lut_max_distance != distance {
                atmosphere.aerial_view_lut_max_distance = distance;
            }
        }
    }
}

pub(crate) fn apply(
    blend: f32,
    water: f32,
    fog: &mut DistanceFog,
    grading: &mut ColorGrading,
    ambient: &mut AmbientLight,
) {
    let enclosed = blend.clamp(0.0, 1.0);
    // Water changes optical absorption, not the room's access to skylight.
    // Previously diving canceled this fill reduction and restored outdoor light.
    ambient.brightness += (25.2 - ambient.brightness) * enclosed;
    ambient.color = ambient.color.mix(&Color::srgb(0.94, 0.93, 0.90), enclosed);
    let blend = enclosed * (1.0 - water);
    if blend <= 0.0 {
        return;
    }
    let c = fog.color.to_srgba();
    fog.color = Color::srgb(
        c.red + (0.008 - c.red) * blend,
        c.green + (0.009 - c.green) * blend,
        c.blue + (0.010 - c.blue) * blend,
    );
    if let FogFalloff::Linear { start, end } = &mut fog.falloff {
        // Never reveal unloaded cave walls at short view distances, or invert
        // the interval when the player has chosen very dense atmospheric fog.
        let cave_end = end.min(128.0);
        let cave_start = start.min(48.0).min(cave_end * 0.5);
        *start += (cave_start - *start) * blend;
        *end += (cave_end - *end) * blend;
    }
    grading.global.post_saturation += (0.98 - grading.global.post_saturation) * blend;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spectator_cave_fill_is_local_and_does_not_change_surface_lighting() {
        let mut app = App::new();
        app.init_resource::<super::super::daylight::Daylight>()
            .insert_resource(CaveState {
                blend: 1.0,
                ..default()
            })
            .init_resource::<terra_network::client::ClientSession>()
            .add_systems(PostUpdate, exposure);
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                AmbientLight {
                    brightness: 25.2,
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<AmbientLight>(camera).unwrap().brightness,
            25.2
        );
        app.world_mut()
            .resource_mut::<terra_network::client::ClientSession>()
            .game_mode = terra_core::GameMode::Spectator;
        app.update();
        assert_eq!(
            app.world().get::<AmbientLight>(camera).unwrap().brightness,
            100.0
        );
        // Spectator readability is a floor, not a cap over a local Night Vision buff.
        app.world_mut()
            .get_mut::<AmbientLight>(camera)
            .unwrap()
            .brightness = 176.4;
        app.update();
        assert_eq!(
            app.world().get::<AmbientLight>(camera).unwrap().brightness,
            176.4
        );
        app.world_mut().resource_mut::<CaveState>().blend = 0.0;
        app.world_mut()
            .get_mut::<AmbientLight>(camera)
            .unwrap()
            .brightness = 760.0;
        app.update();
        assert_eq!(
            app.world().get::<AmbientLight>(camera).unwrap().brightness,
            760.0
        );
    }
    #[test]
    fn cave_intensity_reaches_generated_and_filtered_pbr_lights_and_restores_surface() {
        let mut app = App::new();
        app.init_resource::<super::super::daylight::Daylight>()
            .insert_resource(CaveState {
                blend: 1.0,
                ..default()
            })
            .add_systems(PostUpdate, exposure);
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                Exposure::default(),
                bevy::light::AtmosphereEnvironmentMapLight::default(),
                bevy::light::GeneratedEnvironmentMapLight {
                    intensity: 1.05,
                    ..default()
                },
                bevy::light::EnvironmentMapLight {
                    intensity: 1.05,
                    ..default()
                },
                bevy::pbr::AtmosphereSettings::default(),
            ))
            .id();
        let sun = app
            .world_mut()
            .spawn((super::super::daylight::Sun, DirectionalLight::default()))
            .id();
        app.update();
        for blend in [1.0, 0.5, 0.0] {
            app.world_mut().resource_mut::<CaveState>().blend = blend;
            app.update();
            let day = app
                .world()
                .resource::<super::super::daylight::Daylight>()
                .exposure;
            let cave_exposure = day + (CAVE_EXPOSURE - day) * blend;
            let expected = 1.05 * sky_visibility(blend) * 2.0_f32.powf(cave_exposure - day);
            assert_eq!(
                app.world()
                    .get::<DirectionalLight>(sun)
                    .unwrap()
                    .illuminance,
                app.world()
                    .resource::<super::super::daylight::Daylight>()
                    .illuminance(true)
                    * 2.0_f32.powf(cave_exposure - day)
                    * sky_visibility(blend)
            );
            assert_eq!(
                app.world()
                    .get::<bevy::light::GeneratedEnvironmentMapLight>(camera)
                    .unwrap()
                    .intensity,
                expected
            );
            assert_eq!(
                app.world()
                    .get::<bevy::pbr::AtmosphereSettings>(camera)
                    .unwrap()
                    .aerial_view_lut_max_distance,
                0.01 + (32_000.0 - 0.01) * (1.0 - blend).powi(8)
            );
            assert_eq!(
                app.world()
                    .get::<bevy::light::EnvironmentMapLight>(camera)
                    .unwrap()
                    .intensity,
                expected
            );
        }
    }
    #[test]
    fn cave_fog_never_extends_short_views_or_inverts_dense_fog_ranges() {
        for end in [2.976, 29.76, 88.0, 180.0] {
            let mut fog = DistanceFog {
                falloff: FogFalloff::Linear {
                    start: end * 0.7,
                    end,
                },
                ..default()
            };
            let mut grading = super::super::surface_grading();
            let mut ambient = AmbientLight::default();
            apply(1.0, 0.0, &mut fog, &mut grading, &mut ambient);
            let FogFalloff::Linear { start, end: actual } = fog.falloff else {
                panic!()
            };
            assert!(start >= 0.0 && start < actual && actual <= end);
        }
    }
    #[test]
    fn cave_fill_stays_enclosed_when_diving_and_water_keeps_its_own_fog() {
        let mut fog = DistanceFog {
            falloff: FogFalloff::Linear {
                start: 100.0,
                end: 180.0,
            },
            ..default()
        };
        let mut grading = super::super::surface_grading();
        let mut ambient = AmbientLight {
            brightness: 760.0,
            ..default()
        };
        apply(1.0, 1.0, &mut fog, &mut grading, &mut ambient);
        assert!((ambient.brightness - 25.2).abs() < 0.001);
        apply(1.0, 0.0, &mut fog, &mut grading, &mut ambient);
        assert!((ambient.brightness - 25.2).abs() < 0.001);
        assert!(matches!(
            fog.falloff,
            FogFalloff::Linear {
                start: 48.0,
                end: 128.0
            }
        ));
    }

    #[test]
    fn cave_darkening_preserves_local_lights_and_closes_both_celestial_sources() {
        assert!((2.0_f32.powf(6.621_488 - CAVE_EXPOSURE) - 1.125).abs() < 0.00001);
        assert_eq!(sky_visibility(0.0), 1.0);
        assert_eq!(sky_visibility(1.0), 0.0);
        let mut app = App::new();
        app.init_resource::<super::super::daylight::Daylight>()
            .insert_resource(CaveState {
                blend: 1.0,
                ..default()
            })
            .add_systems(PostUpdate, exposure);
        let sun = app
            .world_mut()
            .spawn((super::super::daylight::Sun, DirectionalLight::default()))
            .id();
        let moon = app
            .world_mut()
            .spawn((super::super::daylight::Moon, DirectionalLight::default()))
            .id();
        let lamp = app
            .world_mut()
            .spawn(PointLight {
                intensity: 3200.0,
                ..default()
            })
            .id();
        app.update();
        for entity in [sun, moon] {
            assert_eq!(
                app.world()
                    .get::<DirectionalLight>(entity)
                    .unwrap()
                    .illuminance,
                0.0
            );
        }
        assert_eq!(
            app.world().get::<PointLight>(lamp).unwrap().intensity,
            3200.0
        );
    }
}
