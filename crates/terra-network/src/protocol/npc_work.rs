//! Owner-authorized work orders; storage contents never travel in public snapshots.
use serde::{Deserialize, Serialize};
use terra_core::npc_jobs::{WorkArea, WorkKind, WorkOrder, WorkStatus};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorkAction {
    Inspect,
    /// Private, nearby inspection. The server rechecks this subscription on every snapshot.
    InspectNpc {
        npc: u64,
    },
    Assign {
        npc: u64,
        kind: WorkKind,
        area: WorkArea,
        storage: [i64; 3],
        priority: u8,
    },
    SetEnabled {
        id: u64,
        revision: u64,
        enabled: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkCommand {
    pub request: u64,
    pub action: WorkAction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerView {
    pub npc: u64,
    pub name: String,
    pub position: [f64; 3],
    pub order: Option<WorkOrder>,
    pub status: WorkStatus,
    pub carried_slots: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkSnapshot {
    pub revision: u64,
    pub workers: Vec<WorkerView>,
    pub detail: Option<WorkerDetail>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkerDetail {
    pub npc: u64,
    pub inventory: terra_core::Inventory,
}

impl WorkSnapshot {
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        self.detail.as_ref().is_none_or(|detail| {
            detail.npc != 0
                && self.workers.iter().any(|worker| worker.npc == detail.npc)
                && detail.inventory.valid()
                && detail.inventory.slots.len() <= terra_core::npc_jobs::WORKER_SLOTS
                && detail.inventory.cursor.is_none()
        }) && self.workers.len() <= 128
            && self.workers.iter().all(|worker| {
                worker.npc != 0
                    && ids.insert(worker.npc)
                    && worker.name.len() <= 64
                    && worker
                        .position
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
                    && worker.carried_slots <= 9
                    && worker
                        .order
                        .as_ref()
                        .is_none_or(|order| order.valid() && order.npc == worker.npc)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkError {
    Invalid,
    Protected,
    Missing,
    Busy,
    Stale,
    Unloaded,
    Storage,
    Limit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkResult {
    pub request: u64,
    pub error: Option<WorkError>,
}
