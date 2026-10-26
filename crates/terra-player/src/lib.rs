//! Collision-aware walking, debug flight and discrete world interaction.
pub mod camera;
pub mod interaction;
mod plugin;
pub use camera::{
    CameraInputState, CameraLook, CameraPerspective, CaptureRequest, ControllerMode, PlayerBody,
    PlayerControllerStatus, PlayerGrounded, PlayerIdentity, PlayerInputBlock, TerraCamera,
};
pub use interaction::{TargetBlock, TargetCreature, TargetWorld, WorldTarget};
pub use plugin::{PlayerSet, TerraPlayerPlugin};
pub use terra_physics::character::CharacterPresentation;
pub use terra_physics::character::{PLAYER_EYE_OFFSET, PLAYER_HALF_HEIGHT, PLAYER_RADIUS};
