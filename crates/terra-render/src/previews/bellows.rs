use super::{sculpt::*, *};
use terra_core::BellowsKind;

pub(super) fn batches(kind: BellowsKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let mut sculpt = Sculpt::new(32, 1);
    let nozzle = match kind {
        BellowsKind::Stone => [0.48, 0.49, 0.47],
        BellowsKind::Iron => [0.66, 0.72, 0.75],
        BellowsKind::Gold => [0.96, 0.65, 0.17],
        BellowsKind::Ruby => [0.75, 0.08, 0.21],
    };
    for z in 6..25i16 {
        let half = if z < 12 { 9 } else { 9 - (z - 12) / 2 };
        let top = 22 - (z - 6) / 3;
        for x in 16 - half..=16 + half {
            for y in 10..=top {
                let color = if y <= 11 || y >= top - 1 {
                    if (x + z / 3) % 5 == 0 {
                        [0.47, 0.26, 0.10]
                    } else {
                        [0.70, 0.43, 0.19]
                    }
                } else if (y + z / 3) % 3 == 0 {
                    [0.24, 0.115, 0.055]
                } else {
                    [0.39, 0.20, 0.085]
                };
                sculpt.cell([x, y, z], shade(color, [x, y, z], 0.06));
            }
        }
        if z % 3 == 0 {
            for x in [16 - half, 16 + half] {
                sculpt.cell([x, top, z], [0.84, 0.63, 0.28]);
            }
        }
    }
    for y in [10, 20] {
        sculpt.cuboid([13, y, 2], [18, y + 1, 7], [0.46, 0.27, 0.12]);
    }
    sculpt.cuboid([6, 10, 8], [7, 22, 10], [0.23, 0.25, 0.24]);
    sculpt.cuboid([25, 10, 8], [26, 22, 10], [0.23, 0.25, 0.24]);
    sculpt.cuboid([12, 10, 23], [20, 17, 26], nozzle);
    for x in 13..20 {
        for y in 11..17 {
            for z in 26..31 {
                if x == 13 || x == 19 || y == 11 || y == 16 {
                    sculpt.cell([x, y, z], shade(nozzle, [x, y, z], 0.08));
                } else if z == 26 {
                    sculpt.cell([x, y, z], [0.06; 3]);
                }
            }
        }
    }
    Ok(vec![sculpt.faceted_mesh(material)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_nozzle_variants_have_distinct_colors_and_shared_bounded_geometry() {
        let mut previous: Option<MeshBatch> = None;
        for kind in [
            BellowsKind::Stone,
            BellowsKind::Iron,
            BellowsKind::Gold,
            BellowsKind::Ruby,
        ] {
            let mut batches = batches(kind, 7).unwrap();
            assert_eq!(batches.len(), 1);
            let batch = batches.pop().unwrap();
            assert!(batch.positions.len() < 30_000);
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
            if let Some(prior) = previous {
                assert_eq!(prior.positions, batch.positions);
                assert_ne!(prior.colors, batch.colors);
            }
            previous = Some(batch);
        }
    }
}
