//! Junctions grant turns; the existing belt substep/occupancy path moves the stack.
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terra_core::{WorldDrop, logistics};
use terra_voxel::{SectionPos, global_to_section};

const DISCOVERY_BUDGET: usize = 128;
const ROUTES_PER_PORT: usize = 16;
const CROSSING_CLEARANCE: f64 = 0.260001;

#[derive(Default)]
pub(super) struct Routing {
    turns: BTreeMap<Cell, u8>,
    active: BTreeMap<Cell, VecDeque<Route>>,
    cursor: Option<Cell>,
    active_cursor: Option<(Cell, u64)>,
    discovery: Option<(SectionPos, Cell)>,
}

struct Route {
    id: u64,
    material: u16,
    facing: u8,
    entry: [f64; 3],
    merge: [f64; 3],
    outlet: [f64; 3],
    source: Span,
    target: Span,
    incoming: u8,
    outgoing: Cell,
    phase: u8,
}

fn same_geometry(a: Span, b: Span) -> bool {
    a.controller == b.controller
        && a.start == b.start
        && a.end == b.end
        && a.step == b.step
        && a.length == b.length
        && a.facing == b.facing
        && a.linked == b.linked
}

fn live_span(span: Span, journal: &EditJournal) -> bool {
    segment_at(journal, span.controller).is_some_and(|segment| same_geometry(segment.span, span))
}

fn single_item(
    route: &mut Route,
    journal: &mut EditJournal,
    indices: &mut BTreeMap<u64, usize>,
    occupancy: &mut DropIndex,
) -> bool {
    let index = indices[&route.id];
    let source = &journal.gameplay().drops[index];
    if source.stack.count == 1 {
        return true;
    }
    let game = journal.gameplay();
    if !game.can_drop() || game.revision == u64::MAX {
        return false;
    }
    let mut single = source.clone();
    single.id = game.next_drop + 1;
    single.stack.count = 1;
    let game = journal.gameplay_mut();
    game.drops[index].stack.count -= 1;
    game.next_drop = single.id;
    game.revision += 1;
    route.id = single.id;
    indices.insert(single.id, game.drops.len());
    occupancy
        .entry(single.position.map(|v| v.floor() as i64))
        .or_default()
        .push((single.id, single.position));
    game.drops.push(single);
    true
}

fn router(position: Cell, journal: &EditJournal) -> Option<(u16, u8)> {
    let material = journal
        .block(position)
        .filter(|m| logistics::is_router(*m))?;
    let state = journal
        .object_state(position)
        .filter(|s| s.valid_for(position, material))?;
    Some((material, state.facing))
}

pub fn loaded(position: Cell, chunks: &ChunkStore, journal: &EditJournal) -> bool {
    router(position, journal)
        .is_some_and(|(material, _)| chunks.block_at(position) == Some(material))
}

fn center(position: Cell, journal: &EditJournal) -> Option<[f64; 3]> {
    let below = [position[0], position[1] - 1, position[2]];
    let segment = segment_at(journal, below)?;
    (segment.span.step[1] == 0).then_some(())?;
    segment.drop_center()
}

/// Pure persisted geometry: waiting/loaded-again junction stacks never become auto-pickup loot.
pub fn carrying(drop: &WorldDrop, journal: &EditJournal) -> Option<Cell> {
    if !journal.has_logistics()
        || drop
            .position
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_001.0)
    {
        return None;
    }
    let [x, y, z] = drop.position.map(|v| v.floor() as i64);
    for [dx, dz] in [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] {
        let px = x + dx;
        let pz = z + dz;
        for dy in [0, 1] {
            let position = [px, y + dy, pz];
            let Some(_) = router(position, journal) else {
                continue;
            };
            let Some(center) = center(position, journal) else {
                continue;
            };
            let relative: [f64; 3] = std::array::from_fn(|i| drop.position[i] - center[i]);
            if relative[1].abs() > 0.08 {
                continue;
            }
            if relative[0].abs() <= 0.51 && relative[2].abs() <= 0.51 {
                return Some(position);
            }
            for orientation in 0..4 {
                let direction = kinetic::forward(orientation);
                let along = dot(relative, direction);
                let across = relative[0] * direction[2] as f64 - relative[2] * direction[0] as f64;
                if (0.5..0.999999).contains(&along)
                    && across.abs() < 0.13
                    && segment_at(
                        journal,
                        [px + direction[0], position[1] - 1, pz + direction[2]],
                    )
                    .is_some()
                {
                    return Some(position);
                }
            }
        }
    }
    None
}

fn speed(segment: Segment, chunks: &ChunkStore, journal: &EditJournal) -> Option<[f64; 3]> {
    if !segment.span.loaded(chunks, journal) || segment.span.step[1] != 0 {
        return None;
    }
    let p = segment.position();
    let drive = journal.kinetic_state(p);
    if !drive.running() || !drive.rpm.is_finite() || drive.rpm.abs() > f32::from(kinetic::MAX_RPM) {
        return None;
    }
    let raw = journal.object_state(p)?.facing;
    let facing = if journal.block(p) == Some(kinetic::CONVEYOR) {
        kinetic::belt_facing(raw)
    } else {
        raw
    };
    let axis = kinetic::axis(segment.span.facing);
    let sign = (0..3)
        .map(|i| kinetic::axis(facing)[i] * axis[i])
        .sum::<i64>() as f32;
    let magnitude = belt_speed(drive.rpm * sign) * segment.span.flow_sign();
    Some(segment.span.direction().map(|v| f64::from(v * magnitude)))
}

fn dot(a: [f64; 3], b: Cell) -> f64 {
    (0..3).map(|i| a[i] * b[i] as f64).sum()
}

fn discover(
    cursor: &mut Option<(SectionPos, Cell)>,
    chunks: &ChunkStore,
    journal: &EditJournal,
    occupancy: &DropIndex,
    mut accept: impl FnMut(Cell) -> bool,
) -> (Vec<Cell>, usize) {
    let mut sections = BTreeSet::new();
    for cell in occupancy.keys() {
        let low = global_to_section([cell[0] - 1, cell[1], cell[2] - 1]).0;
        let high = global_to_section([cell[0] + 1, cell[1] + 1, cell[2] + 1]).0;
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    let section = SectionPos { x, y, z };
                    if chunks.loaded.contains_key(&section) {
                        sections.insert(section);
                    }
                }
            }
        }
    }
    let sections: Vec<_> = sections.into_iter().collect();
    if sections.is_empty() {
        return (Vec::new(), 0);
    }
    let resume = cursor.and_then(|(section, cell)| {
        sections
            .binary_search(&section)
            .ok()
            .map(|index| (index, cell))
    });
    let start = resume.map_or_else(
        || {
            cursor.map_or(0, |(section, _)| sections.partition_point(|s| *s < section))
                % sections.len()
        },
        |(index, _)| index,
    );
    let mut ports = Vec::new();
    let mut inspected = 0;
    for offset in 0..sections.len() + usize::from(resume.is_some()) {
        let section = sections[(start + offset) % sections.len()];
        let after = (offset == 0)
            .then_some(resume.map(|(_, cell)| cell))
            .flatten();
        let through = (offset == sections.len())
            .then_some(resume.map(|(_, cell)| cell))
            .flatten();
        for position in journal.logistics_in_section_after(section, after) {
            if through.is_some_and(|last| position > last) {
                break;
            }
            inspected += 1;
            *cursor = Some((section, position));
            let nearby = (position[0] - 1..=position[0] + 1).any(|x| {
                (position[1] - 1..=position[1]).any(|y| {
                    (position[2] - 1..=position[2] + 1).any(|z| occupancy.contains_key(&[x, y, z]))
                })
            });
            if nearby && accept(position) {
                ports.push(position);
            }
            if inspected >= DISCOVERY_BUDGET || ports.len() >= logistics::PORT_BUDGET {
                ports.sort_unstable();
                return (ports, inspected);
            }
        }
    }
    ports.sort_unstable();
    (ports, inspected)
}

fn output(
    position: Cell,
    direction: Cell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Segment, f64)> {
    let segment = outlet_segment(position, direction, journal)?;
    let magnitude = dot(speed(segment, chunks, journal)?, direction);
    (magnitude > 0.05).then_some((segment, magnitude))
}

fn outlet_segment(position: Cell, direction: Cell, journal: &EditJournal) -> Option<Segment> {
    // The outgoing center crosses into this cell; the strip under the hood alone is not an outlet.
    let below = [
        position[0] + direction[0],
        position[1] - 1,
        position[2] + direction[2],
    ];
    segment_at(journal, below)
}

fn nearby_ids(position: Cell, occupancy: &DropIndex) -> impl Iterator<Item = u64> + '_ {
    (position[0] - 1..=position[0] + 1).flat_map(move |x| {
        (position[2] - 1..=position[2] + 1).flat_map(move |z| {
            (position[1] - 1..=position[1]).flat_map(move |y| {
                occupancy
                    .get(&[x, y, z])
                    .into_iter()
                    .flatten()
                    .map(|(id, _)| *id)
            })
        })
    })
}

fn departed(material: u16, facing: u8, relative: [f64; 3]) -> Option<(u8, Cell)> {
    (0..3).find_map(|choice| {
        let direction = if material == logistics::BELT_MERGER {
            kinetic::forward(facing)
        } else {
            logistics::distributor_output(facing, choice)?
        };
        let across = relative[0] * direction[2] as f64 - relative[2] * direction[0] as f64;
        let on_turn = if material == logistics::BELT_MERGER {
            dot(relative, direction) < 0.5
        } else {
            direction != kinetic::forward(facing)
                && (-0.015..0.5).contains(&dot(relative, kinetic::forward((facing + 2) % 4)))
        };
        (dot(relative, direction) > 0.015 && (across.abs() < 0.015 || on_turn))
            .then_some((choice, direction))
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "Keep the borrowed per-tick lookup views explicit in this admission path"
)]
fn candidate(
    position: Cell,
    direction: Cell,
    center: [f64; 3],
    journal: &EditJournal,
    chunks: &ChunkStore,
    occupancy: &DropIndex,
    indices: &BTreeMap<u64, usize>,
    assigned: &BTreeSet<u64>,
) -> Option<(usize, Segment)> {
    let (material, facing) = router(position, journal)?;
    let mut nearest: Option<(f64, usize, Segment)> = None;
    for x in position[0] - 1..=position[0] + 1 {
        for z in position[2] - 1..=position[2] + 1 {
            for y in position[1] - 1..=position[1] {
                for &(id, _) in occupancy.get(&[x, y, z]).into_iter().flatten() {
                    if assigned.contains(&id) {
                        continue;
                    }
                    let index = *indices.get(&id)?;
                    let drop = &journal.gameplay().drops[index];
                    let delta = std::array::from_fn(|i| drop.position[i] - center[i]);
                    let along = dot(delta, direction);
                    let across = delta[0] * direction[2] as f64 - delta[2] * direction[0] as f64;
                    let recovering = carrying(drop, journal) == Some(position);
                    if recovering && departed(material, facing, delta).is_some() {
                        continue;
                    }
                    // Custody is not an input direction. In particular, a straight
                    // head already under the hood must not masquerade as both sides.
                    if !(-0.000001..=1.3).contains(&along)
                        || across.abs() > 0.38
                        || along + 0.000001 < across.abs()
                        || delta[1].abs() > 0.08
                    {
                        continue;
                    }
                    let segment = carrying_segment(drop, journal).or_else(|| {
                        recovering
                            .then(|| {
                                segment_at(
                                    journal,
                                    [
                                        position[0] + direction[0],
                                        position[1] - 1,
                                        position[2] + direction[2],
                                    ],
                                )
                                .or_else(|| {
                                    segment_at(journal, [position[0], position[1] - 1, position[2]])
                                })
                            })
                            .flatten()
                    });
                    let Some(segment) = segment else {
                        continue;
                    };
                    if (!recovering || along > 0.51)
                        && (along < 0.35
                            || speed(segment, chunks, journal)
                                .is_none_or(|v| dot(v, direction) >= -0.05))
                    {
                        continue;
                    }
                    let distance = delta.iter().map(|v| v * v).sum::<f64>();
                    if nearest.as_ref().is_none_or(|old| {
                        distance < old.0
                            || distance == old.0 && id < journal.gameplay().drops[old.1].id
                    }) {
                        nearest = Some((distance, index, segment));
                    }
                }
            }
        }
    }
    nearest.map(|(_, index, segment)| (index, segment))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Admission combines per-tick lookup views with the port's pending FIFO"
)]
fn grant(
    position: Cell,
    start: u8,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    occupancy: &DropIndex,
    indices: &BTreeMap<u64, usize>,
    assigned: &BTreeSet<u64>,
    pending: Option<&VecDeque<Route>>,
) -> Option<(Route, u8)> {
    let (material, facing) = router(position, journal)?;
    if !loaded(position, chunks, journal) {
        return None;
    }
    let center = center(position, journal)?;
    let recovery = nearby_ids(position, occupancy)
        .filter_map(|id| {
            let index = *indices.get(&id)?;
            let drop = &journal.gameplay().drops[index];
            if assigned.contains(&id) || carrying(drop, journal) != Some(position) {
                return None;
            }
            let relative = std::array::from_fn(|i| drop.position[i] - center[i]);
            let departed = departed(material, facing, relative)
                .filter(|(_, direction)| outlet_segment(position, *direction, journal).is_some())?;
            let source = segment_at(journal, [position[0], position[1] - 1, position[2]])?;
            Some((index, source, departed))
        })
        .max_by(|a, b| {
            let progress = |route: &(usize, Segment, (u8, Cell))| {
                let relative = std::array::from_fn(|i| {
                    journal.gameplay().drops[route.0].position[i] - center[i]
                });
                dot(relative, route.2.1)
            };
            progress(a)
                .total_cmp(&progress(b))
                .then_with(|| b.0.cmp(&a.0))
        });
    for offset in 0..3 {
        let choice = (start + offset) % 3;
        if material == logistics::BELT_MERGER
            && recovery.is_none()
            && pending.is_some_and(|routes| {
                routes
                    .iter()
                    .any(|route| route.incoming == choice && route.phase == 0)
            })
        {
            // One approaching head per inlet is enough to pipeline the turn.
            // Do not prefill other lanes while this round's inlet is still
            // advancing: dense straight queues otherwise overtake side queues.
            return None;
        }
        let incoming = if material == logistics::BELT_MERGER {
            kinetic::forward((facing + [1, 2, 3][usize::from(choice)]) % 4)
        } else {
            kinetic::forward((facing + 2) % 4)
        };
        let (index, source) = if let Some((index, source, (output, _))) = recovery {
            if material != logistics::BELT_MERGER && output != choice {
                continue;
            }
            (index, source)
        } else {
            let Some(candidate) = candidate(
                position, incoming, center, journal, chunks, occupancy, indices, assigned,
            ) else {
                continue;
            };
            candidate
        };
        let drop = &journal.gameplay().drops[index];
        let relative = std::array::from_fn(|i| drop.position[i] - center[i]);
        let departed = recovery.map(|(_, _, (_, direction))| direction);
        let outgoing = if material == logistics::BELT_MERGER {
            kinetic::forward(facing)
        } else {
            let direction = logistics::distributor_output(facing, choice)?;
            if departed.is_some_and(|departed| departed != direction) {
                continue;
            }
            if departed.is_none()
                && !accepts_output(
                    journal
                        .distributor_at(position)
                        .copied()
                        .unwrap_or_default(),
                    choice,
                    drop.stack.item,
                )
            {
                continue;
            }
            direction
        };
        let target = if departed.is_some() && drop.stack.count == 1 {
            outlet_segment(position, outgoing, journal)
        } else {
            output(position, outgoing, chunks, journal).map(|(segment, _)| segment)
        };
        let Some(target) = target else {
            continue;
        };
        let outlet = target.drop_center()?;
        let mut probe = drop.clone();
        probe.position = outlet;
        probe.grounded = true;
        probe.rotation = [0.0, 0.0, 0.0, 1.0];
        if carrying_segment(&probe, journal)
            .is_none_or(|segment| !same_geometry(segment.span, target.span))
        {
            continue;
        }
        let spans = [source.span, target.span];
        let outgoing_along = dot(relative, outgoing);
        let incoming_along = dot(relative, incoming);
        let phase = if departed.is_some() && outgoing_along >= 0.5 {
            2
        } else if departed.is_some() || incoming_along <= 0.5 {
            1
        } else {
            0
        };
        let entry = std::array::from_fn(|i| center[i] + incoming[i] as f64 * 0.5);
        let merge = std::array::from_fn(|i| center[i] + outgoing[i] as f64 * 0.5);
        if drop.stack.count > 1 && !drop_space_clear(occupancy, drop.position, Some(drop.id)) {
            if material == logistics::BELT_MERGER {
                return None;
            }
            continue;
        }
        let first_end = [entry, merge, outlet][usize::from(phase)];
        let delta: [f64; 3] = std::array::from_fn(|i| first_end[i] - drop.position[i]);
        let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
        if distance > 0.000001 && (departed.is_none() || drop.stack.count > 1) {
            let next = std::array::from_fn(|i| {
                drop.position[i] + delta[i] / distance * distance.min(0.02)
            });
            if !crate::kingdom_access::can_move_drop(chunks, journal, drop.position, next)
                || !drop_step_clear(occupancy, drop.position, next, drop.id)
            {
                if material == logistics::BELT_MERGER {
                    return None;
                }
                continue;
            }
        }
        // Preflight fixed geometry, not future drop occupancy. Outgoing items
        // move while the next head approaches; reserving the entire outlet made
        // every turn wait for more than a metre of empty belt.
        let mut previous = drop.position;
        let clear = departed.is_some() && drop.stack.count == 1
            || [entry, merge, outlet]
                .into_iter()
                .skip(usize::from(phase))
                .all(|end| {
                    let initial = previous;
                    (1..=30).all(|step| {
                        let next = std::array::from_fn(|i| {
                            initial[i] + (end[i] - initial[i]) * f64::from(step) / 30.0
                        });
                        let clear = drop_clear_with_carriers(
                            chunks,
                            journal,
                            properties,
                            next,
                            &spans,
                            Some(position),
                        );
                        previous = next;
                        clear
                    })
                });
        if !clear {
            continue;
        }
        return Some((
            Route {
                id: drop.id,
                material,
                facing,
                entry,
                merge,
                outlet,
                source: source.span,
                target: target.span,
                incoming: choice,
                outgoing,
                phase,
            },
            if departed.is_some() {
                start
            } else {
                (choice + 1) % 3
            },
        ));
    }
    None
}

fn accepts_output(config: logistics::DistributorConfig, choice: u8, item: ItemKind) -> bool {
    let reserved = config
        .filters
        .iter()
        .any(|filter| filter.allowed && filter.item == Some(item));
    let filter = config.filters[usize::from(choice)];
    filter.accepts(item) && (!reserved || filter.allowed && filter.item == Some(item))
}

/// A later turn may approach, but cannot occupy an earlier turn's remaining
/// path. The diagonal across the open hood lets alternating inputs zipper into
/// the output instead of stopping twice at a zero-radius central corner.
fn blocks_path(point: [f64; 3], from: [f64; 3], ends: &[[f64; 3]]) -> bool {
    let mut previous = from;
    ends.iter().any(|end| {
        let mut low = 0.0_f64;
        let mut high = 1.0_f64;
        for axis in 0..3 {
            let delta = end[axis] - previous[axis];
            let min = point[axis] - CROSSING_CLEARANCE;
            let max = point[axis] + CROSSING_CLEARANCE;
            if delta.abs() < 1e-12 {
                if previous[axis] <= min || previous[axis] >= max {
                    high = -1.0;
                    break;
                }
            } else {
                let a = (min - previous[axis]) / delta;
                let b = (max - previous[axis]) / delta;
                low = low.max(a.min(b));
                high = high.min(a.max(b));
            }
        }
        previous = *end;
        low < high
    })
}

pub(super) fn advance(
    runtime: &mut Routing,
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    elapsed: f32,
    occupancy: &mut DropIndex,
) -> BTreeSet<u64> {
    if !journal.has_logistics() && runtime.active.is_empty() {
        return BTreeSet::new();
    }
    let mut indices: BTreeMap<_, _> = journal
        .gameplay()
        .drops
        .iter()
        .enumerate()
        .take(terra_core::MAX_WORLD_DROPS)
        .map(|(i, drop)| (drop.id, i))
        .collect();
    runtime.active.retain(|position, routes| {
        routes.retain(|route| {
            indices.contains_key(&route.id)
                && router(*position, journal) == Some((route.material, route.facing))
                && live_span(route.source, journal)
                && live_span(route.target, journal)
        });
        !routes.is_empty()
    });
    runtime
        .turns
        .retain(|position, _| router(*position, journal).is_some());
    let (ports, _) = discover(
        &mut runtime.discovery,
        chunks,
        journal,
        occupancy,
        |position| loaded(position, chunks, journal),
    );
    let start = runtime
        .cursor
        .map_or(0, |cursor| ports.partition_point(|p| *p <= cursor))
        % ports.len().max(1);
    let mut assigned: BTreeSet<_> = runtime
        .active
        .values()
        .flatten()
        .map(|route| route.id)
        .collect();
    for offset in 0..ports.len().min(logistics::PORT_BUDGET) {
        let position = ports[(start + offset) % ports.len()];
        runtime.cursor = Some(position);
        if runtime
            .active
            .get(&position)
            .is_some_and(|routes| routes.len() >= ROUTES_PER_PORT)
            || assigned.len() >= terra_core::MAX_WORLD_DROPS
        {
            continue;
        }
        if let Some((mut route, next)) = grant(
            position,
            runtime.turns.get(&position).copied().unwrap_or(0),
            chunks,
            journal,
            properties,
            occupancy,
            &indices,
            &assigned,
            runtime.active.get(&position),
        ) && !assigned.contains(&route.id)
            && single_item(&mut route, journal, &mut indices, occupancy)
        {
            assigned.insert(route.id);
            if runtime.turns.len() >= 4096 && !runtime.turns.contains_key(&position) {
                runtime.turns.pop_first();
            }
            runtime.turns.insert(position, next);
            runtime.active.entry(position).or_default().push_back(route);
        }
    }
    let mut completed = Vec::new();
    let mut moving = BTreeSet::new();
    let mut changed = false;
    let active: Vec<_> = runtime
        .active
        .iter()
        .flat_map(|(position, routes)| routes.iter().map(|route| (*position, route.id)))
        .collect();
    let start = runtime
        .active_cursor
        .and_then(|cursor| active.iter().position(|key| *key == cursor))
        .map_or(0, |index| index + 1)
        % active.len().max(1);
    for offset in 0..active.len().min(logistics::PORT_BUDGET) {
        let (position, id) = active[(start + offset) % active.len()];
        runtime.active_cursor = Some((position, id));
        let routes = runtime.active.get_mut(&position).unwrap();
        let prior: Vec<_> = routes
            .iter()
            .take_while(|route| route.id != id)
            .map(|route| {
                (
                    journal.gameplay().drops[indices[&route.id]].position,
                    [route.entry, route.merge, route.outlet],
                    usize::from(route.phase),
                )
            })
            .collect();
        let route = routes.iter_mut().find(|route| route.id == id).unwrap();
        let index = indices[&route.id];
        let current = &journal.gameplay().drops[index];
        let original = current.position;
        let mut point = original;
        let motion = loaded(position, chunks, journal)
            .then(|| output(position, route.outgoing, chunks, journal))
            .flatten()
            .filter(|(segment, _)| same_geometry(segment.span, route.target))
            .filter(|_| route.source.loaded(chunks, journal));
        if let Some((_, speed)) = motion {
            let mut travel = speed * f64::from(elapsed.min(0.15));
            for _ in 0..28 {
                if travel <= 0.000001 {
                    break;
                }
                let end = [route.entry, route.merge, route.outlet][usize::from(route.phase)];
                let delta: [f64; 3] = std::array::from_fn(|i| end[i] - point[i]);
                let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
                if distance < 0.000001 {
                    if route.phase == 2 {
                        completed.push((position, route.id));
                        break;
                    }
                    route.phase += 1;
                    continue;
                }
                let step = travel.min(distance).min(0.05);
                let next = std::array::from_fn(|i| point[i] + delta[i] * step / distance);
                if prior
                    .iter()
                    .any(|(from, ends, phase)| blocks_path(next, *from, &ends[*phase..]))
                    || !crate::kingdom_access::can_move_drop(chunks, journal, point, next)
                    || !drop_step_clear(occupancy, point, next, route.id)
                    || !drop_clear_with_carriers(
                        chunks,
                        journal,
                        properties,
                        next,
                        &[route.source, route.target],
                        Some(position),
                    )
                {
                    break;
                }
                point = next;
                travel -= step;
            }
        }
        let velocity =
            std::array::from_fn(|i| ((point[i] - original[i]) / f64::from(elapsed)) as f32);
        if point != original {
            moving.insert(route.id);
        }
        if point != original || current.velocity != velocity {
            move_indexed_drop(occupancy, route.id, original, point);
            let drop = &mut journal.gameplay_mut().drops[index];
            drop.position = point;
            drop.velocity = velocity;
            drop.grounded = true;
            drop.rotation = [0.0, 0.0, 0.0, 1.0];
            drop.angular_velocity = [0.0; 3];
            changed = true;
        }
    }
    for (position, id) in completed {
        if let Some(routes) = runtime.active.get_mut(&position) {
            routes.retain(|route| route.id != id);
            if routes.is_empty() {
                runtime.active.remove(&position);
            }
        }
    }
    // Runtime routes are not saved. A stopped/unloaded restored mouth, or a
    // budget-deferred route, has no motion until this update actually moves it.
    let paused: Vec<_> = journal
        .gameplay()
        .drops
        .iter()
        .enumerate()
        .filter(|(_, drop)| drop.velocity != [0.0; 3] && !moving.contains(&drop.id))
        .filter(|(_, drop)| carrying(drop, journal).is_some())
        .map(|(index, _)| index)
        .collect();
    if !paused.is_empty() {
        for index in paused {
            journal.gameplay_mut().drops[index].velocity = [0.0; 3];
        }
        changed = true;
    }
    // Include completed IDs for this tick: a stack never moves twice in one transport update.
    if changed {
        let game = journal.gameplay_mut();
        game.revision = game.revision.saturating_add(1);
    }
    assigned
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_voxel::BlockSection;

    fn fixture() -> (ChunkStore, EditJournal) {
        let section = SectionPos::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        (chunks, EditJournal::default())
    }
    fn place(chunks: &mut ChunkStore, journal: &mut EditJournal, position: Cell, material: u16) {
        let (section, local) = global_to_section(position);
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, material)
            .unwrap();
        journal.record(position, material).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
    }

    #[test]
    fn restored_occupied_arm_is_assigned_paused_without_starving_an_independent_output() {
        let (mut chunks, mut journal) = fixture();
        let port = [8, 5, 8];
        place(&mut chunks, &mut journal, port, logistics::BELT_DISTRIBUTOR);
        let mut states = BTreeMap::new();
        for (position, facing) in [
            ([8, 4, 8], 0),
            ([8, 4, 7], 0),
            ([9, 4, 8], 1),
            ([8, 4, 9], 0),
        ] {
            place(&mut chunks, &mut journal, position, 148);
            journal
                .set_object_state(
                    position,
                    Some(BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            states.insert(
                position,
                kinetic::KineticState {
                    rpm: 30.0,
                    ..Default::default()
                },
            );
        }
        journal.replace_kinetic_states(states);
        journal
            .set_distributor(
                port,
                logistics::DistributorConfig {
                    filters: [
                        logistics::FilterConfig {
                            item: Some(ItemKind::Block(22)),
                            allowed: true,
                        },
                        logistics::FilterConfig {
                            item: Some(ItemKind::Block(25)),
                            allowed: true,
                        },
                        logistics::FilterConfig {
                            item: Some(ItemKind::Block(23)),
                            allowed: true,
                        },
                    ],
                },
            )
            .unwrap();
        // The downstream stack is deliberately stationary: this unit exercises
        // junction motion against an already occupied, externally blocked outlet.
        for (point, item) in [
            ([9.4, 4.755, 8.5], 22),
            ([9.67, 4.755, 8.5], 22),
            ([8.5, 4.755, 7.95], 25),
        ] {
            assert!(journal.gameplay_mut().drop_stack(
                point,
                terra_core::ItemStack {
                    item: ItemKind::Block(item),
                    count: 1
                },
                [0.0; 3],
                0.0
            ));
        }
        let original = journal.gameplay().drops.clone();
        let mut occupancy = index_drops(&original);
        let mut runtime = Routing::default();
        for _ in 0..100 {
            advance(
                &mut runtime,
                &chunks,
                &mut journal,
                &Default::default(),
                0.02,
                &mut occupancy,
            );
        }
        assert_eq!(journal.gameplay().drops.len(), 3);
        for (drop, before) in journal.gameplay().drops.iter().zip(&original) {
            assert_eq!((drop.id, drop.stack), (before.id, before.stack));
        }
        assert_eq!(journal.gameplay().drops[0].position, original[0].position);
        assert_eq!(journal.gameplay().drops[1].position, original[1].position);
        assert!(
            journal.gameplay().drops[2].position[2] >= 9.49999,
            "{:?}",
            journal.gameplay().drops
        );
    }

    #[test]
    fn distant_drop_cells_do_not_cause_voxel_queries_for_an_unrelated_funnel() {
        let (mut chunks, mut journal) = fixture();
        place(&mut chunks, &mut journal, [0, 4, 0], logistics::FUNNEL);
        let mut occupancy: DropIndex = (0..terra_core::MAX_WORLD_DROPS)
            .map(|index| {
                let cell = [1000 + index as i64, 3, 1000];
                (cell, vec![(index as u64 + 1, cell.map(|v| v as f64 + 0.5))])
            })
            .collect();
        let mut cursor = None;
        let mut queries = 0;
        let (ports, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            queries += 1;
            loaded(position, &chunks, &journal)
        });
        assert!(ports.is_empty());
        assert_eq!((raw, queries), (0, 0));
        occupancy.pop_last();
        occupancy.insert([0, 3, 0], vec![(3000, [0.5, 3.755, 0.5])]);
        let (ports, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            queries += 1;
            loaded(position, &chunks, &journal)
        });
        assert!(ports.is_empty());
        assert_eq!((raw, queries), (1, 1));
    }

    #[test]
    fn sparse_raw_budget_precedes_metadata_checks_and_resumes_to_a_late_router() {
        let (mut chunks, mut journal) = fixture();
        let mut occupancy = DropIndex::new();
        let late = [15, 4, 15];
        for x in 0..16 {
            for z in 0..16 {
                let position = [x, 4, z];
                place(
                    &mut chunks,
                    &mut journal,
                    position,
                    if position == late {
                        logistics::BELT_DISTRIBUTOR
                    } else {
                        logistics::FUNNEL
                    },
                );
                occupancy.insert(
                    [x, 3, z],
                    vec![(
                        x as u64 * 16 + z as u64 + 1,
                        [x as f64 + 0.5, 3.755, z as f64 + 0.5],
                    )],
                );
            }
        }
        let mut cursor = None;
        let mut queries = 0;
        let (ports, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            queries += 1;
            loaded(position, &chunks, &journal)
        });
        assert!(ports.is_empty());
        assert_eq!((raw, queries), (DISCOVERY_BUDGET, DISCOVERY_BUDGET));
        let (ports, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            queries += 1;
            loaded(position, &chunks, &journal)
        });
        assert_eq!(ports, vec![late]);
        assert_eq!((raw, queries), (DISCOVERY_BUDGET, 2 * DISCOVERY_BUDGET));
        // Dense eligible prefixes must not starve a later port either: discovery stops at64 results.
        for x in 0..16 {
            for z in 0..16 {
                place(
                    &mut chunks,
                    &mut journal,
                    [x, 4, z],
                    logistics::BELT_DISTRIBUTOR,
                );
            }
        }
        let mut seen = BTreeSet::new();
        for _ in 0..4 {
            let (ports, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
                loaded(position, &chunks, &journal)
            });
            assert_eq!(ports.len(), logistics::PORT_BUDGET);
            assert!(raw <= DISCOVERY_BUDGET);
            seen.extend(ports);
        }
        assert_eq!(seen.len(), 256);
        assert!(seen.contains(&late));
    }

    #[test]
    fn sparse_discovery_crosses_negative_and_vertical_borders_but_never_unloaded_sections() {
        let (mut chunks, mut journal) = fixture();
        for y in [0, 1] {
            let section = SectionPos { x: -1, y, z: -1 };
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data: BlockSection::empty(section).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let ports = [[-1, 15, -1], [-1, 16, -1]];
        let mut occupancy = DropIndex::new();
        for (index, position) in ports.into_iter().enumerate() {
            place(
                &mut chunks,
                &mut journal,
                position,
                logistics::BELT_DISTRIBUTOR,
            );
            let cell = [position[0], position[1] - 1, position[2]];
            occupancy.insert(cell, vec![(index as u64 + 1, cell.map(|v| v as f64 + 0.5))]);
        }
        let mut cursor = None;
        let (found, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            loaded(position, &chunks, &journal)
        });
        assert_eq!(found, ports);
        assert_eq!(raw, 2);
        chunks.loaded.remove(&global_to_section(ports[1]).0);
        let (found, raw) = discover(&mut cursor, &chunks, &journal, &occupancy, |position| {
            loaded(position, &chunks, &journal)
        });
        assert_eq!(found, vec![ports[0]]);
        assert_eq!(raw, 1);
    }

    #[test]
    fn restored_stopped_or_unloaded_port_custody_clears_saved_motion_and_resumes_continuously() {
        for unloaded in [false, true] {
            let (mut chunks, mut journal) = fixture();
            let port = [8, 5, 8];
            place(&mut chunks, &mut journal, port, logistics::BELT_MERGER);
            let cells = [[9, 4, 8], [8, 4, 8], [8, 4, 9], [8, 4, 10], [8, 4, 11]];
            for position in cells {
                place(&mut chunks, &mut journal, position, kinetic::CONVEYOR);
                if position == cells[0] {
                    journal
                        .set_object_state(
                            position,
                            Some(BlockObjectState {
                                facing: 3,
                                ..Default::default()
                            }),
                        )
                        .unwrap();
                }
            }
            let drives: BTreeMap<_, _> = cells
                .into_iter()
                .map(|position| {
                    (
                        position,
                        kinetic::KineticState {
                            rpm: 30.0,
                            ..Default::default()
                        },
                    )
                })
                .collect();
            journal.replace_kinetic_states(drives.clone());
            let start = segment_at(&journal, cells[0])
                .unwrap()
                .drop_center()
                .unwrap();
            assert!(journal.gameplay_mut().drop_stack(
                start,
                terra_core::ItemStack {
                    item: ItemKind::Block(1),
                    count: 1
                },
                [0.0; 3],
                0.0
            ));
            let mut runtime = Transport::default();
            for _ in 0..65 {
                transport(
                    &mut runtime,
                    &chunks,
                    &mut journal,
                    &Default::default(),
                    0.02,
                );
            }
            let saved = journal.gameplay().drops[0].clone();
            assert_eq!(carrying(&saved, &journal), Some(port));
            assert_ne!(saved.velocity, [0.0; 3]);
            runtime = Transport::default();
            let removed = unloaded.then(|| std::mem::take(&mut chunks.loaded));
            if !unloaded {
                journal.replace_kinetic_states(BTreeMap::new());
            }
            for _ in 0..10 {
                transport(
                    &mut runtime,
                    &chunks,
                    &mut journal,
                    &Default::default(),
                    0.02,
                );
            }
            let paused = &journal.gameplay().drops[0];
            assert_eq!(paused.position, saved.position);
            assert_eq!((paused.id, paused.stack), (saved.id, saved.stack));
            assert_eq!(paused.velocity, [0.0; 3]);
            assert_eq!(paused.age_seconds, saved.age_seconds);
            assert_eq!(carrying(paused, &journal), Some(port));
            if let Some(removed) = removed {
                chunks.loaded = removed;
            }
            journal.replace_kinetic_states(drives);
            for _ in 0..60 {
                let previous = journal.gameplay().drops[0].position;
                transport(
                    &mut runtime,
                    &chunks,
                    &mut journal,
                    &Default::default(),
                    0.02,
                );
                let drop = &journal.gameplay().drops[0];
                let step = (0..3)
                    .map(|i| (drop.position[i] - previous[i]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(step <= 0.02001);
                assert_eq!((drop.id, drop.stack), (saved.id, saved.stack));
            }
            assert!(journal.gameplay().drops[0].position[2] > 9.25);
        }
    }
}
