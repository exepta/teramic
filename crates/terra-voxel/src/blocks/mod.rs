mod coordinates;
mod raycast;
mod section;
#[cfg(test)]
mod tests;
pub use coordinates::{ColumnPos, SectionPos, block_index, global_to_section};
pub use raycast::{BlockHit, raycast_blocks, raycast_blocks_with};
pub use section::{BlockError, BlockSection};

pub const SECTION_SIZE: usize = 16;
pub const SECTION_VOLUME: usize = SECTION_SIZE * SECTION_SIZE * SECTION_SIZE;
