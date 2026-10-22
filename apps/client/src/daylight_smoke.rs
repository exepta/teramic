//! Native celestial and underwater regression views using generated terrain.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::time::{Duration, Instant};
use terra_core::{CaveQuality, GraphicsConfig, ShadowQuality, WorldClock};
use terra_player::TerraCamera;
use terra_world::streaming::{ChunkFocus, WorldTerrain};

const VIEWS: [(&str, f64); 31] = [
    ("day", 12.0),
    ("dawn", 6.2),
    ("dusk", 17.8),
    ("night", 0.0),
    ("moon-stars", 0.0),
    ("underwater-day", 12.0),
    ("underwater-night", 0.0),
    ("sky-no-fog", 0.0),
    ("voxel-sun", 12.0),
    ("evening-1900", 19.0),
    ("evening-2030", 20.5),
    ("evening-2200", 22.0),
    ("late-day-1700", 17.0),
    ("late-day-1730", 17.5),
    ("sunset-1800", 18.0),
    ("dawn-shadow-before", 6.20),
    ("dawn-shadow-after", 6.22),
    ("dusk-shadow-before", 17.78),
    ("dusk-shadow-after", 17.80),
    ("moving-shadow-0", 17.80),
    ("moving-shadow-1", 17.80),
    ("moving-shadow-2", 17.80),
    ("moving-shadow-3", 17.80),
    ("moving-shadow-4", 17.80),
    ("moving-shadow-5", 17.80),
    ("shadow-off", 18.1),
    ("shadow-low-msaa", 18.1),
    ("shadow-high-taa", 18.1),
    ("underwater-msaa", 12.0),
    ("underwater-taa", 12.0),
    ("dawn-shadow-overlap", 5.9),
];
#[derive(Resource)]
struct Smoke {
    started: Instant,
    since: Instant,
    stage: usize,
    captures: usize,
    requested: bool,
    failed: bool,
    water: Vec3,
    torches: Vec<[i64; 3]>,
    lamp_entities: Vec<Entity>,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-daylight") {
        return;
    }
    let now = Instant::now();
    let graphics = terra_core::GraphicsConfig::from_client(
        &app.world().resource::<terra_core::GameConfig>().client,
    );
    app.insert_resource(graphics);
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Smoke {
            started: now,
            since: now,
            stage: 0,
            captures: 0,
            requested: false,
            failed: false,
            water: Vec3::ZERO,
            torches: Vec::new(),
            lamp_entities: Vec::new(),
        })
        .add_systems(PostStartup, setup)
        .add_systems(PreUpdate, advance_fixture_clock)
        .add_systems(Last, run);
}
fn advance_fixture_clock(time: Res<Time<Real>>, mut clock: ResMut<WorldClock>, smoke: Res<Smoke>) {
    if (19..25).contains(&smoke.stage) {
        clock.advance(time.delta_secs_f64());
    }
}
fn setup(world: &mut World) {
    let terrain = &world.resource::<WorldTerrain>().0;
    let point = terrain.oasis_at(-2682.0, -153.0).and_then(|oasis| {
        terrain
            .water_column(oasis.center[0], oasis.center[1])
            .map(|water| {
                Vec3::new(
                    oasis.center[0] as f32,
                    water.level as f32,
                    oasis.center[1] as f32,
                )
            })
    });
    let Some(point) = point else {
        world.resource_mut::<Smoke>().failed = true;
        return;
    };
    world.resource_mut::<Smoke>().water = point;
    view(world, 0, point);
}
fn view(world: &mut World, stage: usize, water: Vec3) {
    let mut clock = world.resource_mut::<WorldClock>();
    if stage <= 19 || stage >= 25 {
        clock.phase = VIEWS[stage].1 / 24.0;
    }
    clock.paused = !(19..25).contains(&stage);
    let eye = if matches!(stage, 5 | 6 | 28 | 29) {
        water - Vec3::Y * 0.65
    } else {
        water + Vec3::new(27.0, 18.0, 32.0)
    };
    let target = match stage {
        1 => eye + Vec3::new(30.0, 1.0, 0.0),
        2 => eye + Vec3::new(-30.0, 1.0, 0.0),
        4 | 7 | 8 => eye + Vec3::new(0.15, 0.9, 0.4358899) * 30.0,
        5 | 6 | 28 | 29 => water + Vec3::new(10.0, -1.0, -12.0),
        _ => water + Vec3::Y * 8.0,
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
    world.resource_mut::<ChunkFocus>().0 = eye.to_array().map(f64::from);
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .fog_distance = if stage == 7 { 0.0 } else { 1.0 };
    let mut graphics = world.resource_mut::<GraphicsConfig>();
    graphics.shadow_quality = match stage {
        25 => ShadowQuality::Off,
        26 | 28 => ShadowQuality::Low,
        _ => ShadowQuality::High,
    };
    graphics.cave_quality = match stage {
        28 => CaveQuality::Low,
        29 | 30 => CaveQuality::Ultra,
        _ => CaveQuality::Medium,
    };
}

fn place_fixture_torches(world: &mut World, water: Vec3) -> Option<Vec<[i64; 3]>> {
    let terrain = &world.resource::<WorldTerrain>().0;
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let mut torches = Vec::new();
    for z in 0..8 {
        for x in 0..8 {
            let x = water.x.floor() as i64 + 15 + x * 2;
            let z = water.z.floor() as i64 + 15 + z * 2;
            let ground =
                (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64;
            for y in ground - 1..=ground + 2 {
                let p = [x, y + 1, z];
                if chunks.block_at(p) == Some(0)
                    && chunks.is_wanted(terra_voxel::global_to_section(p).0)
                    && chunks
                        .block_at([x, y, z])
                        .and_then(terra_voxel::MaterialId::from_raw)
                        .is_some_and(terra_voxel::MaterialId::is_opaque)
                {
                    torches.push(p);
                    break;
                }
            }
            if torches.len() == 24 {
                break;
            }
        }
        if torches.len() == 24 {
            break;
        }
    }
    if torches.len() < 16 {
        error!(
            count = torches.len(),
            "Daylight fixture lacks loaded dry torch supports"
        );
        return None;
    }
    if let Err(reason) = commit_fixture_torches(world, &torches) {
        error!(%reason, "Daylight fixture torch commit failed");
        return None;
    }
    Some(torches)
}

fn commit_fixture_torches(world: &mut World, torches: &[[i64; 3]]) -> Result<(), String> {
    use terra_world::streaming::{BlockEditRequest, ChunkStore, commit_block_edit};
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let mut journal = world.resource_mut::<terra_world::edits::EditJournal>();
        for &position in torches {
            if !commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position,
                    expected_material: 0,
                    material: 40,
                },
                &properties,
            )? {
                return Err(format!("Torch placement rejected at {position:?}"));
            }
            if chunks.block_at(position) != Some(40) || journal.block(position) != Some(40) {
                return Err(format!(
                    "Torch commit did not update loaded data at {position:?}"
                ));
            }
        }
        Ok(())
    })
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut smoke: Mut<Smoke>| {
        if smoke.stage >= VIEWS.len() {
            return;
        }
        if smoke.failed
            || smoke.started.elapsed() > Duration::from_secs(180)
            || world
                .resource::<terra_render::RenderStatus>()
                .error
                .is_some()
        {
            error!("Daylight native smoke failed");
            world.write_message(AppExit::error());
            smoke.stage = VIEWS.len();
            return;
        }
        if !world.resource::<terra_render::RenderStatus>().ready
            || !world.resource::<terra_render::WaterStatus>().ready
        {
            smoke.since = Instant::now();
            return;
        }
        if smoke.since.elapsed() < Duration::from_secs(3) {
            return;
        }
        if !smoke.requested {
            let directory = std::env::var_os("TERRA_CAPTURE_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("terra-daylight"));
            if std::fs::create_dir_all(&directory).is_err() {
                smoke.failed = true;
                return;
            }
            let submerged = world.resource::<terra_render::UnderwaterState>().submerged;
            if smoke.stage == 24 && world.resource::<WorldClock>().hour() <= 17.81 {
                error!("Moving-shadow fixture clock did not advance");
                smoke.failed = true;
                return;
            }
            let temporal = world
                .query_filtered::<&bevy::anti_alias::taa::TemporalAntiAliasing, With<TerraCamera>>()
                .iter(world)
                .next()
                .is_some();
            let graphics = world.resource::<GraphicsConfig>();
            let shadows = graphics.shadow_quality != ShadowQuality::Off;
            let expected_temporal = matches!(
                graphics.shadow_quality,
                ShadowQuality::Medium | ShadowQuality::High
            );
            let budget = match graphics.cave_quality {
                CaveQuality::Low => 4,
                CaveQuality::Medium => 8,
                CaveQuality::High => 12,
                CaveQuality::Ultra => 16,
            };
            let msaa = world
                .query_filtered::<&Msaa, With<TerraCamera>>()
                .iter(world)
                .next()
                .copied();
            if temporal != expected_temporal
                || msaa != Some(if temporal { Msaa::Off } else { Msaa::Sample4 })
                || !world.resource::<terra_render::TorchLightingStatus>().ready
            {
                error!(
                    temporal,
                    ?msaa,
                    "Daylight antialiasing or real engine soft-core contract failed"
                );
                smoke.failed = true;
                return;
            }
            let mut shadow_casters = 0;
            for (light, layers) in world
                .query::<(
                    &DirectionalLight,
                    Option<&bevy::camera::visibility::RenderLayers>,
                )>()
                .iter(world)
            {
                if layers.is_some_and(|layers| {
                    !layers.intersects(&bevy::camera::visibility::RenderLayers::default())
                }) {
                    continue;
                }
                if light.shadow_maps_enabled != (shadows && light.illuminance > 0.0) {
                    error!(
                        lux = light.illuminance,
                        "Active celestial light lost its shadow map"
                    );
                    smoke.failed = true;
                    return;
                }
                shadow_casters += usize::from(light.shadow_maps_enabled);
            }
            let mut lamps = Vec::new();
            let mut local_shadow_casters = 0;
            for (entity, name, light) in world.query::<(Entity, &Name, &PointLight)>().iter(world) {
                if name.as_str() != "Voxel local light" && name.as_str() != "Carried torch light" {
                    continue;
                }
                if !shadows && light.shadow_maps_enabled {
                    error!(?entity, "Disabled local shadows were re-enabled");
                    smoke.failed = true;
                    return;
                }
                local_shadow_casters += usize::from(light.shadow_maps_enabled);
                if name.as_str() == "Voxel local light" {
                    lamps.push(entity);
                }
            }
            lamps.sort_unstable();
            if lamps.len() > budget
                || (smoke.stage >= 19 && lamps.is_empty())
                || (matches!(smoke.stage, 25..=27) && shadows && local_shadow_casters == 0)
                || (matches!(smoke.stage, 25..=27 | 30) && shadows && shadow_casters != 2)
            {
                error!(
                    lamps = lamps.len(),
                    budget,
                    shadow_casters,
                    local_shadow_casters,
                    "Daylight fixture light lifecycle or budget failed"
                );
                smoke.failed = true;
                return;
            }
            if smoke.stage == 24 {
                smoke.lamp_entities = lamps.clone();
            } else if matches!(smoke.stage, 25..=27) && lamps != smoke.lamp_entities {
                error!("Shadow-only roundtrip replaced the existing local light pool");
                smoke.failed = true;
                return;
            }
            let (exposure, ambient) = world
                .query_filtered::<(&bevy::camera::Exposure, &AmbientLight), With<TerraCamera>>()
                .iter(world)
                .next()
                .map(|(exposure, ambient)| (exposure.ev100, ambient.brightness))
                .unzip();
            info!(
                view = VIEWS[smoke.stage].0,
                ?exposure,
                ?ambient,
                submerged,
                hour = world.resource::<WorldClock>().hour(),
                shadow_casters,
                local_lights = lamps.len(),
                local_shadow_casters,
                budget,
                temporal,
                ?msaa,
                fixture_torches = smoke.torches.len(),
                "Daylight capture state"
            );
            if submerged != matches!(smoke.stage, 5 | 6 | 28 | 29) {
                smoke.failed = true;
                return;
            }
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    directory.join(format!("{}.png", VIEWS[smoke.stage].0)),
                ))
                .observe(
                    |capture: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
                        smoke.captures += 1;
                        if let Some(data) = &capture.image.data {
                            let min = data.chunks_exact(4).map(|p| p[0]).min().unwrap_or(0);
                            let max = data.chunks_exact(4).map(|p| p[0]).max().unwrap_or(0);
                            smoke.failed |= max.saturating_sub(min) < 12;
                        } else {
                            smoke.failed = true;
                        }
                    },
                );
            smoke.requested = true;
        } else if smoke.captures > smoke.stage {
            smoke.stage += 1;
            if smoke.stage == VIEWS.len() {
                info!(
                    captures = VIEWS.len(),
                    "Daylight native smoke passed: celestial and underwater captures"
                );
                world.write_message(AppExit::Success);
            } else {
                view(world, smoke.stage, smoke.water);
                if smoke.stage == 19 {
                    match place_fixture_torches(world, smoke.water) {
                        Some(torches) => smoke.torches = torches,
                        None => smoke.failed = true,
                    }
                }
                smoke.requested = false;
                smoke.since = Instant::now();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_torches_commit_loaded_voxels_and_invalidate_the_previous_mesh_epoch() {
        use terra_world::{
            edits::EditJournal,
            streaming::{ChunkStore, LoadedSection},
        };
        let mut world = World::new();
        world.init_resource::<ChunkStore>();
        world.init_resource::<EditJournal>();
        world.insert_resource(
            terra_core::block_properties::BlockPropertiesCatalog::load(
                &terra_core::asset_directory().join("terra/blocks"),
            )
            .unwrap(),
        );
        let section = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        for x in [2, 4] {
            data.set_block([x, 2, 4], 3).unwrap();
        }
        let revision = data.revision();
        world.resource_mut::<ChunkStore>().loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let positions = [[2, 3, 4], [4, 3, 4]];
        commit_fixture_torches(&mut world, &positions).unwrap();
        let chunks = world.resource::<ChunkStore>();
        let journal = world.resource::<EditJournal>();
        for p in positions {
            assert_eq!(chunks.block_at(p), Some(40));
            assert_eq!(journal.block(p), Some(40));
        }
        assert_eq!(chunks.loaded[&section].data.revision(), revision + 2);
        assert_ne!(
            chunks.loaded[&section].mesh_epoch,
            journal.mesh_epoch(section)
        );
    }
}
