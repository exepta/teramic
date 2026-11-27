//! Regional, smoothly varying responses shared by live soil and animal ecology.
use crate::generation::biomes::BiomeSample;
use terra_core::{SeasonSample, WorldWeather};

#[derive(Clone, Copy, Debug)]
pub struct EcologyClimate {
    pub temperature: f32,
    pub moisture: f32,
    pub precipitation: f32,
}

impl EcologyClimate {
    pub fn sample(
        weather: &WorldWeather,
        biome: BiomeSample,
        altitude: f64,
        sea_level: f64,
        season: SeasonSample,
        soil_wetness: f32,
    ) -> Self {
        let local = super::from_climate_seasonal(weather, biome, altitude, sea_level, season);
        Self {
            temperature: local.temperature,
            moisture: ((biome.humidity as f32
                * (1.0 - (biome.aridity() + biome.weights.ashlands) as f32 * 0.9))
                .max(local.wetness)
                .max(soil_wetness)
                + local.rain * 0.35)
                .clamp(0.0, 1.0),
            precipitation: local.rain + local.snow,
        }
    }

    /// Frozen/dry soil waits; moist temperate soil grows, very hot soil slows.
    pub fn grass_rate(self) -> f32 {
        let warmth = ((self.temperature - 1.0) / 9.0).clamp(0.0, 1.0);
        let heat = ((42.0 - self.temperature) / 15.0).clamp(0.0, 1.0);
        let moisture = ((self.moisture - 0.12) / 0.48).clamp(0.0, 1.0);
        warmth * heat * moisture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regional_moisture_and_seasons_gate_growth_without_hard_biome_edges() {
        let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
        let mut biome = terrain.biome_at(0.0, 0.0);
        biome.temperature = 0.5;
        biome.humidity = 0.7;
        biome.weights = crate::generation::biomes::BiomeWeights {
            ocean: 0.,
            plains: 0.,
            forest: 1.,
            desert: 0.,
            highland: 0.,
            mushroom_park: 0.,
            cliffted_desert: 0.,
            cherry_forest: 0.,
            spirit_blossom: 0.,
            blood_forest: 0.,
            ashlands: 0.,
            savanna: 0.0,
            swamp: 0.0,
            petrified_forest: 0.0,
            crystal_highlands: 0.0,
            misty_coast: 0.0,
        };
        let mut weather = WorldWeather::default();
        weather.set_pattern(terra_core::WeatherPattern::Clear);
        weather.advance(20.0, 1);
        let season = |day| {
            terra_core::WorldClock {
                day,
                ..Default::default()
            }
            .season(Default::default())
        };
        let sample = |biome, day, wetness| {
            EcologyClimate::sample(&weather, biome, 8., 8., season(day), wetness)
        };
        assert!(sample(biome, 7, 0.).grass_rate() > 0.9);
        assert_eq!(sample(biome, 21, 0.).grass_rate(), 0.);
        biome.humidity = 0.03;
        assert_eq!(sample(biome, 7, 0.).grass_rate(), 0.);
        assert!(sample(biome, 7, 0.8).grass_rate() > 0.9);
        let mut previous = 0.;
        for step in 0..=100 {
            biome.humidity = f64::from(step) / 100.;
            let rate = sample(biome, 7, 0.).grass_rate();
            assert!((rate - previous).abs() < 0.022);
            previous = rate;
        }
    }
}
