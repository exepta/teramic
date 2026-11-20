use super::*;
use crate::kingdom::KingdomId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuildingRegistry {
    pub revision: u64,
    pub next_id: u64,
    pub settlements: BTreeMap<KingdomId, SettlementBuilding>,
    pub jobs: BTreeMap<u64, BuildingJob>,
    pub inventories: BTreeMap<u64, crate::Inventory>,
}
impl Default for BuildingRegistry {
    fn default() -> Self {
        Self {
            revision: 0,
            next_id: 1,
            settlements: BTreeMap::new(),
            jobs: BTreeMap::new(),
            inventories: BTreeMap::new(),
        }
    }
}
impl BuildingRegistry {
    pub fn valid(&self) -> bool {
        let mut workers = BTreeSet::new();
        let mut roster = BTreeSet::new();
        self.next_id > 0
            && self.inventories.len() <= MAX_SETTLEMENTS * MAX_CRAFTSMEN
            && self.inventories.iter().all(|(id, bag)| {
                *id > 0
                    && bag.valid()
                    && bag.slots.len() <= crate::npc_jobs::WORKER_SLOTS
                    && bag.cursor.is_none()
            })
            && self.settlements.len() <= MAX_SETTLEMENTS
            && self.jobs.len() <= MAX_BUILD_JOBS
            && self
                .settlements
                .values()
                .all(|s| s.valid() && s.craftsmen.iter().all(|id| roster.insert(*id)))
            && self.jobs.iter().all(|(id, job)| {
                *id == job.id
                    && *id < self.next_id
                    && job.valid()
                    && self
                        .settlements
                        .get(&job.kingdom)
                        .is_some_and(|s| s.owner == job.owner)
                    && (!job.active() || job.workers.iter().all(|id| workers.insert(*id)))
            })
    }
    pub fn inventory(&self, npc: u64) -> Option<&crate::Inventory> {
        self.inventories.get(&npc)
    }
    pub fn is_busy(&self, npc: u64) -> bool {
        self.jobs
            .values()
            .any(|j| j.active() && j.enabled && j.workers.contains(&npc))
            || self.is_craftsman(npc)
                && self.inventory(npc).is_some_and(|b| b.occupied_slots() > 0)
                && !self
                    .jobs
                    .values()
                    .any(|j| j.active() && j.workers.contains(&npc))
    }
    pub fn is_craftsman(&self, npc: u64) -> bool {
        self.settlements
            .values()
            .any(|s| s.craftsmen.contains(&npc))
    }
    pub fn insert(&mut self, mut job: BuildingJob) -> Result<u64, BuildError> {
        if self.jobs.len() >= MAX_BUILD_JOBS || self.revision == u64::MAX {
            return Err(BuildError::Limit);
        }
        let id = self.next_id;
        let next = id.checked_add(1).ok_or(BuildError::Limit)?;
        job.id = id;
        if !job.valid() {
            return Err(BuildError::InvalidSite);
        }
        self.jobs.insert(id, job);
        self.next_id = next;
        self.revision += 1;
        Ok(id)
    }
}
