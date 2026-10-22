use crate::meshing::builder::MeshBatch;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// One small visual voxel; these vertices share a section batch, never an ECS entity.
pub(crate) fn add_box(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    center: Vec3,
    size: Vec3,
    rotation: Quat,
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..default()
    });
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let mut normal = Vec3::ZERO;
            normal[axis] = sign;
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            let base = u32::try_from(batch.positions.len()).map_err(|_| "Micro mesh overflow")?;
            base.checked_add(3).ok_or("Micro mesh overflow")?;
            let rotated_normal = (rotation * normal).to_array();
            for [a, b] in [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
                let mut point = normal * size * 0.5;
                point[u] = a * size[u];
                point[v] = b * size[v];
                batch.positions.push((center + rotation * point).to_array());
                batch.normals.push(rotated_normal);
                batch.uvs.push([a + 0.5, b + 0.5]);
            }
            let indices = if sign > 0.0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            batch.indices.extend(indices.map(|i| base + i));
        }
    }
    if !batch.colors.is_empty() {
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
    }
    Ok(())
}
