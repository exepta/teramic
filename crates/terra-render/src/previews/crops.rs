//! Harvested produce uses the same solid crop language as its planted form.
use super::{sculpt::*, *};
use terra_core::CropItem;
use terra_world::props::{
    PropKind,
    visual::{PropDetail, PropPalette},
};

pub(super) fn batches(item: CropItem, material: usize) -> Result<Vec<MeshBatch>, String> {
    if matches!(item, CropItem::Wheat | CropItem::Lettuce) {
        let kind = if item == CropItem::Wheat {
            PropKind::Wheat
        } else {
            PropKind::Lettuce
        };
        let mut parts: Vec<_> = terra_world::props::visual::template(
            kind,
            7,
            PropDetail::Near,
            &PropPalette {
                crop: material,
                ..Default::default()
            },
        )?
        .into_iter()
        .map(|mesh| MeshBatch {
            material: mesh.material,
            positions: mesh.positions,
            normals: mesh.normals,
            uvs: mesh.uvs,
            colors: mesh.colors,
            indices: mesh.indices,
            ..Default::default()
        })
        .collect();
        if item == CropItem::Wheat {
            let mut binding = Sculpt::new(64, 2);
            binding.cuboid([16, 18, 16], [44, 20, 19], [0.40, 0.24, 0.075]);
            binding.cuboid([16, 18, 42], [44, 20, 45], [0.40, 0.24, 0.075]);
            binding.cuboid([16, 18, 18], [19, 20, 44], [0.48, 0.30, 0.10]);
            binding.cuboid([42, 18, 18], [45, 20, 44], [0.48, 0.30, 0.10]);
            let mut band = binding.faceted_mesh(material)?;
            for point in &mut band.positions {
                point[0] -= 0.5;
                point[2] -= 0.5;
            }
            parts.push(band);
        }
        fit_unit(&mut parts)?;
        return Ok(parts);
    }
    let mut sculpt = Sculpt::new(32, 1);
    match item {
        CropItem::WheatSeeds | CropItem::CornSeeds | CropItem::LettuceSeeds => {
            let color = match item {
                CropItem::WheatSeeds => [0.88, 0.66, 0.24],
                CropItem::CornSeeds => [1.0, 0.76, 0.12],
                _ => [0.84, 0.80, 0.55],
            };
            for (index, [x, y, z]) in [
                [9, 12, 10],
                [19, 12, 9],
                [12, 12, 21],
                [22, 12, 20],
                [16, 16, 15],
            ]
            .into_iter()
            .enumerate()
            {
                let size = if item == CropItem::LettuceSeeds { 3 } else { 4 };
                sculpt.cuboid(
                    [x - 2, y - 2, z - 2],
                    [x + size, y + size - 1, z + size],
                    shade(color, [index as i16, y, z], 0.06),
                );
                sculpt.cuboid(
                    [x - 1, y + size, z - 1],
                    [x + size - 1, y + size, z + size - 1],
                    color,
                );
            }
        }
        CropItem::Corn => corn(&mut sculpt),
        CropItem::Bread => bread(&mut sculpt),
        CropItem::Wheat | CropItem::Lettuce => unreachable!(),
    }
    let mut parts = vec![sculpt.faceted_mesh(material)?];
    fit_unit(&mut parts)?;
    Ok(parts)
}

fn corn(sculpt: &mut Sculpt) {
    for y in 5_i16..29 {
        let radius = if y > 24 { 4.0 } else { 6.0 };
        for x in 9_i16..23 {
            for z in 9_i16..23 {
                let dx = f32::from(x) - 15.5;
                let dz = f32::from(z) - 15.5;
                if dx * dx + dz * dz <= radius * radius {
                    let groove = (y - 5) % 4 == 0 || x % 4 == 0 || z % 4 == 0;
                    sculpt.cell(
                        [x, y, z],
                        if groove {
                            [0.86, 0.48, 0.045]
                        } else if y % 3 == 0 {
                            [1.0, 0.84, 0.24]
                        } else {
                            [1.0, 0.71, 0.085]
                        },
                    );
                }
            }
        }
    }
    for y in 3_i16..19 {
        let width = if y < 9 { 6 } else { (20 - y) / 2 };
        for side in [-1_i16, 1] {
            let x = 16 + side * (5 + y / 7);
            sculpt.cuboid(
                [x - 1, y, 16 - width],
                [x + 1, y, 16 + width],
                if side < 0 {
                    [0.23, 0.55, 0.07]
                } else {
                    [0.42, 0.69, 0.10]
                },
            );
        }
        if y < 8 {
            sculpt.cuboid([13, y, 12], [19, y, 20], [0.28, 0.56, 0.075]);
        }
    }
}

fn bread(sculpt: &mut Sculpt) {
    for x in 3_i16..29 {
        for y in 8_i16..25 {
            for z in 6_i16..26 {
                let dx = (f32::from(x) - 15.5) / 13.0;
                let dy = (f32::from(y) - 13.0) / 10.5;
                let dz = (f32::from(z) - 15.5) / 10.0;
                if dx * dx + dy * dy + dz * dz > 1.0 {
                    continue;
                }
                let cut = y > 19 && (x + z / 3).rem_euclid(8) < 2;
                sculpt.cell(
                    [x, y, z],
                    shade(
                        if cut {
                            [1.0, 0.80, 0.45]
                        } else if y < 10 {
                            [0.46, 0.22, 0.055]
                        } else {
                            [0.80, 0.46, 0.13]
                        },
                        [x / 2, y / 2, z / 2],
                        0.07,
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_crop_items_are_finite_solid_cached_models_with_bounded_indices() {
        for item in CropItem::ALL {
            let first = batches(item, 13).unwrap();
            let second = batches(item, 13).unwrap();
            assert_eq!(
                first.iter().map(|m| &m.positions).collect::<Vec<_>>(),
                second.iter().map(|m| &m.positions).collect::<Vec<_>>()
            );
            let count = first.iter().map(|m| m.positions.len()).sum::<usize>();
            assert!(count > 0 && count < 16_000, "{item:?}: {count}");
            for part in first {
                assert_eq!(part.material, 13);
                assert!(
                    part.positions
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && (-0.001..=1.001).contains(v))
                );
                assert_eq!(part.positions.len(), part.colors.len());
                assert!(
                    part.indices
                        .iter()
                        .all(|&i| (i as usize) < part.positions.len())
                );
            }
        }
    }
}
