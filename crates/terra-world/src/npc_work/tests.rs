use super::*;
use crate::streaming::LoadedSection;
use terra_core::{Inventory, ItemStack, PlayerUuid, ToolKind, WorldConfig, npc_jobs::WorkArea};
use terra_voxel::{BlockSection, global_to_section};
mod lumberjack;
mod navigation_paths;
mod planting_cycle;
mod station;
mod station_supplies;
mod suspension;

#[test]
fn an_idle_target_scan_does_not_cancel_the_villagers_ordinary_walking_motion() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    journal.gameplay_mut().jobs.workers.insert(
        npc,
        WorkerState {
            status: WorkStatus::Waiting(WaitReason::NoTargets),
            ..Default::default()
        },
    );
    let animal = journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap();
    animal.velocity = [0.8, 0.0, 0.2];
    animal.behavior = CreatureBehavior::Wander;
    let age = animal.age;
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    let animal = &journal.gameplay().creatures.animals[&npc];
    assert_eq!(animal.velocity, [0.8, 0.0, 0.2]);
    assert_eq!(animal.behavior, CreatureBehavior::Wander);
    assert_eq!(
        animal.age, age,
        "the ordinary resident owns idle scheduling and gravity"
    );
}

fn set(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
    let (owner, local) = global_to_section(cell);
    chunks
        .loaded
        .entry(owner)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        })
        .data
        .set_block(local, material)
        .unwrap();
}

fn fixture(kind: WorkKind) -> (ChunkStore, EditJournal, TerraGenerator, WorkRuntime, u64) {
    let mut chunks = ChunkStore::default();
    for x in -4..=12 {
        for z in -4..=4 {
            for y in -1..=5 {
                set(&mut chunks, [x, y, z], if y <= 0 { 3 } else { 0 });
            }
        }
    }
    let mut journal = EditJournal::default();
    let owner = PlayerUuid::from_u128(1);
    let npc = villagers::id([0, 0], 0).unwrap();
    journal.gameplay_mut().creatures.animals.insert(
        npc,
        Creature {
            id: npc,
            kind: CreatureKind::Villager,
            home: [0, 0],
            position: [2.5, 1.0, 0.5],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: villagers::MINER,
            needs: terra_core::creatures::CreatureNeeds::full(CreatureKind::Villager),
        },
    );
    journal.gameplay_mut().creatures.owners.insert(npc, owner);
    let storage = [0, 1, 0];
    set(&mut chunks, storage, 29);
    journal.record(storage, 29).unwrap();
    journal
        .set_container(storage, Inventory::default())
        .unwrap();
    if kind == WorkKind::Lumberjack {
        set(
            &mut chunks,
            [0, 1, 2],
            terra_core::profession_stations::LUMBERJACK,
        );
        journal
            .record([0, 1, 2], terra_core::profession_stations::LUMBERJACK)
            .unwrap();
    }
    journal.gameplay_mut().jobs.next_id = 2;
    journal.gameplay_mut().jobs.orders.insert(
        1,
        WorkOrder {
            id: 1,
            npc,
            owner,
            kind,
            area: WorkArea::new([5, 1, 0], [8, 2, 2]).unwrap(),
            storage,
            priority: 5,
            enabled: true,
            revision: 0,
        },
    );
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    (chunks, journal, terrain, WorkRuntime::default(), npc)
}

fn run(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    runtime: &mut WorkRuntime,
    times: usize,
) {
    for _ in 0..times {
        tick(
            chunks,
            journal,
            terrain,
            &BlockPropertiesCatalog::default(),
            0.5,
            0.1,
            runtime,
        );
    }
}

#[test]
fn collector_walks_to_real_drop_then_delivers_once_without_remote_inventory_changes() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    assert!(journal.gameplay_mut().drop_stack(
        [7.5, 1.2, 0.5],
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        0
    );
    assert_eq!(journal.gameplay().drops.len(), 1);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 250);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        0
    );
    assert!((journal.gameplay().creatures.animals[&npc].position[0] - 2.5).abs() > 0.2);
    assert!(journal.gameplay().jobs.valid());
}

#[test]
fn collector_initializes_a_fresh_empty_chest_only_when_delivering_actual_items() {
    let (mut chunks, mut journal, terrain, mut runtime, _) = fixture(WorkKind::Collector);
    let storage = [0, 1, 0];
    journal.remove_container(storage).unwrap();
    assert_eq!(chunks.block_at(storage), Some(29));
    assert!(journal.container_at(storage).is_none());
    assert!(journal.gameplay_mut().drop_stack(
        [7.5, 1.2, 0.5],
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert!(
        journal.container_at(storage).is_none(),
        "Assignment and remote approach must not initialize an inventory"
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 250);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at(storage)
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 250);
    assert_eq!(
        journal
            .container_at(storage)
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn worker_delivery_preserves_existing_chest_contents_instead_of_reinitializing_them() {
    let (mut chunks, mut journal, terrain, mut runtime, _) = fixture(WorkKind::Collector);
    let storage = [0, 1, 0];
    journal
        .set_container(
            storage,
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(2),
                    count: 17,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(journal.gameplay_mut().drop_stack(
        [7.5, 1.2, 0.5],
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 250);
    let chest = journal.container_at(storage).unwrap();
    assert_eq!(chest.count(ItemKind::Block(2)), 17);
    assert_eq!(chest.count(ItemKind::Block(3)), 7);
}

#[test]
fn miner_fetches_a_real_tool_walks_mines_and_delivers_while_preserving_the_tool() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Miner);
    set(&mut chunks, [5, 1, 0], 3);
    journal
        .set_container(
            [0, 1, 0],
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Tool(ToolKind::WoodPickaxe),
                    count: 1,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    assert_eq!(chunks.block_at([5, 1, 0]), Some(0));
    let chest = journal.container_at([0, 1, 0]).unwrap();
    assert_eq!(chest.count(ItemKind::Block(3)), 1);
    assert_eq!(chest.count(ItemKind::Tool(ToolKind::WoodPickaxe)), 0);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Tool(ToolKind::WoodPickaxe)),
        1
    );
}

#[test]
fn full_storage_retains_collected_items_and_pausing_does_not_discard_the_bag() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    journal
        .set_container(
            [0, 1, 0],
            Inventory {
                slots: vec![
                    ItemStack {
                        item: ItemKind::Block(2),
                        count: 64
                    };
                    18
                ],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(journal.gameplay_mut().drop_stack(
        [7.5, 1.2, 0.5],
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 250);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::StorageFull)
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        7
    );
    journal
        .gameplay_mut()
        .jobs
        .orders
        .get_mut(&1)
        .unwrap()
        .enabled = false;
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Paused
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        7
    );
}

#[test]
fn missing_tools_and_unloaded_storage_report_waits_without_mining_or_creating_items() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Miner);
    set(&mut chunks, [5, 1, 0], 3);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 50);
    assert_eq!(chunks.block_at([5, 1, 0]), Some(3));
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::MissingTool)
    );
    let storage_owner = global_to_section([0, 1, 0]).0;
    chunks.loaded.remove(&storage_owner);
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Unloaded)
    );
    assert!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .slots
            .is_empty()
    );
}

#[test]
fn ordinary_sleep_state_is_not_overwritten_by_the_worker_night_status() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .behavior = CreatureBehavior::Sleep;
    tick(
        &mut chunks,
        &mut journal,
        &terrain,
        &BlockPropertiesCatalog::default(),
        0.95,
        0.1,
        &mut runtime,
    );
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].behavior,
        CreatureBehavior::Sleep
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Night)
    );
}

fn ripe_crop(chunks: &mut ChunkStore, journal: &mut EditJournal) -> terra_core::CropState {
    use crate::props::{PropId, WorldProp};
    let support = [5, 0, 0];
    set(chunks, support, 2);
    let kind = terra_core::CropKind::Wheat;
    let prop = WorldProp {
        id: PropId(1),
        kind: kind.resource_prop().into(),
        position: [5.5, 1.0, 0.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section(support).0,
        removed: false,
    };
    assert!(
        crate::farming::plant_crop(chunks, journal, &BlockPropertiesCatalog::default(), prop)
            .unwrap()
    );
    let mut state = *journal.crop_at(support).unwrap();
    state.progress_seconds = state.growth_duration_seconds;
    crate::farming::commit_crop_state(chunks, journal, support, state).unwrap();
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().area =
        WorkArea::new(support, support).unwrap();
    state
}

#[test]
fn farmer_consumes_real_seed_replants_same_plot_and_delivers_exactly_one_ripe_harvest() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Farmer);
    let crop = ripe_crop(&mut chunks, &mut journal);
    let seed = ItemKind::Crop(crop.kind.seed());
    let produce = ItemKind::Crop(crop.kind.produce());
    journal
        .set_container(
            [0, 1, 0],
            Inventory {
                slots: vec![
                    ItemStack {
                        item: ItemKind::Tool(ToolKind::WoodHoe),
                        count: 1,
                    },
                    ItemStack {
                        item: seed,
                        count: 1,
                    },
                ],
                ..Default::default()
            },
        )
        .unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    let planted = journal.crop_at([5, 0, 0]).unwrap();
    assert_eq!(planted.prop_id, crop.prop_id);
    assert!(!planted.mature());
    assert_eq!(planted.progress_seconds, 0.0);
    assert_eq!(
        journal.container_at([0, 1, 0]).unwrap().count(produce),
        crop.harvest()[0].count
    );
    let stored_seed = journal.container_at([0, 1, 0]).unwrap().count(seed);
    let carried_seed = journal.gameplay().jobs.workers[&npc].inventory.count(seed);
    assert_eq!(stored_seed + carried_seed, crop.harvest()[1].count);
    assert_eq!(carried_seed, 1);
}

#[test]
fn missing_seed_never_removes_a_crop_or_partially_withdraws_a_tool() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Farmer);
    let crop = ripe_crop(&mut chunks, &mut journal);
    let tool = ItemKind::Tool(ToolKind::WoodHoe);
    journal
        .set_container(
            [0, 1, 0],
            Inventory {
                slots: vec![ItemStack {
                    item: tool,
                    count: 1,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 120);
    assert_eq!(journal.crop_at([5, 0, 0]), Some(&crop));
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::MissingMaterial)
    );
    assert_eq!(journal.container_at([0, 1, 0]).unwrap().count(tool), 1);
    assert!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .slots
            .is_empty()
    );
}

#[test]
fn workers_do_not_cross_a_wall_or_mine_recorded_building_blocks() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Miner);
    set(&mut chunks, [5, 1, 0], 3);
    for z in -4..=4 {
        for y in 1..=4 {
            set(&mut chunks, [3, y, z], 3);
        }
    }
    journal
        .set_container(
            [0, 1, 0],
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Tool(ToolKind::WoodPickaxe),
                    count: 1,
                }],
                ..Default::default()
            },
        )
        .unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 150);
    assert_eq!(chunks.block_at([5, 1, 0]), Some(3));
    assert!(journal.gameplay().creatures.animals[&npc].position[0] < 3.0);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoPath),
        "npc {:?}, travel {:?}, route {:?}",
        journal.gameplay().creatures.animals[&npc],
        runtime.travel.get(&npc),
        runtime
            .navigation
            .inspection(&journal.gameplay().creatures.animals[&npc])
    );
    for _ in 0..90 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        assert_eq!(
            journal.gameplay().jobs.workers[&npc].status,
            WorkStatus::Waiting(WaitReason::NoPath),
            "confirmed wall remains blocked throughout bounded retries"
        );
        assert!(journal.gameplay().creatures.animals[&npc].position[0] < 3.0);
    }
    // A real geometry change invalidates the confirmed failure immediately.
    for z in -4..=4 {
        for y in 1..=4 {
            set(&mut chunks, [3, y, z], 0);
        }
    }
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_ne!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoPath)
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    assert_eq!(chunks.block_at([5, 1, 0]), Some(0));
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        1
    );
    set(&mut chunks, [5, 1, 0], 3);
    journal.record([5, 1, 0], 3).unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(chunks.block_at([5, 1, 0]), Some(3));
    assert!(!actions::mineable(&chunks, &journal, [5, 1, 0], 3));
}

#[test]
fn dead_workers_keep_their_real_items_and_dead_behavior() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    let mut worker = WorkerState::default();
    worker.insert(ItemKind::Block(3), 7);
    journal.gameplay_mut().jobs.workers.insert(npc, worker);
    let creature = journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap();
    creature.health = 0.0;
    creature.behavior = CreatureBehavior::Dead;
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        7
    );
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].behavior,
        CreatureBehavior::Dead
    );
    assert!(journal.gameplay().creatures.animals[&npc].valid());
}

#[test]
fn idle_work_advances_route_time_and_falls_when_its_floor_is_removed() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    let before = journal.gameplay().creatures.animals[&npc].clone();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert!(!controls_movement(journal.gameplay(), npc, 0.5));
    set(&mut chunks, [2, 0, 0], 0);
    let residents = crate::creatures::villagers::Residents::default();
    let mut navigation = Navigation::default();
    // NoTargets hands movement back to ordinary residents. Reproduce the
    // server's resident-before-scanner order rather than advancing only scans.
    for _ in 0..20 {
        let mut animal = journal.gameplay().creatures.animals[&npc].clone();
        residents.step_as(
            &mut animal,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal,
            },
            0.5,
            0.1,
            true,
            &mut navigation,
            journal.gameplay().creatures.owners.get(&npc).copied(),
        );
        journal.gameplay_mut().creatures.animals.insert(npc, animal);
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    }
    let after = &journal.gameplay().creatures.animals[&npc];
    assert!(after.age > before.age + 2.0);
    assert!(
        after.position[1] < before.position[1] - 0.5,
        "{:?}",
        after.position
    );
    assert!(after.valid());
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoTargets)
    );
}
