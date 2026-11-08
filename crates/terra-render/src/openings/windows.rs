use super::*;
use std::collections::BTreeMap;
use terra_world::objects::windows::{Group, Piece};
#[cfg(test)]
mod tests;

#[derive(Default)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
impl Geometry {
    fn append(&mut self, piece: Piece, origin: Vec3) {
        for axis in 0..3 {
            for sign in [-1.0_f32, 1.0] {
                let u = (axis + 1) % 3;
                let v = (axis + 2) % 3;
                let base = self.positions.len() as u32;
                for [a, b] in [[0, 0], [1, 0], [1, 1], [0, 1]] {
                    let mut p = piece.bounds.minimum.map(|x| x as f32);
                    p[axis] = if sign > 0.0 {
                        piece.bounds.maximum[axis] as f32
                    } else {
                        piece.bounds.minimum[axis] as f32
                    };
                    p[u] = if a == 1 {
                        piece.bounds.maximum[u] as f32
                    } else {
                        piece.bounds.minimum[u] as f32
                    };
                    p[v] = if b == 1 {
                        piece.bounds.maximum[v] as f32
                    } else {
                        piece.bounds.minimum[v] as f32
                    };
                    self.positions
                        .push((Vec3::from_array(p) - origin).to_array());
                    let mut normal = [0.0; 3];
                    normal[axis] = sign;
                    self.normals.push(normal);
                    self.uvs.push([a as f32, b as f32]);
                }
                for i in if sign > 0.0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                } {
                    self.indices.push(base + i);
                }
            }
        }
    }
}

pub(super) fn parts(
    id: u16,
    size: [u8; 2],
    assets: &BlockAssets,
    meshes: &mut Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
) -> Option<Vec<Part>> {
    let palette = super::palette::material_palette(id, assets, materials)?;
    let group = Group {
        anchor: [0; 3],
        material: id,
        facing: 0,
        width: size[0],
        height: size[1],
    };
    let mut geometry: BTreeMap<(Option<u8>, u8), Geometry> = BTreeMap::new();
    let pieces = if terra_world::objects::is_door(id) {
        terra_world::objects::doors::pieces()
    } else {
        terra_world::objects::windows::pieces(size[0], size[1])
    };
    for piece in pieces {
        let origin = piece.leaf.map_or(CENTER, |leaf| {
            Vec3::from_array(group.hinge(leaf).map(|v| v as f32))
        });
        geometry
            .entry((piece.leaf, piece.surface))
            .or_default()
            .append(piece, origin);
    }
    let mut parts = Vec::with_capacity(geometry.len());
    for ((leaf, surface), g) in geometry {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, g.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, g.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, g.uvs)
        .with_inserted_indices(Indices::U32(g.indices));
        parts.push(Part {
            mesh: meshes.add(mesh),
            material: assets.materials.get(palette[usize::from(surface)])?.clone(),
            moving: leaf.is_some(),
            glass: surface == 4,
            leaf: leaf.unwrap_or(0),
        });
    }
    Some(parts)
}
