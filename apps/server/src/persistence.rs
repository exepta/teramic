use bevy::{
    app::AppExit,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::WorldId;
use terra_world::edits::EditJournal;

#[derive(Resource)]
pub struct DedicatedPersistence {
    _store: terra_storage::DedicatedStore,
    path: PathBuf,
    world: WorldId,
    task: Option<Task<Result<(), String>>>,
    dirty: bool,
    clock: Option<terra_core::WorldClock>,
    weather: Option<terra_core::WorldWeather>,
    world_rules: Option<terra_core::WorldRuleSettings>,
    player_revision: u64,
    last: Instant,
}
impl DedicatedPersistence {
    pub fn install(
        app: &mut App,
        store: terra_storage::DedicatedStore,
        journal: EditJournal,
        world: WorldId,
    ) {
        app.insert_resource(journal)
            .insert_resource(Self {
                path: store.edits_path.clone(),
                _store: store,
                world,
                task: None,
                dirty: false,
                clock: None,
                weather: None,
                world_rules: None,
                player_revision: 0,
                last: Instant::now(),
            })
            .add_systems(Last, save);
    }
}
fn save(
    mut persistence: ResMut<DedicatedPersistence>,
    journal: Res<EditJournal>,
    mut exit: ParamSet<(MessageReader<AppExit>, MessageWriter<AppExit>)>,
) {
    persistence.dirty |= journal.is_changed()
        || persistence.clock != journal.clock
        || persistence.weather != journal.weather
        || persistence.world_rules != journal.world_rules
        || persistence.player_revision != journal.player_state_revision();
    persistence.clock = journal.clock;
    persistence.weather = journal.weather;
    persistence.world_rules = journal.world_rules;
    persistence.player_revision = journal.player_state_revision();
    if exit.p0().read().next().is_some() {
        if let Some(task) = persistence.task.take() {
            let _ = future::block_on(task);
        }
        if let Err(error) =
            terra_storage::save_edits(&persistence.path, persistence.world, &journal)
        {
            tracing::error!(%error, "Dedicated final save failed");
            exit.p1().write(AppExit::error());
        } else {
            tracing::info!("Dedicated final save completed");
        }
        return;
    }
    let finished = persistence
        .task
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)));
    if let Some(result) = finished {
        persistence.task = None;
        if let Err(error) = result {
            tracing::error!(%error, "Dedicated autosave failed; retrying after interval");
            persistence.dirty = true;
        }
    }
    if persistence.dirty
        && persistence.task.is_none()
        && persistence.last.elapsed() >= Duration::from_secs(10)
    {
        let path = persistence.path.clone();
        let world = persistence.world;
        let snapshot = journal.clone();
        persistence.task = Some(
            AsyncComputeTaskPool::get()
                .spawn(async move { terra_storage::save_edits(&path, world, &snapshot) }),
        );
        persistence.dirty = false;
        persistence.last = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_clock_autosaves_vitals_without_marking_terrain_changed() {
        let directory = std::env::temp_dir().join(format!(
            "terra-survival-worker-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = terra_core::WorldConfig::default();
        let (store, mut journal) =
            terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        journal.clock = Some(terra_core::WorldClock {
            paused: true,
            ..Default::default()
        });
        let clock = journal.clock;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        DedicatedPersistence::install(&mut app, store, journal, config.id);
        #[derive(Resource, Default)]
        struct TerrainChanged(bool);
        app.init_resource::<TerrainChanged>().add_systems(
            Update,
            |journal: Res<EditJournal>, mut observed: ResMut<TerrainChanged>| {
                observed.0 = journal.is_changed();
            },
        );
        app.update();
        {
            let mut persistence = app.world_mut().resource_mut::<DedicatedPersistence>();
            persistence.dirty = false;
            persistence.last = Instant::now() - Duration::from_secs(11);
        }
        let player = terra_core::PlayerUuid::new_v4();
        let stats = terra_core::PlayerStats {
            deaths: 2,
            survival: terra_core::SurvivalVitals {
                health: 41.0,
                hunger: 56.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            app.world_mut()
                .resource_mut::<EditJournal>()
                .bypass_change_detection()
                .update_player_stats(player, stats)
        );
        app.update();
        assert!(!app.world().resource::<TerrainChanged>().0);
        assert_eq!(app.world().resource::<EditJournal>().clock, clock);
        assert!(
            app.world()
                .resource::<DedicatedPersistence>()
                .task
                .is_some()
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while app
            .world()
            .resource::<DedicatedPersistence>()
            .task
            .is_some()
        {
            std::thread::sleep(Duration::from_millis(1));
            app.update();
            assert!(Instant::now() < deadline);
        }
        assert!(!app.world().resource::<DedicatedPersistence>().dirty);
        drop(app);
        let (_, saved) = terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        assert_eq!(saved.gameplay().player_stats[&player], stats);
        assert_eq!(saved.clock, clock);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn shutdown_waits_for_pending_autosave_and_commits_newer_edits() {
        let directory = std::env::temp_dir().join(format!(
            "terra-save-worker-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = terra_core::WorldConfig::default();
        let (store, journal) = terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        DedicatedPersistence::install(&mut app, store, journal, config.id);
        app.update();
        app.world_mut().resource_mut::<DedicatedPersistence>().last =
            Instant::now() - Duration::from_secs(11);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([1, 0, 1], 0)
            .unwrap();
        app.update();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([2, 0, 1], 0)
            .unwrap();
        app.world_mut().write_message(AppExit::Success);
        app.update();
        drop(app);
        let (_, loaded) = terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        assert_eq!(loaded.edited_blocks(), 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
