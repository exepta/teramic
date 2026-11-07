use super::*;

#[test]
fn valve_placement_preview_and_transaction_use_each_clicked_axis() {
    for facing in 0..6 {
        let (mut chunks, mut journal, _) = fixture();
        let support = [7, 6, 7];
        put(&mut chunks, &mut journal, support, 3, 0, 0);
        let direction = kinetic::axis(facing);
        let hit = terra_voxel::blocks::BlockHit {
            position: support,
            normal: direction.map(|v| v as i32),
            distance: 3.5,
        };
        let ray = (
            std::array::from_fn(|i| support[i] as f64 + 0.5 + direction[i] as f64 * 4.0),
            direction.map(|v| -v as f64),
        );
        assert_eq!(
            crate::targeting::placement_facing(&chunks, &journal, hit, VALVE, ray),
            facing
        );
        let plan = crate::placement::resolve(
            &chunks,
            &journal,
            hit,
            terra_core::ItemKind::Block(VALVE),
            ray,
            [0.0; 2],
            false,
        )
        .unwrap();
        let target = std::array::from_fn(|i| support[i] + direction[i]);
        assert_eq!(plan.edits.len(), 1);
        assert_eq!(plan.edits[0].position, target);
        let state = plan
            .states
            .iter()
            .find(|(cell, _)| *cell == target)
            .unwrap()
            .1
            .unwrap();
        assert_eq!(
            state.facing, facing,
            "clicked orientation must override yaw"
        );
        assert!(state.valid_for(target, VALVE));
        assert_eq!(
            ports(VALVE, state.facing),
            3 << (direction.iter().position(|v| *v != 0).unwrap() * 2)
        );
        let bounds = plumbing_boxes(&journal, target, VALVE, state, |cell| chunks.block_at(cell));
        let axis = direction.iter().position(|v| *v != 0).unwrap();
        assert!(bounds.iter().any(|shape| shape.minimum[axis] == 0.0));
        assert!(bounds.iter().any(|shape| shape.maximum[axis] == 1.0));
    }
}

#[test]
fn closing_a_valve_stops_replenishment_but_drains_only_the_finite_downstream_storage() {
    let (mut chunks, mut journal, terrain) = fixture();
    finite_line(&mut chunks, &mut journal);
    let valve = [1, 2, 0];
    let pipes = [[2, 2, 0], [3, 2, 0]];
    let tap = [4, 2, 0];
    for pipe in pipes {
        put(&mut chunks, &mut journal, pipe, GLASS_PIPE, 0, 0);
    }
    put(&mut chunks, &mut journal, tap, FAUCET, 1, 0);
    // A blocked outlet lets the real gravity supply fill the downstream line.
    put(&mut chunks, &mut journal, [4, 1, 0], 3, 0, 0);
    let mut runtime = Runtime::default();
    let initial = total(&journal);
    for _ in 0..12 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), initial);
    }
    let downstream = [pipes[0], pipes[1], tap];
    assert!(
        downstream
            .iter()
            .all(|&cell| journal.plumbing_at(cell).unwrap().water.amount == 8)
    );
    assert!(toggle(&mut chunks, &mut journal, valve, VALVE, 0).unwrap());
    assert!(
        !tap_available(&chunks, &journal, tap, 1),
        "old pressure is invalidated immediately"
    );
    let upstream: Vec<_> = journal
        .plumbing()
        .filter(|(cell, _)| !downstream.contains(cell))
        .map(|(cell, state)| (cell, state.water.amount))
        .collect();
    // Opening a real receiver allows exactly the 24 stored downstream units out.
    let receiver = BasinShape {
        origin: [4, -2, 0],
        size: [1, 4, 1],
    };
    for cell in receiver.cells() {
        put(&mut chunks, &mut journal, cell, BASIN, 0, 0);
    }
    let mut saw_less = false;
    for _ in 0..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(
            total(&journal),
            initial,
            "closing never deletes stored water"
        );
        saw_less |= pipes
            .iter()
            .any(|&cell| journal.plumbing_at(cell).unwrap().water.amount < 8);
        for &(cell, amount) in &upstream {
            assert_eq!(
                journal.plumbing_at(cell).unwrap().water.amount,
                amount,
                "no water crosses the shut valve at {cell:?}"
            );
        }
    }
    assert!(saw_less);
    assert_eq!(basin_amount(&journal, receiver), 24);
    for cell in downstream {
        let state = journal.plumbing_at(cell).unwrap();
        assert_eq!(state.water.amount, 0);
        assert_eq!(state.water.flow, [0; 2]);
        assert!(state.intake.is_none());
    }
    assert!(!tap_available(&chunks, &journal, tap, 1));
}

#[test]
fn finite_stored_water_stays_put_without_an_open_reachable_outlet() {
    for barrier in ["closed_tap", "uphill", "closed_valve", "stopped_pump"] {
        let (mut chunks, mut journal, terrain) = fixture();
        let inlet = [0, 3, 0];
        put(&mut chunks, &mut journal, inlet, GLASS_PIPE, 0, 8);
        let tap = if barrier == "uphill" {
            put(&mut chunks, &mut journal, [1, 3, 0], GLASS_PIPE, 4, 0);
            put(&mut chunks, &mut journal, [1, 4, 0], GLASS_PIPE, 4, 0);
            [2, 4, 0]
        } else {
            let (material, facing) = match barrier {
                "closed_valve" => (VALVE, 8),
                "stopped_pump" => (PUMP, 1),
                _ => (GLASS_PIPE, 0),
            };
            put(&mut chunks, &mut journal, [1, 3, 0], material, facing, 0);
            [2, 3, 0]
        };
        put(
            &mut chunks,
            &mut journal,
            tap,
            FAUCET,
            if barrier == "closed_tap" { 5 } else { 1 },
            0,
        );
        let initial = total(&journal);
        let mut runtime = Runtime::default();
        for _ in 0..12 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
            assert_eq!(total(&journal), initial, "{barrier}");
            assert_eq!(
                journal.plumbing_at(inlet).unwrap().water.amount,
                8,
                "{barrier}"
            );
            assert!(!tap_available(&chunks, &journal, tap, 1), "{barrier}");
            assert!(
                journal
                    .plumbing()
                    .all(|(_, state)| state.intake.is_none() && state.water.flow == [0; 2]),
                "{barrier}"
            );
        }
    }
}

#[test]
fn a_running_pump_can_lift_the_finite_water_already_in_its_inlet_without_a_live_raw_source() {
    let (mut chunks, mut journal, terrain) = fixture();
    let source = [0, 2, -1];
    let pump = [0, 3, 0];
    let receiver = [0, 3, 1];
    put(&mut chunks, &mut journal, source, GLASS_PIPE, 4, 8);
    put(&mut chunks, &mut journal, [0, 3, -1], GLASS_PIPE, 4, 0);
    put(&mut chunks, &mut journal, pump, PUMP, 0, 0);
    put(&mut chunks, &mut journal, receiver, BASIN, 0, 0);
    drive_pump(&mut journal, pump);
    let initial = total(&journal);
    assert_eq!(initial, 8);
    let mut runtime = Runtime::default();
    for _ in 0..20 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), initial);
    }
    assert_eq!(journal.plumbing_at(receiver).unwrap().water.amount, 8);
    assert_eq!(journal.plumbing_at(source).unwrap().water.amount, 0);
    assert!(journal.plumbing().all(|(_, state)| state.intake.is_none()));
}

#[test]
fn a_faucet_drains_its_own_finite_buffer_without_pushing_water_back_through_its_inlet() {
    for buffered_in_tap in [false, true] {
        let (mut chunks, mut journal, terrain) = fixture();
        let upstream = [-1, 3, 0];
        let pipe = [0, 3, 0];
        let tap = [1, 3, 0];
        let receiver = [1, 2, 0];
        put(&mut chunks, &mut journal, upstream, GLASS_PIPE, 0, 0);
        put(
            &mut chunks,
            &mut journal,
            pipe,
            GLASS_PIPE,
            0,
            if buffered_in_tap { 0 } else { 7 },
        );
        put(
            &mut chunks,
            &mut journal,
            tap,
            FAUCET,
            1,
            if buffered_in_tap { 7 } else { 0 },
        );
        put(&mut chunks, &mut journal, receiver, BASIN, 0, 0);
        // This regression starts with a usable outlet, as the native fixture
        // does after priming. Its separate basin component is otherwise first
        // discovered after the faucet component in the initial scheduler tick.
        let shape = basin_shape(&chunks, &journal, receiver).unwrap();
        let mut state = journal.plumbing_at(receiver).unwrap().clone();
        state.basin = Some(shape);
        journal.set_plumbing(receiver, state).unwrap();
        let mut runtime = Runtime::default();
        for received in 1..=7 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
            assert_eq!(total(&journal), 7);
            for cell in [upstream, pipe] {
                assert_eq!(
                    journal.plumbing_at(cell).unwrap().water.amount,
                    0,
                    "finite storage must drain toward the consumer, never into the empty upstream branch"
                );
            }
            assert_eq!(journal.plumbing_at(tap).unwrap().water.amount, 7 - received);
            assert_eq!(
                journal.plumbing_at(receiver).unwrap().water.amount,
                received
            );
        }
    }
}

#[test]
fn native_vertical_and_horizontal_valve_line_drains_all_32_stored_units_at_the_real_faucet_rate() {
    let (mut chunks, mut journal, terrain) = fixture();
    let source = BasinShape {
        origin: [19, 23, -1],
        size: [2, 2, 2],
    };
    let receiver = BasinShape {
        origin: [23, 16, 0],
        size: [2, 4, 1],
    };
    for (shape, amount) in [(source, 8), (receiver, 0)] {
        for cell in shape.cells() {
            put(&mut chunks, &mut journal, cell, BASIN, 0, amount);
        }
    }
    let vertical = [20, 22, 0];
    let horizontal = [21, 20, 0];
    let glass = [[20, 21, 0], [20, 20, 0], [22, 20, 0]];
    let tap = [23, 20, 0];
    put(&mut chunks, &mut journal, vertical, VALVE, 4, 0);
    put(&mut chunks, &mut journal, horizontal, VALVE, 0, 0);
    for (cell, facing) in [(glass[0], 4), (glass[1], 4), (glass[2], 0)] {
        put(&mut chunks, &mut journal, cell, GLASS_PIPE, facing, 0);
    }
    put(
        &mut chunks,
        &mut journal,
        tap,
        FAUCET,
        faucet_facing(1, 0),
        0,
    );
    let mut runtime = Runtime::default();
    for _ in 0..12 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), 64);
    }
    assert!(
        glass
            .iter()
            .all(|&cell| journal.plumbing_at(cell).unwrap().water.amount == 8)
    );
    assert_eq!(journal.plumbing_at(horizontal).unwrap().water.amount, 8);
    assert_eq!(journal.plumbing_at(vertical).unwrap().water.amount, 8);
    assert_eq!(basin_amount(&journal, source), 24);
    assert_eq!(basin_amount(&journal, receiver), 0);
    assert!(toggle(&mut chunks, &mut journal, vertical, VALVE, 4).unwrap());
    assert!(toggle(&mut chunks, &mut journal, tap, FAUCET, faucet_facing(1, 0)).unwrap());
    // One unit every 0.2-second tick is the configured level-one outlet rate.
    // No pressure-priority cycle may push that unit back up the inlet instead.
    for received in 1..=32 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), 64);
        assert_eq!(basin_amount(&journal, source), 24);
        assert_eq!(journal.plumbing_at(vertical).unwrap().water.amount, 8);
        assert_eq!(
            basin_amount(&journal, receiver),
            received,
            "the level-one faucet must drain once per tick"
        );
    }
    for cell in glass.into_iter().chain([horizontal, tap]) {
        assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 0);
    }
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    assert_eq!(total(&journal), 64);
    assert_eq!(basin_amount(&journal, receiver), 32);
    assert!(!tap_available(&chunks, &journal, tap, 1));
}
