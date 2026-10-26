//! Short native check of an actual generated tree and its production collapse.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{PlayerBody, TerraCamera};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkFocus, ChunkStore},
    tree_felling::{Collapse, Plan, Queue},
};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    phase: u8,
    view: Transform,
    focus: [f64; 3],
    since: f64,
    foot: [i64; 3],
    captures: usize,
    pending: bool,
    error: Option<String>,
    plan: Option<Plan>,
    tree: Option<Collapse>,
    bounds: [Vec3; 2],
    fov: f32,
    baseline: Option<Vec<[u8; 3]>>,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-tree-collapse") {
        return;
    }
    app.init_resource::<Queue>()
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(Fixture {
            started: Instant::now(),
            phase: 0,
            view: Transform::from_xyz(4.0, 40.0, 12.0),
            focus: [0.0, 32.0, 0.0],
            since: 0.0,
            foot: [0; 3],
            captures: 0,
            pending: false,
            error: None,
            plan: None,
            tree: None,
            bounds: [Vec3::ZERO; 2],
            fov: 60.0_f32.to_radians(),
            baseline: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn setup(world: &mut World) {
    let mode = match std::env::var("TERRA_TREE_FALL_MODE").as_deref() {
        Ok("hell_fall") => terra_core::TreeFallAnimation::HellFall,
        Ok("magic") => terra_core::TreeFallAnimation::Magic,
        _ => terra_core::TreeFallAnimation::Current,
    };
    if let Some(mut graphics) = world.get_resource_mut::<terra_core::GraphicsConfig>() {
        graphics.tree_fall_animation = mode;
    } else {
        world.insert_resource(terra_core::GraphicsConfig {
            tree_fall_animation: mode,
            ..Default::default()
        });
    }
    info!(?mode, "Native tree fall animation selected");
    let clock = terra_core::WorldClock {
        phase: 0.5,
        paused: true,
        ..Default::default()
    };
    *world.resource_mut::<terra_core::WorldClock>() = clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    world
        .resource_mut::<terra_core::WorldWeather>()
        .set_pattern(terra_core::WeatherPattern::Clear);
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.phase == 255 {
            return;
        }
        if state.phase > 0 {
            for mut camera in world
                .query_filtered::<&mut Transform, With<TerraCamera>>()
                .iter_mut(world)
            {
                *camera = state.view;
            }
            for mut body in world
                .query_filtered::<&mut Transform, With<PlayerBody>>()
                .iter_mut(world)
            {
                body.translation = state.view.translation;
            }
        }
        world.resource_mut::<ChunkFocus>().0 = state.focus;
        let result = advance(world, &mut state);
        match result {
            Ok(true) => {
                info!(
                    captures = state.captures,
                    "Native natural-tree collapse passed; masks, pieces and mesh owners released"
                );
                world.write_message(AppExit::Success);
                state.phase = 255;
            }
            Err(error) => {
                error!(%error, phase=state.phase, "Native tree collapse fixture failed");
                world.write_message(AppExit::error());
                state.phase = 255;
            }
            _ => {}
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    if let Some(error) = &state.error {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(70) {
        return Err("Native tree collapse timed out".into());
    }
    if let Some(error) = &world.resource::<terra_render::RenderStatus>().error {
        return Err(error.clone());
    }
    let now = world.resource::<Time>().elapsed_secs_f64();
    if state.phase == 0 {
        if !world.resource::<terra_render::RenderStatus>().ready {
            return Ok(false);
        }
        let chunks = world.resource::<ChunkStore>();
        let journal = world.resource::<EditJournal>();
        let mut candidates = Vec::new();
        for (&section, loaded) in &chunks.loaded {
            for (i, _) in loaded
                .data
                .blocks()
                .iter()
                .enumerate()
                .filter(|(_, m)| matches!(m, 6 | 10 | 36 | 134))
            {
                let p = section
                    .global_block([i % 16, i / 16 % 16, i / 256])
                    .map_err(|e| e.to_string())?;
                if !terra_world::tree_felling::eligible(chunks, journal, p) {
                    continue;
                }
                if let Some(plan) = Plan::prepare(chunks, journal, p)
                    && (8..=512).contains(&plan.wood_count())
                {
                    let event = plan.collapse([1.0, 0.2]);
                    if event.valid()
                        && event.sections.iter().map(|s| s.leaves.len()).sum::<usize>() <= 2000
                    {
                        candidates.push((p, plan, event));
                    }
                }
            }
        }
        candidates.sort_by_key(|(p, _, _)| p[0].abs() + p[2].abs());
        let config = world.resource::<terra_core::GameConfig>();
        let fov = config.client.fov_degrees.to_radians();
        let aspect = config.client.width as f32 / config.client.height.max(1) as f32;
        let Some((foot, plan, event, view, bounds)) =
            candidates.into_iter().find_map(|(foot, plan, event)| {
                inspection_view(&event, chunks, fov, aspect)
                    .map(|(view, bounds)| (foot, plan, event, view, bounds))
            })
        else {
            return Ok(false);
        };
        state.foot = foot;
        state.focus = foot.map(|v| v as f64);
        state.view = view;
        state.bounds = bounds;
        state.fov = fov;
        info!(
            ?foot,
            wood = event.sections.iter().map(|s| s.wood.len()).sum::<usize>(),
            leaves = event.sections.iter().map(|s| s.leaves.len()).sum::<usize>(),
            camera = ?view.translation,
            "Native natural tree attributed with loaded unobstructed inspection view"
        );
        state.plan = Some(plan);
        state.tree = Some(event);
        state.phase = 10;
        state.since = now;
        return Ok(false);
    }
    if state.phase == 10 {
        if now - state.since < 1.0 {
            return Ok(false);
        }
        capture(world, "tree-before")?;
        state.pending = true;
        state.phase = 11;
        return Ok(false);
    }
    if state.phase == 11 {
        if state.pending || state.captures != 1 {
            return Ok(false);
        }
        let plan = state.plan.take().ok_or("Missing attributed native tree")?;
        if !plan.unchanged(
            world.resource::<ChunkStore>(),
            world.resource::<EditJournal>(),
        ) || !world
            .resource_mut::<Queue>()
            .enqueue(plan, terra_core::PlayerUuid::from_u128(121))
        {
            return Err("Native natural-tree cut rejected".into());
        }
        let event = state.tree.clone().ok_or("Missing native tree collapse")?;
        world.write_message(terra_network::client::TreeCollapseEvent(event));
        state.phase = 1;
        state.since = now;
        return Ok(false);
    }
    if state.phase == 1 {
        if world.resource::<terra_render::TreeFallStatus>().active == 0 {
            return Ok(false);
        }
        state.phase = 2;
        state.since = now;
    }
    world.resource_scope(|world, mut queue: Mut<Queue>| {
        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
            let properties = world
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
                .clone();
            queue.advance(
                &mut chunks,
                &mut world.resource_mut::<EditJournal>(),
                &properties,
                now,
            )
        })
    })?;
    let age = now - state.since;
    if !state.pending && state.phase == 2 && age > 0.65 {
        capture(world, "tree-tilting")?;
        state.pending = true;
        state.phase = 3;
    } else if !state.pending && state.phase == 3 && state.captures == 2 && age > 1.65 {
        capture(world, "tree-fracturing")?;
        state.pending = true;
        state.phase = 4;
    } else if !state.pending && state.phase == 4 && state.captures == 3 && age > 6.0 {
        let status = world.resource::<terra_render::TreeFallStatus>();
        if status.active != 0
            || status.preparing
            || status.meshes != 0
            || !world.resource::<Queue>().is_empty()
        {
            return Ok(false);
        }
        if world.resource::<ChunkStore>().block_at(state.foot) != Some(0) {
            return Err("Authoritative tree removal did not finish".into());
        }
        if world
            .query::<&Name>()
            .iter(world)
            .any(|name| name.as_str() == "Falling tree")
        {
            return Err("Temporary tree root leaked".into());
        }
        capture(world, "tree-cleared")?;
        state.pending = true;
        state.phase = 5;
    } else if !state.pending && state.phase == 5 && state.captures == 4 {
        return Ok(true);
    }
    Ok(false)
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-tree-collapse"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(validate);
    Ok(())
}

fn inspection_view(
    event: &Collapse,
    chunks: &ChunkStore,
    fov: f32,
    aspect: f32,
) -> Option<(Transform, [Vec3; 2])> {
    let cells: HashSet<_> = event
        .sections
        .iter()
        .flat_map(|s| {
            s.cells()
                .filter_map(|(i, _)| event.cell_position(s.position, i))
        })
        .collect();
    let mut bounds = [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)];
    for &cell in &cells {
        let p = Vec3::from_array(cell.map(|v| v as f32));
        bounds[0] = bounds[0].min(p);
        bounds[1] = bounds[1].max(p + Vec3::ONE);
    }
    let foot = Vec3::from_array(event.foot.map(|v| v as f32)) + Vec3::new(0.5, 0.0, 0.5);
    let height = bounds[1].y - foot.y;
    let width = (bounds[1].x - bounds[0].x).max(bounds[1].z - bounds[0].z);
    let target = foot
        + Vec3::Y * height * 0.42
        + Vec3::new(event.direction[0], 0.0, event.direction[1]) * height * 0.15;
    let radius = (height * 1.2 + width * 0.5).clamp(24.0, 36.0);
    let center = (bounds[0] + bounds[1]) * 0.5;
    let probes = [
        foot + Vec3::Y * height * 0.25,
        center,
        Vec3::new(center.x, bounds[1].y, center.z),
        Vec3::new(bounds[0].x, center.y, bounds[0].z),
        Vec3::new(bounds[1].x, center.y, bounds[0].z),
        Vec3::new(bounds[0].x, center.y, bounds[1].z),
        Vec3::new(bounds[1].x, center.y, bounds[1].z),
    ];
    for step in 0..16 {
        let angle = 0.55 + step as f32 * std::f32::consts::TAU / 16.0;
        let eye = Vec3::new(
            target.x + angle.cos() * radius,
            bounds[1].y + 8.0,
            target.z + angle.sin() * radius,
        );
        let view = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        let Some([low, high]) = projected_bounds(view, fov, aspect, bounds) else {
            continue;
        };
        if low.min_element() < -0.88 || high.max_element() > 0.88 {
            continue;
        }
        if (-1..=1).any(|z| {
            (-1..=1).any(|y| {
                (-1..=1).any(|x| {
                    chunks.block_at(
                        (eye + Vec3::new(x as f32, y as f32, z as f32))
                            .floor()
                            .as_i64vec3()
                            .to_array(),
                    ) != Some(0)
                })
            })
        }) {
            continue;
        }
        let clear = probes.iter().all(|target| {
            let delta = *target - eye;
            terra_voxel::blocks::raycast_blocks(
                eye.to_array().map(f64::from),
                delta.to_array().map(f64::from),
                f64::from(delta.length()),
                |p| {
                    Some(u16::from(
                        chunks
                            .block_at(p)
                            .is_none_or(|m| m != 0 && !cells.contains(&p)),
                    ))
                },
            )
            .is_none()
        });
        if clear {
            return Some((view, bounds));
        }
    }
    None
}

fn projected_bounds(
    view: Transform,
    fov: f32,
    aspect: f32,
    bounds: [Vec3; 2],
) -> Option<[Vec2; 2]> {
    let matrix = Mat4::perspective_rh(fov, aspect, 0.1, 1000.0) * view.to_matrix().inverse();
    let mut result = [Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)];
    for x in [bounds[0].x, bounds[1].x] {
        for y in [bounds[0].y, bounds[1].y] {
            for z in [bounds[0].z, bounds[1].z] {
                let clip = matrix * Vec4::new(x, y, z, 1.0);
                if !clip.is_finite() || clip.w <= 0.0 {
                    return None;
                }
                let p = clip.truncate().truncate() / clip.w;
                result[0] = result[0].min(p);
                result[1] = result[1].max(p);
            }
        }
    }
    Some(result)
}

fn validate(capture: On<ScreenshotCaptured>, mut state: ResMut<Fixture>) {
    state.pending = false;
    match capture.image.clone().try_into_dynamic() {
        Ok(image) => {
            let image = image.into_rgba8();
            let min = image
                .pixels()
                .map(|p| u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]))
                .min()
                .unwrap_or(0);
            let max = image
                .pixels()
                .map(|p| u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2]))
                .max()
                .unwrap_or(0);
            if max - min < 80 {
                state.error = Some("Tree collapse screenshot is blank".into());
            }
            let aspect = image.width() as f32 / image.height().max(1) as f32;
            if let Some([low, high]) = projected_bounds(state.view, state.fov, aspect, state.bounds)
            {
                let left = ((low.x + 1.0) * 0.5 * image.width() as f32)
                    .clamp(0.0, image.width().saturating_sub(1) as f32)
                    as u32;
                let right = ((high.x + 1.0) * 0.5 * image.width() as f32)
                    .clamp(left as f32, image.width().saturating_sub(1) as f32)
                    as u32;
                let top = ((1.0 - high.y) * 0.5 * image.height() as f32)
                    .clamp(0.0, image.height().saturating_sub(1) as f32)
                    as u32;
                let bottom = ((1.0 - low.y) * 0.5 * image.height() as f32)
                    .clamp(top as f32, image.height().saturating_sub(1) as f32)
                    as u32;
                if right - left < 48 || bottom - top < 48 {
                    state.error =
                        Some("Tree does not occupy a measurable inspection region".into());
                }
                let mut samples = Vec::with_capacity(4096);
                for y in 0..64 {
                    for x in 0..64 {
                        let p = image.get_pixel(
                            left + (right - left) * x / 64,
                            top + (bottom - top) * y / 64,
                        );
                        samples.push([p[0], p[1], p[2]]);
                    }
                }
                if let Some(baseline) = &state.baseline {
                    let changed = baseline
                        .iter()
                        .zip(&samples)
                        .filter(|(a, b)| {
                            (0..3).map(|i| u16::from(a[i].abs_diff(b[i]))).sum::<u16>() > 30
                        })
                        .count();
                    info!(
                        phase = state.phase,
                        changed,
                        samples = samples.len(),
                        "Native tree inspection differs from pre-fall baseline"
                    );
                    let minimum = if state.phase == 5 { 128 } else { 40 };
                    if changed < minimum {
                        state.error = Some(format!(
                            "Tree animation/clearing did not measurably change its visible region: {changed}/{minimum}"
                        ));
                    }
                } else {
                    state.baseline = Some(samples);
                }
            } else {
                state.error = Some("Tree inspection bounds are behind the camera".into());
            }
            state.captures += 1;
        }
        Err(error) => state.error = Some(error.to_string()),
    }
}
