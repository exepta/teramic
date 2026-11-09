//! Frozen ID13/ID14 layouts migrate block metadata without reinterpreting old fields.
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
            name: String::new(),
            id: old.id,
            owner: old.owner,
            kind: old.kind,
            anchor: old.anchor,
            parts: old.parts,
            cargo: old.cargo,
            blocks: Default::default(),
            payloads: Default::default(),
            motion: Default::default(),
            burn_seconds: 0.0,
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

// Frozen ID14 layout predates fuel and the separate rigid pose.
#[derive(Serialize, Deserialize)]
pub(super) struct TrainIndex14 {
    pub prior: TreeIndex,
    pub trains: Registry14,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Registry14 {
    pub next_id: u64,
    pub revision: u64,
    pub vehicles: BTreeMap<u64, Vehicle14>,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Vehicle14 {
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: [i64; 3],
    pub parts: Vec<TrainPart>,
    pub cargo: BTreeMap<u16, Inventory>,
    pub blocks: BTreeMap<u16, terra_core::trains::TrainBlockView>,
    pub payloads: BTreeMap<u16, Vec<u8>>,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
}
impl From<TrainIndex14> for TrainIndex {
    fn from(old: TrainIndex14) -> Self {
        Self {
            prior: old.prior,
            trains: terra_core::trains::TrainRegistry {
                next_id: old.trains.next_id,
                revision: old.trains.revision,
                vehicles: old
                    .trains
                    .vehicles
                    .into_iter()
                    .map(|(id, car)| {
                        let mut blocks = car.blocks;
                        // ID14 captured the former thin chassis and small wheel boxes.
                        // Update those built-in profiles with their revised native kit.
                        for (index, part) in car.parts.iter().enumerate() {
                            if (210..=216).contains(&part.material)
                                && let Some(view) = blocks.get_mut(&(index as u16))
                            {
                                view.boxes =
                                    terra_core::train_parts::boxes(part.material, part.facing);
                            }
                        }
                        (
                            id,
                            Vehicle {
                                name: String::new(),
                                id: car.id,
                                owner: car.owner,
                                kind: car.kind,
                                anchor: car.anchor,
                                parts: car.parts,
                                cargo: car.cargo,
                                blocks,
                                payloads: car.payloads,
                                couplings: car.couplings,
                                revision: car.revision,
                                motion: Default::default(),
                                burn_seconds: 0.0,
                            },
                        )
                    })
                    .collect(),
            },
        }
    }
}

/// Frozen ID15 layout, before editable vehicle names were added.
#[derive(Serialize, Deserialize)]
pub(super) struct TrainIndex15 {
    pub prior: super::TreeIndex,
    pub trains: Registry15,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Registry15 {
    pub next_id: u64,
    pub revision: u64,
    pub vehicles: BTreeMap<u64, Vehicle15>,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Vehicle15 {
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: [i64; 3],
    pub parts: Vec<TrainPart>,
    pub cargo: BTreeMap<u16, Inventory>,
    pub blocks: BTreeMap<u16, terra_core::trains::TrainBlockView>,
    pub payloads: BTreeMap<u16, Vec<u8>>,
    pub motion: terra_core::train_motion::TrainMotion,
    pub burn_seconds: f64,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
}
impl From<TrainIndex15> for TrainIndex {
    fn from(old: TrainIndex15) -> Self {
        Self {
            prior: old.prior,
            trains: terra_core::trains::TrainRegistry {
                next_id: old.trains.next_id,
                revision: old.trains.revision,
                vehicles: old
                    .trains
                    .vehicles
                    .into_iter()
                    .map(|(id, car)| {
                        (
                            id,
                            Vehicle {
                                name: String::new(),
                                id: car.id,
                                owner: car.owner,
                                kind: car.kind,
                                anchor: car.anchor,
                                parts: car.parts,
                                cargo: car.cargo,
                                blocks: car.blocks,
                                payloads: car.payloads,
                                motion: car.motion,
                                burn_seconds: car.burn_seconds,
                                couplings: car.couplings,
                                revision: car.revision,
                            },
                        )
                    })
                    .collect(),
            },
        }
    }
}
