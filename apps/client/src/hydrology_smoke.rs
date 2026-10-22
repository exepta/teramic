//! Native acceptance path through the production generator, water shader and camera fog.
use bevy::{app::AppExit, prelude::*};
use std::time::{Duration, Instant};
use terra_player::TerraCamera;
use terra_world::streaming::{ChunkFocus, GenerationStatus, WorldGeneration, WorldTerrain};

#[derive(Resource)]
struct Fixture {
    benchmark: bool,
    tunnel: bool,
    bifurcation: Option<[Vec3; 2]>,
    started: Instant,
    since: Instant,
    phase: u8,
    point: Vec3,
    upper: f32,
    spray_check: bool,
}
pub(crate) fn install(app: &mut App) {
    let benchmark = std::env::args().any(|arg| arg == "--smoke-test-river-benchmark");
    if !benchmark && !std::env::args().any(|arg| arg == "--smoke-test-rivers") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(Fixture {
        benchmark,
        tunnel: std::env::var_os("TERRA_RIVER_TUNNEL").is_some(),
        bifurcation: None,
        started: now,
        since: now,
        phase: 0,
        point: Vec3::ZERO,
        upper: 0.0,
        spray_check: std::env::var_os("TERRA_RIVER_SPRAY").is_some()
            || std::env::var_os("TERRA_RIVER_MIST").is_some(),
    })
    .add_systems(PostStartup, locate)
    .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn locate(
    terrain: Res<WorldTerrain>,
    mut state: ResMut<Fixture>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut config: ResMut<terra_core::GameConfig>,
) {
    if state.spray_check {
        config.client.water_quality = terra_core::WaterQuality::High;
        config.client.water_volumetric_mist = std::env::var_os("TERRA_RIVER_MIST").is_some();
    }
    let baseline_view = std::env::var_os("TERRA_RIVER_BASELINE_VIEW").is_some();
    let best = if std::env::var_os("TERRA_RIVER_BIFURCATION").is_some() {
        find_bifurcation(&terrain.0).map(|[main, side, island]| {
            state.bifurcation = Some([side, island]);
            (main, main.y)
        })
    } else if state.tunnel {
        find_river_tunnel(&terrain.0).map(|point| (point, point.y))
    } else if baseline_view {
        // Preserve the measured pre-fix camera when comparing changed terrain.
        // This is a view anchor, not a claim that the old unsupported fall remains.
        Some((Vec3::new(95.5, 43.086_174, 404.5), 48.364_502))
    } else if std::env::var_os("TERRA_RIVER_OVERVIEW").is_some() {
        (-24..144)
            .filter_map(|x| {
                let x = f64::from(x) + 0.5;
                let river = terrain.0.river_at(x, 24.5)?;
                Some((
                    river.distance,
                    Vec3::new(x as f32, river.level as f32, 24.5),
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, point)| (point, point.y))
    } else {
        let minimum = std::env::var("TERRA_RIVER_MIN_FALL")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| (5.0..=256.0).contains(value))
            .unwrap_or(terra_world::water::MIN_WATERFALL_DROP);
        find_waterfall_at_least(&terrain.0, minimum, state.spray_check)
    };
    if let Some((point, upper)) = best {
        state.point = point;
        state.upper = upper;
        for mut camera in &mut cameras {
            if state.spray_check {
                let Some(view) = waterfall_overview(&terrain.0, point, upper) else {
                    state.phase = 254;
                    return;
                };
                *camera = view;
                focus.0 = view.translation.to_array().map(f64::from);
                continue;
            }
            let offset = if state.bifurcation.is_some() {
                Vec3::new(15.0, 24.0, -32.0)
            } else if state.tunnel {
                let origin = (f64::from(point.x) / 512.0).round() * 512.0;
                let x = river_center(
                    &terrain.0,
                    origin,
                    f64::from(point.x),
                    f64::from(point.z) - 12.0,
                )
                .map_or(point.x, |(x, _)| x as f32);
                Vec3::new(x - point.x, 2.5, -12.0)
            } else if upper - point.y >= 25.0 && !baseline_view {
                Vec3::new(0.5, upper - point.y - 1.0, 2.0)
            } else {
                Vec3::new(7.0, (upper - point.y).max(2.0) + 4.0, 12.0)
            };
            let target_height = if state.tunnel {
                2.0
            } else {
                (upper - point.y) * 0.5
            };
            *camera = Transform::from_translation(point + offset)
                .looking_at(point + Vec3::Y * target_height, Vec3::Y);
            focus.0 = camera.translation.to_array().map(f64::from);
        }
        tracing::info!(
            ?point,
            upper,
            baseline_view,
            tunnel = state.tunnel,
            bifurcation = ?state.bifurcation,
            "Native river fixture view anchor (automatic mode uses a generated reach)"
        );
    } else {
        state.phase = 254;
    }
}

fn waterfall_overview(
    terrain: &terra_world::TerraGenerator,
    point: Vec3,
    upper: f32,
) -> Option<Transform> {
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    let target = point + Vec3::Y * (upper - point.y) * 0.5 + Vec3::Z;
    let air = |position: Vec3| {
        generator
            .block_at(position.floor().to_array().map(|v| v as i64))
            .ok()
            == Some(0)
    };
    for distance in [32.0, 24.0, 16.0, 42.0] {
        for lateral in [0.0, 8.0, -8.0, 16.0, -16.0] {
            for elevation in [4.0, 12.0, 24.0] {
                let eye = Vec3::new(point.x + lateral, upper + elevation, point.z + distance);
                let clear_camera = [-0.25, 0.25].into_iter().all(|x| {
                    [-0.25, 0.25].into_iter().all(|y| {
                        [-0.25, 0.25]
                            .into_iter()
                            .all(|z| air(eye + Vec3::new(x, y, z)))
                    })
                });
                if !clear_camera {
                    continue;
                }
                let delta = target - eye;
                let steps = (delta.length() / 0.5).ceil() as u32;
                if (0..steps.saturating_sub(3))
                    .all(|step| air(eye + delta * (step as f32 / steps as f32)))
                {
                    info!(
                        ?eye,
                        ?target,
                        "Waterfall overview verified outside solid terrain"
                    );
                    return Some(Transform::from_translation(eye).looking_at(target, Vec3::Y));
                }
            }
        }
    }
    None
}

/// Fixture-only search. Follow each cached path at eight-meter intervals and
/// inspect individual cells only when the coarse samples contain a real cliff.
pub(crate) fn find_waterfall(terrain: &terra_world::TerraGenerator) -> Option<(Vec3, f32)> {
    find_waterfall_at_least(terrain, terra_world::water::MIN_WATERFALL_DROP, false)
}

fn find_waterfall_at_least(
    terrain: &terra_world::TerraGenerator,
    minimum: f64,
    clear_overview: bool,
) -> Option<(Vec3, f32)> {
    let maximum_ring = if minimum >= 30.0 { 12 } else { 8 };
    for ring in 0_i32..=maximum_ring {
        for cz in -ring..=ring {
            for cx in -ring..=ring {
                if cx.abs().max(cz.abs()) != ring {
                    continue;
                }
                let origin = f64::from(cx * 512);
                let start = f64::from(cz * 512) - 191.5;
                let mut previous = None;
                let mut guess = origin + 64.0;
                for step in 0..=48 {
                    let z = (start + f64::from(step * 8)).min(start + 383.0);
                    let Some((x, river)) = river_center(terrain, origin, guess, z) else {
                        continue;
                    };
                    if let Some((old_z, old_level)) = previous
                        && old_level - river.level >= minimum
                        && let Some(found) = refine_waterfall(terrain, origin, x, old_z, minimum)
                        && (!clear_overview
                            || waterfall_overview(terrain, found.0, found.1).is_some())
                    {
                        return Some(found);
                    }
                    previous = Some((z, river.level));
                    guess = x;
                }
            }
        }
    }
    None
}

fn river_center(
    terrain: &terra_world::TerraGenerator,
    origin: f64,
    guess: f64,
    z: f64,
) -> Option<(f64, terra_world::generation::RiverSample)> {
    let (x, sample) = terrain.river_at(guess, z).map(|r| (guess, r)).or_else(|| {
        (0..8).find_map(|step| {
            let x = origin + 16.0 + f64::from(step) * 16.0;
            terrain.river_at(x, z).map(|r| (x, r))
        })
    })?;
    let plus = terrain.river_at(x + 0.01, z);
    let center = if plus.is_some_and(|next| next.distance < sample.distance) {
        x + sample.distance
    } else {
        x - sample.distance
    };
    terrain.river_at(center, z).map(|r| (center, r))
}

fn refine_waterfall(
    terrain: &terra_world::TerraGenerator,
    origin: f64,
    guess: f64,
    start: f64,
    minimum: f64,
) -> Option<(Vec3, f32)> {
    for step in 0..8 {
        let z = start + f64::from(step);
        let (center, _) = river_center(terrain, origin, guess, z)?;
        let x = center.floor() + 0.5;
        let Some(upper) = terrain.water_column(x, z) else {
            continue;
        };
        let Some(lower) = terrain.water_column(x, z + 1.0) else {
            continue;
        };
        if upper.level - lower.level < minimum {
            continue;
        }
        // Both native fixtures dive downstream at +2/+3 meters.
        if [3.0, 4.0].into_iter().any(|offset| {
            terrain
                .water_column(x, z + offset)
                .is_none_or(|column| column.depth < 1.2)
        }) {
            continue;
        }
        return Some((
            Vec3::new(x as f32, lower.level as f32, (z + 1.0) as f32),
            upper.level as f32,
        ));
    }
    None
}

fn find_river_tunnel(terrain: &terra_world::TerraGenerator) -> Option<Vec3> {
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    for ring in 0_i32..=4 {
        for cz in -ring..=ring {
            for cx in -ring..=ring {
                if cx.abs().max(cz.abs()) != ring {
                    continue;
                }
                let origin = f64::from(cx * 512);
                let start = f64::from(cz * 512) - 191.5;
                let mut guess = origin + 64.0;
                let mut open_run = 0;
                for step in 0..384 {
                    let z = start + f64::from(step);
                    let Some((center, river)) = river_center(terrain, origin, guess, z) else {
                        open_run = 0;
                        continue;
                    };
                    guess = center;
                    let x = center.floor() + 0.5;
                    let column = terrain.surface_column(x, z);
                    let roof = column
                        .river_cavity
                        .map_or(f64::NEG_INFINITY, |c| column.height - c[1]);
                    if river.roof_blend == 0.0 || roof < 0.5 {
                        open_run += 1;
                        continue;
                    }
                    if roof < 2.0 {
                        continue;
                    }
                    if open_run < 16 {
                        open_run = 0;
                        continue;
                    }
                    open_run = 0;
                    // A wet entrance with real roof, enough headroom and water beyond it.
                    let Some([_, ceiling]) = column.river_cavity else {
                        continue;
                    };
                    let Some(water) = terrain.water_column(x, z) else {
                        continue;
                    };
                    if ceiling - water.level < 3.0
                        || column.height - ceiling < 2.0
                        || terrain
                            .water_column(x, z + 3.0)
                            .is_none_or(|w| w.depth < 1.2)
                    {
                        continue;
                    }
                    let point = Vec3::new(x as f32, water.level as f32, z as f32);
                    let [air, roof] = tunnel_voxels(point, ceiling);
                    if generator.block_at(air).ok() != Some(0)
                        || generator.block_at(roof).ok().is_none_or(|id| id == 0)
                    {
                        continue;
                    }
                    return Some(point);
                }
            }
        }
    }
    None
}

fn tunnel_voxels(point: Vec3, ceiling: f64) -> [[i64; 3]; 2] {
    let air = [
        point.x.floor() as i64,
        (point.y + 1.0).floor() as i64,
        point.z.floor() as i64,
    ];
    // Cells sample at y + 0.5. ceil(ceiling) + 1 can already lie above
    // a valid two-metre roof at a partially eroded portal.
    let roof = [air[0], ceiling.ceil() as i64, air[2]];
    [air, roof]
}

fn find_bifurcation(terrain: &terra_world::TerraGenerator) -> Option<[Vec3; 3]> {
    for ring in 0_i32..=4 {
        for cz in -ring..=ring {
            for cx in -ring..=ring {
                if cx.abs().max(cz.abs()) != ring {
                    continue;
                }
                let origin = f64::from(cx * 512);
                let start = f64::from(cz * 512) - 191.5;
                for step in 4..45 {
                    let z = start + f64::from(step * 8);
                    let mut runs = Vec::new();
                    let mut first = None;
                    for dx in -32..=176 {
                        let x = origin + f64::from(dx) + 0.5;
                        let wet = terrain
                            .river_at(x, z)
                            .is_some_and(|r| r.distance < r.width * 0.7 && r.roof_blend < 0.05);
                        if wet {
                            first.get_or_insert(x);
                        } else if let Some(begin) = first.take()
                            && x - begin >= 3.0
                        {
                            runs.push([begin, x - 1.0]);
                        }
                    }
                    for pair in runs.windows(2) {
                        if pair[1][0] - pair[0][1] < 5.0 {
                            continue;
                        }
                        let a = ((pair[0][0] + pair[0][1]) * 0.5).floor() + 0.5;
                        let b = ((pair[1][0] + pair[1][1]) * 0.5).floor() + 0.5;
                        let middle = ((pair[0][1] + pair[1][0]) * 0.5).floor() + 0.5;
                        let Some(left) = terrain.water_column(a, z) else {
                            continue;
                        };
                        let Some(right) = terrain.water_column(b, z) else {
                            continue;
                        };
                        if left.level != right.level || terrain.water_column(middle, z).is_some() {
                            continue;
                        }
                        let (main, side) = if left.depth >= right.depth {
                            (a, b)
                        } else {
                            (b, a)
                        };
                        if terrain
                            .water_column(main, z + 3.0)
                            .is_none_or(|w| w.depth < 1.2)
                        {
                            continue;
                        }
                        return Some([
                            Vec3::new(main as f32, left.level as f32, z as f32),
                            Vec3::new(side as f32, left.level as f32, z as f32),
                            Vec3::new(
                                middle as f32,
                                terrain.surface_height(middle, z) as f32,
                                z as f32,
                            ),
                        ]);
                    }
                }
            }
        }
    }
    None
}

fn capture(world: &mut World, name: &str) {
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-rivers"));
    if let Err(error) = std::fs::create_dir_all(&root) {
        tracing::error!(%error, "Capture path failed");
    }
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            root.join(format!("{name}.png")),
        ));
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        if state.benchmark { return; }
        if state.phase == 255 { return; }
        if state.phase == 254 || state.started.elapsed() > Duration::from_secs(90) {
            tracing::error!(phase=state.phase, "River fixture failed or timed out");
            world.write_message(AppExit::error()); state.phase = 255; return;
        }
        if world.resource::<WorldGeneration>().status != GenerationStatus::Ready
            || !world.resource::<terra_render::RenderStatus>().ready
            || !world.resource::<terra_render::WaterStatus>().ready
            || state.since.elapsed() < Duration::from_secs(3) { return; }
        if state.phase == 0 {
            if world.resource::<terra_render::WaterStatus>().triangles == 0 { return; }
            if state.tunnel {
                let column = world.resource::<WorldTerrain>().0.surface_column(f64::from(state.point.x), f64::from(state.point.z));
                let Some([_, ceiling]) = column.river_cavity else { state.phase = 254; return; };
                let chunks = world.resource::<terra_world::streaming::ChunkStore>();
                let [air, roof] = tunnel_voxels(state.point, ceiling);
                if chunks.block_at(air) != Some(0) || chunks.block_at(roof).is_none_or(|material| material == 0) {
                    tracing::error!(?air, ?roof, air_material=?chunks.block_at(air), roof_material=?chunks.block_at(roof), "Tunnel fixture needs a carved passage and solid roof");
                    state.phase = 254; return;
                }
            }
            if let Some([side, island]) = state.bifurcation {
                let terrain = &world.resource::<WorldTerrain>().0;
                let main_water = terrain.water_column(f64::from(state.point.x), f64::from(state.point.z));
                let side_water = terrain.water_column(f64::from(side.x), f64::from(side.z));
                let dry_island = terrain.water_column(f64::from(island.x), f64::from(island.z)).is_none();
                let ground = [island.x.floor() as i64, (island.y - 0.5).floor() as i64, island.z.floor() as i64];
                let island_solid = world.resource::<terra_world::streaming::ChunkStore>().block_at(ground).is_some_and(|material| material != 0);
                if !dry_island || !island_solid || main_water.zip(side_water).is_none_or(|(a,b)| a.level != b.level) {
                    tracing::error!(?ground, dry_island, island_solid, "River bifurcation requires two wet arms separated by a real island");
                    state.phase = 254; return;
                }
                tracing::info!(main=?state.point, ?side, ?island, "Native river bifurcation has two rendered channels and solid dry island");
            }
            capture(world, if state.bifurcation.is_some() { "river-bifurcation" } else if state.tunnel { "river-tunnel" } else if std::env::var_os("TERRA_RIVER_OVERVIEW").is_some() { "river-overview" } else { "river-waterfall" });
            state.phase = 6;
        } else if state.phase == 6 {
            capture(world, "river-motion");
            if state.spray_check {
                let eye = state.point + Vec3::new(0.5, 2.0, 4.0);
                let chunks = world.resource::<terra_world::streaming::ChunkStore>();
                if ![-0.25, 0.25].into_iter().all(|x| [-0.25, 0.25].into_iter().all(|y| [-0.25, 0.25].into_iter().all(|z| chunks.block_at((eye + Vec3::new(x, y, z)).floor().to_array().map(|v| v as i64)) == Some(0)))) {
                    tracing::error!(?eye, "Waterfall close-up must remain outside solid terrain");
                    state.phase = 254; return;
                }
                for mut camera in world.query_filtered::<&mut Transform, With<TerraCamera>>().iter_mut(world) {
                    *camera = Transform::from_translation(eye).looking_at(state.point + Vec3::Y * 1.2, Vec3::Y);
                }
                world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
                state.phase = 11;
            } else if state.upper - state.point.y >= 25.0 {
                let eye = state.point + Vec3::new(0.5, 2.0, 2.0);
                for mut camera in world.query_filtered::<&mut Transform, With<TerraCamera>>().iter_mut(world) {
                    *camera = Transform::from_translation(eye).looking_at(state.point + Vec3::Y * ((state.upper - state.point.y) * 0.75), Vec3::Y);
                }
                world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
                state.phase = 7;
            } else {
                state.phase = 1;
            }
        } else if state.phase == 7 {
            capture(world, "river-fall-base");
            state.phase = if state.spray_check { 10 } else { 1 };
        } else if state.phase == 11 {
            let count = world.resource::<terra_render::WaterStatus>().spray_particles;
            if count == 0 || count > 96 {
                tracing::error!(count, "Waterfall spray must use a bounded live batch");
                state.phase = 254; return;
            }
            tracing::info!(count, "Native waterfall spray batch visible");
            if std::env::var_os("TERRA_RIVER_MIST").is_some() {
                let volumes = world.resource::<terra_render::WaterStatus>().mist_volumes;
                if volumes == 0 || volumes > 4 {
                    tracing::error!(volumes, "Native optional waterfall mist must have bounded actual volumes");
                    state.phase = 254; return;
                }
                tracing::info!(volumes, "Native raymarched waterfall mist active");
            }
            world.resource_mut::<terra_core::GameConfig>().client.water_quality = terra_core::WaterQuality::Low;
            state.phase = 8;
        } else if state.phase == 8 {
            if world.resource::<terra_render::WaterStatus>().spray_particles != 0 {
                tracing::error!("Low water quality must disable spray");
                state.phase = 254; return;
            }
            if world.resource::<terra_render::WaterStatus>().mist_volumes != 0 {
                tracing::error!("Low water quality must remove volumetric mist");
                state.phase = 254; return;
            }
            capture(world, "river-waterfall-low");
            world.resource_mut::<terra_core::GameConfig>().client.water_quality = terra_core::WaterQuality::High;
            state.phase = 9;
        } else if state.phase == 9 {
            let count = world.resource::<terra_render::WaterStatus>().spray_particles;
            if count == 0 || count > 96 { state.phase = 254; return; }
            capture(world, "river-waterfall-high");
            let eye = state.point + Vec3::new(0.5, 2.0, 4.0);
            for mut camera in world.query_filtered::<&mut Transform, With<TerraCamera>>().iter_mut(world) {
                *camera = Transform::from_translation(eye).looking_at(state.point + Vec3::Y * 1.2, Vec3::Y);
            }
            world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
            state.phase = 7;
        } else if state.phase == 10 {
            capture(world, "river-fall-base-motion");
            state.phase = 1;
        } else if state.phase == 1 {
            let x = state.point.x;
            let z = state.point.z + 3.0;
            let column = world.resource::<WorldTerrain>().0.water_column(f64::from(x), f64::from(z));
            let Some(column) = column else { state.phase = 254; return; };
            let eye = Vec3::new(x, column.level as f32 - 0.8, z);
            for mut camera in world.query_filtered::<&mut Transform, With<TerraCamera>>().iter_mut(world) {
                *camera = Transform::from_translation(eye).looking_at(eye + Vec3::new(0.1, -0.15, 1.0), Vec3::Y);
            }
            world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
            state.phase = 2;
        } else if state.phase == 2 {
            let environment = world.resource::<terra_render::UnderwaterState>();
            if !environment.submerged || environment.blend < 0.95 { state.phase = 254; return; }
            capture(world, "river-underwater");
            state.phase = 3;
        } else if state.phase == 3 {
            for mut camera in world.query_filtered::<&mut Transform, With<TerraCamera>>().iter_mut(world) {
                camera.translation.y = state.point.y + 2.0;
            }
            state.phase = 4;
        } else if state.phase == 4 {
            let environment = world.resource::<terra_render::UnderwaterState>();
            if environment.submerged || environment.blend > 0.05 { state.phase = 254; return; }
            capture(world, "river-surface"); state.phase = 5;
        } else {
            tracing::info!("Native river fixture passed: generated river/fall, underwater fog and environment restoration");
            world.write_message(AppExit::Success); state.phase = 255;
        }
        state.since = Instant::now();
    });
}

#[cfg(test)]
mod tunnel_tests {
    use super::*;

    #[test]
    fn roof_probe_stays_inside_two_metre_portal_cover() {
        for fraction in 0..100 {
            let ceiling = 41.0 + f64::from(fraction) * 0.01;
            let [_, roof] = tunnel_voxels(Vec3::new(-481.5, 35.508, -640.5), ceiling);
            let centre = roof[1] as f64 + 0.5;
            assert!(centre > ceiling && centre < ceiling + 2.0);
        }
    }

    #[test]
    fn native_portal_fixture_has_real_air_and_roof_with_caves_and_trees() {
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            rivers: true,
            ..Default::default()
        })
        .unwrap();
        let point =
            find_river_tunnel(&terrain).expect("default seed needs a visible river entrance");
        let column = terrain.surface_column(f64::from(point.x), f64::from(point.z));
        let [_, ceiling] = column.river_cavity.unwrap();
        let [air, roof] = tunnel_voxels(point, ceiling);
        let generator = terra_world::SectionGenerator::new(terrain);
        assert_eq!(generator.block_at(air).unwrap(), 0);
        assert_ne!(generator.block_at(roof).unwrap(), 0);
    }
}
