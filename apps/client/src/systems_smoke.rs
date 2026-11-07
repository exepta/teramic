//! Explicit native acceptance fixtures; no test world is created in normal gameplay.
use bevy::{app::AppExit, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{CameraPerspective, PlayerBody, PlayerGrounded, TerraCamera};
use terra_world::{
    environment::{SurfaceCondition, SurfaceConditionRequest},
    streaming::{ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration},
};
#[derive(Resource)]
struct Fixture {
    started: Instant,
    since: Instant,
    phase: u8,
    third: bool,
    target: Option<[i64; 3]>,
    frame_times: Vec<f64>,
    captured: bool,
    coast: Option<([f64; 3], [f64; 3])>,
    clearing_frames: u8,
}

#[derive(Resource, Default)]
struct CloudCaptureChecks {
    clear_sun: Option<Vec<f32>>,
    overcast_sun: Option<Vec<f32>>,
    layer_views: u8,
    error: Option<String>,
}

impl CloudCaptureChecks {
    fn verify_cover_change(&self) -> Result<bool, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let (Some(clear), Some(overcast)) = (&self.clear_sun, &self.overcast_sun) else {
            return Ok(false);
        };
        if clear.len() != overcast.len() || clear.is_empty() {
            return Err("Shader-cloud capture samples do not match".into());
        }
        let changes: Vec<_> = clear.iter().zip(overcast).map(|(a, b)| b - a).collect();
        let count = changes.len() as f32;
        let mean = changes.iter().sum::<f32>() / count;
        let difference = changes.iter().map(|change| change.abs()).sum::<f32>() / count;
        let spatial = (changes
            .iter()
            .map(|change| (change - mean).powi(2))
            .sum::<f32>()
            / count)
            .sqrt();
        if difference < 0.025 || spatial < 0.006 {
            return Err(format!(
                "Shader-cloud cover did not visibly change across the sky: mean difference={difference:.4}, spatial variation={spatial:.4}"
            ));
        }
        tracing::info!(
            difference,
            spatial,
            "Shader-cloud captures show spatially varying cover changes outside the sun disc"
        );
        Ok(true)
    }
}
pub(crate) fn install(app: &mut App) {
    let third = std::env::args().any(|arg| arg == "--smoke-test-perspective");
    if !third && !std::env::args().any(|arg| arg == "--smoke-test-weather") {
        return;
    }
    if third {
        app.insert_resource(terra_player::ControllerMode::Walk);
    } else {
        app.insert_resource(terra_render::WeatherPreview {
            kind: terra_core::WeatherKind::Clear,
            intensity: 1.0,
        });
    }
    let now = Instant::now();
    app.insert_resource(Fixture {
        started: now,
        since: now,
        phase: 0,
        third,
        target: None,
        frame_times: Vec::new(),
        captured: false,
        coast: None,
        clearing_frames: 0,
    })
    .init_resource::<CloudCaptureChecks>()
    .add_systems(
        Update,
        run.after(terra_player::PlayerSet::Movement)
            .after(terra_world::streaming::StreamingSet::Dispatch),
    );
}
fn capture(world: &mut World, name: &str) {
    if name.starts_with("cloud-deck-")
        && let Err(error) = verify_cloud_sky_rays(world)
    {
        world.resource_mut::<CloudCaptureChecks>().error = Some(format!("{name}: {error}"));
    }
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-systems"));
    if let Err(error) = std::fs::create_dir_all(&root) {
        tracing::error!(%error,"Cannot create capture directory");
    }
    let mut screenshot = world.spawn(bevy::render::view::screenshot::Screenshot::primary_window());
    screenshot.observe(bevy::render::view::screenshot::save_to_disk(
        root.join(format!("{name}.png")),
    ));
    if matches!(
        name,
        "cloud-deck-0-sun" | "cloud-deck-4-sun" | "shader-cloud-inside" | "shader-cloud-above"
    ) {
        let name = name.to_owned();
        screenshot.observe(move |capture: On<bevy::render::view::screenshot::ScreenshotCaptured>, mut checks: ResMut<CloudCaptureChecks>| {
            let Some(data) = capture.image.data.as_ref() else {
                checks.error = Some(format!("{name}: missing screenshot pixels"));
                return;
            };
            let size = capture.image.texture_descriptor.size;
            let format = capture.image.texture_descriptor.format;
            let bgra = matches!(format,
                bevy::render::render_resource::TextureFormat::Bgra8Unorm
                    | bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb);
            if !bgra && !matches!(format,
                bevy::render::render_resource::TextureFormat::Rgba8Unorm
                    | bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb)
            {
                checks.error = Some(format!("{name}: unsupported screenshot format {format:?}"));
                return;
            }
            let mut samples = Vec::with_capacity(96);
            for row in 0..8 {
                for column in 0..12 {
                    if (5..=6).contains(&column) && (4..=6).contains(&row) {
                        continue;
                    }
                    // Inspect the sky, leaving HUD edges and the central sun out.
                    let x = size.width * (column + 1) / 13;
                    let y = size.height * (row + 1) / 12;
                    let mut luminance = 0.0;
                    for dy in 0..3 {
                        for dx in 0..3 {
                            let offset = (((y + dy).min(size.height - 1) * size.width
                                + (x + dx).min(size.width - 1)) * 4) as usize;
                            if offset + 3 >= data.len() {
                                checks.error = Some(format!("{name}: truncated screenshot"));
                                return;
                            }
                            let pixel = &data[offset..offset + 4];
                            let (r, b) = if bgra { (pixel[2], pixel[0]) } else { (pixel[0], pixel[2]) };
                            luminance += (0.2126 * f32::from(r) + 0.7152 * f32::from(pixel[1]) + 0.0722 * f32::from(b)) / (9.0 * 255.0);
                        }
                    }
                    samples.push(luminance);
                }
            }
            if name == "cloud-deck-0-sun" {
                checks.clear_sun = Some(samples);
            } else if name == "cloud-deck-4-sun" {
                checks.overcast_sun = Some(samples);
            } else {
                let low = samples.iter().copied().fold(f32::INFINITY, f32::min);
                let high = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                if high - low < 0.008 {
                    checks.error = Some(format!("{name}: cloud-layer capture is spatially uniform ({low:.4}..{high:.4})"));
                    return;
                }
                checks.layer_views |= if name == "shader-cloud-inside" { 1 } else { 2 };
                tracing::info!(%name, low, high, "Shader-cloud layer view contains visible spatial detail");
            }
        });
    }
}

fn verify_cloud_sky_rays(world: &mut World) -> Result<(), String> {
    let (camera, projection) = world
        .query_filtered::<(&Transform, &Projection), With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|(camera, projection)| (*camera, projection.clone()))
        .ok_or("Missing cloud-capture camera")?;
    let Projection::Perspective(projection) = projection else {
        return Err("Cloud-capture sky check requires a perspective camera".into());
    };
    let chunks = world.resource::<ChunkStore>();
    let half_fov = (projection.fov * 0.5).tan();
    for row in 0..8 {
        for column in 0..12 {
            if (5..=6).contains(&column) && (4..=6).contains(&row) {
                continue;
            }
            let uv = Vec2::new((column + 1) as f32 / 13.0, (row + 1) as f32 / 12.0);
            let view_ray = Vec3::new(
                (uv.x * 2.0 - 1.0) * projection.aspect_ratio * half_fov,
                (1.0 - uv.y * 2.0) * half_fov,
                -1.0,
            );
            let ray = camera.rotation * view_ray.normalize();
            if ray.y <= 0.0 {
                return Err(format!("Cloud sample {column},{row} points below the sky"));
            }
            let mut distance = 0.0;
            while distance < projection.far {
                let reach = (projection.far - distance).min(128.0);
                let origin = camera.translation + ray * distance;
                if let Some(hit) = terra_voxel::blocks::raycast_blocks(
                    origin.to_array().map(f64::from),
                    ray.to_array().map(f64::from),
                    f64::from(reach),
                    // Only loaded geometry can be foreground in this capture.
                    |position| Some(chunks.block_at(position).unwrap_or(0)),
                ) {
                    return Err(format!(
                        "Cloud sample {column},{row} is blocked by foreground terrain at {:?}",
                        hit.position
                    ));
                }
                distance += reach;
            }
        }
    }
    tracing::info!(eye = ?camera.translation, "Cloud-capture sky samples are clear of foreground terrain");
    Ok(())
}
fn captured(world: &mut World, fixture: &mut Fixture, name: &str) -> bool {
    if fixture.captured {
        return true;
    }
    capture(world, name);
    fixture.captured = true;
    fixture.since = Instant::now();
    false
}
fn ready(world: &World) -> bool {
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && world.resource::<terra_render::RenderStatus>().ready
        && world.resource::<terra_render::PropRenderStatus>().ready
}

fn natural_coast(
    terrain: &terra_world::TerraGenerator,
    center: [f64; 3],
) -> Option<([f64; 3], [f64; 3])> {
    let sea = terrain.config().sea_level;
    for ring in 0..=96 {
        let steps = (ring * 8).max(12);
        for step in 0..steps {
            let angle = f64::from(step) / f64::from(steps) * std::f64::consts::TAU;
            let x = (center[0] + angle.cos() * f64::from(ring) * 24.0).floor();
            let z = (center[2] + angle.sin() * f64::from(ring) * 24.0).floor();
            let height = terrain.surface_height(x + 0.5, z + 0.5);
            let top = (height - 0.5).floor() + 1.0;
            if (top - sea).abs() > 0.65 {
                continue;
            }
            let surface = terrain.surface_column(x + 0.5, z + 0.5);
            if surface.water_level.is_some() {
                continue;
            }
            for [dx, dz] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                let water = terrain.surface_column(x + 0.5 + dx, z + 0.5 + dz);
                if water
                    .water_level
                    .is_some_and(|level| (level - sea).abs() < 0.01)
                    && water.biome.weights.ocean > 0.2
                {
                    let eye = [x + 0.5 - dx * 5.0, sea + 5.5, z + 0.5 - dz * 5.0];
                    let look = [x + 0.5 + dx * 6.0, sea + 0.2, z + 0.5 + dz * 6.0];
                    return Some((eye, look));
                }
            }
        }
    }
    None
}

fn frame_cloud_layer(world: &mut World, height: f32) -> Result<(), String> {
    let observer = world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|camera| camera.translation)
        .ok_or("Missing shader-cloud observer")?;
    let eye = Vec3::new(observer.x, height, observer.z);
    let direction = if height > 312.0 {
        Vec3::new(0.6, -0.55, -1.0)
    } else if height >= 200.0 {
        Vec3::new(0.6, 0.03, -1.0)
    } else {
        Vec3::new(0.6, 0.45, -1.0)
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = Transform::from_translation(eye).looking_at(eye + direction, Vec3::Y);
    }
    let mut focus = world.resource_mut::<ChunkFocus>();
    focus.0 = eye.to_array().map(f64::from);
    tracing::info!(?eye, ?direction, "Native shader-cloud layer inspection");
    Ok(())
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.phase == 255 {
            return;
        }
        let result = advance(world, &mut state);
        if let Err(error) = result {
            tracing::error!(%error,phase=state.phase,"Extended native smoke failed");
            world.write_message(AppExit::error());
            state.phase = 255;
        }
    });
}
fn advance(world: &mut World, state: &mut Fixture) -> Result<(), String> {
    if !state.third && state.phase > 0 && state.since.elapsed() > Duration::from_millis(500) {
        let delta = world.resource::<Time<Real>>().delta_secs_f64() * 1000.0;
        if delta > 0.0 {
            state.frame_times.push(delta);
        }
    }
    if state.started.elapsed() > Duration::from_secs(180) {
        return Err("Fixture timed out".into());
    }
    if state.phase == 0 {
        if !ready(world) || state.started.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        if state.third {
            let body = world
                .query_filtered::<(&Transform, &PlayerGrounded), With<PlayerBody>>()
                .iter(world)
                .find(|(_, ground)| ground.0)
                .map(|(body, _)| body.translation);
            let Some(body) = body else {
                return Ok(());
            };
            let eye = body + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            let mut rotation = None;
            for index in 0..16 {
                let yaw = index as f32 * std::f32::consts::TAU / 16.0;
                let backward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
                if terra_voxel::blocks::raycast_blocks(
                    eye.to_array().map(f64::from),
                    backward.to_array().map(f64::from),
                    4.0,
                    |p| Some(world.resource::<ChunkStore>().block_at(p).unwrap_or(1)),
                )
                .is_none()
                {
                    rotation = Some(Quat::from_rotation_y(yaw));
                    break;
                }
            }
            let rotation = rotation.ok_or("No camera clearance in fixture")?;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                camera.rotation = rotation;
            }
            *world.resource_mut::<CameraPerspective>() = CameraPerspective::ThirdPerson;
        } else {
            let focus = world.resource::<ChunkFocus>().0;
            let chunks = world.resource::<ChunkStore>();
            let mut target = None;
            let mut best = f64::INFINITY;
            for (position, section) in &chunks.loaded {
                if (position.x as f64 * 16.0 - focus[0]).abs() > 32.0
                    || (position.z as f64 * 16.0 - focus[2]).abs() > 32.0
                {
                    continue;
                }
                for (index, &material) in section.data.blocks().iter().enumerate() {
                    if material != 1 {
                        continue;
                    }
                    let p = position
                        .global_block([index % 16, index / 16 % 16, index / 256])
                        .map_err(|e| e.to_string())?;
                    if chunks.block_at([p[0], p[1] + 1, p[2]]) != Some(0) {
                        continue;
                    }
                    let distance =
                        (p[0] as f64 - focus[0]).powi(2) + (p[2] as f64 - focus[2]).powi(2);
                    if distance < best {
                        let point = Vec3::from_array(p.map(|v| v as f32));
                        let eye = point + Vec3::new(3.5, 3.5, 4.5);
                        let ray = point + Vec3::splat(0.5) - eye;
                        let visible = terra_voxel::blocks::raycast_blocks(
                            eye.to_array().map(f64::from),
                            ray.normalize().to_array().map(f64::from),
                            f64::from(ray.length()) + 0.1,
                            |position| chunks.block_at(position),
                        )
                        .is_some_and(|hit| hit.position == p);
                        if !visible {
                            continue;
                        }
                        best = distance;
                        target = Some(p);
                    }
                }
            }
            let target = target.ok_or("No natural grass surface in fixture")?;
            let point = Vec3::from_array(target.map(|v| v as f32));
            let eye = point + Vec3::new(3.5, 3.5, 4.5);
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera =
                    Transform::from_translation(eye).looking_at(point + Vec3::splat(0.5), Vec3::Y);
            }
            world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition {
                    snow: 1.0,
                    ..default()
                },
            });
            state.target = Some(target);
        }
        state.phase = 1;
        state.since = Instant::now();
        return Ok(());
    }
    if state.third {
        if state.since.elapsed() < Duration::from_secs(1) {
            return Ok(());
        }
        let roots = world
            .query::<&Name>()
            .iter(world)
            .filter(|name| name.as_str() == "Local player avatar")
            .count();
        if state.phase == 1 {
            if roots != 1 {
                return Err(format!("Expected one local avatar, got {roots}"));
            }
            let eye = world
                .query_filtered::<&Transform, With<PlayerBody>>()
                .iter(world)
                .next()
                .ok_or("No player body")?
                .translation
                + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            let camera = world
                .query_filtered::<&Transform, With<TerraCamera>>()
                .iter(world)
                .next()
                .ok_or("No camera")?
                .translation;
            if camera.distance(eye) < 1.0 || camera.distance(eye) > 3.6 {
                return Err(format!("Invalid camera boom: {}", camera.distance(eye)));
            }
            capture(world, "third-person");
            state.phase = 2;
            state.since = Instant::now();
        } else if state.phase == 2 {
            *world.resource_mut::<CameraPerspective>() = CameraPerspective::FirstPerson;
            state.phase = 3;
            state.since = Instant::now();
        } else {
            if roots != 0 {
                return Err("Local avatar leaked after first-person switch".into());
            }
            tracing::info!(
                "Native perspective fixture passed: collision-limited boom, shared avatar and first-person return"
            );
            world.write_message(AppExit::Success);
            state.phase = 255;
        }
        return Ok(());
    }
    let delay = if !state.third && matches!(state.phase, 23 | 25 | 49 | 52) && !state.captured {
        Duration::from_millis(25)
    } else if state.captured {
        Duration::from_millis(300)
    } else {
        Duration::from_secs(2)
    };
    if state.since.elapsed() < delay || !ready(world) {
        return Ok(());
    }
    let target = state.target.ok_or("Missing weather target")?;
    let condition = world
        .resource::<terra_world::edits::EditJournal>()
        .surface_condition(target);
    let measured_phase = state.phase;
    match state.phase {
        1 => {
            if condition.snow != 1.0 {
                return Ok(());
            }
            if !captured(world, state, "snow-cover") {
                return Ok(());
            }
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Rain;
            state.phase = 2;
        }
        2 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.rain != 1.0
                || status.batches != 1
                || status.quads < 500
                || status.roof_queries > 24
            {
                return Err(format!("GPU rain batch or roof budget invalid: {status:?}"));
            }
            if !captured(world, state, "rain") {
                return Ok(());
            }
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Snow;
            state.phase = 3;
        }
        3 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.snow != 1.0 || status.batches != 1 {
                return Err(format!("GPU snowfall batch invalid: {status:?}"));
            }
            if !captured(world, state, "snowfall") {
                return Ok(());
            }
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Clear;
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition {
                    fire: 1.0,
                    ..default()
                },
            });
            state.phase = 4;
        }
        4 => {
            if condition.fire <= 0.0 {
                return Err("Fire did not ignite".into());
            }
            if !captured(world, state, "fire") {
                return Ok(());
            }
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition {
                    wetness: 1.0,
                    ..default()
                },
            });
            state.phase = 5;
        }
        5 => {
            if condition.fire != 0.0 {
                return Err("Water did not extinguish the fire".into());
            }
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Rain;
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .weather_quality = terra_core::WeatherQuality::Low;
            state.phase = 6;
        }
        6 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.rain != 1.0 || status.batches != 1 || status.quads > 400 {
                return Err(format!(
                    "Low precipitation quality failed its geometry budget: {status:?}"
                ));
            }
            if status.wet_batches == 0 {
                return Err(
                    "Roof-aware wet surface shader did not receive a damp terrain batch".into(),
                );
            }
            if !captured(world, state, "rain-low") {
                return Ok(());
            }
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .weather_quality = terra_core::WeatherQuality::Off;
            state.phase = 7;
        }
        7 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.rain != 1.0 || status.batches != 0 || status.quads != 0 {
                return Err(format!(
                    "Off quality changed actual weather or retained particle geometry: {status:?}"
                ));
            }
            if !captured(world, state, "rain-effects-off") {
                return Ok(());
            }
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .weather_quality = terra_core::WeatherQuality::High;
            world.resource_mut::<terra_core::WorldClock>().phase = 0.0;
            state.phase = 8;
        }
        8 => {
            if !captured(world, state, "rain-night") {
                return Ok(());
            }
            state.phase = 9;
        }
        9 => {
            world.resource_mut::<terra_core::WorldClock>().phase = 0.5;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                let eye = camera.translation;
                *camera = Transform::from_translation(eye)
                    .looking_at(eye + Vec3::new(0.0, 0.45, -1.0), Vec3::Y);
            }
            state.phase = 10;
        }
        10 => {
            if !captured(world, state, "cloud-front") {
                return Ok(());
            }
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Clear;
            state.phase = 11;
        }
        11 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.cloud_batches != 1
                || status.cloud_faces != 6
                || status.cloud_uploads != 1
                || status.cloud_shaft_batches != 0
                || status.cloud_shaft_vertices != 0
            {
                return Err(format!(
                    "Shader sky did not retain its single bounded volume without sunshafts: {status:?}"
                ));
            }
            if !captured(world, state, "clouds-clear") {
                return Ok(());
            }
            let mut front = terra_core::WorldWeather::new(77);
            front.elapsed_seconds = front.duration_seconds - 10.0;
            front.elapsed_total = front.elapsed_seconds;
            world.remove_resource::<terra_render::WeatherPreview>();
            world.insert_resource(front);
            world.resource_mut::<terra_core::WorldClock>().paused = true;
            state.phase = 12;
        }
        12 => {
            if !captured(world, state, "clouds-approaching") {
                return Ok(());
            }
            let point = Vec3::from_array(target.map(|v| v as f32));
            let eye = point + Vec3::new(3.5, 3.5, 4.5);
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera =
                    Transform::from_translation(eye).looking_at(point + Vec3::splat(0.5), Vec3::Y);
            }
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition::default(),
            });
            state.phase = 13;
        }
        13 => {
            if !captured(world, state, "wet-dry") {
                return Ok(());
            }
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition {
                    wetness: 0.45,
                    ..default()
                },
            });
            state.phase = 14;
        }
        14 => {
            if !captured(world, state, "wet-damp") {
                return Ok(());
            }
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition {
                    wetness: 1.0,
                    ..default()
                },
            });
            state.phase = 15;
        }
        15 => {
            if !captured(world, state, "wet-saturated") {
                return Ok(());
            }
            world.insert_resource(terra_render::WeatherPreview {
                kind: terra_core::WeatherKind::Snow,
                intensity: 1.0,
            });
            world.resource_mut::<terra_core::WorldWeather>().snow_level = 1;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                let eye = camera.translation;
                *camera = Transform::from_translation(eye)
                    .looking_at(eye + Vec3::new(0.0, 0.45, -1.0), Vec3::Y);
            }
            state.phase = 16;
        }
        16..=20 => {
            let level = state.phase - 15;
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            let expected = world.resource::<terra_core::WorldWeather>().snow_profile();
            if status.snow_level != level
                || (status.snow_size - expected.size_multiplier).abs() > 0.001
            {
                return Err(format!(
                    "Snow level {level} did not reach renderer: {status:?}"
                ));
            }
            if !captured(world, state, &format!("snow-level-{level}")) {
                return Ok(());
            }
            if level < 5 {
                let mut weather = world.resource_mut::<terra_core::WorldWeather>();
                weather.snow_level = level + 1;
                if level == 4 {
                    weather.set_pattern(terra_core::WeatherPattern::Blizzard);
                    weather.elapsed_seconds = weather.transition_seconds;
                    weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
                }
                state.phase += 1;
            } else {
                let mut weather = world.resource_mut::<terra_core::WorldWeather>();
                weather.set_pattern(terra_core::WeatherPattern::Thunderstorm);
                weather.elapsed_seconds = weather.transition_seconds;
                weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
                world.resource_mut::<terra_render::WeatherPreview>().kind =
                    terra_core::WeatherKind::Rain;
                state.phase = 21;
            }
        }
        21 => {
            if !captured(world, state, "storm-voxel-clouds") {
                return Ok(());
            }
            state.phase = 22;
        }
        22 => {
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.latest_lightning = Some(terra_core::LightningEvent {
                id: 7401,
                at: weather.elapsed_total,
                position: [
                    target[0] as f64,
                    target[1] as f64 + 60.0,
                    target[2] as f64 - 70.0,
                ],
                ground_strike: false,
            });
            state.phase = 23;
        }
        23 => {
            if !state.captured {
                let status = world.resource::<terra_render::WeatherRenderStatus>();
                if status.lightning_bolts != 1 || status.lightning_flash <= 0.0 {
                    return Err(format!(
                        "Shared lightning event did not produce a visible flash/bolt: {status:?}"
                    ));
                }
            }
            if !captured(world, state, "thunder-flash") {
                return Ok(());
            }
            state.phase = 24;
        }
        24 => {
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.latest_lightning = Some(terra_core::LightningEvent {
                id: 7402,
                at: weather.elapsed_total,
                position: [
                    target[0] as f64,
                    target[1] as f64 + 1.02,
                    target[2] as f64 - 20.0,
                ],
                ground_strike: true,
            });
            state.phase = 25;
        }
        25 => {
            if !state.captured {
                let status = world.resource::<terra_render::WeatherRenderStatus>();
                if status.lightning_bolts != 1 || status.lightning_flash <= 0.0 {
                    return Err(format!("Ground strike visual missing: {status:?}"));
                }
            }
            if !captured(world, state, "lightning-ground-strike") {
                return Ok(());
            }
            state.phase = 26;
        }
        26 => {
            let center = world.resource::<ChunkFocus>().0;
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let coast = natural_coast(terrain, center)
                .ok_or("No natural ocean-connected shore in fixture search radius")?;
            state.coast = Some(coast);
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.set_pattern(terra_core::WeatherPattern::Clear);
            weather.elapsed_seconds = weather.transition_seconds;
            weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
            weather.wetness = 0.0;
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Clear;
            world.resource_mut::<ChunkFocus>().0 = coast.0;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = Transform::from_translation(Vec3::from_array(coast.0.map(|v| v as f32)))
                    .looking_at(Vec3::from_array(coast.1.map(|v| v as f32)), Vec3::Y);
            }
            tracing::info!(eye=?coast.0,look=?coast.1,"Native coastal wash fixture relocated through normal streaming");
            state.phase = 27;
        }
        27 => {
            let quads: usize = world
                .query::<&terra_render::CoastalWashQuads>()
                .iter(world)
                .map(|count| count.0)
                .sum();
            if quads == 0 {
                return Err(
                    "No ocean-connected coastal wash quads uploaded after streaming".into(),
                );
            }
            // Capture opposing points of the live surf cycle, without changing
            // the production clock or mistaking two retreat frames for motion.
            let pulse = (world.resource::<Time>().elapsed_secs() * 0.48 + 0.7).sin();
            if !state.captured && pulse <= 0.985 {
                return Ok(());
            }
            if !captured(world, state, "ocean-shore-wave-a") {
                return Ok(());
            }
            tracing::info!(quads, "Native coastal surf active");
            state.phase = 28;
        }
        28 => {
            let pulse = (world.resource::<Time>().elapsed_secs() * 0.48 + 0.7).sin();
            if !state.captured && pulse >= -0.985 {
                return Ok(());
            }
            if !captured(world, state, "ocean-shore-wave-b") {
                return Ok(());
            }
            state.phase = 29;
        }
        29 => {
            world.resource_mut::<terra_core::WorldClock>().phase = 0.5;
            let mut preview = world.resource_mut::<terra_render::WeatherPreview>();
            preview.kind = terra_core::WeatherKind::Rain;
            preview.intensity = 0.0;
            let mut cloud_eye = None;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                let eye = camera.translation + Vec3::Y * 120.0;
                *camera = Transform::from_translation(eye)
                    .looking_at(eye + Vec3::new(0.0, 0.9, 0.4358899), Vec3::Y);
                cloud_eye = Some(eye);
            }
            if let Some(eye) = cloud_eye {
                world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
                tracing::info!(
                    ?eye,
                    "Native cloud-cover inspection raised above the coastal foreground"
                );
            }
            state.phase = 30;
        }
        30..=34 => {
            let deck = usize::from(state.phase - 30);
            if !captured(world, state, &format!("cloud-deck-{deck}-sun")) {
                return Ok(());
            }
            if deck < 4 {
                world
                    .resource_mut::<terra_render::WeatherPreview>()
                    .intensity = [0.0, 0.2, 0.5, 0.8, 1.0][deck + 1];
            } else {
                world.resource_mut::<terra_core::WorldClock>().phase = 0.0;
            }
            state.phase += 1;
        }
        35 => {
            if !state.captured
                && !world
                    .resource::<CloudCaptureChecks>()
                    .verify_cover_change()?
            {
                if state.since.elapsed() > Duration::from_secs(7) {
                    return Err("Shader-cloud screenshots did not complete".into());
                }
                return Ok(());
            }
            if !captured(world, state, "cloud-deck-night-moon-hidden") {
                return Ok(());
            }
            world
                .resource_mut::<terra_render::WeatherPreview>()
                .intensity = 0.0;
            state.phase = 36;
        }
        36 => {
            if !captured(world, state, "cloud-deck-night-moon-clear") {
                return Ok(());
            }
            state.phase = 37;
        }
        37 => {
            let (eye, look) = state.coast.ok_or("Missing streamed coastal viewpoint")?;
            world.resource_mut::<terra_core::WorldClock>().phase = 0.42;
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.set_pattern(terra_core::WeatherPattern::Storm);
            weather.elapsed_seconds = weather.transition_seconds;
            weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
            let mut preview = world.resource_mut::<terra_render::WeatherPreview>();
            preview.kind = terra_core::WeatherKind::Rain;
            preview.intensity = 1.0;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = Transform::from_translation(Vec3::from_array(eye.map(|v| v as f32)))
                    .looking_at(Vec3::from_array(look.map(|v| v as f32)), Vec3::Y);
            }
            state.phase = 38;
        }
        38 => {
            let wind = world.resource::<terra_core::WorldWeather>().wind();
            if wind[0].hypot(wind[1]) < 5.0 {
                return Err("Storm water fixture has no authoritative strong wind".into());
            }
            if !captured(world, state, "ocean-storm-crossing-waves") {
                return Ok(());
            }
            state.phase = 39;
        }
        39 => {
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.set_pattern(terra_core::WeatherPattern::Clear);
            weather.elapsed_seconds = weather.transition_seconds;
            weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
            weather.wetness = 0.0;
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Clear;
            state.phase = 40;
        }
        40 => {
            if !captured(world, state, "ocean-calm-shared-viewpoint") {
                return Ok(());
            }
            state.phase = 41;
        }
        41 => {
            let (eye, look) = state.coast.ok_or("Missing underwater coastal viewpoint")?;
            let direction =
                Vec3::from_array(std::array::from_fn(|axis| (look[axis] - eye[axis]) as f32));
            let direction = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let search_radius = (world
                .resource::<terra_core::GameConfig>()
                .client
                .view_distance as i32
                * 8)
            .clamp(24, 128);
            let sample = (0..=search_radius)
                .flat_map(|ring| {
                    (-ring..=ring).flat_map(move |z: i32| {
                        (-ring..=ring).filter_map(move |x: i32| {
                            (x.abs().max(z.abs()) == ring).then_some([x * 2, z * 2])
                        })
                    })
                })
                .find_map(|[x, z]| {
                    let p = Vec3::from_array(look.map(|v| v as f32))
                        + Vec3::new(x as f32, 0.0, z as f32);
                    let water = terrain.water_column(f64::from(p.x), f64::from(p.z))?;
                    let bed = [
                        p.x.floor() as i64,
                        water.bed.floor() as i64 - 1,
                        p.z.floor() as i64,
                    ];
                    (water.depth > 0.55
                        && world
                            .resource::<ChunkStore>()
                            .block_at(bed)
                            .and_then(terra_voxel::MaterialId::from_raw)
                            .is_some_and(terra_voxel::MaterialId::is_solid))
                    .then_some((p, water))
                })
                .ok_or("No shallow underwater bed for native rain-mask capture")?;
            let bed = [
                sample.0.x.floor() as i64,
                sample.1.bed.floor() as i64 - 1,
                sample.0.z.floor() as i64,
            ];
            let eye = Vec3::new(sample.0.x, sample.1.level as f32 - 0.25, sample.0.z);
            let at = Vec3::new(sample.0.x, sample.1.bed as f32, sample.0.z) + direction * 3.0;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = Transform::from_translation(eye).looking_at(at, Vec3::Y);
            }
            state.target = Some(bed);
            world.write_message(SurfaceConditionRequest {
                position: bed,
                condition: SurfaceCondition {
                    wetness: 1.0,
                    ..default()
                },
            });
            let mut preview = world.resource_mut::<terra_render::WeatherPreview>();
            preview.kind = terra_core::WeatherKind::Rain;
            preview.intensity = 1.0;
            tracing::info!(?bed, ?eye, "Native submerged saved-wetness/rain fixture");
            state.phase = 42;
        }
        42 => {
            if !captured(world, state, "underwater-rain-no-wet-film") {
                return Ok(());
            }
            state.phase = 43;
        }
        43 => {
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Clear;
            world.write_message(SurfaceConditionRequest {
                position: target,
                condition: SurfaceCondition::default(),
            });
            state.phase = 44;
        }
        44 => {
            if !captured(world, state, "underwater-dry-material-reference") {
                return Ok(());
            }
            state.phase = 45;
        }
        45 => {
            world.resource_mut::<terra_render::WeatherPreview>().kind =
                terra_core::WeatherKind::Rain;
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let eye = (0..=32)
                .flat_map(|ring| {
                    (-ring..=ring).flat_map(move |z: i32| {
                        (-ring..=ring).filter_map(move |x: i32| {
                            (x.abs().max(z.abs()) == ring).then_some([x, z])
                        })
                    })
                })
                .find_map(|[x, z]| {
                    let p = [target[0] + i64::from(x), target[2] + i64::from(z)];
                    let water = terrain.water_column(p[0] as f64 + 0.5, p[1] as f64 + 0.5)?;
                    let bed = [p[0], water.bed.floor() as i64 - 1, p[1]];
                    (water.depth > 1.6
                        && world
                            .resource::<ChunkStore>()
                            .block_at(bed)
                            .and_then(terra_voxel::MaterialId::from_raw)
                            .is_some_and(terra_voxel::MaterialId::is_solid))
                    .then_some(Vec3::new(
                        p[0] as f32 + 0.5,
                        water.level as f32 - 1.2,
                        p[1] as f32 + 0.5,
                    ))
                })
                .ok_or("No loaded deeper water for upward rain-impact capture")?;
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = Transform::from_translation(eye)
                    .looking_at(eye + Vec3::new(0.15, 1.0, -0.20), Vec3::Y);
            }
            state.phase = 46;
        }
        46 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.batches != 1 || status.quads == 0 || status.quads >= 300 {
                return Err(format!(
                    "Submerged rain must retain only the bounded surface-impact batch: {status:?}"
                ));
            }
            if !state.captured {
                tracing::info!(?status, "Native upward underwater rain-impact batch");
            }
            if !captured(world, state, "rain-impacts-seen-from-underwater") {
                return Ok(());
            }
            state.phase = 47;
        }
        47 => {
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.set_pattern(terra_core::WeatherPattern::Thunderstorm);
            weather.elapsed_seconds = weather.transition_seconds;
            weather.elapsed_total = weather.elapsed_total.max(weather.elapsed_seconds);
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                let eye = camera.translation + Vec3::Y * 6.0;
                *camera = Transform::from_translation(eye)
                    .looking_at(eye + Vec3::new(0.0, 300.0, -180.0), Vec3::Y);
            }
            state.phase = 48;
        }
        48 => {
            let eye = world
                .query_filtered::<&Transform, With<TerraCamera>>()
                .iter(world)
                .next()
                .map(|camera| camera.translation)
                .ok_or("Missing cloud-lightning observer")?;
            let mut weather = world.resource_mut::<terra_core::WorldWeather>();
            weather.latest_lightning = Some(terra_core::LightningEvent {
                id: 7603,
                at: weather.elapsed_total,
                position: [eye.x as f64, target[1] as f64, eye.z as f64 - 180.0],
                ground_strike: false,
            });
            state.phase = 49;
        }
        49 => {
            if !state.captured {
                let status = world.resource::<terra_render::WeatherRenderStatus>();
                if status.lightning_bolts != 1 || status.lightning_flash <= 0.0 {
                    return Err(format!("Cloud discharge missing: {status:?}"));
                }
            }
            if !captured(world, state, "lilac-white-horizontal-cloud-lightning") {
                return Ok(());
            }
            state.phase = 50;
        }
        50 => {
            state.clearing_frames = 0;
            {
                let mut clock = world.resource_mut::<terra_core::WorldClock>();
                clock.phase = 10.0 / 24.0;
                clock.paused = true;
            }
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .weather_quality = terra_core::WeatherQuality::High;
            world
                .resource_mut::<terra_core::WorldWeather>()
                .latest_lightning = None;
            world.insert_resource(terra_render::WeatherPreview {
                kind: terra_core::WeatherKind::Rain,
                intensity: 1.0,
            });
            frame_cloud_layer(world, 140.0)?;
            state.phase = 51;
        }
        51 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.clouds < 0.9 || status.cloud_batches != 1 {
                if state.since.elapsed() < Duration::from_secs(8) {
                    return Ok(());
                }
                return Err(format!(
                    "Clearing-cloud fixture did not establish an overcast bank: {status:?}"
                ));
            }
            state.phase = 52;
        }
        52 => {
            if state.captured {
                world.resource_mut::<terra_render::WeatherPreview>().kind =
                    terra_core::WeatherKind::Clear;
                world
                    .resource_mut::<terra_core::GameConfig>()
                    .client
                    .weather_quality = terra_core::WeatherQuality::Low;
                state.phase = 53;
            } else {
                let status = world.resource::<terra_render::WeatherRenderStatus>();
                let cover = status.clouds;
                if (0.50..=0.60).contains(&cover) {
                    state.clearing_frames = state.clearing_frames.saturating_add(1);
                    if state.clearing_frames < 3 {
                        world
                            .resource_mut::<terra_render::WeatherPreview>()
                            .intensity = (cover - 0.32) / 0.65;
                        return Ok(());
                    }
                    if status.cloud_shaft_batches != 0
                        || status.cloud_shaft_vertices != 0
                        || status.cloud_faces != 6
                        || status.cloud_batches != 1
                        || status.cloud_uploads != 1
                    {
                        return Err(format!(
                            "Clearing shader clouds changed their fixed volume or retained sunshafts: {status:?}"
                        ));
                    }
                    tracing::info!(
                        cover,
                        shaft_batches = status.cloud_shaft_batches,
                        shaft_vertices = status.cloud_shaft_vertices,
                        "Native clearing shader clouds ready without sunshafts"
                    );
                    // Keep the actual accepted cover stable during this capture.
                    world
                        .resource_mut::<terra_render::WeatherPreview>()
                        .intensity = (cover - 0.32) / 0.65;
                    captured(world, state, "shader-cloud-clearing-below");
                    return Ok(());
                }
                if state.since.elapsed() > Duration::from_secs(7) {
                    return Err(format!(
                        "Clearing cloud cover skipped the shader-layer inspection window: {status:?}"
                    ));
                }
                // WeatherPreview is intentionally instantaneous. Animate its
                // intensity rather than changing production weather smoothing.
                world
                    .resource_mut::<terra_render::WeatherPreview>()
                    .intensity = (1.0 - state.since.elapsed().as_secs_f32() / 4.0).clamp(0.0, 1.0);
                return Ok(());
            }
        }
        53 => {
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            if status.cloud_shaft_batches != 0 || status.cloud_shaft_vertices != 0 {
                return Err(format!(
                    "Low weather quality retained clearing-cloud sunshafts: {status:?}"
                ));
            }
            if !captured(world, state, "shader-cloud-clear-low") {
                return Ok(());
            }
            world
                .resource_mut::<terra_core::GameConfig>()
                .client
                .weather_quality = terra_core::WeatherQuality::High;
            let mut preview = world.resource_mut::<terra_render::WeatherPreview>();
            preview.kind = terra_core::WeatherKind::Rain;
            preview.intensity = 1.0;
            frame_cloud_layer(world, 256.0)?;
            state.phase = 54;
        }
        54 => {
            if !captured(world, state, "shader-cloud-inside") {
                return Ok(());
            }
            frame_cloud_layer(world, 400.0)?;
            state.phase = 55;
        }
        55 => {
            if !captured(world, state, "shader-cloud-above") {
                return Ok(());
            }
            state.phase = 56;
        }
        _ => {
            let checks = world.resource::<CloudCaptureChecks>();
            if let Some(error) = &checks.error {
                return Err(error.clone());
            }
            if checks.layer_views != 3 {
                if state.since.elapsed() > Duration::from_secs(7) {
                    return Err("Shader-cloud inside/above captures did not complete".into());
                }
                return Ok(());
            }
            let status = world.resource::<terra_render::WeatherRenderStatus>();
            tracing::info!(
                quads = status.quads,
                batches = status.batches,
                mesh_uploads = status.mesh_uploads,
                "Native weather fixture passed: rain, five budgeted snow levels/blizzard, five shader-cloud covers/celestial occlusion, spatial cloud-cover changes and views below/inside/above the layer without sunshafts, shared horizontal white/lilac lightning/ground strike, ocean-connected coastal wash, storm/calm ocean waves, submerged rain exclusion/visible surface impacts, low/off budgets, night, snow cover, ignition and extinction"
            );
            world.write_message(AppExit::Success);
            state.phase = 255;
        }
    }
    state.since = Instant::now();
    state.captured = false;
    if !state.third && !state.frame_times.is_empty() {
        let total: f64 = state.frame_times.iter().sum();
        state.frame_times.sort_unstable_by(f64::total_cmp);
        let count = state.frame_times.len();
        let status = world.resource::<terra_render::WeatherRenderStatus>();
        tracing::info!(
            phase = measured_phase,
            frames = count,
            fps = count as f64 * 1000.0 / total,
            p95_ms = state.frame_times[(count * 95 / 100).min(count - 1)],
            p99_ms = state.frame_times[(count * 99 / 100).min(count - 1)],
            quads = status.quads,
            batches = status.batches,
            uploads = status.mesh_uploads,
            cloud_faces = status.cloud_faces,
            cloud_batches = status.cloud_batches,
            cloud_uploads = status.cloud_uploads,
            cloud_shaft_batches = status.cloud_shaft_batches,
            cloud_shaft_vertices = status.cloud_shaft_vertices,
            snow_level = status.snow_level,
            snow_size = status.snow_size,
            "Weather render fixture timings (short windows, not a parity benchmark)"
        );
        state.frame_times.clear();
    }
    Ok(())
}
