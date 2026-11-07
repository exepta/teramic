//! Bounded solid apple meshes shared by world fruit, items, drops and picking.
use super::{PropDetail, PropMesh, PropPalette};

fn cuboid(mesh: &mut PropMesh, center: [f32; 3], size: [f32; 3], color: [f32; 3]) {
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let base = mesh.positions.len() as u32;
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            let mut normal = [0.0; 3];
            normal[axis] = sign;
            for [a, b] in [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
                let mut p = center;
                p[axis] += sign * size[axis] * 0.5;
                p[u] += a * size[u];
                p[v] += b * size[v];
                mesh.positions.push(p);
                mesh.normals.push(normal);
                mesh.uvs.push([a + 0.5, b + 0.5]);
                let light = if axis == 1 && sign > 0.0 {
                    1.0
                } else if axis == 2 {
                    0.88
                } else {
                    0.94
                };
                mesh.colors
                    .push([color[0] * light, color[1] * light, color[2] * light, 1.0]);
            }
            mesh.indices.extend(
                if sign > 0.0 {
                    [0, 1, 2, 0, 2, 3]
                } else {
                    [0, 2, 1, 0, 3, 2]
                }
                .map(|i| base + i),
            );
        }
    }
}

/// 0 ripe, 1 unripe, 2 blossom, 3 eaten core. Geometry has no soil pedestal.
pub fn apple(variant: u8, level: PropDetail, palette: &PropPalette) -> Vec<PropMesh> {
    let mut mesh = PropMesh {
        material: palette.crop,
        ..Default::default()
    };
    if variant == 2 {
        for [x, z] in [
            [0.0, 0.0],
            [-0.095, 0.0],
            [0.095, 0.0],
            [0.0, -0.095],
            [0.0, 0.095],
        ] {
            cuboid(
                &mut mesh,
                [x, 0.17, z],
                [0.105, 0.055, 0.105],
                [1.0, 0.77, 0.78],
            );
        }
        cuboid(
            &mut mesh,
            [0.0, 0.205, 0.0],
            [0.055, 0.025, 0.055],
            [0.98, 0.72, 0.20],
        );
    } else if variant == 3 {
        cuboid(
            &mut mesh,
            [0.0, 0.045, 0.0],
            [0.21, 0.065, 0.19],
            [0.70, 0.07, 0.035],
        );
        cuboid(
            &mut mesh,
            [0.0, 0.285, 0.0],
            [0.24, 0.055, 0.21],
            [0.85, 0.10, 0.05],
        );
        for (y, width) in [
            (0.09, 0.13),
            (0.135, 0.085),
            (0.18, 0.07),
            (0.225, 0.105),
            (0.255, 0.15),
        ] {
            cuboid(
                &mut mesh,
                [0.0, y, 0.0],
                [width, 0.055, width * 0.82],
                [0.96, 0.83, 0.49],
            );
        }
        for x in [-0.024, 0.024] {
            cuboid(
                &mut mesh,
                [x, 0.18, -0.033],
                [0.012, 0.035, 0.012],
                [0.28, 0.11, 0.035],
            );
        }
    } else {
        let green = variant == 1;
        let color = if green {
            [0.48, 0.67, 0.085]
        } else {
            [0.85, 0.065, 0.025]
        };
        let rings = if level == PropDetail::Far { 3 } else { 5 };
        for i in 0..rings {
            let t = (i as f32 + 0.5) / rings as f32;
            let width = 0.32 * (1.0 - ((t - 0.57) * 1.3).powi(2));
            cuboid(
                &mut mesh,
                [0.0, 0.03 + t * 0.28, 0.0],
                [width, 0.28 / rings as f32 + 0.001, width * 0.91],
                color,
            );
        }
        for x in [-0.07, 0.07] {
            cuboid(&mut mesh, [x, 0.31, 0.0], [0.115, 0.045, 0.18], color);
        }
        if !green && level == PropDetail::Near {
            cuboid(
                &mut mesh,
                [-0.065, 0.26, -0.132],
                [0.064, 0.043, 0.008],
                [1.0, 0.36, 0.20],
            );
        }
    }
    cuboid(
        &mut mesh,
        [0.012, 0.36, 0.0],
        [0.035, 0.11, 0.033],
        [0.35, 0.16, 0.045],
    );
    cuboid(
        &mut mesh,
        [0.075, 0.395, 0.004],
        [0.10, 0.035, 0.065],
        [0.30, 0.52, 0.055],
    );
    cuboid(
        &mut mesh,
        [0.105, 0.415, 0.004],
        [0.06, 0.025, 0.045],
        [0.43, 0.63, 0.075],
    );
    vec![mesh]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apple_and_eaten_core_are_solid_distinct_bounded_meshes_with_stem_and_leaf() {
        let palette = PropPalette {
            crop: 7,
            ..Default::default()
        };
        let whole = apple(0, PropDetail::Near, &palette);
        let core = apple(3, PropDetail::Near, &palette);
        assert_ne!(whole[0].positions, core[0].positions);
        for variant in 0..4 {
            for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let parts = apple(variant, detail, &palette);
                let mesh = &parts[0];
                assert_eq!(mesh.material, 7);
                assert_eq!(mesh.positions.len(), mesh.colors.len());
                assert_eq!(mesh.positions.len(), mesh.normals.len());
                assert!(mesh.positions.len() < 1024);
                assert!(
                    mesh.indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.positions.len())
                );
                assert!(
                    mesh.positions
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && v.abs() < 0.5)
                );
                assert!(mesh.positions.iter().all(|p| p[1] >= 0.0));
            }
        }
    }
}
