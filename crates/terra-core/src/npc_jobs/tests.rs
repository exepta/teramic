use super::*;
use crate::{ItemKind, ItemStack, PlayerUuid};

fn order(id: u64, npc: u64, priority: u8) -> WorkOrder {
    WorkOrder {
        id,
        npc,
        owner: PlayerUuid::from_u128(1),
        kind: WorkKind::Miner,
        area: WorkArea::new([-1, 4, -1], [2, 7, 2]).unwrap(),
        storage: [4, 4, 4],
        priority,
        enabled: true,
        revision: 0,
    }
}

#[test]
fn work_areas_are_inclusive_normalized_negative_safe_and_bounded() {
    let area = WorkArea::new([2, 7, 2], [-1, 4, -1]).unwrap();
    assert_eq!(area.min, [-1, 4, -1]);
    assert_eq!(area.max, [2, 7, 2]);
    assert_eq!(area.cells().count(), 64);
    assert!(area.contains([-1, 4, -1]));
    assert!(!area.contains([-2, 4, -1]));
    assert_eq!(area.cells().next(), Some([-1, 4, -1]));
    assert_eq!(area.cells().last(), Some([2, 7, 2]));
    assert_eq!(area.volume(), 64);
    for (index, cell) in area.cells().enumerate() {
        assert_eq!(area.cell_at(index), Some(cell));
    }
    assert_eq!(area.cell_at(64), None);
    assert!(WorkArea::new([0; 3], [31, 15, 31]).is_ok());
    assert!(WorkArea::new([0; 3], [32, 15, 31]).is_err());
    assert!(WorkArea::new([0; 3], [31, 16, 31]).is_err());
    assert!(WorkArea::new([i64::MIN; 3], [i64::MAX; 3]).is_err());
    assert_eq!(
        WorkArea {
            min: [0; 3],
            max: [i64::MAX; 3]
        }
        .cells()
        .count(),
        0
    );
}

#[test]
fn order_priority_is_stable_and_disabled_orders_do_not_run() {
    let mut jobs = WorkRegistry {
        next_id: 5,
        ..Default::default()
    };
    for (id, priority) in [(4, 5), (2, 9), (1, 5), (3, 0)] {
        jobs.orders.insert(id, order(id, id, priority));
    }
    jobs.orders.get_mut(&2).unwrap().enabled = false;
    assert!(jobs.valid());
    assert_eq!(jobs.ordered_orders(), [1, 4, 3]);
}

#[test]
fn targets_are_reserved_once_but_storage_is_shareable_and_release_keeps_items() {
    let mut jobs = WorkRegistry {
        next_id: 3,
        ..Default::default()
    };
    jobs.orders.insert(1, order(1, 12, 5));
    jobs.orders.insert(2, order(2, 13, 5));
    let target = WorkTarget::Block([-1, 4, -1]);
    assert!(jobs.reserve(12, target));
    assert!(jobs.reserve(12, target));
    assert!(!jobs.reserve(13, target));
    assert_eq!(jobs.reservation_owner(&target), Some(12));
    assert_eq!(
        jobs.workers
            .get_mut(&12)
            .unwrap()
            .insert(ItemKind::Block(3), 7),
        0
    );
    jobs.release(12);
    assert!(jobs.reserve(13, target));
    for npc in [12, 13] {
        assert!(jobs.reserve(npc, WorkTarget::Storage([4, 4, 4])));
    }
    assert_eq!(
        jobs.reservation_owner(&WorkTarget::Storage([4, 4, 4])),
        None
    );
    assert_eq!(jobs.workers[&12].inventory.count(ItemKind::Block(3)), 7);
    assert!(jobs.valid());
}

#[test]
fn worker_bags_merge_real_metadata_without_growing_beyond_nine_slots() {
    let mut worker = WorkerState::default();
    let item = ItemKind::for_wood(28, crate::WoodSpecies::Cherry);
    assert_eq!(worker.insert(item, item.max_stack() + 1), 0);
    assert_eq!(
        worker.inventory.slot(0),
        Some(ItemStack {
            item,
            count: item.max_stack()
        })
    );
    assert_eq!(worker.inventory.slot(1), Some(ItemStack { item, count: 1 }));
    worker.inventory.slots[0] = ItemStack::EMPTY;
    assert_eq!(worker.insert(ItemKind::Block(3), 1), 0);
    assert_eq!(worker.inventory.slot(1), Some(ItemStack { item, count: 1 }));
    let stone = ItemKind::Block(3);
    let free = (WORKER_SLOTS as u16 - 1) * stone.max_stack() - 1;
    let remainder = worker.insert(stone, free + 7);
    assert_eq!(remainder, 7);
    assert_eq!(worker.inventory.slots.len(), WORKER_SLOTS);
    assert!(worker.valid());
}

#[test]
fn invalid_worker_inputs_and_revision_overflow_are_atomic() {
    let mut worker = WorkerState::default();
    assert_eq!(worker.insert(ItemKind::Block(0), 2), 2);
    worker.inventory.revision = u64::MAX;
    let before = worker.clone();
    assert_eq!(worker.insert(ItemKind::Block(3), 2), 2);
    assert_eq!(worker, before);
    worker.work_seconds = f32::NAN;
    assert!(!worker.valid());
}

#[test]
fn persisted_jobs_validate_ids_owners_bounds_and_exclusive_targets() {
    let mut jobs = WorkRegistry {
        next_id: 3,
        ..Default::default()
    };
    jobs.orders.insert(1, order(1, 12, 5));
    jobs.orders.insert(2, order(2, 13, 5));
    jobs.reserve(12, WorkTarget::Drop(7));
    jobs.workers.insert(13, jobs.workers[&12].clone());
    assert!(!jobs.valid());
    jobs.release(13);
    assert!(jobs.valid());
    let roundtrip: WorkRegistry =
        serde_json::from_str(&serde_json::to_string(&jobs).unwrap()).unwrap();
    assert_eq!(roundtrip, jobs);
    assert!(roundtrip.valid());
    jobs.orders.get_mut(&1).unwrap().priority = 10;
    assert!(!jobs.valid());
    jobs.orders.get_mut(&1).unwrap().priority = 9;
    jobs.orders.get_mut(&1).unwrap().owner = PlayerUuid::nil();
    assert!(!jobs.valid());
}

#[test]
fn worn_axes_are_individual_valid_items_and_tree_reservations_survive_delivery() {
    let tool = crate::ToolKind::IronAxe;
    let worn = ItemKind::WornTool {
        tool,
        remaining: tool.durability() - 1,
    };
    assert!(worn.valid());
    assert_eq!(worn.max_stack(), 1);
    assert_eq!(worn.tool_kind(), Some(tool));
    assert_eq!(worn.preview_item(), ItemKind::Tool(tool));
    assert!(!ItemKind::WornTool { tool, remaining: 0 }.valid());
    assert!(
        !ItemKind::WornTool {
            tool: crate::ToolKind::IronPickaxe,
            remaining: 1
        }
        .valid()
    );
    let mut jobs = WorkRegistry {
        next_id: 3,
        ..Default::default()
    };
    jobs.orders.insert(1, order(1, 12, 5));
    jobs.orders.insert(2, order(2, 13, 5));
    let tree = TreeWork {
        root: [0, 4, 0],
        foot: [0, 4, 0],
        species: crate::TreeSpecies::Oak,
        planted: None,
        wood: vec![[0, 4, 0]],
        leaves: vec![],
        drops: vec![],
        stage: TreeWorkStage::Cutting,
        sapling_loaded: false,
        sapling_dropped: false,
        started: false,
    };
    jobs.workers.insert(
        12,
        WorkerState {
            tree: Some(tree),
            target: Some(WorkTarget::Storage([4, 4, 4])),
            ..Default::default()
        },
    );
    assert_eq!(
        jobs.reservation_owner(&WorkTarget::Tree([0, 4, 0])),
        Some(12)
    );
    assert!(!jobs.reserve(13, WorkTarget::Tree([0, 4, 0])));
    let encoded = serde_json::to_string(&jobs).unwrap();
    assert_eq!(
        serde_json::from_str::<WorkRegistry>(&encoded).unwrap(),
        jobs
    );
    assert!(jobs.valid());
}
