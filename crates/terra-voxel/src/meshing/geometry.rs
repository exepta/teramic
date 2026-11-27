use thiserror::Error;
pub const PADDED_SIZE: usize = 18;
pub const PADDED_VOLUME: usize = PADDED_SIZE * PADDED_SIZE * PADDED_SIZE;
/// One read-only neighboring block around a 16^3 section, X fastest.
pub type PaddedBlocks = Vec<u16>;
/// Stable side indices match asset catalog face order: West, East, Down, Up, North, South.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockFace {
    pub minimum: [u32; 3],
    pub width: u32,
    pub height: u32,
    pub u_axis: usize,
    pub v_axis: usize,
    pub side: usize,
    pub material: u16,
    /// This visible face touches an occupied alpha-masked neighbor, not air.
    /// Keep its geometric joins canonical even though its pixels must render.
    pub occupied_neighbor: bool,
    /// Glass frame edges: U-, U+, V-, V+. Padding suppresses joined edges.
    pub edge_mask: u8,
}
#[derive(Debug, Default, Clone)]
pub struct BlockMesh {
    /// Local support profile for procedural plants and mineral segments.
    pub segments: Vec<BlockSegment>,
    /// Cached opaque occupancy including the one-cell halo (736 bytes, no ECS/GPU state).
    pub occluders: Vec<u64>,
    /// Sparse, clearance-tested vertical mineral clusters: cell, side, length.
    pub mineral_growth: Vec<([u8; 3], u8, f32)>,
    pub faces: Vec<BlockFace>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub materials: Vec<u16>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}
#[derive(Debug, Clone, Copy)]
pub struct BlockSegment {
    pub material: u16,
    pub cell: [u8; 3],
    pub below: u8,
    pub above: u8,
    pub hanging: bool,
    pub joined: bool,
    pub fan: [u8; 2],
}
impl BlockMesh {
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.indices.is_empty()
    }
}
#[derive(Debug, Error)]
pub enum MeshError {
    #[error("expected {PADDED_VOLUME} padded blocks, got {0}")]
    InvalidPadding(usize),
}
