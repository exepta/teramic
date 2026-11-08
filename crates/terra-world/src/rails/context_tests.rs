use super::*;
use terra_core::{WoodSpecies, rails::BuildChoice};

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
fn ray_to(
    chunks: &ChunkStore,
    journal: &EditJournal,
    eye: [f64; 3],
    point: [f64; 3],
) -> (
    terra_voxel::blocks::BlockHit,
    ([f64; 3], [f64; 3]),
    [f32; 2],
) {
    let delta: [f64; 3] = std::array::from_fn(|axis| point[axis] - eye[axis]);
    let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    assert!(length <= 4.5);
    let direction = delta.map(|value| value / length);
    let hit = crate::interaction_shapes::raycast_solid(
        chunks,
        journal,
        &Default::default(),
        eye,
        direction,
        4.5,
    )
    .expect("real fixture surface must be visible");
    (
        hit,
        (eye, direction),
        [
            (-direction[0]).atan2(-direction[2]) as f32,
            direction[1].asin() as f32,
        ],
    )
}
fn fixture_step(facing: u8) -> (ChunkStore, EditJournal, Piece, Piece, Piece) {
    let old = Piece::new([-33, 16, -49], rails::STRAIGHT, facing).unwrap();
    let behind = Piece::new(
        offset(old.anchor, [0, 0, -4], facing),
        rails::STRAIGHT,
        facing,
    )
    .unwrap();
    let upper = Piece::new(
        offset(old.anchor, [0, 1, 4], facing),
        rails::STRAIGHT,
        facing,
    )
    .unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in -2..=2 {
        for z in -6..=10 {
            block(&mut chunks, offset(old.anchor, [x, -1, z], facing), 3);
            if z >= 4 {
                block(&mut chunks, offset(old.anchor, [x, 0, z], facing), 3);
            }
        }
    }
    for piece in [behind, old, upper] {
        prepare(&mut chunks, piece);
        install(&mut chunks, &mut journal, piece);
    }
    (chunks, journal, behind, old, upper)
}

#[test]
fn zero_gap_step_replaces_only_the_last_straight_and_keeps_both_real_connections_in_all_yaws() {
    for facing in 0..4 {
        let (mut chunks, mut journal, behind, old, upper) = fixture_step(facing);
        let near = old.ports()[1];
        let forward = near.tangent.map(f64::from);
        let position = near.position.position();
        let eye = std::array::from_fn(|axis| {
            position[axis] - forward[axis] * 2.0 + if axis == 1 { 2.0 } else { 0.0 }
        });
        let point = [position[0], old.anchor[1] as f64 + 0.75, position[2]];
        let (hit, ray, look) = ray_to(&chunks, &journal, eye, point);
        assert_eq!(hit.normal, near.tangent.map(|value| -i32::from(value)));
        assert_eq!(chunks.block_at(hit.position), Some(3));
        let choice = context_choice(
            &chunks,
            &journal,
            hit,
            ray,
            look,
            WoodSpecies::Oak,
            BuildChoice::Automatic,
        );
        assert!(choice.valid, "facing {facing}: {:?}", choice.error);
        assert_eq!(choice.choice, BuildChoice::Straight);
        assert_eq!(choice.replaced, Some(old));
        let signature = context_signature(&choice);
        let repeated = context_choice(
            &chunks,
            &journal,
            hit,
            ray,
            look,
            WoodSpecies::Oak,
            BuildChoice::Straight,
        );
        assert_eq!(context_signature(&repeated), signature);
        let sites = context_sites(&journal, &choice).unwrap();
        assert!(
            cells(behind)
                .into_iter()
                .chain(cells(upper))
                .all(|cell| sites.contains(&cell))
        );
        let layout = choice.layout.as_ref().unwrap();
        let next = placement::proposed_piece(layout).unwrap();
        assert!(next.curve.is_some());
        assert!(behind.ports()[1].connects(next.ports()[0]));
        assert!(upper.ports()[0].connects(next.ports()[1]));
        assert!(next.curve.unwrap().validate().unwrap().max_grade <= rails::MAX_CURVE_GRADE);
        assert_eq!(piece_cost(next), 2);
        assert!(
            layout
                .edits
                .iter()
                .any(|edit| rails::is_rail(edit.expected_material))
        );
        assert!(layout.edits.iter().any(|edit| edit.expected_material == 0));
        assert!(commit(&mut chunks, &mut journal, layout).unwrap());
        assert!(complete(&journal, next));
        assert!(complete(&journal, behind));
        assert!(complete(&journal, upper));
        assert_eq!(journal.rail_module_count(), 3);
        let route = network(&journal)
            .unwrap()
            .route(
                Endpoint {
                    anchor: behind.anchor,
                    port: 0,
                },
                Endpoint {
                    anchor: upper.anchor,
                    port: 1,
                },
                16,
            )
            .unwrap();
        assert_eq!(route.legs.len(), 3);
        let mut replica = EditJournal::default();
        let sections: std::collections::BTreeSet<_> = layout
            .edits
            .iter()
            .map(|edit| global_to_section(edit.position).0)
            .chain(
                cells(behind)
                    .into_iter()
                    .chain(cells(upper))
                    .map(|cell| global_to_section(cell).0),
            )
            .collect();
        for section in sections {
            replica
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert_eq!(network(&replica).unwrap(), network(&journal).unwrap());
    }
}

#[test]
fn a_real_rail_end_keeps_straight_building_but_rejects_a_compact_perpendicular_join() {
    for facing in 0..4 {
        let source = Piece::new([-33, 16, -49], rails::STRAIGHT, facing).unwrap();
        let start = source.ports()[1];
        let forward = rotate([0, 0, 1], facing);
        let right = rotate([1, 0, 0], facing);
        let target_facing = (facing + 1) % 4;
        let local = Piece::new([0; 3], rails::STRAIGHT, target_facing)
            .unwrap()
            .ports()[0];
        let end: Cell =
            std::array::from_fn(|axis| start.position.0[axis] + (forward[axis] + right[axis]) * 72);
        let target = Piece::new(
            std::array::from_fn(|axis| (end[axis] - local.position.0[axis]) / 16),
            rails::STRAIGHT,
            target_facing,
        )
        .unwrap();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        for x in -3..=11 {
            for z in -2..=14 {
                block(&mut chunks, offset(source.anchor, [x, -1, z], facing), 3);
            }
        }
        for piece in [source, target] {
            prepare(&mut chunks, piece);
            install(&mut chunks, &mut journal, piece);
        }
        let from = start.position.position();
        let to = target.ports()[0].position.position();
        let eye = std::array::from_fn(|axis| {
            (from[axis] + to[axis]) * 0.5 + if axis == 1 { 1.5 } else { 0.0 }
        });
        let point = std::array::from_fn(|axis| {
            from[axis] + right[axis] as f64 - forward[axis] as f64 * 0.2
                + if axis == 1 { 0.18 } else { 0.0 }
        });
        let (hit, ray, look) = ray_to(&chunks, &journal, eye, point);
        assert_eq!(owner_at(&journal, hit.position), Some(source));
        let choices = context_choices(&chunks, &journal, hit, ray, look, WoodSpecies::Oak);
        let curved = choices
            .iter()
            .find(|choice| choice.choice == BuildChoice::Curve)
            .expect("real nearby perpendicular end supplies a curve option");
        let straight = choices
            .iter()
            .find(|choice| choice.choice == BuildChoice::Straight)
            .unwrap();
        assert!(!curved.valid);
        assert!(
            curved
                .error
                .as_ref()
                .is_some_and(|error| error.contains("16 m"))
        );
        assert!(straight.valid, "facing {facing}: {:?}", straight.error);
        assert_ne!(context_signature(curved), context_signature(straight));
        assert!(curved.layout.is_none());
        assert_eq!(journal.rail_module_count(), 2);
    }
}

#[test]
fn ramp_replacement_refuses_another_wood_and_stale_or_obstructed_union_without_mutation() {
    let (mut chunks, mut journal, _, old, upper) = fixture_step(0);
    let near = old.ports()[1].position.position();
    let (hit, ray, look) = ray_to(
        &chunks,
        &journal,
        [near[0], near[1] + 2.0, near[2] - 2.0],
        [near[0], old.anchor[1] as f64 + 0.75, near[2]],
    );
    let wrong = context_choice(
        &chunks,
        &journal,
        hit,
        ray,
        look,
        WoodSpecies::Birch,
        BuildChoice::Straight,
    );
    assert!(!wrong.valid);
    assert!(
        wrong
            .error
            .as_ref()
            .is_some_and(|error| error.contains("same wood"))
    );
    let choice = context_choice(
        &chunks,
        &journal,
        hit,
        ray,
        look,
        WoodSpecies::Oak,
        BuildChoice::Straight,
    );
    assert!(choice.valid, "{:?}", choice.error);
    let layout = choice.layout.unwrap();
    let before = journal.edited_blocks();
    let added = layout
        .edits
        .iter()
        .find(|edit| edit.expected_material == 0)
        .unwrap()
        .position;
    block(&mut chunks, added, 3);
    assert!(commit(&mut chunks, &mut journal, &layout).is_err());
    assert!(complete(&journal, old));
    assert!(complete(&journal, upper));
    assert_eq!(journal.edited_blocks(), before);
    block(&mut chunks, added, 0);
    let mut forged = layout.clone();
    forged
        .states
        .iter_mut()
        .find_map(|(_, state)| state.as_mut().and_then(|state| state.rail_curve.as_mut()))
        .unwrap()
        .points[1][0] += 1;
    assert!(commit(&mut chunks, &mut journal, &forged).is_err());
    assert!(complete(&journal, old));
    assert_eq!(journal.edited_blocks(), before);
}

#[test]
fn an_unconnected_floor_does_not_capture_context_and_a_middle_rail_hit_does_not_select_a_remote_end()
 {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    block(&mut chunks, [0, 15, 0], 3);
    let (hit, ray, look) = ray_to(&chunks, &journal, [0.5, 18.0, 0.5], [0.5, 16.0, 0.5]);
    assert!(context_choices(&chunks, &journal, hit, ray, look, WoodSpecies::Oak).is_empty());
    let source = Piece::new([0, 16, 0], rails::STRAIGHT, 0).unwrap();
    prepare(&mut chunks, source);
    install(&mut chunks, &mut journal, source);
    let (hit, ray, look) = ray_to(&chunks, &journal, [1.5, 19.0, 2.0], [1.5, 16.3, 2.0]);
    assert_eq!(owner_at(&journal, hit.position), Some(source));
    assert!(context_choices(&chunks, &journal, hit, ray, look, WoodSpecies::Oak).is_empty());
}

fn fixture_bare_step(
    origin: Cell,
    facing: u8,
    gap: i64,
) -> (ChunkStore, EditJournal, Piece, Piece) {
    let old = Piece::new(origin, rails::STRAIGHT, facing).unwrap();
    let behind = Piece::new(offset(origin, [0, 0, -4], facing), rails::STRAIGHT, facing).unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in -3..=3 {
        for z in -6..=14 {
            block(&mut chunks, offset(origin, [x, -1, z], facing), 3);
            if z >= 4 + gap {
                block(&mut chunks, offset(origin, [x, 0, z], facing), 3);
            }
        }
    }
    for piece in [behind, old] {
        prepare(&mut chunks, piece);
        install(&mut chunks, &mut journal, piece);
    }
    (chunks, journal, behind, old)
}

#[test]
fn aiming_the_lower_rail_wall_or_platform_builds_the_same_real_ramp_without_an_upper_track() {
    for facing in 0..4 {
        for gap in 0..=4 {
            let mut first_signature = None;
            for aim in ["rail end", "step wall", "platform top"] {
                let (mut chunks, mut journal, behind, old) =
                    fixture_bare_step([-33, 16, -49], facing, gap);
                let start = old.ports()[1];
                let position = start.position.position();
                let forward = start.tangent.map(f64::from);
                let right = [forward[2], 0.0, -forward[0]];
                let eye = std::array::from_fn(|axis| {
                    position[axis]
                        + forward[axis] * (gap as f64 * 0.5 - 0.25)
                        + right[axis] * 2.3
                        + if axis == 1 { 1.8 } else { 0.0 }
                });
                let point = match aim {
                    "rail end" => std::array::from_fn(|axis| {
                        position[axis] - forward[axis] * 0.2
                            + right[axis]
                            + if axis == 1 { 0.18 } else { 0.0 }
                    }),
                    "step wall" => std::array::from_fn(|axis| {
                        position[axis]
                            + forward[axis] * gap as f64
                            + if axis == 1 { 0.625 } else { 0.0 }
                    }),
                    _ => std::array::from_fn(|axis| {
                        position[axis]
                            + forward[axis] * (gap as f64 + 1.5)
                            + if axis == 1 { 0.875 } else { 0.0 }
                    }),
                };
                let (hit, ray, look) = ray_to(&chunks, &journal, eye, point);
                if aim == "rail end" {
                    assert_eq!(owner_at(&journal, hit.position), Some(old));
                } else if aim == "step wall" {
                    assert_eq!(hit.normal, start.tangent.map(|value| -i32::from(value)));
                } else {
                    assert_eq!(hit.normal, [0, 1, 0]);
                }
                let choice = context_choice(
                    &chunks,
                    &journal,
                    hit,
                    ray,
                    look,
                    WoodSpecies::Oak,
                    BuildChoice::Automatic,
                );
                assert!(
                    choice.valid,
                    "facing {facing}, gap {gap}, aim {aim}: {:?}",
                    choice.error
                );
                assert_eq!(choice.choice, BuildChoice::Straight);
                assert_eq!(choice.replaced, Some(old));
                let signature = context_signature(&choice);
                if let Some(first) = first_signature {
                    assert_eq!(
                        signature, first,
                        "all genuine targets describe the same first platform edge"
                    );
                } else {
                    first_signature = Some(signature);
                }
                let sites = context_sites(&journal, &choice).unwrap();
                assert!(sites.iter().all(|cell| chunks.block_at(*cell).is_some()));
                let layout = choice.layout.unwrap();
                let ramp = placement::proposed_piece(&layout).unwrap();
                let end = ramp.ports()[1];
                assert_eq!(
                    end.position.0,
                    std::array::from_fn(|axis| start.position.0[axis]
                        + if axis == 1 {
                            16
                        } else {
                            i64::from(start.tangent[axis]) * gap * 16
                        })
                );
                assert!(behind.ports()[1].connects(ramp.ports()[0]));
                let landing = offset(old.anchor, [0, 0, 4 + gap], facing);
                assert_eq!(chunks.block_at(landing), Some(3));
                assert!(commit(&mut chunks, &mut journal, &layout).unwrap());
                assert_eq!(
                    chunks.block_at(landing),
                    Some(3),
                    "the actual raised floor is preserved"
                );
                assert!(complete(&journal, behind));
                assert!(complete(&journal, ramp));
                assert_eq!(journal.rail_module_count(), 2);
                assert_eq!(
                    network(&journal)
                        .unwrap()
                        .route(
                            Endpoint {
                                anchor: behind.anchor,
                                port: 0
                            },
                            Endpoint {
                                anchor: ramp.anchor,
                                port: 1
                            },
                            16
                        )
                        .unwrap()
                        .legs
                        .len(),
                    2
                );
            }
        }
    }
}

#[test]
fn an_aimed_rail_cannot_turn_a_tall_wall_or_missing_gap_section_into_a_valid_ramp() {
    for obstructed in [false, true] {
        let (mut chunks, journal, _, old) = fixture_bare_step([15, 16, 12], 0, 2);
        if obstructed {
            for x in -1..=1 {
                block(&mut chunks, offset(old.anchor, [x, 1, 6], 0), 3);
            }
        } else {
            chunks
                .loaded
                .remove(&global_to_section(offset(old.anchor, [0, 0, 4], 0)).0);
        }
        let start = old.ports()[1].position.position();
        let eye = [start[0] + 2.3, start[1] + 1.8, start[2] - 0.25];
        let point = [start[0] + 1.0, start[1] + 0.18, start[2] - 0.2];
        let (hit, ray, look) = ray_to(&chunks, &journal, eye, point);
        assert_eq!(owner_at(&journal, hit.position), Some(old));
        let choices = context_choices(&chunks, &journal, hit, ray, look, WoodSpecies::Oak);
        assert!(!choices.is_empty());
        assert!(choices.iter().all(|choice| !choice.valid));
        assert!(
            choices.iter().all(|choice| choice.replaced.is_none()),
            "a landing must be real, loaded, and have headroom"
        );
    }
}
