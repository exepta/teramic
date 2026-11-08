use super::*;
use terra_core::{WoodSpecies, rails::FixedPoint};
use terra_voxel::blocks::BlockHit;

fn look(facing: u8) -> [f32; 2] {
    [
        f32::from(facing) * std::f32::consts::FRAC_PI_2 - std::f32::consts::PI,
        -0.5,
    ]
}
fn rotate([x, y, z]: Cell, facing: u8) -> Cell {
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn offset(origin: Cell, local: Cell, facing: u8) -> Cell {
    let local = rotate(local, facing);
    std::array::from_fn(|axis| origin[axis] + local[axis])
}

#[test]
fn starter_placement_has_three_tiles_one_item_and_exact_receipt_geometry_for_all_woods_and_headings()
 {
    for facing in 0..4 {
        for wood in [
            WoodSpecies::Oak,
            WoodSpecies::Birch,
            WoodSpecies::Spruce,
            WoodSpecies::Cherry,
        ] {
            let piece = Piece::seed([-17, 16, -17], facing).unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            prepare(&mut chunks, piece);
            let hit = BlockHit {
                position: [-17, 15, -17],
                normal: [0, 1, 0],
                distance: 2.0,
            };
            let ray = ([-16.5, 18.0, -16.5], [0.0, -1.0, 0.0]);
            let plan = seed(&chunks, &journal, hit, ray, look(facing), wood).unwrap();
            assert_eq!(plan.edits.len(), 3);
            assert_eq!(cost(&plan), Some(1));
            assert_eq!(
                next_endpoint(&plan),
                Some(Endpoint {
                    anchor: piece.anchor,
                    port: 1
                })
            );
            let mut other_wood = plan.clone();
            set_wood(
                &mut other_wood,
                if wood == WoodSpecies::Cherry {
                    WoodSpecies::Oak
                } else {
                    WoodSpecies::Cherry
                },
            );
            assert_ne!(seed_signature(&plan), seed_signature(&other_wood));
            assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
            assert!(complete(&journal, piece));
            for cell in cells(piece) {
                assert_eq!(
                    journal.object_state(cell).unwrap(),
                    object_state_with_wood(piece, cell, wood)
                );
            }
            let destination = piece.ports()[1].position;
            assert_eq!(
                continuation_endpoint(&journal, piece, destination),
                next_endpoint(&plan)
            );
            assert_eq!(
                continuation_endpoint(
                    &journal,
                    Piece::new(piece.anchor, rails::STRAIGHT, facing).unwrap(),
                    destination
                ),
                None
            );
            assert_eq!(
                continuation_endpoint(&journal, piece, FixedPoint([0; 3])),
                None
            );
            let mut replica = EditJournal::default();
            let owners: std::collections::BTreeSet<_> = cells(piece)
                .into_iter()
                .map(|cell| global_to_section(cell).0)
                .collect();
            assert!(owners.len() > 1);
            let total = owners.len();
            for (index, owner) in owners.into_iter().enumerate() {
                replica.import_delta(journal.section_delta(owner)).unwrap();
                assert_eq!(
                    continuation_endpoint(&replica, piece, destination),
                    if index + 1 == total {
                        next_endpoint(&plan)
                    } else {
                        None
                    }
                );
            }
            let existing_hit = BlockHit {
                position: piece.anchor,
                normal: [0, 1, 0],
                distance: 1.0,
            };
            assert!(seed(&chunks, &journal, existing_hit, ray, look(facing), wood).is_err());
            assert_eq!(journal.rail_module_count(), 1);
        }
    }
}

#[test]
fn a_one_metre_starter_stretches_to_real_straight_curved_or_higher_targets_without_rewriting_it() {
    for facing in 0..4 {
        for target in [[0, -1, 7], [4, -1, 25], [0, 0, 5]] {
            let piece = Piece::seed([-17, 16, -17], facing).unwrap();
            let mut chunks = ChunkStore::default();
            let mut journal = EditJournal::default();
            prepare(&mut chunks, piece);
            let initial = seed(
                &chunks,
                &journal,
                BlockHit {
                    position: [-17, 15, -17],
                    normal: [0, 1, 0],
                    distance: 2.0,
                },
                ([-16.5, 18.0, -16.5], [0.0, -1.0, 0.0]),
                look(facing),
                WoodSpecies::Spruce,
            )
            .unwrap();
            assert!(commit(&mut chunks, &mut journal, &initial).unwrap());
            let start = next_endpoint(&initial).unwrap();
            let floor = offset(piece.anchor, target, facing);
            block(&mut chunks, floor, 3);
            let hit = BlockHit {
                position: floor,
                normal: [0, 1, 0],
                distance: 2.0,
            };
            let proposal = proposal_connection(
                &chunks,
                &journal,
                start,
                hit,
                look(facing),
                WoodSpecies::Spruce,
            )
            .unwrap();
            let next = placement::proposed_piece(&proposal).unwrap();
            assert!(piece.ports()[1].connects(next.ports()[0]));
            let old_cells: std::collections::BTreeSet<_> = cells(piece).into_iter().collect();
            assert!(cells(next).iter().all(|cell| !old_cells.contains(cell)));
            prepare(&mut chunks, next);
            let plan = connection(
                &chunks,
                &journal,
                start,
                hit,
                look(facing),
                WoodSpecies::Spruce,
            )
            .unwrap();
            assert!(
                plan.edits
                    .iter()
                    .all(|edit| !rails::is_rail(edit.expected_material))
            );
            assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
            assert!(complete(&journal, piece));
            assert!(complete(&journal, next));
            assert_eq!(journal.rail_module_count(), 2);
            assert_eq!(
                continuation_endpoint(&journal, piece, piece.ports()[1].position),
                None,
                "a consumed old endpoint cannot be selected again"
            );
            let destination = next.ports()[1].position;
            assert_eq!(
                continuation_endpoint(&journal, next, destination),
                next_endpoint(&plan)
            );
            assert_eq!(
                network(&journal)
                    .unwrap()
                    .route(
                        Endpoint {
                            anchor: piece.anchor,
                            port: 0
                        },
                        next_endpoint(&plan).unwrap(),
                        16
                    )
                    .unwrap()
                    .legs
                    .len(),
                2
            );
            for cell in cells(piece) {
                assert_eq!(
                    journal.object_state(cell),
                    Some(object_state_with_wood(piece, cell, WoodSpecies::Spruce))
                );
            }
            if target[1] == 0 {
                assert_eq!(chunks.block_at(floor), Some(3));
                assert_eq!(destination.0[1], piece.ports()[1].position.0[1] + 16);
            }
        }
    }
}

#[test]
fn invalid_starter_space_keeps_a_real_three_tile_red_candidate_and_cannot_publish() {
    let piece = Piece::seed([0, 16, 0], 0).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    prepare(&mut chunks, piece);
    block(&mut chunks, [1, 16, 0], 3);
    let hit = BlockHit {
        position: [0, 15, 0],
        normal: [0, 1, 0],
        distance: 2.0,
    };
    let ray = ([0.5, 18.0, 0.5], [0.0, -1.0, 0.0]);
    let candidate = proposal_seed(&chunks, &journal, hit, ray, look(0), WoodSpecies::Oak).unwrap();
    assert_eq!(candidate.edits.len(), 3);
    assert!(seed(&chunks, &journal, hit, ray, look(0), WoodSpecies::Oak).is_err());
    assert!(commit(&mut chunks, &mut journal, &candidate).is_err());
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(chunks.block_at([1, 16, 0]), Some(3));
}
