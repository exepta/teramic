//! Brief hurt recoil follows confirmed health changes without changing aim.
use super::{CameraLook, TerraCamera};
use bevy::prelude::*;

const DURATION: f32 = 0.36;

#[derive(Default)]
pub(crate) struct HurtMotion {
    previous: Option<(terra_core::PlayerUuid, f32)>,
    remaining: f32,
    strength: f32,
    direction: f32,
}

impl HurtMotion {
    fn sample(&mut self, current: Option<(terra_core::PlayerUuid, f32, f32)>, dt: f32) -> Vec2 {
        self.remaining = (self.remaining - dt.clamp(0.0, 0.1)).max(0.0);
        let Some((uuid, health, maximum)) = current else {
            *self = Self::default();
            return Vec2::ZERO;
        };
        if let Some((old_uuid, previous)) = self.previous {
            if uuid != old_uuid || health <= 0.0 {
                self.remaining = 0.0;
            } else if health < previous.min(maximum) {
                self.remaining = DURATION;
                self.strength = (0.55 + (previous.min(maximum) - health) * 0.025).min(1.0);
                self.direction = if self.direction > 0.0 { -1.0 } else { 1.0 };
            }
        }
        self.previous = Some((uuid, health));
        let t = 1.0 - self.remaining / DURATION;
        let envelope = (std::f32::consts::PI * t).sin().max(0.0) * (1.0 - t);
        Vec2::new(0.035, 0.08 * self.direction) * envelope * self.strength
    }
}

pub(crate) fn update(
    time: Res<Time>,
    session: Option<Res<terra_network::client::ClientSession>>,
    construction: Option<Res<super::ConstructionCamera>>,
    mut motion: Local<HurtMotion>,
    mut cameras: Query<(&mut Transform, &mut CameraLook), With<TerraCamera>>,
) {
    let current = session
        .as_ref()
        .filter(|s| s.ready && s.game_mode == terra_core::GameMode::Survival && !s.teleporting)
        .and_then(|s| {
            s.uuid
                .map(|uuid| (uuid, s.stats.survival.health, s.stats.survival.max_health))
        });
    let offset = motion.sample(current, time.delta_secs());
    if construction.as_ref().is_some_and(|c| c.active) || offset == Vec2::ZERO {
        return;
    }
    for (mut camera, mut look) in &mut cameras {
        camera.rotation *= Quat::from_euler(EulerRot::XYZ, offset.x, 0.0, offset.y);
        // Targeting continues using CameraLook's logical yaw/pitch. Mark the
        // rendered recoil so sync_external cannot feed it back into aim.
        look.presented(camera.rotation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cactus_recoil_is_a_bounded_pulse_and_never_retriggers_on_unchanged_health() {
        let id = terra_core::PlayerUuid::from_bytes([1; 16]);
        let mut motion = HurtMotion::default();
        assert_eq!(motion.sample(Some((id, 100.0, 100.0)), 0.016), Vec2::ZERO);
        motion.sample(Some((id, 98.0, 100.0)), 0.016);
        let mut peak = 0.0f32;
        for _ in 0..60 {
            peak = peak.max(motion.sample(Some((id, 98.0, 100.0)), 0.016).length());
        }
        assert!(peak > 0.01 && peak < 0.09);
        assert_eq!(motion.remaining, 0.0);
        assert_eq!(motion.sample(Some((id, 100.0, 100.0)), 0.016), Vec2::ZERO);
        assert_eq!(
            motion.sample(
                Some((terra_core::PlayerUuid::from_bytes([2; 16]), 20.0, 100.0)),
                0.016
            ),
            Vec2::ZERO
        );
        assert_eq!(motion.sample(None, 0.016), Vec2::ZERO);
    }

    #[test]
    fn confirmed_cactus_damage_moves_rendered_camera_and_preserves_exact_aim() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_network::client::ClientSession>()
            .add_systems(Update, update);
        {
            let mut session = app
                .world_mut()
                .resource_mut::<terra_network::client::ClientSession>();
            session.ready = true;
            session.uuid = Some(terra_core::PlayerUuid::from_bytes([7; 16]));
            session.game_mode = terra_core::GameMode::Survival;
        }
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Transform::default()))
            .id();
        app.world_mut()
            .get_mut::<CameraLook>(camera)
            .unwrap()
            .aim(Quat::IDENTITY);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();
        {
            let mut session = app
                .world_mut()
                .resource_mut::<terra_network::client::ClientSession>();
            session
                .stats
                .survival
                .hurt(2.0, terra_core::DamageCause::Cactus);
        }
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(40));
        app.update();
        let world = app.world_mut();
        let rendered = world.get::<Transform>(camera).unwrap().rotation;
        assert!(!rendered.abs_diff_eq(Quat::IDENTITY, 0.001));
        let mut look = world.get_mut::<CameraLook>(camera).unwrap();
        look.sync_external(rendered);
        assert_eq!(look.rotation(), Quat::IDENTITY);
    }
}
