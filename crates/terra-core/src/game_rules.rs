//! Mutable world policy. Terrain identity and voxel generation are not affected.
use crate::{SeasonConfig, WorldClock, WorldConfig, WorldTimeConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameRules {
    /// Missing fields retain the historical keep-on-death behavior of saved worlds.
    pub keep_inventory: bool,
    pub daylight_cycle: bool,
    pub weather_cycle: bool,
    pub mob_griefing: bool,
    pub fire_spread: bool,
    pub pvp: bool,
    pub sleep_percentage: f64,
    pub game_time_speed: f64,
}
impl Default for GameRules {
    fn default() -> Self {
        Self {
            keep_inventory: true,
            daylight_cycle: true,
            weather_cycle: true,
            mob_griefing: true,
            fire_spread: true,
            pvp: false,
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
    KeepInventory,
    DaylightCycle,
    WeatherCycle,
    MobGriefing,
    FireSpread,
    Pvp,
}
impl GameRule {
    pub const ALL: [Self; 9] = [
        Self::SleepPercentage,
        Self::SeasonSwapInterval,
        Self::GameTimeSpeed,
        Self::KeepInventory,
        Self::DaylightCycle,
        Self::WeatherCycle,
        Self::MobGriefing,
        Self::FireSpread,
        Self::Pvp,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::SleepPercentage => "sleep_percentage",
            Self::SeasonSwapInterval => "season_swap_interval",
            Self::GameTimeSpeed => "game_time_speed",
            Self::KeepInventory => "keep_inventory",
            Self::DaylightCycle => "daylight_cycle",
            Self::WeatherCycle => "weather_cycle",
            Self::MobGriefing => "mob_griefing",
            Self::FireSpread => "fire_spread",
            Self::Pvp => "pvp",
        }
    }
    pub const fn description(self) -> &'static str {
        match self {
            Self::SleepPercentage => {
                "Percent of eligible players needed to skip the night (0-100)."
            }
            Self::SeasonSwapInterval => "In-game days per season (1-3650).",
            Self::GameTimeSpeed => "Multiplier 0.0001-1000; 0 pauses time, 1 is normal.",
            Self::KeepInventory => "Keep items and equipment on death (0 off, 1 on).",
            Self::DaylightCycle => "Advance the day/night calendar automatically (0 off, 1 on).",
            Self::WeatherCycle => "Advance weather automatically (0 off, 1 on).",
            Self::MobGriefing => "Allow animal grazing, pollination and spider webs (0 off, 1 on).",
            Self::FireSpread => "Allow fire to ignite neighboring blocks (0 off, 1 on).",
            Self::Pvp => "Allow players to damage other Survival players (0 off, 1 on).",
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
                Self::KeepInventory
                | Self::DaylightCycle
                | Self::WeatherCycle
                | Self::MobGriefing
                | Self::FireSpread
                | Self::Pvp => value == 0.0 || value == 1.0,
            }
    }
    pub fn value(self, world: &WorldConfig) -> f64 {
        match self {
            Self::SleepPercentage => world.game_rules.sleep_percentage,
            Self::SeasonSwapInterval => f64::from(world.seasons.spring_days),
            Self::GameTimeSpeed => world.game_rules.game_time_speed,
            Self::KeepInventory => f64::from(u8::from(world.game_rules.keep_inventory)),
            Self::DaylightCycle => f64::from(u8::from(world.game_rules.daylight_cycle)),
            Self::WeatherCycle => f64::from(u8::from(world.game_rules.weather_cycle)),
            Self::MobGriefing => f64::from(u8::from(world.game_rules.mob_griefing)),
            Self::FireSpread => f64::from(u8::from(world.game_rules.fire_spread)),
            Self::Pvp => f64::from(u8::from(world.game_rules.pvp)),
        }
    }
}

/// The existing world.toml owns these settings; region/player formats are unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldRuleSettings {
    pub game_rules: GameRules,
    pub seasons: SeasonConfig,
    pub time: WorldTimeConfig,
    #[serde(default)]
    pub kingdom_rules: crate::kingdom::KingdomRules,
}
impl WorldRuleSettings {
    pub fn from_world(world: &WorldConfig) -> Self {
        Self {
            game_rules: world.game_rules,
            seasons: world.seasons,
            time: world.time,
            kingdom_rules: world.kingdom_rules,
        }
    }
    pub fn valid(self) -> bool {
        self.game_rules.valid()
            && self.seasons.valid()
            && self.time.valid()
            && self.kingdom_rules.valid()
    }
    pub fn apply(self, world: &mut WorldConfig) {
        world.game_rules = self.game_rules;
        world.seasons = self.seasons;
        world.time = self.time;
        world.kingdom_rules = self.kingdom_rules;
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
            GameRule::KeepInventory => next.game_rules.keep_inventory = value == 1.0,
            GameRule::DaylightCycle => next.game_rules.daylight_cycle = value == 1.0,
            GameRule::WeatherCycle => next.game_rules.weather_cycle = value == 1.0,
            GameRule::MobGriefing => next.game_rules.mob_griefing = value == 1.0,
            GameRule::FireSpread => next.game_rules.fire_spread = value == 1.0,
            GameRule::Pvp => next.game_rules.pvp = value == 1.0,
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
        clock.paused = self.time.paused || speed == 0.0 || !self.game_rules.daylight_cycle;
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

#[cfg(test)]
mod boolean_tests {
    use super::*;
    #[test]
    fn legacy_rules_preserve_behavior_and_boolean_options_round_trip() {
        let legacy: GameRules =
            toml::from_str("sleep_percentage = 75.0\ngame_time_speed = 1.0").unwrap();
        assert!(
            legacy.keep_inventory
                && legacy.daylight_cycle
                && legacy.weather_cycle
                && legacy.mob_griefing
                && legacy.fire_spread
                && !legacy.pvp
        );
        let mut world = WorldConfig::default();
        let mut clock = WorldClock::new(world.time);
        for rule in [
            GameRule::KeepInventory,
            GameRule::DaylightCycle,
            GameRule::WeatherCycle,
            GameRule::MobGriefing,
            GameRule::FireSpread,
            GameRule::Pvp,
        ] {
            assert!(!rule.valid(0.5));
            for value in [0.0, 1.0] {
                let policy = WorldRuleSettings::set(&world, &mut clock, rule, value).unwrap();
                let saved = toml::to_string(&policy).unwrap();
                let restored: WorldRuleSettings = toml::from_str(&saved).unwrap();
                assert_eq!(restored, policy);
                restored.apply(&mut world);
                assert_eq!(rule.value(&world), value);
            }
        }
    }
    #[test]
    fn daylight_toggle_freezes_the_calendar_and_resumes_without_rewinding() {
        let mut world = WorldConfig::default();
        let mut clock = WorldClock::new(world.time);
        clock.advance(10.0);
        let instant = (clock.day, clock.phase);
        WorldRuleSettings::set(&world, &mut clock, GameRule::DaylightCycle, 0.0)
            .unwrap()
            .apply(&mut world);
        clock.advance(1000.0);
        assert_eq!((clock.day, clock.phase), instant);
        WorldRuleSettings::set(&world, &mut clock, GameRule::DaylightCycle, 1.0)
            .unwrap()
            .apply(&mut world);
        clock.advance(10.0);
        assert!(clock.phase > instant.1);
    }
}
