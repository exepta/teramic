use bevy::{
    app::AppExit,
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future},
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
    StartupProgress,
};

struct PreparedSession {
    host: Option<IntegratedServer>,
    token: Vec<u8>,
    info: SessionInfo,
}
#[derive(Resource)]
pub(crate) struct StartupLaunch(pub Vec<u8>);

fn startup_barrier(startup: Option<&terra_render::RenderPreviewStartup>) -> Result<bool, String> {
    let Some(startup) = startup.filter(|startup| startup.required) else {
        return Ok(false);
    };
    if let Some(error) = &startup.error {
        return Err(format!("Inventory icon preparation failed: {error}"));
    }
    Ok(!startup.ready)
}

fn all_startup_barriers(world: &World) -> Result<bool, String> {
    let catalog = startup_barrier(world.get_resource::<terra_render::RenderPreviewStartup>())?
        || world
            .get_resource::<StartupProgress>()
            .is_some_and(|progress| progress.fading);
    if let Some(ui) = world
        .get_resource::<crate::bootstrap::startup_ui::UiStartupAssets>()
        .filter(|ui| ui.required)
    {
        if let Some(error) = &ui.error {
            return Err(error.clone());
        }
        return Ok(catalog || !ui.ready);
    }
    Ok(catalog)
}

fn update_startup(world: &mut World) {
    let preview_startup = world.get_resource::<terra_render::RenderPreviewStartup>();
    let ui_startup = world.get_resource::<crate::bootstrap::startup_ui::UiStartupAssets>();
    let active = preview_startup.is_some_and(|preview_startup| {
        preview_startup.required && (!preview_startup.ready || preview_startup.error.is_some())
    }) || ui_startup.is_some_and(|ui_startup| {
        ui_startup.required && (!ui_startup.ready || ui_startup.error.is_some())
    });
    if !active {
        let required = preview_startup.is_some_and(|preview_startup| preview_startup.required)
            || ui_startup.is_some_and(|ui_startup| ui_startup.required);
        let total = preview_startup.map_or(0, |preview_startup| preview_startup.total)
            + ui_startup.map_or(0, |ui_startup| ui_startup.total);
        let frame = world
            .get_resource::<Time<Real>>()
            .map_or(0.0, |time| time.delta_secs().min(0.05));
        let mut progress = world.resource_mut::<StartupProgress>();
        if required && progress.active {
            progress.fading = true;
            progress.reveal = (progress.reveal + frame / 0.65).min(1.0);
            progress.fraction = Some(1.0);
            progress.label = format!("{total} / {total}");
            progress.detail = "Bereit".into();
            progress.error.clear();
            if progress.reveal >= 1.0 {
                progress.active = false;
                progress.fading = false;
            }
        } else {
            progress.active = false;
            progress.fading = false;
            progress.reveal = if required { 1.0 } else { 0.0 };
            progress.fraction = required.then_some(1.0);
            progress.detail.clear();
            progress.label.clear();
            progress.error.clear();
        }
        return;
    }
    let completed = preview_startup.map_or(0, |preview_startup| preview_startup.completed)
        + ui_startup.map_or(0, |ui_startup| ui_startup.completed);
    let total = preview_startup.map_or(0, |preview_startup| preview_startup.total)
        + ui_startup.map_or(0, |ui_startup| ui_startup.total);
    let fraction = (total > 0
        && preview_startup.is_none_or(|state| !state.required || state.initialized))
    .then(|| completed as f32 / total as f32);
    let error = preview_startup
        .and_then(|preview_startup| preview_startup.error.clone())
        .or_else(|| ui_startup.and_then(|ui_startup| ui_startup.error.clone()))
        .unwrap_or_default();
    let label = format!("{completed} / {total}");
    let detail = preview_startup
        .filter(|state| !state.ready && !state.current_path.is_empty())
        .map(|state| state.current_path.clone())
        .or_else(|| {
            ui_startup
                .filter(|state| !state.current_path.is_empty())
                .map(|state| state.current_path.clone())
        })
        .unwrap_or_else(|| "Modelle, Texturen, Icons, Schriftarten und Audio".into());
    let detail = shorten_asset_path(&detail, 72);
    let mut progress = world.resource_mut::<StartupProgress>();
    if progress.active != active
        || progress.fraction != fraction
        || progress.detail != detail
        || progress.label != label
        || progress.error != error
    {
        progress.active = active;
        progress.fading = false;
        progress.reveal = 0.0;
        progress.fraction = fraction;
        progress.detail = detail;
        progress.label = label;
        progress.error = error;
    }
}
/// Keep both the source root and filename readable, including Unicode asset names.
fn shorten_asset_path(path: &str, limit: usize) -> String {
    if path.chars().count() <= limit {
        return path.into();
    }
    if limit < 2 {
        return path.chars().take(limit).collect();
    }
    let tail = limit.saturating_sub(20);
    let head: String = path.chars().take(limit.saturating_sub(tail + 1)).collect();
    let end: String = path
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}…{end}")
}

#[derive(Resource, Default)]
struct SessionJobs {
    start: Option<Task<Result<PreparedSession, String>>>,
    stop: Option<Task<Result<(), String>>>,
    closing: Option<(bool, Instant)>,
    save: Option<Task<Result<(), String>>>,
    loading_updated: Option<Instant>,
    reveal_elapsed: Option<Duration>,
    deferred_start: Option<SessionRequest>,
}

#[derive(Resource)]
struct ViewWarmup {
    next: Instant,
}
const STARTUP_VIEW_RADIUS: u8 = 15; // 31 × 31 = 961 chunk columns, independent of far view.

fn reset_streaming_limit(world: &mut World) {
    let requested = world.resource::<GameConfig>().client.view_distance;
    world
        .resource_mut::<terra_render::RenderStatus>()
        .scene_revealed = false;
    world
        .resource_mut::<terra_world::streaming::StreamingRadiusLimit>()
        .0 = Some(requested.min(STARTUP_VIEW_RADIUS));
    world.resource_mut::<ViewWarmup>().next = Instant::now() + Duration::from_millis(750);
}

fn update_streaming_limit(world: &mut World) {
    use terra_world::streaming::{GenerationStatus, StreamingRadiusLimit, WorldGeneration};
    let menu = world.resource::<MenuState>();
    let active = menu.session_active;
    let loading = menu.page == MenuPage::Loading;
    let playing = menu.page == MenuPage::Playing;
    let requested = world
        .resource::<terra_core::GameConfig>()
        .client
        .view_distance;
    if !active || loading {
        let radius = requested.min(STARTUP_VIEW_RADIUS);
        if world.resource::<StreamingRadiusLimit>().0 != Some(radius) {
            world.resource_mut::<StreamingRadiusLimit>().0 = Some(radius);
        }
        world.resource_mut::<ViewWarmup>().next = Instant::now() + Duration::from_millis(750);
        return;
    }
    if !playing || Instant::now() < world.resource::<ViewWarmup>().next {
        return;
    }
    let current = world.resource::<StreamingRadiusLimit>().0;
    let Some(current) = current else {
        return;
    };
    if current >= requested {
        world.resource_mut::<StreamingRadiusLimit>().0 = None;
        return;
    }
    world.resource_mut::<ViewWarmup>().next = Instant::now() + Duration::from_millis(750);
    if world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && world.resource::<terra_render::WaterStatus>().initial_ready
        && presented_sections(world).all(|(_, presented)| presented)
    {
        world.resource_mut::<StreamingRadiusLimit>().0 = Some(current + 1);
    }
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<SessionJobs>()
        .init_resource::<terra_world::streaming::StreamingRadiusLimit>()
        .insert_resource(ViewWarmup {
            next: Instant::now(),
        })
        .init_resource::<StartupProgress>()
        .add_systems(
            Update,
            update.after(terra_world::streaming::StreamingSet::Dispatch),
        )
        .add_systems(PostUpdate, update_loading)
        .add_systems(PreUpdate, update_streaming_limit)
        .add_systems(Last, limit_fps);
}
fn update(world: &mut World) {
    update_startup(world);
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
    let barrier = all_startup_barriers(world);
    if !matches!(barrier, Ok(true)) {
        if let Some(request) = world.resource_mut::<SessionJobs>().deferred_start.take() {
            let result = barrier.clone().and_then(|_| start_request(world, request));
            if let Err(error) = result {
                fail(world, error);
            }
        }
        if let Some(StartupLaunch(token)) = world.remove_resource::<StartupLaunch>() {
            match barrier {
                Ok(false) => {
                    reset_streaming_limit(world);
                    connect_session(world, token);
                }
                Err(error) => fail(world, error),
                Ok(true) => unreachable!(),
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
                    reset_streaming_limit(world);
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
                IoTaskPool::get()
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
        terra_ui::restore_character_profile(world);
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
        world.resource_mut::<SessionJobs>().save = Some(IoTaskPool::get().spawn(async move {
            receiver
                .recv_timeout(Duration::from_secs(30))
                .map_err(|e| e.to_string())?
        }));
        return Ok(());
    }
    if matches!(request, SessionRequest::Leave | SessionRequest::Quit) {
        world.remove_resource::<StartupLaunch>();
        let mut jobs = world.resource_mut::<SessionJobs>();
        jobs.deferred_start = None;
        jobs.reveal_elapsed = None;
        if jobs.closing.is_none() {
            jobs.closing = Some((matches!(request, SessionRequest::Quit), Instant::now()));
        }
        world.resource_mut::<LoadingProgress>().reveal = 0.0;
        return Ok(());
    }
    if all_startup_barriers(world)? {
        let mut jobs = world.resource_mut::<SessionJobs>();
        if jobs.deferred_start.is_some() {
            return Err("A world is already waiting for inventory icons".into());
        }
        jobs.deferred_start = Some(request);
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
    let Some(profile) = terra_ui::prepare_character_profile(world, &request)? else {
        return Ok(());
    };
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
    terra_ui::activate_character_profile(world, profile.clone());
    let task = match request {
        SessionRequest::PlayWorld(id) => {
            let library = world.resource::<MenuPaths>().worlds.clone();
            let mut config = world.resource::<GameConfig>().clone();
            // The host generates terrain and saves on the async compute pool.
            // Keep this blocking startup handshake off those workers.
            IoTaskPool::get().spawn(async move {
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
            IoTaskPool::get().spawn(async move {
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
        SessionRequest::JoinAddress(address) => IoTaskPool::get().spawn(async move {
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
    if !matches!(all_startup_barriers(world), Ok(false)) {
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
            world
                .resource_mut::<terra_render::RenderStatus>()
                .scene_revealed = true;
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
    let (completed, total) = chunk_counts(presented_sections(world));
    if total > 0
        && completed == total
        && generation.status == GenerationStatus::Ready
        && props.ready
        && world.resource::<terra_render::WaterStatus>().initial_ready
    {
        world.resource_mut::<MenuState>().status.clear();
        world.insert_resource(LoadingProgress {
            fraction: Some(1.0),
            detail: format!("{completed} / {total} chunks"),
            reveal: 0.0,
        });
        world.resource_mut::<SessionJobs>().reveal_elapsed = Some(Duration::ZERO);
        return;
    }
    let status = if total == 0 || completed < total {
        "Loading chunks ..."
    } else if !props.ready {
        "Preparing world objects ..."
    } else if !world.resource::<terra_render::WaterStatus>().initial_ready {
        "Preparing water surfaces ..."
    } else {
        "Preparing terrain ..."
    };
    world.resource_mut::<MenuState>().status = status.into();
    world.insert_resource(LoadingProgress {
        fraction: pending_chunk_fraction(completed, total),
        detail: format!("{completed} / {total} chunks"),
        reveal: 0.0,
    });
}

fn pending_chunk_fraction(completed: usize, total: usize) -> Option<f32> {
    // The UI rounds to whole percentages. Reserve 100% for the confirmed reveal.
    (total > 0).then(|| (completed as f32 / total as f32).min(0.99))
}

fn reveal_fraction(elapsed: Duration) -> f32 {
    // Present 100% before revealing the world; input and world audio remain gated.
    let progress = ((elapsed.as_secs_f32() - 0.12) / 0.9).clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

fn presented_sections(world: &World) -> impl Iterator<Item = (terra_voxel::SectionPos, bool)> + '_ {
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let rendered = world.resource::<terra_render::RenderedSections>();
    let visibility = world.get_resource::<terra_render::TerrainVisibility>();
    chunks.wanted_sections().map(move |position| {
        (
            position,
            rendered.section_presented(position, chunks.loaded.get(&position), visibility),
        )
    })
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
    if !world.resource::<MenuState>().session_active {
        terra_ui::restore_character_profile(world);
    }
    let mut jobs = world.resource_mut::<SessionJobs>();
    jobs.reveal_elapsed = None;
    jobs.deferred_start = None;
    world.resource_mut::<LoadingProgress>().reveal = 0.0;
    let mut menu = world.resource_mut::<MenuState>();
    menu.page = MenuPage::Loading;
    menu.error = error;
    menu.status = "Session could not be opened".into();
    menu.quitting = false;
}
fn limit_fps(
    graphics: Res<terra_core::GraphicsConfig>,
    windows: Query<&Window>,
    mut previous: Local<Option<Instant>>,
) {
    let now = Instant::now();
    static SMOKE: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::args().any(|arg| arg.starts_with("--smoke-test")));
    let limit = frame_limit(
        graphics.fps_limit,
        windows.iter().next().filter(|_| !*SMOKE).map(|w| {
            (
                w.focused,
                w.resolution.physical_width() == 0 || w.resolution.physical_height() == 0,
            )
        }),
    );
    if limit != 0
        && let Some(previous) = *previous
    {
        let budget = Duration::from_secs_f64(1.0 / f64::from(limit));
        if let Some(remaining) = budget.checked_sub(now.duration_since(previous)) {
            std::thread::sleep(remaining);
        }
    }
    *previous = Some(Instant::now());
}

fn frame_limit(requested: u16, window: Option<(bool, bool)>) -> u16 {
    match window {
        Some((_, true)) => {
            if requested == 0 {
                5
            } else {
                requested.min(5)
            }
        }
        Some((false, _)) => {
            if requested == 0 {
                30
            } else {
                requested.min(30)
            }
        }
        _ => requested,
    }
}

#[cfg(test)]
mod tests {
    use super::{chunk_counts, reveal_fraction};
    use std::time::Duration;
    use terra_voxel::SectionPos;

    #[test]
    fn entering_play_enables_streaming_budgets_until_the_next_session() {
        use super::*;
        let mut world = World::new();
        world.init_resource::<GameConfig>();
        world.init_resource::<MenuState>();
        world.init_resource::<ClientSession>();
        world.init_resource::<LoadingProgress>();
        world.init_resource::<SessionInfo>();
        world.init_resource::<SessionJobs>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Messages<terra_player::CaptureRequest>>();
        world.init_resource::<terra_render::RenderStatus>();
        world.init_resource::<terra_world::streaming::StreamingRadiusLimit>();
        world.insert_resource(ViewWarmup {
            next: Instant::now(),
        });
        world.resource_mut::<ClientSession>().ready = true;
        world.resource_mut::<MenuState>().page = MenuPage::Loading;
        world.resource_mut::<MenuState>().session_active = true;
        world.resource_mut::<SessionJobs>().reveal_elapsed = Some(Duration::from_secs(2));
        update_loading(&mut world);
        assert_eq!(world.resource::<MenuState>().page, MenuPage::Playing);
        assert!(
            world
                .resource::<terra_render::RenderStatus>()
                .scene_revealed
        );
        reset_streaming_limit(&mut world);
        assert!(
            !world
                .resource::<terra_render::RenderStatus>()
                .scene_revealed
        );
    }

    #[test]
    fn initial_load_stays_below_one_thousand_columns_then_waits_for_finished_background_rings() {
        use super::*;
        use terra_world::streaming::{GenerationStatus, StreamingRadiusLimit, WorldGeneration};
        let mut world = World::new();
        let mut config = GameConfig::default();
        config.client.view_distance = 32;
        world.insert_resource(config);
        world.init_resource::<MenuState>();
        world.init_resource::<StreamingRadiusLimit>();
        world.insert_resource(ViewWarmup {
            next: Instant::now(),
        });
        world.init_resource::<WorldGeneration>();
        world.init_resource::<terra_world::streaming::ChunkStore>();
        world.init_resource::<terra_render::RenderedSections>();
        world.init_resource::<terra_render::WaterStatus>();
        world.resource_mut::<MenuState>().page = MenuPage::Loading;
        world.resource_mut::<MenuState>().session_active = true;
        update_streaming_limit(&mut world);
        assert_eq!(world.resource::<StreamingRadiusLimit>().0, Some(15));
        assert_eq!((u32::from(STARTUP_VIEW_RADIUS) * 2 + 1).pow(2), 961);
        world.resource_mut::<MenuState>().page = MenuPage::Playing;
        world.resource_mut::<ViewWarmup>().next = Instant::now();
        update_streaming_limit(&mut world);
        assert_eq!(world.resource::<StreamingRadiusLimit>().0, Some(15));
        world.resource_mut::<WorldGeneration>().status = GenerationStatus::Ready;
        world.resource_mut::<ViewWarmup>().next = Instant::now();
        world
            .resource_mut::<terra_render::WaterStatus>()
            .initial_ready = true;
        update_streaming_limit(&mut world);
        assert_eq!(world.resource::<StreamingRadiusLimit>().0, Some(16));
        update_streaming_limit(&mut world);
        assert_eq!(world.resource::<StreamingRadiusLimit>().0, Some(16));
        assert_eq!(world.resource::<GameConfig>().client.view_distance, 32);
        world.resource_mut::<MenuState>().page = MenuPage::Loading;
        update_streaming_limit(&mut world);
        assert_eq!(world.resource::<StreamingRadiusLimit>().0, Some(15));
    }

    #[test]
    fn background_window_releases_cpu_even_with_an_unlimited_foreground_setting() {
        assert_eq!(super::frame_limit(0, Some((true, false))), 0);
        assert_eq!(super::frame_limit(144, Some((true, false))), 144);
        assert_eq!(super::frame_limit(0, Some((false, false))), 30);
        assert_eq!(super::frame_limit(60, Some((false, false))), 30);
        assert_eq!(super::frame_limit(60, Some((true, true))), 5);
    }

    #[test]
    fn startup_barrier_is_absent_for_headless_and_finished_or_demand_only_clients() {
        use super::startup_barrier;
        assert_eq!(startup_barrier(None), Ok(false));
        assert_eq!(
            startup_barrier(Some(&terra_render::RenderPreviewStartup::default())),
            Ok(false)
        );
        let mut startup = terra_render::RenderPreviewStartup {
            required: true,
            ..Default::default()
        };
        assert_eq!(startup_barrier(Some(&startup)), Ok(true));
        startup.ready = true;
        assert_eq!(startup_barrier(Some(&startup)), Ok(false));
        startup.error = Some("missing item model".into());
        assert!(
            startup_barrier(Some(&startup))
                .unwrap_err()
                .contains("missing item model")
        );
    }

    #[test]
    fn pending_startup_preload_defers_world_and_network_start_but_cancel_and_quit_still_work() {
        use super::*;
        for request in [
            SessionRequest::PlayWorld(terra_core::WorldId(42)),
            SessionRequest::JoinAddress("127.0.0.1:5000".parse().unwrap()),
        ] {
            for cancel in [SessionRequest::Leave, SessionRequest::Quit] {
                let mut world = World::new();
                world.init_resource::<SessionJobs>();
                world.init_resource::<LoadingProgress>();
                world.insert_resource(terra_render::RenderPreviewStartup {
                    required: true,
                    ..default()
                });
                start_request(&mut world, request.clone()).unwrap();
                assert!(world.resource::<SessionJobs>().deferred_start.is_some());
                assert!(world.resource::<SessionJobs>().start.is_none());
                assert!(!world.contains_resource::<IntegratedServer>());
                start_request(&mut world, cancel.clone()).unwrap();
                assert!(world.resource::<SessionJobs>().deferred_start.is_none());
                assert!(world.resource::<SessionJobs>().closing.is_some());
            }
        }
    }

    #[test]
    fn startup_paths_keep_real_filenames_and_shorten_unicode_without_breaking_characters() {
        use super::*;
        let path = "terra/blocks/models/basin.gltf";
        assert_eq!(shorten_asset_path(path, 72), path);
        let long = format!(
            "terra/blocks/{}/Bächlein.gltf",
            "langes_verzeichnis/".repeat(8)
        );
        let short = shorten_asset_path(&long, 72);
        assert_eq!(short.chars().count(), 72);
        assert!(short.starts_with("terra/blocks/") && short.ends_with("/Bächlein.gltf"));
        assert!(short.contains('…'));
        assert_eq!(shorten_asset_path(&long, 0), "");
        assert_eq!(shorten_asset_path(&long, 1), "t");
    }

    fn finish_startup_fade(world: &mut bevy::prelude::World) {
        use super::*;
        world.init_resource::<Time<Real>>();
        world
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(50));
        for _ in 0..14 {
            update_startup(world);
        }
    }

    #[test]
    fn startup_fade_uses_real_frame_time_and_blocks_launch_until_its_last_frame() {
        use super::*;
        let mut world = World::new();
        world.insert_resource(StartupProgress {
            active: true,
            ..default()
        });
        world.insert_resource(terra_render::RenderPreviewStartup {
            required: true,
            ready: true,
            ..default()
        });
        world.init_resource::<Time<Real>>();
        world
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(1));
        update_startup(&mut world);
        let progress = world.resource::<StartupProgress>();
        assert!(
            progress.active && progress.fading && progress.reveal > 0.0 && progress.reveal < 0.1
        );
        assert!(
            all_startup_barriers(&world).unwrap(),
            "A long frame must not skip the visual transition or launch a world"
        );
        finish_startup_fade(&mut world);
        let progress = world.resource::<StartupProgress>();
        assert!(!progress.active && !progress.fading);
        assert_eq!(progress.blend(), 1.0);
        assert!(!all_startup_barriers(&world).unwrap());
    }

    #[test]
    fn startup_progress_is_finite_and_clears_after_preloading_instead_of_becoming_world_work() {
        use super::*;
        let mut world = World::new();
        world.init_resource::<StartupProgress>();
        world.insert_resource(terra_render::RenderPreviewStartup {
            required: true,
            initialized: true,
            total: 400,
            completed: 200,
            current_path: "terra/blocks/models/basin.gltf".into(),
            ..default()
        });
        update_startup(&mut world);
        assert!(world.resource::<StartupProgress>().active);
        assert_eq!(world.resource::<StartupProgress>().fraction, Some(0.5));
        assert_eq!(world.resource::<StartupProgress>().label, "200 / 400");
        assert_eq!(
            world.resource::<StartupProgress>().detail,
            "terra/blocks/models/basin.gltf"
        );
        world
            .resource_mut::<terra_render::RenderPreviewStartup>()
            .ready = true;
        world
            .resource_mut::<terra_render::RenderPreviewStartup>()
            .completed = 400;
        update_startup(&mut world);
        assert!(
            world.resource::<StartupProgress>().active
                && world.resource::<StartupProgress>().fading
        );
        assert_eq!(world.resource::<StartupProgress>().label, "400 / 400");
        finish_startup_fade(&mut world);
        assert!(!world.resource::<StartupProgress>().active);
        assert_eq!(world.resource::<StartupProgress>().fraction, Some(1.0));
    }

    #[test]
    fn failed_startup_stays_on_the_loading_gate_and_reports_the_error_even_after_catalog_finishes()
    {
        use super::*;
        let mut world = World::new();
        world.init_resource::<StartupProgress>();
        world.insert_resource(terra_render::RenderPreviewStartup {
            required: true,
            initialized: true,
            ready: true,
            total: 10,
            completed: 10,
            error: Some("A model texture is missing".into()),
            ..default()
        });
        update_startup(&mut world);
        let progress = world.resource::<StartupProgress>();
        assert!(progress.active);
        assert_eq!(progress.error, "A model texture is missing");
        assert!(all_startup_barriers(&world).is_err());
        world
            .resource_mut::<terra_render::RenderPreviewStartup>()
            .error = None;
        update_startup(&mut world);
        assert!(world.resource::<StartupProgress>().fading);
        assert!(world.resource::<StartupProgress>().error.is_empty());
        finish_startup_fade(&mut world);
        assert!(!world.resource::<StartupProgress>().active);
    }

    #[test]
    fn session_start_waits_for_static_ui_icons_and_fonts_as_well_as_item_previews() {
        use super::*;
        let mut world = World::new();
        world.insert_resource(terra_render::RenderPreviewStartup {
            required: true,
            ready: true,
            ..default()
        });
        world.init_resource::<crate::bootstrap::startup_ui::UiStartupAssets>();
        world
            .resource_mut::<crate::bootstrap::startup_ui::UiStartupAssets>()
            .required = true;
        assert_eq!(all_startup_barriers(&world), Ok(true));
        world
            .resource_mut::<crate::bootstrap::startup_ui::UiStartupAssets>()
            .ready = true;
        assert_eq!(all_startup_barriers(&world), Ok(false));
        world
            .resource_mut::<crate::bootstrap::startup_ui::UiStartupAssets>()
            .error = Some("missing UI font".into());
        assert!(
            all_startup_barriers(&world)
                .unwrap_err()
                .contains("missing UI font")
        );
    }

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
    fn pending_chunk_progress_reserves_one_hundred_percent_for_the_reveal() {
        use super::pending_chunk_fraction;
        assert_eq!(pending_chunk_fraction(0, 0), None);
        assert_eq!(pending_chunk_fraction(1, 2), Some(0.5));
        for completed in [960, 961] {
            let fraction = pending_chunk_fraction(completed, 961).unwrap();
            assert_eq!(format!("{:.0}%", fraction * 100.0), "99%");
        }
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
