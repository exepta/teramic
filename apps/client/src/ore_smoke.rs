//! Opt-in native inspection of naturally generated exposed ore, without injected deposits.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{OreKind, block_properties::BlockPropertiesCatalog};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration},
};

#[derive(Resource)]
struct OreSmoke {
    started: Instant,
    since: Instant,
    stage: usize,
    view: Option<Transform>,
    position: Option<[i64; 3]>,
    requested: bool,
    captured: usize,
    home: Vec3,
    search: usize,
}
#[derive(Component)]
struct InspectionLight;
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-ores") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(OreSmoke {
            started: now,
            since: now,
            stage: 0,
            view: None,
            position: None,
            requested: false,
            captured: 0,
            home: Vec3::ZERO,
            search: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}
fn setup(world: &mut World) {
    let home = std::env::var("TERRA_ORE_FOCUS")
        .ok()
        .and_then(|text| {
            let values: Vec<_> = text
                .split(',')
                .map(str::parse::<f32>)
                .collect::<Result<_, _>>()
                .ok()?;
            (values.len() == 3).then(|| Vec3::new(values[0], values[1], values[2]))
        })
        .unwrap_or(Vec3::new(-114.0, -33.0, 108.0));
    world.resource_mut::<OreSmoke>().home = home;
    let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
    let focus = if terrain.config().geology_revision > 0 {
        (-96..=96)
            .find_map(|z| {
                (-96..=96).find_map(|x| {
                    terrain.salt_chamber_center(
                        f64::from(x) * 192.0 + 96.0,
                        f64::from(z) * 192.0 + 96.0,
                    )
                })
            })
            .map_or(home, |point| {
                Vec3::from_array(point.map(|value| value as f32))
            })
    } else {
        home
    };
    set_camera(
        world,
        Transform::from_translation(focus).looking_at(focus + Vec3::NEG_Z, Vec3::Y),
    );
    world.spawn((
        InspectionLight,
        PointLight {
            intensity: 16_000.0,
            range: 16.0,
            radius: 0.6,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(focus),
    ));
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .fov_degrees = 55.0;
}
fn set_camera(world: &mut World, view: Transform) {
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = view;
    }
    world.resource_mut::<ChunkFocus>().0 = view.translation.to_array().map(f64::from);
    for mut light in world
        .query_filtered::<&mut Transform, With<InspectionLight>>()
        .iter_mut(world)
    {
        light.translation = view.translation;
    }
}
fn ready(world: &World) -> bool {
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && world.resource::<terra_render::RenderStatus>().ready
}
fn view_of(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &BlockPropertiesCatalog,
    ore: OreKind,
    center: Vec3,
) -> Option<([i64; 3], Transform)> {
    let mut candidates = Vec::new();
    for (&owner, section) in &chunks.loaded {
        for (index, &material) in section.data.blocks().iter().enumerate() {
            if material != ore.material() {
                continue;
            }
            let position = owner
                .global_block([index % 16, index / 16 % 16, index / 256])
                .ok()?;
            let target = Vec3::from_array(position.map(|value| value as f32 + 0.5));
            if target.distance_squared(center) < 180.0 * 180.0 {
                candidates.push((position, target.distance_squared(center)));
            }
        }
    }
    candidates.sort_by_cached_key(|&(position, distance)| {
        let tall = ore == OreKind::Salt
            && terra_world::ore_shapes::salt_growth_height(position, 3, |p| chunks.block_at(p))
                >= 2.0;
        let top_open = chunks.block_at([position[0], position[1] + 1, position[2]]) == Some(0);
        (!tall, !top_open, distance as u64, position)
    });
    for (position, _) in candidates {
        for side in [3, 0, 1, 4, 5, 2] {
            let normal: [i32; 3] = std::array::from_fn(|axis| {
                if axis == side / 2 {
                    if side % 2 == 0 { -1 } else { 1 }
                } else {
                    0
                }
            });
            let outside = std::array::from_fn(|axis| position[axis] + i64::from(normal[axis]));
            if chunks.block_at(outside) != Some(0) {
                continue;
            }
            let outward = Vec3::from_array(normal.map(|value| value as f32));
            let growth = if ore == OreKind::Salt {
                terra_world::ore_shapes::salt_growth_height(position, side, |p| chunks.block_at(p))
            } else {
                0.9
            };
            let target = Vec3::from_array(position.map(|value| value as f32 + 0.5))
                + outward * 0.5
                + if side == 3 {
                    Vec3::Y * growth * 0.45
                } else {
                    Vec3::ZERO
                };
            for distance in [3.8, 2.6, 1.8, 1.15] {
                let eye = target
                    + outward * distance
                    + if normal[1] == 0 {
                        Vec3::Y * 0.2
                    } else {
                        Vec3::new(1.3, 0.0, 1.8)
                    };
                if chunks.block_at(eye.floor().to_array().map(|value| value as i64)) != Some(0) {
                    continue;
                }
                let direction = (target - eye).normalize();
                let (_, hit) = terra_world::targeting::pick_world_with_context(
                    chunks,
                    journal,
                    properties,
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                    terra_world::props::PropPose::Rest,
                );
                if matches!(hit, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == position)
                {
                    return Some((
                        position,
                        Transform::from_translation(eye).looking_at(target, Vec3::Y),
                    ));
                }
            }
        }
    }
    None
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<OreSmoke>| {
        if state.stage >= 6 { return; }
        if state.started.elapsed() > Duration::from_secs(240)
            || world.resource::<terra_render::RenderStatus>().error.is_some() {
            error!(stage=state.stage, "Native ore inspection timed out or renderer failed");
            state.stage = 6;
            world.write_message(AppExit::error());
            return;
        }
        if let Some(view) = state.view { set_camera(world, view); }
        if !ready(world) || state.since.elapsed() < Duration::from_secs(2) { return; }
        let ore = OreKind::ALL[state.stage];
        if state.view.is_none() {
            let center = Vec3::from_array(world.resource::<ChunkFocus>().0.map(|value| value as f32));
            let Some((position, view)) = view_of(world.resource::<ChunkStore>(), world.resource::<EditJournal>(),
                world.resource::<BlockPropertiesCatalog>(), ore, center) else {
                // Rare ores can fall outside the streamed columns surrounding the
                // previous camera. Survey another real underground volume.
                if ore != OreKind::Salt && state.since.elapsed() >= Duration::from_secs(4) {
                    state.search += 1;
                    let depth = match ore { OreKind::Diamond => -44., OreKind::Ruby => -55., _ => -10. };
                    let offsets = [[0.,0.],[60.,-30.],[-45.,-47.],[65.,68.],[-72.,57.],[12.,-85.]];
                    let offset = offsets[(state.search - 1) % offsets.len()];
                    let point = state.home + Vec3::new(offset[0], depth, offset[1]);
                    set_camera(world,Transform::from_translation(point).looking_at(point + Vec3::NEG_Z,Vec3::Y));
                    state.since = Instant::now();
                }
                return;
            };
            info!(?ore, ?position, "Native ore view uses a naturally generated exposed deposit");
            state.position = Some(position);
            state.view = Some(view);
            state.since = Instant::now();
            set_camera(world, view);
            return;
        }
        if !state.requested {
            let directory = std::env::var_os("TERRA_CAPTURE_DIR").map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp/terra-ores"));
            if let Err(error) = std::fs::create_dir_all(&directory) {
                error!(%error, "Cannot create native ore capture directory");
                state.stage = 6;
                world.write_message(AppExit::error());
                return;
            }
            world.spawn(Screenshot::primary_window())
                .observe(save_to_disk(directory.join(format!("ore-{}.png", ore.resource().slug()))))
                .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<OreSmoke>| state.captured += 1);
            info!(?ore, position=?state.position, "Native ore screenshot requested with diagnostic inspection light");
            state.requested = true;
        } else if state.captured > state.stage {
            state.stage += 1;
            state.search = 0;
            if state.stage == 1 {
                let home = state.home;
                set_camera(world, Transform::from_translation(home).looking_at(home + Vec3::NEG_Z, Vec3::Y));
            }
            state.view = None;
            state.position = None;
            state.requested = false;
            state.since = Instant::now();
            if state.stage == 6 {
                info!("Native ore inspection passed: six naturally generated exposed minerals, shared picking, inspection light, real GPU screenshots");
                world.write_message(AppExit::Success);
            }
        }
    });
}
