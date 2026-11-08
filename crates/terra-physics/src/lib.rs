//! Shared Avian configuration.
pub mod character;
pub mod current;
mod plugin;
mod profile;
mod terrain;
#[cfg(test)]
mod tests;
mod trains;
pub use plugin::TerraPhysicsPlugin;
pub use profile::CharacterProfile;
pub use terrain::{TerrainCollider, TerrainCollisionState};
pub use trains::{TrainCollider, TrainCollisionViews};
