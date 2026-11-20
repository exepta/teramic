use super::*;

fn ocean() -> crate::TerraGenerator {
    crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: 4.,
        sea_level: 8.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap()
}

fn lifted_intake(chunks: &mut ChunkStore, journal: &mut EditJournal) -> Cell {
    let pump = [0, 10, 0];
    put(chunks, journal, pump, PUMP, 0, 0);
    for y in 7..=10 {
        put(chunks, journal, [0, y, -1], IRON_PIPE, 4, 0);
    }
    put(chunks, journal, [0, 10, 1], BASIN, 0, 0);
    drive_pump(journal, pump);
    pump
}

#[test]
fn natural_ocean_intake_lifts_water_to_a_pump_above_the_surface() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = ocean();
    let source = [0, 7, -2];
    let water = chunks.fluid_cell_at(&terrain, &journal, source, 0);
    assert!(water.source && water.amount == 8);
    lifted_intake(&mut chunks, &mut journal);
    let mut runtime = Runtime::default();
    for _ in 0..20 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(journal.plumbing_at([0, 10, 1]).unwrap().water.amount, 8);
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, source, 0), water);
    assert!(
        journal.fluid(source).is_none(),
        "the implicit sea stays intact"
    );
    assert!(journal.plumbing().all(|(_, state)| !state.water.source));
}

#[test]
fn natural_intake_requires_power_an_open_valve_and_a_live_water_source() {
    for blocked in ["stopped", "valve", "dry", "reversed", "unloaded"] {
        let (mut chunks, mut journal, _) = fixture();
        let terrain = ocean();
        let pump = lifted_intake(&mut chunks, &mut journal);
        match blocked {
            "stopped" => journal.replace_kinetic_states(Default::default()),
            "valve" => put(&mut chunks, &mut journal, [0, 9, -1], VALVE, 4 | 8, 0),
            "dry" => {
                // Explicitly drained natural water must not silently refill the inlet.
                for direction in DIRECTIONS {
                    let cell = std::array::from_fn(|axis| [0, 7, -1][axis] + direction[axis]);
                    if chunks.block_at(cell).is_some_and(is_part) {
                        continue;
                    }
                    put(&mut chunks, &mut journal, cell, 0, 0, 0);
                    journal.set_fluid(cell, FluidCell::default()).unwrap();
                }
            }
            "reversed" => {
                let state = journal.kinetic_state(pump);
                journal.replace_kinetic_states(std::collections::BTreeMap::from([(
                    pump,
                    kinetic::KineticState {
                        rpm: -state.rpm,
                        ..state
                    },
                )]));
            }
            "unloaded" => {
                chunks.loaded.remove(&global_to_section([0, 7, -1]).0);
            }
            _ => unreachable!(),
        }
        let mut runtime = Runtime::default();
        for _ in 0..20 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert_eq!(
            journal.plumbing_at([0, 10, 1]).unwrap().water.amount,
            0,
            "{blocked}"
        );
    }
}

#[test]
fn suction_lifts_a_finite_basin_without_creating_more_water() {
    let (mut chunks, mut journal, terrain) = fixture();
    let pump = lifted_intake(&mut chunks, &mut journal);
    // This generator has dry ground here; this is a finite reservoir, not the sea.
    put(&mut chunks, &mut journal, [0, 7, -2], BASIN, 0, 8);
    let initial = total(&journal);
    let mut runtime = Runtime::default();
    for _ in 0..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), initial);
        assert!(journal.kinetic_state(pump).running());
    }
    assert!(journal.plumbing_at([0, 10, 1]).unwrap().water.amount > 0);
}

#[test]
fn a_finite_pool_lowers_evenly_then_stops_at_the_dry_downward_mouth() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    lifted_intake(&mut chunks, &mut journal);
    let inlet = [0, 7, -1];
    let source = [0, 6, -1];
    put(&mut chunks, &mut journal, source, 27, 0, 0);
    put(&mut chunks, &mut journal, [1, 6, -1], 27, 0, 0);
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let activity = journal.plumbing_at(inlet).unwrap().intake.unwrap();
    assert_eq!(activity.source, source);
    assert_eq!(activity.direction, [0, -1, 0]);
    assert_eq!(activity.units, 8);
    for _ in 0..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(
        journal
            .fluid_cells()
            .map(|(_, water)| u16::from(water.amount))
            .sum::<u16>(),
        8
    );
    // Drawing eight units lowers both cells to half depth. The actual opening
    // is at Y=7, above the remaining Y=6.5 surface; it must not reach down and
    // teleport the residue into the pipe merely to finish emptying this pool.
    for p in [source, [1, 6, -1]] {
        let water = journal.fluid(p).unwrap();
        assert_eq!(water.amount, 4);
        assert!(!water.source);
        assert!(p[1] as f64 + water.height() < inlet[1] as f64);
    }
    assert_eq!(
        journal
            .plumbing()
            .map(|(_, node)| u32::from(node.water.amount))
            .sum::<u32>(),
        8
    );
    assert_eq!(total(&journal), 16);
    assert!(journal.plumbing_at([0, 10, 1]).unwrap().water.amount > 0);
    assert!(journal.plumbing_at(inlet).unwrap().intake.is_none());
}

#[test]
fn submerged_vertical_pipes_cannot_suck_through_their_closed_side_walls() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    lifted_intake(&mut chunks, &mut journal);
    let source = [1, 7, -1];
    put(&mut chunks, &mut journal, source, 27, 0, 0);
    let before = journal.fluid(source);
    let initial = total(&journal);
    let mut runtime = Runtime::default();
    for _ in 0..20 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(total(&journal), initial);
    assert!(journal.plumbing().all(|(_, node)| node.water.amount == 0));
    assert_eq!(journal.fluid(source), before);
    assert!(journal.plumbing().all(|(_, node)| node.intake.is_none()));
}

#[test]
fn an_upward_mouth_below_a_small_pool_drains_all_its_finite_water() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let pump = [0, 4, 0];
    let inlet = [0, 5, -1];
    for (p, material, facing) in [
        (pump, PUMP, 0),
        ([0, 4, -1], IRON_PIPE, 4),
        (inlet, IRON_PIPE, 4),
        ([0, 4, 1], BASIN, 0),
    ] {
        put(&mut chunks, &mut journal, p, material, facing, 0);
    }
    drive_pump(&mut journal, pump);
    let sources = [[0, 6, -1], [1, 6, -1]];
    for source in sources {
        put(&mut chunks, &mut journal, source, 27, 0, 0);
    }
    let initial = total(&journal);
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let activity = journal.plumbing_at(inlet).unwrap().intake.unwrap();
    assert_eq!(activity.source, sources[0]);
    assert_eq!(activity.direction, [0, 1, 0]);
    assert_eq!(activity.units, 8);
    for _ in 0..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(initial, 16);
    assert_eq!(total(&journal), initial);
    for source in sources {
        assert_eq!(journal.fluid(source), Some(FluidCell::default()));
    }
    assert_eq!(
        journal
            .plumbing()
            .map(|(_, state)| u32::from(state.water.amount))
            .sum::<u32>(),
        16
    );
    assert_eq!(journal.plumbing_at([0, 4, 1]).unwrap().water.amount, 8);
    assert!(journal.plumbing().all(|(_, state)| !state.water.source));
}

#[test]
fn filled_two_by_two_pool_drains_after_finite_pump_withdrawal_across_replica_reload() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    for x in -2..=1 {
        for z in -2..=1 {
            put(&mut chunks, &mut journal, [x, 5, z], 3, 0, 0);
            if x == -2 || x == 1 || z == -2 || z == 1 {
                put(&mut chunks, &mut journal, [x, 6, z], 3, 0, 0);
            }
        }
    }
    let positions = [[-1, 6, -1], [0, 6, -1], [-1, 6, 0], [0, 6, 0]];
    for p in positions {
        put(&mut chunks, &mut journal, p, 0, 0, 0);
    }
    for p in [positions[0], positions[3]] {
        put(&mut chunks, &mut journal, p, 27, 0, 0);
    }
    for _ in 0..32 {
        crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    for (index, &p) in positions.iter().enumerate() {
        let cell = journal.fluid(p).unwrap();
        assert_eq!(cell.amount, 8);
        assert_eq!(cell.source, index == 0 || index == 3);
    }
    let withdrawn =
        super::super::intake::take(&chunks, &mut journal, &terrain, positions[0], 8).unwrap();
    assert_eq!(withdrawn, 8);
    for p in positions {
        assert_eq!(
            journal.fluid(p),
            Some(FluidCell {
                amount: 6,
                flow: [0; 2],
                source: false
            })
        );
    }
    let mut replica = EditJournal::default();
    for section in journal.changed_sections() {
        replica
            .import_delta(journal.section_delta(section))
            .unwrap();
    }
    let mut drained = 0;
    for _ in 0..32 {
        let step = crate::water::simulate_fluid_step(&terrain, &chunks, &mut replica, 256).unwrap();
        assert_eq!(step.sourced + step.replenished, 0);
        drained += step.drained;
    }
    for p in positions {
        assert_eq!(replica.fluid(p).unwrap_or_default(), FluidCell::default());
    }
    assert_eq!(drained + u64::from(withdrawn), 32);
    assert_eq!(replica.fluid_work_counts().queued, 0);
}
