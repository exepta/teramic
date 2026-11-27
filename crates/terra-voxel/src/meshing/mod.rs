mod geometry;
mod greedy;
#[cfg(test)]
mod tests;
pub use geometry::{
    BlockFace, BlockMesh, BlockSegment, MeshError, PADDED_SIZE, PADDED_VOLUME, PaddedBlocks,
};
pub use greedy::{mesh_section, mesh_section_faces, occludes_face};
