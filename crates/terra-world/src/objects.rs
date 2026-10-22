//! Shared poses for interactive openings and the single adaptive chimney item.
use crate::edits::BlockObjectState;

pub type Direction = [i64; 3];
pub const DIRECTIONS: [Direction; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [1, 0, 0],
    [-1, 0, 0],
    [0, 0, 1],
    [0, 0, -1],
];

pub fn is_door(material: u16) -> bool {
    (65..=67).contains(&material)
}
pub fn is_window(material: u16) -> bool {
    (68..=70).contains(&material)
}
pub fn is_opening(material: u16) -> bool {
    is_door(material) || is_window(material)
}
pub fn is_open(material: u16, state: Option<BlockObjectState>) -> bool {
    is_opening(material) && state.is_some_and(|s| (4..8).contains(&s.facing))
}

pub fn opening_box(
    source: terra_core::block_shapes::ShapeBox,
    owner: [i64; 3],
    state: BlockObjectState,
) -> terra_core::block_shapes::ShapeBox {
    let mut result = terra_core::block_shapes::ShapeBox {
        minimum: [f64::INFINITY; 3],
        maximum: [f64::NEG_INFINITY; 3],
        hinged: false,
    };
    for corner in 0..8 {
        let mut point = std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                source.minimum[axis]
            } else {
                source.maximum[axis]
            }
        });
        if source.hinged && state.facing >= 4 {
            point = terra_core::block_shapes::hinge_point(point, 1.0);
        }
        let [x, y, z] = [point[0] - 0.5, point[1], point[2] - 0.5];
        let p = match state.facing % 4 {
            0 => [x, y, z],
            1 => [z, y, -x],
            2 => [-x, y, -z],
            _ => [-z, y, x],
        };
        for axis in 0..3 {
            let value = p[axis] + owner[axis] as f64 + if axis == 1 { 0.0 } else { 0.5 };
            result.minimum[axis] = result.minimum[axis].min(value);
            result.maximum[axis] = result.maximum[axis].max(value);
        }
    }
    result
}

/// New poses occupy 8..23; historic facing 0..3 on block 60 stays vertical.
/// Unordered pairs include straight pipes and all twelve orthogonal elbows.
pub fn chimney_pose(ports: [Direction; 2]) -> Option<u8> {
    let mut pose = 8;
    for (index, &a) in DIRECTIONS.iter().enumerate() {
        for &b in DIRECTIONS.iter().skip(index + 1) {
            if ports == [a, b] || ports == [b, a] {
                return Some(pose);
            }
            pose += 1;
        }
    }
    None
}

pub fn chimney_ports(material: u16, facing: u8) -> Option<[Direction; 2]> {
    if material == 60 && (40..55).contains(&facing) {
        let ports = chimney_ports(material, facing - 32)?;
        return (ports[0] != ports[1].map(|v| -v)).then_some(ports);
    }
    if material == 60 && facing >= 8 {
        let mut pose = 8;
        for (index, &a) in DIRECTIONS.iter().enumerate() {
            for &b in DIRECTIONS.iter().skip(index + 1) {
                if pose == facing {
                    return Some([a, b]);
                }
                pose += 1;
            }
        }
        return None;
    }
    let forward = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing % 4)];
    match material {
        60 => Some([DIRECTIONS[0], DIRECTIONS[1]]),
        61 | 63 => Some([DIRECTIONS[0], forward]),
        62 => Some([forward.map(|v| -v), forward]),
        64 => Some([DIRECTIONS[1], forward]),
        _ => None,
    }
}

/// Canonical mesh ID and its orthonormal basis, shared by rendering and collision.
pub fn chimney_basis(facing: u8) -> Option<(u16, [Direction; 3])> {
    if facing < 8 {
        return None;
    }
    let [a, b] = chimney_ports(60, facing)?;
    let cross = |a: Direction, b: Direction| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    if a == b.map(|v| -v) {
        let y = b;
        let x = if y[0] != 0 { [0, 0, 1] } else { [1, 0, 0] };
        Some((60, [x, y, cross(x, y)]))
    } else {
        let y = a.map(|v| -v);
        let z = b;
        Some((if facing >= 40 { 63 } else { 61 }, [cross(y, z), y, z]))
    }
}

pub fn chimney_point(facing: u8, point: [f64; 3]) -> [f64; 3] {
    let Some((_, basis)) = chimney_basis(facing) else {
        return point;
    };
    std::array::from_fn(|axis| {
        0.5 + (0..3)
            .map(|i| (point[i] - 0.5) * basis[i][axis] as f64)
            .sum::<f64>()
    })
}

/// At least three opaque neighbors form a fitted surround. Shared corners have
/// one owner, and the pipe lumen always remains open.
pub fn chimney_infill(
    material: u16,
    facing: u8,
    mut neighbor: impl FnMut(Direction) -> Option<u16>,
) -> Vec<(terra_core::block_shapes::ShapeBox, u16)> {
    let Some([a, b]) = chimney_ports(material, facing) else {
        return Vec::new();
    };
    if a != b.map(|v| -v) {
        return Vec::new();
    }
    let axis = a.iter().position(|v| *v != 0).unwrap_or(1);
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    // The shared straight tube has radius 0.24; mortar touches its outer wall.
    let inner_low = 0.26;
    let inner_high = 0.74;
    let directions = [(u, -1), (u, 1), (v, -1), (v, 1)];
    let materials: Vec<_> = directions
        .iter()
        .map(|&(axis, sign)| {
            let mut direction = [0; 3];
            direction[axis] = sign;
            neighbor(direction).filter(|m| {
                terra_voxel::MaterialId::from_raw(*m)
                    .is_some_and(terra_voxel::MaterialId::is_opaque)
            })
        })
        .collect();
    if materials.iter().flatten().count() < 3 {
        return Vec::new();
    }
    directions
        .into_iter()
        .zip(materials.iter().copied())
        .filter_map(|(direction, material)| material.map(|material| (direction, material)))
        .map(|((side, sign), material)| {
            let mut low = [0.0; 3];
            let mut high = [1.0; 3];
            if side == v {
                low[u] = if materials[0].is_some() {
                    inner_low
                } else {
                    0.0
                };
                high[u] = if materials[1].is_some() {
                    inner_high
                } else {
                    1.0
                };
            }
            if sign < 0 {
                high[side] = inner_low;
            } else {
                low[side] = inner_high;
            }
            (
                terra_core::block_shapes::ShapeBox {
                    minimum: low,
                    maximum: high,
                    hinged: false,
                },
                material,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitted_pipe_surrounds_inherit_four_materials_and_keep_the_lumen_free() {
        for axis in 0..3 {
            let mut direction = [0; 3];
            direction[axis] = 1;
            let pose = chimney_pose([direction, direction.map(|v| -v)]).unwrap();
            let pieces = chimney_infill(60, pose, |d| {
                Some(match d {
                    [1, 0, 0] | [0, 1, 0] => 16,
                    [-1, 0, 0] | [0, -1, 0] => 3,
                    [0, 0, 1] => 4,
                    _ => 58,
                })
            });
            assert_eq!(pieces.len(), 4);
            let volume: f64 = pieces
                .iter()
                .map(|(b, _)| (0..3).map(|i| b.maximum[i] - b.minimum[i]).product::<f64>())
                .sum();
            assert!((volume - (1.0 - 0.48 * 0.48)).abs() < 1.0e-10);
            assert!(
                pieces
                    .iter()
                    .all(|(b, _)| !b.intersects([0.27; 3], [0.73; 3]))
            );
            assert!(chimney_infill(60, pose, |_| None).is_empty());
            assert!(chimney_infill(60, pose, |_| Some(15)).is_empty());
        }
    }
    #[test]
    fn pipe_surround_requires_three_neighbors_without_requiring_a_complete_border() {
        for axis in 0..3 {
            let mut direction = [0; 3];
            direction[axis] = 1;
            let pose = chimney_pose([direction, direction.map(|v| -v)]).unwrap();
            let sides: Vec<_> = DIRECTIONS.into_iter().filter(|d| d[axis] == 0).collect();
            for mask in 1_u8..16 {
                let pieces = chimney_infill(60, pose, |d| {
                    sides
                        .iter()
                        .position(|side| *side == d)
                        .filter(|i| mask & (1 << i) != 0)
                        .map(|i| if i % 2 == 0 { 3 } else { 16 })
                });
                if mask.count_ones() < 3 {
                    assert!(pieces.is_empty());
                    continue;
                }
                assert_eq!(pieces.len(), mask.count_ones() as usize);
                for (index, (a, _)) in pieces.iter().enumerate() {
                    assert!(!a.intersects([0.27; 3], [0.73; 3]));
                    for (b, _) in &pieces[index + 1..] {
                        assert!(!a.intersects(b.minimum, b.maximum));
                    }
                }
                for side in sides
                    .iter()
                    .enumerate()
                    .filter_map(|(i, d)| (mask & (1 << i) != 0).then_some(d))
                {
                    let mut point = [0.1; 3];
                    for tangent in 0..3 {
                        if side[tangent] > 0 {
                            point[tangent] = 0.95;
                        }
                        if side[tangent] < 0 {
                            point[tangent] = 0.05;
                        }
                    }
                    assert!(
                        pieces
                            .iter()
                            .any(|(b, _)| b.intersects(point, point.map(|v| v + 0.001))),
                        "side {side:?} has an unfilled corner"
                    );
                }
            }
        }
    }
    #[test]
    fn chimney_pairs_roundtrip_without_changing_legacy_poses() {
        let mut seen = std::collections::BTreeSet::new();
        for (index, &a) in DIRECTIONS.iter().enumerate() {
            assert_eq!(chimney_pose([a; 2]), None);
            for &b in DIRECTIONS.iter().skip(index + 1) {
                let ports = [a, b];
                let pose = chimney_pose(ports).unwrap();
                assert!(seen.insert(pose));
                assert_eq!(chimney_pose([ports[1], ports[0]]), Some(pose));
                assert_eq!(chimney_ports(60, pose), Some(ports));
            }
        }
        assert_eq!(seen.len(), 15);
        for facing in 0..4 {
            assert_eq!(chimney_ports(60, facing), Some([[0, -1, 0], [0, 1, 0]]));
        }
        assert_eq!(chimney_ports(60, 23), None);
        for pose in 8..23 {
            let ports = chimney_ports(60, pose).unwrap();
            if ports[0] == ports[1].map(|v| -v) {
                assert_eq!(chimney_ports(60, pose + 32), None);
            } else {
                assert_eq!(chimney_ports(60, pose + 32), Some(ports));
                assert_eq!(chimney_basis(pose + 32).unwrap().0, 63);
            }
        }
    }
}
