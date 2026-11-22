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
    TerraGenerator, TerrainGenerator,
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
    shell: bool,
    salt_gallery: bool,
    connector: Option<connectors::View>,
    gallery_view: Option<(usize, Vec3, Vec3)>,
}
impl Inspection {
    fn end(&self) -> usize {
        if self.shell || self.salt_gallery {
            2
        } else if self.connector.is_some() {
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
        shell: std::env::var_os("TERRA_CAVE_SHELL_INSPECT").is_some(),
        salt_gallery: std::env::var_os("TERRA_SALT_GALLERY_INSPECT").is_some(),
        connector: None,
        gallery_view: None,
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
    if world.resource::<Inspection>().shell {
        let room = (-16..0)
            .flat_map(|z| (-16..0).map(move |x| (x, z)))
            .filter_map(|(x, z)| {
                terrain
                    .cave_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
            })
            .map(|p| terrain.cave_structure_at(p[0], p[2]))
            .find(|room| {
                room.active()
                    && room.ceiling - room.floor > 48.0
                    && room.kind != CaveEcology::Lush
                    && room.ceiling < terrain.surface_height(room.center[0], room.center[2]) - 6.0
            });
        match room {
            Some(room) => {
                info!(seed=terrain.config().seed.0,revision=terrain.config().cave_structure_revision,center=?room.center,floor=room.floor,ceiling=room.ceiling,radius=room.radius,"Located tall natural cave shell fixture");
                let mut state = world.resource_mut::<Inspection>();
                state.rooms = vec![room];
                state.stage = 0;
            }
            None => {
                world.resource_mut::<Inspection>().failure =
                    Some("No enclosed >48m natural chamber for shell fixture".into())
            }
        }
    }
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
    let shell = world.resource::<Inspection>().shell;
    let gallery = world.resource::<Inspection>().salt_gallery;
    let mut rooms = [None; 4];
    if !shell {
        for (x, z) in
            std::iter::once((87, -94)).chain((-96..96).flat_map(|z| (-96..96).map(move |x| (x, z))))
        {
            let Some(center) = terrain
                .cave_chamber_center(f64::from(x) * 192.0 + 96.0, f64::from(z) * 192.0 + 96.0)
            else {
                continue;
            };
            let room = terrain.cave_structure_at(center[0], center[2]);
            if gallery
                && (room.kind != CaveEcology::Salt
                    || room.radius < 34.0
                    || room.dynamic_water
                    || !(-2..=2).any(|i| {
                        (-2..=2).any(|j| {
                            terrain
                                .cave_structure_at(
                                    room.center[0] + room.radius * f64::from(i) * 0.18,
                                    room.center[2] + room.radius * f64::from(j) * 0.18,
                                )
                                .open_to_sky()
                        })
                    }))
            {
                continue;
            }
            if terrain.config().cave_structure_revision >= 12
                && room.kind == CaveEcology::Lush
                && (room.radius < 38.0 || !room.open_to_sky())
            {
                continue;
            }
            if let Some(index) = KINDS.iter().position(|kind| *kind == room.kind)
                && room.active()
                && rooms[index].is_none()
            {
                info!(kind=?room.kind,center=?room.center,radius=room.radius,"Located natural cave ecology");
                rooms[index] = Some(room);
            }
            if gallery && rooms[0].is_some() || rooms.iter().all(Option::is_some) {
                break;
            }
        }
    }
    let mut state = world.resource_mut::<Inspection>();
    if !shell
        && ((!gallery && rooms.iter().any(Option::is_none)) || (gallery && rooms[0].is_none()))
    {
        state.failure = Some(format!(
            "Missing cave rooms: {rooms:?}; requires cave structures enabled, geology_revision=4"
        ));
    } else if !shell {
        state.rooms = rooms.into_iter().flatten().collect();
    }
    if let Err(error) = std::fs::create_dir_all(&state.folder) {
        state.failure = Some(error.to_string());
    }
    let connector = state.connector.is_some();
    for i in 0..if spectator || gallery { 0 } else { 2 } {
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
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if let Err(error) = advance(world, &mut state) {
            error!(%error,"Natural cave-structures acceptance failed");
            world.write_message(AppExit::error());
        } else if state.stage == state.end()
            && state.entered.elapsed() >= Duration::from_millis(250)
        {
            // Drain screenshot readbacks before closing the render-world channel.
            let spectator = std::env::var_os("TERRA_CAVE_SPECTATOR").is_some();
            info!(
                spectator,
                inspection_lamps = if spectator || state.salt_gallery {
                    0
                } else {
                    2
                },
                "Natural cave-structures acceptance passed"
            );
            world.write_message(AppExit::Success);
        }
    });
}

/// Inspect a real spilling lip from the lower terrace. The same outlet API
/// locates cells for the gravity/retention regression tests.
fn falls_view(terrain: &TerraGenerator, room: CaveStructure) -> Option<(Vec3, Vec3)> {
    let center = Vec3::from_array(room.center.map(|v| v as f32));
    let [x, z] = room.river_flow(room.center[0]);
    let downstream = Vec3::new(x, 0., z).try_normalize()?;
    let bank = downstream.cross(Vec3::Y);
    for distance in (0..room.radius.ceil() as i32 / 2).map(|d| d as f32) {
        for across in [0., 2., -2., 4., -4.] {
            let source = center + downstream * distance + bank * across;
            let x = f64::from(source.x.floor()) + 0.5;
            let z = f64::from(source.z.floor()) + 0.5;
            let column = terrain.cave_structure_at(x, z);
            let Some([wx, wy, wz]) = column.fluid_outlet(x, z) else {
                continue;
            };
            let lower = terrain.cave_structure_at(wx as f64 + 0.5, wz as f64 + 0.5);
            if lower.floor > wy as f64 - 4.5 {
                continue;
            }
            let mouth = Vec3::new(wx as f32 + 0.5, wy as f32 + 0.5, wz as f32 + 0.5);
            let target = mouth - Vec3::Y * 1.4;
            for offset in [5., -5., 2., -2.] {
                for reach in [13., 10., 7.] {
                    let mut eye = mouth + downstream * reach + bank * offset;
                    let bank = terrain.cave_structure_at(f64::from(eye.x), f64::from(eye.z));
                    eye.y = bank.water.map_or(bank.floor, |w| w.level.max(bank.floor)) as f32 + 2.3;
                    if eye.y > mouth.y + 1. {
                        continue;
                    }
                    if (0..=10).all(|step| {
                        let p = eye
                            .lerp(target, step as f32 / 10.)
                            .to_array()
                            .map(f64::from);
                        terrain.sample(p).density > 0.2
                    }) {
                        return Some((eye, target));
                    }
                }
            }
        }
    }
    None
}

/// Frame the bank and its opposite terrace along an open reach of the stream.
/// Reject intersecting camera paths using generated terrain, including arches.
fn gallery_view(
    terrain: &TerraGenerator,
    room: CaveStructure,
    stage: usize,
) -> Option<(Vec3, Vec3)> {
    if room.kind == CaveEcology::Lush
        && stage.is_multiple_of(2)
        && let Some(framing) = falls_view(terrain, room)
    {
        return Some(framing);
    }
    let center = Vec3::from_array(room.center.map(|v| v as f32));
    let [x, z] = room.river_flow(room.center[0]);
    let flow = Vec3::new(x, 0., z).try_normalize()?;
    let bank = flow.cross(Vec3::Y);
    let sign = if stage.is_multiple_of(2) { 1.0 } else { -1.0 };
    for lateral in [0.15, -0.13, 0.03, 0.24, -0.28] {
        for distance in [0.58, 0.45, 0.32] {
            let mut eye = center + (flow * distance + bank * lateral) * sign * room.radius as f32;
            let mut target =
                center - (flow * 0.22 + bank * lateral * 0.32) * sign * room.radius as f32;
            let column = |p: Vec3| terrain.cave_structure_at(f64::from(p.x), f64::from(p.z));
            let local = column(eye);
            let far = column(target);
            eye.y = local
                .water
                .map_or(local.floor, |w| w.level.max(local.floor)) as f32
                + 3.5;
            target.y = far.water.map_or(far.floor, |w| w.level.max(far.floor)) as f32 + 2.8;
            if !local.active() || !far.active() || eye.y > local.ceiling as f32 - 2. {
                continue;
            }
            let visible = (0..=12).all(|step| {
                let point = eye.lerp(target, step as f32 / 12.);
                let sample = terrain.sample(point.to_array().map(f64::from));
                sample.density > 1.0
                    && (step > 2 || sample.material == terra_voxel::MaterialId::Air as u16)
            });
            if visible {
                return Some((eye, target));
            }
        }
    }
    None
}

fn advance(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if let Some(error) = state.failure.take() {
        return Err(error);
    }
    if state.started.elapsed() > Duration::from_secs(if state.shell { 110 } else { 240 }) {
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
    if state.salt_gallery
        && let Some(([x, z], _)) = room.salt_skylight(room.center[1])
    {
        let portal = Vec3::new(x as f32, center.y, z as f32);
        let inward = (portal - center).normalize_or_zero();
        let side = inward.cross(Vec3::Y);
        let offset = if state.stage == 0 {
            -inward * 0.48 + side * 0.21
        } else {
            -inward * 0.27 - side * 0.49
        };
        let direction = room
            .gallery_axis()
            .map(|(_, axis)| Vec3::new(axis[0] as f32, 0., axis[1] as f32));
        eye = if let Some(along) = direction {
            let bank = along.cross(Vec3::Y);
            if state.stage == 0 {
                center + (-along * 0.93 - bank * 0.17) * room.radius as f32
            } else {
                center + (-along * 0.56 + bank * 0.12) * room.radius as f32
            }
        } else {
            center + offset * room.radius as f32
        };
        let bank = terrain.cave_structure_at(f64::from(eye.x), f64::from(eye.z));
        eye.y = bank.floor as f32 + if state.stage == 0 { 4.0 } else { 2.2 };
        if let Some(water) = bank.water {
            eye.y = eye.y.max(water.level as f32 + 2.0);
        }
        target = portal;
        target.y = room.floor as f32 + (room.ceiling - room.floor) as f32 * 0.40;
    }
    if state.shell {
        eye = center + Vec3::new(-0.14, 0.0, 0.12) * room.radius as f32;
        let local = terrain.cave_structure_at(f64::from(eye.x), f64::from(eye.z));
        eye.y = local.floor as f32 + 5.0;
        if let Some(water) = local.water {
            eye.y = eye.y.max(water.level as f32 + 2.0);
        }
        target = center + Vec3::new(0.0, 0.0, -room.radius as f32 * 0.24);
        target.y = if state.stage == 0 {
            local.floor as f32 + 4.0
        } else {
            room.ceiling as f32 - 1.0
        };
    }
    if !state.benchmark
        && let Some(water) = local.water
    {
        eye.y = eye.y.max(water.level as f32 + 2.0);
    }
    if room.dynamic_water
        && terrain.config().cave_structure_revision >= 12
        && matches!(room.kind, CaveEcology::Lush | CaveEcology::Waterfall)
        && state.connector.is_none()
    {
        if state
            .gallery_view
            .is_none_or(|(stage, _, _)| stage != state.stage)
        {
            let (framed_eye, framed_target) =
                gallery_view(terrain, room, state.stage).unwrap_or((eye, target));
            state.gallery_view = Some((state.stage, framed_eye, framed_target));
        }
        if let Some((_, framed_eye, framed_target)) = state.gallery_view {
            eye = framed_eye;
            target = framed_target;
        }
    }
    if let Some(view) = &state.connector {
        eye = view.eye;
        target = view.target;
        center = eye;
    }
    crate::reef_smoke::set_camera(
        world,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
        if state.shell { eye } else { center },
    );
    for (mut transform, lamp) in world
        .query::<(&mut Transform, &InspectionLamp)>()
        .iter_mut(world)
    {
        transform.translation = if state.connector.is_some() {
            eye + Vec3::Y * 0.3
        } else {
            let lamp_y = if state.shell && state.stage == 1 {
                room.ceiling as f32 - 8.0
            } else {
                center.y - 4.0
            };
            center
                + Vec3::new(
                    if lamp.0 == 0 { -12.0 } else { 12.0 },
                    lamp_y - center.y,
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
    if state.shell {
        // Check real generated geometry, not only a darkened background: the
        // entire centre-column floor/roof envelope must be loaded and meshed.
        let column = [
            (room.center[0].floor() as i64).div_euclid(16),
            (room.center[2].floor() as i64).div_euclid(16),
        ];
        let low = ((room.floor - 1.0).floor() as i64).div_euclid(16).max(-8);
        let high = ((room.ceiling + 1.0).ceil() as i64).div_euclid(16).min(22);
        for y in low..=high {
            let p = terra_voxel::SectionPos::new(column[0], y, column[1]);
            if !chunks.loaded.contains_key(&p) {
                return Err(format!("Missing actual tall cave shell section {p:?}"));
            }
        }
        for y in [
            room.floor.floor() as i64 - 2,
            room.ceiling.ceil() as i64 + 2,
        ] {
            let p = [
                room.center[0].floor() as i64,
                y,
                room.center[2].floor() as i64,
            ];
            if !chunks
                .block_at(p)
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(terra_voxel::MaterialId::is_solid)
            {
                return Err(format!(
                    "Natural cave shell at {p:?} has no solid roof/floor"
                ));
            }
        }
    }
    let mut counts = [0; 7];
    let mut salt_props = 0;
    for &material in chunks.loaded.values().flat_map(|s| s.data.blocks()) {
        salt_props += usize::from((459..=463).contains(&material));
        if (50..=56).contains(&material) {
            counts[(material - 50) as usize] += 1;
        }
    }
    let water = world.resource::<terra_render::WaterStatus>();
    let spray = water.spray_particles;
    let water = water.visible_tiles;
    if state.salt_gallery {
        info!(cave = ?world.resource::<terra_render::CaveState>(), "Salt gallery light sample");
    }
    info!(stage=state.stage,kind=?room.kind,?counts,salt_props,water_tiles=water,spray_particles=spray,loaded=chunks.loaded.len(),"Natural cave capture");
    if state.connector.is_none()
        && (room.kind == CaveEcology::Salt && counts[1] + salt_props == 0
            || state.salt_gallery && (salt_props == 0 || water == 0)
            || room.kind == CaveEcology::Waterfall && water == 0)
    {
        return Err("Missing cave mineral/water geometry".into());
    }
    state.requested = true;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(state.folder.join(
            state.connector.as_ref().map_or_else(
                || {
                    if state.shell {
                        format!("cave-natural-shell-{}.png", state.stage)
                    } else {
                        format!("cave-{:?}-{}.png", room.kind, state.stage % 2)
                    }
                },
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
