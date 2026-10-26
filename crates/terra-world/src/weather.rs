//! Climate-local precipitation and bounded, non-colliding surface accumulation.
use crate::{TerraGenerator, generation::biomes::BiomeSample};
use terra_core::{WeatherPattern, WorldWeather};

/// Furniture sheds precipitation; it remains a roof for the floor below it.
pub fn retains_snow(material: u16) -> bool {
    !matches!(material, 28 | 29 | 59 | 71..=73)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    let continental = (biome.temperature - 0.5) * 22.0 + 12.0;
    let maritime = biome.weights.ocean.clamp(0.0, 1.0);
    let temperature = (continental + (continental.max(8.0) - continental) * maritime
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

/// Precipitation belongs to the geographic surface, not the height of an observer.
pub fn from_surface(
    weather: &WorldWeather,
    surface: &crate::SurfaceColumn,
    sea_level: f64,
    season: terra_core::SeasonSample,
) -> LocalWeather {
    from_climate_seasonal(
        weather,
        surface.biome,
        surface
            .height
            .max(surface.water_level.unwrap_or(surface.height)),
        sea_level,
        season,
    )
}

#[cfg(feature = "bevy")]
#[derive(Clone, Copy)]
struct FallbackClimate {
    world: terra_core::WorldId,
    seed: terra_core::WorldSeed,
    column: [i64; 2],
    biome: BiomeSample,
    altitude: f64,
}

/// One bounded fallback for not-yet-streamed columns; prepared data always wins.
#[cfg(feature = "bevy")]
#[derive(Default)]
pub struct SurfaceWeatherProbe {
    fallback: Option<FallbackClimate>,
    #[cfg(test)]
    fallback_samples: usize,
}

#[cfg(feature = "bevy")]
impl SurfaceWeatherProbe {
    pub fn sample(
        &mut self,
        weather: &WorldWeather,
        terrain: &TerraGenerator,
        chunks: &crate::streaming::ChunkStore,
        position: [f64; 3],
        clock: terra_core::WorldClock,
    ) -> LocalWeather {
        let x = position[0].floor() as i64;
        let z = position[2].floor() as i64;
        let config = terrain.config();
        let season = clock.season(config.seasons);
        if let Some(surface) = chunks.surface_column_at(x, z) {
            return from_surface(weather, surface, config.sea_level, season);
        }
        let column = [x.div_euclid(16), z.div_euclid(16)];
        if self.fallback.is_none_or(|cached| {
            cached.world != config.id || cached.seed != config.seed || cached.column != column
        }) {
            let surface = terrain
                .surface_column(column[0] as f64 * 16.0 + 8.0, column[1] as f64 * 16.0 + 8.0);
            self.fallback = Some(FallbackClimate {
                world: config.id,
                seed: config.seed,
                column,
                biome: surface.biome,
                altitude: surface
                    .height
                    .max(surface.water_level.unwrap_or(surface.height)),
            });
            #[cfg(test)]
            {
                self.fallback_samples += 1;
            }
        }
        let cached = self.fallback.expect("Sampled surface climate");
        from_climate_seasonal(
            weather,
            cached.biome,
            cached.altitude,
            config.sea_level,
            season,
        )
    }
}

#[cfg(feature = "bevy")]
mod surface;
#[cfg(feature = "bevy")]
pub(crate) use surface::tick;

#[cfg(test)]
mod tests {
    use super::*;
    fn wet_weather() -> WorldWeather {
        let mut weather = WorldWeather::new(116);
        weather.from_intensity = 1.0;
        weather.target_intensity = 1.0;
        weather
    }

    #[test]
    fn cold_ocean_and_coasts_are_maritime_in_spring_but_still_snow_in_winter() {
        use crate::generation::biomes::{BiomeSample, BiomeWeights};
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let mut surface = terrain.surface_column(0.0, 0.0);
        surface.height = -30.0;
        surface.water_level = Some(8.0);
        let spring = terra_core::WorldClock::default().season(terrain.config().seasons);
        let weather = wet_weather();
        let mut previous: Option<f32> = None;
        for step in 0..=100 {
            let ocean = f64::from(step) / 100.0;
            surface.biome = BiomeSample {
                temperature: 0.0,
                humidity: 0.8,
                weights: BiomeWeights {
                    ocean,
                    forest: 1.0 - ocean,
                    plains: 0.0,
                    desert: 0.0,
                    highland: 0.0,
                    mushroom_park: 0.0,
                    cliffted_desert: 0.0,
                    cherry_forest: 0.0,
                },
            };
            let local = from_surface(&weather, &surface, 8.0, spring);
            if let Some(before) = previous {
                assert!((local.temperature - before).abs() < 0.071);
            }
            previous = Some(local.temperature);
            if ocean >= 0.25 {
                assert!(local.rain > 0.99 && local.snow == 0.0);
            }
        }
        let winter = terra_core::WorldClock {
            day: 21,
            ..Default::default()
        }
        .season(terrain.config().seasons);
        let winter_weather = from_surface(&weather, &surface, 8.0, winter);
        assert!(winter_weather.temperature < 0.0 && winter_weather.snow > 0.99);
        assert_eq!(winter_weather.rain, 0.0);
        let mut explicit = weather;
        explicit.set_pattern(WeatherPattern::Snow);
        explicit.advance(8.0, 116);
        let forced = from_surface(&explicit, &surface, 8.0, spring);
        assert!(forced.snow > 0.8 && forced.rain == 0.0);
        surface.biome.temperature = 1.0;
        surface.biome.weights.ocean = 0.0;
        surface.biome.weights.highland = 1.0;
        let highland = from_surface(&weather, &surface, 8.0, spring);
        assert!(highland.snow > 0.99 && highland.rain == 0.0);
    }

    #[cfg(feature = "bevy")]
    #[test]
    fn surface_probe_ignores_camera_height_and_never_repeats_fallback_noise_while_flying_vertically()
     {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            base_height: 20.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        let weather = wet_weather();
        let chunks = crate::streaming::ChunkStore::default();
        let mut probe = SurfaceWeatherProbe::default();
        let low = probe.sample(
            &weather,
            &terrain,
            &chunks,
            [-4.5, 20.0, -4.5],
            Default::default(),
        );
        for altitude in [-128.0, 50.0, 200.0, 312.0, 356.0] {
            assert_eq!(
                probe.sample(
                    &weather,
                    &terrain,
                    &chunks,
                    [-4.5, altitude, -4.5],
                    Default::default()
                ),
                low
            );
        }
        assert!(low.rain > 0.99 && low.snow == 0.0);
        assert_eq!(probe.fallback_samples, 1);
        let border_left = probe.sample(
            &weather,
            &terrain,
            &chunks,
            [-0.001, 240.0, -4.5],
            Default::default(),
        );
        let border_right = probe.sample(
            &weather,
            &terrain,
            &chunks,
            [0.001, 240.0, -4.5],
            Default::default(),
        );
        assert_eq!(border_left, border_right);
        assert_eq!(probe.fallback_samples, 2);
    }

    #[cfg(feature = "bevy")]
    #[test]
    fn prepared_surface_weather_uses_exact_cached_column_without_generation_and_respects_seasons() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            biomes: false,
            caves: false,
            trees: false,
            rivers: false,
            landscape_revision: 0,
            base_height: 20.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        let prepared = crate::SectionGenerator::new(terrain.clone())
            .prepare_column([-1, -1], true)
            .unwrap();
        let mut chunks = crate::streaming::ChunkStore::default();
        chunks.set_prepared_column_for_test([-1, -1], Some(std::sync::Arc::new(prepared)));
        let weather = wet_weather();
        let mut probe = SurfaceWeatherProbe::default();
        let clock = terra_core::WorldClock::default();
        let near = probe.sample(&weather, &terrain, &chunks, [-1.0, 20.0, -1.0], clock);
        assert_eq!(
            probe.sample(&weather, &terrain, &chunks, [-1.0, 356.0, -1.0], clock),
            near
        );
        assert_eq!(
            near,
            from_surface(
                &weather,
                chunks.surface_column_at(-1, -1).unwrap(),
                terrain.config().sea_level,
                clock.season(terrain.config().seasons)
            )
        );
        assert_eq!(probe.fallback_samples, 0);
        let winter = probe.sample(
            &weather,
            &terrain,
            &chunks,
            [-1.0, 356.0, -1.0],
            terra_core::WorldClock { day: 21, ..clock },
        );
        assert!(winter.snow > 0.99 && winter.rain == 0.0);
        assert_eq!(probe.fallback_samples, 0);
    }

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
