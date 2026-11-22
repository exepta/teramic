//! Canonical, deterministic mineral facets, shared by section meshes and picking.
use terra_core::OreKind;

mod salt;
use salt::visit_salt;

/// Re-evaluate only exposed salt carriers after neighboring edits. The caller
/// supplies its worker snapshot; this never runs voxel generation on the main thread.
pub fn salt_growth_snapshot(
    faces: &[terra_voxel::meshing::BlockFace],
    origin: [i64; 3],
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> Vec<([u8; 3], u8, f32)> {
    let mut result = Vec::new();
    for face in faces {
        if !matches!(face.material, 30 | 37)
            || !matches!(face.side, 2 | 3)
            || face.occupied_neighbor
        {
            continue;
        }
        for v in 0..face.height {
            for u in 0..face.width {
                let mut cell = face.minimum;
                cell[face.u_axis] += u;
                cell[face.v_axis] += v;
                let p = std::array::from_fn(|axis| origin[axis] + i64::from(cell[axis]));
                let h = mineral_growth_height(face.material, p, face.side, &mut block_at);
                if h > 0.0 {
                    result.push((cell.map(|v| v as u8), face.side as u8, h));
                }
            }
        }
    }
    result
}
pub use salt::{MINERAL_REACH, SALT_REACH, mineral_growth_height, salt_growth_height};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OreDetail {
    Near,
    Middle,
    Far,
}

fn hash(position: [i64; 3], side: usize, salt: u64) -> u64 {
    let mut value = salt ^ (side as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for coordinate in position {
        value ^= (coordinate as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 30)).wrapping_mul(0x94d0_49bb_1331_11eb);
    }
    value ^ (value >> 31)
}

fn sample(seed: u64, shift: u32) -> f32 {
    ((seed >> shift) & 255) as f32 / 255.0
}

fn point(side: usize, [u, v, height]: [f32; 3]) -> [f32; 3] {
    let axis = side / 2;
    let mut result = [0.0; 3];
    result[(axis + 1) % 3] = u;
    result[(axis + 2) % 3] = v;
    result[axis] = if side.is_multiple_of(2) {
        -height
    } else {
        1.0 + height
    };
    result
}

mod geodes;

/// Tapered mineral growth shared by salt and dry dripstone. Only supported
/// horizontal faces carry points; vertical walls keep sparse shallow inlays.
pub fn visit_dripstone_face(
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    visit: impl FnMut([[f32; 3]; 3]),
) {
    salt::visit_growth(37, position, side, detail, 0.0, visit);
}

pub fn visit_material_face(
    material: u16,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    mut visit: impl FnMut([[f32; 3]; 3]),
) {
    visit_shaded_material_face(material, position, side, detail, |triangle, _| {
        visit(triangle)
    });
}

pub fn visit_face(
    ore: OreKind,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    visit: impl FnMut([[f32; 3]; 3]),
) {
    visit_material_face(ore.material(), position, side, detail, visit);
}

/// Fracture beds and mineral inlays share the same canonical triangles as picking.
/// Tone is linear vertex color, so the dark bed needs no extra material or draw.
pub fn visit_shaded_material_face(
    material: u16,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    mut visit: impl FnMut([[f32; 3]; 3], f32),
) {
    visit_shaded_face_with_growth(material, position, side, detail, 0.0, &mut visit);
}

/// Growth is measured against the edited world before meshing or picking.
pub fn visit_shaded_face_with_growth(
    material: u16,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    growth: f32,
    mut visit: impl FnMut([[f32; 3]; 3], f32),
) {
    if material == OreKind::Salt.material() {
        visit_salt(position, side, detail, growth, |triangle| {
            visit(triangle, 1.0)
        });
        return;
    }
    if let Some(ore) = OreKind::from_material(material) {
        geodes::deposit(ore, position, side, detail, &mut visit);
        if ore != OreKind::Salt && side < 6 {
            visit_veins(ore, position, side, detail, visit);
        }
    } else if material == 37 {
        salt::visit_growth(material, position, side, detail, growth, |triangle| {
            visit(triangle, 1.0)
        });
    } else if matches!(material, 38 | 39) && side < 6 {
        let facet = hash(position, side, 0x4652_4143_5455_5245);
        let prominent = matches!(side, 2 | 3);
        let height = if prominent {
            0.79 + 0.15 * sample(facet, 24)
        } else {
            0.03 + 0.06 * sample(facet, 16)
        };
        let emit = |triangle| visit(triangle, 1.);
        if material == 38 {
            salt::visit_halite(position, side, detail, height, prominent, emit);
        } else {
            salt::visit_calcite(position, side, detail, height, prominent, emit);
        }
    }
}

fn visit_veins(
    ore: OreKind,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    mut visit: impl FnMut([[f32; 3]; 3], f32),
) {
    let seed = hash(
        position,
        side,
        0x6f72_655f_6465_7461 ^ u64::from(ore.material()),
    );
    let random = hash(position, side, seed.wrapping_add(300));
    let center = [
        0.44 + sample(random, 0) * 0.12,
        0.44 + sample(random, 8) * 0.12,
    ];
    let angle = sample(seed, 40) * std::f32::consts::TAU;
    let direction = [angle.cos(), angle.sin()];
    let perpendicular = [-direction[1], direction[0]];
    let at = |along: f32, across: f32| {
        std::array::from_fn::<_, 2, _>(|axis| {
            (center[axis] + direction[axis] * along + perpendicular[axis] * across)
                .clamp(0.08, 0.92)
        })
    };
    let start = at(
        -0.28 - sample(seed, 0) * 0.12,
        (sample(seed, 8) - 0.5) * 0.18,
    );
    let end = at(
        0.26 + sample(seed, 16) * 0.15,
        (sample(seed, 24) - 0.5) * 0.22,
    );
    let elbow = at(-0.12, 0.035 + sample(seed, 32) * 0.05);
    let branch = at(-0.10 + sample(seed, 48) * 0.13, -0.12);
    let segments = [
        (start, elbow),
        (elbow, center),
        (center, end),
        (elbow, branch),
        (branch, at(-0.04 + sample(seed, 56) * 0.17, -0.27)),
    ];
    let count = match detail {
        OreDetail::Near | OreDetail::Middle => 2,
        OreDetail::Far => 1,
    };
    for (index, &(a, b)) in segments[..count].iter().enumerate() {
        let delta = [b[0] - a[0], b[1] - a[1]];
        let length = delta[0].hypot(delta[1]);
        let width =
            0.004 + sample(hash(position, side, seed.wrapping_add(index as u64)), 24) * 0.004;
        for (width, height, tone) in [(width, 0.002, 0.28), (width * 0.46, 0.006, 1.08)] {
            let offset = [-delta[1] / length * width, delta[0] / length * width];
            let vertices = [
                [a[0] - offset[0], a[1] - offset[1], height],
                [b[0] - offset[0], b[1] - offset[1], height],
                [b[0] + offset[0], b[1] + offset[1], height],
                [a[0] + offset[0], a[1] + offset[1], height],
            ]
            .map(|vertex| point(side, vertex));
            for mut indices in [[0, 1, 2], [0, 2, 3]] {
                if side.is_multiple_of(2) {
                    indices.swap(1, 2);
                }
                visit(indices.map(|index| vertices[index]), tone);
            }
        }
    }
}

pub fn normal(triangle: [[f32; 3]; 3]) -> [f32; 3] {
    let a: [f32; 3] = std::array::from_fn(|i| triangle[1][i] - triangle[0][i]);
    let b: [f32; 3] = std::array::from_fn(|i| triangle[2][i] - triangle[0][i]);
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = cross.iter().map(|v| v * v).sum::<f32>().sqrt();
    cross.map(|v| v / length.max(1.0e-12))
}

/// Two-sided facet test, so a ray originating inside a detail is also valid.
pub fn ray_triangle(
    triangle: [[f32; 3]; 3],
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<f64> {
    let subtract = |a: [f64; 3], b: [f64; 3]| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| a[i] * b[i]).sum::<f64>();
    let a = triangle[0].map(f64::from);
    let edge1 = subtract(triangle[1].map(f64::from), a);
    let edge2 = subtract(triangle[2].map(f64::from), a);
    let p = cross(direction, edge2);
    let determinant = dot(edge1, p);
    if determinant.abs() < 1.0e-10 {
        return None;
    }
    let offset = subtract(origin, a);
    let u = dot(offset, p) / determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(offset, edge1);
    let v = dot(direction, q) / determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = dot(edge2, q) / determinant;
    (distance >= 0.0 && distance <= reach).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_wall_deposits_keep_sparse_small_points_instead_of_formation_caps() {
        for side in 0..6 {
            let mut expected = Vec::new();
            visit_dripstone_face([-17, -33, 16], side, OreDetail::Near, |triangle| {
                expected.push(triangle)
            });
            let mut actual = Vec::new();
            visit_material_face(37, [-17, -33, 16], side, OreDetail::Near, |triangle| {
                actual.push(triangle)
            });
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn formation_caps_have_wide_pointed_floor_and_ceiling_tips_at_every_lod() {
        for material in [38, 39] {
            for detail in [OreDetail::Near, OreDetail::Middle, OreDetail::Far] {
                for side in [2, 3] {
                    let mut vertices = Vec::new();
                    visit_material_face(material, [-17, -33, 16], side, detail, |triangle| {
                        vertices.extend(triangle)
                    });
                    assert!(vertices.iter().flatten().all(|value| value.is_finite()));
                    assert!(vertices.iter().any(|point| if side == 3 {
                        point[1] > 1.7
                    } else {
                        point[1] < -0.7
                    }));
                    assert!(vertices.iter().all(|point| point[0] >= 0.0
                        && point[0] <= 1.0
                        && point[2] >= 0.0
                        && point[2] <= 1.0));
                }
            }
        }
    }
    #[test]
    fn fracture_inlays_have_dark_beds_bright_minerals_and_the_same_pick_triangles() {
        for ore in OreKind::ALL.into_iter().filter(|ore| *ore != OreKind::Salt) {
            for side in 0..6 {
                let mut shaded = Vec::new();
                visit_shaded_material_face(
                    ore.material(),
                    [-17, -31, 16],
                    side,
                    OreDetail::Near,
                    |triangle, tone| shaded.push((triangle, tone)),
                );
                let mut picked = Vec::new();
                visit_material_face(
                    ore.material(),
                    [-17, -31, 16],
                    side,
                    OreDetail::Near,
                    |triangle| picked.push(triangle),
                );
                assert_eq!(
                    picked,
                    shaded
                        .iter()
                        .map(|(triangle, _)| *triangle)
                        .collect::<Vec<_>>()
                );
                assert!(shaded.iter().any(|(_, tone)| *tone < 0.4));
                assert!(shaded.iter().any(|(_, tone)| *tone > 1.0));
                assert!(shaded.len() <= 68);
            }
        }
    }
    #[test]
    fn facets_are_stable_bounded_and_keep_far_primary_crystal() {
        for ore in OreKind::ALL {
            for side in 0..6 {
                let mesh = |detail| {
                    let mut triangles = Vec::new();
                    visit_face(ore, [-17, -33, 16], side, detail, |t| triangles.push(t));
                    triangles
                };
                let near = mesh(OreDetail::Near);
                let far = mesh(OreDetail::Far);
                assert_eq!(near, mesh(OreDetail::Near));
                assert!(near.len() <= 104 && far.len() < near.len());
                assert!(far.iter().all(|triangle| near.contains(triangle)));
                for triangle in near {
                    let normal = normal(triangle);
                    assert!(normal.iter().all(|v| v.is_finite()));
                    assert!(normal.iter().map(|v| v * v).sum::<f32>() > 0.99);
                    for vertex in triangle {
                        for (axis, value) in vertex.into_iter().enumerate() {
                            assert!(value.is_finite());
                            if axis == side / 2 {
                                assert!((-0.99..=1.99).contains(&value));
                            } else {
                                assert!((0.0..=1.0).contains(&value));
                            }
                        }
                    }
                }
            }
        }
    }
}
