//! Server-owned calendar, independent of rendering and wall-clock time zones.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldTimeConfig {
    /// Real seconds for one complete 24-hour cycle.
    pub day_length_seconds: f64,
    /// Used only when a world has no saved clock yet.
    pub start_hour: f64,
    pub paused: bool,
}
impl Default for WorldTimeConfig {
    fn default() -> Self {
        Self {
            day_length_seconds: 1440.0,
            start_hour: 10.0,
            paused: false,
        }
    }
}
impl WorldTimeConfig {
    pub fn valid(self) -> bool {
        self.day_length_seconds.is_finite()
            && (60.0..=604800.0).contains(&self.day_length_seconds)
            && self.start_hour.is_finite()
            && (0.0..24.0).contains(&self.start_hour)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Resource))]
pub struct WorldClock {
    pub day: u64,
    /// Fraction of the current day, midnight = 0, noon = 0.5.
    pub phase: f64,
    pub day_length_seconds: f64,
    pub paused: bool,
}
impl Default for WorldClock {
    fn default() -> Self {
        Self::new(WorldTimeConfig::default())
    }
}
impl WorldClock {
    pub fn new(config: WorldTimeConfig) -> Self {
        Self {
            day: 0,
            phase: config.start_hour / 24.0,
            day_length_seconds: config.day_length_seconds,
            paused: config.paused,
        }
    }
    pub fn valid(self) -> bool {
        self.phase.is_finite()
            && (0.0..1.0).contains(&self.phase)
            && self.day_length_seconds.is_finite()
            && (0.001..=6_048_000_000.0).contains(&self.day_length_seconds)
    }
    pub fn advance(&mut self, seconds: f64) {
        if self.paused || !seconds.is_finite() || seconds <= 0.0 || !self.valid() {
            return;
        }
        let phase = self.phase + seconds / self.day_length_seconds;
        self.day = self.day.saturating_add(phase.floor() as u64);
        self.phase = phase.fract();
    }
    pub fn hour(self) -> f64 {
        self.phase * 24.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn midnight_pause_and_multiday_progress_preserve_calendar() {
        let mut clock = WorldClock::new(WorldTimeConfig {
            start_hour: 23.0,
            ..Default::default()
        });
        clock.advance(180.0);
        assert_eq!(clock.day, 1);
        assert!((clock.hour() - 2.0).abs() < 1e-10);
        clock.advance(2880.0);
        assert_eq!(clock.day, 3);
        clock.paused = true;
        let saved = clock;
        clock.advance(200.0);
        assert_eq!(clock, saved);
        clock.paused = false;
        clock.advance(f64::NAN);
        assert!(clock.valid());
    }
}
