use bevy::ecs::system::SystemParam;
use bevy::{app::AppExit, prelude::*};
use std::time::{Duration, Instant};
use terra_player::{TargetBlock, TargetWorld, TerraCamera, WorldTarget};
use terra_render::{PropRenderStatus, RenderStatus, RenderedSections, WaterStatus};
use terra_world::streaming::{
    BlockEditRequest, ChunkStore, GenerationStatus, PropRemoveRequest, StreamingSet,
    WorldGeneration,
};
use terra_world::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
};

mod scope_timeline;
use scope_timeline::FrameScopeTrace;

#[derive(Resource)]
struct SmokeTest {
    elapsed: Duration,
    captured: bool,
    captured_at: Duration,
    stable_ready: Duration,
    mine: bool,
    mined: Option<[i64; 3]>,
    mining_complete: bool,
    water_check: bool,
    motion_captured: bool,
    started_at: Instant,
    previous_frame: Instant,
    timeline_frame: u64,
    initial_ready: Option<Duration>,
    frame_times: Vec<f64>,
    skip_frames: u8,
    sample_seconds: f64,
    load_grace_seconds: f64,
    diagnostics: crate::diagnostic_report::NativeDiagnostics,
    sample_scope_start: Option<Vec<(&'static str, terra_core::profiling::ScopeSample)>>,
    sample_scope_end: Option<Vec<(&'static str, terra_core::profiling::ScopeSample)>>,
    flythrough: bool,
    flight_origin: Option<Transform>,
    flight_captures: u8,
    worst_frames: Vec<FrameSpike>,
    previous_context: FrameContext,
    readiness_logged: u32,
}

pub(crate) fn benchmark_ready_time(world: &World) -> Option<Duration> {
    world
        .get_resource::<SmokeTest>()
        .filter(|test| test.captured)
        .map(|test| test.captured_at)
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameContext {
    generation_jobs: usize,
    terrain_jobs: usize,
    prop_jobs: usize,
    upload_vertices: usize,
    active_animations: usize,
    queued_animations: usize,
    window_focused: Option<bool>,
    window_visible: Option<bool>,
    present_mode: Option<bevy::window::PresentMode>,
}
struct FrameSpike {
    seconds: f64,
    sample_time: f64,
    context: FrameContext,
    scopes: Option<FrameScopeTrace>,
}
pub(crate) fn install(app: &mut App) {
    crate::custom_smoke::install(app);
    if std::env::args().any(|arg| arg == "--smoke-test-animations") {
        app.add_systems(PostStartup, overview_camera);
        return;
    }
    let mine = std::env::args().any(|arg| arg == "--smoke-test-mining");
    let props = std::env::args().any(|arg| arg == "--smoke-test-props");
    let flythrough = std::env::args().any(|arg| arg == "--smoke-test-lod");
    let overview = std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--smoke-test-overview"
                | "--smoke-test-landscape"
                | "--smoke-test-water"
                | "--smoke-test-forest"
                | "--smoke-test-desert"
                | "--smoke-test-plains"
                | "--smoke-test-ocean"
                | "--smoke-test-reference"
                | "--smoke-test-custom"
                | "--smoke-test-lod"
        )
    });
    if overview {
        app.add_systems(PostStartup, overview_camera);
    }
    if mine {
        app.add_systems(PostStartup, mining_camera);
    }
    if props {
        app.init_resource::<PropSmoke>().add_systems(
            Update,
            check_prop_removal
                .after(StreamingSet::Dispatch)
                .before(smoke_test),
        );
    }
    if mine
        || props
        || overview
        || std::env::args().any(|arg| {
            matches!(
                arg.as_str(),
                "--smoke-test" | "--smoke-test-benchmark" | "--smoke-test-river-benchmark"
            )
        })
    {
        let now = Instant::now();
        if std::env::var("TERRA_BENCH_DEBUG_OVERLAY").is_ok_and(|value| value == "1") {
            app.add_systems(Startup, |mut debug: ResMut<terra_core::DebugSettings>| {
                debug.enabled = true;
            });
        }
        app.insert_resource(SmokeTest {
            elapsed: Duration::ZERO,
            captured: false,
            captured_at: Duration::ZERO,
            stable_ready: Duration::ZERO,
            mine,
            mined: None,
            mining_complete: !mine,
            water_check: std::env::args().any(|arg| arg == "--smoke-test-water"),
            motion_captured: false,
            started_at: now,
            previous_frame: now,
            timeline_frame: 0,
            initial_ready: None,
            frame_times: Vec::new(),
            skip_frames: 0,
            sample_seconds: std::env::var("TERRA_BENCH_SECONDS")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(if flythrough { 48.0 } else { 3.0 })
                .clamp(if flythrough { 24.0 } else { 3.0 }, 1800.0),
            load_grace_seconds: std::env::var("TERRA_BENCH_LOAD_SECONDS")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(30.0, 240.0))
                .unwrap_or(if flythrough { 180.0 } else { 90.0 }),
            diagnostics: default(),
            sample_scope_start: None,
            sample_scope_end: None,
            flythrough,
            flight_origin: None,
            flight_captures: 0,
            worst_frames: Vec::with_capacity(12),
            previous_context: default(),
            readiness_logged: 0,
        })
        .add_systems(Update, smoke_test.after(StreamingSet::Dispatch));
        if flythrough {
            app.add_systems(
                Update,
                flythrough_camera
                    .after(terra_player::PlayerSet::Movement)
                    .before(StreamingSet::Interest),
            );
        }
    }
}

fn mining_camera(mut camera: Query<&mut Transform, With<terra_player::TerraCamera>>) {
    for mut transform in &mut camera {
        let eye = transform.translation;
        *transform =
            Transform::from_translation(eye).looking_at(eye + Vec3::new(0.0, -4.0, -0.8), Vec3::Y);
    }
}

fn overview_camera(
    mut camera: Query<&mut Transform, With<terra_player::TerraCamera>>,
    mut focus: ResMut<terra_world::streaming::ChunkFocus>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
) {
    for mut transform in &mut camera {
        if std::env::args().any(|a| a == "--smoke-test-reference") {
            let eye = Vec3::new(1064.0, 15.0, -240.0);
            *transform = Transform::from_translation(eye)
                .looking_at(Vec3::new(1024.0, 19.0, -192.0), Vec3::Y);
            focus.0 = eye.to_array().map(f64::from);
            continue;
        }
        let point = if std::env::args().any(|a| a == "--smoke-test-water") {
            Some([1152.0, -128.0])
        } else if std::env::args().any(|a| {
            matches!(
                a.as_str(),
                "--smoke-test-forest" | "--smoke-test-lod" | "--smoke-test-animations"
            )
        }) {
            Some([-640.0, 640.0])
        } else if std::env::args().any(|a| a == "--smoke-test-desert") {
            Some([2304.0, -8192.0])
        } else if std::env::args().any(|a| a == "--smoke-test-plains") {
            Some([-256.0, 1536.0])
        } else if std::env::args().any(|a| a == "--smoke-test-landscape") {
            Some([512.0, -512.0])
        } else if std::env::args().any(|a| a == "--smoke-test-ocean") {
            Some([1408.0, 0.0])
        } else {
            None
        };
        let mut base = if let Some([x, z]) = point {
            Vec3::new(
                x as f32,
                terrain
                    .0
                    .surface_height(x, z)
                    .max(terrain.0.config().sea_level) as f32
                    + 2.0,
                z as f32,
            )
        } else {
            transform.translation
        };
        // Keep the pre-biome camera reproducible even when terrain height changes.
        if std::env::var_os("TERRA_FOREST_BASELINE_VIEW").is_some()
            && std::env::args().any(|arg| arg == "--smoke-test-forest")
        {
            base.y = 29.233_925;
        }
        let eye = base + Vec3::new(18.0, 28.0, 40.0);
        *transform =
            Transform::from_translation(eye).looking_at(base + Vec3::new(0.0, 8.0, 0.0), Vec3::Y);
        focus.0 = eye.to_array().map(f64::from);
    }
}

fn flythrough_camera(
    mut commands: Commands,
    mut smoke: ResMut<SmokeTest>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<terra_world::streaming::ChunkFocus>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
) {
    if !smoke.captured {
        return;
    }
    let elapsed = smoke
        .elapsed
        .saturating_sub(smoke.captured_at)
        .as_secs_f64();
    let progress = ((elapsed - 0.5) / smoke.sample_seconds).clamp(0.0, 1.0) as f32;
    for mut camera in &mut camera {
        let origin = *smoke.flight_origin.get_or_insert(*camera);
        // Two out-and-back trips cross both LOD thresholds and negative section coordinates.
        let trips = (smoke.sample_seconds / 120.0).floor().max(1.0) as f32;
        let leg = progress * 4.0 * trips;
        let triangle = if leg % 2.0 < 1.0 {
            leg % 1.0
        } else {
            1.0 - leg % 1.0
        };
        let mut eye = origin.translation + Vec3::new(triangle * 224.0, 0.0, 0.0);
        eye.y = eye
            .y
            .max(terrain.0.surface_height(f64::from(eye.x), f64::from(eye.z)) as f32 + 30.0);
        *camera = Transform::from_translation(eye).with_rotation(origin.rotation);
        focus.0 = eye.to_array().map(f64::from);
    }
    if progress >= f32::from(smoke.flight_captures + 1) * 0.25 && smoke.flight_captures < 4 {
        smoke.flight_captures += 1;
        if let Some(path) = std::env::var_os("TERRA_SCREENSHOT") {
            let path = std::path::PathBuf::from(path);
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let frame = path.with_file_name(format!("{stem}-{}.png", smoke.flight_captures));
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(frame));
            smoke.skip_frames = 3;
        }
    }
}
#[derive(SystemParam)]
struct SmokeWorld<'w, 's> {
    config: Res<'w, terra_core::GameConfig>,
    graphics: Option<Res<'w, terra_core::GraphicsConfig>>,
    weather: Option<Res<'w, terra_core::WorldWeather>>,
    weather_render: Res<'w, terra_render::WeatherRenderStatus>,
    scenario: Option<Res<'w, crate::bootstrap::BenchmarkScenario>>,
    ui: Option<Res<'w, crate::bootstrap::BenchmarkUi>>,
    resolution: Option<Res<'w, crate::bootstrap::BenchmarkResolution>>,
    menu: Option<Res<'w, terra_ui::menu::MenuState>>,
    inventory: Res<'w, terra_ui::InventoryView>,
    minimap: Res<'w, terra_ui::minimap::MiniMapView>,
    survival: Res<'w, terra_ui::SurvivalView>,
    debug_overlay: Res<'w, terra_ui::DebugOverlay>,
    integrated: Option<Res<'w, terra_server::IntegratedServer>>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    creatures: Option<Res<'w, terra_network::client::ReplicatedCreatures>>,
    creature_models: Query<'w, 's, &'static terra_render::creatures::CreatureAvatar>,
    upload: Res<'w, terra_render::MeshUploadStatus>,
    draws: Option<Res<'w, terra_render::DrawCallStatus>>,
    windows: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    controller_profile: Res<'w, terra_physics::CharacterProfile>,
    camera: Query<'w, 's, &'static Transform, With<TerraCamera>>,
    generation: Res<'w, WorldGeneration>,
    renderer: Res<'w, RenderStatus>,
    visibility: Option<Res<'w, terra_render::TerrainVisibility>>,
    water: Res<'w, WaterStatus>,
    props: Res<'w, PropRenderStatus>,
    previews: Res<'w, terra_render::RenderBlockPreviews>,
    portrait: Res<'w, terra_render::PlayerPreviewImage>,
    rendered: Res<'w, RenderedSections>,
    meshes: Res<'w, Assets<Mesh>>,
    visible_meshes: Query<'w, 's, &'static ViewVisibility, With<Mesh3d>>,
    point_lights: Query<'w, 's, &'static PointLight>,
    ranges: Query<'w, 's, &'static bevy::camera::visibility::VisibilityRange, With<Mesh3d>>,
    prop_smoke: Option<Res<'w, PropSmoke>>,
    diagnostics: Res<'w, bevy::diagnostic::DiagnosticsStore>,
    swaps: Res<'w, terra_render::LodSwapStatus>,
    animations: Res<'w, terra_render::ChunkAnimationStatus>,
    custom: Option<Res<'w, crate::custom_smoke::CustomSmoke>>,
}
fn smoke_test(
    mut commands: Commands,
    time: Res<Time>,
    scene: SmokeWorld,
    chunks: Res<ChunkStore>,
    target: Res<TargetBlock>,
    mut smoke: ResMut<SmokeTest>,
    mut exit: MessageWriter<AppExit>,
) {
    let world = &scene.generation;
    let now = Instant::now();
    let previous_frame = smoke.previous_frame;
    let frame_seconds = now.duration_since(previous_frame).as_secs_f64();
    smoke.previous_frame = now;
    smoke.timeline_frame = smoke.timeline_frame.saturating_add(1);
    let timeline_frame = smoke.timeline_frame;
    for frame in &mut smoke.worst_frames {
        if let Some(scopes) = &mut frame.scopes {
            scopes.resolve(timeline_frame);
        }
    }
    smoke.elapsed = now.duration_since(smoke.started_at);
    if let Err(error) = validate_benchmark_resolution(
        scene.resolution.as_deref(),
        scene.windows.iter().next(),
        smoke.elapsed,
        smoke.captured,
    ) {
        tracing::error!(%error, "Invalid benchmark resolution; no valid FPS result will be reported");
        smoke.frame_times.clear();
        exit.write(AppExit::error());
        return;
    }
    if smoke.skip_frames > 0 {
        smoke.skip_frames -= 1;
    } else if smoke.captured
        && smoke.elapsed - smoke.captured_at > Duration::from_millis(500)
        && (smoke.elapsed - smoke.captured_at).as_secs_f64() <= smoke.sample_seconds + 0.5
    {
        if smoke.sample_scope_start.is_none() {
            smoke.sample_scope_start = Some(terra_core::profiling::snapshot());
        }
        smoke.frame_times.push(frame_seconds);
        smoke.diagnostics.observe(&scene.diagnostics);
        let worst = FrameSpike {
            seconds: frame_seconds,
            sample_time: (smoke.elapsed - smoke.captured_at).as_secs_f64(),
            context: smoke.previous_context,
            scopes: FrameScopeTrace::new(previous_frame, now, timeline_frame),
        };
        if smoke.worst_frames.len() < 12
            || smoke
                .worst_frames
                .last()
                .is_some_and(|last| last.seconds < frame_seconds)
        {
            smoke.worst_frames.push(worst);
            smoke
                .worst_frames
                .sort_by(|a, b| b.seconds.total_cmp(&a.seconds));
            smoke.worst_frames.truncate(12);
        }
    }
    if smoke.sample_scope_start.is_some()
        && smoke.sample_scope_end.is_none()
        && (smoke.elapsed - smoke.captured_at).as_secs_f64() > smoke.sample_seconds + 0.5
    {
        smoke.sample_scope_end = Some(terra_core::profiling::snapshot());
    }
    smoke.previous_context = FrameContext {
        generation_jobs: scene.generation.pending_jobs,
        terrain_jobs: scene.renderer.pending_jobs,
        prop_jobs: scene.props.pending_jobs,
        upload_vertices: scene.upload.admitted_vertices,
        active_animations: scene.animations.active,
        queued_animations: scene.animations.queued,
        window_focused: scene.windows.iter().next().map(|window| window.focused),
        window_visible: scene.windows.iter().next().map(|window| window.visible),
        present_mode: scene
            .windows
            .iter()
            .next()
            .map(|window| window.present_mode),
    };
    if let Some(error) = scene
        .renderer
        .error
        .as_ref()
        .or(scene.props.error.as_ref())
        .or(scene.previews.error.as_ref())
        .or(scene.portrait.error.as_ref())
    {
        tracing::error!(%error, "Client terrain/prop rendering failed");
        exit.write(AppExit::error());
        return;
    }
    if smoke.flythrough && scene.ranges.iter().any(|range| !range.is_abrupt()) {
        tracing::error!("LOD flight found a dithered mesh in the depth-only pipeline");
        exit.write(AppExit::error());
        return;
    }
    if let GenerationStatus::Failed(error) = &world.status {
        tracing::error!(%error,"Client generation failed");
        exit.write(AppExit::error());
        return;
    }
    if smoke.mine
        && smoke.mined.is_none()
        && world.status == GenerationStatus::Ready
        && let Some(hit) = target.0
        && let Some(material) = chunks.block_at(hit.position)
    {
        commands.write_message(BlockEditRequest {
            position: hit.position,
            expected_material: material,
            material: 0,
        });
        smoke.mined = Some(hit.position);
    }
    if let Some(position) = smoke.mined
        && chunks.block_at(position) == Some(0)
        && world.status == GenerationStatus::Ready
    {
        if !smoke.mining_complete {
            tracing::info!(?position, "Mining and remeshing smoke test completed");
        }
        smoke.mining_complete = true;
    }
    let uploaded = chunks.loaded.iter().all(|(&position, section)| {
        scene
            .renderer
            .section_ready(position, Some(section), scene.visibility.as_deref())
    });
    let water_ready = scene.water.ready && (!smoke.water_check || scene.water.visible_tiles > 0);
    let props_ready = scene.props.ready
        && scene.prop_smoke.as_ref().is_none_or(|props| props.complete)
        && scene.custom.as_ref().is_none_or(|custom| custom.complete);
    let animations_ready = scene.animations.active == 0 && scene.animations.queued_in_view == 0;
    let scenario_ready = scene
        .scenario
        .as_ref()
        .is_none_or(|scenario| scenario.ready);
    let ui_ready = scene.ui.as_ref().is_none_or(|ui| {
        ui.ready
            && scene
                .menu
                .as_ref()
                .is_some_and(|menu| menu.page == terra_ui::menu::MenuPage::Playing)
    });
    let readiness_period = (smoke.elapsed.as_secs() / 30) as u32;
    if !smoke.captured && readiness_period > smoke.readiness_logged {
        smoke.readiness_logged = readiness_period;
        tracing::info!(generation = ?world.status, terrain_ready = scene.renderer.ready, props_ready, water_ready, uploaded, scenario_ready, ui_ready, camera = ?scene.camera.iter().next().map(|t| t.translation), animations = ?scene.animations.as_ref(), "Benchmark initial scene readiness");
    }
    if world.status == GenerationStatus::Ready
        && uploaded
        && scene.renderer.ready
        && water_ready
        && smoke.mining_complete
        && props_ready
        && animations_ready
        && scenario_ready
        && ui_ready
    {
        smoke.stable_ready += time.delta();
        if smoke.initial_ready.is_none() {
            smoke.initial_ready = Some(smoke.elapsed);
        }
    } else {
        smoke.stable_ready = Duration::ZERO;
    }
    if smoke.elapsed > Duration::from_secs(5)
        && world.status == GenerationStatus::Ready
        && uploaded
        && scene.renderer.ready
        && water_ready
        && smoke.stable_ready > Duration::from_millis(500)
        && smoke.mining_complete
        && props_ready
        && animations_ready
        && scenario_ready
        && ui_ready
        && !smoke.captured
    {
        if let Some(path) = std::env::var_os("TERRA_SCREENSHOT") {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    std::path::PathBuf::from(path),
                ));
            smoke.skip_frames = 3;
        }
        smoke.captured = true;
        tracing::info!(
            water_tiles = scene.water.visible_tiles,
            water_triangles = scene.water.triangles,
            "Scene capture ready"
        );
        smoke.captured_at = smoke.elapsed;
    }
    if smoke.water_check
        && smoke.captured
        && !smoke.motion_captured
        && smoke.elapsed - smoke.captured_at > Duration::from_secs(1)
    {
        if let Some(path) = std::env::var_os("TERRA_SCREENSHOT") {
            let path = std::path::PathBuf::from(path);
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let motion = path.with_file_name(format!("{stem}_motion.png"));
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(motion));
            smoke.skip_frames = 3;
        }
        smoke.motion_captured = true;
    }
    if smoke.captured
        && (smoke.elapsed - smoke.captured_at).as_secs_f64() > smoke.sample_seconds + 0.5
        // Preview cameras also own screenshot channels. Drain them before AppExit;
        // frame sampling above still ends at the original measurement boundary.
        && scene.previews.pending == 0
        && scene.portrait.ready
        && smoke.worst_frames.iter().all(|frame| {
            frame.scopes.as_ref().is_none_or(FrameScopeTrace::resolved)
        })
        && (!smoke.flythrough
            || (scene.renderer.ready
                && props_ready
                && animations_ready
                && smoke.flight_captures == 4
                // Allow the final screenshot readback to complete before AppExit.
                && (smoke.elapsed - smoke.captured_at).as_secs_f64() > smoke.sample_seconds + 1.0))
    {
        if let Err(error) = report_metrics(&smoke, &scene, &chunks) {
            tracing::error!(%error, "Invalid benchmark resolution at final summary");
            exit.write(AppExit::error());
            return;
        }
        if smoke.flythrough {
            if scene.swaps.completed == 0 {
                tracing::error!("LOD flight did not exercise a completed mesh swap");
                exit.write(AppExit::error());
                return;
            }
            tracing::info!(
                completed_lod_swaps = scene.swaps.completed,
                "LOD out-and-back flight completed and all mesh jobs finished"
            );
        }
        tracing::info!("Client smoke test completed");
        exit.write(AppExit::Success);
    } else if smoke.elapsed.as_secs_f64() > smoke.sample_seconds + smoke.load_grace_seconds {
        tracing::error!("Client smoke test timed out");
        exit.write(AppExit::error());
    }
}

fn validate_benchmark_resolution(
    requested: Option<&crate::bootstrap::BenchmarkResolution>,
    window: Option<&Window>,
    elapsed: Duration,
    measuring: bool,
) -> Result<(), String> {
    let Some(requested) = requested else {
        return Ok(());
    };
    // Match the existing initial-scene wait, allowing the OS to finish its
    // first DPI/configure events. Once measurement starts, every frame is strict.
    if !measuring && elapsed < Duration::from_secs(5) {
        return Ok(());
    }
    let actual = window.map(|window| [window.physical_width(), window.physical_height()]);
    if actual == Some(requested.physical) {
        return Ok(());
    }
    Err(format!(
        "Requested physical resolution {}x{}, actual primary-window resolution {:?} (scale factor {:?}); the window manager may have constrained or resized the benchmark window",
        requested.physical[0],
        requested.physical[1],
        actual,
        window.map(Window::scale_factor),
    ))
}

fn report_metrics(
    smoke: &SmokeTest,
    scene: &SmokeWorld,
    chunks: &ChunkStore,
) -> Result<(), String> {
    validate_benchmark_resolution(
        scene.resolution.as_deref(),
        scene.windows.iter().next(),
        smoke.elapsed,
        true,
    )?;
    let residents: Vec<_> = scene
        .creatures
        .as_ref()
        .into_iter()
        .flat_map(|c| c.creatures.values())
        .filter(|t| t.current.kind == terra_core::creatures::CreatureKind::Villager)
        .collect();
    let sleeping_residents = residents
        .iter()
        .filter(|t| t.current.behavior == terra_core::creatures::CreatureBehavior::Sleep)
        .count();
    let rendered_residents = scene
        .creature_models
        .iter()
        .filter(|avatar| residents.iter().any(|t| t.current.id == avatar.0))
        .count();
    if let Ok(require) = std::env::var("TERRA_BENCH_REQUIRE_VILLAGERS") {
        if residents.is_empty()
            || rendered_residents != residents.len()
            || require == "sleep" && sleeping_residents == 0
        {
            return Err(format!(
                "Generated villagers were not demonstrated: replicated={}, rendered={rendered_residents}, sleeping={sleeping_residents}",
                residents.len()
            ));
        }
        tracing::info!(residents = residents.len(), rendered_residents, sleeping_residents, names = ?residents.iter().map(|t| t.current.title()).collect::<Vec<_>>(), "Generated settlement residents verified through authoritative streaming and client rendering");
    }
    let mut samples = smoke.frame_times.clone();
    if std::env::var("TERRA_BENCH_DEBUG_OVERLAY").is_ok_and(|value| value == "1") {
        let minimum = scene.debug_overlay.fps_min.parse::<f64>().ok();
        let maximum = scene.debug_overlay.fps_max.parse::<f64>().ok();
        if !scene.debug_overlay.visible
            || !minimum.zip(maximum).is_some_and(|(low, high)| {
                low.is_finite() && high.is_finite() && low > 0.0 && high >= low
            })
        {
            return Err("Session FPS extrema were not displayed in the debug overlay".into());
        }
        tracing::info!(?minimum, ?maximum, "Session FPS overlay verified");
    }
    samples.sort_by(f64::total_cmp);
    let sampled_seconds: f64 = samples.iter().sum();
    let average_fps = samples.len() as f64 / sampled_seconds.max(f64::EPSILON);
    let active_props = chunks
        .loaded
        .values()
        .flat_map(|section| &section.props)
        .filter(|prop| !prop.removed)
        .count();
    let (controller_calls, controller_mean_ms, controller_max_ms) =
        scene.controller_profile.snapshot();
    let mode = if scene.integrated.is_some() {
        "integrated"
    } else if scene.session.as_ref().is_some_and(|session| session.ready) {
        "dedicated"
    } else {
        "offline"
    };
    tracing::info!(
        benchmark_mode = mode,
        requested_benchmark_mode = ?std::env::var("TERRA_BENCH_MODE").ok(),
        benchmark_ui = scene.ui.is_some(),
        menu_page = ?scene.menu.as_ref().map(|menu| menu.page),
        hotbar_visible = scene.inventory.visible,
        minimap_visible = scene.minimap.visible,
        survival_hud_visible = scene.survival.visible,
        graphics_source = if scene.ui.is_some() || std::env::var("TERRA_BENCH_GRAPHICS").is_ok_and(|value| value == "1") { "TERRA_CONFIG_DIR/graphics.toml" } else if scene.graphics.is_some() { "runtime GraphicsConfig" } else { "client.toml only; no GraphicsConfig" },
        graphics = ?scene.graphics.as_deref(),
        requested_physical_resolution = ?scene.resolution.as_ref().map(|resolution| resolution.physical),
        configured_resolution = ?(scene.config.client.width, scene.config.client.height),
        physical_resolution = ?scene.windows.iter().next().map(|window| (window.physical_width(), window.physical_height())),
        render_distance = scene.config.client.view_distance,
        vertical_view_distance = scene.config.client.vertical_view_distance,
        terrain_lod = ?(scene.config.client.lod_near_distance, scene.config.client.lod_far_distance),
        fov_degrees = scene.config.client.fov_degrees,
        vsync = scene.config.client.vsync,
        water_quality = ?scene.config.client.water_quality,
        weather_quality = ?scene.config.client.weather_quality,
        configured_weather = ?scene.config.client.weather,
        world_weather = ?scene.weather.as_deref(),
        rendered_weather = ?scene.weather_render.as_ref(),
        "Effective benchmark configuration (graphics file loading is explicit and opt-in)"
    );
    tracing::info!(
        benchmark_mode = mode,
        requested_physical_resolution = ?scene.resolution.as_ref().map(|resolution| resolution.physical),
        physical_resolution = ?scene.windows.iter().next().map(|window| (window.physical_width(), window.physical_height())),
        controller_calls,
        controller_mean_ms,
        controller_max_ms,
        camera = ?scene.camera.iter().next().map(|transform| (transform.translation, transform.rotation)),
        sample_frames = samples.len(),
        sample_window_seconds = smoke.sample_seconds,
        average_fps,
        minimum_fps = ?samples.last().filter(|seconds| **seconds > 0.0).map(|seconds| seconds.recip()),
        maximum_fps = ?samples.first().filter(|seconds| **seconds > 0.0).map(|seconds| seconds.recip()),
        worst_frame_ms = ?samples.last().map(|seconds| seconds * 1000.0),
        frames_below_32_fps = samples.iter().filter(|&&seconds| seconds > 1.0 / 32.0).count(),
        p95_frame_ms = percentile(&samples, 0.95) * 1000.0,
        p99_frame_ms = percentile(&samples, 0.99) * 1000.0,
        initial_load_seconds = smoke.initial_ready.map(|elapsed| elapsed.as_secs_f64()),
        resident_memory_kib = resident_memory_kib(),
        loaded_sections = chunks.loaded.len(),
        rendered_sections = scene.rendered.sections.len(),
        occluded_sections = scene.renderer.occluded_sections,
        mesh_assets = scene.meshes.len(),
        terrain_composition_jobs = scene.renderer.completed_mesh_jobs,
        far_groups = scene.renderer.far_groups,
        far_source_batches = scene.renderer.far_source_batches,
        far_cache_bytes = scene.renderer.far_cache_bytes,
        terrain_composition_worker_seconds = scene.renderer.composition_seconds,
        terrain_composition_max_ms = scene.renderer.max_composition_seconds * 1000.0,
        visible_mesh_entities = scene.visible_meshes.iter().filter(|v| v.get()).count(),
        point_lights = scene.point_lights.iter().count(),
        shadowed_point_lights = scene.point_lights.iter().filter(|light| light.shadow_maps_enabled && light.intensity > 0.0).count(),
        loaded_active_props = active_props,
        represented_props = scene.props.visible_props,
        prop_batches = scene.props.batches,
        prop_composition_jobs = scene.props.completed_jobs,
        prop_composition_worker_seconds = scene.props.composition_seconds,
        water_tiles = scene.water.visible_tiles,
        peak_admitted_upload_vertices = scene.upload.peak_admitted_vertices,
        peak_admitted_upload_bytes = scene.upload.peak_admitted_bytes,
        deferred_upload_batches = scene.upload.deferred_batches,
        chunk_animations = ?scene.animations.as_ref(),
        mesh_draw_submissions = ?scene.draws.as_ref().and_then(|status| status.snapshot()),
        "Native smoke benchmark (CPU wall frames; screenshot-adjacent frames excluded; GPU pass timings reported separately when enabled)"
    );
    smoke.diagnostics.report();
    if let (Some(start), Some(end)) = (&smoke.sample_scope_start, &smoke.sample_scope_end) {
        for &(name, sample) in end {
            let before = start
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| *value)
                .unwrap_or_default();
            let calls = sample.calls.saturating_sub(before.calls);
            let total_ms = sample.total_ms - before.total_ms;
            if calls > 0 {
                tracing::info!(
                    scope = name,
                    calls,
                    total_ms,
                    mean_ms = total_ms / calls as f64,
                    "CPU scope within native sampling window (wall intervals may overlap)"
                );
            }
        }
    }
    for (scope, sample) in terra_core::profiling::snapshot() {
        tracing::info!(
            scope,
            calls = sample.calls,
            total_ms = sample.total_ms,
            max_ms = sample.max_ms,
            over_8_ms = sample.over_8_ms,
            "CPU scope (includes startup; nested scopes are not additive)"
        );
    }
    for frame in &smoke.worst_frames {
        tracing::info!(
            frame_ms = frame.seconds * 1000.0,
            sample_time = frame.sample_time,
            generation_jobs = frame.context.generation_jobs,
            terrain_jobs = frame.context.terrain_jobs,
            prop_jobs = frame.context.prop_jobs,
            upload_vertices = frame.context.upload_vertices,
            active_animations = frame.context.active_animations,
            queued_animations = frame.context.queued_animations,
            window_focused = ?frame.context.window_focused,
            window_visible = ?frame.context.window_visible,
            present_mode = ?frame.context.present_mode,
            "Worst sampled frame with preceding frame context (correlation, not a GPU stall attribution)"
        );
        if let Some(scopes) = &frame.scopes {
            scopes.report(frame.seconds * 1000.0, frame.sample_time);
        }
    }
    Ok(())
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let rank = (sorted.len() as f64 * fraction).ceil() as usize;
    sorted.get(rank.saturating_sub(1)).copied().unwrap_or(0.0)
}

fn resident_memory_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[derive(Resource, Default)]
struct PropSmoke {
    fixture: Option<PropFixture>,
    sent: bool,
    complete: bool,
}

struct PropFixture {
    selected: WorldProp,
    neighbor: PropId,
    active_count: usize,
    selected_at: Instant,
}

#[derive(SystemParam)]
struct PropSmokeWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    renderer: Res<'w, RenderStatus>,
    props: Res<'w, PropRenderStatus>,
    journal: Res<'w, EditJournal>,
    target: Res<'w, TargetWorld>,
}

fn check_prop_removal(
    mut commands: Commands,
    world: PropSmokeWorld,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut smoke: ResMut<PropSmoke>,
    mut exit: MessageWriter<AppExit>,
) {
    if smoke.complete || world.generation.status != GenerationStatus::Ready {
        return;
    }
    if smoke.fixture.is_none() {
        let Some(mut camera) = camera.iter_mut().next() else {
            return;
        };
        let eye = camera.translation;
        let selected = world
            .chunks
            .loaded
            .values()
            .filter(|section| section.props.iter().filter(|p| !p.removed).count() >= 2)
            .flat_map(|section| &section.props)
            .filter(|p| !p.removed && matches!(p.kind, PropKind::Grass | PropKind::Flower))
            .filter(|p| {
                (0..=3).all(|dy| {
                    let [x, y, z] = p.position.map(|v| v.floor() as i64);
                    world.chunks.block_at([x, y + dy, z]) == Some(0)
                })
            })
            .min_by(|a, b| {
                let distance = |p: &WorldProp| {
                    Vec3::from_array(p.position.map(|v| v as f32)).distance_squared(eye)
                };
                distance(a).total_cmp(&distance(b)).then(a.id.cmp(&b.id))
            })
            .copied();
        let Some(selected) = selected else { return };
        let Some(section) = world.chunks.loaded.get(&selected.section) else {
            return;
        };
        let Some(neighbor) = section
            .props
            .iter()
            .find(|p| !p.removed && p.id != selected.id)
        else {
            return;
        };
        let root = Vec3::from_array(selected.position.map(|v| v as f32));
        *camera = Transform::from_translation(root + Vec3::new(0.0, 2.0, 0.1))
            .looking_at(root + Vec3::Y * 0.1, Vec3::Z);
        smoke.fixture = Some(PropFixture {
            selected,
            neighbor: neighbor.id,
            active_count: section.props.iter().filter(|p| !p.removed).count(),
            selected_at: Instant::now(),
        });
        return;
    }
    let Some(fixture) = &smoke.fixture else {
        return;
    };
    if fixture.selected_at.elapsed() > Duration::from_secs(20) {
        tracing::error!(
            prop_id = fixture.selected.id.0,
            "Prop smoke picking/removal timed out"
        );
        exit.write(AppExit::error());
        return;
    }
    if !smoke.sent {
        if matches!(world.target.0, Some(WorldTarget::Prop(hit)) if hit.prop.id == fixture.selected.id)
        {
            commands.write_message(PropRemoveRequest {
                id: fixture.selected.id,
                section: fixture.selected.section,
            });
            smoke.sent = true;
        }
        return;
    }
    let Some(section) = world.chunks.loaded.get(&fixture.selected.section) else {
        return;
    };
    if !world
        .journal
        .prop_removed(fixture.selected.section, fixture.selected.id)
    {
        return;
    }
    let selected_removed = section
        .props
        .iter()
        .any(|p| p.id == fixture.selected.id && p.removed);
    let neighbor_retained = section
        .props
        .iter()
        .any(|p| p.id == fixture.neighbor && !p.removed);
    let active_count = section.props.iter().filter(|p| !p.removed).count();
    if !selected_removed || !neighbor_retained || active_count + 1 != fixture.active_count {
        tracing::error!(
            selected_removed,
            neighbor_retained,
            active_count,
            "Prop removal affected the wrong objects"
        );
        exit.write(AppExit::error());
    } else if world.renderer.ready && world.props.ready {
        tracing::info!(
            prop_id = fixture.selected.id.0,
            neighbor_id = fixture.neighbor.0,
            "Individual prop picked, removed and journaled; neighbor retained"
        );
        smoke.complete = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::BenchmarkResolution;

    #[test]
    fn benchmark_resolution_compares_physical_pixels_independently_of_dpi() {
        let requested = BenchmarkResolution {
            physical: [3840, 2160],
        };
        let window = Window {
            resolution: bevy::window::WindowResolution::new(3840, 2160)
                .with_scale_factor_override(1.5),
            ..default()
        };
        assert_eq!(window.width(), 2560.0);
        assert_eq!(window.height(), 1440.0);
        assert_eq!(
            validate_benchmark_resolution(
                Some(&requested),
                Some(&window),
                Duration::from_secs(30),
                true
            ),
            Ok(())
        );
    }

    #[test]
    fn constrained_initial_window_gets_settle_time_but_never_starts_sampling() {
        let requested = BenchmarkResolution {
            physical: [3840, 2160],
        };
        let window = Window {
            resolution: (3840, 2124).into(),
            ..default()
        };
        assert_eq!(
            validate_benchmark_resolution(
                Some(&requested),
                Some(&window),
                Duration::from_millis(4999),
                false
            ),
            Ok(())
        );
        let error = validate_benchmark_resolution(
            Some(&requested),
            Some(&window),
            Duration::from_secs(5),
            false,
        )
        .unwrap_err();
        assert!(error.contains("3840x2160"));
        assert!(error.contains("[3840, 2124]"));
        assert!(
            validate_benchmark_resolution(Some(&requested), Some(&window), Duration::ZERO, true)
                .is_err()
        );
    }

    #[test]
    fn intentional_gui_resizes_and_client_only_benchmarks_have_no_resolution_contract() {
        let window = Window {
            resolution: (800, 600).into(),
            ..default()
        };
        assert_eq!(
            validate_benchmark_resolution(None, Some(&window), Duration::from_secs(60), true),
            Ok(())
        );
        assert_eq!(
            validate_benchmark_resolution(None, None, Duration::from_secs(60), true),
            Ok(())
        );
    }

    #[test]
    fn resize_or_missing_primary_window_invalidates_an_active_benchmark() {
        let requested = BenchmarkResolution {
            physical: [3840, 2124],
        };
        let mut window = Window {
            resolution: (3840, 2124).into(),
            ..default()
        };
        assert!(
            validate_benchmark_resolution(
                Some(&requested),
                Some(&window),
                Duration::from_secs(10),
                true
            )
            .is_ok()
        );
        window.resolution.set_physical_resolution(1920, 1080);
        assert!(
            validate_benchmark_resolution(
                Some(&requested),
                Some(&window),
                Duration::from_secs(11),
                true
            )
            .is_err()
        );
        assert!(
            validate_benchmark_resolution(Some(&requested), None, Duration::from_secs(11), true)
                .is_err()
        );
    }
}
