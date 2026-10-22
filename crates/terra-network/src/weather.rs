use bevy::prelude::*;
use terra_core::{GameConfig, WorldClock, WorldWeather};

#[cfg(feature = "server")]
pub(crate) fn initialize(
    config: Res<GameConfig>,
    mut weather: ResMut<WorldWeather>,
    journal: Res<terra_world::edits::EditJournal>,
) {
    *weather = journal
        .weather
        .filter(|state| state.valid())
        .unwrap_or_else(|| WorldWeather::new(config.world.seed.0));
}

#[cfg(feature = "server")]
pub(crate) fn advance_server(
    time: Res<Time<Real>>,
    config: Res<GameConfig>,
    clock: Res<WorldClock>,
    mut weather: ResMut<WorldWeather>,
    mut journal: ResMut<terra_world::edits::EditJournal>,
) {
    if !clock.paused {
        weather.advance_seasonal(
            time.delta_secs_f64() * 1440.0 / clock.day_length_seconds,
            config.world.seed.0,
            *clock,
            config.world.seasons,
        );
    }
    if journal.weather != Some(*weather) {
        journal.bypass_change_detection().weather = Some(*weather);
    }
}

#[derive(Resource, Default)]
#[cfg(feature = "client")]
pub(crate) struct WeatherReplica(pub Option<WorldWeather>);

#[cfg(feature = "client")]
pub(crate) fn advance_client(
    time: Res<Time<Real>>,
    clock: Res<WorldClock>,
    config: Res<GameConfig>,
    mut weather: ResMut<WorldWeather>,
    mut replica: ResMut<WeatherReplica>,
) {
    if replica.0.is_none() || clock.paused {
        return;
    }
    let dt = time.delta_secs_f64() * 1440.0 / clock.day_length_seconds;
    weather.advance_seasonal(dt, config.world.seed.0, *clock, config.world.seasons);
    if let Some(target) = replica.0.as_mut() {
        target.advance_seasonal(dt, config.world.seed.0, *clock, config.world.seasons);
        // Snap only genuine resynchronization; correct clock skew gradually.
        let error = target.elapsed_total - weather.elapsed_total;
        if target.sequence != weather.sequence || error.abs() > 2.0 {
            *weather = *target;
        } else if error > 0.0 {
            weather.advance_seasonal(
                error * (1.0 - (-2.0 * dt).exp()),
                config.world.seed.0,
                *clock,
                config.world.seasons,
            );
        }
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn advancing_weather_does_not_dirty_terrain_and_pause_preserves_timing() {
        #[derive(Resource, Default)]
        struct Changed(bool);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(1),
            ))
            .init_resource::<GameConfig>()
            .init_resource::<WorldClock>()
            .init_resource::<WorldWeather>()
            .init_resource::<terra_world::edits::EditJournal>()
            .init_resource::<Changed>()
            .add_systems(
                Update,
                (
                    advance_server,
                    |journal: Res<terra_world::edits::EditJournal>,
                     mut observed: ResMut<Changed>| {
                        observed.0 = journal.is_changed();
                    },
                )
                    .chain(),
            );
        app.update();
        app.update();
        assert!(!app.world().resource::<Changed>().0);
        let before = *app.world().resource::<WorldWeather>();
        assert!(before.elapsed_total > 0.0);
        app.world_mut().resource_mut::<WorldClock>().paused = true;
        app.update();
        assert_eq!(*app.world().resource::<WorldWeather>(), before);
        assert_eq!(
            app.world()
                .resource::<terra_world::edits::EditJournal>()
                .weather,
            Some(before)
        );
    }
}
