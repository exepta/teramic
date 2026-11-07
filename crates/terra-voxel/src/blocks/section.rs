use super::{SECTION_VOLUME, SectionPos, block_index};
use crate::MaterialId;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlockError {
    #[error("block coordinate {0:?} outside 0..16")]
    OutOfBounds([usize; 3]),
    #[error("expected {SECTION_VOLUME} blocks, got {0}")]
    InvalidBlockCount(usize),
    #[error("block coordinate exceeds i64 range")]
    CoordinateOverflow,
    #[error("block revision exhausted")]
    RevisionOverflow,
    #[error("block is bound to an active structure; dissolve it before removal")]
    ProtectedStructure,
}
/// Each block has exactly one owner. Neighbor data is read-only meshing padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSection {
    position: SectionPos,
    revision: u64,
    blocks: Vec<u16>,
    solid_count: usize,
}
impl BlockSection {
    pub fn empty(position: SectionPos) -> Result<Self, BlockError> {
        Self::from_blocks(position, 0, vec![MaterialId::Air as u16; SECTION_VOLUME])
    }
    pub fn from_blocks(
        position: SectionPos,
        revision: u64,
        blocks: Vec<u16>,
    ) -> Result<Self, BlockError> {
        if blocks.len() != SECTION_VOLUME {
            return Err(BlockError::InvalidBlockCount(blocks.len()));
        }
        position.global_block([15; 3])?;
        let solid_count = blocks
            .iter()
            .filter(|&&b| b != MaterialId::Air as u16)
            .count();
        Ok(Self {
            position,
            revision,
            blocks,
            solid_count,
        })
    }
    pub fn position(&self) -> SectionPos {
        self.position
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn blocks(&self) -> &[u16] {
        &self.blocks
    }
    pub fn is_empty(&self) -> bool {
        self.solid_count == 0
    }
    pub fn block(&self, local: [usize; 3]) -> Result<u16, BlockError> {
        Ok(self.blocks[block_index(local)?])
    }
    pub fn set_block(&mut self, local: [usize; 3], material: u16) -> Result<bool, BlockError> {
        let index = block_index(local)?;
        let old = self.blocks[index];
        if old == material {
            return Ok(false);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(BlockError::RevisionOverflow)?;
        self.solid_count -= usize::from(old != MaterialId::Air as u16);
        self.solid_count += usize::from(material != MaterialId::Air as u16);
        self.blocks[index] = material;
        self.revision = revision;
        Ok(true)
    }
}
