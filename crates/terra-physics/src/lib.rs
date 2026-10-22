//! Shared Avian configuration.
pub mod character;
pub mod current;
mod plugin;
mod profile;
mod terrain;
#[cfg(test)]
mod tests;
pub use plugin::TerraPhysicsPlugin;
pub use profile::CharacterProfile;
pub use terrain::{TerrainCollider, TerrainCollisionState};
