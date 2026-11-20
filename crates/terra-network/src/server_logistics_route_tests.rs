use super::{
    tests::{fixture as powered_fixture, place},
    *,
};
use terra_core::{Inventory, ItemKind, ItemStack, beds::DyeColor};
use terra_world::edits::BlockObjectState;

fn fixture() -> (ChunkStore, EditJournal, Cell, Cell) {
    let (chunks, mut journal, source, belt) = powered_fixture();
    journal.replace_kinetic_states(Default::default());
    (chunks, journal, source, belt)
}

#[test]
fn saved_crank_side_ports_export_machine_output_and_reverse_into_real_inputs() {
    for material in [99, 156] {
        for legacy in [false, true] {
            if material == 156 && legacy {
                continue;
            }
            let (mut chunks, mut journal, _, _) = fixture();
            let anchor = [7, 3, 8];
            place(
                &mut chunks,
                &mut journal,
                anchor,
                material,
                BlockObjectState {
                    peer: legacy.then_some([8, 3, 8]),
                    ..Default::default()
                },
            );
            if legacy {
                place(
                    &mut chunks,
                    &mut journal,
                    [8, 3, 8],
                    material,
                    BlockObjectState {
                        peer: Some(anchor),
                        secondary: true,
                        ..Default::default()
                    },
                );
            }
            let crank = [if legacy { 9 } else { 8 }, 3, 8];
            let port = [crank[0] + 1, 3, 8];
            let belt = [port[0], 2, 8];
            place(
                &mut chunks,
                &mut journal,
                crank,
                100,
                BlockObjectState {
                    peer: Some(anchor),
                    ..Default::default()
                },
            );
            place(
                &mut chunks,
                &mut journal,
                port,
                153,
                BlockObjectState {
                    peer: Some(crank),
                    facing: 4,
                    ..Default::default()
                },
            );
            place(
                &mut chunks,
                &mut journal,
                belt,
                148,
                BlockObjectState::default(),
            );
            let (input, output) = if material == 99 {
                journal.set_crusher(anchor, Default::default()).unwrap();
                let recipe = &terra_core::crusher::recipes()[0];
                (recipe.input, recipe.output)
            } else {
                journal.set_mixer(anchor, Default::default()).unwrap();
                (
                    ItemKind::Dye(DyeColor::ALL[0]),
                    ItemStack {
                        item: ItemKind::Dye(DyeColor::Rgb([17, 95, 213])),
                        count: 2,
                    },
                )
            };
            let output_slot = logistics::source_output(material).unwrap();
            let mut inventory = Inventory::default();
            inventory.slots.resize(output_slot + 1, ItemStack::EMPTY);
            inventory.slots[output_slot] = output;
            journal.set_container(anchor, inventory).unwrap();
            let mut runtime = Runtime::default();
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert_eq!(journal.gameplay().drops.len(), 1);
            assert_eq!(
                journal.gameplay().drops[0].stack,
                ItemStack { count: 1, ..output }
            );
            assert_eq!(
                journal.container_at(anchor).unwrap().slot(output_slot),
                (output.count > 1).then_some(ItemStack {
                    count: output.count - 1,
                    ..output
                })
            );
            let drop = &mut journal.gameplay_mut().drops[0];
            drop.stack = ItemStack {
                item: input,
                count: 3,
            };
            journal
                .set_object_state(
                    port,
                    Some(BlockObjectState {
                        peer: Some(crank),
                        ..Default::default()
                    }),
                )
                .unwrap();
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert_eq!(
                journal.gameplay().drops[0].stack,
                ItemStack {
                    item: input,
                    count: 2
                }
            );
            assert_eq!(
                journal.container_at(anchor).unwrap().slot(0),
                Some(ItemStack {
                    item: input,
                    count: 1
                })
            );
            assert!(journal.gameplay().valid());
            let unrelated = [crank[0], crank[1] + 1, crank[2]];
            assert!(
                terra_world::placement::logistics_inventory_at(&chunks, &journal, unrelated, crank)
                    .is_none()
            );
        }
    }
}

#[test]
fn free_tunnel_inserts_into_inventory_in_front_at_three_heights_and_reverses() {
    for material in [29, 99, 156] {
        for dy in [-1, 0, 1] {
            let (mut chunks, mut journal, _, _) = fixture();
            let port = [8, 4, 8];
            let target = [8, 4 + dy, 9];
            let belt = [8, 3, 8];
            for (cell, id) in [(port, 152), (belt, 148), (target, material)] {
                place(
                    &mut chunks,
                    &mut journal,
                    cell,
                    id,
                    BlockObjectState::default(),
                );
            }
            if material == 99 {
                journal.set_crusher(target, Default::default()).unwrap();
            }
            if material == 156 {
                journal.set_mixer(target, Default::default()).unwrap();
            }
            let item = match material {
                99 => terra_core::crusher::recipes()[0].input,
                156 => ItemKind::Dye(DyeColor::ALL[0]),
                _ => ItemKind::Block(3),
            };
            let center = terra_world::conveyors::segment_at(&journal, belt)
                .unwrap()
                .drop_center()
                .unwrap();
            assert!(journal.gameplay_mut().drop_stack(
                center,
                ItemStack { item, count: 3 },
                [0.0; 3],
                0.0
            ));
            let mut runtime = Runtime::default();
            assert_eq!(
                inventory_target(port, 152, BlockObjectState::default(), &chunks, &journal),
                Some((target, false))
            );
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert_eq!(
                journal.gameplay().drops[0].stack,
                ItemStack { item, count: 2 },
                "material {material}, height {dy}"
            );
            assert_eq!(
                journal.container_at(target).unwrap().slot(0),
                Some(ItemStack { item, count: 1 })
            );
            journal
                .set_object_state(
                    port,
                    Some(BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            assert_eq!(
                inventory_target(
                    port,
                    152,
                    journal.object_state(port).unwrap(),
                    &chunks,
                    &journal
                ),
                Some((target, true))
            );
            assert!(journal.gameplay().valid());
        }
    }
}

#[test]
fn funnel_extracts_real_crusher_output_without_powered_belt_and_stops_for_obstacles_or_unload() {
    for blocked in [false, true] {
        let (mut chunks, mut journal, _, _) = fixture();
        let crusher = [8, 3, 7];
        let port = [8, 3, 8];
        place(
            &mut chunks,
            &mut journal,
            crusher,
            99,
            BlockObjectState::default(),
        );
        journal.set_crusher(crusher, Default::default()).unwrap();
        place(
            &mut chunks,
            &mut journal,
            port,
            153,
            BlockObjectState {
                peer: Some(crusher),
                facing: 4,
                ..Default::default()
            },
        );
        let recipe = &terra_core::crusher::recipes()[0];
        let mut inventory = Inventory::default();
        inventory
            .slots
            .resize(terra_core::crusher::OUTPUT + 1, ItemStack::EMPTY);
        inventory.slots[terra_core::crusher::OUTPUT] = recipe.output;
        journal.set_container(crusher, inventory.clone()).unwrap();
        if blocked {
            let cell = [8, 2, 8];
            let (section, local) = terra_voxel::global_to_section(cell);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
            journal.record(cell, 3).unwrap();
        }
        advance(
            &mut Runtime::default(),
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        if blocked {
            assert!(journal.gameplay().drops.is_empty());
            assert_eq!(journal.container_at(crusher), Some(&inventory));
        } else {
            let drop = &journal.gameplay().drops[0];
            assert_eq!(drop.position, [8.5, 2.84, 8.5]);
            assert_eq!(
                drop.stack,
                ItemStack {
                    count: 1,
                    ..recipe.output
                }
            );
            assert_eq!(
                journal
                    .container_at(crusher)
                    .unwrap()
                    .slot(terra_core::crusher::OUTPUT),
                (recipe.output.count > 1).then_some(ItemStack {
                    count: recipe.output.count - 1,
                    ..recipe.output
                })
            );
        }
        assert!(journal.gameplay().valid());
    }
    let (mut chunks, mut journal, _, _) = fixture();
    let crusher = [8, 16, 7];
    let port = [8, 16, 8];
    place(
        &mut chunks,
        &mut journal,
        crusher,
        99,
        BlockObjectState::default(),
    );
    journal.set_crusher(crusher, Default::default()).unwrap();
    place(
        &mut chunks,
        &mut journal,
        port,
        153,
        BlockObjectState {
            peer: Some(crusher),
            facing: 4,
            ..Default::default()
        },
    );
    let mut inventory = Inventory::default();
    inventory
        .slots
        .resize(terra_core::crusher::OUTPUT + 1, ItemStack::EMPTY);
    inventory.slots[terra_core::crusher::OUTPUT] = terra_core::crusher::recipes()[0].output;
    journal.set_container(crusher, inventory.clone()).unwrap();
    chunks.loaded.remove(&terra_voxel::SectionPos::default());
    assert!(chunks.block_at([8, 15, 8]).is_none());
    advance(
        &mut Runtime::default(),
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(journal.container_at(crusher), Some(&inventory));
}

#[test]
fn four_off_mouth_drops_never_starve_attached_free_or_legacy_carried_funnel_stacks() {
    for (attached, carried) in [(false, false), (true, false), (false, true)] {
        let (mut chunks, mut journal, _, _) = fixture();
        let port = [8, 3, 8];
        let crusher = [8, 3, 7];
        if attached {
            place(
                &mut chunks,
                &mut journal,
                crusher,
                99,
                BlockObjectState::default(),
            );
            journal.set_crusher(crusher, Default::default()).unwrap();
        }
        place(
            &mut chunks,
            &mut journal,
            port,
            153,
            BlockObjectState {
                peer: attached.then_some(crusher),
                facing: if attached { 4 } else { 0 },
                ..Default::default()
            },
        );
        for _ in 0..4 {
            assert!(journal.gameplay_mut().drop_stack(
                [7.5, 1.5, 7.5],
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 1
                },
                [0.0; 3],
                0.0
            ));
        }
        let bystanders = journal.gameplay().drops.clone();
        let center = if carried {
            let belt = [8, 2, 8];
            place(
                &mut chunks,
                &mut journal,
                belt,
                148,
                BlockObjectState::default(),
            );
            journal.replace_kinetic_states(BTreeMap::from([(
                belt,
                terra_core::kinetic::KineticState {
                    rpm: 30.0,
                    ..Default::default()
                },
            )]));
            terra_world::conveyors::segment_at(&journal, belt)
                .unwrap()
                .drop_center()
                .unwrap()
        } else {
            [8.5, 3.7, 8.5]
        };
        assert!(journal.gameplay_mut().drop_stack(
            center,
            ItemStack {
                item: ItemKind::Block(3),
                count: 13
            },
            [0.0; 3],
            0.0
        ));
        let original = journal.gameplay().drops.last().unwrap().clone();
        let mut index = BTreeMap::<Cell, Vec<(u64, [f64; 3])>>::new();
        for drop in &journal.gameplay().drops {
            index
                .entry(drop.position.map(|v| v.floor() as i64))
                .or_default()
                .push((drop.id, drop.position));
        }
        assert_eq!(
            &nearby_drops(port, &index)[..4],
            &bystanders.iter().map(|drop| drop.id).collect::<Vec<_>>()
        );
        advance(
            &mut Runtime::default(),
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(&journal.gameplay().drops[..4], &bystanders);
        assert_eq!(journal.gameplay().drops.len(), 5);
        let drop = journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == original.id)
            .unwrap();
        assert_eq!(drop.stack, original.stack);
        assert_eq!(
            drop.position,
            if carried {
                [8.5, center[1], 9.3]
            } else {
                [8.5, 2.84, 8.5]
            }
        );
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn attached_extraction_funnel_passes_loose_stacks_before_empty_or_full_crusher_output() {
    for full_output in [false, true] {
        let (mut chunks, mut journal, _, _) = fixture();
        let crusher = [8, 3, 7];
        let port = [8, 3, 8];
        place(
            &mut chunks,
            &mut journal,
            crusher,
            99,
            BlockObjectState::default(),
        );
        journal.set_crusher(crusher, Default::default()).unwrap();
        place(
            &mut chunks,
            &mut journal,
            port,
            153,
            BlockObjectState {
                peer: Some(crusher),
                facing: 4,
                ..Default::default()
            },
        );
        if full_output {
            let output = terra_core::crusher::recipes()[0].output;
            journal
                .set_container(
                    crusher,
                    Inventory {
                        slots: vec![
                            ItemStack::EMPTY,
                            ItemStack {
                                count: output.item.max_stack(),
                                ..output
                            },
                        ],
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let inventory = journal.container_at(crusher).cloned();
        assert!(journal.gameplay_mut().drop_stack(
            [8.5, 3.7, 8.5],
            ItemStack {
                item: ItemKind::Block(3),
                count: 13
            },
            [0.0, -1.0, 0.0],
            0.0
        ));
        let original = journal.gameplay().drops[0].clone();
        let mut runtime = Runtime::default();
        advance(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(journal.gameplay().drops.len(), 1);
        let drop = &journal.gameplay().drops[0];
        assert_eq!(drop.id, original.id);
        assert_eq!(drop.stack, original.stack);
        assert_eq!(drop.position, [8.5, 2.84, 8.5]);
        assert_eq!(
            journal.container_at(crusher),
            inventory.as_ref(),
            "Only one transfer per port turn"
        );
        advance(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(
            journal.gameplay().drops.len(),
            1,
            "Occupied outlet provides backpressure"
        );
        assert_eq!(journal.gameplay().drops[0].id, original.id);
        assert_eq!(journal.container_at(crusher), inventory.as_ref());
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn attached_insertion_funnel_keeps_inventory_routing_and_preserves_rejected_loose_stacks() {
    for compatible in [false, true] {
        let (mut chunks, mut journal, _, _) = fixture();
        let crusher = [8, 3, 7];
        let port = [8, 3, 8];
        place(
            &mut chunks,
            &mut journal,
            crusher,
            99,
            BlockObjectState::default(),
        );
        journal.set_crusher(crusher, Default::default()).unwrap();
        place(
            &mut chunks,
            &mut journal,
            port,
            153,
            BlockObjectState {
                peer: Some(crusher),
                ..Default::default()
            },
        );
        let item = if compatible {
            terra_core::crusher::recipes()[0].input
        } else {
            let dirt = ItemKind::Block(2);
            assert!(terra_core::crusher::recipe_for(dirt).is_none());
            dirt
        };
        assert!(journal.gameplay_mut().drop_stack(
            [8.5, 3.7, 8.5],
            ItemStack { item, count: 3 },
            [0.0, -1.0, 0.0],
            0.0
        ));
        let original = journal.gameplay().drops[0].clone();
        advance(
            &mut Runtime::default(),
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        if compatible {
            assert_eq!(
                journal.gameplay().drops[0].stack,
                ItemStack {
                    count: 2,
                    ..original.stack
                }
            );
            assert_eq!(
                journal.container_at(crusher).unwrap().slot(0),
                Some(ItemStack {
                    count: 1,
                    ..original.stack
                })
            );
        } else {
            assert_eq!(journal.gameplay().drops, [original]);
            assert!(journal.container_at(crusher).unwrap().slot(0).is_none());
        }
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn loose_funnel_mouth_exits_underneath_preserving_id_count_and_backpressure() {
    for blocked in [false, true] {
        let (mut chunks, mut journal, _, _) = fixture();
        let port = [8, 3, 8];
        place(
            &mut chunks,
            &mut journal,
            port,
            153,
            BlockObjectState::default(),
        );
        assert!(journal.gameplay_mut().drop_stack(
            [8.5, 3.7, 8.5],
            ItemStack {
                item: ItemKind::Block(3),
                count: 13
            },
            [0.0, -1.0, 0.0],
            0.0
        ));
        let original = journal.gameplay().drops[0].clone();
        if blocked {
            let p = [8, 2, 8];
            let (s, l) = terra_voxel::global_to_section(p);
            chunks
                .loaded
                .get_mut(&s)
                .unwrap()
                .data
                .set_block(l, 3)
                .unwrap();
            journal.record(p, 3).unwrap();
        }
        advance(
            &mut Runtime::default(),
            &chunks,
            &mut journal,
            &Default::default(),
            0.5,
        );
        assert_eq!(journal.gameplay().drops.len(), 1);
        let drop = &journal.gameplay().drops[0];
        assert_eq!(drop.id, original.id);
        assert_eq!(drop.stack, original.stack);
        assert_eq!(
            drop.position,
            if blocked {
                original.position
            } else {
                [8.5, 2.84, 8.5]
            }
        );
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn actual_cranked_crusher_funnel_owned_belt_tunnel_chest_roundtrip_conserves_every_item() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let crusher = [2, 2, 2];
    let crank = [0, 2, 2];
    let start = [2, 1, 2];
    let end = [2, 1, 9];
    let chest = [2, 2, 10];
    for (p, material) in [
        (crusher, 99),
        ([1, 2, 2], 141),
        ([1, 1, 2], 141),
        (start, 144),
        (end, 144),
        (chest, 29),
    ] {
        place(
            &mut chunks,
            &mut journal,
            p,
            material,
            BlockObjectState::default(),
        );
    }
    place(
        &mut chunks,
        &mut journal,
        crank,
        100,
        BlockObjectState {
            peer: Some([1, 2, 2]),
            facing: 2,
            ..Default::default()
        },
    );
    place(
        &mut chunks,
        &mut journal,
        [2, 2, 3],
        153,
        BlockObjectState {
            peer: Some(crusher),
            facing: 4,
            ..Default::default()
        },
    );
    place(
        &mut chunks,
        &mut journal,
        [2, 2, 9],
        152,
        BlockObjectState::default(),
    );
    let layout =
        terra_world::conveyors::layout(&chunks, &journal, start, end, ItemKind::Block(148))
            .unwrap();
    terra_world::conveyors::commit(&mut chunks, &mut journal, &layout).unwrap();
    journal.set_crusher(crusher, Default::default()).unwrap();
    let recipe = &terra_core::crusher::recipes()[0];
    journal
        .set_container(
            crusher,
            Inventory {
                slots: vec![ItemStack {
                    item: recipe.input,
                    count: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
    let mut kinetic_runtime = super::super::kinetics::Runtime::default();
    let mut runtime = Runtime::default();
    let properties = Default::default();
    let active = std::collections::HashSet::from([crank]);
    for _ in 0..240 {
        super::super::kinetics::advance(
            &mut kinetic_runtime,
            &terrain,
            &chunks,
            &mut journal,
            &active,
        );
        super::super::crushers::process(crusher, 0.1, &chunks, &mut journal).unwrap();
        super::super::kinetics::transport(
            &mut kinetic_runtime,
            &chunks,
            &mut journal,
            &properties,
            0.1,
        );
        advance(&mut runtime, &chunks, &mut journal, &properties, 0.1);
        assert!(journal.gameplay().valid());
    }
    assert!(journal.crusher_driven(crusher));
    assert_eq!(journal.kinetic_state(start).rpm, 30.0);
    assert_eq!(journal.crusher_at(crusher).unwrap().completed, 2);
    assert!(
        journal
            .container_at(crusher)
            .unwrap()
            .slots
            .iter()
            .all(|stack| *stack == ItemStack::EMPTY)
    );
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal.container_at(chest).unwrap().slot(0),
        Some(ItemStack {
            count: recipe.output.count * 2,
            ..recipe.output
        })
    );
}
