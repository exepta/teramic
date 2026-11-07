use super::{PressureSource, SupplyToken, reservoirs::basin_shape};
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
use terra_core::plumbing::{self as core, BasinShape, Cell, DIRECTIONS};

const RAW_BUDGET: usize = 64;
const TICK: f32 = 0.2;
#[derive(Default)]
pub struct Runtime {
    elapsed: f32,
    cursor: Option<Cell>,
}
#[derive(Default, Debug, Clone, Copy)]
pub struct WorkCounts {
    pub scanned: usize,
    pub visited: usize,
    pub moved: u32,
}

#[derive(Clone)]
struct Pressure {
    source: PressureSource,
    head: f64,
    distance: usize,
    previous: Option<Cell>,
}
impl Pressure {
    fn preferred_to(&self, old: &Self) -> bool {
        // Stored water is a fallback for a disconnected finite remainder.
        // Its local pipe-center head must never displace a live source that
        // already reaches this cell, including a lower sea feeding suction.
        match (
            matches!(self.source, PressureSource::Stored(_)),
            matches!(old.source, PressureSource::Stored(_)),
        ) {
            (false, true) => true,
            (true, false) => false,
            _ => self.head > old.head || self.head == old.head && self.distance < old.distance,
        }
    }
}
fn next(cell: Cell, direction: Cell) -> Cell {
    std::array::from_fn(|axis| cell[axis] + direction[axis])
}
fn pose(journal: &EditJournal, cell: Cell) -> u8 {
    journal.object_state(cell).unwrap_or_default().facing
}
fn live(chunks: &ChunkStore, journal: &EditJournal, cell: Cell) -> bool {
    chunks.block_at(cell).is_some_and(|material| {
        core::is_part(material)
            && journal.block(cell) == Some(material)
            && journal.object_state(cell).is_some()
    })
}
fn pump_edge(
    journal: &EditJournal,
    cell: Cell,
    material: u16,
    direction: Cell,
    outgoing: bool,
) -> bool {
    if material != core::PUMP {
        return true;
    }
    let state = journal.kinetic_state(cell);
    if !state.running() {
        return false;
    }
    let forward = terra_core::kinetic::forward(pose(journal, cell));
    let sign = (if state.rpm > 0. { 1 } else { -1 }) * (if outgoing { 1 } else { -1 });
    direction == forward.map(|value| value * sign)
}

/// Each inlet pipe points toward the closest running pump. Follow only these
/// directed edges when lifting water to the inlet; the outlet gets pressure only
/// after crossing the pump. Reservoirs terminate the search at their wet wall.
fn suction_routes(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cells: &BTreeSet<Cell>,
) -> BTreeMap<Cell, Cell> {
    let mut seen: BTreeSet<_> = cells
        .iter()
        .copied()
        .filter(|&cell| {
            chunks.block_at(cell) == Some(core::PUMP) && journal.kinetic_state(cell).running()
        })
        .collect();
    let mut queue: VecDeque<_> = seen.iter().copied().collect();
    let mut routes = BTreeMap::new();
    while let Some(cell) = queue.pop_front() {
        let material = chunks.block_at(cell).unwrap();
        if matches!(material, core::BASIN | core::FLUID_INFUSER) {
            continue;
        }
        for direction in DIRECTIONS {
            let upstream = next(cell, direction);
            if !cells.contains(&upstream)
                || seen.contains(&upstream)
                || !crate::kingdom_access::can_transport(chunks, journal, upstream, cell)
            {
                continue;
            }
            let other = chunks.block_at(upstream).unwrap();
            if other == core::PUMP
                || matches!(other, core::FAUCET | core::FLUID_INFUSER)
                || !core::open(material, pose(journal, cell))
                || !core::open(other, pose(journal, upstream))
                || !core::joined(
                    material,
                    pose(journal, cell),
                    other,
                    pose(journal, upstream),
                    direction,
                )
                || !pump_edge(journal, cell, material, direction, false)
            {
                continue;
            }
            seen.insert(upstream);
            routes.insert(upstream, cell);
            queue.push_back(upstream);
        }
    }
    routes
}

#[derive(Clone, Copy)]
struct DemandHead {
    head: f64,
    distance: usize,
    cell: Cell,
}
impl PartialEq for DemandHead {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for DemandHead {}
impl PartialOrd for DemandHead {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DemandHead {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.head
            .total_cmp(&other.head)
            .then_with(|| self.distance.cmp(&other.distance))
            .then_with(|| self.cell.cmp(&other.cell))
    }
}

/// Lowest hydrostatic head that can reach an open consumer or a reservoir
/// with room. Bare/closed pipework is not a withdrawal request. Reservoirs
/// terminate routes and pump suction is handled separately with its direction.
fn gravity_demand(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cells: &BTreeSet<Cell>,
    states: &BTreeMap<Cell, super::PlumbingState>,
    basin_totals: &BTreeMap<BasinShape, u16>,
) -> BTreeMap<Cell, (f64, usize)> {
    let mut demand = BTreeMap::new();
    let mut pending = BTreeSet::new();
    let mut settled = BTreeSet::new();
    for &cell in cells {
        let material = chunks.block_at(cell).unwrap();
        if !core::open(material, pose(journal, cell)) {
            continue;
        }
        let required = match material {
            core::FAUCET | core::FLUID_INFUSER => cell[1] as f64 + 0.125,
            core::BASIN => {
                let Some(shape) = states[&cell].basin else {
                    continue;
                };
                let total = basin_totals[&shape];
                if total >= shape.capacity() {
                    continue;
                }
                let head = shape.origin[1] as f64
                    + f64::from(total) / (f64::from(shape.size[0]) * f64::from(shape.size[2]) * 8.);
                head.max(cell[1] as f64 + 0.125)
            }
            _ => continue,
        };
        demand.insert(cell, (required, 0));
        pending.insert(DemandHead {
            head: required,
            distance: 0,
            cell,
        });
    }
    // A min-head traversal settles each component member once. The pending
    // set owns at most one label per node, so the existing 512-node budget
    // also bounds this reverse proof and its six outgoing edge checks.
    for _ in 0..cells.len().min(core::MAX_COMPONENT) {
        let Some(DemandHead {
            head: required,
            distance,
            cell,
        }) = pending.pop_first()
        else {
            break;
        };
        settled.insert(cell);
        let material = chunks.block_at(cell).unwrap();
        for direction in DIRECTIONS {
            let upstream = next(cell, direction);
            if !cells.contains(&upstream)
                || settled.contains(&upstream)
                || !crate::kingdom_access::can_transport(chunks, journal, upstream, cell)
            {
                continue;
            }
            let other = chunks.block_at(upstream).unwrap();
            if matches!(other, core::PUMP | core::BASIN)
                || !core::open(other, pose(journal, upstream))
                || !core::joined(
                    material,
                    pose(journal, cell),
                    other,
                    pose(journal, upstream),
                    direction,
                )
            {
                continue;
            }
            let required = required.max(upstream[1] as f64 + 0.125);
            let distance = distance + 1;
            if demand
                .get(&upstream)
                .is_none_or(|&(old_head, old_distance)| {
                    required < old_head || required == old_head && distance < old_distance
                })
            {
                if let Some((head, distance)) = demand.insert(upstream, (required, distance)) {
                    pending.remove(&DemandHead {
                        head,
                        distance,
                        cell: upstream,
                    });
                }
                pending.insert(DemandHead {
                    head: required,
                    distance,
                    cell: upstream,
                });
            }
        }
    }
    demand
}

pub fn advance(
    runtime: &mut Runtime,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    elapsed: f32,
) -> Result<WorkCounts, String> {
    let mut work = WorkCounts::default();
    if !elapsed.is_finite() || elapsed <= 0. {
        return Ok(work);
    }
    runtime.elapsed = (runtime.elapsed + elapsed).min(1.);
    if runtime.elapsed < TICK {
        return Ok(work);
    }
    let elapsed = runtime.elapsed;
    runtime.elapsed = 0.;
    let mut candidates: Vec<_> = journal
        .plumbing_after(runtime.cursor)
        .take(RAW_BUDGET)
        .collect();
    if candidates.is_empty() {
        runtime.cursor = None;
        candidates = journal.plumbing_positions().take(RAW_BUDGET).collect();
    }
    let mut visited = BTreeSet::<Cell>::new();
    for start in candidates {
        runtime.cursor = Some(start);
        work.scanned += 1;
        if visited.contains(&start) || !live(chunks, journal, start) {
            continue;
        }
        let mut component = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        let mut complete = true;
        while let Some(cell) = queue.pop_front() {
            let material = chunks.block_at(cell).unwrap();
            if !core::open(material, pose(journal, cell)) {
                continue;
            }
            for direction in DIRECTIONS {
                if !core::connects(material, pose(journal, cell), direction) {
                    continue;
                }
                let adjacent = next(cell, direction);
                let Some(other) = chunks.block_at(adjacent) else {
                    complete = false;
                    continue;
                };
                if core::is_part(other)
                    && crate::kingdom_access::can_transport(chunks, journal, cell, adjacent)
                    && core::open(other, pose(journal, adjacent))
                    && core::joined(
                        material,
                        pose(journal, cell),
                        other,
                        pose(journal, adjacent),
                        direction,
                    )
                {
                    if !live(chunks, journal, adjacent) {
                        complete = false;
                        continue;
                    }
                    if component.insert(adjacent) {
                        if component.len() > core::MAX_COMPONENT {
                            complete = false;
                            queue.clear();
                            break;
                        }
                        queue.push_back(adjacent);
                    }
                }
            }
        }
        if work.visited + component.len() > core::MAX_COMPONENT {
            break;
        }
        work.visited += component.len();
        visited.extend(component.iter().copied());
        for &cell in &component {
            journal.clear_plumbing_supply_at(cell);
        }
        if !complete {
            for &cell in &component {
                if let Some(mut state) = journal.plumbing_at(cell).cloned()
                    && (state.intake.is_some() || state.flowing || state.water.flow != [0; 2])
                {
                    // An incomplete route cannot confirm intake or transit.
                    // Preserve stored water while retiring both activity flags.
                    state.intake = None;
                    state.flowing = false;
                    state.water.flow = [0; 2];
                    journal.set_plumbing(cell, state)?;
                }
                if chunks.block_at(cell) != Some(core::BASIN) {
                    continue;
                }
                if let Some(mut state) = journal.plumbing_at(cell).cloned()
                    && state.basin.take().is_some()
                {
                    journal.set_plumbing(cell, state)?;
                }
                if let Some(mut state) = journal.object_state(cell)
                    && state.facing != 0
                {
                    state.facing = 0;
                    let affected = journal.set_object_state(cell, Some(state))?;
                    chunks.invalidate_sections(affected);
                }
            }
            continue;
        }
        process(chunks, journal, terrain, &component, elapsed, &mut work)?;
    }
    Ok(work)
}

fn process(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    cells: &BTreeSet<Cell>,
    elapsed: f32,
    work: &mut WorkCounts,
) -> Result<(), String> {
    if journal.gameplay().revision > u64::MAX - ((cells.len() * 3 + 1) as u64) {
        return Err("Plumbing revision exhausted".into());
    }
    let mut states: BTreeMap<_, _> = cells
        .iter()
        .map(|&cell| (cell, journal.plumbing_at(cell).cloned().unwrap_or_default()))
        .collect();
    for state in states.values_mut() {
        state.water.flow = [0; 2];
        state.flowing = false;
        state.intake = None;
    }
    let mut groups = BTreeSet::new();
    let mut grouped = BTreeSet::new();
    for &cell in cells {
        if chunks.block_at(cell) != Some(core::BASIN) || grouped.contains(&cell) {
            continue;
        }
        let shape = basin_shape(chunks, journal, cell)
            .filter(|shape| shape.cells().all(|member| cells.contains(&member)));
        let mut members = BTreeSet::from([cell]);
        let mut pending = VecDeque::from([cell]);
        while let Some(member) = pending.pop_front() {
            for direction in DIRECTIONS {
                let adjacent = next(member, direction);
                if cells.contains(&adjacent)
                    && chunks.block_at(adjacent) == Some(core::BASIN)
                    && crate::kingdom_access::can_transport(chunks, journal, member, adjacent)
                    && members.insert(adjacent)
                {
                    pending.push_back(adjacent);
                }
            }
        }
        for member in members {
            if !states.contains_key(&member) {
                continue;
            }
            grouped.insert(member);
            states.get_mut(&member).unwrap().basin = shape;
            let mask = shape.map_or(0, |shape| {
                DIRECTIONS
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (side, direction)| {
                        mask | if shape.contains(next(member, *direction)) {
                            1 << side
                        } else {
                            0
                        }
                    })
            });
            if pose(journal, member) != mask {
                let mut state = journal.object_state(member).unwrap();
                state.facing = mask;
                let affected = journal.set_object_state(member, Some(state))?;
                chunks.invalidate_sections(affected);
            }
        }
        if let Some(shape) = shape {
            groups.insert((shape.origin, shape.size));
        }
    }
    let mut pressures: BTreeMap<Cell, Pressure> = BTreeMap::new();
    let suction = suction_routes(chunks, journal, cells);
    let mut basin_totals = BTreeMap::new();
    // A finite basin is levelled bottom-up; its head, not its module count, drives gravity.
    for (origin, size) in &groups {
        let shape = BasinShape {
            origin: *origin,
            size: *size,
        };
        let total: u32 = shape
            .cells()
            .map(|cell| u32::from(states.get(&cell).map_or(0, |state| state.water.amount)))
            .sum();
        let head =
            origin[1] as f64 + f64::from(total) / (f64::from(size[0]) * f64::from(size[2]) * 8.);
        basin_totals.insert(shape, total as u16);
        for (cell, amount) in shape.levelled_cells(total as u16) {
            states.get_mut(&cell).unwrap().water.amount = amount;
        }
        for cell in shape.cells() {
            pressures.insert(
                cell,
                Pressure {
                    source: PressureSource::Basin(shape),
                    head,
                    distance: 0,
                    previous: None,
                },
            );
        }
    }
    let gravity_demand = gravity_demand(chunks, journal, cells, &states, &basin_totals);
    // Closing an upstream valve retires its pressure, not the water already
    // beyond it. Nonempty passive storage can drain along a live gravity route
    // to an open consumer. Its head is the pipe center, so it cannot lift water
    // into a higher cell or reactivate a stopped pump. The finite draw list
    // below accounts for every unit, including the final unit in a level pipe.
    for &cell in cells {
        let material = chunks.block_at(cell).unwrap();
        let head = cell[1] as f64 + 0.5;
        if states[&cell].water.amount == 0
            || matches!(material, core::BASIN | core::PUMP)
            || !core::open(material, pose(journal, cell))
            || !suction.contains_key(&cell)
                && gravity_demand
                    .get(&cell)
                    .is_none_or(|(required, _)| head <= *required)
            || pressures.get(&cell).is_some_and(|old| old.head >= head)
        {
            continue;
        }
        pressures.insert(
            cell,
            Pressure {
                source: PressureSource::Stored(cell),
                head,
                distance: 0,
                previous: None,
            },
        );
    }
    for &cell in cells {
        let material = chunks.block_at(cell).unwrap();
        if matches!(material, core::BASIN | core::FLUID_INFUSER)
            || !core::open(material, pose(journal, cell))
        {
            continue;
        }
        let intake_ports = if core::is_pipe(material) {
            super::connection_mask(journal, cell, material, pose(journal, cell), |p| {
                chunks.block_at(p)
            })
        } else {
            core::ports(material, pose(journal, cell))
        };
        for (side, direction) in DIRECTIONS.into_iter().enumerate() {
            if intake_ports & (1 << side) == 0
                || !pump_edge(journal, cell, material, direction, false)
            {
                continue;
            }
            let source = next(cell, direction);
            let Some(other) = chunks.block_at(source) else {
                continue;
            };
            if core::is_part(other)
                || !crate::kingdom_access::can_transport(chunks, journal, source, cell)
            {
                continue;
            }
            let fluid = chunks.fluid_cell_at(terrain, journal, source, other);
            if fluid.amount == 0 {
                continue;
            }
            let mouth: [f64; 3] = std::array::from_fn(|axis| 0.5 - direction[axis] as f64 * 0.4999);
            if !crate::water::space::FluidSpace::at(journal, source, |p| chunks.block_at(p))
                .contains(fluid, mouth)
            {
                continue;
            }
            let head = source[1] as f64 + fluid.height();
            let pumped = suction.contains_key(&cell)
                || material == core::PUMP && journal.kinetic_state(cell).running();
            if !pumped
                && gravity_demand
                    .get(&cell)
                    .is_none_or(|(required, _)| head <= *required)
            {
                continue;
            }
            if material != core::PUMP
                && !suction.contains_key(&cell)
                && head <= cell[1] as f64 + 0.125
            {
                continue;
            }
            let room = 8 - states[&cell].water.amount;
            let admitted = super::intake::take(chunks, journal, terrain, source, room)?;
            if admitted > 0 {
                let state = states.get_mut(&cell).unwrap();
                state.water.amount += admitted;
                state.flowing = true;
                state.intake = Some(core::IntakeActivity {
                    source,
                    direction: direction.map(|v| v as i8),
                    units: admitted,
                });
                work.moved += u32::from(admitted);
            }
            pressures.insert(
                cell,
                Pressure {
                    source: PressureSource::Natural(source, other),
                    head,
                    distance: 0,
                    previous: None,
                },
            );
        }
        if material == core::PUMP && journal.kinetic_state(cell).running() {
            let state = states.get_mut(&cell).unwrap();
            state.pump_credit = (state.pump_credit
                + journal.kinetic_state(cell).rpm.abs() / 30. * 8. * elapsed)
                .min(8.);
            if state.water.amount > 0 {
                pressures.insert(
                    cell,
                    Pressure {
                        source: PressureSource::Pump(
                            cell,
                            journal.kinetic_state(cell).rpm.signum() as i8,
                        ),
                        head: f64::INFINITY,
                        distance: 0,
                        previous: None,
                    },
                );
            }
        }
    }
    let mut queue: VecDeque<_> = pressures.keys().copied().collect();
    while let Some(cell) = queue.pop_front() {
        let pressure = pressures[&cell].clone();
        let material = chunks.block_at(cell).unwrap();
        // Both terminals have an inlet, never a return outlet. In particular,
        // finite water already in a faucet must drain there rather than pump
        // itself backwards into a pipe emptied by the preceding tick.
        if matches!(material, core::FAUCET | core::FLUID_INFUSER) {
            continue;
        }
        for direction in DIRECTIONS {
            let adjacent = next(cell, direction);
            if !cells.contains(&adjacent)
                || !crate::kingdom_access::can_transport(chunks, journal, cell, adjacent)
            {
                continue;
            }
            if matches!(pressure.source, PressureSource::Stored(_))
                && suction
                    .get(&cell)
                    .is_some_and(|&pumpward| adjacent != pumpward)
            {
                continue;
            }
            let other = chunks.block_at(adjacent).unwrap();
            // An open reservoir keeps its own hydrostatic head. Inlet admission
            // below handles its aggregate room; pump pressure cannot cross empty tank air.
            if other == core::BASIN {
                continue;
            }
            if !core::open(material, pose(journal, cell))
                || !core::open(other, pose(journal, adjacent))
                || !core::joined(
                    material,
                    pose(journal, cell),
                    other,
                    pose(journal, adjacent),
                    direction,
                )
                || !pump_edge(journal, cell, material, direction, true)
                || !pump_edge(
                    journal,
                    adjacent,
                    other,
                    direction.map(|value| -value),
                    false,
                )
            {
                continue;
            }
            let boosted = other == core::PUMP && journal.kinetic_state(adjacent).running();
            let drawn_to_inlet = suction.get(&cell) == Some(&adjacent);
            if !boosted && !drawn_to_inlet && pressure.head <= adjacent[1] as f64 + 0.125 {
                continue;
            }
            let candidate = if boosted {
                Pressure {
                    source: PressureSource::Pump(
                        adjacent,
                        journal.kinetic_state(adjacent).rpm.signum() as i8,
                    ),
                    head: f64::INFINITY,
                    distance: pressure.distance + 1,
                    previous: Some(cell),
                }
            } else {
                Pressure {
                    distance: pressure.distance + 1,
                    previous: Some(cell),
                    ..pressure.clone()
                }
            };
            if pressures
                .get(&adjacent)
                .is_none_or(|old| candidate.preferred_to(old))
            {
                pressures.insert(adjacent, candidate);
                queue.push_back(adjacent);
            }
        }
    }
    let mut ordered: Vec<_> = pressures
        .iter()
        .map(|(&cell, pressure)| (pressure.distance, cell))
        .collect();
    ordered.sort();
    for (_, cell) in ordered {
        let material = chunks.block_at(cell).unwrap();
        if matches!(material, core::FAUCET | core::FLUID_INFUSER) {
            continue;
        }
        for direction in DIRECTIONS {
            let adjacent = next(cell, direction);
            if !crate::kingdom_access::can_transport(chunks, journal, cell, adjacent) {
                continue;
            }
            let Some(target) = pressures.get(&adjacent) else {
                continue;
            };
            let other = chunks.block_at(adjacent).unwrap();
            if target.distance <= pressures[&cell].distance
                && !matches!(other, core::PUMP | core::BASIN)
            {
                continue;
            }
            if matches!(pressures[&cell].source, PressureSource::Stored(_)) {
                if let Some(&pumpward) = suction.get(&cell) {
                    if adjacent != pumpward {
                        continue;
                    }
                } else {
                    let (Some(&(head, distance)), Some(&(target_head, target_distance))) =
                        (gravity_demand.get(&cell), gravity_demand.get(&adjacent))
                    else {
                        continue;
                    };
                    // Passive finite storage only drains toward a real outlet.
                    // Other distance-zero storage seeds must not pump it into an
                    // emptied upstream branch and back on the following tick.
                    if target_head > head || target_head == head && target_distance >= distance {
                        continue;
                    }
                }
            }
            if !core::joined(
                material,
                pose(journal, cell),
                other,
                pose(journal, adjacent),
                direction,
            ) || !pump_edge(journal, cell, material, direction, true)
                || !pump_edge(
                    journal,
                    adjacent,
                    other,
                    direction.map(|value| -value),
                    false,
                )
            {
                continue;
            }
            if material == core::BASIN && other == core::BASIN {
                continue;
            }
            // Equal head cannot return water to the reservoir that supplied this pipe.
            if other == core::BASIN && pressures[&cell].head <= target.head {
                continue;
            }
            let target_basin = if other == core::BASIN {
                states[&adjacent].basin
            } else {
                None
            };
            let room = if other == core::BASIN {
                let Some(shape) = target_basin else {
                    continue;
                };
                let total = basin_totals[&shape];
                let head = pressures[&cell].head;
                let capacity = if head.is_finite() {
                    ((head - shape.origin[1] as f64)
                        * f64::from(shape.size[0])
                        * f64::from(shape.size[2])
                        * 8.)
                        .floor()
                        .clamp(0., f64::from(shape.capacity())) as u16
                } else {
                    shape.capacity()
                };
                capacity.saturating_sub(total).min(8) as u8
            } else {
                8 - states[&adjacent].water.amount
            };
            let mut amount = states[&cell].water.amount.min(room);
            if material == core::PUMP {
                amount = amount.min(states[&cell].pump_credit.floor() as u8);
            }
            if amount == 0 {
                continue;
            }
            states.get_mut(&cell).unwrap().water.amount -= amount;
            if let Some(shape) = target_basin {
                *basin_totals.get_mut(&shape).unwrap() += u16::from(amount);
            } else {
                states.get_mut(&adjacent).unwrap().water.amount += amount;
            }
            if material == core::BASIN
                && let Some(shape) = states[&cell].basin
            {
                *basin_totals.get_mut(&shape).unwrap() -= u16::from(amount);
            }
            states.get_mut(&cell).unwrap().water.flow = [direction[0] as i8, direction[2] as i8];
            states.get_mut(&cell).unwrap().flowing = true;
            states.get_mut(&adjacent).unwrap().water.flow =
                [direction[0] as i8, direction[2] as i8];
            states.get_mut(&adjacent).unwrap().flowing = true;
            if material == core::PUMP {
                states.get_mut(&cell).unwrap().pump_credit -= f32::from(amount);
            }
            work.moved += u32::from(amount);
        }
    }
    for (origin, size) in groups {
        let shape = BasinShape { origin, size };
        for (cell, units) in shape.levelled_cells(basin_totals[&shape]) {
            states.get_mut(&cell).unwrap().water.amount = units;
        }
    }
    for (&cell, state) in &states {
        journal.set_plumbing(cell, state.clone())?;
    }
    for (&cell, pressure) in &pressures {
        if chunks.block_at(cell).is_some_and(|material| {
            matches!(material, core::FAUCET | core::FLUID_INFUSER)
                && core::open(material, pose(journal, cell))
        }) {
            let mut route = Vec::new();
            let mut draw = Vec::new();
            let mut cursor = Some(cell);
            let mut seen = BTreeSet::new();
            let mut crossed_pump = false;
            while let Some(member) = cursor {
                if !seen.insert(member) || route.len() >= core::MAX_COMPONENT {
                    route.clear();
                    break;
                }
                let material = chunks.block_at(member).unwrap();
                route.push((member, material, pose(journal, member)));
                crossed_pump |= material == core::PUMP;
                if !crossed_pump {
                    draw.push(member);
                }
                cursor = pressures[&member].previous;
            }
            if route.is_empty() {
                continue;
            }
            if let PressureSource::Basin(shape) = pressure.source {
                let mut draw_seen: BTreeSet<_> = draw.iter().copied().collect();
                for member in shape.cells() {
                    if draw_seen.insert(member) {
                        draw.push(member);
                    }
                }
            }
            journal.set_plumbing_supply(
                cell,
                SupplyToken {
                    epoch: journal.plumbing_epoch(),
                    source: pressure.source.clone(),
                    pump_directions: route
                        .iter()
                        .filter(|(_, material, _)| *material == core::PUMP)
                        .map(|(cell, _, _)| {
                            (*cell, journal.kinetic_state(*cell).rpm.signum() as i8)
                        })
                        .collect(),
                    route: Arc::new(route),
                    draw: Arc::new(draw),
                },
            );
        }
    }
    for &cell in cells {
        if chunks.block_at(cell) == Some(core::FAUCET)
            && super::tap_available(chunks, journal, cell, 1)
        {
            discharge(chunks, journal, terrain, cell)?;
        }
    }
    Ok(())
}

fn discharge(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    tap: Cell,
) -> Result<(), String> {
    let below = next(tap, [0, -1, 0]);
    let Some(mut material) = chunks.block_at(below) else {
        return Ok(());
    };
    // A falling stream belongs to this finite transfer until it reaches a basin.
    // The terrain-fluid solver cannot enter a basin module's sealed voxel.
    let mut destination_cell = below;
    for depth in 1..=32 {
        let cell = next(tap, [0, -depth, 0]);
        if !crate::kingdom_access::can_transport(chunks, journal, tap, cell) {
            return Ok(());
        }
        let Some(candidate) = chunks.block_at(cell) else {
            break;
        };
        if candidate == core::BASIN {
            destination_cell = cell;
            material = candidate;
            break;
        }
        if !crate::water::can_hold_fluid(candidate) {
            break;
        }
        let direction = terra_core::kinetic::forward(pose(journal, tap));
        let mouth = [
            0.5 + direction[0] as f64 * 0.13,
            0.5 + direction[2] as f64 * 0.13,
        ];
        if crate::water::space::FluidSpace::at(journal, cell, |p| chunks.block_at(p))
            .solids
            .iter()
            .any(|solid| {
                mouth[0] >= solid.minimum[0]
                    && mouth[0] <= solid.maximum[0]
                    && mouth[1] >= solid.minimum[2]
                    && mouth[1] <= solid.maximum[2]
            })
        {
            break;
        }
    }
    let maximum = core::faucet_units(pose(journal, tap));
    let mut units = (1..=maximum)
        .rev()
        .find(|&units| super::tap_available(chunks, journal, tap, units))
        .unwrap_or(0);
    if units == 0 {
        return Ok(());
    }
    if journal.gameplay().revision > u64::MAX - (core::MAX_BASIN as u64 + 1) {
        return Err("Faucet revision exhausted".into());
    }
    if material == core::BASIN {
        let Some(shape) = journal
            .plumbing_at(destination_cell)
            .and_then(|state| state.basin)
        else {
            return Ok(());
        };
        let total: u16 = shape
            .cells()
            .map(|cell| {
                u16::from(
                    journal
                        .plumbing_at(cell)
                        .map_or(0, |state| state.water.amount),
                )
            })
            .sum();
        units = units.min(shape.capacity().saturating_sub(total).min(8) as u8);
        if units == 0 || !super::fill_basin(chunks, journal, destination_cell, units)? {
            return Ok(());
        }
    } else {
        if !crate::water::can_hold_fluid(material) {
            return Ok(());
        }
        let mut destination = chunks.fluid_cell_at(terrain, journal, below, material);
        if destination.source || destination.amount >= 8 {
            return Ok(());
        }
        let owner = terra_voxel::global_to_section(below).0;
        if journal.fluid_epoch(owner) == u64::MAX
            || (-1..=1).any(|z| {
                (-1..=1).any(|y| {
                    (-1..=1).any(|x| {
                        owner.offset([x, y, z]).map_or(true, |section| {
                            journal.water_mesh_epoch(section) > u64::MAX - 16
                        })
                    })
                })
            })
        {
            return Err("Faucet fluid epoch exhausted".into());
        }
        units = units.min(8 - destination.amount);
        let space =
            crate::water::space::FluidSpace::at(journal, below, |cell| chunks.block_at(cell));
        while units > 0
            && space
                .clamp(crate::water::FluidCell {
                    amount: destination.amount + units,
                    ..destination
                })
                .amount
                != destination.amount + units
        {
            units -= 1;
        }
        if units == 0 {
            return Ok(());
        }
        destination.amount += units;
        journal
            .set_fluid(below, destination)
            .map_err(|error| error.to_string())?;
    }
    if !super::take_tap(chunks, journal, tap, units)? {
        return Err("Faucet supply changed during transfer".into());
    }
    Ok(())
}
