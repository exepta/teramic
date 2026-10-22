//! Opt-in closed/open/closed comparison of the actual integrated inventory UI.
use bevy::{app::AppExit, diagnostic::DiagnosticsStore, prelude::*};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use terra_ui::{InventoryAction, InventoryView};

const WINDOW: Duration = Duration::from_secs(12);
const SETTLE: Duration = Duration::from_secs(2);
const DEADLINE: Duration = Duration::from_secs(180);
const MAX_SAMPLES: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Loading,
    Closed,
    Opening,
    Open,
    Closing,
    ClosedAgain,
    Finished,
}

impl Stage {
    fn measuring(self) -> bool {
        matches!(self, Self::Closed | Self::Open | Self::ClosedAgain)
    }

    fn expected_open(self) -> bool {
        self == Self::Open
    }
}

#[derive(Resource)]
struct InventoryBenchmark {
    started: Instant,
    entered: Instant,
    previous: Instant,
    stable_since: Option<Instant>,
    stage: Stage,
    frames: u64,
    times: Vec<f64>,
    diagnostics: crate::diagnostic_report::NativeDiagnostics,
    scopes: BTreeMap<&'static str, terra_core::profiling::ScopeSample>,
    camera: Option<Transform>,
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    if !std::env::var("TERRA_BENCH_INVENTORY").is_ok_and(|value| value == "1") {
        return Ok(());
    }
    validate_options(
        std::env::args().any(|arg| arg == "--smoke-test-benchmark"),
        std::env::var("TERRA_BENCH_MODE").as_deref() == Ok("integrated"),
        std::env::var("TERRA_BENCH_UI").as_deref() == Ok("1"),
        std::env::var("TERRA_BENCH_SECONDS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok()),
        std::env::var_os("TERRA_SOAK_ROUTE").is_some(),
    )?;
    let now = Instant::now();
    app.insert_resource(InventoryBenchmark {
        started: now,
        entered: now,
        previous: now,
        stable_since: None,
        stage: Stage::Loading,
        frames: 0,
        times: Vec::with_capacity(2048),
        diagnostics: default(),
        scopes: BTreeMap::new(),
        camera: None,
    })
    .add_systems(Last, sample);
    Ok(())
}

fn validate_options(
    benchmark: bool,
    integrated: bool,
    ui: bool,
    seconds: Option<f64>,
    route: bool,
) -> Result<(), String> {
    if !benchmark || !integrated || !ui {
        return Err("TERRA_BENCH_INVENTORY=1 requires --smoke-test-benchmark, TERRA_BENCH_MODE=integrated and TERRA_BENCH_UI=1".into());
    }
    if !seconds.is_some_and(|seconds| seconds.is_finite() && seconds >= 60.0) {
        return Err("The three-phase inventory benchmark requires TERRA_BENCH_SECONDS=90 (at least 60) so the ordinary smoke test cannot finish first".into());
    }
    if route {
        return Err("The stationary inventory comparison cannot run with TERRA_SOAK_ROUTE".into());
    }
    Ok(())
}

fn runtime_ready(world: &World) -> bool {
    world.contains_resource::<terra_server::IntegratedServer>()
        && world
            .get_resource::<crate::bootstrap::BenchmarkUi>()
            .is_some_and(|ui| ui.ready)
        && world
            .get_resource::<terra_network::client::ClientSession>()
            .is_some_and(|session| session.ready && !session.teleporting && !session.dead())
        && world
            .get_resource::<terra_ui::UiDocumentState>()
            .is_some_and(|document| document.0.load == terra_ui::UiLoadState::Ready)
        && world
            .get_resource::<terra_ui::menu::MenuState>()
            .is_some_and(|menu| menu.page == terra_ui::menu::MenuPage::Playing)
}

fn panel_ready(world: &mut World) -> bool {
    world
        .query::<(&terra_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .any(|(id, node)| id.0 == "inventory-panel" && node.size().min_element() > 0.0)
}

fn start_window(state: &mut InventoryBenchmark, stage: Stage, now: Instant) {
    state.stage = stage;
    state.entered = now;
    state.previous = now;
    state.frames = 0;
    state.times.clear();
    state.stable_since = None;
    state.diagnostics = default();
    state.scopes = terra_core::profiling::snapshot().into_iter().collect();
    tracing::info!(
        ?stage,
        seconds = WINDOW.as_secs(),
        "Inventory benchmark phase started"
    );
}

fn fail(world: &mut World, state: &mut InventoryBenchmark, reason: &str) {
    tracing::error!(stage = ?state.stage, reason, "Inventory benchmark failed");
    state.stage = Stage::Finished;
    world.write_message(AppExit::error());
}

fn sample(world: &mut World) {
    let now = Instant::now();
    world.resource_scope(|world, mut state: Mut<InventoryBenchmark>| {
        if state.stage == Stage::Finished {
            return;
        }
        if now.duration_since(state.started) > DEADLINE {
            fail(world, &mut state, "180-second deadline exceeded");
            return;
        }
        if state.stage == Stage::Loading {
            if crate::smoke_test::benchmark_ready_time(world).is_none() || !runtime_ready(world) {
                return;
            }
            if world.resource::<InventoryView>().open {
                fail(
                    world,
                    &mut state,
                    "Inventory was already open before the comparison",
                );
                return;
            }
            state.camera = world
                .query_filtered::<&Transform, With<terra_player::TerraCamera>>()
                .iter(world)
                .next()
                .copied();
            if state.camera.is_none() {
                fail(world, &mut state, "No actual gameplay camera exists");
                return;
            }
            start_window(&mut state, Stage::Closed, now);
            return;
        }
        if !runtime_ready(world) {
            fail(
                world,
                &mut state,
                "Integrated authority or actual TiltUI became unavailable",
            );
            return;
        }
        let camera = world
            .query_filtered::<&Transform, With<terra_player::TerraCamera>>()
            .iter(world)
            .next()
            .copied();
        if !same_camera(state.camera.as_ref(), camera.as_ref()) {
            fail(
                world,
                &mut state,
                "Gameplay camera moved during the stationary comparison",
            );
            return;
        }
        let open = world.resource::<InventoryView>().open;
        if matches!(state.stage, Stage::Opening | Stage::Closing) {
            let ready = open == (state.stage == Stage::Opening)
                && panel_ready(world) == (state.stage == Stage::Opening);
            if !ready {
                state.stable_since = None;
                return;
            }
            let stable_since = *state.stable_since.get_or_insert(now);
            if now.duration_since(stable_since) >= SETTLE {
                let next = if state.stage == Stage::Opening {
                    Stage::Open
                } else {
                    Stage::ClosedAgain
                };
                start_window(&mut state, next, now);
            }
            return;
        }
        if !state.stage.measuring() || open != state.stage.expected_open() {
            fail(
                world,
                &mut state,
                "Inventory visibility changed outside the scripted toggles",
            );
            return;
        }
        let frame_ms = now.duration_since(state.previous).as_secs_f64() * 1000.0;
        state.previous = now;
        state.frames += 1;
        if state.times.len() < MAX_SAMPLES {
            state.times.push(frame_ms);
            if let Some(diagnostics) = world.get_resource::<DiagnosticsStore>() {
                state.diagnostics.observe(diagnostics);
            }
        }
        if now.duration_since(state.entered) < WINDOW {
            return;
        }
        report(world, &state, now);
        match state.stage {
            Stage::Closed => {
                world.write_message(InventoryAction::Toggle);
                state.stage = Stage::Opening;
            }
            Stage::Open => {
                world.write_message(InventoryAction::Toggle);
                state.stage = Stage::Closing;
            }
            Stage::ClosedAgain => {
                state.stage = Stage::Finished;
                tracing::info!(
                    "Inventory benchmark completed all three real closed/open/closed phases"
                );
                world.write_message(AppExit::Success);
            }
            _ => unreachable!(),
        }
    });
}

fn same_camera(before: Option<&Transform>, current: Option<&Transform>) -> bool {
    before.zip(current).is_some_and(|(before, current)| {
        before.translation.distance_squared(current.translation) <= 0.0025
            && before.rotation.dot(current.rotation).abs() >= 0.99999
    })
}

fn frame_summary(times: &[f64]) -> [f64; 4] {
    if times.is_empty() {
        return [0.0; 4];
    }
    let mut sorted = times.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    let percentile = |fraction: f64| sorted[(sorted.len() as f64 * fraction).ceil() as usize - 1];
    [
        percentile(0.5),
        percentile(0.95),
        percentile(0.99),
        *sorted.last().unwrap(),
    ]
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

fn report(world: &mut World, state: &InventoryBenchmark, now: Instant) {
    let summary = frame_summary(&state.times);
    let elapsed = now.duration_since(state.entered).as_secs_f64();
    let cameras: Vec<_> = world
        .query::<(Entity, &Camera, Has<terra_player::TerraCamera>)>()
        .iter(world)
        .filter(|(_, camera, _)| camera.is_active)
        .map(|(entity, camera, gameplay)| (entity, camera.order, gameplay))
        .collect();
    let ui_nodes = world
        .query_filtered::<Entity, With<Node>>()
        .iter(world)
        .count();
    let tilt_nodes = world
        .query_filtered::<Entity, With<tilt_ui::TiltElement>>()
        .iter(world)
        .count();
    let property_binding_nodes = world
        .query_filtered::<Entity, With<tilt_ui::PropertyBindings>>()
        .iter(world)
        .count();
    let event_binding_nodes = world
        .query_filtered::<Entity, With<tilt_ui::EventBindings>>()
        .iter(world)
        .count();
    let image_nodes = world
        .query_filtered::<Entity, With<ImageNode>>()
        .iter(world)
        .count();
    tracing::info!(
        phase = ?state.stage,
        window_seconds = elapsed,
        frames = state.frames,
        sample_frames = state.times.len(),
        dropped_samples = state.frames.saturating_sub(state.times.len() as u64),
        fps = state.frames as f64 / elapsed,
        median_ms = summary[0], p95_ms = summary[1], p99_ms = summary[2], max_ms = summary[3],
        resident_memory_kib = ?resident_memory_kib(),
        entities = world.entities().count_spawned(),
        images = world.get_resource::<Assets<Image>>().map_or(0, Assets::len),
        meshes = world.get_resource::<Assets<Mesh>>().map_or(0, Assets::len),
        ui_nodes, tilt_nodes, property_binding_nodes, event_binding_nodes, image_nodes,
        active_cameras = ?cameras,
        inventory_open = world.resource::<InventoryView>().open,
        camera = ?state.camera,
        "Inventory benchmark phase: CPU wall frames, unchanged scene and quality"
    );
    tracing::info!(phase = ?state.stage, "Inventory benchmark GPU diagnostics follow; individual passes are not total GPU frame time");
    state.diagnostics.report();
    for (scope, sample) in terra_core::profiling::snapshot() {
        let previous = state.scopes.get(scope).copied().unwrap_or_default();
        let calls = sample.calls.saturating_sub(previous.calls);
        if calls == 0 {
            continue;
        }
        let total_ms = (sample.total_ms - previous.total_ms).max(0.0);
        tracing::info!(
            phase = ?state.stage, scope, calls, total_ms,
            mean_ms = total_ms / calls as f64,
            over_8_ms = sample.over_8_ms.saturating_sub(previous.over_8_ms),
            cumulative_max_ms = sample.max_ms,
            "Inventory benchmark CPU scope delta; cumulative maximum includes earlier phases and startup, nested scopes are not additive"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_requires_real_integrated_ui_and_enough_time_without_a_route() {
        assert!(validate_options(true, true, true, Some(90.0), false).is_ok());
        for options in [
            (false, true, true, Some(90.0), false),
            (true, false, true, Some(90.0), false),
            (true, true, false, Some(90.0), false),
            (true, true, true, None, false),
            (true, true, true, Some(3.0), false),
            (true, true, true, Some(f64::NAN), false),
            (true, true, true, Some(90.0), true),
        ] {
            assert!(
                validate_options(options.0, options.1, options.2, options.3, options.4).is_err()
            );
        }
    }

    #[test]
    fn summary_is_nearest_rank_and_camera_changes_are_rejected() {
        assert_eq!(frame_summary(&[]), [0.0; 4]);
        assert_eq!(
            frame_summary(&(1..=100).rev().map(f64::from).collect::<Vec<_>>()),
            [50.0, 95.0, 99.0, 100.0]
        );
        let camera = Transform::default();
        assert!(same_camera(Some(&camera), Some(&camera)));
        assert!(!same_camera(Some(&camera), None));
        assert!(!same_camera(
            Some(&camera),
            Some(&Transform::from_xyz(0.1, 0.0, 0.0))
        ));
    }
}
