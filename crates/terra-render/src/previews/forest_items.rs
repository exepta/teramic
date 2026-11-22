//! Closed cached voxel solids, shared by inventory icons, held items and drops.
use super::{
    ItemKind, MeshBatch,
    sculpt::{Sculpt, shade},
};

pub(crate) fn batches(item: ItemKind, material: usize) -> Result<Vec<MeshBatch>, String> {
    let mut s = Sculpt::new(32, 2);
    let red = [0.73, 0.04, 0.12];
    let bright = [0.96, 0.15, 0.25];
    let brown = [0.20, 0.06, 0.07];
    for x in 2i16..30 {
        for y in 2i16..30 {
            for z in 2i16..30 {
                let [dx, dy, dz] = [x - 16, y - 15, z - 16].map(f32::from);
                let color = match item {
                    ItemKind::BloodBerry => ((dx * dx + dy * dy + dz * dz) < 95.0)
                        .then_some(if y > 19 { bright } else { red }),
                    ItemKind::CrimsonMushroom => {
                        if dx * dx + dz * dz < 18.0 && (4..21).contains(&y) {
                            Some([0.91, 0.74, 0.58])
                        } else if (19..26).contains(&y)
                            && dx * dx + dz * dz < (26 - y) as f32 * 25.0
                        {
                            Some(if (x + z * 3) % 11 < 3 {
                                [1.0, 0.84, 0.73]
                            } else {
                                red
                            })
                        } else {
                            None
                        }
                    }
                    ItemKind::BloodCrystal => ((dx.abs() + dz.abs()) < (29 - y) as f32 * 0.65
                        && y > 4)
                        .then_some(if x > 15 { bright } else { red }),
                    ItemKind::ThornTwig => {
                        let stem = dx.abs() < 2.3 && dz.abs() < 2.3 && (3..28).contains(&y);
                        let thorn = ((y - 8).abs() < 2 || (y - 18).abs() < 2)
                            && dz.abs() < 2.0
                            && dx.abs() < 8.0;
                        (stem || thorn).then_some(if thorn { red } else { brown })
                    }
                    ItemKind::DarkSap | ItemKind::BloodSlimeGel => {
                        let radius = (29 - y) as f32 * 0.6;
                        let occupied = if y < 16 {
                            dx * dx + (dy * 0.85).powi(2) + dz * dz < 105.0
                        } else {
                            dx * dx + dz * dz < radius * radius
                        };
                        occupied.then_some(if item == ItemKind::DarkSap {
                            [0.22, 0.07, 0.075]
                        } else if x < 12 && z < 13 {
                            [1.0, 0.36, 0.44]
                        } else {
                            red
                        })
                    }
                    ItemKind::BloodLeaf => {
                        let width = (1.0 - (dy / 13.0).abs()) * 10.0;
                        ((4..29).contains(&y)
                            && dx.abs() < width
                            && (z - 16).abs() <= 1 + ((x + y) % 4 == 0) as i16)
                            .then_some(if dx.abs() < 1.0 {
                                [0.36, 0.03, 0.07]
                            } else {
                                bright
                            })
                    }
                    ItemKind::BloodSpore => {
                        let centers = [
                            [-5.0, 0.0, -3.0],
                            [4.0, 2.0, 4.0],
                            [0.0, 7.0, -2.0],
                            [5.0, -5.0, -4.0],
                        ];
                        centers
                            .iter()
                            .any(|c| {
                                (dx - c[0]).powi(2) + (dy - c[1]).powi(2) + (dz - c[2]).powi(2)
                                    < 22.0
                            })
                            .then_some([0.94, 0.24, 0.51])
                    }
                    ItemKind::Bone => {
                        let stem = dx.abs() < 2.5 && dz.abs() < 2.5 && (7..25).contains(&y);
                        let ends = [-8.0, 9.0].iter().any(|yy| {
                            [-3.0, 3.0]
                                .iter()
                                .any(|xx| (dx - xx).powi(2) + (dy - yy).powi(2) + dz * dz < 19.0)
                        });
                        (stem || ends).then_some([0.93, 0.87, 0.70])
                    }
                    ItemKind::Bitumen => {
                        let blob = (dx * 0.9).powi(2) + (dy + 4.0).powi(2) + dz.powi(2) < 105.0;
                        let drip = (dx - 2.0).powi(2) + (dz + 1.0).powi(2) < (28 - y) as f32 * 0.35
                            && (13..27).contains(&y);
                        (blob || drip).then_some(if x < 12 && z < 13 {
                            [0.12, 0.13, 0.14]
                        } else {
                            [0.025, 0.028, 0.032]
                        })
                    }
                    ItemKind::DragonLoot(loot) => dragon_sample(loot, dx, dy, dz),
                    ItemKind::GloomEssence => {
                        let bulb = dx * dx + (dy + 4.0).powi(2) + dz * dz < 68.0;
                        let curl = (dx - 3.0).powi(2) + dz * dz < (28 - y) as f32 * 0.30
                            && (13..27).contains(&y);
                        (bulb || curl).then_some(if x < 14 {
                            [0.70, 0.98, 1.0]
                        } else {
                            [0.14, 0.68, 0.85]
                        })
                    }
                    ItemKind::LightSplitter => {
                        let prism = (dx.abs() + dz.abs()) < (13.0 - dy.abs()) * 0.45;
                        (prism && (4..28).contains(&y)).then_some(if x < 16 {
                            [1.0, 0.88, 0.38]
                        } else {
                            [0.51, 0.84, 1.0]
                        })
                    }
                    ItemKind::RoadAggregate => {
                        let chips = [
                            ([-7.0, -5.0, -4.0], 6.0),
                            ([5.0, -5.0, 4.0], 7.0),
                            ([0.0, 2.0, -1.0], 5.0),
                        ];
                        chips.iter().enumerate().find_map(|(i, (c, radius))| {
                            (((dx - c[0]).abs() + (dy - c[1]).abs() * 1.2 + (dz - c[2]).abs())
                                < *radius)
                                .then_some(if i == 1 {
                                    [0.44, 0.46, 0.49]
                                } else {
                                    [0.61, 0.62, 0.63]
                                })
                        })
                    }
                    _ => return Err("Not a forest material item".into()),
                };
                if let Some(color) = color {
                    s.cell([x, y, z], shade(color, [x / 2, y / 2, z / 2], 0.045));
                }
            }
        }
    }
    if item == ItemKind::BloodBerry {
        s.cuboid([15, 24, 15], [17, 28, 17], [0.30, 0.17, 0.08]);
        s.cuboid([9, 25, 15], [15, 26, 19], [0.15, 0.28, 0.12]);
    }
    Ok(vec![s.faceted_mesh(material)?])
}

fn dragon_sample(
    loot: terra_core::creatures::dragons::DragonLoot,
    x: f32,
    y: f32,
    z: f32,
) -> Option<[f32; 3]> {
    use terra_core::creatures::dragons::DragonLoot::*;
    let blue = [0.11, 0.54, 0.90];
    let violet = [0.46, 0.12, 0.66];
    let amber = [0.88, 0.51, 0.19];
    let (inside, color) = match loot {
        DarkScale | EarthScale | WaterScale => {
            let body = x.abs() * 0.87 + (y - 1.0).abs() * 0.65 < 10.0
                && (z + x * 0.14).abs() < 2.8 - (y.abs() * 0.08);
            (
                body,
                match loot {
                    DarkScale => violet,
                    EarthScale => amber,
                    _ => blue,
                },
            )
        }
        Fang | Claw | Horn => {
            let t = (y + 12.) / 25.;
            let r = (1. - t) * 4.6 + 0.4;
            (
                (0.0..1.0).contains(&t) && (x - 6. * t * t).powi(2) + z * z < r * r,
                if y < -7. {
                    [0.45, 0.31, 0.24]
                } else {
                    [0.94, 0.82, 0.63]
                },
            )
        }
        Hide | Membrane => (
            (x.abs() * 0.7 + y.abs() * 0.63) < 12.
                && (z - 0.16 * x - 0.4 * (y * 0.5).sin()).abs() < 1.5,
            if loot == Hide {
                [0.39, 0.17, 0.10]
            } else {
                violet
            },
        ),
        ShadowHeart | WaterHeart => (
            ((x.abs() - 3.6).powi(2) + (y - 3.).powi(2) + z * z < 38.)
                || (y < 2. && x.abs() + z.abs() < 8. + y * 0.7),
            if loot == WaterHeart { blue } else { violet },
        ),
        Blood => {
            let r = if y < 1. { 5.5 } else { 2.5 };
            (
                y.abs() < 9. && x * x + z * z < r * r,
                if y < 0. {
                    [0.54, 0.06, 0.45]
                } else {
                    [0.65, 0.78, 0.9]
                },
            )
        }
        RawMeat | CookedMeat => (
            (x * 0.9).powi(2) + (y * 1.1).powi(2) + (z * 1.7).powi(2) < 80.,
            if loot == RawMeat {
                [0.76, 0.17, 0.19]
            } else {
                [0.53, 0.26, 0.13]
            },
        ),
        Dust => {
            let r = (6. - y) * 0.78;
            (y > -8. && x * x + z * z < r * r, amber)
        }
        ShadowCore | EarthCrystal | ShadowCrystal | IceEssence | WaterEssence => (
            (x - y * 0.18).abs() + z.abs() + 0.50 * y.abs() < 7.3,
            match loot {
                EarthCrystal => amber,
                IceEssence | WaterEssence => blue,
                _ => violet,
            },
        ),
    };
    inside.then_some(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_items_have_distinct_finite_closed_volume_previews() {
        let mut fingerprints = std::collections::BTreeSet::new();
        for item in [
            ItemKind::BloodBerry,
            ItemKind::CrimsonMushroom,
            ItemKind::ThornTwig,
            ItemKind::BloodCrystal,
            ItemKind::DarkSap,
            ItemKind::BloodLeaf,
            ItemKind::BloodSpore,
            ItemKind::BloodSlimeGel,
            ItemKind::Bone,
            ItemKind::Bitumen,
            ItemKind::RoadAggregate,
            ItemKind::GloomEssence,
            ItemKind::LightSplitter,
        ] {
            let batches = batches(item, 0).unwrap();
            assert_eq!(batches.len(), 1);
            let b = &batches[0];
            assert!(!b.indices.is_empty());
            assert!(b.positions.len() < 20_000, "{item:?}");
            assert!(b.indices.iter().all(|&i| (i as usize) < b.positions.len()));
            assert!(
                b.positions
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (0.0..=1.1).contains(v))
            );
            let signature = b
                .positions
                .iter()
                .flatten()
                .chain(b.colors.iter().flatten())
                .fold(0u64, |h, v| h.wrapping_mul(31) ^ u64::from(v.to_bits()));
            assert!(fingerprints.insert(signature), "distinct {item:?}");
        }
    }
}
