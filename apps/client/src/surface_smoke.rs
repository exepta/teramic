use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{RenderStatus, RenderedSections};
use terra_voxel::global_to_section;
use terra_world::{
    edits::EditJournal,
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
};

#[derive(Resource)]
struct SurfaceSmoke {
    started: Instant,
    stage: u8,
    since: Instant,
    base: [i64; 3],
    captured: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-surface") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(SurfaceSmoke {
            started: now,
            stage: 0,
            since: now,
            base: [0; 3],
            captured: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run);
}

fn setup(
    mut state: ResMut<SurfaceSmoke>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
) {
    let y = ((terrain.0.surface_height(-16.0, -16.0) / 16.0).ceil() as i64 + 2) * 16 - 1;
    state.base = [-17, y, -17];
    let base = Vec3::from_array(state.base.map(|v| v as f32));
    let eye = base + Vec3::new(5.8, 2.7, 6.5);
    focus.0 = eye.to_array().map(f64::from);
    for mut transform in &mut camera {
        *transform =
            Transform::from_translation(eye).looking_at(base + Vec3::new(1.5, 1.5, 1.5), Vec3::Y);
    }
}

#[derive(SystemParam)]
struct SurfaceWorld<'w> {
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    render: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    journal: Res<'w, EditJournal>,
}

fn run(
    mut commands: Commands,
    mut state: ResMut<SurfaceSmoke>,
    world: SurfaceWorld,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.stage == 3 {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(90) || world.render.error.is_some() {
        error!(stage = state.stage, "Surface fixture timed out or failed");
        state.stage = 3;
        exit.write(AppExit::error());
        return;
    }
    if world.generation.status != GenerationStatus::Ready || !world.render.ready {
        return;
    }
    let base = state.base;
    let cell = |x, y, z| [base[0] + x, base[1] + y, base[2] + z];
    if state.stage == 0 {
        // Session-only joined wood, turf and rock, spanning negative X/Z and Y section borders.
        for z in -1..=3 {
            for y in -1..=4 {
                for x in -1..=3 {
                    let position = cell(x, y, z);
                    let Some(expected_material) = world.chunks.block_at(position) else {
                        return;
                    };
                    let material = if y == -1 {
                        if x < 1 { 1 } else { 3 }
                    } else if ((0..=2).contains(&x) && (0..=2).contains(&z) && y <= 3)
                        || (x == 3 && z == 2 && y == 0)
                    {
                        6
                    } else {
                        0
                    };
                    if expected_material != material {
                        edits.write(BlockEditRequest {
                            position,
                            expected_material,
                            material,
                        });
                    }
                }
            }
        }
        state.stage = 1;
        state.since = Instant::now();
        return;
    }
    if state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    for position in [cell(0, -1, 0), cell(1, 0, 1), cell(2, 3, 2)] {
        let section = global_to_section(position).0;
        if world
            .rendered
            .sections
            .get(&section)
            .is_none_or(|rendered| rendered.mesh_epoch != world.journal.mesh_epoch(section))
        {
            return;
        }
    }
    if !state.captured {
        let mined = state.stage == 2;
        if world.chunks.block_at(cell(2, 1, 2)) != Some(if mined { 0 } else { 6 }) {
            return;
        }
        let path = std::env::var_os("TERRA_SCREENSHOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/terra-surface.png"));
        let stem = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("surface");
        let label = if mined { "mined" } else { "joined" };
        let path = path.with_file_name(format!("{stem}-{label}.png"));
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
        state.captured = true;
        info!(label, "Joined surface fixture captured");
    }
    if state.since.elapsed() > Duration::from_secs(4) {
        if state.stage == 1 {
            edits.write(BlockEditRequest {
                position: cell(2, 1, 2),
                expected_material: 6,
                material: 0,
            });
            state.stage = 2;
            state.since = Instant::now();
            state.captured = false;
        } else {
            state.stage = 3;
            info!("Surface fixture passed: joined and mined wood across section borders");
            exit.write(AppExit::Success);
        }
    }
}
