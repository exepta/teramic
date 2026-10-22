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
    let cycles = std::env::var("TERRA_SOAK_EDIT_CYCLES")
        .map_or(Ok(12), |value| value.parse::<u32>())
        .map_err(|_| "TERRA_SOAK_EDIT_CYCLES requires an integer from 1 through 60")?;
    if !(1..=60).contains(&cycles) {
        return Err("TERRA_SOAK_EDIT_CYCLES requires an integer from 1 through 60".into());
    }
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
                Operation::Seal => MaterialId::Stone as u16,
                Operation::RemoveSeal | Operation::BreakGround => 0,
                Operation::RestoreGround => state.original,
            };
            let valid_expected = match operation {
                Operation::Source | Operation::RestoreGround => expected == 0,
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

fn transition(state: &mut Edits, stage: Stage) {
    state.stage = stage;
    state.entered = Instant::now();
}

fn operation_position(ground: [i64; 3], operation: Operation) -> [i64; 3] {
    match operation {
        Operation::Source | Operation::Seal | Operation::RemoveSeal => {
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
        Operation::Source => ground[1] as f32 + 0.99,
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
        Operation::Source | Operation::BreakGround => hit.position == ground,
        Operation::Seal | Operation::RemoveSeal => hit.position == operation_position(ground, operation),
        Operation::RestoreGround => hit.adjacent() == ground,
    })
}

fn choose_ground(world: &mut World) -> Option<([i64; 3], u16)> {
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
            if supported_outlets < 2 {
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
    fn all_operations_remain_in_the_two_fixture_cells_at_negative_coordinates() {
        let ground = [-1, -17, -16];
        for operation in [Operation::Source, Operation::Seal, Operation::RemoveSeal] {
            assert_eq!(operation_position(ground, operation), [-1, -16, -16]);
        }
        for operation in [Operation::BreakGround, Operation::RestoreGround] {
            assert_eq!(operation_position(ground, operation), ground);
        }
    }
}
