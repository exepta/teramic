//! Native acceptance of generated habitat layout, without placed showcase props.
use super::*;
use terra_world::generation::OceanArea;

#[derive(Resource)]
struct Habitats {
    started: Instant,
    since: Instant,
    locations: Vec<Vec3>,
    stage: usize,
    requested: bool,
    captured: usize,
    failure: Option<String>,
    folder: PathBuf,
    benchmark: bool,
}

const AREAS: [OceanArea; 3] = [
    OceanArea::CoralReef,
    OceanArea::SeagrassForest,
    OceanArea::ColdOcean,
];

pub(super) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-ocean-habitats") {
        return;
    }
    if !app
        .world()
        .contains_resource::<terra_core::GraphicsConfig>()
    {
        let graphics = terra_core::GraphicsConfig::from_client(
            &app.world().resource::<terra_core::GameConfig>().client,
        );
        app.insert_resource(graphics);
    }
    let benchmark = std::env::var("TERRA_HABITAT_BENCH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&area| area < AREAS.len());
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Habitats {
            started: Instant::now(),
            since: Instant::now(),
            locations: Vec::new(),
            stage: benchmark.unwrap_or(0) * 2,
            requested: false,
            captured: 0,
            failure: None,
            folder: std::env::var_os("TERRA_CAPTURE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| "target/ocean-habitats".into()),
            benchmark: benchmark.is_some(),
        })
        .add_systems(PostStartup, locate)
        .add_systems(Last, run);
}

fn locate(world: &mut World) {
    let terrain = &world.resource::<WorldTerrain>().0;
    let mut locations = [None; 3];
    for z in (-8192..8192).step_by(96) {
        for x in (-8192..8192).step_by(96) {
            let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
            let habitat = terrain.ocean_habitat(x, z);
            let Some(area) = habitat.area() else {
                continue;
            };
            let index = AREAS.iter().position(|a| *a == area).unwrap_or(0);
            if locations[index].is_some() || habitat.weights[index] < 0.7 || habitat.cover < 0.65 {
                continue;
            }
            if index == 0 && habitat.rock < 0.65 {
                continue;
            }
            let column = terrain.surface_column(x, z);
            let Some(level) = column.water_level else {
                continue;
            };
            if !(7.0..=24.0).contains(&(column.ice_bottom().unwrap_or(level) - column.water_bed()))
            {
                continue;
            }
            let center = Vec3::new(x as f32, column.water_bed() as f32, z as f32);
            info!(
                ?area,
                ?center,
                ?habitat,
                "Located natural ocean habitat without fixture edits"
            );
            locations[index] = Some(center);
        }
        if locations.iter().all(Option::is_some) {
            break;
        }
    }
    let mut state = world.resource_mut::<Habitats>();
    if locations.iter().any(Option::is_none) {
        state.failure = Some(format!(
            "Missing generated ocean habitats: {locations:?}; use ocean_revision = 1"
        ));
    } else {
        state.locations = locations.into_iter().flatten().collect();
    }
    if let Err(error) = std::fs::create_dir_all(&state.folder) {
        state.failure = Some(error.to_string());
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Habitats>| {
        let result = advance(world, &mut state);
        if let Err(error) = result {
            error!(%error, "Ocean habitat acceptance failed");
            world.write_message(AppExit::error());
        } else if state.stage == 7 {
            info!("Natural reef, seagrass forest and cold ocean acceptance passed");
            world.write_message(AppExit::Success);
        }
    });
}

fn advance(world: &mut World, state: &mut Habitats) -> Result<(), String> {
    if let Some(error) = state.failure.take() {
        return Err(error);
    }
    if state.started.elapsed() > Duration::from_secs(240) {
        return Err("Ocean habitat timeout".into());
    }
    if state.requested && state.captured > state.stage {
        state.stage += 1;
        state.requested = false;
        state.since = Instant::now();
    }
    if state.stage == 7 {
        return Ok(());
    }
    let index = (state.stage / 2).min(2);
    let center = state.locations[index];
    let terrain = &world.resource::<WorldTerrain>().0;
    let column = terrain.surface_column(f64::from(center.x), f64::from(center.z));
    let top = column.water_level.ok_or("Ocean water disappeared")? as f32;
    let mut target = center + Vec3::Y * 1.5;
    let mut eye = center + Vec3::new(6.0, 3.0, 9.0);
    if state.stage == 1 {
        eye = center + Vec3::new(-8.0, 4.5, 12.0);
    }
    if state.stage == 5 {
        eye = center + Vec3::new(-4.0, 2.8, 6.0);
        target = center + Vec3::Y * 0.5;
    }
    if state.stage == 6 {
        eye = Vec3::new(center.x + 10.0, top + 8.0, center.z + 12.0);
        target = Vec3::new(center.x, top, center.z);
    } else {
        let floor = terrain.surface_column(f64::from(eye.x), f64::from(eye.z));
        let ceiling = floor.ice_bottom().unwrap_or(f64::from(top)) as f32;
        eye.y = eye
            .y
            .max(floor.water_bed() as f32 + 1.0)
            .min(ceiling - 0.65);
    }
    set_camera(
        world,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
        center,
    );
    // The existing benchmark system owns sampling and exit in this mode.
    if state.benchmark {
        return Ok(());
    }
    if state.requested || state.since.elapsed() < Duration::from_secs(4) || !ready(world) {
        return Ok(());
    }
    let chunks = world.resource::<ChunkStore>();
    let natural = chunks
        .loaded
        .values()
        .flat_map(|s| &s.props)
        .filter(|p| p.kind.aquatic() && !p.removed)
        .count();
    let ice = chunks
        .loaded
        .values()
        .flat_map(|s| s.data.blocks())
        .filter(|&&m| m == 49)
        .count();
    let seagrass = chunks
        .loaded
        .values()
        .flat_map(|s| s.data.blocks())
        .filter(|&&m| m == 50)
        .count();
    let marine_props = chunks
        .loaded
        .values()
        .flat_map(|s| s.data.blocks())
        .filter(|&&m| (545..=580).contains(&m))
        .count();
    let modern = world.resource::<WorldTerrain>().0.marine_overhaul();
    if natural + seagrass < 10 || (!modern && ice != 0) || (modern && marine_props == 0) {
        return Err(format!(
            "Missing habitat geometry: aquatic props {natural}, marine blocks {marine_props}, ice {ice}"
        ));
    }
    info!(stage=state.stage, area=?AREAS[index], natural_props=natural, ice_blocks=ice,
        batches=world.resource::<terra_render::PropRenderStatus>().batches,
        "Natural habitat capture");
    state.requested = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            state.folder.join(format!("habitat-{}.png", state.stage)),
        ))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Habitats>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_ref()
                    .into_iter()
                    .flat_map(|b| b.chunks_exact(4).step_by(53))
                    .map(|p| [p[0] / 8, p[1] / 8, p[2] / 8])
                    .collect();
                if colors.len() < 16 {
                    state.failure = Some("Blank habitat screenshot".into());
                }
                state.captured += 1;
            },
        );
    Ok(())
}
