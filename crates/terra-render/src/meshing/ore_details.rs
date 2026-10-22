use super::{DetailLevel, builder::MeshBatch};
use bevy::prelude::*;
use std::collections::BTreeMap;
use terra_world::ore_shapes::{OreDetail, normal, visit_shaded_face_with_growth};

pub(crate) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    material: usize,
    material_id: u16,
    position: [i64; 3],
    local: Vec3,
    side: usize,
    (level, growth): (DetailLevel, f32),
) -> Result<(), String> {
    let batch = batches.entry(material).or_insert_with(|| MeshBatch {
        material,
        ..default()
    });
    let level = match level {
        DetailLevel::Near => OreDetail::Near,
        DetailLevel::Middle => OreDetail::Middle,
        DetailLevel::Far => OreDetail::Far,
    };
    let mut overflow = false;
    visit_shaded_face_with_growth(
        material_id,
        position,
        side,
        level,
        growth,
        |triangle, tone| {
            let Ok(base) = u32::try_from(batch.positions.len()) else {
                overflow = true;
                return;
            };
            if base > u32::MAX - 3 {
                overflow = true;
                return;
            }
            let normal = normal(triangle);
            let shade = (0.88 + (normal[1] * 0.06 + normal[0] * 0.04).clamp(-0.09, 0.09)) * tone;
            batch
                .positions
                .extend(triangle.map(|vertex| (local + Vec3::from_array(vertex)).to_array()));
            batch.normals.extend([normal; 3]);
            // A common face projection keeps touching fracture strips identical at
            // joins instead of sampling a different texture triangle at each turn.
            let axis = if tone != 1.0 {
                side / 2
            } else {
                (0..3)
                    .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
                    .unwrap_or(side / 2)
            };
            batch
                .uvs
                .extend(triangle.map(|vertex| [vertex[(axis + 1) % 3], vertex[(axis + 2) % 3]]));
            batch
                .colors
                .resize(batch.positions.len(), [shade, shade, shade, 1.0]);
            batch.indices.extend([base, base + 1, base + 2]);
        },
    );
    if overflow {
        Err("Mineral geometry exceeds index capacity".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mineral_facets_batch_by_material_and_keep_finite_indexed_geometry_at_all_lods() {
        for ore in terra_core::OreKind::ALL {
            let mut previous = usize::MAX;
            for lod in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
                let mut batches = BTreeMap::new();
                for side in 0..6 {
                    append(
                        &mut batches,
                        7,
                        ore.material(),
                        [-17, -33, 16],
                        Vec3::ZERO,
                        side,
                        (lod, 0.0),
                    )
                    .unwrap();
                }
                assert_eq!(batches.len(), 1);
                let batch = &batches[&7];
                assert_eq!(batch.positions.len(), batch.normals.len());
                assert_eq!(batch.positions.len(), batch.uvs.len());
                assert!(batch.positions.len() < previous);
                previous = batch.positions.len();
                assert!(batch.positions.iter().flatten().all(|v| v.is_finite()));
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
