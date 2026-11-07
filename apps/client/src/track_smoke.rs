//! Explicit visual/expiry fixture. It injects contact messages, not real input.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use terra_core::footsteps::{Footstep, FootstepSet, Walker};
#[derive(Resource)]
struct Probe {
    started: std::time::Instant,
    born: Option<f32>,
    shots: usize,
    done: usize,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-tracks") {
        return;
    }
    app.insert_resource(terra_player::ControllerMode::Fly)
        .insert_resource(Probe {
            started: std::time::Instant::now(),
            born: None,
            shots: 0,
            done: 0,
        })
        .add_systems(PostStartup, setup)
        .add_systems(
            Update,
            run.after(terra_player::PlayerSet::Movement)
                .before(FootstepSet),
        );
}
fn setup(mut journal: ResMut<terra_world::edits::EditJournal>) {
    for z in 16..32 {
        for x in 0..16 {
            if let Err(error) = journal.record([x, 31, z], if x < 8 { 4 } else { 5 }) {
                error!(%error, "Cannot prepare native track fixture");
            }
        }
    }
}
fn run(world: &mut World) {
    let age = world
        .resource::<Probe>()
        .born
        .map(|born| world.resource::<Time>().elapsed_secs() - born);
    let eye = match age {
        Some(age) if (0.6..1.4).contains(&age) => {
            Transform::from_xyz(3.3, 32.7, 23.5).looking_at(Vec3::new(3.3, 32.0, 22.1), Vec3::Y)
        }
        Some(age) if (1.4..2.4).contains(&age) => {
            Transform::from_xyz(11.3, 32.7, 23.5).looking_at(Vec3::new(11.3, 32.0, 22.1), Vec3::Y)
        }
        _ => Transform::from_xyz(8.0, 40.0, 34.0).looking_at(Vec3::new(8.0, 32.0, 24.0), Vec3::Y),
    };
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        *camera = eye;
    }
    world.resource_mut::<terra_world::streaming::ChunkFocus>().0 = [8.0, 33.0, 24.0];
    if world.resource::<Probe>().started.elapsed().as_secs() > 90 {
        error!("Native track fixture timed out");
        world.write_message(AppExit::error());
        return;
    }
    if world.resource::<Probe>().born.is_none() {
        if world.resource::<Probe>().started.elapsed().as_secs() < 8
            || world.resource::<terra_render::RenderStatus>().pending_jobs != 0
        {
            return;
        }
        for material in [4, 5] {
            for walker in [Walker::Player, Walker::Cow] {
                for i in 0..5 {
                    world.write_message(Footstep {
                        position: Vec3::new(
                            if material == 4 { 3.0 } else { 11.0 } + i as f32 * 0.2,
                            32.0,
                            21.5 + i as f32 * 0.75 + if walker == Walker::Cow { 3.0 } else { 0.0 },
                        ),
                        forward: Vec2::Y,
                        walker,
                        material,
                        alternate: i % 2 == 0,
                    });
                }
            }
        }
        let now = world.resource::<Time>().elapsed_secs();
        world.resource_mut::<Probe>().born = Some(now);
        return;
    }
    let age =
        world.resource::<Time>().elapsed_secs() - world.resource::<Probe>().born.unwrap_or(0.0);
    let status = world.resource::<terra_render::FootprintStatus>();
    if age > 22.5 {
        if status.player_sand + status.player_snow + status.animal_sand + status.animal_snow != 0
            || status.vertices != 0
        {
            error!(?status, "Tracks did not expire");
            world.write_message(AppExit::error());
        } else {
            info!(
                "Native tracks passed: player 10/22s, animals 3/5s, one shared mesh; injected-contact fixture"
            );
            world.write_message(AppExit::Success);
        }
        return;
    }
    let probe = world.resource::<Probe>();
    let expected = if (probe.shots == 0 && age > 0.3)
        || (probe.shots == 1 && age > 0.85)
        || (probe.shots == 2 && age > 1.65)
    {
        Some((5, 5, 5, 5))
    } else if probe.shots == 3 && age > 11.0 {
        Some((0, 5, 0, 0))
    } else {
        None
    };
    if probe.shots != probe.done {
        return;
    }
    let Some(expected) = expected else {
        return;
    };
    if (
        status.player_sand,
        status.player_snow,
        status.animal_sand,
        status.animal_snow,
    ) != expected
    {
        error!(?status, ?expected, "Wrong native track counts");
        world.write_message(AppExit::error());
        return;
    }
    let folder = std::path::PathBuf::from(
        std::env::var("TERRA_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/terra-tracks".into()),
    );
    if let Err(error) = std::fs::create_dir_all(&folder) {
        error!(%error);
        world.write_message(AppExit::error());
        return;
    }
    let shot = world.resource::<Probe>().shots;
    let sample = if matches!(shot, 1 | 2) {
        let point = Vec3::new(
            if shot == 1 { 3.33 } else { 11.33 },
            if shot == 1 { 31.974 } else { 31.955 },
            22.32,
        );
        world
            .query_filtered::<&Camera, With<terra_player::TerraCamera>>()
            .iter(world)
            .next()
            .and_then(|camera| camera.world_to_ndc(&GlobalTransform::from(eye), point))
            .map(|ndc: Vec3| Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5))
    } else {
        None
    };
    world.resource_mut::<Probe>().shots += 1;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("tracks-{shot}.png"))))
        .observe(move |capture: On<ScreenshotCaptured>, mut probe: ResMut<Probe>, mut exit: MessageWriter<AppExit>| {
            if matches!(shot, 1 | 2) {
                let color = sample.and_then(|uv| {
                    let size = capture.image.size();
                    capture.image.get_color_at((uv.x * size.x as f32) as u32, (uv.y * size.y as f32) as u32).ok().map(|c| c.to_srgba())
                });
                let visible = color.is_some_and(|c| if shot == 1 { c.red > c.blue + 0.12 } else { c.red > c.blue - 0.10 });
                if !visible {
                    error!(shot, ?sample, ?color, "Footprint floor is missing or sky-colored; lifecycle counters alone are insufficient");
                    exit.write(AppExit::error());
                } else {
                    info!(shot, ?color, "Native recessed footprint floor pixel verified");
                }
            }
            probe.done += 1;
        });
}
