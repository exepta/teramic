//! Native acceptance through real block edits, the chunk renderer and fog source index.
use bevy::{
    app::AppExit,
    ecs::system::SystemParam,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{
    ChunkBorderGizmos, FogBlockStatus, RenderStatus, RenderedSections, SpiritMistStatus,
};
use terra_voxel::global_to_section;
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkFocus, ChunkStore, WorldAccess, WorldTerrain},
};

type Plan = BTreeMap<[i64; 3], u16>;
const CENTER: i64 = 16;
const FOG: u16 = 430;

#[derive(Resource)]
struct Review {
    started: Instant,
    since: Instant,
    last_heartbeat: Instant,
    last_retry: Instant,
    stage: u8,
    base: i64,
    pending: bool,
    failed: bool,
    captures: usize,
    exposed_generation: u64,
    buried_generation: u64,
    first_drift: [f32; 2],
    measurements: Vec<serde_json::Value>,
}

pub(super) fn install(app: &mut App) {
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(Review {
            started: now,
            since: now,
            last_heartbeat: now,
            last_retry: now,
            stage: 0,
            base: 0,
            pending: false,
            failed: false,
            captures: 0,
            exposed_generation: 0,
            buried_generation: 0,
            first_drift: [0.; 2],
            measurements: Vec::new(),
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn camera(base: i64, cave: bool) -> Transform {
    let y = base as f32;
    if cave {
        Transform::from_xyz(16.5, y + 5.1, 23.5).looking_at(Vec3::new(16.5, y + 5.2, 8.5), Vec3::Y)
    } else {
        Transform::from_xyz(32.5, y + 13.0, 34.5)
            .looking_at(Vec3::new(16.5, y + 3.2, 16.5), Vec3::Y)
    }
}
fn setup(
    mut state: ResMut<Review>,
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut config: ResMut<terra_core::GameConfig>,
    mut gizmos: ResMut<bevy::gizmos::config::GizmoConfigStore>,
) {
    state.base = ((terrain.0.surface_height(16.0, 16.0) / 16.).ceil() as i64 + 3) * 16;
    let transform = camera(state.base, false);
    focus.0 = transform.translation.to_array().map(f64::from);
    for mut current in &mut cameras {
        *current = transform;
    }
    config.client.fov_degrees = 48.0;
    gizmos
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    gizmos.config_mut::<ChunkBorderGizmos>().0.enabled = false;
}
fn source(base: i64) -> [i64; 3] {
    [CENTER, base + 1, CENTER]
}
fn floor(base: i64, cover: bool) -> Plan {
    let y = base + if cover { 2 } else { 0 };
    let mut plan = Plan::new();
    for x in CENTER - 10..=CENTER + 10 {
        for z in CENTER - 10..=CENTER + 10 {
            plan.insert([x, y, z], 1);
        }
    }
    if !cover {
        plan.insert(source(base), FOG);
    }
    plan
}
fn chamber(base: i64) -> Plan {
    let mut plan = Plan::new();
    for x in 7..=25 {
        for z in 7..=27 {
            plan.insert([x, base + 2, z], 3);
            plan.insert([x, base + 9, z], 3);
        }
    }
    for y in base + 3..=base + 8 {
        for z in 7..=27 {
            plan.insert([7, y, z], 3);
            plan.insert([25, y, z], 3);
        }
        for x in 7..=25 {
            plan.insert([x, y, 7], 3);
        }
    }
    // Matched rear-wall panels under one roof: Spirit Stone left, ordinary
    // Stone right, with no point light or emissive object added by the fixture.
    for x in 11..=14 {
        for y in base + 3..=base + 7 {
            plan.insert([x, y, 8], 308);
        }
    }
    for x in 18..=21 {
        for y in base + 3..=base + 7 {
            plan.insert([x, y, 8], 3);
        }
    }
    plan
}
#[derive(SystemParam)]
struct Scene<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    rendered: Res<'w, RenderedSections>,
    render: Res<'w, RenderStatus>,
    fog: Res<'w, FogBlockStatus>,
    ambient: Res<'w, SpiritMistStatus>,
    terrain: Res<'w, WorldTerrain>,
    access: Res<'w, WorldAccess>,
    camera_world: Query<'w, 's, &'static GlobalTransform, With<TerraCamera>>,
    focus: ResMut<'w, ChunkFocus>,
    camera: Query<'w, 's, &'static mut Transform, With<TerraCamera>>,
    lights: Query<'w, 's, (&'static PointLight, &'static GlobalTransform)>,
}
fn queue(plan: &Plan, scene: &Scene, edits: &mut MessageWriter<BlockEditRequest>) -> bool {
    if plan.keys().any(|p| scene.chunks.block_at(*p).is_none()) {
        return false;
    }
    for (&position, &material) in plan {
        let expected_material = scene.chunks.block_at(position).unwrap();
        if expected_material != material {
            edits.write(BlockEditRequest {
                position,
                expected_material,
                material,
            });
        }
    }
    true
}
fn ready(plan: &Plan, scene: &Scene) -> bool {
    if plan
        .iter()
        .any(|(&p, &material)| scene.chunks.block_at(p) != Some(material))
    {
        return false;
    }
    let sections: BTreeSet<_> = plan.keys().map(|p| global_to_section(*p).0).collect();
    sections.into_iter().all(|p| {
        scene
            .rendered
            .sections
            .get(&p)
            .is_some_and(|render| render.mesh_epoch == scene.journal.mesh_epoch(p))
    })
}
fn progress(state: &Review, scene: &Scene) -> serde_json::Value {
    let plan = match state.stage {
        1 => floor(state.base, false),
        2 | 3 => floor(state.base, true),
        5 => [(source(state.base), 0)].into(),
        7 => chamber(state.base),
        _ => Plan::new(),
    };
    let missing: Vec<_> = plan.iter().filter(|(p, raw)|scene.chunks.block_at(**p)!=Some(**raw))
        .map(|(&p,&expected)|serde_json::json!({"cell":p,"expected":expected,"actual":scene.chunks.block_at(p),"journal":scene.journal.block(p)})).collect();
    let sections: BTreeSet<_> = plan.keys().map(|p| global_to_section(*p).0).collect();
    let sections:Vec<_>=sections.into_iter().map(|p| {
        let loaded=scene.chunks.loaded.get(&p);
        let rendered=scene.rendered.sections.get(&p);
        serde_json::json!({"section":[p.x,p.y,p.z],"loaded":loaded.is_some(),
            "block_revision":loaded.map(|s|s.data.revision()),
            "may_contain_source":loaded.is_some_and(|s|s.data.may_contain(FOG)),
            "journal_mesh_epoch":scene.journal.mesh_epoch(p),"rendered_mesh_epoch":rendered.map(|s|s.mesh_epoch)})
    }).collect();
    serde_json::json!({"stage":state.stage,"base":state.base,"plan_blocks":plan.len(),
        "mismatches":missing.len(),"first_mismatches":missing.into_iter().take(8).collect::<Vec<_>>(),
        "sections":sections,"access":format!("{:?}",*scene.access),"focus":scene.focus.0,
        "eye":scene.camera_world.iter().next().map(|eye|eye.translation().to_array()),
        "render_ready":scene.render.ready,"render_pending_jobs":scene.render.pending_jobs,
        "source_material":scene.chunks.block_at(source(state.base)),"source_journal":scene.journal.block(source(state.base)),
        "above_source":(1..=9).map(|up|scene.chunks.block_at([CENTER,state.base+1+up,CENTER])).collect::<Vec<_>>(),
        "fog_sources":scene.fog.sources,"fog_volumes":scene.fog.volumes,"radius":scene.fog.radius,
        "sampled_cells":scene.fog.sampled_cells,"generation":scene.fog.generation,"drift":scene.fog.drift,
        "ambient_volumes":scene.ambient.volumes})
}

fn fog_ready(scene: &Scene, sources: usize) -> bool {
    scene.fog.sources == sources
        && scene.fog.volumes == usize::from(sources > 0)
        && (scene.fog.radius - 8.0).abs() < 0.001
        && scene.ambient.volumes == 0
        && scene.fog.sources <= 16
        && scene.fog.sampled_cells <= 41 * 41
        && ((sources > 0 && scene.fog.sampled_cells > 0)
            || (sources == 0 && scene.fog.sampled_cells == 0))
        && scene.fog.drift.into_iter().all(f32::is_finite)
}
fn measurement(phase: &str, state: &Review, scene: &Scene) -> serde_json::Value {
    serde_json::json!({"phase":phase,"source_cell":source(state.base),
        "source_material":scene.chunks.block_at(source(state.base)),
        "above_source_material":scene.chunks.block_at([CENTER,state.base+2,CENTER]),
        "sources":scene.fog.sources,"volumes":scene.fog.volumes,"radius":scene.fog.radius,
        "sampled_cells":scene.fog.sampled_cells,"generation":scene.fog.generation,
        "drift":scene.fog.drift,"ambient_biome_volumes":scene.ambient.volumes})
}
fn output() -> PathBuf {
    std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-fog-block"))
}
fn capture(commands: &mut Commands, state: &mut Review, name: &str) -> Result<(), String> {
    std::fs::create_dir_all(output()).map_err(|e| e.to_string())?;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(output().join(format!("{name}.png"))))
        .observe(|event: On<ScreenshotCaptured>, mut state: ResMut<Review>| {
            let colors: std::collections::HashSet<_> = event
                .image
                .data
                .as_deref()
                .unwrap_or_default()
                .chunks_exact(4)
                .step_by(127)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            state.failed |=
                event.image.width() != 1920 || event.image.height() != 1080 || colors.len() < 16;
            state.captures += 1;
            state.stage += 1;
            state.pending = false;
            state.since = Instant::now();
        });
    state.pending = true;
    Ok(())
}
fn stone_review(scene: &Scene, base: i64) -> Result<serde_json::Value, String> {
    let path = terra_core::asset_directory().join("terra/blocks/spirit_stone.json");
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if value["id"] != 308 || value.get("light").is_some() {
        return Err("Spirit Stone still defines a block light".into());
    }
    for material in value["materials"]
        .as_object()
        .ok_or("Spirit Stone has no material")?
        .values()
    {
        if material
            .get("emission")
            .and_then(|v| v.as_array())
            .is_some_and(|rgb| rgb.iter().any(|v| v.as_f64().unwrap_or(0.0) != 0.0))
        {
            return Err("Spirit Stone still has emissive material".into());
        }
    }
    let center = Vec3::new(16.0, base as f32 + 5.0, 16.0);
    let active_lights = scene
        .lights
        .iter()
        .filter(|(light, transform)| {
            light.intensity > 0.001 && transform.translation().distance(center) < 18.0
        })
        .count();
    if active_lights != 0 {
        return Err(format!(
            "Unexpected active point light in stone chamber: {active_lights}"
        ));
    }
    Ok(
        serde_json::json!({"spirit_stone":308,"control_stone":3,"emission":[0,0,0],
        "roof":true,"active_point_lights_near_chamber":active_lights}),
    )
}
fn run(
    mut commands: Commands,
    mut state: ResMut<Review>,
    mut scene: Scene,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.stage >= 9 {
        return;
    }
    if state.last_heartbeat.elapsed() > Duration::from_secs(5) {
        let report = progress(&state, &scene);
        info!(report=%report,"Fog block fixture progress");
        let _ = std::fs::create_dir_all(output());
        let _ = std::fs::write(
            output().join("fog-block-progress.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        );
        state.last_heartbeat = Instant::now();
    }
    if state.failed
        || state.started.elapsed() > Duration::from_secs(180)
        || scene.render.error.is_some()
    {
        error!(
            stage = state.stage,
            "Fog block acceptance failed or timed out"
        );
        state.stage = 9;
        exit.write(AppExit::error());
        return;
    }
    if scene.terrain.0.config().biomes {
        error!("Fog proof requires biomes=false");
        state.stage = 9;
        exit.write(AppExit::error());
        return;
    }
    let transform = camera(state.base, state.stage >= 6);
    for mut current in &mut scene.camera {
        *current = transform;
    }
    scene.focus.0 = transform.translation.to_array().map(f64::from);
    if !scene.render.ready || state.pending {
        return;
    }
    let result = (|| -> Result<(), String> {
        match state.stage {
            0 => {
                if queue(&floor(state.base, false), &scene, &mut edits) {
                    state.stage = 1;
                    state.since = Instant::now();
                }
            }
            1 => {
                if state.last_retry.elapsed() > Duration::from_millis(500) {
                    // Only still-mismatched cells are resubmitted, with their
                    // current expected value; committed cells never churn epochs.
                    queue(&floor(state.base, false), &scene, &mut edits);
                    state.last_retry = Instant::now();
                }
                if !ready(&floor(state.base, false), &scene)
                    || !fog_ready(&scene, 1)
                    || state.since.elapsed() < Duration::from_secs(2)
                {
                    return Ok(());
                }
                let sample = measurement("exposed_on_floor", &state, &scene);
                state.measurements.push(sample);
                state.exposed_generation = scene.fog.generation;
                if queue(&floor(state.base, true), &scene, &mut edits) {
                    state.stage = 2;
                    state.since = Instant::now();
                }
            }
            2 => {
                if !ready(&floor(state.base, true), &scene)
                    || !fog_ready(&scene, 1)
                    || scene.fog.generation <= state.exposed_generation
                    || state.since.elapsed() < Duration::from_secs(3)
                {
                    return Ok(());
                }
                if scene.chunks.block_at(source(state.base)) != Some(FOG) {
                    return Err("Fog source disappeared under its floor".into());
                }
                state.first_drift = scene.fog.drift;
                state.buried_generation = scene.fog.generation;
                let sample = measurement("buried_a", &state, &scene);
                state.measurements.push(sample);
                capture(&mut commands, &mut state, "fog-buried-a")?;
            }
            3 => {
                if !fog_ready(&scene, 1) || state.since.elapsed() < Duration::from_secs(3) {
                    return Ok(());
                }
                if scene.fog.drift == state.first_drift {
                    return Err("Fog drift did not animate".into());
                }
                if scene.fog.generation != state.buried_generation {
                    return Err(
                        "Static fog field regenerated while only drift should change".into(),
                    );
                }
                let sample = measurement("buried_b", &state, &scene);
                state.measurements.push(sample);
                capture(&mut commands, &mut state, "fog-buried-b")?;
            }
            4 => {
                let removal = [(source(state.base), 0)].into();
                if queue(&removal, &scene, &mut edits) {
                    state.stage = 5;
                    state.since = Instant::now();
                }
            }
            5 => {
                let removal = [(source(state.base), 0)].into();
                if !ready(&removal, &scene)
                    || !fog_ready(&scene, 0)
                    || state.since.elapsed() < Duration::from_secs(2)
                {
                    return Ok(());
                }
                let sample = measurement("removed", &state, &scene);
                state.measurements.push(sample);
                capture(&mut commands, &mut state, "fog-removed")?;
            }
            6 => {
                if queue(&chamber(state.base), &scene, &mut edits) {
                    state.stage = 7;
                    state.since = Instant::now();
                }
            }
            7 => {
                if !ready(&chamber(state.base), &scene)
                    || !fog_ready(&scene, 0)
                    || state.since.elapsed() < Duration::from_secs(3)
                {
                    return Ok(());
                }
                let sample = stone_review(&scene, state.base)?;
                state.measurements.push(sample);
                capture(&mut commands, &mut state, "spirit-stone-roofed-comparison")?;
            }
            8 => {
                if state.captures != 4 {
                    return Err(format!(
                        "Expected four fog/stone captures, got {}",
                        state.captures
                    ));
                }
                let report = serde_json::json!({"status":"passed","biomes":false,"source_id":FOG,"radius":8,
                    "floor_size":[21,21],"captures":state.captures,"measurements":state.measurements,
                    "scope":"Real edit/render path and public fog state. Stone uses a roofed comparison without local point lights; not a luminance benchmark."});
                std::fs::write(
                    output().join("fog-block-report.json"),
                    serde_json::to_string_pretty(&report).unwrap(),
                )
                .map_err(|e| e.to_string())?;
                info!(
                    "Fog block acceptance passed: buried circle, animated drift, removal, non-emissive Spirit Stone"
                );
                state.stage = 9;
                exit.write(AppExit::Success);
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!(%error,stage=state.stage,"Fog block acceptance failed");
        state.stage = 9;
        exit.write(AppExit::error());
    }
}
