//! Opt-in native reef acceptance in a real generated ocean, not a model-only stage.
use bevy::{
    camera::primitives::Aabb,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::TerraCamera;
use terra_world::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
};
mod habitats;
mod ink;

const KINDS: [PropKind; 7] = [
    PropKind::Seagrass,
    PropKind::SeaCucumber,
    PropKind::BranchingCoral,
    PropKind::TubeCoral,
    PropKind::PlateCoral,
    PropKind::SeaAnemone,
    PropKind::SmallStones,
];
const LABELS: [&str; 14] = [
    "seagrass",
    "sea-cucumbers",
    "branching-coral",
    "tube-coral",
    "plate-coral",
    "sea-anemone",
    "small-stones",
    "reef-blocks",
    "reef-wide",
    "surface-clear-water",
    "underwater-depth",
    "squid-ink-hit",
    "squid-ink-cleared",
    "squid-ink-low-quality",
];

#[derive(Resource)]
struct ReefSmoke {
    started: Instant,
    since: Instant,
    center: Option<Vec3>,
    level: f32,
    props: Vec<WorldProp>,
    blocks: Vec<([i64; 3], u16)>,
    stage: usize,
    requested: bool,
    captured: usize,
    view: Option<Transform>,
    failure: Option<String>,
    folder: PathBuf,
    ink: ink::InkProbe,
}

pub(crate) fn install(app: &mut App) {
    habitats::install(app);
    if !std::env::args().any(|arg| arg == "--smoke-test-reef") {
        return;
    }
    let now = Instant::now();
    app.init_resource::<terra_network::client::ReplicatedCreatures>()
        .insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(ReefSmoke {
            started: now,
            since: now,
            center: None,
            level: 0.0,
            props: Vec::new(),
            blocks: Vec::new(),
            stage: 0,
            requested: false,
            captured: 0,
            view: None,
            failure: None,
            folder: std::env::var_os("TERRA_CAPTURE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target/reef-captures")),
            ink: default(),
        })
        .add_systems(PostStartup, locate)
        .add_systems(Last, run);
}

fn locate(world: &mut World) {
    let found = find_ocean(&world.resource::<WorldTerrain>().0);
    let mut state = world.resource_mut::<ReefSmoke>();
    match found {
        Some((center, level)) => {
            state.center = Some(center);
            state.level = level;
        }
        None => {
            state.failure = Some("No suitable generated ocean floor found within 2048m".into());
            return;
        }
    }
    let folder = state.folder.clone();
    if let Err(error) = std::fs::create_dir_all(folder) {
        state.failure = Some(format!("Cannot create reef capture directory: {error}"));
        return;
    }
    if let Some(center) = state.center {
        info!(
            ?center,
            level = state.level,
            "Reef native fixture located an actual generated ocean floor"
        );
        set_camera(
            world,
            Transform::from_translation(center + Vec3::new(3.0, 2.0, 6.0))
                .looking_at(center, Vec3::Y),
            center,
        );
    }
}

fn find_ocean(terrain: &terra_world::TerraGenerator) -> Option<(Vec3, f32)> {
    for ring in 0_i32..=32 {
        for z in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(z.abs()) != ring {
                    continue;
                }
                let (x, z) = (f64::from(x) * 64.0 + 8.5, f64::from(z) * 64.0 + 8.5);
                let column = terrain.surface_column(x, z);
                let Some(level) = column.water_level else {
                    continue;
                };
                let bed = column.water_bed();
                if column.biome.weights.dominant()
                    != terra_world::generation::biomes::BiomeKind::Ocean
                    || !(5.0..=12.0).contains(&(level - bed))
                {
                    continue;
                }
                let clear_floor = [-11.0, 0.0, 11.0].into_iter().all(|dx| {
                    [-6.0, 0.0, 6.0].into_iter().all(|dz| {
                        let sample = terrain.surface_column(x + dx, z + dz);
                        sample
                            .water_level
                            .is_some_and(|water| water - sample.water_bed() >= 3.5)
                            && (sample.water_bed() - bed).abs() < 1.5
                    })
                });
                if clear_floor {
                    return Some((Vec3::new(x as f32, bed as f32, z as f32), level as f32));
                }
            }
        }
    }
    None
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<ReefSmoke>| {
        let result = advance(world, &mut state);
        if let Err(error) = result {
            error!(%error, stage = state.stage, "Native reef acceptance failed");
            world.write_message(AppExit::error());
        } else if state.stage == LABELS.len() && state.ink.finished {
            info!("Native reef acceptance passed: real generated ocean, seven section-batched aquatic props, four coral cube blocks, mud and ice, surface transparency, depth-aware underwater views, body-hit squid ink in High/Low water quality, natural ink decay and surfacing reset; fixture placement is explicit, not a claim of naturally co-located species");
            world.write_message(AppExit::Success);
        }
    });
}

fn ready(world: &World) -> bool {
    let terrain = world.resource::<terra_render::RenderStatus>();
    let props = world.resource::<terra_render::PropRenderStatus>();
    let water = world.resource::<terra_render::WaterStatus>();
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && terrain.ready
        && terrain.pending_jobs == 0
        && props.ready
        && props.pending_jobs == 0
        && water.initial_ready
        && water.ready
        && water.visible_tiles > 0
}

fn advance(world: &mut World, state: &mut ReefSmoke) -> Result<(), String> {
    if let Some(error) = state.failure.take() {
        return Err(error);
    }
    if state.started.elapsed() > Duration::from_secs(150) {
        return Err("Timed out waiting for real reef geometry and screenshots".into());
    }
    if let Some(error) = world
        .resource::<terra_render::RenderStatus>()
        .error
        .as_ref()
        .or_else(|| {
            world
                .resource::<terra_render::PropRenderStatus>()
                .error
                .as_ref()
        })
    {
        return Err(error.clone());
    }
    let center = state.center.ok_or("Reef search did not produce an ocean")?;
    let view = state.view.unwrap_or_else(|| {
        Transform::from_translation(center + Vec3::new(3.0, 2.0, 6.0)).looking_at(center, Vec3::Y)
    });
    set_camera(world, view, center);
    if state.requested {
        if state.captured <= state.stage {
            return Ok(());
        }
        state.stage += 1;
        state.requested = false;
        state.view = None;
        state.since = Instant::now();
        if state.stage == LABELS.len() {
            ink::begin_finish(state);
        }
    }
    if state.stage == LABELS.len() {
        ink::finish(world, state, center)?;
        return Ok(());
    }
    if !ready(world) {
        return Ok(());
    }
    if state.props.is_empty() {
        assemble(world, state, center)?;
        state.since = Instant::now();
        return Ok(());
    }
    if state.view.is_none() {
        let view = stage_view(state, center);
        state.view = Some(view);
        state.since = Instant::now();
        set_camera(world, view, center);
        if state.stage >= 11 {
            ink::prepare(world, state, view)?;
        }
        return Ok(());
    }
    if state.stage >= 11 {
        if !ink::ready(world, state)? {
            return Ok(());
        }
    } else if state.since.elapsed() < Duration::from_millis(1800) {
        return Ok(());
    }
    let chunks = world.resource::<ChunkStore>();
    if state
        .blocks
        .iter()
        .any(|(p, id)| chunks.block_at(*p) != Some(*id))
    {
        return Err("Reef block placement did not reach the production edit/mesh path".into());
    }
    if state.props.iter().any(|prop| {
        chunks
            .loaded
            .get(&prop.section)
            .is_none_or(|section| !section.props.iter().any(|p| p.id == prop.id && !p.removed))
    }) {
        return Err("A fixture prop disappeared from its logical section".into());
    }
    let water = world.resource::<terra_render::UnderwaterState>();
    let (submerged, underwater_blend, underwater_depth) =
        (water.submerged, water.blend, water.depth);
    let above = state.stage == 9;
    if above && underwater_blend > 0.05 {
        return Err(format!(
            "Above-water reef view retained underwater grading: {water:?}"
        ));
    }
    if !above && (!submerged || underwater_blend < 0.9) {
        return Err(format!(
            "Reef closeup did not enter the real underwater pipeline: {water:?}"
        ));
    }
    let batches = world
        .query_filtered::<(&Mesh3d, &Aabb), With<terra_render::WindProp>>()
        .iter(world)
        .filter(|(mesh, bounds)| {
            world.resource::<Assets<Mesh>>().contains(mesh.id())
                && bounds.half_extents.is_finite()
                && bounds.half_extents.max_element() > 0.01
        })
        .count();
    if batches == 0 {
        return Err("No nonempty shared prop mesh with valid render bounds".into());
    }
    let props = world.resource::<terra_render::PropRenderStatus>();
    info!(
        stage = state.stage,
        label = LABELS[state.stage],
        prop_batches = batches,
        represented_props = props.visible_props,
        pending_jobs = props.pending_jobs,
        underwater_blend,
        underwater_depth,
        "Native reef geometry ready for screenshot pixel validation"
    );
    state.requested = true;
    let path = state.folder.join(format!(
        "reef-{:02}-{}.png",
        state.stage, LABELS[state.stage]
    ));
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<ReefSmoke>| {
                let valid = event.image.data.as_ref().is_some_and(|bytes| {
                    let mut colors = std::collections::HashSet::new();
                    let mut bright = 0;
                    let mut samples = 0;
                    for pixel in bytes.chunks_exact(4).step_by(31) {
                        samples += 1;
                        if pixel[..3].iter().any(|value| *value > 20) {
                            bright += 1;
                        }
                        colors.insert([pixel[0] / 8, pixel[1] / 8, pixel[2] / 8]);
                    }
                    colors.len() >= 16 && bright * 100 > samples
                });
                if valid {
                    state.captured += 1;
                } else {
                    state.failure = Some(
                        "Reef screenshot was blank or lacked meaningful colour variation".into(),
                    );
                }
            },
        );
    Ok(())
}

fn sea_floor(world: &World, x: i64, z: i64) -> Option<[i64; 3]> {
    let chunks = world.resource::<ChunkStore>();
    let column = chunks.prepared_column([x.div_euclid(16), z.div_euclid(16)])?;
    let surface = column.surface_at(x, z)?;
    let bed = surface.water_bed() as i64;
    (bed - 2..=bed + 2).find_map(|y| {
        let floor = [x, y - 1, z];
        (chunks.block_at([x, y, z]) == Some(0)
            && chunks.block_at(floor).is_some_and(|id| {
                terra_voxel::MaterialId::from_raw(id).is_some_and(|m| m.is_solid())
            })
            && surface
                .water_level
                .is_some_and(|level| level - y as f64 >= 2.5))
        .then_some(floor)
    })
}

fn assemble(world: &mut World, state: &mut ReefSmoke, center: Vec3) -> Result<(), String> {
    let (x, z) = (center.x.floor() as i64, center.z.floor() as i64);
    let natural = world
        .resource::<ChunkStore>()
        .loaded
        .values()
        .flat_map(|section| &section.props)
        .filter(|prop| prop.kind.aquatic() && !prop.removed)
        .count();
    for (index, kind) in KINDS.into_iter().enumerate() {
        let floor = sea_floor(world, x + index as i64 * 3 - 9, z)
            .ok_or("No loaded submerged support for aquatic prop")?;
        let prop = WorldProp {
            id: PropId(0xffff_7800_0000_0000 | index as u64),
            kind,
            position: [
                floor[0] as f64 + 0.5,
                floor[1] as f64 + 1.0,
                floor[2] as f64 + 0.5,
            ],
            section: terra_voxel::global_to_section(floor).0,
            rotation: 0.25,
            scale: 1.0,
            variant: 0,
            removed: false,
        };
        world.resource_scope(|world, mut chunks: Mut<ChunkStore>| -> Result<(), String> {
            let mut journal = world.resource_mut::<EditJournal>();
            if !journal.place_prop(prop).map_err(|e| e.to_string())? {
                return Err("Fixture prop placement was rejected".into());
            }
            let section = chunks
                .loaded
                .get_mut(&prop.section)
                .ok_or("Prop support section unloaded")?;
            journal.apply_prop_removals(prop.section, &mut section.props);
            Ok(())
        })?;
        state.props.push(prop);
    }
    for (index, material) in (44..=49).enumerate() {
        let floor = sea_floor(world, x + index as i64 * 2 - 4, z - 4)
            .ok_or("No loaded coral block support")?;
        let position = [floor[0], floor[1] + 1, floor[2]];
        world.write_message(BlockEditRequest {
            position,
            expected_material: 0,
            material,
        });
        state.blocks.push((position, material));
    }
    info!(
        natural_aquatic_props = natural,
        fixture_props = state.props.len(),
        cube_blocks = state.blocks.len(),
        "Reef fixture uses real production prop journals and local block-edit requests"
    );
    Ok(())
}

fn stage_view(state: &ReefSmoke, center: Vec3) -> Transform {
    let (eye, target) = if state.stage < KINDS.len() {
        let prop = state.props[state.stage];
        let root = Vec3::from_array(prop.position.map(|v| v as f32));
        if prop.kind == PropKind::Seagrass {
            (root + Vec3::new(2.4, 1.9, 4.5), root + Vec3::Y * 1.6)
        } else {
            (root + Vec3::new(0.95, 0.85, 1.55), root + Vec3::Y * 0.38)
        }
    } else {
        match state.stage {
            7 => {
                let target = center + Vec3::new(0.0, 0.7, -4.0);
                (target + Vec3::new(4.8, 2.3, 7.0), target)
            }
            8 => (center + Vec3::new(8.0, 3.0, 12.0), center + Vec3::Y * 0.5),
            9 => (
                Vec3::new(center.x + 3.0, state.level + 2.5, center.z + 5.0),
                center,
            ),
            _ => (
                Vec3::new(center.x, state.level - 2.0, center.z + 5.0),
                center + Vec3::Y,
            ),
        }
    };
    Transform::from_translation(eye).looking_at(target, Vec3::Y)
}

pub(crate) fn set_camera(world: &mut World, view: Transform, center: Vec3) {
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = view;
    }
    for mut body in world
        .query_filtered::<&mut Transform, With<terra_player::PlayerBody>>()
        .iter_mut(world)
    {
        body.translation = view.translation - Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
    }
    world.resource_mut::<ChunkFocus>().0 = center.to_array().map(f64::from);
}
