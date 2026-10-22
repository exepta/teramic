use crate::MaterialId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VoxelSample {
    pub density: f32,
    pub material: u16,
}
impl VoxelSample {
    pub const AIR: Self = Self {
        density: 1.0,
        material: MaterialId::Air as u16,
    };
    pub const fn new(density: f32, material: MaterialId) -> Self {
        Self {
            density,
            material: material as u16,
        }
    }
}
