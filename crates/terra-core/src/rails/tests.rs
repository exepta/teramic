use super::*;
use std::collections::BTreeSet;

fn piece(material: u16, anchor: [i64; 3], facing: u8) -> Piece {
    Piece::new(anchor, material, facing).unwrap()
}
fn endpoint(piece: Piece, port: u8) -> Endpoint {
    Endpoint {
        anchor: piece.anchor,
        port,
    }
}
fn occupied(piece: Piece) -> BTreeSet<[i64; 3]> {
    piece
        .footprint()
        .iter()
        .map(|cell| std::array::from_fn(|axis| piece.anchor[axis] + i64::from(cell[axis])))
        .collect()
}
fn assert_joint(first: Piece, first_port: u8, second: Piece, second_port: u8) {
    assert!(first.ports()[first_port as usize].connects(second.ports()[second_port as usize]));
    assert!(
        occupied(first).is_disjoint(&occupied(second)),
        "module cells overlap at {first:?} -> {second:?}"
    );
}

#[test]
fn material_tags_poses_and_span_limits_are_explicit() {
    for (index, kind) in Kind::ALL.into_iter().enumerate() {
        assert_eq!(kind.material(), MATERIALS[index]);
        assert_eq!(Kind::from_material(kind.material()), Some(kind));
        for facing in 0..8 {
            assert_eq!(
                valid_facing(kind.material(), facing),
                facing < 4 || kind.is_switch()
            );
            if let Some(piece) = Piece::from_facing([-16, 0, -17], kind.material(), facing) {
                assert_eq!(piece.encoded_facing(), facing);
                assert!(piece.valid());
            }
        }
    }
    assert!(!is_rail(195));
    assert!(!is_rail(206));
    assert!(Piece::new([i64::MAX; 3], STRAIGHT, 0).is_none());
    assert!(Piece::new([i64::MIN; 3], STRAIGHT, 0).is_none());
    assert!(Piece::new([MAX_COORDINATE; 3], STRAIGHT, 0).is_none());
}

#[test]
fn every_sample_endpoint_agrees_with_its_exact_fixed_port_and_grade() {
    for kind in Kind::ALL {
        for facing in 0..4 {
            let piece = piece(kind.material(), [-17, 13, -16], facing);
            let ports = piece.ports();
            for path in piece.paths() {
                for (index, t) in [(0, 0.0), (1, 1.0)] {
                    let sample = path.sample(t);
                    let port = ports[path.ports[index] as usize];
                    for axis in 0..3 {
                        assert!(
                            (sample.position[axis] - port.position.position()[axis]).abs() < 1.0e-8
                        );
                    }
                    let tangent = port.tangent.map(f64::from);
                    let magnitude = tangent
                        .iter()
                        .map(|value| value * value)
                        .sum::<f64>()
                        .sqrt();
                    for (axis, value) in tangent.iter().enumerate() {
                        let expected = *value / magnitude * if index == 0 { -1.0 } else { 1.0 };
                        assert!((sample.tangent[axis] - expected).abs() < 1.0e-10);
                    }
                }
            }
        }
    }
}

#[test]
fn curves_attach_on_integer_anchors_in_all_yaws_and_across_negative_chunk_seams() {
    for material in [CURVE_SMALL, CURVE_LARGE] {
        for facing in 0..4 {
            let curve = piece(material, [-16, 4, -17], facing);
            let exit = curve.ports()[1];
            let next_facing = (facing + 1) % 4;
            let origin = piece(STRAIGHT, [0; 3], next_facing).ports()[0].position.0;
            let difference =
                std::array::from_fn::<_, 3, _>(|axis| exit.position.0[axis] - origin[axis]);
            assert!(difference.iter().all(|value| value % FIXED_SCALE == 0));
            let next = piece(
                STRAIGHT,
                difference.map(|value| value / FIXED_SCALE),
                next_facing,
            );
            assert_joint(curve, 1, next, 0);
            let network = RouteNetwork::from_pieces([curve, next]).unwrap();
            assert_eq!(
                network
                    .route(endpoint(curve, 0), endpoint(next, 1), 16)
                    .unwrap()
                    .legs
                    .len(),
                2
            );
        }
    }
}

#[test]
fn slope_transitions_preserve_height_grade_and_disjoint_cell_ownership() {
    let entry = piece(SLOPE_ENTRY, [-16, 0, -16], 0);
    let slope = piece(SLOPE, [-16, 1, -8], 0);
    let exit = piece(SLOPE_EXIT, [-16, 2, -4], 0);
    let flat = piece(STRAIGHT, [-16, 3, 4], 0);
    for (first, second) in [(entry, slope), (slope, exit), (exit, flat)] {
        assert_joint(first, 1, second, 0);
    }
    let network = RouteNetwork::from_pieces([entry, slope, exit, flat]).unwrap();
    let route = network
        .route(endpoint(entry, 0), endpoint(flat, 1), 32)
        .unwrap();
    assert_eq!(route.legs.len(), 4);
    assert!(route.length() > 24.0 && route.length() < 25.0);
    let wrong_grade = piece(STRAIGHT, slope.anchor, 0);
    assert!(!entry.ports()[1].connects(wrong_grade.ports()[0]));
    let wrong_height = piece(SLOPE, [-16, 0, -8], 0);
    assert!(!entry.ports()[1].connects(wrong_height.ports()[0]));
}

#[test]
fn cached_geometry_cells_supports_and_three_wide_flat_tracks_share_one_shape() {
    for kind in Kind::ALL {
        for facing in 0..4 {
            let piece = piece(kind.material(), [0; 3], facing);
            let footprint = piece.footprint();
            assert!(footprint.contains(&[0; 3]));
            assert!(
                !footprint.is_empty()
                    && footprint.len() < if kind == Kind::Turntable { 1024 } else { 256 }
            );
            assert!(Arc::ptr_eq(&piece.footprint(), &piece.footprint()));
            assert!(Arc::ptr_eq(
                &piece.local_geometry(),
                &piece.local_geometry()
            ));
            for &cell in footprint.iter() {
                assert!(contains_offset(
                    piece.material,
                    piece.encoded_facing(),
                    cell
                ));
                if piece.cell_boxes(cell).is_empty() {
                    assert!(
                        kind.is_switch(),
                        "only historic switch reservations may be empty"
                    );
                    assert!(
                        piece.local_geometry().iter().all(|beam| {
                            let bounds = beam.bounds();
                            (0..3).any(|axis| {
                                bounds.maximum[axis].min(f64::from(cell[axis]) + 1.0)
                                    - bounds.minimum[axis].max(f64::from(cell[axis]))
                                    <= 1e-8
                            })
                        }),
                        "empty ownership cannot discard an authored physical beam"
                    );
                }
                assert!(piece.cell_boxes(cell).iter().all(|shape| shape.valid()));
                assert!(Arc::ptr_eq(
                    &piece.cell_boxes(cell),
                    &piece.cell_boxes(cell)
                ));
            }
            for support in piece.supports().iter() {
                assert_eq!(
                    support.support,
                    [support.cell[0], support.cell[1] - 1, support.cell[2]]
                );
                assert!(footprint.contains(&support.cell));
                assert!(!footprint.contains(&support.support));
                assert!(!piece.cell_boxes(support.cell).is_empty());
            }
            assert!(piece.local_geometry().iter().all(|beam| {
                beam.corners()
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            }));
        }
    }
    let flat = piece(STRAIGHT, [0; 3], 0);
    let x_columns = flat
        .footprint()
        .iter()
        .map(|cell| cell[0])
        .collect::<BTreeSet<_>>();
    assert_eq!(x_columns, BTreeSet::from([-1, 0, 1]));
    assert_eq!(flat.footprint().len(), 12);
}

#[test]
fn switch_selection_is_live_and_crossing_tracks_never_change_axis() {
    let switch = piece(SWITCH_RIGHT, [0; 3], 0);
    let main = piece(STRAIGHT, [0, 0, 8], 0);
    let branch = piece(STRAIGHT, [5, 0, 4], 1);
    assert_joint(switch, 1, main, 0);
    assert_joint(switch, 2, branch, 0);
    let mut network = RouteNetwork::from_pieces([switch, main, branch]).unwrap();
    assert!(
        network
            .route(endpoint(switch, 0), endpoint(main, 1), 32)
            .is_ok()
    );
    assert_eq!(
        network.route(endpoint(switch, 0), endpoint(branch, 1), 32),
        Err(RailError::Disconnected)
    );
    network.set_switched(switch.anchor, true).unwrap();
    assert!(
        network
            .route(endpoint(switch, 0), endpoint(branch, 1), 32)
            .is_ok()
    );
    assert_eq!(
        network.route(endpoint(switch, 0), endpoint(main, 1), 32),
        Err(RailError::Disconnected)
    );
    assert_ne!(
        switch.local_geometry(),
        piece(SWITCH_RIGHT, [0; 3], 4).local_geometry()
    );
    for material in [SWITCH_LEFT, SWITCH_RIGHT] {
        for facing in 0..4 {
            assert_eq!(
                piece(material, [0; 3], facing).footprint(),
                piece(material, [0; 3], facing | 4).footprint(),
                "switch reservations must survive a live toggle"
            );
        }
    }
    let crossing = piece(CROSSING, [0; 3], 0);
    let crossing_network = RouteNetwork::from_pieces([crossing]).unwrap();
    assert!(
        crossing_network
            .route(endpoint(crossing, 0), endpoint(crossing, 1), 8)
            .is_ok()
    );
    assert!(
        crossing_network
            .route(endpoint(crossing, 2), endpoint(crossing, 3), 8)
            .is_ok()
    );
    assert_eq!(
        crossing_network.route(endpoint(crossing, 0), endpoint(crossing, 2), 8),
        Err(RailError::Disconnected)
    );
}

#[test]
fn reconstruction_is_deterministic_and_routing_never_jumps_a_missing_or_misaligned_module() {
    let pieces = (0..8)
        .map(|index| piece(STRAIGHT, [-16, 0, -16 + index * 4], 0))
        .collect::<Vec<_>>();
    let mut network = RouteNetwork::from_pieces(pieces.iter().copied()).unwrap();
    let reversed = RouteNetwork::from_pieces(pieces.iter().rev().copied()).unwrap();
    assert_eq!(
        network, reversed,
        "the complete index is independent of insertion order"
    );
    let mut alternating = RouteNetwork::default();
    for index in (0..pieces.len())
        .step_by(2)
        .chain((1..pieces.len()).step_by(2))
    {
        alternating.insert(pieces[index]).unwrap();
    }
    assert_eq!(network, alternating);
    alternating.remove(pieces[3].anchor).unwrap();
    alternating.insert(pieces[3]).unwrap();
    assert_eq!(
        network, alternating,
        "repairing a route restores the same canonical index"
    );
    let start = endpoint(pieces[0], 0);
    let goal = endpoint(pieces[7], 1);
    assert_eq!(
        network.route(start, goal, 32),
        reversed.route(start, goal, 32)
    );
    assert_eq!(
        network.route(start, goal, 32),
        alternating.route(start, goal, 32)
    );
    assert_eq!(
        network.route(start, goal, 1),
        Err(RailError::BudgetExceeded)
    );
    assert_eq!(
        network.route(start, goal, 0),
        Err(RailError::BudgetExceeded)
    );
    let records = network.records().copied().collect::<Vec<_>>();
    let decoded: Vec<Piece> =
        serde_json::from_str(&serde_json::to_string(&records).unwrap()).unwrap();
    assert_eq!(network, RouteNetwork::from_pieces(decoded).unwrap());
    assert_eq!(
        network.nearest_endpoint(pieces[0].ports()[0].position.position(), 0.0),
        Some(start)
    );
    assert!(network.nearest_endpoint([f64::NAN; 3], 4.0).is_none());
    network.remove(pieces[3].anchor).unwrap();
    assert_eq!(network.route(start, goal, 32), Err(RailError::Disconnected));
    network
        .insert(piece(STRAIGHT, [-15, 0, pieces[3].anchor[2]], 0))
        .unwrap();
    assert_eq!(network.route(start, goal, 32), Err(RailError::Disconnected));
    let before = network.clone();
    assert_eq!(network.insert(pieces[0]), Err(RailError::DuplicateAnchor));
    assert_eq!(network, before);
}

#[test]
fn buffer_has_a_terminal_route_but_cannot_connect_through_the_stop() {
    let buffer = piece(BUFFER, [0; 3], 0);
    let after = piece(STRAIGHT, [0, 0, 4], 0);
    let network = RouteNetwork::from_pieces([buffer, after]).unwrap();
    assert!(
        network
            .route(endpoint(buffer, 0), endpoint(buffer, 1), 8)
            .is_ok()
    );
    assert_eq!(
        network.route(endpoint(buffer, 0), endpoint(after, 1), 8),
        Err(RailError::Disconnected)
    );
}

#[test]
fn separate_switch_stands_preserve_every_historic_owned_cell_without_invisible_lever_solids() {
    let left = [
        [-4, 0, 3],
        [-4, 0, 4],
        [-4, 0, 5],
        [-3, 0, 2],
        [-3, 0, 3],
        [-3, 0, 4],
        [-3, 0, 5],
        [-2, 0, 1],
        [-2, 0, 2],
        [-2, 0, 3],
        [-2, 0, 4],
        [-2, 0, 5],
        [-1, 0, 0],
        [-1, 0, 1],
        [-1, 0, 2],
        [-1, 0, 3],
        [-1, 0, 4],
        [-1, 0, 5],
        [-1, 0, 6],
        [-1, 0, 7],
        [0, 0, 0],
        [0, 0, 1],
        [0, 0, 2],
        [0, 0, 3],
        [0, 0, 4],
        [0, 0, 5],
        [0, 0, 6],
        [0, 0, 7],
        [1, 0, 0],
        [1, 0, 1],
        [1, 0, 2],
        [1, 0, 3],
        [1, 0, 4],
        [1, 0, 5],
        [1, 0, 6],
        [1, 0, 7],
        [2, 0, 0],
        [2, 0, 1],
    ];
    for material in [SWITCH_LEFT, SWITCH_RIGHT] {
        for yaw in 0..4 {
            let expected: BTreeSet<_> = left
                .into_iter()
                .map(|[mut x, y, z]| {
                    if material == SWITCH_RIGHT {
                        x = -x;
                    }
                    match yaw {
                        0 => [x, y, z],
                        1 => [z, y, -x],
                        2 => [-x, y, -z],
                        _ => [-z, y, x],
                    }
                })
                .collect();
            for branch in [0, 4] {
                let switch = Piece::new([0; 3], material, yaw | branch).unwrap();
                assert_eq!(
                    switch.footprint().iter().copied().collect::<BTreeSet<_>>(),
                    expected
                );
                assert!(switch.local_geometry().iter().all(|beam| {
                    beam.material != PartMaterial::Red && beam.bounds().maximum[1] <= 0.3125 + 1e-8
                }));
                assert!(
                    switch
                        .supports()
                        .iter()
                        .all(|point| { !switch.cell_boxes(point.cell).is_empty() }),
                    "empty legacy ownership slices must not require phantom floors"
                );
            }
        }
    }
}

#[test]
fn turntable_footprint_is_invariant_and_connects_at_exact_endpoints_in_every_yaw() {
    let table = piece(TURNTABLE, [0; 3], 0);
    for facing in 0..4 {
        let rotated = piece(TURNTABLE, [0; 3], facing);
        assert_eq!(table.footprint(), rotated.footprint());
        assert!((rotated.paths()[0].length() - 25.0).abs() < 1e-9);
        let exit = rotated.ports()[1];
        let base = piece(STRAIGHT, [0; 3], facing).ports()[0];
        let delta: [i64; 3] =
            std::array::from_fn(|axis| exit.position.0[axis] - base.position.0[axis]);
        assert!(delta.iter().all(|value| value % FIXED_SCALE == 0));
        let next = piece(STRAIGHT, delta.map(|value| value / FIXED_SCALE), facing);
        assert_joint(rotated, 1, next, 0);
    }
}

#[test]
fn gentle_turnouts_keep_legacy_records_and_bound_every_branch_heading_below_twenty_degrees() {
    for material in [SWITCH_LEFT, SWITCH_RIGHT] {
        for facing in 0..4 {
            let old = Piece::new([0; 3], material, facing).unwrap();
            assert!(old.curve.is_none());
            let piece = Piece::for_build([0; 3], material, facing).unwrap();
            let switched = Piece::for_build([0; 3], material, facing | 4).unwrap();
            assert!(piece.is_gentle_switch());
            assert_eq!(piece.footprint(), switched.footprint());
            assert_eq!(
                serde_json::from_str::<Piece>(&serde_json::to_string(&piece).unwrap()).unwrap(),
                piece
            );
            let forward = piece.paths()[0].sample(0.).tangent;
            for path in piece.paths() {
                let ports = piece.ports();
                assert_eq!(
                    path.sample(1.).position,
                    ports[usize::from(path.ports[1])].position.position()
                );
                for step in 0..=256 {
                    let tangent = path.sample(step as f64 / 256.).tangent;
                    let angle = (0..3)
                        .map(|i| tangent[i] * forward[i])
                        .sum::<f64>()
                        .clamp(-1., 1.)
                        .acos()
                        .to_degrees();
                    assert!(angle <= 20., "heading={angle}");
                }
            }
            assert!(piece.curve.unwrap().validate().unwrap().min_radius >= 16.);
            assert_eq!(piece.paths()[0].length(), 20.);
            assert!(old.footprint() != piece.footprint());
        }
    }
}
#[test]
fn sixty_four_meter_connections_preserve_quarter_meter_tessellation() {
    let curve = CurveSpec::new([[8, 2, 0], [8, 2, 341], [8, 2, 683], [8, 2, 1024]]).unwrap();
    assert!((curve.length().unwrap() - 64.).abs() < 1e-6);
    let parameters = curve.parameters().unwrap();
    assert!(parameters.len() <= MAX_CURVE_SAMPLES);
    for pair in parameters.windows(2) {
        let a = curve.sample(pair[0]).position;
        let b = curve.sample(pair[1]).position;
        assert!((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt() <= 0.250001);
    }
    assert!(CurveSpec::new([[8, 2, 0], [8, 2, 341], [8, 2, 683], [8, 2, 1040]]).is_none());
    assert_eq!(RAIL_BUILD_REACH, 64.);
}
