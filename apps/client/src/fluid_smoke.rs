//! Opt-in native source/outlet fixture. All solids use the normal edit path;
//! the installed authoritative tick and asynchronous water jobs do the work.
use bevy::{app::AppExit, prelude::*};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, PlayerBody, TerraCamera};
use terra_render::{RenderStatus, WaterStatus};
use terra_voxel::{SectionPos, global_to_section};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkFocus, ChunkStore, WorldTerrain, commit_block_edit},
    water::{FLUID_CAPACITY, fluid_cell_at},
};

const SOURCE_COUNT: usize = 3 * 5;

#[derive(Resource)]
struct Fixture {
    started: Instant,
    since: Instant,
    stage: u8,
    floor: i64,
    height: i64,
    view: Transform,
    terrain_epochs: HashMap<SectionPos, u64>,
    volume_after_close: u64,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-fluid") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Fixture {
            started: now,
            since: now,
            stage: 0,
            floor: 64,
            height: std::env::var("TERRA_FLUID_HEIGHT")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v| (2..=32).contains(v))
                .unwrap_or(2),
            view: Transform::default(),
            terrain_epochs: HashMap::new(),
            volume_after_close: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn setup(
    terrain: Res<WorldTerrain>,
    mut state: ResMut<Fixture>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
) {
    let mut floor = 64;
    for x in 12..=22 {
        for z in 4..=10 {
            floor = floor.max(terrain.0.surface_height(x as f64, z as f64) as i64 + 24);
        }
    }
    state.floor = floor;
    state.view = Transform::from_xyz(52.0, floor as f32 + state.height as f32 * 0.9 + 10.0, 42.0)
        .looking_at(
            Vec3::new(17.0, floor as f32 + state.height as f32 * 0.5, 7.0),
            Vec3::Y,
        );
    focus.0 = [17.0, floor as f64 + 3.0, 7.0];
    for mut camera in &mut cameras {
        *camera = state.view;
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.stage == 255 {
            return;
        }
        match advance(world, &mut state) {
            Ok(true) => {
                tracing::info!("Native fluid fixture completed");
                world.write_message(AppExit::Success);
                state.stage = 255;
            }
            Err(error) => {
                tracing::error!(%error, stage=state.stage, "Native fluid fixture failed");
                world.write_message(AppExit::error());
                state.stage = 255;
            }
            Ok(false) => {}
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    if state.started.elapsed() > Duration::from_secs(90) {
        return Err("Fluid fixture exceeded 90 seconds".into());
    }
    if !world.resource::<WorldTerrain>().0.config().water {
        return Err("Water must be enabled for fluid fixture".into());
    }
    if let Some(error) = &world.resource::<RenderStatus>().error {
        return Err(error.clone());
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = state.view;
    }
    for mut body in world
        .query_filtered::<&mut Transform, With<PlayerBody>>()
        .iter_mut(world)
    {
        body.translation = state.view.translation;
    }
    world.resource_mut::<ChunkFocus>().0 = [17.0, state.floor as f64 + 3.0, 7.0];
    match state.stage {
        0 => {
            if !basin_loaded(world, state.floor, state.height)
                || !world.resource::<RenderStatus>().ready
            {
                return Ok(false);
            }
            build_basin(world, state.floor, state.height)?;
            state.since = Instant::now();
            state.stage = 1;
        }
        1 => {
            if state.since.elapsed() < Duration::from_secs(3) || !basin_rendered(world, state.floor)
            {
                return Ok(false);
            }
            check_sources(world)?;
            if !(13..=15).all(|x| {
                (5..=9).all(|z| {
                    (1..=state.height).all(|y| {
                        world
                            .resource::<EditJournal>()
                            .fluid([x, state.floor + y, z])
                            .is_some_and(|cell| cell.amount == FLUID_CAPACITY)
                    })
                })
            }) {
                return Ok(false);
            }
            if right_volume(world.resource::<EditJournal>(), state.floor) != 0 {
                return Err("Water crossed a closed stone barrier".into());
            }
            capture(world, "fluid-closed")?;
            state.since = Instant::now();
            state.stage = 2;
        }
        2 if state.since.elapsed() > Duration::from_millis(700) => {
            let material = if std::env::var_os("TERRA_FLUID_PARTIAL").is_some() {
                82
            } else {
                0
            };
            edit(world, [16, state.floor + 1, 7], material)?;
            state.terrain_epochs = epochs(world, state.floor);
            state.since = Instant::now();
            state.stage = 3;
        }
        3 => {
            check_sources(world)?;
            if epochs(world, state.floor) != state.terrain_epochs {
                return Err("Fluid steps invalidated terrain meshing".into());
            }
            let right = right_volume(world.resource::<EditJournal>(), state.floor);
            if right < 24
                || state.since.elapsed() < Duration::from_secs(4)
                || !basin_rendered(world, state.floor)
            {
                return Ok(false);
            }
            let chunks = world.resource::<ChunkStore>();
            let journal = world.resource::<EditJournal>();
            let terrain = world.resource::<WorldTerrain>();
            let probe = journal
                .fluid_cells()
                .find(|(p, c)| p[0] >= 17 && p[1] == state.floor + 1 && c.amount > 0)
                .map(|(p, _)| p)
                .ok_or("No water crossed section x=16")?;
            if chunks.water_column_at(&terrain.0, journal, probe).is_none() {
                return Err("Swimming query cannot see simulated water".into());
            }
            if std::env::var_os("TERRA_FLUID_PARTIAL").is_some() {
                let wet = chunks.water_column_at_point(
                    &terrain.0,
                    journal,
                    [16.5, state.floor as f64 + 1.6, 7.5],
                );
                let solid = chunks.water_column_at_point(
                    &terrain.0,
                    journal,
                    [16.5, state.floor as f64 + 1.25, 7.5],
                );
                if wet.is_none_or(|water| (water.bed - (state.floor as f64 + 1.5)).abs() > 1e-6)
                    || solid.is_some()
                {
                    return Err(format!(
                        "Partial-block occupancy disagrees with collision: wet={wet:?}, solid={solid:?}"
                    ));
                }
                tracing::info!(
                    ?wet,
                    "Water crossed the real slab at section seam; solid half is dry and free half is swimmable"
                );
            }
            capture(world, "fluid-flowing")?;
            tracing::info!(
                right_units = right,
                "Source-fed water crossed the mined section-boundary gate"
            );
            state.since = Instant::now();
            state.stage = 4;
        }
        4 if state.since.elapsed() > Duration::from_millis(700) => {
            edit(world, [16, state.floor + 1, 7], 3)?;
            state.volume_after_close = right_volume(world.resource::<EditJournal>(), state.floor);
            state.terrain_epochs = epochs(world, state.floor);
            state.since = Instant::now();
            state.stage = 5;
        }
        5 => {
            check_sources(world)?;
            if epochs(world, state.floor) != state.terrain_epochs {
                return Err("Closed-basin fluid steps invalidated terrain".into());
            }
            let gate = [16, state.floor + 1, 7];
            let chunks = world.resource::<ChunkStore>();
            let journal = world.resource::<EditJournal>();
            let terrain = world.resource::<WorldTerrain>();
            if chunks.block_at(gate) != Some(3)
                || fluid_cell_at(&terrain.0, journal, gate, 3).amount != 0
                || chunks.water_column_at(&terrain.0, journal, gate).is_some()
            {
                return Err("Placed barrier still contains renderable/swimmable water".into());
            }
            let remaining = right_volume(journal, state.floor);
            if remaining > state.volume_after_close {
                return Err("Water crossed the restored barrier".into());
            }
            if state.since.elapsed() < Duration::from_secs(3)
                || remaining > 0
                || !basin_rendered(world, state.floor)
            {
                return Ok(false);
            }
            capture(world, "fluid-restored")?;
            state.since = Instant::now();
            state.stage = 6;
        }
        6 if state.since.elapsed() > Duration::from_secs(1) => return Ok(true),
        _ => {}
    }
    Ok(false)
}

fn basin_loaded(world: &World, floor: i64, height: i64) -> bool {
    let chunks = world.resource::<ChunkStore>();
    [12, 22].into_iter().all(|x| {
        [4, 10].into_iter().all(|z| {
            [floor, floor + height + 1]
                .into_iter()
                .all(|y| chunks.block_at([x, y, z]).is_some())
        })
    })
}

fn epochs(world: &World, floor: i64) -> HashMap<SectionPos, u64> {
    let journal = world.resource::<EditJournal>();
    [12, 22]
        .into_iter()
        .flat_map(|x| {
            [floor, floor + 4]
                .into_iter()
                .map(move |y| global_to_section([x, y, 7]).0)
        })
        .map(|p| (p, journal.mesh_epoch(p)))
        .collect()
}

fn basin_rendered(world: &World, floor: i64) -> bool {
    let journal = world.resource::<EditJournal>();
    let chunks = world.resource::<ChunkStore>();
    epochs(world, floor).keys().all(|p| {
        chunks.loaded.get(p).is_some_and(|s| {
            s.mesh_epoch == journal.mesh_epoch(*p) && s.water_epoch == journal.water_mesh_epoch(*p)
        })
    }) && world.resource::<WaterStatus>().ready
        && world.resource::<WaterStatus>().visible_tiles > 0
        && world.resource::<RenderStatus>().ready
}

fn edit(world: &mut World, position: [i64; 3], material: u16) -> Result<(), String> {
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let expected_material = chunks
            .block_at(position)
            .ok_or("Fixture block not loaded")?;
        if expected_material == material {
            return Ok(());
        }
        let properties = world
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .clone();
        let mut journal = world.resource_mut::<EditJournal>();
        if !commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position,
                expected_material,
                material,
            },
            &properties,
        )? {
            return Err("Fixture edit was rejected".into());
        }
        Ok(())
    })
}

fn build_basin(world: &mut World, floor: i64, height: i64) -> Result<(), String> {
    for x in 12..=22 {
        for z in 4..=10 {
            for y in floor..=floor + 4 {
                let solid = y == floor
                    || (y <= floor + 3 && (x == 12 || x == 22 || z == 4 || z == 10 || x == 16));
                edit(world, [x, y, z], if solid { 3 } else { 0 })?;
            }
        }
    }
    for x in 13..=15 {
        for z in 5..=9 {
            edit(world, [x, floor + height, z], 27)?;
        }
    }
    Ok(())
}

fn right_volume(journal: &EditJournal, floor: i64) -> u64 {
    journal
        .fluid_cells()
        .filter(|(p, _)| {
            (17..=21).contains(&p[0])
                && (5..=9).contains(&p[2])
                && p[1] > floor
                && p[1] <= floor + 3
        })
        .map(|(_, c)| u64::from(c.amount))
        .sum()
}

fn check_sources(world: &World) -> Result<(), String> {
    let journal = world.resource::<EditJournal>();
    let chunks = world.resource::<ChunkStore>();
    let mut sources = 0;
    for (p, cell) in journal.fluid_cells() {
        if cell.source {
            sources += 1;
            if cell.amount != FLUID_CAPACITY || chunks.block_at(p) != Some(27) {
                return Err("Fixture source was depleted or lost its block".into());
            }
        }
        if cell.amount > 0 {
            match chunks.block_at(p) {
                Some(0 | 27) => {}
                Some(id)
                    if terra_core::construction::is_slab(id)
                        || terra_core::construction::is_stair(id) =>
                {
                    check_partial_occupancy(world, p, cell)?;
                }
                _ => return Err(format!("Water inside a solid or unloaded cell at {p:?}")),
            }
        }
    }
    if sources != SOURCE_COUNT {
        return Err(format!(
            "Source count changed: expected {SOURCE_COUNT}, got {sources}"
        ));
    }
    Ok(())
}

fn check_partial_occupancy(
    world: &World,
    position: [i64; 3],
    cell: terra_world::water::FluidCell,
) -> Result<(), String> {
    let journal = world.resource::<EditJournal>();
    let chunks = world.resource::<ChunkStore>();
    let terrain = world.resource::<WorldTerrain>();
    let properties = world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    let mut solids = Vec::new();
    if !terra_world::interaction_shapes::visit_boxes(chunks, journal, properties, position, |b| {
        solids.push(b)
    }) {
        return Err(format!(
            "Waterlogged construction has no collision geometry at {position:?}"
        ));
    }
    let mut wet_samples = 0;
    // Thirds/halves in construction geometry and eighths in liquid levels all
    // align with this lattice; check the solid and free parts independently.
    for x in 0..6 {
        for y in 0..24 {
            for z in 0..6 {
                let local = [
                    (f64::from(x) + 0.5) / 6.0,
                    (f64::from(y) + 0.5) / 24.0,
                    (f64::from(z) + 0.5) / 6.0,
                ];
                let point = std::array::from_fn(|i| position[i] as f64 + local[i]);
                let solid = solids
                    .iter()
                    .any(|b| (0..3).all(|i| point[i] >= b.minimum[i] && point[i] < b.maximum[i]));
                let expected = local[1] < cell.height() && !solid;
                let actual = chunks
                    .water_column_at_point(&terrain.0, journal, point)
                    .is_some_and(|water| point[1] >= water.bed && point[1] < water.level);
                if expected != actual {
                    return Err(format!(
                        "Water/collision disagreement at {point:?}: solid={solid}, expected_wet={expected}, actual_wet={actual}"
                    ));
                }
                wet_samples += usize::from(actual);
            }
        }
    }
    if wet_samples == 0 {
        return Err(format!(
            "Recorded water has no free construction volume at {position:?}"
        ));
    }
    Ok(())
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-fluid"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            directory.join(format!("{name}.png")),
        ));
    Ok(())
}
