use super::{Part, shape};
use crate::players::model::Builder;
use bevy::prelude::*;
use terra_core::creatures::CreatureKind as Kind;

const DARK: [f32; 3] = [0.075, 0.065, 0.08];
const SHELL: [f32; 3] = [0.18, 0.145, 0.16];
const ORANGE: [f32; 3] = [0.38, 0.19, 0.08];
const RED: [f32; 3] = [0.27, 0.115, 0.06];

pub(super) fn parts(kind: Kind) -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let [w, h, l] = match kind {
        Kind::SmallSpider => [0.145, 0.1, 0.195],
        Kind::MediumSpider => [0.645, 0.5, 0.695],
        Kind::LargeSpider => [1.345, 1.25, 1.895],
        _ => return Err("Not a spider species".to_owned()),
    };
    let mut body = Builder::default();
    // All body voxels occupy the requested dimensions; legs are independently articulated.
    let cell = [w / 9.03, h / 7.03, l / 13.03];
    for z in 0..13 {
        let rear = z >= 5;
        let zt = if rear {
            (z as f32 - 8.5) / 4.5
        } else {
            (z as f32 - 2.0) / 3.2
        };
        let radius = (1.0 - zt * zt * 0.55).max(0.25).sqrt();
        let width = if rear { 4.4 } else { 3.5 } * radius;
        let height = if rear { 3.4 } else { 2.3 } * radius;
        for y in 0..7 {
            for x in 0..9 {
                let dx = x as f32 - 4.0;
                let dy = y as f32 - 3.0;
                if dx * dx / (width * width) + dy * dy / (height * height) > 1.0 {
                    continue;
                }
                // Only the shell is visible: internal micro-voxels are never emitted.
                if dx.abs() + 1.1 < width * 0.65
                    && dy.abs() + 1.1 < height * 0.65
                    && z > 0
                    && z < 12
                {
                    continue;
                }
                let color = if dx.abs() <= 0.6 && y >= 4 {
                    ORANGE
                } else if rear && z % 3 == 0 && y >= 3 && dx.abs() <= width - 0.5 {
                    RED
                } else if (x + y * 3 + z * 5) % 7 == 0 {
                    SHELL
                } else {
                    DARK
                };
                body.voxel(
                    [
                        dx * cell[0],
                        h * 0.85 + dy * cell[1],
                        (z as f32 - 6.0) * cell[2],
                    ],
                    cell.map(|v| v * 1.03),
                    color,
                )?;
            }
        }
    }
    let mut head = Builder::default();
    let mut eyes = Builder::default();
    for i in 0..3 {
        head.voxel(
            [0.0, (i as f32 - 1.0) * h * 0.085, 0.0],
            [w * [0.57, 0.65, 0.52][i], h * 0.12, l * 0.24],
            if i == 2 { SHELL } else { DARK },
        )?;
    }
    for side in [-1.0, 1.0] {
        for i in 0..2 {
            eyes.voxel(
                [
                    side * w * (0.12 + i as f32 * 0.12),
                    h * (0.025 + i as f32 * 0.10),
                    -l * (0.145 - i as f32 * 0.005),
                ],
                [w * 0.09, h * 0.10, l * 0.035],
                [0.95, 0.035, 0.012],
            )?;
            eyes.voxel(
                [
                    side * w * (0.12 + i as f32 * 0.12),
                    h * (0.03 + i as f32 * 0.10),
                    -l * (0.165 - i as f32 * 0.005),
                ],
                [w * 0.045, h * 0.045, l * 0.009],
                [1.0, 0.65, 0.16],
            )?;
        }
        shape::segment(
            &mut head,
            Vec3::new(side * w * 0.20, -h * 0.04, -l * 0.10),
            Vec3::new(side * w * 0.22, -h * 0.25, -l * 0.20),
            w * 0.10,
            w * 0.11,
            RED,
        )?;
        shape::segment(
            &mut head,
            Vec3::new(side * w * 0.22, -h * 0.25, -l * 0.20),
            Vec3::new(side * w * 0.11, -h * 0.30, -l * 0.22),
            w * 0.075,
            w * 0.08,
            DARK,
        )?;
    }
    let mut parts = vec![
        (Part::Body, Vec3::ZERO, body.finish()?),
        (
            Part::Head,
            Vec3::new(0.0, h * 0.66, -l * 0.46),
            head.finish()?,
        ),
        (
            Part::SpiderEyes,
            Vec3::new(0.0, h * 0.66, -l * 0.46),
            eyes.finish()?,
        ),
    ];
    let mut thread = Builder::default();
    thread.voxel(
        [0.0, 0.5, 0.0],
        [w * 0.012, 1.0, w * 0.012],
        [0.88, 0.82, 0.70],
    )?;
    parts.push((Part::Silk, Vec3::ZERO, thread.finish()?));
    for side in [-1.0, 1.0] {
        for index in 0..4 {
            let z = (index as f32 - 1.5) * l * 0.19;
            let origin = Vec3::new(side * w * 0.32, h * 0.71, z);
            let fan = (index as f32 - 1.5) * l * 0.20;
            let knee = Vec3::new(side * w * 0.49, h * (0.18 + index as f32 * 0.018), fan);
            let foot = Vec3::new(side * w * 0.87, -origin.y, fan * 1.60);
            let mut leg = Builder::default();
            for (a, b, thickness) in [(Vec3::ZERO, knee, 0.13), (knee, foot, 0.12)] {
                let d = b - a;
                for step in 0..4 {
                    let p = a + d * (step as f32 / 4.0);
                    let q = a + d * ((step + 1) as f32 / 4.0);
                    let color = if step == 1 || step == 2 { ORANGE } else { DARK };
                    shape::segment(&mut leg, p, q, w * thickness, w * thickness * 0.94, color)?;
                    if step == 1 {
                        shape::segment(
                            &mut leg,
                            p,
                            p + d * 0.10,
                            w * (thickness + 0.045),
                            w * (thickness + 0.045),
                            SHELL,
                        )?;
                    }
                }
            }
            leg.voxel(knee.to_array(), [w * 0.18; 3], SHELL)?;
            parts.push((
                Part::SpiderLeg {
                    side,
                    index: index as u8,
                },
                origin,
                leg.finish()?,
            ));
        }
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
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
            assert_eq!(parts.len(), 12);
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
                let target =
                    quad.iter().map(|p| Vec3::from_array(*p)).sum::<Vec3>() * 0.25 + *offset;
                assert!(
                    target.z < -length * 0.51,
                    "{kind:?}: eye embedded in body shell at {target:?}"
                );
                let origin = target + Vec3::NEG_Z * length;
                for (part, offset, mesh) in &parts {
                    if matches!(part, Part::SpiderEyes | Part::Silk) {
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
            assert_eq!(tested, 8, "four eye sockets and four bright pupils");
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
}
