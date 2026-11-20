//! Short offline gallery exercising the actual batched prop shader and lifecycle.
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    mesh::VertexAttributeValues,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, Season, WorldClock};
use terra_render::{PropRenderStatus, WindProp};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp, seasonal::flower_rank},
    streaming::{ChunkFocus, ChunkStore, LoadedSection, SectionPublished, StreamingSettings},
};

const OWNER: SectionPos = SectionPos::new(0, 2, 1);

#[derive(Clone, Copy, Debug, Default)]
struct Pixels {
    plant: usize,
    brown: usize,
}

#[derive(Resource)]
struct Inspection {
    started: Instant,
    wither_started: Option<Instant>,
    pending: bool,
    captures: usize,
    failure: Option<String>,
    pixels: [Pixels; 3],
    meshes: HashSet<AssetId<Mesh>>,
    geometry: u64,
    jobs: usize,
    finish_frames: u8,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-flower-wither") {
        return;
    }
    {
        let mut config = app.world_mut().resource_mut::<GameConfig>();
        config.world.seasons = terra_core::SeasonConfig::default();
        config.client.fov_degrees = 45.0;
        config.client.weather = terra_core::WeatherKind::Clear;
        config.client.weather_intensity = 0.0;
    }
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Inspection {
            started: Instant::now(),
            wither_started: None,
            pending: false,
            captures: 0,
            failure: None,
            pixels: [Pixels::default(); 3],
            meshes: HashSet::new(),
            geometry: 0,
            jobs: 0,
            finish_frames: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            pin.after(terra_player::PlayerSet::Movement)
                .before(terra_world::streaming::StreamingSet::Interest),
        )
        .add_systems(Last, (retain_cpu_copy, run).chain());
}

fn season_clock(world: &World, season: Season) -> WorldClock {
    let config = world.resource::<GameConfig>().world.seasons;
    let lengths = config.lengths();
    let year: u64 = lengths.into_iter().map(u64::from).sum();
    let offset: u64 = lengths[..config.start_season.index()]
        .iter()
        .map(|&d| u64::from(d))
        .sum();
    let start: u64 = lengths[..season.index()]
        .iter()
        .map(|&d| u64::from(d))
        .sum();
    WorldClock {
        day: (start + year - offset
            + if season == Season::Autumn {
                u64::from(lengths[season.index()] - 1)
            } else {
                0
            })
            % year,
        phase: 0.5,
        paused: true,
        ..Default::default()
    }
}

fn setup(world: &mut World) {
    // Only this opt-in offline gallery suppresses unrelated generation work.
    {
        let mut settings = world.resource_mut::<StreamingSettings>();
        settings.radius = 0;
        settings.vertical_radius = 0;
        settings.max_jobs = 0;
        settings.meshes = false;
    }
    world.resource_mut::<ChunkFocus>().0 = [8.0, 33.0, 18.0];
    let clock = season_clock(world, Season::Spring);
    *world.resource_mut::<WorldClock>() = clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    world
        .resource_mut::<terra_core::WorldWeather>()
        .set_pattern(terra_core::WeatherPattern::Clear);
    let mut props = Vec::new();
    for id in 1..100 {
        let prop = WorldProp {
            id: PropId(0x666c_6f77_6572_0000 + id),
            kind: PropKind::Flower,
            position: [
                7.4 + (props.len() % 3) as f64 * 0.6,
                33.0,
                17.5 + (props.len() / 3) as f64 * 0.5,
            ],
            rotation: 0.0,
            scale: 0.65,
            variant: 3,
            section: OWNER,
            removed: false,
        };
        if flower_rank(&prop) < 0.90 {
            props.push(prop);
        }
        if props.len() == 9 {
            break;
        }
    }
    let mut data = BlockSection::empty(OWNER).expect("bounded fixture section");
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 0, z], 3).expect("fixture floor");
        }
    }
    world.resource_mut::<ChunkStore>().loaded.insert(
        OWNER,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props,
        },
    );
    world.write_message(SectionPublished { position: OWNER });
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(12.0, 7.0, 0.1));
    let material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.035, 0.055, 0.10),
            unlit: true,
            fog_enabled: false,
            ..default()
        });
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        bevy::light::NotShadowCaster,
        Transform::from_xyz(8.0, 35.0, 20.0),
    ));
    world.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 36.0, 12.0).looking_at(Vec3::new(8.0, 33.0, 18.0), Vec3::Y),
    ));
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
}

fn pin(world: &mut World) {
    let view =
        Transform::from_xyz(8.0, 34.0, 14.0).looking_at(Vec3::new(8.0, 33.35, 18.0), Vec3::Y);
    world.resource_mut::<ChunkFocus>().0 = [8.0, 33.0, 18.0];
    for mut transform in world
        .query_filtered::<&mut Transform, Or<(
            With<terra_player::TerraCamera>,
            With<terra_player::PlayerBody>,
        )>>()
        .iter_mut(world)
    {
        *transform = view;
    }
}

fn geometry(world: &mut World) -> Result<(HashSet<AssetId<Mesh>>, u64), String> {
    let handles: Vec<_> = world
        .query_filtered::<&Mesh3d, With<WindProp>>()
        .iter(world)
        .map(|mesh| mesh.0.clone())
        .collect();
    if handles.len() != 1 {
        return Err(format!(
            "expected one section-batched flower mesh, got {}",
            handles.len()
        ));
    }
    let assets = world.resource::<Assets<Mesh>>();
    let mesh = assets.get(&handles[0]).ok_or("flower mesh absent")?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut anchored = false;
    for (attribute, values) in mesh.attributes() {
        if let VertexAttributeValues::Float32x3(points) = values {
            if attribute.name == "Terra_GrowthAnchor" {
                anchored = true;
                if points.len() != mesh.count_vertices() {
                    return Err("incomplete flower root anchors".into());
                }
            }
            for point in points {
                point.map(f32::to_bits).hash(&mut hash);
            }
        }
    }
    if !anchored {
        return Err("actual prop mesh lacks its rooted shader layout".into());
    }
    Ok((handles.iter().map(Handle::id).collect(), hash.finish()))
}

fn retain_cpu_copy(world: &mut World) {
    let handles: Vec<_> = world
        .query_filtered::<&Mesh3d, With<WindProp>>()
        .iter(world)
        .map(|mesh| mesh.0.clone())
        .collect();
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    for handle in handles {
        if meshes
            .get(&handle)
            .is_some_and(|mesh| !mesh.asset_usage.contains(RenderAssetUsages::MAIN_WORLD))
            && let Some(mut mesh) = meshes.get_mut(&handle)
        {
            // Opt-in inspection only, before the mesh's first render extraction.
            mesh.asset_usage |= RenderAssetUsages::MAIN_WORLD;
        }
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if let Err(error) = advance(world, &mut state) {
            error!(%error, captures=state.captures, "Native flower withering fixture failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if let Some(error) = &state.failure {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(25) {
        return Err(format!("timed out after {} captures", state.captures));
    }
    if state.pending {
        return Ok(());
    }
    if state.captures == 3 {
        state.finish_frames += 1;
        if state.finish_frames >= 3 {
            info!(?state.pixels, "Native flower withering passed: actual GPU brown/shrink/disappearance, unchanged roots/mesh, bounded 10s real clock");
            world.write_message(AppExit::Success);
        }
        return Ok(());
    }
    let status = world.resource::<PropRenderStatus>();
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    if !status.ready
        || status.pending_jobs != 0
        || status.visible_props != 9
        || state.started.elapsed() < Duration::from_secs(3)
    {
        return Ok(());
    }
    let (meshes, hash) = geometry(world)?;
    if state.meshes.is_empty() {
        state.meshes = meshes;
        state.geometry = hash;
        state.jobs = world.resource::<PropRenderStatus>().completed_jobs;
    } else if state.meshes != meshes
        || state.geometry != hash
        || state.jobs != world.resource::<PropRenderStatus>().completed_jobs
    {
        return Err("seasonal withering rebuilt or replaced its retained flower batch".into());
    }
    if state.captures == 1 && state.wither_started.is_none() {
        let clock = season_clock(world, Season::Autumn);
        *world.resource_mut::<WorldClock>() = clock;
        world
            .resource_mut::<EditJournal>()
            .bypass_change_detection()
            .clock = Some(clock);
        state.wither_started = Some(Instant::now());
        return Ok(());
    }
    if state.captures > 0 {
        let threshold = if state.captures == 1 { 5.0 } else { 10.2 };
        if state.wither_started.unwrap().elapsed().as_secs_f32() < threshold {
            return Ok(());
        }
    }
    let journal = world.resource::<EditJournal>();
    let season = world
        .resource::<WorldClock>()
        .season(world.resource::<GameConfig>().world.seasons);
    let props = &world
        .resource::<ChunkStore>()
        .loaded
        .get(&OWNER)
        .ok_or("fixture section evicted")?
        .props;
    if props.len() != 9
        || props
            .iter()
            .any(|p| p.removed || journal.prop_removed(OWNER, p.id))
        || journal.edited_blocks() != 0
        || journal.mesh_epoch(OWNER) != 0
        || journal.prop_epoch(OWNER) != 0
    {
        return Err("seasonal animation altered persistent prop identities or world edits".into());
    }
    for prop in props {
        let phase = journal.flowers.progress(prop, season);
        if state.captures == 0 && phase != 0.0
            || state.captures == 1 && !(0.45..0.62).contains(&phase)
            || state.captures == 2 && phase != 1.0
        {
            return Err(format!(
                "unexpected real-time flower phase {phase} at capture {}",
                state.captures
            ));
        }
    }
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-flower-wither"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let stage = state.captures;
    let name = ["0s-alive", "5s-brown-shrunk", "10s-dormant"][stage];
    state.pending = true;
    world.spawn(Screenshot::primary_window()).observe(save_to_disk(directory.join(format!("flower-{name}.png")))).observe(move |capture: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
        match pixels(&capture.image) {
            Ok(sample) => {
                let live = state.pixels[0];
                let okay = match stage {
                    0 => sample.plant > 300,
                    1 => sample.plant > 10 && sample.plant * 4 < live.plant * 3 && sample.brown as f64 / sample.plant as f64 > live.brown as f64 / live.plant as f64 + 0.20,
                    _ => sample.plant < live.plant / 20 + 30,
                };
                info!(stage, ?sample, ?live, okay, "Native flower GPU pixels measured");
                if !okay { state.failure = Some(format!("GPU withering mismatch stage={stage} sample={sample:?} baseline={live:?}")); }
                state.pixels[stage] = sample;
                state.captures += 1;
            }
            Err(error) => state.failure = Some(error),
        }
        state.pending = false;
    });
    Ok(())
}

fn pixels(image: &Image) -> Result<Pixels, String> {
    let rgba = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?
        .into_rgba8();
    if rgba.width() < 320 || rgba.height() < 180 {
        return Err("capture too small for GPU plant inspection".into());
    }
    let mut sample = Pixels::default();
    for y in rgba.height() / 5..rgba.height() * 4 / 5 {
        for x in rgba.width() / 5..rgba.width() * 4 / 5 {
            let pixel = rgba.get_pixel(x, y).0;
            let [r, g, b] = [pixel[0], pixel[1], pixel[2]].map(f64::from);
            if r.max(g) > 25.0 && (r > b * 1.25 || g > b * 1.15) {
                sample.plant += 1;
                sample.brown += usize::from(r > g * 1.10 && g > b * 1.10);
            }
        }
    }
    Ok(sample)
}
