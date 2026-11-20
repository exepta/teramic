//! Faceted ivory thorns attached to exposed cactus faces, never decal lines.
use super::{DetailLevel, builder::MeshBatch};
use bevy::prelude::*;
use std::collections::BTreeMap;

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    local: Vec3,
    global: [i64; 3],
    side: usize,
    level: DetailLevel,
) -> Result<(), String> {
    if side == 2 || level == DetailLevel::Far {
        return Ok(());
    }
    let axis = side / 2;
    let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    let mut normal = Vec3::ZERO;
    normal[axis] = sign;
    let (u, v) = if axis == 0 {
        (Vec3::Z, Vec3::Y)
    } else if axis == 2 {
        (Vec3::X, Vec3::Y)
    } else {
        (Vec3::X, Vec3::Z)
    };
    // A stable global-cell seed avoids repeating a comb on every block and
    // preserves the same thorn clusters when a section changes detail level.
    let seed = cell_seed(global, side);
    let count = if side == 3 {
        3 + seed % 3
    } else {
        4 + seed % 4
    };
    let batch = batches.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..Default::default()
    });
    for i in 0..count {
        // Middle LOD is a stable subset, not a different arrangement.
        if level == DetailLevel::Middle && i % 2 != 0 {
            continue;
        }
        let sample = seed.wrapping_add(i.wrapping_mul(0x9e37_79b9));
        let a = 0.16 + unit(sample ^ 0x2c1b_3c6d) * 0.68;
        let b = if side == 3 {
            0.16 + unit(sample ^ 0x297a_2d39) * 0.68
        } else {
            // Staggered vertical bands keep the random clusters apart, while
            // each cluster varies in height and lateral offset.
            (i as f32 + 0.25 + unit(sample ^ 0x297a_2d39) * 0.5) / count as f32
        };
        let center = local + Vec3::splat(0.5) + normal * 0.488 + u * (a - 0.5) + v * (b - 0.5);
        let reach = terra_world::hazards::THORN_REACH as f32 + 0.012;
        let length = reach * (0.65 + unit(sample ^ 0x68e3_1da4) * 0.35);
        let lean = (unit(sample ^ 0xb529_7a4d) - 0.5) * 0.045;
        let tip = center
            + normal * length
            + u * lean
            + if side == 3 {
                v * lean
            } else {
                Vec3::Y * (0.018 + unit(sample) * 0.045)
            };
        let radius = 0.017 + unit(sample ^ 0x1b56_c4e9) * 0.012;
        let ring = [
            center + (u + v) * radius,
            center + (-u + v) * radius,
            center + (-u - v) * radius,
            center + (u - v) * radius,
        ];
        for edge in 0..4 {
            let mut points = [ring[edge], ring[(edge + 1) % 4], tip];
            let mid = (points[0] + points[1] + points[2]) / 3.0;
            let mut n = (points[1] - points[0]).cross(points[2] - points[0]);
            if n.dot(mid - (center + tip) * 0.5) < 0.0 {
                points.swap(0, 1);
                n = -n;
            }
            let start =
                u32::try_from(batch.positions.len()).map_err(|_| "Cactus thorn mesh overflow")?;
            start.checked_add(2).ok_or("Cactus thorn mesh overflow")?;
            batch.positions.extend(points.map(|p| p.to_array()));
            batch.normals.extend([n.normalize().to_array(); 3]);
            batch.uvs.extend([[0.37, 0.37], [0.63, 0.37], [0.5, 0.63]]);
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            batch.indices.extend([start, start + 1, start + 2]);
        }
    }
    Ok(())
}

fn cell_seed(global: [i64; 3], side: usize) -> u32 {
    global
        .into_iter()
        .zip([0x8da6_b343_u32, 0xd816_3841, 0xcb1a_b31f])
        .fold(side as u32 * 1777, |seed, (coordinate, prime)| {
            let bytes = coordinate.to_le_bytes();
            let lo = u32::from_le_bytes(bytes[..4].try_into().unwrap());
            let hi = u32::from_le_bytes(bytes[4..].try_into().unwrap());
            seed ^ lo.wrapping_mul(prime) ^ hi.rotate_left(13)
        })
}

fn unit(mut seed: u32) -> f32 {
    seed = (seed ^ (seed >> 16)).wrapping_mul(0x7feb_352d);
    seed = (seed ^ (seed >> 15)).wrapping_mul(0x846c_a68b);
    ((seed ^ (seed >> 16)) & 0xffff) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neighboring_cacti_have_irregular_but_reproducible_lod_stable_thorns() {
        let build = |global, level| {
            let mut batches = BTreeMap::new();
            append(&mut batches, 0, Vec3::ZERO, global, 1, level).unwrap();
            batches.remove(&0).unwrap()
        };
        let near = build([-16, 62, 9], DetailLevel::Near);
        let again = build([-16, 62, 9], DetailLevel::Near);
        let neighbor = build([-15, 62, 9], DetailLevel::Near);
        let middle = build([-16, 62, 9], DetailLevel::Middle);
        assert_eq!(near.positions, again.positions);
        assert_ne!(near.positions, neighbor.positions);
        for point in &middle.positions {
            assert!(near.positions.contains(point));
        }
        let tips: Vec<_> = near.positions.chunks_exact(12).map(|p| p[2]).collect();
        assert!(
            tips.windows(2)
                .all(|p| p[0][1] != p[1][1] && p[0][2] != p[1][2])
        );
        assert!(tips.windows(2).any(|p| (p[0][0] - p[1][0]).abs() > 0.005));
    }

    #[test]
    fn actual_spines_project_outward_on_five_faces_with_bounded_lods() {
        for level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
            let mut batches = BTreeMap::new();
            for side in 0..6 {
                append(&mut batches, 0, Vec3::ZERO, [0; 3], side, level).unwrap();
            }
            if level == DetailLevel::Far {
                assert!(batches.is_empty());
                continue;
            }
            let batch = &batches[&0];
            assert!(batch.indices.len() / 3 <= if level == DetailLevel::Near { 132 } else { 76 });
            assert!(
                batch
                    .positions
                    .iter()
                    .all(|p| p.iter().all(|v| (-0.161..=1.161).contains(v)))
            );
            for axis in [0, 2] {
                assert!(batch.positions.iter().any(|p| p[axis] < -0.09));
                assert!(batch.positions.iter().any(|p| p[axis] > 1.09));
            }
            assert!(batch.positions.iter().any(|p| p[1] > 1.09));
            assert!(batch.positions.iter().all(|p| p[1] >= 0.0));
            assert!(
                batch
                    .normals
                    .iter()
                    .all(|n| (Vec3::from_array(*n).length() - 1.0).abs() < 0.0001)
            );
        }
    }
}
