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
    pub properties: terra_core::block_properties::BlockPropertiesCatalog,
    pub source_motion: Option<terra_core::train_motion::TrainMotion>,
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
pub fn bogie_supported(chunks: &ChunkStore, journal: &EditJournal, cell: Cell) -> bool {
    [0, 1]
        .into_iter()
        .any(|facing| bogie_supported_with_facing(chunks, journal, cell, facing))
}
/// Both native wheel axles lie at local Z=0/1, not at the owner-cell center.
/// Rail ownership may live in a neighboring gauge slice rather than below its air gap.
pub fn bogie_supported_with_facing(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: Cell,
    facing: u8,
) -> bool {
    if facing >= 4 {
        return false;
    }
    let along = if facing.is_multiple_of(2) { 2 } else { 0 };
    let contacts: [[f64; 3]; 2] = [0.0, 1.0].map(|offset| {
        std::array::from_fn(|axis| {
            cell[axis] as f64
                + if axis == 1 {
                    -1.0 + terra_core::rails::DECK_HEIGHT
                } else if axis == along {
                    offset
                } else {
                    0.5
                }
        })
    });
    let mut candidates = BTreeMap::new();
    for contact in contacts {
        let base = contact.map(|value| value.floor() as i64);
        for x in -1..=1 {
            for z in -1..=1 {
                if let Some(piece) = rails::owner_at(journal, [base[0] + x, base[1], base[2] + z]) {
                    candidates.insert(piece.anchor, piece);
                }
            }
        }
    }
    let mut supported = [false; 2];
    for piece in candidates.into_values() {
        if !rails::complete(journal, piece)
            || rails::cells(piece)
                .into_iter()
                .any(|cell| chunks.block_at(cell) != Some(piece.material))
        {
            continue;
        }
        for path in piece.active_paths() {
            let length = path.length();
            let mut previous = path.sample_distance(0.0).position;
            for step in 1..=128 {
                let position = path
                    .sample_distance(length * f64::from(step) / 128.0)
                    .position;
                let delta: [f64; 3] = std::array::from_fn(|i| position[i] - previous[i]);
                let squared = delta.iter().map(|v| v * v).sum::<f64>();
                let segment_length = squared.sqrt();
                if segment_length < 1e-10
                    || delta[along].abs() / segment_length < 0.995
                    || delta[1].abs() / segment_length > 0.02
                {
                    previous = position;
                    continue;
                }
                for (index, contact) in contacts.iter().enumerate() {
                    let t = ((0..3)
                        .map(|i| (contact[i] - previous[i]) * delta[i])
                        .sum::<f64>()
                        / squared)
                        .clamp(0.0, 1.0);
                    let nearest: [f64; 3] = std::array::from_fn(|i| previous[i] + delta[i] * t);
                    let error =
                        (nearest[0] - contact[0]).powi(2) + (nearest[2] - contact[2]).powi(2);
                    if error <= 0.18_f64.powi(2) && (nearest[1] - contact[1]).abs() <= 0.08 {
                        supported[index] = true;
                    }
                }
                if supported == [true; 2] {
                    return true;
                }
                previous = position;
            }
        }
    }
    false
}
pub fn plan_assembly(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    start: Cell,
    kind: VehicleKind,
) -> Result<AssemblyPlan, TrainError> {
    plan_assembly_with_properties(
        chunks,
        journal,
        actor,
        station,
        start,
        kind,
        &Default::default(),
    )
}
/// Capture registered block mechanics once; no machine is simulated while mounted.
pub fn plan_assembly_with_properties(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    start: Cell,
    kind: VehicleKind,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Result<AssemblyPlan, TrainError> {
    station_ready(chunks, journal, actor, station)?;
    if (0..3).any(|axis| start[axis].abs_diff(station[axis]) > 8) {
        return Err(TrainError::OutOfReach);
    }
    // The rail deck forms a hard lower boundary. Natural terrain never joins
    // a train: ordinary parts must be explicit edits in the building envelope.
    let floor = (1..=12)
        .find_map(|drop| {
            let y = start[1] - drop;
            (-4..=4).find_map(|x| {
                (-4..=4).find_map(|z| {
                    let cell = [start[0] + x, y, start[2] + z];
                    rails::owner_at(journal, cell)
                        .filter(|piece| rails::complete(journal, *piece))
                        .map(|_| y + 1)
                })
            })
        })
        .ok_or(TrainError::Unsupported)?;
    let mut queue = VecDeque::from([start]);
    let mut visited = BTreeSet::new();
    let mut cells = BTreeMap::new();
    while let Some(cell) = queue.pop_front() {
        if !visited.insert(cell) {
            continue;
        }
        if cell[1] < floor {
            continue;
        }
        let material = chunks.block_at(cell).ok_or(TrainError::Incomplete)?;
        if material == 0
            || material == terra_voxel::MaterialId::FluidWater as u16
            || material == train_parts::STATION
            || terra_core::rails::is_rail(material)
        {
            continue;
        }
        if !train_parts::is_part(material)
            && (journal.block(cell) != Some(material)
                || terra_voxel::MaterialId::from_raw(material).is_none()
                    && !properties.0.contains_key(&material))
        {
            continue;
        }
        if cells.len() >= MAX_PARTS
            || (0..3).any(|axis| cell[axis].abs_diff(start[axis]) > if axis == 1 { 12 } else { 64 })
        {
            return Err(TrainError::Limit);
        }
        cells.insert(cell, material);
        let facing = journal.object_state(cell).map_or(0, |state| state.facing);
        for offset in terra_core::train_boiler::attachment_offsets(material, facing).iter() {
            queue.push_back(std::array::from_fn(|axis| {
                cell[axis] + i64::from(offset[axis])
            }));
        }
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
    // Workshop assemblies start on a straight, level axle line; subsequent
    // rigid travel follows the rail's continuous curve and grade.
    let bogies: Vec<_> = cells
        .iter()
        .filter(|(_, material)| **material == train_parts::BOGIE)
        .map(|(cell, _)| *cell)
        .collect();
    if bogies.len() >= 2 {
        let same_x = bogies.iter().all(|cell| cell[0] == bogies[0][0]);
        let same_z = bogies.iter().all(|cell| cell[2] == bogies[0][2]);
        let axis = if same_x && !same_z {
            2
        } else if same_z && !same_x {
            0
        } else {
            return Err(TrainError::BogieAlignment);
        };
        let across = 2 - axis;
        let first = bogies[0];
        let mut along: Vec<_> = bogies.iter().map(|cell| cell[axis]).collect();
        along.sort_unstable();
        if bogies
            .iter()
            .any(|cell| cell[1] != first[1] || cell[across] != first[across])
        {
            return Err(TrainError::BogieAlignment);
        }
        if along.windows(2).any(|pair| pair[1] - pair[0] < 3)
            || along.last().unwrap() - along[0] > 15
        {
            return Err(TrainError::Wheelbase);
        }
        if bogies.iter().any(|cell| {
            journal.object_state(*cell).unwrap_or_default().facing % 2 != u8::from(axis == 0)
        }) || cells.iter().any(|(cell, material)| {
            *material == train_parts::COUPLER
                && journal.object_state(*cell).unwrap_or_default().facing % 2 != u8::from(axis == 0)
        }) {
            return Err(TrainError::BogieAlignment);
        }
    }
    if cells.keys().any(|cell| journal.crop_at(*cell).is_some()) {
        return Err(TrainError::AttachedProps);
    }
    let owners: BTreeSet<_> = cells
        .keys()
        .map(|cell| terra_voxel::global_to_section(*cell).0)
        .collect();
    for owner in owners {
        if chunks
            .loaded
            .get(&owner)
            .into_iter()
            .flat_map(|section| section.props.iter())
            .chain(journal.placed_props(owner))
            .any(|prop| {
                !prop.removed
                    && !journal.prop_removed(prop.section, prop.id)
                    && cells.contains_key(&crate::props::placement::support(prop))
            })
        {
            return Err(TrainError::AttachedProps);
        }
    }
    let anchor = *cells.keys().next().unwrap();
    let mut parts = Vec::new();
    let mut edits = Vec::new();
    let mut cargo = BTreeMap::new();
    let mut blocks = BTreeMap::new();
    let mut payloads = BTreeMap::new();
    let mut occupied_boxes = BTreeSet::new();
    for (&cell, &material) in &cells {
        if (0..3).any(|axis| cell[axis].abs_diff(station[axis]) > 64) {
            return Err(TrainError::OutOfReach);
        }
        if journal.gameplay().kingdoms.active_at(cell).is_some()
            || journal.gameplay().trees.owner(cell, material).is_some()
        {
            return Err(TrainError::Protected);
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
        if material == train_parts::BOGIE
            && !bogie_supported_with_facing(
                chunks,
                journal,
                cell,
                journal.object_state(cell).unwrap_or_default().facing,
            )
        {
            return Err(TrainError::Unsupported);
        }
        let state = journal.object_state(cell);
        if state.is_some_and(|state| {
            !state.valid_for(cell, material)
                || state.peer.is_some_and(|peer| !cells.contains_key(&peer))
        }) {
            // Multi-cell objects must enter and leave the world as a whole.
            return Err(TrainError::Incomplete);
        }
        if journal
            .belt_owner(cell)
            .is_some_and(|belt| !cells.contains_key(&belt.start) || !cells.contains_key(&belt.end))
        {
            return Err(TrainError::Incomplete);
        }
        let pose = state.unwrap_or_default();
        let index = parts.len() as u16;
        let inventory = matches!(material, 29 | train_parts::CARGO | 218)
            .then(|| journal.container_at(cell).cloned().unwrap_or_default());
        if let Some(inventory) = &inventory {
            cargo.insert(index, inventory.clone());
        }
        let mut boxes = Vec::new();
        let authored = crate::interaction_shapes::visit_collision_boxes_with(
            journal,
            properties,
            cell,
            |position| Some(cells.get(&position).copied().unwrap_or(0)),
            |mut bounds| {
                // Joined openings return their compound volumes for each cell.
                // One captured owner supplies each physical volume exactly once.
                let key: [u64; 6] = std::array::from_fn(|index| {
                    if index < 3 {
                        bounds.minimum[index].to_bits()
                    } else {
                        bounds.maximum[index - 3].to_bits()
                    }
                });
                if !occupied_boxes.insert(key) {
                    return;
                }
                for (axis, value) in cell.into_iter().enumerate() {
                    bounds.minimum[axis] -= value as f64;
                    bounds.maximum[axis] -= value as f64;
                }
                boxes.push(bounds);
            },
        );
        if !authored
            && terra_voxel::MaterialId::from_raw(material)
                .is_none_or(terra_voxel::MaterialId::is_solid)
        {
            boxes.push(ShapeBox {
                minimum: [0.0; 3],
                maximum: [1.0; 3],
                hinged: false,
            });
        }
        let registry = &journal.gameplay().kingdoms;
        let heraldry = registry.heraldry_at(cell).or_else(|| {
            chunks
                .village_settlements()
                .find(|site| site.domain().contains(cell))
                .and_then(|site| registry.domain_heraldry(&site.domain(), cell))
        });
        blocks.insert(
            index,
            terra_core::trains::TrainBlockView {
                boxes,
                tint: journal.tint_at(cell),
                heraldry,
                peer: pose
                    .peer
                    .map(|peer| std::array::from_fn(|axis| (peer[axis] - cell[axis]) as i16)),
                secondary: pose.secondary,
            },
        );
        payloads.insert(index, journal.train_payload(cell)?);
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
    let mut vehicle = Vehicle {
        name: String::new(),
        id: 1,
        owner: actor,
        kind,
        anchor,
        parts,
        cargo,
        blocks,
        payloads,
        couplings: [None, None],
        revision: 1,
        motion: Default::default(),
        steam: Default::default(),
        burn_seconds: cells
            .iter()
            .find(|(_, material)| **material == 218)
            .and_then(|(cell, _)| journal.furnace_at(*cell))
            .map_or(0.0, |state| f64::from(state.burn_remaining)),
    };
    vehicle.motion.fuel_available = crate::train_motion::fuel_available(&vehicle);
    if !vehicle.valid() {
        return Err(TrainError::Incomplete);
    }
    model_clearance(
        chunks,
        journal,
        &vehicle,
        &edits.iter().map(|part| part.cell).collect(),
        properties,
    )?;
    Ok(AssemblyPlan {
        station,
        start,
        actor,
        vehicle,
        assembling: true,
        registry_revision: journal.gameplay().trains.revision,
        parts: edits,
        properties: properties.clone(),
        source_motion: None,
    })
}
pub fn plan_dismantle(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    station: Cell,
    id: u64,
) -> Result<AssemblyPlan, TrainError> {
    crate::train_disassembly::prepare(chunks, journal, actor, station, id)
}

pub fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &AssemblyPlan,
) -> Result<u64, TrainError> {
    journal.commit_train_plan(chunks, plan)
}
pub fn part_boxes(origin: [f64; 3], part: &TrainPart) -> impl Iterator<Item = ShapeBox> {
    part_boxes_with(origin, part, train_parts::boxes(part.material, part.facing))
}
pub fn part_boxes_with(
    origin: [f64; 3],
    part: &TrainPart,
    boxes: Vec<ShapeBox>,
) -> impl Iterator<Item = ShapeBox> {
    boxes.into_iter().map(move |shape| ShapeBox {
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
                .enumerate()
                .flat_map(move |(index, _)| vehicle.transformed_shapes(index))
        })
        .filter(|shape| shape.intersects(low, high))
        .collect()
}
fn shapes(
    parts: &[TrainPart],
    blocks: &BTreeMap<u16, terra_core::trains::TrainBlockView>,
    index: usize,
) -> Vec<ShapeBox> {
    blocks.get(&(index as u16)).map_or_else(
        || train_parts::boxes(parts[index].material, parts[index].facing),
        |block| block.boxes.clone(),
    )
}
/// A borrowed rigid pose keeps authority, picking and placement in the same space.
struct Geometry<'a> {
    id: u64,
    origin: bevy_math::DVec3,
    rotation: bevy_math::DQuat,
    parts: &'a [TrainPart],
    blocks: &'a BTreeMap<u16, terra_core::trains::TrainBlockView>,
    bounds: [[f64; 3]; 2],
}
impl<'a> Geometry<'a> {
    fn vehicle(vehicle: &'a Vehicle) -> Self {
        let (origin, rotation) = vehicle.model_transform();
        Self {
            id: vehicle.id,
            origin: origin.into(),
            rotation: bevy_math::DQuat::from_array(rotation),
            parts: &vehicle.parts,
            blocks: &vehicle.blocks,
            bounds: vehicle.model_bounds(),
        }
    }
    fn view(vehicle: &'a terra_core::trains::VehicleView) -> Self {
        let (origin, rotation) = vehicle.model_transform();
        Self {
            id: vehicle.id,
            origin: origin.into(),
            rotation: bevy_math::DQuat::from_array(rotation),
            parts: &vehicle.parts,
            blocks: &vehicle.blocks,
            bounds: vehicle.model_bounds(),
        }
    }
    fn point(&self, point: [f64; 3]) -> [f64; 3] {
        (self.origin + self.rotation * bevy_math::DVec3::from_array(point)).to_array()
    }
    fn boxes(&self, index: usize) -> Vec<ShapeBox> {
        let part = &self.parts[index];
        shapes(self.parts, self.blocks, index)
            .into_iter()
            .map(|shape| {
                let mut minimum = [f64::INFINITY; 3];
                let mut maximum = [f64::NEG_INFINITY; 3];
                for corner in 0..8 {
                    let point = self.point(std::array::from_fn(|axis| {
                        f64::from(part.offset[axis])
                            + if corner & (1 << axis) == 0 {
                                shape.minimum[axis]
                            } else {
                                shape.maximum[axis]
                            }
                    }));
                    for axis in 0..3 {
                        minimum[axis] = minimum[axis].min(point[axis]);
                        maximum[axis] = maximum[axis].max(point[axis]);
                    }
                }
                ShapeBox {
                    minimum,
                    maximum,
                    hinged: false,
                }
            })
            .collect()
    }
    fn reserved(&self, cell: Cell) -> bool {
        let low = cell.map(|v| v as f64);
        let high = low.map(|v| v + 1.0);
        if !(0..3).all(|axis| self.bounds[0][axis] < high[axis] && self.bounds[1][axis] > low[axis])
        {
            return false;
        }
        self.parts.iter().enumerate().any(|(index, part)| {
            self.point(part.offset.map(|v| f64::from(v) + 0.5))
                .map(|v| v.floor() as i64)
                == cell
                || self
                    .boxes(index)
                    .iter()
                    .any(|shape| shape.intersects(low, high))
        })
    }
}
pub fn reserved(journal: &EditJournal, cell: Cell) -> bool {
    journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .any(|vehicle| Geometry::vehicle(vehicle).reserved(cell))
}
pub fn reserved_in(views: &[terra_core::trains::VehicleView], cell: Cell) -> bool {
    views
        .iter()
        .any(|vehicle| Geometry::view(vehicle).reserved(cell))
}
pub fn raycast(
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<TrainHit> {
    raycast_parts(
        journal
            .gameplay()
            .trains
            .vehicles
            .values()
            .map(Geometry::vehicle),
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
    raycast_parts(views.iter().map(Geometry::view), origin, direction, reach)
}
fn raycast_parts<'a>(
    vehicles: impl Iterator<Item = Geometry<'a>>,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
) -> Option<TrainHit> {
    if origin
        .iter()
        .chain(direction.iter())
        .any(|v| !v.is_finite())
        || !reach.is_finite()
        || !(0.0..=terra_core::rails::RAIL_BUILD_REACH).contains(&reach)
    {
        return None;
    }
    let mut hit: Option<TrainHit> = None;
    for vehicle in vehicles {
        if (ShapeBox {
            minimum: vehicle.bounds[0],
            maximum: vehicle.bounds[1],
            hinged: false,
        })
        .ray(origin, direction, reach)
        .is_none()
        {
            continue;
        }
        let inverse = vehicle.rotation.inverse();
        let local_origin =
            (inverse * (bevy_math::DVec3::from_array(origin) - vehicle.origin)).to_array();
        let local_direction = (inverse * bevy_math::DVec3::from_array(direction)).to_array();
        for (index, part) in vehicle.parts.iter().enumerate() {
            for shape in
                part_boxes_with([0.0; 3], part, shapes(vehicle.parts, vehicle.blocks, index))
            {
                if let Some((distance, _)) = shape.ray(local_origin, local_direction, reach)
                    && hit.is_none_or(|old| distance < old.distance)
                {
                    hit = Some(TrainHit {
                        vehicle: vehicle.id,
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
    for (index, part) in vehicle.parts.iter().enumerate() {
        for mut shape in vehicle.transformed_shapes(index) {
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
                        if terra_core::rails::is_rail(material)
                            && (part.material == train_parts::BOGIE
                                || matches!(part.material, 232..=234))
                        {
                            continue;
                        }
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

#[path = "train_building.rs"]
mod building;
pub use building::{BuildPlan, BuildRequest, commit_blueprint, plan_blueprint};
