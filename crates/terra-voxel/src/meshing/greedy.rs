use super::{BlockFace, BlockMesh, MeshError, PADDED_VOLUME};
use crate::MaterialId;
use block_mesh::ndshape::{ConstShape, ConstShape3u32};
use block_mesh::{
    GreedyQuadsBuffer, MergeVoxel, RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad, Voxel,
    VoxelVisibility, greedy_quads,
};
type Shape = ConstShape3u32<18, 18, 18>;
#[derive(Clone, Copy, PartialEq, Eq)]
struct MeshVoxel {
    id: u16,
    visibility: VoxelVisibility,
}

// Face culling treats unknown IDs as solid; ambient occlusion only uses
// registered materials. Share the catalog lookup between both hot loops.
const MATERIAL_COUNT: usize = MaterialId::LAST as usize + 1;
static OCCLUSION: [Option<bool>; MATERIAL_COUNT] = {
    let mut table = [None; MATERIAL_COUNT];
    let mut i = 0;
    while i < table.len() {
        if let Some(material) = MaterialId::from_raw(i as u16) {
            table[i] = Some(!matches!(material, MaterialId::Lava) && material.is_opaque());
        }
        i += 1;
    }
    table
};

fn known_occlusion(id: u16) -> Option<bool> {
    OCCLUSION.get(id as usize).copied().flatten()
}

/// Perforated foliage and the sloping lava surface can expose a touching solid.
pub fn occludes_face(id: u16) -> bool {
    known_occlusion(id).unwrap_or(true)
}

impl Voxel for MeshVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        self.visibility
    }
}
impl MergeVoxel for MeshVoxel {
    type MergeValue = u16;
    fn merge_value(&self) -> u16 {
        self.id
    }
}
/// Greedy meshing merges only coplanar faces of the same material.
/// Neighbor padding hides internal faces even before the adjacent section is loaded.
pub fn mesh_section(blocks: &[u16]) -> Result<BlockMesh, MeshError> {
    build_section_mesh::<true>(blocks)
}

/// Canonical greedy faces for material-aware renderers that build their own vertices.
pub fn mesh_section_faces(blocks: &[u16]) -> Result<BlockMesh, MeshError> {
    build_section_mesh::<false>(blocks)
}

fn build_section_mesh<const EXPAND: bool>(blocks: &[u16]) -> Result<BlockMesh, MeshError> {
    if blocks.len() != PADDED_VOLUME {
        return Err(MeshError::InvalidPadding(blocks.len()));
    }
    let mut has_leaves = false;
    let mut has_other_cutouts = false;
    let mut has_water = false;
    let voxels: Vec<_> = blocks
        .iter()
        .copied()
        .map(|id| {
            let visibility = match id {
                0 => VoxelVisibility::Empty,
                27 => {
                    has_water = true;
                    VoxelVisibility::Empty
                }
                id if occludes_face(id) => VoxelVisibility::Opaque,
                id => {
                    if MaterialId::FOLIAGE.contains(&id) {
                        has_leaves = true;
                    } else {
                        has_other_cutouts = true;
                    }
                    VoxelVisibility::Translucent
                }
            };
            MeshVoxel { id, visibility }
        })
        .collect();
    let config = RIGHT_HANDED_Y_UP_CONFIG;
    let mut buffer = GreedyQuadsBuffer::new(PADDED_VOLUME);
    greedy_quads(
        &voxels,
        &Shape {},
        [0; 3],
        [17; 3],
        &config.faces,
        &mut buffer,
    );
    if has_leaves && has_other_cutouts {
        // The greedy library drops both faces between any two translucent
        // cells. Retain the canopy boundary behind panes and partial models.
        // Touching leaves across a section boundary share the same crown.
        for (i, voxel) in voxels.iter().enumerate() {
            if voxel.visibility != VoxelVisibility::Translucent
                || MaterialId::FOLIAGE.contains(&voxel.id)
            {
                continue;
            }
            let cell = Shape::delinearize(i as u32);
            for (group, face) in buffer.quads.groups.iter_mut().zip(config.faces) {
                let normal = face.signed_normal();
                let leaf: [i32; 3] = std::array::from_fn(|axis| cell[axis] as i32 - normal[axis]);
                if !leaf.iter().all(|n| (1..=16).contains(n)) {
                    continue;
                }
                let leaf = leaf.map(|n| n as u32);
                if MaterialId::FOLIAGE.contains(&blocks[Shape::linearize(leaf) as usize]) {
                    group.push(UnorientedQuad {
                        minimum: leaf,
                        width: 1,
                        height: 1,
                    });
                }
            }
        }
    }
    let count = buffer.quads.num_quads();
    let expanded = if EXPAND { count } else { 0 };
    let has_masked_neighbors = has_leaves || has_other_cutouts || has_water;
    let mut mesh = BlockMesh {
        segments: Vec::new(),
        occluders: if count == 0 {
            Vec::new()
        } else {
            let mut bits = vec![0_u64; PADDED_VOLUME.div_ceil(64)];
            for (i, &id) in blocks.iter().enumerate() {
                if known_occlusion(id) == Some(true) {
                    bits[i / 64] |= 1 << (i % 64);
                }
            }
            bits
        },
        mineral_growth: Vec::new(),
        faces: Vec::with_capacity(count),
        positions: Vec::with_capacity(expanded * 4),
        normals: Vec::with_capacity(expanded * 4),
        materials: Vec::with_capacity(expanded * 4),
        uvs: Vec::with_capacity(expanded * 4),
        indices: Vec::with_capacity(expanded * 6),
    };
    for (group, face) in buffer.quads.groups.iter().zip(config.faces.iter()) {
        for quad in group {
            let material = blocks[Shape::linearize(quad.minimum) as usize];
            let corners = face.quad_mesh_positions(quad, 1.0);
            let normal = face.quad_mesh_normals()[0];
            let n_axis = normal.iter().position(|v| v.abs() > 0.5).unwrap_or(0);
            let u_axis = (0..3)
                .find(|&i| (corners[1][i] - corners[0][i]).abs() > 0.5)
                .unwrap_or(0);
            let v_axis = (0..3)
                .find(|&i| (corners[2][i] - corners[0][i]).abs() > 0.5)
                .unwrap_or(0);
            let side = n_axis * 2 + usize::from(normal[n_axis] > 0.0);
            let emit = |quad: UnorientedQuad, occupied_neighbor| {
                mesh.faces.push(BlockFace {
                    minimum: quad.minimum.map(|v| v - 1),
                    width: quad.width,
                    height: quad.height,
                    u_axis,
                    v_axis,
                    side,
                    material,
                    occupied_neighbor,
                    edge_mask: if material == MaterialId::Glass as u16 {
                        glass_edges(blocks, quad.minimum, u_axis, v_axis)
                    } else {
                        15
                    },
                });
                if !EXPAND {
                    return;
                }
                let start = mesh.positions.len() as u32;
                mesh.positions.extend(
                    face.quad_mesh_positions(&quad, 1.0)
                        .map(|p| p.map(|v| v - 1.0)),
                );
                mesh.normals.extend(face.quad_mesh_normals());
                mesh.materials.extend([material; 4]);
                mesh.uvs
                    .extend(face.tex_coords(config.u_flip_face, true, &quad));
                mesh.indices.extend(face.quad_mesh_indices(start));
            };
            if material == MaterialId::Glass as u16 {
                let mut emit = emit;
                for v in 0..quad.height {
                    for u in 0..quad.width {
                        let mut cell = *quad;
                        cell.minimum[u_axis] += u;
                        cell.minimum[v_axis] += v;
                        cell.width = 1;
                        cell.height = 1;
                        emit(cell, false);
                    }
                }
            } else if has_masked_neighbors && occludes_face(material)
                || (has_other_cutouts || has_water) && MaterialId::FOLIAGE.contains(&material)
            {
                split_contacts(blocks, *quad, u_axis, v_axis, side, emit);
            } else {
                let mut emit = emit;
                emit(*quad, false);
            }
        }
    }
    Ok(mesh)
}

fn glass_edges(blocks: &[u16], position: [u32; 3], u_axis: usize, v_axis: usize) -> u8 {
    let mut edges = 0;
    for (bit, axis, positive) in [
        (1, u_axis, false),
        (2, u_axis, true),
        (4, v_axis, false),
        (8, v_axis, true),
    ] {
        let mut adjacent = position;
        if positive {
            adjacent[axis] += 1;
        } else {
            adjacent[axis] -= 1;
        }
        if blocks[Shape::linearize(adjacent) as usize] != MaterialId::Glass as u16 {
            edges |= bit;
        }
    }
    edges
}

/// Split only mixed air/translucent interfaces. The flag fits existing BlockFace
/// padding; normal terrain keeps both its original quads and its cache size.
fn split_contacts(
    blocks: &[u16],
    quad: UnorientedQuad,
    u_axis: usize,
    v_axis: usize,
    side: usize,
    mut emit: impl FnMut(UnorientedQuad, bool),
) {
    let width = quad.width as usize;
    let height = quad.height as usize;
    let mut occupied = [false; 16 * 16];
    for v in 0..height {
        for u in 0..width {
            let mut p = quad.minimum;
            p[u_axis] += u as u32;
            p[v_axis] += v as u32;
            if side.is_multiple_of(2) {
                p[side / 2] -= 1;
            } else {
                p[side / 2] += 1;
            }
            occupied[u + v * width] = blocks[Shape::linearize(p) as usize] != 0;
        }
    }
    if occupied[..width * height].iter().all(|v| *v == occupied[0]) {
        emit(quad, occupied[0]);
        return;
    }
    let mut visited = [false; 16 * 16];
    for v in 0..height {
        for u in 0..width {
            if visited[u + v * width] {
                continue;
            }
            let contact = occupied[u + v * width];
            let same =
                |x: usize, y: usize| !visited[x + y * width] && occupied[x + y * width] == contact;
            let mut w = 1;
            while u + w < width && same(u + w, v) {
                w += 1;
            }
            let mut h = 1;
            while v + h < height && (u..u + w).all(|x| same(x, v + h)) {
                h += 1;
            }
            for y in v..v + h {
                visited[u + y * width..u + w + y * width].fill(true);
            }
            let mut part = quad;
            part.minimum[u_axis] += u as u32;
            part.minimum[v_axis] += v as u32;
            part.width = w as u32;
            part.height = h as u32;
            emit(part, contact);
        }
    }
}
