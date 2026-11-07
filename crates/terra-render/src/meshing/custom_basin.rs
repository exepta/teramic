//! Disjoint shell cells give each basin wall/rim face exactly one owner.
use super::*;
use crate::blocks::{PrimitiveTemplate, tangent_frame};
use bevy::math::Vec3;

fn material_at(mask: u8, p: [f32; 3], slots: [usize; 3]) -> Option<usize> {
    let floor = mask & 4 == 0 && p[1] < 0.13;
    let wall = (mask & 1 == 0 && p[0] < 0.16)
        || (mask & 2 == 0 && p[0] > 0.84)
        || (mask & 16 == 0 && p[2] < 0.16)
        || (mask & 32 == 0 && p[2] > 0.84);
    let stacked = mask & 8 != 0;
    if floor {
        return Some(slots[0]);
    }
    if !wall || p[1] >= if stacked { 1.0 } else { 0.95 } {
        return None;
    }
    Some(if !stacked && p[1] >= 0.85 {
        slots[1]
    } else if (0.18..0.25).contains(&p[1]) || (0.685..0.755).contains(&p[1]) {
        slots[2]
    } else {
        slots[0]
    })
}

pub(super) fn profile(
    model: &ModelCell,
    authored: &[PrimitiveTemplate],
    standalone: bool,
) -> Vec<PrimitiveTemplate> {
    let wood = authored
        .iter()
        .find(|p| p.positions.iter().all(|v| v[1] <= 0.131))
        .map_or(0, |p| p.material);
    let stone = authored
        .iter()
        .find(|p| p.positions.iter().all(|v| v[1] >= 0.849 && v[1] <= 0.951))
        .map_or(wood, |p| p.material);
    let iron = authored
        .iter()
        .find(|p| p.material != wood && p.material != stone)
        .map_or(wood, |p| p.material);
    let slots = [wood, stone, iron];
    let mask = if standalone { 0 } else { model.state.facing };
    let xz = [0., 0.16, 0.84, 1.];
    let ys = [0., 0.13, 0.18, 0.25, 0.685, 0.755, 0.85, 0.95, 1.];
    let mut parts = std::collections::BTreeMap::<usize, PrimitiveTemplate>::new();
    for x in 0..3 {
        for y in 0..8 {
            for z in 0..3 {
                let low = [xz[x], ys[y], xz[z]];
                let high = [xz[x + 1], ys[y + 1], xz[z + 1]];
                let center: [f32; 3] = std::array::from_fn(|i| (low[i] + high[i]) * 0.5);
                let Some(material) = material_at(mask, center, slots) else {
                    continue;
                };
                for side in 0..6 {
                    let axis = side / 2;
                    let positive = side % 2 == 1;
                    let plane = if positive { high[axis] } else { low[axis] };
                    let mut adjacent = center;
                    adjacent[axis] = plane + if positive { 0.0001 } else { -0.0001 };
                    let occupied = if (0.0..1.0).contains(&adjacent[axis]) {
                        material_at(mask, adjacent, slots).is_some()
                    } else if !standalone && model.neighbors[side] == Some(169) {
                        adjacent[axis] += if positive { -1.0 } else { 1.0 };
                        material_at(model.neighbor_facing[side], adjacent, slots).is_some()
                    } else {
                        false
                    };
                    if occupied {
                        continue;
                    }
                    let u = (axis + 1) % 3;
                    let v = (axis + 2) % 3;
                    let mut points = [[0.; 3]; 4];
                    for (i, [a, b]) in [[0, 0], [1, 0], [1, 1], [0, 1]].into_iter().enumerate() {
                        points[i][axis] = plane;
                        points[i][u] = if a == 0 { low[u] } else { high[u] };
                        points[i][v] = if b == 0 { low[v] } else { high[v] };
                    }
                    if !positive {
                        points.reverse();
                    }
                    let mut normal = Vec3::ZERO;
                    normal[axis] = if positive { 1.0 } else { -1.0 };
                    let (tu, tv) = match axis {
                        0 => (2, 1),
                        1 => (0, 2),
                        _ => (0, 1),
                    };
                    let mut tangent = Vec3::ZERO;
                    tangent[tu] = 1.0;
                    let handedness =
                        normal
                            .cross(tangent)
                            .dot(Vec3::from_array(std::array::from_fn(|i| {
                                if i == tv { 1.0 } else { 0.0 }
                            })));
                    let part = parts.entry(material).or_insert_with(|| PrimitiveTemplate {
                        positions: vec![],
                        normals: vec![],
                        tangents: vec![],
                        uvs: vec![],
                        indices: vec![],
                        material,
                    });
                    let base = part.positions.len() as u32;
                    part.positions.extend(points);
                    part.normals.extend([normal.to_array(); 4]);
                    part.tangents
                        .extend([tangent_frame(tangent, normal, handedness); 4]);
                    part.uvs.extend(points.map(|p| [p[tu], p[tv]]));
                    part.indices
                        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
    }
    parts.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basin_shell_has_no_duplicate_coplanar_faces_for_any_connection_mask() {
        for mask in 0..64 {
            let model = ModelCell {
                cell: [0; 3],
                material: 169,
                state: terra_world::edits::BlockObjectState {
                    facing: mask,
                    ..Default::default()
                },
                peer_wood: Default::default(),
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
            };
            let parts = profile(&model, &[], false);
            let mut faces = std::collections::BTreeSet::new();
            for part in parts {
                assert!(
                    part.positions
                        .iter()
                        .flatten()
                        .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
                );
                for indices in part.indices.chunks_exact(3) {
                    let [a, b, c] = [indices[0], indices[1], indices[2]]
                        .map(|i| Vec3::from_array(part.positions[i as usize]));
                    let normal = Vec3::from_array(part.normals[indices[0] as usize]);
                    assert!(
                        (b - a).cross(c - a).dot(normal) > 0.0,
                        "mask {mask}: wrong winding"
                    );
                }
                for quad in part.positions.chunks_exact(4) {
                    let mut key: Vec<_> = quad
                        .iter()
                        .map(|p| p.map(|v| (v * 100_000.).round() as i32))
                        .collect();
                    key.sort();
                    assert!(faces.insert(key), "mask {mask} has overlapping faces");
                }
                assert_eq!(part.positions.len(), part.tangents.len());
            }
        }
    }
    #[test]
    fn connected_basin_modules_do_not_emit_any_touching_wall_end_caps() {
        for (side, mask, peer_mask) in [(1, 2, 1), (0, 1, 2), (5, 32, 16), (4, 16, 32)] {
            let mut model = ModelCell {
                cell: [0; 3],
                material: 169,
                state: terra_world::edits::BlockObjectState {
                    facing: mask,
                    ..Default::default()
                },
                peer_wood: Default::default(),
                neighbors: [None; 6],
                neighbor_facing: [0; 6],
            };
            model.neighbors[side] = Some(169);
            model.neighbor_facing[side] = peer_mask;
            let axis = side / 2;
            let plane = if side % 2 == 0 { 0.0 } else { 1.0 };
            for part in profile(&model, &[], false) {
                assert!(
                    part.positions
                        .iter()
                        .zip(&part.normals)
                        .all(|(p, n)| p[axis] != plane || n[axis].abs() < 0.5)
                );
            }
        }
    }
}
