use super::*;
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use terra_voxel::BlockSection;

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: Cell, material: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
}
fn rectangular_fixture(
    origin: Cell,
    size: [i64; 2],
) -> (TerraGenerator, ChunkStore, EditJournal, Vec<Cell>) {
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        sea_level: -100.,
        base_height: -50.,
        rivers: false,
        caves: false,
        biomes: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    for x in origin[0] - 1..=origin[0] + size[0] {
        for y in origin[1] - 1..=origin[1] + 1 {
            for z in origin[2] - 1..=origin[2] + size[1] {
                let p = [x, y, z];
                let (section, _) = global_to_section(p);
                chunks
                    .loaded
                    .entry(section)
                    .or_insert_with(|| LoadedSection {
                        data: BlockSection::from_blocks(section, 0, vec![3; 4096]).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: vec![],
                    });
                journal.record(p, 3).unwrap();
            }
        }
    }
    let mut positions = vec![];
    for z in 0..size[1] {
        for x in 0..size[0] {
            let p = [origin[0] + x, origin[1], origin[2] + z];
            positions.push(p);
            put(&mut chunks, &mut journal, p, 0);
            put(&mut chunks, &mut journal, [p[0], p[1] + 1, p[2]], 0);
        }
    }
    (terrain, chunks, journal, positions)
}
fn fixture(origin: Cell) -> (TerraGenerator, ChunkStore, EditJournal, [Cell; 4]) {
    let (terrain, chunks, journal, positions) = rectangular_fixture(origin, [2, 2]);
    (terrain, chunks, journal, positions.try_into().unwrap())
}
fn sources(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    positions: [Cell; 4],
    indices: &[usize],
) {
    for &index in indices {
        put(chunks, journal, positions[index], 27);
    }
}
fn run(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    steps: usize,
) -> FluidStep {
    let mut total = FluidStep::default();
    for _ in 0..steps {
        let step = simulate_fluid_step(terrain, chunks, journal, 256).unwrap();
        total.sourced += step.sourced;
        total.replenished += step.replenished;
        total.drained += step.drained;
        total.changed |= step.changed;
    }
    total
}
fn snapshot(journal: &EditJournal) -> BTreeMap<Cell, FluidCell> {
    journal.fluid_cells().collect()
}
fn assert_quiet(terrain: &TerraGenerator, chunks: &ChunkStore, journal: &mut EditJournal) {
    let before = snapshot(journal);
    let epochs: Vec<_> = journal.water_mesh_epochs().collect();
    let fluid_epochs: Vec<_> = journal
        .changed_sections()
        .map(|p| (p, journal.fluid_epoch(p)))
        .collect();
    for _ in 0..64 {
        let step = simulate_fluid_step(terrain, chunks, journal, 256).unwrap();
        assert!(!step.changed);
        assert_eq!(step.sourced + step.replenished + step.drained, 0);
    }
    assert_eq!(snapshot(journal), before);
    assert_eq!(
        journal.water_mesh_epochs().collect::<BTreeMap<_, _>>(),
        epochs.into_iter().collect()
    );
    for (p, epoch) in fluid_epochs {
        assert_eq!(journal.fluid_epoch(p), epoch);
    }
    assert_eq!(journal.fluid_work_counts().queued, 0);
}

#[test]
fn diagonal_sources_form_only_the_other_two_sources_across_positive_and_negative_chunk_edges() {
    for origin in [[3, 18, 3], [-1, 16, -1], [15, -16, 15]] {
        for diagonal in [[0, 3], [1, 2]] {
            let (terrain, mut chunks, mut journal, positions) = fixture(origin);
            sources(&mut chunks, &mut journal, positions, &diagonal);
            let mesh_epochs: Vec<_> = journal
                .changed_sections()
                .map(|p| (p, journal.mesh_epoch(p)))
                .collect();
            let step = run(&terrain, &chunks, &mut journal, 32);
            for p in positions {
                assert_eq!(
                    journal.fluid(p),
                    Some(FluidCell {
                        amount: 8,
                        flow: [0; 2],
                        source: true
                    })
                );
            }
            assert_eq!(
                journal
                    .fluid_cells()
                    .filter(|(_, cell)| cell.source)
                    .count(),
                4
            );
            assert_eq!(step.sourced, 16);
            assert_eq!(step.replenished + step.drained, 0);
            for (p, epoch) in mesh_epochs {
                assert_eq!(
                    journal.mesh_epoch(p),
                    epoch,
                    "resting water must not rebuild terrain"
                );
            }
            assert_quiet(&terrain, &chunks, &mut journal);
        }
    }
}

#[test]
fn single_or_adjacent_sources_fill_a_calm_pool_without_becoming_new_sources() {
    for initial in [vec![0], vec![0, 1]] {
        let (terrain, mut chunks, mut journal, positions) = fixture([3, 18, 3]);
        sources(&mut chunks, &mut journal, positions, &initial);
        let step = run(&terrain, &chunks, &mut journal, 32);
        for (index, p) in positions.into_iter().enumerate() {
            assert_eq!(
                journal.fluid(p),
                Some(FluidCell {
                    amount: 8,
                    flow: [0; 2],
                    source: initial.contains(&index)
                })
            );
        }
        assert_eq!(step.sourced, 32 - initial.len() as u64 * 8);
        assert_quiet(&terrain, &chunks, &mut journal);
        // A newly placed diagonal seed intentionally enables the 2-neighbour rule.
        sources(&mut chunks, &mut journal, positions, &[3]);
        run(&terrain, &chunks, &mut journal, 8);
        assert!(
            positions
                .into_iter()
                .all(|p| journal.fluid(p).unwrap().source)
        );
    }
}

#[test]
fn source_free_supported_pools_keep_every_finite_unit_and_clear_old_flow_without_remeshing() {
    for total in [1_u16, 7, 8, 11, 23, 31, 32] {
        let (terrain, chunks, mut journal, positions) = fixture([-1, 18, 15]);
        let mut left = total;
        for p in positions {
            let amount = left.min(8) as u8;
            left -= u16::from(amount);
            if amount > 0 {
                journal
                    .set_fluid(
                        p,
                        FluidCell {
                            amount,
                            source: false,
                            flow: [1, 0],
                        },
                    )
                    .unwrap();
            }
        }
        // Start from a dry member to prove quantity does not depend on whether
        // the wet cell or a potentially receiving empty cell is queued first.
        journal.take_fluid_work(4096);
        for p in positions.into_iter().rev() {
            journal.wake_fluid(p);
        }
        let step = run(&terrain, &chunks, &mut journal, 32);
        assert_eq!(
            positions
                .into_iter()
                .map(|p| u16::from(journal.fluid(p).unwrap_or_default().amount))
                .sum::<u16>(),
            total
        );
        assert_eq!(step.sourced + step.replenished + step.drained, 0);
        for (index, p) in positions.into_iter().enumerate() {
            assert_eq!(
                journal.fluid(p).unwrap_or_default(),
                FluidCell {
                    amount: (total / 4 + u16::from(index < (total % 4) as usize)) as u8,
                    flow: [0; 2],
                    source: false
                }
            );
        }
        assert_quiet(&terrain, &chunks, &mut journal);
    }
}

#[test]
fn unsupported_open_falling_or_waterlogged_arrangements_never_grow_new_sources() {
    for case in ["floor_hole", "open_wall", "falling", "lower_slab", "pipe"] {
        let (terrain, mut chunks, mut journal, positions) = fixture([3, 18, 3]);
        sources(&mut chunks, &mut journal, positions, &[0, 3]);
        match case {
            "floor_hole" => put(&mut chunks, &mut journal, [3, 17, 3], 0),
            "open_wall" => put(&mut chunks, &mut journal, [2, 18, 3], 0),
            "falling" => put(&mut chunks, &mut journal, [3, 19, 3], 27),
            "lower_slab" => {
                let p = [3, 17, 3];
                put(&mut chunks, &mut journal, p, 82);
                journal
                    .set_object_state(p, Some(BlockObjectState::default()))
                    .unwrap();
            }
            "pipe" => {
                let p = positions[1];
                put(
                    &mut chunks,
                    &mut journal,
                    p,
                    terra_core::plumbing::IRON_PIPE,
                );
                journal
                    .set_object_state(
                        p,
                        Some(BlockObjectState {
                            facing: 4,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        run(&terrain, &chunks, &mut journal, 32);
        assert!(
            journal
                .fluid_cells()
                .filter(|(_, cell)| cell.source)
                .all(|(p, _)| positions[0] == p
                    || positions[3] == p
                    || (case == "falling" && p == [3, 19, 3])),
            "{case}: unexpected source creation"
        );
    }
}

#[test]
fn a_top_slab_supports_a_pool_but_an_unloaded_border_defers_source_formation_until_reload() {
    let (terrain, mut chunks, mut journal, positions) = fixture([-1, 18, -1]);
    sources(&mut chunks, &mut journal, positions, &[0, 3]);
    for p in positions {
        let floor = [p[0], p[1] - 1, p[2]];
        put(&mut chunks, &mut journal, floor, 82);
        journal
            .set_object_state(
                floor,
                Some(BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let missing = global_to_section(positions[1]).0;
    let unloaded = chunks.loaded.remove(&missing).unwrap();
    run(&terrain, &chunks, &mut journal, 16);
    assert_eq!(
        journal
            .fluid_cells()
            .filter(|(_, cell)| cell.source)
            .count(),
        2
    );
    assert!(journal.fluid_work_counts().sleeping > 0);
    chunks.loaded.insert(missing, unloaded);
    journal.resume_fluids(missing);
    run(&terrain, &chunks, &mut journal, 32);
    assert!(
        positions
            .into_iter()
            .all(|p| journal.fluid(p).unwrap().source)
    );
    assert_quiet(&terrain, &chunks, &mut journal);
}

#[test]
fn source_formation_preflights_all_cross_section_counters_before_any_source_changes() {
    let (terrain, mut chunks, mut journal, positions) = fixture([-1, 18, -1]);
    sources(&mut chunks, &mut journal, positions, &[0, 3]);
    let owner = global_to_section(positions[1]).0;
    let mut delta = journal.section_delta(owner);
    delta.fluid_epoch = u64::MAX;
    journal.import_delta(delta).unwrap();
    let before = snapshot(&journal);
    let mut sampler = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
        river_columns: HashMap::new(),
        river_origins: HashMap::new(),
    };
    assert!(
        Resting::default()
            .settle(
                &mut sampler,
                &mut journal,
                positions[0],
                &mut FluidStep::default()
            )
            .is_err()
    );
    assert_eq!(snapshot(&journal), before);
    assert_eq!(
        journal
            .fluid_cells()
            .filter(|(_, cell)| cell.source)
            .count(),
        2
    );
}

#[test]
fn other_supported_closed_sizes_conserve_finite_water_and_stay_quiet_after_reload() {
    for (size, total) in [([1, 1], 5_u16), ([3, 3], 21), ([4, 3], 37)] {
        let (terrain, chunks, mut journal, positions) = rectangular_fixture([-1, 18, 15], size);
        let mut left = total;
        for &p in &positions {
            let amount = left.min(8) as u8;
            left -= u16::from(amount);
            if amount > 0 {
                journal
                    .set_fluid(
                        p,
                        FluidCell {
                            amount,
                            flow: [1, 0],
                            source: false,
                        },
                    )
                    .unwrap();
            }
        }
        journal.take_fluid_work(4096);
        for &p in positions.iter().rev() {
            journal.wake_fluid(p);
        }
        let step = run(&terrain, &chunks, &mut journal, 32);
        assert_eq!(step.sourced + step.replenished + step.drained, 0);
        assert_eq!(
            positions
                .iter()
                .map(|&p| u16::from(journal.fluid(p).unwrap_or_default().amount))
                .sum::<u16>(),
            total
        );
        assert!(positions.iter().all(|&p| {
            let cell = journal.fluid(p).unwrap_or_default();
            cell.flow == [0; 2] && !cell.source
        }));
        assert_quiet(&terrain, &chunks, &mut journal);
        let mut restored = EditJournal::default();
        for section in journal.changed_sections() {
            restored
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        let first = simulate_fluid_step(&terrain, &chunks, &mut restored, 256).unwrap();
        assert!(
            !first.changed,
            "reloaded stable pool must need no corrective fluid write"
        );
        assert_quiet(&terrain, &chunks, &mut restored);
        assert_eq!(
            positions
                .iter()
                .map(|&p| u16::from(restored.fluid(p).unwrap_or_default().amount))
                .sum::<u16>(),
            total
        );
    }
}

#[test]
fn exhausted_open_water_proof_budget_defers_a_closed_finite_pool_without_copying_quantity() {
    let (terrain, mut chunks, mut journal, _) = rectangular_fixture([0, 18, 0], [64, 16]);
    // A real opening in the remote edge makes this a leaking shallow area.
    put(&mut chunks, &mut journal, [-1, 18, 8], 0);
    put(&mut chunks, &mut journal, [-1, 17, 8], 0);
    let (_, closed_chunks, closed_journal, positions) = rectangular_fixture([100, 18, 0], [3, 3]);
    chunks.loaded.extend(closed_chunks.loaded);
    for section in closed_journal.changed_sections() {
        journal
            .import_delta(closed_journal.section_delta(section))
            .unwrap();
    }
    journal
        .set_fluid(
            positions[0],
            FluidCell {
                amount: 8,
                flow: [1, 0],
                source: false,
            },
        )
        .unwrap();
    journal
        .set_fluid(
            positions[3],
            FluidCell {
                amount: 3,
                flow: [1, 0],
                source: false,
            },
        )
        .unwrap();
    let before: Vec<_> = positions.iter().map(|&p| (p, journal.fluid(p))).collect();
    let mut sampler = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
        river_columns: HashMap::new(),
        river_origins: HashMap::new(),
    };
    let mut resting = Resting::default();
    let mut step = FluidStep::default();
    for x in [8, 24, 40, 56] {
        assert!(
            !resting
                .settle(&mut sampler, &mut journal, [x, 18, 8], &mut step)
                .unwrap()
        );
    }
    assert_eq!(resting.examined, STEP_POOL_BUDGET);
    assert!(resting.checked.len() <= STEP_POOL_BUDGET);
    // This dry member borders an eight-unit finite cell. Returning false here
    // would let generic lateral flow copy seven units before the proof resumes.
    assert!(
        resting
            .settle(&mut sampler, &mut journal, positions[1], &mut step)
            .unwrap()
    );
    assert!(resting.handled(positions[1]));
    assert!(!step.changed);
    assert_eq!(
        positions
            .iter()
            .map(|&p| (p, journal.fluid(p)))
            .collect::<Vec<_>>(),
        before
    );
    let mut next_step = Resting::default();
    assert!(
        next_step
            .settle(&mut sampler, &mut journal, positions[1], &mut step)
            .unwrap()
    );
    assert_eq!(
        positions
            .iter()
            .map(|&p| u16::from(journal.fluid(p).unwrap_or_default().amount))
            .sum::<u16>(),
        11
    );
    assert_eq!(step.sourced + step.replenished + step.drained, 0);
    assert!(next_step.examined <= MAX_POOL_CELLS);
}

#[test]
fn diagonal_sources_in_a_larger_pool_do_not_cascade_source_creation() {
    let (terrain, mut chunks, mut journal, positions) = rectangular_fixture([3, 18, 3], [3, 3]);
    for index in [0, 4] {
        put(&mut chunks, &mut journal, positions[index], 27);
        journal
            .set_fluid(
                positions[index],
                FluidCell {
                    amount: 8,
                    flow: [1, 0],
                    source: true,
                },
            )
            .unwrap();
    }
    run(&terrain, &chunks, &mut journal, 32);
    assert_eq!(
        positions
            .iter()
            .filter(|&&p| journal.fluid(p).unwrap().source)
            .count(),
        2
    );
    assert_eq!(
        positions
            .iter()
            .map(|&p| u16::from(journal.fluid(p).unwrap().amount))
            .sum::<u16>(),
        72
    );
    assert_quiet(&terrain, &chunks, &mut journal);
}

#[test]
fn an_unloaded_edge_does_not_hide_a_proven_too_large_loaded_water_plane() {
    let (terrain, mut chunks, mut journal, _) = rectangular_fixture([0, 18, 0], [32, 32]);
    chunks.loaded.retain(|p, _| p.x >= 0 && p.z >= 0);
    let mut sampler = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
        river_columns: HashMap::new(),
        river_origins: HashMap::new(),
    };
    let mut resting = Resting::default();
    assert!(
        !resting
            .settle(
                &mut sampler,
                &mut journal,
                [0, 18, 0],
                &mut FluidStep::default()
            )
            .unwrap()
    );
    assert_eq!(resting.examined, MAX_POOL_CELLS);
    assert_eq!(resting.checked.len(), MAX_POOL_CELLS);
    assert_eq!(journal.fluid_work_counts().sleeping, 0);
}

#[test]
fn exhausted_pool_proof_budget_never_pauses_locally_unsupported_waterfall_cells() {
    let (terrain, chunks, mut journal, _) = fixture([3, 18, 3]);
    let mut sampler = Sampler {
        terrain: &terrain,
        chunks: &chunks,
        columns: HashMap::new(),
        spaces: HashMap::new(),
        river_columns: HashMap::new(),
        river_origins: HashMap::new(),
    };
    let mut resting = Resting {
        examined: STEP_POOL_BUDGET,
        ..Default::default()
    };
    let p = [3, 19, 3];
    journal.set_fluid(p, FULL).unwrap();
    assert!(
        !resting
            .settle(&mut sampler, &mut journal, p, &mut FluidStep::default())
            .unwrap()
    );
    assert!(!resting.handled(p));
    assert_eq!(resting.examined, STEP_POOL_BUDGET);
}
