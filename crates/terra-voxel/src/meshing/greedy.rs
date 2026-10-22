use super::{BlockFace, BlockMesh, MeshError, PADDED_VOLUME};
use crate::MaterialId;
use block_mesh::ndshape::{ConstShape, ConstShape3u32};
use block_mesh::{
    GreedyQuadsBuffer, MergeVoxel, RIGHT_HANDED_Y_UP_CONFIG, UnorientedQuad, Voxel,
    VoxelVisibility, greedy_quads,
};
type Shape = ConstShape3u32<18, 18, 18>;
#[derive(Clone, Copy, PartialEq, Eq)]
struct MeshVoxel(u16);
impl Voxel for MeshVoxel {
    fn get_visibility(&self) -> VoxelVisibility {
        match self.0 {
            0 | 27 => VoxelVisibility::Empty,
            // Alpha-masked leaf cubes do not hide solid faces behind their
            // cutouts. Adjacent leaf cubes still cull their shared interface.
            id if MaterialId::from_raw(id).is_some_and(|m| !m.is_opaque()) => {
                VoxelVisibility::Translucent
            }
            _ => VoxelVisibility::Opaque,
        }
    }
}
impl MergeVoxel for MeshVoxel {
    type MergeValue = u16;
    fn merge_value(&self) -> u16 {
        self.0
    }
}
/// Greedy meshing merges only coplanar faces of the same material.
/// Neighbor padding hides internal faces even before the adjacent section is loaded.
pub fn mesh_section(blocks: &[u16]) -> Result<BlockMesh, MeshError> {
    if blocks.len() != PADDED_VOLUME {
        return Err(MeshError::InvalidPadding(blocks.len()));
    }
    let voxels: Vec<_> = blocks.iter().copied().map(MeshVoxel).collect();
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
    let count = buffer.quads.num_quads();
    let has_masked_neighbors = blocks
        .iter()
        .any(|&id| id != 0 && MaterialId::from_raw(id).is_some_and(|m| !m.is_opaque()));
    let mut mesh = BlockMesh {
        segments: Vec::new(),
        occluders: if count == 0 {
            Vec::new()
        } else {
            let mut bits = vec![0_u64; PADDED_VOLUME.div_ceil(64)];
            for (i, &id) in blocks.iter().enumerate() {
                if MaterialId::from_raw(id).is_some_and(MaterialId::is_opaque) {
                    bits[i / 64] |= 1 << (i % 64);
                }
            }
            bits
        },
        mineral_growth: Vec::new(),
        faces: Vec::with_capacity(count),
        positions: Vec::with_capacity(count * 4),
        normals: Vec::with_capacity(count * 4),
        materials: Vec::with_capacity(count * 4),
        uvs: Vec::with_capacity(count * 4),
        indices: Vec::with_capacity(count * 6),
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
                let start = mesh.positions.len() as u32;
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
            } else if has_masked_neighbors
                && MaterialId::from_raw(material).is_none_or(MaterialId::is_opaque)
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

/// Split only mixed air/leaf interfaces. The flag fits existing BlockFace
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
