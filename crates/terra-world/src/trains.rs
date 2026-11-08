//! Bounded assembly plans for real modular parked vehicles and their private cargo.
use crate::{
    edits::{BlockObjectState, EditJournal},
    rails,
    streaming::ChunkStore,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terra_core::{
    Inventory, PlayerUuid,
    block_shapes::ShapeBox,
    kingdom::KingdomPermission,
    train_parts,
    trains::{Cell, MAX_PARTS, TrainError, TrainPart, Vehicle, VehicleKind},
};

#[derive(Clone, Debug)]
pub struct PartEdit {
    pub cell: Cell,
    pub expected: u16,
    pub state: Option<BlockObjectState>,
    pub cargo: Option<Inventory>,
}
#[derive(Clone, Debug)]
pub struct AssemblyPlan {
    pub station: Cell,
    pub start: Cell,
    pub actor: PlayerUuid,
    pub vehicle: Vehicle,
    pub assembling: bool,
    pub registry_revision: u64,
    pub parts: Vec<PartEdit>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainHit {
    pub vehicle: u64,
    pub part: u16,
    pub distance: f64,
}

fn allowed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    cell: Cell,
    permission: KingdomPermission,
) -> bool {
    crate::kingdom_access::can_use(
        &journal.gameplay().kingdoms,
        chunks,
        actor,
        permission,
        cell,
    )
}
fn station_ready(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
) -> Result<(), TrainError> {
    if actor.is_nil()
        || station.iter().any(|value| value.unsigned_abs() > 990_000)
        || chunks.block_at(station) != Some(train_parts::STATION)
    {
        return Err(TrainError::Missing);
    }
    if !allowed(
        chunks,
        journal,
        actor,
        station,
        KingdomPermission::Inventories,
    ) {
        return Err(TrainError::Protected);
    }
    Ok(())
}
pub fn candidate_start(chunks: &ChunkStore, station: Cell) -> Option<Cell> {
    let mut candidates = Vec::new();
    for x in -4_i64..=4 {
        for y in -3_i64..=3 {
            for z in -4_i64..=4 {
                let cell = [station[0] + x, station[1] + y, station[2] + z];
                if chunks.block_at(cell).is_some_and(train_parts::is_part) {
                    candidates.push((x * x + y * y + z * z, cell));
                }
            }
        }
    }
    candidates.sort_unstable();
    candidates.first().map(|(_, cell)| *cell)
}
fn bogie_supported(chunks: &ChunkStore, journal: &EditJournal, cell: Cell) -> bool {
    let below = [cell[0], cell[1] - 1, cell[2]];
    let Some(piece) = rails::owner_at(journal, below) else {
        return false;
    };
    if !rails::complete(journal, piece) || chunks.block_at(below) != Some(piece.material) {
        return false;
    }
    piece.active_paths().iter().any(|path| {
        (0..=128).any(|step| {
            let position = path.sample(f64::from(step) / 128.0).position;
            let dx = position[0] - (cell[0] as f64 + 0.5);
            let dz = position[2] - (cell[2] as f64 + 0.5);
            dx * dx + dz * dz <= 0.55 * 0.55
                && (position[1] - (cell[1] as f64 - 1.0 + terra_core::rails::DECK_HEIGHT)).abs()
                    < 0.55
        })
    })
}
pub fn plan_assembly(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    start: Cell,
    kind: VehicleKind,
) -> Result<AssemblyPlan, TrainError> {
    station_ready(chunks, journal, actor, station)?;
    if (0..3).any(|axis| start[axis].abs_diff(station[axis]) > 8) {
        return Err(TrainError::OutOfReach);
    }
    let mut queue = VecDeque::from([start]);
    let mut visited = BTreeSet::new();
    let mut cells = BTreeMap::new();
    while let Some(cell) = queue.pop_front() {
        if !visited.insert(cell) {
            continue;
        }
        let material = chunks.block_at(cell).ok_or(TrainError::Incomplete)?;
        if !train_parts::is_part(material) {
            continue;
        }
        if cells.len() >= MAX_PARTS
            || (0..3).any(|axis| cell[axis].abs_diff(start[axis]) > if axis == 1 { 12 } else { 64 })
        {
            return Err(TrainError::Limit);
        }
        cells.insert(cell, material);
        for delta in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let next = std::array::from_fn(|axis| cell[axis] + delta[axis]);
            // Adjacent couplers belong to two separately assembled cars.
            if material == train_parts::COUPLER
                && chunks.block_at(next) == Some(train_parts::COUPLER)
            {
                continue;
            }
            queue.push_back(next);
        }
    }
    if cells.is_empty() {
        return Err(TrainError::Missing);
    }
    let anchor = *cells.keys().next().unwrap();
    let mut parts = Vec::new();
    let mut edits = Vec::new();
    let mut cargo = BTreeMap::new();
    for (cell, material) in cells {
        if (0..3).any(|axis| cell[axis].abs_diff(station[axis]) > 64) {
            return Err(TrainError::OutOfReach);
        }
        for permission in [
            KingdomPermission::Build,
            KingdomPermission::Break,
            KingdomPermission::Inventories,
        ] {
            if !allowed(chunks, journal, actor, cell, permission) {
                return Err(TrainError::Protected);
            }
        }
        if material == train_parts::BOGIE && !bogie_supported(chunks, journal, cell) {
            return Err(TrainError::Unsupported);
        }
        let state = journal.object_state(cell);
        if state.is_some_and(|state| {
            state.facing > 3
                || state.peer.is_some()
                || state.secondary
                || state.rail_curve.is_some()
        }) {
            return Err(TrainError::Invalid);
        }
        let pose = state.unwrap_or_default();
        let index = parts.len() as u16;
        let inventory = (material == train_parts::CARGO)
            .then(|| journal.container_at(cell).cloned().unwrap_or_default());
        if let Some(inventory) = &inventory {
            cargo.insert(index, inventory.clone());
        }
        parts.push(TrainPart {
            offset: std::array::from_fn(|axis| (cell[axis] - anchor[axis]) as i16),
            material,
            facing: pose.facing,
            wood: pose.wood,
        });
        edits.push(PartEdit {
            cell,
            expected: material,
            state,
            cargo: inventory,
        });
    }
    let vehicle = Vehicle {
        id: 1,
        owner: actor,
        kind,
        anchor,
        parts,
        cargo,
        couplings: [None, None],
        revision: 1,
    };
    if !vehicle.valid() {
        return Err(TrainError::Incomplete);
    }
    model_clearance(
        chunks,
        journal,
        &vehicle,
        &edits.iter().map(|part| part.cell).collect(),
        &Default::default(),
    )?;
    Ok(AssemblyPlan {
        station,
        start,
        actor,
        vehicle,
        assembling: true,
        registry_revision: journal.gameplay().trains.revision,
        parts: edits,
    })
}
pub fn plan_dismantle(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    id: u64,
) -> Result<AssemblyPlan, TrainError> {
    station_ready(chunks, journal, actor, station)?;
    let vehicle = journal
        .gameplay()
        .trains
        .vehicles
        .get(&id)
        .ok_or(TrainError::Missing)?
        .clone();
    if vehicle.owner != actor {
        return Err(TrainError::NotOwner);
    }
    if vehicle.couplings.iter().any(Option::is_some) {
        return Err(TrainError::Coupled);
    }
    if (0..3).any(|axis| vehicle.anchor[axis].abs_diff(station[axis]) > 64) {
        return Err(TrainError::OutOfReach);
    }
    let mut edits = Vec::new();
    for (index, part) in vehicle.parts.iter().enumerate() {
        let cell = vehicle.part_cell(index).ok_or(TrainError::Invalid)?;
        if chunks.block_at(cell) != Some(0) {
            return Err(TrainError::Occupied);
        }
        if !allowed(chunks, journal, actor, cell, KingdomPermission::Build)
            || !allowed(chunks, journal, actor, cell, KingdomPermission::Inventories)
        {
            return Err(TrainError::Protected);
        }
        edits.push(PartEdit {
            cell,
            expected: 0,
            state: Some(BlockObjectState {
                facing: part.facing,
                wood: part.wood,
                ..Default::default()
            }),
            cargo: vehicle.cargo.get(&(index as u16)).cloned(),
        });
    }
    Ok(AssemblyPlan {
        station,
        start: vehicle.anchor,
        actor,
        vehicle,
        assembling: false,
        registry_revision: journal.gameplay().trains.revision,
        parts: edits,
    })
}
pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &AssemblyPlan,
) -> Result<u64, TrainError> {
    journal.commit_train_plan(chunks, plan)
}
pub fn part_boxes(origin: [f64; 3], part: &TrainPart) -> impl Iterator<Item = ShapeBox> {
    train_parts::boxes(part.material, part.facing)
        .into_iter()
        .map(move |shape| ShapeBox {
            minimum: std::array::from_fn(|axis| {
                origin[axis] + f64::from(part.offset[axis]) + shape.minimum[axis]
            }),
            maximum: std::array::from_fn(|axis| {
                origin[axis] + f64::from(part.offset[axis]) + shape.maximum[axis]
            }),
            ..shape
        })
}
pub fn boxes_in(journal: &EditJournal, low: [f64; 3], high: [f64; 3]) -> Vec<ShapeBox> {
    if low
        .iter()
        .chain(high.iter())
        .any(|value| !value.is_finite())
    {
        return Vec::new();
    }
    journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .filter(|vehicle| {
            let [minimum, maximum] = vehicle.model_bounds();
            (0..3).all(|axis| minimum[axis] < high[axis] && maximum[axis] > low[axis])
        })
        .flat_map(|vehicle| {
            vehicle
                .parts
                .iter()
                .flat_map(move |part| part_boxes(vehicle.model_origin(), part))
        })
        .filter(|shape| shape.intersects(low, high))
        .collect()
}
fn reserved_parts(
    anchor: Cell,
    origin: [f64; 3],
    parts: &[TrainPart],
    model_bounds: [[f64; 3]; 2],
    cell: Cell,
) -> bool {
    if parts
        .iter()
        .any(|part| (0..3).all(|axis| anchor[axis] + i64::from(part.offset[axis]) == cell[axis]))
    {
        return true;
    }
    let low = cell.map(|value| value as f64);
    let high = cell.map(|value| value as f64 + 1.0);
    let [minimum, maximum] = model_bounds;
    (0..3).all(|axis| minimum[axis] < high[axis] && maximum[axis] > low[axis])
        && parts
            .iter()
            .any(|part| part_boxes(origin, part).any(|shape| shape.intersects(low, high)))
}
pub fn reserved(journal: &EditJournal, cell: Cell) -> bool {
    journal.gameplay().trains.vehicles.values().any(|vehicle| {
        reserved_parts(
            vehicle.anchor,
            vehicle.model_origin(),
            &vehicle.parts,
            vehicle.model_bounds(),
            cell,
        )
    })
}
pub fn reserved_in(views: &[terra_core::trains::VehicleView], cell: Cell) -> bool {
    views.iter().any(|vehicle| {
        reserved_parts(
            vehicle.anchor,
            vehicle.model_origin(),
            &vehicle.parts,
            vehicle.model_bounds(),
            cell,
        )
    })
}
pub fn raycast(
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<TrainHit> {
    raycast_parts(
        journal.gameplay().trains.vehicles.values().map(|vehicle| {
            (
                vehicle.id,
                vehicle.model_origin(),
                vehicle.parts.as_slice(),
                vehicle.model_bounds(),
            )
        }),
        origin,
        direction,
        reach,
    )
}
pub fn raycast_views(
    views: &[terra_core::trains::VehicleView],
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<TrainHit> {
    raycast_parts(
        views.iter().map(|vehicle| {
            (
                vehicle.id,
                vehicle.model_origin(),
                vehicle.parts.as_slice(),
                vehicle.model_bounds(),
            )
        }),
        origin,
        direction,
        reach,
    )
}
fn raycast_parts<'a>(
    vehicles: impl Iterator<Item = (u64, [f64; 3], &'a [TrainPart], [[f64; 3]; 2])>,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<TrainHit> {
    if origin
        .iter()
        .chain(direction.iter())
        .any(|value| !value.is_finite())
        || !reach.is_finite()
        || !(0.0..=terra_core::rails::RAIL_BUILD_REACH).contains(&reach)
    {
        return None;
    }
    let mut hit: Option<TrainHit> = None;
    for (id, model_origin, parts, [low, high]) in vehicles {
        if (ShapeBox {
            minimum: low,
            maximum: high,
            hinged: false,
        })
        .ray(origin, direction, reach)
        .is_none()
        {
            continue;
        }
        for (index, part) in parts.iter().enumerate() {
            for shape in part_boxes(model_origin, part) {
                if let Some((distance, _)) = shape.ray(origin, direction, reach)
                    && hit.is_none_or(|old| distance < old.distance)
                {
                    hit = Some(TrainHit {
                        vehicle: id,
                        part: index as u16,
                        distance,
                    });
                }
            }
        }
    }
    hit
}
/// Verify lowered physical parts against surviving terrain and other vehicles.
/// Source cells are removed by the same atomic transaction, never ignored later.
pub fn model_clearance(
    chunks: &ChunkStore,
    journal: &EditJournal,
    vehicle: &Vehicle,
    ignored: &BTreeSet<Cell>,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<(), TrainError> {
    for part in &vehicle.parts {
        for mut shape in part_boxes(vehicle.model_origin(), part) {
            for axis in 0..3 {
                shape.minimum[axis] += 1e-8;
                shape.maximum[axis] -= 1e-8;
            }
            if boxes_in(journal, shape.minimum, shape.maximum)
                .iter()
                .any(|other| other.intersects(shape.minimum, shape.maximum))
            {
                return Err(TrainError::Occupied);
            }
            let low = shape.minimum.map(|value| value.floor() as i64);
            let high = shape.maximum.map(|value| value.floor() as i64);
            for x in low[0]..=high[0] {
                for y in low[1]..=high[1] {
                    for z in low[2]..=high[2] {
                        let cell = [x, y, z];
                        if ignored.contains(&cell) {
                            continue;
                        }
                        let material = chunks.block_at(cell).ok_or(TrainError::Incomplete)?;
                        let mut hit = false;
                        if crate::interaction_shapes::visit_collision_boxes(
                            chunks,
                            journal,
                            properties,
                            cell,
                            |other| hit |= other.intersects(shape.minimum, shape.maximum),
                        ) {
                            if hit {
                                return Err(TrainError::Occupied);
                            }
                        } else if terra_voxel::MaterialId::from_raw(material)
                            .is_none_or(terra_voxel::MaterialId::is_solid)
                        {
                            return Err(TrainError::Occupied);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "trains_tests.rs"]
mod tests;
