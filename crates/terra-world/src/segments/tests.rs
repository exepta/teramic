use super::*;

#[test]
fn every_clicked_cell_extends_only_the_free_tip_and_requires_loaded_support() {
    for material in [51, 52, 53] {
        for hanging in [false, true] {
            let root = if hanging { -11 } else { -18 };
            let block = |p: [i64; 3]| {
                Some(if p[1] == root {
                    3
                } else if (-17..=-12).contains(&p[1]) {
                    material
                } else {
                    0
                })
            };
            for y in -17..=-12 {
                assert_eq!(
                    extension([-17, y, -1], material, block),
                    Some([-17, if hanging { -18 } else { -11 }, -1])
                );
            }
            assert_eq!(
                extension([-17, -15, -1], material, |p| {
                    if p[1] == root { None } else { block(p) }
                }),
                None
            );
            assert_eq!(
                extension([-17, -15, -1], material, |p| {
                    if p[1] == -11 || p[1] == -18 {
                        Some(3)
                    } else {
                        block(p)
                    }
                }),
                None
            );
        }
    }
    assert_eq!(
        extension([0, 0, 0], 51, |p| Some(if p[1] < 0 {
            3
        } else if p[1] < MAX_CHAIN {
            51
        } else {
            0
        })),
        None
    );
}

#[test]
fn mineral_shafts_remain_broad_until_the_short_terminal_bevel() {
    let root = Profile {
        below: 0,
        above: 9,
        hanging: false,
    };
    let tip = Profile {
        below: 9,
        above: 0,
        hanging: false,
    };
    assert!(radius(root, 1.0) > 0.4);
    assert!(radius(tip, 0.55) > 0.3);
    assert!(radius(tip, 1.0) > 0.12);
    for material in [51, 52, 53] {
        let mut count = 0;
        visit(material, [0; 3], root, false, |_, _| count += 1);
        assert_eq!(count, 56, "bounded stepped silhouette in the near batch");
        let mut coarse_count = 0;
        visit(material, [0; 3], root, true, |_, _| coarse_count += 1);
        assert_eq!(coarse_count, 24, "distant shafts retain the cheap profile");
    }
}

#[test]
fn all_segments_have_finite_bounded_non_degenerate_geometry_with_outward_crystal_normals() {
    for material in 50..=56 {
        for coarse in [false, true] {
            let mut triangles = 0;
            visit(
                material,
                [-17, -16, -1],
                Profile::default(),
                coarse,
                |triangle, color| {
                    triangles += 1;
                    assert!(
                        triangle
                            .into_iter()
                            .flatten()
                            .all(|v| v.is_finite() && (-0.05..=1.05).contains(&v))
                    );
                    assert!(
                        color
                            .into_iter()
                            .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
                    );
                    let normal = crate::ore_shapes::normal(triangle);
                    assert!(normal.iter().map(|v| v * v).sum::<f32>() > 0.99);
                    if is_crystal(material) {
                        let center = std::array::from_fn::<_, 3, _>(|a| {
                            triangle.iter().map(|v| v[a]).sum::<f32>() / 3.0 - 0.5
                        });
                        assert!(
                            center.iter().zip(normal).map(|(v, n)| v * n).sum::<f32>() > 0.0,
                            "inward triangle {triangle:?}"
                        );
                    }
                },
            );
            assert!(triangles > 0 && triangles <= 180);
        }
    }
}

#[test]
fn taper_and_underwater_wind_are_continuous_across_negative_section_boundaries() {
    let mut blocks = std::collections::HashMap::new();
    blocks.insert([-1, -18, -17], 3);
    for y in -17..=-13 {
        blocks.insert([-1, y, -17], 50);
    }
    let lower = profile([-1, -17, -17], 50, |p| {
        Some(blocks.get(&p).copied().unwrap_or(0))
    });
    let upper = profile([-1, -16, -17], 50, |p| {
        Some(blocks.get(&p).copied().unwrap_or(0))
    });
    assert_eq!((lower.below, lower.above, lower.hanging), (0, 4, false));
    assert_eq!(water_weight(lower, 0.0), 0.0);
    assert_eq!(water_weight(lower, 1.0), water_weight(upper, 0.0));
    assert!((radius(lower, 1.0) - radius(upper, 0.0)).abs() < 0.00001);
    for time in [0.0, 2.7, 50.0] {
        assert_eq!(
            displace([-0.5, -17.0, -16.5], 0.0, time),
            [-0.5, -17.0, -16.5]
        );
        assert_eq!(
            displace([-0.5, -16.0, -16.5], water_weight(lower, 1.0), time),
            displace([-0.5, -16.0, -16.5], water_weight(upper, 0.0), time)
        );
    }
    blocks.remove(&[-1, -18, -17]);
    blocks.insert([-1, -12, -17], 3);
    assert!(
        profile([-1, -15, -17], 50, |p| Some(
            blocks.get(&p).copied().unwrap_or(0)
        ))
        .hanging
    );
}
