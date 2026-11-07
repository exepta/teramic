use super::*;

const INLET: [Cell; 3] = [[0, 8, -1], [0, 9, -1], [0, 10, -1]];
const PUMP_CELL: Cell = [0, 10, 0];
const OUTLET: [Cell; 3] = [[0, 10, 1], [0, 10, 2], [0, 10, 3]];
const TAP_CELL: Cell = [0, 10, 4];
const RECEIVER: BasinShape = BasinShape {
    origin: [0, 0, 4],
    size: [2, 8, 1],
};
const FINITE_SOURCE: BasinShape = BasinShape {
    origin: [0, 7, -1],
    size: [4, 1, 1],
};

fn line(gate: u16, natural: bool) -> (ChunkStore, EditJournal, crate::TerraGenerator) {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: if natural { 4. } else { -100. },
        sea_level: if natural { 8. } else { -200. },
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    if !natural {
        // All 32 units occupy the intake's connected layer. A second layer
        // below it would correctly retain water after the upper wall dries.
        // The connected cell is index zero, including the final levelled unit.
        for cell in FINITE_SOURCE.cells() {
            put(&mut chunks, &mut journal, cell, BASIN, 0, 8);
        }
    }
    for cell in RECEIVER.cells() {
        put(&mut chunks, &mut journal, cell, BASIN, 0, 0);
    }
    for (index, cell) in INLET.into_iter().enumerate() {
        put(
            &mut chunks,
            &mut journal,
            cell,
            if index == 1 { gate } else { GLASS_PIPE },
            4,
            if index == 2 { 8 } else { 0 },
        );
    }
    put(&mut chunks, &mut journal, PUMP_CELL, PUMP, 0, 0);
    for (index, cell) in OUTLET.into_iter().enumerate() {
        put(
            &mut chunks,
            &mut journal,
            cell,
            if index == 1 { gate } else { GLASS_PIPE },
            1,
            0,
        );
    }
    put(
        &mut chunks,
        &mut journal,
        TAP_CELL,
        FAUCET,
        faucet_facing(0, 3),
        0,
    );
    drive_pump(&mut journal, PUMP_CELL);
    (chunks, journal, terrain)
}

#[test]
fn open_vertical_and_horizontal_valves_match_straight_pipes_on_every_tick() {
    for natural in [false, true] {
        let (mut pipe_chunks, mut pipe_journal, terrain) = line(IRON_PIPE, natural);
        let (mut valve_chunks, mut valve_journal, _) = line(VALVE, natural);
        let mut pipe_runtime = Runtime::default();
        let mut valve_runtime = Runtime::default();
        let initial = total(&pipe_journal);
        let mut admitted = 0;
        let source = [0, 7, -1];
        let original_source = pipe_chunks.fluid_cell_at(&terrain, &pipe_journal, source, 0);
        for tick in 0..60 {
            let pipe_work = advance(
                &mut pipe_runtime,
                &mut pipe_chunks,
                &mut pipe_journal,
                &terrain,
                0.2,
            )
            .unwrap();
            let valve_work = advance(
                &mut valve_runtime,
                &mut valve_chunks,
                &mut valve_journal,
                &terrain,
                0.2,
            )
            .unwrap();
            assert_eq!(
                pipe_work.moved, valve_work.moved,
                "same throughput at tick {tick}, natural={natural}"
            );
            let pipe_states: Vec<_> = pipe_journal
                .plumbing()
                .map(|(cell, state)| (cell, state.clone()))
                .collect();
            let valve_states: Vec<_> = valve_journal
                .plumbing()
                .map(|(cell, state)| (cell, state.clone()))
                .collect();
            assert_eq!(
                pipe_states, valve_states,
                "open valves must add no pressure, storage, source demand or transit delay at tick {tick}, natural={natural}"
            );
            admitted += pipe_journal
                .plumbing()
                .filter_map(|(_, state)| state.intake)
                .map(|intake| u32::from(intake.units))
                .sum::<u32>();
            assert_eq!(
                total(&pipe_journal),
                initial + admitted,
                "every admitted unit stays accounted for"
            );
            assert_eq!(total(&valve_journal), initial + admitted);
            if natural {
                assert_eq!(
                    pipe_chunks.fluid_cell_at(&terrain, &pipe_journal, source, 0),
                    original_source
                );
                assert_eq!(
                    valve_chunks.fluid_cell_at(&terrain, &valve_journal, source, 0),
                    original_source
                );
                if tick >= 12 {
                    for cell in INLET {
                        assert!(
                            pipe_journal.plumbing_at(cell).unwrap().water.amount > 0,
                            "a continuously supplied suction route must not periodically empty at {cell:?}, tick {tick}"
                        );
                    }
                    for cell in OUTLET {
                        let state = pipe_journal.plumbing_at(cell).unwrap();
                        assert!(
                            state.water.amount > 0 || state.flowing,
                            "real water or confirmed transit must stay visible at {cell:?}, tick {tick}"
                        );
                    }
                }
            }
        }
        if !natural {
            assert_eq!(basin_amount(&pipe_journal, FINITE_SOURCE), 0);
            assert_eq!(basin_amount(&pipe_journal, RECEIVER), initial as u16);
        }
    }
}

#[test]
fn a_higher_stored_suction_cell_cannot_push_water_back_down_or_block_the_live_source() {
    for gate in [IRON_PIPE, VALVE] {
        let (mut chunks, mut journal, terrain) = line(gate, true);
        let mut runtime = Runtime::default();
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(
            journal.plumbing_at(INLET[0]).unwrap().water.amount,
            0,
            "the fresh sea intake must move up toward the drive"
        );
        assert_eq!(journal.plumbing_at(INLET[1]).unwrap().water.amount, 8);
        assert_eq!(
            journal.plumbing_at(INLET[2]).unwrap().water.amount,
            0,
            "stored upper water must cross the pump, never fall back through the lower inlet"
        );
        assert_eq!(
            u16::from(journal.plumbing_at(PUMP_CELL).unwrap().water.amount)
                + basin_amount(&journal, RECEIVER),
            8
        );
        assert_eq!(total(&journal), 16);
        assert_eq!(
            journal.plumbing_at(INLET[1]).unwrap().water.flow,
            [0; 2],
            "vertical transit must not invent a horizontal direction"
        );
        assert!(journal.plumbing_at(INLET[0]).unwrap().flowing);
        assert!(journal.plumbing_at(INLET[1]).unwrap().flowing);
    }
}
