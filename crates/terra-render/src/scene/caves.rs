//! Cave readability: local environment blending plus a depth-aware, cave-only pass.
pub(crate) mod lights;
pub(crate) mod quality;
pub(crate) mod shader;
mod skylight;
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
    target_ceiling: f32,
    entrance_mist: f32,
    inside_lair: bool,
    lair_blend: f32,
    pub(crate) salt_skylight: f32,
    salt_target: f32,
    salt_window: Vec4,
    beam_depths: Vec4,
    beam_radii: Vec4,
    beam_cross_radii: Vec4,
}

impl CaveState {
    /// Gallery dust is an atmospheric effect. The clear air in a dragon lair
    /// still admits physical sunlight and light from its crystals and torches.
    fn gallery_air(&self) -> f32 {
        1.0 - self.lair_blend
    }

    fn air_parameters(&self, haze: f32, sunlight: f32, submersion: f32) -> Vec4 {
        let air = self.gallery_air() * (1.0 - submersion);
        Vec4::new(
            if haze > 0.0 { self.blend * air } else { 0.0 },
            haze,
            self.entrance_mist * self.blend * air,
            self.salt_skylight * sunlight * (1.0 - submersion),
        )
    }

    fn set_opening(&mut self, candidate: Option<skylight::Opening>) {
        let Some(opening) = candidate else {
            self.salt_target = 0.;
            self.salt_skylight = 0.;
            self.beam_depths = Vec4::ZERO;
            self.beam_radii = Vec4::ZERO;
            self.beam_cross_radii = Vec4::ZERO;
            return;
        };
        // Keep eye adaptation continuous while measuring the same skylight
        // from another cell or after one block along its rim changes.
        let overlap = self.salt_window.w.min(opening.window.w);
        if self
            .salt_window
            .truncate()
            .distance_squared(opening.window.truncate())
            > overlap * overlap
        {
            self.salt_skylight = 0.;
        }
        self.salt_window = opening.window;
        self.salt_target = opening.transmission;
        self.beam_depths = opening.depths;
        self.beam_radii = opening.radii;
        self.beam_cross_radii = opening.cross_radii;
    }

    /// World-space window and its effective radius. Occlusion, cloud cover,
    /// twilight and the eye's transition all reduce the admitted solid angle.
    pub(crate) fn gallery_aperture(&self, daylight: f32) -> Vec4 {
        let factor = (self.salt_skylight * daylight).clamp(0., 1.);
        self.salt_window.with_w(self.salt_window.w * factor.sqrt())
    }
}

pub(crate) fn aperture_irradiance(aperture: Vec4, position: Vec3) -> f32 {
    if aperture.w <= 0. || position.y > aperture.y {
        return 0.;
    }
    let distance = (aperture.truncate() - position).length_squared();
    let area = aperture.w * aperture.w;
    area / (area + distance)
}

fn adapt(current: f32, target: f32, dt: f32) -> f32 {
    // Eyes adapt to darkness more slowly than to daylight. Bound a resumed
    // frame so loading or an unfocused window cannot cause a full EV jump.
    let rate = if target > current { 1.8 } else { 4.0 };
    let next = current + (target - current) * (1.0 - (-rate * dt.clamp(0.0, 0.1)).exp());
    if (next - target).abs() < 0.0001 {
        target
    } else {
        next
    }
}

fn entrance_density(spirit: f32, burial: f32, enclosure: f32) -> f32 {
    let shallow = 1.0 - ((burial - 6.0) / 20.0).clamp(0.0, 1.0);
    let mouth = 4.0 * enclosure * (1.0 - enclosure);
    spirit * shallow.max(mouth).clamp(0.0, 1.0) * (enclosure * 5.0).clamp(0.0, 1.0)
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

type GalleryEnvironment<'w> = (
    Res<'w, crate::water::UnderwaterState>,
    Option<Res<'w, GraphicsConfig>>,
    Res<'w, super::daylight::Daylight>,
    Option<Res<'w, crate::weather::WeatherVisual>>,
);
fn probe(
    time: Res<Time>,
    terrain: Res<WorldTerrain>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    (underwater, graphics, day, weather): GalleryEnvironment,
    mut state: ResMut<CaveState>,
    mut cameras: Query<(&Transform, &Projection, &mut shader::CaveSettings), With<TerraCamera>>,
) {
    for (transform, projection, mut settings) in &mut cameras {
        if time.elapsed_secs() >= state.next_probe {
            state.next_probe = time.elapsed_secs() + 0.18;
            let p = transform.translation.to_array().map(f64::from);
            let cell = transform.translation.floor().as_i64vec3().to_array();
            if let Some(column) =
                chunks.prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
            {
                state.inside_lair = column.in_dragon_lair(cell);
            }
            let height = terrain.0.surface_height(p[0], p[2]);
            let enclosure = terra_world::enclosure::sample(p, height, |p| {
                chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b))
            });
            // Missing roof chunks are not an opening to the sky.
            if enclosure.complete || enclosure.cave >= state.target {
                state.target = enclosure.cave;
                state.target_ceiling = enclosure.ceiling;
                let column = chunks.surface_column_at(p[0].floor() as i64, p[2].floor() as i64);
                let spirit = column.map_or(0.0, |c| c.biome.weights.spirit_blossom as f32);
                let eye = transform.translation;
                let opening = skylight::measure(&chunks, &journal, eye, state.salt_window, day.sun);
                state.set_opening(opening);
                if state.salt_target > 0. && eye.y < state.salt_window.y {
                    let fraction = aperture_irradiance(state.salt_window, eye) * state.salt_target;
                    let ev_range = (day.exposure - CAVE_EXPOSURE).max(1.);
                    let adapted = (-fraction.max(0.012).log2() / ev_range).clamp(0., 1.);
                    // A nearby roof or bridge only shades the eye locally. The
                    // bright opening still occupies part of its field of view.
                    let influence = (state.salt_target * day.daylight).clamp(0., 1.);
                    state.target += (adapted - state.target) * influence;
                }
                state.entrance_mist =
                    entrance_density(spirit, (height - p[1]) as f32, enclosure.cave);
            } else {
                state.set_opening(None);
            }
        }
        state.ceiling = adapt(state.ceiling, state.target_ceiling, time.delta_secs());
        state.lair_blend = adapt(
            state.lair_blend,
            f32::from(u8::from(state.inside_lair)),
            time.delta_secs(),
        );
        let sky = adapt(state.salt_skylight, state.salt_target, time.delta_secs());
        if sky != state.salt_skylight {
            state.salt_skylight = sky;
        }
        let blend = adapt(state.blend, state.target, time.delta_secs());
        if blend != state.blend {
            state.blend = blend;
        }
        let matrix = match projection {
            Projection::Perspective(p) => p.get_clip_from_view().inverse(),
            Projection::Orthographic(p) => p.get_clip_from_view().inverse(),
            Projection::Custom(_) => Mat4::IDENTITY,
        };
        let low_quality = graphics
            .as_ref()
            .is_some_and(|g| g.cave_quality == terra_core::CaveQuality::Low);
        let haze = if low_quality {
            0.0
        } else {
            ((state.ceiling - 6.0) / 18.0).clamp(0.0, 1.0) * 0.004
        };
        // The analytic beam only runs at a confirmed opening. Even the low
        // preset can show direct shafts while volumetric room haze stays off.
        let clouds = weather.as_ref().map_or(1., |w| 1. - w.clouds * 0.5);
        let parameters = state.air_parameters(haze, day.daylight * clouds, underwater.blend);
        if settings.gallery_dust != state.gallery_air() {
            settings.gallery_dust = state.gallery_air();
        }
        if parameters.w > 0.0 {
            settings.salt_window = state.salt_window;
            settings.sun_direction = day.sun.extend(day.twilight);
            settings.beam_depths = state.beam_depths;
            settings.beam_radii = state.beam_radii;
            settings.beam_cross_radii = state.beam_cross_radii;
        }
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
        // Enclosure controls local fill, not the sun itself. Disabling a global
        // light from the camera position also extinguishes the outdoor terrain
        // visible through an entrance. Actual roof geometry casts the shadow.
        let illuminance = day.illuminance(sun) * solar_compensation * cloud_attenuation;
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
        let sky = sky_visibility(cave.blend);
        let reflected = cave.salt_skylight * (1.0 - sky) * 0.27;
        let intensity = 1.05 * (sky + reflected) * solar_compensation;
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
    _fog: &mut DistanceFog,
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
    // DistanceFog affects every surface in the view, including sunlit terrain
    // outside. Keep the existing outdoor/water medium; the depth-aware cave
    // pass adds only subtle haze on actual geometry, never on the sky.
    grading.global.post_saturation += (0.98 - grading.global.post_saturation) * blend;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_sunlit_dragon_lair_keeps_its_skylight_without_the_gallery_veil() {
        let mut room = CaveState {
            blend: 1.,
            entrance_mist: 1.,
            salt_skylight: 1.,
            salt_window: Vec4::new(20., 42., 15., 12.),
            ..default()
        };
        let natural = room.air_parameters(0.004, 1., 0.);
        assert!(natural.x > 0. && natural.z > 0. && natural.w > 0.);
        room.lair_blend = 1.;
        let clear = room.air_parameters(0.004, 1., 0.);
        assert_eq!([clear.x, clear.z, room.gallery_air()], [0.; 3]);
        assert_eq!(clear.w, natural.w);
        assert!(aperture_irradiance(room.gallery_aperture(1.), Vec3::new(20., 34., 15.)) > 0.);
    }

    #[test]
    fn only_daylight_through_a_measured_gallery_window_reaches_rock_and_brine() {
        let mut room = CaveState {
            blend: 1.,
            salt_skylight: 0.85,
            salt_window: Vec4::new(50., 24., -8., 11.),
            ..default()
        };
        let near = Vec3::new(50., 10., -8.);
        let far = near - Vec3::X * 40.;
        let day = room.gallery_aperture(1.);
        assert!(aperture_irradiance(day, near) > 2. * aperture_irradiance(day, far));
        assert!(
            aperture_irradiance(room.gallery_aperture(0.4), near) < aperture_irradiance(day, near)
        );
        assert_eq!(aperture_irradiance(room.gallery_aperture(0.), near), 0.);
        assert_eq!(aperture_irradiance(day, near + 22. * Vec3::Y), 0.);
        // The view directly beneath a tall natural shaft can have an almost
        // unobstructed vertical probe. The same water still sees the same sky.
        room.blend = 0.05;
        assert_eq!(room.gallery_aperture(1.), day);
        room.salt_skylight = 0.;
        assert_eq!(aperture_irradiance(room.gallery_aperture(1.), near), 0.);
    }

    #[test]
    fn sealing_a_roof_cuts_the_beam_and_a_new_opening_adapts_from_darkness() {
        let old = Vec4::new(-12., 35., 8., 7.);
        let new = Vec4::new(48., 30., 40., 7.);
        let mut room = CaveState {
            salt_window: old,
            salt_skylight: 1.,
            ..default()
        };
        room.set_opening(None);
        assert_eq!(room.salt_target, 0.);
        assert_eq!(room.gallery_aperture(1.).w, 0.);
        let window = || skylight::Opening {
            window: new,
            depths: Vec4::splat(8.),
            radii: Vec4::ONE,
            cross_radii: Vec4::ONE,
            transmission: 0.8,
        };
        room.set_opening(Some(window()));
        assert_eq!(room.salt_window, new);
        assert_eq!(room.salt_target, 0.8);
        assert!(room.gallery_aperture(1.).w < 0.3);
    }

    #[test]
    fn eye_adaptation_is_continuous_and_recovers_daylight_faster() {
        for fps in [30, 60, 144] {
            let mut entering = 0.0;
            let mut leaving = 1.0;
            for _ in 0..fps {
                let next = adapt(entering, 1.0, 1.0 / fps as f32);
                assert!(next >= entering && next - entering < 0.065);
                entering = next;
                leaving = adapt(leaving, 0.0, 1.0 / fps as f32);
            }
            assert!((entering - 0.8347).abs() < 0.001);
            assert!((leaving - 0.0183).abs() < 0.001);
        }
        assert!(adapt(0.0, 1.0, 30.0) < 0.17);
        assert_eq!(entrance_density(0.0, 8.0, 0.5), 0.0);
        assert_eq!(entrance_density(1.0, 40.0, 1.0), 0.0);
        assert!(entrance_density(1.0, 8.0, 0.5) > 0.8);
    }

    #[test]
    fn entrance_optics_shader_parses_in_both_multisample_modes() {
        use wesl::{
            pass::{Features, condcomp},
            syntax::TranslationUnit,
        };
        for multisampled in [false, true] {
            let mut module = include_str!("../../../../assets/shaders/terra_caves.wesl")
                .parse::<TranslationUnit>()
                .unwrap();
            let mut features = Features::new();
            features.set("MULTISAMPLED", multisampled);
            condcomp(&mut module, &features).unwrap();
        }
    }

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
    fn cave_fog_preserves_visible_outdoor_geometry_and_surface_ranges() {
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
            let original_color = fog.color;
            let original_start = end * 0.7;
            apply(1.0, 0.0, &mut fog, &mut grading, &mut ambient);
            let FogFalloff::Linear { start, end: actual } = fog.falloff else {
                panic!()
            };
            assert!(start >= 0.0 && start < actual && actual <= end);
            assert_eq!(fog.color, original_color);
            assert_eq!(start, original_start);
            assert_eq!(actual, end);
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
                start: 100.0,
                end: 180.0
            }
        ));
    }

    #[test]
    fn cave_darkening_keeps_sunlit_entrances_and_local_lights() {
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
        let day = app.world().resource::<super::super::daylight::Daylight>();
        for (entity, sunlight) in [(sun, true), (moon, false)] {
            let light = app.world().get::<DirectionalLight>(entity).unwrap();
            // Eye adaptation must not make a sunlit surface darker or brighter
            // just because the camera is standing underneath a cave roof.
            let exposed = light.illuminance * 2.0_f32.powf(-CAVE_EXPOSURE);
            let outdoor = day.illuminance(sunlight) * 2.0_f32.powf(-day.exposure);
            assert!((exposed - outdoor).abs() < 0.000_01);
            assert_eq!(light.shadow_maps_enabled, light.illuminance > 0.0);
        }
        assert_eq!(
            app.world().get::<PointLight>(lamp).unwrap().intensity,
            3200.0
        );
    }
}
