mod capture;
mod components;
mod controller;
mod gait;
mod movement;
pub(crate) mod perspective;
#[cfg(test)]
mod tests;
pub use capture::{CameraInputState, CaptureRequest};
pub(crate) use capture::{capture_on_start, update_capture};
pub use components::FlightMotion;
pub use components::{CameraLook, CameraPerspective, PlayerIdentity, TerraCamera};
pub use controller::{
    ControllerMode, PlayerBody, PlayerControllerStatus, PlayerGrounded, PlayerInputBlock,
};
pub(crate) use controller::{
    WalkIntent, block_overlaps_body, follow_player_camera, read_walk_input, sync_controller,
    walk_step,
};
pub(crate) use movement::move_camera;
