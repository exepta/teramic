use super::{sculpt::*, *};
use terra_core::crusher::DustKind;

pub(super) fn batches(kind: DustKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let color = match kind {
        DustKind::Iron => [0.63, 0.38, 0.26],
        DustKind::Gold => [0.98, 0.68, 0.19],
        DustKind::Coal => [0.14, 0.16, 0.18],
        DustKind::Diamond => [0.10, 0.80, 0.88],
        DustKind::Salt => [0.96, 0.93, 0.86],
        DustKind::Ruby => [0.85, 0.055, 0.17],
    };
    let mut sculpt = Sculpt::new(32, 1);
    for x in 3_i16..29 {
        for z in 3_i16..29 {
            let radius =
                ((f32::from(x) - 15.5).powi(2) + (f32::from(z) - 15.5).powi(2)).sqrt() / 12.7;
            if radius >= 1.0 {
                continue;
            }
            let variation = (x * 13 + z * 7).rem_euclid(5);
            let top = 9 + ((1.0 - radius).powf(0.7) * 10.0) as i16 + i16::from(variation == 0);
            for y in 8..=top {
                sculpt.cell([x, y, z], shade(color, [x, y, z], 0.12));
            }
        }
    }
    for (x, y, z) in [(12, 19, 16), (18, 17, 11), (19, 16, 20)] {
        sculpt.cuboid(
            [x, y, z],
            [x + 2, y + 1, z + 2],
            color.map(|v| (v * 1.04).min(1.0)),
        );
    }
    Ok(vec![sculpt.faceted_mesh(material)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_mineral_dust_piles_are_distinct_solid_bounded_item_meshes() {
        let mut previous = None;
        for kind in DustKind::ALL {
            let batch = batches(kind, 12).unwrap().remove(0);
            assert_eq!(batch.material, 12);
            assert!(batch.positions.len() > 64 && batch.positions.len() < 16_384);
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
                    .all(|i| (*i as usize) < batch.positions.len())
            );
            if let Some(prior) = previous {
                assert_ne!(prior, batch.colors);
            }
            previous = Some(batch.colors);
        }
    }
}
