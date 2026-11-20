//! A pinnate stepped silhouette: open notches between paired leaflets, rather
//! than three overlapping solid planks. The Near mesh stays at 192 triangles.
use super::{PropDetail, PropMesh, PropPalette, geometry::add_box, leaf::add_profile};
use bevy_math::{Quat, Vec2, Vec3};
use std::collections::BTreeMap;

pub(super) fn append(
    batches: &mut BTreeMap<usize, PropMesh>,
    variant: usize,
    level: PropDetail,
    palette: &PropPalette,
) -> Result<(), String> {
    add_box(
        batches,
        palette.stem,
        Vec3::Y * 0.16,
        Vec3::new(0.026, 0.32, 0.026),
        Quat::IDENTITY,
    )?;
    for frond in 0..if level == PropDetail::Near { 3 } else { 2 } {
        let yaw = frond as f32 * std::f32::consts::TAU / 3.0 + variant as f32 * 0.3;
        let rotation = Quat::from_rotation_y(yaw);
        let material = palette.foliage[(frond + variant) % 3];
        let root = Vec3::Y * (0.20 + frond as f32 * 0.04);
        if level == PropDetail::Far {
            add_box(
                batches,
                material,
                root + rotation * Vec3::X * 0.12,
                Vec3::new(0.24, 0.018, 0.13),
                rotation,
            )?;
        } else if level == PropDetail::Middle {
            let outline = [
                [0.0, -0.018],
                [0.055, -0.018],
                [0.055, -0.09],
                [0.23, -0.09],
                [0.23, 0.09],
                [0.055, 0.09],
                [0.055, 0.018],
                [0.0, 0.018],
            ]
            .map(Vec2::from_array);
            add_profile(
                batches,
                material,
                root,
                &outline,
                rotation * Vec3::X,
                rotation * Vec3::Z,
                0.018,
            )?;
        } else {
            let outline = [
                [0.0, -0.018],
                [0.04, -0.018],
                [0.04, -0.10],
                [0.09, -0.10],
                [0.09, -0.018],
                [0.14, -0.018],
                [0.14, -0.065],
                [0.23, -0.065],
                [0.23, 0.065],
                [0.14, 0.065],
                [0.14, 0.018],
                [0.09, 0.018],
                [0.09, 0.10],
                [0.04, 0.10],
                [0.04, 0.018],
                [0.0, 0.018],
            ]
            .map(Vec2::from_array);
            add_profile(
                batches,
                material,
                root,
                &outline,
                rotation * Vec3::X,
                rotation * Vec3::Z,
                0.018,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fern_has_real_gaps_between_leaflet_pairs_within_the_existing_triangle_budget() {
        let palette = PropPalette {
            foliage: [1, 2, 3],
            stem: 4,
            ..Default::default()
        };
        let mut batches = BTreeMap::new();
        append(&mut batches, 0, PropDetail::Near, &palette).unwrap();
        assert_eq!(
            batches.values().map(|m| m.indices.len() / 3).sum::<usize>(),
            192
        );
        let frond = &batches[&1];
        let covers = |p: Vec2| {
            frond.indices.chunks_exact(3).any(|triangle| {
                if frond.normals[triangle[0] as usize][1] < 0.9 {
                    return false;
                }
                let points = [triangle[0], triangle[1], triangle[2]].map(|i| {
                    let point = frond.positions[i as usize];
                    Vec2::new(point[0], point[2])
                });
                let crosses =
                    (0..3).map(|i| (points[(i + 1) % 3] - points[i]).perp_dot(p - points[i]));
                let values: Vec<_> = crosses.collect();
                values.iter().all(|v| *v >= -1e-6) || values.iter().all(|v| *v <= 1e-6)
            })
        };
        assert!(covers(Vec2::new(0.065, 0.07)));
        assert!(!covers(Vec2::new(0.115, 0.07)));
        assert!(covers(Vec2::new(0.18, 0.05)));
        assert!(covers(Vec2::new(0.115, 0.0)), "continuous central rachis");
    }
}
