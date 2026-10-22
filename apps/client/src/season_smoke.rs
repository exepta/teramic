//! Opt-in native season captures using one unchanged generated forest view.
use bevy::{
    asset::AssetId,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, Season, WeatherPattern, WorldClock, WorldWeather};
use terra_player::{ControllerMode, PlayerBody, TerraCamera};
use terra_render::{FoliageStatus, RenderStatus, RenderedSections};
use terra_voxel::SectionPos;
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{
        ChunkFocus, ChunkStore, GenerationStatus, StreamingSet, WorldGeneration, WorldTerrain,
    },
};

const SEASONS: [(Season, &str); 7] = [
    (Season::Spring, "spring"),
    (Season::Summer, "summer"),
    (Season::Autumn, "autumn"),
    (Season::Winter, "winter"),
    (Season::Winter, "winter-snow"),
    (Season::Spring, "spring-melt-mid"),
    (Season::Spring, "spring-melt"),
];

struct Geometry {
    blocks: HashMap<SectionPos, u64>,
    meshes: HashMap<SectionPos, Vec<AssetId<Mesh>>>,
    mesh_assets: usize,
    completed_jobs: usize,
    leaves: usize,
    grass: usize,
    edited_blocks: usize,
}

#[derive(Resource)]
struct Smoke {
    started: Instant,
    since: Instant,
    stage: usize,
    captures: usize,
    requested: bool,
    failure: Option<String>,
    view: Option<Transform>,
    anchor: Vec3,
    baseline: Option<Geometry>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-seasons") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Smoke {
            started: now,
            since: now,
            stage: 0,
            captures: 0,
            requested: false,
            failure: None,
            view: None,
            anchor: Vec3::ZERO,
            baseline: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            pin_view
                .after(terra_player::PlayerSet::Movement)
                .before(StreamingSet::Interest),
        )
        .add_systems(Last, run);
}

fn forest_view(terrain: &TerraGenerator) -> Option<(Vec3, Transform)> {
    let candidate = |x: f64, z: f64| {
        let biome = terrain.biome_at(x, z);
        let height = terrain.surface_height(x, z);
        if biome.weights.forest < 0.35
            || biome.weights.forest + biome.weights.plains < 0.80
            || biome.aridity() > 0.10
            || height >= terrain.config().snow_height - 12.0
            || terrain.forest_probability(x, z) < 0.20
            || terrain.water_column(x, z).is_some()
        {
            return None;
        }
        let anchor = Vec3::new(x as f32, height as f32, z as f32);
        let mut eye = anchor + Vec3::new(25.0, 11.0, 28.0);
        eye.y = eye
            .y
            .max(terrain.surface_height(f64::from(eye.x), f64::from(eye.z)) as f32 + 6.0);
        Some((
            anchor,
            Transform::from_translation(eye).looking_at(anchor + Vec3::Y * 4.0, Vec3::Y),
        ))
    };
    if let Some(view) = candidate(-256.0, 1536.0) {
        return Some(view);
    }
    // A bounded climate search supports seeds other than the default fixture.
    for ring in 0_i32..=32 {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(z.abs()) == ring
                    && let Some(view) = candidate(f64::from(x) * 128.0, f64::from(z) * 128.0)
                {
                    return Some(view);
                }
            }
        }
    }
    None
}

fn setup(world: &mut World) {
    let config = world.resource::<GameConfig>();
    if !config.world.seasons.enabled || !config.world.biomes || !config.world.trees {
        world.resource_mut::<Smoke>().failure =
            Some("Season fixture requires enabled seasons, biomes and trees".into());
        return;
    }
    let terrain = world.resource::<WorldTerrain>().0.clone();
    let Some((anchor, view)) = forest_view(&terrain) else {
        world.resource_mut::<Smoke>().failure =
            Some("No temperate forest/plains view found within the bounded climate search".into());
        return;
    };
    let biome = terrain.biome_at(f64::from(anchor.x), f64::from(anchor.z));
    info!(?anchor, eye = ?view.translation, forest = biome.weights.forest, plains = biome.weights.plains,
        temperature = biome.temperature, humidity = biome.humidity, "Season fixture located generated forest");
    {
        let mut smoke = world.resource_mut::<Smoke>();
        smoke.anchor = anchor;
        smoke.view = Some(view);
    }
    world.resource_mut::<GameConfig>().client.fov_degrees = 55.0;
    if let Some(mut gizmos) = world.get_resource_mut::<bevy::gizmos::config::GizmoConfigStore>() {
        gizmos
            .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
            .0
            .enabled = false;
    }
    *world.resource_mut::<WorldWeather>() = WorldWeather::new(terrain.config().seed.0);
    set_season(world, 0);
}

fn set_season(world: &mut World, stage: usize) {
    let seasons = world.resource::<GameConfig>().world.seasons;
    let lengths = seasons.lengths();
    let year: u64 = lengths.into_iter().map(u64::from).sum();
    let offset: u64 = lengths[..seasons.start_season.index()]
        .iter()
        .map(|days| u64::from(*days))
        .sum();
    let start: u64 = lengths[..SEASONS[stage].0.index()]
        .iter()
        .map(|days| u64::from(*days))
        .sum();
    let clock = {
        let mut clock = world.resource_mut::<WorldClock>();
        clock.day = (start + year - offset) % year;
        clock.phase = 0.5;
        clock.paused = true;
        *clock
    };
    if matches!(stage, 4 | 5) {
        let seed = world.resource::<GameConfig>().world.seed.0;
        let mut weather = world.resource_mut::<WorldWeather>();
        weather.set_pattern(if stage == 4 {
            WeatherPattern::Snow
        } else {
            WeatherPattern::Clear
        });
        weather.duration_seconds = 3600.0;
        weather.from_intensity = if stage == 4 { 1.0 } else { 0.0 };
        weather.target_intensity = weather.from_intensity;
        weather.wetness = weather.from_intensity;
        // The same bounded authoritative catch-up used by reconnect/sleep.
        weather.advance_seasonal(600.0, seed, clock, seasons);
    }
}

type SeasonObservers<'w, 's> =
    Query<'w, 's, &'static mut Transform, Or<(With<TerraCamera>, With<PlayerBody>)>>;

fn pin_view(smoke: Res<Smoke>, mut focus: ResMut<ChunkFocus>, mut transforms: SeasonObservers) {
    let Some(view) = smoke.view else { return };
    focus.0 = view.translation.to_array().map(f64::from);
    for mut transform in &mut transforms {
        *transform = view;
    }
}

fn geometry(world: &World, eye: Vec3) -> Geometry {
    let chunks = world.resource::<ChunkStore>();
    let mut blocks = HashMap::new();
    let (mut leaves, mut grass) = (0, 0);
    for (position, loaded) in &chunks.loaded {
        if (position.x as f32 * 16.0 - eye.x).abs() > 64.0
            || (position.z as f32 * 16.0 - eye.z).abs() > 64.0
        {
            continue;
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        loaded.data.blocks().hash(&mut hash);
        blocks.insert(*position, hash.finish());
        for &material in loaded.data.blocks() {
            leaves += usize::from((7..=9).contains(&material));
            grass += usize::from(material == 1);
        }
    }
    let meshes = world
        .resource::<RenderedSections>()
        .sections
        .iter()
        .filter(|(position, _)| blocks.contains_key(*position))
        .map(|(position, rendered)| (*position, rendered.meshes.iter().map(Handle::id).collect()))
        .collect();
    Geometry {
        blocks,
        meshes,
        mesh_assets: world.resource::<Assets<Mesh>>().len(),
        completed_jobs: world.resource::<RenderStatus>().completed_mesh_jobs,
        leaves,
        grass,
        edited_blocks: world.resource::<EditJournal>().edited_blocks(),
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut smoke: Mut<Smoke>| {
        if smoke.stage >= SEASONS.len() {
            return;
        }
        if let Err(error) = advance(world, &mut smoke) {
            error!(%error, "Native season fixture failed");
            smoke.stage = SEASONS.len();
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, smoke: &mut Smoke) -> Result<(), String> {
    if let Some(error) = &smoke.failure {
        return Err(error.clone());
    }
    if smoke.started.elapsed() > Duration::from_secs(180) {
        return Err("Timed out waiting for native season captures".into());
    }
    let status = world.resource::<RenderStatus>();
    if let Some(error) = &status.error {
        return Err(error.clone());
    }
    if !status.ready || world.resource::<WorldGeneration>().status != GenerationStatus::Ready {
        smoke.since = Instant::now();
        return Ok(());
    }
    if smoke.requested {
        if smoke.captures > smoke.stage {
            smoke.stage += 1;
            if smoke.stage == SEASONS.len() {
                info!(
                    captures = smoke.captures,
                    "Native season fixture passed: four seasons, snowfall and animated melt with unchanged retained terrain blocks"
                );
                world.write_message(AppExit::Success);
            } else {
                set_season(world, smoke.stage);
                smoke.since = Instant::now();
                smoke.requested = false;
            }
        }
        return Ok(());
    }
    let settle = match smoke.stage {
        4 => Duration::from_secs(5),
        5 => Duration::from_millis(500),
        _ => Duration::from_secs(3),
    };
    if smoke.since.elapsed() < settle {
        return Ok(());
    }
    let clock = *world.resource::<WorldClock>();
    let seasons = world.resource::<GameConfig>().world.seasons;
    let sample = clock.season(seasons);
    if sample.season != SEASONS[smoke.stage].0 || !clock.paused || clock.phase != 0.5 {
        return Err(
            "Native fixture calendar did not remain at the requested season and paused noon".into(),
        );
    }
    let view = smoke.view.ok_or("Season fixture has no generated view")?;
    let geometry = geometry(world, view.translation);
    if geometry.leaves == 0 || geometry.grass == 0 || geometry.meshes.is_empty() {
        return Err(
            "Generated season view does not contain loaded crowns, grass and terrain meshes".into(),
        );
    }
    let (mut retained_blocks, mut unchanged_meshes, mut rebuilt_meshes) = (0, 0, 0);
    if let Some(baseline) = &smoke.baseline {
        for (position, hash) in &geometry.blocks {
            if let Some(previous) = baseline.blocks.get(position) {
                retained_blocks += 1;
                if previous != hash {
                    return Err(format!(
                        "Season change altered generated voxel data in {position:?}"
                    ));
                }
            }
        }
        if retained_blocks == 0 || geometry.edited_blocks != baseline.edited_blocks {
            return Err(
                "Season capture lost the retained terrain or changed player block edits".into(),
            );
        }
        for (position, handles) in &geometry.meshes {
            if let Some(previous) = baseline.meshes.get(position) {
                if handles == previous {
                    unchanged_meshes += 1;
                } else {
                    rebuilt_meshes += 1;
                }
            }
        }
    }
    let terrain = &world.resource::<WorldTerrain>().0;
    let local = terra_world::weather::sample_at(
        world.resource::<WorldWeather>(),
        terrain,
        smoke.anchor.to_array().map(f64::from),
        clock,
    );
    let mut wet_probe = *world.resource::<WorldWeather>();
    wet_probe.from_intensity = 1.0;
    wet_probe.target_intensity = 1.0;
    let wet_probe = terra_world::weather::sample_at(
        &wet_probe,
        terrain,
        smoke.anchor.to_array().map(f64::from),
        clock,
    );
    let foliage = world.resource::<FoliageStatus>();
    info!(season = SEASONS[smoke.stage].1, day = clock.day, temperature = local.temperature, leaf_loss = sample.leaf_loss(),
        rain = local.rain, snow = local.snow, probe_rain = wet_probe.rain, probe_snow = wet_probe.snow,
        leaf_tint = ?sample.leaf_tint(), grass_tint = ?sample.grass_tint(), crowns = geometry.leaves, grass = geometry.grass,
        loaded_sections = world.resource::<ChunkStore>().loaded.len(), retained_blocks, unchanged_meshes, rebuilt_meshes,
        mesh_assets = geometry.mesh_assets, baseline_mesh_assets = smoke.baseline.as_ref().map(|base| base.mesh_assets),
        mesh_jobs = geometry.completed_jobs, baseline_mesh_jobs = smoke.baseline.as_ref().map(|base| base.completed_jobs),
        leaf_materials = foliage.material_variants, falling_leaves = foliage.falling_leaves,
        "Native season capture state");
    if smoke.baseline.is_none() {
        smoke.baseline = Some(geometry);
    }
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-seasons"));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let label = SEASONS[smoke.stage].1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            folder.join(format!("season-{}-{label}.png", smoke.stage)),
        ))
        .observe(
            move |capture: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| match image_samples(
                &capture.image,
            ) {
                Ok((samples, colors, bright, mean)) => {
                    info!(
                        season = label,
                        samples,
                        colors,
                        bright,
                        ?mean,
                        "Native season GPU pixels verified"
                    );
                    smoke.captures += 1;
                }
                Err(error) => smoke.failure = Some(error),
            },
        );
    smoke.requested = true;
    Ok(())
}

fn image_samples(image: &Image) -> Result<(usize, usize, usize, [f64; 3]), String> {
    let rgba = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?
        .into_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let mut colors = HashSet::new();
    let (mut samples, mut bright, mut total) = (0, 0, [0_u64; 3]);
    for (index, pixel) in rgba.as_raw().chunks_exact(4).enumerate().step_by(17) {
        let (x, y) = (index % width.max(1), index / width.max(1));
        if x < width / 8 || x >= width * 7 / 8 || y < height / 3 || y >= height * 7 / 8 {
            continue;
        }
        samples += 1;
        bright += usize::from(pixel[..3].iter().any(|channel| *channel > 25));
        colors.insert([pixel[0] / 8, pixel[1] / 8, pixel[2] / 8]);
        for axis in 0..3 {
            total[axis] += u64::from(pixel[axis]);
        }
    }
    if samples < 100 || colors.len() < 24 || bright * 10 < samples {
        return Err(format!(
            "Season screenshot is blank or lacks terrain detail: samples={samples}, colors={}, bright={bright}",
            colors.len()
        ));
    }
    Ok((
        samples,
        colors.len(),
        bright,
        total.map(|value| value as f64 / samples as f64),
    ))
}
