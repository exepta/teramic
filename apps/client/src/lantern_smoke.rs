//! Native authored lantern gallery using committed world blocks and real lights.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::time::{Duration, Instant};
use terra_core::{
    WorldClock,
    lantern::{CHAIN, LANTERN},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, ChunkFocus, ChunkStore, commit_block_edit},
};

const VIEWS: [(&str, f64, [f32; 3], [f32; 3]); 5] = [
    ("lantern-gallery-day", 12., [19., 24., 30.], [6., 19., 8.]),
    ("lantern-gallery-night", 0., [19., 24., 30.], [6., 19., 8.]),
    (
        "lantern-wall-close",
        0.,
        [-0.7, 20., 12.],
        [-3.5, 18.5, 8.5],
    ),
    ("lantern-table-close", 0., [7., 19.1, 13.], [4.5, 17.5, 9.5]),
    ("lantern-chain-close", 0., [12., 21., 13.], [9.5, 19.8, 9.5]),
];
#[derive(Resource)]
struct Gallery {
    started: Instant,
    since: Instant,
    stage: usize,
    captures: usize,
    placed: bool,
    requested: bool,
    failed: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-lanterns") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Gallery {
            started: now,
            since: now,
            stage: 0,
            captures: 0,
            placed: false,
            requested: false,
            failed: false,
        })
        .add_systems(PostStartup, |world: &mut World| view(world, 0))
        .add_systems(Last, run);
}
fn view(world: &mut World, stage: usize) {
    let (_, hour, eye, target) = VIEWS[stage];
    let mut clock = world.resource_mut::<WorldClock>();
    clock.phase = hour / 24.;
    clock.paused = true;
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = Transform::from_translation(Vec3::from_array(eye))
            .looking_at(Vec3::from_array(target), Vec3::Y);
    }
    world.resource_mut::<ChunkFocus>().0 = eye.map(f64::from);
}
fn build(world: &mut World) -> Result<(), String> {
    let properties = world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .clone();
    world.resource_scope(|world, mut chunks: Mut<ChunkStore>| {
        let mut journal = world.resource_mut::<EditJournal>();
        let mut put = |position, material, state: Option<u8>| -> Result<(), String> {
            let expected = chunks
                .block_at(position)
                .ok_or("Gallery column is not loaded")?;
            if expected != material
                && !commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material: expected,
                        material,
                    },
                    &properties,
                )?
            {
                return Err("Gallery edit conflict".into());
            }
            if let Some(facing) = state {
                if !terra_world::lanterns::supported(
                    &chunks,
                    &properties,
                    position,
                    material,
                    facing,
                ) {
                    return Err(format!("Unsupported gallery mount {position:?}/{facing}"));
                }
                journal.set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )?;
            }
            Ok(())
        };
        for x in -7..=18 {
            for z in -2..=14 {
                put([x, 15, z], 43, None)?;
                for y in 16..=24 {
                    put([x, y, z], 0, None)?;
                }
            }
        }
        // Table foot and four cardinal wall brackets are genuine supported poses.
        for x in 3..=5 {
            for z in 8..=10 {
                put([x, 16, z], 16, None)?;
            }
        }
        put([4, 17, 9], LANTERN, Some(0))?;
        for (p, pose) in [
            ([-4, 18, 8], 6),
            ([14, 18, 8], 4),
            ([-4, 18, 3], 5),
            ([5, 18, 1], 7),
        ] {
            let support = terra_core::lantern::support(p, LANTERN, pose).unwrap();
            for y in 16..=21 {
                put([support[0], y, support[2]], 36, None)?;
            }
            put(p, LANTERN, Some(pose))?;
        }
        for y in 16..=23 {
            put([11, y, 9], 36, None)?;
        }
        for x in 9..=11 {
            put([x, 23, 9], 36, None)?;
        }
        for y in (19..=22).rev() {
            put([9, y, 9], CHAIN, Some(0))?;
        }
        put([9, 18, 9], LANTERN, Some(8))?;
        Ok(())
    })
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut gallery: Mut<Gallery>| {
        if gallery.stage >= VIEWS.len() {
            return;
        }
        if gallery.failed
            || gallery.started.elapsed() > Duration::from_secs(150)
            || world
                .resource::<terra_render::RenderStatus>()
                .error
                .is_some()
        {
            error!(stage = gallery.stage, "Lantern native gallery failed");
            gallery.stage = VIEWS.len();
            world.write_message(AppExit::error());
            return;
        }
        if !gallery.placed {
            let chunks = world.resource::<ChunkStore>();
            if [[-7, 15, -2], [18, 24, 14], [-7, 24, 14], [18, 15, -2]]
                .into_iter()
                .any(|p| chunks.block_at(p).is_none())
            {
                return;
            }
            if let Err(reason) = build(world) {
                error!(%reason,"Lantern native gallery placement failed");
                gallery.failed = true;
                return;
            }
            gallery.placed = true;
            gallery.since = Instant::now();
            return;
        }
        let status = world.resource::<terra_render::RenderStatus>();
        if !status.ready {
            gallery.since = Instant::now();
            return;
        }
        if gallery.since.elapsed() < Duration::from_secs(3) {
            return;
        }
        if !gallery.requested {
            let lights = world
                .query::<(&Name, &PointLight)>()
                .iter(world)
                .filter(|(name, light)| {
                    name.as_str() == "Voxel local light" && light.intensity > 0.
                })
                .count();
            if lights < 2 {
                return;
            }
            let directory = std::env::var_os("TERRA_CAPTURE_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("terra-lanterns"));
            if let Err(reason) = std::fs::create_dir_all(&directory) {
                error!(%reason,"Cannot create lantern capture directory");
                gallery.failed = true;
                return;
            }
            info!(
                view = VIEWS[gallery.stage].0,
                real_point_lights = lights,
                "Lantern native capture"
            );
            world
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    directory.join(format!("{}.png", VIEWS[gallery.stage].0)),
                ))
                .observe(
                    |capture: On<ScreenshotCaptured>, mut gallery: ResMut<Gallery>| {
                        gallery.captures += 1;
                        gallery.failed |= capture.image.data.as_ref().is_none_or(|data| {
                            let min = data.chunks_exact(4).map(|p| p[0]).min().unwrap_or(0);
                            let max = data.chunks_exact(4).map(|p| p[0]).max().unwrap_or(0);
                            max.saturating_sub(min) < 12
                        });
                    },
                );
            gallery.requested = true;
        } else if gallery.captures > gallery.stage {
            gallery.stage += 1;
            if gallery.stage == VIEWS.len() {
                info!(captures = gallery.captures, "Lantern native gallery passed");
                world.write_message(AppExit::Success);
            } else {
                view(world, gallery.stage);
                gallery.since = Instant::now();
                gallery.requested = false;
            }
        }
    });
}
