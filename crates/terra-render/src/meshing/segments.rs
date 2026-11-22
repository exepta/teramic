use super::{DetailLevel, builder::MeshBatch};
use std::collections::BTreeMap;
use terra_world::segments::Profile;

struct PreviousTriangle {
    points: [[f32; 3]; 3],
    indices: [u32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

pub(crate) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material_slot: usize,
    material: u16,
    position: [i64; 3],
    local: [u32; 3],
    profile: Profile,
    level: DetailLevel,
) -> Result<(), String> {
    let batch = batches.entry(material_slot).or_insert_with(|| MeshBatch {
        material: material_slot,
        ..Default::default()
    });
    let mut overflow = false;
    let mut previous: Option<PreviousTriangle> = None;
    terra_world::segments::visit(
        material,
        position,
        profile,
        level == DetailLevel::Far,
        |triangle, color| {
            let normal = terra_world::ore_shapes::normal(triangle);
            let shared = previous.as_ref().filter(|p| {
                p.color == color
                    && p.normal
                        .iter()
                        .zip(normal)
                        .all(|(a, b)| (a - b).abs() < 0.00001)
            });
            let mut indices = [0; 3];
            // Adjacent triangles share a quad edge. Reuse only equal attributes;
            // sharp normals and color seams keep separate vertices.
            for (i, vertex) in triangle.into_iter().enumerate() {
                if let Some(index) = shared.and_then(|p| {
                    p.points
                        .iter()
                        .position(|v| *v == vertex)
                        .map(|i| p.indices[i])
                }) {
                    indices[i] = index;
                    continue;
                }
                let Ok(index) = u32::try_from(batch.positions.len()) else {
                    overflow = true;
                    return;
                };
                indices[i] = index;
                batch
                    .positions
                    .push(std::array::from_fn(|i| vertex[i] + local[i] as f32));
                batch.normals.push(normal);
                let uv = if terra_world::segments::is_crystal(material) {
                    // Use the same metre scale as the surrounding rock. Facets
                    // have real tangent frames for their mineral normal map.
                    let p: [f32; 3] = std::array::from_fn(|axis| {
                        position[axis].rem_euclid(16) as f32 + vertex[axis]
                    });
                    let side = if normal[1].abs() >= normal[0].abs().max(normal[2].abs()) {
                        2
                    } else if normal[0].abs() >= normal[2].abs() {
                        0
                    } else {
                        4
                    };
                    let frame = crate::blocks::projected_tangent(normal.into(), side);
                    batch.append_tangents(index as usize, [frame].into_iter());
                    match side {
                        0 => [p[2], p[1]],
                        2 => [p[0], p[2]],
                        _ => [p[0], p[1]],
                    }
                } else {
                    [vertex[0], vertex[1]]
                };
                batch.uvs.push(uv);
                batch.colors.push([color[0], color[1], color[2], 1.0]);
                if matches!(material, 50 | 54) {
                    let weight = if material == 50 {
                        terra_world::segments::water_weight(profile, vertex[1])
                    } else {
                        ((f32::from(profile.above) + 1.0 - vertex[1])
                            / (f32::from(profile.above) + f32::from(profile.below) + 1.0))
                            .powi(2)
                            * 0.5
                    };
                    batch.rooted_wind.push([0.0, weight]);
                }
            }
            batch.indices.extend(indices);
            previous = Some(PreviousTriangle {
                points: triangle,
                indices,
                normal,
                color,
            });
        },
    );
    if overflow {
        Err("Segment vertex capacity exceeded".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glacial_and_mineral_faces_use_metre_scale_with_matching_normal_map_frames() {
        for material in 51..=53 {
            let mut batches = BTreeMap::new();
            append(
                &mut batches,
                0,
                material,
                [-17, -17, -17],
                [0; 3],
                Profile {
                    below: 2,
                    above: 3,
                    ..Default::default()
                },
                DetailLevel::Near,
            )
            .unwrap();
            let b = &batches[&0];
            assert_eq!(b.positions.len(), b.tangents.len());
            let mut v_min = f32::MAX;
            let mut v_max = f32::MIN;
            for ((normal, tangent), uv) in b.normals.iter().zip(&b.tangents).zip(&b.uvs) {
                assert!(
                    (normal[0] * tangent[0] + normal[1] * tangent[1] + normal[2] * tangent[2])
                        .abs()
                        < 1e-4
                );
                if normal[1].abs() < 0.5 {
                    v_min = v_min.min(uv[1]);
                    v_max = v_max.max(uv[1]);
                }
            }
            assert!((v_max - v_min - 1.).abs() < 1e-4);
        }
    }
}
