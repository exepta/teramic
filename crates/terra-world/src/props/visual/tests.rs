use super::geometry::add_stalk;
use super::{PropDetail, PropPalette, template};
use crate::props::{PropId, PropKind, PropPose, WorldProp, raycast_props_with_pose};
use bevy_math::{Vec2, Vec3};
use terra_voxel::SectionPos;

#[test]
fn reference_botany_has_square_voxel_surfaces_and_bounded_shared_geometry() {
    for (kind, budget) in [
        (PropKind::Grass, 1168),
        (PropKind::Bush, 2112),
        (PropKind::Mushroom, 1128),
        (PropKind::Flower, 1836),
    ] {
        for variant in 0..8 {
            let meshes =
                template(kind, variant, PropDetail::Near, &PropPalette::default()).unwrap();
            assert!(
                meshes
                    .iter()
                    .map(|mesh| mesh.positions.len())
                    .sum::<usize>()
                    <= budget
            );
            for mesh in meshes {
                for normal in &mesh.normals {
                    assert_eq!(
                        normal.iter().filter(|v| v.abs() > 0.0001).count(),
                        1,
                        "{kind:?}/{variant}: square voxel surfaces, not diagonal ribbons: {normal:?}"
                    );
                }
                assert!(
                    mesh.indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.positions.len())
                );
            }
        }
    }
}

#[test]
fn attached_tree_fungi_keep_three_shelf_silhouettes_and_precise_pick_surfaces() {
    use crate::props::raycast_props;
    use bevy_math::Quat;
    let palette = PropPalette {
        flowers: [1, 2, 3, 4, 5, 6],
        mushroom: 7,
        ..Default::default()
    };
    for variant in 128..131 {
        let near = template(PropKind::Mushroom, variant, PropDetail::Near, &palette).unwrap();
        let middle = template(PropKind::Mushroom, variant, PropDetail::Middle, &palette).unwrap();
        let far = template(PropKind::Mushroom, variant, PropDetail::Far, &palette).unwrap();
        let vertices =
            |parts: &[super::PropMesh]| parts.iter().map(|p| p.positions.len()).sum::<usize>();
        assert!(vertices(&near) > vertices(&middle));
        assert!(vertices(&middle) > vertices(&far));
        assert!(vertices(&near) <= 432);
        let cap = [1, 7, 3][usize::from(variant - 128)];
        assert!(near.iter().any(|mesh| mesh.material == cap));
        let prop = WorldProp {
            id: PropId(u64::from(variant)),
            kind: PropKind::Mushroom,
            position: [0.14, 4.20, -0.5],
            rotation: std::f32::consts::FRAC_PI_2,
            scale: 0.4,
            variant,
            section: SectionPos::new(-1, 0, -1),
            removed: false,
        };
        assert!(prop.valid());
        assert_eq!(crate::props::placement::support(&prop), [-1, 4, -1]);
        for mesh in &near {
            for triangle in mesh.indices.chunks_exact(3).step_by(13) {
                let rotation = Quat::from_rotation_y(prop.rotation);
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| {
                    rotation * Vec3::from_array(mesh.positions[i as usize]) * prop.visual_scale()
                        + Vec3::from_array(prop.position.map(|v| v as f32))
                });
                let normal = (b - a).cross(c - a).normalize();
                let origin = (a + b + c) / 3.0 + normal * 0.04;
                assert!(
                    raycast_props(
                        origin.to_array().map(f64::from),
                        (-normal).to_array().map(f64::from),
                        0.1,
                        [&prop]
                    )
                    .is_some()
                );
            }
        }
    }
}
#[test]
fn grass_ends_are_square_and_keep_the_full_blade_cross_section() {
    for yaw in [0.0, 0.4, 1.9, 4.3] {
        let mut batches = std::collections::BTreeMap::new();
        add_stalk(
            &mut batches,
            0,
            Vec3::ZERO,
            Vec2::splat(0.10),
            &[
                Vec3::ZERO,
                Vec3::new(0.01, 0.2, 0.0),
                Vec3::new(0.10, 0.4, 0.0),
            ],
            yaw,
        )
        .unwrap();
        let vertices = &batches[&0].positions;
        assert_eq!(vertices.len(), 36);
        let cap = &vertices[32..36];
        assert!(cap.iter().all(|point| (point[1] - 0.4).abs() < 1e-6));
        let edges = [(0, 1), (1, 2)]
            .map(|(a, b)| (Vec3::from_array(cap[a]) - Vec3::from_array(cap[b])).length());
        assert!((edges[0].max(edges[1]) - 0.10).abs() < 1e-6);
        assert!((edges[0].min(edges[1]) - 0.10).abs() < 1e-6);
        for edge in 0..4 {
            assert_eq!(vertices[edge * 4 + 1], vertices[16 + edge * 4]);
            assert_eq!(vertices[edge * 4 + 2], vertices[16 + edge * 4 + 3]);
        }
    }
}

#[test]
fn grass_templates_have_valid_bounded_meshes_and_leave_companion_corners_clear() {
    let palette = PropPalette {
        foliage: [0, 1, 2],
        stem: 3,
        dry_grass: [4, 5, 6],
        flowers: [7, 8, 9, 10, 11, 12],
        ..Default::default()
    };
    for variant in (0..8).chain(128..136) {
        for (detail, budget) in [
            (PropDetail::Near, 1168),
            (PropDetail::Middle, 160),
            (PropDetail::Far, 32),
        ] {
            let meshes = template(PropKind::Grass, variant, detail, &palette).unwrap();
            assert!(meshes.iter().map(|m| m.positions.len()).sum::<usize>() <= budget);
            for mesh in meshes {
                assert_eq!(mesh.positions.len(), mesh.normals.len());
                assert_eq!(mesh.positions.len(), mesh.uvs.len());
                assert!(mesh.colors.is_empty() || mesh.positions.len() == mesh.colors.len());
                assert_eq!(mesh.indices.len() % 3, 0);
                assert!(
                    mesh.indices
                        .iter()
                        .all(|&i| (i as usize) < mesh.positions.len())
                );
                assert!(
                    mesh.normals
                        .iter()
                        .all(|n| Vec3::from_array(*n).is_normalized())
                );
                for [x, y, z] in mesh.positions {
                    assert!(x.is_finite() && y.is_finite() && z.is_finite());
                    assert!(x.hypot(z) <= 0.4, "rotated broad-phase bounds: {variant}");
                    assert!((0.0..=0.65).contains(&y));
                    assert!(
                        !(x.abs() > 0.125 && z.abs() > 0.125),
                        "companion corner occupied: {variant}, {x}, {z}"
                    );
                }
                assert!(mesh.colors.iter().all(|c| c[3] == 1.0));
            }
        }
    }
}

#[test]
fn centered_grass_spans_approximately_eighty_percent_of_the_soil_footprint() {
    for variant in 0..8 {
        let meshes = template(
            PropKind::Grass,
            variant,
            PropDetail::Near,
            &PropPalette::default(),
        )
        .unwrap();
        let mut minimum = Vec3::splat(f32::INFINITY);
        let mut maximum = Vec3::splat(f32::NEG_INFINITY);
        for point in meshes.iter().flat_map(|m| &m.positions) {
            minimum = minimum.min(Vec3::from_array(*point));
            maximum = maximum.max(Vec3::from_array(*point));
        }
        let width = (maximum - minimum) * 1.3;
        assert!(
            (0.84..0.96).contains(&width.x),
            "variant {variant}: {width:?}"
        );
        assert!(
            (0.84..0.96).contains(&width.z),
            "variant {variant}: {width:?}"
        );
        assert!((0.74..0.88).contains(&(width.x * width.z)));
    }
}

#[test]
fn grass_has_real_pickable_stalks_and_open_gaps_even_in_wind() {
    for variant in 0..8 {
        for turn in 0..4 {
            let prop = WorldProp {
                id: PropId(1),
                kind: PropKind::Grass,
                position: [0.5, 1.0, 0.5],
                rotation: turn as f32 * std::f32::consts::FRAC_PI_2,
                scale: 1.0,
                variant,
                section: SectionPos::default(),
                removed: false,
            };
            let (sin, cos) = f64::from(prop.rotation).sin_cos();
            for pose in [PropPose::Rest, PropPose::Wind(0.7), PropPose::Swept] {
                let ray = |x: f64, z: f64| {
                    raycast_props_with_pose(
                        [0.5 + cos * x + sin * z, 2.0, 0.5 - sin * x + cos * z],
                        [0.0, -1.0, 0.0],
                        2.0,
                        [&prop],
                        pose,
                    )
                };
                assert!(ray(0.28, 0.28).is_none());
                assert!(ray(-0.28, -0.28).is_none());
                assert!(ray(-0.28, 0.28).is_none());
                assert!(ray(0.28, -0.28).is_none());
                let hits = (-4..=4)
                    .flat_map(|x| (-4..=4).map(move |z| (x, z)))
                    .filter(|&(x, z)| ray(f64::from(x) * 0.08, f64::from(z) * 0.08).is_some())
                    .count();
                assert!(
                    hits >= 8,
                    "stalks must remain selectable: {variant}, {turn}"
                );
                assert!(hits <= 60, "air gaps must not become solid picking boxes");
            }
        }
    }
}

#[test]
fn small_props_fit_quarter_block_footprints_and_requested_flower_mushroom_heights() {
    for (kind, minimum_scale, maximum_scale) in [
        (PropKind::Flower, 0.65, 0.95),
        (PropKind::Mushroom, 0.35, 0.55),
        (PropKind::Bush, 1.3, 1.3),
        (PropKind::Fern, 1.3, 1.3),
        (PropKind::SmallPlant, 1.3, 1.3),
    ] {
        for variant in (0..8).chain(128..136) {
            for detail in [PropDetail::Near, PropDetail::Middle, PropDetail::Far] {
                let meshes = template(kind, variant, detail, &PropPalette::default()).unwrap();
                let mut minimum = Vec3::splat(f32::INFINITY);
                let mut maximum = Vec3::splat(f32::NEG_INFINITY);
                for mesh in &meshes {
                    assert!(
                        mesh.normals
                            .iter()
                            .all(|n| Vec3::from_array(*n).is_normalized())
                    );
                    assert!(
                        mesh.indices
                            .iter()
                            .all(|&i| (i as usize) < mesh.positions.len())
                    );
                    for point in &mesh.positions {
                        let point = Vec3::from_array(*point);
                        assert!(
                            point.x.abs() * maximum_scale <= 0.25001
                                && point.z.abs() * maximum_scale <= 0.25001,
                            "{kind:?} {variant} {detail:?}: off-center footprint {point:?}"
                        );
                        minimum = minimum.min(point);
                        maximum = maximum.max(point);
                    }
                }
                let width = (maximum - minimum) * maximum_scale;
                assert!(
                    width.x <= 0.50001 && width.z <= 0.50001,
                    "{kind:?} {variant} {detail:?}: {width:?}"
                );
                if matches!(kind, PropKind::Flower | PropKind::Mushroom) {
                    assert!(
                        maximum.y <= 1.00001,
                        "{kind:?} {variant} {detail:?}: {}",
                        maximum.y
                    );
                    if detail == PropDetail::Near {
                        assert!((maximum.y - 1.0).abs() < 1e-6);
                        assert!((maximum.y * minimum_scale - minimum_scale).abs() < 1e-6);
                        assert!((maximum.y * maximum_scale - maximum_scale).abs() < 1e-6);
                    }
                }
                for angle in [0.0_f32, 0.3, std::f32::consts::FRAC_PI_2, 2.9] {
                    let prop = WorldProp {
                        id: PropId(1),
                        kind,
                        position: [-17.5, 16.0, -0.5],
                        rotation: angle,
                        scale: maximum_scale / 1.3,
                        variant,
                        section: SectionPos::new(-2, 0, -1),
                        removed: false,
                    };
                    let [minimum, maximum] = prop.bounds();
                    let (sin, cos) = f64::from(angle).sin_cos();
                    for point in meshes.iter().flat_map(|m| &m.positions) {
                        let [x, y, z] =
                            point.map(|v| f64::from(v) * f64::from(prop.visual_scale()));
                        let world = [
                            prop.position[0] + cos * x + sin * z,
                            prop.position[1] + y,
                            prop.position[2] - sin * x + cos * z,
                        ];
                        assert!(
                            (0..3).all(|axis| world[axis] >= minimum[axis] - 1e-6
                                && world[axis] <= maximum[axis] + 1e-6),
                            "{kind:?} {variant} {detail:?}: {world:?} outside {:?}",
                            prop.bounds()
                        );
                    }
                }
            }
        }
    }
}
