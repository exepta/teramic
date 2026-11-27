use bevy::prelude::*;

mod clouds;
mod fire;
mod lightning;
mod precipitation;
pub(crate) mod storage;
pub(crate) mod surfaces;
pub use precipitation::{WeatherPreview, WeatherRenderStatus};
pub(crate) use precipitation::{WeatherSet, WeatherVisual};

/// Opt-in cosmetic frost for isolated visual galleries, never a networked world.
#[derive(Resource)]
pub struct CosmeticSnowPreview;

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
pub(crate) fn install(app: &mut App) {
    storage::install(app);
    precipitation::install(app);
    clouds::install(app);
    lightning::install(app);
    surfaces::install(app);
    fire::install(app);
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
