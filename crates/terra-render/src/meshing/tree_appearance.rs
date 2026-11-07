use crate::appearance::GroundTint;
use terra_voxel::meshing::BlockFace;
use terra_world::generation::{TreeAppearance, TreeAppearanceSnapshot};

pub(super) struct SurfaceTint {
    pub caves: Option<
        Box<
            [(
                terra_world::generation::CaveStructure,
                terra_world::generation::CaveBiomeColumn,
            ); 256],
        >,
    >,
    pub ground: Option<GroundTint>,
    pub trees: Option<std::sync::Arc<TreeAppearanceSnapshot>>,
    pub origin: [i64; 3],
}

impl SurfaceTint {
    pub fn tree(&self, cell: [u32; 3], material: u16) -> Option<TreeAppearance> {
        self.trees.as_ref()?.at(
            std::array::from_fn(|axis| self.origin[axis] + i64::from(cell[axis])),
            material,
        )
    }

    pub fn tree_tint(&self, cell: [u32; 3], material: u16) -> [f32; 3] {
        match (material, self.tree(cell, material)) {
            (6, Some(TreeAppearance::Spruce)) => [0.54, 0.58, 0.55],
            (7, Some(TreeAppearance::Spruce)) => [0.34, 0.48, 1.18],
            (7, Some(TreeAppearance::Fir)) => [0.24, 0.34, 0.95],
            (6, Some(TreeAppearance::Palm)) => [1.16, 1.08, 0.88],
            _ => [1.0; 3],
        }
    }

    pub fn split(&self, face: BlockFace) -> Vec<BlockFace> {
        let grid: Vec<_> = (0..face.height)
            .flat_map(|v| {
                (0..face.width).map(move |u| {
                    let mut cell = face.minimum;
                    cell[face.u_axis] += u;
                    cell[face.v_axis] += v;
                    self.tree(cell, face.material)
                })
            })
            .collect();
        super::builder::condition_faces::partition(face, &grid)
    }
}
