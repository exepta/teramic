use super::*;
use terra_core::{ItemKind, ItemStack, MineralItem, furnace, logistics};
use terra_world::edits::BlockObjectState;

const SOURCE: [i64; 3] = [-513, 15, -1];
const UPPER: [i64; 3] = [-513, 16, -1];
const HOPPER: [i64; 3] = [-513, 15, 0];
const IMPORTER: [i64; 3] = [-513, 15, 3];
const CHEST: [i64; 3] = [-512, 15, 3];
const DROP_POSITION: [f64; 3] = [-512.5, 14.747, 0.5];

fn output_total(journal: &EditJournal, item: ItemKind) -> u16 {
    [SOURCE, CHEST]
        .into_iter()
        .filter_map(|position| journal.container_at(position))
        .map(|inventory| inventory.count(item))
        .chain(
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| drop.stack.item == item)
                .map(|drop| drop.stack.count),
        )
        .sum()
}

#[test]
fn logistics_ports_in_flight_output_and_chest_survive_real_region_commits_and_continue_losslessly()
{
    let root =
        std::env::temp_dir().join(format!("terra-logistics-regions-{}", PlayerUuid::new_v4()));
    let world = WorldId(8152);
    let item = ItemKind::Mineral(MineralItem::IronIngot);
    let recipe = furnace::recipe_for(ItemKind::Mineral(MineralItem::RawIron)).unwrap();
    let mut journal = EditJournal::default();
    for (position, peer, secondary) in [(SOURCE, UPPER, false), (UPPER, SOURCE, true)] {
        journal.record(position, 59).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    journal.record(CHEST, 29).unwrap();
    journal
        .set_object_state(
            CHEST,
            Some(BlockObjectState {
                wood: terra_core::WoodSpecies::Cherry,
                ..Default::default()
            }),
        )
        .unwrap();
    for (position, material, peer, facing) in [
        (HOPPER, logistics::HOPPER, SOURCE, 0),
        (IMPORTER, logistics::IMPORTER, CHEST, 1),
    ] {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let belt = [HOPPER[0], HOPPER[1] - 1, HOPPER[2]];
    journal.record(belt, 148).unwrap();
    journal
        .set_object_state(belt, Some(BlockObjectState::default()))
        .unwrap();
    let input = ItemStack {
        item: recipe.input,
        count: 3,
    };
    let fuel = ItemStack {
        item: ItemKind::Mineral(MineralItem::Coal),
        count: 2,
    };
    journal
        .set_container(
            SOURCE,
            Inventory {
                slots: vec![input, fuel, ItemStack { item, count: 9 }],
                revision: 17,
                ..Default::default()
            },
        )
        .unwrap();
    journal
        .set_container(
            CHEST,
            Inventory {
                slots: vec![ItemStack { item, count: 3 }],
                revision: 9,
                ..Default::default()
            },
        )
        .unwrap();
    let state = furnace::FurnaceState {
        progress_seconds: 1.5,
        recipe: Some(recipe.id),
        burn_remaining: 8.0,
        burn_total: 20.0,
        completed: 31,
        outlet: [-513, 17, -1],
        ..Default::default()
    };
    journal.set_furnace(SOURCE, state.clone()).unwrap();
    assert!(journal.eject_machine_output(SOURCE, DROP_POSITION).unwrap());
    assert_eq!(output_total(&journal, item), 12);
    assert_eq!(
        journal.gameplay().drops[0].stack.count,
        logistics::ITEMS_PER_TRANSFER
    );
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.container_at(SOURCE), journal.container_at(SOURCE));
    assert_eq!(loaded.container_at(CHEST), journal.container_at(CHEST));
    assert_eq!(loaded.furnace_at(SOURCE), Some(&state));
    for position in [SOURCE, UPPER, HOPPER, IMPORTER, CHEST, belt] {
        assert_eq!(loaded.block(position), journal.block(position));
        assert_eq!(
            loaded.object_state(position),
            journal.object_state(position)
        );
    }
    for position in [HOPPER, IMPORTER] {
        assert!(
            loaded
                .logistics_in_section(terra_voxel::global_to_section(position).0)
                .any(|port| port == position)
        );
    }
    let id = loaded.gameplay().drops[0].id;
    assert!(loaded.import_drop_to_chest(CHEST, id).unwrap());
    assert!(!loaded.import_drop_to_chest(CHEST, id).unwrap());
    assert!(loaded.eject_machine_output(SOURCE, DROP_POSITION).unwrap());
    assert_eq!(output_total(&loaded, item), 12);
    assert_eq!(loaded.gameplay().drops[0].stack.count, 1);
    assert_eq!(loaded.gameplay().drops[0].id, id + 1);
    save_edits(&root, world, &loaded).unwrap();
    let mut reloaded = load_edits(&root, world).unwrap();
    assert_eq!(reloaded.gameplay(), loaded.gameplay());
    assert_eq!(reloaded.container_at(SOURCE), loaded.container_at(SOURCE));
    assert_eq!(reloaded.container_at(CHEST), loaded.container_at(CHEST));
    assert_eq!(reloaded.furnace_at(SOURCE), Some(&state));
    let id = reloaded.gameplay().drops[0].id;
    assert!(reloaded.import_drop_to_chest(CHEST, id).unwrap());
    let mut previous_id = id;
    for transferred in 3..=9 {
        assert!(
            reloaded
                .eject_machine_output(SOURCE, DROP_POSITION)
                .unwrap()
        );
        let drop = &reloaded.gameplay().drops[0];
        assert_eq!(drop.stack.count, 1);
        assert_eq!(drop.id, previous_id + 1);
        previous_id = drop.id;
        assert!(reloaded.import_drop_to_chest(CHEST, previous_id).unwrap());
        assert!(!reloaded.import_drop_to_chest(CHEST, previous_id).unwrap());
        assert_eq!(
            reloaded.container_at(CHEST).unwrap().count(item),
            3 + transferred
        );
        assert_eq!(output_total(&reloaded, item), 12);
        if transferred == 6 {
            save_edits(&root, world, &reloaded).unwrap();
            let checkpoint = load_edits(&root, world).unwrap();
            assert_eq!(checkpoint.gameplay(), reloaded.gameplay());
            assert_eq!(
                checkpoint.container_at(SOURCE),
                reloaded.container_at(SOURCE)
            );
            assert_eq!(checkpoint.container_at(CHEST), reloaded.container_at(CHEST));
            reloaded = checkpoint;
        }
    }
    assert!(
        !reloaded
            .eject_machine_output(SOURCE, DROP_POSITION)
            .unwrap()
    );
    assert!(reloaded.gameplay().drops.is_empty());
    assert_eq!(reloaded.container_at(CHEST).unwrap().count(item), 12);
    assert_eq!(output_total(&reloaded, item), 12);
    let inventory = reloaded.container_at(SOURCE).unwrap();
    assert_eq!(inventory.slot(furnace::INPUT), Some(input));
    assert_eq!(inventory.slot(furnace::FUEL), Some(fuel));
    assert!(inventory.slot(furnace::OUTPUT).is_none());
    assert_eq!(reloaded.furnace_at(SOURCE), Some(&state));
    assert!(reloaded.gameplay().valid());
    save_edits(&root, world, &reloaded).unwrap();
    let settled = load_edits(&root, world).unwrap();
    assert_eq!(settled.gameplay(), reloaded.gameplay());
    assert_eq!(settled.container_at(CHEST), reloaded.container_at(CHEST));
    assert_eq!(settled.container_at(SOURCE), reloaded.container_at(SOURCE));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dismantled_train_engine_burn_budget_survives_region_save_and_load() {
    let root = std::env::temp_dir().join(format!("terra-engine-regions-{}", PlayerUuid::new_v4()));
    let world = WorldId(221);
    let cell = [8, 105, 24];
    let mut journal = EditJournal::default();
    journal.record(cell, 218).unwrap();
    journal
        .set_object_state(cell, Some(BlockObjectState::default()))
        .unwrap();
    let mut inventory = Inventory::default();
    inventory.creative_set(
        0,
        ItemStack {
            item: ItemKind::Mineral(MineralItem::Coal),
            count: 7,
        },
    );
    journal.set_container(cell, inventory.clone()).unwrap();
    let state = furnace::FurnaceState {
        burn_remaining: 23.5,
        burn_total: 80.0,
        outlet: cell,
        ..Default::default()
    };
    journal.set_furnace(cell, state.clone()).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.furnace_at(cell), Some(&state));
    assert_eq!(loaded.container_at(cell), Some(&inventory));
    assert_eq!(loaded.block(cell), Some(218));
    std::fs::remove_dir_all(root).unwrap();
}
