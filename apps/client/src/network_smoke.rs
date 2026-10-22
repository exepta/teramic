//! Drives the production input/network path; no world edits or fake peers.
use bevy::{
    app::AppExit, ecs::system::SystemParam, prelude::*,
    render::view::screenshot::ScreenshotCaptured,
};
use std::time::{Duration, Instant};
use terra_network::client::{ClientSession, ReplicatedPlayers};
use terra_player::{CameraInputState, PlayerBody, PlayerSet, TerraCamera};

#[derive(Resource)]
struct NetworkSmoke {
    started: Instant,
    ready: Option<Instant>,
    first_remote: Option<Instant>,
    observer: bool,
    companion: bool,
    captured: bool,
    captured_at: Option<Instant>,
    screenshot_complete: bool,
    start_position: Vec3,
    reported: Instant,
    frames: Vec<f32>,
    seconds: u64,
}
pub(crate) fn install(app: &mut App) {
    let args: Vec<_> = std::env::args().collect();
    if !args.iter().any(|a| a.starts_with("--smoke-test-network")) {
        return;
    }
    app.insert_resource(NetworkSmoke {
        started: Instant::now(),
        ready: None,
        first_remote: None,
        observer: args.iter().any(|a| a == "--smoke-test-network-observer"),
        companion: args.iter().any(|a| a == "--smoke-test-network-companion"),
        captured: false,
        captured_at: None,
        screenshot_complete: false,
        start_position: Vec3::ZERO,
        reported: Instant::now(),
        frames: Vec::new(),
        seconds: std::env::var("TERRA_NETWORK_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(5)
            .clamp(5, 300),
    })
    .add_systems(Update, drive.before(PlayerSet::Movement))
    .add_systems(
        Update,
        inspect.after(terra_world::streaming::StreamingSet::Dispatch),
    );
}
fn drive(
    state: Res<NetworkSmoke>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut capture: ResMut<CameraInputState>,
) {
    keys.release(KeyCode::KeyD);
    if state
        .ready
        .is_some_and(|since| since.elapsed() < Duration::from_millis(1200))
        && !state.observer
    {
        capture.captured = true;
        keys.press(KeyCode::KeyD);
    }
}

type RemoteAvatarQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static terra_render::RemoteAvatar,
        &'static Transform,
        &'static Children,
    ),
    (Without<TerraCamera>, Without<PlayerBody>),
>;

#[derive(SystemParam)]
struct NetworkWorld<'w, 's> {
    time: Res<'w, Time<Real>>,
    creatures: Res<'w, terra_network::client::ReplicatedCreatures>,
    session: Res<'w, ClientSession>,
    players: Res<'w, ReplicatedPlayers>,
    render: Res<'w, terra_render::RenderStatus>,
    props: Res<'w, terra_render::PropRenderStatus>,
    generation: Res<'w, terra_world::streaming::WorldGeneration>,
    bodies: Query<'w, 's, &'static Transform, (With<PlayerBody>, Without<TerraCamera>)>,
    avatars: RemoteAvatarQuery<'w, 's>,
    parts: Query<'w, 's, &'static Mesh3d>,
    meshes: Res<'w, Assets<Mesh>>,
    integrated: Option<Res<'w, terra_server::IntegratedServer>>,
}
fn inspect(
    mut commands: Commands,
    mut state: ResMut<NetworkSmoke>,
    world: NetworkWorld,
    mut cameras: Query<&mut Transform, (With<TerraCamera>, Without<PlayerBody>)>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(error) = &world.session.error {
        error!(%error,"Native network smoke failed");
        exit.write(AppExit::error());
        return;
    }
    if state.started.elapsed() > Duration::from_secs(90) {
        error!("Native network smoke timed out");
        exit.write(AppExit::error());
        return;
    }
    let Some(confirmed) = world.session.confirmed_state() else {
        return;
    };
    if state.reported.elapsed() > Duration::from_secs(1) {
        state.reported = Instant::now();
        info!(
            ack = confirmed.ack,
            pending = world.session.pending_inputs,
            tick = world.players.tick,
            "Native network progress"
        );
    }
    let Some(body) = world.bodies.iter().next() else {
        return;
    };
    if state.ready.is_none()
        && world.session.ready
        && world.render.ready
        && world.props.ready
        && world.generation.status == terra_world::streaming::GenerationStatus::Ready
        && confirmed.grounded
    {
        state.ready = Some(Instant::now());
        state.start_position = body.translation;
        info!(
            integrated = world.integrated.is_some(),
            player = confirmed.player.0,
            "Native authoritative client ready"
        );
    }
    if state.observer
        && let Some(track) = world.players.players.values().next()
        && !world.avatars.is_empty()
        && track.position().distance(body.translation) > 2.0
    {
        let valid = world.avatars.iter().any(|(avatar, transform, children)| {
            world.players.players.get(&avatar.0).is_some_and(|remote| {
                transform.translation.distance(remote.position()) < 0.15
                    && children.len() == 6
                    && children.iter().all(|child| {
                        world.parts.get(child).ok().is_some_and(|handle| {
                            world.meshes.get(&handle.0).is_some_and(|mesh| {
                                mesh.count_vertices() > 0
                                    && mesh.indices().is_some_and(|indices| !indices.is_empty())
                            })
                        })
                    })
            })
        });
        if !valid || world.avatars.iter().count() != world.players.players.len() {
            return;
        }
        state.first_remote.get_or_insert_with(Instant::now);
        for mut camera in &mut cameras {
            camera.translation = track.position() + Vec3::new(5.0, 3.8, 7.0);
            camera.look_at(track.position() + Vec3::Y * 0.2, Vec3::Y);
        }
    }
    let Some(ready) = state.ready else {
        return;
    };
    if ready.elapsed() > Duration::from_secs(2) && !state.captured && state.frames.len() < 120_000 {
        state.frames.push(world.time.delta_secs() * 1000.0);
    }
    let duration = state.seconds + if state.companion { 10 } else { 0 };
    if ready.elapsed() < Duration::from_secs(duration) {
        return;
    }
    if state.observer
        && state
            .first_remote
            .is_none_or(|t| t.elapsed() < Duration::from_secs(1))
    {
        return;
    }
    if confirmed.ack < 20 || world.session.pending_inputs >= 64 {
        error!(
            ack = confirmed.ack,
            pending = world.session.pending_inputs,
            "Authoritative input acknowledgement stalled"
        );
        exit.write(AppExit::error());
        return;
    }
    if !state.captured {
        if !state.frames.is_empty() {
            let mut frames = state.frames.clone();
            frames.sort_by(f32::total_cmp);
            let mean = frames.iter().sum::<f32>() / frames.len() as f32;
            info!(
                samples = frames.len(),
                fps = 1000.0 / mean,
                p95_ms = frames[(frames.len() * 95 / 100).min(frames.len() - 1)],
                p99_ms = frames[(frames.len() * 99 / 100).min(frames.len() - 1)],
                animals = world.creatures.creatures.len(),
                "Two-client native frame sample (not a parity benchmark)"
            );
        }
        let path = std::env::var_os("TERRA_SCREENSHOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/terra-network.png"));
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path))
            .observe(
                |event: On<ScreenshotCaptured>, mut state: ResMut<NetworkSmoke>| {
                    let colors: std::collections::HashSet<_> = event
                        .image
                        .data
                        .as_deref()
                        .unwrap_or_default()
                        .chunks_exact(4)
                        .step_by(101)
                        .take(4096)
                        .map(|rgba| [rgba[0], rgba[1], rgba[2]])
                        .collect();
                    state.screenshot_complete =
                        event.image.width() > 0 && event.image.height() > 0 && colors.len() >= 16;
                },
            );
        state.captured = true;
        state.captured_at = Some(Instant::now());
        info!(
            player = confirmed.player.0,
            ack = confirmed.ack,
            pending = world.session.pending_inputs,
            remote_players = world.players.players.len(),
            avatars = world.avatars.iter().count(),
            distance = body.translation.distance(state.start_position),
            "Native authoritative session captured"
        );
    }
    if state.screenshot_complete
        && state
            .captured_at
            .is_some_and(|time| time.elapsed() > Duration::from_secs(1))
    {
        info!("Native network smoke completed");
        exit.write(AppExit::Success);
    }
}
