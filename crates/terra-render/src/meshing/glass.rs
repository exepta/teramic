//! Joined panes keep one transparent face and frame only the group's perimeter.
use super::builder::MeshBatch;
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_voxel::meshing::BlockFace;

pub(super) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    face: BlockFace,
    pane: usize,
    frame: usize,
) -> Result<(), String> {
    let width = face.width as f32;
    let height = face.height as f32;
    let thickness = 0.035;
    let low_u = if face.edge_mask & 1 != 0 {
        thickness
    } else {
        0.0
    };
    let high_u = width
        - if face.edge_mask & 2 != 0 {
            thickness
        } else {
            0.0
        };
    let low_v = if face.edge_mask & 4 != 0 {
        thickness
    } else {
        0.0
    };
    let high_v = height
        - if face.edge_mask & 8 != 0 {
            thickness
        } else {
            0.0
        };
    quad(batches, face, pane, [low_u, low_v, high_u, high_v])?;
    if low_u > 0.0 {
        quad(batches, face, frame, [0.0, 0.0, low_u, height])?;
    }
    if high_u < width {
        quad(batches, face, frame, [high_u, 0.0, width, height])?;
    }
    if low_v > 0.0 {
        quad(batches, face, frame, [low_u, 0.0, high_u, low_v])?;
    }
    if high_v < height {
        quad(batches, face, frame, [low_u, high_v, high_u, height])?;
    }
    Ok(())
}

fn quad(
    batches: &mut BTreeMap<usize, MeshBatch>,
    face: BlockFace,
    material: usize,
    [u0, v0, u1, v1]: [f32; 4],
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..default()
    });
    let base = u32::try_from(batch.positions.len()).map_err(|_| "Glass mesh index overflow")?;
    let point = |u, v| {
        let mut p = face.minimum.map(|v| v as f32);
        p[face.side / 2] += (face.side % 2) as f32;
        p[face.u_axis] += u;
        p[face.v_axis] += v;
        p
    };
    let points = [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)];
    let mut normal = Vec3::ZERO;
    normal[face.side / 2] = if face.side.is_multiple_of(2) {
        -1.0
    } else {
        1.0
    };
    let cross = (Vec3::from_array(points[1]) - Vec3::from_array(points[0]))
        .cross(Vec3::from_array(points[3]) - Vec3::from_array(points[0]));
    let indices = if cross.dot(normal) > 0.0 {
        [0, 1, 2, 0, 2, 3]
    } else {
        [0, 2, 1, 0, 3, 2]
    };
    batch.positions.extend(points);
    batch.normals.extend([normal.to_array(); 4]);
    batch
        .uvs
        .extend(points.map(|p| [p[face.u_axis], p[face.v_axis]]));
    batch.indices.extend(indices.map(|i| base + i));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connected_panes_leave_no_internal_frames_or_overlapping_glass() {
        for mask in 0_u8..16 {
            let mut batches = BTreeMap::new();
            append(
                &mut batches,
                BlockFace {
                    minimum: [0; 3],
                    side: 3,
                    u_axis: 0,
                    v_axis: 2,
                    width: 1,
                    height: 1,
                    material: 15,
                    occupied_neighbor: false,
                    edge_mask: mask,
                },
                0,
                1,
            )
            .unwrap();
            assert_eq!(batches[&0].indices.len(), 6);
            assert_eq!(
                batches.get(&1).map_or(0, |b| b.indices.len()),
                mask.count_ones() as usize * 6
            );
            for batch in batches.values() {
                assert!(
                    batch
                        .positions
                        .iter()
                        .flatten()
                        .all(|v| (0.0..=1.0).contains(v))
                );
                assert!(
                    batch
                        .indices
                        .iter()
                        .all(|&i| (i as usize) < batch.positions.len())
                );
            }
        }
    }
}
