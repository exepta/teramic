use super::gait::{GaitFrame, WalkCameraMotion};
use super::{CameraInputState, CameraLook, TerraCamera};
#[cfg(test)]
use avian3d::prelude::*;
use bevy::prelude::*;
use terra_core::{GameAction as Action, GameplayInput};
use terra_network::client::ClientSession;
#[cfg(test)]
use terra_physics::TerrainCollisionState;
pub use terra_physics::character::{
    CharacterBody as PlayerBody, CharacterGrounded as PlayerGrounded, PLAYER_EYE_OFFSET,
    PLAYER_HALF_HEIGHT, PLAYER_RADIUS,
};
use terra_physics::character::{
    CharacterInput, CharacterSimulation, CharacterState, body_bundle, space_clear,
};
#[cfg(test)]
use terra_world::edits::EditJournal;
use terra_world::streaming::{ChunkFocus, ChunkStore};

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ControllerMode {
    #[default]
    Walk,
    Fly,
}
#[derive(Resource, Debug, Default)]
pub struct PlayerInputBlock(pub bool);
#[derive(Resource, Debug, Default)]
pub struct PlayerControllerStatus {
    pub waiting_for_terrain: bool,
}
#[derive(Resource, Default)]
pub(crate) struct WalkIntent {
    direction: Vec3,
    jump: bool,
    sprint: bool,
    flight_speed: Option<f32>,
    swimming: bool,
    diving: bool,
}

pub(crate) fn sync_controller(
    mut commands: Commands,
    mut mode: ResMut<ControllerMode>,
    chunks: Res<ChunkStore>,
    mut cameras: Query<(&mut Transform, &mut CameraLook), With<TerraCamera>>,
    bodies: Query<Entity, With<PlayerBody>>,
    session: Option<ResMut<ClientSession>>,
    mut status: ResMut<PlayerControllerStatus>,
) {
    if session
        .as_ref()
        .is_some_and(|s| s.ready && s.game_mode == terra_core::GameMode::Spectator)
    {
        *mode = ControllerMode::Fly;
    }
    if session
        .as_ref()
        .is_some_and(|s| s.ready && (!s.allow_flight || s.mounted.is_some()))
        && *mode == ControllerMode::Fly
    {
        *mode = ControllerMode::Walk;
    }
    if session.as_ref().is_some_and(|s| !s.ready) {
        for body in &bodies {
            commands.entity(body).despawn();
        }
        status.waiting_for_terrain = true;
        return;
    }
    if *mode == ControllerMode::Fly && session.is_none() {
        for body in &bodies {
            commands.entity(body).despawn();
        }
        status.waiting_for_terrain = false;
        return;
    }
    if !bodies.is_empty() {
        return;
    }
    status.waiting_for_terrain = true;
    if let Some(mut session) = session {
        if session.ready {
            if let Some(location) = session.initial_location.take() {
                *mode = if location.flying && session.allow_flight {
                    ControllerMode::Fly
                } else {
                    ControllerMode::Walk
                };
                session.look = location.look;
                for (mut camera, mut look) in &mut cameras {
                    camera.translation = session.spawn + Vec3::Y * PLAYER_EYE_OFFSET;
                    camera.rotation =
                        Quat::from_euler(EulerRot::YXZ, location.look[0], location.look[1], 0.0);
                    look.aim(camera.rotation);
                }
            }
            commands.spawn(body_bundle(session.spawn));
        }
        return;
    }
    let Some((camera, _)) = cameras.iter().next() else {
        return;
    };
    let initial = camera.translation - Vec3::Y * PLAYER_EYE_OFFSET;
    if let Some(position) = (0..=64)
        .map(|step| initial + Vec3::Y * (step as f32 * 0.5))
        .find(|&p| space_clear(&chunks, p))
    {
        commands.spawn(body_bundle(position));
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WalkControl<'w> {
    input: Res<'w, CameraInputState>,
    blocked: Res<'w, PlayerInputBlock>,
    mode: Res<'w, ControllerMode>,
    session: Option<ResMut<'w, ClientSession>>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    controls: Option<Res<'w, terra_core::ControlsConfig>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    chunks: Option<Res<'w, ChunkStore>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
}
pub(crate) fn read_walk_input(
    keys: GameplayInput,
    cameras: Query<(&Transform, &CameraLook), With<TerraCamera>>,
    bodies: Query<&Transform, With<PlayerBody>>,
    mut control: WalkControl,
    mut intent: ResMut<WalkIntent>,
) {
    let Some((camera, look)) = cameras.iter().next() else {
        return;
    };
    let rotation = look.rotation();
    let gameplay_active =
        !control.blocked.0 && control.input.captured && !control.input.just_captured;
    if let Some(session) = control.session.as_mut() {
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        session.look = [yaw, pitch];
        if gameplay_active && session.mounted.is_some() && keys.just_pressed(Action::Dismount) {
            session.dismount_requested = true;
        }
        if session.sleeping().is_some() {
            if gameplay_active
                && [
                    Action::Forward,
                    Action::Backward,
                    Action::Left,
                    Action::Right,
                    Action::Jump,
                    Action::Break,
                ]
                .into_iter()
                .any(|action| keys.just_pressed(action))
            {
                session.wake_requested = true;
            }
            *intent = WalkIntent::default();
            return;
        }
    }
    let network_flight = control
        .session
        .as_ref()
        .is_some_and(|s| s.allow_flight && s.mounted.is_none())
        && *control.mode == ControllerMode::Fly;
    if (*control.mode != ControllerMode::Walk && !network_flight)
        || control.blocked.0
        || !control.input.captured
        || control.input.just_captured
    {
        *intent = WalkIntent::default();
        return;
    }
    let aim = rotation * Vec3::NEG_Z;
    let forward = Vec3::new(aim.x, 0.0, aim.z).normalize_or_zero();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    let horizontal = f32::from(u8::from(keys.pressed(Action::Right)))
        - f32::from(u8::from(keys.pressed(Action::Left)));
    let vertical = f32::from(u8::from(keys.pressed(Action::Forward)))
        - f32::from(u8::from(keys.pressed(Action::Backward)));
    intent.direction = (forward * vertical + right * horizontal).normalize_or_zero();
    let mounted = control
        .session
        .as_ref()
        .is_some_and(|s| s.mounted.is_some());
    intent.sprint = keys.pressed(Action::Sprint) || mounted && keys.pressed(Action::FlyDown);
    intent.jump |= keys.just_pressed(Action::Jump);
    intent.flight_speed = None;
    intent.swimming = false;
    if let (Some(config), Some(terrain), Some(chunks), Some(journal)) = (
        &control.config,
        &control.terrain,
        &control.chunks,
        &control.journal,
    ) && !mounted
        && terra_physics::character::in_water(
            &config.world,
            &terrain.0,
            chunks,
            journal,
            bodies
                .iter()
                .next()
                .map_or(camera.translation - Vec3::Y * PLAYER_EYE_OFFSET, |body| {
                    body.translation
                }),
        )
    {
        intent.swimming = true;
        if keys.just_pressed(Action::Sprint) {
            intent.diving = !intent.diving;
        }
        if keys.pressed(Action::Jump) {
            intent.diving = false;
        }
        intent.sprint = intent.diving;
        intent.jump = keys.pressed(Action::Jump);
        intent.direction = ((if intent.diving { aim } else { forward }) * vertical
            + right * horizontal)
            .normalize_or_zero();
        intent.direction.y = if keys.pressed(Action::FlyDown) {
            -1.0
        } else {
            intent.direction.y
        };
        intent.direction = intent.direction.clamp_length_max(1.0);
    }
    if !intent.swimming {
        intent.diving = false;
    }
    if network_flight {
        intent.direction.y = f32::from(u8::from(keys.pressed(Action::Jump)))
            - f32::from(u8::from(keys.pressed(Action::FlyDown)));
        intent.direction = intent.direction.normalize_or_zero();
        let speed = control.config.as_ref().map_or(20.0, |c| c.client.fly_speed);
        let multiplier = if keys.pressed(Action::Precision) {
            control
                .config
                .as_ref()
                .map_or(0.25, |c| c.client.fly_precision_multiplier)
        } else if intent.sprint {
            control
                .config
                .as_ref()
                .map_or(3.0, |c| c.client.fly_boost_multiplier)
        } else {
            1.0
        };
        intent.flight_speed = Some((speed * multiplier).clamp(0.0, 60.0));
        intent.jump = false;
    }
}

pub(crate) fn walk_step(
    mut bodies: terra_physics::character::ControlledBodies,
    simulation: CharacterSimulation,
    mut intent: ResMut<WalkIntent>,
    mut control: WalkControl,
    mut status: ResMut<PlayerControllerStatus>,
) {
    let accepts = control.input.captured && !control.blocked.0;
    let flight = *control.mode == ControllerMode::Fly
        && control
            .session
            .as_ref()
            .is_some_and(|s| s.allow_flight && s.mounted.is_none());
    let input = CharacterInput {
        direction: if accepts {
            intent.direction
        } else {
            Vec3::ZERO
        },
        jump: accepts && intent.jump,
        sprint: accepts && intent.sprint,
        auto_climb: accepts && control.controls.as_ref().is_some_and(|c| c.auto_climb),
        flight_speed: flight.then_some(if accepts {
            intent.flight_speed.unwrap_or(0.0)
        } else {
            0.0
        }),
    };
    let mut consumed_input = false;
    for (entity, mut transform, mut velocity, collider, mut grounded, pose) in &mut bodies {
        if let Some(mut pose) = pose {
            pose.previous = transform.translation;
        }
        let mut state = CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        let moved = if let Some(session) = control.session.as_mut() {
            session.predict(entity, collider, &mut state, input, &simulation)
        } else {
            simulation.step(entity, collider, &mut state, input)
        };
        status.waiting_for_terrain = !moved;
        consumed_input |= moved;
        transform.translation = state.position;
        velocity.0 = state.velocity;
        grounded.0 = state.grounded;
    }
    if consumed_input && !intent.swimming {
        intent.jump = false;
    }
}

type PresentedBodies<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static mut terra_physics::character::CharacterPresentation>,
        Option<&'static PlayerGrounded>,
        Option<&'static avian3d::prelude::LinearVelocity>,
    ),
    (With<PlayerBody>, Without<TerraCamera>),
>;

type PresentedCameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static CameraLook,
        &'static mut WalkCameraMotion,
    ),
    (With<TerraCamera>, Without<PlayerBody>),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct CameraPresentationControl<'w> {
    mode: Res<'w, ControllerMode>,
    session: Option<Res<'w, ClientSession>>,
    fixed: Res<'w, Time<Fixed>>,
    time: Res<'w, Time>,
    input: Res<'w, CameraInputState>,
    blocked: Res<'w, PlayerInputBlock>,
    intent: Res<'w, WalkIntent>,
    perspective: Option<Res<'w, super::CameraPerspective>>,
    gameplay: Option<Res<'w, terra_core::GameplayConfig>>,
}

pub(crate) fn follow_player_camera(
    mut bodies: PresentedBodies,
    mut cameras: PresentedCameras,
    mut focus: ResMut<ChunkFocus>,
    control: CameraPresentationControl,
) {
    if *control.mode != ControllerMode::Walk && control.session.is_none() {
        return;
    }
    let Some((entity, body, pose, grounded, velocity)) = bodies.iter_mut().next() else {
        return;
    };
    let mut visual = if let Some(mut pose) = pose {
        pose.correction *= (-18.0 * control.time.delta_secs()).exp();
        pose.visual_position(body.translation, control.fixed.overstep_fraction())
    } else {
        body.translation
    };
    if control
        .session
        .as_ref()
        .is_some_and(|s| s.mounted.is_some())
    {
        visual.y -= 0.60;
    }
    let enabled = *control.mode == ControllerMode::Walk
        && control.session.as_ref().is_none_or(|s| s.mounted.is_none())
        && control
            .session
            .as_ref()
            .is_none_or(|session| session.sleeping().is_none())
        && control.input.captured
        && !control.input.just_captured
        && !control.blocked.0
        && !control.intent.swimming
        && control
            .perspective
            .as_deref()
            .is_none_or(|perspective| !perspective.is_third_person())
        && control
            .gameplay
            .as_deref()
            .is_none_or(|settings| settings.view_bobbing);
    for (mut camera, look, mut motion) in &mut cameras {
        let step_offset = if control
            .session
            .as_ref()
            .is_some_and(|s| s.mounted.is_some())
        {
            motion.reset_step(entity, visual);
            0.0
        } else {
            motion.step_height(
                entity,
                visual,
                grounded.is_some_and(|grounded| grounded.0),
                control.time.delta_secs(),
            )
        };
        let offset = motion.offset(GaitFrame {
            body: entity,
            position: visual,
            velocity: velocity.map_or(Vec3::ZERO, |velocity| velocity.0),
            grounded: grounded.is_some_and(|grounded| grounded.0),
            enabled,
            moving: control.intent.direction.length_squared() > 0.001,
            delta: control.time.delta_secs(),
        });
        let right = look.rotation() * Vec3::X;
        let right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
        camera.translation =
            visual + Vec3::Y * (PLAYER_EYE_OFFSET + offset.y + step_offset) + right * offset.x;
        focus.0 = (body.translation + Vec3::Y * PLAYER_EYE_OFFSET)
            .to_array()
            .map(f64::from);
    }
}
pub(crate) fn block_overlaps_body(block: [i64; 3], center: Vec3) -> bool {
    let minimum = Vec3::from_array(block.map(|v| v as f32));
    let maximum = minimum + Vec3::ONE;
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    (center + half).cmpgt(minimum).all() && (center - half).cmplt(maximum).all()
}
#[cfg(test)]
mod tests;
