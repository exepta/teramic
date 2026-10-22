//! Climate-local precipitation and bounded, non-colliding surface accumulation.
use crate::{TerraGenerator, generation::biomes::BiomeSample};
use terra_core::{WeatherPattern, WorldWeather};

/// Furniture sheds precipitation; it remains a roof for the floor below it.
pub fn retains_snow(material: u16) -> bool {
    !matches!(material, 28 | 29 | 59 | 71..=73)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalWeather {
    pub rain: f32,
    pub snow: f32,
    pub clouds: f32,
    pub wetness: f32,
    pub wind: [f32; 2],
    pub temperature: f32,
}

pub fn sample(
    weather: &WorldWeather,
    terrain: &TerraGenerator,
    position: [f64; 3],
) -> LocalWeather {
    from_climate(
        weather,
        terrain.biome_at(position[0], position[2]),
        position[1],
        terrain.config().sea_level,
    )
}

pub fn sample_at(
    weather: &WorldWeather,
    terrain: &TerraGenerator,
    position: [f64; 3],
    clock: terra_core::WorldClock,
) -> LocalWeather {
    from_climate_seasonal(
        weather,
        terrain.biome_at(position[0], position[2]),
        position[1],
        terrain.config().sea_level,
        clock.season(terrain.config().seasons),
    )
}

pub fn from_climate(
    weather: &WorldWeather,
    biome: BiomeSample,
    altitude: f64,
    sea_level: f64,
) -> LocalWeather {
    from_climate_seasonal(
        weather,
        biome,
        altitude,
        sea_level,
        terra_core::WorldClock::default().season(terra_core::SeasonConfig {
            enabled: false,
            ..Default::default()
        }),
    )
}

pub fn from_climate_seasonal(
    weather: &WorldWeather,
    biome: BiomeSample,
    altitude: f64,
    sea_level: f64,
    season: terra_core::SeasonSample,
) -> LocalWeather {
    let temperature = ((biome.temperature - 0.5) * 22.0 + 12.0
        - (altitude - sea_level).max(0.0) * 0.11) as f32
        + season.temperature_offset();
    let cold = ((2.0 - temperature) / 3.0).clamp(0.0, 1.0);
    let highland = biome.weights.dominant() == crate::generation::biomes::BiomeKind::Highland;
    let snow = match weather.pattern {
        WeatherPattern::Snow | WeatherPattern::Blizzard => 1.0,
        WeatherPattern::Rain
        | WeatherPattern::Storm
        | WeatherPattern::Thunder
        | WeatherPattern::Thunderstorm => {
            if highland {
                1.0
            } else {
                0.0
            }
        }
        _ => {
            if highland {
                1.0
            } else {
                cold * cold * (3.0 - 2.0 * cold)
            }
        }
    };
    let climate_moisture = ((biome.humidity as f32 * 0.7 + 0.5)
        * (1.0 - biome.aridity() as f32 * 0.94))
        .clamp(0.02, 1.0);
    let moisture = match weather.pattern {
        WeatherPattern::Rain
        | WeatherPattern::Snow
        | WeatherPattern::Storm
        | WeatherPattern::Thunder
        | WeatherPattern::Thunderstorm
        | WeatherPattern::Blizzard => 1.0,
        _ => climate_moisture,
    };
    let precipitation = weather.intensity() * moisture;
    LocalWeather {
        rain: precipitation * (1.0 - snow),
        snow: precipitation * snow,
        clouds: weather.clouds() * (0.4 + moisture * 0.6),
        wetness: weather.wetness * moisture,
        wind: weather.wind(),
        temperature: if highland {
            temperature.min(-1.0)
        } else {
            temperature
        },
    }
}

#[cfg(feature = "bevy")]
mod surface;
#[cfg(feature = "bevy")]
pub(crate) use surface::tick;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlands_always_receive_snow_instead_of_rain_even_during_summer_storms() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut biome = terrain.biome_at(0.0, 0.0);
        biome.weights.highland = 1.0;
        biome.weights.plains = 0.0;
        biome.temperature = 1.0;
        let season = terra_core::WorldClock {
            day: 7,
            ..Default::default()
        }
        .season(Default::default());
        for pattern in [
            WeatherPattern::Rain,
            WeatherPattern::Storm,
            WeatherPattern::Thunderstorm,
            WeatherPattern::Snow,
        ] {
            let mut weather = WorldWeather::new(9);
            weather.set_pattern(pattern);
            weather.advance(8.0, 9);
            let local = from_climate_seasonal(&weather, biome, 80.0, 8.0, season);
            assert_eq!(local.rain, 0.0);
            assert!(local.snow > 0.6);
            assert!(local.temperature <= -1.0);
        }
    }

    #[test]
    fn seasons_change_lowland_precipitation_but_keep_hot_deserts_and_cold_peaks_local() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut weather = WorldWeather::new(5);
        weather.from_intensity = 1.0;
        weather.target_intensity = 1.0;
        let mut clock = terra_core::WorldClock {
            day: 7,
            ..Default::default()
        };
        let summer = sample_at(&weather, &terrain, [0.0, 20.0, 0.0], clock);
        clock.day = 21;
        let winter = sample_at(&weather, &terrain, [0.0, 20.0, 0.0], clock);
        assert!(summer.rain > 0.9 && summer.snow == 0.0);
        assert!(winter.snow > 0.9 && winter.rain == 0.0);
        let mut desert = terrain.biome_at(0.0, 0.0);
        desert.temperature = 1.0;
        desert.humidity = 0.05;
        desert.weights.plains = 0.0;
        desert.weights.desert = 1.0;
        let dry_winter = from_climate_seasonal(
            &weather,
            desert,
            20.0,
            8.0,
            clock.season(terrain.config().seasons),
        );
        assert!(dry_winter.temperature > 0.0 && dry_winter.snow == 0.0);
        clock.day = 7;
        assert!(sample_at(&weather, &terrain, [0.0, 320.0, 0.0], clock).snow > 0.9);
    }

    #[test]
    fn explicit_weather_commands_select_precipitation_in_any_biome() {
        let terrain = TerraGenerator::new(Default::default()).unwrap();
        let mut biome = terrain.biome_at(0.0, 0.0);
        biome.weights.forest = 0.0;
        biome.weights.desert = 1.0;
        let mut weather = WorldWeather::new(77);
        for (pattern, expect_snow) in [
            (WeatherPattern::Snow, true),
            (WeatherPattern::Rain, false),
            (WeatherPattern::Storm, false),
            (WeatherPattern::Thunder, false),
            (WeatherPattern::Thunderstorm, false),
            (WeatherPattern::Blizzard, true),
        ] {
            weather.set_pattern(pattern);
            weather.advance(8.0, 77);
            let local = from_climate(&weather, biome, 300.0, 8.0);
            assert!(if expect_snow {
                local.snow > 0.8 && local.rain == 0.0
            } else {
                local.rain > 0.6 && local.snow == 0.0
            });
        }
    }

    #[test]
    fn local_precipitation_changes_smoothly_with_height_and_desert_climate() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut weather = WorldWeather::new(1);
        weather.from_intensity = 1.0;
        weather.target_intensity = 1.0;
        let low = sample(&weather, &terrain, [0.0, 20.0, 0.0]);
        let high = sample(&weather, &terrain, [0.0, 180.0, 0.0]);
        assert!(low.rain > 0.9 && low.snow == 0.0);
        assert!(high.snow > 0.9 && high.rain == 0.0);
        let biome = terrain.biome_at(0.0, 0.0);
        let mut previous = from_climate(&weather, biome, 90.0, 8.0).snow;
        for altitude in 901..1500 {
            let next = from_climate(&weather, biome, f64::from(altitude) * 0.1, 8.0).snow;
            assert!((next - previous).abs() < 0.01);
            previous = next;
        }
        let mut desert = biome;
        desert.weights.forest = 0.0;
        desert.weights.desert = 1.0;
        assert!(from_climate(&weather, desert, 20.0, 8.0).rain < low.rain * 0.1);
    }
}
