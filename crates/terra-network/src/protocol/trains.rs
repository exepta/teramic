//! Reliable owner-authorized parked-vehicle assembly, coupling, seats and cargo.
use serde::{Deserialize, Serialize};
use terra_core::{
    Inventory,
    containers::ContainerGesture,
    trains::{CouplerEnd, SeatBinding, TrainError, VehicleKind},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainAction {
    Inspect {
        station: [i64; 3],
    },
    Build {
        station: [i64; 3],
        chassis: [i64; 3],
        facing: u8,
        kind: VehicleKind,
        wheelbase: u8,
        width: u8,
        player_revision: u64,
    },
    Assemble {
        station: [i64; 3],
        start: [i64; 3],
        kind: VehicleKind,
        revision: u64,
    },
    Dismantle {
        station: [i64; 3],
        vehicle: u64,
        revision: u64,
    },
    Couple {
        station: [i64; 3],
        first: u64,
        first_end: CouplerEnd,
        second: u64,
        second_end: CouplerEnd,
        revision: u64,
    },
    Uncouple {
        station: [i64; 3],
        vehicle: u64,
        end: CouplerEnd,
        revision: u64,
    },
    OpenCargo {
        vehicle: u64,
        part: u16,
    },
    Cargo {
        vehicle: u64,
        part: u16,
        revision: u64,
        player_revision: u64,
        gesture: ContainerGesture,
        reserved: [u16; terra_core::INVENTORY_SLOTS],
    },
    Seat {
        vehicle: u64,
        part: u16,
    },
    Drive {
        vehicle: u64,
        throttle: i8,
        brake: bool,
    },
    Name {
        vehicle: u64,
        name: String,
        station: Option<[i64; 3]>,
    },
    Turntable {
        vehicle: u64,
        clockwise: bool,
        station: Option<[i64; 3]>,
    },
    RotateEmpty {
        station: [i64; 3],
        anchor: [i64; 3],
        clockwise: bool,
    },
    Leave,
    Destroy {
        vehicle: u64,
        revision: u64,
    },
    FillWater {
        vehicle: u64,
        player_revision: u64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainCommand {
    pub request: u64,
    pub action: TrainAction,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainCargoSnapshot {
    pub vehicle: u64,
    pub part: u16,
    pub revision: u64,
    pub inventory: Inventory,
}
impl TrainCargoSnapshot {
    pub fn valid(&self) -> bool {
        self.vehicle > 0
            && usize::from(self.part) < terra_core::trains::MAX_PARTS
            && terra_core::containers::valid_chest(&self.inventory)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainOutcome {
    Inspected,
    Changed { vehicle: Option<u64> },
    Cargo(TrainCargoSnapshot),
    Seated(Option<SeatBinding>),
    Rejected(TrainError),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainResult {
    pub request: u64,
    pub outcome: TrainOutcome,
}
