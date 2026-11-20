//! Presentation only: the detached construction camera never moves the actor.
use bevy::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct ConstructionCamera {
    pub active: bool,
    pub center: Vec3,
    pub height: f32,
}

impl Default for ConstructionCamera {
    fn default() -> Self {
        Self {
            active: false,
            center: Vec3::ZERO,
            height: 38.0,
        }
    }
}

impl ConstructionCamera {
    pub fn pose(&self) -> Transform {
        Transform::from_translation(self.center + Vec3::new(0.0, self.height, self.height * 0.55))
            .looking_at(self.center, Vec3::Y)
    }
}
