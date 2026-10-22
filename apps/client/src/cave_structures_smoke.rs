//! Natural cave-ecology acceptance. Inspection lamps are fixtures, not generated content.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_world::{
    generation::{CaveEcology, CaveStructure},
    streaming::{ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain},
};

const KINDS: [CaveEcology; 4] = [
    CaveEcology::Salt,
    CaveEcology::Dripstone,
    CaveEcology::Lush,
    CaveEcology::Waterfall,
];
mod connectors;

#[derive(Resource)]
struct Inspection {
    started: Instant,
    entered: Instant,
    rooms: Vec<CaveStructure>,
    stage: usize,
    captured: usize,
    requested: bool,
    folder: PathBuf,
    failure: Option<String>,
    benchmark: bool,
    connector: Option<connectors::View>,
}
impl Inspection {
    fn end(&self) -> usize {
        if self.connector.is_some() {
            1
        } else {
            KINDS.len() * 2
        }
    }
}
#[derive(Component)]
struct InspectionLamp(usize);

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-cave-structures") {
        return;
    }
    let benchmark = std::env::var("TERRA_CAVE_STRUCTURE_BENCH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&i| i < KINDS.len());
    let now = Instant::now();
    app.insert_resource(Inspection {
        started: now,
        entered: now,
        rooms: Vec::new(),
        stage: benchmark.unwrap_or(0) * 2,
        captured: 0,
        requested: false,
        folder: std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "target/cave-structures".into()),
        failure: None,
        benchmark: benchmark.is_some(),
        connector: None,
    })
    .insert_resource(terra_player::ControllerMode::Fly)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PostStartup, locate)
    .add_systems(Last, run);
}

fn locate(world: &mut World) {
    if std::env::var_os("TERRA_CAVE_DEBUG").is_some() {
        world.resource_mut::<terra_core::DebugSettings>().enabled = true;
    }
    let spectator = std::env::var_os("TERRA_CAVE_SPECTATOR").is_some();
    if spectator {
        world.init_resource::<terra_network::client::ClientSession>();
        world
            .resource_mut::<terra_network::client::ClientSession>()
            .game_mode = terra_core::GameMode::Spectator;
    }
    let terrain = world.resource::<WorldTerrain>().0.clone();
    if let Some(index) = std::env::var("TERRA_CAVE_CONNECTOR")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        match connectors::locate(&terrain, index) {
            Ok(view) => {
                info!(name=%view.name, eye=?view.eye, target=?view.target, "Located natural cave connector");
                let mut state = world.resource_mut::<Inspection>();
                state.connector = Some(view);
                state.stage = 0;
            }
            Err(error) => world.resource_mut::<Inspection>().failure = Some(error),
        }
    }
    let mut rooms = [None; 4];
    for (x, z) in
        std::iter::once((87, -94)).chain((-96..96).flat_map(|z| (-96..96).map(move |x| (x, z))))
    {
        let Some(center) =
            terrain.cave_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
        else {
            continue;
        };
        let room = terrain.cave_structure_at(center[0], center[2]);
        if let Some(index) = KINDS.iter().position(|kind| *kind == room.kind)
            && room.active()
            && rooms[index].is_none()
        {
            info!(kind=?room.kind,center=?room.center,radius=room.radius,"Located natural cave ecology");
            rooms[index] = Some(room);
        }
        if rooms.iter().all(Option::is_some) {
            break;
        }
    }
    let mut state = world.resource_mut::<Inspection>();
    if rooms.iter().any(Option::is_none) {
        state.failure = Some(format!(
            "Missing cave rooms: {rooms:?}; requires cave_structure_revision=1, geology_revision=4"
        ));
    } else {
        state.rooms = rooms.into_iter().flatten().collect();
    }
    if let Err(error) = std::fs::create_dir_all(&state.folder) {
        state.failure = Some(error.to_string());
    }
    let connector = state.connector.is_some();
    for i in 0..if spectator { 0 } else { 2 } {
        world.spawn((
            PointLight {
                color: Color::srgb(0.88, 0.94, 1.0),
                intensity: if connector { 6_000.0 } else { 650_000.0 },
                range: if connector { 16.0 } else { 80.0 },
                radius: 1.5,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::default(),
            InspectionLamp(i),
        ));
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world,mut state:Mut<Inspection>| {
        if let Err(error)=advance(world,&mut state) {
            error!(%error,"Natural cave-structures acceptance failed");
            world.write_message(AppExit::error());
        } else if state.stage==state.end() && state.entered.elapsed() >= Duration::from_millis(250) {
            // Drain screenshot readbacks before closing the render-world channel.
            info!(spectator = std::env::var_os("TERRA_CAVE_SPECTATOR").is_some(), "Natural cave-structures acceptance passed; generated geometry, spectator fill or two explicit inspection lamps");
            world.write_message(AppExit::Success);
        }
    });
}

fn advance(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if let Some(error) = state.failure.take() {
        return Err(error);
    }
    if state.started.elapsed() > Duration::from_secs(240) {
        return Err("Cave structure timeout".into());
    }
    if state.requested && state.captured > state.stage {
        state.stage += 1;
        state.requested = false;
        state.entered = Instant::now();
    }
    if state.stage == state.end() {
        return Ok(());
    }
    let room = state.rooms[state.stage / 2];
    let mut center = Vec3::from_array(room.center.map(|v| v as f32));
    let terrain = &world.resource::<WorldTerrain>().0;
    let offset = if state.stage.is_multiple_of(2) {
        Vec3::new(-0.24, 0.0, 0.33)
    } else {
        Vec3::new(0.34, 0.0, 0.15)
    } * room.radius as f32;
    let mut eye = center + offset;
    let local = terrain.cave_structure_at(f64::from(eye.x), f64::from(eye.z));
    eye.y = (local.floor as f32 + 5.5).max(room.floor as f32 + 6.0);
    let mut target = center + Vec3::new(4.0, -4.0, -7.0);
    if room.kind == CaveEcology::Waterfall {
        target.y = (room.center[1] - room.height * 0.72 - 0.25).floor() as f32 + 4.0;
        eye.y = local.floor as f32 + 5.5;
    }
    if room.kind == CaveEcology::Lush && !state.stage.is_multiple_of(2) {
        target.x = center.x + room.radius as f32 * 0.45;
        target.z = center.z - room.radius as f32 * 0.25;
        target.y = terrain
            .cave_structure_at(f64::from(target.x), f64::from(target.z))
            .floor as f32
            + 2.0;
        eye.y = local.floor as f32 + 3.0;
    }
    if !state.benchmark
        && let Some(water) = local.water
    {
        eye.y = eye.y.max(water.level as f32 + 2.0);
    }
    if let Some(view) = &state.connector {
        eye = view.eye;
        target = view.target;
        center = eye;
    }
    crate::reef_smoke::set_camera(
        world,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
        center,
    );
    for (mut transform, lamp) in world
        .query::<(&mut Transform, &InspectionLamp)>()
        .iter_mut(world)
    {
        transform.translation = if state.connector.is_some() {
            eye + Vec3::Y * 0.3
        } else {
            center
                + Vec3::new(
                    if lamp.0 == 0 { -12.0 } else { 12.0 },
                    -4.0,
                    if lamp.0 == 0 { 12.0 } else { -12.0 },
                )
        };
    }
    if state.benchmark {
        return Ok(());
    }
    let status = world.resource::<terra_render::RenderStatus>();
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    if state.requested
        || state.entered.elapsed() < Duration::from_secs(if room.dynamic_water { 20 } else { 4 })
        || !status.ready
        || status.pending_jobs != 0
        || world.resource::<WorldGeneration>().status != GenerationStatus::Ready
    {
        return Ok(());
    }
    let chunks = world.resource::<ChunkStore>();
    let mut counts = [0; 7];
    for &material in chunks.loaded.values().flat_map(|s| s.data.blocks()) {
        if (50..=56).contains(&material) {
            counts[(material - 50) as usize] += 1;
        }
    }
    let water = world.resource::<terra_render::WaterStatus>();
    let spray = water.spray_particles;
    let water = water.visible_tiles;
    info!(stage=state.stage,kind=?room.kind,?counts,water_tiles=water,spray_particles=spray,loaded=chunks.loaded.len(),"Natural cave capture");
    if state.connector.is_none()
        && (room.kind == CaveEcology::Salt && counts[1] == 0
            || room.kind == CaveEcology::Waterfall && water == 0)
    {
        return Err("Missing cave mineral/water geometry".into());
    }
    state.requested = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(state.folder.join(
            state.connector.as_ref().map_or_else(
                || format!("cave-{:?}-{}.png", room.kind, state.stage % 2),
                |view| format!("connector-{}.png", view.name),
            ),
        )))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_ref()
                    .into_iter()
                    .flat_map(|data| data.chunks_exact(4).step_by(53))
                    .map(|p| [p[0] / 8, p[1] / 8, p[2] / 8])
                    .collect();
                if colors.len() < 16 {
                    state.failure = Some("Blank cave screenshot".into());
                }
                state.captured += 1;
            },
        );
    Ok(())
}
