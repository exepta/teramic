use super::*;
use bevy::mesh::VertexAttributeValues;
#[test]
fn spiders_have_eight_separate_banded_legs_and_body_specific_dimensions() {
    for (kind, expected) in [
        (Kind::SmallSpider, [0.145, 0.1, 0.195]),
        (Kind::MediumSpider, [0.645, 0.5, 0.695]),
        (Kind::LargeSpider, [1.345, 1.25, 1.895]),
    ] {
        let parts = parts(kind).unwrap();
        assert_eq!(parts.len(), 12 + usize::from(kind != Kind::SmallSpider));
        assert_eq!(
            parts
                .iter()
                .filter(|(part, _, _)| matches!(part, Part::SpiderLeg { .. }))
                .count(),
            8
        );
        let total: usize = parts.iter().map(|(_, _, mesh)| mesh.count_vertices()).sum();
        assert!(total < 16_000, "{kind:?}: {total}");
        let Some(VertexAttributeValues::Float32x3(points)) =
            parts[0].2.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("body positions")
        };
        let min = points
            .iter()
            .map(|p| Vec3::from_array(*p))
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let max = points
            .iter()
            .map(|p| Vec3::from_array(*p))
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        for (actual, expected) in (max - min).to_array().into_iter().zip(expected) {
            assert!(
                (actual / expected - 1.0).abs() < 0.005,
                "{kind:?}: {actual} / {expected}"
            );
        }
    }
}

#[test]
fn spider_eye_fronts_are_outside_the_shell_and_unoccluded_by_body_head_or_legs() {
    for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
        let parts = parts(kind).unwrap();
        let (_, offset, eyes) = parts
            .iter()
            .find(|(part, _, _)| matches!(part, Part::SpiderEyes))
            .unwrap();
        let Some(VertexAttributeValues::Float32x3(positions)) =
            eyes.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("eye positions")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            eyes.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("eye normals")
        };
        let length = kind.size()[2] as f32;
        let mut tested = 0;
        for (quad, normal) in positions.chunks_exact(4).zip(normals.chunks_exact(4)) {
            if normal[0][2] > -0.99 {
                continue;
            }
            let target = quad.iter().map(|p| Vec3::from_array(*p)).sum::<Vec3>() * 0.25 + *offset;
            assert!(
                target.z < -length * 0.51,
                "{kind:?}: eye embedded in body shell at {target:?}"
            );
            let origin = target + Vec3::NEG_Z * length;
            for (part, offset, mesh) in &parts {
                if matches!(part, Part::SpiderEyes | Part::Silk | Part::WebShot) {
                    continue;
                }
                let Some(VertexAttributeValues::Float32x3(points)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("occluder positions")
                };
                let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
                for triangle in indices.chunks_exact(3) {
                    let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                        .map(|i| Vec3::from_array(points[i]) + *offset);
                    assert!(
                        !blocks_eye(origin, length, a, b, c),
                        "{kind:?}: forward eye surface occluded by model part"
                    );
                }
            }
            tested += 1;
        }
        // A voxel pupil replaces covered socket cells; its exposed socket
        // border can now consist of several merged rectangles, not one quad.
        assert!(tested >= 4, "all four eyes retain exposed forward surfaces");
        let Some(VertexAttributeValues::Float32x4(colors)) = eyes.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("eye colors")
        };
        let bright = Color::srgb(1.0, 0.65, 0.16).to_linear().to_f32_array();
        assert!(
            colors
                .iter()
                .any(|color| color.iter().zip(bright).all(|(a, b)| (*a - b).abs() < 1e-6)),
            "bright eye highlights must survive voxel meshing"
        );
    }
}

fn blocks_eye(origin: Vec3, distance: f32, a: Vec3, b: Vec3, c: Vec3) -> bool {
    let e1 = b - a;
    let e2 = c - a;
    let cross = Vec3::Z.cross(e2);
    let determinant = e1.dot(cross);
    if determinant.abs() < 1e-10 {
        return false;
    }
    let delta = origin - a;
    let u = delta.dot(cross) / determinant;
    let q = delta.cross(e1);
    let v = Vec3::Z.dot(q) / determinant;
    let t = e2.dot(q) / determinant;
    u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < distance * 0.9999
}

#[test]
fn armored_abdomen_is_solid_and_wider_than_the_thorax_with_eight_bent_legs() {
    for kind in [Kind::SmallSpider, Kind::MediumSpider, Kind::LargeSpider] {
        let parts = parts(kind).unwrap();
        let [w, h, l] = kind.size().map(|v| v as f32);
        let body = &parts[0].2;
        let Some(VertexAttributeValues::Float32x3(p)) = body.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("body positions");
        };
        let ids: Vec<_> = body.indices().unwrap().iter().collect();
        let volume: f32 = ids
            .chunks_exact(3)
            .map(|i| {
                let [a, b, c] = [i[0], i[1], i[2]].map(|i| Vec3::from_array(p[i]));
                a.dot(b.cross(c)) / 6.0
            })
            .sum();
        assert!(
            volume > w * h * l * 0.35,
            "{kind:?}: body must be a solid abdomen, volume {volume}"
        );
        let rear_width = p
            .iter()
            .filter(|p| p[2] > 0.0)
            .map(|p| p[0].abs())
            .fold(0.0_f32, f32::max);
        let front_width = p
            .iter()
            .filter(|p| p[2] < -l * 0.30)
            .map(|p| p[0].abs())
            .fold(0.0_f32, f32::max);
        assert!(
            rear_width > front_width * 1.15,
            "{kind:?}: abdomen silhouette"
        );
        for (part, pivot, mesh) in &parts {
            if let Part::SpiderLeg { side, index } = part {
                assert_eq!(
                    *pivot,
                    Vec3::new(
                        side * w * 0.32,
                        h * 0.71,
                        (f32::from(*index) - 1.5) * l * 0.19
                    )
                );
                let Some(VertexAttributeValues::Float32x3(p)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("leg positions");
                };
                assert!(
                    p.iter()
                        .any(|p| p[0] * side > w * 0.82 && p[1] + pivot.y < h * 0.15),
                    "distal foot"
                );
                assert!(
                    p.iter().any(|p| p[1] > h * 0.16 && p[0] * side > w * 0.40),
                    "raised knee"
                );
                let Some(VertexAttributeValues::Float32x4(c)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("leg colors");
                };
                let band = Color::srgb(ORANGE[0], ORANGE[1], ORANGE[2])
                    .to_linear()
                    .to_f32_array();
                assert!(
                    c.contains(&band),
                    "exposed middle segment between armored joints"
                );
            }
        }
    }
}
