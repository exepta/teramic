use super::{MAX_PRIORITY, WORKER_SLOTS, valid_cell};
use crate::{Inventory, ItemKind, ItemStack, PlayerUuid, ToolKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkKind {
    Miner,
    Farmer,
    Collector,
    Lumberjack,
}

impl WorkKind {
    pub const ALL: [Self; 4] = [Self::Miner, Self::Farmer, Self::Collector, Self::Lumberjack];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Miner => "Miner",
            Self::Farmer => "Farmer",
            Self::Collector => "Collector",
            Self::Lumberjack => "Lumberjack",
        }
    }

    pub fn accepts_tool(self, tool: ToolKind) -> bool {
        match self {
            Self::Miner => tool.class() == Some(crate::block_properties::ToolClass::Pickaxe),
            Self::Farmer => matches!(
                tool,
                ToolKind::WoodHoe
                    | ToolKind::IronHoe
                    | ToolKind::GoldHoe
                    | ToolKind::DiamondHoe
                    | ToolKind::RubyHoe
            ),
            Self::Collector => false,
            Self::Lumberjack => tool.durability() > 0,
        }
    }
}

/// Inclusive voxel bounds. No chunk alignment or positive-coordinate assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkArea {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

impl WorkArea {
    pub fn new(a: [i64; 3], b: [i64; 3]) -> Result<Self, &'static str> {
        let area = Self {
            min: std::array::from_fn(|axis| a[axis].min(b[axis])),
            max: std::array::from_fn(|axis| a[axis].max(b[axis])),
        };
        area.valid()
            .then_some(area)
            .ok_or("Work areas must fit within 32 x 16 x 32 blocks and world coordinates")
    }

    pub fn valid(self) -> bool {
        valid_cell(self.min)
            && valid_cell(self.max)
            && [32_i64, 16, 32]
                .into_iter()
                .enumerate()
                .all(|(axis, size)| {
                    self.max[axis] >= self.min[axis] && self.max[axis] - self.min[axis] < size
                })
    }

    pub fn contains(self, cell: [i64; 3]) -> bool {
        (0..3).all(|axis| (self.min[axis]..=self.max[axis]).contains(&cell[axis]))
    }

    pub fn volume(self) -> usize {
        if !self.valid() {
            return 0;
        }
        (0..3)
            .map(|axis| (self.max[axis] - self.min[axis] + 1) as usize)
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

    /// Stable Y/Z/X scan order, suitable for a bounded resume cursor.
    pub fn cells(self) -> impl Iterator<Item = [i64; 3]> {
        let valid = self.valid();
        (self.min[1]..=self.max[1])
            .take(if valid { 16 } else { 0 })
            .flat_map(move |y| {
                (self.min[2]..=self.max[2])
                    .flat_map(move |z| (self.min[0]..=self.max[0]).map(move |x| [x, y, z]))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    pub id: u64,
    pub npc: u64,
    pub owner: PlayerUuid,
    pub kind: WorkKind,
    pub area: WorkArea,
    pub storage: [i64; 3],
    pub priority: u8,
    pub enabled: bool,
    pub revision: u64,
}

impl WorkOrder {
    pub fn valid(&self) -> bool {
        self.id > 0
            && self.npc > 0
            && !self.owner.is_nil()
            && self.area.valid()
            && valid_cell(self.storage)
            && self.priority <= MAX_PRIORITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkTarget {
    Block([i64; 3]),
    Prop { position: [i64; 3], id: u64 },
    Drop(u64),
    Storage([i64; 3]),
    Tree([i64; 3]),
}

impl WorkTarget {
    pub fn valid(self) -> bool {
        match self {
            Self::Block(cell) | Self::Storage(cell) | Self::Tree(cell) => valid_cell(cell),
            Self::Prop { position, id } => id > 0 && valid_cell(position),
            Self::Drop(id) => id > 0,
        }
    }

    pub fn exclusive(self) -> bool {
        !matches!(self, Self::Storage(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaitReason {
    MissingTool,
    MissingMaterial,
    StorageFull,
    NoPath,
    Unloaded,
    Protected,
    NoTargets,
    Night,
    NoPlantingSite,
}

impl WaitReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingTool => "Missing tool",
            Self::MissingMaterial => "Missing material",
            Self::StorageFull => "Storage full",
            Self::NoPath => "No reachable path",
            Self::Unloaded => "Work area or storage unloaded",
            Self::Protected => "Permission denied",
            Self::NoTargets => "No work available",
            Self::Night => "Resting at night",
            Self::NoPlantingSite => "No suitable planting site",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkStatus {
    #[default]
    Idle,
    Travelling,
    Working,
    Delivering,
    Waiting(WaitReason),
    Paused,
}

impl WorkStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Travelling => "Travelling",
            Self::Working => "Working",
            Self::Delivering => "Delivering",
            Self::Waiting(reason) => reason.label(),
            Self::Paused => "Paused",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkerState {
    pub inventory: Inventory,
    pub status: WorkStatus,
    pub target: Option<WorkTarget>,
    pub work_seconds: f32,
    pub tree: Option<TreeWork>,
}

impl WorkerState {
    pub fn valid(&self) -> bool {
        self.inventory.valid()
            && self.inventory.slots.len() <= WORKER_SLOTS
            && self.inventory.cursor.is_none()
            && self.target.is_none_or(WorkTarget::valid)
            && self.work_seconds.is_finite()
            && (0.0..=3600.0).contains(&self.work_seconds)
            && self.tree.as_ref().is_none_or(TreeWork::valid)
    }

    /// Returns the remainder. Native inventories can grow to 27 slots; worker bags cannot.
    pub fn insert(&mut self, item: ItemKind, count: u16) -> u16 {
        if count == 0 || !item.valid() || !self.valid() {
            return count;
        }
        let Some(revision) = self.inventory.revision.checked_add(1) else {
            return count;
        };
        let mut remaining = count;
        for stack in &mut self.inventory.slots {
            if stack.item == item && stack.valid() {
                let moved = remaining.min(item.max_stack() - stack.count);
                stack.count += moved;
                remaining -= moved;
            }
        }
        for index in 0..WORKER_SLOTS {
            if remaining == 0 {
                break;
            }
            if self.inventory.slot(index).is_some() {
                continue;
            }
            let moved = remaining.min(item.max_stack());
            self.inventory
                .slots
                .resize(self.inventory.slots.len().max(index + 1), ItemStack::EMPTY);
            self.inventory.slots[index] = ItemStack { item, count: moved };
            remaining -= moved;
        }
        if remaining != count {
            self.inventory.revision = revision;
        }
        remaining
    }
}

/// Exact attribution is copied from the existing felling plan. Progress survives saves,
/// pauses and nightfall, including already spawned real drop identities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeWork {
    pub root: [i64; 3],
    pub foot: [i64; 3],
    pub species: crate::TreeSpecies,
    pub planted: Option<u64>,
    pub wood: Vec<[i64; 3]>,
    pub leaves: Vec<[i64; 3]>,
    pub drops: Vec<u64>,
    pub stage: TreeWorkStage,
    pub sapling_loaded: bool,
    pub sapling_dropped: bool,
    pub started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeWorkStage {
    Cutting,
    Collecting,
    Planting,
}

impl TreeWork {
    pub fn valid(&self) -> bool {
        valid_cell(self.root)
            && valid_cell(self.foot)
            && self.planted.is_none_or(|id| id > 0)
            && self.wood.len() <= crate::trees::MAX_TREE_WOOD
            && self.leaves.len() <= crate::trees::MAX_TREE_LEAVES
            && self.drops.len() <= crate::trees::MAX_TREE_WOOD + 1
            && self.drops.iter().all(|id| *id > 0)
            && self.wood.iter().chain(&self.leaves).all(|cell| {
                valid_cell(*cell) && (0..3).all(|axis| cell[axis].abs_diff(self.root[axis]) <= 127)
            })
    }
}
