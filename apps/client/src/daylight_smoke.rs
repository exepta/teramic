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
const CONTACT_VIEWS: [(&str, f64); 2] = [
    ("natural-water-contact-front", 12.0),
    ("natural-water-contact-back", 12.0),
];
fn capture_view(stage: usize) -> (&'static str, f64) {
    if stage < VIEWS.len() {
        VIEWS[stage]
    } else {
        CONTACT_VIEWS[stage - VIEWS.len()]
    }
}
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
    contact_smoke: bool,
    water_contacts: Vec<[i64; 3]>,
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
            contact_smoke: std::env::var("TERRA_WATER_CONTACT_SMOKE")
                .is_ok_and(|value| value == "1"),
            water_contacts: Vec::new(),
        })
        .add_systems(PostStartup, setup)
        .add_systems(PreUpdate, advance_fixture_clock)
        .add_systems(Last, run);
}
fn advance_fixture_clock(time: Res<Time<Real>>, mut clock: ResMut<WorldClock>, smoke: Res<Smoke>) {
    if (19..25).contains(&smoke.stage) || smoke.stage >= VIEWS.len() {
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
    if world.resource::<Smoke>().contact_smoke {
        match place_natural_water_contacts(world, point) {
            Ok(contacts) => world.resource_mut::<Smoke>().water_contacts = contacts,
            Err(reason) => {
                error!(%reason, "Natural-water contact fixture failed");
                world.resource_mut::<Smoke>().failed = true;
            }
        }
    }
    view(world, 0, point);
}
fn view(world: &mut World, stage: usize, water: Vec3) {
    let mut clock = world.resource_mut::<WorldClock>();
    if stage <= 19 || stage >= 25 && stage != VIEWS.len() + 1 {
        clock.phase = capture_view(stage).1 / 24.0;
    }
    clock.paused = !(19..25).contains(&stage) && stage < VIEWS.len();
    let eye = if stage == VIEWS.len() {
        water + Vec3::new(-8.0, 3.0, 5.0)
    } else if stage == VIEWS.len() + 1 {
        water + Vec3::new(8.0, 2.5, -10.0)
    } else if matches!(stage, 5 | 6 | 28 | 29) {
        water - Vec3::Y * 0.65
    } else {
        water + Vec3::new(27.0, 18.0, 32.0)
    };
    let target = match stage {
        stage if stage >= VIEWS.len() => water + Vec3::new(0.0, -0.35, -3.0),
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

fn place_natural_water_contacts(world: &mut World, water: Vec3) -> Result<Vec<[i64; 3]>, String> {
    use terra_core::plumbing;
    use terra_world::{
        edits::{BlockObjectState, EditJournal},
        streaming::{BlockEditRequest, ChunkStore, commit_block_edit, commit_object_state},
    };
    let terrain = world.resource::<WorldTerrain>().0.clone();
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    let mut parts = Vec::new();
    for (index, material) in [
        plumbing::WOOD_PIPE,
        plumbing::STONE_PIPE,
        plumbing::IRON_PIPE,
        plumbing::GLASS_PIPE,
        plumbing::VALVE,
        plumbing::PUMP,
    ]
    .into_iter()
    .enumerate()
    {
        let x = water.x.floor() as i64 - 5 + index as i64 * 2;
        let z = water.z.floor() as i64 - 3;
        let column = terrain
            .water_column(x as f64 + 0.5, z as f64 + 0.5)
            .ok_or("Contact row left the natural oasis")?;
        if column.depth < 1.5 || (column.level - f64::from(water.y)).abs() > 0.001 {
            return Err(format!(
                "Natural contact lacks a deep shared water surface at {x}/{z}: {column:?}"
            ));
        }
        let position = [x, column.level.ceil() as i64 - 1, z];
        let expected = generator.block_at(position).map_err(|e| e.to_string())?;
        let source = terra_world::water::fluid_cell_at(
            &terrain,
            world.resource::<EditJournal>(),
            position,
            expected,
        );
        if !source.source || source.amount == 0 {
            return Err(format!(
                "Contact must preserve existing natural source at {position:?}"
            ));
        }
        parts.push((
            position,
            material,
            if material == plumbing::PUMP { 0 } else { 4 },
            source,
        ));
    }
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| -> Result<(), String> {
        let mut journal = world.resource_mut::<EditJournal>();
        for &(position, material, facing, source) in &parts {
            if let Some(expected) = chunks.block_at(position) {
                if !commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material: expected,
                        material,
                    },
                    &properties,
                )? {
                    return Err(format!("Loaded natural contact rejected at {position:?}"));
                }
                commit_object_state(
                    &mut chunks,
                    &mut journal,
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )?;
            } else {
                journal
                    .record(position, material)
                    .map_err(|e| e.to_string())?;
                journal.set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )?;
            }
            journal
                .set_fluid(position, source)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })?;
    // Probe real generated CPU water geometry, including the edited halo. The
    // water worker adds the exact profiles later; the native views show those.
    let journal = world.resource::<EditJournal>();
    let mut moving = 0;
    let mut pinned = 0;
    let owners: std::collections::BTreeSet<_> = parts
        .iter()
        .map(|(p, _, _, _)| terra_voxel::global_to_section(*p).0)
        .collect();
    for owner in owners {
        let edits = journal.snapshot(owner).map_err(|e| e.to_string())?;
        let generated = generator
            .build(owner, 0, &edits, true)
            .map_err(|e| e.to_string())?;
        let Some(patch) = generated.water else {
            continue;
        };
        let origin = owner.origin().map_err(|e| e.to_string())?;
        for (point, &weight) in patch.positions.iter().zip(&patch.wave_weights) {
            moving += usize::from(weight > 0.0);
            let point: [f64; 3] =
                std::array::from_fn(|axis| f64::from(point[axis]) + origin[axis] as f64);
            pinned += usize::from(
                weight == 0.0
                    && parts.iter().any(|(p, _, _, _)| {
                        (point[0] - p[0] as f64).abs() <= 1.001
                            && (point[2] - p[2] as f64).abs() <= 1.001
                            && (point[1] - f64::from(water.y)).abs() < 0.001
                    }),
            );
        }
    }
    if moving == 0 || pinned == 0 {
        return Err(format!(
            "Natural water needs moving far waves and pinned contacts: moving={moving}, pinned={pinned}"
        ));
    }
    info!(moving, pinned, parts = ?parts, "Natural oasis contact row retains source water and active remote waves");
    Ok(parts
        .into_iter()
        .map(|(position, _, _, _)| position)
        .collect())
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
        let view_count = VIEWS.len()
            + if smoke.contact_smoke {
                CONTACT_VIEWS.len()
            } else {
                0
            };
        if smoke.stage >= view_count {
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
            smoke.stage = view_count;
            return;
        }
        if !world.resource::<terra_render::RenderStatus>().ready
            || !world.resource::<terra_render::WaterStatus>().ready
        {
            smoke.since = Instant::now();
            return;
        }
        if smoke.since.elapsed()
            < Duration::from_secs(if smoke.stage >= VIEWS.len() { 1 } else { 3 })
        {
            return;
        }
        if !smoke.requested {
            if smoke.stage >= VIEWS.len() {
                let chunks = world.resource::<terra_world::streaming::ChunkStore>();
                let journal = world.resource::<terra_world::edits::EditJournal>();
                if smoke.water_contacts.iter().any(|position| {
                    chunks.block_at(*position) != journal.block(*position)
                        || journal.object_state(*position).is_none()
                }) {
                    return;
                }
                if world
                    .resource::<terra_core::GameConfig>()
                    .client
                    .water_wave_strength
                    <= 0.0
                    || smoke.water_contacts.iter().any(|position| {
                        journal
                            .fluid(*position)
                            .is_none_or(|cell| !cell.source || cell.amount == 0)
                    })
                {
                    error!("Natural contact views lost active waves or preserved exterior sources");
                    smoke.failed = true;
                    return;
                }
            }
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
                || ((19..VIEWS.len()).contains(&smoke.stage) && lamps.is_empty())
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
                view = capture_view(smoke.stage).0,
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
                natural_water_contacts = smoke.water_contacts.len(),
                "Daylight capture state"
            );
            if submerged != matches!(smoke.stage, 5 | 6 | 28 | 29) {
                smoke.failed = true;
                return;
            }
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    directory.join(format!("{}.png", capture_view(smoke.stage).0)),
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
            if smoke.stage == view_count {
                info!(
                    captures = view_count,
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
