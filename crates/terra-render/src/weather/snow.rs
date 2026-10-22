//! Continuous cosmetic snow, carried by the existing weather and mesh payloads.
use terra_core::SeasonSample;
use terra_world::TerraGenerator;

#[derive(Default)]
pub(crate) struct SnowVisual {
    initialized: bool,
    accumulation: f32,
    weather_elapsed: Option<f64>,
    pub reserve: f32,
    pub temperature_offset: f32,
}

impl SnowVisual {
    pub fn advance(
        &mut self,
        season: Option<SeasonSample>,
        snowfall: f32,
        seconds: f32,
        weather_elapsed: Option<f64>,
    ) {
        let winter = season.filter(|s| s.enabled).map_or(0.0, |s| s.weights[3]);
        let temperature = season.map_or(0.0, SeasonSample::temperature_offset);
        if !self.initialized {
            self.initialized = true;
            self.temperature_offset = temperature;
        }
        let seconds = if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        };
        let elapsed = weather_elapsed.map_or(seconds, |now| {
            let previous = self.weather_elapsed.replace(now).unwrap_or(now);
            (now - previous).clamp(0.0, 3600.0) as f32
        });
        let snowfall = snowfall.clamp(0.0, 8.0);
        self.accumulation = (self.accumulation + snowfall * elapsed * 0.003
            - (1.0 - winter) * (1.0 - snowfall).max(0.0) * elapsed * 0.004)
            .clamp(0.0, 1.0);
        let target = self.accumulation;
        self.reserve += (target - self.reserve) * (1.0 - (-1.5 * seconds).exp());
        self.temperature_offset +=
            (temperature - self.temperature_offset) * (1.0 - (-1.5 * seconds).exp());
        if (target - self.reserve).abs() < 0.0001 {
            self.reserve = target;
        }
    }
}

/// Four climate probes per worker job; altitude is evaluated at each vertex.
pub(crate) struct SnowClimate {
    temperature: [f32; 4],
    origin_y: f64,
    sea: f64,
}

impl SnowClimate {
    pub fn new(terrain: &TerraGenerator, origin: [i64; 3]) -> Self {
        Self {
            temperature: std::array::from_fn(|i| {
                let x = origin[0] as f64 + if i % 2 == 0 { 0.0 } else { 16.0 };
                let z = origin[2] as f64 + if i / 2 == 0 { 0.0 } else { 16.0 };
                terrain.biome_at(x, z).temperature as f32
            }),
            origin_y: origin[1] as f64,
            sea: terrain.config().sea_level,
        }
    }

    pub fn temperature(&self, position: [f32; 3]) -> f32 {
        let x = (position[0] / 16.0).clamp(0.0, 1.0);
        let z = (position[2] / 16.0).clamp(0.0, 1.0);
        let north = self.temperature[0] + (self.temperature[1] - self.temperature[0]) * x;
        let south = self.temperature[2] + (self.temperature[3] - self.temperature[2]) * x;
        let biome = north + (south - north) * z;
        (biome - 0.5) * 22.0 + 12.0
            - ((self.origin_y + f64::from(position[1]) - self.sea).max(0.0) * 0.11) as f32
    }
}

/// Leave alpha 0/1 as untagged sentinels; this remains valid Unorm8 color data.
pub(crate) fn encode_temperature(temperature: f32) -> f32 {
    (2.0 + ((temperature + 64.0) / 128.0).clamp(0.0, 1.0) * 251.0) / 255.0
}

/// UV1.x retains full wetness precision and adds one byte of saved snow.
pub(crate) fn encode_surface(wetness: f32, snow: f32) -> f32 {
    (snow.clamp(0.0, 1.0) * 255.0).round() * 2.0 + wetness.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn season(day: u64) -> SeasonSample {
        terra_core::WorldClock {
            day,
            ..Default::default()
        }
        .season(Default::default())
    }

    fn decode_temperature(alpha: f32) -> f32 {
        ((alpha * 255.0 - 2.0) / 251.0) * 128.0 - 64.0
    }

    fn coverage(reserve: f32, temperature: f32) -> f32 {
        let cold = ((3.0 - temperature) / 4.0).clamp(0.0, 1.0);
        let cold = cold * cold * (3.0 - 2.0 * cold);
        let amount = (reserve * cold / 0.95).clamp(0.0, 1.0);
        amount * amount * (3.0 - 2.0 * amount)
    }

    #[test]
    fn accumulation_and_melt_are_continuous_and_frame_rate_independent() {
        let mut snow = SnowVisual::default();
        snow.advance(Some(season(7)), 0.0, 0.0, None);
        let mut coarse = SnowVisual::default();
        coarse.advance(Some(season(7)), 0.0, 0.0, None);
        for _ in 0..600 {
            let before = snow.reserve;
            snow.advance(Some(season(7)), 1.0, 0.1, None);
            assert!(snow.reserve > before && snow.reserve - before < 0.005);
        }
        coarse.advance(Some(season(7)), 1.0, 60.0, None);
        assert!((snow.accumulation - coarse.accumulation).abs() < 0.00001);
        assert!(snow.reserve > 0.17);
        let before = snow.reserve;
        snow.advance(Some(season(7)), 0.0, 1.0, None);
        assert!(snow.reserve < before && snow.reserve > 0.17);
        snow.advance(Some(season(7)), 0.0, 600.0, None);
        assert_eq!(snow.reserve, 0.0);
    }

    #[test]
    fn authoritative_catchup_blends_loaded_meshes_over_a_few_seconds() {
        let mut snow = SnowVisual::default();
        snow.advance(Some(season(14)), 0.0, 0.0, Some(0.0));
        snow.advance(Some(season(14)), 1.0, 0.1, Some(600.0));
        assert!(snow.reserve > 0.0 && snow.reserve < 0.2);
        for _ in 0..30 {
            snow.advance(Some(season(14)), 1.0, 0.1, Some(600.0));
        }
        assert!(snow.reserve > 0.99);
        assert_eq!(coverage(snow.reserve, -5.0), 1.0);
    }

    #[test]
    fn complete_cover_has_no_spatial_holes_and_warm_desert_stays_clear() {
        let winter = season(21);
        let mut snow = SnowVisual::default();
        snow.advance(Some(winter), 0.0, 0.0, None);
        assert_eq!(snow.reserve, 0.0, "a dry winter never creates snow");
        snow.advance(Some(winter), 1.0, 600.0, None);
        assert_eq!(snow.reserve, 1.0);
        for x in 0..256 {
            assert_eq!(coverage(snow.reserve, -5.0 + x as f32 * 0.001), 1.0);
        }
        assert_eq!(coverage(snow.reserve, 7.0), 0.0);
        assert_eq!(coverage(snow.reserve, -20.0), 1.0);
        let mut previous = coverage(1.0, -1.0);
        for step in 1..=400 {
            let next = coverage(1.0, -1.0 + step as f32 * 0.01);
            assert!(next <= previous && previous - next < 0.02);
            previous = next;
        }
        assert_eq!(previous, 0.0);
    }

    #[test]
    fn entering_winter_requires_snowfall_and_then_preserves_it() {
        let mut snow = SnowVisual::default();
        for day in [0, 14, 20, 21, 22] {
            snow.advance(Some(season(day)), 0.0, 60.0, None);
            assert_eq!(snow.reserve, 0.0);
        }
        snow.advance(Some(season(22)), 1.0, 60.0, None);
        let before = snow.reserve;
        assert!(before > 0.0 && before < 0.2);
        snow.advance(Some(season(23)), 0.0, 600.0, None);
        assert!((snow.reserve - before).abs() < 0.0001);
        let mut heavy = SnowVisual::default();
        heavy.advance(Some(season(22)), 6.0, 60.0, None);
        assert!(heavy.reserve > before * 4.0);
    }

    #[test]
    fn mesh_metadata_survives_unorm_packing_without_growing_the_vertex_payload() {
        for temperature in [-40.0, -5.0, 0.0, 12.0, 35.0, 60.0] {
            let alpha = encode_temperature(temperature);
            let packed = (alpha * 255.0).round() / 255.0;
            assert!(packed > 0.0 && packed < 1.0);
            assert!((decode_temperature(packed) - temperature).abs() < 0.26);
        }
        for snow in [0.0, 0.125, 0.5, 1.0] {
            for wetness in [0.0, 0.123_456, 0.5, 1.0] {
                let packed = encode_surface(wetness, snow);
                let bin = (packed * 0.5).floor();
                assert!((bin / 255.0 - snow).abs() < 1.0 / 255.0);
                assert!((packed - bin * 2.0 - wetness).abs() < 0.00002);
                if snow == 0.0 {
                    assert_eq!(packed, wetness);
                }
                let sky_packed = packed + 1024.0;
                let sky = (sky_packed / 1024.0).floor();
                let surface = sky_packed - sky * 1024.0;
                let sky_bin = (surface * 0.5).floor();
                assert_eq!(sky, 1.0);
                assert_eq!(sky_bin, bin);
                assert!((surface - sky_bin * 2.0 - wetness).abs() < 0.0001);
            }
        }
    }
}
