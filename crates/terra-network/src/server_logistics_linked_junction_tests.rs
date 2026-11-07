use super::{tests::place, *};
use std::collections::{BTreeMap, HashSet};
use terra_core::{ItemKind, ItemStack, kinetic};
use terra_voxel::{BlockSection, SectionPos};
use terra_world::{conveyors, edits::BlockObjectState, streaming::LoadedSection};

fn at(origin: Cell, direction: Cell, distance: i64) -> Cell {
    std::array::from_fn(|i| origin[i] + direction[i] * distance)
}

fn point(center: [f64; 3], direction: Cell, distance: f64) -> [f64; 3] {
    std::array::from_fn(|i| center[i] + direction[i] as f64 * distance)
}

fn projection(point: [f64; 3], center: [f64; 3], direction: Cell) -> f64 {
    (0..3)
        .map(|i| (point[i] - center[i]) * direction[i] as f64)
        .sum()
}

fn span(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    start: Cell,
    end: Cell,
    facing: u8,
) -> conveyors::Span {
    for p in [start, end] {
        place(
            chunks,
            journal,
            p,
            144,
            BlockObjectState {
                facing,
                ..Default::default()
            },
        );
    }
    let layout = conveyors::layout(chunks, journal, start, end, ItemKind::Block(148)).unwrap();
    let span = conveyors::proposed_span(&layout).unwrap();
    assert!(conveyors::commit(chunks, journal, &layout).unwrap());
    span
}

struct Fixture {
    chunks: ChunkStore,
    journal: EditJournal,
    center: [f64; 3],
    port: Cell,
    forward: Cell,
    left: Cell,
    spans: Vec<conveyors::Span>,
    cranks: HashSet<Cell>,
}

fn fixture(facing: u8, material: u16) -> Fixture {
    let mut chunks = ChunkStore::default();
    for x in [-1, 0] {
        for z in [-1, 0] {
            let position = SectionPos { x, y: 0, z };
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    let mut journal = EditJournal::default();
    let deck = [0, 4, 0];
    let port = [0, 5, 0];
    let forward = kinetic::forward(facing);
    // Independent of distributor_output: this is the player's left/right contract.
    let left = [forward[2], 0, -forward[0]];
    let main = span(
        &mut chunks,
        &mut journal,
        at(deck, forward, -6),
        at(deck, forward, 6),
        facing,
    );
    let mut spans = vec![main];
    for (side, axis) in [
        (left, (facing + 3) % 4),
        (left.map(|v| -v), (facing + 1) % 4),
    ] {
        let flow_axis = if material == logistics::BELT_MERGER {
            axis
        } else {
            (axis + 2) % 4
        };
        spans.push(span(
            &mut chunks,
            &mut journal,
            at(deck, side, 1),
            at(deck, side, 6),
            flow_axis,
        ));
    }
    place(
        &mut chunks,
        &mut journal,
        port,
        material,
        BlockObjectState {
            facing,
            ..Default::default()
        },
    );
    let mut cranks = HashSet::new();
    for belt in &spans {
        let anchor = belt.end;
        let crank = kinetic::offset(anchor, kinetic::axis(belt.facing));
        place(
            &mut chunks,
            &mut journal,
            crank,
            100,
            BlockObjectState {
                facing: belt.facing,
                peer: Some(anchor),
                ..Default::default()
            },
        );
        assert_eq!(journal.crank_anchor(crank), Some(anchor));
        cranks.insert(crank);
    }
    let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
    crate::server::kinetics::advance(
        &mut Default::default(),
        &terrain,
        &chunks,
        &mut journal,
        &cranks,
    );
    for belt in &spans {
        for p in belt.cells().chain([belt.start, belt.end]) {
            assert!(
                journal.kinetic_state(p).running(),
                "unpowered actual crank/belt at {p:?}"
            );
            assert_eq!(journal.kinetic_state(p).rpm, 30.0);
        }
    }
    let center = conveyors::segment_at(&journal, deck)
        .unwrap()
        .drop_center()
        .unwrap();
    Fixture {
        chunks,
        journal,
        center,
        port,
        forward,
        left,
        spans,
        cranks,
    }
}

fn repower(fixture: &mut Fixture, skip: Option<usize>) {
    let mut cranks = fixture.cranks.clone();
    if let Some(index) = skip {
        let belt = fixture.spans[index];
        cranks.remove(&kinetic::offset(belt.end, kinetic::axis(belt.facing)));
    }
    let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
    crate::server::kinetics::advance(
        &mut Default::default(),
        &terrain,
        &fixture.chunks,
        &mut fixture.journal,
        &cranks,
    );
}

fn drop_at(fixture: &mut Fixture, item: ItemKind, direction: Cell, distance: f64) {
    assert!(fixture.journal.gameplay_mut().drop_stack(
        point(fixture.center, direction, distance),
        ItemStack { item, count: 1 },
        [0.0; 3],
        0.0,
    ));
    let drop = fixture.journal.gameplay_mut().drops.last_mut().unwrap();
    drop.rotation = [0.0, 0.0, 0.0, 1.0];
    drop.angular_velocity = [0.0; 3];
    drop.grounded = true;
}

fn tick(fixture: &mut Fixture, runtime: &mut conveyors::Transport) {
    let before: BTreeMap<_, _> = fixture
        .journal
        .gameplay()
        .drops
        .iter()
        .map(|drop| (drop.id, drop.position))
        .collect();
    conveyors::transport(
        runtime,
        &fixture.chunks,
        &mut fixture.journal,
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        0.02,
    );
    for drop in &fixture.journal.gameplay().drops {
        let old = before[&drop.id];
        let distance = (0..3)
            .map(|i| (old[i] - drop.position[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(
            distance <= 0.020001,
            "junction teleported {} by {distance}: {old:?}->{:?}",
            drop.id,
            drop.position
        );
        assert!((drop.position[1] - fixture.center[1]).abs() < 1e-8);
        assert_eq!(drop.stack.count, 1);
        for direction in [fixture.forward, fixture.left, fixture.left.map(|v| -v)] {
            let along = projection(drop.position, fixture.center, direction);
            let across = [direction[2], 0, -direction[0]];
            if !(0.51..0.999999).contains(&along)
                || projection(drop.position, fixture.center, across).abs() > 1e-8
            {
                continue;
            }
            assert_eq!(
                conveyors::routing_carrying(drop, &fixture.journal),
                Some(fixture.port)
            );
            let mut origin = drop.position;
            origin[1] += 0.5;
            assert_eq!(
                conveyors::carried_drop_hit(
                    &fixture.journal.gameplay().drops,
                    &fixture.journal,
                    origin,
                    [0.0, -1.0, 0.0],
                    1.0
                )
                .map(|hit| hit.0),
                Some(drop.id)
            );
            let mut held = drop.clone();
            crate::server::gameplay::step_drop_with_context(
                &mut held,
                &fixture.chunks,
                &fixture.journal,
                &Default::default(),
                0.02,
            );
            assert_eq!(
                held.position, drop.position,
                "gravity must not lose the handoff bridge"
            );
        }
    }
    assert!(fixture.journal.gameplay().valid());
}

#[test]
fn linked_inline_and_side_queues_zipper_fairly_and_densely_at_all_yaws() {
    for facing in 0..4 {
        let mut fixture = fixture(facing, logistics::BELT_MERGER);
        let red = ItemKind::Block(22);
        let blue = ItemKind::Block(25);
        for index in 0..6 {
            let rear = fixture.forward.map(|v| -v);
            let left = fixture.left;
            drop_at(&mut fixture, red, rear, 0.55 + index as f64 * 0.27);
            drop_at(&mut fixture, blue, left, 1.0 + index as f64 * 0.27);
        }
        let initial_ids: HashSet<_> = fixture
            .journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| drop.id)
            .collect();
        let mut runtime = conveyors::Transport::default();
        let mut emitted = Vec::new();
        let mut times = Vec::new();
        for step in 0..800 {
            tick(&mut fixture, &mut runtime);
            let exits: Vec<_> = fixture
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| projection(drop.position, fixture.center, fixture.forward) >= 1.2)
                .map(|drop| (drop.id, drop.stack.item))
                .collect();
            for (id, item) in exits {
                assert!(initial_ids.contains(&id));
                emitted.push(item);
                times.push(step as f64 * 0.02);
                fixture
                    .journal
                    .gameplay_mut()
                    .drops
                    .retain(|drop| drop.id != id);
            }
            assert_eq!(emitted.len() + fixture.journal.gameplay().drops.len(), 12);
            if emitted.len() == 12 {
                break;
            }
        }
        assert_eq!(
            emitted.len(),
            12,
            "yaw {facing}: {emitted:?}, waiting {:?}",
            fixture.journal.gameplay().drops
        );
        assert_eq!(
            emitted.first(),
            Some(&blue),
            "default first input is player's left, yaw {facing}"
        );
        assert!(
            emitted.windows(2).all(|pair| pair[0] != pair[1]),
            "yaw {facing}: {emitted:?}"
        );
        let gaps: Vec<_> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(
            gaps.iter().all(|gap| *gap <= 0.5),
            "1m/s output gaps, yaw {facing}: {gaps:?}"
        );
        assert!(fixture.journal.gameplay().drops.is_empty());
    }
}

#[test]
fn linked_distributor_three_allowed_mushroom_filters_reach_the_real_correct_outputs() {
    for facing in 0..4 {
        let mut fixture = fixture(facing, logistics::BELT_DISTRIBUTOR);
        let items = [
            ItemKind::Block(22),
            ItemKind::Block(25),
            ItemKind::Block(23),
        ];
        fixture
            .journal
            .set_distributor(
                fixture.port,
                logistics::DistributorConfig {
                    filters: items.map(|item| logistics::FilterConfig {
                        item: Some(item),
                        allowed: true,
                    }),
                },
            )
            .unwrap();
        for index in 0..9 {
            let rear = fixture.forward.map(|v| -v);
            drop_at(
                &mut fixture,
                items[index % 3],
                rear,
                0.55 + index as f64 * 0.27,
            );
        }
        let outputs = [fixture.left, fixture.forward, fixture.left.map(|v| -v)];
        let mut runtime = conveyors::Transport::default();
        let mut emitted = [0; 3];
        let mut crossed_real_endpoint_gap = false;
        for _ in 0..800 {
            tick(&mut fixture, &mut runtime);
            crossed_real_endpoint_gap |= fixture.journal.gameplay().drops.iter().any(|drop| {
                conveyors::carrying_segment(drop, &fixture.journal).is_none()
                    && conveyors::routing_carrying(drop, &fixture.journal) == Some(fixture.port)
                    && projection(drop.position, fixture.center, fixture.left).abs() > 0.51
            });
            let exits: Vec<_> = fixture
                .journal
                .gameplay()
                .drops
                .iter()
                .filter_map(|drop| {
                    outputs
                        .iter()
                        .position(|direction| {
                            projection(drop.position, fixture.center, *direction) >= 1.2
                        })
                        .map(|output| (drop.id, drop.stack.item, output))
                })
                .collect();
            for (id, item, output) in exits {
                assert_eq!(item, items[output], "yaw {facing}, output {output}");
                emitted[output] += 1;
                fixture
                    .journal
                    .gameplay_mut()
                    .drops
                    .retain(|drop| drop.id != id);
            }
            assert_eq!(
                emitted.iter().sum::<usize>() + fixture.journal.gameplay().drops.len(),
                9
            );
            if emitted == [3; 3] {
                break;
            }
        }
        assert_eq!(
            emitted,
            [3; 3],
            "yaw {facing}: {:?}",
            fixture.journal.gameplay().drops
        );
        assert!(
            crossed_real_endpoint_gap,
            "a legacy-only fixture would not exercise the actual handoff gap"
        );
        assert!(
            fixture
                .spans
                .iter()
                .all(|span| span.loaded(&fixture.chunks, &fixture.journal))
        );
    }
}

#[test]
fn linked_distributor_empty_outputs_round_robin_but_matching_allow_filter_is_reserved() {
    for facing in 0..4 {
        for filtered in [false, true] {
            let mut fixture = fixture(facing, logistics::BELT_DISTRIBUTOR);
            let items = [
                ItemKind::Block(22),
                ItemKind::Block(25),
                ItemKind::Block(23),
            ];
            if filtered {
                let mut config = logistics::DistributorConfig::default();
                config.filters[0] = logistics::FilterConfig {
                    item: Some(items[0]),
                    allowed: true,
                };
                fixture
                    .journal
                    .set_distributor(fixture.port, config)
                    .unwrap();
                // A matching but stopped output must not be bypassed through an
                // empty filter. Real crank removal leaves the other belts powered.
                repower(&mut fixture, Some(1));
                assert!(
                    !fixture
                        .journal
                        .kinetic_state(fixture.spans[1].controller)
                        .running()
                );
                assert!(
                    fixture
                        .journal
                        .kinetic_state(fixture.spans[0].controller)
                        .running()
                );
            }
            let mut expected = BTreeMap::new();
            for index in 0..9 {
                let rear = fixture.forward.map(|v| -v);
                drop_at(
                    &mut fixture,
                    items[index % 3],
                    rear,
                    0.55 + index as f64 * 0.27,
                );
                expected.insert(
                    fixture.journal.gameplay().drops.last().unwrap().id,
                    index % 3,
                );
            }
            let mut runtime = conveyors::Transport::default();
            if filtered {
                for _ in 0..60 {
                    tick(&mut fixture, &mut runtime);
                }
                assert_eq!(fixture.journal.gameplay().drops.len(), 9);
                assert!(fixture.journal.gameplay().drops.iter().all(|drop| {
                    projection(drop.position, fixture.center, fixture.forward) < 0.0
                }));
                repower(&mut fixture, None);
            }
            let outputs = [fixture.left, fixture.forward, fixture.left.map(|v| -v)];
            let mut emitted = [0; 3];
            for _ in 0..800 {
                tick(&mut fixture, &mut runtime);
                let exits: Vec<_> = fixture
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter_map(|drop| {
                        outputs
                            .iter()
                            .position(|direction| {
                                projection(drop.position, fixture.center, *direction) >= 1.2
                            })
                            .map(|output| (drop.id, output))
                    })
                    .collect();
                for (id, output) in exits {
                    assert_eq!(
                        output, expected[&id],
                        "yaw {facing}, filtered {filtered}, id {id}"
                    );
                    emitted[output] += 1;
                    fixture
                        .journal
                        .gameplay_mut()
                        .drops
                        .retain(|drop| drop.id != id);
                }
                assert_eq!(
                    emitted.iter().sum::<usize>() + fixture.journal.gameplay().drops.len(),
                    9
                );
                if emitted == [3; 3] {
                    break;
                }
            }
            assert_eq!(
                emitted,
                [3; 3],
                "yaw {facing}, filtered {filtered}: {:?}",
                fixture.journal.gameplay().drops
            );
        }
    }
}

#[test]
fn linked_merger_restores_both_halves_of_a_diagonal_turn_and_endpoint_handoff() {
    for facing in 0..4 {
        for along in [0.15, 0.35, 0.75] {
            let mut fixture = fixture(facing, logistics::BELT_MERGER);
            let left = fixture.left;
            drop_at(&mut fixture, ItemKind::Block(25), left, 1.0);
            let id = fixture.journal.gameplay().drops[0].id;
            let mut runtime = conveyors::Transport::default();
            let mut reached = false;
            for _ in 0..200 {
                tick(&mut fixture, &mut runtime);
                let drop = &fixture.journal.gameplay().drops[0];
                if projection(drop.position, fixture.center, fixture.forward) >= along {
                    assert_eq!(
                        conveyors::routing_carrying(drop, &fixture.journal),
                        Some(fixture.port)
                    );
                    reached = true;
                    break;
                }
            }
            assert!(reached, "yaw {facing}, checkpoint {along}");
            let saved = fixture.journal.gameplay().drops[0].position;
            runtime = conveyors::Transport::default();
            // Unloaded output retains custody and zero motion; restoring the
            // same section resumes the remaining path, never the old entry.
            let section = terra_voxel::global_to_section(fixture.spans[0].end).0;
            let unloaded = fixture.chunks.loaded.remove(&section).unwrap();
            for _ in 0..3 {
                tick(&mut fixture, &mut runtime);
            }
            assert_eq!(fixture.journal.gameplay().drops[0].position, saved);
            assert_eq!(fixture.journal.gameplay().drops[0].velocity, [0.0; 3]);
            fixture.chunks.loaded.insert(section, unloaded);
            let mut previous = saved;
            for _ in 0..150 {
                tick(&mut fixture, &mut runtime);
                let drop = &fixture.journal.gameplay().drops[0];
                assert_eq!(drop.id, id);
                assert!(projection(drop.position, previous, fixture.forward) >= -1e-9);
                assert!(
                    projection(drop.position, fixture.center, fixture.left)
                        <= projection(previous, fixture.center, fixture.left) + 1e-9
                );
                previous = drop.position;
            }
            assert!(
                projection(previous, fixture.center, fixture.forward) > 1.2,
                "yaw {facing}, {along}: {previous:?}"
            );
        }
    }
}

#[test]
fn linked_merger_restores_multiple_outgoing_heads_front_to_back_without_priority_deadlock() {
    for facing in 0..4 {
        let mut fixture = fixture(facing, logistics::BELT_MERGER);
        for distance in [0.30, 0.60] {
            let forward = fixture.forward;
            drop_at(&mut fixture, ItemKind::Block(25), forward, distance);
        }
        let rear = fixture.journal.gameplay().drops[0].id;
        let front = fixture.journal.gameplay().drops[1].id;
        let mut runtime = conveyors::Transport::default();
        let mut emitted = Vec::new();
        for _ in 0..200 {
            tick(&mut fixture, &mut runtime);
            let exits: Vec<_> = fixture
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| projection(drop.position, fixture.center, fixture.forward) >= 1.2)
                .map(|drop| drop.id)
                .collect();
            for id in exits {
                emitted.push(id);
                fixture
                    .journal
                    .gameplay_mut()
                    .drops
                    .retain(|drop| drop.id != id);
            }
            assert_eq!(emitted.len() + fixture.journal.gameplay().drops.len(), 2);
            if emitted.len() == 2 {
                break;
            }
        }
        assert_eq!(
            emitted,
            [front, rear],
            "yaw {facing}: {:?}",
            fixture.journal.gameplay().drops
        );
    }
}

#[test]
fn a_restored_paused_distributor_arm_does_not_starve_other_outputs_or_change_an_admitted_turn() {
    for facing in 0..4 {
        for checkpoint in [0.20, 0.90] {
            let mut fixture = fixture(facing, logistics::BELT_DISTRIBUTOR);
            let mut config = logistics::DistributorConfig::default();
            config.filters[0] = logistics::FilterConfig {
                item: Some(ItemKind::Block(22)),
                allowed: true,
            };
            fixture
                .journal
                .set_distributor(fixture.port, config)
                .unwrap();
            let rear = fixture.forward.map(|v| -v);
            drop_at(&mut fixture, ItemKind::Block(22), rear, 0.55);
            let id = fixture.journal.gameplay().drops[0].id;
            let mut runtime = conveyors::Transport::default();
            for _ in 0..200 {
                tick(&mut fixture, &mut runtime);
                if projection(
                    fixture.journal.gameplay().drops[0].position,
                    fixture.center,
                    fixture.left,
                ) >= checkpoint
                {
                    break;
                }
            }
            let saved = fixture.journal.gameplay().drops[0].position;
            assert!(projection(saved, fixture.center, fixture.left) >= checkpoint);
            repower(&mut fixture, Some(1));
            runtime = conveyors::Transport::default();
            // Changing the filter cannot redirect a partly completed turn.
            fixture
                .journal
                .set_distributor(fixture.port, Default::default())
                .unwrap();
            let rear = fixture.forward.map(|v| -v);
            if checkpoint > 0.5 {
                drop_at(&mut fixture, ItemKind::Block(25), rear, 0.55);
            }
            let mut blue_left = false;
            for _ in 0..160 {
                tick(&mut fixture, &mut runtime);
                assert_eq!(
                    fixture
                        .journal
                        .gameplay()
                        .drops
                        .iter()
                        .find(|drop| drop.id == id)
                        .unwrap()
                        .position,
                    saved
                );
                let blue = fixture
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .find(|drop| drop.id != id);
                if blue.is_some_and(|drop| {
                    projection(drop.position, fixture.center, fixture.forward) > 1.2
                }) {
                    blue_left = true;
                    fixture
                        .journal
                        .gameplay_mut()
                        .drops
                        .retain(|drop| drop.id == id);
                }
            }
            assert_eq!(
                blue_left,
                checkpoint > 0.5,
                "paused outbound arm, yaw {facing}"
            );
            repower(&mut fixture, None);
            for _ in 0..120 {
                tick(&mut fixture, &mut runtime);
            }
            assert_eq!(fixture.journal.gameplay().drops.len(), 1);
            let drop = &fixture.journal.gameplay().drops[0];
            assert_eq!(drop.id, id);
            assert!(
                projection(drop.position, fixture.center, fixture.left) > 1.2,
                "paused turn redirected at yaw {facing}: {drop:?}"
            );
        }
    }
}
