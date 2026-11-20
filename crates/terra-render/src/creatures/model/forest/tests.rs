use super::*;
use bevy::mesh::VertexAttributeValues;

#[test]
fn thorn_guardian_has_visible_heart_eyes_antlers_and_bounded_closed_part_geometry() {
    let parts = brute().unwrap();
    assert_eq!(parts.len(), 8);
    let count: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
    eprintln!(
        "Thorn guardian: {count} vertices in {} shared parts",
        parts.len()
    );
    assert!(
        count < 16_000,
        "guardian part meshes exceed shared geometry budget: {count}"
    );
    let find = |select: fn(Part) -> bool| parts.iter().find(|(part, _, _)| select(*part)).unwrap();
    let (_, head_pivot, head) = find(|p| matches!(p, Part::Head));
    let (_, eyes_pivot, eyes) = find(|p| matches!(p, Part::ThornGlow { head: true }));
    assert_eq!(head_pivot, eyes_pivot);
    for side in [-1.0, 1.0] {
        let ray = Vec3::new(side * 0.115, 0.025, -1.0);
        assert!(first_hit(eyes, ray, Vec3::Z) < first_hit(head, ray, Vec3::Z));
    }
    let (_, _, body) = find(|p| matches!(p, Part::Body));
    let (_, _, heart) = find(|p| matches!(p, Part::ThornGlow { head: false }));
    let ray = Vec3::new(0.0, 1.35, -1.0);
    assert!(first_hit(heart, ray, Vec3::Z) < first_hit(body, ray, Vec3::Z));
    for (part, pivot, mesh) in &parts {
        let Some(VertexAttributeValues::Float32x3(points)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals")
        };
        assert!(points.iter().flatten().all(|p| p.is_finite()));
        let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(points[i]));
            let area = (b - a).cross(c - a);
            assert!(
                area.length_squared() > 1e-18 && area.dot(Vec3::from_array(normals[tri[0]])) > 0.0
            );
        }
        if matches!(part, Part::Head) {
            assert!(
                points
                    .iter()
                    .any(|p| p[0].abs() > 0.4 && p[1] + pivot.y > 2.3),
                "real antler silhouette"
            );
        }
        if matches!(part, Part::Leg(_)) {
            let low = points
                .iter()
                .map(|p| p[1] + pivot.y)
                .fold(f32::INFINITY, f32::min);
            assert!(low.abs() < 0.01, "root feet remain on the ground: {low}");
        }
    }
}

#[test]
fn skeleton_variants_have_open_ribcages_equipment_and_separate_gait_parts() {
    let mut silhouettes = std::collections::BTreeSet::new();
    for variant in 0..3 {
        let parts = skeleton(variant).unwrap();
        assert_eq!(parts.len(), 7);
        assert_eq!(
            parts
                .iter()
                .filter(|(p, _, _)| matches!(p, Part::Leg(_)))
                .count(),
            2
        );
        assert_eq!(
            parts
                .iter()
                .filter(|(p, _, _)| matches!(p, Part::VillagerArm(_)))
                .count(),
            2
        );
        let count: usize = parts.iter().map(|(_, _, m)| m.count_vertices()).sum();
        eprintln!(
            "Skeleton loadout {variant}: {count} vertices in {} shared parts",
            parts.len()
        );
        assert!(
            count < 16_000,
            "skeleton {variant}: {count}; parts: {:?}",
            parts
                .iter()
                .map(|(part, _, mesh)| (part, mesh.count_vertices()))
                .collect::<Vec<_>>()
        );
        silhouettes.insert(
            parts
                .iter()
                .flat_map(|(_, _, mesh)| {
                    let Some(VertexAttributeValues::Float32x3(p)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    else {
                        panic!("positions")
                    };
                    p.iter().flatten().map(|v| v.to_bits())
                })
                .collect::<Vec<_>>(),
        );
        for (part, offset, mesh) in &parts {
            let Some(VertexAttributeValues::Float32x3(p)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("positions")
            };
            if matches!(part, Part::Leg(_)) {
                let minimum = p
                    .iter()
                    .map(|v| v[1] + offset.y)
                    .fold(f32::INFINITY, f32::min);
                assert!(minimum.abs() < 0.001, "grounded skeleton toes {minimum}");
            }
            if matches!(part, Part::Body) && variant < 2 {
                let ray_origin = Vec3::new(0.14, 1.158, 1.0);
                let ray_direction = -Vec3::Z;
                let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
                let hit = indices.chunks_exact(3).any(|t| {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from_array(p[i]));
                    let e1 = b - a;
                    let e2 = c - a;
                    let h = ray_direction.cross(e2);
                    let det = e1.dot(h);
                    if det.abs() < 1e-7 {
                        return false;
                    }
                    let inv = 1.0 / det;
                    let delta = ray_origin - a;
                    let u = delta.dot(h) * inv;
                    let q = delta.cross(e1);
                    let v = ray_direction.dot(q) * inv;
                    (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && e2.dot(q) * inv > 0.0
                });
                assert!(!hit, "rib gap must be open geometry, not a painted stripe");
            }
            if matches!(part, Part::VillagerArm(side) if *side < 0.0) {
                let front = p.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min);
                assert_eq!(
                    front < -0.90,
                    variant > 0,
                    "Only the fighter and guardian carry a real sword"
                );
            }
        }
    }
    assert_eq!(
        silhouettes.len(),
        3,
        "Armor and weapons retain distinct geometry after hidden surfaces are removed"
    );
}

#[test]
fn forest_silhouettes_have_real_meshes_and_bounded_articulation() {
    for (kind, parts) in [
        ("Raven", raven().unwrap()),
        ("BloodSlime", slime().unwrap()),
        ("BloodStalker", stalker().unwrap()),
        ("ThornBrute", brute().unwrap()),
        ("BloodWisp", wisp().unwrap()),
    ] {
        let count: usize = parts.iter().map(|(_, _, m)| m.count_vertices()).sum();
        assert!((100..16_000).contains(&count), "{kind} forest mesh {count}");
        for (_, _, mesh) in parts {
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
    }
    assert_eq!(
        stalker()
            .unwrap()
            .iter()
            .filter(|(p, _, _)| matches!(p, Part::SpiderLeg { .. }))
            .count(),
        8
    );
    assert_eq!(
        raven()
            .unwrap()
            .iter()
            .filter(|(p, _, _)| matches!(p, Part::Wing(_)))
            .count(),
        2
    );
    assert_eq!(
        wisp()
            .unwrap()
            .iter()
            .filter(|(p, _, _)| matches!(p, Part::SpiritOrbit(_)))
            .count(),
        2
    );
}

fn first_hit(mesh: &Mesh, origin: Vec3, direction: Vec3) -> f32 {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions")
    };
    let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
    indices
        .chunks_exact(3)
        .filter_map(|triangle| {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|index| Vec3::from_array(positions[index]));
            let e1 = b - a;
            let e2 = c - a;
            let h = direction.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-7 {
                return None;
            }
            let delta = origin - a;
            let u = delta.dot(h) / det;
            let q = delta.cross(e1);
            let v = direction.dot(q) / det;
            let distance = e2.dot(q) / det;
            ((0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && distance > 0.0)
                .then_some(distance)
        })
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn all_skeleton_loadouts_have_red_pupils_visible_inside_recessed_sockets() {
    for variant in 0..3 {
        let parts = skeleton(variant).unwrap();
        let (_, head_pivot, head) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::Head))
            .unwrap();
        let (_, eye_pivot, eyes) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::SkeletonEyes))
            .unwrap();
        assert_eq!(head_pivot, eye_pivot);
        let Some(VertexAttributeValues::Float32x4(colors)) = eyes.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("pupil colors")
        };
        // Mesh colors are linear, whereas authoring colors are sRGB. Keep the
        // exact saturated red palette (including its small red highlight).
        let palette = [[0.95, 0.008, 0.012], [1.0, 0.05, 0.04]]
            .map(|rgb| Color::srgb_from_array(rgb).to_linear().to_f32_array());
        for expected in palette {
            assert!(
                colors.iter().any(|actual| {
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, e)| (a - e).abs() < 1e-6)
                }),
                "both pupil and highlight colors must survive meshing"
            );
        }
        assert!(colors.iter().all(|actual| {
            actual[3] == 1.0
                && palette.iter().any(|expected| {
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, e)| (a - e).abs() < 1e-6)
                })
        }));
        for side in [-1.0, 1.0] {
            for dx in [-0.01, 0.0, 0.01] {
                for dy in [-0.012, 0.0, 0.012] {
                    let origin = Vec3::new(side * 0.085 + dx, -0.045 + dy, -1.0);
                    let eye_hit = first_hit(eyes, origin, Vec3::Z);
                    let skull_hit = first_hit(head, origin, Vec3::Z);
                    assert!(eye_hit.is_finite(), "missing pupil in loadout {variant}");
                    assert!(
                        eye_hit + 0.01 < skull_hit,
                        "skull or helmet occludes pupil in loadout {variant}"
                    );
                }
            }
        }
    }
}

#[test]
fn blood_slime_face_is_a_separate_opaque_mesh_in_front_of_the_jelly_surface() {
    let parts = slime().unwrap();
    assert_eq!(parts.len(), 2);
    let (_, body_pivot, body) = parts
        .iter()
        .find(|(p, _, _)| matches!(p, Part::Body))
        .unwrap();
    let (_, face_pivot, face) = parts
        .iter()
        .find(|(p, _, _)| matches!(p, Part::Head))
        .unwrap();
    assert_eq!(body_pivot, face_pivot);
    for (_, _, mesh) in &parts {
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("colors")
        };
        assert!(
            colors.iter().all(|c| c[3] == 1.0),
            "opacity belongs to the body material only"
        );
    }
    for side in [-1.0, 1.0] {
        let origin = Vec3::new(side * 0.15, 0.31, -1.0);
        assert!(first_hit(face, origin, Vec3::Z) < first_hit(body, origin, Vec3::Z));
    }
}

#[test]
fn faceted_ellipsoid_is_closed_outward_and_has_no_collapsed_caps() {
    let mut b = Builder::default();
    ellipsoid(&mut b, Vec3::ZERO, Vec3::new(0.3, 0.2, 0.4), [1.0; 3]).unwrap();
    let mesh = b.finish().unwrap();
    let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        panic!()
    };
    let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from_array(p[i]));
        assert!((b - a).cross(c - a).dot((a + b + c) / 3.0) > 0.0);
    }
}
