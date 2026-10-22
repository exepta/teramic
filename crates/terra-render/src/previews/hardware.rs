//! Cached, closed item solids in the same sculpted palette as food and tools.
use super::{sculpt::*, *};

pub(super) fn torch(batches: Vec<MeshBatch>, templates: &TemplateSet) -> Result<MeshBatch, String> {
    let mut item = MeshBatch {
        material: templates.item_materials.torch,
        ..default()
    };
    // Reuse the exact imported world geometry, baking only the palette into one
    // isolated material. Neither duplicated geometry definitions nor per-part
    // item draw calls are needed, and the held light cannot bleach its handle.
    for batch in batches {
        let palette = templates
            .torch_palette
            .get(&batch.material)
            .ok_or("Missing torch item palette")?;
        let base = u32::try_from(item.positions.len()).map_err(|_| "Torch item overflow")?;
        for index in batch.indices {
            item.indices
                .push(base.checked_add(index).ok_or("Torch item index overflow")?);
        }
        for (index, normal) in batch.normals.iter().enumerate() {
            let gain = 0.73
                + 0.27
                    * Vec3::from_array(*normal)
                        .dot(Vec3::new(-0.35, 0.78, 0.52))
                        .max(0.0);
            let tint = batch.colors.get(index).copied().unwrap_or([1.0; 4]);
            item.colors.push([
                palette[0] * tint[0] * gain,
                palette[1] * tint[1] * gain,
                palette[2] * tint[2] * gain,
                1.0,
            ]);
        }
        item.positions.extend(batch.positions);
        item.normals.extend(batch.normals);
        item.uvs.extend(batch.uvs);
    }
    Ok(item)
}

pub(super) fn ingot(item: terra_core::MineralItem, material: usize) -> Result<MeshBatch, String> {
    let gold = item == terra_core::MineralItem::GoldIngot;
    let base = if gold {
        [0.92, 0.61, 0.12]
    } else {
        [0.66, 0.71, 0.73]
    };
    let mut s = Sculpt::new(32, 1);
    for y in 10i16..20 {
        let bevel = match y {
            10 | 19 => 2,
            11 | 18 => 1,
            _ => 0,
        };
        let taper = (y - 10) / 4;
        for x in 2 + bevel + taper..30 - bevel - taper {
            for z in 8 + bevel + taper..24 - bevel - taper {
                let corner =
                    (x < 5 + taper || x > 26 - taper) && (z < 11 + taper || z > 20 - taper);
                if corner {
                    continue;
                }
                // Shallow irregular casting marks and a recessed maker's stamp.
                if y == 19
                    && (((x * 11 + z * 7) % 29 == 0)
                        || ((14..=17).contains(&x) && (14..=17).contains(&z)))
                {
                    continue;
                }
                let gain = if y >= 18 {
                    1.10
                } else if y <= 11 {
                    0.78
                } else {
                    0.93
                };
                s.cell(
                    [x, y, z],
                    shade(base.map(|v| v * gain), [x / 2, y / 2, z / 2], 0.045),
                );
            }
        }
    }
    let mut batch = s.faceted_mesh(material)?;
    fit_unit(std::slice::from_mut(&mut batch))?;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_has_coloured_closed_bounded_geometry_in_one_shared_batch() {
        for batch in [
            ingot(terra_core::MineralItem::GoldIngot, 7).unwrap(),
            ingot(terra_core::MineralItem::IronIngot, 7).unwrap(),
        ] {
            assert_eq!(batch.material, 7);
            assert!(batch.indices.len() / 3 > 1000);
            assert!(batch.indices.len() / 3 < 20_000);
            assert_eq!(batch.positions.len(), batch.colors.len());
            assert!(
                batch
                    .positions
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (-0.001..=1.001).contains(v))
            );
            for triangle in batch.indices.chunks_exact(3) {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|i| Vec3::from_array(batch.positions[i as usize]));
                let normal = Vec3::from_array(batch.normals[triangle[0] as usize]);
                assert!((b - a).cross(c - a).dot(normal) > 0.0);
            }
        }
    }

    #[test]
    fn torch_item_reuses_world_positions_and_indices_without_emissive_materials() {
        let mut templates = TemplateSet::default();
        templates.item_materials.torch = 7;
        templates.torch_palette.insert(2, [0.3, 0.17, 0.07, 1.0]);
        templates.torch_palette.insert(3, [1.0, 0.72, 0.15, 1.0]);
        let mut a = Sculpt::new(8, 1);
        a.cuboid([3, 0, 3], [4, 4, 4], [1.0; 3]);
        let mut b = Sculpt::new(8, 1);
        b.cuboid([2, 5, 2], [5, 7, 5], [1.0; 3]);
        let (a, b) = (a.mesh(2).unwrap(), b.mesh(3).unwrap());
        let count = a.indices.len() + b.indices.len();
        let positions: Vec<_> = a.positions.iter().chain(&b.positions).copied().collect();
        let item = torch(vec![a, b], &templates).unwrap();
        assert_eq!(item.material, 7);
        assert_eq!(item.positions, positions);
        assert_eq!(item.indices.len(), count);
        assert_eq!(item.colors.len(), positions.len());
        assert!(
            item.colors
                .iter()
                .all(|c| c[0] > c[1] && c[1] > c[2] && c[0] <= 1.0)
        );
        assert!(
            item.indices
                .iter()
                .all(|i| (*i as usize) < item.positions.len())
        );
    }
}
