use bevy::{app::AppExit, camera::primitives::Aabb, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TargetWorld, TerraCamera, WorldTarget};
use terra_render::{PropRenderStatus, RenderStatus, WindProp};
use terra_world::{
    props::{PropId, PropKind, WorldProp},
    streaming::{ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain},
};

#[derive(Resource)]
struct BotanicalSmoke {
    started: Instant,
    stage: usize,
    framed: Option<Instant>,
    captured: bool,
    wind_captured: bool,
    center: Vec3,
    view: Transform,
    selected: Option<PropId>,
}

const STAGES: [&str; 6] = [
    "grass",
    "flowers",
    "terrain",
    "flowers-detail",
    "bush",
    "mushrooms",
];

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-botanical") {
        return;
    }
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(BotanicalSmoke {
            started: Instant::now(),
            stage: 0,
            framed: None,
            captured: false,
            wind_captured: false,
            center: Vec3::ZERO,
            view: Transform::default(),
            selected: None,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}

fn setup(
    terrain: Res<WorldTerrain>,
    mut focus: ResMut<ChunkFocus>,
    mut state: ResMut<BotanicalSmoke>,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut debug: ResMut<terra_core::DebugSettings>,
) {
    // This fixture explicitly verifies the HUD; gameplay still defaults to hidden.
    debug.enabled = true;
    state.center = Vec3::new(
        -256.0,
        terrain.0.surface_height(-256.0, 1536.0) as f32 + 2.0,
        1536.0,
    );
    focus.0 = state.center.to_array().map(f64::from);
    for mut camera in &mut cameras {
        camera.translation = state.center;
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct BotanicalWorld<'w, 's> {
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    terrain: Res<'w, RenderStatus>,
    props: Res<'w, PropRenderStatus>,
    process: Res<'w, terra_ui::AppProcessStats>,
    overlay: Res<'w, terra_ui::DebugOverlay>,
    target: Res<'w, TargetWorld>,
    wind: Query<'w, 's, &'static Aabb, With<WindProp>>,
}

fn run(
    mut commands: Commands,
    mut state: ResMut<BotanicalSmoke>,
    world: BotanicalWorld,
    mut cameras: Query<&mut Transform, With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.stage >= STAGES.len() {
        return;
    }
    if state.started.elapsed() > Duration::from_secs(90)
        || world.terrain.error.is_some()
        || world.props.error.is_some()
    {
        error!(stage = state.stage, "Botanical smoke failed or timed out");
        state.stage = STAGES.len();
        exit.write(AppExit::error());
        return;
    }
    if let Some(framed) = state.framed {
        for mut camera in &mut cameras {
            *camera = state.view;
        }
        let wind_pair = matches!(state.stage, 0 | 4);
        if !state.captured
            && framed.elapsed() > Duration::from_secs(2)
            && world.terrain.ready
            && world.props.ready
        {
            let expected = match state.stage {
                0 => Some("Grasbueschel"),
                1 | 3 => Some("Wildblumen"),
                4 => Some("Busch"),
                _ => None,
            };
            if expected.is_some_and(|name| world.overlay.block_name != name)
                || (expected.is_some()
                    && !matches!(world.target.0, Some(WorldTarget::Prop(hit)) if Some(hit.prop.id) == state.selected))
                || world.wind.is_empty()
                || world.overlay.xyz.is_empty()
                || world.overlay.chunk.is_empty()
                || (cfg!(target_os = "linux")
                    && (world.process.cpu_percent.is_none()
                        || world.process.resident_bytes.is_none()))
            {
                error!(stage=state.stage, looked_at=%world.overlay.block_name, expected_prop=?state.selected, actual_target=?world.target.0, xyz=%world.overlay.xyz, chunk=%world.overlay.chunk, wind_batches=world.wind.iter().count(), cpu=?world.process.cpu_percent, resident=?world.process.resident_bytes, "Botanical HUD did not reflect the scene or process metrics");
                state.stage = STAGES.len();
                exit.write(AppExit::error());
                return;
            }
            let label = STAGES[state.stage];
            capture(&mut commands, label);
            state.captured = true;
            info!(label, selected=?state.selected, wind_batches=world.wind.iter().count(), cpu_percent=?world.process.cpu_percent, resident_bytes=?world.process.resident_bytes,"Botanical close-up captured");
        }
        if wind_pair
            && state.captured
            && !state.wind_captured
            && framed.elapsed() > Duration::from_secs(3)
            && world.terrain.ready
            && world.props.ready
        {
            let label = format!("{}-wind", STAGES[state.stage]);
            capture(&mut commands, &label);
            state.wind_captured = true;
            info!(label, selected=?state.selected, "Botanical fixed-camera wind pair captured");
        }
        if state.captured
            && (!wind_pair || state.wind_captured)
            && framed.elapsed() > Duration::from_secs(if wind_pair { 4 } else { 3 })
        {
            state.stage += 1;
            state.framed = None;
            state.captured = false;
            state.wind_captured = false;
            if state.stage == STAGES.len() {
                info!("Botanical smoke completed");
                exit.write(AppExit::Success);
            }
        }
        return;
    }
    if world.generation.status != GenerationStatus::Ready
        || !world.terrain.ready
        || !world.props.ready
    {
        return;
    }
    let kind = if matches!(state.stage, 1 | 3) {
        PropKind::Flower
    } else if state.stage == 5 {
        PropKind::Mushroom
    } else if state.stage == 4 {
        PropKind::Bush
    } else {
        PropKind::Grass
    };
    let mut candidates: Vec<_> = world
        .chunks
        .loaded
        .values()
        .flat_map(|section| section.props.iter())
        .filter(|prop| {
            prop.kind == kind && !prop.removed && (state.stage != 5 || prop.variant < 128)
        })
        .copied()
        .collect();
    candidates.sort_by(|a, b| {
        let distance = |p: &terra_world::props::WorldProp| {
            Vec3::from_array(p.position.map(|v| v as f32)).distance_squared(state.center)
        };
        distance(a).total_cmp(&distance(b)).then(a.id.cmp(&b.id))
    });
    let selected = candidates.into_iter().find_map(|prop| {
        if matches!(state.stage, 0 | 4) && !sunlit(&world.chunks, &prop) {
            return None;
        }
        if state.stage == 4 {
            bush_view(&world.chunks, &prop).map(|view| (prop, view))
        } else {
            let view = familiar_view(&prop, state.stage);
            if state.stage != 2 {
                let (_, selected) = terra_world::targeting::pick_world(
                    &world.chunks,
                    view.translation.to_array().map(f64::from),
                    view.forward().to_array().map(f64::from),
                );
                if !matches!(selected, Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id) {
                    return None;
                }
            }
            Some((prop, view))
        }
    });
    let Some((prop, view)) = selected else {
        return;
    };
    state.view = view;
    state.selected = Some(prop.id);
    for mut camera in &mut cameras {
        *camera = view;
    }
    focus.0 = view.translation.to_array().map(f64::from);
    state.framed = Some(Instant::now());
}

fn sunlit(chunks: &ChunkStore, prop: &WorldProp) -> bool {
    let root = Vec3::from_array(prop.position.map(|v| v as f32)) + Vec3::Y * 0.35;
    let sun = Vec3::new(40.0, 80.0, 30.0).normalize();
    (1..=96).all(|step| {
        let point = root + sun * step as f32 * 0.4;
        chunks
            .block_at(point.floor().to_array().map(|v| v as i64))
            .is_none_or(|block| block == 0)
    })
}

fn familiar_view(prop: &WorldProp, stage: usize) -> Transform {
    let root = Vec3::from_array(prop.position.map(|v| v as f32));
    let (offset, target) = if stage == 2 {
        (Vec3::new(2.2, 1.8, 3.4), root - Vec3::Y * 0.35)
    } else if matches!(stage, 3 | 5) {
        let scale = prop.visual_scale();
        let head = prop_head(prop).unwrap_or(Vec3::Y * scale * 0.85);
        (Vec3::new(0.85, 1.4, 1.35) * scale, root + head)
    } else {
        (Vec3::new(0.9, 0.85, 1.35), root + Vec3::Y * 0.18)
    };
    Transform::from_translation(root + offset).looking_at(target, Vec3::Y)
}

fn prop_head(prop: &WorldProp) -> Option<Vec3> {
    use terra_world::props::visual::{PropDetail, PropPalette, template};
    let meshes = template(
        prop.kind,
        prop.variant,
        PropDetail::Near,
        &PropPalette::default(),
    )
    .ok()?;
    let mut highest: Option<Vec3> = None;
    for mesh in meshes {
        for face in mesh.indices.chunks_exact(3) {
            let center = face.iter().fold(Vec3::ZERO, |sum, index| {
                sum + Vec3::from_array(mesh.positions[*index as usize])
            }) / 3.0;
            if highest.is_none_or(|old| center.y > old.y) {
                highest = Some(center);
            }
        }
    }
    highest.map(|point| Quat::from_rotation_y(prop.rotation) * point * prop.visual_scale())
}

fn bush_view(chunks: &ChunkStore, prop: &WorldProp) -> Option<Transform> {
    use terra_world::props::PropPose;
    let root = Vec3::from_array(prop.position.map(|v| v as f32));
    let target = root + prop_head(prop)?;
    for direction in 0..16 {
        let angle = direction as f32 * std::f32::consts::TAU / 16.0 + 0.6;
        let eye =
            root + Vec3::new(angle.sin() * 0.85, 1.15, angle.cos() * 0.85) * prop.visual_scale();
        let clear = [-0.10, 0.10].into_iter().all(|x| {
            [-0.10, 0.10].into_iter().all(|z| {
                let point = eye + Vec3::new(x, 0.0, z);
                chunks.block_at(point.floor().to_array().map(|v| v as i64)) == Some(0)
            })
        });
        if !clear {
            continue;
        }
        let ray = (target - eye).normalize();
        let visible_in_wind = [
            PropPose::Rest,
            PropPose::Swept,
            PropPose::Wind(0.0),
            PropPose::Wind(4.0),
            PropPose::Wind(12.0),
            PropPose::Wind(20.0),
            PropPose::Wind(28.0),
        ]
        .into_iter()
        .all(|pose| {
            let (_, selected) = terra_world::targeting::pick_world_with_pose(
                chunks,
                eye.to_array().map(f64::from),
                ray.to_array().map(f64::from),
                pose,
            );
            matches!(selected, Some(WorldTarget::Prop(hit)) if hit.prop.id == prop.id)
        });
        if visible_in_wind {
            return Some(Transform::from_translation(eye).looking_at(target, Vec3::Y));
        }
    }
    None
}

fn capture(commands: &mut Commands, label: &str) {
    let base = std::env::var_os("TERRA_SCREENSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-botanical.png"));
    let stem = base
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("botanical");
    let path = base.with_file_name(format!("{stem}-{label}.png"));
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}
