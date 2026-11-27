//! Greedy exterior quads of a small voxel grid, with separately removable
//! interfaces between metre cells. Neighbour queries may lie one step outside.
use bevy_math::{IVec3, Vec3};

pub fn visit_cell_faces<const N: usize>(
    occupied: impl Fn(IVec3) -> bool,
    mut emit: impl FnMut([Vec3; 4], Vec3, usize, bool),
) {
    for side in 0..6 {
        let axis = side / 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let sign = if side % 2 == 0 { -1 } else { 1 };
        let mut normal = IVec3::ZERO;
        normal[axis] = sign;
        for plane in 0..N {
            let mut mask = [[None; N]; N];
            for (row, line) in mask.iter_mut().enumerate() {
                for (col, tile) in line.iter_mut().enumerate() {
                    let mut p = IVec3::ZERO;
                    p[axis] += plane as i32;
                    p[u] += col as i32;
                    p[v] += row as i32;
                    if occupied(p) {
                        let cut = occupied(p + normal);
                        if !cut || (sign < 0 && plane == 0) || (sign > 0 && plane == N - 1) {
                            *tile = Some(cut);
                        }
                    }
                }
            }
            // Merge both the visible steps and the hidden fracture planes; a mined
            // neighbour reveals just its share of the common boundary.
            for row in 0..N {
                for col in 0..N {
                    let Some(cut) = mask[row][col] else { continue };
                    let end = (col..N).find(|&i| mask[row][i] != Some(cut)).unwrap_or(N);
                    let bottom = (row + 1..N)
                        .find(|&j| mask[j][col..end].iter().any(|&t| t != Some(cut)))
                        .unwrap_or(N);
                    for line in &mut mask[row..bottom] {
                        line[col..end].fill(None);
                    }
                    let mut vertices =
                        [(col, row), (end, row), (end, bottom), (col, bottom)].map(|(i, j)| {
                            let mut p = Vec3::ZERO;
                            p[axis] = (plane + usize::from(sign > 0)) as f32;
                            p[u] = i as f32;
                            p[v] = j as f32;
                            p / N as f32
                        });
                    if sign < 0 {
                        vertices.swap(1, 3);
                    }
                    emit(vertices, normal.as_vec3(), side, cut);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_neighbor_hides_only_its_shared_fracture_area() {
        let mut visible = 0.;
        let mut fracture = 0.;
        visit_cell_faces::<4>(
            |p| {
                (0..4).contains(&p.y)
                    && (0..4).contains(&p.z)
                    && ((0..4).contains(&p.x) || (p.x == 4 && p.y < 2))
            },
            |[a, b, c, _], normal, side, cut| {
                let area = (b - a).cross(c - a);
                assert!(area.dot(normal) > 0.);
                if side == 1 {
                    if cut {
                        fracture += area.length();
                    } else {
                        visible += area.length();
                    }
                }
            },
        );
        assert_eq!((visible, fracture), (0.5, 0.5));
    }
}
