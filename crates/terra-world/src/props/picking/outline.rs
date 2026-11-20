//! Once-only extraction of selection edges from the canonical indexed mesh.
//! Coplanar triangulation diagonals stay hidden, including concave plant faces.
use bevy_math::Vec3;
use std::collections::BTreeMap;

struct Edge {
    indices: [u32; 2],
    normal: Vec3,
    uses: usize,
    crease: bool,
}

pub(super) fn from_triangles(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[u32; 2]> {
    let mut edges: BTreeMap<[[u32; 3]; 2], Edge> = BTreeMap::new();
    let key =
        |index: u32| positions[index as usize].map(|v| if v == 0.0 { 0 } else { v.to_bits() });
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
            .map(|i| Vec3::from_array(positions[i as usize]));
        let normal = (b - a).cross(c - a).normalize_or_zero();
        if normal == Vec3::ZERO {
            continue;
        }
        for side in 0..3 {
            let indices = [triangle[side], triangle[(side + 1) % 3]];
            let mut key = indices.map(key);
            key.sort_unstable();
            edges
                .entry(key)
                .and_modify(|edge| {
                    edge.uses += 1;
                    edge.crease |= edge.normal.dot(normal) < 0.9999;
                })
                .or_insert(Edge {
                    indices,
                    normal,
                    uses: 1,
                    crease: false,
                });
        }
    }
    edges
        .into_values()
        .filter(|edge| edge.uses == 1 || edge.crease)
        .map(|edge| edge.indices)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_indexed_leaf_outline_has_no_triangulation_diagonals() {
        let positions = [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [0., 2., 0.],
        ];
        let indices = [0, 1, 3, 1, 2, 3, 0, 3, 5, 3, 4, 5];
        let edges = from_triangles(&positions, &indices);
        assert_eq!(edges.len(), 6);
        for i in 0..6 {
            assert!(
                edges
                    .iter()
                    .any(|&[a, b]| [a, b] == [i, (i + 1) % 6] || [b, a] == [i, (i + 1) % 6])
            );
        }
    }

    #[test]
    fn welded_crease_is_kept_but_duplicate_face_vertices_do_not_create_a_diagonal() {
        // Two indexed squares meet at a right angle; their seam is duplicated
        // in storage because the renderer needs different face normals.
        let positions = [
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [1., 0., 0.],
            [1., 0., 1.],
            [1., 1., 1.],
            [1., 1., 0.],
        ];
        let indices = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
        let edges = from_triangles(&positions, &indices);
        assert_eq!(edges.len(), 7);
        assert!(
            edges
                .iter()
                .any(|&[a, b]| positions[a as usize] == positions[1]
                    && positions[b as usize] == positions[2])
        );
    }
}
