use super::*;
use terra_core::{Inventory, ItemKind, ItemStack, MineralItem, WoodSpecies};

fn stack(item: ItemKind, count: u16) -> ItemStack {
    ItemStack { item, count }
}

#[test]
fn random_chest_extraction_is_repeatable_filtered_and_reaches_both_halves() {
    let mut journal = EditJournal::default();
    let source = [-17, 2, 15];
    let peer = [-16, 2, 15];
    for (position, other, secondary, items) in
        [(source, peer, false, [1, 3]), (peer, source, true, [4, 5])]
    {
        journal.record(position, 29).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(other),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal
            .set_container(
                position,
                Inventory {
                    slots: items
                        .into_iter()
                        .map(|material| stack(ItemKind::Block(material), 128))
                        .collect(),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let mut repeat = journal.clone();
    let mut counts = std::collections::BTreeMap::new();
    for _ in 0..128 {
        for candidate in [&mut journal, &mut repeat] {
            assert!(
                candidate
                    .eject_inventory_output_selected(
                        source,
                        [-16.5, 2.0, 16.5],
                        Default::default(),
                        true
                    )
                    .unwrap()
            );
            assert_eq!(candidate.gameplay().drops.last().unwrap().stack.count, 1);
        }
        assert_eq!(journal.gameplay(), repeat.gameplay());
        for position in [source, peer] {
            assert_eq!(
                journal.container_at(position),
                repeat.container_at(position)
            );
        }
        let ItemKind::Block(material) = journal.gameplay().drops.last().unwrap().stack.item else {
            panic!()
        };
        *counts.entry(material).or_insert(0) += 1;
        journal.gameplay_mut().drops.clear();
        repeat.gameplay_mut().drops.clear();
    }
    assert_eq!(counts.len(), 4);
    assert!(counts.values().all(|count| (10..=60).contains(count)));
    let forbidden = ItemKind::Block(1);
    let before = journal.container_at(source).unwrap().slot(0);
    for _ in 0..32 {
        assert!(
            journal
                .eject_inventory_output_selected(
                    source,
                    [-16.5, 2.0, 16.5],
                    terra_core::logistics::FilterConfig {
                        item: Some(forbidden),
                        allowed: false
                    },
                    true
                )
                .unwrap()
        );
        assert_ne!(
            journal.gameplay().drops.last().unwrap().stack.item,
            forbidden
        );
        journal.gameplay_mut().drops.clear();
    }
    assert_eq!(journal.container_at(source).unwrap().slot(0), before);
    let snapshot = journal.container_at(source).cloned();
    journal.gameplay_mut().next_drop = u64::MAX;
    assert!(
        !journal
            .eject_inventory_output_selected(source, [-16.5, 2.0, 16.5], Default::default(), true)
            .unwrap()
    );
    assert_eq!(journal.container_at(source), snapshot.as_ref());
}

#[test]
fn ordered_chest_extraction_remains_the_default() {
    let mut journal = EditJournal::default();
    journal.record([0; 3], 29).unwrap();
    journal
        .set_container(
            [0; 3],
            Inventory {
                slots: vec![stack(ItemKind::Block(1), 16), stack(ItemKind::Block(3), 16)],
                ..Default::default()
            },
        )
        .unwrap();
    for _ in 0..8 {
        assert!(
            journal
                .eject_inventory_output([0; 3], [0.5, 1.0, 0.5])
                .unwrap()
        );
        assert_eq!(
            journal.gameplay().drops.last().unwrap().stack,
            stack(ItemKind::Block(1), 1)
        );
        journal.gameplay_mut().drops.clear();
    }
    assert_eq!(
        journal.container_at([0; 3]).unwrap().slot(1).unwrap().count,
        16
    );
}

#[test]
fn mixer_routes_standard_pigments_to_inputs_and_exact_rgb_only_from_output() {
    use terra_core::{beds::DyeColor, mixer};
    let mut journal = EditJournal::default();
    let position = [-17, 15, 16];
    journal.record(position, mixer::MATERIAL).unwrap();
    journal
        .set_object_state(position, Some(BlockObjectState::default()))
        .unwrap();
    journal.set_mixer(position, Default::default()).unwrap();
    let pigment = ItemKind::Dye(DyeColor::ALL[0]);
    assert!(journal.gameplay_mut().drop_stack(
        [-16.5, 16.13, 16.5],
        stack(pigment, 9),
        [0.0; 3],
        0.0
    ));
    let id = journal.gameplay().drops[0].id;
    assert!(journal.import_drop_to_inventory(position, id).unwrap());
    assert_eq!(
        journal.container_at(position).unwrap().slot(0),
        Some(stack(pigment, 9))
    );
    assert!(journal.gameplay().drops.is_empty());
    let rgb = ItemKind::Dye(DyeColor::Rgb([12, 96, 211]));
    assert!(
        journal
            .gameplay_mut()
            .drop_stack([-16.5, 16.13, 16.5], stack(rgb, 7), [0.0; 3], 0.0)
    );
    let rejected = journal.gameplay().drops[0].clone();
    assert!(
        !journal
            .import_drop_to_inventory(position, rejected.id)
            .unwrap()
    );
    assert_eq!(journal.gameplay().drops[0], rejected);
    let mut inventory = journal.container_at(position).unwrap().clone();
    inventory.slots.resize(mixer::SLOTS, ItemStack::EMPTY);
    inventory.slots[mixer::OUTPUT] = stack(rgb, 7);
    journal.set_container(position, inventory).unwrap();
    assert!(
        journal
            .eject_inventory_output(position, [-16.5, 14.84, 16.5])
            .unwrap()
    );
    assert_eq!(
        journal.gameplay().drops.last().unwrap().stack,
        stack(rgb, 1)
    );
    assert_eq!(
        journal.container_at(position).unwrap().slot(mixer::OUTPUT),
        Some(stack(rgb, 6))
    );
    assert_eq!(
        journal.container_at(position).unwrap().slot(0),
        Some(stack(pigment, 9))
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn chest_exports_then_reimports_without_touching_other_inventory_slots() {
    let mut journal = EditJournal::default();
    let a = [-17, 2, 15];
    let b = [-16, 2, 15];
    for (p, peer, secondary, wood) in [
        (a, b, false, WoodSpecies::Oak),
        (b, a, true, WoodSpecies::Birch),
    ] {
        journal.record(p, 29).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    wood,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let item = ItemKind::Mineral(MineralItem::IronIngot);
    journal
        .set_container(
            b,
            Inventory {
                slots: vec![stack(item, 9)],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        journal
            .eject_inventory_output(a, [-16.5, 1.755, 16.5])
            .unwrap()
    );
    assert_eq!(journal.container_at(b).unwrap().slot(0).unwrap().count, 8);
    let drop = journal.gameplay().drops[0].clone();
    assert_eq!(drop.stack, stack(item, 1));
    assert!(journal.import_drop_to_inventory(a, drop.id).unwrap());
    assert_eq!(
        journal.container_at(a).unwrap().slot(0),
        Some(stack(item, 1))
    );
    assert_eq!(
        journal.container_at(b).unwrap().slot(0),
        Some(stack(item, 8))
    );
    assert!(journal.gameplay().drops.is_empty());
    let before = journal.container_at(a).unwrap().clone();
    journal.gameplay_mut().next_drop = u64::MAX;
    assert!(
        !journal
            .eject_inventory_output(a, [-16.5, 1.755, 16.5])
            .unwrap()
    );
    assert_eq!(journal.container_at(a), Some(&before));
}

#[test]
fn machine_import_accepts_only_recipe_inputs_or_fuel_never_outputs() {
    let mut journal = EditJournal::default();
    let source = [2, 2, 2];
    let peer = [2, 3, 2];
    for (p, other, secondary) in [(source, peer, false), (peer, source, true)] {
        journal.record(p, 59).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    peer: Some(other),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let input = terra_core::furnace::recipes()[0].input;
    assert!(
        journal
            .gameplay_mut()
            .drop_stack([1.5, 1.755, 2.5], stack(input, 3), [0.0; 3], 0.0)
    );
    let id = journal.gameplay().drops[0].id;
    assert!(journal.import_drop_to_inventory(source, id).unwrap());
    assert_eq!(
        journal
            .container_at(source)
            .unwrap()
            .slot(terra_core::furnace::INPUT),
        Some(stack(input, 3))
    );
    assert!(
        journal
            .container_at(source)
            .unwrap()
            .slot(terra_core::furnace::OUTPUT)
            .is_none()
    );
    let item = ItemKind::Tool(terra_core::ToolKind::Wrench);
    assert!(
        journal
            .gameplay_mut()
            .drop_stack([1.5, 1.755, 2.5], stack(item, 1), [0.0; 3], 0.0)
    );
    let before = journal.gameplay().clone();
    assert!(
        !journal
            .import_drop_to_inventory(source, before.drops[0].id)
            .unwrap()
    );
    assert_eq!(journal.gameplay(), &before);
}

#[test]
fn port_wrench_reverses_flow_without_moving_attached_inventory_or_losing_mode_on_reload() {
    use crate::streaming::{ChunkStore, LoadedSection};
    use terra_voxel::{BlockSection, global_to_section};
    for material in [152, 153] {
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        let port = [4, 4, 4];
        let chest = [4, 4, 3];
        for (p, m) in [(port, material), (chest, 29)] {
            let (s, l) = global_to_section(p);
            chunks
                .loaded
                .entry(s)
                .or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(s).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                })
                .data
                .set_block(l, m)
                .unwrap();
            journal.record(p, m).unwrap();
        }
        journal
            .set_object_state(
                port,
                Some(BlockObjectState {
                    peer: Some(chest),
                    ..Default::default()
                }),
            )
            .unwrap();
        journal
            .set_container(
                chest,
                Inventory {
                    slots: vec![stack(ItemKind::Block(3), 12)],
                    ..Default::default()
                },
            )
            .unwrap();
        let original = journal.container_at(chest).unwrap().clone();
        let mode = terra_core::logistics::extracts(material, 0);
        let plan = crate::wrench::propose(&chunks, &journal, port, false).unwrap();
        crate::wrench::commit(&mut chunks, &mut journal, &plan).unwrap();
        let state = journal.object_state(port).unwrap();
        assert_eq!(state.peer, Some(chest));
        assert_eq!(state.facing, 4);
        assert_eq!(
            terra_core::logistics::extracts(material, state.facing),
            !mode
        );
        assert_eq!(journal.container_at(chest), Some(&original));
        let mut restored = EditJournal::default();
        restored
            .import_delta(journal.section_delta(global_to_section(port).0))
            .unwrap();
        assert_eq!(restored.object_state(port), Some(state));
    }
}
