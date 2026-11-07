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
    gameplay_revision: u64,
    last: Instant,
}
impl DedicatedPersistence {
    pub fn install(
        app: &mut App,
        store: terra_storage::DedicatedStore,
        mut journal: EditJournal,
        world: WorldId,
    ) {
        if let Some(config) = app.world().get_resource::<terra_core::GameConfig>()
            && config.server.kingdom_rules.is_some()
        {
            journal.world_rules = Some(terra_core::WorldRuleSettings::from_world(&config.world));
        }
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
                gameplay_revision: 0,
                last: Instant::now(),
            })
            .add_systems(Last, (finish_felling_on_exit, save).chain());
    }
}

pub(crate) fn finish_tree_felling(world: &mut World) -> Result<(), String> {
    if !world.contains_resource::<terra_world::tree_felling::Queue>() {
        return Ok(());
    }
    world.resource_scope(|world, mut queue: Mut<terra_world::tree_felling::Queue>| {
        world.resource_scope(
            |world, mut chunks: Mut<terra_world::streaming::ChunkStore>| {
                world.resource_scope(|world, mut journal: Mut<EditJournal>| {
                    queue.finish(
                        &mut chunks,
                        &mut journal,
                        world.resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
                    )
                })
            },
        )
    })
}

fn finish_felling_on_exit(world: &mut World) {
    if world.resource::<Messages<AppExit>>().is_empty() {
        return;
    }
    if let Err(error) = finish_tree_felling(world) {
        tracing::error!(%error, "Cannot finish pending tree felling before final save");
        world.write_message(AppExit::error());
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
        || persistence.player_revision != journal.player_state_revision()
        || persistence.gameplay_revision != journal.gameplay().revision;
    persistence.clock = journal.clock;
    persistence.weather = journal.weather;
    persistence.world_rules = journal.world_rules;
    persistence.player_revision = journal.player_state_revision();
    persistence.gameplay_revision = journal.gameplay().revision;
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
    fn explicit_startup_policy_overwrites_old_journal_rules_and_survives_real_final_save() {
        let directory = std::env::temp_dir().join(format!(
            "terra-startup-policy-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let mut config = terra_core::GameConfig::default();
        config.world.structures_revision = 8;
        config.world.structure_border_gap = 16;
        config.world.kingdom_rules.realm_min_border_gap = 6;
        let (store, mut journal) =
            terra_storage::DedicatedStore::open(&directory, &config.world).unwrap();
        journal.world_rules = Some(terra_core::WorldRuleSettings::from_world(&config.world));
        let mut rules = config.world.kingdom_rules;
        rules.realm_min_border_gap = 42;
        rules.kingdom_build_base = 73;
        config.server.kingdom_rules = Some(rules);
        config.world.kingdom_rules = rules;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(config.clone());
        DedicatedPersistence::install(&mut app, store, journal, config.world.id);
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .world_rules
                .unwrap()
                .kingdom_rules,
            rules
        );
        app.world_mut().write_message(AppExit::Success);
        app.update();
        drop(app);
        let restored = terra_storage::DedicatedStore::resolve_world_config(
            &directory,
            &terra_core::WorldConfig::default(),
        )
        .unwrap();
        assert_eq!(restored.kingdom_rules, rules);
        assert_eq!(restored.structures_revision, 8);
        assert_eq!(restored.structure_border_gap, 16);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn paused_clock_autosaves_player_location_without_terrain_invalidation() {
        let directory = std::env::temp_dir().join(format!(
            "terra-location-worker-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = terra_core::WorldConfig::default();
        let (store, mut journal) =
            terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        journal.clock = Some(terra_core::WorldClock {
            paused: true,
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        DedicatedPersistence::install(&mut app, store, journal, config.id);
        #[derive(Resource, Default)]
        struct TerrainChanged(bool);
        app.init_resource::<TerrainChanged>().add_systems(
            Update,
            |journal: Res<EditJournal>, mut changed: ResMut<TerrainChanged>| {
                changed.0 = journal.is_changed()
            },
        );
        app.update();
        {
            let mut persistence = app.world_mut().resource_mut::<DedicatedPersistence>();
            persistence.dirty = false;
            persistence.last = Instant::now() - Duration::from_secs(11);
        }
        let player = terra_core::PlayerUuid::new_v4();
        let location = terra_core::PlayerLocation {
            position: [-42.5, -28.08, -17.5],
            look: [0.4, -0.2],
            flying: false,
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .bypass_change_detection()
            .update_player_location(player, location)
            .unwrap();
        app.update();
        assert!(!app.world().resource::<TerrainChanged>().0);
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
        drop(app);
        let (_, loaded) = terra_storage::DedicatedStore::open(&directory, &config).unwrap();
        assert_eq!(loaded.player_location(player), Some(location));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn paused_clock_autosaves_crusher_progress_and_output_without_marking_terrain_changed() {
        use terra_core::{Inventory, ItemKind, ItemStack, MineralItem, crusher};
        use terra_world::edits::BlockObjectState;
        let directory = std::env::temp_dir().join(format!(
            "terra-crusher-worker-{}",
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
        let anchor = [4, 5, 4];
        let peer = [5, 5, 4];
        for (position, other, secondary) in [(anchor, peer, false), (peer, anchor, true)] {
            journal.record(position, crusher::MATERIAL).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        peer: Some(other),
                        secondary,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        journal.set_crusher(anchor, Default::default()).unwrap();
        journal
            .set_container(
                anchor,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Mineral(MineralItem::RawIron),
                        count: 3,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
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
        let mut inventory = app
            .world()
            .resource::<EditJournal>()
            .container_at(anchor)
            .unwrap()
            .clone();
        let mut state = app
            .world()
            .resource::<EditJournal>()
            .crusher_at(anchor)
            .unwrap()
            .clone();
        for _ in 0..4 {
            assert!(crusher::advance(&mut state, &mut inventory, 1.0));
        }
        assert!(crusher::advance(&mut state, &mut inventory, 0.5));
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress_seconds, 0.5);
        assert_eq!(inventory.slot(crusher::INPUT).unwrap().count, 2);
        assert_eq!(
            inventory.slot(crusher::OUTPUT).unwrap().item,
            ItemKind::Dust(terra_core::DustKind::Iron)
        );
        assert!(
            app.world_mut()
                .resource_mut::<EditJournal>()
                .bypass_change_detection()
                .commit_crusher(anchor, state.clone(), inventory.clone())
                .unwrap()
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
        assert_eq!(saved.crusher_at(anchor), Some(&state));
        assert_eq!(saved.container_at(anchor), Some(&inventory));
        assert_eq!(saved.clock, clock);
        std::fs::remove_dir_all(directory).unwrap();
    }

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
