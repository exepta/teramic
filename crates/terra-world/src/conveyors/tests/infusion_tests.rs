use super::*;
use crate::{
    plumbing::{self, PlumbingState},
    water::FluidCell,
};
use terra_core::{
    ToolKind, infusion,
    plumbing::{BASIN, IRON_PIPE},
};

const HOLD: Cell = [6, 4, 6];
const NOZZLE: Cell = infusion::infuser_above(HOLD);

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: Cell, material: u16, water: u8) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    journal
        .set_object_state(p, Some(BlockObjectState::default()))
        .unwrap();
    if terra_core::plumbing::is_part(material) {
        journal
            .set_plumbing(
                p,
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

fn setup(mounted: bool, water: bool) -> (ChunkStore, EditJournal, crate::TerraGenerator, Span) {
    setup_direction(mounted, water, 0)
}

fn setup_direction(
    mounted: bool,
    water: bool,
    facing: u8,
) -> (ChunkStore, EditJournal, crate::TerraGenerator, Span) {
    let (mut chunks, mut journal) = fixture();
    let direction = kinetic::forward(facing);
    let axis = kinetic::facing([direction[2], 0, -direction[0]]).unwrap();
    let along = |distance: i64| std::array::from_fn(|i| HOLD[i] + direction[i] * distance);
    let incoming = connect(&mut chunks, &mut journal, along(-4), along(-1), axis);
    let outgoing = connect(&mut chunks, &mut journal, along(1), along(4), axis);
    let powered = [incoming, outgoing]
        .into_iter()
        .flat_map(|span| span.cells().chain([span.start, span.end]))
        .map(|p| {
            (
                p,
                KineticState {
                    rpm: 30.,
                    ..Default::default()
                },
            )
        })
        .collect();
    journal.replace_kinetic_states(powered);
    put(&mut chunks, &mut journal, HOLD, infusion::HOLD_BLOCK, 0);
    journal
        .set_object_state(
            HOLD,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )
        .unwrap();
    if mounted {
        put(
            &mut chunks,
            &mut journal,
            NOZZLE,
            infusion::FLUID_INFUSER,
            0,
        );
        for x in 2..=4 {
            for y in 7..=8 {
                put(
                    &mut chunks,
                    &mut journal,
                    [x, y, 6],
                    BASIN,
                    if water { 8 } else { 0 },
                );
            }
        }
        for x in 4..=5 {
            put(&mut chunks, &mut journal, [x, NOZZLE[1], 6], IRON_PIPE, 0);
        }
    }
    let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
    (chunks, journal, terrain, incoming)
}

fn drop_item(journal: &mut EditJournal, incoming: Span, item: ItemKind, count: u16) -> u64 {
    let p = Segment {
        span: incoming,
        index: incoming.length as i16,
    }
    .drop_center()
    .unwrap();
    journal
        .gameplay_mut()
        .drop_stack(p, ItemStack { item, count }, [0.; 3], 0.);
    let drop = journal.gameplay_mut().drops.last_mut().unwrap();
    drop.rotation = [0., 0., 0., 1.];
    drop.grounded = true;
    drop.id
}

fn water(journal: &EditJournal) -> u16 {
    journal
        .plumbing()
        .map(|(_, s)| u16::from(s.water.amount))
        .sum()
}

fn run(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &crate::TerraGenerator,
    seconds: f32,
) {
    let mut runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..(seconds / 0.05) as usize {
        plumbing::advance(&mut pipes, chunks, journal, terrain, 0.05).unwrap();
        transport(&mut runtime, chunks, journal, &properties, 0.05);
    }
}

#[test]
fn real_gravity_pipe_supply_fills_each_vessel_and_releases_it_without_kinetic_infuser() {
    for (item, expected, cost) in [
        (ItemKind::GlassBottle, ItemKind::WaterBottle, 1),
        (
            ItemKind::Tool(ToolKind::Bucket),
            ItemKind::Tool(ToolKind::WaterBucket),
            8,
        ),
        (
            ItemKind::Tool(ToolKind::RubyWateringCan)
                .with_watering_units(19)
                .unwrap(),
            ItemKind::Tool(ToolKind::RubyWateringCan)
                .with_watering_units(24)
                .unwrap(),
            5,
        ),
    ] {
        let (mut chunks, mut journal, terrain, input) = setup(true, true);
        let id = drop_item(&mut journal, input, item, 1);
        let before = water(&journal);
        run(&mut chunks, &mut journal, &terrain, 7.);
        let drop = journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == id)
            .unwrap();
        assert_eq!(
            drop.stack.item,
            expected,
            "position={:?}, supply={:?}, water={:?}",
            drop.position,
            plumbing::visual_data(&chunks, &journal, NOZZLE),
            journal.plumbing().collect::<Vec<_>>()
        );
        assert!(
            drop.position[2] >= 8.,
            "filled vessel must leave the station: {:?}",
            drop.position
        );
        assert_eq!(water(&journal) + cost, before);
        assert!(!journal.kinetic_state(NOZZLE).running());
        assert!(journal.fluid_cells().all(|(_, water)| water.amount == 0));
        let owner = terra_core::PlayerUuid::from_u128(77);
        assert!(journal.gameplay_mut().pickup(owner, id));
        assert_eq!(
            journal.gameplay().inventories[&owner].slot(0).unwrap().item,
            expected
        );
    }
}

#[test]
fn unsupported_items_full_vessels_and_unmounted_hold_pass_while_empty_supply_holds() {
    for (mounted, wet, item, passes) in [
        (true, false, ItemKind::Block(3), true),
        (true, false, ItemKind::WaterBottle, true),
        (false, false, ItemKind::Tool(ToolKind::Bucket), true),
        (true, false, ItemKind::Tool(ToolKind::Bucket), false),
    ] {
        let (mut chunks, mut journal, terrain, input) = setup(mounted, wet);
        let id = drop_item(&mut journal, input, item, 1);
        run(&mut chunks, &mut journal, &terrain, 7.);
        let drop = journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == id)
            .unwrap();
        assert_eq!(drop.stack.item, item);
        if passes {
            assert!(drop.position[2] >= 8., "{:?}", drop.position);
        } else {
            assert!((drop.position[2] - 6.5).abs() < 0.02);
            assert_eq!(infusion_carrying(drop, &journal), Some(HOLD));
            assert_eq!(water(&journal), 0);
            let before = drop.clone();
            chunks.loaded.clear();
            transport(
                &mut Transport::default(),
                &chunks,
                &mut journal,
                &BlockPropertiesCatalog::default(),
                0.1,
            );
            assert_eq!(journal.gameplay().drops[0], before);
            assert_eq!(infusion_carrying(&before, &journal), Some(HOLD));
        }
    }
}

#[test]
fn stacked_bottles_are_split_one_at_a_time_preserving_count_and_exact_water() {
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    drop_item(&mut journal, input, ItemKind::GlassBottle, 4);
    let before = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 10.);
    assert!(
        journal
            .gameplay()
            .drops
            .iter()
            .all(|drop| drop.stack.item == ItemKind::WaterBottle)
    );
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| drop.stack.count)
            .sum::<u16>(),
        4
    );
    assert_eq!(water(&journal) + 4, before);
}

#[test]
fn stacked_legacy_empty_cans_finish_every_filling_portion_before_leaving_the_hold() {
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let empty = ItemKind::Tool(ToolKind::IronWateringCan);
    let full = empty.with_watering_units(12).unwrap();
    drop_item(&mut journal, input, empty, 2);
    let before = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 10.);
    assert_eq!(journal.gameplay().drops.len(), 2);
    assert!(
        journal
            .gameplay()
            .drops
            .iter()
            .all(|drop| drop.stack.item == full && drop.position[2] >= 7.5)
    );
    assert_eq!(water(&journal) + 24, before);
}

#[test]
fn hold_preserves_straight_entry_and_exit_at_every_yaw() {
    for facing in 0..4 {
        let (mut chunks, mut journal, terrain, input) = setup_direction(true, true, facing);
        drop_item(&mut journal, input, ItemKind::GlassBottle, 1);
        let before = water(&journal);
        run(&mut chunks, &mut journal, &terrain, 7.);
        let drop = &journal.gameplay().drops[0];
        let direction = kinetic::forward(facing);
        let progress = (0..3)
            .map(|i| (drop.position[i] - HOLD[i] as f64 - 0.5) * direction[i] as f64)
            .sum::<f64>();
        assert_eq!(drop.stack.item, ItemKind::WaterBottle);
        assert!(progress >= 1.5, "yaw {facing}: {:?}", drop.position);
        assert_eq!(water(&journal) + 1, before);
    }
}

#[test]
fn blocked_outlet_keeps_the_filled_vessel_and_never_debits_it_twice() {
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    drop_item(&mut journal, input, ItemKind::Tool(ToolKind::Bucket), 1);
    let p = [6, 4, 7];
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(p, 3).unwrap();
    let before = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 7.);
    let drop = &journal.gameplay().drops[0];
    assert_eq!(drop.stack.item, ItemKind::Tool(ToolKind::WaterBucket));
    assert_eq!(infusion_carrying(drop, &journal), Some(HOLD));
    assert_eq!(water(&journal) + 8, before);
    assert_eq!(journal.gameplay().drops.len(), 1);
}

#[test]
fn dense_mixed_queue_preserves_filling_exclusivity_and_splits_stacks_without_teleporting() {
    use terra_core::infusion::HoldPhase;
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    // A stale player yaw must never turn a straight Z conveyor into an X station.
    journal
        .set_object_state(
            HOLD,
            Some(BlockObjectState {
                facing: 1,
                ..Default::default()
            }),
        )
        .unwrap();
    for (item, count, z) in [
        (ItemKind::GlassBottle, 4, 5.5),
        (ItemKind::Tool(ToolKind::Bucket), 1, 4.9),
        (ItemKind::Tool(ToolKind::IronWateringCan), 2, 4.3),
        (ItemKind::Block(3), 1, 3.7),
    ] {
        drop_item(&mut journal, input, item, count);
        journal.gameplay_mut().drops.last_mut().unwrap().position[2] = z;
    }
    let mut transport_runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    let initial_water = water(&journal);
    let mut saw_gate = false;
    let mut saw_filling = false;
    for _ in 0..700 {
        let before: BTreeMap<_, _> = journal
            .gameplay()
            .drops
            .iter()
            .map(|d| (d.id, d.position))
            .collect();
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(
            &mut transport_runtime,
            &chunks,
            &mut journal,
            &properties,
            0.05,
        );
        let drops = &journal.gameplay().drops;
        for drop in drops {
            if let Some(old) = before.get(&drop.id) {
                assert!(
                    (drop.position[2] - old[2]).abs() <= 0.05001,
                    "existing items may only move by swept belt velocity"
                );
            } else {
                assert!(
                    drop.position[2] < 6.0,
                    "split must happen upstream, never under nozzle"
                );
                assert!(
                    before
                        .values()
                        .any(|p| (p[2] - drop.position[2]).abs() < 1e-6),
                    "split preserves the source position"
                );
            }
        }
        let stored: u16 = drops
            .iter()
            .map(|drop| {
                let per = match drop.stack.item {
                    ItemKind::WaterBottle => 1,
                    ItemKind::Tool(ToolKind::WaterBucket) => 8,
                    item => u16::from(item.watering_units()),
                };
                per * drop.stack.count
            })
            .sum();
        assert_eq!(water(&journal) + stored, initial_water);
        assert_eq!(drops.iter().map(|d| d.stack.count).sum::<u16>(), 8);
        if let Some(view) = journal.infusion_view(HOLD) {
            assert!(view.valid());
            assert_eq!(view.facing, 0);
            saw_gate |= view.gates;
            if view.phase == HoldPhase::Filling {
                saw_filling = true;
                let active = drops
                    .iter()
                    .find(|d| Some(d.id) == view.active_drop)
                    .unwrap();
                assert!((active.position[2] - 6.5).abs() <= 0.015);
                assert_eq!(active.stack.count, 1);
                assert!(view.gates);
            }
            if matches!(view.phase, HoldPhase::Filling | HoldPhase::WaitingForFluid) {
                assert!(
                    drops
                        .iter()
                        .filter(|d| (6.0..7.0).contains(&d.position[2]))
                        .count()
                        <= 1
                );
                for drop in drops
                    .iter()
                    .filter(|drop| Some(drop.id) != view.active_drop)
                {
                    assert!(
                        drop.position[2] <= 5.80001 || drop.position[2] >= 7.49999,
                        "non-active item crossed a filling separator: {drop:?}, view={view:?}"
                    );
                }
            }
        }
    }
    assert!(saw_gate && saw_filling);
    let count = |item| {
        journal
            .gameplay()
            .drops
            .iter()
            .filter(|d| d.stack.item == item)
            .map(|d| d.stack.count)
            .sum::<u16>()
    };
    assert_eq!(count(ItemKind::WaterBottle), 4);
    assert_eq!(count(ItemKind::Tool(ToolKind::WaterBucket)), 1);
    assert_eq!(
        count(
            ItemKind::Tool(ToolKind::IronWateringCan)
                .with_watering_units(12)
                .unwrap()
        ),
        2
    );
    assert_eq!(count(ItemKind::Block(3)), 1);
}

#[test]
fn unsupported_stack_crosses_the_station_at_continuous_belt_speed() {
    let (mut chunks, mut journal, terrain, input) = setup(true, false);
    let id = drop_item(&mut journal, input, ItemKind::Block(3), 12);
    let mut runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..80 {
        let before = journal
            .gameplay()
            .drops
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .position;
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        let drop = journal
            .gameplay()
            .drops
            .iter()
            .find(|d| d.id == id)
            .unwrap();
        if (5.65..7.65).contains(&before[2]) {
            assert!((drop.position[2] - before[2] - 0.05).abs() < 1e-5);
        }
        if let Some(view) = journal.infusion_view(HOLD) {
            assert!(!view.gates);
        }
    }
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_eq!(journal.gameplay().drops[0].stack.count, 12);
    assert!(journal.gameplay().drops[0].position[2] > 8.0);
}

#[test]
fn reversing_both_belts_infers_the_opposite_inlet_and_fills_without_manual_rotation() {
    let (mut chunks, mut journal, terrain, _) = setup(true, true);
    let reverse = journal
        .kinetic_states()
        .map(|(p, mut state)| {
            state.rpm = -state.rpm;
            (p, state)
        })
        .collect();
    journal.replace_kinetic_states(reverse);
    journal.gameplay_mut().drop_stack(
        [6.5, 4.755, 7.5],
        ItemStack {
            item: ItemKind::GlassBottle,
            count: 1,
        },
        [0.; 3],
        0.,
    );
    let drop = journal.gameplay_mut().drops.last_mut().unwrap();
    drop.rotation = [0., 0., 0., 1.];
    drop.grounded = true;
    assert_eq!(infusion_geometry(HOLD, &journal).unwrap().facing, 2);
    let before = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 7.);
    assert_eq!(
        journal.gameplay().drops[0].stack.item,
        ItemKind::WaterBottle
    );
    assert!(journal.gameplay().drops[0].position[2] < 5.0);
    assert_eq!(water(&journal) + 1, before);
}

#[test]
fn infuser_placement_snaps_over_the_hold_and_hold_yaw_follows_actual_belt_geometry() {
    let (chunks, journal, _, _) = setup(false, false);
    let hit = terra_voxel::blocks::BlockHit {
        position: HOLD,
        normal: [1, 0, 0],
        distance: 2.0,
    };
    assert_eq!(
        crate::targeting::item_placement_with_state(
            &chunks,
            &journal,
            hit,
            infusion::FLUID_INFUSER
        ),
        Some((NOZZLE, 0))
    );
    let plan = crate::placement::resolve(
        &chunks,
        &journal,
        hit,
        ItemKind::Block(infusion::FLUID_INFUSER),
        ([8.5, 5.5, 6.5], [-1., -0.2, 0.]),
        [std::f32::consts::FRAC_PI_2, 0.],
        false,
    )
    .unwrap();
    assert_eq!(plan.edits[0].position, NOZZLE);
    assert_eq!(plan.states[0].1.unwrap().facing, 0);
    let hold = crate::placement::layout(
        &chunks,
        &journal,
        HOLD,
        infusion::HOLD_BLOCK,
        infusion::HOLD_BLOCK,
        [std::f32::consts::FRAC_PI_2, 0.],
        None,
    )
    .unwrap();
    assert_eq!(hold.states[0].1.unwrap().facing, 0);
    let free_ground = terra_voxel::blocks::BlockHit {
        position: [12, 3, 12],
        normal: [0, 1, 0],
        distance: 2.0,
    };
    assert!(
        crate::targeting::item_placement_with_state(
            &chunks,
            &journal,
            free_ground,
            infusion::FLUID_INFUSER
        )
        .is_none()
    );
}

#[test]
fn reversing_an_active_filling_station_drains_the_new_outlet_queue_before_readmission() {
    use terra_core::infusion::HoldPhase;
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let glass = drop_item(&mut journal, input, ItemKind::GlassBottle, 1);
    drop_item(&mut journal, input, ItemKind::Tool(ToolKind::Bucket), 1);
    journal.gameplay_mut().drops.last_mut().unwrap().position[2] = 4.9;
    drop_item(&mut journal, input, ItemKind::Block(3), 1);
    journal.gameplay_mut().drops.last_mut().unwrap().position[2] = 4.3;
    let before_water = water(&journal);
    let mut runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    let mut filling = false;
    for _ in 0..80 {
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        if journal
            .infusion_view(HOLD)
            .is_some_and(|view| view.active_drop == Some(glass) && view.phase == HoldPhase::Filling)
        {
            filling = true;
            break;
        }
    }
    assert!(filling);
    assert_eq!(water(&journal), before_water);
    let reverse = journal
        .kinetic_states()
        .map(|(p, mut state)| {
            state.rpm = -state.rpm;
            (p, state)
        })
        .collect();
    journal.replace_kinetic_states(reverse);
    for _ in 0..200 {
        let old: BTreeMap<_, _> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.position))
            .collect();
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        if journal.infusion_view(HOLD).is_some_and(|view| {
            matches!(view.phase, HoldPhase::Filling | HoldPhase::WaitingForFluid)
        }) {
            assert!(
                journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| (6.0..7.0).contains(&drop.position[2]))
                    .count()
                    <= 1
            );
        }
        for drop in &journal.gameplay().drops {
            assert!((drop.position[2] - old[&drop.id][2]).abs() <= 0.05001);
        }
    }
    let filled = journal
        .gameplay()
        .drops
        .iter()
        .find(|drop| drop.id == glass)
        .unwrap();
    assert_eq!(filled.stack.item, ItemKind::WaterBottle);
    assert!(
        filled.position[2] < 5.0,
        "reversed queued station stalled: {:?}",
        journal.gameplay().drops
    );
    assert_eq!(water(&journal) + 1, before_water);
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| drop.stack.count)
            .sum::<u16>(),
        3
    );
}

#[test]
fn stopped_active_queue_retains_actual_direction_despite_wrong_saved_yaw_and_recovers_after_reload()
{
    for reload in [false, true] {
        let (mut chunks, mut journal, terrain, input) = setup(true, true);
        journal
            .set_object_state(
                HOLD,
                Some(BlockObjectState {
                    facing: 2,
                    ..Default::default()
                }),
            )
            .unwrap();
        let glass = drop_item(&mut journal, input, ItemKind::GlassBottle, 1);
        drop_item(&mut journal, input, ItemKind::Tool(ToolKind::Bucket), 1);
        journal.gameplay_mut().drops.last_mut().unwrap().position[2] = 4.9;
        let before_water = water(&journal);
        let powered: BTreeMap<_, _> = journal.kinetic_states().collect();
        let mut runtime = Transport::default();
        let mut pipes = plumbing::Runtime::default();
        let properties = BlockPropertiesCatalog::default();
        for _ in 0..40 {
            plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
            if journal
                .gameplay()
                .drops
                .iter()
                .find(|drop| drop.id == glass)
                .unwrap()
                .position[2]
                >= 6.4
            {
                break;
            }
        }
        assert_eq!(
            journal.infusion_view(HOLD).unwrap().active_drop,
            Some(glass)
        );
        assert!(
            journal
                .gameplay()
                .drops
                .iter()
                .any(|drop| drop.stack.item == ItemKind::Tool(ToolKind::Bucket)
                    && drop.position[2] >= 5.75)
        );
        let positions: BTreeMap<_, _> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.position))
            .collect();
        journal.replace_kinetic_states(
            powered
                .iter()
                .map(|(&p, &state)| (p, KineticState { rpm: 0., ..state }))
                .collect(),
        );
        if reload {
            runtime = Transport::default();
            journal.replace_infusion_views([]);
        }
        for _ in 0..20 {
            plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
            if !reload {
                assert_eq!(infusion_geometry(HOLD, &journal).unwrap().facing, 0);
                assert_eq!(
                    journal.infusion_view(HOLD).unwrap().active_drop,
                    Some(glass)
                );
            }
            for drop in &journal.gameplay().drops {
                assert_eq!(drop.position, positions[&drop.id]);
            }
        }
        journal.replace_kinetic_states(powered);
        for _ in 0..200 {
            plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
            transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        }
        assert!(journal.gameplay().drops.iter().any(|drop| drop.id == glass
            && drop.stack.item == ItemKind::WaterBottle
            && drop.position[2] > 7.5));
        assert!(journal.gameplay().drops.iter().any(|drop| drop.stack.item
            == ItemKind::Tool(ToolKind::WaterBucket)
            && drop.position[2] > 7.5));
        assert_eq!(water(&journal) + 9, before_water);
    }
}

#[test]
fn dense_bypass_row_shares_deck_without_slot_or_new_spacing_and_preserves_full_vessels() {
    use terra_core::infusion::HoldPhase;
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let items = [
        ItemKind::Block(3),
        ItemKind::Block(2),
        ItemKind::Block(1),
        ItemKind::WaterBottle,
        ItemKind::Tool(ToolKind::WaterBucket),
        ItemKind::Tool(ToolKind::IronWateringCan)
            .with_watering_units(12)
            .unwrap(),
        ItemKind::Block(3),
    ];
    let mut identities = BTreeMap::new();
    for (i, item) in items.into_iter().enumerate() {
        let id = drop_item(&mut journal, input, item, 1);
        journal.gameplay_mut().drops.last_mut().unwrap().position[2] -= i as f64 * 0.27;
        identities.insert(id, item);
    }
    let initial_water = water(&journal);
    let mut runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    let mut simultaneous = 0;
    for _ in 0..80 {
        let previous: BTreeMap<_, _> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.position))
            .collect();
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        let drops = &journal.gameplay().drops;
        simultaneous = simultaneous.max(
            drops
                .iter()
                .filter(|drop| (6.0..7.0).contains(&drop.position[2]))
                .count(),
        );
        for drop in drops {
            assert_eq!(drop.stack.item, identities[&drop.id]);
            if (5.65..7.65).contains(&previous[&drop.id][2]) {
                assert!(
                    (drop.position[2] - previous[&drop.id][2] - 0.05).abs() < 1e-5,
                    "bypass paused or jumped: {drop:?}"
                );
            }
        }
        for pair in drops.windows(2) {
            // The incoming belt's ordinary work cursor can already vary spacing.
            // The station must preserve the interval it actually receives.
            if previous[&pair[1].id][2] >= 5.2 && previous[&pair[0].id][2] <= 7.65 {
                let incoming_gap = previous[&pair[0].id][2] - previous[&pair[1].id][2];
                assert!(
                    (pair[0].position[2] - pair[1].position[2] - incoming_gap).abs() < 1e-5,
                    "station added spacing: front={:?}, back={:?}, previous={previous:?}, view={:?}",
                    pair[0],
                    pair[1],
                    journal.infusion_view(HOLD)
                );
            }
        }
        if let Some(view) = journal.infusion_view(HOLD) {
            assert_eq!(view.phase, HoldPhase::Idle);
            assert!(view.active_drop.is_none() && !view.gates && view.speed > 0.9);
        }
    }
    assert!(
        simultaneous >= 3,
        "the platform must carry a dense group simultaneously"
    );
    assert_eq!(water(&journal), initial_water);
    assert_eq!(journal.gameplay().drops.len(), items.len());
}

#[test]
fn bypass_predecessors_drain_before_bucket_capture_and_followers_resume_tightly() {
    use terra_core::infusion::HoldPhase;
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let mut ids = Vec::new();
    for (i, item) in [
        ItemKind::Block(3),
        ItemKind::Block(2),
        ItemKind::Block(1),
        ItemKind::Tool(ToolKind::Bucket),
        ItemKind::Block(3),
        ItemKind::WaterBottle,
        ItemKind::Tool(ToolKind::WaterBucket),
    ]
    .into_iter()
    .enumerate()
    {
        ids.push(drop_item(&mut journal, input, item, 1));
        journal.gameplay_mut().drops.last_mut().unwrap().position[2] -= i as f64 * 0.27;
    }
    let mut runtime = Transport::default();
    let mut pipes = plumbing::Runtime::default();
    let properties = BlockPropertiesCatalog::default();
    let initial_water = water(&journal);
    let mut saw_drain = false;
    let mut saw_fill = false;
    let mut simultaneous_bypass = 0;
    for _ in 0..240 {
        let before: BTreeMap<_, _> = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.position))
            .collect();
        plumbing::advance(&mut pipes, &mut chunks, &mut journal, &terrain, 0.05).unwrap();
        transport(&mut runtime, &chunks, &mut journal, &properties, 0.05);
        if let Some(view) = journal.infusion_view(HOLD) {
            let drops = &journal.gameplay().drops;
            if view.phase == HoldPhase::Approaching
                && ids[..3].iter().any(|id| {
                    drops
                        .iter()
                        .any(|drop| drop.id == *id && drop.position[2] < 7.49999)
                })
            {
                saw_drain = true;
                assert!(
                    !view.gates,
                    "do not raise separators into predecessor traffic"
                );
            }
            if view.phase == HoldPhase::Filling {
                saw_fill = true;
                assert_eq!(view.active_drop, Some(ids[3]));
                assert_eq!(
                    drops
                        .iter()
                        .filter(|drop| (6.0..7.0).contains(&drop.position[2]))
                        .count(),
                    1
                );
                assert!(view.gates);
            }
            if view.phase == HoldPhase::Idle {
                simultaneous_bypass = simultaneous_bypass.max(
                    drops
                        .iter()
                        .filter(|drop| (6.0..7.0).contains(&drop.position[2]))
                        .count(),
                );
            }
            for drop in drops {
                assert!((drop.position[2] - before[&drop.id][2]).abs() <= 0.05001);
                if ids[..3].contains(&drop.id) && (5.65..7.65).contains(&before[&drop.id][2]) {
                    assert!(
                        (drop.position[2] - before[&drop.id][2] - 0.05).abs() < 1e-5,
                        "preceding bypass traffic was held by later vessel: old={:?}, new={drop:?}, view={view:?}, drops={drops:?}",
                        before[&drop.id]
                    );
                }
            }
        }
    }
    assert!(saw_drain && saw_fill && simultaneous_bypass >= 2);
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == ids[3])
            .unwrap()
            .stack
            .item,
        ItemKind::Tool(ToolKind::WaterBucket)
    );
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == ids[5])
            .unwrap()
            .stack
            .item,
        ItemKind::WaterBottle
    );
    assert_eq!(water(&journal) + 8, initial_water);
    assert_eq!(journal.gameplay().drops.len(), ids.len());
}

#[test]
fn required_air_gap_blocks_placement_and_filling_without_debit_then_resumes() {
    use terra_core::infusion::HoldPhase;
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let gap = infusion::gap_above(HOLD);
    let (section, local) = global_to_section(gap);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(gap, 3).unwrap();
    let id = drop_item(&mut journal, input, ItemKind::Tool(ToolKind::Bucket), 1);
    let initial = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 4.);
    assert_eq!(
        journal.gameplay().drops[0].stack.item,
        ItemKind::Tool(ToolKind::Bucket)
    );
    assert_eq!(
        journal.infusion_view(HOLD).unwrap().phase,
        HoldPhase::WaitingForFluid
    );
    assert_eq!(water(&journal), initial);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    journal.record(gap, 0).unwrap();
    run(&mut chunks, &mut journal, &terrain, 5.);
    assert!(journal.gameplay().drops.iter().any(|drop| drop.id == id
        && drop.stack.item == ItemKind::Tool(ToolKind::WaterBucket)
        && drop.position[2] > 7.5));
    assert_eq!(water(&journal) + 8, initial);
    let (mut chunks, mut journal, _, _) = setup(false, false);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(gap, 3).unwrap();
    let hit = terra_voxel::blocks::BlockHit {
        position: HOLD,
        normal: [0, 1, 0],
        distance: 2.,
    };
    assert!(
        crate::targeting::item_placement_with_state(
            &chunks,
            &journal,
            hit,
            infusion::FLUID_INFUSER
        )
        .is_none()
    );
    let proposal = crate::placement::layout(
        &chunks,
        &journal,
        NOZZLE,
        0,
        infusion::FLUID_INFUSER,
        [0.; 2],
        None,
    )
    .unwrap();
    assert!(crate::placement::environment(&proposal, &chunks, &journal, false).is_err());
}

#[test]
fn legacy_infuser_one_cell_above_hold_does_not_capture_or_fill_a_vessel() {
    let (mut chunks, mut journal, terrain, input) = setup(true, true);
    let (section, local) = global_to_section(NOZZLE);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    journal.record(NOZZLE, 0).unwrap();
    put(
        &mut chunks,
        &mut journal,
        infusion::gap_above(HOLD),
        infusion::FLUID_INFUSER,
        0,
    );
    drop_item(&mut journal, input, ItemKind::Tool(ToolKind::Bucket), 1);
    let initial = water(&journal);
    run(&mut chunks, &mut journal, &terrain, 5.);
    assert_eq!(
        journal.gameplay().drops[0].stack.item,
        ItemKind::Tool(ToolKind::Bucket)
    );
    assert!(journal.gameplay().drops[0].position[2] > 7.5);
    assert_eq!(water(&journal), initial);
}
