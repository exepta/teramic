//! Finite opt-in source/terrain churn through real authoritative edit messages.
use bevy::{app::AppExit, prelude::*};
use std::time::{Duration, Instant};
use terra_network::{client::ClientSession, protocol::session::EditOutcome};
use terra_player::{CameraInputState, TargetWorld, TerraCamera, WorldTarget};
use terra_voxel::{MaterialId, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, WorldGeneration, WorldTerrain},
};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Source,
    PlaceDryStone,
    Seal,
    RemoveSeal,
    BreakGround,
    RestoreGround,
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Select,
    Aim(Operation),
    Confirm {
        operation: Operation,
        previous_request: u64,
        previous_revision: u64,
        material: u16,
    },
    Flow,
    Drain,
    CycleWait,
    Complete,
}

#[derive(Resource)]
pub(crate) struct Edits {
    ready_at: Option<Instant>,
    entered: Instant,
    cycle_started: Instant,
    cycle: u32,
    cycles: u32,
    stage: Stage,
    ground: [i64; 3],
    original: u16,
    baseline_fluid_records: usize,
    last_liquid_check: Instant,
    failed: bool,
    rapid: Option<RapidEdits>,
}

struct RapidEdits {
    material_requested: bool,
    stop_after: Duration,
    submitted: Option<Instant>,
    latencies_ms: Vec<f64>,
    pending_geometry_acks: u32,
    pending_render_acks: u32,
    pending_collision_acks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditMode {
    Water,
    Rapid,
}

fn edit_mode(value: Option<&str>) -> Result<EditMode, String> {
    match value {
        None | Some("water") => Ok(EditMode::Water),
        Some("rapid") => Ok(EditMode::Rapid),
        _ => Err("TERRA_SOAK_EDIT_MODE must be water or rapid".into()),
    }
}

fn edit_cycles(mode: EditMode, value: Option<&str>) -> Result<u32, String> {
    let (default, maximum) = match mode {
        EditMode::Water => (12, 60),
        EditMode::Rapid => (120, 1_000),
    };
    let cycles = value.map_or(Ok(default), str::parse::<u32>);
    cycles
        .ok()
        .filter(|cycles| (1..=maximum).contains(cycles))
        .ok_or_else(|| {
            format!("TERRA_SOAK_EDIT_CYCLES requires an integer from 1 through {maximum}")
        })
}

fn rapid_stop_after(value: Option<&str>) -> Duration {
    let seconds = value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite())
        .unwrap_or(3.0)
        .clamp(3.0, 1_800.0);
    // Finish the final acknowledged removal before the benchmark exits.
    Duration::from_secs_f64(seconds - 1.0)
}

impl Edits {
    pub(crate) fn started(&self) -> bool {
        self.ready_at.is_some()
    }
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    if !std::env::var("TERRA_SOAK_EDITS").is_ok_and(|value| value == "1") {
        return Ok(());
    }
    if !std::env::args().any(|arg| arg == "--smoke-test-benchmark")
        || !std::env::var("TERRA_BENCH_MODE").is_ok_and(|mode| mode == "integrated")
    {
        return Err(
            "TERRA_SOAK_EDITS=1 requires --smoke-test-benchmark and TERRA_BENCH_MODE=integrated"
                .into(),
        );
    }
    if std::env::var_os("TERRA_SOAK_ROUTE").is_some() {
        return Err("Run TERRA_SOAK_EDITS separately from the moving exploration route".into());
    }
    let mode = edit_mode(std::env::var("TERRA_SOAK_EDIT_MODE").ok().as_deref())?;
    let cycles = edit_cycles(
        mode,
        std::env::var("TERRA_SOAK_EDIT_CYCLES").ok().as_deref(),
    )?;
    let rapid = (mode == EditMode::Rapid).then(|| RapidEdits {
        material_requested: false,
        stop_after: rapid_stop_after(std::env::var("TERRA_BENCH_SECONDS").ok().as_deref()),
        submitted: None,
        latencies_ms: Vec::new(),
        pending_geometry_acks: 0,
        pending_render_acks: 0,
        pending_collision_acks: 0,
    });
    let now = Instant::now();
    app.insert_resource(Edits {
        ready_at: None,
        entered: now,
        cycle_started: now,
        cycle: 0,
        cycles,
        stage: Stage::Select,
        ground: [0; 3],
        original: 0,
        baseline_fluid_records: 0,
        last_liquid_check: now,
        failed: false,
        rapid,
    })
    .add_systems(PostUpdate, run);
    Ok(())
}

fn run(world: &mut World) {
    if crate::smoke_test::benchmark_ready_time(world).is_none() {
        return;
    }
    world.resource_scope(|world, mut state: Mut<Edits>| {
        if state.failed || matches!(state.stage, Stage::Complete) {
            return;
        }
        if let Err(error) = advance(world, &mut state) {
            state.failed = true;
            tracing::error!(%error, stage=?state.stage, cycle=state.cycle, "Authoritative soak edits failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, state: &mut Edits) -> Result<(), String> {
    let session = world
        .get_resource::<ClientSession>()
        .ok_or("Soak edits require the real network client")?;
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready || session.teleporting {
        return Ok(());
    }
    if session.game_mode != terra_core::GameMode::Creative || !session.allow_build {
        return Err("Soak edits require authoritative Creative building permission".into());
    }
    if !world.resource::<WorldTerrain>().0.config().water {
        return Err("Soak edits require the normal water simulation to be enabled".into());
    }
    if world.resource::<terra_player::PlayerInputBlock>().0 {
        return Err("Gameplay input is blocked during the edit benchmark".into());
    }
    world.resource_mut::<CameraInputState>().captured = true;
    *world.resource_mut::<terra_player::ControllerMode>() = terra_player::ControllerMode::Fly;
    let now = Instant::now();
    if state.ready_at.is_none() {
        state.ready_at = Some(now);
        state.entered = now;
        state.cycle_started = now;
    }
    let elapsed = state.entered.elapsed();
    if elapsed > Duration::from_secs(40) {
        return Err(format!(
            "Soak edit stage {:?} exceeded 40 seconds",
            state.stage
        ));
    }
    if state.rapid.is_some() {
        return advance_rapid(world, state, elapsed);
    }
    match state.stage {
        Stage::Select => {
            let Some((ground, original)) = choose_ground(world) else {
                if elapsed > Duration::from_secs(8) {
                    return Err("No reachable dry full-block ground beside the actual player; choose a low dry surface benchmark camera".into());
                }
                return Ok(());
            };
            state.ground = ground;
            state.original = original;
            state.baseline_fluid_records = world
                .resource::<EditJournal>()
                .fluid_work_counts()
                .recorded_cells;
            tracing::info!(
                ?ground,
                original,
                cycles = state.cycles,
                "Authoritative soak edit fixture selected two real nearby cells"
            );
            transition(state, Stage::Aim(Operation::Source));
        }
        Stage::Aim(operation) => {
            aim(world, state.ground, operation)?;
            if elapsed < Duration::from_millis(300) {
                return Ok(());
            }
            if !target_matches(world, state.ground, operation) {
                if elapsed > Duration::from_secs(5) {
                    return Err(format!(
                        "Production picking cannot reach {operation:?}: {:?}",
                        world.resource::<TargetWorld>().0
                    ));
                }
                return Ok(());
            }
            let position = operation_position(state.ground, operation);
            let expected = world
                .resource::<ChunkStore>()
                .block_at(position)
                .ok_or("Edit target unloaded before submission")?;
            let material = match operation {
                Operation::Source => MaterialId::FluidWater as u16,
                Operation::Seal | Operation::PlaceDryStone => MaterialId::Stone as u16,
                Operation::RemoveSeal | Operation::BreakGround => 0,
                Operation::RestoreGround => state.original,
            };
            let valid_expected = match operation {
                Operation::Source | Operation::RestoreGround => expected == 0,
                Operation::PlaceDryStone => expected == 0,
                Operation::Seal => expected == MaterialId::FluidWater as u16,
                Operation::RemoveSeal => expected == MaterialId::Stone as u16,
                Operation::BreakGround => {
                    MaterialId::from_raw(expected).is_some_and(MaterialId::is_opaque)
                }
            };
            if !valid_expected {
                return Err(format!(
                    "Unexpected material {expected} before {operation:?}"
                ));
            }
            let previous_request = world
                .resource::<ClientSession>()
                .last_edit
                .map_or(0, |(request, _)| request);
            let previous_revision = world
                .resource::<EditJournal>()
                .revision(global_to_section(position).0);
            world.write_message(BlockEditRequest {
                position,
                expected_material: expected,
                material,
            });
            transition(
                state,
                Stage::Confirm {
                    operation,
                    previous_request,
                    previous_revision,
                    material,
                },
            );
        }
        Stage::Confirm {
            operation,
            previous_request,
            previous_revision,
            material,
        } => {
            aim(world, state.ground, operation)?;
            let session = world.resource::<ClientSession>();
            let Some((request, outcome)) = session.last_edit else {
                return Ok(());
            };
            if request <= previous_request {
                return Ok(());
            }
            if outcome != EditOutcome::Applied {
                return Err(format!("Server rejected {operation:?}: {outcome:?}"));
            }
            let position = operation_position(state.ground, operation);
            let journal = world.resource::<EditJournal>();
            if world.resource::<ChunkStore>().block_at(position) != Some(material)
                || journal.revision(global_to_section(position).0) <= previous_revision
            {
                return Ok(());
            }
            if matches!(operation, Operation::Source)
                && !journal
                    .fluid(position)
                    .is_some_and(|cell| cell.source && cell.amount == 8)
            {
                return Err("Applied source block has no authoritative full source cell".into());
            }
            log_state(world, state, operation, request);
            let next = match operation {
                Operation::Source => Stage::Flow,
                Operation::PlaceDryStone => unreachable!("rapid edits have their own driver"),
                Operation::Seal => Stage::Drain,
                Operation::RemoveSeal => Stage::Aim(Operation::BreakGround),
                Operation::BreakGround => Stage::Aim(Operation::RestoreGround),
                Operation::RestoreGround => Stage::CycleWait,
            };
            transition(state, next);
        }
        Stage::Flow if elapsed >= Duration::from_secs(2) => {
            if nearby_fluid_cells(world.resource::<EditJournal>(), state.ground) < 2 {
                return Err("Placed source did not produce real flowing neighboring cells".into());
            }
            transition(state, Stage::Aim(Operation::Seal));
        }
        Stage::Drain
            if elapsed >= Duration::from_millis(500)
                && state.last_liquid_check.elapsed() >= Duration::from_millis(250) =>
        {
            state.last_liquid_check = now;
            let journal = world.resource::<EditJournal>();
            if nearby_fluid_cells(journal, state.ground) == 0
                && journal.fluid_work_counts().recorded_cells <= state.baseline_fluid_records
            {
                transition(state, Stage::Aim(Operation::RemoveSeal));
            }
        }
        Stage::CycleWait if state.cycle_started.elapsed() >= Duration::from_secs(10) => {
            let source = operation_position(state.ground, Operation::Source);
            if world.resource::<ChunkStore>().block_at(state.ground) != Some(state.original)
                || world.resource::<ChunkStore>().block_at(source) != Some(0)
                || nearby_fluid_cells(world.resource::<EditJournal>(), state.ground) != 0
            {
                return Err(
                    "Edit cycle did not restore its ground, source cell and local liquid".into(),
                );
            }
            state.cycle += 1;
            tracing::info!(
                cycle = state.cycle,
                elapsed_ready_s = state
                    .ready_at
                    .map_or(0.0, |started| started.elapsed().as_secs_f64()),
                "Authoritative source/terrain cycle restored its edited cells"
            );
            if state.cycle >= state.cycles {
                transition(state, Stage::Complete);
                tracing::info!(
                    cycles = state.cycle,
                    "Finite authoritative soak edits complete; benchmark continues idle for lifecycle observation"
                );
            } else {
                state.cycle_started = now;
                transition(state, Stage::Aim(Operation::Source));
            }
        }
        _ => {}
    }
    Ok(())
}

fn advance_rapid(world: &mut World, state: &mut Edits, elapsed: Duration) -> Result<(), String> {
    match state.stage {
        Stage::Select => {
            if !prepare_rapid_material(world, &mut state.rapid.as_mut().unwrap().material_requested)
            {
                return Ok(());
            }
            let Some((ground, original)) = choose_ground_for(world, false) else {
                if elapsed > Duration::from_secs(8) {
                    return Err(
                        "No reachable dry ground beside the real player for rapid edits".into(),
                    );
                }
                return Ok(());
            };
            state.ground = ground;
            state.original = original;
            tracing::info!(
                ?ground,
                original,
                requested_cycles = state.cycles,
                stop_after_s = state.rapid.as_ref().unwrap().stop_after.as_secs_f64(),
                "Rapid authoritative edits selected a real empty cell above dry ground"
            );
            transition(state, Stage::Aim(Operation::PlaceDryStone));
        }
        Stage::Aim(operation @ (Operation::PlaceDryStone | Operation::RemoveSeal)) => {
            if matches!(operation, Operation::PlaceDryStone)
                && state.ready_at.unwrap().elapsed() >= state.rapid.as_ref().unwrap().stop_after
            {
                return finish_rapid(world, state);
            }
            aim(world, state.ground, operation)?;
            let rapid = state.rapid.as_ref().unwrap();
            // Let production picking observe the new aim. Stay below the real
            // server's ten-edits-per-second rate even when acknowledgements are fast.
            if elapsed < Duration::from_millis(20)
                || rapid
                    .submitted
                    .is_some_and(|submitted| submitted.elapsed() < Duration::from_millis(105))
            {
                return Ok(());
            }
            if !target_matches(world, state.ground, operation) {
                if elapsed > Duration::from_secs(5) {
                    return Err(format!(
                        "Production picking cannot reach rapid {operation:?}: {:?}",
                        world.resource::<TargetWorld>().0
                    ));
                }
                return Ok(());
            }
            let position = operation_position(state.ground, operation);
            let (expected, material) = match operation {
                Operation::PlaceDryStone => (0, MaterialId::Stone as u16),
                Operation::RemoveSeal => (MaterialId::Stone as u16, 0),
                _ => unreachable!(),
            };
            if world.resource::<ChunkStore>().block_at(position) != Some(expected) {
                return Err(format!(
                    "Rapid {operation:?} target changed before submission"
                ));
            }
            let previous_request = world
                .resource::<ClientSession>()
                .last_edit
                .map_or(0, |(request, _)| request);
            let previous_revision = world
                .resource::<EditJournal>()
                .revision(global_to_section(position).0);
            world.write_message(BlockEditRequest {
                position,
                expected_material: expected,
                material,
            });
            state.rapid.as_mut().unwrap().submitted = Some(Instant::now());
            transition(
                state,
                Stage::Confirm {
                    operation,
                    previous_request,
                    previous_revision,
                    material,
                },
            );
        }
        Stage::Confirm {
            operation,
            previous_request,
            previous_revision,
            material,
        } => {
            aim(world, state.ground, operation)?;
            let Some((request, outcome)) = world.resource::<ClientSession>().last_edit else {
                return Ok(());
            };
            if request <= previous_request {
                return Ok(());
            }
            if outcome != EditOutcome::Applied {
                return Err(format!("Server rejected rapid {operation:?}: {outcome:?}"));
            }
            let position = operation_position(state.ground, operation);
            let owner = global_to_section(position).0;
            let journal = world.resource::<EditJournal>();
            let chunks = world.resource::<ChunkStore>();
            if chunks.block_at(position) != Some(material)
                || journal.revision(owner) <= previous_revision
            {
                return Ok(());
            }
            let epoch = journal.mesh_epoch(owner);
            let geometry_ready = chunks
                .loaded
                .get(&owner)
                .is_some_and(|section| section.mesh_epoch == epoch);
            let render_ready = world
                .get_resource::<terra_render::RenderedSections>()
                .is_some_and(|rendered| {
                    rendered
                        .sections
                        .get(&owner)
                        .is_some_and(|section| section.mesh_epoch == epoch)
                });
            let minimum = Vec3::from_array(position.map(|v| v as f32)) + Vec3::splat(0.1);
            let collision_ready = world
                .get_resource::<terra_physics::TerrainCollisionState>()
                .is_some_and(|collision| {
                    collision.ready_for_aabb(minimum, minimum + Vec3::splat(0.8), chunks, journal)
                });
            let rapid = state.rapid.as_mut().unwrap();
            let latency_ms = rapid.submitted.unwrap().elapsed().as_secs_f64() * 1_000.0;
            rapid.latencies_ms.push(latency_ms);
            rapid.pending_geometry_acks += u32::from(!geometry_ready);
            rapid.pending_render_acks += u32::from(!render_ready);
            rapid.pending_collision_acks += u32::from(!collision_ready);
            if matches!(operation, Operation::PlaceDryStone) {
                transition(state, Stage::Aim(Operation::RemoveSeal));
            } else {
                state.cycle += 1;
                if state.cycle == 1 || state.cycle.is_multiple_of(10) {
                    tracing::info!(
                        cycle = state.cycle,
                        request,
                        latency_ms,
                        geometry_ready,
                        render_ready,
                        collision_ready,
                        "Rapid authoritative edit cycle acknowledged and replicated"
                    );
                }
                if state.cycle >= state.cycles {
                    finish_rapid(world, state)?;
                } else {
                    transition(state, Stage::Aim(Operation::PlaceDryStone));
                }
            }
        }
        Stage::Complete => {}
        _ => return Err("Unexpected stage in the rapid edit fixture".into()),
    }
    Ok(())
}

fn prepare_rapid_material(world: &mut World, requested: &mut bool) -> bool {
    use terra_network::{
        client::{InventoryRequest, SelectedInventorySlot},
        protocol::inventory::{InventoryAction, InventoryCommand},
    };
    let session = world.resource::<ClientSession>();
    let stone = terra_core::ItemKind::Block(MaterialId::Stone as u16);
    if session
        .inventory
        .slot(0)
        .is_some_and(|stack| stack.valid() && stack.item == stone)
    {
        // The normal client attaches its selected source slot to block edits.
        // Use a real server-granted stack and production hotbar selection so
        // the empty startup slot cannot masquerade as the held stone item.
        world.resource_mut::<SelectedInventorySlot>().0 = Some(0);
        if world.contains_resource::<Messages<terra_ui::InventoryAction>>() {
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
        }
        return true;
    }
    if !*requested {
        let revision = session.inventory.revision;
        world.write_message(InventoryRequest(InventoryCommand {
            revision,
            action: InventoryAction::CreativeSet {
                slot: 0,
                item: stone,
                count: 1,
            },
        }));
        *requested = true;
    }
    false
}

fn rapid_ground_allowed(before: u16, after: u16) -> bool {
    // Repeatedly covering grass with stone can legitimately turn it into dirt.
    // Restoring the edited source cell must not fake reversal of natural growth.
    before == after || before == MaterialId::Grass as u16 && after == MaterialId::Dirt as u16
}

fn finish_rapid(world: &World, state: &mut Edits) -> Result<(), String> {
    let position = operation_position(state.ground, Operation::PlaceDryStone);
    let chunks = world.resource::<ChunkStore>();
    let ground_after = chunks
        .block_at(state.ground)
        .ok_or("Rapid edit ground unloaded")?;
    if !rapid_ground_allowed(state.original, ground_after) || chunks.block_at(position) != Some(0) {
        return Err(format!(
            "Rapid source cell was not restored or unrelated ground changed: before={}, after={ground_after}, source={:?}",
            state.original,
            chunks.block_at(position)
        ));
    }
    let epoch = world
        .resource::<EditJournal>()
        .mesh_epoch(global_to_section(position).0);
    let final_geometry_ready = chunks
        .loaded
        .get(&global_to_section(position).0)
        .is_some_and(|section| section.mesh_epoch == epoch);
    let final_render_ready = world
        .get_resource::<terra_render::RenderedSections>()
        .is_some_and(|rendered| {
            rendered
                .sections
                .get(&global_to_section(position).0)
                .is_some_and(|section| section.mesh_epoch == epoch)
        });
    let minimum = Vec3::from_array(position.map(|v| v as f32)) + Vec3::splat(0.1);
    let final_collision_ready = world
        .get_resource::<terra_physics::TerrainCollisionState>()
        .is_some_and(|collision| {
            collision.ready_for_aabb(
                minimum,
                minimum + Vec3::splat(0.8),
                chunks,
                world.resource::<EditJournal>(),
            )
        });
    let rapid = state.rapid.as_mut().unwrap();
    let count = rapid.latencies_ms.len();
    if state.cycle == 0 || count != state.cycle as usize * 2 {
        return Err(
            "Rapid fixture did not demonstrate a complete acknowledged placement/removal pair"
                .into(),
        );
    }
    rapid.latencies_ms.sort_by(f64::total_cmp);
    let mean = rapid.latencies_ms.iter().sum::<f64>() / count.max(1) as f64;
    let p95 = rapid
        .latencies_ms
        .get(count.saturating_sub(1) * 95 / 100)
        .copied()
        .unwrap_or(0.0);
    tracing::info!(
        cycles = state.cycle,
        acknowledged_edits = count,
        elapsed_ready_s = state.ready_at.unwrap().elapsed().as_secs_f64(),
        mean_ack_replication_ms = mean,
        p95_ack_replication_ms = p95,
        max_ack_replication_ms = rapid.latencies_ms.last().copied().unwrap_or(0.0),
        pending_geometry_acks = rapid.pending_geometry_acks,
        pending_render_acks = rapid.pending_render_acks,
        pending_collision_acks = rapid.pending_collision_acks,
        restoration_scope = "source_cell",
        source_restored = true,
        ground_before = state.original,
        ground_after,
        ground_unchanged = ground_after == state.original,
        natural_grass_conversion =
            state.original == MaterialId::Grass as u16 && ground_after == MaterialId::Dirt as u16,
        final_geometry_ready,
        final_render_ready,
        final_collision_ready,
        restored = true,
        "Rapid authoritative edits complete; benchmark continues idle for lifecycle observation"
    );
    transition(state, Stage::Complete);
    Ok(())
}

fn transition(state: &mut Edits, stage: Stage) {
    state.stage = stage;
    state.entered = Instant::now();
}

fn operation_position(ground: [i64; 3], operation: Operation) -> [i64; 3] {
    match operation {
        Operation::Source | Operation::PlaceDryStone | Operation::Seal | Operation::RemoveSeal => {
            [ground[0], ground[1] + 1, ground[2]]
        }
        Operation::BreakGround | Operation::RestoreGround => ground,
    }
}

fn eye(world: &mut World) -> Option<Vec3> {
    world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|camera| camera.translation)
}

fn aim(world: &mut World, ground: [i64; 3], operation: Operation) -> Result<(), String> {
    let eye = eye(world).ok_or("No actual player camera for soak edits")?;
    let y = match operation {
        Operation::Source | Operation::PlaceDryStone => ground[1] as f32 + 0.99,
        Operation::Seal | Operation::RemoveSeal => ground[1] as f32 + 1.5,
        Operation::BreakGround => ground[1] as f32 + 0.99,
        Operation::RestoreGround => ground[1] as f32 + 0.01,
    };
    let target = Vec3::new(ground[0] as f32 + 0.5, y, ground[2] as f32 + 0.5);
    let rotation = Transform::from_translation(eye)
        .looking_at(target, Vec3::Y)
        .rotation;
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        camera.rotation = rotation;
    }
    Ok(())
}

fn target_matches(world: &World, ground: [i64; 3], operation: Operation) -> bool {
    matches!(world.resource::<TargetWorld>().0, Some(WorldTarget::Block(hit)) if match operation {
        Operation::PlaceDryStone => hit.position == ground && hit.adjacent() == operation_position(ground, operation),
        Operation::Source | Operation::BreakGround => hit.position == ground,
        Operation::Seal | Operation::RemoveSeal => hit.position == operation_position(ground, operation),
        Operation::RestoreGround => hit.adjacent() == ground,
    })
}

fn choose_ground(world: &mut World) -> Option<([i64; 3], u16)> {
    choose_ground_for(world, true)
}

fn choose_ground_for(world: &mut World, water: bool) -> Option<([i64; 3], u16)> {
    let confirmed = world.resource::<ClientSession>().confirmed_state()?;
    let body = Vec3::from_array(confirmed.position);
    let eye = eye(world)?;
    if eye.distance(body + Vec3::Y * terra_player::PLAYER_EYE_OFFSET) > 0.5 {
        return None;
    }
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let terrain = &world.resource::<WorldTerrain>().0;
    let properties = world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    let pose = terra_world::props::PropPose::WeatherSwept {
        frame: terra_world::props::weather_wind_frame(
            world.resource::<terra_core::WorldWeather>().wind(),
        ),
    };
    let base = body.floor().as_i64vec3().to_array();
    for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
        for dy in -3..=0 {
            let ground = [base[0] + dx, base[1] + dy, base[2] + dz];
            let source = [ground[0], ground[1] + 1, ground[2]];
            let material = chunks.block_at(ground)?;
            if !MaterialId::from_raw(material).is_some_and(MaterialId::is_opaque)
                || !properties.shape(material).is_empty()
                || chunks.block_at(source) != Some(0)
                || !chunks
                    .block_at([ground[0], ground[1] - 1, ground[2]])
                    .and_then(MaterialId::from_raw)
                    .is_some_and(MaterialId::is_opaque)
                || chunks.fluid_cell_at(terrain, journal, source, 0).amount != 0
                || nearby_fluid_cells(journal, ground) != 0
            {
                continue;
            }
            let supported_outlets = [[-1, 0], [1, 0], [0, -1], [0, 1]]
                .into_iter()
                .filter(|[x, z]| {
                    chunks.block_at([source[0] + x, source[1], source[2] + z]) == Some(0)
                        && chunks
                            .block_at([source[0] + x, ground[1], source[2] + z])
                            .and_then(MaterialId::from_raw)
                            .is_some_and(MaterialId::is_opaque)
                })
                .count();
            if water && supported_outlets < 2 {
                continue;
            }
            let top = Vec3::new(
                ground[0] as f32 + 0.5,
                ground[1] as f32 + 0.99,
                ground[2] as f32 + 0.5,
            );
            if top.distance(eye) > 4.0 {
                continue;
            }
            let (_, target) = terra_world::targeting::pick_world_with_context(
                chunks,
                journal,
                properties,
                eye.to_array().map(f64::from),
                (top - eye).normalize_or_zero().to_array().map(f64::from),
                pose,
            );
            if matches!(target, Some(WorldTarget::Block(hit)) if hit.position == ground && hit.adjacent() == source)
            {
                return Some((ground, material));
            }
        }
    }
    None
}

fn nearby_fluid_cells(journal: &EditJournal, ground: [i64; 3]) -> usize {
    journal
        .fluid_cells()
        .filter(|(position, _)| {
            (position[0] - ground[0]).abs() <= 12 && (position[2] - ground[2]).abs() <= 12
        })
        .count()
}

fn log_state(world: &World, state: &Edits, operation: Operation, request: u64) {
    let journal = world.resource::<EditJournal>();
    let counts = journal.fluid_work_counts();
    tracing::info!(
        cycle = state.cycle,
        ?operation,
        request,
        revision = journal.revision(global_to_section(state.ground).0),
        active_fluid = counts.active,
        sleeping_fluid = counts.sleeping,
        recorded_fluid = counts.recorded_cells,
        source_count = counts.sources,
        sections = world.resource::<ChunkStore>().loaded.len(),
        generation_jobs = world.resource::<WorldGeneration>().pending_jobs,
        mesh_assets = world.resource::<Assets<Mesh>>().len(),
        standard_material_assets = world.resource::<Assets<StandardMaterial>>().len(),
        "Authoritative soak edit acknowledged and replicated"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_ground_accepts_only_unchanged_blocks_or_natural_grass_to_dirt_decay() {
        let grass = MaterialId::Grass as u16;
        let dirt = MaterialId::Dirt as u16;
        let stone = MaterialId::Stone as u16;
        assert!(rapid_ground_allowed(grass, grass));
        assert!(rapid_ground_allowed(stone, stone));
        assert!(rapid_ground_allowed(grass, dirt));
        for (before, after) in [(dirt, grass), (grass, stone), (stone, dirt), (stone, 0)] {
            assert!(!rapid_ground_allowed(before, after));
        }
    }

    #[test]
    fn rapid_edits_wait_for_a_server_granted_source_stack_before_hotbar_selection() {
        use terra_network::{
            client::{InventoryRequest, SelectedInventorySlot},
            protocol::inventory::InventoryAction,
        };
        let mut world = World::new();
        world.init_resource::<ClientSession>();
        world.init_resource::<SelectedInventorySlot>();
        world.init_resource::<Messages<InventoryRequest>>();
        world.init_resource::<Messages<terra_ui::InventoryAction>>();
        let mut requested = false;
        assert!(!prepare_rapid_material(&mut world, &mut requested));
        assert!(!prepare_rapid_material(&mut world, &mut requested));
        let requests = world
            .resource_mut::<Messages<InventoryRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        let stone = terra_core::ItemKind::Block(MaterialId::Stone as u16);
        assert_eq!(
            requests[0].0.action,
            InventoryAction::CreativeSet {
                slot: 0,
                item: stone,
                count: 1
            }
        );
        assert!(
            world
                .resource::<Messages<terra_ui::InventoryAction>>()
                .is_empty()
        );
        // A fresh client has no slots until the first authoritative inventory.
        world.resource_mut::<ClientSession>().inventory.slots =
            vec![terra_core::ItemStack::EMPTY; terra_core::INVENTORY_SLOTS];
        world.resource_mut::<ClientSession>().inventory.slots[0] = terra_core::ItemStack {
            item: stone,
            count: 1,
        };
        assert!(prepare_rapid_material(&mut world, &mut requested));
        assert_eq!(world.resource::<SelectedInventorySlot>().0, Some(0));
        assert!(matches!(
            world
                .resource_mut::<Messages<terra_ui::InventoryAction>>()
                .drain()
                .next(),
            Some(terra_ui::InventoryAction::Hotbar(0))
        ));
    }

    #[test]
    fn rapid_edit_mode_is_explicit_and_keeps_water_defaults_and_limits() {
        assert_eq!(edit_mode(None).unwrap(), EditMode::Water);
        assert_eq!(edit_mode(Some("water")).unwrap(), EditMode::Water);
        assert_eq!(edit_mode(Some("rapid")).unwrap(), EditMode::Rapid);
        assert!(edit_mode(Some("camera-only")).is_err());
        assert_eq!(edit_cycles(EditMode::Water, None).unwrap(), 12);
        assert_eq!(edit_cycles(EditMode::Rapid, None).unwrap(), 120);
        assert_eq!(edit_cycles(EditMode::Rapid, Some("1000")).unwrap(), 1000);
        for value in ["0", "1001", "-1", "NaN"] {
            assert!(edit_cycles(EditMode::Rapid, Some(value)).is_err());
        }
        assert!(edit_cycles(EditMode::Water, Some("61")).is_err());
    }

    #[test]
    fn rapid_edits_reserve_a_cleanup_interval_before_the_benchmark_ends() {
        assert_eq!(rapid_stop_after(Some("30")), Duration::from_secs(29));
        assert_eq!(rapid_stop_after(Some("3")), Duration::from_secs(2));
        assert_eq!(rapid_stop_after(Some("0")), Duration::from_secs(2));
        assert_eq!(rapid_stop_after(Some("inf")), Duration::from_secs(2));
    }

    #[test]
    fn all_operations_remain_in_the_two_fixture_cells_at_negative_coordinates() {
        let ground = [-1, -17, -16];
        for operation in [
            Operation::Source,
            Operation::PlaceDryStone,
            Operation::Seal,
            Operation::RemoveSeal,
        ] {
            assert_eq!(operation_position(ground, operation), [-1, -16, -16]);
        }
        for operation in [Operation::BreakGround, Operation::RestoreGround] {
            assert_eq!(operation_position(ground, operation), ground);
        }
    }
}
