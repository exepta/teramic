use super::*;

#[test]
fn uneven_fans_share_faces_and_keep_free_tips_on_floor_and_ceiling() {
    for material in [51, 52, 53] {
        for dy in [-1, 1] {
            let read = |[x, y, z]: [i64; 3]| {
                let d = (y + 16) * dy;
                let length = match (x, z) {
                    (-17, -17) => 10,
                    (-16, -17) => 7,
                    (-17, -16) => 5,
                    (-16, -16) => 3,
                    _ => 0,
                };
                Some(if d == -1 {
                    3
                } else if (0..length).contains(&d) {
                    material
                } else {
                    0
                })
            };
            let points = |x, z, layer, coarse, axis, coordinate| {
                let pos = [x, -16 + layer * dy, z];
                let profile = profile(pos, material, read);
                assert_eq!(profile.hanging, dy < 0);
                let mut rim = std::collections::BTreeSet::new();
                visit(material, pos, profile, coarse, |tri, _| {
                    assert!(crate::ore_shapes::normal(tri).iter().all(|v| v.is_finite()));
                    if axis == 0 && layer == 1 {
                        assert!(tri.iter().any(|p| p[axis] != coordinate), "buried face");
                    }
                    for p in tri.into_iter().filter(|p| p[axis] == coordinate) {
                        // Four touching cells have no surface along the interior
                        // grid edge. A neighbouring layer may terminate there.
                        if p[0].fract() == 0.0 && p[2].fract() == 0.0 {
                            continue;
                        }
                        rim.insert(std::array::from_fn::<_, 3, _>(|i| {
                            pos[i] * 10_000 + (p[i] * 10_000.).round() as i64
                        }));
                    }
                });
                rim
            };
            // The cursor can pick outer faces. Fully interior edges need no vertices:
            // compare the exposed outer rim of the joined face across a section border.
            for coarse in [false, true] {
                let left = points(-17, -17, 1, coarse, 0, 1.);
                let right = points(-16, -17, 1, coarse, 0, 0.);
                assert!(!left.is_empty());
                assert_eq!(left, right);
                assert!(points(-17, -17, 9, coarse, 0, 1.).is_empty());
                for layer in 0..9 {
                    let top = f32::from(dy > 0);
                    // Horizontal closures against the rock are the only caps on a run.
                    assert_eq!(
                        points(-17, -17, layer, coarse, 1, top),
                        points(-17, -17, layer + 1, !coarse, 1, 1. - top),
                    );
                }
            }
        }
    }
}

#[test]
fn seagrass_stalks_meet_exactly_across_negative_vertical_section_seams() {
    for coarse in [false, true] {
        let seam = |position: [i64; 3], profile: Profile, height: f32| {
            let mut points = std::collections::BTreeSet::new();
            visit(50, position, profile, coarse, |triangle, _| {
                for p in triangle {
                    if (p[1] - height).abs() < 1e-6 {
                        points.insert((
                            (p[0] * 1_000_000.0).round() as i32,
                            (p[2] * 1_000_000.0).round() as i32,
                        ));
                    }
                }
            });
            points
        };
        let lower = seam(
            [-17, -17, -1],
            Profile {
                below: 0,
                above: 2,
                hanging: false,
                joined: false,
                fan: [0; 2],
            },
            1.0,
        );
        let upper = seam(
            [-17, -16, -1],
            Profile {
                below: 1,
                above: 1,
                hanging: false,
                joined: false,
                fan: [0; 2],
            },
            0.0,
        );
        assert!(!lower.is_empty());
        assert_eq!(
            lower, upper,
            "every metre must grow from the same rooted stalks"
        );
    }
}

#[test]
fn hanging_plants_need_a_contiguous_loaded_ceiling_chain_not_a_distant_rock() {
    let start = [-17, -17, -1];
    let mut cells = std::collections::HashMap::from([
        ([-17, -17, -1], 54),
        ([-17, -16, -1], 55),
        ([-17, -15, -1], 54),
        ([-17, -14, -1], 3),
    ]);
    assert_eq!(
        hanging_support(start, |p| cells.get(&p).copied()),
        Some(true)
    );
    cells.remove(&[-17, -16, -1]);
    assert_eq!(hanging_support(start, |p| cells.get(&p).copied()), None);
    cells.insert([-17, -16, -1], 0);
    assert_eq!(
        hanging_support(start, |p| cells.get(&p).copied()),
        Some(false)
    );
    for material in [14, 27, 50, 51, 56, 65, 82] {
        cells.insert([-17, -16, -1], material);
        assert_eq!(
            hanging_support(start, |p| cells.get(&p).copied()),
            Some(false)
        );
    }
    let mut reads = 0;
    assert_eq!(
        hanging_support(start, |_| {
            reads += 1;
            Some(54)
        }),
        Some(false)
    );
    assert_eq!(reads, MAX_CHAIN + 1);
}

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
fn salt_calcite_and_ice_have_distinct_rooted_profiles_and_terminal_faces() {
    let root = Profile {
        below: 0,
        above: 9,
        hanging: false,
        joined: false,
        fan: [0; 2],
    };
    let tip = Profile {
        below: 9,
        above: 0,
        hanging: false,
        joined: false,
        fan: [0; 2],
    };
    let mut outlines = std::collections::BTreeSet::new();
    for material in [51, 52, 53] {
        assert!(radius(material, root, 0.) > 0.4);
        assert!(radius(material, tip, 1.) < radius(material, tip, 0.4));
        let mut near = Vec::new();
        visit(material, [-17, -16, -1], root, false, |t, _| near.push(t));
        let mut far = Vec::new();
        visit(material, [-17, -16, -1], root, true, |t, _| far.push(t));
        assert!(near.len() <= 180 && far.len() < near.len());
        let outline = |tris: &Vec<[[f32; 3]; 3]>, y: f32| {
            tris.iter()
                .flatten()
                .filter(|p| p[1] == y)
                .map(|p| ((p[0] * 1e5) as i32, (p[2] * 1e5) as i32))
                .collect::<std::collections::BTreeSet<_>>()
        };
        assert_eq!(outline(&near, 1.), outline(&far, 1.));
        let mut next = Vec::new();
        visit(
            material,
            [-17, -15, -1],
            Profile {
                below: 1,
                above: 8,
                hanging: false,
                joined: false,
                fan: [0; 2],
            },
            false,
            |t, _| next.push(t),
        );
        assert_eq!(outline(&near, 1.), outline(&next, 0.));
        outlines.insert(outline(&near, 1.));
    }
    assert_eq!(outlines.len(), 3);
}

#[test]
fn all_segments_have_finite_bounded_non_degenerate_geometry_with_outward_crystal_normals() {
    for material in 50..=56 {
        for coarse in [false, true] {
            let mut triangles = 0;
            let mut halite_edges = std::collections::BTreeMap::new();
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
                    if material == 51 {
                        // A daughter prism faces outwards from its own axis.
                        // Every directed edge, including each cap, must be
                        // matched by an oppositely wound neighbouring face.
                        let vertices = triangle.map(|p| p.map(f32::to_bits));
                        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                            let (a, b) = (vertices[a], vertices[b]);
                            *halite_edges.entry((a.min(b), a.max(b))).or_insert(0_i32) +=
                                if a < b { 1 } else { -1 };
                        }
                    } else if is_crystal(material) {
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
            assert!(halite_edges.values().all(|count| *count == 0));
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
    assert!((radius(51, lower, 1.0) - radius(51, upper, 0.0)).abs() < 0.00001);
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

#[test]
fn rooted_minerals_keep_their_orientation_through_a_negative_section_seam() {
    let p = [-1, -15, -2];
    let read = |p: [i64; 3]| {
        Some(match p[1] {
            -22..=-13 => 51,
            -12 => 3,
            _ => 0,
        })
    };
    assert_eq!(mineral_support(p, 51, read), Some(true));
    assert!(profile(p, 51, read).hanging);
    assert_eq!(
        mineral_support(p, 51, |v| if v[1] == -14 { Some(0) } else { read(v) }),
        Some(false)
    );
    assert_eq!(
        mineral_support(p, 51, |v| if v[1] == -12 { None } else { read(v) }),
        None
    );
    let joined = profile(p, 52, |v| {
        Some(match v[1] {
            -22..=-13 => 52,
            -23 | -12 => 3,
            _ => 0,
        })
    });
    assert!(joined.joined);
}

#[test]
fn signed_halite_caps_face_towards_the_air_for_both_growth_directions() {
    for hanging in [true, false] {
        let mut tip = 0;
        let mut base = 0;
        visit(
            51,
            [9, -16, -4],
            Profile {
                hanging,
                ..Default::default()
            },
            false,
            |triangle, _| {
                let normal = crate::ore_shapes::normal(triangle);
                if triangle.iter().all(|p| p[1] == 1.) {
                    assert!(normal[1] > 0.99);
                    tip += 1;
                }
                if triangle.iter().all(|p| p[1] == 0.) {
                    assert!(normal[1] < -0.99);
                    base += 1;
                }
            },
        );
        assert!(if hanging {
            tip > base && base > 0
        } else {
            base > tip && tip > 0
        });
    }
}
