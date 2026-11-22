//! Project the shared world-space trajectory back into the spider's animated
//! local frame, including wall and ceiling attachment and juvenile scale.
use bevy::prelude::*;
use terra_core::creatures::{
    Creature,
    spiders::{WEB_CAST_TIME, WEB_WINDUP, WebShot},
};

#[derive(Component, Default)]
pub(super) struct Presentation(pub Option<Transform>);

impl Presentation {
    pub fn sample(animal: &Creature, root: &Transform, since: f32) -> Self {
        let Some(shot) = WebShot::of(animal) else {
            return Self(None);
        };
        let elapsed = (WEB_CAST_TIME - animal.timer + since).max(0.);
        let flight = elapsed - WEB_WINDUP;
        if flight > shot.flight_time {
            return Self(None);
        }
        let p = shot.at(flight).as_vec3();
        let direction = (shot.target - shot.origin).as_vec3().normalize_or_zero();
        let radius = if flight < 0. {
            0.13 + 0.26 * elapsed / WEB_WINDUP
        } else {
            0.39 + 0.71 * (flight * 7.).min(1.)
        };
        let mut pose = Transform::from_translation(
            root.rotation.inverse() * (p - root.translation) / root.scale,
        );
        pose.rotation = root.rotation.inverse() * Quat::from_rotation_arc(Vec3::NEG_Z, direction);
        pose.scale = Vec3::splat(radius) / root.scale;
        Self(Some(pose))
    }
}
