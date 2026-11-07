use super::definitions::BlockRenderKind;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct PrimitiveTemplate {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Empty unless the authored mesh provides a tangent frame for every vertex.
    pub tangents: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: usize,
}

pub(crate) fn tangent_frame(
    direction: bevy::math::Vec3,
    normal: bevy::math::Vec3,
    handedness: f32,
) -> [f32; 4] {
    let normal = normal.normalize_or_zero();
    let mut tangent = (direction - normal * direction.dot(normal)).normalize_or_zero();
    if tangent.length_squared() < 0.5 {
        let axis = if normal.x.abs() < 0.9 {
            bevy::math::Vec3::X
        } else {
            bevy::math::Vec3::Y
        };
        tangent = (axis - normal * axis.dot(normal)).normalize_or_zero();
    }
    [
        tangent.x,
        tangent.y,
        tangent.z,
        if handedness < 0.0 { -1.0 } else { 1.0 },
    ]
}

pub(crate) fn projected_tangent(normal: bevy::math::Vec3, side: usize) -> [f32; 4] {
    use bevy::math::Vec3;
    let (u, v) = match side / 2 {
        0 => (Vec3::Z, Vec3::Y),
        1 => (Vec3::X, Vec3::Z),
        _ => (Vec3::X, Vec3::Y),
    };
    tangent_frame(u, normal, normal.cross(u).dot(v))
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
    pub crop: usize,
    pub foliage: [usize; 3],
    pub stem: usize,
    pub dry_grass: [usize; 3],
    pub twig: usize,
    pub flowers: [usize; 8],
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
    pub animated_chests: bool,
    pub block_details: terra_core::BlockDetailConfig,
    pub blocks: HashMap<u16, BlockTemplate>,
    /// Tileable PBR cube fallback for authored face-group terrain, shared by every section.
    pub cube_lods: HashMap<u16, BlockTemplate>,
    /// Opaque authored cubes validated by the loader; only these can replace detail shadows.
    pub opaque_cube_lods: std::collections::HashSet<u16>,
    pub frame_materials: HashMap<u16, usize>,
    pub ore_materials: HashMap<u16, usize>,
    pub prop_materials: PropMaterials,
    pub item_materials: ItemMaterials,
    pub torch_palette: HashMap<usize, [f32; 4]>,
    /// The lantern has three complete authored poses, selected before batching.
    pub lantern_variants: HashMap<usize, u8>,
    pub wood_materials: std::collections::HashSet<usize>,
    /// Authored moving door/window leaf primitives; their fixed frame stays put.
    pub hinge_materials: std::collections::HashSet<usize>,
    /// Rigid crusher parts: left roller, right roller and side drive cog.
    pub crusher_moving_materials: HashMap<usize, u8>,
    pub conveyor_ribbon_materials: std::collections::HashSet<usize>,
    pub logistics_arrow_materials: std::collections::HashSet<usize>,
    pub logistics_support_materials: std::collections::HashSet<usize>,
    pub campfire_support_material: Option<usize>,
    pub blanket_materials: std::collections::HashSet<usize>,
    /// Semantic slots retain one shared PBR material; realm colors are per cell.
    pub castle_cloth_materials: std::collections::HashSet<usize>,
    pub castle_icon_materials: HashMap<usize, terra_core::kingdom::KingdomIcon>,
    pub castle_arch_join_materials: std::collections::HashSet<usize>,
    pub castle_decor_materials: std::collections::HashSet<usize>,
}

impl TemplateSet {
    pub(crate) fn register_castle_material(&mut self, name: &str, slot: usize) {
        match name {
            "heraldry_cloth" => {
                self.castle_cloth_materials.insert(slot);
            }
            "arch_join" => {
                self.castle_arch_join_materials.insert(slot);
            }
            "crack" | "ivy" | "leaf" | "leaf_light" | "moss" => {
                self.castle_decor_materials.insert(slot);
            }
            _ => {
                if let Some(icon) = name
                    .strip_prefix("heraldry_icon_")
                    .and_then(terra_core::kingdom::KingdomIcon::from_key)
                {
                    self.castle_icon_materials.insert(slot, icon);
                }
            }
        }
    }

    pub(crate) fn untinted_wool_material(&self, slot: usize) -> bool {
        [103, 104, 105].into_iter().any(|id| {
            self.blocks
                .get(&id)
                .is_some_and(|block| block.face_materials.contains(&slot))
        })
    }

    pub(crate) fn authored_terrain_material(&self, slot: usize) -> bool {
        self.cube_lods.iter().any(|(id, coarse)| {
            coarse.face_materials.contains(&slot)
                || self
                    .blocks
                    .get(id)
                    .is_some_and(|block| block.face_materials.contains(&slot))
        })
    }

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
