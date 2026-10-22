use crate::players::model::Builder;
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

mod aquatic;
mod bear;
mod bee;
mod clownfish;
mod horse;
mod mammal;
mod shape;

#[derive(Clone, Copy)]
pub(super) enum Part {
    Body,
    Head,
    Leg(f32),
    Tail,
    Wing(f32),
    Proboscis,
    Eyelids,
    Bell,
    Tentacles(f32),
    Fin(f32),
    Jaw,
}

pub(super) fn parts(kind: Kind) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    parts_variant(kind, 0)
}
pub(super) fn parts_variant(kind: Kind, variant: u8) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    if kind == Kind::Fish && variant >= 2 {
        return river_fish();
    }
    match kind {
        Kind::Bee => bee::parts(),
        Kind::Fish => clownfish::parts(),
        Kind::Cow | Kind::Pig | Kind::Sheep => mammal::parts(kind),
        Kind::Bear => bear::parts(),
        Kind::Horse => horse::parts(variant),
        Kind::Squid => aquatic::squid(),
        Kind::MoonJelly
        | Kind::FireJelly
        | Kind::VoidJelly
        | Kind::SunJelly
        | Kind::CrystalJelly => aquatic::jelly(kind),
        Kind::ReefShark | Kind::HammerheadShark | Kind::WhaleShark => aquatic::shark(kind),
    }
}

fn river_fish() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut body = Builder::default();
    let teal = [0.035, 0.60, 0.67];
    let gold = [0.98, 0.65, 0.15];
    let cream = [0.96, 0.91, 0.72];
    for z in 0..9 {
        let t = z as f32 / 8.0;
        let w = 0.13 + (std::f32::consts::PI * t).sin() * 0.12;
        let h = 0.13 + (std::f32::consts::PI * t).sin() * 0.15;
        body.voxel([0.0, 0.16, -0.24 + t * 0.48], [w, h, 0.065], teal)?;
        body.voxel(
            [0.0, 0.16 - h * 0.35, -0.24 + t * 0.48],
            [w + 0.006, h * 0.3, 0.066],
            cream,
        )?;
        if z > 1 && z < 7 {
            let rise = 0.045 + (1.0 - t) * 0.12;
            body.voxel(
                [0.0, 0.16 + h * 0.5 + rise * 0.5, -0.24 + t * 0.48],
                [0.028, rise, 0.065],
                if z % 3 == 0 { gold } else { teal },
            )?;
        }
    }
    for side in [-1.0, 1.0] {
        body.voxel([side * 0.10, 0.20, -0.205], [0.035, 0.10, 0.075], cream)?;
        body.voxel(
            [side * 0.121, 0.20, -0.214],
            [0.015, 0.070, 0.045],
            [0.025, 0.035, 0.04],
        )?;
        body.voxel([side * 0.13, 0.09, -0.04], [0.13, 0.026, 0.13], gold)?;
        for i in 0..3 {
            body.voxel(
                [side * 0.10, 0.22, -0.08 + i as f32 * 0.07],
                [0.02, 0.025, 0.024],
                gold,
            )?;
        }
    }
    let mut tail = Builder::default();
    for i in 0..4 {
        tail.voxel(
            [0.0, 0.0, 0.035 + i as f32 * 0.045],
            [0.035, 0.11 + i as f32 * 0.07, 0.05],
            if i >= 2 { gold } else { teal },
        )?;
    }
    Ok(vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (Part::Tail, Vec3::new(0.0, 0.16, 0.25), tail.finish()?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    #[test]
    fn horse_coats_have_sixteen_distinct_shared_indexed_models_and_grounded_hooves() {
        let mut coats = std::collections::HashSet::new();
        for variant in 0..16 {
            let parts = parts_variant(Kind::Horse, variant).unwrap();
            assert_eq!(parts.len(), 8);
            let mut fingerprint = 0_u64;
            let mut vertices = 0;
            for (part, offset, mesh) in parts {
                vertices += mesh.count_vertices();
                assert!(
                    mesh.indices()
                        .unwrap()
                        .iter()
                        .all(|i| i < mesh.count_vertices())
                );
                if let Some(VertexAttributeValues::Float32x4(colors)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                {
                    for color in colors {
                        for value in color {
                            fingerprint = fingerprint.wrapping_mul(1099511628211)
                                ^ u64::from(value.to_bits());
                        }
                    }
                }
                if matches!(part, Part::Leg(_))
                    && let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    let bottom = positions
                        .iter()
                        .map(|v| v[1] + offset.y)
                        .fold(f32::INFINITY, f32::min);
                    assert!(bottom.abs() < 0.001);
                }
                if matches!(part, Part::Body)
                    && let Some(VertexAttributeValues::Float32x3(positions)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    let width = positions
                        .iter()
                        .map(|p| p[0])
                        .fold(f32::NEG_INFINITY, f32::max)
                        - positions.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                    assert!((0.50..0.53).contains(&width), "narrow barrel: {width}");
                }
            }
            assert!(vertices < 16000);
            assert!(coats.insert(fingerprint), "duplicate coat {variant}");
        }
    }

    #[test]
    fn all_species_have_bounded_shared_indexed_models() {
        for kind in Kind::ALL {
            let parts = parts(kind).unwrap();
            assert!(parts.len() <= 8);
            let mut total = 0;
            let mut triangles = 0;
            for (_, offset, mesh) in parts {
                let count = mesh.count_vertices();
                total += count;
                assert!(mesh.indices().unwrap().iter().all(|i| i < count));
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing positions");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("Missing normals");
                };
                for (position, normal) in positions.iter().zip(normals) {
                    let position = Vec3::from_array(*position) + offset;
                    let normal = Vec3::from_array(*normal);
                    assert!(position.is_finite() && normal.is_finite(), "{kind:?}");
                    assert!(position.abs().max_element() < 2.5, "{kind:?}: {position}");
                    assert!(
                        (normal.length_squared() - 1.0).abs() < 0.001,
                        "{kind:?}: {normal}"
                    );
                }
                let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
                triangles += indices.len() / 3;
                for triangle in indices.chunks_exact(3) {
                    let a = Vec3::from_array(positions[triangle[0]]);
                    let b = Vec3::from_array(positions[triangle[1]]);
                    let c = Vec3::from_array(positions[triangle[2]]);
                    assert!(
                        (b - a).cross(c - a).length_squared() > 1e-16,
                        "{kind:?}: degenerate triangle"
                    );
                }
            }
            assert!(total < 16_000, "{kind:?}: {total}");
            println!("{kind:?}: {total} vertices, {triangles} triangles");
        }
    }
    #[test]
    fn riverfin_variant_has_valid_shared_geometry() {
        let fish = parts_variant(Kind::Fish, 2).unwrap();
        assert_eq!(fish.len(), 2);
        let total: usize = fish.iter().map(|(_, _, m)| m.count_vertices()).sum();
        // The established river fish is deliberately unchanged, including its geometry budget.
        assert_eq!(total, 936);
        assert_eq!(
            fish.iter()
                .map(|(_, _, m)| m.indices().unwrap().len() / 3)
                .sum::<usize>(),
            468
        );
        for (_, _, mesh) in fish {
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
    }

    #[test]
    fn aquatic_models_use_meter_bounds_and_a_small_fixed_articulation_budget() {
        for kind in Kind::ALL
            .into_iter()
            .filter(|kind| kind.is_aquatic() && *kind != Kind::Fish)
        {
            let parts = parts(kind).unwrap();
            assert_eq!(parts.len(), 5, "{kind:?}: bounded shared articulation");
            assert_eq!(
                parts
                    .iter()
                    .filter(|(part, _, _)| matches!(part, Part::Bell))
                    .count(),
                usize::from(kind.is_jelly())
            );
            let size = Vec3::from_array(kind.size().map(|v| v as f32));
            for (_, origin, mesh) in parts {
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("Missing positions")
                };
                for vertex in positions {
                    let point = Vec3::from_array(*vertex) + origin;
                    assert!(
                        point.x.abs() <= size.x * 0.55,
                        "{kind:?}: width {point:?}/{size:?}"
                    );
                    assert!(
                        point.y >= -0.005 && point.y <= size.y * 1.025,
                        "{kind:?}: height {point:?}/{size:?}"
                    );
                    assert!(
                        point.z.abs() <= size.z * 0.55,
                        "{kind:?}: length {point:?}/{size:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn bear_coat_claws_and_face_share_eight_parts_with_grounded_paws() {
        let parts = parts(Kind::Bear).unwrap();
        assert_eq!(parts.len(), 8);
        assert_eq!(
            parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::Leg(_)))
                .count(),
            4
        );
        for (part, offset, mesh) in &parts {
            if !matches!(part, Part::Leg(_)) {
                continue;
            }
            let Some(VertexAttributeValues::Float32x3(vertices)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("Bear paw positions missing");
            };
            let bottom = vertices
                .iter()
                .map(|p| p[1] + offset.y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                bottom.abs() < 0.001,
                "Bear paw must rest on the authoritative ground plane: {bottom}"
            );
            assert!(
                vertices.iter().any(|p| p[2] < -0.28),
                "Claws should project ahead of the broad paw"
            );
        }
        let vertices: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
        assert!((10_000..16_000).contains(&vertices), "{vertices}");
    }

    #[test]
    fn bevels_are_closed_and_outward_facing() {
        let mut builder = Builder::default();
        shape::rounded(&mut builder, [0.0; 3], [0.8, 0.7, 1.2], 0.12, [0.8; 3]).unwrap();
        let mesh = builder.finish().unwrap();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!()
        };
        assert_eq!(positions.len(), 120);
        for (positions, normals) in positions.chunks_exact(4).zip(normals.chunks_exact(4)) {
            let center = positions.iter().map(|p| Vec3::from_array(*p)).sum::<Vec3>() / 4.0;
            assert!(center.dot(Vec3::from_array(normals[0])) > 0.0);
        }
    }
}
