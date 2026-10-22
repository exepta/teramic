use super::{BlockError, SECTION_SIZE};
use serde::{Deserialize, Serialize};

/// Horizontal 16 x 16 chunk column. Y is represented by stacked sections.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnPos {
    pub x: i64,
    pub z: i64,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionPos {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}
impl SectionPos {
    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }
    pub const fn column(self) -> ColumnPos {
        ColumnPos {
            x: self.x,
            z: self.z,
        }
    }
    pub fn offset(self, delta: [i64; 3]) -> Result<Self, BlockError> {
        let axis = [self.x, self.y, self.z];
        let mut p = [0; 3];
        for i in 0..3 {
            p[i] = axis[i]
                .checked_add(delta[i])
                .ok_or(BlockError::CoordinateOverflow)?;
        }
        Ok(Self::new(p[0], p[1], p[2]))
    }
    pub fn origin(self) -> Result<[i64; 3], BlockError> {
        let mut p = [0; 3];
        for (axis, value) in [self.x, self.y, self.z].into_iter().enumerate() {
            p[axis] = value
                .checked_mul(SECTION_SIZE as i64)
                .ok_or(BlockError::CoordinateOverflow)?;
        }
        Ok(p)
    }
    pub fn global_block(self, local: [usize; 3]) -> Result<[i64; 3], BlockError> {
        block_index(local)?;
        let origin = self.origin()?;
        let mut p = origin;
        for i in 0..3 {
            p[i] = origin[i]
                .checked_add(local[i] as i64)
                .ok_or(BlockError::CoordinateOverflow)?;
        }
        Ok(p)
    }
}
pub fn global_to_section(global: [i64; 3]) -> (SectionPos, [usize; 3]) {
    let p = global.map(|v| v.div_euclid(SECTION_SIZE as i64));
    let local = global.map(|v| v.rem_euclid(SECTION_SIZE as i64) as usize);
    (SectionPos::new(p[0], p[1], p[2]), local)
}
pub fn block_index(local: [usize; 3]) -> Result<usize, BlockError> {
    if local.iter().any(|&v| v >= SECTION_SIZE) {
        return Err(BlockError::OutOfBounds(local));
    }
    Ok(local[0] + SECTION_SIZE * (local[1] + SECTION_SIZE * local[2]))
}
