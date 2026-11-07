use super::*;

#[test]
fn stepped_faucet_fills_a_separate_basin_across_an_air_gap_without_losing_water() {
    for level in 1..=3 {
        let (mut chunks, mut journal, terrain) = fixture();
        let tap = finite_line(&mut chunks, &mut journal);
        for y in -4..=-3 {
            put(&mut chunks, &mut journal, [2, y, 0], BASIN, 0, 0);
        }
        let mut pose = journal.object_state(tap).unwrap();
        pose.facing = faucet_facing(pose.facing, level);
        journal.set_object_state(tap, Some(pose)).unwrap();
        let mut runtime = Runtime::default();
        let initial = total(&journal);
        for _ in 0..2 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        let shape = journal.plumbing_at([2, -3, 0]).unwrap().basin.unwrap();
        let before = basin_amount(&journal, shape);
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(
            basin_amount(&journal, shape) - before,
            u16::from(faucet_units(pose.facing)),
            "level {level}"
        );
        assert_eq!(total(&journal), initial);
        assert!(
            journal.fluid_cells().all(|(_, cell)| cell.amount == 0),
            "a captured stream cannot also spawn terrain water"
        );
        for _ in 0..30 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert_eq!(basin_amount(&journal, shape), shape.capacity());
        assert_eq!(total(&journal), initial);
    }
}

#[test]
fn faucet_stream_does_not_fill_a_basin_through_a_solid_obstacle() {
    let (mut chunks, mut journal, terrain) = fixture();
    finite_line(&mut chunks, &mut journal);
    put(&mut chunks, &mut journal, [2, -3, 0], BASIN, 0, 0);
    put(&mut chunks, &mut journal, [2, 0, 0], 3, 0, 0);
    let mut runtime = Runtime::default();
    let initial = total(&journal);
    for _ in 0..20 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(journal.plumbing_at([2, -3, 0]).unwrap().water.amount, 0);
    assert_eq!(total(&journal), initial);
}

#[test]
fn rain_fills_only_open_basins_slowly_and_respects_saved_unloaded_roofs() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -10.0,
        sea_level: -30.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        biomes: false,
        trees: false,
        caves: false,
        rivers: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let open = [7, 2, 7];
    let covered = [9, 2, 7];
    put(&mut chunks, &mut journal, open, BASIN, 0, 0);
    put(&mut chunks, &mut journal, covered, BASIN, 0, 0);
    // The roof remains effective while its section is not streamed.
    journal.record([9, 40, 7], 3).unwrap();
    let mut network = Runtime::default();
    step(&mut network, &mut chunks, &mut journal, &terrain);
    let mut weather = terra_core::WorldWeather::default();
    weather.set_pattern(terra_core::WeatherPattern::Storm);
    weather.advance(8.0, terrain.config().seed.0);
    let clock = terra_core::WorldClock::default();
    assert!(crate::weather::sample_at(&weather, &terrain, [7.5, 2.5, 7.5], clock).rain > 0.5);
    let mut rain = super::super::RainRuntime::default();
    for _ in 0..10 {
        super::super::collect_rain(
            &mut rain,
            &chunks,
            &mut journal,
            &terrain,
            &weather,
            clock,
            1.0,
        )
        .unwrap();
    }
    assert_eq!(
        journal.plumbing_at(open).unwrap().water.amount,
        0,
        "ten seconds of rain cannot fill a module"
    );
    for _ in 0..55 {
        super::super::collect_rain(
            &mut rain,
            &chunks,
            &mut journal,
            &terrain,
            &weather,
            clock,
            1.0,
        )
        .unwrap();
    }
    assert!((1..=2).contains(&journal.plumbing_at(open).unwrap().water.amount));
    assert_eq!(journal.plumbing_at(covered).unwrap().water.amount, 0);
    journal.record([9, 40, 7], 0).unwrap();
    for _ in 0..300 {
        super::super::collect_rain(
            &mut rain,
            &chunks,
            &mut journal,
            &terrain,
            &weather,
            clock,
            1.0,
        )
        .unwrap();
    }
    assert_eq!(journal.plumbing_at(open).unwrap().water.amount, 8);
    assert_eq!(journal.plumbing_at(covered).unwrap().water.amount, 8);
}
