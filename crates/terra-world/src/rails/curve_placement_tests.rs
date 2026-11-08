use super::*;
use std::collections::BTreeSet;
use terra_core::WoodSpecies;
use terra_voxel::blocks::BlockHit;

fn proposed_curve(layout: &crate::placement::Layout) -> Piece {
    layout
        .states
        .iter()
        .find_map(|(cell, state)| {
            let state = (*state)?;
            (!state.secondary).then(|| piece_from_state(*cell, rails::STRAIGHT, state).unwrap())
        })
        .unwrap()
}

fn rotate([x, y, z]: Cell, facing: u8) -> Cell {
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}

fn ahead_floor(port: rails::Port) -> BlockHit {
    let position = port.position.position();
    let forward = port.tangent.map(f64::from);
    let length = forward
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    let forward = forward.map(|value| value / length);
    let right = [forward[2], 0.0, -forward[0]];
    BlockHit {
        position: std::array::from_fn(|axis| {
            if axis == 1 {
                position[axis].floor() as i64 - 1
            } else {
                (position[axis] + forward[axis] * 24.0 + right[axis] * 4.0).floor() as i64
            }
        }),
        normal: [0, 1, 0],
        distance: 2.0,
    }
}

#[test]
fn floor_connections_follow_the_route_in_every_yaw_independent_of_camera_heading() {
    for facing in 0..4 {
        let source = Piece::new([-33, 16, -49], rails::STRAIGHT, facing).unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, source);
        install(&mut chunks, &mut journal, source);
        let start = Endpoint {
            anchor: source.anchor,
            port: 1,
        };
        let hit = ahead_floor(source.ports()[1]);
        let first = proposal_connection(
            &chunks,
            &journal,
            start,
            hit,
            [0.0, -0.5],
            WoodSpecies::Birch,
        )
        .unwrap();
        let curve = proposed_curve(&first);
        assert!(source.ports()[1].connects(curve.ports()[0]));
        let source_cells: BTreeSet<_> = cells(source).into_iter().collect();
        assert!(cells(curve).iter().all(|cell| !source_cells.contains(cell)));
        for yaw in [
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            -std::f32::consts::FRAC_PI_2,
        ] {
            let proposal = proposal_connection(
                &chunks,
                &journal,
                start,
                hit,
                [yaw, -0.5],
                WoodSpecies::Birch,
            )
            .unwrap();
            assert_eq!(proposed_curve(&proposal), curve);
        }
        prepare(&mut chunks, curve);
        let layout = connection(
            &chunks,
            &journal,
            start,
            hit,
            [std::f32::consts::PI, -0.5],
            WoodSpecies::Birch,
        )
        .unwrap();
        assert_eq!(proposed_curve(&layout), curve);
        assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
        assert!(complete(&journal, source));
        assert!(complete(&journal, curve));
    }
}

#[test]
fn perpendicular_free_ends_join_exactly_and_occupied_turns_keep_a_red_candidate() {
    for facing in 0..4 {
        let source = Piece::new([-33, 16, -49], rails::STRAIGHT, facing).unwrap();
        let start_port = source.ports()[1];
        let right = rotate([1, 0, 0], facing);
        let forward = rotate([0, 0, 1], facing);
        let target_facing = (facing + 1) % 4;
        let local_port = Piece::new([0; 3], rails::STRAIGHT, target_facing)
            .unwrap()
            .ports()[0];
        let end: Cell = std::array::from_fn(|axis| {
            start_port.position.0[axis] + (right[axis] + forward[axis]) * 296
        });
        let translation: Cell = std::array::from_fn(|axis| end[axis] - local_port.position.0[axis]);
        assert!(
            translation
                .iter()
                .all(|coordinate| coordinate.rem_euclid(16) == 0)
        );
        let target = Piece::new(
            translation.map(|coordinate| coordinate / 16),
            rails::STRAIGHT,
            target_facing,
        )
        .unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, source);
        prepare(&mut chunks, target);
        install(&mut chunks, &mut journal, source);
        install(&mut chunks, &mut journal, target);
        let start = Endpoint {
            anchor: source.anchor,
            port: 1,
        };
        let hit = BlockHit {
            position: target.anchor,
            normal: [0, 1, 0],
            distance: 2.0,
        };
        let proposal =
            proposal_connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak)
                .unwrap();
        let curve = proposed_curve(&proposal);
        assert!(start_port.connects(curve.ports()[0]));
        assert!(target.ports()[0].connects(curve.ports()[1]));
        let existing: BTreeSet<_> = cells(source).into_iter().chain(cells(target)).collect();
        assert!(
            cells(curve).iter().all(|cell| !existing.contains(cell)),
            "facing {facing}"
        );
        prepare(&mut chunks, curve);
        let obstruction = cells(curve)[cells(curve).len() / 2];
        block(&mut chunks, obstruction, 3);
        assert!(connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak).is_err());
        let red = proposal_connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak)
            .unwrap();
        assert_eq!(proposed_curve(&red), curve);
        assert!(commit(&mut chunks, &mut journal, &red).is_err());
        assert_eq!(chunks.block_at(obstruction), Some(3));
        assert_eq!(journal.rail_module_count(), 2);
        block(&mut chunks, obstruction, 0);
        let layout =
            connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak).unwrap();
        assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
        let route = network(&journal)
            .unwrap()
            .route(
                Endpoint {
                    anchor: source.anchor,
                    port: 0,
                },
                Endpoint {
                    anchor: target.anchor,
                    port: 1,
                },
                16,
            )
            .unwrap();
        assert_eq!(route.legs.len(), 3);
    }
}

#[test]
fn tracks_touching_a_side_cannot_be_converted_into_an_overlapping_hidden_branch() {
    for facing in 0..4 {
        let source = Piece::new([-33, 16, -49], rails::STRAIGHT, facing).unwrap();
        let offset = rotate([2, 0, 1], facing);
        let target = Piece::new(
            std::array::from_fn(|axis| source.anchor[axis] + offset[axis]),
            rails::STRAIGHT,
            (facing + 1) % 4,
        )
        .unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        prepare(&mut chunks, source);
        prepare(&mut chunks, target);
        install(&mut chunks, &mut journal, source);
        install(&mut chunks, &mut journal, target);
        let start = Endpoint {
            anchor: source.anchor,
            port: 1,
        };
        let hit = BlockHit {
            position: target.anchor,
            normal: [0, 1, 0],
            distance: 2.0,
        };
        let before = journal.edited_blocks();
        let error =
            connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak).unwrap_err();
        assert!(
            error.contains("switch") && error.contains("free end"),
            "{error}"
        );
        assert!(
            proposal_connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak)
                .is_err()
        );
        assert_eq!(journal.edited_blocks(), before);
        assert!(complete(&journal, source));
        assert!(complete(&journal, target));
    }
}

#[test]
fn both_switch_branches_accept_curves_and_route_only_when_selected() {
    for material in [rails::SWITCH_LEFT, rails::SWITCH_RIGHT] {
        for facing in 0..4 {
            let source = Piece::new([-33, 16, -49], material, facing).unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            prepare(&mut chunks, source);
            install(&mut chunks, &mut journal, source);
            let start = Endpoint {
                anchor: source.anchor,
                port: 2,
            };
            let hit = ahead_floor(source.ports()[2]);
            let proposal =
                proposal_connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak)
                    .unwrap();
            let curve = proposed_curve(&proposal);
            let existing: BTreeSet<_> = cells(source).into_iter().collect();
            assert!(
                cells(curve).iter().all(|cell| !existing.contains(cell)),
                "{material} facing {facing}"
            );
            prepare(&mut chunks, curve);
            let layout =
                connection(&chunks, &journal, start, hit, [0.0, -0.5], WoodSpecies::Oak).unwrap();
            assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
            let from = Endpoint {
                anchor: source.anchor,
                port: 0,
            };
            let to = Endpoint {
                anchor: curve.anchor,
                port: 1,
            };
            assert!(network(&journal).unwrap().route(from, to, 16).is_err());
            let toggle = toggle_switch_plan(&chunks, &journal, source.anchor).unwrap();
            assert!(commit(&mut chunks, &mut journal, &toggle).unwrap());
            assert_eq!(
                network(&journal)
                    .unwrap()
                    .route(from, to, 16)
                    .unwrap()
                    .legs
                    .len(),
                2
            );
        }
    }
}
