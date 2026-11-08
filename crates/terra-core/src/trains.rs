//! Persistent, parked modular vehicles; propulsion and arbitrary moving blocks are separate.
use crate::{Inventory, PlayerUuid, WoodSpecies, containers, train_parts};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_VEHICLES: usize = 64;
pub const MODEL_Y_OFFSET: f64 = -0.7375;
pub const MAX_PARTS: usize = 256;
pub const MAX_TOTAL_PARTS: usize = 4096;
pub const MAX_CONSIST: usize = 16;
pub const MAX_SEATED: usize = 1024;
pub type Cell = [i64; 3];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleKind {
    #[default]
    Locomotive,
    Tender,
    Passenger,
    Freight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CouplerEnd {
    Front,
    Rear,
}
impl CouplerEnd {
    pub const fn index(self) -> usize {
        match self {
            Self::Front => 0,
            Self::Rear => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coupling {
    pub vehicle: u64,
    pub end: CouplerEnd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainPart {
    pub offset: [i16; 3],
    pub material: u16,
    pub facing: u8,
    pub wood: WoodSpecies,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: Cell,
    pub parts: Vec<TrainPart>,
    pub cargo: BTreeMap<u16, Inventory>,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleView {
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: Cell,
    pub parts: Vec<TrainPart>,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
    pub mass_kg: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatBinding {
    pub player: PlayerUuid,
    pub vehicle: u64,
    pub part: u16,
}
impl SeatBinding {
    pub fn valid(self) -> bool {
        !self.player.is_nil() && self.vehicle > 0 && usize::from(self.part) < MAX_PARTS
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainSnapshot {
    pub revision: u64,
    pub vehicles: Vec<VehicleView>,
    pub seats: Vec<SeatBinding>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainRegistry {
    pub next_id: u64,
    pub revision: u64,
    pub vehicles: BTreeMap<u64, Vehicle>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainError {
    Invalid,
    Limit,
    Missing,
    NotOwner,
    Stale,
    Incomplete,
    Unsupported,
    Occupied,
    Protected,
    OutOfReach,
    Coupled,
    Cycle,
    Busy,
    RateLimited,
    Overflow,
}
impl std::fmt::Display for TrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TrainError {}

fn part_cell(anchor: Cell, parts: &[TrainPart], index: usize) -> Option<Cell> {
    let part = parts.get(index)?;
    Some(std::array::from_fn(|axis| {
        anchor[axis] + i64::from(part.offset[axis])
    }))
}
fn bounds(anchor: Cell, parts: &[TrainPart]) -> [Cell; 2] {
    let mut low = anchor;
    let mut high = anchor;
    for part in parts {
        for axis in 0..3 {
            let cell = anchor[axis] + i64::from(part.offset[axis]);
            low[axis] = low[axis].min(cell);
            high[axis] = high[axis].max(cell + 1);
        }
    }
    [low, high]
}
fn seat_pose(anchor: Cell, parts: &[TrainPart], index: usize) -> Option<([f64; 3], f32)> {
    let part = parts.get(index)?;
    if !matches!(part.material, train_parts::SEAT | train_parts::CAB) {
        return None;
    }
    let cell = part_cell(anchor, parts, index)?;
    Some((
        [
            cell[0] as f64 + 0.5,
            cell[1] as f64 + 0.485 + MODEL_Y_OFFSET,
            cell[2] as f64 + 0.5,
        ],
        f32::from(part.facing) * std::f32::consts::FRAC_PI_2,
    ))
}
fn valid_parts(anchor: Cell, parts: &[TrainPart], kind: VehicleKind) -> bool {
    if parts.is_empty()
        || parts.len() > MAX_PARTS
        || anchor.iter().any(|v| v.unsigned_abs() > 990_000)
    {
        return false;
    }
    let mut offsets = BTreeSet::new();
    if parts.iter().any(|part| {
        !train_parts::is_part(part.material)
            || part.facing > 3
            || !offsets.insert(part.offset)
            || part.offset[0].unsigned_abs() > 64
            || part.offset[1].unsigned_abs() > 12
            || part.offset[2].unsigned_abs() > 64
    }) {
        return false;
    }
    let mut pending = vec![parts[0].offset];
    let mut connected = BTreeSet::new();
    while let Some(cell) = pending.pop() {
        if !connected.insert(cell) {
            continue;
        }
        for delta in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let neighbor = std::array::from_fn(|axis| cell[axis] + delta[axis]);
            if offsets.contains(&neighbor) && !connected.contains(&neighbor) {
                pending.push(neighbor);
            }
        }
    }
    if connected.len() != parts.len() {
        return false;
    }
    let [low, high] = bounds(anchor, parts);
    if high[0] - low[0] > 64 || high[1] - low[1] > 12 || high[2] - low[2] > 64 {
        return false;
    }
    let axis = parts
        .iter()
        .find(|part| part.material == train_parts::CHASSIS)
        .map(|part| part.facing % 2);
    if axis.is_some_and(|axis| {
        parts.iter().any(|part| {
            matches!(
                part.material,
                train_parts::CHASSIS | train_parts::BOGIE | train_parts::COUPLER
            ) && part.facing % 2 != axis
        })
    }) {
        return false;
    }
    let count = |material| {
        parts
            .iter()
            .filter(|part| part.material == material)
            .count()
    };
    if count(train_parts::CHASSIS) == 0
        || count(train_parts::BOGIE) < 2
        || count(train_parts::FRAME) == 0
        || count(train_parts::COUPLER) == 0
    {
        return false;
    }
    match kind {
        VehicleKind::Locomotive => count(train_parts::CAB) > 0,
        VehicleKind::Tender | VehicleKind::Freight => {
            count(train_parts::CARGO) > 0
                && count(train_parts::CAB) == 0
                && count(train_parts::SEAT) == 0
        }
        VehicleKind::Passenger => count(train_parts::SEAT) > 0 && count(train_parts::CAB) == 0,
    }
}
macro_rules! geometry_methods {
    () => {
        pub fn part_cell(&self, index: usize) -> Option<Cell> {
            part_cell(self.anchor, &self.parts, index)
        }
        pub fn bounds(&self) -> [Cell; 2] {
            bounds(self.anchor, &self.parts)
        }
        pub fn model_origin(&self) -> [f64; 3] {
            [
                self.anchor[0] as f64,
                self.anchor[1] as f64 + MODEL_Y_OFFSET,
                self.anchor[2] as f64,
            ]
        }
        pub fn model_bounds(&self) -> [[f64; 3]; 2] {
            let origin = self.model_origin();
            let mut low = [f64::INFINITY; 3];
            let mut high = [f64::NEG_INFINITY; 3];
            for part in &self.parts {
                for shape in train_parts::boxes(part.material, part.facing) {
                    for axis in 0..3 {
                        low[axis] = low[axis]
                            .min(origin[axis] + f64::from(part.offset[axis]) + shape.minimum[axis]);
                        high[axis] = high[axis]
                            .max(origin[axis] + f64::from(part.offset[axis]) + shape.maximum[axis]);
                    }
                }
            }
            if self.parts.is_empty() {
                [origin, origin]
            } else {
                [low, high]
            }
        }
        pub fn seat_pose(&self, index: usize) -> Option<([f64; 3], f32)> {
            seat_pose(self.anchor, &self.parts, index)
        }
        pub fn cargo_indices(&self) -> impl Iterator<Item = u16> + '_ {
            self.parts
                .iter()
                .enumerate()
                .filter(|(_, part)| part.material == train_parts::CARGO)
                .map(|(index, _)| index as u16)
        }
        pub fn coupler_cell(&self, end: CouplerEnd) -> Option<Cell> {
            let facing = self
                .parts
                .iter()
                .find(|part| part.material == train_parts::CHASSIS)?
                .facing;
            let score = |part: &TrainPart| match facing {
                0 => i32::from(part.offset[2]),
                1 => i32::from(part.offset[0]),
                2 => -i32::from(part.offset[2]),
                _ => -i32::from(part.offset[0]),
            };
            let mut couplers = self
                .parts
                .iter()
                .enumerate()
                .filter(|(_, part)| part.material == train_parts::COUPLER)
                .collect::<Vec<_>>();
            couplers.sort_by_key(|(index, part)| (score(part), *index));
            if couplers.len() == 1 {
                let chassis = self
                    .parts
                    .iter()
                    .find(|part| part.material == train_parts::CHASSIS)?;
                let actual = if score(couplers[0].1) >= score(chassis) {
                    CouplerEnd::Front
                } else {
                    CouplerEnd::Rear
                };
                if end != actual {
                    return None;
                }
            }
            let index = match end {
                CouplerEnd::Front => couplers.last()?.0,
                CouplerEnd::Rear => couplers.first()?.0,
            };
            self.part_cell(index)
        }
        pub fn coupler_tangent(&self, end: CouplerEnd) -> Option<Cell> {
            self.coupler_cell(end)?;
            let facing = self
                .parts
                .iter()
                .find(|part| part.material == train_parts::CHASSIS)?
                .facing;
            let direction = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing)];
            Some(direction.map(|value| {
                if end == CouplerEnd::Front {
                    value
                } else {
                    -value
                }
            }))
        }
    };
}
impl Vehicle {
    geometry_methods!();
    pub fn valid(&self) -> bool {
        self.id > 0
            && !self.owner.is_nil()
            && valid_parts(self.anchor, &self.parts, self.kind)
            && self.cargo.len() == self.cargo_indices().count()
            && self.cargo.iter().all(|(index, inventory)| {
                self.parts
                    .get(usize::from(*index))
                    .is_some_and(|part| part.material == train_parts::CARGO)
                    && containers::valid_chest(inventory)
            })
    }
    pub fn mass_kg(&self) -> u32 {
        self.parts
            .iter()
            .filter_map(|part| train_parts::mass_kg(part.material))
            .sum::<u32>()
            + self
                .cargo
                .values()
                .flat_map(|inventory| inventory.slots.iter())
                .map(|stack| u32::from(stack.count) * 2)
                .sum::<u32>()
    }
    pub fn view(&self) -> VehicleView {
        VehicleView {
            id: self.id,
            owner: self.owner,
            kind: self.kind,
            anchor: self.anchor,
            parts: self.parts.clone(),
            couplings: self.couplings,
            revision: self.revision,
            mass_kg: self.mass_kg(),
        }
    }
}
impl VehicleView {
    geometry_methods!();
    pub fn valid(&self) -> bool {
        self.id > 0
            && !self.owner.is_nil()
            && valid_parts(self.anchor, &self.parts, self.kind)
            && self.mass_kg
                >= self
                    .parts
                    .iter()
                    .filter_map(|part| train_parts::mass_kg(part.material))
                    .sum::<u32>()
            && self.mass_kg < 1_000_000_000
    }
}
impl TrainRegistry {
    pub fn valid(&self) -> bool {
        if self
            .vehicles
            .values()
            .map(|vehicle| vehicle.parts.len())
            .sum::<usize>()
            > MAX_TOTAL_PARTS
            || self.vehicles.len() > MAX_VEHICLES
            || self
                .vehicles
                .iter()
                .any(|(id, vehicle)| *id != vehicle.id || *id > self.next_id || !vehicle.valid())
        {
            return false;
        }
        let mut cells = BTreeSet::new();
        if self.vehicles.values().any(|vehicle| {
            (0..vehicle.parts.len()).any(|index| !cells.insert(vehicle.part_cell(index).unwrap()))
        }) {
            return false;
        }
        self.vehicles.keys().all(|id| self.consist(*id).is_ok())
    }
    pub fn consist(&self, id: u64) -> Result<Vec<u64>, TrainError> {
        if !self.vehicles.contains_key(&id) {
            return Err(TrainError::Missing);
        }
        let mut pending = vec![(id, None)];
        let mut visited = BTreeSet::new();
        while let Some((current, parent)) = pending.pop() {
            if !visited.insert(current) {
                return Err(TrainError::Cycle);
            }
            if visited.len() > MAX_CONSIST {
                return Err(TrainError::Limit);
            }
            let vehicle = self.vehicles.get(&current).ok_or(TrainError::Incomplete)?;
            for (end, link) in vehicle
                .couplings
                .iter()
                .enumerate()
                .filter_map(|(end, link)| link.map(|link| (end, link)))
            {
                let other = self
                    .vehicles
                    .get(&link.vehicle)
                    .ok_or(TrainError::Incomplete)?;
                if other.owner != vehicle.owner
                    || other.couplings[link.end.index()]
                        != Some(Coupling {
                            vehicle: current,
                            end: if end == 0 {
                                CouplerEnd::Front
                            } else {
                                CouplerEnd::Rear
                            },
                        })
                {
                    return Err(TrainError::Incomplete);
                }
                if parent != Some(link.vehicle) {
                    pending.push((link.vehicle, Some(current)));
                }
            }
        }
        Ok(visited.into_iter().collect())
    }
    pub fn insert(&mut self, mut vehicle: Vehicle) -> Result<u64, TrainError> {
        if self.vehicles.len() >= MAX_VEHICLES
            || self
                .vehicles
                .values()
                .map(|old| old.parts.len())
                .sum::<usize>()
                + vehicle.parts.len()
                > MAX_TOTAL_PARTS
        {
            return Err(TrainError::Limit);
        }
        let id = self.next_id.checked_add(1).ok_or(TrainError::Overflow)?;
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        vehicle.id = id;
        vehicle.revision = 1;
        if !vehicle.valid() || vehicle.couplings != [None, None] {
            return Err(TrainError::Invalid);
        }
        let occupied: BTreeSet<_> = self
            .vehicles
            .values()
            .flat_map(|old| (0..old.parts.len()).filter_map(|index| old.part_cell(index)))
            .collect();
        if (0..vehicle.parts.len())
            .any(|index| occupied.contains(&vehicle.part_cell(index).unwrap()))
        {
            return Err(TrainError::Occupied);
        }
        self.vehicles.insert(id, vehicle);
        self.next_id = id;
        self.revision = revision;
        Ok(id)
    }
    pub fn remove(&mut self, actor: PlayerUuid, id: u64) -> Result<Vehicle, TrainError> {
        let vehicle = self.vehicles.get(&id).ok_or(TrainError::Missing)?;
        if vehicle.owner != actor {
            return Err(TrainError::NotOwner);
        }
        if vehicle.couplings.iter().any(Option::is_some) {
            return Err(TrainError::Coupled);
        }
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        let removed = self.vehicles.remove(&id).unwrap();
        self.revision = revision;
        Ok(removed)
    }
    pub fn couple(
        &mut self,
        actor: PlayerUuid,
        first: u64,
        first_end: CouplerEnd,
        second: u64,
        second_end: CouplerEnd,
    ) -> Result<(), TrainError> {
        if first == second {
            return Err(TrainError::Cycle);
        }
        let a = self.vehicles.get(&first).ok_or(TrainError::Missing)?;
        let b = self.vehicles.get(&second).ok_or(TrainError::Missing)?;
        if a.owner != actor || b.owner != actor {
            return Err(TrainError::NotOwner);
        }
        if a.couplings[first_end.index()].is_some() || b.couplings[second_end.index()].is_some() {
            return Err(TrainError::Coupled);
        }
        let ca = self.consist(first)?;
        let cb = self.consist(second)?;
        if ca.contains(&second) {
            return Err(TrainError::Cycle);
        }
        if ca.len() + cb.len() > MAX_CONSIST {
            return Err(TrainError::Limit);
        }
        let ac = a.coupler_cell(first_end).ok_or(TrainError::Incomplete)?;
        let bc = b.coupler_cell(second_end).ok_or(TrainError::Incomplete)?;
        let distance = (0..3).map(|axis| ac[axis].abs_diff(bc[axis])).sum::<u64>();
        let direction = a.coupler_tangent(first_end).ok_or(TrainError::Incomplete)?;
        if b.coupler_tangent(second_end) != Some(direction.map(|value| -value))
            || !(1..=2).contains(&distance)
            || (0..3).any(|axis| bc[axis] - ac[axis] != direction[axis] * distance as i64)
        {
            return Err(TrainError::OutOfReach);
        }
        self.change_link(
            first,
            first_end,
            Some(Coupling {
                vehicle: second,
                end: second_end,
            }),
            second,
            second_end,
            Some(Coupling {
                vehicle: first,
                end: first_end,
            }),
        )
    }
    pub fn uncouple(
        &mut self,
        actor: PlayerUuid,
        id: u64,
        end: CouplerEnd,
    ) -> Result<(), TrainError> {
        let vehicle = self.vehicles.get(&id).ok_or(TrainError::Missing)?;
        if vehicle.owner != actor {
            return Err(TrainError::NotOwner);
        }
        let link = vehicle.couplings[end.index()].ok_or(TrainError::Missing)?;
        let other = self
            .vehicles
            .get(&link.vehicle)
            .ok_or(TrainError::Incomplete)?;
        if other.owner != actor
            || other.couplings[link.end.index()] != Some(Coupling { vehicle: id, end })
        {
            return Err(TrainError::Incomplete);
        }
        self.change_link(id, end, None, link.vehicle, link.end, None)
    }
    fn change_link(
        &mut self,
        a: u64,
        ae: CouplerEnd,
        al: Option<Coupling>,
        b: u64,
        be: CouplerEnd,
        bl: Option<Coupling>,
    ) -> Result<(), TrainError> {
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        let ar = self.vehicles[&a]
            .revision
            .checked_add(1)
            .ok_or(TrainError::Overflow)?;
        let br = self.vehicles[&b]
            .revision
            .checked_add(1)
            .ok_or(TrainError::Overflow)?;
        let first = self.vehicles.get_mut(&a).unwrap();
        first.couplings[ae.index()] = al;
        first.revision = ar;
        let second = self.vehicles.get_mut(&b).unwrap();
        second.couplings[be.index()] = bl;
        second.revision = br;
        self.revision = revision;
        Ok(())
    }
    pub fn snapshot(&self, seats: Vec<SeatBinding>) -> TrainSnapshot {
        TrainSnapshot {
            revision: self.revision,
            vehicles: self.vehicles.values().map(Vehicle::view).collect(),
            seats,
        }
    }
}
impl TrainSnapshot {
    pub fn valid(&self) -> bool {
        if self
            .vehicles
            .iter()
            .map(|vehicle| vehicle.parts.len())
            .sum::<usize>()
            > MAX_TOTAL_PARTS
            || self.vehicles.len() > MAX_VEHICLES
            || self.seats.len() > MAX_SEATED
        {
            return false;
        }
        let mut ids = BTreeSet::new();
        if self
            .vehicles
            .iter()
            .any(|vehicle| !vehicle.valid() || !ids.insert(vehicle.id))
        {
            return false;
        }
        let mut players = BTreeSet::new();
        let mut occupied = BTreeSet::new();
        self.seats.iter().all(|seat| {
            !seat.player.is_nil()
                && players.insert(seat.player)
                && occupied.insert((seat.vehicle, seat.part))
                && self
                    .vehicles
                    .iter()
                    .find(|vehicle| vehicle.id == seat.vehicle)
                    .is_some_and(|vehicle| vehicle.seat_pose(usize::from(seat.part)).is_some())
        }) && self.vehicles.iter().all(|vehicle| {
            vehicle.couplings.iter().enumerate().all(|(end, link)| {
                link.is_none_or(|link| {
                    self.vehicles
                        .iter()
                        .find(|other| other.id == link.vehicle)
                        .is_some_and(|other| {
                            other.owner == vehicle.owner
                                && other.couplings[link.end.index()]
                                    == Some(Coupling {
                                        vehicle: vehicle.id,
                                        end: if end == 0 {
                                            CouplerEnd::Front
                                        } else {
                                            CouplerEnd::Rear
                                        },
                                    })
                        })
                })
            })
        })
    }
}

#[cfg(test)]
mod tests;
