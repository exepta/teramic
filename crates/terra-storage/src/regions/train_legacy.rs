//! Frozen ID13 layout predates arbitrary block appearance and machine payloads.
use super::*;
use terra_core::trains::{Coupling, TrainPart, Vehicle, VehicleKind};

#[derive(Serialize, Deserialize)]
pub(super) struct TrainIndex13 {
    pub prior: TreeIndex,
    pub trains: Registry13,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Registry13 {
    pub next_id: u64,
    pub revision: u64,
    pub vehicles: BTreeMap<u64, Vehicle13>,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Vehicle13 {
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: [i64; 3],
    pub parts: Vec<TrainPart>,
    pub cargo: BTreeMap<u16, Inventory>,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
}
impl From<Vehicle13> for Vehicle {
    fn from(old: Vehicle13) -> Self {
        Self {
            id: old.id,
            owner: old.owner,
            kind: old.kind,
            anchor: old.anchor,
            parts: old.parts,
            cargo: old.cargo,
            blocks: Default::default(),
            payloads: Default::default(),
            couplings: old.couplings,
            revision: old.revision,
        }
    }
}
impl From<TrainIndex13> for TrainIndex {
    fn from(old: TrainIndex13) -> Self {
        Self {
            prior: old.prior,
            trains: terra_core::trains::TrainRegistry {
                next_id: old.trains.next_id,
                revision: old.trains.revision,
                vehicles: old
                    .trains
                    .vehicles
                    .into_iter()
                    .map(|(id, vehicle)| (id, vehicle.into()))
                    .collect(),
            },
        }
    }
}
