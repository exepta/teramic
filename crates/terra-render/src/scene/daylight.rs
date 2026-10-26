//! One celestial state drives the atmosphere, terrain, water and underwater view.
use bevy::{camera::Exposure, light::SunDisk, prelude::*};
use terra_core::{GraphicsConfig, ShadowQuality, WorldClock};
use terra_player::TerraCamera;

#[derive(Component)]
pub(super) struct Sun;
#[derive(Component)]
pub(super) struct Moon;

#[derive(Resource, Clone, Copy)]
pub(crate) struct Daylight {
    pub sun: Vec3,
    pub daylight: f32,
    pub twilight: f32,
    pub fog: Vec3,
    pub ambient: f32,
    pub ambient_color: Vec3,
    pub exposure: f32,
    pub rotation: f32,
}
fn smooth(a: f32, b: f32, value: f32) -> f32 {
    let t = ((value - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
impl Default for Daylight {
    fn default() -> Self {
        Self::at(WorldClock::default().phase)
    }
}
impl Daylight {
    pub(super) fn illuminance(&self, sun: bool) -> f32 {
        if sun {
            // Adaptation must not amplify the remaining sunset light. Blend
            // perceived energy first, then convert back to scene illuminance.
            100_000.0
                * smooth(-0.07, 0.18, self.sun.y)
                * 2.0_f32.powf(self.exposure - super::SURFACE_EXPOSURE)
        } else {
            2200.0
                // Hand over before the sun's last afterglow reaches zero. Waiting
                // for the moon to clear the horizon created a dark trough at dawn/dusk.
                * (1.0 - smooth(-0.07, 0.10, self.sun.y))
                * 2.0_f32.powf(self.exposure - (super::SURFACE_EXPOSURE - 3.0))
        }
    }

    pub fn at(phase: f64) -> Self {
        let rotation = ((phase - 0.25) * std::f64::consts::TAU) as f32;
        let sun = Vec3::new(
            rotation.cos(),
            rotation.sin() * 0.9,
            rotation.sin() * 0.4358899,
        );
        let daylight = smooth(-0.16, 0.22, sun.y);
        let twilight = (1.0 - smooth(0.0, 0.35, sun.y.abs())) * daylight;
        let fog = Vec3::new(0.025, 0.045, 0.085)
            .lerp(super::SURFACE_FOG, daylight)
            .lerp(Vec3::new(0.66, 0.37, 0.25), twilight * 0.5);
        Self {
            sun,
            daylight,
            twilight,
            fog,
            // Keep shadowed ground readable after dusk without exposing the sky more.
            ambient: (300.0 * 8.0 + (super::SURFACE_AMBIENT - 300.0 * 8.0) * daylight)
                * 2.0_f32.powf(-3.0 * (1.0 - daylight)),
            ambient_color: Vec3::new(0.65, 0.75, 1.0).lerp(Vec3::new(0.86, 0.92, 1.0), daylight),
            exposure: super::SURFACE_EXPOSURE - 3.0 * (1.0 - daylight),
            rotation,
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Daylight>().add_systems(
        Update,
        update
            .after(crate::graphics::apply)
            .before(crate::water::environment::update),
    );
}

type CelestialLights<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut DirectionalLight,
        &'static mut Transform,
        Has<Sun>,
    ),
    Or<(With<Sun>, With<Moon>)>,
>;

fn terrain_light_direction(direction: Vec3) -> Vec3 {
    const MIN_HEIGHT: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const SHOULDER: f32 = 0.06;
    if direction.y >= MIN_HEIGHT + SHOULDER {
        return direction;
    }
    // A smooth maximum keeps horizon shadows no longer than their caster's
    // height. Matching slopes at both ends avoids a sun-motion kink; the sky
    // still uses its actual direction, including the below-horizon afterglow.
    let shoulder = (direction.y - MIN_HEIGHT + SHOULDER).max(0.0);
    let height = MIN_HEIGHT + shoulder * shoulder / (4.0 * SHOULDER);
    let horizontal =
        Vec2::new(direction.x, direction.z).normalize_or_zero() * (1.0 - height * height).sqrt();
    Vec3::new(horizontal.x, height, horizontal.y)
}

fn update(
    clock: Res<WorldClock>,
    config: Res<terra_core::GameConfig>,
    graphics: Option<Res<GraphicsConfig>>,
    mut state: ResMut<Daylight>,
    mut lights: CelestialLights,
    mut cameras: Query<(&mut Exposure, &mut AmbientLight), With<TerraCamera>>,
) {
    if !clock.is_changed()
        && !config.is_changed()
        && graphics.as_ref().is_none_or(|g| !g.is_changed())
    {
        return;
    }
    *state = Daylight::at(clock.phase);
    let season = clock.season(config.world.seasons);
    if season.enabled {
        let winter = season.weights[terra_core::Season::Winter.index()];
        let autumn = season.weights[terra_core::Season::Autumn.index()];
        state.fog = state
            .fog
            .lerp(Vec3::new(0.63, 0.69, 0.73), winter * state.daylight * 0.16)
            .lerp(Vec3::new(0.72, 0.59, 0.41), autumn * state.daylight * 0.07);
        state.ambient_color = state
            .ambient_color
            .lerp(Vec3::new(0.74, 0.84, 1.0), winter * 0.16);
    }
    let shadows = graphics
        .as_ref()
        .is_none_or(|g| g.shadow_quality != ShadowQuality::Off);
    for (mut light, mut transform, sun) in &mut lights {
        let direction = if sun { state.sun } else { -state.sun };
        *transform = Transform::from_translation(terrain_light_direction(direction))
            .looking_at(Vec3::ZERO, Vec3::Y);
        light.illuminance = state.illuminance(sun);
        light.color = if sun {
            Color::srgb(
                1.0,
                0.95 - state.twilight * 0.27,
                0.84 - state.twilight * 0.36,
            )
        } else {
            Color::srgb(0.60, 0.72, 1.0)
        };
        // Keep the map until its light fades to zero. A horizon-angle cutoff
        // exposes still strongly lit foliage/props for an abrupt unshadowed flash.
        light.shadow_maps_enabled = shadows && light.illuminance > 0.0;
    }
    for (mut exposure, mut ambient) in &mut cameras {
        exposure.ev100 = state.exposure - graphics.as_ref().map_or(1.0, |g| g.brightness).log2();
        ambient.color = Color::srgb(
            state.ambient_color.x,
            state.ambient_color.y,
            state.ambient_color.z,
        );
    }
}

pub(super) fn moon(commands: &mut Commands, range: f32) {
    commands.spawn((
        Name::new("Moonlight"),
        Moon,
        DirectionalLight::default(),
        SunDisk::OFF,
        bevy::camera::visibility::RenderLayers::default()
            .with(crate::meshing::shadows::LAYER)
            .with(crate::transitions::QUEUE_SHADOW_LAYER),
        bevy::light::CascadeShadowConfigBuilder {
            num_cascades: 4,
            maximum_distance: crate::graphics::shadow_distance(range, ShadowQuality::High),
            ..default()
        }
        .build(),
        Transform::default(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn celestial_handoff_never_drops_below_the_adapted_night_light_floor() {
        let exposed = |day: Daylight| {
            (day.illuminance(true) + day.illuminance(false)) * 2.0_f32.powf(-day.exposure)
        };
        let night = exposed(Daylight::at(0.0));
        let mut previous = exposed(Daylight::at(0.5));
        for minute in 12 * 60..=24 * 60 {
            let day = Daylight::at(f64::from(minute) / (24.0 * 60.0));
            let energy = exposed(day);
            assert!(
                energy >= night - 0.00001,
                "minute {minute}: {energy} < {night}"
            );
            assert!(
                energy <= previous + 0.00001,
                "minute {minute}: {previous} -> {energy}"
            );
            previous = energy;
        }
        for minute in 0..=12 * 60 {
            let day = Daylight::at(f64::from(minute) / (24.0 * 60.0));
            let energy = exposed(day);
            assert!(
                energy >= previous - 0.00001,
                "minute {minute}: {previous} -> {energy}"
            );
            previous = energy;
        }
    }
    #[test]
    fn exposed_sunlight_fades_monotonically_without_a_twilight_brightness_pulse() {
        let mut previous = f32::MAX;
        for minute in 12 * 60..=20 * 60 {
            let day = Daylight::at(f64::from(minute) / (24.0 * 60.0));
            let exposed = day.illuminance(true) * 2.0_f32.powf(-day.exposure);
            assert!(
                exposed <= previous + 0.00001,
                "minute {minute}: {previous} -> {exposed}"
            );
            previous = exposed;
        }
        for second in 0..24 * 60 * 60 {
            let a = Daylight::at(f64::from(second) / (24.0 * 60.0 * 60.0));
            let b = Daylight::at(f64::from(second + 1) / (24.0 * 60.0 * 60.0));
            for sun in [true, false] {
                let energy = |d: Daylight| d.illuminance(sun) * 2.0_f32.powf(-d.exposure);
                assert!((energy(a) - energy(b)).abs() < 0.005);
            }
        }
    }
    #[test]
    fn twilight_never_disables_shadows_on_a_still_luminous_light() {
        let mut app = App::new();
        app.init_resource::<WorldClock>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<Daylight>()
            .add_systems(Update, update);
        let sun = app
            .world_mut()
            .spawn((Sun, DirectionalLight::default()))
            .id();
        let moon = app
            .world_mut()
            .spawn((Moon, DirectionalLight::default()))
            .id();
        // Sweep both transitions, including the old y=0.05 shadow cutoff and
        // the sun's below-horizon afterglow. Wind/masked materials keep casting.
        for base_hour in [5.0, 17.0] {
            for step in 0..=1200 {
                let hour = base_hour + f64::from(step) / 600.0;
                app.world_mut().resource_mut::<WorldClock>().phase = hour / 24.0;
                app.update();
                for entity in [sun, moon] {
                    let light = app.world().get::<DirectionalLight>(entity).unwrap();
                    assert_eq!(
                        light.shadow_maps_enabled,
                        light.illuminance > 0.0,
                        "hour={hour}, lux={}",
                        light.illuminance
                    );
                }
            }
        }
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .shadow_quality = ShadowQuality::Off;
        app.update();
        for entity in [sun, moon] {
            assert!(
                !app.world()
                    .get::<DirectionalLight>(entity)
                    .unwrap()
                    .shadow_maps_enabled
            );
        }
    }

    #[test]
    fn paused_twilight_shadow_roundtrips_keep_energy_and_restore_both_casters() {
        let mut app = App::new();
        app.init_resource::<WorldClock>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<Daylight>()
            .add_systems(Update, update);
        let sun = app
            .world_mut()
            .spawn((Sun, DirectionalLight::default()))
            .id();
        let moon = app
            .world_mut()
            .spawn((Moon, DirectionalLight::default()))
            .id();
        for hour in [5.9, 18.1] {
            let expected = Daylight::at(hour / 24.0);
            assert!(expected.illuminance(true) > 0.0);
            assert!(expected.illuminance(false) > 0.0);
            {
                let mut clock = app.world_mut().resource_mut::<WorldClock>();
                clock.phase = hour / 24.0;
                clock.paused = true;
            }
            for quality in [
                ShadowQuality::High,
                ShadowQuality::Off,
                ShadowQuality::Low,
                ShadowQuality::Off,
                ShadowQuality::Medium,
                ShadowQuality::High,
            ] {
                app.world_mut()
                    .resource_mut::<GraphicsConfig>()
                    .shadow_quality = quality;
                for _ in 0..3 {
                    app.update();
                    for (entity, is_sun) in [(sun, true), (moon, false)] {
                        let light = app.world().get::<DirectionalLight>(entity).unwrap();
                        assert_eq!(light.illuminance, expected.illuminance(is_sun));
                        assert_eq!(light.shadow_maps_enabled, quality != ShadowQuality::Off);
                    }
                }
            }
        }
    }

    #[test]
    fn cycle_is_continuous_and_night_remains_lit() {
        let noon = Daylight::at(0.5);
        let night = Daylight::at(0.0);
        assert_eq!(noon.daylight, 1.0);
        assert_eq!(night.daylight, 0.0);
        assert!(night.ambient > 0.0 && night.exposure < noon.exposure);
        for i in 0..24000 {
            let a = Daylight::at(f64::from(i) / 24000.0);
            let b = Daylight::at(f64::from(i + 1) / 24000.0);
            assert!(a.fog.distance(b.fog) < 0.005);
            assert!((a.exposure - b.exposure).abs() < 0.005);
            assert!((a.ambient - b.ambient).abs() < 1.0);
            assert!(a.ambient_color.distance(b.ambient_color) < 0.005);
            assert!((a.sun.length() - 1.0).abs() < 1e-5);
        }
        assert!(night.sun.distance(Daylight::at(1.0).sun) < 1e-5);
    }

    #[test]
    fn evening_shadows_receive_more_fill_without_changing_day_or_sky_exposure() {
        let legacy_night_ambient = 90.0;
        for hour in 19..=22 {
            let evening = Daylight::at(f64::from(hour) / 24.0);
            assert!(evening.ambient >= legacy_night_ambient * 3.0);
            assert!(evening.ambient <= legacy_night_ambient * 4.0);
            assert!(evening.ambient_color.y > 0.70);
            assert!(evening.ambient_color.z > evening.ambient_color.y);
            assert_eq!(evening.exposure, super::super::SURFACE_EXPOSURE - 3.0);
        }
        let noon = Daylight::at(0.5);
        assert_eq!(noon.ambient, super::super::SURFACE_AMBIENT);
        assert_eq!(noon.exposure, super::super::SURFACE_EXPOSURE);
        assert!(noon.ambient_color.distance(Vec3::new(0.86, 0.92, 1.0)) < 1e-5);
    }

    #[test]
    fn evening_sun_and_moon_shadows_are_bounded_without_moving_the_visible_sky() {
        for hour in [17.0, 17.5, 18.0, 19.0, 20.0, 22.0] {
            let sky = Daylight::at(hour / 24.0);
            let direction = if sky.sun.y > 0.0 { sky.sun } else { -sky.sun };
            let light = terrain_light_direction(direction);
            let projected_length = Vec2::new(light.x, light.z).length() / light.y;
            assert!(
                projected_length <= 1.0 + 1e-5,
                "hour={hour}, length={projected_length}"
            );
            assert!((light.length() - 1.0).abs() < 1e-5);
            assert!(
                Vec2::new(light.x, light.z)
                    .normalize_or_zero()
                    .distance(Vec2::new(direction.x, direction.z).normalize_or_zero())
                    < 1e-5
            );
        }
        let noon = Daylight::at(0.5);
        assert_eq!(terrain_light_direction(noon.sun), noon.sun);
    }

    #[test]
    fn shadow_direction_has_no_dawn_dusk_or_clamp_velocity_jump() {
        let step = 0.0001;
        for height in [
            -0.07,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2 - 0.06,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2 + 0.06,
        ] {
            let direction = |y: f32| Vec3::new((1.0 - y * y).sqrt(), y, 0.0);
            let before = terrain_light_direction(direction(height - step));
            let at = terrain_light_direction(direction(height));
            let after = terrain_light_direction(direction(height + step));
            let incoming = (at - before) / step;
            let outgoing = (after - at) / step;
            assert!(incoming.distance(outgoing) < 0.01, "height={height}");
        }
    }

    #[test]
    fn all_day_active_sun_and_moon_cast_short_shadows_without_disabling_them() {
        let mut app = App::new();
        app.init_resource::<WorldClock>()
            .init_resource::<terra_core::GameConfig>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<Daylight>()
            .add_systems(Update, update);
        let sun = app
            .world_mut()
            .spawn((Sun, DirectionalLight::default()))
            .id();
        let moon = app
            .world_mut()
            .spawn((Moon, DirectionalLight::default()))
            .id();
        for minute in 0..24 * 60 {
            let phase = f64::from(minute) / (24.0 * 60.0);
            app.world_mut().resource_mut::<WorldClock>().phase = phase;
            app.update();
            let sky = Daylight::at(phase);
            assert_eq!(app.world().resource::<Daylight>().sun, sky.sun);
            for (entity, is_sun) in [(sun, true), (moon, false)] {
                let light = app.world().get::<DirectionalLight>(entity).unwrap();
                assert_eq!(light.illuminance, sky.illuminance(is_sun));
                assert_eq!(light.shadow_maps_enabled, light.illuminance > 0.0);
                if light.shadow_maps_enabled {
                    let transform = app.world().get::<Transform>(entity).unwrap();
                    let direction = transform.rotation * Vec3::Z;
                    let length = Vec2::new(direction.x, direction.z).length() / direction.y;
                    assert!(length <= 1.0 + 1e-5, "minute={minute}, length={length}");
                    assert!(direction.distance(transform.translation) < 1e-5);
                }
            }
        }
    }
}
