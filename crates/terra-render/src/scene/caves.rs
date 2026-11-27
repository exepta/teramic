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
use terra_world::{edits::EditJournal, streaming::ChunkStore};

// Retain Phase 111's darker caves, with 12.5% more background readability.
const CAVE_EXPOSURE: f32 = 6.451_563;

pub(crate) fn sky_visibility(blend: f32) -> f32 {
    let open = 1.0 - blend.clamp(0.0, 1.0);
    open * open * (3.0 - 2.0 * open)
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct CaveState {
    pub blend: f32,
    roof_coverage: f32,
    ceiling: f32,
    target_ceiling: f32,
    entrance_mist: f32,
    inside_lair: bool,
    lair_blend: f32,
    pub(crate) salt_skylight: f32,
    salt_target: f32,
    salt_window: Vec4,
    opening_anchor: Option<(i64, i64)>,
    pub(crate) indirect_skylight: f32,
    beam_depths: Vec4,
    beam_radii: Vec4,
    beam_cross_radii: Vec4,
}

impl CaveState {
    fn exposure_ev(&self, daylight_ev: f32) -> f32 {
        daylight_ev + (CAVE_EXPOSURE - daylight_ev) * self.blend
    }

    pub(super) fn skylight_gain(&self, daylight_ev: f32) -> f32 {
        (self.exposure_ev(daylight_ev) - daylight_ev).exp2()
    }

    pub(crate) fn bounced_illuminance(&self, daylight_ev: f32) -> f32 {
        // Aperture measurements are outdoor lux. Apply the same eye-adaptation
        // compensation as direct sunlight before adding them to the room fill.
        9_000. * self.indirect_skylight * self.blend * self.skylight_gain(daylight_ev)
    }

    /// Gallery dust is an atmospheric effect. The clear air in a dragon lair
    /// still admits physical sunlight and light from its crystals and torches.
    fn gallery_air(&self) -> f32 {
        1.0 - self.lair_blend
    }

    pub(crate) fn outdoor_mist(&self) -> f32 {
        (1.0 - self.blend.max(self.lair_blend)).powi(2)
    }

    fn air_parameters(&self, haze: f32, sunlight: f32, submersion: f32) -> Vec4 {
        let air = self.gallery_air() * (1.0 - submersion);
        Vec4::new(
            if haze > 0.0 { self.blend * air } else { 0.0 },
            haze,
            self.entrance_mist * self.blend * air,
            // Artificial lair bounds can encompass an open crater and dozens of
            // metres of outdoor air. Only dust inside a measured cave scatters a beam.
            self.salt_skylight * sunlight * air * self.roof_coverage * f32::from(!self.inside_lair),
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
        self.opening_anchor = Some(opening.anchor);
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

    fn observe_roof(&mut self, roof: terra_world::enclosure::Enclosure) -> bool {
        // Partial coverage is a lower bound until the rest of the roof streams in.
        if roof.complete || roof.cave > self.roof_coverage {
            self.roof_coverage = roof.cave;
            self.target_ceiling = roof.ceiling;
            true
        } else {
            false
        }
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
                .after(super::daylight::update)
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
fn probe_terrain(
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: Vec3,
    day: &super::daylight::Daylight,
    state: &mut CaveState,
) {
    let p = eye.to_array().map(f64::from);
    let cell = eye.floor().as_i64vec3().to_array();
    let Some(column) = chunks.surface_column_at(cell[0], cell[2]) else {
        // The camera moved into a streaming column. Retain its last measured
        // enclosure until actual terrain arrives.
        return;
    };
    let lair_top = chunks
        .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
        .and_then(|prepared| prepared.dragon_lair_top(cell));
    state.inside_lair = lair_top.is_some();
    // A ravine or daylight shaft is open terrain. Use its visible floor
    // when testing burial and haze; the old uncut hill may be far overhead.
    let mut outdoor_height = column.height;
    if let Some([floor, ceiling]) = column.cave_cavity.or(column.river_cavity)
        && ceiling >= column.height
    {
        outdoor_height = floor;
    }
    if column.cave.open_to_sky() && column.lava_level.is_none() && column.water_level.is_none() {
        outdoor_height = outdoor_height.min(column.cave.floor);
    }
    if outdoor_height < column.height && cell[1] as f64 >= outdoor_height - 1. {
        // A player-built roof can seal a natural shaft. Keep the full roof
        // probe whenever even one overhead voxel is solid or still streaming.
        let sky_clear = (cell[1] + 1..=column.height.ceil() as i64 + 1).all(|y| {
            let at = [cell[0], y, cell[2]];
            journal
                .block(at)
                .or_else(|| chunks.block_at(at))
                .is_some_and(|id| {
                    terra_voxel::MaterialId::from_raw(id).is_some_and(|m| !m.is_opaque())
                })
        });
        if !sky_clear {
            outdoor_height = column.height;
        }
    }
    let height = lair_top.map_or(outdoor_height, |top| outdoor_height.max(top as f64));
    let enclosure = terra_world::enclosure::sample(p, height, |p| {
        chunks.block_at(p).map(|b| journal.block(p).unwrap_or(b))
    });
    let accepted = state.observe_roof(enclosure);
    if accepted {
        state.entrance_mist = entrance_density(
            column.biome.weights.spirit_blossom as f32,
            (height - p[1]) as f32,
            enclosure.cave,
        );
    } else if state.salt_target == 0. {
        return;
    }
    // A lowered dune, open river cut or crater can sit below the original
    // heightfield without having a ceiling. Its exposed air is not a cave
    // aperture. An authored skylight can legitimately span the whole roof fan.
    if enclosure.complete
        && enclosure.cave == 0.
        && column.cave.skylight(f64::from(eye.y)).is_none()
    {
        state.set_opening(None);
        return;
    }
    // A partial roof fan cannot remove a measured ceiling. A newly blocked
    // window can still extinguish its direct beam on this probe.
    let opening = skylight::measure(
        chunks,
        journal,
        eye,
        state.opening_anchor,
        day.sun,
        lair_top,
    );
    state.set_opening(opening);
}

fn probe(
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    (underwater, graphics, day, weather): GalleryEnvironment,
    mut published: ResMut<CaveState>,
    mut next_probe: Local<f32>,
    mut cameras: Query<(&Transform, &Projection, &mut shader::CaveSettings), With<TerraCamera>>,
) {
    let mut state = *published;
    for (transform, projection, mut uniform) in &mut cameras {
        let mut settings = *uniform;
        if time.elapsed_secs() >= *next_probe {
            *next_probe = time.elapsed_secs() + 0.18;
            probe_terrain(&chunks, &journal, transform.translation, &day, &mut state);
        }
        state.ceiling = adapt(state.ceiling, state.target_ceiling, time.delta_secs());
        state.lair_blend = adapt(
            state.lair_blend,
            f32::from(u8::from(state.inside_lair)),
            time.delta_secs(),
        );
        state.salt_skylight = adapt(state.salt_skylight, state.salt_target, time.delta_secs());
        let clouds = weather.as_ref().map_or(1., |w| 1. - w.clouds * 0.5);
        let irradiance = aperture_irradiance(
            state.gallery_aperture(day.daylight * clouds),
            transform.translation,
        );
        state.indirect_skylight = adapt(state.indirect_skylight, irradiance, time.delta_secs());
        // Crossing a sunbeam inside the same room should light the stone it
        // strikes. Only an actual change of roof cover adapts the whole view.
        state.blend = adapt(state.blend, state.roof_coverage, time.delta_secs());
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
        let parameters = state.air_parameters(haze, day.daylight * clouds, underwater.blend);
        settings.gallery_dust = state.gallery_air();
        if parameters.w > 0.0 || underwater.blend > 0.001 {
            settings.sun_direction = day.sun.extend(day.twilight);
        }
        if parameters.w > 0.0 {
            settings.salt_window = state.salt_window;
            settings.beam_depths = state.beam_depths;
            settings.beam_radii = state.beam_radii;
            settings.beam_cross_radii = state.beam_cross_radii;
        }
        settings.view_from_clip = matrix;
        settings.parameters = parameters;
        uniform.set_if_neq(settings);
    }
    // Water, fog and grading observe this resource. A periodic terrain probe
    // without a change in room lighting must not invalidate their uniforms.
    published.set_if_neq(state);
}
type LitCameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Exposure,
        Option<&'static mut bevy::light::AtmosphereEnvironmentMapLight>,
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
            fill.brightness = fill.brightness.max(100.0 * cave.blend);
        }
    }
    let cave_exposure = cave.exposure_ev(day.exposure);
    let solar_compensation = cave.skylight_gain(day.exposure);
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
    for (mut exposure, environment, generated, filtered, atmosphere) in &mut cameras {
        let brightness = graphics.as_ref().map_or(1.0, |g| g.brightness).log2();
        let target = cave_exposure - brightness;
        if exposure.ev100 != target {
            exposure.ev100 = target;
        }
        // Compensate eye adaptation: otherwise lowering EV while linearly
        // fading the sky briefly makes the entrance brighter/bluer than outside.
        let sky = sky_visibility(cave.blend);
        let reflected = cave.indirect_skylight * (1.0 - sky) * 0.27;
        let intensity = 1.05 * (sky + reflected) * solar_compensation;
        if let Some(mut environment) = environment
            && environment.intensity != intensity
        {
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
            let outdoor_air = (1.0 - cave.blend) * cave.gallery_air();
            let distance = 0.01 + (32_000.0 - 0.01) * outdoor_air.powi(8);
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
    fn settled_air_keeps_view_uniforms_until_light_or_medium_changes() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<terra_core::GameConfig>()
            .insert_resource(terra_world::streaming::WorldTerrain(
                terra_world::TerraGenerator::new(default()).unwrap(),
            ))
            .init_resource::<super::super::daylight::Daylight>()
            .init_resource::<crate::weather::WeatherVisual>()
            .init_resource::<CaveState>()
            .init_resource::<crate::water::UnderwaterState>()
            .add_systems(Update, (probe, crate::water::environment::update).chain());
        let camera = app
            .world_mut()
            .spawn((
                TerraCamera,
                Transform::from_xyz(1., 80., 1.),
                Projection::default(),
                shader::CaveSettings::default(),
                DistanceFog::default(),
                ColorGrading::default(),
                AmbientLight::default(),
            ))
            .id();
        let ticks = |app: &App| {
            let world = app.world();
            let entity = world.entity(camera);
            [
                world
                    .get_resource_ref::<CaveState>()
                    .unwrap()
                    .last_changed(),
                world
                    .get_resource_ref::<crate::water::UnderwaterState>()
                    .unwrap()
                    .last_changed(),
                entity
                    .get_ref::<shader::CaveSettings>()
                    .unwrap()
                    .last_changed(),
                entity.get_ref::<DistanceFog>().unwrap().last_changed(),
                entity.get_ref::<ColorGrading>().unwrap().last_changed(),
                entity.get_ref::<AmbientLight>().unwrap().last_changed(),
            ]
        };
        app.update();
        let settled = ticks(&app);
        for frame in 1..=5 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(220));
            app.world_mut()
                .resource_mut::<crate::weather::WeatherVisual>()
                .wind
                .x = frame as f32;
            app.update();
            assert_eq!(ticks(&app), settled);
        }
        let old_brightness = app.world().get::<AmbientLight>(camera).unwrap().brightness;
        app.world_mut()
            .resource_mut::<crate::weather::WeatherVisual>()
            .flash = 1.;
        app.update();
        assert_eq!(
            app.world().get::<AmbientLight>(camera).unwrap().brightness,
            old_brightness * 3.
        );
        assert_ne!(ticks(&app)[5], settled[5]);
    }
    #[test]
    fn a_sunlit_dragon_lair_lights_its_surfaces_without_projecting_a_beam_outdoors() {
        let mut room = CaveState {
            roof_coverage: 1.,
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
        assert_eq!(clear.w, 0.);
        room.lair_blend = 0.;
        room.inside_lair = true;
        assert_eq!(room.air_parameters(0.004, 1., 0.).w, 0.);
        room.inside_lair = false;
        room.roof_coverage = 0.;
        assert_eq!(room.air_parameters(0.004, 1., 0.).w, 0.);
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
            anchor: (48, 40),
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
    fn streamed_roof_samples_do_not_alternate_full_dark_and_skylight_exposure() {
        use terra_world::enclosure::Enclosure;
        let mut room = CaveState::default();
        let roof = Enclosure {
            cave: 1.,
            ceiling: 13.,
            complete: true,
        };
        assert!(room.observe_roof(roof));
        for coverage in [0.7, 0.45, 1., 0.8] {
            assert!(!room.observe_roof(Enclosure {
                cave: coverage,
                complete: false,
                ..roof
            }));
            assert_eq!(room.roof_coverage, 1.);
        }
        assert!(room.observe_roof(Enclosure {
            cave: 0.,
            complete: true,
            ..roof
        }));
        assert_eq!(room.roof_coverage, 0.);
    }

    #[test]
    fn seeing_a_sliver_of_sky_from_a_covered_room_does_not_flash_its_exposure_or_fill() {
        let day = super::super::SURFACE_EXPOSURE;
        let mut room = CaveState {
            blend: 1.,
            roof_coverage: 1.,
            ..default()
        };
        for transmission in [0., 0.25, 0.75, 0.25, 0.] {
            room.salt_skylight = transmission;
            room.indirect_skylight = 0.02 * transmission;
            assert!((room.exposure_ev(day) - CAVE_EXPOSURE).abs() < 1e-5);
            assert!(room.bounced_illuminance(day) < 1.);
        }
        room.blend = 0.;
        assert_eq!(room.exposure_ev(day), day);
        assert_eq!(room.skylight_gain(day), 1.);
    }

    #[test]
    fn daylight_ticks_keep_the_adapted_exposure_between_render_stages() {
        let mut app = App::new();
        app.init_resource::<terra_core::WorldClock>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<super::super::daylight::Daylight>()
            .insert_resource(CaveState {
                blend: 1.,
                ..default()
            })
            .add_systems(Update, super::super::daylight::update)
            .add_systems(PostUpdate, exposure);
        let eye = app
            .world_mut()
            .spawn((TerraCamera, Exposure::default(), AmbientLight::default()))
            .id();
        app.update();
        for phase in [0.1, 0.45, 0.52, 0.8] {
            app.world_mut()
                .resource_mut::<terra_core::WorldClock>()
                .phase = phase;
            app.world_mut().run_schedule(Update);
            assert!((app.world().get::<Exposure>(eye).unwrap().ev100 - CAVE_EXPOSURE).abs() < 1e-5);
            app.world_mut().run_schedule(PostUpdate);
        }
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
        for (blend, lair) in [(1.0, 0.0), (0.5, 0.0), (0.25, 1.0), (0.0, 0.0)] {
            app.world_mut().resource_mut::<CaveState>().blend = blend;
            app.world_mut().resource_mut::<CaveState>().lair_blend = lair;
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
                0.01 + (32_000.0 - 0.01) * ((1.0 - blend) * (1.0 - lair)).powi(8)
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
