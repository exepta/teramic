//! Frozen ID21 worker layout, before persistent woodcutting transactions.
use super::TrainIndex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use terra_core::npc_jobs::{WorkOrder, WorkRegistry, WorkStatus, WorkTarget, WorkerState};

#[derive(Serialize, Deserialize)]
pub(super) struct WorkIndex21 {
    pub prior: TrainIndex,
    pub jobs: WorkRegistry21,
}

#[derive(Serialize, Deserialize)]
pub(super) struct WorkRegistry21 {
    pub revision: u64,
    pub next_id: u64,
    pub orders: BTreeMap<u64, WorkOrder>,
    pub workers: BTreeMap<u64, WorkerState21>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct WorkerState21 {
    pub inventory: terra_core::Inventory,
    pub status: WorkStatus,
    pub target: Option<WorkTarget>,
    pub work_seconds: f32,
}

impl From<WorkRegistry21> for WorkRegistry {
    fn from(old: WorkRegistry21) -> Self {
        Self {
            revision: old.revision,
            next_id: old.next_id,
            orders: old.orders,
            workers: old
                .workers
                .into_iter()
                .map(|(id, worker)| {
                    (
                        id,
                        WorkerState {
                            inventory: worker.inventory,
                            status: worker.status,
                            target: worker.target,
                            work_seconds: worker.work_seconds,
                            tree: None,
                        },
                    )
                })
                .collect(),
        }
    }
}
