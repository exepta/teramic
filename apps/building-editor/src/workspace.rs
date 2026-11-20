//! Editor-only support geometry, deliberately separate from exported model data.
use bevy::prelude::Resource;

pub const PLATFORM_Y: i64 = -3;
pub const PLATFORM_MATERIAL: u16 = terra_voxel::MaterialId::StructureStone as u16;
pub const PLATFORM_SIZES: [u16; 4] = [9, 17, 25, 33];

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspacePlatform {
    size: u16,
}

impl Default for WorkspacePlatform {
    fn default() -> Self {
        Self { size: 9 }
    }
}

impl WorkspacePlatform {
    pub fn new(size: u16) -> Result<Self, String> {
        if !PLATFORM_SIZES.contains(&size) {
            return Err("Choose 9, 17, 25 or 33 / 9, 17, 25 oder 33 wählen".into());
        }
        Ok(Self { size })
    }

    pub fn size(&self) -> u16 {
        self.size
    }

    pub fn half_size(&self) -> i64 {
        i64::from(self.size / 2)
    }

    pub fn view_distance(&self) -> u8 {
        // Keep the enlarged surface loaded around the local authoring camera.
        (self.size.div_ceil(16) + 1) as u8
    }
}
