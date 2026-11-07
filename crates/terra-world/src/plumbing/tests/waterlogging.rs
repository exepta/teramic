use super::*;
use crate::streaming::{WorldAccess, WorldTerrain};
use bevy::prelude::*;

#[derive(Resource, Default)]
struct NetworkClock(Runtime);

fn tick_network(
    time: Res<Time>,
    terrain: Res<WorldTerrain>,
    mut runtime: ResMut<NetworkClock>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
) {
    advance(
        &mut runtime.0,
        &mut chunks,
        &mut journal,
        &terrain.0,
        time.delta_secs(),
    )
    .unwrap();
}

fn dry_terrain() -> crate::TerraGenerator {
    crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        caves: false,
        rivers: false,
        biomes: false,
        trees: false,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn combined_fluid_and_plumbing_ticks_do_not_drain_a_pool_just_because_idle_pipes_are_placed() {
    for material in [WOOD_PIPE, STONE_PIPE, IRON_PIPE, GLASS_PIPE] {
        for facing in [0, 1, 4] {
            let (mut chunks, mut journal, _) = fixture();
            for x in 14..=17 {
                for z in 6..=9 {
                    for y in 16..=20 {
                        let wall = x == 14 || x == 17 || z == 6 || z == 9;
                        let raw = if y == 16 || wall {
                            3
                        } else if y < 20 {
                            27
                        } else {
                            0
                        };
                        put(&mut chunks, &mut journal, [x, y, z], raw, 0, 0);
                    }
                }
            }
            let pipe = [16, 17, 8];
            put(&mut chunks, &mut journal, pipe, material, facing, 0);
            let expected: std::collections::BTreeMap<_, _> = journal.fluid_cells().collect();
            assert_eq!(
                expected
                    .values()
                    .map(|water| u32::from(water.amount))
                    .sum::<u32>(),
                96
            );
            assert!(
                expected
                    .values()
                    .all(|water| water.amount == 8 && water.source)
            );
            let initial = total(&journal);
            let revision = journal.gameplay().revision;
            let epochs: Vec<_> = journal
                .changed_sections()
                .map(|p| (p, journal.fluid_epoch(p), journal.water_mesh_epoch(p)))
                .collect();
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(WorldAccess::Server)
                .insert_resource(WorldTerrain(dry_terrain()))
                .insert_resource(chunks)
                .insert_resource(journal)
                .init_resource::<NetworkClock>()
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                    std::time::Duration::from_millis(110),
                ))
                .add_systems(Update, (crate::water::tick_fluids, tick_network).chain());
            for _ in 0..48 {
                app.update();
            }
            assert!(app.world().resource::<Time>().elapsed_secs_f64() > 4.);
            let journal = app.world().resource::<EditJournal>();
            assert_eq!(
                journal
                    .fluid_cells()
                    .collect::<std::collections::BTreeMap<_, _>>(),
                expected
            );
            assert_eq!(
                journal.plumbing_at(pipe).unwrap().water,
                FluidCell::default()
            );
            assert!(journal.plumbing_at(pipe).unwrap().intake.is_none());
            assert_eq!(journal.gameplay().revision, revision);
            assert_eq!(
                total(journal),
                initial,
                "ambient and stored water must never be duplicated"
            );
            for (p, fluid, mesh) in epochs {
                assert_eq!(journal.fluid_epoch(p), fluid);
                assert_eq!(journal.water_mesh_epoch(p), mesh);
            }
        }
    }
}

#[test]
fn a_closed_valve_or_stopped_pump_cannot_request_raw_water_for_a_distant_faucet() {
    for blocked in ["closed_valve", "stopped_pump"] {
        let (mut chunks, mut journal, _) = fixture();
        let terrain = dry_terrain();
        let source = [0, 6, 0];
        let pipe = [0, 5, 0];
        let gate = [1, 4, 0];
        put(&mut chunks, &mut journal, source, 27, 0, 0);
        put(&mut chunks, &mut journal, pipe, GLASS_PIPE, 4, 0);
        put(&mut chunks, &mut journal, [0, 4, 0], GLASS_PIPE, 4, 0);
        let (material, facing) = if blocked == "closed_valve" {
            (VALVE, 8)
        } else {
            (PUMP, 1)
        };
        put(&mut chunks, &mut journal, gate, material, facing, 0);
        put(&mut chunks, &mut journal, [2, 4, 0], FAUCET, 1, 0);
        assert!(joined(GLASS_PIPE, 4, material, facing, [1, 0, 0]));
        assert!(joined(material, facing, FAUCET, 1, [1, 0, 0]));
        let original = journal.fluid(source);
        let initial = total(&journal);
        let mut runtime = Runtime::default();
        for _ in 0..12 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert_eq!(journal.fluid(source), original, "{blocked}");
        assert_eq!(total(&journal), initial);
        assert!(
            journal
                .plumbing()
                .all(|(_, state)| state.water.amount == 0 && state.intake.is_none())
        );
    }
}

#[test]
fn actual_gravity_demand_and_running_suction_still_debit_finite_raw_water() {
    for powered in [false, true] {
        let (mut chunks, mut journal, _) = fixture();
        let terrain = dry_terrain();
        let source = [0, 6, 0];
        let gate = [1, 4, 0];
        put(&mut chunks, &mut journal, source, 27, 0, 0);
        put(&mut chunks, &mut journal, [0, 5, 0], GLASS_PIPE, 4, 0);
        put(&mut chunks, &mut journal, [0, 4, 0], GLASS_PIPE, 4, 0);
        put(
            &mut chunks,
            &mut journal,
            gate,
            if powered { PUMP } else { VALVE },
            // Valves use kinetic::axis; pumps use kinetic::forward.
            if powered { 1 } else { 0 },
            0,
        );
        let tap = [2, 4, 0];
        put(&mut chunks, &mut journal, tap, FAUCET, 1, 0);
        if powered {
            drive_pump(&mut journal, gate);
        }
        let gate_material = journal.block(gate).unwrap();
        let gate_facing = journal.object_state(gate).unwrap().facing;
        assert!(joined(GLASS_PIPE, 4, gate_material, gate_facing, [1, 0, 0]));
        assert!(joined(gate_material, gate_facing, FAUCET, 1, [1, 0, 0]));
        let initial = total(&journal);
        let mut runtime = Runtime::default();
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        let water = journal.fluid(source).unwrap();
        assert_eq!(water.amount, 0, "powered={powered}");
        assert!(!water.source);
        assert_eq!(total(&journal), initial);
        assert!(journal.plumbing_at([0, 5, 0]).unwrap().intake.is_some());
        assert!(
            journal.plumbing().any(|(_, state)| state.water.amount > 0)
                || journal
                    .fluid([2, 3, 0])
                    .is_some_and(|water| water.amount > 0)
        );
    }
}

#[test]
fn a_consumer_above_the_available_head_does_not_prime_an_idle_gravity_route() {
    let (mut chunks, mut journal, _) = fixture();
    let terrain = dry_terrain();
    let source = [0, 6, 0];
    put(&mut chunks, &mut journal, source, 27, 0, 0);
    put(&mut chunks, &mut journal, [0, 5, 0], GLASS_PIPE, 4, 0);
    put(&mut chunks, &mut journal, [0, 4, 0], GLASS_PIPE, 4, 0);
    for y in 4..=8 {
        put(&mut chunks, &mut journal, [1, y, 0], GLASS_PIPE, 4, 0);
    }
    put(&mut chunks, &mut journal, [2, 8, 0], FAUCET, 1, 0);
    let original = journal.fluid(source);
    let initial = total(&journal);
    let mut runtime = Runtime::default();
    for _ in 0..8 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(journal.fluid(source), original);
    assert_eq!(total(&journal), initial);
    assert!(
        journal
            .plumbing()
            .all(|(_, state)| state.water.amount == 0 && state.intake.is_none())
    );
}
