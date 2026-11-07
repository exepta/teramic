use super::*;
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use terra_core::{
    kinetic::{self, Node},
    plumbing::*,
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[path = "tests/faucet.rs"]
mod faucet;
#[path = "tests/intake.rs"]
mod intake;
#[path = "tests/kingdom_edges.rs"]
mod kingdom_edges;
#[path = "tests/transit.rs"]
mod transit;
#[path = "tests/valve.rs"]
mod valve;
#[path = "tests/valve_transparency.rs"]
mod valve_transparency;
#[path = "tests/waterlogging.rs"]
mod waterlogging;

fn fixture() -> (ChunkStore, EditJournal, crate::TerraGenerator) {
    let mut chunks = ChunkStore::default();
    for x in -2..=1 {
        for y in -1..=1 {
            for z in -2..=1 {
                let position = SectionPos { x, y, z };
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
    }
    (
        chunks,
        EditJournal::default(),
        crate::TerraGenerator::new(Default::default()).unwrap(),
    )
}
fn put(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    cell: Cell,
    material: u16,
    facing: u8,
    water: u8,
) {
    let (section, local) = global_to_section(cell);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(cell, material).unwrap();
    if is_part(material) {
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal
            .set_plumbing(
                cell,
                PlumbingState {
                    water: FluidCell {
                        amount: water,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
    }
}
fn finite_line(chunks: &mut ChunkStore, journal: &mut EditJournal) -> Cell {
    finite_line_at(chunks, journal, 0)
}
fn finite_line_at(chunks: &mut ChunkStore, journal: &mut EditJournal, x_offset: i64) -> Cell {
    for x in -2..=-1 {
        for y in 3..=4 {
            for z in -1..=0 {
                put(chunks, journal, [x + x_offset, y, z], BASIN, 0, 8);
            }
        }
    }
    for (cell, material, facing) in [
        ([-1, 2, 0], WOOD_PIPE, 4),
        ([0, 2, 0], IRON_PIPE, 0),
        ([1, 2, 0], VALVE, 0),
        ([2, 2, 0], FAUCET, 1),
    ] {
        put(
            chunks,
            journal,
            [cell[0] + x_offset, cell[1], cell[2]],
            material,
            facing,
            0,
        );
    }
    [2 + x_offset, 2, 0]
}

fn drive_pump(journal: &mut EditJournal, pump: Cell) {
    let source = Node {
        position: [pump[0] - 1, pump[1], pump[2]],
        material: 146,
        facing: 0,
        secondary: false,
        source_rpm: 30.,
        source_capacity: 12800,
        links: [None; 2],
        belt_load: false,
    };
    let driven = Node {
        position: pump,
        material: PUMP,
        source_rpm: 0.,
        source_capacity: 0,
        ..source
    };
    let nodes =
        std::collections::BTreeMap::from([(source.position, source), (driven.position, driven)]);
    journal.replace_kinetic_states(
        kinetic::solve_component(pump, &mut 64, |cell| nodes.get(&cell).copied())
            .into_iter()
            .map(|(node, state)| (node.position, state))
            .collect(),
    );
    assert!(journal.kinetic_state(pump).running());
}

fn bottom_inlet(chunks: &mut ChunkStore, journal: &mut EditJournal, y: i64) -> (Cell, BasinShape) {
    let pump = [0, y, 0];
    for cell in [[0, y + 1, -1], [0, y + 2, -1]] {
        put(chunks, journal, cell, BASIN, 0, 8);
    }
    put(chunks, journal, [0, y, -1], IRON_PIPE, 0, 0);
    put(chunks, journal, pump, PUMP, 0, 0);
    let target = BasinShape {
        origin: [0, y, 1],
        size: [1, 2, 1],
    };
    for cell in target.cells() {
        put(chunks, journal, cell, BASIN, 0, 0);
    }
    drive_pump(journal, pump);
    (pump, target)
}

fn basin_amount(journal: &EditJournal, shape: BasinShape) -> u16 {
    shape
        .cells()
        .map(|cell| u16::from(journal.plumbing_at(cell).unwrap().water.amount))
        .sum()
}
fn total(journal: &EditJournal) -> u32 {
    journal
        .plumbing()
        .map(|(_, state)| u32::from(state.water.amount))
        .sum::<u32>()
        + journal
            .fluid_cells()
            .map(|(_, state)| u32::from(state.amount))
            .sum::<u32>()
}
fn step(
    runtime: &mut Runtime,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &crate::TerraGenerator,
) {
    advance(runtime, chunks, journal, terrain, 0.2).unwrap();
}

#[test]
fn finite_cross_section_gravity_supply_draws_bucket_after_real_outflow_without_creating_water() {
    let (mut chunks, mut journal, terrain) = fixture();
    let tap = finite_line(&mut chunks, &mut journal);
    let mut runtime = Runtime::default();
    let initial = total(&journal);
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(total(&journal), initial);
    assert!(
        journal
            .fluid([2, 1, 0])
            .is_some_and(|water| water.amount > 0 && !water.source)
    );
    assert!(tap_available(&chunks, &journal, tap, 8));
    assert_eq!(visual_data(&chunks, &journal, tap).unwrap().amount, 8);
    assert!(take_tap(&chunks, &mut journal, tap, 8).unwrap());
    assert_eq!(total(&journal) + 8, initial);
    assert!(
        journal
            .plumbing()
            .all(|(cell, state)| state.valid(journal.block(cell).unwrap()))
    );
}

#[test]
fn shut_valve_unload_and_removal_invalidate_cached_pressure_immediately_without_losing_water() {
    let (mut chunks, mut journal, terrain) = fixture();
    let tap = finite_line(&mut chunks, &mut journal);
    let mut runtime = Runtime::default();
    for _ in 0..3 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(tap_available(&chunks, &journal, tap, 1));
    let before = total(&journal);
    let valve = [1, 2, 0];
    let mut state = journal.object_state(valve).unwrap();
    state.facing |= 8;
    let affected = journal.set_object_state(valve, Some(state)).unwrap();
    chunks.invalidate_sections(affected);
    assert!(!take_tap(&chunks, &mut journal, tap, 1).unwrap());
    assert_eq!(total(&journal), before);
    state.facing &= !8;
    journal.set_object_state(valve, Some(state)).unwrap();
    for _ in 0..3 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(tap_available(&chunks, &journal, tap, 1));
    let section = global_to_section([-1, 3, 0]).0;
    let loaded = chunks.loaded.remove(&section).unwrap();
    let before = total(&journal);
    assert!(!take_tap(&chunks, &mut journal, tap, 1).unwrap());
    for _ in 0..3 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert_eq!(total(&journal), before);
    chunks.loaded.insert(section, loaded);
    for _ in 0..3 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(tap_available(&chunks, &journal, tap, 1));
    put(&mut chunks, &mut journal, [0, 2, 0], 0, 0, 0);
    assert!(!take_tap(&chunks, &mut journal, tap, 1).unwrap());
    assert!(journal.plumbing_at([0, 2, 0]).is_none());
}

#[test]
fn an_unloaded_route_clears_confirmed_pipe_transit_without_losing_stored_or_external_water() {
    for material in [IRON_PIPE, GLASS_PIPE] {
        let (mut chunks, mut journal, terrain) = fixture();
        let tap = finite_line(&mut chunks, &mut journal);
        let pipe = [0, 2, 0];
        put(&mut chunks, &mut journal, pipe, material, 0, 0);
        // The independent waterlogged exterior is not the network's storage.
        let exterior = FluidCell {
            amount: 8,
            source: true,
            flow: [0; 2],
        };
        journal.set_fluid(pipe, exterior).unwrap();
        let mut runtime = Runtime::default();
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        let flowing = journal.plumbing_at(pipe).unwrap();
        assert_ne!(
            flowing.water.flow, [0; 2],
            "the complete line must first confirm a real transfer"
        );
        assert!(
            flowing.intake.is_none(),
            "transit alone must also clear when the route unloads"
        );
        assert!(tap_available(&chunks, &journal, tap, 1));
        let stored: Vec<_> = journal
            .plumbing()
            .map(|(p, state)| (p, state.water.amount, state.water.source))
            .collect();
        let external: std::collections::BTreeMap<_, _> = journal.fluid_cells().collect();
        let before = total(&journal);
        let section = global_to_section([-1, 3, 0]).0;
        let loaded = chunks.loaded.remove(&section).unwrap();
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert!(!tap_available(&chunks, &journal, tap, 1));
        for (p, amount, source) in &stored {
            let state = journal.plumbing_at(*p).unwrap();
            assert_eq!((state.water.amount, state.water.source), (*amount, *source));
            if chunks.block_at(*p).is_some() {
                assert_eq!(
                    state.water.flow, [0; 2],
                    "loaded node {p:?} retained stale transit"
                );
                assert!(state.intake.is_none());
            }
        }
        assert_eq!(
            journal
                .fluid_cells()
                .collect::<std::collections::BTreeMap<_, _>>(),
            external
        );
        assert_eq!(journal.fluid(pipe), Some(exterior));
        assert_eq!(total(&journal), before);
        let revision = journal.gameplay().revision;
        for _ in 0..4 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert_eq!(
            journal.gameplay().revision,
            revision,
            "an already paused route must not keep publishing identical activity clears"
        );
        assert_eq!(total(&journal), before);
        chunks.loaded.insert(section, loaded);
        for _ in 0..3 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert!(tap_available(&chunks, &journal, tap, 1));
        assert_eq!(total(&journal), before);
        assert_eq!(journal.fluid(pipe), Some(exterior));
    }
}

#[test]
fn dry_closed_and_unpowered_pump_never_supply_a_faucet() {
    let (mut chunks, mut journal, terrain) = fixture();
    let mut runtime = Runtime::default();
    for (cell, material, facing) in [
        ([0, 2, 0], IRON_PIPE, 0),
        ([1, 2, 0], VALVE, 0),
        ([2, 2, 0], FAUCET, 1),
    ] {
        put(&mut chunks, &mut journal, cell, material, facing, 0);
    }
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(!tap_available(&chunks, &journal, [2, 2, 0], 1));
    assert_eq!(total(&journal), 0);
    put(&mut chunks, &mut journal, [0, 2, 0], PUMP, 1, 8);
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(!tap_available(&chunks, &journal, [2, 2, 0], 1));
    assert_eq!(total(&journal), 8);
}

#[test]
fn actual_shaft_solver_drives_a_directional_pump_and_stopping_it_denies_buffer_draws() {
    let (mut chunks, mut journal, terrain) = fixture();
    let mut runtime = Runtime::default();
    let pump = [0, 3, 0];
    put(&mut chunks, &mut journal, pump, PUMP, 0, 8);
    put(&mut chunks, &mut journal, [0, 3, 1], FAUCET, 0, 0);
    // The pump is a real 12-load kinetic member, driven axially along X.
    let source = Node {
        position: [-1, 3, 0],
        material: 146,
        facing: 0,
        secondary: false,
        source_rpm: 30.,
        source_capacity: 12800,
        links: [None; 2],
        belt_load: false,
    };
    let driven = Node {
        position: pump,
        material: PUMP,
        source_rpm: 0.,
        source_capacity: 0,
        ..source
    };
    let nodes =
        std::collections::BTreeMap::from([(source.position, source), (driven.position, driven)]);
    let solved = kinetic::solve_component(pump, &mut 64, |position| nodes.get(&position).copied());
    assert!(
        solved
            .iter()
            .find(|(node, _)| node.position == pump)
            .unwrap()
            .1
            .running()
    );
    journal.replace_kinetic_states(
        solved
            .into_iter()
            .map(|(node, state)| (node.position, state))
            .collect(),
    );
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(tap_available(&chunks, &journal, [0, 3, 1], 1));
    let powered = journal.kinetic_state(pump);
    journal.replace_kinetic_states(std::collections::BTreeMap::from([(
        pump,
        kinetic::KineticState {
            rpm: -powered.rpm,
            ..powered
        },
    )]));
    assert!(
        !tap_available(&chunks, &journal, [0, 3, 1], 1),
        "direction reversal invalidates the old outlet before the next water tick"
    );
    journal.replace_kinetic_states(std::collections::BTreeMap::from([(pump, powered)]));
    let before = total(&journal);
    journal.replace_kinetic_states(Default::default());
    assert!(!take_tap(&chunks, &mut journal, [0, 3, 1], 1).unwrap());
    assert_eq!(total(&journal), before);
}

#[test]
fn unrelated_unpowered_pump_branch_cannot_disable_the_actual_gravity_route() {
    let (mut chunks, mut journal, terrain) = fixture();
    let tap = finite_line(&mut chunks, &mut journal);
    put(&mut chunks, &mut journal, [0, 2, 1], PUMP, 0, 0);
    let mut runtime = Runtime::default();
    for _ in 0..4 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
    }
    assert!(!journal.kinetic_state([0, 2, 1]).running());
    assert!(take_tap(&chunks, &mut journal, tap, 8).unwrap());
}

#[test]
fn basin_unload_restores_solid_exterior_and_reload_reforms_without_losing_contents() {
    let (mut chunks, mut journal, terrain) = fixture();
    for cell in [[-1, 3, 0], [0, 3, 0]] {
        put(&mut chunks, &mut journal, cell, BASIN, 0, 4);
    }
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    assert_eq!(journal.object_state([-1, 3, 0]).unwrap().facing, 2);
    let before = total(&journal);
    let section = global_to_section([0, 3, 0]).0;
    let loaded = chunks.loaded.remove(&section).unwrap();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    assert_eq!(journal.object_state([-1, 3, 0]).unwrap().facing, 0);
    assert!(journal.plumbing_at([-1, 3, 0]).unwrap().basin.is_none());
    assert_eq!(total(&journal), before);
    chunks.loaded.insert(section, loaded);
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    assert_eq!(journal.object_state([-1, 3, 0]).unwrap().facing, 2);
    assert_eq!(total(&journal), before);
}

#[test]
fn direct_bucket_fill_is_exactly_eight_and_full_or_unloaded_reservoir_rejection_is_atomic() {
    let (mut chunks, mut journal, terrain) = fixture();
    let cell = [-1, 3, -1];
    put(&mut chunks, &mut journal, cell, BASIN, 0, 0);
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let properties = terra_core::block_properties::BlockPropertiesCatalog::default();
    assert_eq!(
        crate::water::buckets::target(
            &chunks,
            &journal,
            &properties,
            true,
            [-0.5, 5.0, -0.5],
            [0., -1., 0.]
        ),
        Some(cell)
    );
    assert!(
        crate::water::buckets::transfer(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            cell,
            true
        )
        .unwrap()
    );
    assert_eq!(total(&journal), 8);
    let before = journal.plumbing_at(cell).unwrap().clone();
    let revision = journal.gameplay().revision;
    assert!(!fill_basin(&chunks, &mut journal, cell, 8).unwrap());
    assert_eq!(journal.plumbing_at(cell), Some(&before));
    assert_eq!(journal.gameplay().revision, revision);
    chunks.loaded.remove(&global_to_section(cell).0);
    assert!(!fill_basin(&chunks, &mut journal, cell, 1).unwrap());
    assert_eq!(total(&journal), 8);
}

#[test]
fn driven_bottom_inlet_fills_both_reservoir_layers_and_full_capacity_keeps_real_water_upstream() {
    let (mut chunks, mut journal, terrain) = fixture();
    let (pump, target) = bottom_inlet(&mut chunks, &mut journal, 3);
    let mut runtime = Runtime::default();
    let initial = total(&journal);
    assert_eq!(initial, 16);
    let source = BasinShape {
        origin: [0, 4, -1],
        size: [1, 2, 1],
    };
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    assert!(
        u32::from(basin_amount(&journal, source)) < initial,
        "the intake must not immediately return its water to the equal-head source"
    );
    let mut previous = basin_amount(&journal, target);
    assert!(previous > 0, "the first powered tick must reach the target");
    assert_eq!(total(&journal), initial);
    for _ in 1..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        let amount = basin_amount(&journal, target);
        assert!(amount >= previous);
        previous = amount;
        assert_eq!(total(&journal), initial);
        assert!(journal.kinetic_state(pump).running());
    }
    assert_eq!(basin_amount(&journal, target), 16);
    assert_eq!(
        journal.plumbing_at([0, 4, 1]).unwrap().water.amount,
        8,
        "bottom inlet must fill the upper layer too"
    );
    let mut state = journal.plumbing_at(pump).unwrap().clone();
    state.water.amount = 8;
    journal.set_plumbing(pump, state).unwrap();
    let before = total(&journal);
    for _ in 0..5 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), before);
    }
    assert_eq!(basin_amount(&journal, target), 16);
    assert_eq!(journal.plumbing_at(pump).unwrap().water.amount, 8);
}

#[test]
fn bottom_inlet_rejects_an_unformed_hole_and_pauses_across_an_unloaded_vertical_seam() {
    let (mut chunks, mut journal, terrain) = fixture();
    let (pump, _) = bottom_inlet(&mut chunks, &mut journal, 3);
    put(&mut chunks, &mut journal, [0, 4, 1], 0, 0, 0);
    for cell in [[1, 3, 1], [0, 3, 2]] {
        put(&mut chunks, &mut journal, cell, BASIN, 0, 0);
    }
    let mut runtime = Runtime::default();
    let before = total(&journal);
    for _ in 0..20 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), before);
    }
    for cell in [[0, 3, 1], [1, 3, 1], [0, 3, 2]] {
        let state = journal.plumbing_at(cell).unwrap();
        assert!(state.basin.is_none());
        assert_eq!(state.water.amount, 0);
    }
    assert!(journal.kinetic_state(pump).running());

    let (mut chunks, mut journal, terrain) = fixture();
    let (_, target) = bottom_inlet(&mut chunks, &mut journal, 15);
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let section = global_to_section([0, 16, 1]).0;
    let loaded = chunks.loaded.remove(&section).unwrap();
    let before = total(&journal);
    let amount = basin_amount(&journal, target);
    for _ in 0..5 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), before);
        assert_eq!(basin_amount(&journal, target), amount);
    }
    chunks.loaded.insert(section, loaded);
    for _ in 0..40 {
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(total(&journal), before);
    }
    assert_eq!(basin_amount(&journal, target), 16);
}

#[test]
fn faucet_uses_group_capacity_when_its_local_basin_module_is_full_and_rejects_full_or_unloaded_group()
 {
    for unload in [false, true] {
        let (mut chunks, mut journal, terrain) = fixture();
        let tap = finite_line_at(&mut chunks, &mut journal, 13);
        assert!(toggle(&mut chunks, &mut journal, tap, FAUCET, 1).unwrap());
        let target = BasinShape {
            origin: [15, 1, 0],
            size: [2, 1, 1],
        };
        for (cell, amount) in target.cells().zip([8, 7]) {
            put(&mut chunks, &mut journal, cell, BASIN, 0, amount);
        }
        let mut runtime = Runtime::default();
        for _ in 0..2 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert_eq!(
            journal.plumbing_at(target.origin).unwrap().basin,
            Some(target)
        );
        assert_eq!(journal.plumbing_at(target.origin).unwrap().water.amount, 8);
        assert_eq!(basin_amount(&journal, target), 15);
        if unload {
            chunks.loaded.remove(&global_to_section([16, 1, 0]).0);
        }
        assert!(toggle(&mut chunks, &mut journal, tap, FAUCET, 5).unwrap());
        let before = total(&journal);
        for _ in 0..5 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
            assert_eq!(total(&journal), before);
        }
        assert_eq!(basin_amount(&journal, target), if unload { 15 } else { 16 });
        assert_eq!(total(&journal), 79);
        assert!(
            journal.fluid_cells().all(|(_, water)| water.amount == 0),
            "tank admission must not spill or mint a separate world-fluid cell"
        );
    }
}

#[test]
fn faucet_does_not_debit_water_into_solid_or_clamped_away_slab_liquid() {
    for facing in [0, terra_core::construction::DOUBLE_SLAB] {
        let (mut chunks, mut journal, terrain) = fixture();
        let tap = finite_line(&mut chunks, &mut journal);
        let below = [tap[0], tap[1] - 1, tap[2]];
        put(&mut chunks, &mut journal, below, 85, 0, 0);
        journal
            .set_object_state(
                below,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut runtime = Runtime::default();
        let before = total(&journal);
        for _ in 0..4 {
            step(&mut runtime, &mut chunks, &mut journal, &terrain);
        }
        assert!(tap_available(&chunks, &journal, tap, 1));
        assert_eq!(total(&journal), before);
        assert!(journal.fluid(below).is_none_or(|water| water.amount == 0));
        assert_eq!(
            chunks.fluid_cell_at(&terrain, &journal, below, 85).amount,
            0
        );
    }
}

#[test]
fn basin_validation_handles_largest_rectangle_holes_negative_borders_and_stacked_interior_masks() {
    let (mut chunks, mut journal, terrain) = fixture();
    let shape = BasinShape {
        origin: [-17, 0, -17],
        size: [5, 10, 5],
    };
    for cell in shape.cells() {
        put(&mut chunks, &mut journal, cell, BASIN, 0, 0);
    }
    assert_eq!(basin_shape(&chunks, &journal, shape.origin), Some(shape));
    let mut runtime = Runtime::default();
    step(&mut runtime, &mut chunks, &mut journal, &terrain);
    let inside = [-16, 4, -16];
    assert_eq!(journal.object_state(inside).unwrap().facing, 63);
    assert!(
        plumbing_boxes(
            &journal,
            inside,
            BASIN,
            journal.object_state(inside).unwrap(),
            |cell| chunks.block_at(cell)
        )
        .is_empty()
    );
    put(&mut chunks, &mut journal, inside, 0, 0, 0);
    assert_eq!(basin_shape(&chunks, &journal, shape.origin), None);
}

#[test]
fn discovery_has_raw_and_component_bounds_and_reaches_late_loaded_nodes() {
    let (mut chunks, mut journal, terrain) = fixture();
    let mut runtime = Runtime::default();
    // Most candidates are unloaded and must not monopolize the sparse cursor.
    for x in 0..130 {
        let cell = [-2000 + x * 16, 2, 0];
        journal.record(cell, WOOD_PIPE).unwrap();
        journal
            .set_object_state(cell, Some(Default::default()))
            .unwrap();
        journal.set_plumbing(cell, Default::default()).unwrap();
    }
    let tap = finite_line(&mut chunks, &mut journal);
    let mut supplied = false;
    for _ in 0..8 {
        let work = advance(&mut runtime, &mut chunks, &mut journal, &terrain, 0.2).unwrap();
        assert!(work.scanned <= 64 && work.visited <= MAX_COMPONENT);
        supplied |= tap_available(&chunks, &journal, tap, 1);
    }
    assert!(supplied);
}

#[test]
fn pipe_collision_keeps_an_aligned_open_end_and_valve_preserves_open_flag_during_rotation() {
    let (mut chunks, mut journal, _) = fixture();
    let pipe = [-1, 2, -1];
    put(&mut chunks, &mut journal, pipe, WOOD_PIPE, 0, 0);
    put(&mut chunks, &mut journal, [-1, 3, -1], WOOD_PIPE, 4, 0);
    let state = journal.object_state(pipe).unwrap();
    assert_eq!(
        connection_mask(&journal, pipe, WOOD_PIPE, 0, |cell| chunks.block_at(cell)),
        12 // The upper neighbor aligns a full vertical tube with an open lower end.
    );
    let boxes = plumbing_boxes(&journal, pipe, WOOD_PIPE, state, |cell| {
        chunks.block_at(cell)
    });
    assert_eq!(boxes.len(), 3);
    assert!(boxes.iter().any(|bounds| bounds.minimum[1] == 0.0));
    assert!(boxes.iter().any(|bounds| bounds.maximum[1] == 1.0));
    assert!(boxes.iter().all(|bounds| {
        [0, 2]
            .into_iter()
            .all(|axis| bounds.minimum[axis] >= 0.35 && bounds.maximum[axis] <= 0.65)
    }));
    put(&mut chunks, &mut journal, [3, 2, 0], VALVE, 8, 0);
    let plan = crate::wrench::propose(&chunks, &journal, [3, 2, 0], false).unwrap();
    assert_eq!(plan.states[0].1.unwrap().facing, 9);
}

#[test]
fn realm_border_splits_a_live_basin_and_stale_shape_cannot_transfer_foreign_water() {
    use terra_core::{
        PlayerUuid,
        kingdom::{DiplomaticRelation, DiplomaticTarget},
    };
    let (mut chunks, mut journal, terrain) = fixture();
    let mut runtime = Runtime::default();
    let left = [15, 4, 0];
    let right = [16, 4, 0];
    put(&mut chunks, &mut journal, left, BASIN, 0, 8);
    put(&mut chunks, &mut journal, right, BASIN, 0, 0);
    advance(&mut runtime, &mut chunks, &mut journal, &terrain, 0.2).unwrap();
    assert_eq!(journal.plumbing_at(left).unwrap().water.amount, 4);
    assert_eq!(journal.plumbing_at(right).unwrap().water.amount, 4);
    let a = PlayerUuid::from_u128(1);
    let b = PlayerUuid::from_u128(2);
    let first = journal
        .gameplay_mut()
        .kingdoms
        .found(a, [0, 20, 0], "First basin")
        .unwrap();
    let second = journal
        .gameplay_mut()
        .kingdoms
        .found(b, [16, 20, 0], "Second basin")
        .unwrap();
    assert!(
        !fill_basin(&chunks, &mut journal, left, 1).unwrap(),
        "a cached pre-claim shape must not bypass a new border"
    );
    advance(&mut runtime, &mut chunks, &mut journal, &terrain, 0.2).unwrap();
    for cell in [left, right] {
        assert_eq!(
            journal.plumbing_at(cell).unwrap().basin.unwrap().size,
            [1, 1, 1]
        );
        assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 4);
    }
    assert!(fill_basin(&chunks, &mut journal, left, 4).unwrap());
    assert_eq!(journal.plumbing_at(right).unwrap().water.amount, 4);
    journal
        .gameplay_mut()
        .kingdoms
        .set_relation(
            a,
            first,
            DiplomaticTarget::Player(second),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .set_relation(
            b,
            second,
            DiplomaticTarget::Player(first),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    advance(&mut runtime, &mut chunks, &mut journal, &terrain, 0.2).unwrap();
    for cell in [left, right] {
        assert_eq!(
            journal.plumbing_at(cell).unwrap().basin.unwrap().size,
            [2, 1, 1]
        );
        assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 6);
    }
    journal
        .gameplay_mut()
        .kingdoms
        .set_relation(
            b,
            second,
            DiplomaticTarget::Player(first),
            DiplomaticRelation::Hostile,
        )
        .unwrap();
    assert!(!fill_basin(&chunks, &mut journal, left, 1).unwrap());
    advance(&mut runtime, &mut chunks, &mut journal, &terrain, 0.2).unwrap();
    for cell in [left, right] {
        assert_eq!(
            journal.plumbing_at(cell).unwrap().basin.unwrap().size,
            [1, 1, 1]
        );
        assert_eq!(journal.plumbing_at(cell).unwrap().water.amount, 6);
    }
}
