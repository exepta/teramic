use super::*;
use terra_core::npc_jobs::{WorkArea, WorkKind, WorkOrder, WorkStatus, WorkerState};

#[test]
fn work_orders_carried_tools_and_waiting_state_survive_reload_without_changing_legacy_saves() {
    let root = std::env::temp_dir().join(format!(
        "terra-work-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(21158);
    let mut journal = EditJournal::default();
    let jobs = &mut journal.gameplay_mut().jobs;
    let owner = PlayerUuid::from_u128(21);
    jobs.next_id = 2;
    jobs.orders.insert(
        1,
        WorkOrder {
            id: 1,
            npc: 99,
            owner,
            kind: WorkKind::Miner,
            area: WorkArea::new([-17, 8, -17], [-1, 9, -1]).unwrap(),
            storage: [-20, 8, -20],
            priority: 9,
            enabled: false,
            revision: 3,
        },
    );
    let mut worker = WorkerState {
        status: WorkStatus::Paused,
        ..Default::default()
    };
    assert_eq!(
        worker.insert(
            terra_core::ItemKind::Tool(terra_core::ToolKind::IronPickaxe),
            1
        ),
        0
    );
    assert_eq!(worker.insert(terra_core::ItemKind::Block(3), 42), 0);
    jobs.workers.insert(99, worker);
    assert!(journal.gameplay().valid());
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID22".as_slice())
    );
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().jobs, journal.gameplay().jobs);
    assert!(loaded.gameplay().valid());
    assert_eq!(
        loaded.gameplay().jobs.workers[&99]
            .inventory
            .count(terra_core::ItemKind::Block(3)),
        42
    );
    // Old ID21 files carry four worker fields. Defaults alone cannot migrate
    // bincode sequence records: exercise the frozen decoder with a real worker.
    let current: WorkIndex = decode(b"ID22", &read(&root.join(INDEX)).unwrap()).unwrap();
    let old = work_legacy::WorkIndex21 {
        prior: current.prior,
        jobs: work_legacy::WorkRegistry21 {
            revision: current.jobs.revision,
            next_id: current.jobs.next_id,
            orders: current.jobs.orders,
            workers: current
                .jobs
                .workers
                .into_iter()
                .map(|(id, worker)| {
                    (
                        id,
                        work_legacy::WorkerState21 {
                            inventory: worker.inventory,
                            status: worker.status,
                            target: worker.target,
                            work_seconds: worker.work_seconds,
                        },
                    )
                })
                .collect(),
        },
    };
    // Deliberately replace the already saved fixture. Production write() uses
    // create_new for transaction staging and correctly rejects this existing file.
    fs::write(root.join(INDEX), encode(b"ID21", &old).unwrap()).unwrap();
    let migrated = load_edits(&root, world).unwrap();
    assert_eq!(migrated.gameplay().jobs, loaded.gameplay().jobs);
    assert!(migrated.gameplay().jobs.workers[&99].tree.is_none());
    // New resumable work and individual tool wear survive a second binary save.
    let mut resumed = migrated;
    let worker = resumed.gameplay_mut().jobs.workers.get_mut(&99).unwrap();
    assert_eq!(
        worker.insert(
            terra_core::ItemKind::WornTool {
                tool: terra_core::ToolKind::IronAxe,
                remaining: 7
            },
            1
        ),
        0
    );
    worker.tree = Some(terra_core::npc_jobs::TreeWork {
        root: [-1, 8, -1],
        foot: [-1, 8, -1],
        species: terra_core::TreeSpecies::Birch,
        planted: None,
        wood: vec![[-1, 9, -1]],
        leaves: vec![[-1, 10, -1]],
        drops: vec![],
        stage: terra_core::npc_jobs::TreeWorkStage::Cutting,
        started: true,
        sapling_loaded: false,
        sapling_dropped: true,
    });
    save_edits(&root, world, &resumed).unwrap();
    let restored = load_edits(&root, world).unwrap();
    assert_eq!(restored.gameplay().jobs, resumed.gameplay().jobs);
    fs::remove_dir_all(root).unwrap();
}
