use super::{MAX_WORK_ORDERS, MAX_WORKERS, WorkOrder, WorkTarget, WorkerState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkRegistry {
    pub revision: u64,
    pub next_id: u64,
    pub orders: BTreeMap<u64, WorkOrder>,
    pub workers: BTreeMap<u64, WorkerState>,
}

impl Default for WorkRegistry {
    fn default() -> Self {
        Self {
            revision: 0,
            next_id: 1,
            orders: BTreeMap::new(),
            workers: BTreeMap::new(),
        }
    }
}

impl WorkRegistry {
    pub fn valid(&self) -> bool {
        let mut targets = BTreeSet::new();
        let mut npcs = BTreeSet::new();
        let mut trees = BTreeSet::new();
        self.next_id > 0
            && self.orders.len() <= MAX_WORK_ORDERS
            && self.workers.len() <= MAX_WORKERS
            && self.orders.iter().all(|(id, order)| {
                *id == order.id && *id < self.next_id && order.valid() && npcs.insert(order.npc)
            })
            && self.workers.iter().all(|(npc, worker)| {
                *npc > 0
                    && npcs.contains(npc)
                    && worker.valid()
                    && worker
                        .tree
                        .as_ref()
                        .is_none_or(|tree| trees.insert(tree.root))
                    && worker
                        .target
                        .is_none_or(|target| !target.exclusive() || targets.insert(target))
            })
    }

    /// Higher priority first, oldest ID for ties. Never depends on map insertion order.
    pub fn ordered_orders(&self) -> Vec<u64> {
        let mut orders: Vec<_> = self.orders.values().filter(|order| order.enabled).collect();
        orders.sort_by_key(|order| (std::cmp::Reverse(order.priority), order.id));
        orders.into_iter().map(|order| order.id).collect()
    }

    pub fn reservation_owner(&self, target: &WorkTarget) -> Option<u64> {
        target.exclusive().then(|| {
            self.workers
                .iter()
                .find_map(|(npc, worker)| (worker.target.as_ref() == Some(target)
                    || matches!(target, WorkTarget::Tree(root) if worker.tree.as_ref().is_some_and(|tree| tree.root == *root)))
                    .then_some(*npc))
        })?
    }

    pub fn reserve(&mut self, npc: u64, target: WorkTarget) -> bool {
        if npc == 0
            || !self
                .orders
                .values()
                .any(|order| order.npc == npc && order.enabled)
            || !target.valid()
            || self
                .reservation_owner(&target)
                .is_some_and(|owner| owner != npc)
            || (!self.workers.contains_key(&npc) && self.workers.len() >= MAX_WORKERS)
        {
            return false;
        }
        self.workers.entry(npc).or_default().target = Some(target);
        true
    }

    pub fn release(&mut self, npc: u64) {
        if let Some(worker) = self.workers.get_mut(&npc) {
            worker.target = None;
            worker.work_seconds = 0.0;
        }
    }
}
