//! Opt-in views of actual generated settlements, never a separate showcase generator.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::{
    edits::EditJournal,
    generation::structures::{SiteSummary, StructureKind, VillageStyle},
    streaming::{ChunkFocus, ChunkStore, WorldTerrain},
};

#[derive(Resource)]
struct Fixture {
    scene: String,
    started: Instant,
    since: Instant,
    site: Option<SiteSummary>,
    views: [Transform; 2],
    captures: usize,
    pending: bool,
    drain_frames: u8,
    failed: Option<String>,
    done: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-villages") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Fixture {
            scene: std::env::var("TERRA_STRUCTURE_SCENE").unwrap_or_else(|_| "temperate".into()),
            started: now,
            since: now,
            site: None,
            views: [Transform::default(); 2],
            captures: 0,
            pending: false,
            drain_frames: 0,
            failed: None,
            done: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn matches_scene(site: &SiteSummary, scene: &str) -> bool {
    match scene {
        "temperate" => site.kind == StructureKind::Village && site.style == VillageStyle::Temperate,
        "desert" => site.kind == StructureKind::Village && site.style == VillageStyle::Desert,
        "highland" => site.kind == StructureKind::Village && site.style == VillageStyle::Highland,
        "ruin" => site.kind == StructureKind::Ruin,
        "mine" => site.kind == StructureKind::Mine,
        _ => false,
    }
}

fn setup(world: &mut World) {
    let terrain = world.resource::<WorldTerrain>().0.clone();
    let scene = world.resource::<Fixture>().scene.clone();
    let site = if let Ok(region) = std::env::var("TERRA_STRUCTURE_REGION") {
        let p: Vec<_> = region
            .split(',')
            .filter_map(|v| v.trim().parse::<i64>().ok())
            .collect();
        (p.len() == 2)
            .then(|| terrain.structure_site([p[0], p[1]]))
            .flatten()
            .filter(|s| matches_scene(s, &scene))
    } else {
        let mut found = None;
        'search: for radius in 0_i64..=12 {
            for z in -radius..=radius {
                for x in -radius..=radius {
                    if radius > 0 && x.abs() != radius && z.abs() != radius {
                        continue;
                    }
                    if let Some(site) = terrain.structure_site([x, z])
                        && matches_scene(&site, &scene)
                    {
                        found = Some(site);
                        break 'search;
                    }
                }
            }
        }
        found
    };
    let Some(site) = site else {
        world.resource_mut::<Fixture>().failed =
            Some(format!("No actual generated {scene} site found"));
        return;
    };
    let low = Vec3::from_array(site.bounds[0].map(|v| v as f32));
    let high = Vec3::from_array(site.bounds[1].map(|v| v as f32));
    let width = (high.x - low.x).max(high.z - low.z).max(20.0);
    let center = Vec3::from_array(site.center.map(|v| v as f32));
    let target = (low + high) * 0.5;
    let overview =
        Transform::from_translation(target + Vec3::new(width * 0.2, width, width * 0.65))
            .looking_at(target, Vec3::Y);
    let building = site
        .buildings
        .iter()
        .find(|b| b.role == terra_world::generation::structures::BuildingRole::Church)
        .or_else(|| site.buildings.first());
    let close = building.map_or(overview, |b| {
        let entrance = Vec3::from_array(b.entrance.map(|v| v as f32));
        let middle = Vec3::from_array(std::array::from_fn(|i| {
            (b.bounds[0][i] + b.bounds[1][i]) as f32 * 0.5
        }));
        let difference = entrance - middle;
        let outward = if difference.x.abs() > difference.z.abs() {
            Vec3::X * difference.x.signum()
        } else {
            Vec3::Z * difference.z.signum()
        };
        if scene == "mine" {
            return Transform::from_translation(entrance + Vec3::Y * 1.6 + outward * 1.3)
                .looking_at(entrance + Vec3::Y * 1.6 - outward * 12.0, Vec3::Y);
        }
        let mut eye = entrance + Vec3::Y * 7.0 + outward * 11.0;
        eye.y = eye
            .y
            .max(terrain.surface_height(f64::from(eye.x), f64::from(eye.z)) as f32 + 3.0);
        Transform::from_translation(eye).looking_at(entrance + Vec3::Y * 4.0, Vec3::Y)
    });
    let clock = terra_core::WorldClock {
        day: if scene == "highland" { 21 } else { 0 },
        phase: 0.5,
        paused: scene != "highland",
        ..Default::default()
    };
    *world.resource_mut::<terra_core::WorldClock>() = clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    {
        let mut weather = world.resource_mut::<terra_core::WorldWeather>();
        weather.set_pattern(if scene == "highland" {
            terra_core::WeatherPattern::Snow
        } else {
            terra_core::WeatherPattern::Clear
        });
        if scene == "highland" {
            weather.advance(8.0, terrain.config().seed.0);
        }
    }
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .fov_degrees = 55.0;
    world.resource_mut::<ChunkFocus>().0 = center.to_array().map(f64::from);
    info!(%scene, seed=terrain.config().seed.0, id=site.id, center=?site.center,
        bounds=?site.bounds, buildings=site.buildings.len(), containers=site.containers.len(),
        "Native structure fixture selected actual immutable generation plan");
    let mut state = world.resource_mut::<Fixture>();
    state.site = Some(site);
    state.views = [overview, close];
    state.since = Instant::now();
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.done { return; }
        let result = advance(world, &mut state);
        match result {
            Ok(true) => { info!(scene=%state.scene, captures=state.captures, "Native generated structure fixture passed");
                world.write_message(AppExit::Success); state.done = true; }
            Err(error) => { error!(%error, scene=%state.scene, "Native generated structure fixture failed");
                world.write_message(AppExit::error()); state.done = true; }
            _ => {}
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    if let Some(error) = &state.failed {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(110) {
        return Err("Structure fixture timed out".into());
    }
    let site = state.site.as_ref().ok_or("Missing actual generated site")?;
    let view = state.views[state.captures.min(1)];
    for mut t in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *t = view;
    }
    for mut t in world
        .query_filtered::<&mut Transform, With<PlayerBody>>()
        .iter_mut(world)
    {
        t.translation = view.translation;
    }
    world.resource_mut::<ChunkFocus>().0 = site.center.map(|v| v as f64);
    if state.pending {
        return Ok(false);
    }
    if state.captures == 2 {
        state.drain_frames = state.drain_frames.saturating_add(1);
        return Ok(state.drain_frames >= 3 && state.since.elapsed() >= Duration::from_millis(500));
    }
    let render = world.resource::<terra_render::RenderStatus>();
    if let Some(error) = &render.error {
        return Err(error.clone());
    }
    if !render.ready
        || state.since.elapsed()
            < Duration::from_secs(if state.scene == "highland" && state.captures == 0 {
                16
            } else {
                3
            })
    {
        return Ok(false);
    }
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    for &position in &site.containers {
        if chunks.block_at(position) != Some(29) || journal.container_at(position).is_none() {
            return Ok(false);
        }
    }
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-structures"));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let name = format!(
        "{}-{}.png",
        state.scene,
        if state.captures == 0 {
            "overview"
        } else {
            "close"
        }
    );
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(name)))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Fixture>| {
                if let Ok(image) = event.image.clone().try_into_dynamic() {
                    let rgba = image.into_rgba8();
                    let visible = rgba
                        .pixels()
                        .filter(|p| p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]) > 20)
                        .count();
                    if visible < 5000 {
                        state.failed = Some("Generated structure screenshot is blank".into());
                    }
                } else {
                    state.failed = Some("Cannot read native structure screenshot".into());
                }
                state.pending = false;
                state.captures += 1;
                state.since = Instant::now();
            },
        );
    state.pending = true;
    Ok(false)
}
