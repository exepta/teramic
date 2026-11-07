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
        #[serde(with = "inventory_array")]
        slots: [Option<u8>; terra_core::INVENTORY_SLOTS],
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
        #[serde(with = "inventory_array")]
        reserved: [u16; terra_core::INVENTORY_SLOTS],
    },
    TransferCursor {
        item: ItemKind,
        #[serde(with = "inventory_array")]
        counts: [u16; terra_core::INVENTORY_SLOTS],
        to_cursor: bool,
    },
    OpenContainer {
        position: [i64; 3],
    },
    CloseContainer,
    ViewDrag {
        slots: [Option<u8>; 32],
        #[serde(with = "inventory_array")]
        extra: [Option<u8>; terra_core::containers::CONTAINER_VIEW_SLOTS - 32],
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
    SetMixerColor {
        station: [i64; 3],
        target: [u8; 3],
    },
    SetFunnelFilter {
        station: [i64; 3],
        filter: terra_core::logistics::FilterConfig,
    },
    SetDistributorFilter {
        station: [i64; 3],
        output: u8,
        filter: terra_core::logistics::FilterConfig,
    },
    SetTunnelRandom {
        station: [i64; 3],
        random: bool,
    },
    SetBrewingRecipe {
        station: [i64; 3],
        recipe: u16,
    },
    SetCampfireLit {
        station: [i64; 3],
        lit: bool,
    },
    FillBottle {
        slot: u8,
        position: [i64; 3],
    },
    OpenCampfire {
        position: [i64; 3],
    },
}

// Serde's built-in array implementation stops at 32 elements. Keep the bounded
// tuple encoding rather than accepting an unbounded network allocation.
mod inventory_array {
    use serde::{
        Deserialize, Deserializer, Serialize, Serializer,
        de::{Error, SeqAccess, Visitor},
        ser::SerializeTuple,
    };
    pub fn serialize<S: Serializer, T: Serialize, const COUNT: usize>(
        slots: &[T; COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(COUNT)?;
        for slot in slots {
            tuple.serialize_element(slot)?;
        }
        tuple.end()
    }
    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        T: Deserialize<'de> + Default + Copy,
        const COUNT: usize,
    >(
        deserializer: D,
    ) -> Result<[T; COUNT], D::Error> {
        struct Slots<T, const COUNT: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de> + Default + Copy, const COUNT: usize> Visitor<'de>
            for Slots<T, COUNT>
        {
            type Value = [T; COUNT];
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "exactly {COUNT} inventory entries")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut slots = [T::default(); COUNT];
                for (i, slot) in slots.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| A::Error::invalid_length(i, &self))?;
                }
                Ok(slots)
            }
        }
        deserializer.deserialize_tuple(COUNT, Slots(std::marker::PhantomData))
    }
}

#[cfg(test)]
mod inventory_capacity_tests {
    use super::*;
    #[test]
    fn all_inventory_commands_roundtrip_through_binary_and_json() {
        let mut slots = [None; terra_core::INVENTORY_SLOTS];
        slots[26] = Some(26);
        let mut counts = [0; terra_core::INVENTORY_SLOTS];
        counts[26] = 17;
        let mut extra = [None; terra_core::containers::CONTAINER_VIEW_SLOTS - 32];
        *extra.last_mut().unwrap() = Some((terra_core::containers::CONTAINER_VIEW_SLOTS - 1) as u8);
        for action in [
            InventoryAction::Drag {
                slots,
                secondary: false,
            },
            InventoryAction::ProtectedMove {
                slot: 26,
                kind: ProtectedMoveKind::QuickMove,
                reserved: counts,
            },
            InventoryAction::TransferCursor {
                item: ItemKind::Block(3),
                counts,
                to_cursor: true,
            },
            InventoryAction::ViewDrag {
                slots: [None; 32],
                extra,
                secondary: true,
            },
        ] {
            let command = InventoryCommand {
                revision: 71,
                action,
            };
            let bytes = postcard::to_stdvec(&command).unwrap();
            assert_eq!(
                postcard::from_bytes::<InventoryCommand>(&bytes).unwrap(),
                command
            );
            let json = serde_json::to_value(command).unwrap();
            assert_eq!(
                serde_json::from_value::<InventoryCommand>(json.clone()).unwrap(),
                command
            );
            let mut invalid = json;
            let field = match action {
                InventoryAction::Drag { .. } => "slots",
                InventoryAction::ProtectedMove { .. } => "reserved",
                InventoryAction::ViewDrag { .. } => "extra",
                _ => "counts",
            };
            let variant = match action {
                InventoryAction::Drag { .. } => "Drag",
                InventoryAction::ProtectedMove { .. } => "ProtectedMove",
                InventoryAction::ViewDrag { .. } => "ViewDrag",
                _ => "TransferCursor",
            };
            invalid["action"][variant][field]
                .as_array_mut()
                .unwrap()
                .pop();
            assert!(serde_json::from_value::<InventoryCommand>(invalid).is_err());
        }
        let container = ContainerCommand {
            inventory_revision: 71,
            anchor: [-1, 8, 16],
            view: ContainerView::Primary,
            revisions: vec![1, 2],
            reserved: counts,
            action: terra_core::containers::ContainerGesture::Drag {
                slots: vec![(terra_core::containers::CONTAINER_VIEW_SLOTS - 1) as u8],
                secondary: false,
            },
        };
        let decoded: ContainerCommand =
            postcard::from_bytes(&postcard::to_stdvec(&container).unwrap()).unwrap();
        assert_eq!(decoded.reserved, counts);
        assert_eq!(decoded.action, container.action);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerView {
    #[default]
    Primary,
    AttachedCampfire,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerHalf {
    pub position: [i64; 3],
    pub inventory: terra_core::Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub anchor: [i64; 3],
    pub view: ContainerView,
    pub attached_campfire: bool,
    pub workbench: Option<[i64; 3]>,
    pub furnace: Option<terra_core::furnace::FurnaceView>,
    pub crusher: Option<terra_core::crusher::CrusherView>,
    pub mixer: Option<terra_core::mixer::MixerView>,
    pub filter: Option<terra_core::logistics::FilterConfig>,
    pub distributor: Option<terra_core::logistics::DistributorConfig>,
    pub tunnel_random: Option<bool>,
    pub cauldron: Option<terra_core::brewing::CauldronView>,
    pub campfire: Option<terra_core::campfire::CampfireView>,
    pub halves: Vec<ContainerHalf>,
}
impl ContainerSnapshot {
    pub fn valid(&self) -> bool {
        if [
            self.workbench.is_some(),
            self.furnace.is_some(),
            self.crusher.is_some(),
            self.mixer.is_some(),
            self.filter.is_some() || self.tunnel_random.is_some(),
            self.distributor.is_some(),
            self.cauldron.is_some(),
            self.campfire.is_some(),
        ]
        .into_iter()
        .filter(|v| *v)
        .count()
            > 1
            || self.cauldron.as_ref().is_some_and(|view| !view.valid())
            || self.campfire.as_ref().is_some_and(|view| !view.valid())
            || self.view == ContainerView::AttachedCampfire
                && (!self.attached_campfire || self.campfire.is_none())
            || self.attached_campfire && self.cauldron.is_none() && self.campfire.is_none()
            || self.attached_campfire
                && self.campfire.is_some()
                && self.view != ContainerView::AttachedCampfire
        {
            return false;
        }
        if let Some(config) = self.distributor {
            return config.valid()
                && self.filter.is_none()
                && self.halves.is_empty()
                && self.workbench.is_none()
                && self.furnace.is_none()
                && self.crusher.is_none()
                && self.mixer.is_none()
                && self.anchor.iter().all(|p| p.unsigned_abs() <= 990_000);
        }
        if self.filter.is_some() || self.tunnel_random.is_some() {
            return self.filter.is_none_or(|filter| filter.valid())
                && self.halves.is_empty()
                && self.workbench.is_none()
                && self.furnace.is_none()
                && self.crusher.is_none()
                && self.mixer.is_none()
                && self.anchor.iter().all(|p| p.unsigned_abs() <= 990_000);
        }
        (1..=2).contains(&self.halves.len())
            && (!(self.cauldron.is_some() || self.campfire.is_some()) || self.halves.len() == 1)
            && self.mixer.as_ref().is_none_or(|m| {
                m.valid()
                    && self.halves.len() == 1
                    && self.furnace.is_none()
                    && self.crusher.is_none()
                    && self.workbench.is_none()
            })
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
                    && if self.cauldron.is_some() {
                        terra_core::brewing::valid_inventory(&half.inventory)
                    } else if self.campfire.is_some() {
                        terra_core::campfire::valid_inventory(&half.inventory)
                    } else if self.furnace.is_some() {
                        terra_core::furnace::valid_inventory(&half.inventory)
                    } else if self.crusher.is_some() {
                        terra_core::crusher::valid_inventory(&half.inventory)
                    } else if self.mixer.is_some() {
                        terra_core::mixer::valid_inventory(&half.inventory)
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
    pub view: ContainerView,
    pub revisions: Vec<u64>,
    #[serde(with = "inventory_array")]
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
