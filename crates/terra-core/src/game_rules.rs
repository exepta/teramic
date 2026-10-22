//! Mutable world policy. Terrain identity and voxel generation are not affected.
use crate::{SeasonConfig, WorldClock, WorldConfig, WorldTimeConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameRules {
    pub sleep_percentage: f64,
    pub game_time_speed: f64,
}
impl Default for GameRules {
    fn default() -> Self {
        Self {
            sleep_percentage: 100.0,
            game_time_speed: 1.0,
        }
    }
}
impl GameRules {
    pub fn valid(self) -> bool {
        GameRule::SleepPercentage.valid(self.sleep_percentage)
            && GameRule::GameTimeSpeed.valid(self.game_time_speed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRule {
    SleepPercentage,
    SeasonSwapInterval,
    GameTimeSpeed,
}
impl GameRule {
    pub const ALL: [Self; 3] = [
        Self::SleepPercentage,
        Self::SeasonSwapInterval,
        Self::GameTimeSpeed,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::SleepPercentage => "sleep_percentage",
            Self::SeasonSwapInterval => "season_swap_interval",
            Self::GameTimeSpeed => "game_time_speed",
        }
    }
    pub const fn description(self) -> &'static str {
        match self {
            Self::SleepPercentage => {
                "Percent of eligible players needed to skip the night (0-100)."
            }
            Self::SeasonSwapInterval => "In-game days per season (1-3650).",
            Self::GameTimeSpeed => "Multiplier 0.0001-1000; 0 pauses time, 1 is normal.",
        }
    }
    pub fn lookup(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.name() == name)
    }
    pub fn valid(self, value: f64) -> bool {
        value.is_finite()
            && match self {
                Self::SleepPercentage => (0.0..=100.0).contains(&value),
                Self::SeasonSwapInterval => (1.0..=3650.0).contains(&value) && value.fract() == 0.0,
                Self::GameTimeSpeed => value == 0.0 || (0.0001..=1000.0).contains(&value),
            }
    }
    pub fn value(self, world: &WorldConfig) -> f64 {
        match self {
            Self::SleepPercentage => world.game_rules.sleep_percentage,
            Self::SeasonSwapInterval => f64::from(world.seasons.spring_days),
            Self::GameTimeSpeed => world.game_rules.game_time_speed,
        }
    }
}

/// The existing world.toml owns these settings; region/player formats are unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldRuleSettings {
    pub game_rules: GameRules,
    pub seasons: SeasonConfig,
    pub time: WorldTimeConfig,
}
impl WorldRuleSettings {
    pub fn from_world(world: &WorldConfig) -> Self {
        Self {
            game_rules: world.game_rules,
            seasons: world.seasons,
            time: world.time,
        }
    }
    pub fn valid(self) -> bool {
        self.game_rules.valid() && self.seasons.valid() && self.time.valid()
    }
    pub fn apply(self, world: &mut WorldConfig) {
        world.game_rules = self.game_rules;
        world.seasons = self.seasons;
        world.time = self.time;
    }
    pub fn set(
        world: &WorldConfig,
        clock: &mut WorldClock,
        rule: GameRule,
        value: f64,
    ) -> Result<Self, &'static str> {
        if !rule.valid(value) {
            return Err("Invalid gamerule value. Use /gamerule list for limits.");
        }
        let mut next = Self::from_world(world);
        match rule {
            GameRule::SleepPercentage => next.game_rules.sleep_percentage = value,
            GameRule::GameTimeSpeed => {
                next.game_rules.game_time_speed = value;
                next.time.paused = false;
            }
            GameRule::SeasonSwapInterval => {
                let current = world.seasons.sample(*clock).season;
                let days = value as u32;
                next.seasons.spring_days = days;
                next.seasons.summer_days = days;
                next.seasons.autumn_days = days;
                next.seasons.winter_days = days;
                next.seasons.transition_days = 1.0;
                // Preserve the season and hour without rewinding the saved calendar.
                if next.seasons.enabled && !next.seasons.set_season(clock, current) {
                    return Err("The calendar cannot advance.");
                }
            }
        }
        next.configure_clock(clock);
        Ok(next)
    }
    pub fn configure_clock(self, clock: &mut WorldClock) {
        let speed = self.game_rules.game_time_speed;
        clock.paused = self.time.paused || speed == 0.0;
        clock.day_length_seconds =
            self.time.day_length_seconds / if speed > 0.0 { speed } else { 1.0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn values_validate_and_time_multiplier_preserves_the_current_instant() {
        let mut world = WorldConfig::default();
        let original = WorldClock::new(world.time);
        for speed in [2.0, 0.5, 0.0, 1.0] {
            let mut clock = original;
            let settings =
                WorldRuleSettings::set(&world, &mut clock, GameRule::GameTimeSpeed, speed).unwrap();
            assert_eq!((clock.day, clock.phase), (original.day, original.phase));
            clock.advance(10.0);
            assert!(
                (clock.phase - original.phase - 10.0 * speed / world.time.day_length_seconds).abs()
                    < 1e-10
            );
            settings.apply(&mut world);
            assert!(clock.valid());
        }
        for invalid in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(GameRule::ALL.into_iter().all(|r| !r.valid(invalid)));
        }
        assert!(!GameRule::SleepPercentage.valid(101.0));
        assert!(!GameRule::SeasonSwapInterval.valid(1.5));
        assert!(GameRule::SleepPercentage.valid(0.0));
    }
}
