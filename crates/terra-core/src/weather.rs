//! Server-owned weather; presentation quality never changes the world state.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherPattern {
    #[default]
    Climate,
    Clear,
    Rain,
    Snow,
    Storm,
    Thunder,
    Thunderstorm,
    Blizzard,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightningEvent {
    pub id: u64,
    pub at: f64,
    pub position: [f64; 3],
    pub ground_strike: bool,
}

impl LightningEvent {
    /// Weather uses normalized day seconds; transient presentation uses wall time.
    pub fn real_age(self, elapsed_total: f64, day_length_seconds: f64) -> Option<f32> {
        if !elapsed_total.is_finite()
            || !self.at.is_finite()
            || !day_length_seconds.is_finite()
            || day_length_seconds <= 0.0
        {
            return None;
        }
        Some(
            ((elapsed_total - self.at).max(0.0) * day_length_seconds / 1440.0)
                .min(f64::from(f32::MAX)) as f32,
        )
    }
}

/// Session-local baseline, shared by light and audio presentation. Historical
/// snapshots establish the first seen event, never replaying it on world entry.
#[derive(Clone, Copy, Debug, Default)]
pub struct LightningObserver {
    active: bool,
    seen: Option<(u64, u64)>,
}

impl LightningObserver {
    pub fn active(self) -> bool {
        self.active
    }

    pub fn observe(
        &mut self,
        active: bool,
        event: Option<LightningEvent>,
    ) -> Option<LightningEvent> {
        let baseline = !self.active;
        self.active = active;
        if !active {
            self.seen = None;
            return None;
        }
        let key = event.map(|event| (event.id, event.at.to_bits()));
        if baseline || self.seen == key {
            self.seen = key;
            return None;
        }
        self.seen = key;
        event
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnowProfile {
    pub flake_multiplier: f32,
    pub size_multiplier: f32,
    pub blizzard: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct WorldWeather {
    pub sequence: u64,
    pub elapsed_seconds: f64,
    pub elapsed_total: f64,
    pub duration_seconds: f64,
    pub transition_seconds: f64,
    pub from_intensity: f32,
    pub target_intensity: f32,
    pub from_wind: [f32; 2],
    pub target_wind: [f32; 2],
    pub wetness: f32,
    #[serde(default)]
    pub pattern: WeatherPattern,
    #[serde(default = "default_snow_level")]
    pub snow_level: u8,
    #[serde(default)]
    pub latest_lightning: Option<LightningEvent>,
}

fn default_snow_level() -> u8 {
    1
}

impl Default for WorldWeather {
    fn default() -> Self {
        Self::new(42042)
    }
}

impl WorldWeather {
    pub fn new(seed: u64) -> Self {
        Self {
            sequence: 0,
            elapsed_seconds: 0.0,
            elapsed_total: 0.0,
            duration_seconds: 300.0 + random(seed, 0, 0) * 300.0,
            transition_seconds: 24.0,
            from_intensity: 0.0,
            target_intensity: 0.0,
            from_wind: [0.35, 0.15],
            target_wind: [0.35, 0.15],
            wetness: 0.0,
            pattern: WeatherPattern::Climate,
            snow_level: 1,
            latest_lightning: None,
        }
    }

    pub fn valid(self) -> bool {
        (1..=5).contains(&self.snow_level)
            && self.latest_lightning.is_none_or(|event| {
                event.at.is_finite()
                    && (0.0..=self.elapsed_total).contains(&event.at)
                    && event
                        .position
                        .into_iter()
                        .all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
            })
            && self.elapsed_seconds.is_finite()
            && self.elapsed_seconds >= 0.0
            && self.elapsed_seconds <= self.duration_seconds
            && self.elapsed_total.is_finite()
            && self.elapsed_total >= self.elapsed_seconds
            && self.duration_seconds.is_finite()
            && (30.0..=3600.0).contains(&self.duration_seconds)
            && self.transition_seconds.is_finite()
            && (1.0..=60.0).contains(&self.transition_seconds)
            && self.transition_seconds <= self.duration_seconds
            && [self.from_intensity, self.target_intensity, self.wetness]
                .into_iter()
                .all(|n| n.is_finite() && (0.0..=1.0).contains(&n))
            && self
                .from_wind
                .into_iter()
                .chain(self.target_wind)
                .all(|n| n.is_finite() && n.abs() <= 8.0)
    }

    fn blend(self) -> f32 {
        let t = (self.elapsed_seconds / self.transition_seconds).clamp(0.0, 1.0) as f32;
        t * t * (3.0 - 2.0 * t)
    }

    pub fn intensity(self) -> f32 {
        self.from_intensity + (self.target_intensity - self.from_intensity) * self.blend()
    }

    pub fn wind(self) -> [f32; 2] {
        std::array::from_fn(|i| {
            self.from_wind[i] + (self.target_wind[i] - self.from_wind[i]) * self.blend()
        })
    }

    pub fn snow_profile(self) -> SnowProfile {
        let level = if self.pattern == WeatherPattern::Blizzard {
            5
        } else {
            self.snow_level
        };
        // The previous blizzard is the new baseline. Percentages are relative
        // to that baseline, except +800% at level five relative to level four.
        let (flake_multiplier, size_multiplier) = match level {
            2 => (16.0, 2.0),
            3 => (18.0, 2.6),
            4 => (20.0, 3.5),
            // A whiteout is denser snow, not oversized foreground snow cards.
            5 => (180.0, 2.0),
            _ => (4.0, 2.0),
        };
        SnowProfile {
            flake_multiplier,
            size_multiplier,
            blizzard: level == 5,
        }
    }

    /// Denser snowfall deposits faster without making whiteouts instant solid snow.
    pub fn snow_accumulation_multiplier(self) -> f32 {
        (self.snow_profile().flake_multiplier / 4.0).sqrt()
    }

    /// A stable event slot, not a random test each frame. The server resolves
    /// the actual loaded surface and publishes one shared event to all peers.
    pub fn lightning_slot(self, seed: u64) -> Option<(u64, bool, [f64; 2])> {
        let interval = match self.pattern {
            WeatherPattern::Thunder => 26.0,
            WeatherPattern::Thunderstorm => 13.0,
            WeatherPattern::Climate if self.target_intensity >= 0.84 => 22.0,
            _ => return None,
        };
        let start = self.transition_seconds + 5.0 + random(seed, self.sequence, 87) * 4.0;
        if self.elapsed_seconds < start {
            return None;
        }
        let slot = ((self.elapsed_seconds - start) / interval).floor() as u64;
        let id = self.sequence.wrapping_mul(0x9e3779b97f4a7c15) ^ slot;
        let angle = random(seed, id, 91) * std::f64::consts::TAU;
        let radius = 20.0 + random(seed, id, 92) * 120.0;
        Some((
            id,
            random(seed, id, 93) < 0.12,
            [angle.cos() * radius, angle.sin() * radius],
        ))
    }

    pub fn clouds(self) -> f32 {
        // A deterministic front becomes visible before precipitation starts.
        // No additional replicated timeline or random per-client cloud state.
        let lead = if self.sequence.is_multiple_of(2) && self.pattern == WeatherPattern::Climate {
            ((self.elapsed_seconds - (self.duration_seconds - 75.0)) / 75.0).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        (0.32 + self.intensity() * 0.65 + lead * 0.48).min(1.0)
    }

    /// An authoritative command starts a short smooth transition. Automatic
    /// climate resumes at the next cycle, preserving deterministic generation.
    pub fn set_pattern(&mut self, pattern: WeatherPattern) {
        self.from_intensity = self.intensity();
        self.from_wind = self.wind();
        self.sequence = self.sequence.saturating_add(1);
        self.elapsed_seconds = 0.0;
        self.pattern = pattern;
        self.snow_level = if pattern == WeatherPattern::Blizzard {
            5
        } else {
            1
        };
        self.latest_lightning = None;
        self.transition_seconds = 8.0;
        self.duration_seconds = 240.0;
        self.target_intensity = match pattern {
            WeatherPattern::Climate | WeatherPattern::Clear => 0.0,
            WeatherPattern::Rain => 0.78,
            WeatherPattern::Snow => 0.85,
            WeatherPattern::Storm | WeatherPattern::Thunderstorm | WeatherPattern::Blizzard => 1.0,
            WeatherPattern::Thunder => 0.64,
        };
        let speed = match pattern {
            WeatherPattern::Storm => 7.5,
            WeatherPattern::Thunder => 5.5,
            WeatherPattern::Thunderstorm | WeatherPattern::Blizzard => 7.8,
            WeatherPattern::Rain | WeatherPattern::Snow => 2.1,
            _ => 0.4,
        };
        let length = self.from_wind[0].hypot(self.from_wind[1]);
        self.target_wind = if length >= 0.01 {
            self.from_wind.map(|v| v / length * speed)
        } else {
            [speed * 0.92, speed * 0.39]
        };
    }

    /// Seconds are normalized to a 24-minute world day by the authority.
    pub fn advance(&mut self, seconds: f64, seed: u64) {
        self.advance_inner(seconds, seed, None);
    }

    /// `clock` is the date at the end of this update. Fronts use their boundary
    /// date, so a catch-up step and many short updates select the same weather.
    pub fn advance_seasonal(
        &mut self,
        seconds: f64,
        seed: u64,
        clock: crate::WorldClock,
        seasons: crate::SeasonConfig,
    ) {
        self.advance_inner(
            seconds,
            seed,
            (seasons.enabled && seasons.valid()).then_some((clock, seasons)),
        );
    }

    fn advance_inner(
        &mut self,
        seconds: f64,
        seed: u64,
        calendar: Option<(crate::WorldClock, crate::SeasonConfig)>,
    ) {
        if !self.valid() || !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        // Bounded catch-up also rejects maliciously large network/calendar deltas.
        let mut remaining = seconds.min(3600.0);
        while remaining > 0.0 {
            let boundary = if self.elapsed_seconds < self.transition_seconds {
                self.transition_seconds
            } else {
                self.duration_seconds
            };
            let step = remaining.min((boundary - self.elapsed_seconds).max(0.0));
            self.wetness = self.integrated_wetness(step);
            self.elapsed_seconds += step;
            self.elapsed_total += step;
            remaining -= step;
            if self.elapsed_seconds < self.duration_seconds {
                if remaining <= 0.0 {
                    break;
                }
                continue;
            }
            self.from_intensity = self.intensity();
            self.from_wind = self.wind();
            self.sequence = self.sequence.saturating_add(1);
            self.elapsed_seconds = 0.0;
            self.transition_seconds = 24.0;
            self.pattern = WeatherPattern::Climate;
            self.snow_level = 1;
            let season = calendar.map(|(mut clock, seasons)| {
                let phase = clock.phase - remaining / 1440.0;
                let days = phase.floor();
                clock.day = if days < 0.0 {
                    clock.day.saturating_sub((-days) as u64)
                } else {
                    clock.day.saturating_add(days as u64)
                };
                clock.phase = phase.rem_euclid(1.0);
                clock.season(seasons)
            });
            let wet = season.map_or_else(
                || !self.sequence.is_multiple_of(2),
                |season| {
                    random(seed, self.sequence, 4) < f64::from(season.precipitation_probability())
                },
            );
            self.target_intensity = if wet {
                let storm = season.is_none_or(|season| {
                    random(seed, self.sequence, 5) < f64::from(season.storm_probability())
                });
                (0.28 + random(seed, self.sequence, 1) * if storm { 0.66 } else { 0.50 }) as f32
            } else {
                0.0
            };
            self.duration_seconds = if wet { 120.0 } else { 300.0 }
                + random(seed, self.sequence, 2) * if wet { 180.0 } else { 300.0 };
            let angle = random(seed, self.sequence, 3) * std::f64::consts::TAU;
            let speed = if self.target_intensity >= 0.84 {
                6.6
            } else {
                0.3 + f64::from(self.target_intensity) * 2.3
            };
            self.target_wind = [(angle.cos() * speed) as f32, (angle.sin() * speed) as f32];
        }
    }

    // Exact exponential response to the smoothstep polynomial. Endpoint sampling
    // would make wetness depend on client FPS and diverge from the authority.
    fn integrated_wetness(self, step: f64) -> f32 {
        let speed = if self.target_intensity > 0.0 {
            0.055
        } else {
            0.004
        };
        let response = (-speed * step).exp();
        let before = f64::from(self.wetness);
        if self.elapsed_seconds >= self.transition_seconds {
            let target = f64::from(self.target_intensity);
            return (target + (before - target) * response).clamp(0.0, 1.0) as f32;
        }
        let t = self.transition_seconds;
        let a = f64::from(self.from_intensity);
        let b = f64::from(self.target_intensity - self.from_intensity);
        let particular = |seconds: f64| {
            let u = seconds / t;
            a + b * (3.0 * u * u - 2.0 * u * u * u) - b * (6.0 * u - 6.0 * u * u) / (t * speed)
                + b * (6.0 - 12.0 * u) / (t * t * speed * speed)
                + 12.0 * b / (t * t * t * speed * speed * speed)
        };
        (particular(self.elapsed_seconds + step)
            + (before - particular(self.elapsed_seconds)) * response)
            .clamp(0.0, 1.0) as f32
    }
}

fn random(seed: u64, sequence: u64, lane: u64) -> f64 {
    let mut h = seed ^ sequence.wrapping_mul(0x9e3779b97f4a7c15) ^ lane.rotate_left(39);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((h ^ (h >> 31)) >> 11) as f64 / ((1_u64 << 53) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasonal_fronts_are_reproducible_across_catchup_and_short_frames() {
        let config = crate::SeasonConfig::default();
        let mut clock = crate::WorldClock {
            day: 6,
            phase: 0.95,
            ..Default::default()
        };
        let mut stepped = WorldWeather::new(81);
        let mut catchup = stepped;
        for _ in 0..1200 {
            clock.advance(0.5);
            stepped.advance_seasonal(0.5, 81, clock, config);
        }
        catchup.advance_seasonal(600.0, 81, clock, config);
        assert_eq!(stepped.sequence, catchup.sequence);
        assert_eq!(stepped.target_intensity, catchup.target_intensity);
        assert_eq!(stepped.target_wind, catchup.target_wind);
        assert!((stepped.elapsed_seconds - catchup.elapsed_seconds).abs() < 1e-6);
        assert!((stepped.wetness - catchup.wetness).abs() < 0.0003);
    }

    #[test]
    fn autumn_has_more_wet_fronts_than_summer_and_disabled_seasons_keep_legacy_weather() {
        let config = crate::SeasonConfig::default();
        let mut wet = [0; 2];
        for seed in 0..1024 {
            for (index, day) in [7, 14].into_iter().enumerate() {
                let mut weather = WorldWeather::new(seed);
                let dt = weather.duration_seconds + 1.0;
                let clock = crate::WorldClock {
                    day,
                    ..Default::default()
                };
                weather.advance_seasonal(dt, seed, clock, config);
                wet[index] += usize::from(weather.target_intensity > 0.0);
                assert!(weather.valid());
            }
        }
        assert!(wet[1] > wet[0] * 2, "summer/autumn wet fronts: {wet:?}");
        let mut legacy = WorldWeather::new(22);
        let mut disabled = legacy;
        legacy.advance(1800.0, 22);
        disabled.advance_seasonal(
            1800.0,
            22,
            Default::default(),
            crate::SeasonConfig {
                enabled: false,
                ..config
            },
        );
        assert_eq!(legacy, disabled);
    }

    #[test]
    fn lightning_real_time_age_handles_short_and_long_world_days() {
        let event = LightningEvent {
            id: 7,
            at: 100.0,
            position: [0.0; 3],
            ground_strike: false,
        };
        assert_eq!(event.real_age(124.0, 60.0), Some(1.0));
        assert_eq!(event.real_age(101.0, 1440.0), Some(1.0));
        assert_eq!(event.real_age(100.5, 2880.0), Some(1.0));
        assert_eq!(event.real_age(99.9, 1440.0), Some(0.0));
        assert!(event.real_age(f64::NAN, 1440.0).is_none());
        assert!(event.real_age(101.0, 0.0).is_none());
    }

    #[test]
    fn lightning_observer_skips_join_baselines_and_repeated_snapshots_not_live_events() {
        let old = LightningEvent {
            id: 7,
            at: 100.0,
            position: [0.0; 3],
            ground_strike: false,
        };
        let next = LightningEvent {
            id: 8,
            at: 113.0,
            ..old
        };
        let mut observer = LightningObserver::default();
        assert_eq!(observer.observe(false, Some(old)), None);
        assert_eq!(observer.observe(true, Some(old)), None);
        assert_eq!(observer.observe(true, Some(old)), None);
        assert_eq!(observer.observe(true, Some(next)), Some(next));
        assert_eq!(observer.observe(true, Some(next)), None);
        assert_eq!(observer.observe(false, None), None);
        assert!(!observer.active());
        assert_eq!(
            observer.observe(true, Some(next)),
            None,
            "rejoin does not replay frozen weather event"
        );
        observer.observe(false, None);
        assert_eq!(observer.observe(true, None), None);
        assert_eq!(
            observer.observe(true, Some(old)),
            Some(old),
            "empty baseline must not consume a later live event"
        );
    }

    #[test]
    fn snow_levels_match_requested_density_and_size_and_blizzard_wind() {
        let mut weather = WorldWeather::new(22);
        weather.set_pattern(WeatherPattern::Snow);
        for (level, density, size) in [
            (1, 4.0, 2.0),
            (2, 16.0, 2.0),
            (3, 18.0, 2.6),
            (4, 20.0, 3.5),
            (5, 180.0, 2.0),
        ] {
            weather.snow_level = level;
            let profile = weather.snow_profile();
            assert_eq!(profile.flake_multiplier, density);
            assert_eq!(profile.size_multiplier, size);
            assert_eq!(profile.blizzard, level == 5);
            assert!(weather.valid());
        }
        weather.set_pattern(WeatherPattern::Blizzard);
        weather.advance(8.0, 22);
        assert!(weather.wind()[0].hypot(weather.wind()[1]) > 7.5);
        assert!(weather.snow_profile().blizzard);
        weather.snow_level = 0;
        assert!(!weather.valid());
    }

    #[test]
    fn extreme_snow_is_nine_times_level_four_not_an_unbounded_particle_request() {
        let mut weather = WorldWeather::new(22);
        weather.snow_level = 4;
        let four = weather.snow_profile().flake_multiplier;
        weather.snow_level = 5;
        assert_eq!(weather.snow_profile().flake_multiplier, four * 9.0);
    }

    #[test]
    fn lightning_has_stable_bounded_slots_and_rare_ground_candidates() {
        let mut weather = WorldWeather::new(73);
        assert!(weather.lightning_slot(73).is_none());
        weather.set_pattern(WeatherPattern::Thunderstorm);
        assert!(weather.lightning_slot(73).is_none());
        let mut ids = std::collections::BTreeSet::new();
        let mut strikes = 0;
        for _ in 0..2200 {
            weather.advance(0.1, 73);
            if let Some((id, ground, offset)) = weather.lightning_slot(73) {
                assert_eq!(weather.lightning_slot(73), Some((id, ground, offset)));
                assert!((20.0..=140.01).contains(&offset[0].hypot(offset[1])));
                if ids.insert(id) {
                    strikes += usize::from(ground);
                }
            }
        }
        assert!((14..=17).contains(&ids.len()));
        assert!(strikes < ids.len() / 2);
        weather.latest_lightning = Some(LightningEvent {
            id: 1,
            at: weather.elapsed_total + 1.0,
            position: [0.0; 3],
            ground_strike: false,
        });
        assert!(!weather.valid());
        weather.latest_lightning.as_mut().unwrap().at = weather.elapsed_total;
        assert!(weather.valid());
    }

    #[test]
    fn severe_automatic_fronts_can_thunder_without_changing_the_climate_contract() {
        let (seed, mut weather) = (0..512)
            .find_map(|seed| {
                let mut weather = WorldWeather::new(seed);
                let initial_duration = weather.duration_seconds;
                weather.advance(initial_duration + 40.0, seed);
                (weather.pattern == WeatherPattern::Climate && weather.target_intensity >= 0.84)
                    .then_some((seed, weather))
            })
            .expect("deterministic severe climate front");
        assert!(weather.valid());
        assert!((weather.wind()[0].hypot(weather.wind()[1]) - 6.6).abs() < 0.001);
        assert!(weather.lightning_slot(seed).is_some());
        weather.target_intensity = 0.83;
        assert!(weather.lightning_slot(seed).is_none());
        weather.set_pattern(WeatherPattern::Storm);
        weather.advance(40.0, seed);
        assert!(
            weather.lightning_slot(seed).is_none(),
            "windstorm is not automatically a thunderstorm"
        );
    }

    #[test]
    fn cloud_front_precedes_rain_and_commands_preserve_valid_smooth_state() {
        let mut weather = WorldWeather::new(77);
        weather.elapsed_seconds = weather.duration_seconds - 1.0;
        weather.elapsed_total = weather.elapsed_seconds;
        assert_eq!(weather.intensity(), 0.0);
        assert!(weather.clouds() > 0.75);
        let before = weather;
        weather.set_pattern(WeatherPattern::Storm);
        assert_eq!(weather.sequence, before.sequence + 1);
        assert_eq!(weather.intensity(), before.intensity());
        assert_eq!(weather.elapsed_total, before.elapsed_total);
        assert!(weather.valid());
        weather.advance(8.0, 77);
        assert_eq!(weather.intensity(), 1.0);
        assert!(weather.wind()[0].hypot(weather.wind()[1]) > 5.0);
        weather.set_pattern(WeatherPattern::Clear);
        assert_eq!(weather.intensity(), 1.0);
        weather.advance(8.0, 77);
        assert_eq!(weather.intensity(), 0.0);
        assert!(weather.wetness > 0.2);
        weather.advance(240.0, 77);
        assert_eq!(weather.pattern, WeatherPattern::Climate);
        assert!(weather.valid());
    }

    #[test]
    fn weather_cycles_are_deterministic_smooth_and_mostly_clear() {
        let mut a = WorldWeather::new(12);
        let mut b = a;
        let mut raining = 0;
        for _ in 0..7200 {
            let before = a.intensity();
            a.advance(1.0, 12);
            b.advance(1.0, 12);
            assert_eq!(a, b);
            assert!(a.valid());
            assert!((a.intensity() - before).abs() < 0.07);
            raining += usize::from(a.intensity() > 0.1);
        }
        assert!((1000..3600).contains(&raining), "wet seconds: {raining}");
        assert!(a.sequence > 10);
    }

    #[test]
    fn invalid_inputs_do_not_poison_weather_and_wetness_lingers() {
        let mut state = WorldWeather::new(2);
        let original = state;
        for dt in [f64::NAN, f64::INFINITY, -1.0] {
            state.advance(dt, 2);
            assert_eq!(state, original);
        }
        state.from_intensity = 0.9;
        state.target_intensity = 0.0;
        state.wetness = 1.0;
        state.advance(30.0, 2);
        assert_eq!(state.intensity(), 0.0);
        assert!(state.wetness > 0.8);
    }

    #[test]
    fn wetness_and_transitions_are_independent_of_frame_rate() {
        let mut sixty = WorldWeather::new(52);
        let mut twenty_four = sixty;
        let mut catch_up = sixty;
        for _ in 0..36_000 {
            sixty.advance(1.0 / 60.0, 52);
        }
        for _ in 0..14_400 {
            twenty_four.advance(1.0 / 24.0, 52);
        }
        catch_up.advance(600.0, 52);
        assert_eq!(sixty.sequence, catch_up.sequence);
        assert_eq!(twenty_four.sequence, catch_up.sequence);
        assert!((sixty.elapsed_seconds - catch_up.elapsed_seconds).abs() < 1e-6);
        assert!((sixty.wetness - catch_up.wetness).abs() < 0.0003);
        assert!((twenty_four.wetness - catch_up.wetness).abs() < 0.0003);
    }
}
