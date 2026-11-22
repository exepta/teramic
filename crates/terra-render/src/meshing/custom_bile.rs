//! Adapt the shared water surface to Galle's material and interpolation labels.
use super::*;
use bevy::math::Vec3;

pub(super) fn batch(
    surface: &terra_world::bile::BileSurface,
    material: usize,
) -> Option<MeshBatch> {
    let patch = surface.mesh()?;
    let mut batch = MeshBatch {
        material,
        positions: patch.positions,
        normals: patch.normals,
        indices: patch.indices,
        ..Default::default()
    };
    for (i, (&p, &normal)) in batch.positions.iter().zip(&batch.normals).enumerate() {
        let axis = (0..3)
            .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
            .unwrap();
        batch.uvs.push([p[(axis + 1) % 3], p[(axis + 2) % 3]]);
        batch.tangents.push(crate::blocks::projected_tangent(
            Vec3::from_array(normal),
            axis * 2 + usize::from(normal[axis] > 0.),
        ));
        batch
            .surface_water
            .push([patch.depth_shore[i][0].clamp(0., 1.), -4.]);
        let [x, z] = patch.flow[i];
        let dir = if x < 0. {
            1.
        } else if x > 0. {
            2.
        } else if z < 0. {
            3.
        } else if z > 0. {
            4.
        } else {
            0.
        };
        let [r, c, h, marker] = patch.surface_effects[i];
        batch.bile_motion.push([
            r,
            c,
            h,
            if marker == terra_world::water::POUR_SURFACE {
                dir
            } else {
                -dir
            },
        ]);
    }
    Some(batch)
}
