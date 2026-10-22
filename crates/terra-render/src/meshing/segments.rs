use super::{DetailLevel, builder::MeshBatch};
use std::collections::BTreeMap;
use terra_world::segments::Profile;

struct PreviousTriangle {
    points: [[f32; 3]; 3],
    indices: [u32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

pub(super) fn append(
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
                batch.uvs.push([vertex[0], vertex[1]]);
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
