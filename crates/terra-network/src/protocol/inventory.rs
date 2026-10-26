use serde::{Deserialize, Serialize};
use terra_core::ItemKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryCommand {
    pub revision: u64,
    pub action: InventoryAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtectedMoveKind {
    QuickMove,
    Collect,
    CollectFrom,
    WheelToCursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryAction {
    CraftGrid {
        slots: [Option<u8>; 9],
        station: [i64; 3],
        count: u16,
        destination: terra_core::crafting::CraftDestination,
    },
    Recipe {
        recipe: u64,
        count: u16,
        station: Option<[i64; 3]>,
        destination: terra_core::crafting::CraftDestination,
    },
    Click {
        slot: u8,
        secondary: bool,
    },
    EquipmentClick {
        slot: u8,
        secondary: bool,
    },
    EquipmentQuickMove {
        slot: u8,
    },
    Drag {
        slots: [Option<u8>; 27],
        secondary: bool,
    },
    QuickMove {
        slot: u8,
    },
    Collect {
        slot: u8,
    },
    CollectFrom {
        slot: u8,
    },
    Wheel {
        slot: u8,
        to_cursor: bool,
    },
    CreativeCursor {
        item: ItemKind,
        count: u16,
    },
    CreativeQuickMove {
        item: ItemKind,
        count: u16,
    },
    DropSlot {
        slot: u8,
        all: bool,
    },
    DropCursor {
        all: bool,
    },
    ReturnCursor,
    CraftCursor {
        slots: [Option<u8>; 4],
    },
    Move {
        from: u8,
        to: u8,
        count: u16,
    },
    Craft {
        slots: [Option<u8>; 4],
    },
    CreativeSet {
        slot: u8,
        item: ItemKind,
        count: u16,
    },
    Eat {
        slot: u8,
    },
    CraftReserved {
        cells: [Option<terra_core::crafting::CraftGridCell>; 9],
        size: u8,
        station: Option<[i64; 3]>,
        count: u16,
        destination: terra_core::crafting::CraftDestination,
    },
    TakeToCursor {
        slot: u8,
        count: u16,
    },
    ProtectedMove {
        slot: u8,
        kind: ProtectedMoveKind,
        reserved: [u16; terra_core::INVENTORY_SLOTS],
    },
    TransferCursor {
        item: ItemKind,
        counts: [u16; terra_core::INVENTORY_SLOTS],
        to_cursor: bool,
    },
    OpenContainer {
        position: [i64; 3],
    },
    CloseContainer,
    ViewDrag {
        slots: [Option<u8>; 32],
        extra: [Option<u8>; 31],
        secondary: bool,
    },
    LoadSmeltingRecipe {
        recipe: u64,
        station: [i64; 3],
    },
    CreativeTrash {
        slot: Option<u8>,
    },
    LoadCrushingRecipe {
        recipe: u64,
        station: [i64; 3],
    },
    CreativeClear,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerHalf {
    pub position: [i64; 3],
    pub inventory: terra_core::Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub anchor: [i64; 3],
    pub workbench: Option<[i64; 3]>,
    pub furnace: Option<terra_core::furnace::FurnaceView>,
    pub crusher: Option<terra_core::crusher::CrusherView>,
    pub halves: Vec<ContainerHalf>,
}
impl ContainerSnapshot {
    pub fn valid(&self) -> bool {
        (1..=2).contains(&self.halves.len())
            && self.furnace.as_ref().is_none_or(|f| {
                f.valid()
                    && self.halves.len() == 1
                    && self.workbench.is_none()
                    && self.crusher.is_none()
            })
            && self.crusher.as_ref().is_none_or(|c| {
                c.valid()
                    && self.halves.len() == 1
                    && self.workbench.is_none()
                    && self.furnace.is_none()
            })
            && self
                .workbench
                .is_none_or(|position| position.iter().all(|p| p.unsigned_abs() <= 990_000))
            && self.halves[0].position == self.anchor
            && self.halves.iter().all(|half| {
                half.position.iter().all(|p| p.unsigned_abs() <= 990_000)
                    && if self.furnace.is_some() {
                        terra_core::furnace::valid_inventory(&half.inventory)
                    } else if self.crusher.is_some() {
                        terra_core::crusher::valid_inventory(&half.inventory)
                    } else {
                        terra_core::containers::valid_chest(&half.inventory)
                    }
            })
            && (self.halves.len() == 1 || {
                let other = self.halves[1].position;
                self.anchor[1] == other[1]
                    && (i128::from(self.anchor[0]) - i128::from(other[0])).abs()
                        + (i128::from(self.anchor[2]) - i128::from(other[2])).abs()
                        == 1
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerCommand {
    pub inventory_revision: u64,
    pub anchor: [i64; 3],
    pub revisions: Vec<u64>,
    pub reserved: [u16; terra_core::INVENTORY_SLOTS],
    pub action: terra_core::containers::ContainerGesture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryOutcome {
    Applied,
    Conflict,
    Invalid,
    Denied,
    RateLimited,
    MissingIngredients,
    NoSpace,
    WrongStation,
    UnknownRecipe,
    ContainerUnavailable,
}

impl InventoryOutcome {
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingIngredients => "Missing ingredients.",
            Self::NoSpace => "No room for the result. Free inventory space or empty the cursor.",
            Self::WrongStation => "The workbench is missing, obstructed or out of reach.",
            Self::UnknownRecipe => "Recipe is unavailable. Rejoin to refresh recipes.",
            Self::Applied => "",
            Self::Conflict => "Inventory changed. Please try again.",
            Self::Invalid => "This move or recipe is not possible, or the inventory is full.",
            Self::Denied => "Creating items requires Creative mode.",
            Self::RateLimited => "Inventory commands are arriving too quickly.",
            Self::ContainerUnavailable => {
                "Chest is out of reach, obstructed, or no longer available."
            }
        }
    }
}
