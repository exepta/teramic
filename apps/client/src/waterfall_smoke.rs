//! Short opt-in inspection of real stepped, cross-section fluid flow and meshes.
use bevy::{
    app::AppExit,
    camera::primitives::Aabb,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{WeatherPattern, WorldClock, WorldWeather};
use terra_player::{ControllerMode, PlayerBody, TerraCamera};
use terra_render::{RenderStatus, WaterStatus};
use terra_world::{
    edits::EditJournal,
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, WorldAccess, WorldTerrain, commit_block_edit,
    },
};

#[derive(Resource)]
struct Fixture {
    started: Instant,
    since: Instant,
    phase: u8,
    floor: i64,
    view: Transform,
    captures: usize,
    pending: bool,
    failure: Option<String>,
    before_close: u64,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-waterfalls") {
        return;
    }
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Fixture {
            started: Instant::now(),
            since: Instant::now(),
            phase: 0,
            floor: 64,
            view: Transform::default(),
            captures: 0,
            pending: false,
            failure: None,
            before_close: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
}

fn setup(world: &mut World) {
    let terrain = &world.resource::<WorldTerrain>().0;
    let highest = (12..=24)
        .flat_map(|x| (3..=11).map(move |z| (x, z)))
        .map(|(x, z)| terrain.surface_height(f64::from(x), f64::from(z)))
        .fold(32.0_f64, f64::max);
    let floor = ((highest as i64 + 47).div_euclid(16)) * 16;
    let view = overview(floor, false);
    {
        let mut state = world.resource_mut::<Fixture>();
        state.floor = floor;
        state.view = view;
    }
    let clock = WorldClock {
        phase: 0.5,
        paused: true,
        ..Default::default()
    };
    *world.resource_mut::<WorldClock>() = clock;
    world.resource_mut::<EditJournal>().clock = Some(clock);
    world
        .resource_mut::<WorldWeather>()
        .set_pattern(WeatherPattern::Clear);
    world.resource_mut::<ChunkFocus>().0 = [17.0, floor as f64 + 12.0, 7.0];
}

fn overview(floor: i64, close: bool) -> Transform {
    let target = Vec3::new(18.5, floor as f32 + if close { 11.5 } else { 12.0 }, 7.0);
    Transform::from_translation(
        target
            + if close {
                Vec3::new(9.0, 2.0, 9.0)
            } else {
                Vec3::new(16.0, 8.0, 22.0)
            },
    )
    .looking_at(target, Vec3::Y)
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.phase == 255 {
            return;
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
        world.resource_mut::<ChunkFocus>().0 = [17.0, state.floor as f64 + 12.0, 7.0];
        match advance(world, &mut state) {
            Ok(true) => {
                info!(
                    captures = state.captures,
                    "Native stepped waterfall fixture passed"
                );
                world.write_message(AppExit::Success);
                state.phase = 255;
            }
            Err(error) => {
                error!(%error,phase=state.phase,"Native stepped waterfall fixture failed");
                world.write_message(AppExit::error());
                state.phase = 255;
            }
            Ok(false) => {}
        }
    });
}

fn advance(world: &mut World, state: &mut Fixture) -> Result<bool, String> {
    if let Some(error) = &state.failure {
        return Err(error.clone());
    }
    if state.started.elapsed() > Duration::from_secs(75) {
        return Err("Waterfall fixture exceeded75 seconds".into());
    }
    if !world.resource::<WorldAccess>().authoritative() {
        return Err("Waterfall fixture needs the existing authoritative offline world tick".into());
    }
    if !world.resource::<WorldTerrain>().0.config().water {
        return Err("Water must be enabled".into());
    }
    if let Some(error) = &world.resource::<RenderStatus>().error {
        return Err(error.clone());
    }
    if state.pending {
        return Ok(false);
    }
    match state.phase {
        0 => {
            if !loaded(world, state.floor) || !world.resource::<RenderStatus>().ready {
                return Ok(false);
            }
            build(world, state.floor)?;
            state.phase = 1;
            state.since = Instant::now();
        }
        1 => {
            if state.since.elapsed() < Duration::from_secs(5) || !rendered(world, state.floor) {
                return Ok(false);
            }
            let journal = world.resource::<EditJournal>();
            let source = journal.fluid([15, state.floor + 21, 7]);
            if source.is_none_or(|c| !c.source || c.amount != 8) {
                return Err("Source cell changed or vanished".into());
            }
            let vertical = journal
                .fluid([17, state.floor + 16, 7])
                .is_some_and(|c| c.amount > 0 && !c.source);
            let slab = journal
                .fluid([20, state.floor + 11, 7])
                .is_some_and(|c| c.amount > 4 && !c.source);
            let low = journal.fluid_cells().any(|(p, c)| {
                p[0] >= 21
                    && p[1] < state.floor + 5
                    && p[1] > state.floor
                    && c.amount > 0
                    && !c.source
            });
            if !(vertical && slab && low) {
                return Ok(false);
            }
            verify_render_batches(world, state.floor)?;
            capture(world, "waterfall-stepped-overview")?;
            state.pending = true;
            state.phase = 2;
            state.since = Instant::now();
        }
        2 => {
            if state.captures != 1 {
                return Ok(false);
            }
            state.view = overview(state.floor, true);
            state.phase = 3;
            state.since = Instant::now();
        }
        3 => {
            if state.since.elapsed() < Duration::from_secs(2) || !rendered(world, state.floor) {
                return Ok(false);
            }
            capture(world, "waterfall-partial-step-close")?;
            state.pending = true;
            state.phase = 4;
        }
        4 => {
            if state.captures != 2 {
                return Ok(false);
            }
            state.before_close = downstream_volume(world, state.floor);
            edit(world, [16, state.floor + 21, 7], 3)?;
            state.phase = 5;
            state.since = Instant::now();
        }
        5 => {
            let after = downstream_volume(world, state.floor);
            if state.since.elapsed() < Duration::from_secs(5)
                || !rendered(world, state.floor)
                || after >= state.before_close / 2
            {
                return Ok(false);
            }
            let journal = world.resource::<EditJournal>();
            if journal
                .fluid([15, state.floor + 21, 7])
                .is_none_or(|c| !c.source || c.amount != 8)
            {
                return Err("Closing the outlet corrupted its source".into());
            }
            if journal.fluid_cells().any(|(p, c)| {
                p[0] >= 17
                    && (state.floor..=state.floor + 24).contains(&p[1])
                    && (4..=10).contains(&p[2])
                    && c.source
            }) {
                return Err("Waterfall generated an unauthorized downstream source".into());
            }
            info!(
                before = state.before_close,
                after, "Actual waterfall receded after closing its upstream gate"
            );
            state.phase = 6;
        }
        6 => return Ok(captures_finished(world)),
        _ => {}
    }
    Ok(false)
}

fn captures_finished(world: &mut World) -> bool {
    // Preview rendering uses the same asynchronous screenshot channel.
    // Let those callbacks finish before shutting down the render sub-app.
    world
        .get_resource::<terra_render::RenderBlockPreviews>()
        .is_none_or(|previews| previews.pending == 0)
        && world.query::<&Screenshot>().iter(world).next().is_none()
}

fn loaded(world: &World, floor: i64) -> bool {
    let chunks = world.resource::<ChunkStore>();
    [12, 23].into_iter().all(|x| {
        [4, 10].into_iter().all(|z| {
            [floor, floor + 23]
                .into_iter()
                .all(|y| chunks.block_at([x, y, z]).is_some())
        })
    })
}

fn rendered(world: &World, floor: i64) -> bool {
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    [0, 1].into_iter().all(|x| {
        [floor.div_euclid(16), floor.div_euclid(16) + 1]
            .into_iter()
            .all(|y| {
                let p = terra_voxel::SectionPos::new(x, y, 0);
                chunks.loaded.get(&p).is_some_and(|s| {
                    s.mesh_epoch == journal.mesh_epoch(p)
                        && s.water_epoch == journal.water_mesh_epoch(p)
                })
            })
    }) && world.resource::<RenderStatus>().ready
        && world.resource::<WaterStatus>().ready
}

fn edit(world: &mut World, position: [i64; 3], material: u16) -> Result<(), String> {
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let expected_material = chunks
            .block_at(position)
            .ok_or("Waterfall fixture block is not loaded")?;
        if expected_material == material {
            return Ok(());
        }
        let properties = world
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .clone();
        let mut journal = world.resource_mut::<EditJournal>();
        if !commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position,
                expected_material,
                material,
            },
            &properties,
        )? {
            return Err("Waterfall fixture block commit was rejected".into());
        }
        Ok(())
    })
}

fn build(world: &mut World, floor: i64) -> Result<(), String> {
    for x in 12..=23 {
        for z in 4..=10 {
            for y in 0..=23 {
                let wall = y == 0
                    || x == 12
                    || z == 4
                    || ((x == 23 || z == 10) && y <= 2)
                    || (z == 10 && x <= 16 && y >= 20);
                let upper = y == 20 && (13..=16).contains(&x) && (5..=9).contains(&z);
                let divider = x == 16 && y >= 21 && !(y == 21 && z == 7);
                let middle = y == 10 && (17..=19).contains(&x) && (6..=8).contains(&z);
                edit(
                    world,
                    [x, floor + y, z],
                    if wall || upper || divider || middle {
                        3
                    } else {
                        0
                    },
                )?;
            }
        }
    }
    edit(world, [20, floor + 11, 7], 82)?;
    edit(world, [18, floor + 11, 6], 74)?;
    edit(world, [15, floor + 21, 7], 27)?;
    Ok(())
}

fn downstream_volume(world: &World, floor: i64) -> u64 {
    world
        .resource::<EditJournal>()
        .fluid_cells()
        .filter(|(p, _)| {
            (17..=22).contains(&p[0])
                && (floor + 1..=floor + 22).contains(&p[1])
                && (5..=9).contains(&p[2])
        })
        .map(|(_, c)| u64::from(c.amount))
        .sum()
}

fn verify_render_batches(world: &mut World, floor: i64) -> Result<(), String> {
    let count = world
        .query::<(&Name, &Aabb, &GlobalTransform)>()
        .iter(world)
        .filter(|(name, bounds, transform)| {
            let center = transform.transform_point(Vec3::from(bounds.center));
            name.as_str().starts_with("Water ")
                && (12.0..24.0).contains(&center.x)
                && center.y > floor as f32
                && center.y < floor as f32 + 24.0
                && bounds.half_extents.y > 2.0
        })
        .count();
    if count < 2 {
        return Err(format!(
            "Expected real water batches across the vertical section seam, got {count}"
        ));
    }
    Ok(())
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-waterfalls"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(validate_capture);
    Ok(())
}

fn validate_capture(capture: On<ScreenshotCaptured>, mut state: ResMut<Fixture>) {
    state.pending = false;
    let image = match capture.image.clone().try_into_dynamic() {
        Ok(image) => image.into_rgba8(),
        Err(error) => {
            state.failure = Some(error.to_string());
            return;
        }
    };
    let data = image.as_raw();
    let width = image.width() as usize;
    let height = image.height() as usize;
    let stride = 4;
    let mut water = 0;
    let mut edges = 0;
    for y in height / 5..height * 4 / 5 {
        for x in width / 5..width * 4 / 5 {
            let p = (x + y * width) * stride;
            let [r, g, b] = [data[p], data[p + 1], data[p + 2]].map(u16::from);
            if g > r + 8 && b > r + 5 && g > 25 {
                water += 1;
            }
            let q = p + stride;
            if [0, 1, 2]
                .into_iter()
                .any(|i| data[p + i].abs_diff(data[q + i]) > 20)
            {
                edges += 1;
            }
        }
    }
    if water < 100 || edges < 100 {
        state.failure = Some(format!(
            "Waterfall render is blank or missing teal water: water_pixels={water}, edges={edges}"
        ));
        return;
    }
    info!(
        water_pixels = water,
        edges, "Waterfall screenshot has visible geometry and actual water coloration"
    );
    state.captures += 1;
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    fn exit_waits_for_preview_jobs_and_in_flight_screenshots() {
        let mut world = World::new();
        world.insert_resource(terra_render::RenderBlockPreviews {
            pending: 1,
            ..default()
        });
        assert!(!captures_finished(&mut world));
        world
            .resource_mut::<terra_render::RenderBlockPreviews>()
            .pending = 0;
        let capture = world.spawn(Screenshot::primary_window()).id();
        assert!(!captures_finished(&mut world));
        world.despawn(capture);
        assert!(captures_finished(&mut world));
    }
}
