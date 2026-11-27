//! Five small voxel trees: no dirt plinth, each silhouette follows its full-grown species.
use super::{PropDetail, PropMesh, PropPalette, geometry::add_box};
use bevy_math::{Quat, Vec3};
use std::collections::BTreeMap;
use terra_core::TreeSpecies;
fn voxel(
    b: &mut BTreeMap<usize, PropMesh>,
    m: usize,
    p: [f32; 3],
    s: [f32; 3],
    c: [f32; 3],
) -> Result<(), String> {
    let start = b.get(&m).map_or(0, |v| v.positions.len());
    add_box(
        b,
        m,
        Vec3::from_array(p),
        Vec3::from_array(s),
        Quat::IDENTITY,
    )?;
    let mesh = b.get_mut(&m).unwrap();
    mesh.colors.resize(mesh.positions.len(), [1.; 4]);
    let c = c.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    mesh.colors[start..].fill([c[0], c[1], c[2], 1.]);
    Ok(())
}
pub(super) fn template(
    species: TreeSpecies,
    detail: PropDetail,
    p: &PropPalette,
) -> Result<Vec<PropMesh>, String> {
    let mut b = BTreeMap::new();
    let m = p.crop;
    let bark = match species {
        TreeSpecies::Birch => [0.84, 0.80, 0.66],
        TreeSpecies::Spruce => [0.30, 0.18, 0.11],
        _ => [0.42, 0.25, 0.13],
    };
    let green = match species {
        TreeSpecies::Spruce => [0.24, 0.43, 0.25],
        TreeSpecies::Acacia => [0.58, 0.63, 0.17],
        TreeSpecies::Birch => [0.75, 0.80, 0.34],
        TreeSpecies::Cherry => [0.93, 0.54, 0.67],
        _ => [0.48, 0.67, 0.20],
    };
    let h = if species == TreeSpecies::Spruce {
        0.94
    } else {
        0.82
    };
    let stem_height = if species == TreeSpecies::Spruce {
        h
    } else {
        h * 0.76
    };
    voxel(
        &mut b,
        m,
        [0., stem_height * 0.5, 0.],
        [0.085, stem_height, 0.085],
        bark,
    )?;
    if species == TreeSpecies::Birch && detail != PropDetail::Far {
        for y in [0.12, 0.27, 0.42] {
            voxel(
                &mut b,
                m,
                [0., y, 0.045],
                [0.09, 0.025, 0.008],
                [0.24, 0.19, 0.15],
            )?;
        }
    }
    if species == TreeSpecies::Spruce {
        for (y, w) in [
            (0.35, 0.57),
            (0.51, 0.43),
            (0.67, 0.30),
            (0.82, 0.17),
            (0.94, 0.09),
        ] {
            voxel(&mut b, m, [0., y, 0.], [w, 0.12, w], green)?;
            if detail == PropDetail::Near {
                for (x, z) in [
                    (-w * 0.34, 0.),
                    (w * 0.34, 0.),
                    (0., -w * 0.34),
                    (0., w * 0.34),
                ] {
                    voxel(
                        &mut b,
                        m,
                        [x, y - 0.025, z],
                        [w * 0.36, 0.12, w * 0.36],
                        [green[0] * 0.9, green[1] * 0.94, green[2] * 0.9],
                    )?;
                }
            }
        }
    } else {
        let crown = if species == TreeSpecies::Acacia {
            [
                [-0.27, 0.70, 0.07],
                [0.22, 0.75, 0.03],
                [-0.12, 0.79, -0.20],
                [0.12, 0.80, 0.18],
                [0., 0.76, 0.],
            ]
        } else {
            [
                [-0.18, 0.49, 0.09],
                [0.19, 0.57, 0.04],
                [-0.10, 0.72, -0.13],
                [0.10, h, 0.10],
                [0.0, 0.64, 0.0],
            ]
        };
        let w = if species == TreeSpecies::Birch {
            0.20
        } else {
            0.25
        };
        let mut foliage = Vec::new();
        for (i, [x, y, z]) in crown.into_iter().enumerate() {
            voxel(
                &mut b,
                m,
                [x * 0.5, y * 0.70, z * 0.5],
                [0.055, y * 0.35, 0.055],
                bark,
            )?;
            let c = if i % 2 == 0 {
                green
            } else {
                green.map(|v| (v * 1.1).min(1.))
            };
            voxel(&mut b, m, [x, y, z], [w, 0.17, w], c)?;
            foliage.push(Bounds::new([x, y, z], [w, 0.17, w]));
            if detail == PropDetail::Near {
                voxel(
                    &mut b,
                    m,
                    [x + 0.055, y + 0.085, z - 0.045],
                    [w * 0.64, 0.10, w * 0.6],
                    green,
                )?;
                foliage.push(Bounds::new(
                    [x + 0.055, y + 0.085, z - 0.045],
                    [w * 0.64, 0.10, w * 0.6],
                ));
            }
        }
        if species == TreeSpecies::Apple {
            // Fruit used to share the leaf's +Z plane while penetrating its
            // lower edge. Hang it below exposed sides, with a stem into the leaf.
            for ([x, y, z], [dx, dz]) in
                crown
                    .into_iter()
                    .take(3)
                    .zip([[-1., 0.], [1., 0.], [0., -1.]])
            {
                let center = [x + dx * w * 0.36, y - 0.125, z + dz * w * 0.36];
                voxel(&mut b, m, center, [0.075, 0.07, 0.07], [0.80, 0.13, 0.08])?;
                voxel(
                    &mut b,
                    m,
                    [center[0], center[1] + 0.041, center[2]],
                    [0.012, 0.025, 0.012],
                    bark,
                )?;
            }
        }
        if species == TreeSpecies::Cherry && detail != PropDetail::Far {
            // Keep the tiny voxel blossoms, but select exposed faces around all
            // four sides instead of stamping every flower on the same +Z face.
            for normal in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let mut order = [0, 1, 2, 3, 4];
                order.sort_by(|&a, &c| {
                    Vec3::from_array(crown[c])
                        .dot(normal)
                        .total_cmp(&Vec3::from_array(crown[a]).dot(normal))
                });
                let mut count = 0;
                for index in order {
                    let center = Vec3::from_array(crown[index])
                        + Vec3::new(0., -0.035, 0.)
                        + normal * (w * 0.5 + 0.013);
                    let size = if normal.x != 0. {
                        [0.023, 0.13, 0.13]
                    } else {
                        [0.13, 0.13, 0.023]
                    };
                    if foliage.iter().any(|leaf| {
                        leaf.overlaps(Bounds::new((center + normal * 0.0025).to_array(), size))
                    }) {
                        continue;
                    }
                    blossom(&mut b, m, center, normal)?;
                    count += 1;
                    if count == if detail == PropDetail::Near { 2 } else { 1 } {
                        break;
                    }
                }
            }
        }
    }
    Ok(b.into_values().collect())
}

#[derive(Clone, Copy)]
struct Bounds {
    minimum: Vec3,
    maximum: Vec3,
}
impl Bounds {
    fn new(center: [f32; 3], size: [f32; 3]) -> Self {
        let center = Vec3::from_array(center);
        let half = Vec3::from_array(size) * 0.5;
        Self {
            minimum: center - half,
            maximum: center + half,
        }
    }
    fn overlaps(self, other: Self) -> bool {
        (0..3).all(|axis| {
            self.minimum[axis] < other.maximum[axis] - 0.00001
                && other.minimum[axis] < self.maximum[axis] - 0.00001
        })
    }
}
fn blossom(
    b: &mut BTreeMap<usize, PropMesh>,
    m: usize,
    center: Vec3,
    normal: Vec3,
) -> Result<(), String> {
    let tangent = Vec3::new(normal.z, 0., -normal.x);
    for (u, v, width, height) in [
        (-0.04, 0., 0.05, 0.03),
        (0.04, 0., 0.05, 0.03),
        (0., -0.04, 0.03, 0.05),
        (0., 0.04, 0.03, 0.05),
    ] {
        let p = center + tangent * u + Vec3::Y * v;
        let size = if normal.x != 0. {
            [0.018, height, width]
        } else {
            [width, height, 0.018]
        };
        voxel(b, m, p.to_array(), size, [1., 0.73, 0.80])?;
    }
    let size = if normal.x != 0. {
        [0.022, 0.03, 0.03]
    } else {
        [0.03, 0.03, 0.022]
    };
    voxel(
        b,
        m,
        (center + normal * 0.003).to_array(),
        size,
        [0.95, 0.76, 0.24],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cube {
        bounds: Bounds,
        color: [f32; 4],
    }
    fn meshes(species: TreeSpecies, detail: PropDetail) -> Vec<PropMesh> {
        crate::props::visual::template(
            species.sapling().into(),
            0,
            detail,
            &PropPalette {
                crop: 7,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn cubes(meshes: &[PropMesh]) -> Vec<Cube> {
        meshes
            .iter()
            .flat_map(|mesh| {
                mesh.positions
                    .chunks_exact(24)
                    .enumerate()
                    .map(|(i, vertices)| {
                        let mut minimum = Vec3::splat(f32::INFINITY);
                        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
                        for &vertex in vertices {
                            minimum = minimum.min(Vec3::from_array(vertex));
                            maximum = maximum.max(Vec3::from_array(vertex));
                        }
                        Cube {
                            bounds: Bounds { minimum, maximum },
                            color: mesh.colors[i * 24],
                        }
                    })
            })
            .collect()
    }
    fn apple(cube: &Cube) -> bool {
        cube.color[0] > 0.5 && cube.color[1] < 0.05 && cube.color[2] < 0.05
    }
    fn gold(cube: &Cube) -> bool {
        cube.color[0] > 0.8 && cube.color[1] > 0.45 && cube.color[2] < 0.1
    }

    #[test]
    fn apple_fruit_has_clear_space_from_every_crown_box_at_every_detail_level() {
        for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
            let meshes = meshes(TreeSpecies::Apple, detail);
            let cubes = cubes(&meshes);
            let fruit = cubes.iter().filter(|cube| apple(cube)).collect::<Vec<_>>();
            let leaves = cubes
                .iter()
                .filter(|cube| cube.color[1] > cube.color[0])
                .collect::<Vec<_>>();
            assert_eq!(fruit.len(), 3);
            assert!(!leaves.is_empty());
            for fruit in fruit {
                for leaf in &leaves {
                    assert!(
                        !fruit.bounds.overlaps(leaf.bounds),
                        "fruit must not penetrate the canopy: {detail:?}"
                    );
                    // A positive gap also excludes touching coplanar skin/leaf
                    // faces, which caused the original flickering red texture.
                    assert!(
                        (0..3).any(|axis| fruit.bounds.maximum[axis]
                            < leaf.bounds.minimum[axis] - 0.003
                            || leaf.bounds.maximum[axis] < fruit.bounds.minimum[axis] - 0.003),
                        "fruit face must not coincide with leaf face: {detail:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn cherry_flowers_are_frontmost_from_all_four_sides_without_leaf_intersections() {
        for detail in [PropDetail::Near, PropDetail::Middle] {
            let meshes = meshes(TreeSpecies::Cherry, detail);
            let cubes = cubes(&meshes);
            let flowers = cubes.iter().filter(|cube| gold(cube)).collect::<Vec<_>>();
            assert_eq!(
                flowers.len(),
                if detail == PropDetail::Near { 8 } else { 4 }
            );
            for normal in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let axis = if normal.x != 0. { 0 } else { 2 };
                assert!(
                    flowers.iter().any(|flower| {
                        let center = (flower.bounds.minimum + flower.bounds.maximum) * 0.5;
                        let origin = center + normal * 2.;
                        let frontmost = cubes
                            .iter()
                            .filter_map(|candidate| {
                                if (0..3).any(|a| {
                                    a != axis
                                        && (center[a] < candidate.bounds.minimum[a]
                                            || center[a] > candidate.bounds.maximum[a])
                                }) {
                                    return None;
                                }
                                let distance = if normal[axis] > 0. {
                                    origin[axis] - candidate.bounds.maximum[axis]
                                } else {
                                    candidate.bounds.minimum[axis] - origin[axis]
                                };
                                (distance >= 0.).then_some((distance, candidate))
                            })
                            .min_by(|a, b| a.0.total_cmp(&b.0));
                        frontmost.is_some_and(|(_, cube)| gold(cube))
                    }),
                    "no visible gold flower center from {normal:?} at {detail:?}"
                );
            }
            let leaves = cubes
                .iter()
                .filter(|cube| (0.2..0.55).contains(&cube.color[2]))
                .collect::<Vec<_>>();
            for flower in flowers {
                assert!(
                    leaves
                        .iter()
                        .all(|leaf| !flower.bounds.overlaps(leaf.bounds)),
                    "flower center is buried in canopy: {detail:?}"
                );
            }
        }
    }

    #[test]
    fn every_sapling_cube_has_complete_uvs_outward_winding_and_bounded_all_sided_geometry() {
        for species in TreeSpecies::ALL {
            for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let meshes = meshes(species, detail);
                assert!(
                    meshes
                        .iter()
                        .map(|mesh| mesh.positions.len())
                        .sum::<usize>()
                        <= 1536
                );
                for mesh in meshes {
                    assert_eq!(mesh.material, 7);
                    assert_eq!(mesh.positions.len(), mesh.colors.len());
                    assert_eq!(mesh.positions.len(), mesh.normals.len());
                    assert_eq!(mesh.positions.len(), mesh.uvs.len());
                    assert!(
                        mesh.positions
                            .iter()
                            .flatten()
                            .all(|value| value.is_finite())
                    );
                    assert!(
                        mesh.colors
                            .iter()
                            .flatten()
                            .all(|value| (0.0..=1.0).contains(value))
                    );
                    for face in mesh.uvs.chunks_exact(4) {
                        assert_eq!(face, [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]);
                    }
                    for triangle in mesh.indices.chunks_exact(3) {
                        let [a, b, c] = std::array::from_fn(|index| {
                            Vec3::from_array(mesh.positions[triangle[index] as usize])
                        });
                        let normal = Vec3::from_array(mesh.normals[triangle[0] as usize]);
                        assert!((b - a).cross(c - a).dot(normal) > 0.);
                    }
                    for normal in [
                        Vec3::X,
                        Vec3::NEG_X,
                        Vec3::Y,
                        Vec3::NEG_Y,
                        Vec3::Z,
                        Vec3::NEG_Z,
                    ] {
                        assert!(
                            mesh.normals.contains(&normal.to_array()),
                            "missing textured side {species:?}/{detail:?}/{normal:?}"
                        );
                    }
                }
            }
        }
    }
}
