use bevy::prelude::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use terra_core::{Config, GameConfig};
use terra_network::server::{ServerEndpoint, ServerRuntime};

/// Owns a separate headless Bevy World; dropping it stops and joins the server.
#[derive(Resource)]
pub struct IntegratedServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    pub address: std::net::SocketAddr,
    save: mpsc::SyncSender<mpsc::SyncSender<Result<(), String>>>,
}
impl IntegratedServer {
    pub fn start(config: GameConfig) -> Result<(Self, Vec<u8>), String> {
        Self::start_with_save(config, None)
    }
    pub fn start_with_save(
        mut config: GameConfig,
        save_path: Option<std::path::PathBuf>,
    ) -> Result<(Self, Vec<u8>), String> {
        // Dedicated configuration is unrelated to this singleplayer world,
        // including validation of an override that this server never applies.
        config.server.kingdom_rules = None;
        config.server.validate().map_err(|e| e.to_string())?;
        config.world.validate().map_err(|e| e.to_string())?;
        let world_id = config.world.id;
        let save_lock = save_path
            .as_ref()
            .map(|path| terra_storage::lock_world(path))
            .transpose()?;
        let journal = save_path
            .as_ref()
            .map(|path| terra_storage::load_edits(path, world_id))
            .transpose()?;
        let tick = Duration::from_secs_f64(1.0 / f64::from(config.server.tick_rate));
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let (save, saves) = mpsc::sync_channel::<mpsc::SyncSender<Result<(), String>>>(1);
        let thread = thread::Builder::new()
            .name("terra-integrated-server".into())
            .spawn(move || {
                let _save_lock = save_lock;
                let mut config = config;
                config.server.lan_discovery = false;
                config.server.require_verified_accounts = false;
                let mut app = crate::bootstrap::build_server(config, false);
                if let Some(journal) = journal {
                    app.insert_resource(journal);
                }
                app.insert_resource(ServerEndpoint {
                    bind: ([127, 0, 0, 1], 0).into(),
                    advertised: None,
                });
                app.finish();
                app.cleanup();
                isolate_simulation(&mut app);
                app.update();
                let runtime = app.world().resource::<ServerRuntime>();
                let result = runtime.error.clone().map_or_else(
                    || {
                        runtime
                            .address
                            .ok_or_else(|| "Integrated server did not bind".to_string())
                            .and_then(|address| {
                                runtime.issue_token(1).map(|token| (address, token))
                            })
                    },
                    Err,
                );
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    return Ok(());
                }
                let mut last_save = Instant::now();
                use bevy::tasks::{AsyncComputeTaskPool, Task, futures_lite::future};
                let mut save_task: Option<Task<Result<(), String>>> = None;
                let mut requested = None;
                while !signal.load(Ordering::Acquire) {
                    let started = Instant::now();
                    {
                        let _scope = terra_core::profiling::ScopeTimer::new("integrated.tick");
                        app.update();
                    }
                    if requested.is_none() {
                        requested = saves.try_recv().ok();
                    }
                    if let Some(result) = save_task.as_mut().and_then(|task| future::block_on(future::poll_once(task))) {
                        save_task = None;
                        if let Err(error) = result {
                            tracing::error!(%error, "World autosave failed; final save will be retried on exit");
                        }
                    }
                    if save_task.is_none() && (requested.is_some() || last_save.elapsed() > Duration::from_secs(10)) {
                        let reply = requested.take();
                        if let Some(path) = save_path.clone() {
                            let snapshot = {
                                let _scope = terra_core::profiling::ScopeTimer::new("integrated.autosave.snapshot");
                                app.world().resource::<terra_world::edits::EditJournal>().clone()
                            };
                            save_task = Some(AsyncComputeTaskPool::get().spawn(async move {
                                let result = terra_storage::save_edits(&path, world_id, &snapshot);
                                if let Some(reply) = reply { let _ = reply.send(result.clone()); }
                                result
                            }));
                        } else if let Some(reply) = reply {
                            let _ = reply.send(Err("No persistent world is active".into()));
                        }
                        last_save = Instant::now();
                    }
                    if let Some(remaining) = tick.checked_sub(started.elapsed()) {
                        thread::sleep(remaining);
                    }
                }
                if let Some(task) = save_task { let _ = future::block_on(task); }
                if let Some(path) = &save_path {
                    let felling = crate::persistence::finish_tree_felling(app.world_mut());
                    terra_storage::save_edits(
                        path,
                        world_id,
                        app.world().resource::<terra_world::edits::EditJournal>(),
                    )?;
                    felling?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        // The first update selects a safe spawn before binding. Its cost depends
        // on the saved world; the worker reports either readiness or the actual error.
        let ready = receiver.recv().unwrap_or_else(|_| {
            Err("Integrated server exited before reporting readiness".to_string())
        });
        match ready {
            Ok((address, token)) => Ok((
                Self {
                    stop,
                    thread: Some(thread),
                    address,
                    save,
                },
                token,
            )),
            Err(error) => {
                thread
                    .join()
                    .map_err(|_| "Integrated server thread terminated during startup")??;
                Err(error)
            }
        }
    }
    pub fn finish(mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| "Integrated server thread terminated unexpectedly".to_string())?,
            None => Ok(()),
        }
    }
    pub fn request_save(&self) -> Result<mpsc::Receiver<Result<(), String>>, String> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.save
            .try_send(reply)
            .map_err(|e| format!("Cannot queue world save: {e}"))?;
        Ok(receiver)
    }
}

/// Keep synchronous server systems on their owning OS thread. Client parallel
/// queries otherwise can steal a long server simulation job from the shared
/// ComputeTaskPool while waiting for their own tiny visibility/material jobs.
/// Explicit generation, collision cooking and save jobs retain their async pool.
fn isolate_simulation(app: &mut App) {
    use bevy::ecs::schedule::{Schedules, SingleThreadedExecutor};
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}
impl Drop for IntegratedServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            match thread.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::error!(%error,"Cannot save integrated world"),
                Err(_) => tracing::error!("Integrated server thread terminated unexpectedly"),
            }
        }
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;

    #[test]
    fn integrated_systems_and_nested_schedules_stay_on_the_server_thread() {
        use bevy::ecs::schedule::ScheduleLabel;
        #[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
        struct Nested;
        #[derive(Resource, Default)]
        struct Threads(Vec<std::thread::ThreadId>);
        fn capture(mut threads: ResMut<Threads>) {
            threads.0.push(std::thread::current().id());
        }
        let mut app = App::new();
        app.init_resource::<Threads>()
            .add_systems(Update, capture)
            .add_systems(FixedPostUpdate, capture)
            .add_systems(Nested, capture);
        isolate_simulation(&mut app);
        app.update();
        app.world_mut().run_schedule(FixedPostUpdate);
        app.world_mut().run_schedule(Nested);
        assert_eq!(
            app.world().resource::<Threads>().0,
            vec![std::thread::current().id(); 3]
        );
    }
    #[test]
    fn actual_singleplayer_join_and_save_keep_world_custom_rules_and_ignore_dedicated_override() {
        let root = std::env::temp_dir().join(format!(
            "terra-singleplayer-policy-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let library = terra_storage::WorldLibrary { root: root.clone() };
        let template = terra_core::WorldConfig {
            trees: false,
            kingdom_rules: terra_core::kingdom::KingdomRules {
                realm_min_border_gap: 5,
                kingdom_build_base: 71,
                require_adjacent_claims: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let entry = library
            .create(
                "Singleplayer rules",
                Some(202202),
                terra_core::GameMode::Creative,
                &template,
            )
            .unwrap();
        let mut invalid_override = template.kingdom_rules;
        invalid_override.kingdom_build_base = 0;
        let config = GameConfig {
            world: entry.manifest.world.clone(),
            server: terra_core::ServerConfig {
                kingdom_rules: Some(invalid_override),
                ..Default::default()
            },
            ..Default::default()
        };
        let path = library.path(config.world.id);
        let (server, token) =
            IntegratedServer::start_with_save(config.clone(), Some(path)).unwrap();
        // This client has no renderer or window; exercise the real welcome/rules wire path.
        let mut client = App::new();
        let mut client_config = GameConfig::default();
        client_config.client.view_distance = 1;
        client_config.client.vertical_view_distance = 1;
        client
            .add_plugins((MinimalPlugins, TransformPlugin))
            .add_plugins(terra_core::TerraCorePlugin(client_config))
            .add_plugins((
                terra_world::TerraWorldPlugin {
                    mode: terra_world::WorldMode::Client,
                },
                terra_physics::TerraPhysicsPlugin,
                terra_network::TerraNetworkPlugin(terra_network::connection::NetworkRole::Client),
            ))
            .insert_resource(terra_network::client::ClientLaunch { token });
        client.finish();
        client.cleanup();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            client.update();
            if client
                .world()
                .resource::<terra_network::client::ClientSession>()
                .ready
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "headless singleplayer join timed out"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let received = &client.world().resource::<GameConfig>().world;
        assert_eq!(received.kingdom_rules, config.world.kingdom_rules);
        assert_eq!(received.structure_border_gap, 5);
        assert_eq!(
            received.structures_revision,
            terra_core::WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        drop(client);
        server
            .request_save()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        server.finish().unwrap();
        let reloaded = library.load(config.world.id).unwrap();
        assert_eq!(
            reloaded.manifest.world.kingdom_rules,
            config.world.kingdom_rules
        );
        assert_eq!(reloaded.manifest.world.structure_border_gap, 5);
        assert!(reloaded.manifest.world.same_terrain(&config.world));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn integrated_server_serializes_manual_background_save_and_reload_without_another_writer() {
        let root = std::env::temp_dir().join(format!(
            "terra-integrated-regions-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = GameConfig::default();
        let mut journal = terra_world::edits::EditJournal::default();
        journal.record([-1, 250, -1], 27).unwrap();
        journal.clock = Some(terra_core::WorldClock {
            day: 19,
            phase: 0.25,
            ..Default::default()
        });
        let player = terra_core::PlayerUuid::new_v4();
        journal
            .gameplay_mut()
            .inventories
            .entry(player)
            .or_default()
            .insert(terra_core::ItemKind::Block(3), 128);
        terra_storage::save_edits(&root, config.world.id, &journal).unwrap();
        let (server, _) =
            IntegratedServer::start_with_save(config.clone(), Some(root.clone())).unwrap();
        assert!(IntegratedServer::start_with_save(config.clone(), Some(root.clone())).is_err());
        server
            .request_save()
            .unwrap()
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        server.finish().unwrap();
        let loaded = terra_storage::load_edits(&root, config.world.id).unwrap();
        let clock = loaded.clock.unwrap();
        assert_eq!(clock.day, 19);
        assert!((0.25..0.26).contains(&clock.phase));
        assert_eq!(loaded.block([-1, 250, -1]), Some(27));
        assert!(loaded.fluid([-1, 250, -1]).unwrap().source);
        assert_eq!(
            loaded.gameplay().inventories[&player].count(terra_core::ItemKind::Block(3)),
            128
        );
        assert!(root.join(format!("players/{player}.data")).is_file());
        assert!(!root.join("edits.toml").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
