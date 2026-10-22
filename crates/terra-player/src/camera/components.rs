use bevy::prelude::*;
use terra_core::PlayerId;
#[derive(Component, Debug)]
pub struct PlayerIdentity(pub PlayerId);
#[derive(Component)]
#[require(FlightMotion, CameraLook, super::gait::WalkCameraMotion)]
pub struct TerraCamera;
#[derive(Component, Debug, Default)]
pub struct FlightMotion {
    pub(crate) velocity: Vec3,
}
/// Player aim is independent from the camera looking back at the player's face.
#[derive(Component, Debug)]
pub struct CameraLook {
    rotation: Quat,
    presented: Quat,
    initialized: bool,
}
impl Default for CameraLook {
    fn default() -> Self {
        Self {
            rotation: Quat::IDENTITY,
            presented: Quat::IDENTITY,
            initialized: false,
        }
    }
}
impl CameraLook {
    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    pub(crate) fn sync_external(&mut self, rotation: Quat) {
        if !self.initialized || !self.presented.abs_diff_eq(rotation, 0.00001) {
            self.rotation = rotation;
            self.presented = rotation;
            self.initialized = true;
        }
    }

    pub(crate) fn aim(&mut self, rotation: Quat) {
        self.rotation = rotation;
        self.presented = rotation;
        self.initialized = true;
    }

    pub(crate) fn presented(&mut self, rotation: Quat) {
        self.presented = rotation;
    }
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CameraPerspective {
    #[default]
    FirstPerson,
    ThirdPerson,
    ThirdPersonFront,
}
impl CameraPerspective {
    pub fn is_third_person(self) -> bool {
        self != Self::FirstPerson
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::FirstPerson => Self::ThirdPerson,
            Self::ThirdPerson => Self::ThirdPersonFront,
            Self::ThirdPersonFront => Self::FirstPerson,
        }
    }
}
