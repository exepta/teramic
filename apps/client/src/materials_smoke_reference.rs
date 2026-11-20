//! Reference palette and botany through real section edits, props and PBR materials.
use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

#[path = "materials_smoke_reference/transitions.rs"]
mod transitions;
#[path = "materials_smoke_reference/waterline.rs"]
mod waterline;

const SHOTS: usize = 4;
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
    if std::env::var("TERRA_WATERLINE_INSPECT").as_deref() == Ok("1") {
        waterline::install(app);
        return;
    }
    if std::env::var("TERRA_BIOME_TRANSITION_GALLERY").as_deref() == Ok("1") {
        transitions::install(app);
        return;
    }
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
fn street_gallery() -> bool {
    std::env::var("TERRA_STREET_INSPECT").as_deref() == Ok("1")
}
fn samples(y: i64) -> Vec<([i64; 3], u16, u8)> {
    if street_gallery() {
        let mut result = vec![
            ([9, y, 9], 440, 0),
            ([13, y, 9], 441, 0),
            ([17, y, 9], 448, 0),
        ];
        for (i, id) in (442..=447).enumerate() {
            result.push(([8 + i as i64 * 3, y, 15], id, 0));
            for x in 0..2 {
                result.push(([8 + i as i64 * 3 + x, y, 14], 432, 0));
            }
        }
        for x in 8..=22 {
            for z in 19..=22 {
                result.push(([x, y, z], 1, 0));
            }
        }
        return result;
    }
    let mut samples = Vec::new();
    for (i, material) in [1, 4, 42, 3, 13].into_iter().enumerate() {
        samples.push(([8 + i as i64 * 3, y, 8], material, 0));
    }
    for (i, (log, planks)) in [
        (6, 16),
        (10, 17),
        (36, 18),
        (405, 404),
        (313, 314),
        (134, 135),
    ]
    .into_iter()
    .enumerate()
    {
        samples.push(([8 + i as i64 * 3, y, 13], log, 0));
        samples.push(([8 + i as i64 * 3, y, 16], planks, 0));
    }
    samples.push(([10, y, 21], 317, 0));
    samples.push(([13, y, 21], 426, 0));
    samples
}
fn pose(y: i64, shot: usize) -> Transform {
    let y = y as f32;
    if street_gallery() {
        return match shot {
            1 => Transform::from_xyz(21., y + 4., 3.)
                .looking_at(Vec3::new(13., y + 0.4, 9.), Vec3::Y),
            2 => Transform::from_xyz(15., y + 6., 9.)
                .looking_at(Vec3::new(15., y + 0.25, 15.), Vec3::Y),
            3 => Transform::from_xyz(20., y + 5., 17.)
                .looking_at(Vec3::new(15., y + 1., 21.), Vec3::Y),
            _ => Transform::from_xyz(30., y + 12., 1.)
                .looking_at(Vec3::new(15., y + 0.5, 14.), Vec3::Y),
        };
    }
    match shot {
        1 => {
            Transform::from_xyz(20., y + 3.5, 3.).looking_at(Vec3::new(14., y + 0.45, 8.5), Vec3::Y)
        }
        2 => {
            Transform::from_xyz(4., y + 4., 21.).looking_at(Vec3::new(16., y + 0.5, 14.5), Vec3::Y)
        }
        3 => Transform::from_xyz(23., y + 2.0, 17.)
            .looking_at(Vec3::new(16., y + 0.35, 21.5), Vec3::Y),
        _ => Transform::from_xyz(32., y + 12., 1.)
            .looking_at(Vec3::new(15.5, y + 0.5, 14.5), Vec3::Y),
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
        error!(error = ?world.render.error, "Reference palette gallery timed out or failed");
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
        let platform: Vec<_> = (6..=25)
            .flat_map(|x| (6..=23).map(move |z| [x, y, z]))
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
                material: if position[2] >= 20 { 4 } else { 2 },
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
    if !state.oriented && street_gallery() {
        for &(position, material, facing) in &samples {
            if !terra_core::street::is_shape(material) && !terra_core::roads::is_shape(material) {
                continue;
            }
            if let Err(error) = terra_world::streaming::commit_object_state(
                &mut world.chunks,
                &mut world.journal,
                position,
                Some(terra_world::edits::BlockObjectState {
                    facing,
                    ..default()
                }),
            ) {
                error!(%error, "Street profile state failed");
                exit.write(AppExit::error());
                return;
            }
        }
        state.oriented = true;
        state.since = Instant::now();
        return;
    }
    if !state.oriented {
        for (i, x) in [17, 19, 21, 23].into_iter().enumerate() {
            let prop = terra_world::props::WorldProp {
                id: terra_world::props::PropId(0x8000_0000_0000_1800 + i as u64),
                kind: terra_world::props::PropKind::Bush,
                position: [x as f64 + 0.5, state.base as f64, 21.5],
                rotation: i as f32 * 0.4,
                scale: 1.0,
                variant: 128 + i as u8,
                section: global_to_section([x, state.base - 1, 21]).0,
                removed: false,
            };
            assert!(world.journal.place_prop(prop).unwrap());
            world
                .chunks
                .loaded
                .get_mut(&prop.section)
                .unwrap()
                .props
                .push(prop);
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
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-reference-palette"));
    if std::fs::create_dir_all(&directory).is_err() {
        exit.write(AppExit::error());
        return;
    }
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(directory.join(format!("reference-palette-{}.png", state.shot)))).observe(
        |capture: On<ScreenshotCaptured>, mut state: ResMut<Inspection>, mut exit: MessageWriter<AppExit>| {
            if capture.image.width() < 1000 { error!("Reference palette capture invalid"); exit.write(AppExit::error()); return; }
            state.shot += 1;
            state.pending = false;
            state.since = Instant::now();
            if state.shot == SHOTS { if street_gallery() { info!("Street block gallery passed: bile stone, full paver, tied straw, all six sidewalk profiles beside asphalt and a connected grass patch"); } info!("Reference palette gallery passed: natural mineral blocks, six timber families, open flowering shrubs and four rooted dead bushes; four native views"); exit.write(AppExit::Success); }
        }
    );
    state.pending = true;
}
