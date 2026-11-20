//! Owner-authorized local settlement construction; clients never place job voxels.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    ItemStack, PlayerUuid,
    building_jobs::{
        BuildArea, BuildError, BuildStatus, BuildingJob, MAX_BUILD_JOBS, MAX_HOUSE_WORKERS,
        MAX_SETTLEMENTS, SettlementBuilding, TemplatePreview, TownStock,
        template::{MAX_TEMPLATES, TemplateSummary, valid_template_id},
    },
    kingdom::KingdomId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingAction {
    Inspect {
        kingdom: KingdomId,
    },
    Preview {
        kingdom: KingdomId,
        template: String,
    },
    ReloadCatalog {
        kingdom: KingdomId,
    },
    SetStorage {
        kingdom: KingdomId,
        area: BuildArea,
    },
    SetResidential {
        kingdom: KingdomId,
        area: Option<BuildArea>,
        template: String,
    },
    AssignCraftsman {
        kingdom: KingdomId,
        npc: u64,
        enabled: bool,
    },
    QueueHouse {
        kingdom: KingdomId,
        origin: [i64; 3],
        facing: u8,
        template: String,
    },
    SetEnabled {
        id: u64,
        revision: u64,
        enabled: bool,
    },
    Cancel {
        id: u64,
        revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingCommand {
    pub request: u64,
    pub action: BuildingAction,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildingSnapshot {
    pub revision: u64,
    /// Validated server assets. Materials are permanent authored inputs; an
    /// actual site's job demand additionally includes its paid scaffolding.
    pub templates: Vec<TemplateSummary>,
    /// A server asset failure is visible without pretending a template exists.
    pub catalog_error: Option<String>,
    pub settlements: BTreeMap<KingdomId, SettlementBuilding>,
    pub jobs: BTreeMap<u64, BuildingJobView>,
    pub stock_kingdom: Option<KingdomId>,
    pub stock: TownStock,
    pub unlimited_resources: bool,
    pub preview: Option<TemplatePreview>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingJobView {
    pub id: u64,
    pub kingdom: KingdomId,
    pub owner: PlayerUuid,
    pub origin: [i64; 3],
    pub facing: u8,
    pub template: String,
    pub workers: Vec<u64>,
    pub progress: f64,
    pub required_materials: Vec<ItemStack>,
    pub status: BuildStatus,
    pub enabled: bool,
    pub revision: u64,
    pub work_seconds: f64,
    pub free_resources: bool,
    pub remaining_seconds: f64,
}
impl From<&BuildingJob> for BuildingJobView {
    fn from(job: &BuildingJob) -> Self {
        let duration =
            terra_core::building_jobs::house_seconds(job.workers.len()).unwrap_or(1800.0);
        Self {
            id: job.id,
            kingdom: job.kingdom,
            owner: job.owner,
            origin: job.origin,
            facing: job.facing,
            template: job.template.clone(),
            workers: job.workers.clone(),
            progress: job.progress(),
            required_materials: job.required_materials(),
            status: job.status,
            enabled: job.enabled,
            revision: job.revision,
            work_seconds: job.work_seconds,
            free_resources: job.free_resources,
            remaining_seconds: (1800.0 - job.work_seconds).max(0.0) * duration / 1800.0,
        }
    }
}
impl BuildingJobView {
    pub fn progress(&self) -> f64 {
        self.progress
    }
    pub fn required_materials(&self) -> Vec<ItemStack> {
        self.required_materials.clone()
    }
    pub fn active(&self) -> bool {
        !matches!(self.status, BuildStatus::Complete | BuildStatus::Cancelled)
    }
}
impl BuildingSnapshot {
    pub fn template(&self, id: &str) -> Option<&TemplateSummary> {
        self.templates.iter().find(|template| template.id == id)
    }
    pub fn is_craftsman(&self, npc: u64) -> bool {
        self.settlements
            .values()
            .any(|s| s.craftsmen.contains(&npc))
    }
    pub fn valid(&self) -> bool {
        let mut workers = std::collections::BTreeSet::new();
        let mut templates = BTreeSet::new();
        self.templates.len() <= MAX_TEMPLATES
            && self.stock.valid()
            && (self.stock_kingdom.is_some()
                || self.stock == TownStock::default()
                    && !self.unlimited_resources
                    && self.preview.is_none())
            && self
                .stock_kingdom
                .is_none_or(|id| self.settlements.contains_key(&id))
            && self.preview.as_ref().is_none_or(TemplatePreview::valid)
            && self
                .templates
                .iter()
                .all(|template| template.valid() && templates.insert(&template.id))
            && self
                .catalog_error
                .as_ref()
                .is_none_or(|error| !error.is_empty() && error.len() <= 512)
            && self.settlements.len() <= MAX_SETTLEMENTS
            && self.settlements.values().all(SettlementBuilding::valid)
            && self.jobs.len() <= MAX_BUILD_JOBS
            && self.jobs.iter().all(|(id, j)| {
                *id == j.id
                    && j.id > 0
                    && self.settlements.contains_key(&j.kingdom)
                    // Saved jobs own their resolved steps. Removing an asset
                    // must not invalidate an already authorized project's view.
                    && valid_template_id(&j.template)
                    && j.facing < 4
                    && !j.owner.is_nil()
                    && j.origin.iter().all(|v| v.unsigned_abs() <= 999_000)
                    && j.workers.len() <= MAX_HOUSE_WORKERS
                    && j.workers.iter().all(|id| *id > 0)
                    && (!j.active() || j.workers.iter().all(|id| workers.insert(*id)))
                    && j.progress.is_finite()
                    && (0.0..=1.0).contains(&j.progress)
                    && j.work_seconds.is_finite()
                    && (0.0..=1800.01).contains(&j.work_seconds)
                    && j.remaining_seconds.is_finite()
                    && (0.0..=1800.01).contains(&j.remaining_seconds)
                    && j.required_materials.len() <= 256
                    && j.required_materials
                        .iter()
                        .all(|s| s.item.valid() && s.count > 0)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildingResult {
    pub request: u64,
    pub error: Option<BuildError>,
}

#[cfg(test)]
#[path = "building_jobs/tests.rs"]
mod tests;
