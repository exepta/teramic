use super::{tests::place, *};
use terra_core::{Inventory, ItemKind, ItemStack, kinetic, logistics::FilterConfig};
use terra_world::{conveyors, edits::BlockObjectState};

fn tick(
    runtime: &mut Runtime,
    transport: &mut conveyors::Transport,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    steps: usize,
) {
    for _ in 0..steps {
        conveyors::transport(transport, chunks, journal, &Default::default(), 0.1);
        advance(runtime, chunks, journal, &Default::default(), 0.1);
        for (index, drop) in journal.gameplay().drops.iter().enumerate() {
            for other in &journal.gameplay().drops[index + 1..] {
                assert!(
                    (0..3).any(|axis| (drop.position[axis] - other.position[axis]).abs() >= 0.26),
                    "items may queue, not overlap"
                );
            }
        }
    }
}

#[test]
fn an_actual_full_belt_retains_inventory_and_resumes_one_item_transfers_without_loss() {
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let source = [9, 4, 3];
    let target = [9, 4, 5];
    let export = [8, 4, 3];
    let import = [8, 4, 5];
    for (position, material, state) in [
        (source, 29, BlockObjectState::default()),
        (target, 29, BlockObjectState::default()),
        (
            export,
            terra_core::logistics::FILTERED_FUNNEL,
            BlockObjectState {
                facing: 5,
                peer: Some(source),
                ..Default::default()
            },
        ),
        (
            import,
            153,
            BlockObjectState {
                facing: 1,
                peer: Some(target),
                ..Default::default()
            },
        ),
    ] {
        place(&mut chunks, &mut journal, position, material, state);
    }
    let item = ItemKind::Dye(terra_core::beds::DyeColor::Rgb([71, 93, 184]));
    journal
        .set_container(
            source,
            Inventory {
                slots: vec![ItemStack { item, count: 32 }],
                ..Default::default()
            },
        )
        .unwrap();
    journal
        .set_container(
            target,
            Inventory {
                slots: vec![
                    ItemStack {
                        item: ItemKind::Block(1),
                        count: ItemKind::Block(1).max_stack()
                    };
                    terra_core::containers::CHEST_SLOTS
                ],
                ..Default::default()
            },
        )
        .unwrap();
    let mut states = BTreeMap::new();
    for z in 3..=5 {
        let position = [8, 3, z];
        place(
            &mut chunks,
            &mut journal,
            position,
            148,
            BlockObjectState::default(),
        );
        states.insert(position, kinetic::KineticState::default());
    }
    let wall = [8, 3, 6];
    let (section, local) = terra_voxel::global_to_section(wall);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(wall, 3).unwrap();
    journal.replace_kinetic_states(states.clone());
    let mut runtime = Runtime::default();
    let mut transport = conveyors::Transport::default();
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 30);
    assert_eq!(
        journal.gameplay().drops.len(),
        1,
        "a stopped empty belt accepts the first item only"
    );
    assert_eq!(
        journal.gameplay().drops[0].stack,
        ItemStack { item, count: 1 }
    );
    assert_eq!(journal.container_at(source).unwrap().count(item), 31);
    for drive in states.values_mut() {
        drive.rpm = 30.0;
    }
    journal.replace_kinetic_states(states);
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 200);
    assert!(
        journal.container_at(source).unwrap().count(item) > 0,
        "the source must retain items when the carrying run fills"
    );
    let held = journal.gameplay().clone();
    let source_inventory = journal.container_at(source).unwrap().clone();
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 30);
    assert_eq!(journal.container_at(source), Some(&source_inventory));
    assert_eq!(journal.gameplay().drops, held.drops);
    assert_eq!(
        source_inventory.count(item)
            + journal
                .gameplay()
                .drops
                .iter()
                .map(|drop| drop.stack.count)
                .sum::<u16>(),
        32
    );
    let loaded = std::mem::take(&mut chunks.loaded);
    let unloaded = journal.gameplay().clone();
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 10);
    assert_eq!(journal.gameplay(), &unloaded);
    chunks.loaded = loaded;
    journal
        .set_logistics_filter(
            export,
            FilterConfig {
                item: Some(item),
                allowed: false,
            },
        )
        .unwrap();
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 10);
    assert_eq!(journal.container_at(source), Some(&source_inventory));
    journal
        .set_logistics_filter(
            export,
            FilterConfig {
                item: Some(item),
                allowed: true,
            },
        )
        .unwrap();
    journal.set_container(target, Inventory::default()).unwrap();
    tick(&mut runtime, &mut transport, &chunks, &mut journal, 240);
    assert!(journal.container_at(source).unwrap().slot(0).is_none());
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(journal.container_at(target).unwrap().count(item), 32);
    assert!(journal.gameplay().valid());
}

#[test]
fn importing_one_item_from_an_old_stack_preserves_id_and_rest_without_replaying_a_consumed_id() {
    for material in [29, 99, 156] {
        let mut journal = EditJournal::default();
        let station = [-17, 4, 15];
        journal.record(station, material).unwrap();
        journal
            .set_object_state(station, Some(BlockObjectState::default()))
            .unwrap();
        if material == 99 {
            journal.set_crusher(station, Default::default()).unwrap();
        }
        if material == 156 {
            journal.set_mixer(station, Default::default()).unwrap();
        }
        let item = match material {
            99 => terra_core::crusher::recipes()[0].input,
            156 => ItemKind::Dye(terra_core::beds::DyeColor::ALL[0]),
            _ => ItemKind::Block(3),
        };
        assert!(journal.gameplay_mut().drop_stack(
            [-16.5, 4.755, 15.5],
            ItemStack { item, count: 11 },
            [0.0; 3],
            0.0
        ));
        let id = journal.gameplay().drops[0].id;
        let unchanged = journal.gameplay().clone();
        assert!(
            !journal
                .import_drop_to_inventory_count(station, id, 0)
                .unwrap()
        );
        assert_eq!(journal.gameplay(), &unchanged);
        for remaining in (1..=10).rev() {
            assert!(
                journal
                    .import_drop_to_inventory_count(station, id, 1)
                    .unwrap()
            );
            assert_eq!(
                (
                    journal.gameplay().drops[0].id,
                    journal.gameplay().drops[0].stack
                ),
                (
                    id,
                    ItemStack {
                        item,
                        count: remaining
                    }
                )
            );
        }
        assert!(
            journal
                .import_drop_to_inventory_count(station, id, 1)
                .unwrap()
        );
        assert!(journal.gameplay().drops.is_empty());
        assert_eq!(journal.container_at(station).unwrap().count(item), 11);
        assert!(
            !journal
                .import_drop_to_inventory_count(station, id, 1)
                .unwrap()
        );
        assert!(journal.gameplay().valid());
    }
}
