//! Opt-in closed/open/closed comparison of the actual integrated inventory UI.
use bevy::{app::AppExit, diagnostic::DiagnosticsStore, prelude::*};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use terra_ui::{InventoryAction, InventoryView};

#[path = "inventory_benchmark_chest.rs"]
pub(crate) mod chest;

const WINDOW: Duration = Duration::from_secs(12);
const SETTLE: Duration = Duration::from_secs(2);
const DEADLINE: Duration = Duration::from_secs(180);
const MAX_SAMPLES: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Player,
    Chest,
}

#[derive(Clone, Copy, Debug)]
struct BenchmarkOptions {
    rounds: u8,
    surface: Surface,
    extended: bool,
}

fn benchmark_options(
    rounds: Option<&str>,
    surface: Option<&str>,
) -> Result<BenchmarkOptions, String> {
    let count = rounds
        .map_or(Ok(1), str::parse::<u8>)
        .ok()
        .filter(|count| (1..=2).contains(count))
        .ok_or("TERRA_BENCH_INVENTORY_ROUNDS must be 1 or 2")?;
    let selected = match surface {
        None | Some("player") => Surface::Player,
        Some("chest") => Surface::Chest,
        _ => return Err("TERRA_BENCH_INVENTORY_SURFACE must be player or chest".into()),
    };
    Ok(BenchmarkOptions {
        rounds: count,
        surface: selected,
        extended: rounds.is_some() || surface.is_some(),
    })
}

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
    options: BenchmarkOptions,
    round: u8,
    chest: Option<chest::ChestFixture>,
    retained_owners: BTreeMap<&'static str, (Entity, Option<Entity>)>,
    trace_dom_transitions: bool,
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
    transition_times: Vec<f64>,
    transition_reported: bool,
    visible_frames: u8,
    transition_trace: Option<TransitionTrace>,
}

struct TransitionTrace {
    stage: Stage,
    start: Instant,
    end: Option<Instant>,
    worst_frames: Vec<(Instant, Instant)>,
    remaining_frames: u8,
}

impl TransitionTrace {
    fn new(stage: Stage, start: Instant) -> Option<Self> {
        terra_core::profiling::timeline_enabled().then(|| Self {
            stage,
            start,
            end: None,
            worst_frames: Vec::with_capacity(3),
            remaining_frames: 2,
        })
    }

    fn observe(&mut self, start: Instant, end: Instant) {
        let elapsed = end.duration_since(start);
        let index = self
            .worst_frames
            .partition_point(|(old_start, old_end)| old_end.duration_since(*old_start) >= elapsed);
        if index < 3 {
            if self.worst_frames.len() == 3 {
                self.worst_frames.pop();
            }
            self.worst_frames.insert(index, (start, end));
        }
    }

    fn ready(&mut self) -> bool {
        if self.end.is_none() {
            return false;
        }
        self.remaining_frames = self.remaining_frames.saturating_sub(1);
        self.remaining_frames == 0
    }

    fn report(self) {
        let Some(end) = self.end else { return };
        self.report_window("request-to-visible", self.start, end);
        for (start, end) in &self.worst_frames {
            self.report_window("transition-frame", *start, *end);
        }
    }

    fn report_window(&self, window: &str, start: Instant, end: Instant) {
        let Some(trace) = terra_core::profiling::timeline_snapshot(start, end, 12) else {
            return;
        };
        let interval_ms = end.duration_since(start).as_secs_f64() * 1000.0;
        tracing::info!(stage = ?self.stage, window, interval_ms,
            overlapping_closed_scopes = trace.overlapping_scopes,
            history_truncated = trace.history_truncated, ring_evictions = trace.evicted_scopes,
            "Inventory transition CPU scope window (closed spans only; delayed two frames; nested/concurrent spans are not additive; no GPU attribution)");
        for group in trace.groups {
            tracing::info!(stage = ?self.stage, window, interval_ms, scope = group.name,
                calls = group.calls, summed_overlap_ms = group.overlap_ms,
                summed_total_scope_ms = group.total_ms, max_individual_scope_ms = group.max_ms,
                "Inventory transition CPU scope group");
        }
        for scope in trace.scopes {
            let offset = |point: Instant| {
                if point >= start {
                    point.duration_since(start).as_secs_f64() * 1000.0
                } else {
                    -start.duration_since(point).as_secs_f64() * 1000.0
                }
            };
            tracing::info!(stage = ?self.stage, window, interval_ms, scope = scope.name,
                thread = ?scope.thread, overlap_ms = scope.overlap_ms, total_scope_ms = scope.total_ms,
                start_from_window_ms = offset(scope.start), end_from_window_ms = offset(scope.end),
                "Inventory transition closed CPU scope");
        }
    }
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
    let options = benchmark_options(
        std::env::var("TERRA_BENCH_INVENTORY_ROUNDS")
            .ok()
            .as_deref(),
        std::env::var("TERRA_BENCH_INVENTORY_SURFACE")
            .ok()
            .as_deref(),
    )?;
    let seconds = std::env::var("TERRA_BENCH_SECONDS")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let required_seconds = (f64::from(options.rounds) * 40.0
        + 10.0
        + if options.surface == Surface::Chest {
            20.0
        } else {
            0.0
        })
    .max(60.0);
    if seconds < required_seconds {
        return Err(format!(
            "The selected inventory surface/rounds require TERRA_BENCH_SECONDS of at least {required_seconds:.0}; 120 covers both chest rounds"
        ));
    }
    let now = Instant::now();
    app.insert_resource(InventoryBenchmark {
        options,
        round: 1,
        chest: (options.surface == Surface::Chest).then(chest::ChestFixture::default),
        retained_owners: BTreeMap::new(),
        trace_dom_transitions: std::env::var("TERRA_BENCH_INVENTORY_DOM_TRACE")
            .is_ok_and(|value| value == "1"),
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
        transition_times: Vec::with_capacity(64),
        transition_reported: false,
        visible_frames: 0,
        transition_trace: None,
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
    if world.resource::<InventoryView>().layout_style != "opacity: 1; pointer-events: auto;" {
        return false;
    }
    world
        .query::<(&terra_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .any(|(id, node)| id.0 == "inventory-panel" && node.size().min_element() > 0.0)
}

fn painted_element(world: &mut World, id: &str) -> bool {
    let entity = world
        .query::<(Entity, &terra_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, name)| (name.0 == id).then_some(entity));
    let Some(entity) = entity else { return false };
    if world
        .get::<ComputedNode>(entity)
        .is_none_or(|node| node.size().min_element() <= 0.0)
        || !world
            .get::<InheritedVisibility>(entity)
            .is_some_and(|visibility| visibility.get())
    {
        return false;
    }
    let mut ancestor = Some(entity);
    for _ in 0..64 {
        let Some(entity) = ancestor else { return true };
        if world
            .get::<Node>(entity)
            .is_some_and(|node| node.display == Display::None)
            || world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
            || world
                .get::<tilt_ui::RuntimeComputedStyle>(entity)
                .is_some_and(|style| style.0.opacity.is_some_and(|alpha| alpha <= 0.0))
        {
            return false;
        }
        ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    false
}

fn surface_ready(world: &mut World, state: &InventoryBenchmark, open: bool) -> bool {
    let session = world.resource::<terra_network::client::ClientSession>();
    if let Some(chest) = &state.chest {
        let position = chest.position();
        if open {
            if !session
                .container
                .as_ref()
                .is_some_and(|container| container.anchor == position && container.valid())
                || !session.open_chests.contains(&position)
                || !world.resource::<InventoryView>().container
                || world.resource::<InventoryView>().container_items.len()
                    != terra_core::containers::CHEST_SLOTS
            {
                return false;
            }
        } else if session.container.is_some() || session.open_chests.contains(&position) {
            return false;
        }
    } else if session.container.is_some() {
        return false;
    }
    !open
        || (painted_element(world, "inventory-backdrop")
            && painted_element(world, "inventory-panel")
            && (state.chest.is_none() || painted_element(world, "container-grid")))
}

fn focus_within_inventory(world: &World, focus: Option<Entity>) -> bool {
    let mut ancestor = focus;
    for _ in 0..64 {
        let Some(entity) = ancestor else { return false };
        if world.get::<terra_ui::ElementId>(entity).is_some_and(|id| {
            matches!(
                id.0.as_str(),
                "inventory-backdrop" | "inventory-shell" | "inventory-panel" | "inventory-search"
            ) || id.0.starts_with("craft-")
        }) {
            return true;
        }
        ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    false
}

fn trace_transition_dom(world: &mut World, state: &InventoryBenchmark, now: Instant) {
    if !state.trace_dom_transitions || state.transition_times.len() > 32 {
        return;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("benchmark.inventory_dom_trace");
    let view = world.resource::<InventoryView>();
    let (open, player, container, assembly, slots, prewarming, layout_style) = (
        view.open,
        view.player,
        view.container,
        view.assembly,
        view.container_items.len(),
        view.prewarming,
        view.layout_style.clone(),
    );
    let session = world.resource::<terra_network::client::ClientSession>();
    let snapshot_anchor = session.container.as_ref().map(|snapshot| snapshot.anchor);
    let inventory_response = session.inventory_response;
    // The private InventoryState pending-response value can additionally be
    // inspected with TERRA_INPUT_TRACE=1. This flag is explicitly derived from
    // the public benchmark request and received authoritative snapshot.
    let awaiting_snapshot = state.options.surface == Surface::Chest
        && state.stage == Stage::Opening
        && open
        && snapshot_anchor.is_none();
    let mut ui_nodes = 0;
    let mut tilt_nodes = 0;
    let mut binding_nodes = 0;
    let mut parked_nodes = 0;
    let mut player_grid = None;
    let mut container_grid = None;
    let mut equipment_grid = None;
    let mut crafting_grid = None;
    let mut first_cells = Vec::with_capacity(6);
    for (entity, id, node, tilt, binding, parked) in world
        .query_filtered::<(
            Entity,
            Option<&terra_ui::ElementId>,
            Has<Node>,
            Has<tilt_ui::TiltElement>,
            Has<tilt_ui::PropertyBindings>,
            Has<terra_ui::ParkedUi>,
        ), Or<(With<Node>, With<tilt_ui::TiltElement>)>>()
        .iter(world)
    {
        ui_nodes += usize::from(node);
        tilt_nodes += usize::from(tilt);
        binding_nodes += usize::from(binding);
        parked_nodes += usize::from(parked);
        match id.map(|id| id.0.as_str()) {
            Some("inventory-grid") => player_grid = Some(entity),
            Some("container-grid") => container_grid = Some(entity),
            Some("inventory-equipment") => equipment_grid = Some(entity),
            Some("inventory-crafting") => crafting_grid = Some(entity),
            Some("inventory-cell") if first_cells.len() < 6 => first_cells.push(entity),
            _ => {}
        }
    }
    tracing::info!(surface = ?state.options.surface, round = state.round, stage = ?state.stage,
        transition_frame = state.transition_times.len(),
        since_request_ms = now.duration_since(state.entered).as_secs_f64() * 1000.0,
        preceding_frame_ms = ?state.transition_times.last(),
        open, player, container, assembly, slots, prewarming, %layout_style,
        ?snapshot_anchor, inventory_response, awaiting_snapshot,
        ui_nodes, tilt_nodes, binding_nodes, parked_nodes,
        ?player_grid, ?container_grid, ?equipment_grid, ?crafting_grid, ?first_cells,
        "Inventory benchmark transition DOM snapshot; opt-in observer work, awaiting_snapshot is derived and counts include the entire native UI");
}

fn interaction_evidence(world: &mut World, state: &mut InventoryBenchmark) -> Result<(), String> {
    let open = world.resource::<InventoryView>().open;
    let blocked = world.resource::<terra_player::PlayerInputBlock>().0;
    let focus = world
        .get_resource::<bevy::input_focus::InputFocus>()
        .and_then(|focus| focus.get());
    let focus_id = focus
        .and_then(|entity| world.get::<terra_ui::ElementId>(entity))
        .map(|id| id.0.clone());
    let focus_is_window = focus.is_some_and(|entity| world.get::<Window>(entity).is_some());
    let focus_within_inventory = focus_within_inventory(world, focus);
    let camera = world.resource::<terra_player::CameraInputState>();
    let cursor_captured = camera.captured;
    let (prewarming, container, container_slots) = {
        let view = world.resource::<InventoryView>();
        (view.prewarming, view.container, view.container_items.len())
    };
    let visible_tooltips = world
        .query::<(&terra_ui::TooltipOwner, &Visibility)>()
        .iter(world)
        .filter(|(_, visibility)| **visibility != Visibility::Hidden)
        .count();
    let backdrop_display = world
        .query::<(&terra_ui::ElementId, &Node)>()
        .iter(world)
        .find_map(|(id, node)| (id.0 == "inventory-backdrop").then_some(node.display));
    let panel_painted = painted_element(world, "inventory-panel");
    let container_grid_painted = painted_element(world, "container-grid");
    for id in ["inventory-backdrop", "inventory-shell", "inventory-panel"] {
        let entity = world
            .query::<(Entity, &terra_ui::ElementId)>()
            .iter(world)
            .find_map(|(entity, name)| (name.0 == id).then_some(entity));
        let Some(entity) = entity else { continue };
        let owner = world
            .get::<tilt_ui::ComponentStyleOwner>(entity)
            .map(|owner| owner.0);
        let previous = state.retained_owners.insert(id, (entity, owner));
        let retained_entity = previous.is_some_and(|(old, _)| old == entity);
        let retained_owner =
            previous.is_some_and(|(old, old_owner)| old == entity && old_owner == owner);
        tracing::info!(surface = ?state.options.surface, round = state.round, stage = ?state.stage,
            id, ?entity, ?owner, retained_entity, retained_owner,
            owner_is_ghost = owner.is_some_and(|owner| world.get::<bevy::ui::experimental::GhostNode>(owner).is_some()),
            "Inventory benchmark retained style owner");
        if previous.is_some_and(|(old, old_owner)| old == entity && old_owner != owner) {
            return Err(format!(
                "Retained inventory element {id} changed its style owner"
            ));
        }
    }
    let backdrop_owner = state
        .retained_owners
        .get("inventory-backdrop")
        .and_then(|(_, owner)| *owner);
    let shell_owner = state
        .retained_owners
        .get("inventory-shell")
        .and_then(|(_, owner)| *owner);
    let content_root = shell_owner.filter(|owner| Some(*owner) != backdrop_owner);
    let content_root_is_ghost = content_root.is_some_and(|owner| {
        world
            .get::<bevy::ui::experimental::GhostNode>(owner)
            .is_some()
    });
    tracing::info!(surface = ?state.options.surface, round = state.round, stage = ?state.stage,
        ?backdrop_owner, ?shell_owner, ?content_root, content_root_is_ghost,
        "Inventory benchmark content ownership boundary; legacy templates may share the backdrop owner");
    tracing::info!(surface = ?state.options.surface, round = state.round, stage = ?state.stage,
        open, blocked, cursor_captured, os_capture_exercised = false, ?focus, ?focus_id,
        focus_is_window, focus_within_inventory,
        prewarming, container, container_slots, panel_painted, container_grid_painted,
        visible_tooltips, ?backdrop_display,
        "Inventory benchmark surface/input validation; native OS capture is disabled by smoke mode");
    if blocked != open {
        return Err("Gameplay input blocking disagrees with the settled inventory state".into());
    }
    if !open && (focus_within_inventory || visible_tooltips != 0) {
        return Err("Closed inventory retained its keyboard focus or a visible tooltip".into());
    }
    Ok(())
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
    if state.options.extended {
        tracing::info!(surface = ?state.options.surface, round = state.round, ?stage,
            "Inventory benchmark extended phase context");
    }
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
        if state.transition_trace.as_mut().is_some_and(TransitionTrace::ready) {
            state.transition_trace.take().unwrap().report();
        }
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
            if let Some(chest) = &mut state.chest {
                world.resource_mut::<terra_player::CameraInputState>().captured = true;
                match chest.prepare(world) {
                    Ok(true) => {
                        // Production picking needs logical capture for placement.
                        // The stationary UI measurement must then ignore OS mouse
                        // motion, just like the ordinary player smoke benchmark.
                        world.resource_mut::<terra_player::CameraInputState>().captured = false;
                    }
                    Ok(false) => return,
                    Err(error) => {
                        fail(world, &mut state, &error);
                        return;
                    }
                }
            }
            if world.resource::<InventoryView>().open {
                fail(
                    world,
                    &mut state,
                    "Inventory was already open before the comparison",
                );
                return;
            }
            if state.stable_since.is_none() {
                let window = world
                    .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
                    .iter(world)
                    .next()
                    .map(|window| (window.physical_width(), window.physical_height()));
                tracing::info!(
                    benchmark_mode = "integrated",
                    benchmark_ui = true,
                    physical_resolution = ?window,
                    graphics = ?world.resource::<terra_core::GraphicsConfig>(),
                    client = ?world.resource::<terra_core::GameConfig>().client,
                    seed = world.resource::<terra_core::GameConfig>().world.seed.0,
                    "Effective benchmark configuration (graphics file loading is explicit and opt-in)"
                );
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
            if !state.transition_reported && state.transition_times.len() < MAX_SAMPLES {
                let elapsed = now.duration_since(state.previous).as_secs_f64() * 1000.0;
                state.transition_times.push(elapsed);
                let previous = state.previous;
                if let Some(trace) = &mut state.transition_trace {
                    trace.observe(previous, now);
                }
            }
            state.previous = now;
            if !state.transition_reported {
                trace_transition_dom(world, &state, now);
            }
            let opening = state.stage == Stage::Opening;
            let ready = open == opening && panel_ready(world) == opening
                && (!state.options.extended || surface_ready(world, &state, opening));
            if !ready {
                state.stable_since = None;
                state.visible_frames = 0;
                return;
            }
            // The resource can change in PostUpdate before TiltUI consumes it.
            // Extended regressions require two frames with the actual painted
            // native subtree, not just the open flag or its next CSS snapshot.
            state.visible_frames = state.visible_frames.saturating_add(1);
            if state.options.extended && state.visible_frames < 2 {
                return;
            }
            if !state.transition_reported {
                if state.options.extended
                    && let Err(error) = interaction_evidence(world, &mut state)
                {
                    fail(world, &mut state, &error);
                    return;
                }
                let summary = frame_summary(&state.transition_times);
                tracing::info!(stage = ?state.stage,
                    request_to_visible_ms = now.duration_since(state.entered).as_secs_f64() * 1000.0,
                    frames = state.transition_times.len(), median_ms = summary[0], p95_ms = summary[1],
                    p99_ms = summary[2], max_ms = summary[3],
                    "Inventory benchmark real UI transition; opening includes settled visible slot layout, not only the open flag");
                state.transition_reported = true;
                if let Some(trace) = &mut state.transition_trace {
                    trace.end = Some(now);
                }
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
        if state.options.extended
            && let Err(error) = interaction_evidence(world, &mut state)
        {
            fail(world, &mut state, &error);
            return;
        }
        report(world, &state, now);
        match state.stage {
            Stage::Closed => {
                // Phase reporting is observer work before the actual request.
                // Keep it out of both first-frame and request-to-visible timing.
                let requested = Instant::now();
                world.write_message(state.chest.as_ref().map_or(InventoryAction::Toggle,
                    |chest| InventoryAction::OpenChest(chest.position())));
                state.stage = Stage::Opening;
                state.entered = requested;
                state.previous = requested;
                state.transition_times.clear();
                state.transition_reported = false;
                state.visible_frames = 0;
                state.transition_trace = TransitionTrace::new(Stage::Opening, requested);
                trace_transition_dom(world, &state, requested);
            }
            Stage::Open => {
                let requested = Instant::now();
                world.write_message(InventoryAction::Toggle);
                state.stage = Stage::Closing;
                state.entered = requested;
                state.previous = requested;
                state.transition_times.clear();
                state.transition_reported = false;
                state.visible_frames = 0;
                state.transition_trace = TransitionTrace::new(Stage::Closing, requested);
                trace_transition_dom(world, &state, requested);
            }
            Stage::ClosedAgain => {
                if state.round < state.options.rounds {
                    state.round += 1;
                    start_window(&mut state, Stage::Closed, Instant::now());
                    return;
                }
                state.stage = Stage::Finished;
                if state.options.extended {
                    tracing::info!(surface = ?state.options.surface, rounds = state.round,
                        "Inventory benchmark completed all requested real surface rounds");
                }
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
    let _scope = terra_core::profiling::ScopeTimer::new("benchmark.inventory_report");
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
    fn closed_inventory_focus_guard_allows_window_focus_but_finds_nested_and_detached_inputs() {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let unrelated = world.spawn_empty().id();
        let backdrop = world
            .spawn(terra_ui::ElementId("inventory-backdrop".into()))
            .id();
        let input = world
            .spawn((
                terra_ui::ElementId("inventory-search".into()),
                ChildOf(backdrop),
            ))
            .id();
        let editable_child = world.spawn(ChildOf(input)).id();
        let detached_craft_input = world.spawn(terra_ui::ElementId("craft-search".into())).id();
        assert!(!focus_within_inventory(&world, None));
        assert!(!focus_within_inventory(&world, Some(window)));
        assert!(!focus_within_inventory(&world, Some(unrelated)));
        assert!(focus_within_inventory(&world, Some(input)));
        assert!(focus_within_inventory(&world, Some(editable_child)));
        assert!(focus_within_inventory(&world, Some(detached_craft_input)));
    }

    #[test]
    fn extended_workloads_are_bounded_and_do_not_change_default_player_comparison() {
        let ordinary = benchmark_options(None, None).unwrap();
        assert_eq!(ordinary.rounds, 1);
        assert_eq!(ordinary.surface, Surface::Player);
        assert!(!ordinary.extended);
        let chest = benchmark_options(Some("2"), Some("chest")).unwrap();
        assert_eq!(chest.rounds, 2);
        assert_eq!(chest.surface, Surface::Chest);
        assert!(chest.extended);
        for rounds in ["0", "3", "255", "256", "", "nan"] {
            assert!(benchmark_options(Some(rounds), None).is_err());
        }
        assert!(benchmark_options(None, Some("flag-only-container")).is_err());
    }

    #[test]
    fn transition_trace_keeps_three_worst_exact_frames_and_waits_for_pipeline_close() {
        let start = Instant::now();
        let mut trace = TransitionTrace {
            stage: Stage::Opening,
            start,
            end: None,
            worst_frames: Vec::new(),
            remaining_frames: 2,
        };
        let mut cursor = start;
        for milliseconds in [8, 2, 101, 19, 40] {
            let end = cursor + Duration::from_millis(milliseconds);
            trace.observe(cursor, end);
            cursor = end;
        }
        assert_eq!(
            trace
                .worst_frames
                .iter()
                .map(|(start, end)| end.duration_since(*start).as_millis())
                .collect::<Vec<_>>(),
            [101, 40, 19]
        );
        assert!(!trace.ready());
        trace.end = Some(cursor);
        assert!(!trace.ready());
        assert!(trace.ready());
        assert_eq!(trace.start, start);
        assert_eq!(trace.end, Some(cursor));
    }

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
