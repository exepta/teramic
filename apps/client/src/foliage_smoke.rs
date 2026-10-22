//! Opt-in native foliage acceptance capture, using real generated trees.
use bevy::{app::AppExit, camera::primitives::Aabb, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, PlayerBody, TerraCamera};
use terra_render::{FoliageStatus, RenderStatus, WindFoliage};
use terra_world::streaming::{
    ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    framed: Option<Instant>,
    stage: u8,
    target: Vec3,
    view: Transform,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-foliage") {
        return;
    }
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Fixture {
            started: Instant::now(),
            framed: None,
            stage: 0,
            target: Vec3::ZERO,
            view: Transform::default(),
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn setup(
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
) {
    let center = Vec3::new(
        -256.0,
        terrain.0.surface_height(-256.0, 1536.0) as f32 + 5.0,
        1536.0,
    );
    focus.0 = center.to_array().map(f64::from);
    for mut camera in &mut cameras {
        camera.translation = center;
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.stage == 255 {
            return;
        }
        match advance(world, &mut state) {
            Ok(true) => {
                world.write_message(AppExit::Success);
                state.stage = 255;
            }
            Err(error) => {
                tracing::error!(%error, "Foliage native fixture failed");
                world.write_message(AppExit::error());
                state.stage = 255;
            }
            Ok(false) => {}
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    if state.started.elapsed() > Duration::from_secs(100) {
        return Err("Timed out waiting for foliage".into());
    }
    if let Some(error) = &world.resource::<RenderStatus>().error {
        return Err(error.clone());
    }
    if state.framed.is_none() {
        if world.resource::<WorldGeneration>().status != GenerationStatus::Ready
            || !world.resource::<RenderStatus>().ready
            || world.resource::<FoliageStatus>().material_variants != 3
        {
            return Ok(false);
        }
        let focus = Vec3::from_array(world.resource::<ChunkFocus>().0.map(|v| v as f32));
        let chunks = world.resource::<ChunkStore>();
        let mut best = f32::INFINITY;
        let mut selected = None;
        for (section, loaded) in &chunks.loaded {
            let origin = section.origin().map_err(|e| e.to_string())?;
            if (origin[0] as f32 - focus.x).abs() > 48.0
                || (origin[2] as f32 - focus.z).abs() > 48.0
            {
                continue;
            }
            for (index, &kind) in loaded.data.blocks().iter().enumerate() {
                if !(7..=9).contains(&kind) {
                    continue;
                }
                let p = section
                    .global_block([index % 16, index / 16 % 16, index / 256])
                    .map_err(|e| e.to_string())?;
                if chunks.block_at([p[0], p[1] - 1, p[2]]) != Some(0) {
                    continue;
                }
                let target = Vec3::from_array(p.map(|v| v as f32)) + Vec3::splat(0.5);
                let distance = target.distance_squared(focus);
                if distance >= best {
                    continue;
                }
                for direction in 0..16 {
                    let angle = direction as f32 * std::f32::consts::TAU / 16.0;
                    let eye = target + Vec3::new(angle.sin() * 7.5, -1.8, angle.cos() * 7.5);
                    if !clear_camera(chunks, eye) {
                        continue;
                    }
                    let ray = target - eye;
                    let hit = terra_voxel::blocks::raycast_blocks(
                        eye.to_array().map(f64::from),
                        ray.normalize().to_array().map(f64::from),
                        f64::from(ray.length()) + 0.1,
                        |position| chunks.block_at(position),
                    );
                    if hit.is_some_and(|hit| hit.position == p) {
                        best = distance;
                        selected = Some((target, eye));
                        break;
                    }
                }
            }
        }
        let (target, eye) =
            selected.ok_or("No loaded exposed natural crown near forest fixture")?;
        state.target = target;
        state.view = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        state.framed = Some(Instant::now());
        world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
    }
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
    let status = world.resource::<FoliageStatus>();
    if status.falling_leaves > 48 || status.particle_batches > 1 {
        return Err("Leaf particle budget exceeded".into());
    }
    let elapsed = state.framed.map_or(Duration::ZERO, |t| t.elapsed());
    match state.stage {
        0 if elapsed > Duration::from_secs(3) => {
            let batches = world
                .query_filtered::<&Aabb, With<WindFoliage>>()
                .iter(world)
                .count();
            if batches == 0 {
                return Err("No wind foliage section batches with expanded bounds".into());
            }
            capture(world, "foliage-still")?;
            state.stage = 1;
        }
        1 if elapsed > Duration::from_millis(4200) => {
            capture(world, "foliage-wind")?;
            state.stage = 2;
        }
        2 if elapsed > Duration::from_secs(10) => {
            let status = world.resource::<FoliageStatus>();
            if status.spawned_leaves == 0 {
                return Err("No real loaded tree emitted a falling leaf".into());
            }
            tracing::info!(
                variants = status.material_variants,
                active_leaves = status.falling_leaves,
                batches = status.particle_batches,
                emitted = status.spawned_leaves,
                "Native foliage budgets verified"
            );
            capture(world, "foliage-leaves")?;
            state.stage = 3;
        }
        3 if elapsed > Duration::from_secs(12) => return Ok(true),
        _ => {}
    }
    Ok(false)
}

fn clear_camera(chunks: &ChunkStore, eye: Vec3) -> bool {
    let min = (eye - Vec3::splat(1.0)).floor().as_i64vec3();
    let max = (eye + Vec3::splat(1.0)).floor().as_i64vec3();
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                if chunks.block_at([x, y, z]) != Some(0) {
                    return false;
                }
            }
        }
    }
    true
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-foliage"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            directory.join(format!("{name}.png")),
        ));
    Ok(())
}
