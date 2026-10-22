//! Opt-in native views of real generated fungi and the authoritative growth tick.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{PropGrowthStatus, PropRenderStatus, RenderStatus, WindFibre};
use terra_voxel::MaterialId;
use terra_world::{
    props::{PropId, WorldProp, placement::support},
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
};

#[derive(Resource)]
struct EcologySmoke {
    started: Instant,
    since: Instant,
    stage: u8,
    view: Option<Transform>,
    selected: Option<PropId>,
    dirt: Option<[i64; 3]>,
    requested: bool,
    captures: usize,
    waiting_for: usize,
    seen: [bool; 4],
    sprout_photos: [bool; 2],
    last_variant: Option<u8>,
    fibre_photo: bool,
    growth_early: bool,
    growth_mid: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-ecology") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(EcologySmoke {
            started: now,
            since: now,
            stage: 0,
            view: None,
            selected: None,
            dirt: None,
            requested: false,
            captures: 0,
            waiting_for: 0,
            seen: [false; 4],
            sprout_photos: [false; 2],
            last_variant: None,
            fibre_photo: false,
            growth_early: false,
            growth_mid: false,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}

fn setup(world: &mut World) {
    let terrain = &world.resource::<WorldTerrain>().0;
    let point = Vec3::new(
        -256.0,
        terrain.surface_height(-256.0, 1536.0) as f32 + 3.0,
        1536.0,
    );
    set_camera(
        world,
        Transform::from_translation(point).looking_at(point - Vec3::Z, Vec3::Y),
    );
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .fov_degrees = 55.0;
}

fn set_camera(world: &mut World, view: Transform) {
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = view;
    }
    world.resource_mut::<ChunkFocus>().0 = view.translation.to_array().map(f64::from);
}

fn ready(world: &World) -> bool {
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && world.resource::<RenderStatus>().ready
        && world.resource::<PropRenderStatus>().ready
}

fn capture(world: &mut World, state: &mut EcologySmoke, label: &str) -> Result<bool, String> {
    capture_after(world, state, label, Duration::from_secs(2))
}

fn capture_after(
    world: &mut World,
    state: &mut EcologySmoke,
    label: &str,
    delay: Duration,
) -> Result<bool, String> {
    if !state.requested && ready(world) && state.since.elapsed() >= delay {
        let directory = std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/terra-ecology"));
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(directory.join(format!("ecology-{label}.png"))))
            .observe(
                |_: On<ScreenshotCaptured>, mut state: ResMut<EcologySmoke>| {
                    state.captures += 1;
                },
            );
        state.requested = true;
        state.waiting_for = state.captures + 1;
        info!(label, "Ecology native screenshot requested");
    }
    Ok(state.requested && state.captures >= state.waiting_for)
}

fn next(state: &mut EcologySmoke, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    state.requested = false;
}

fn fungus_view(chunks: &ChunkStore, prop: &WorldProp) -> Option<Transform> {
    use terra_world::props::visual::{PropDetail, PropPalette, template};
    let root = Vec3::from_array(prop.position.map(|value| value as f32));
    let rotation = Quat::from_rotation_y(prop.rotation);
    let outward = rotation * Vec3::Z;
    let eye = root + outward * 1.8 + Vec3::Y * 0.35;
    if chunks.block_at(eye.floor().to_array().map(|v| v as i64)) != Some(0) {
        return None;
    }
    for mesh in template(
        prop.kind,
        prop.variant,
        PropDetail::Near,
        &PropPalette::default(),
    )
    .ok()?
    {
        for triangle in mesh.indices.chunks_exact(3).step_by(7) {
            let point = triangle.iter().fold(Vec3::ZERO, |sum, &i| {
                sum + Vec3::from_array(mesh.positions[i as usize])
            }) / 3.0;
            let target = root + rotation * point * prop.visual_scale();
            let direction = (target - eye).normalize();
            let (_, hit) = terra_world::targeting::pick_world(
                chunks,
                eye.to_array().map(f64::from),
                direction.to_array().map(f64::from),
            );
            if matches!(hit, Some(terra_world::targeting::WorldTarget::Prop(hit)) if hit.prop.id == prop.id)
            {
                return Some(Transform::from_translation(eye).looking_at(target, Vec3::Y));
            }
        }
    }
    None
}

fn growth_site(chunks: &ChunkStore, center: Vec3) -> Option<[i64; 3]> {
    let mut candidates = Vec::new();
    for (&owner, section) in &chunks.loaded {
        for (index, &material) in section.data.blocks().iter().enumerate() {
            if material != MaterialId::Grass as u16 {
                continue;
            }
            let position = owner
                .global_block([index % 16, index / 16 % 16, index / 256])
                .ok()?;
            let above = [position[0], position[1] + 1, position[2]];
            if chunks.block_at(above) != Some(0)
                || section
                    .props
                    .iter()
                    .any(|p| !p.removed && support(p) == position)
                || ![[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .any(|delta| {
                        chunks.block_at(std::array::from_fn(|axis| position[axis] + delta[axis]))
                            == Some(1)
                    })
            {
                continue;
            }
            let point = Vec3::from_array(position.map(|v| v as f32));
            if point.distance_squared(center) <= 400.0 {
                candidates.push(position);
            }
        }
    }
    candidates.sort_by(|a, b| {
        let distance =
            |p: &[i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(center);
        distance(a).total_cmp(&distance(b)).then(a.cmp(b))
    });
    candidates.into_iter().next()
}

fn underside_view(chunks: &ChunkStore, center: Vec3) -> Option<Transform> {
    let mut caps = Vec::new();
    for (&owner, section) in &chunks.loaded {
        for (index, &material) in section.data.blocks().iter().enumerate() {
            if !(21..=26).contains(&material) {
                continue;
            }
            let position = owner
                .global_block([index % 16, index / 16 % 16, index / 256])
                .ok()?;
            if chunks.block_at([position[0], position[1] - 1, position[2]]) == Some(0) {
                caps.push(position);
            }
        }
    }
    caps.sort_by(|a, b| {
        let distance =
            |p: &[i64; 3]| Vec3::from_array(p.map(|v| v as f32)).distance_squared(center);
        distance(a).total_cmp(&distance(b)).then(a.cmp(b))
    });
    for position in caps {
        let target = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, -0.8, 0.5);
        for direction in 0..8 {
            let angle = direction as f32 * std::f32::consts::FRAC_PI_4;
            let eye = target + Vec3::new(angle.sin() * 4.0, -1.6, angle.cos() * 4.0);
            if chunks.block_at(eye.floor().to_array().map(|v| v as i64)) == Some(0) {
                return Some(Transform::from_translation(eye).looking_at(target, Vec3::Y));
            }
        }
    }
    None
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<EcologySmoke>| {
        if state.stage == 9 { return; }
        if state.started.elapsed() > Duration::from_secs(240)
            || world.resource::<RenderStatus>().error.is_some()
            || world.resource::<PropRenderStatus>().error.is_some()
        {
            error!(stage=state.stage, seen=?state.seen, "Ecology native smoke timed out or renderer failed");
            state.stage = 9;
            world.write_message(AppExit::error());
            return;
        }
        if let Some(view) = state.view { set_camera(world, view); }
        let result = step(world, &mut state);
        if let Err(error) = result {
            error!(%error, stage=state.stage, "Ecology native smoke failed");
            state.stage = 9;
            world.write_message(AppExit::error());
        }
    });
}

fn step(world: &mut World, state: &mut EcologySmoke) -> Result<(), String> {
    let stage = state.stage;
    match stage {
        0 if ready(world) => {
            let chunks = world.resource::<ChunkStore>();
            let mut props: Vec<_> = chunks
                .loaded
                .values()
                .flat_map(|section| &section.props)
                .filter(|prop| prop.wall_attached() && !prop.removed)
                .copied()
                .collect();
            props.sort_by_key(|prop| prop.id);
            let selected = props
                .into_iter()
                .find_map(|prop| fungus_view(chunks, &prop).map(|view| (prop, view)));
            let Some((prop, view)) = selected else {
                return Err("No visible generated tree fungus in forest fixture".into());
            };
            state.dirt = growth_site(chunks, Vec3::from_array(prop.position.map(|v| v as f32)));
            if state.dirt.is_none() {
                return Err("No exposed grass site beside generated forest fungus".into());
            }
            state.selected = Some(prop.id);
            state.view = Some(view);
            set_camera(world, view);
            info!(id=?prop.id, anchor=?support(&prop), variant=prop.variant, "Native ecology selected real generated trunk fungus");
            next(state, 1);
        }
        1 => {
            if state.since.elapsed() >= Duration::from_secs(2)
                && ready(world)
                && !matches!(world.resource::<terra_player::TargetWorld>().0,
                    Some(terra_player::WorldTarget::Prop(hit)) if Some(hit.prop.id) == state.selected)
            {
                return Err("Generated trunk fungus no longer matches exact native picking".into());
            }
            if capture(world, state, "trunk-fungus")? {
                let terrain = &world.resource::<WorldTerrain>().0;
                let point = Vec3::new(
                    -640.0,
                    terrain.surface_height(-640.0, 640.0) as f32 + 5.0,
                    640.0,
                );
                let view = Transform::from_translation(point).looking_at(point + Vec3::Y, Vec3::Z);
                state.view = Some(view);
                set_camera(world, view);
                next(state, 2);
            }
        }
        2 if ready(world) => {
            let eye = state.view.ok_or("Missing ecology view")?.translation;
            let view = underside_view(world.resource::<ChunkStore>(), eye)
                .ok_or("No generated mushroom underside found")?;
            state.view = Some(view);
            set_camera(world, view);
            next(state, 3);
        }
        3 if !state.fibre_photo && capture(world, state, "cap-fibres")? => {
            if world
                .query_filtered::<Entity, With<WindFibre>>()
                .iter(world)
                .next()
                .is_none()
            {
                return Err(
                    "Generated cap fibres did not use their rooted shared wind material".into(),
                );
            }
            state.fibre_photo = true;
            state.requested = false;
            state.since = Instant::now();
        }
        3 if state.fibre_photo && capture(world, state, "cap-fibres-wind")? => {
            let position = state.dirt.ok_or("Missing growth site")?;
            let point = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 1.0, 0.5);
            let view = Transform::from_translation(point + Vec3::new(1.2, 1.0, 1.5))
                .looking_at(point, Vec3::Y);
            state.view = Some(view);
            set_camera(world, view);
            next(state, 4);
        }
        4 if ready(world) => {
            let position = state.dirt.ok_or("Missing growth site")?;
            if world.resource::<ChunkStore>().block_at(position) != Some(1) {
                return Err("Generated growth site changed before test edit".into());
            }
            world.write_message(BlockEditRequest {
                position,
                expected_material: 1,
                material: 2,
            });
            next(state, 5);
        }
        5 => {
            let position = state.dirt.ok_or("Missing growth site")?;
            if world.resource::<ChunkStore>().block_at(position) == Some(2)
                && capture(world, state, "bare-dirt")?
            {
                next(state, 6);
            }
        }
        6 => {
            let position = state.dirt.ok_or("Missing growth site")?;
            let owner = terra_voxel::global_to_section(position).0;
            let sprout = world
                .resource::<ChunkStore>()
                .loaded
                .get(&owner)
                .and_then(|section| {
                    section.props.iter().find(|prop| {
                        !prop.removed && prop.growing_grass() && support(prop) == position
                    })
                })
                .copied();
            if let Some(prop) = sprout {
                if state.last_variant != Some(prop.variant) {
                    state.last_variant = Some(prop.variant);
                    state.since = Instant::now();
                    state.requested = false;
                }
                state.seen[usize::from(prop.variant - 64)] = true;
                if prop.variant == 64 {
                    let clock = world.resource::<PropGrowthStatus>();
                    let progress = clock.progress(prop.id);
                    if !state.growth_early && progress.is_some_and(|progress| progress < 0.5) {
                        if capture_after(world, state, "growth-early", Duration::from_millis(150))?
                        {
                            state.growth_early = true;
                            state.requested = false;
                        }
                        return Ok(());
                    }
                    if state.growth_early
                        && !state.growth_mid
                        && progress.is_some_and(|progress| (0.5..0.9).contains(&progress))
                    {
                        if capture_after(world, state, "growth-mid", Duration::ZERO)? {
                            state.growth_mid = true;
                            state.requested = false;
                        }
                        return Ok(());
                    }
                }
                let index = usize::from(prop.variant >= 66);
                if matches!(prop.variant, 64 | 66) && !state.sprout_photos[index] {
                    let label = if prop.variant == 64 {
                        "sprout-small"
                    } else {
                        "sprout-grown"
                    };
                    if capture(world, state, label)? {
                        state.sprout_photos[index] = true;
                        state.requested = false;
                    }
                }
            }
            if world.resource::<ChunkStore>().block_at(position) == Some(1) {
                if !state.seen.iter().all(|seen| *seen) || !state.growth_early || !state.growth_mid
                {
                    return Err(format!(
                        "Grass completed without observing every saved visible stage: {:?}",
                        state.seen
                    ));
                }
                info!(position=?position, elapsed=?state.started.elapsed(), "Authoritative ecology grew Dirt through four saved stages into Grass");
                next(state, 7);
            }
        }
        7 if capture(world, state, "grass-complete")? => {
            info!(captures=state.captures, seen=?state.seen, gpu_transitions=world.resource::<PropGrowthStatus>().transitions,
                "Ecology native smoke passed: generated fungi, rooted wind fibres, smooth GPU growth and real-time authoritative grass succession");
            state.stage = 9;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}
