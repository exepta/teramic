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
    settings: Option<Res<terra_core::GameplayConfig>>,
    mut motion: Local<HurtMotion>,
    bodies: Query<&Transform, (With<super::PlayerBody>, Without<TerraCamera>)>,
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
    if construction.as_ref().is_some_and(|c| c.active) {
        return;
    }
    let event = session
        .as_deref()
        .filter(|_| settings.as_ref().is_none_or(|s| s.view_bobbing))
        .and_then(terra_network::client::ClientSession::current_eruption);
    let listener = bodies
        .iter()
        .next()
        .map(|body| body.translation.as_dvec3().to_array());
    for (mut camera, mut look) in &mut cameras {
        let mut tilt = Vec3::new(offset.x, 0., offset.y);
        if let Some(event) = event {
            let gain =
                event.tremor(listener.unwrap_or_else(|| camera.translation.as_dvec3().to_array()));
            let t = event.age;
            tilt += Vec3::new(
                0.009 * (t * 28.1).sin() + 0.004 * (t * 41.9).sin(),
                0.005 * (t * 21.3).sin(),
                0.012 * (t * 18.6 + (t * 4.9).sin()).sin(),
            ) * gain;
        }
        if tilt == Vec3::ZERO {
            continue;
        }
        camera.rotation *= Quat::from_euler(EulerRot::XYZ, tilt.x, tilt.y, tilt.z);
        // Targeting continues using CameraLook's logical yaw/pitch. Mark the
        // rendered recoil so sync_external cannot feed it back into aim.
        look.presented(camera.rotation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quake_moves_only_the_rendered_camera_and_respects_view_motion() {
        use terra_world::volcano::{Eruption, Vent};
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<terra_network::client::ClientSession>()
            .init_resource::<terra_core::GameplayConfig>()
            .add_systems(Update, update);
        let camera = app
            .world_mut()
            .spawn((TerraCamera, Transform::default()))
            .id();
        app.world_mut()
            .get_mut::<CameraLook>(camera)
            .unwrap()
            .aim(Quat::IDENTITY);
        let mut session = app
            .world_mut()
            .resource_mut::<terra_network::client::ClientSession>();
        session.ready = true;
        session.eruption = Some((
            Eruption {
                vent: Vent {
                    origin: [0., 0., 0.],
                    seed: 1,
                },
                cycle: 3,
                age: 3.4,
                landed: 0,
            },
            std::time::Instant::now(),
        ));
        app.update();
        assert_ne!(
            app.world().get::<Transform>(camera).unwrap().rotation,
            Quat::IDENTITY
        );
        assert_eq!(
            app.world().get::<CameraLook>(camera).unwrap().rotation(),
            Quat::IDENTITY
        );
        app.world_mut()
            .resource_mut::<terra_core::GameplayConfig>()
            .view_bobbing = false;
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .rotation = Quat::IDENTITY;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().rotation,
            Quat::IDENTITY
        );
        app.world_mut()
            .resource_mut::<terra_core::GameplayConfig>()
            .view_bobbing = true;
        app.world_mut()
            .get_mut::<Transform>(camera)
            .unwrap()
            .translation = Vec3::X * 101.;
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().rotation,
            Quat::IDENTITY
        );
    }

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
