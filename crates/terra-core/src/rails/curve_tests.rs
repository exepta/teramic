use super::*;
use std::collections::BTreeSet;

fn gentle() -> CurveSpec {
    CurveSpec::new([[8, 2, 0], [8, 2, 64], [40, 6, 128], [72, 10, 192]]).unwrap()
}
fn occupied(piece: Piece) -> BTreeSet<[i64; 3]> {
    piece
        .footprint()
        .iter()
        .map(|cell| std::array::from_fn(|axis| piece.anchor[axis] + i64::from(cell[axis])))
        .collect()
}

#[test]
fn true_cubic_sampling_uses_every_control_point_instead_of_selecting_a_prefab() {
    let curve = gentle();
    let mut changed = curve;
    changed.points[1][2] += 16;
    assert!(changed.valid());
    let first = Piece::new_curve([-17, 16, -17], curve).unwrap();
    let second = Piece::new_curve(first.anchor, changed).unwrap();
    assert_eq!(first.material, STRAIGHT);
    assert_ne!(first.paths()[0].sample(0.5), second.paths()[0].sample(0.5));
    assert_ne!(first.local_geometry(), second.local_geometry());
    assert_ne!(first.local_surfaces(), second.local_surfaces());
    assert!(first.paths()[0].length() > 12.0);
    assert!(first.paths()[0].length() < MAX_CURVE_LENGTH);
    assert!(
        !Kind::ALL
            .into_iter()
            .map(|kind| Piece::new(first.anchor, kind.material(), 0)
                .unwrap()
                .local_geometry())
            .any(|geometry| geometry == first.local_geometry())
    );
    let parameters = curve.parameters().unwrap();
    assert!(parameters.len() <= MAX_CURVE_SAMPLES);
    assert_eq!(parameters[0], 0.0);
    assert_eq!(parameters[parameters.len() - 1], 1.0);
    for sample in parameters.windows(2) {
        let start = curve.sample(sample[0]).position;
        let end = curve.sample(sample[1]).position;
        assert!(
            (0..3)
                .map(|axis| (end[axis] - start[axis]).powi(2))
                .sum::<f64>()
                .sqrt()
                <= 0.251
        );
    }
}

#[test]
fn bezier_ports_preserve_exact_sixteenths_and_primitive_diagonal_grades() {
    let diagonal =
        CurveSpec::new([[0, 2, 0], [64, 18, 64], [128, 34, 128], [192, 50, 192]]).unwrap();
    let piece = Piece::new_curve([-32, 16, -16], diagonal).unwrap();
    let ports = piece.ports();
    assert_eq!(ports[0].tangent, [-4, -1, -4]);
    assert_eq!(ports[1].tangent, [4, 1, 4]);
    assert_eq!(
        ports[1].position.0,
        [-32 * 16 + 192, 16 * 16 + 50, -16 * 16 + 192]
    );
    for (index, t) in [(0, 0.0), (1, 1.0)] {
        assert_eq!(
            piece.paths()[0].sample(t).position,
            ports[index].position.position()
        );
    }
    let decoded: Piece = serde_json::from_str(&serde_json::to_string(&piece).unwrap()).unwrap();
    assert_eq!(decoded, piece);
    let legacy: Piece =
        serde_json::from_str(r#"{"anchor":[0,0,0],"material":196,"facing":0,"switched":false}"#)
            .unwrap();
    assert_eq!(legacy.curve, None);
    assert!(legacy.valid());
}

#[test]
fn arbitrary_length_s_bend_joins_cardinal_ports_without_voxel_overlap_in_all_yaws() {
    for facing in 0..4 {
        let old = Piece::new([-17, 16, -17], STRAIGHT, facing).unwrap();
        let start = old.ports()[1];
        let start_position = start.position.position();
        let direction = start.tangent.map(f64::from);
        let anchor = std::array::from_fn(|axis| {
            (start_position[axis] + direction[axis] * if axis == 1 { 0.0 } else { 1e-5 }).floor()
                as i64
        });
        let right = [direction[2], 0.0, -direction[0]];
        let target = FixedPoint(std::array::from_fn(|axis| {
            start.position.0[axis] + (direction[axis] * 12.0 + right[axis] * 4.0) as i64 * 16
        }));
        let end = Port {
            position: target,
            tangent: start.tangent.map(|value| -value),
            index: 0,
            connectable: true,
        };
        let curve = CurveSpec::between(start, end, anchor).unwrap();
        let connection = Piece::new_curve(anchor, curve).unwrap();
        assert!(start.connects(connection.ports()[0]));
        assert!(end.connects(connection.ports()[1]));
        assert!(
            occupied(old).is_disjoint(&occupied(connection)),
            "facing{facing}"
        );
        let next_start = Piece::new([0; 3], STRAIGHT, facing).unwrap().ports()[0]
            .position
            .0;
        let next_anchor =
            std::array::from_fn(|axis| (end.position.0[axis] - next_start[axis]) / 16);
        let next = Piece::new(next_anchor, STRAIGHT, facing).unwrap();
        assert!(connection.ports()[1].connects(next.ports()[0]));
        assert!(
            occupied(connection).is_disjoint(&occupied(next)),
            "end facing{facing}"
        );
        let network = RouteNetwork::from_pieces([old, connection, next]).unwrap();
        let route = network
            .route(
                Endpoint {
                    anchor: old.anchor,
                    port: 0,
                },
                Endpoint {
                    anchor: next.anchor,
                    port: 1,
                },
                16,
            )
            .unwrap();
        assert_eq!(route.legs.len(), 3);
        assert!((route.length() - (8.0 + connection.paths()[0].length())).abs() < 0.002);
    }
}

#[test]
fn curved_collision_support_and_surface_data_share_a_bounded_owned_cache() {
    let piece = Piece::new_curve([0; 3], gentle()).unwrap();
    assert!(Arc::ptr_eq(
        &piece.local_geometry(),
        &piece.local_geometry()
    ));
    assert!(Arc::ptr_eq(
        &piece.local_surfaces(),
        &piece.local_surfaces()
    ));
    assert!(piece.footprint().len() < 512);
    for cell in piece.footprint().iter() {
        assert!(piece.contains_offset(*cell));
        let boxes = piece.cell_boxes(*cell);
        assert!(!boxes.is_empty());
        assert!(boxes.iter().all(|shape| shape.valid()));
        assert!(Arc::ptr_eq(&boxes, &piece.cell_boxes(*cell)));
    }
    for point in piece.supports().iter() {
        assert!(piece.contains_offset(point.cell));
        assert!(!piece.contains_offset(point.support));
    }
    for surface in piece.local_surfaces().iter() {
        assert!(surface.area() > 0.0);
        assert!(
            surface
                .points
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        let plane = surface.points[0]
            .iter()
            .zip(surface.normal)
            .map(|(value, normal)| value * normal)
            .sum::<f64>();
        assert!(surface.points.iter().all(|point| {
            (point
                .iter()
                .zip(surface.normal)
                .map(|(value, normal)| value * normal)
                .sum::<f64>()
                - plane)
                .abs()
                < 1e-8
        }));
    }
    let initial = CurveSpec::new([[8, 2, 0], [8, 2, 33], [8, 2, 213], [8, 2, 377]]).unwrap();
    let geometry = Piece::new_curve([0; 3], initial).unwrap().local_geometry();
    let retained = Arc::downgrade(&geometry);
    drop(geometry);
    for offset in 0..DYNAMIC_CACHE_LIMIT + 8 {
        let curve = CurveSpec::new([
            [8, 2, 0],
            [8, 2, 40 + offset as i32],
            [8, 2, 180],
            [8, 2, 256],
        ])
        .unwrap();
        assert!(
            !Piece::new_curve([0; 3], curve)
                .unwrap()
                .local_geometry()
                .is_empty()
        );
    }
    assert!(
        retained.upgrade().is_none(),
        "evicted layouts must not leak static geometry"
    );
}

#[test]
fn invalid_tight_steep_reversed_and_oversized_curves_fail_before_geometry() {
    for controls in [
        [[8, 2, 0], [8, 2, 0], [8, 2, 64], [8, 2, 128]],
        [[8, 2, 0], [8, 2, 16], [24, 2, 32], [40, 2, 32]],
        [[8, 2, 0], [8, 66, 64], [8, 130, 128], [8, 194, 192]],
        [[8, 2, 0], [8, 2, -64], [8, 2, 128], [8, 2, 192]],
        [[8, 2, 0], [8, 2, 160], [8, 2, 320], [8, 2, 544]],
        [[8, 2, 0], [i32::MAX, 2, 64], [8, 2, 128], [8, 2, 192]],
    ] {
        let spec = CurveSpec { points: controls };
        assert!(!spec.valid(), "{spec:?}");
        assert!(Piece::new_curve([0; 3], spec).is_none());
    }
    let mut wrong_material = Piece::new([0; 3], CURVE_SMALL, 0).unwrap();
    wrong_material.curve = Some(gentle());
    assert!(!wrong_material.valid());
}

#[test]
fn adaptive_handles_fit_asymmetric_right_angle_turns_without_changing_existing_curves() {
    let anchor = [-33, 16, -49];
    let rotate = |[x, y, z]: [i32; 3], facing| match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    };
    for facing in 0..4 {
        let first = [8, 2, 8];
        let delta = rotate([52, 0, 112], facing);
        let last = std::array::from_fn(|axis| first[axis] + delta[axis]);
        let start = Port {
            position: FixedPoint(std::array::from_fn(|axis| {
                anchor[axis] * 16 + i64::from(first[axis])
            })),
            tangent: rotate([0, 0, 1], facing).map(|value| value as i16),
            index: 1,
            connectable: true,
        };
        let end = Port {
            position: FixedPoint(std::array::from_fn(|axis| {
                anchor[axis] * 16 + i64::from(last[axis])
            })),
            tangent: rotate([-1, 0, 0], facing).map(|value| value as i16),
            index: 0,
            connectable: true,
        };
        let old_handle = 49;
        let old = CurveSpec {
            points: [
                first,
                std::array::from_fn(|axis| {
                    first[axis] + i32::from(start.tangent[axis]) * old_handle
                }),
                std::array::from_fn(|axis| last[axis] + i32::from(end.tangent[axis]) * old_handle),
                last,
            ],
        };
        assert_eq!(old.validate(), Err(CurveError::TooTight));
        let fitted = CurveSpec::between_checked(start, end, anchor).unwrap();
        let metrics = fitted.validate().unwrap();
        assert!(metrics.min_radius >= MIN_CURVE_RADIUS);
        assert!(metrics.length <= MAX_CURVE_LENGTH);
        assert!(metrics.max_grade <= MAX_CURVE_GRADE);
        assert_ne!(fitted.points, old.points);
        let piece = Piece::new_curve(anchor, fitted).unwrap();
        assert!(start.connects(piece.ports()[0]));
        assert!(end.connects(piece.ports()[1]));
    }
    let start = Port {
        position: FixedPoint([8, 2, 0]),
        tangent: [0, 0, 1],
        index: 1,
        connectable: true,
    };
    let end = Port {
        position: FixedPoint([72, 2, 192]),
        tangent: [0, 0, -1],
        index: 0,
        connectable: true,
    };
    assert_eq!(
        CurveSpec::between_checked(start, end, [0; 3])
            .unwrap()
            .points,
        [[8, 2, 0], [8, 2, 81], [72, 2, 111], [72, 2, 192]]
    );
    let reversed = Port {
        tangent: [0, 0, -1],
        ..start
    };
    assert_eq!(
        CurveSpec::between_checked(reversed, end, [0; 3]),
        Err(CurveError::Reverses)
    );
}

#[test]
fn compact_one_block_rises_have_flat_exact_ports_and_bounded_smooth_curvature() {
    for span in [4, 6] {
        let start = Port {
            position: FixedPoint([8, 2, 0]),
            tangent: [0, 0, 1],
            index: 1,
            connectable: true,
        };
        let end = Port {
            position: FixedPoint([8, 18, span * 16]),
            tangent: [0, 0, -1],
            index: 0,
            connectable: true,
        };
        let curve = CurveSpec::between_checked(start, end, [0; 3]).unwrap();
        let metrics = curve.validate().unwrap();
        assert!(metrics.max_grade <= MAX_CURVE_GRADE);
        assert!(metrics.min_radius >= MIN_CURVE_RADIUS);
        assert_eq!(curve.points[0][1], curve.points[1][1]);
        assert_eq!(curve.points[2][1], curve.points[3][1]);
        assert!(metrics.length > span as f64);
        if span == 4 {
            assert!(metrics.max_grade > 0.25);
        }
        let piece = Piece::new_curve([0; 3], curve).unwrap();
        assert!(start.connects(piece.ports()[0]));
        assert!(end.connects(piece.ports()[1]));
    }
    assert_eq!(
        Piece::new([0; 3], CURVE_SMALL, 0).unwrap().paths()[0].length(),
        std::f64::consts::FRAC_PI_2 * 4.5
    );
}

#[test]
fn new_starters_are_exactly_one_by_three_in_every_heading_while_saved_native_straights_stay_four_long()
 {
    let anchor = [-17, 16, -17];
    for facing in 0..4 {
        let seed = Piece::seed(anchor, facing).unwrap();
        assert_eq!(seed.material, STRAIGHT);
        assert!(seed.curve.is_some());
        assert_eq!(seed.paths()[0].length(), 1.0);
        let actual: BTreeSet<_> = seed.footprint().iter().copied().collect();
        let expected: BTreeSet<_> = if facing % 2 == 0 {
            [[-1, 0, 0], [0, 0, 0], [1, 0, 0]].into_iter().collect()
        } else {
            [[0, 0, -1], [0, 0, 0], [0, 0, 1]].into_iter().collect()
        };
        assert_eq!(actual, expected, "heading {facing}");
        assert_eq!(seed.supports().len(), 3);
        for support in seed.supports().iter() {
            assert_eq!(support.support[1], -1);
        }
        let ports = seed.ports();
        assert_eq!(
            ports[1]
                .position
                .0
                .iter()
                .zip(ports[0].position.0)
                .map(|(end, start)| end.abs_diff(start))
                .sum::<u64>(),
            16
        );
        assert_eq!(ports[0].tangent, ports[1].tangent.map(|value| -value));
        let saved = serde_json::to_string(&seed).unwrap();
        assert_eq!(serde_json::from_str::<Piece>(&saved).unwrap(), seed);
        let legacy = Piece::new(anchor, STRAIGHT, facing).unwrap();
        assert_eq!(legacy.curve, None);
        assert_eq!(legacy.paths()[0].length(), 4.0);
    }
    assert!(Piece::seed(anchor, 4).is_none());
}

#[test]
fn new_horizontal_track_requires_room_but_legacy_curves_remain_readable() {
    let small = CurveSpec::new([[0, 2, 0], [0, 2, 53], [43, 2, 96], [96, 2, 96]]).unwrap();
    let piece = Piece::new_curve([0; 3], small).unwrap();
    let ports = piece.ports();
    let start = Port {
        tangent: ports[0].tangent.map(|v| -v),
        ..ports[0]
    };
    let end = Port {
        tangent: ports[1].tangent.map(|v| -v),
        ..ports[1]
    };
    assert!(CurveSpec::between_checked(start, end, [0; 3]).is_ok());
    assert_eq!(
        CurveSpec::between_for_build(start, end, [0; 3]),
        Err(CurveError::TooTight)
    );
    let broad = CurveSpec::new([[0, 2, 0], [0, 2, 159], [129, 2, 288], [288, 2, 288]]).unwrap();
    let ports = Piece::new_curve([0; 3], broad).unwrap().ports();
    assert!(
        CurveSpec::between_for_build(
            Port {
                tangent: ports[0].tangent.map(|v| -v),
                ..ports[0]
            },
            Port {
                tangent: ports[1].tangent.map(|v| -v),
                ..ports[1]
            },
            [0; 3]
        )
        .is_ok()
    );
}

#[test]
fn section_occupancy_survives_eviction_of_heavy_rail_geometry() {
    let curve = CurveSpec::new([[8, 2, 0], [8, 2, 16], [8, 2, 96], [8, 2, 256]]).unwrap();
    let piece = Piece::new_curve([0; 3], curve).unwrap();
    let original = piece.footprint();
    let collision = piece.cell_boxes(original[0]);
    for index in 1..=DYNAMIC_CACHE_LIMIT as i32 + 16 {
        let curve = CurveSpec::new([
            [8, 2, 0],
            [8, 2, 16 + index],
            [8, 2, 96 + index],
            [8, 2, 256],
        ])
        .unwrap();
        let _ = Piece::new_curve([0; 3], curve).unwrap().local_geometry();
    }
    assert!(std::sync::Arc::ptr_eq(&original, &piece.footprint()));
    assert!(original.iter().all(|offset| piece.contains_offset(*offset)));
    assert!(std::sync::Arc::ptr_eq(
        &collision,
        &piece.cell_boxes(original[0])
    ));
}
