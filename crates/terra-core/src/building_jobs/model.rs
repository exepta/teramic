use super::*;
use crate::{ItemKind, ItemStack, PlayerUuid, kingdom::KingdomId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildArea {
    pub min: [i64; 3],
    pub max: [i64; 3],
}
impl BuildArea {
    pub fn new(a: [i64; 3], b: [i64; 3]) -> Result<Self, BuildError> {
        let area = Self {
            min: std::array::from_fn(|i| a[i].min(b[i])),
            max: std::array::from_fn(|i| a[i].max(b[i])),
        };
        area.valid().then_some(area).ok_or(BuildError::InvalidArea)
    }
    pub fn valid(self) -> bool {
        valid_cell(self.min)
            && valid_cell(self.max)
            && [64_i64, 32, 64]
                .into_iter()
                .enumerate()
                .all(|(i, size)| self.max[i] >= self.min[i] && self.max[i] - self.min[i] < size)
    }
    pub fn contains(self, p: [i64; 3]) -> bool {
        (0..3).all(|i| (self.min[i]..=self.max[i]).contains(&p[i]))
    }
    pub fn volume(self) -> usize {
        if !self.valid() {
            return 0;
        }
        (0..3)
            .map(|i| (self.max[i] - self.min[i] + 1) as usize)
            .product()
    }
    pub fn cell_at(self, index: usize) -> Option<[i64; 3]> {
        if index >= self.volume() {
            return None;
        }
        let x = (self.max[0] - self.min[0] + 1) as usize;
        let z = (self.max[2] - self.min[2] + 1) as usize;
        Some([
            self.min[0] + (index % x) as i64,
            self.min[1] + (index / (x * z)) as i64,
            self.min[2] + ((index / x) % z) as i64,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BuildPhase {
    Foundation,
    Floor,
    Walls,
    Roof,
    Furnishing,
    Cleanup,
}
impl BuildPhase {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Foundation => "Foundation",
            Self::Floor => "Floor",
            Self::Walls => "Walls",
            Self::Roof => "Roof",
            Self::Furnishing => "Furnishing",
            Self::Cleanup => "Scaffold cleanup",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildWait {
    StorageFull,
    NoWorkers,
    MissingStorage,
    MissingMaterials,
    Unloaded,
    Protected,
    NoPath,
    Blocked,
}
impl BuildWait {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NoWorkers => "No craftsmen available",
            Self::StorageFull => "Town storage full",
            Self::MissingStorage => "Town storage required",
            Self::MissingMaterials => "Building materials missing",
            Self::Unloaded => "Construction site unloaded",
            Self::Protected => "Permission denied",
            Self::NoPath => "No reachable construction position",
            Self::Blocked => "Construction site blocked",
        }
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildStatus {
    #[default]
    Queued,
    Travelling,
    Building(BuildPhase),
    Waiting(BuildWait),
    Paused,
    Complete,
    Cancelled,
}
impl BuildStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Travelling => "Travelling",
            Self::Building(phase) => phase.label(),
            Self::Waiting(reason) => reason.label(),
            Self::Paused => "Paused",
            Self::Complete => "Complete",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// Paired openings are grouped in one step and are always committed together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildCell {
    pub position: [i64; 3],
    pub material: u16,
    pub facing: u8,
    pub peer: Option<[i64; 3]>,
    pub secondary: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildStep {
    pub phase: BuildPhase,
    pub cells: Vec<BuildCell>,
    pub costs: Vec<ItemStack>,
}
impl BuildStep {
    pub fn valid(&self) -> bool {
        (1..=2).contains(&self.cells.len())
            && self.costs.len() <= 3
            && self.costs.iter().all(|s| s.valid())
            && self.cells.iter().all(|c| {
                valid_cell(c.position)
                    && !crate::roads::is_tar(c.material)
                    && (c.material == 0 || terra_voxel::MaterialId::from_raw(c.material).is_some())
                    && c.peer.is_none_or(valid_cell)
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildingJob {
    pub id: u64,
    pub kingdom: KingdomId,
    pub owner: PlayerUuid,
    pub origin: [i64; 3],
    pub facing: u8,
    pub template: String,
    pub workers: Vec<u64>,
    pub steps: Vec<BuildStep>,
    pub next_step: usize,
    /// Earned solo-equivalent seconds. A completed house earns 1800 seconds.
    pub work_seconds: f64,
    pub status: BuildStatus,
    pub enabled: bool,
    pub revision: u64,
    /// Granted by the authoritative owner's Creative mode, never by a client request.
    #[serde(default)]
    pub free_resources: bool,
    /// First step funded freely; older paid scaffolds still refund actual wood.
    #[serde(default)]
    pub free_from_step: usize,
}
impl BuildingJob {
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        self.id > 0
            && !self.owner.is_nil()
            && valid_cell(self.origin)
            && self.facing < 4
            && template::valid_template_id(&self.template)
            && self.workers.len() <= MAX_HOUSE_WORKERS
            && self.workers.iter().all(|id| *id > 0 && ids.insert(*id))
            && !self.steps.is_empty()
            && self.steps.len() <= MAX_BUILD_STEPS
            && self.steps.iter().all(BuildStep::valid)
            && self.next_step <= self.steps.len()
            && self.free_from_step <= self.next_step
            && (self.free_resources || self.free_from_step == 0)
            && self.work_seconds.is_finite()
            && (0.0..=1800.01).contains(&self.work_seconds)
    }
    pub fn progress(&self) -> f64 {
        self.next_step as f64 / self.steps.len().max(1) as f64
    }
    pub fn active(&self) -> bool {
        !matches!(self.status, BuildStatus::Complete | BuildStatus::Cancelled)
    }
    pub fn required_materials(&self) -> Vec<ItemStack> {
        let mut counts = BTreeMap::<ItemKind, u32>::new();
        for step in &self.steps[self.next_step.min(self.steps.len())..] {
            for stack in &step.costs {
                *counts.entry(stack.item).or_default() += u32::from(stack.count);
            }
        }
        counts
            .into_iter()
            .flat_map(|(item, mut count)| {
                let mut stacks = Vec::new();
                while count > 0 {
                    let moved = count.min(u32::from(u16::MAX));
                    stacks.push(ItemStack {
                        item,
                        count: moved as u16,
                    });
                    count -= moved;
                }
                stacks
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementBuilding {
    pub owner: PlayerUuid,
    pub storage: Option<BuildArea>,
    pub residential: Option<BuildArea>,
    /// Server catalog ID for future automatic houses; existing jobs retain their own plan.
    #[serde(default = "default_template_id")]
    pub selected_template: String,
    pub craftsmen: Vec<u64>,
}
impl SettlementBuilding {
    pub fn new(owner: PlayerUuid) -> Self {
        Self {
            owner,
            storage: None,
            residential: None,
            selected_template: default_template_id(),
            craftsmen: Vec::new(),
        }
    }
    pub fn valid(&self) -> bool {
        let mut ids = std::collections::BTreeSet::new();
        !self.owner.is_nil()
            && self.storage.is_none_or(BuildArea::valid)
            && self.residential.is_none_or(BuildArea::valid)
            && template::valid_template_id(&self.selected_template)
            && self.craftsmen.len() <= MAX_CRAFTSMEN
            && self.craftsmen.iter().all(|id| *id > 0 && ids.insert(*id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildError {
    InvalidArea,
    InvalidSite,
    MissingKingdom,
    Protected,
    MissingWorker,
    Busy,
    Limit,
    Stale,
    Unloaded,
    Blocked,
    MissingStorage,
    MissingJob,
    InvalidTemplate,
}
impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidArea => "Invalid building area",
                Self::InvalidSite => "Invalid house location",
                Self::MissingKingdom => "Kingdom not found",
                Self::Protected => "Building permission denied",
                Self::MissingWorker => "Craftsman not found",
                Self::Busy => "Craftsman already has work",
                Self::Limit => "Construction limit reached",
                Self::Stale => "Construction changed; refresh",
                Self::Unloaded => "Construction area unloaded",
                Self::Blocked => "House footprint is occupied",
                Self::MissingStorage => "Choose town storage first",
                Self::MissingJob => "Building job not found",
                Self::InvalidTemplate => "Invalid house template",
            }
        )
    }
}
impl std::error::Error for BuildError {}
