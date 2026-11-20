//! Opt-in native inspection of prepared paths and real-time finite tar drying.
use super::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use terra_core::roads;

#[derive(Resource)]
struct Inspection {
    started: Instant,
    since: Instant,
    base: i64,
    prepared: bool,
    poured: Option<Instant>,
    shot: usize,
    pending: bool,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Inspection {
            started: Instant::now(),
            since: Instant::now(),
            base: 0,
            prepared: false,
            poured: None,
            shot: 0,
            pending: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}

fn paths(y: i64) -> Vec<[i64; 3]> {
    [8, 10, 12]
        .into_iter()
        .flat_map(|z| (8..=15).map(move |x| [x, y, z]))
        .collect()
}

fn pose(y: i64) -> Transform {
    Transform::from_xyz(20.5, y as f32 + 6.0, 2.5)
        .looking_at(Vec3::new(11.5, y as f32 + 0.6, 10.0), Vec3::Y)
}

fn setup(
    mut state: ResMut<Inspection>,
    terrain: Res<WorldTerrain>,
    mut camera: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    state.base = ((terrain.0.surface_height(12.0, 10.0) / 16.0).ceil() as i64 + 3) * 16;
    let transform = pose(state.base);
    for mut camera in &mut camera {
        *camera = transform;
    }
    focus.0 = transform.translation.to_array().map(f64::from);
    config.client.fov_degrees = 45.0;
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}

#[derive(SystemParam)]
struct View<'w, 's> {
    chunks: ResMut<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    render: Res<'w, RenderStatus>,
    rendered: Res<'w, RenderedSections>,
    camera: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
    focus: ResMut<'w, ChunkFocus>,
}

fn run(
    mut commands: Commands,
    mut state: ResMut<Inspection>,
    mut world: View,
    mut exit: MessageWriter<AppExit>,
) {
    if state.shot >= 4 {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(170) || world.render.error.is_some() {
        error!(error = ?world.render.error, "Native tar inspection timed out or failed");
        state.shot = 4;
        exit.write(AppExit::error());
        return;
    }
    let transform = pose(state.base);
    for mut camera in &mut world.camera {
        *camera = transform;
    }
    world.focus.0 = transform.translation.to_array().map(f64::from);
    if !world.render.ready {
        return;
    }
    let paths = paths(state.base);
    if !state.prepared {
        let base = state.base;
        let platform: Vec<_> = (6..=22)
            .flat_map(|x| (6..=14).flat_map(move |z| (-1..=1).map(move |dy| [x, base + dy, z])))
            .collect();
        if platform.iter().any(|&p| world.chunks.block_at(p).is_none()) {
            return;
        }
        let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
        for p in platform {
            let bile_wall = (16..=20).contains(&p[0])
                && (8..=12).contains(&p[2])
                && (p[0] == 16 || p[0] == 20 || p[2] == 8 || p[2] == 12);
            let material = if p[1] == state.base + 1 && bile_wall {
                3
            } else if p[1] < state.base {
                12
            } else if p[1] > state.base {
                0
            } else {
                [1, 2, 4, 42][p[0].rem_euclid(4) as usize]
            };
            let expected_material = world.chunks.block_at(p).unwrap();
            let View {
                chunks, journal, ..
            } = &mut world;
            terra_world::streaming::commit_block_edit(
                chunks,
                journal,
                &BlockEditRequest {
                    position: p,
                    expected_material,
                    material,
                },
                &properties,
            )
            .unwrap();
        }
        for &p in &paths {
            let View {
                chunks, journal, ..
            } = &mut world;
            assert!(terra_world::roads::make_path(chunks, journal, p).unwrap());
        }
        state.prepared = true;
        state.since = Instant::now();
        return;
    }
    if state.shot == 1 && state.poured.is_none() {
        for z in [8, 10, 12] {
            let View {
                chunks, journal, ..
            } = &mut world;
            assert!(terra_world::roads::pour(chunks, journal, [8, state.base, z]).unwrap());
        }
        {
            let View {
                chunks, journal, ..
            } = &mut world;
            assert!(
                terra_world::bile::deposit_from_bucket(chunks, journal, [18, state.base + 1, 10])
                    .unwrap()
            );
        }
        state.poured = Some(Instant::now());
        state.since = Instant::now();
        return;
    }
    let (mut wet, mut dried, mut units) = (0, 0, 0u32);
    let mut cells = Vec::new();
    for &p in &paths {
        let material = world.chunks.block_at(p).unwrap();
        let object = world.journal.object_state(p).unwrap_or_default();
        let facing = object.facing;
        if roads::is_tar(material) {
            units += u32::from(
                object
                    .liquid
                    .map_or(roads::tar_amount(facing) * 8, |s| s.volume),
            );
            wet += usize::from(material == roads::WET_TAR);
            dried += usize::from(material == roads::DRIED_TAR);
        }
        cells.push(serde_json::json!({"position": p, "material": material, "state": facing,"liquid":object.liquid}));
    }
    if state.poured.is_some() && units != 192 {
        error!(units, "Native tar inspection lost or duplicated liquid");
        state.shot = 4;
        exit.write(AppExit::error());
        return;
    }
    let bile_cells: Vec<_> = world
        .journal
        .object_states()
        .filter(|(_, id, _)| *id == terra_core::bile::FLUID)
        .map(|(p, _, s)| serde_json::json!({"position":p,"liquid":s.liquid}))
        .collect();
    let elapsed = state.poured.map_or(0.0, |at| at.elapsed().as_secs_f64());
    if state.pending
        || state.since.elapsed() < Duration::from_secs(3)
        || state.shot == 2 && (elapsed < 10.0 || wet <= 3)
        || state.shot == 3 && (wet != 0 || dried <= 3)
    {
        return;
    }
    if paths.iter().any(|&p| {
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
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-tar"));
    std::fs::create_dir_all(&directory).unwrap();
    let stem = ["tar-0-path", "tar-1-poured", "tar-2-flowing", "tar-3-dried"][state.shot];
    std::fs::write(
        directory.join(format!("{stem}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":"passed", "elapsed_seconds":elapsed, "wet_cells":wet,
            "dried_cells":dried, "units":units, "cells":cells,"bile_cells":bile_cells}),
        )
        .unwrap(),
    )
    .unwrap();
    info!(
        shot = state.shot,
        elapsed, wet, dried, units, "Native tar phase captured"
    );
    commands.spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{stem}.png"))))
        .observe(|capture: On<ScreenshotCaptured>, mut state: ResMut<Inspection>, mut exit: MessageWriter<AppExit>| {
            if capture.image.width() < 1000 { exit.write(AppExit::error()); return; }
            state.shot += 1; state.pending = false; state.since = Instant::now();
            if state.shot == 4 {
                info!("Native tar inspection passed: prepared soils, finite viscous spread, real-time drying, 192 conserved fine units, four GPU captures");
                exit.write(AppExit::Success);
            }
        });
    state.pending = true;
}
