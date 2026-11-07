//! Real session loading verifies the 100% hold, reveal and world-audio ordering.
use bevy::{app::AppExit, audio::AudioSink, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_ui::menu::{LoadingProgress, MenuAction, MenuPage, MenuPaths, MenuState, WorldMenu};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    full_frames: u16,
    full: bool,
    halfway: bool,
    reveal: f32,
    playing: Option<Instant>,
    output: PathBuf,
    requested: bool,
    next_diagnostic: Duration,
    stalled_capture: bool,
    ready_since: Option<Instant>,
    gameplay_probe_seconds: f64,
    save_result: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Result<(), String>>>>,
    gameplay_frames: Vec<f64>,
    previous_frame: Option<Instant>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-loading") {
        return;
    }
    let output = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("teramic-loading-smoke"));
    if let Err(error) = std::fs::create_dir_all(&output) {
        tracing::error!(%error, "Cannot create loading fixture output");
    }
    app.insert_resource(Fixture {
        started: Instant::now(),
        full_frames: 0,
        full: false,
        halfway: false,
        reveal: 0.0,
        playing: None,
        output,
        requested: false,
        next_diagnostic: Duration::from_secs(5),
        stalled_capture: false,
        ready_since: None,
        gameplay_probe_seconds: std::env::var("TERRA_LOADING_GAMEPLAY_PROBE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && (0.0..=60.0).contains(value))
            .unwrap_or(0.4),
        save_result: None,
        gameplay_frames: Vec::new(),
        previous_frame: None,
    })
    .add_systems(Last, verify);
}

fn capture(world: &mut World, output: &std::path::Path, name: &str) {
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            output.join(format!("{name}.png")),
        ));
}

fn verify(world: &mut World) {
    let Some(mut fixture) = world.remove_resource::<Fixture>() else {
        return;
    };
    let result = check(world, &mut fixture);
    if let Err(error) = result {
        tracing::error!(%error, "Native loading reveal fixture failed");
        world.write_message(AppExit::error());
    }
    world.insert_resource(fixture);
}

fn check(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    if fixture.started.elapsed() >= fixture.next_diagnostic {
        fixture.next_diagnostic = fixture.started.elapsed() + Duration::from_secs(5);
        diagnostics(world, fixture.started.elapsed());
    }
    if fixture.started.elapsed() >= Duration::from_secs(30)
        && !fixture.stalled_capture
        && world.resource::<MenuState>().page != MenuPage::Playing
    {
        capture(world, &fixture.output, "loading-stall");
        fixture.stalled_capture = true;
    }
    if fixture.started.elapsed() > Duration::from_secs(90) {
        return Err("Timed out waiting for a fully rendered session reveal".into());
    }
    let menu = world.resource::<MenuState>();
    if !menu.error.is_empty() {
        return Err(menu.error.clone());
    }
    let page = menu.page;
    if page == MenuPage::Main && menu.profile_loaded && !fixture.requested {
        let template = world.resource::<terra_core::GameConfig>().world.clone();
        let library = terra_storage::WorldLibrary {
            root: fixture.output.join("worlds"),
        };
        let entry = if let Ok(title) = std::env::var("TERRA_LOADING_USE_WORLD") {
            library
                .list()?
                .0
                .into_iter()
                .find(|entry| entry.title == title)
                .ok_or_else(|| format!("Missing copied loading fixture world {title}"))?
        } else {
            library.create(
                "Loading reveal fixture",
                Some(template.seed.0),
                terra_storage::GameMode::Creative,
                &template,
            )?
        };
        let id = entry.manifest.world.id;
        world.resource_mut::<MenuPaths>().worlds = library;
        let mut worlds = world.resource_mut::<WorldMenu>();
        worlds.entries = vec![entry];
        worlds.selected = Some(id);
        world.write_message(MenuAction::Command("world-play".into()));
        fixture.requested = true;
        return Ok(());
    }
    let streams: usize = world
        .query::<&Name>()
        .iter(world)
        .filter(|name| {
            matches!(
                name.as_str(),
                "Water soundscape" | "Recorded animal/environment soundscape"
            )
        })
        .count();
    if page == MenuPage::Loading {
        if streams != 0 {
            return Err("World audio started underneath the loading screen".into());
        }
        let progress = world.resource::<LoadingProgress>();
        let full = progress.fraction == Some(1.0);
        let reveal = progress.reveal;
        let chunks = world.resource::<terra_world::streaming::ChunkStore>();
        let scene_ready = world
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready
            && chunks
                .wanted_sections()
                .all(|position| chunks.loaded.contains_key(&position))
            && world.resource::<terra_render::RenderStatus>().ready
            && world.resource::<terra_render::PropRenderStatus>().ready
            && world.resource::<terra_render::WaterStatus>().initial_ready;
        if scene_ready && !full {
            let since = fixture.ready_since.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_secs(2) {
                return Err("Loading progress waits for meshes that the ready renderer intentionally culled".into());
            }
        } else {
            fixture.ready_since = None;
        }
        if reveal < fixture.reveal || (reveal > 0.0 && !full) {
            return Err("Reveal started before 100% or moved backwards".into());
        }
        fixture.reveal = reveal;
        if full && reveal == 0.0 {
            fixture.full_frames += 1;
            if fixture.full_frames >= 3 && !fixture.full {
                capture(world, &fixture.output, "loading-100");
                fixture.full = true;
            }
        }
        if (0.35..0.65).contains(&reveal) && !fixture.halfway {
            capture(world, &fixture.output, "loading-reveal");
            fixture.halfway = true;
        }
    } else if page == MenuPage::Playing {
        let now = Instant::now();
        if fixture.gameplay_probe_seconds > 10.0
            && fixture
                .playing
                .is_some_and(|at| at.elapsed().as_secs_f64() >= 10.0)
            && let Some(previous) = fixture.previous_frame
            && fixture.gameplay_frames.len() < 65_536
        {
            fixture
                .gameplay_frames
                .push(now.duration_since(previous).as_secs_f64());
        }
        fixture.previous_frame = Some(now);
        if !fixture.full || !fixture.halfway || fixture.reveal < 0.8 {
            return Err("Gameplay skipped the visible 100% hold or smooth reveal".into());
        }
        if streams == 0 || world.query::<&AudioSink>().iter(world).next().is_none() {
            return Ok(());
        }
        if fixture.playing.is_none() {
            capture(world, &fixture.output, "loading-complete");
            fixture.playing = Some(Instant::now());
        }
        if fixture
            .playing
            .is_some_and(|at| at.elapsed().as_secs_f64() >= fixture.gameplay_probe_seconds)
        {
            if fixture.gameplay_probe_seconds > 0.4 {
                if fixture.save_result.is_none() {
                    fixture.save_result = Some(std::sync::Mutex::new(
                        world
                            .resource::<terra_server::IntegratedServer>()
                            .request_save()?,
                    ));
                    return Ok(());
                }
                match fixture
                    .save_result
                    .as_ref()
                    .unwrap()
                    .lock()
                    .map_err(|error| error.to_string())?
                    .try_recv()
                {
                    Ok(result) => result?,
                    Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(()),
                    Err(error) => {
                        return Err(format!("Gameplay probe save channel failed: {error}"));
                    }
                }
                tracing::info!(
                    seconds = fixture.gameplay_probe_seconds,
                    "Native integrated gameplay probe saved successfully"
                );
                if !fixture.gameplay_frames.is_empty() {
                    let sum: f64 = fixture.gameplay_frames.iter().sum();
                    fixture.gameplay_frames.sort_unstable_by(f64::total_cmp);
                    let count = fixture.gameplay_frames.len();
                    tracing::info!(
                        sample_frames = count,
                        sample_seconds = sum,
                        average_fps = count as f64 / sum,
                        minimum_fps = 1.0 / fixture.gameplay_frames[count - 1],
                        p99_ms =
                            1000.0 * fixture.gameplay_frames[(count * 99 / 100).min(count - 1)],
                        previews_pending = world
                            .resource::<terra_render::RenderBlockPreviews>()
                            .pending,
                        "Native integrated gameplay probe frame sample (after 10s gameplay warmup)"
                    );
                }
            }
            tracing::info!(
                streams,
                "Native loading reveal passed: 100% hold, smooth reveal, world audio only after completion"
            );
            world.write_message(AppExit::Success);
        }
    }
    Ok(())
}

fn diagnostics(world: &mut World, elapsed: Duration) {
    let player = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|transform| transform.translation);
    let camera = world
        .query_filtered::<&Transform, With<terra_player::TerraCamera>>()
        .iter(world)
        .next()
        .map(|transform| transform.translation);
    let menu = world.resource::<MenuState>();
    let session = world.resource::<terra_network::client::ClientSession>();
    let progress = world.resource::<LoadingProgress>();
    let generation = world.resource::<terra_world::streaming::WorldGeneration>();
    let render = world.resource::<terra_render::RenderStatus>();
    let props = world.resource::<terra_render::PropRenderStatus>();
    let water = world.resource::<terra_render::WaterStatus>();
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let wanted = chunks.wanted_sections().count();
    let missing = chunks
        .wanted_sections()
        .filter(|position| !chunks.loaded.contains_key(position))
        .count();
    let unconsumed_meshes = chunks
        .loaded
        .values()
        .filter(|section| section.mesh.is_some())
        .count();
    let visibility = world.get_resource::<terra_render::TerrainVisibility>();
    let hidden_unconsumed_meshes = chunks
        .loaded
        .iter()
        .filter(|(position, section)| {
            section.mesh.is_some() && visibility.is_some_and(|v| v.hidden(**position))
        })
        .count();
    tracing::info!(
        elapsed_seconds = elapsed.as_secs(),
        page = ?menu.page,
        session_active = menu.session_active,
        connected = session.ready,
        fraction = ?progress.fraction,
        detail = %progress.detail,
        reveal = progress.reveal,
        generation = ?generation.status,
        generation_jobs = generation.pending_jobs,
        wanted,
        loaded = chunks.loaded.len(),
        missing,
        unconsumed_meshes,
        hidden_unconsumed_meshes,
        terrain_ready = render.ready,
        terrain_jobs = render.pending_jobs,
        terrain_pending_columns = render.pending_columns.len(),
        props_ready = props.ready,
        prop_jobs = props.pending_jobs,
        visible_props = props.visible_props,
        water_initial_ready = water.initial_ready,
        water_ready = water.ready,
        water_tiles = water.visible_tiles,
        ?player,
        ?camera,
        "Native loading fixture progress (throttled)"
    );
}
