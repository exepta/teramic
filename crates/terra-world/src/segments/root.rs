//! Eight-cell bark courses form a bent taproot and two short, trailing forks.
use super::Profile;
use bevy_math::IVec3;
use std::sync::LazyLock;

struct Quad {
    points: [[f32; 3]; 4],
    color: [f32; 3],
}

fn model(tip: bool, collar: bool) -> Vec<Quad> {
    let occupied = |p: IVec3| {
        let (x, y, z) = (p.x, p.y, p.z);
        if !(0..8).contains(&y) {
            return ((y < 0 && !tip) || (y >= 8 && !collar))
                && (3..=4).contains(&x)
                && (3..=4).contains(&z);
        }
        let offset = i32::from((2..6).contains(&y));
        let main = (3 + offset..=4 + offset).contains(&x) && (3..=4).contains(&z);
        if tip && y == 0 {
            return x == 4 && z == 3;
        }
        main || (collar && y >= 6 && (2..=5).contains(&x) && (3..=5).contains(&z))
            || ((3..=4).contains(&y) && x == 6 && z == 3)
            || (y == 3 && x == 7 && z == 3)
            || ((1..=2).contains(&y) && x == 7 && z == 3)
            || ((4..=5).contains(&y) && x == 4 && z == 5)
            || ((3..=4).contains(&y) && x == 4 && z == 6)
    };
    let mut faces = Vec::new();
    terra_core::voxel_surface::visit_cell_faces::<8>(occupied, |vertices, _, side, cut| {
        if !cut {
            let color = match side {
                2 => [0.34, 0.19, 0.10],
                3 => [0.54, 0.35, 0.19],
                _ => [0.47, 0.28, 0.15],
            };
            faces.push(Quad {
                points: vertices.map(|v| v.to_array()),
                color,
            });
        }
    });
    faces
}

static MODELS: LazyLock<[Vec<Quad>; 4]> =
    LazyLock::new(|| std::array::from_fn(|i| model(i & 1 != 0, i & 2 != 0)));

pub(super) fn visit(
    seed: u64,
    y: i64,
    profile: Profile,
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let index = usize::from(profile.below == 0) + 2 * usize::from(profile.above == 0);
    let turn = seed.wrapping_add(y as u64) % 4;
    for quad in &MODELS[index] {
        let [a, b, c, d] = quad.points.map(|[x, y, z]| match turn {
            1 => [1. - z, y, x],
            2 => [1. - x, y, 1. - z],
            3 => [z, y, 1. - x],
            _ => [x, y, z],
        });
        emit([a, b, c], quad.color);
        emit([a, c, d], quad.color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_bark_has_grid_faces_and_an_open_square_sleeve_at_the_seam() {
        let profile = Profile {
            below: 1,
            above: 1,
            hanging: true,
            ..Default::default()
        };
        for y in -4..0 {
            let mut rim = std::collections::BTreeSet::new();
            visit(7, y, profile, &mut |triangle, _| {
                let normal = crate::ore_shapes::normal(triangle);
                assert_eq!(normal.iter().map(|v| v.abs()).sum::<f32>(), 1.);
                for p in triangle {
                    assert_eq!(p.map(|v| (v * 8.).round() / 8.), p);
                    if p[1] == 0. || p[1] == 1. {
                        assert_eq!(normal[1], 0.);
                        rim.insert((p[1] as u8, (8. * p[0]) as u8, (8. * p[2]) as u8));
                    }
                }
            });
            for end in [0, 1] {
                for x in [3, 5] {
                    for z in [3, 5] {
                        assert!(rim.contains(&(end, x, z)));
                    }
                }
            }
        }
    }
}
