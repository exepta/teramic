//! Frozen ID24 layout: postcard cannot default an appended positional field.
use super::*;
use terra_core::building_jobs::{
    BuildingJob, BuildingRegistry, HOUSE_TEMPLATE, SettlementBuilding,
};

#[derive(Serialize, Deserialize)]
pub(super) struct BuildingIndex24 {
    pub prior: StarterIndex,
    pub buildings: BuildingRegistry24,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SettlementBuilding24 {
    pub owner: PlayerUuid,
    pub storage: Option<terra_core::building_jobs::BuildArea>,
    pub residential: Option<terra_core::building_jobs::BuildArea>,
    pub craftsmen: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct BuildingRegistry24 {
    pub revision: u64,
    pub next_id: u64,
    pub settlements: BTreeMap<terra_core::kingdom::KingdomId, SettlementBuilding24>,
    pub jobs: BTreeMap<u64, BuildingJob25>,
    pub inventories: BTreeMap<u64, Inventory>,
}

impl BuildingIndex24 {
    pub(super) fn migrate(self) -> Result<BuildingIndex, String> {
        if self
            .buildings
            .jobs
            .values()
            .any(|job| job.template != HOUSE_TEMPLATE)
        {
            return Err("Invalid legacy construction template ID".into());
        }
        Ok(BuildingIndex {
            prior: self.prior,
            buildings: BuildingRegistry {
                revision: self.buildings.revision,
                next_id: self.buildings.next_id,
                settlements: self
                    .buildings
                    .settlements
                    .into_iter()
                    .map(|(id, old)| {
                        (
                            id,
                            SettlementBuilding {
                                owner: old.owner,
                                storage: old.storage,
                                residential: old.residential,
                                craftsmen: old.craftsmen,
                                selected_template: HOUSE_TEMPLATE.into(),
                            },
                        )
                    })
                    .collect(),
                jobs: self
                    .buildings
                    .jobs
                    .into_iter()
                    .map(|(id, job)| (id, job.migrate()))
                    .collect(),
                inventories: self.buildings.inventories,
            },
        })
    }
}

// Frozen ID25 job layout: funding policy was introduced only in ID26.
#[derive(Serialize, Deserialize)]
pub(super) struct BuildingJob25 {
    pub id: u64,
    pub kingdom: terra_core::kingdom::KingdomId,
    pub owner: PlayerUuid,
    pub origin: [i64; 3],
    pub facing: u8,
    pub template: String,
    pub workers: Vec<u64>,
    pub steps: Vec<terra_core::building_jobs::BuildStep>,
    pub next_step: usize,
    pub work_seconds: f64,
    pub status: terra_core::building_jobs::BuildStatus,
    pub enabled: bool,
    pub revision: u64,
}
impl BuildingJob25 {
    fn migrate(self) -> BuildingJob {
        BuildingJob {
            id: self.id,
            kingdom: self.kingdom,
            owner: self.owner,
            origin: self.origin,
            facing: self.facing,
            template: self.template,
            workers: self.workers,
            steps: self.steps,
            next_step: self.next_step,
            work_seconds: self.work_seconds,
            status: self.status,
            enabled: self.enabled,
            revision: self.revision,
            free_resources: false,
            free_from_step: 0,
        }
    }
}
impl From<BuildingJob> for BuildingJob25 {
    fn from(job: BuildingJob) -> Self {
        Self {
            id: job.id,
            kingdom: job.kingdom,
            owner: job.owner,
            origin: job.origin,
            facing: job.facing,
            template: job.template,
            workers: job.workers,
            steps: job.steps,
            next_step: job.next_step,
            work_seconds: job.work_seconds,
            status: job.status,
            enabled: job.enabled,
            revision: job.revision,
        }
    }
}
#[derive(Serialize, Deserialize)]
pub(super) struct BuildingIndex25 {
    pub prior: StarterIndex,
    pub buildings: BuildingRegistry25,
}
#[derive(Serialize, Deserialize)]
pub(super) struct BuildingRegistry25 {
    pub revision: u64,
    pub next_id: u64,
    pub settlements: BTreeMap<terra_core::kingdom::KingdomId, SettlementBuilding>,
    pub jobs: BTreeMap<u64, BuildingJob25>,
    pub inventories: BTreeMap<u64, Inventory>,
}
impl BuildingIndex25 {
    pub(super) fn migrate(self) -> BuildingIndex {
        BuildingIndex {
            prior: self.prior,
            buildings: BuildingRegistry {
                revision: self.buildings.revision,
                next_id: self.buildings.next_id,
                settlements: self.buildings.settlements,
                jobs: self
                    .buildings
                    .jobs
                    .into_iter()
                    .map(|(id, job)| (id, job.migrate()))
                    .collect(),
                inventories: self.buildings.inventories,
            },
        }
    }
}
