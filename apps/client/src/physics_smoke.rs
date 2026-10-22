use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use std::time::{Duration, Instant};
use terra_physics::TerrainCollisionState;
use terra_player::{
    CameraInputState, ControllerMode, PlayerBody, PlayerControllerStatus, PlayerInputBlock,
    TerraCamera,
};
use terra_render::RenderStatus;
use terra_world::streaming::{BlockEditRequest, ChunkStore, GenerationStatus, WorldGeneration};

#[derive(Resource)]
struct PhysicsSmoke {
    started: Instant,
    stage: u8,
    stage_elapsed: f32,
    base: [i64; 3],
    ground_height: f32,
    peak_height: f32,
    hold_position: Vec3,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|argument| argument == "--smoke-test-physics") {
        return;
    }
    app.insert_resource(ControllerMode::Walk)
        .insert_resource(PhysicsSmoke {
            started: Instant::now(),
            stage: 0,
            stage_elapsed: 0.0,
            base: [0; 3],
            ground_height: 0.0,
            peak_height: 0.0,
            hold_position: Vec3::ZERO,
        })
        .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
}

#[derive(SystemParam)]
struct SmokeWorld<'w> {
    time: Res<'w, Time>,
    chunks: Res<'w, ChunkStore>,
    generation: Res<'w, WorldGeneration>,
    collision: Res<'w, TerrainCollisionState>,
    controller: Res<'w, PlayerControllerStatus>,
    renderer: Res<'w, RenderStatus>,
    mode: Res<'w, ControllerMode>,
}

#[derive(SystemParam)]
struct SmokeInput<'w> {
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    capture: ResMut<'w, CameraInputState>,
    blocked: ResMut<'w, PlayerInputBlock>,
}

#[derive(SystemParam)]
struct SmokeActors<'w, 's> {
    bodies: terra_physics::character::ControlledBodies<'w, 's>,
    cameras: Query<'w, 's, &'static mut Transform, (With<TerraCamera>, Without<PlayerBody>)>,
}

fn advance(smoke: &mut PhysicsSmoke, stage: u8) {
    smoke.stage = stage;
    smoke.stage_elapsed = 0.0;
}

fn run(
    mut commands: Commands,
    mut smoke: ResMut<PhysicsSmoke>,
    world: SmokeWorld,
    mut input: SmokeInput,
    mut actors: SmokeActors,
    mut edits: MessageWriter<BlockEditRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if smoke.stage == 9 {
        return;
    }
    let error = world
        .collision
        .error
        .as_deref()
        .or(world.renderer.error.as_deref());
    if let Some(error) = error {
        tracing::error!(%error,"Physics smoke failed");
        smoke.stage = 9;
        exit.write(AppExit::error());
        return;
    }
    if smoke.started.elapsed() > Duration::from_secs(150) || *world.mode != ControllerMode::Walk {
        tracing::error!(
            stage = smoke.stage,
            "Physics smoke timed out or left Walk mode"
        );
        smoke.stage = 9;
        exit.write(AppExit::error());
        return;
    }
    smoke.stage_elapsed += world.time.delta_secs();
    let Some((_, mut body, mut velocity, _, mut grounded, presentation)) =
        actors.bodies.iter_mut().next()
    else {
        return;
    };
    input.capture.captured = true;
    match smoke.stage {
        0 => {
            if world.generation.status != GenerationStatus::Ready {
                return;
            }
            let base = [
                body.translation.x.floor() as i64 - 1,
                body.translation.y.ceil() as i64 + 4,
                body.translation.z.floor() as i64 - 1,
            ];
            // Session-only fixture: a floor, a wall and clear headroom in loaded voxel cells.
            for z in 0..3 {
                for y in 0..5 {
                    for x in 0..7 {
                        let position = [base[0] + x, base[1] + y, base[2] + z];
                        let Some(expected_material) = world.chunks.block_at(position) else {
                            tracing::error!(?position, "Physics fixture outside loaded world");
                            smoke.stage = 9;
                            exit.write(AppExit::error());
                            return;
                        };
                        let material = if y == 0 || (x == 5 && y <= 3) { 3 } else { 0 };
                        if material != expected_material {
                            edits.write(BlockEditRequest {
                                position,
                                expected_material,
                                material,
                            });
                        }
                    }
                }
            }
            smoke.base = base;
            body.translation = Vec3::new(
                base[0] as f32 + 1.5,
                base[1] as f32 + 4.0,
                base[2] as f32 + 1.5,
            );
            // Teleporting the fixture must not reuse the previous ground contact
            // on a render frame before the next fixed simulation tick.
            grounded.0 = false;
            velocity.0 = Vec3::ZERO;
            if let Some(mut presentation) = presentation {
                presentation.previous = body.translation;
                presentation.correction = Vec3::ZERO;
            }
            for mut camera in &mut actors.cameras {
                camera.rotation = Quat::IDENTITY;
            }
            input.blocked.0 = false;
            advance(&mut smoke, 1);
        }
        1 => {
            if world.generation.status != GenerationStatus::Ready
                || world.controller.waiting_for_terrain
            {
                return;
            }
            if grounded.0 {
                let expected = smoke.base[1] as f32 + 1.9;
                if (body.translation.y - expected).abs() > 0.1 {
                    tracing::error!(
                        height = body.translation.y,
                        expected,
                        "Physics landing height mismatch"
                    );
                    smoke.stage = 9;
                    exit.write(AppExit::error());
                    return;
                }
                smoke.ground_height = body.translation.y;
                smoke.peak_height = body.translation.y;
                input.keys.press(KeyCode::Space);
                advance(&mut smoke, 2);
                tracing::info!(
                    height = body.translation.y,
                    colliders = world.collision.active_colliders,
                    "Physics smoke landed on generated collider"
                );
            }
        }
        2 => {
            input.keys.release(KeyCode::Space);
            smoke.peak_height = smoke.peak_height.max(body.translation.y);
            if smoke.stage_elapsed > 1.5 && grounded.0 {
                if smoke.peak_height - smoke.ground_height < 0.7 {
                    tracing::error!(peak = smoke.peak_height, "Physics smoke jump did not rise");
                    smoke.stage = 9;
                    exit.write(AppExit::error());
                    return;
                }
                input.keys.press(KeyCode::KeyD);
                advance(&mut smoke, 3);
            }
        }
        3 => {
            input.keys.press(KeyCode::KeyD);
            if smoke.stage_elapsed > 1.6 {
                let relative = body.translation.x - smoke.base[0] as f32;
                if !(4.5..4.8).contains(&relative) {
                    tracing::error!(relative, "Physics smoke did not stop at voxel wall");
                    smoke.stage = 9;
                    exit.write(AppExit::error());
                    return;
                }
                input.keys.release(KeyCode::KeyD);
                input.keys.press(KeyCode::KeyA);
                input.keys.press(KeyCode::Space);
                input.blocked.0 = true;
                smoke.hold_position = body.translation;
                advance(&mut smoke, 4);
                tracing::info!(
                    relative,
                    jump_height = smoke.peak_height - smoke.ground_height,
                    "Physics smoke jump and wall collision passed"
                );
            }
        }
        4 => {
            if smoke.stage_elapsed < 0.6 {
                return;
            }
            if body.translation.distance(smoke.hold_position) > 0.05 {
                tracing::error!(position=?body.translation,"Physics smoke menu leaked movement");
                smoke.stage = 9;
                exit.write(AppExit::error());
                return;
            }
            input.keys.release(KeyCode::KeyA);
            input.keys.release(KeyCode::Space);
            let position = [
                body.translation.x.floor() as i64,
                smoke.base[1],
                body.translation.z.floor() as i64,
            ];
            let Some(expected_material) = world.chunks.block_at(position) else {
                return;
            };
            edits.write(BlockEditRequest {
                position,
                expected_material,
                material: 0,
            });
            advance(&mut smoke, 5);
        }
        5 if body.translation.y < smoke.ground_height - 1.0 => {
            tracing::info!(
                height = body.translation.y,
                colliders = world.collision.active_colliders,
                elapsed_seconds = smoke.started.elapsed().as_secs_f64(),
                "Physics smoke passed: fall, grounding, jump, wall, menu and mined-support fall"
            );
            if let Some(path) = std::env::var_os("TERRA_SCREENSHOT") {
                commands
                    .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(
                        std::path::PathBuf::from(path),
                    ));
            }
            advance(&mut smoke, 6);
        }
        6 if smoke.stage_elapsed > 1.0 => {
            smoke.stage = 9;
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
