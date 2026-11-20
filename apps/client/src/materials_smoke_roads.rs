//! Opt-in F68 native gallery through real section edits, models and PBR materials.
use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use terra_core::roads;

const SHOTS: usize = 3;
#[derive(Resource)]
struct Inspection {
    started: Instant,
    since: Instant,
    base: i64,
    placed: bool,
    oriented: bool,
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
            oriented: false,
            shot: 0,
            pending: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn samples(y: i64) -> Vec<([i64; 3], u16, u8)> {
    let mut samples = Vec::new();
    for x in 8..=11 {
        for (z, material) in [
            (7, roads::COVER),
            (8, roads::COVER),
            (9, roads::ENTRY),
            (10, roads::EXIT),
            (11, roads::ASPHALT),
            (12, roads::ASPHALT),
        ] {
            samples.push(([x, y, z], material, 0));
        }
    }
    for x in [7, 12] {
        for z in 7..=8 {
            samples.push(([x, y, z], roads::CURB, 1));
        }
        for z in 11..=12 {
            samples.push(([x, y, z], roads::ASPHALT, 0));
            samples.push(([x, y + 1, z], roads::CURB, 1));
        }
    }
    for facing in 0..4 {
        samples.push(([8 + i64::from(facing) * 3, y, 16], roads::SLOPE, facing));
        samples.push(([8 + i64::from(facing) * 3, y, 19], roads::CURB, facing));
    }
    samples
}
fn pose(y: i64, shot: usize) -> Transform {
    let y = y as f32;
    match shot {
        1 => {
            Transform::from_xyz(14., y + 1.5, 5.).looking_at(Vec3::new(10., y + 0.45, 10.), Vec3::Y)
        }
        2 => Transform::from_xyz(20., y + 4.8, 22.)
            .looking_at(Vec3::new(12.5, y + 0.4, 17.5), Vec3::Y),
        _ => {
            Transform::from_xyz(23., y + 11., 4.).looking_at(Vec3::new(12., y + 0.35, 13.), Vec3::Y)
        }
    }
}
fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
    mut config: ResMut<terra_core::GameConfig>,
) {
    state.base = ((terrain.0.surface_height(12., 13.) / 16.).ceil() as i64 + 3) * 16;
    config.client.fov_degrees = 50.;
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
    chunks: ResMut<'w, ChunkStore>,
    render: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    journal: ResMut<'w, EditJournal>,
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
        error!(error = ?world.render.error, "F68 road gallery timed out or failed");
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
    let samples = samples(state.base);
    if !state.placed {
        let y = state.base - 1;
        let platform: Vec<_> = (6..=19)
            .flat_map(|x| (6..=20).map(move |z| [x, y, z]))
            .collect();
        if platform
            .iter()
            .chain(samples.iter().map(|s| &s.0))
            .any(|&p| world.chunks.block_at(p).is_none())
        {
            return;
        }
        for position in platform {
            edits.write(BlockEditRequest {
                position,
                expected_material: world.chunks.block_at(position).unwrap(),
                material: 12,
            });
        }
        for &(position, material, _) in &samples {
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
    if samples
        .iter()
        .any(|&(p, material, _)| world.chunks.block_at(p) != Some(material))
    {
        return;
    }
    if !state.oriented {
        for &(position, material, facing) in &samples {
            if roads::is_shape(material) {
                let View {
                    chunks, journal, ..
                } = &mut world;
                if let Err(error) = terra_world::streaming::commit_object_state(
                    chunks,
                    journal,
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                ) {
                    error!(%error, "F68 road pose failed");
                    state.shot = SHOTS;
                    exit.write(AppExit::error());
                    return;
                }
            }
        }
        state.oriented = true;
        state.since = Instant::now();
        return;
    }
    if state.pending || state.since.elapsed() < Duration::from_secs(3) {
        return;
    }
    if samples.iter().any(|&(p, _, _)| {
        let section = global_to_section(p).0;
        world
            .rendered
            .sections
            .get(&section)
            .is_none_or(|s| s.mesh_epoch != world.journal.mesh_epoch(section))
    }) {
        return;
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-roads"));
    if std::fs::create_dir_all(&directory).is_err() {
        exit.write(AppExit::error());
        return;
    }
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(directory.join(format!("roads-{}.png", state.shot)))).observe(
        |capture: On<ScreenshotCaptured>, mut state: ResMut<Inspection>, mut exit: MessageWriter<AppExit>| {
            if capture.image.width() < 1000 { error!("F68 road capture invalid"); exit.write(AppExit::error()); return; }
            state.shot += 1;
            state.pending = false;
            state.since = Instant::now();
            if state.shot == SHOTS { info!("F68 road gallery passed: cover/entry/exit/fullblock joins, four ramp/curb facings, three native views"); exit.write(AppExit::Success); }
        }
    );
    state.pending = true;
}
