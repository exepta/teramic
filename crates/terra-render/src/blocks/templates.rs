use super::definitions::BlockRenderKind;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct PrimitiveTemplate {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DetailTemplate {
    pub primitives: Vec<PrimitiveTemplate>,
    pub probability: f32,
    pub water_clearance: Option<[f32; 2]>,
    pub density_field: Option<super::definitions::DetailDensity>,
}

#[derive(Clone, Debug)]
pub(crate) struct BlockTemplate {
    pub surface: Option<super::surface::SurfaceTemplate>,
    pub render_kind: BlockRenderKind,
    pub bevel_width: f32,
    pub faces: [Vec<PrimitiveTemplate>; 6],
    /// Explicit dominant material, independent of imported primitive ordering.
    pub face_materials: [usize; 6],
    pub side_cap: Option<SideCap>,
    pub details: Vec<DetailTemplate>,
    /// Whole static scene, emitted once per occupied cell without voxel-face culling.
    pub model: Vec<PrimitiveTemplate>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SideCap {
    pub material: usize,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PropMaterials {
    pub foliage: [usize; 3],
    pub stem: usize,
    pub dry_grass: [usize; 3],
    pub twig: usize,
    pub flowers: [usize; 6],
    pub mushroom: usize,
    pub stone: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ItemMaterials {
    pub opaque: usize,
    pub glass: usize,
    pub metal: usize,
    pub torch: usize,
}

#[derive(Default, Clone)]
pub(crate) struct TemplateSet {
    pub block_details: terra_core::BlockDetailConfig,
    pub blocks: HashMap<u16, BlockTemplate>,
    pub frame_materials: HashMap<u16, usize>,
    pub ore_materials: HashMap<u16, usize>,
    pub prop_materials: PropMaterials,
    pub item_materials: ItemMaterials,
    pub torch_palette: HashMap<usize, [f32; 4]>,
    pub wood_materials: std::collections::HashSet<usize>,
    /// Authored moving door/window leaf primitives; their fixed frame stays put.
    pub hinge_materials: std::collections::HashSet<usize>,
    pub blanket_materials: std::collections::HashSet<usize>,
}

impl TemplateSet {
    pub(crate) fn torch_material(&self, slot: usize) -> bool {
        self.blocks
            .get(&40)
            .is_some_and(|b| b.model.iter().any(|p| p.material == slot))
    }
    pub(crate) fn water_material(&self, slot: usize) -> bool {
        [14, 27].into_iter().any(|id| {
            self.blocks
                .get(&id)
                .is_some_and(|block| block.face_materials.contains(&slot))
        })
    }

    pub(crate) fn glass_material(&self, slot: usize) -> bool {
        self.blocks
            .get(&15)
            .is_some_and(|block| block.face_materials.contains(&slot))
            || self.frame_materials.get(&15) == Some(&slot)
    }
}
