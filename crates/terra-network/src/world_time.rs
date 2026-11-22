use bevy::prelude::*;
#[cfg(feature = "server")]
use terra_core::GameConfig;
use terra_core::WorldClock;
#[cfg(feature = "server")]
use terra_world::edits::EditJournal;

/// Unconsumed time actually played on this server. Restoring a save or setting
/// the clock by hand never adds to this counter.
#[cfg(feature = "server")]
#[derive(Resource, Default)]
pub(crate) struct PlayedDays {
    days: f64,
    pub(crate) slept: bool,
}

#[cfg(feature = "server")]
impl PlayedDays {
    fn advance(&mut self, from: WorldClock, to: WorldClock) {
        if from.valid() && to.valid() && to.day >= from.day {
            self.days += ((to.day - from.day) as f64 + (to.phase - from.phase)).max(0.);
        }
    }

    pub(crate) fn sleep(&mut self, skipped_days: f64) {
        if skipped_days > 0. && skipped_days.is_finite() {
            self.days += skipped_days;
            self.slept = true;
        }
    }

    pub(crate) fn take(&mut self) -> f64 {
        self.slept = false;
        std::mem::take(&mut self.days)
    }
}

#[cfg(feature = "server")]
pub(crate) fn initialize(
    config: Res<GameConfig>,
    mut clock: ResMut<WorldClock>,
    journal: Res<EditJournal>,
) {
    *clock = journal
        .clock
        .unwrap_or_else(|| WorldClock::new(config.world.time));
    // Saved calendar progress survives changes to the host's clock settings.
    terra_core::WorldRuleSettings::from_world(&config.world).configure_clock(&mut clock);
}
#[cfg(feature = "server")]
pub(crate) fn advance_server(
    time: Res<Time<Real>>,
    config: Res<GameConfig>,
    mut clock: ResMut<WorldClock>,
    mut journal: ResMut<EditJournal>,
    mut played: ResMut<PlayedDays>,
) {
    let mut next = *clock;
    terra_core::WorldRuleSettings::from_world(&config.world).configure_clock(&mut next);
    next.advance(time.delta_secs_f64());
    played.advance(*clock, next);
    if *clock != next {
        *clock = next;
    }
    if journal.clock != Some(*clock) {
        // Calendar metadata must not invalidate fluid/terrain change detection.
        journal.bypass_change_detection().clock = Some(*clock);
    }
}

#[derive(Resource, Default)]
#[cfg(feature = "client")]
pub(crate) struct ClockReplica(pub Option<WorldClock>);

#[cfg(feature = "client")]
pub(crate) fn advance_client(
    time: Res<Time<Real>>,
    mut clock: ResMut<WorldClock>,
    mut replica: ResMut<ClockReplica>,
) {
    let Some(target) = replica.0.as_mut() else {
        return;
    };
    let dt = time.delta_secs_f64();
    target.advance(dt);
    let mut next = *clock;
    next.advance(dt);
    let error = (target.day as f64 - next.day as f64) + target.phase - next.phase;
    if next.paused || error.abs() > 0.02 {
        next = *target;
    } else {
        let phase = next.phase + error * (1.0 - (-3.0 * dt).exp());
        if phase < 0.0 {
            next.day = next.day.saturating_sub(1);
        } else if phase >= 1.0 {
            next.day = next.day.saturating_add(1);
        }
        next.phase = phase.rem_euclid(1.0);
    }
    if *clock != next {
        *clock = next;
    }
}

#[cfg(all(test, feature = "client"))]
mod replica_tests {
    use super::*;

    #[test]
    fn two_clients_share_the_host_calendar_across_a_season_boundary() {
        use terra_core::Config;
        let host_config = terra_core::WorldConfig {
            seasons: terra_core::SeasonConfig {
                summer_days: 12,
                ..Default::default()
            },
            ..Default::default()
        };
        let host = WorldClock {
            day: 6,
            phase: 0.99,
            day_length_seconds: 60.0,
            ..Default::default()
        };
        let wire = serde_json::to_string(&(
            host_config,
            crate::protocol::session::ServerControl::WorldTime(host),
        ))
        .unwrap();
        let mut peers = Vec::new();
        for _ in 0..2 {
            let (config, snapshot): (
                terra_core::WorldConfig,
                crate::protocol::session::ServerControl,
            ) = serde_json::from_str(&wire).unwrap();
            config.validate().unwrap();
            let crate::protocol::session::ServerControl::WorldTime(clock) = snapshot else {
                panic!("clock snapshot")
            };
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_secs(1),
                ))
                .insert_resource(clock)
                .insert_resource(ClockReplica(Some(clock)))
                .add_systems(Update, advance_client);
            peers.push((app, config.seasons));
        }
        for _ in 0..10 {
            for (app, _) in &mut peers {
                app.update();
            }
        }
        let first = *peers[0].0.world().resource::<WorldClock>();
        let second = *peers[1].0.world().resource::<WorldClock>();
        assert_eq!(first, second);
        assert_eq!(first.season(peers[0].1), second.season(peers[1].1));
        assert_eq!(first.season(peers[0].1).season, terra_core::Season::Summer);
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    #[derive(Resource, Default)]
    struct JournalChanged(bool);

    #[test]
    fn saved_calendar_date_survives_host_clock_settings_and_uses_saved_seasons() {
        let mut app = App::new();
        let mut config = GameConfig::default();
        config.world.time.day_length_seconds = 7200.0;
        config.world.time.paused = true;
        config.world.seasons.winter_days = 14;
        let saved = WorldClock {
            day: 25,
            phase: 0.82,
            ..Default::default()
        };
        let expected = saved.season(config.world.seasons);
        let mut journal = EditJournal::default();
        journal.clock = Some(saved);
        app.insert_resource(config)
            .init_resource::<WorldClock>()
            .insert_resource(journal)
            .add_systems(Startup, initialize);
        app.update();
        let loaded = *app.world().resource::<WorldClock>();
        assert_eq!((loaded.day, loaded.phase), (saved.day, saved.phase));
        assert_eq!(loaded.day_length_seconds, 7200.0);
        assert!(loaded.paused);
        assert_eq!(
            loaded.season(app.world().resource::<GameConfig>().world.seasons),
            expected
        );
    }
    #[test]
    fn dragon_time_starts_from_the_saved_calendar_and_counts_only_days_actually_played() {
        let mut app = App::new();
        let saved = WorldClock {
            day: 1000,
            phase: 0.99,
            ..Default::default()
        };
        let mut config = GameConfig::default();
        config.world.time.day_length_seconds = 60.;
        config.world.time.paused = true;
        let mut journal = EditJournal::default();
        journal.clock = Some(saved);
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(3),
            ))
            .insert_resource(config)
            .insert_resource(journal)
            .init_resource::<WorldClock>()
            .init_resource::<PlayedDays>()
            .add_systems(Startup, initialize)
            .add_systems(Update, advance_server);
        app.update();
        app.update();
        assert_eq!(app.world_mut().resource_mut::<PlayedDays>().take(), 0.);
        // A clock-setting command in a paused world changes the date without
        // playing those hours. On resuming we count just the actual midnight crossing.
        app.world_mut().resource_mut::<WorldClock>().phase = 0.98;
        app.world_mut()
            .resource_mut::<GameConfig>()
            .world
            .time
            .paused = false;
        app.update();
        assert_eq!(app.world().resource::<WorldClock>().day, 1001);
        assert!((app.world_mut().resource_mut::<PlayedDays>().take() - 0.05).abs() < 1e-12);
        app.world_mut()
            .resource_mut::<GameConfig>()
            .world
            .time
            .day_length_seconds = 120.;
        app.update();
        assert!((app.world_mut().resource_mut::<PlayedDays>().take() - 0.025).abs() < 1e-12);
        app.world_mut()
            .resource_mut::<GameConfig>()
            .world
            .time
            .paused = true;
        app.update();
        assert_eq!(app.world_mut().resource_mut::<PlayedDays>().take(), 0.);
    }

    #[test]
    fn advancing_calendar_does_not_invalidate_terrain_or_fluid_jobs() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(1),
            ))
            .init_resource::<GameConfig>()
            .init_resource::<WorldClock>()
            .init_resource::<EditJournal>()
            .init_resource::<JournalChanged>()
            .init_resource::<PlayedDays>()
            .add_systems(
                Update,
                (
                    advance_server,
                    |journal: Res<EditJournal>, mut changed: ResMut<JournalChanged>| {
                        changed.0 = journal.is_changed();
                    },
                )
                    .chain(),
            );
        app.update();
        let before = app.world().resource::<WorldClock>().phase;
        app.update();
        assert!(app.world().resource::<WorldClock>().phase > before);
        assert!(!app.world().resource::<JournalChanged>().0);
        assert_eq!(
            app.world().resource::<EditJournal>().clock,
            Some(*app.world().resource::<WorldClock>())
        );
    }
}
