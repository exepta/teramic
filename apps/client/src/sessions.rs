use bevy::{
    app::AppExit,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::HashMap,
    net::ToSocketAddrs,
    time::{Duration, Instant},
};
use terra_core::GameConfig;
use terra_network::client::{ClientSession, connect_session, disconnect_session};
use terra_server::IntegratedServer;
use terra_ui::menu::{
    LoadingProgress, MenuPage, MenuPaths, MenuState, ServerMenu, SessionInfo, SessionRequest,
};

struct PreparedSession {
    host: Option<IntegratedServer>,
    token: Vec<u8>,
    info: SessionInfo,
}
#[derive(Resource, Default)]
struct SessionJobs {
    start: Option<Task<Result<PreparedSession, String>>>,
    stop: Option<Task<Result<(), String>>>,
    closing: Option<(bool, Instant)>,
    save: Option<Task<Result<(), String>>>,
    loading_updated: Option<Instant>,
    reveal_elapsed: Option<Duration>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<SessionJobs>()
        .add_systems(
            Update,
            update.after(terra_world::streaming::StreamingSet::Dispatch),
        )
        .add_systems(PostUpdate, update_loading)
        .add_systems(Last, limit_fps);
}
fn update(world: &mut World) {
    let requests: Vec<_> = world
        .resource_mut::<Messages<SessionRequest>>()
        .drain()
        .collect();
    for request in requests {
        let save = matches!(request, SessionRequest::Save);
        let result = start_request(world, request);
        if let Err(error) = result {
            if save {
                world.resource_mut::<MenuState>().error = error;
            } else {
                fail(world, error);
            }
        }
    }
    let ready = world
        .resource_mut::<SessionJobs>()
        .start
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)));
    if let Some(result) = ready {
        world.resource_mut::<SessionJobs>().start = None;
        match result {
            Ok(prepared) => {
                world.insert_resource(prepared.info);
                if let Some(host) = prepared.host {
                    world.insert_resource(host);
                }
                if world.resource::<SessionJobs>().closing.is_none() {
                    connect_session(world, prepared.token);
                    world.resource_mut::<MenuState>().session_active = true;
                    world.resource_mut::<MenuState>().status = "Connecting to server ...".into();
                }
            }
            Err(error) => fail(world, error),
        }
    }
    let saved = world
        .resource_mut::<SessionJobs>()
        .save
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)));
    if let Some(result) = saved {
        world.resource_mut::<SessionJobs>().save = None;
        world.resource_mut::<SessionInfo>().saving = false;
        match result {
            Ok(()) => world.resource_mut::<MenuState>().status = "World saved".into(),
            Err(error) => {
                world.resource_mut::<MenuState>().error = format!("World save failed: {error}")
            }
        }
    }
    if let Some((_, since)) = world.resource::<SessionJobs>().closing {
        let jobs = world.resource::<SessionJobs>();
        let session = world.resource::<ClientSession>();
        if jobs.start.is_none()
            && jobs.stop.is_none()
            && jobs.save.is_none()
            && (!session.edits_pending()
                || session.error.is_some()
                || since.elapsed() > Duration::from_secs(5))
        {
            let host = world.remove_resource::<IntegratedServer>();
            world.resource_mut::<SessionJobs>().stop = Some(
                AsyncComputeTaskPool::get()
                    .spawn(async move { host.map_or(Ok(()), IntegratedServer::finish) }),
            );
        }
    }
    let stopped = world
        .resource_mut::<SessionJobs>()
        .stop
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)));
    if let Some(result) = stopped {
        let mut jobs = world.resource_mut::<SessionJobs>();
        jobs.stop = None;
        let quit = jobs.closing.take().is_some_and(|(quit, _)| quit);
        disconnect_session(world);
        let mut menu = world.resource_mut::<MenuState>();
        menu.page = MenuPage::Main;
        menu.session_active = false;
        menu.status.clear();
        if let Err(error) = result {
            menu.error = format!("World save failed: {error}");
            menu.quitting = false;
        } else if quit {
            world.write_message(AppExit::Success);
        }
    }
    let menu = world.resource::<MenuState>();
    if world.resource::<SessionJobs>().closing.is_none()
        && menu.session_active
        && matches!(menu.page, MenuPage::Loading | MenuPage::Playing)
    {
        let session = world.resource::<ClientSession>();
        if let Some(error) = &session.error {
            fail(world, format!("Connection failed: {error}"));
        } else if !session.ready && menu.page == MenuPage::Loading {
            let message = if *world.resource::<terra_network::connection::ConnectionStatus>()
                == terra_network::connection::ConnectionStatus::Synchronizing
            {
                "Synchronizing world ..."
            } else {
                "Connecting to server ..."
            };
            if world.resource::<MenuState>().status != message {
                world.resource_mut::<MenuState>().status = message.into();
            }
        }
    }
}
fn start_request(world: &mut World, request: SessionRequest) -> Result<(), String> {
    if matches!(request, SessionRequest::Save) {
        if world.resource::<SessionJobs>().save.is_some() {
            return Ok(());
        }
        let host = world
            .get_resource::<IntegratedServer>()
            .ok_or("Only local worlds can be saved from this client")?;
        let receiver = host.request_save()?;
        world.resource_mut::<SessionInfo>().saving = true;
        world.resource_mut::<SessionJobs>().save =
            Some(AsyncComputeTaskPool::get().spawn(async move {
                receiver
                    .recv_timeout(Duration::from_secs(30))
                    .map_err(|e| e.to_string())?
            }));
        return Ok(());
    }
    if matches!(request, SessionRequest::Leave | SessionRequest::Quit) {
        let mut jobs = world.resource_mut::<SessionJobs>();
        jobs.reveal_elapsed = None;
        if jobs.closing.is_none() {
            jobs.closing = Some((matches!(request, SessionRequest::Quit), Instant::now()));
        }
        world.resource_mut::<LoadingProgress>().reveal = 0.0;
        return Ok(());
    }
    let jobs = world.resource::<SessionJobs>();
    if jobs.start.is_some()
        || jobs.stop.is_some()
        || jobs.closing.is_some()
        || world.contains_resource::<IntegratedServer>()
    {
        return Err("Close the current session before starting another".into());
    }
    if !world.resource::<MenuState>().profile_loaded {
        return Err("Repair profile.toml before joining a world".into());
    }
    let profile = world.resource::<terra_core::PlayerProfile>().clone();
    let title = match &request {
        SessionRequest::PlayWorld(id) => world
            .resource::<terra_ui::menu::WorldMenu>()
            .entries
            .iter()
            .find(|entry| entry.manifest.world.id == *id)
            .map(|entry| entry.title.clone())
            .unwrap_or_default(),
        SessionRequest::JoinServer(id) => world
            .resource::<ServerMenu>()
            .list
            .servers
            .iter()
            .find(|server| server.id == *id)
            .map(|server| server.name.clone())
            .unwrap_or_default(),
        SessionRequest::JoinAddress(address) => address.to_string(),
        _ => String::new(),
    };
    world.insert_resource(SessionInfo { title, ..default() });
    world.resource_mut::<SessionJobs>().loading_updated = None;
    world.resource_mut::<SessionJobs>().reveal_elapsed = None;
    world.insert_resource(LoadingProgress::default());
    let task = match request {
        SessionRequest::PlayWorld(id) => {
            let library = world.resource::<MenuPaths>().worlds.clone();
            let mut config = world.resource::<GameConfig>().clone();
            AsyncComputeTaskPool::get().spawn(async move {
                let entry = library.mark_played(id)?;
                entry.compatible()?;
                config.world = entry.manifest.world;
                config.server.natural_mob_limit = None;
                config.server.allow_flight = entry.mode == terra_storage::GameMode::Creative;
                config.server.game_mode = entry.mode;
                let mut permissions = config.server.permissions_for(profile.uuid);
                if !config.server.player_permissions.contains_key(&profile.uuid) {
                    // The owner's flight follows /gamemode; keep other existing host permissions.
                    permissions.flight = false;
                }
                permissions.game_mode = true;
                config
                    .server
                    .player_permissions
                    .insert(profile.uuid, permissions);
                let (host, _) = IntegratedServer::start_with_save(config, Some(library.path(id)))?;
                let token = terra_network::browser::join(host.address, &profile)?;
                Ok(PreparedSession {
                    host: Some(host),
                    token,
                    info: SessionInfo {
                        title: entry.title,
                        mode: entry.mode.label().into(),
                        local: true,
                        ..default()
                    },
                })
            })
        }
        SessionRequest::JoinServer(id) => {
            let server = world
                .resource::<ServerMenu>()
                .list
                .servers
                .iter()
                .find(|s| s.id == id)
                .ok_or("Server is no longer available")?
                .clone();
            AsyncComputeTaskPool::get().spawn(async move {
                server.validate().map_err(|e| e.to_string())?;
                let addresses = (server.address.as_str(), server.port)
                    .to_socket_addrs()
                    .map_err(|e| format!("Cannot resolve server: {e}"))?;
                let mut result = Err("Server address has no usable endpoints".into());
                for address in addresses.take(4) {
                    result = terra_network::browser::join(address, &profile);
                    if result.is_ok() {
                        break;
                    }
                }
                let token = result?;
                Ok(PreparedSession {
                    host: None,
                    token,
                    info: SessionInfo {
                        title: server.name,
                        mode: "Multiplayer".into(),
                        ..default()
                    },
                })
            })
        }
        SessionRequest::JoinAddress(address) => AsyncComputeTaskPool::get().spawn(async move {
            let status = terra_network::browser::query_status(address)?;
            let token = terra_network::browser::join(address, &profile)?;
            Ok(PreparedSession {
                host: None,
                token,
                info: SessionInfo {
                    title: status.title,
                    mode: "Multiplayer".into(),
                    ..default()
                },
            })
        }),
        SessionRequest::Save => return Err("No local world is available to save".into()),
        SessionRequest::Leave | SessionRequest::Quit => return Ok(()),
    };
    world.resource_mut::<SessionJobs>().start = Some(task);
    Ok(())
}
fn update_loading(world: &mut World) {
    use terra_world::streaming::{GenerationStatus, WorldGeneration};
    let menu = world.resource::<MenuState>();
    if menu.page != MenuPage::Loading
        || !menu.session_active
        || !menu.error.is_empty()
        || world.resource::<SessionJobs>().closing.is_some()
        || !world.resource::<ClientSession>().ready
    {
        return;
    }
    let now = Instant::now();
    if let Some(elapsed) = world.resource::<SessionJobs>().reveal_elapsed {
        // Shader compilation or one stalled frame must not skip the reveal.
        let frame = world
            .resource::<Time<Real>>()
            .delta()
            .min(Duration::from_millis(50));
        let elapsed = elapsed + frame;
        world.resource_mut::<SessionJobs>().reveal_elapsed = Some(elapsed);
        let reveal = reveal_fraction(elapsed);
        world.resource_mut::<LoadingProgress>().reveal = reveal;
        if reveal >= 1.0 {
            world.resource_mut::<SessionJobs>().reveal_elapsed = None;
            world.resource_mut::<SessionInfo>().started = Some(now);
            let mut menu = world.resource_mut::<MenuState>();
            menu.page = MenuPage::Playing;
            menu.status.clear();
            world.write_message(terra_player::CaptureRequest(true));
        }
        return;
    }
    if world
        .resource::<SessionJobs>()
        .loading_updated
        .is_some_and(|previous| now.duration_since(previous) < Duration::from_millis(100))
    {
        return;
    }
    world.resource_mut::<SessionJobs>().loading_updated = Some(now);
    let generation = world.resource::<WorldGeneration>();
    let render = world.resource::<terra_render::RenderStatus>();
    if let Some(error) = &render.error {
        fail(world, format!("Rendering failed: {error}"));
        return;
    }
    if let GenerationStatus::Failed(error) = &generation.status {
        fail(world, format!("World generation failed: {error}"));
        return;
    }
    let props = world.resource::<terra_render::PropRenderStatus>();
    if let Some(error) = &props.error {
        fail(world, format!("Vegetation rendering failed: {error}"));
        return;
    }
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let (completed, total) = chunk_counts(chunks.wanted_sections().map(|position| {
        let ready = chunks
            .loaded
            .get(&position)
            .is_some_and(|section| section.mesh.is_none())
            && !render.pending_columns.contains(&[position.x, position.z]);
        (position, ready)
    }));
    if total > 0
        && completed == total
        && generation.status == GenerationStatus::Ready
        && render.ready
        && props.ready
        && world.resource::<terra_render::WaterStatus>().initial_ready
    {
        world.insert_resource(LoadingProgress {
            fraction: Some(1.0),
            detail: format!("{completed} / {total} chunks"),
            reveal: 0.0,
        });
        world.resource_mut::<SessionJobs>().reveal_elapsed = Some(Duration::ZERO);
        return;
    }
    world.resource_mut::<MenuState>().status = "Loading chunks ...".into();
    world.insert_resource(LoadingProgress {
        fraction: (total > 0).then(|| completed as f32 / total as f32),
        detail: format!("{completed} / {total} chunks"),
        reveal: 0.0,
    });
}

fn reveal_fraction(elapsed: Duration) -> f32 {
    // Present 100% before revealing the world; input and world audio remain gated.
    let progress = ((elapsed.as_secs_f32() - 0.12) / 0.9).clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}
fn chunk_counts(
    sections: impl IntoIterator<Item = (terra_voxel::SectionPos, bool)>,
) -> (usize, usize) {
    let mut columns = HashMap::new();
    for (position, ready) in sections {
        *columns.entry([position.x, position.z]).or_insert(true) &= ready;
    }
    (
        columns.values().filter(|&&ready| ready).count(),
        columns.len(),
    )
}
fn fail(world: &mut World, error: String) {
    world.resource_mut::<SessionJobs>().reveal_elapsed = None;
    world.resource_mut::<LoadingProgress>().reveal = 0.0;
    let mut menu = world.resource_mut::<MenuState>();
    menu.page = MenuPage::Loading;
    menu.error = error;
    menu.status = "Session could not be opened".into();
    menu.quitting = false;
}
fn limit_fps(graphics: Res<terra_core::GraphicsConfig>, mut previous: Local<Option<Instant>>) {
    let now = Instant::now();
    if graphics.fps_limit != 0
        && let Some(previous) = *previous
    {
        let budget = Duration::from_secs_f64(1.0 / f64::from(graphics.fps_limit));
        if let Some(remaining) = budget.checked_sub(now.duration_since(previous)) {
            std::thread::sleep(remaining);
        }
    }
    *previous = Some(Instant::now());
}

#[cfg(test)]
mod tests {
    use super::{chunk_counts, reveal_fraction};
    use std::time::Duration;
    use terra_voxel::SectionPos;

    #[test]
    fn chunk_progress_waits_for_every_vertical_section_including_negative_coordinates() {
        let sections = [
            (SectionPos::new(-1, -2, -3), true),
            (SectionPos::new(-1, -1, -3), false),
            (SectionPos::new(-1, 0, -3), true),
            (SectionPos::new(0, 0, -3), true),
            (SectionPos::new(0, 1, -3), true),
            (SectionPos::new(1, 0, -3), false),
        ];
        assert_eq!(chunk_counts(sections), (1, 3));
        assert_eq!(
            chunk_counts(sections.map(|(position, _)| (position, true))),
            (3, 3)
        );
        assert_eq!(chunk_counts([]), (0, 0));
    }

    #[test]
    fn world_reveal_holds_one_hundred_percent_then_eases_monotonically_to_gameplay() {
        let sample = |seconds| reveal_fraction(Duration::from_secs_f32(seconds));
        assert_eq!(sample(0.0), 0.0);
        assert_eq!(sample(0.12), 0.0);
        assert!((sample(0.57) - 0.5).abs() < 0.001);
        assert_eq!(sample(1.02), 1.0);
        let mut previous = 0.0;
        for millisecond in 0..=1100 {
            let next = reveal_fraction(Duration::from_millis(millisecond));
            assert!((previous..=1.0).contains(&next));
            previous = next;
        }
    }
}
