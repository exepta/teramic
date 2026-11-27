//! Solid fruit meshes shared by world fruit, inventory items, drops and picking.
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
        // A flattened base, broad shoulders and two raised lobes frame the stem.
        // Each tier has clipped voxel corners. Far fruit uses a single box per tier.
        let mut bottom = 0.027;
        for (top, width, tone) in [
            (0.057, 0.16, 0.78),
            (0.109, 0.26, 0.88),
            (0.234, 0.33, 0.96),
            (0.285, 0.29, 1.0),
        ] {
            let height = top - bottom;
            let y = bottom + height * 0.5;
            let pigment = color.map(|channel| channel * tone);
            let depth = width * 0.94;
            if level == PropDetail::Far {
                cuboid(&mut mesh, [0., y, 0.], [width, height, depth], pigment);
            } else {
                cuboid(
                    &mut mesh,
                    [0., y, 0.],
                    [width, height, depth * 0.62],
                    pigment,
                );
                for side in [-1., 1.] {
                    cuboid(
                        &mut mesh,
                        [0., y, side * depth * 0.405],
                        [width * 0.64, height, depth * 0.19],
                        pigment,
                    );
                }
            }
            bottom = top;
        }
        for x in [-0.066, 0.066] {
            cuboid(&mut mesh, [x, 0.296, 0.], [0.085, 0.022, 0.13], color);
        }
    }
    cuboid(
        &mut mesh,
        [0.012, 0.352, 0.0],
        [0.031, 0.151, 0.032],
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

/// Stepped husk, seam and the three germination pores on a ripe coconut.
pub fn coconut(variant: u8, palette: &PropPalette) -> Vec<PropMesh> {
    let mut mesh = PropMesh {
        material: palette.crop,
        ..Default::default()
    };
    for (height, width, depth, color) in [
        (0.060, 0.21, 0.22, [0.46, 0.24, 0.09]),
        (0.145, 0.33, 0.31, [0.60, 0.32, 0.12]),
        (0.235, 0.38, 0.34, [0.70, 0.40, 0.16]),
        (0.325, 0.33, 0.30, [0.63, 0.35, 0.13]),
        (0.400, 0.21, 0.21, [0.50, 0.27, 0.10]),
    ] {
        cuboid(&mut mesh, [0.0, height, 0.0], [width, 0.10, depth], color);
    }
    // Pale coir follows the steps of the husk on both sides of the pores.
    for side in [-1.0, 1.0] {
        for (x, y, z) in [
            (0.081, 0.145, 0.155),
            (0.121, 0.235, 0.170),
            (0.093, 0.325, 0.150),
        ] {
            cuboid(
                &mut mesh,
                [side * x, y, z],
                [0.023, 0.074, 0.008],
                [0.90, 0.62, 0.28],
            );
        }
    }
    for [x, y] in [[-0.055, 0.26], [0.045, 0.25], [-0.010, 0.21]] {
        cuboid(
            &mut mesh,
            [x, y, 0.173],
            [0.035, 0.035, 0.015],
            [0.15, 0.08, 0.037],
        );
    }
    if variant >= 128 {
        cuboid(
            &mut mesh,
            [0.0, 0.46, -0.025],
            [0.055, 0.12, 0.06],
            [0.38, 0.32, 0.11],
        );
        cuboid(
            &mut mesh,
            [0.0, 0.51, -0.16],
            [0.055, 0.05, 0.30],
            [0.44, 0.34, 0.12],
        );
    }
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
