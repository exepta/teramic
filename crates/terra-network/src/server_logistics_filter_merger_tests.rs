use super::{tests::place, *};
use std::collections::BTreeMap;
use terra_core::{Inventory, ItemKind, ItemStack, kinetic, logistics::FilterConfig};
use terra_world::edits::BlockObjectState;

fn step_loose_drops(
    runtime: &mut Runtime,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    steps: usize,
) {
    for _ in 0..steps {
        let mut drops = std::mem::take(&mut journal.gameplay_mut().drops);
        for drop in &mut drops {
            crate::server::gameplay::step_drop_with_context(
                drop, chunks, journal, properties, 0.05,
            );
        }
        journal.gameplay_mut().drops = drops;
        advance(runtime, chunks, journal, properties, 0.05);
    }
}

fn authored_properties() -> terra_core::block_properties::BlockPropertiesCatalog {
    terra_core::block_properties::BlockPropertiesCatalog::load(
        &terra_core::asset_directory().join("terra/blocks"),
    )
    .unwrap()
}

#[test]
fn all_tunnel_variants_randomize_chest_exports_without_bypassing_filters_or_backpressure() {
    for material in [logistics::TUNNEL, logistics::FILTERED_TUNNEL] {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let port = [8, 2, 8];
        let chest = [8, 2, 7];
        let belt = [8, 1, 8];
        for (position, material, state) in [
            (
                port,
                material,
                BlockObjectState {
                    peer: Some(chest),
                    ..Default::default()
                },
            ),
            (chest, 29, BlockObjectState::default()),
            (belt, 148, BlockObjectState::default()),
        ] {
            place(&mut chunks, &mut journal, position, material, state);
        }
        journal.replace_kinetic_states(BTreeMap::from([(
            belt,
            kinetic::KineticState {
                rpm: 30.0,
                ..Default::default()
            },
        )]));
        journal.set_tunnel_random(port, true).unwrap();
        journal
            .set_container(
                chest,
                Inventory {
                    slots: [1, 3, 4]
                        .into_iter()
                        .map(|material| ItemStack {
                            item: ItemKind::Block(material),
                            count: 128,
                        })
                        .collect(),
                    ..Default::default()
                },
            )
            .unwrap();
        if logistics::supports_filter(material) {
            journal
                .set_logistics_filter(
                    port,
                    FilterConfig {
                        item: Some(ItemKind::Block(1)),
                        allowed: false,
                    },
                )
                .unwrap();
        }
        let mut runtime = Runtime::default();
        let mut counts = BTreeMap::new();
        for _ in 0..64 {
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert_eq!(journal.gameplay().drops.len(), 1);
            let drop = &journal.gameplay().drops[0];
            assert_eq!(drop.stack.count, 1);
            if logistics::supports_filter(material) {
                assert_ne!(drop.stack.item, ItemKind::Block(1));
            }
            let ItemKind::Block(id) = drop.stack.item else {
                panic!()
            };
            *counts.entry(id).or_insert(0) += 1;
            let before = journal.container_at(chest).cloned();
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.5,
            );
            assert_eq!(journal.container_at(chest), before.as_ref());
            assert_eq!(journal.gameplay().drops.len(), 1);
            journal.gameplay_mut().drops.clear();
        }
        assert_eq!(
            counts.len(),
            if logistics::supports_filter(material) {
                2
            } else {
                3
            }
        );
        assert!(counts.values().all(|count| *count >= 10));
        let remaining = journal.container_at(chest).unwrap().slot(0).unwrap().count;
        if logistics::supports_filter(material) {
            assert_eq!(remaining, 128);
        } else {
            assert!(
                remaining < 128,
                "ordinary tunnel randomization includes every item"
            );
        }
    }
}

#[test]
fn filtered_funnel_selects_a_later_chest_slot_and_denies_without_consumption() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let port = [8, 4, 8];
    let chest = [8, 4, 7];
    let belt = [8, 3, 8];
    for (p, material, state) in [
        (
            port,
            logistics::FILTERED_FUNNEL,
            BlockObjectState {
                facing: 4,
                peer: Some(chest),
                ..Default::default()
            },
        ),
        (chest, 29, BlockObjectState::default()),
        (belt, 148, BlockObjectState::default()),
    ] {
        place(&mut chunks, &mut journal, p, material, state);
    }
    let first = ItemKind::Block(1);
    let selected = ItemKind::Block(3);
    journal
        .set_container(
            chest,
            Inventory {
                slots: vec![
                    ItemStack {
                        item: first,
                        count: 8,
                    },
                    ItemStack {
                        item: selected,
                        count: 8,
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(selected),
                allowed: true,
            },
        )
        .unwrap();
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
        ItemStack {
            item: selected,
            count: 1
        }
    );
    assert_eq!(
        journal.container_at(chest).unwrap().slot(0).unwrap().count,
        8
    );
    assert_eq!(
        journal.container_at(chest).unwrap().slot(1).unwrap().count,
        7
    );
    journal.gameplay_mut().drops.clear();
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(first),
                allowed: false,
            },
        )
        .unwrap();
    advance(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert_eq!(journal.gameplay().drops[0].stack.item, selected);
    assert_eq!(
        journal.container_at(chest).unwrap().slot(0).unwrap().count,
        8
    );
    assert_eq!(
        journal.container_at(chest).unwrap().slot(1).unwrap().count,
        6
    );
    for remaining in (0..6).rev() {
        journal.gameplay_mut().drops.clear();
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
                item: selected,
                count: 1
            }
        );
        assert_eq!(
            journal
                .container_at(chest)
                .unwrap()
                .slot(1)
                .map_or(0, |stack| stack.count),
            remaining
        );
    }
    assert!(journal.container_at(chest).unwrap().slot(1).is_none());
    journal.gameplay_mut().drops.clear();
    advance(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn filtered_funnel_checks_loose_stacks_and_never_duplicates_the_ghost_item() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let port = [8, 4, 8];
    place(
        &mut chunks,
        &mut journal,
        port,
        logistics::FILTERED_FUNNEL,
        BlockObjectState::default(),
    );
    let item = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184]));
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(item),
                allowed: false,
            },
        )
        .unwrap();
    assert!(journal.gameplay().drops.is_empty());
    assert!(journal.gameplay_mut().drop_stack(
        [8.5, 4.7, 8.5],
        ItemStack { item, count: 11 },
        [0.0; 3],
        0.0
    ));
    journal.gameplay_mut().drops[0].angular_velocity = [0.0; 3];
    journal.gameplay_mut().drops[0].rotation = [0.0, 0.0, 0.0, 1.0];
    let before = journal.gameplay().drops[0].clone();
    let mut runtime = Runtime::default();
    let properties = authored_properties();
    step_loose_drops(&mut runtime, &chunks, &mut journal, &properties, 80);
    let held = &journal.gameplay().drops[0];
    assert_eq!(held.id, before.id);
    assert_eq!(held.stack, before.stack);
    assert_eq!(terra_world::funnels::carrying(held, &journal), Some(port));
    assert_eq!(held.position, [8.5, 4.63, 8.5]);
    assert!(held.grounded);
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(item),
                allowed: true,
            },
        )
        .unwrap();
    step_loose_drops(&mut runtime, &chunks, &mut journal, &properties, 80);
    let after = &journal.gameplay().drops[0];
    assert_eq!(after.id, before.id);
    assert_eq!(after.stack, before.stack);
    assert!(after.position[1] < 4.0);
    assert_eq!(terra_world::funnels::carrying(after, &journal), None);
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(item),
                allowed: false,
            },
        )
        .unwrap();
    assert!(
        journal
            .gameplay_mut()
            .drop_stack([8.5, 4.7, 8.5], before.stack, [0.0; 3], 0.0,)
    );
    let next = journal.gameplay_mut().drops.last_mut().unwrap();
    next.angular_velocity = [0.0; 3];
    next.rotation = [0.0, 0.0, 0.0, 1.0];
    let next_id = next.id;
    step_loose_drops(&mut runtime, &chunks, &mut journal, &properties, 80);
    let drops = &journal.gameplay().drops;
    assert_eq!(drops.len(), 2);
    assert_eq!(drops[0].id, before.id);
    assert!(drops[0].position[1] < 4.0);
    assert_eq!(terra_world::funnels::carrying(&drops[0], &journal), None);
    assert_eq!(drops[1].id, next_id);
    assert_eq!(
        terra_world::funnels::carrying(&drops[1], &journal),
        Some(port)
    );
    assert!(drops.iter().all(|drop| drop.stack == before.stack));
    assert!(journal.gameplay().valid());
}

#[test]
fn funnel_mouth_gravity_respects_full_inventory_wall_and_unloaded_backpressure() {
    let properties = authored_properties();
    let item = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184]));
    for blockage in ["inventory", "wall", "unloaded_outlet", "unloaded_inventory"] {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let port = if blockage == "unloaded_outlet" {
            [15, 0, 8]
        } else {
            [15, 4, 8]
        };
        let chest = [16, port[1], 8];
        let attached = matches!(blockage, "inventory" | "unloaded_inventory");
        place(
            &mut chunks,
            &mut journal,
            port,
            logistics::FILTERED_FUNNEL,
            BlockObjectState {
                peer: attached.then_some(chest),
                ..Default::default()
            },
        );
        journal
            .set_logistics_filter(
                port,
                FilterConfig {
                    item: Some(item),
                    allowed: true,
                },
            )
            .unwrap();
        if attached {
            place(
                &mut chunks,
                &mut journal,
                chest,
                29,
                BlockObjectState::default(),
            );
            journal
                .set_container(
                    chest,
                    if blockage == "inventory" {
                        Inventory {
                            slots: vec![
                                ItemStack {
                                    item: ItemKind::Block(3),
                                    count: ItemKind::Block(3).max_stack(),
                                };
                                terra_core::containers::CHEST_SLOTS
                            ],
                            ..Default::default()
                        }
                    } else {
                        Inventory::default()
                    },
                )
                .unwrap();
            if blockage == "unloaded_inventory" {
                chunks
                    .loaded
                    .remove(&terra_voxel::global_to_section(chest).0);
            }
        }
        if blockage == "wall" {
            let below = [port[0], port[1] - 1, port[2]];
            let (section, local) = terra_voxel::global_to_section(below);
            chunks
                .loaded
                .get_mut(&section)
                .unwrap()
                .data
                .set_block(local, 3)
                .unwrap();
        }
        let inventories = journal.container_at(chest).cloned();
        assert!(journal.gameplay_mut().drop_stack(
            [
                port[0] as f64 + 0.5,
                port[1] as f64 + 0.7,
                port[2] as f64 + 0.5
            ],
            ItemStack { item, count: 11 },
            [0.0; 3],
            0.0,
        ));
        let id = journal.gameplay().drops[0].id;
        journal.gameplay_mut().drops[0].angular_velocity = [0.0; 3];
        journal.gameplay_mut().drops[0].rotation = [0.0, 0.0, 0.0, 1.0];
        let mut runtime = Runtime::default();
        step_loose_drops(&mut runtime, &chunks, &mut journal, &properties, 100);
        assert_eq!(journal.gameplay().drops.len(), 1, "{blockage}");
        let held = &journal.gameplay().drops[0];
        assert_eq!(held.id, id, "{blockage}");
        assert_eq!(held.stack, ItemStack { item, count: 11 }, "{blockage}");
        assert_eq!(
            terra_world::funnels::carrying(held, &journal),
            Some(port),
            "{blockage} {held:?}"
        );
        assert!(held.grounded, "{blockage}");
        assert_eq!(journal.container_at(chest), inventories.as_ref());
        assert!(journal.gameplay().valid());
    }
}

#[test]
fn occupied_stopped_belt_retains_the_next_funnel_stack_instead_of_bypassing_it() {
    let properties = authored_properties();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let port = [8, 4, 8];
    let belt = [8, 3, 8];
    for (position, material) in [(port, logistics::FILTERED_FUNNEL), (belt, 148)] {
        place(
            &mut chunks,
            &mut journal,
            position,
            material,
            BlockObjectState::default(),
        );
    }
    assert!(!journal.kinetic_state(belt).running());
    let item = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184]));
    let mut runtime = Runtime::default();
    for _ in 0..2 {
        assert!(journal.gameplay_mut().drop_stack(
            [8.5, 4.7, 8.5],
            ItemStack { item, count: 11 },
            [0.0; 3],
            0.0,
        ));
        let drop = journal.gameplay_mut().drops.last_mut().unwrap();
        drop.rotation = [0.0, 0.0, 0.0, 1.0];
        drop.angular_velocity = [0.0; 3];
        step_loose_drops(&mut runtime, &chunks, &mut journal, &properties, 40);
    }
    let drops = &journal.gameplay().drops;
    assert_eq!(drops.len(), 2);
    assert_ne!(drops[0].id, drops[1].id);
    assert!(
        drops
            .iter()
            .all(|drop| drop.stack == ItemStack { item, count: 11 })
    );
    assert_eq!(terra_world::funnels::carrying(&drops[0], &journal), None);
    assert_eq!(
        terra_world::conveyors::carrying_segment(&drops[0], &journal)
            .map(|segment| segment.position()),
        Some(belt)
    );
    assert_eq!(
        terra_world::funnels::carrying(&drops[1], &journal),
        Some(port)
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn filtered_funnel_only_exports_accepted_machine_result_not_input() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let crusher = [7, 4, 8];
    let port = [8, 4, 8];
    place(
        &mut chunks,
        &mut journal,
        crusher,
        99,
        BlockObjectState::default(),
    );
    place(
        &mut chunks,
        &mut journal,
        port,
        logistics::FILTERED_FUNNEL,
        BlockObjectState {
            facing: 4,
            peer: Some(crusher),
            ..Default::default()
        },
    );
    journal.set_crusher(crusher, Default::default()).unwrap();
    let recipe = &terra_core::crusher::recipes()[0];
    let inventory = Inventory {
        slots: vec![
            ItemStack {
                item: recipe.input,
                count: 8,
            },
            recipe.output,
        ],
        ..Default::default()
    };
    journal.set_container(crusher, inventory.clone()).unwrap();
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(recipe.output.item),
                allowed: false,
            },
        )
        .unwrap();
    let mut runtime = Runtime::default();
    advance(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(journal.container_at(crusher), Some(&inventory));
    journal
        .set_logistics_filter(
            port,
            FilterConfig {
                item: Some(recipe.output.item),
                allowed: true,
            },
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
            count: 1,
            ..recipe.output
        }
    );
    assert_eq!(
        journal
            .container_at(crusher)
            .unwrap()
            .slot(terra_core::crusher::INPUT),
        inventory.slot(terra_core::crusher::INPUT)
    );
    let remaining = journal
        .container_at(crusher)
        .unwrap()
        .slot(terra_core::crusher::OUTPUT);
    assert_eq!(
        remaining,
        (recipe.output.count > 1).then_some(ItemStack {
            count: recipe.output.count - 1,
            ..recipe.output
        })
    );
    assert_eq!(
        journal.gameplay().drops[0].stack.count + remaining.map_or(0, |stack| stack.count),
        recipe.output.count
    );
}

fn merger_fixture(facing: u8) -> (ChunkStore, EditJournal, Cell, [u64; 3]) {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let port = [8, 5, 8];
    let target = [8, 4, 8];
    place(
        &mut chunks,
        &mut journal,
        port,
        logistics::BELT_MERGER,
        BlockObjectState {
            facing,
            ..Default::default()
        },
    );
    place(
        &mut chunks,
        &mut journal,
        target,
        148,
        BlockObjectState {
            facing,
            ..Default::default()
        },
    );
    let mut drives = BTreeMap::from([(
        target,
        kinetic::KineticState {
            rpm: 30.0,
            ..Default::default()
        },
    )]);
    let forward = kinetic::forward(facing);
    for distance in 1..=3 {
        let cell = std::array::from_fn(|axis| target[axis] + forward[axis] * distance);
        place(
            &mut chunks,
            &mut journal,
            cell,
            148,
            BlockObjectState {
                facing,
                ..Default::default()
            },
        );
        drives.insert(
            cell,
            kinetic::KineticState {
                rpm: 30.0,
                ..Default::default()
            },
        );
    }
    let mut ids = [0; 3];
    for (input, id) in ids.iter_mut().enumerate() {
        let side_facing = (facing + [1, 2, 3][input]) % 4;
        let side = kinetic::forward(side_facing);
        let belt = std::array::from_fn(|axis| target[axis] + side[axis]);
        place(
            &mut chunks,
            &mut journal,
            belt,
            148,
            BlockObjectState {
                facing: (side_facing + 2) % 4,
                ..Default::default()
            },
        );
        drives.insert(
            belt,
            kinetic::KineticState {
                rpm: 30.0,
                ..Default::default()
            },
        );
        let center = terra_world::conveyors::segment_at(&journal, belt)
            .unwrap()
            .drop_center()
            .unwrap();
        assert!(journal.gameplay_mut().drop_stack(
            center,
            ItemStack {
                item: ItemKind::Block(1),
                count: input as u16 + 1
            },
            [0.0; 3],
            0.0
        ));
        *id = journal.gameplay().drops.last().unwrap().id;
    }
    journal.replace_kinetic_states(drives);
    (chunks, journal, port, ids)
}

#[test]
fn merger_round_robin_takes_one_item_from_each_connection_without_teleport_or_loss() {
    for facing in 0..4 {
        let (chunks, mut journal, port, ids) = merger_fixture(facing);
        let mut transport = terra_world::conveyors::Transport::default();
        let forward = kinetic::forward(facing);
        let items = [[255, 0, 0], [0, 255, 0], [0, 0, 255]]
            .map(|rgb| ItemKind::Dye(terra_core::beds::DyeColor::Rgb(rgb)));
        for (drop, item) in journal.gameplay_mut().drops.iter_mut().zip(items) {
            drop.stack = ItemStack { item, count: 2 };
        }
        let mut collected = Vec::new();
        for (input, item) in items.into_iter().chain(items).enumerate() {
            let mut reached = None;
            for _ in 0..100 {
                let before = journal
                    .gameplay()
                    .drops
                    .iter()
                    .map(|drop| (drop.id, (drop.position, drop.stack.item)))
                    .collect::<BTreeMap<_, _>>();
                terra_world::conveyors::transport(
                    &mut transport,
                    &chunks,
                    &mut journal,
                    &Default::default(),
                    0.02,
                );
                for drop in &journal.gameplay().drops {
                    let previous = before
                        .get(&drop.id)
                        .map(|entry| entry.0)
                        .unwrap_or_else(|| {
                            before
                                .values()
                                .find(|(_, item)| *item == drop.stack.item)
                                .unwrap()
                                .0
                        });
                    let distance = (0..3)
                        .map(|i| (drop.position[i] - previous[i]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    assert!(
                        distance <= 0.02001,
                        "junctions may not teleport: {distance}"
                    );
                    let along = (0..3)
                        .map(|i| {
                            (drop.position[i]
                                - (port[i] as f64 + if i == 1 { -0.245 } else { 0.5 }))
                                * forward[i] as f64
                        })
                        .sum::<f64>();
                    if along >= 0.75 {
                        reached = Some(drop.id);
                    }
                }
                if reached.is_some() {
                    break;
                }
            }
            let id = reached.unwrap_or_else(|| panic!("round robin facing={facing} input={input}"));
            let drop = journal
                .gameplay()
                .drops
                .iter()
                .find(|drop| drop.id == id)
                .unwrap();
            assert_eq!(drop.stack, ItemStack { item, count: 1 });
            assert_eq!(
                id == ids[input % 3],
                input >= 3,
                "rest retains source ID until its final item"
            );
            assert!(terra_world::conveyors::carrying_segment(drop, &journal).is_some());
            assert!(drop.grounded);
            journal.gameplay_mut().drops.retain(|drop| drop.id != id);
            collected.push(item);
            for item in items {
                let count = journal
                    .gameplay()
                    .drops
                    .iter()
                    .filter(|drop| drop.stack.item == item)
                    .map(|drop| usize::from(drop.stack.count))
                    .sum::<usize>()
                    + collected
                        .iter()
                        .filter(|collected| **collected == item)
                        .count();
                assert_eq!(count, 2);
            }
        }
        assert!(journal.gameplay().drops.is_empty());
    }
}

#[test]
fn merger_stopped_backwards_occupied_and_unloaded_output_never_consumes_or_moves_stacks() {
    for mode in 0..4 {
        let (mut chunks, mut journal, _port, _) = merger_fixture(0);
        if mode == 0 || mode == 1 {
            let mut states = journal.kinetic_states().collect::<BTreeMap<_, _>>();
            states.get_mut(&[8, 4, 8]).unwrap().rpm = if mode == 0 { 0.0 } else { -30.0 };
            journal.replace_kinetic_states(states);
        } else if mode == 2 {
            let center = terra_world::conveyors::segment_at(&journal, [8, 4, 8])
                .unwrap()
                .drop_center()
                .unwrap();
            assert!(journal.gameplay_mut().drop_stack(
                center,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 4
                },
                [0.0; 3],
                0.0
            ));
        } else {
            chunks.loaded.clear();
        }
        // Stop the entire output run rather than leave an independently powered neighbor.
        if mode <= 1 {
            let mut states = journal.kinetic_states().collect::<BTreeMap<_, _>>();
            for z in 8..=11 {
                states.get_mut(&[8, 4, z]).unwrap().rpm = if mode == 0 { 0.0 } else { -30.0 };
            }
            journal.replace_kinetic_states(states);
        }
        // An occupied stopped output cannot be advanced away by the test itself.
        if mode == 2 {
            let mut states = journal.kinetic_states().collect::<BTreeMap<_, _>>();
            for z in 8..=11 {
                states.get_mut(&[8, 4, z]).unwrap().rpm = 0.0;
            }
            journal.replace_kinetic_states(states);
        }
        let before = journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.stack))
            .collect::<Vec<_>>();
        let mut transport = terra_world::conveyors::Transport::default();
        for _ in 0..40 {
            terra_world::conveyors::transport(
                &mut transport,
                &chunks,
                &mut journal,
                &Default::default(),
                0.05,
            );
        }
        assert_eq!(
            journal
                .gameplay()
                .drops
                .iter()
                .map(|drop| (drop.id, drop.stack))
                .collect::<Vec<_>>(),
            before
        );
        for drop in journal.gameplay().drops.iter().take(3) {
            let across = (drop.position[0] - 8.5)
                .abs()
                .max((drop.position[2] - 8.5).abs());
            assert!(
                across >= 0.4999,
                "ungranted input crossed the entry: {:?}",
                drop.position
            );
        }
    }
}

#[test]
fn normal_belt_transport_waits_for_merger_turns_and_keeps_waiting_stacks_owned() {
    let (chunks, mut journal, _port, ids) = merger_fixture(0);
    let mut transport = terra_world::conveyors::Transport::default();
    // Stop the output, allowing incoming strips to back up at the hood's real entry.
    let mut states = journal.kinetic_states().collect::<BTreeMap<_, _>>();
    for z in 8..=11 {
        states.get_mut(&[8, 4, z]).unwrap().rpm = 0.0;
    }
    journal.replace_kinetic_states(states);
    for _ in 0..40 {
        terra_world::conveyors::transport(
            &mut transport,
            &chunks,
            &mut journal,
            &Default::default(),
            0.05,
        );
    }
    for id in ids {
        let drop = journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == id)
            .unwrap();
        assert_ne!(
            terra_world::conveyors::carrying_segment(drop, &journal)
                .unwrap()
                .position(),
            [8, 4, 8]
        );
    }
    let before = journal
        .gameplay()
        .drops
        .iter()
        .map(|drop| (drop.id, drop.stack))
        .collect::<Vec<_>>();
    let mut runtime = Runtime::default();
    advance(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| (drop.id, drop.stack))
            .collect::<Vec<_>>(),
        before
    );
    assert!(
        journal
            .gameplay()
            .drops
            .iter()
            .all(|drop| drop.velocity == [0.0; 3])
    );
    // A second turn sees the occupied center and leaves every source intact.
    let first = journal.gameplay().drops.clone();
    advance(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.5,
    );
    assert_eq!(journal.gameplay().drops, first);
}

#[test]
fn distributor_filters_each_output_and_moves_rgb_stacks_continuously_after_runtime_reload() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let port = [8, 5, 8];
    place(
        &mut chunks,
        &mut journal,
        port,
        logistics::BELT_DISTRIBUTOR,
        BlockObjectState::default(),
    );
    let mut states = BTreeMap::new();
    for (direction, facing) in [
        ([0, 0, -1], 0),
        ([-1, 0, 0], 3),
        ([0, 0, 1], 0),
        ([1, 0, 0], 1),
    ] {
        for distance in 1..=3 {
            let cell = [8 + direction[0] * distance, 4, 8 + direction[2] * distance];
            place(
                &mut chunks,
                &mut journal,
                cell,
                148,
                BlockObjectState {
                    facing,
                    ..Default::default()
                },
            );
            states.insert(
                cell,
                kinetic::KineticState {
                    rpm: 30.0,
                    ..Default::default()
                },
            );
        }
    }
    place(
        &mut chunks,
        &mut journal,
        [8, 4, 8],
        148,
        BlockObjectState::default(),
    );
    states.insert(
        [8, 4, 8],
        kinetic::KineticState {
            rpm: 30.0,
            ..Default::default()
        },
    );
    journal.replace_kinetic_states(states);
    let red = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([255, 0, 0]));
    let blue = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([0, 0, 255]));
    journal
        .set_distributor(
            port,
            logistics::DistributorConfig {
                filters: [
                    FilterConfig {
                        item: Some(red),
                        allowed: true,
                    },
                    FilterConfig {
                        item: Some(blue),
                        allowed: true,
                    },
                    FilterConfig {
                        item: Some(red),
                        allowed: false,
                    },
                ],
            },
        )
        .unwrap();
    let mut transport = terra_world::conveyors::Transport::default();
    // The explicit Allowed-Blue output is reserved; a general Blocked-Red
    // output must not steal its matching items on the second turn.
    for (item, expected) in [(blue, 1), (blue, 1), (red, 0)] {
        let start = terra_world::conveyors::segment_at(&journal, [8, 4, 7])
            .unwrap()
            .drop_center()
            .unwrap();
        let stack = ItemStack { item, count: 1 };
        assert!(
            journal
                .gameplay_mut()
                .drop_stack(start, stack, [0.0; 3], 0.0)
        );
        let id = journal.gameplay().drops.last().unwrap().id;
        let mut reloaded = false;
        let mut reached = false;
        let direction = logistics::distributor_output(0, expected).unwrap();
        for _ in 0..150 {
            let previous = journal.gameplay().drops[0].position;
            terra_world::conveyors::transport(
                &mut transport,
                &chunks,
                &mut journal,
                &Default::default(),
                0.02,
            );
            let drop = &journal.gameplay().drops[0];
            assert_eq!((drop.id, drop.stack), (id, stack));
            let distance = (0..3)
                .map(|i| (drop.position[i] - previous[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(
                distance <= 0.02001,
                "continuous dispatcher movement: {distance}"
            );
            if reloaded {
                assert!(
                    (0..3)
                        .map(|i| (drop.position[i] - previous[i]) * direction[i] as f64)
                        .sum::<f64>()
                        >= -0.000001,
                    "reload must not reverse the outgoing leg"
                );
            }
            if item == red
                && !reloaded
                && terra_world::conveyors::routing_carrying(drop, &journal) == Some(port)
                && drop.position[0] > 8.7
                && drop.position[0] < 9.0
            {
                assert!(
                    terra_world::conveyors::carried_drop_hit(
                        &journal.gameplay().drops,
                        &journal,
                        [drop.position[0], drop.position[1] + 2.0, drop.position[2]],
                        [0.0, -1.0, 0.0],
                        3.0
                    )
                    .is_some()
                );
                transport = terra_world::conveyors::Transport::default();
                reloaded = true;
            }
            let along = (drop.position[0] - 8.5) * direction[0] as f64
                + (drop.position[2] - 8.5) * direction[2] as f64;
            if along >= 0.75 {
                reached = true;
                break;
            }
        }
        assert!((item != red || reloaded) && reached, "output={expected}");
        journal.gameplay_mut().drops.clear();
    }
}

#[test]
fn junction_single_item_admission_preserves_source_when_drop_capacity_or_ids_are_exhausted() {
    let (chunks, mut journal, _, ids) = merger_fixture(0);
    journal
        .gameplay_mut()
        .drops
        .retain(|drop| drop.id == ids[0]);
    journal.gameplay_mut().drops[0].stack.count = 7;
    while journal.gameplay().drops.len() < terra_core::MAX_WORLD_DROPS {
        let x = 30.5 + journal.gameplay().drops.len() as f64;
        assert!(journal.gameplay_mut().drop_stack(
            [x, 4.755, 30.5],
            ItemStack {
                item: ItemKind::Block(3),
                count: 1
            },
            [0.0; 3],
            0.0
        ));
    }
    let mut runtime = terra_world::conveyors::Transport::default();
    let total = journal
        .gameplay()
        .drops
        .iter()
        .map(|drop| usize::from(drop.stack.count))
        .sum::<usize>();
    terra_world::conveyors::transport(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.02,
    );
    assert_eq!(journal.gameplay().drops.len(), terra_core::MAX_WORLD_DROPS);
    assert_eq!(journal.gameplay().drops[0].stack.count, 7);
    journal.gameplay_mut().drops.pop();
    let next = journal.gameplay().next_drop;
    journal.gameplay_mut().next_drop = u64::MAX;
    terra_world::conveyors::transport(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.02,
    );
    assert_eq!(
        journal.gameplay().drops.len(),
        terra_core::MAX_WORLD_DROPS - 1
    );
    assert_eq!(journal.gameplay().drops[0].stack.count, 7);
    journal.gameplay_mut().next_drop = next;
    journal.gameplay_mut().revision = u64::MAX;
    terra_world::conveyors::transport(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.02,
    );
    assert_eq!(journal.gameplay().drops[0].stack.count, 7);
    journal.gameplay_mut().revision = 0;
    terra_world::conveyors::transport(
        &mut runtime,
        &chunks,
        &mut journal,
        &Default::default(),
        0.02,
    );
    assert_eq!(
        (
            journal.gameplay().drops[0].id,
            journal.gameplay().drops[0].stack.count
        ),
        (ids[0], 6)
    );
    let single = journal.gameplay().drops.last().unwrap();
    assert_eq!(
        single.stack,
        ItemStack {
            item: ItemKind::Block(1),
            count: 1
        }
    );
    assert_ne!(single.id, ids[0]);
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .map(|drop| usize::from(drop.stack.count))
            .sum::<usize>(),
        total - 1
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn active_junction_routes_hold_on_removed_outputs_and_resume_on_a_rebuilt_span() {
    let (mut chunks, mut journal, port, ids) = merger_fixture(0);
    journal
        .gameplay_mut()
        .drops
        .retain(|drop| drop.id == ids[0]);
    let mut runtime = terra_world::conveyors::Transport::default();
    for _ in 0..65 {
        terra_world::conveyors::transport(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.02,
        );
    }
    assert_eq!(
        terra_world::conveyors::routing_carrying(&journal.gameplay().drops[0], &journal),
        Some(port)
    );
    for z in 8..=11 {
        let p = [8, 4, z];
        let (section, local) = terra_voxel::global_to_section(p);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        journal.record(p, 0).unwrap();
    }
    // Keep the central carrying strip, but the absent neighboring output may not be bypassed.
    place(
        &mut chunks,
        &mut journal,
        [8, 4, 8],
        148,
        BlockObjectState::default(),
    );
    let held = journal.gameplay().drops[0].position;
    for _ in 0..10 {
        terra_world::conveyors::transport(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.02,
        );
    }
    assert_eq!(journal.gameplay().drops[0].position, held);
    for (p, material, state) in [
        ([8, 4, 8], 144, BlockObjectState::default()),
        ([8, 4, 11], 144, BlockObjectState::default()),
    ] {
        place(&mut chunks, &mut journal, p, material, state);
    }
    let plan = terra_world::conveyors::layout(
        &chunks,
        &journal,
        [8, 4, 8],
        [8, 4, 11],
        ItemKind::Block(148),
    )
    .unwrap();
    assert!(terra_world::conveyors::commit(&mut chunks, &mut journal, &plan).unwrap());
    journal.replace_kinetic_states(
        (8..=11)
            .map(|z| {
                (
                    [8, 4, z],
                    kinetic::KineticState {
                        rpm: 30.0,
                        ..Default::default()
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    );
    for _ in 0..60 {
        terra_world::conveyors::transport(
            &mut runtime,
            &chunks,
            &mut journal,
            &Default::default(),
            0.02,
        );
    }
    let drop = &journal.gameplay().drops[0];
    assert_eq!((drop.id, drop.stack.count), (ids[0], 1));
    assert!(
        drop.position[2] > 9.25,
        "rebuilt output must not leave a permanently assigned stale route"
    );
}
