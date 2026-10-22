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
        config: GameConfig,
        save_path: Option<std::path::PathBuf>,
    ) -> Result<(Self, Vec<u8>), String> {
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
                    app.update();
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
                    terra_storage::save_edits(
                        path,
                        world_id,
                        app.world().resource::<terra_world::edits::EditJournal>(),
                    )?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        match receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok((address, token))) => Ok((
                Self {
                    stop,
                    thread: Some(thread),
                    address,
                    save,
                },
                token,
            )),
            result => {
                stop.store(true, Ordering::Release);
                let _ = thread.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.to_string(),
                    _ => "Integrated server failed".into(),
                })
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
