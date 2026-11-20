//! Opt-in leaf fixture rendered through journal edits and normal section batches.
use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
const LEAVES: [u16; 10] = [7, 8, 9, 132, 133, 195, 310, 311, 312, 406];
const SHOTS: usize = 7;
#[derive(Resource)]
struct Inspection {
    started: Instant,
    since: Instant,
    base: i64,
    placed: bool,
    shot: usize,
    pending: bool,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Inspection {
            started: Instant::now(),
            since: Instant::now(),
            base: 0,
            placed: false,
            shot: 0,
            pending: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn cell(i: usize, y: i64) -> [i64; 3] {
    [8 + (i % 5) as i64 * 2, y, 8 + (i / 5) as i64 * 3]
}
fn pose(y: i64, shot: usize) -> Transform {
    if shot == 0 {
        return Transform::from_xyz(19., y as f32 + 5.5, 20.)
            .looking_at(Vec3::new(12.5, y as f32 + 0.3, 10.), Vec3::Y);
    }
    let i = [0, 2, 3, 4, 7, 9][shot - 1];
    let target = Vec3::from_array(cell(i, y).map(|v| v as f32)) + Vec3::splat(0.5);
    Transform::from_translation(target + Vec3::new(1.8, 1.15, 2.1)).looking_at(target, Vec3::Y)
}
fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    state.base = ((terrain.0.surface_height(12., 10.) / 16.).ceil() as i64 + 3) * 16;
    let transform = pose(state.base, 0);
    focus.0 = transform.translation.to_array().map(f64::from);
    for mut camera in &mut camera {
        *camera = transform;
    }
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}
#[derive(SystemParam)]
struct View<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    render: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    journal: Res<'w, EditJournal>,
    camera: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
    focus: ResMut<'w, ChunkFocus>,
}
fn run(
    mut commands: Commands,
    mut state: ResMut<Inspection>,
    mut world: View,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.shot >= SHOTS {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(150) || world.render.error.is_some() {
        error!("Foliage gallery timed out or failed");
        state.shot = SHOTS;
        exit.write(AppExit::error());
        return;
    }
    let transform = pose(state.base, state.shot);
    for mut camera in &mut world.camera {
        *camera = transform;
    }
    world.focus.0 = transform.translation.to_array().map(f64::from);
    if !world.render.ready {
        return;
    }
    if !state.placed {
        if (0..LEAVES.len()).any(|i| world.chunks.block_at(cell(i, state.base)).is_none()) {
            return;
        }
        for (i, &material) in LEAVES.iter().enumerate() {
            let position = cell(i, state.base);
            edits.write(BlockEditRequest {
                position,
                expected_material: world.chunks.block_at(position).unwrap(),
                material,
            });
        }
        state.placed = true;
        state.since = Instant::now();
        return;
    }
    if state.pending || state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    for (i, &material) in LEAVES.iter().enumerate() {
        let position = cell(i, state.base);
        if world.chunks.block_at(position) != Some(material) {
            return;
        }
        let section = global_to_section(position).0;
        if world
            .rendered
            .sections
            .get(&section)
            .is_none_or(|s| s.mesh_epoch != world.journal.mesh_epoch(section))
        {
            return;
        }
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-foliage"));
    if std::fs::create_dir_all(&directory).is_err() {
        exit.write(AppExit::error());
        return;
    }
    let path = directory.join(format!("foliage-{}.png", state.shot));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |capture: On<ScreenshotCaptured>,
             mut state: ResMut<Inspection>,
             mut exit: MessageWriter<AppExit>| {
                if capture.image.width() < 1000 {
                    error!("Foliage gallery capture invalid");
                    exit.write(AppExit::error());
                    return;
                }
                state.shot += 1;
                state.pending = false;
                state.since = Instant::now();
                if state.shot == SHOTS {
                    info!("Foliage gallery passed: ten leaf materials, seven native views");
                    exit.write(AppExit::Success);
                }
            },
        );
    state.pending = true;
}
