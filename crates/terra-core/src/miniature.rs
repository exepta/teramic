//! Shared opt-in visibility for the live-world miniature camera.
use bevy::prelude::*;

pub const WORLD_LAYER: usize = 28;
pub const CREATURE_LAYER: usize = 27;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct MiniatureView {
    pub active: bool,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniatureMesh {
    Terrain,
    Water,
    Prop,
    Creature,
    Player,
    Drop,
}
