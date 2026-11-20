use super::{
    CameraInputState, CameraLook, ControllerMode, FlightMotion, PlayerInputBlock, TerraCamera,
};
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
use terra_core::GameConfig;
use terra_core::{GameAction as Action, GameplayInput};
use terra_world::streaming::ChunkFocus;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct CameraControl<'w> {
    construction: Option<Res<'w, super::ConstructionCamera>>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    config: Res<'w, GameConfig>,
    mode: Res<'w, ControllerMode>,
    blocked: Res<'w, PlayerInputBlock>,
}

pub(crate) fn move_camera(
    time: Res<Time>,
    keys: GameplayInput,
    mouse: Res<AccumulatedMouseMotion>,
    mut cameras: Query<(&mut Transform, &mut FlightMotion, &mut CameraLook), With<TerraCamera>>,
    mut focus: ResMut<ChunkFocus>,
    mut input: ResMut<CameraInputState>,
    control: CameraControl,
) {
    if control
        .construction
        .as_ref()
        .is_some_and(|camera| camera.active)
    {
        for (_, mut motion, _) in &mut cameras {
            motion.velocity = Vec3::ZERO;
        }
        return;
    }
    if !input.captured || control.blocked.0 {
        for (transform, mut motion, mut look) in &mut cameras {
            look.sync_external(transform.rotation);
            motion.velocity = Vec3::ZERO;
        }
        return;
    }
    let delta = if input.suppress_motion {
        Vec2::ZERO
    } else {
        mouse.delta
    };
    input.suppress_motion = false;
    for (mut transform, mut motion, mut look) in &mut cameras {
        look.sync_external(transform.rotation);
        if input.just_captured {
            motion.velocity = Vec3::ZERO;
        }
        let (mut yaw, mut pitch, _) = look.rotation().to_euler(EulerRot::YXZ);
        yaw -= delta.x * control.config.client.mouse_sensitivity;
        pitch = (pitch - delta.y * control.config.client.mouse_sensitivity).clamp(-1.5, 1.5);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        look.aim(transform.rotation);
        if *control.mode == ControllerMode::Walk || control.session.is_some() {
            motion.velocity = Vec3::ZERO;
            continue;
        }
        let mut forward = *transform.forward();
        forward.y = 0.0;
        forward = forward.normalize_or_zero();
        let right = Vec3::new(-forward.z, 0.0, forward.x);
        let mut direction = Vec3::ZERO;
        if keys.pressed(Action::Forward) {
            direction += forward;
        }
        if keys.pressed(Action::Backward) {
            direction -= forward;
        }
        if keys.pressed(Action::Left) {
            direction -= right;
        }
        if keys.pressed(Action::Right) {
            direction += right;
        }
        if keys.pressed(Action::Jump) {
            direction += Vec3::Y;
        }
        if keys.pressed(Action::FlyDown) {
            direction -= Vec3::Y;
        }
        let precision = keys.pressed(Action::Precision);
        let boost = keys.pressed(Action::Sprint);
        let multiplier = if precision {
            control.config.client.fly_precision_multiplier
        } else if boost {
            control.config.client.fly_boost_multiplier
        } else {
            1.0
        };
        let target = direction.normalize_or_zero() * control.config.client.fly_speed * multiplier;
        if target == Vec3::ZERO {
            motion.velocity = Vec3::ZERO;
        } else {
            let dt = time.delta_secs();
            // Integrate the exponential response exactly so frame rate cannot change travel.
            let retention = (-10.0 * dt).exp();
            let previous = motion.velocity;
            transform.translation += target * dt + (previous - target) * ((1.0 - retention) / 10.0);
            motion.velocity = target + (previous - target) * retention;
        }
        focus.0 = transform.translation.to_array().map(f64::from);
    }
}
