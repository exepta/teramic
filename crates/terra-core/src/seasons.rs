//! A calendar derived from the authoritative saved world clock.
use crate::WorldClock;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    #[default]
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Spring => "Spring",
            Self::Summer => "Summer",
            Self::Autumn => "Autumn",
            Self::Winter => "Winter",
        }
    }
    pub const fn index(self) -> usize {
        match self {
            Self::Spring => 0,
            Self::Summer => 1,
            Self::Autumn => 2,
            Self::Winter => 3,
        }
    }

    pub const fn sleep_hour(self) -> f64 {
        match self {
            Self::Spring | Self::Autumn => 19.0,
            Self::Summer => 20.0,
            Self::Winter => 18.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeasonConfig {
    pub enabled: bool,
    pub spring_days: u32,
    pub summer_days: u32,
    pub autumn_days: u32,
    pub winter_days: u32,
    pub start_season: Season,
    /// Colors and climate blend into the next season during these final days.
    pub transition_days: f64,
}

impl Default for SeasonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            spring_days: 7,
            summer_days: 7,
            autumn_days: 7,
            winter_days: 7,
            start_season: Season::Spring,
            transition_days: 1.0,
        }
    }
}

impl SeasonConfig {
    /// Advances to the requested season's first day without rewinding saved time.
    pub fn set_season(self, clock: &mut WorldClock, season: Season) -> bool {
        if !self.enabled || !self.valid() {
            return false;
        }
        let lengths = self.lengths();
        let year: u64 = lengths.into_iter().map(u64::from).sum();
        let start: u64 = lengths[..self.start_season.index()]
            .iter()
            .map(|v| u64::from(*v))
            .sum();
        let target: u64 = lengths[..season.index()]
            .iter()
            .map(|v| u64::from(*v))
            .sum();
        let current = (clock.day % year + start) % year;
        let delta = (target + year - current) % year;
        let Some(day) = clock.day.checked_add(delta) else {
            return false;
        };
        clock.day = day;
        true
    }
    pub const fn lengths(self) -> [u32; 4] {
        [
            self.spring_days,
            self.summer_days,
            self.autumn_days,
            self.winter_days,
        ]
    }

    pub fn valid(self) -> bool {
        self.lengths().iter().all(|days| (1..=3650).contains(days))
            && self.transition_days.is_finite()
            && self.transition_days > 0.0
            && self
                .lengths()
                .iter()
                .all(|days| self.transition_days <= f64::from(*days))
    }

    pub fn sample(self, clock: WorldClock) -> SeasonSample {
        let config = if self.valid() { self } else { Self::default() };
        let lengths = config.lengths();
        let year_days: u64 = lengths.into_iter().map(u64::from).sum();
        let offset: u64 = lengths[..config.start_season.index()]
            .iter()
            .map(|d| u64::from(*d))
            .sum();
        // Reduce before adding the offset so even saturated clocks stay useful.
        let calendar_day = clock.day % year_days + offset;
        let mut day = calendar_day % year_days;
        let mut index = 0;
        while day >= u64::from(lengths[index]) {
            day -= u64::from(lengths[index]);
            index += 1;
        }
        let season = [
            Season::Spring,
            Season::Summer,
            Season::Autumn,
            Season::Winter,
        ][index];
        let phase = if clock.phase.is_finite() {
            clock.phase.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let within = day as f64 + phase;
        let length = f64::from(lengths[index]);
        let transition = config.transition_days.min(1.0);
        let blend = ((within - (length - transition)) / transition).clamp(0.0, 1.0);
        let blend = (blend * blend * (3.0 - 2.0 * blend)) as f32;
        let mut weights = [0.0; 4];
        weights[index] = 1.0 - blend;
        weights[(index + 1) % 4] = blend;
        SeasonSample {
            season,
            day: day as u32,
            year: clock.day / year_days + calendar_day / year_days,
            progress: (within / length) as f32,
            weights,
            enabled: config.enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeasonSample {
    pub season: Season,
    /// Zero-based day within the season.
    pub day: u32,
    pub year: u64,
    pub progress: f32,
    pub weights: [f32; 4],
    pub enabled: bool,
}

impl SeasonSample {
    fn blend(self, values: [f32; 4], disabled: f32) -> f32 {
        if self.enabled {
            self.weights
                .into_iter()
                .zip(values)
                .map(|(w, value)| w * value)
                .sum()
        } else {
            disabled
        }
    }

    pub fn temperature_offset(self) -> f32 {
        self.blend([0.0, 6.0, -2.0, -16.0], 0.0)
    }

    pub fn precipitation_probability(self) -> f32 {
        self.blend([0.62, 0.30, 0.72, 0.52], 0.5)
    }

    pub fn storm_probability(self) -> f32 {
        self.blend([0.12, 0.30, 0.24, 0.10], 0.16)
    }

    /// Natural deciduous crowns shed into winter and regrow into spring.
    pub fn leaf_loss(self) -> f32 {
        self.blend([0.0, 0.0, 0.0, 1.0], 0.0)
    }

    pub fn flower_loss(self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        match self.season {
            Season::Autumn => {
                let t = self.progress.clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
            Season::Winter => 1.0 - self.weights[Season::Spring.index()],
            _ => 0.0,
        }
    }

    pub fn leaf_tint(self) -> [f32; 3] {
        std::array::from_fn(|axis| {
            self.blend(
                [
                    [0.92, 1.06, 0.86][axis],
                    [1.0, 1.0, 1.0][axis],
                    [1.55, 0.62, 0.25][axis],
                    [0.82, 0.84, 0.78][axis],
                ],
                1.0,
            )
        })
    }

    pub fn grass_tint(self) -> [f32; 3] {
        std::array::from_fn(|axis| {
            self.blend(
                [
                    [0.91, 1.06, 0.90][axis],
                    [1.0, 1.0, 1.0][axis],
                    [1.16, 0.83, 0.57][axis],
                    [0.90, 0.86, 0.76][axis],
                ],
                1.0,
            )
        })
    }
}

impl WorldClock {
    pub fn season(self, config: SeasonConfig) -> SeasonSample {
        config.sample(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flower_dormancy_progresses_through_autumn_and_reverses_at_the_spring_transition() {
        let config = SeasonConfig::default();
        let sample = |day, phase| {
            WorldClock {
                day,
                phase,
                ..Default::default()
            }
            .season(config)
        };
        assert_eq!(sample(0, 0.0).flower_loss(), 0.0);
        assert_eq!(sample(7, 0.5).flower_loss(), 0.0);
        assert_eq!(sample(14, 0.0).flower_loss(), 0.0);
        let mut previous = 0.0;
        for step in 0..700 {
            let loss = sample(14 + step / 100, (step % 100) as f64 / 100.0).flower_loss();
            assert!(loss >= previous && loss <= 1.0);
            previous = loss;
        }
        assert!((sample(21, 0.0).flower_loss() - previous).abs() < 0.001);
        assert!(sample(27, 0.5).flower_loss() < 0.51);
        assert!((sample(27, 0.9999).flower_loss() - sample(28, 0.0).flower_loss()).abs() < 0.001);
        assert_eq!(
            WorldClock {
                day: 21,
                ..Default::default()
            }
            .season(SeasonConfig {
                enabled: false,
                ..config
            })
            .flower_loss(),
            0.0
        );
    }
    use crate::{Config, WorldConfig};
    #[test]
    fn seasonal_transition_only_blends_during_the_last_day_and_is_continuous() {
        let config = SeasonConfig::default();
        for day in 0..6 {
            for phase in [0.0, 0.5, 0.999] {
                let sample = config.sample(WorldClock {
                    day,
                    phase,
                    ..Default::default()
                });
                assert_eq!(sample.weights, [1.0, 0.0, 0.0, 0.0]);
            }
        }
        let mut previous = 0.0;
        for tick in 0..=100 {
            let sample = config.sample(WorldClock {
                day: 6,
                phase: f64::from(tick) / 100.0,
                ..Default::default()
            });
            assert!(sample.weights[1] >= previous);
            assert!((sample.weights.iter().sum::<f32>() - 1.0).abs() < 0.00001);
            previous = sample.weights[1];
        }
        assert_eq!(previous, 1.0);
    }
    #[test]
    fn changing_season_preserves_the_hour_and_never_rewinds_saved_days() {
        for start in [
            Season::Spring,
            Season::Summer,
            Season::Autumn,
            Season::Winter,
        ] {
            let cfg = SeasonConfig {
                start_season: start,
                ..Default::default()
            };
            let mut clock = WorldClock {
                day: 83,
                phase: 0.5,
                ..Default::default()
            };
            for season in [
                Season::Winter,
                Season::Spring,
                Season::Autumn,
                Season::Summer,
            ] {
                let before = clock.day;
                assert!(cfg.set_season(&mut clock, season));
                assert!(clock.day >= before);
                assert_eq!(clock.phase, 0.5);
                assert_eq!(clock.season(cfg).season, season);
                assert_eq!(clock.season(cfg).day, 0);
            }
        }
    }

    #[test]
    fn defaults_and_old_world_configuration_roundtrip() {
        let world: WorldConfig = toml::from_str("").unwrap();
        assert_eq!(world.seasons, SeasonConfig::default());
        assert_eq!(
            world.seasons.sample(WorldClock::default()).season,
            Season::Spring
        );
        let saved = toml::to_string(&world).unwrap();
        assert_eq!(toml::from_str::<WorldConfig>(&saved).unwrap(), world);
        let mut custom = world;
        custom.seasons.winter_days = 21;
        custom.seasons.start_season = Season::Autumn;
        assert!(custom.validate().is_ok());
        assert_eq!(
            toml::from_str::<WorldConfig>(&toml::to_string(&custom).unwrap()).unwrap(),
            custom
        );
    }

    #[test]
    fn validation_rejects_empty_and_excessive_or_nonfinite_calendars() {
        for invalid in [
            SeasonConfig {
                spring_days: 0,
                ..Default::default()
            },
            SeasonConfig {
                winter_days: 3651,
                ..Default::default()
            },
            SeasonConfig {
                transition_days: f64::NAN,
                ..Default::default()
            },
            SeasonConfig {
                transition_days: 0.0,
                ..Default::default()
            },
            SeasonConfig {
                transition_days: 8.0,
                ..Default::default()
            },
        ] {
            assert!(!invalid.valid());
            assert!(
                WorldConfig {
                    seasons: invalid,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn midnight_boundaries_years_and_unloaded_calendar_catchup() {
        let config = SeasonConfig::default();
        let mut clock = WorldClock::default();
        for (day, season, year) in [
            (0, Season::Spring, 0),
            (6, Season::Spring, 0),
            (7, Season::Summer, 0),
            (14, Season::Autumn, 0),
            (21, Season::Winter, 0),
            (28, Season::Spring, 1),
            (1_000_013, Season::Winter, 35_714),
        ] {
            clock.day = day;
            let sample = clock.season(config);
            assert_eq!((sample.season, sample.year), (season, year));
            assert!((sample.weights.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        }
        clock.day = 6;
        clock.phase = 1.0 - 1e-8;
        let before = clock.season(config);
        clock.day = 7;
        clock.phase = 0.0;
        let after = clock.season(config);
        assert!((before.temperature_offset() - after.temperature_offset()).abs() < 1e-5);
        // A newly loaded section samples the same absolute date as its neighbors.
        let saved = toml::to_string(&clock).unwrap();
        let replica: WorldClock = toml::from_str(&saved).unwrap();
        assert_eq!(replica.season(config), after);
    }

    #[test]
    fn configurable_lengths_start_offset_and_disabled_effects() {
        let config = SeasonConfig {
            spring_days: 2,
            summer_days: 3,
            autumn_days: 4,
            winter_days: 5,
            transition_days: 1.0,
            start_season: Season::Winter,
            ..Default::default()
        };
        let mut clock = WorldClock::default();
        assert_eq!(clock.season(config).season, Season::Winter);
        clock.day = 5;
        assert_eq!(clock.season(config).season, Season::Spring);
        assert_eq!(clock.season(config).year, 1);
        let disabled = clock.season(SeasonConfig {
            enabled: false,
            ..config
        });
        assert_eq!(disabled.grass_tint(), [1.0; 3]);
        assert_eq!(disabled.leaf_tint(), [1.0; 3]);
        assert_eq!(disabled.temperature_offset(), 0.0);
        assert_eq!(disabled.leaf_loss(), 0.0);
    }
}
