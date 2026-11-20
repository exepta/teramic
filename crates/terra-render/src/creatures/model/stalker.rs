//! Shared layered bark meshes: no per-voxel entities or runtime mesh rebuilding.
use super::{Builder, Part};
use bevy::prelude::*;

const BARK: [[f32; 3]; 5] = [
    [0.26, 0.18, 0.12],
    [0.34, 0.24, 0.15],
    [0.43, 0.31, 0.20],
    [0.50, 0.37, 0.24],
    [0.19, 0.15, 0.11],
];

fn hash(seed: u32) -> u32 {
    let x = (seed ^ (seed >> 16)).wrapping_mul(0x7feb_352d);
    (x ^ (x >> 15)).wrapping_mul(0x846c_a68b)
}
fn bark(b: &mut Builder, start: Vec3, end: Vec3, radius: f32, seed: u32) -> Result<(), String> {
    let delta = end - start;
    let direction = delta.normalize();
    let rotation = Quat::from_rotation_arc(Vec3::Y, direction);
    let rows = (delta.length() / 0.085).ceil() as u32;
    let row_pitch = delta.length() / rows as f32;
    b.rotated(
        ((start + end) * 0.5).to_array(),
        [radius * 1.35, delta.length(), radius * 1.35],
        BARK[4],
        rotation,
    )?;
    for row in 0..rows {
        for side in 0..8 {
            let h = hash(seed ^ row.wrapping_mul(131) ^ (side * 7919));
            let angle = side as f32 * std::f32::consts::FRAC_PI_4;
            let r = radius * (0.86 + (h & 255) as f32 / 255.0 * 0.22);
            let local = Vec3::new(
                angle.cos() * r,
                ((row as f32 + 0.5) / rows as f32 - 0.5) * delta.length(),
                angle.sin() * r,
            );
            let color = if h.is_multiple_of(23) {
                [0.26, 0.32, 0.10]
            } else {
                BARK[(h as usize / 13) % BARK.len()]
            };
            b.rotated(
                ((start + end) * 0.5 + rotation * local).to_array(),
                [
                    radius * 0.70,
                    row_pitch * (0.82 + (h % 5) as f32 * 0.02),
                    // Keep neighboring 45-degree bark sectors disjoint even
                    // at their smallest inner radius; their caps share a plane.
                    radius * 0.36,
                ],
                color,
                rotation * Quat::from_rotation_y(-angle),
            )?;
        }
    }
    Ok(())
}
fn moss(b: &mut Builder, p: Vec3, length: f32, seed: u32) -> Result<(), String> {
    for strand in 0..3 {
        for j in 0..(length / 0.075) as u32 {
            let h = hash(seed ^ (strand * 83) ^ (j * 11));
            b.voxel(
                (p + Vec3::new(
                    strand as f32 * 0.035,
                    j as f32 * -0.075,
                    (h % 5) as f32 * 0.009,
                ))
                .to_array(),
                [0.033, 0.064, 0.035],
                if h.is_multiple_of(3) {
                    [0.34, 0.36, 0.10]
                } else {
                    [0.22, 0.27, 0.075]
                },
            )?;
        }
    }
    Ok(())
}
pub(super) fn parts() -> Result<Vec<(Part, Vec3, Mesh)>, String> {
    let mut out = Vec::new();
    let mut body = Builder::default();
    for row in 0..13 {
        let y = 1.10 + row as f32 * 0.085;
        let t = row as f32 / 12.0;
        let width = 0.26 + 0.23 * (t * std::f32::consts::PI * 0.72).sin();
        let z = 0.18 + t * 0.03;
        body.voxel([0.0, y, z], [width * 1.75, 0.105, 0.43], BARK[4])?;
        for side in 0..12 {
            let angle = side as f32 * std::f32::consts::TAU / 12.0;
            let h = hash(row * 101 + side * 337);
            body.rotated(
                [
                    angle.cos() * width,
                    y + (h % 3) as f32 * 0.002,
                    z + angle.sin() * 0.27,
                ],
                [
                    0.12 + (h % 5) as f32 * 0.009,
                    0.066 + (h % 4) as f32 * 0.003,
                    0.09,
                ],
                BARK[h as usize % 4],
                Quat::from_rotation_y(-angle),
            )?;
        }
    }
    for side in [-1.0, 1.0] {
        bark(
            &mut body,
            Vec3::new(side * 0.18, 1.78, 0.27),
            Vec3::new(side * 0.46, 2.20, 0.12),
            0.16,
            if side < 0.0 { 711 } else { 712 },
        )?;
        moss(&mut body, Vec3::new(side * 0.3, 2.02, 0.49), 0.68, 713)?;
        moss(&mut body, Vec3::new(side * 0.41, 1.65, 0.36), 0.43, 714)?;
    }
    out.push((Part::Body, Vec3::ZERO, body.finish()?));
    let pivot = Vec3::new(0.0, 2.19, -0.29);
    let mut head = Builder::default();
    head.voxel([0.0, 0.0, 0.045], [0.58, 0.48, 0.44], BARK[4])?;
    for x in -3..=3_i32 {
        for y in -3..=3_i32 {
            for z in -2..=2_i32 {
                if x.abs() < 3 && y.abs() < 3 && z.abs() < 2 {
                    continue;
                }
                if z == -2 && (-1..=1).contains(&y) && x.abs() <= 2 {
                    continue;
                }
                let h = hash((x + 3) as u32 * 711 + (y + 3) as u32 * 53 + (z + 2) as u32 * 13);
                head.voxel(
                    [x as f32 * 0.091, y as f32 * 0.082, z as f32 * 0.113],
                    [0.083, 0.071 + (h % 3) as f32 * 0.002, 0.104],
                    BARK[h as usize % 4],
                )?;
            }
        }
    }
    head.voxel(
        [0.0, -0.13, -0.26],
        [0.4, 0.21, 0.05],
        [0.035, 0.027, 0.018],
    )?;
    for side in [-1.0, 1.0] {
        head.voxel([side * 0.185, 0.02, -0.30], [0.16, 0.15, 0.08], BARK[4])?;
        head.voxel([side * 0.18, 0.14, -0.325], [0.20, 0.075, 0.10], BARK[2])?;
        bark(
            &mut head,
            Vec3::new(side * 0.25, 0.24, 0.0),
            Vec3::new(side * 0.40, 0.55, 0.05),
            0.028,
            987,
        )?;
        bark(
            &mut head,
            Vec3::new(side * 0.34, 0.43, 0.03),
            Vec3::new(side * 0.52, 0.48, 0.04),
            0.022,
            988,
        )?;
        for tooth in 0..3 {
            head.voxel(
                [side * (0.07 + tooth as f32 * 0.055), -0.19, -0.31],
                [0.038, 0.13 - tooth as f32 * 0.025, 0.05],
                BARK[2],
            )?;
        }
        moss(&mut head, Vec3::new(side * 0.26, -0.21, 0.03), 0.35, 919)?;
    }
    let mut eyes = Builder::default();
    for side in [-1.0, 1.0] {
        eyes.voxel(
            [side * 0.185, 0.015, -0.40],
            [0.105, 0.095, 0.035],
            [1.0, 0.66, 0.12],
        )?;
    }
    out.push((Part::Head, pivot, head.finish()?));
    out.push((Part::StalkerEyes, pivot, eyes.finish()?));
    for side in [-1.0, 1.0] {
        let shoulder = Vec3::new(side * 0.51, 2.06, 0.03);
        let elbow = Vec3::new(side * 0.80, 1.17, -0.16);
        let wrist = Vec3::new(side * 1.01, 0.52, -0.45);
        let mut upper = Builder::default();
        bark(&mut upper, Vec3::ZERO, elbow - shoulder, 0.15, 211)?;
        moss(&mut upper, Vec3::new(side * 0.10, -0.22, 0.10), 0.41, 212)?;
        out.push((Part::StalkerArm(side), shoulder, upper.finish()?));
        let mut forearm = Builder::default();
        bark(&mut forearm, Vec3::ZERO, wrist - elbow, 0.105, 213)?;
        out.push((Part::StalkerForearm(side), elbow, forearm.finish()?));
        let mut hand = Builder::default();
        bark(
            &mut hand,
            Vec3::ZERO,
            Vec3::new(side * 0.035, -0.22, -0.04),
            0.14,
            214,
        )?;
        for finger in 0..4 {
            let x = (finger as f32 - 1.5) * 0.073;
            let start = Vec3::new(x, -0.18, -0.035);
            let middle = Vec3::new(x * 1.6, -0.40, -0.06 - finger as f32 * 0.015);
            bark(&mut hand, start, middle, 0.027, 215 + finger)?;
            bark(
                &mut hand,
                middle,
                Vec3::new(x * 1.6, -0.44, -0.18),
                0.022,
                220 + finger,
            )?;
        }
        bark(
            &mut hand,
            Vec3::new(-side * 0.12, -0.05, 0.0),
            Vec3::new(-side * 0.25, -0.19, -0.11),
            0.033,
            229,
        )?;
        out.push((Part::StalkerHand(side), wrist, hand.finish()?));
        let hip = Vec3::new(side * 0.29, 1.24, 0.13);
        let knee = Vec3::new(side * 0.46, 0.62, 0.34);
        let mut leg = Builder::default();
        bark(&mut leg, Vec3::ZERO, knee - hip, 0.14, 310)?;
        bark(
            &mut leg,
            knee - hip,
            Vec3::new(side * 0.38, 0.13, 0.03) - hip,
            0.10,
            311,
        )?;
        for toe in 0..3 {
            bark(
                &mut leg,
                Vec3::new(side * 0.38, 0.12, 0.02) - hip,
                Vec3::new(side * (0.26 + toe as f32 * 0.10), 0.055, -0.26) - hip,
                0.037,
                315 + toe,
            )?;
        }
        out.push((Part::Leg(side), hip, leg.finish()?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use std::collections::BTreeMap;

    #[test]
    fn stalker_detail_tiles_have_no_coplanar_overlapping_exterior_faces() {
        for (index, (_, _, mesh)) in parts().unwrap().into_iter().enumerate() {
            assert_eq!(
                overlapping_faces(&mesh),
                0,
                "overlapping coplanar detail faces in part {index} cause camera-dependent flickering"
            );
        }
    }

    #[test]
    fn coplanar_overlap_regression_detects_the_old_moss_spacing() {
        let mut moss = crate::players::model::Builder::default();
        for y in [0.0, -0.075] {
            moss.voxel([0.0, y, 0.0], [0.033, 0.09, 0.035], BARK[0])
                .unwrap();
        }
        assert_eq!(overlapping_faces(&moss.finish().unwrap()), 4);
    }

    fn overlapping_faces(mesh: &Mesh) -> usize {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("model positions missing")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("model normals missing")
        };
        type Face = ([Vec3; 4], Vec3, f32);
        let mut planes = BTreeMap::<[i32; 3], Vec<Face>>::new();
        for (quad, normals) in positions.chunks_exact(4).zip(normals.chunks_exact(4)) {
            let normal = Vec3::from_array(normals[0]).normalize();
            let quad = std::array::from_fn(|i| Vec3::from_array(quad[i]));
            let key = normal.to_array().map(|v| (v * 1000.0).round() as i32);
            planes
                .entry(key)
                .or_default()
                .push((quad, normal, normal.dot(quad[0])));
        }
        let mut overlaps = 0;
        for faces in planes.values_mut() {
            faces.sort_by(|a, b| a.2.total_cmp(&b.2));
            for (i, (a, normal, distance)) in faces.iter().enumerate() {
                for (b, other_normal, _) in faces[i + 1..]
                    .iter()
                    .take_while(|b| b.2 - distance < 0.00001)
                {
                    if normal.dot(*other_normal) < 0.99999
                        || b.iter()
                            .any(|p| (normal.dot(*p) - distance).abs() > 0.00001)
                    {
                        continue;
                    }
                    let positive_overlap = [a[1] - a[0], a[3] - a[0], b[1] - b[0], b[3] - b[0]]
                        .into_iter()
                        .map(Vec3::normalize)
                        .all(|axis| {
                            let interval = |quad: &[Vec3; 4]| {
                                quad.iter()
                                    .fold((f32::INFINITY, f32::NEG_INFINITY), |r, p| {
                                        let v = p.dot(axis);
                                        (r.0.min(v), r.1.max(v))
                                    })
                            };
                            let (a0, a1) = interval(a);
                            let (b0, b1) = interval(b);
                            a1.min(b1) - a0.max(b0) > 0.00001
                        });
                    if positive_overlap {
                        eprintln!("coplanar model faces: normal={normal:?}, a={a:?}, b={b:?}");
                    }
                    overlaps += usize::from(positive_overlap);
                }
            }
        }
        overlaps
    }

    #[test]
    fn stalker_has_articulated_long_arms_layered_bark_and_unobstructed_amber_eyes() {
        let parts = parts().unwrap();
        assert_eq!(parts.len(), 11);
        assert_eq!(
            parts
                .iter()
                .filter(|(p, _, _)| matches!(p, Part::StalkerHand(_)))
                .count(),
            2
        );
        let mut total = 0;
        for (_, _, mesh) in &parts {
            total += mesh.count_vertices();
            assert!(
                mesh.indices()
                    .unwrap()
                    .iter()
                    .all(|i| i < mesh.count_vertices())
            );
        }
        assert!(
            (20_001..64_000).contains(&total),
            "detail is shared and bounded: {total}"
        );
        let (_, eye_offset, eye_mesh) = parts
            .iter()
            .find(|(p, _, _)| matches!(p, Part::StalkerEyes))
            .unwrap();
        let Some(VertexAttributeValues::Float32x3(eyes)) =
            eye_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("eyes missing")
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            eye_mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("eye normals missing")
        };
        let mut fronts = 0;
        for (quad, normals) in eyes.chunks_exact(4).zip(normals.chunks_exact(4)) {
            if normals[0][2] > -0.99 {
                continue;
            }
            fronts += 1;
            let quad = quad
                .iter()
                .copied()
                .map(Vec3::from_array)
                .collect::<Vec<_>>();
            for u in [0.1, 0.5, 0.9] {
                for v in [0.1, 0.5, 0.9] {
                    let target =
                        quad[0] + (quad[1] - quad[0]) * u + (quad[3] - quad[0]) * v + *eye_offset;
                    assert!(
                        target.z < eye_offset.z - 0.40,
                        "pupil front must be outside the bark socket"
                    );
                    let origin = target - Vec3::Z;
                    for (part_index, (part, offset, mesh)) in parts.iter().enumerate() {
                        if matches!(part, Part::StalkerEyes) {
                            continue;
                        }
                        let Some(VertexAttributeValues::Float32x3(points)) =
                            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        else {
                            panic!("occluder positions missing")
                        };
                        let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
                        for triangle in indices.chunks_exact(3) {
                            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                                .map(|i| Vec3::from_array(points[i]) + *offset);
                            assert!(
                                !blocks_eye(origin, a, b, c),
                                "forward pupil surface is occluded by model part {part_index}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(
            fronts, 2,
            "both amber eyes need independently visible front surfaces"
        );
    }

    fn blocks_eye(origin: Vec3, a: Vec3, b: Vec3, c: Vec3) -> bool {
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
        u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < 0.9999
    }
}
