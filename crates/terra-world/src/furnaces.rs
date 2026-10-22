//! Bounded chimney routing and authoritative smoke cells in loaded rooms.
use crate::{edits::EditJournal, streaming::ChunkStore};
use std::collections::{HashMap, HashSet, VecDeque};
use terra_core::furnace::{FurnaceState, MAX_SMOKE_CELLS};

type Cell = [i64; 3];
const UP: Cell = [0, 1, 0];
const DOWN: Cell = [0, -1, 0];
const NEIGHBORS: [Cell; 6] = [UP, [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], DOWN];
const PIPE_LIMIT: usize = 64;
const ROOM_LIMIT: usize = 4096;
const WORLD_TOP: i64 = 356;
const SKY_PROBE_LIMIT: usize = 32_768;

fn offset(cell: Cell, direction: Cell) -> Option<Cell> {
    let mut next = cell;
    for axis in 0..3 {
        next[axis] = cell[axis].checked_add(direction[axis])?;
    }
    next.iter()
        .all(|axis| axis.unsigned_abs() <= 1_000_000)
        .then_some(next)
}

fn front(facing: u8) -> Cell {
    [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing % 4)]
}

fn ports(material: u16, facing: u8) -> Option<[Cell; 2]> {
    crate::objects::chimney_ports(material, facing)
}

/// Smoke can occupy non-solid cells, but never water or solid geometry.
pub fn smoke_passable(material: u16) -> bool {
    terra_voxel::MaterialId::from_raw(material).is_some_and(|material| {
        material != terra_voxel::MaterialId::FluidWater && !material.is_solid()
    })
}

fn aperture(material: u16) -> bool {
    crate::objects::is_door(material) || crate::objects::is_window(material)
}

/// Unlike collision, an open door/window is a rate-limited vent, not unlimited air.
pub fn smoke_cell_passable(material: u16, facing: u8) -> bool {
    smoke_passable(material) || aperture(material) && (4..8).contains(&facing)
}

#[derive(Debug, PartialEq, Eq)]
enum Route {
    Outlet(Cell, Cell),
    Blocked,
    Unknown,
}

fn route(
    anchor: Cell,
    block: &mut impl FnMut(Cell) -> Option<u16>,
    facing: &mut impl FnMut(Cell) -> u8,
) -> Route {
    let Some(mut cell) = offset(anchor, [0, 2, 0]) else {
        return Route::Unknown;
    };
    let mut incoming = DOWN;
    let mut visited = HashSet::new();
    for _ in 0..=PIPE_LIMIT {
        let Some(material) = block(cell) else {
            return Route::Unknown;
        };
        if smoke_passable(material) {
            return Route::Outlet(cell, incoming.map(|axis| -axis));
        }
        let Some(ports) = ports(material, facing(cell)) else {
            return Route::Blocked;
        };
        if !visited.insert(cell) || visited.len() > PIPE_LIMIT {
            return Route::Blocked;
        }
        let outgoing = if ports[0] == incoming {
            ports[1]
        } else if ports[1] == incoming {
            ports[0]
        } else {
            return Route::Blocked;
        };
        let Some(next) = offset(cell, outgoing) else {
            return Route::Unknown;
        };
        cell = next;
        incoming = outgoing.map(|axis| -axis);
    }
    Route::Blocked
}

#[derive(Debug, PartialEq, Eq)]
enum Room {
    Enclosed(Vec<Cell>, usize, usize),
    Vented,
    Unknown,
}

fn sky(cell: Cell, block: &mut impl FnMut(Cell) -> Option<u16>) -> Option<bool> {
    for height in cell[1] + 1..=WORLD_TOP {
        let above = [cell[0], height, cell[2]];
        let material = block(above)?;
        if !smoke_passable(material) {
            return Some(false);
        }
    }
    Some(true)
}

fn room(
    outlet: Cell,
    block: &mut impl FnMut(Cell) -> Option<u16>,
    sky_block: &mut impl FnMut(Cell) -> Option<u16>,
    facing: &mut impl FnMut(Cell) -> u8,
    inspect_vents: bool,
    remaining: &mut usize,
    exits: &mut Vec<(Cell, Cell)>,
) -> Room {
    let mut queue = VecDeque::from([(outlet, UP)]);
    let mut visited = HashSet::from([outlet]);
    let mut cells = Vec::new();
    let mut unknown = false;
    let mut vents = HashMap::new();
    while let Some((cell, incoming)) = queue.pop_front() {
        if cells.len() == ROOM_LIMIT || *remaining == 0 {
            return Room::Unknown;
        }
        *remaining -= 1;
        cells.push(cell);
        match sky(cell, sky_block) {
            Some(true) => {
                exits.push((cell, incoming));
                return Room::Vented;
            }
            None => unknown = true,
            Some(false) => {}
        }
        for direction in NEIGHBORS {
            let Some(next) = offset(cell, direction) else {
                unknown = true;
                continue;
            };
            if visited.contains(&next) {
                continue;
            }
            match block(next) {
                Some(material) if !aperture(material) && smoke_passable(material) => {
                    visited.insert(next);
                    queue.push_back((next, direction));
                }
                None => unknown = true,
                Some(material)
                    if inspect_vents && aperture(material) && (4..8).contains(&facing(next)) =>
                {
                    // Only the front/back of a real opening conduct smoke.
                    let forward = front(facing(next));
                    if (direction == forward || direction == forward.map(|axis| -axis))
                        && let Some(outside) = offset(next, direction)
                    {
                        vents.insert(next, (material, outside, direction));
                    }
                }
                Some(_) => {}
            }
        }
    }
    if unknown {
        Room::Unknown
    } else {
        let mut windows = 0;
        let mut doors = 0;
        let mut vents: Vec<_> = vents.into_iter().collect();
        vents.sort_unstable_by_key(|(cell, _)| *cell);
        for (opening, (material, outside, direction)) in vents.into_iter().take(64) {
            if block(outside).is_some_and(smoke_passable)
                && matches!(
                    room(
                        outside,
                        block,
                        sky_block,
                        facing,
                        false,
                        remaining,
                        &mut Vec::new()
                    ),
                    Room::Vented
                )
            {
                exits.push((opening, direction));
                if crate::objects::is_window(material) {
                    windows += 1;
                } else {
                    doors += 1;
                }
            }
        }
        Room::Enclosed(cells, windows, doors)
    }
}

/// Visible exits use the same bounded room/sky test as authoritative ventilation.
/// This samples loaded geometry only; callers cache results between edits.
pub fn ventilation_outlets(
    seed: Cell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Vec<(Cell, Cell)> {
    let mut cache = HashMap::new();
    let mut block = |cell| {
        *cache.entry(cell).or_insert_with(|| {
            chunks
                .block_at(cell)
                .map(|raw| journal.block(cell).unwrap_or(raw))
        })
    };
    if !block(seed).is_some_and(smoke_passable) {
        return Vec::new();
    }
    let mut sky_cache = HashMap::new();
    let mut sky_block = |cell: Cell| {
        if let Some(&value) = sky_cache.get(&cell) {
            return value;
        }
        if sky_cache.len() >= SKY_PROBE_LIMIT {
            return None;
        }
        let value = chunks
            .block_at(cell)
            .map(|raw| journal.block(cell).unwrap_or(raw))
            .or_else(|| {
                let column =
                    chunks.prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])?;
                (cell[1] as f64 > column.maximum_height()).then(|| journal.block(cell).unwrap_or(0))
            });
        sky_cache.insert(cell, value);
        value
    };
    let mut exits = Vec::new();
    let result = room(
        seed,
        &mut block,
        &mut sky_block,
        &mut |cell| journal.object_state(cell).map_or(0, |s| s.facing),
        true,
        &mut (ROOM_LIMIT * 2),
        &mut exits,
    );
    if result == Room::Unknown {
        exits.clear();
    }
    exits.truncate(8);
    exits
}

fn dissipate(state: &mut FurnaceState, cells: usize) {
    state
        .smoke_cells
        .truncate(state.smoke_cells.len().saturating_sub(cells));
}

#[derive(Clone, Copy)]
struct SmokeSource<'a> {
    anchor: Cell,
    outlet: Cell,
    cells: &'a [Cell],
}

fn can_emit(state: &FurnaceState, inventory: Option<&terra_core::Inventory>) -> bool {
    if state.burn_remaining > 0.0 || !state.smoke_cells.is_empty() {
        return true;
    }
    let Some(inventory) = inventory else {
        return false;
    };
    if inventory.slot(terra_core::furnace::INPUT).is_none()
        || inventory.slot(terra_core::furnace::FUEL).is_none()
    {
        return false;
    }
    // Count ready-to-ignite peers before their turn in the fair scheduler, but
    // not blocked/invalid recipes. This uses the authoritative fuel rules.
    let mut state = state.clone();
    let mut inventory = inventory.clone();
    terra_core::furnace::advance(&mut state, &mut inventory, 0.001, 0);
    state.burn_remaining > 0.0
}

/// The actual connected outlet and its outgoing axis, shared by simulation and the plume.
pub fn chimney_outlet(
    anchor: Cell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<(Cell, Cell)> {
    let mut block = |cell| {
        chunks
            .block_at(cell)
            .map(|raw| journal.block(cell).unwrap_or(raw))
    };
    match route(anchor, &mut block, &mut |cell| {
        journal.object_state(cell).map_or(0, |object| object.facing)
    }) {
        Route::Outlet(cell, direction) => Some((cell, direction)),
        Route::Blocked => {
            let direction = front(
                journal
                    .object_state(anchor)
                    .map_or(0, |object| object.facing),
            );
            offset(anchor, [0, 1, 0])
                .and_then(|cell| offset(cell, direction))
                .filter(|cell| block(*cell).is_some_and(smoke_passable))
                .map(|cell| (cell, direction))
        }
        Route::Unknown => None,
    }
}

/// Missing sections never generate terrain or count as ventilation.
pub fn update_smoke(
    anchor: Cell,
    state: &mut FurnaceState,
    lit: bool,
    elapsed: f32,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> bool {
    if !elapsed.is_finite() || elapsed <= 0.0 || !state.valid() {
        return false;
    }
    if !lit && state.smoke_cells.is_empty() {
        let changed = state.smoke_credit != 0.0;
        state.smoke_credit = 0.0;
        return changed;
    }
    // One pass over station metadata per bounded smoke update, never per voxel.
    let sources: Vec<_> = journal
        .furnaces()
        .filter(|(position, state)| {
            *position != anchor
                && chunks.block_at(*position) == Some(59)
                && chunks.block_at([position[0], position[1] + 1, position[2]]) == Some(59)
                && can_emit(state, journal.container_at(*position))
        })
        .map(|(position, state)| SmokeSource {
            anchor: position,
            outlet: if state.outlet == [0; 3] {
                chimney_outlet(position, chunks, journal).map_or(state.outlet, |(cell, _)| cell)
            } else {
                state.outlet
            },
            cells: &state.smoke_cells,
        })
        .collect();
    update_with_sky(
        anchor,
        state,
        lit,
        elapsed,
        &sources,
        |cell| {
            chunks
                .block_at(cell)
                .map(|material| journal.block(cell).unwrap_or(material))
        },
        |cell| journal.object_state(cell).map_or(0, |object| object.facing),
        |cell| {
            chunks
                .block_at(cell)
                .map(|raw| journal.block(cell).unwrap_or(raw))
                .or_else(|| {
                    // Above the immutable generator bound there can only be edits.
                    // This is not the old "32 empty cells mean outdoors" heuristic.
                    let column =
                        chunks.prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])?;
                    (cell[1] as f64 > column.maximum_height())
                        .then(|| journal.block(cell).unwrap_or(0))
                })
        },
    )
}

#[cfg(test)]
fn update_with(
    anchor: Cell,
    state: &mut FurnaceState,
    lit: bool,
    elapsed: f32,
    sample: impl FnMut(Cell) -> Option<u16>,
    facing: impl FnMut(Cell) -> u8,
) -> bool {
    let sample = std::cell::RefCell::new(sample);
    update_with_sky(
        anchor,
        state,
        lit,
        elapsed,
        &[],
        |cell| sample.borrow_mut()(cell),
        facing,
        |cell| sample.borrow_mut()(cell),
    )
}

#[allow(clippy::too_many_arguments)]
fn update_with_sky(
    anchor: Cell,
    state: &mut FurnaceState,
    lit: bool,
    elapsed: f32,
    sources: &[SmokeSource<'_>],
    mut sample: impl FnMut(Cell) -> Option<u16>,
    mut facing: impl FnMut(Cell) -> u8,
    mut sky_sample: impl FnMut(Cell) -> Option<u16>,
) -> bool {
    if !elapsed.is_finite() || elapsed <= 0.0 || !state.valid() {
        return false;
    }
    if !lit && state.smoke_cells.is_empty() {
        let changed = state.smoke_credit != 0.0;
        state.smoke_credit = 0.0;
        return changed;
    }
    let previous_credit = state.smoke_credit;
    let previous_cells = state.smoke_cells.clone();
    let previous_outlet = state.outlet;
    let previous_vented = state.vented;
    {
        let mut cache = HashMap::new();
        let mut block = |cell| *cache.entry(cell).or_insert_with(|| sample(cell));
        let mut sky_cache = HashMap::new();
        let mut sky_block = |cell| {
            if let Some(&cached) = sky_cache.get(&cell) {
                return cached;
            }
            if sky_cache.len() >= SKY_PROBE_LIMIT {
                return None;
            }
            let value = sky_sample(cell);
            sky_cache.insert(cell, value);
            value
        };
        let outlet = match route(anchor, &mut block, &mut facing) {
            Route::Outlet(cell, _) => Some(cell),
            Route::Blocked => offset(anchor, [0, 1, 0])
                .and_then(|cell| offset(cell, front(facing(anchor))))
                .filter(|cell| block(*cell).is_some_and(smoke_passable)),
            Route::Unknown => None,
        };
        if let Some(outlet) = outlet {
            state.outlet = outlet;
            let mut candidates = Vec::new();
            let rate = match room(
                outlet,
                &mut block,
                &mut sky_block,
                &mut facing,
                true,
                &mut (ROOM_LIMIT * 2),
                &mut Vec::new(),
            ) {
                Room::Vented => {
                    state.vented = true;
                    -2.0
                }
                Room::Enclosed(mut cells, windows, doors) => {
                    let reachable: HashSet<_> = cells.iter().copied().collect();
                    let room_sources: Vec<_> = sources
                        .iter()
                        .filter(|source| {
                            reachable.contains(&source.outlet)
                                || source.cells.iter().any(|cell| reachable.contains(cell))
                        })
                        .collect();
                    let ventilation = (windows as f32 * 0.5 + doors as f32 * 0.75)
                        / (room_sources.len() + 1) as f32;
                    state.vented = ventilation > 0.0;
                    let mut owners: HashMap<Cell, Cell> = HashMap::new();
                    for source in room_sources {
                        for &cell in source.cells {
                            if reachable.contains(&cell) {
                                owners
                                    .entry(cell)
                                    .and_modify(|owner| *owner = (*owner).min(source.anchor))
                                    .or_insert(source.anchor);
                            }
                        }
                    }
                    // Old saves can contain overlapping source lists. The stable
                    // lowest anchor owns those cells; new growth always avoids peers.
                    state.smoke_cells.retain(|cell| {
                        reachable.contains(cell)
                            && owners.get(cell).is_none_or(|owner| anchor < *owner)
                    });
                    cells.retain(|cell| !owners.contains_key(cell));
                    cells.sort_unstable_by_key(|cell| {
                        (
                            std::cmp::Reverse(cell[1]),
                            (cell[0] - outlet[0]).abs()
                                + (cell[1] - outlet[1]).abs()
                                + (cell[2] - outlet[2]).abs(),
                            *cell,
                        )
                    });
                    candidates = cells;
                    if lit {
                        1.0 - ventilation
                    } else if ventilation > 0.0 {
                        -ventilation
                    } else {
                        -2.0
                    }
                }
                Room::Unknown => {
                    state.vented = false;
                    0.0
                }
            };
            let total = state.smoke_credit + elapsed.min(MAX_SMOKE_CELLS as f32) * rate;
            let whole = total.trunc();
            state.smoke_credit = total - whole;
            if whole < 0.0 {
                dissipate(state, (-whole) as usize);
            } else if whole > 0.0 && lit {
                let mut occupied: HashSet<_> = state.smoke_cells.iter().copied().collect();
                let available = MAX_SMOKE_CELLS.saturating_sub(state.smoke_cells.len());
                state.smoke_cells.extend(
                    candidates
                        .into_iter()
                        .filter(|cell| occupied.insert(*cell))
                        .take((whole as usize).min(available)),
                );
            }
            if state.smoke_cells.is_empty() && state.smoke_credit < 0.0 {
                state.smoke_credit = 0.0;
            }
        } else {
            state.vented = false;
        }
    }
    state.smoke_credit != previous_credit
        || state.smoke_cells != previous_cells
        || state.outlet != previous_outlet
        || state.vented != previous_vented
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enclosed(cell: Cell) -> Option<u16> {
        Some(
            if (-3..=3).contains(&cell[0])
                && (1..=4).contains(&cell[1])
                && (-3..=3).contains(&cell[2])
                && cell != [0, 1, 0]
            {
                0
            } else {
                3
            },
        )
    }

    #[test]
    fn furnace_smoke_fills_one_unique_air_cell_per_second_and_stops_at_capacity() {
        let mut state = FurnaceState::default();
        for frame in 0..8 {
            update_with([0; 3], &mut state, true, 0.25, enclosed, |_| 0);
            assert_eq!(state.smoke_cells.len(), (frame + 1) / 4);
        }
        assert_eq!(state.outlet, [0, 2, 0]);
        assert!(!state.vented);
        assert!(state.smoke_cells.iter().all(|cell| cell[1] == 4));
        update_with([0; 3], &mut state, true, 1000.0, enclosed, |_| 0);
        assert_eq!(state.smoke_cells.len(), 7 * 7 * 4 - 1);
        assert!(state.valid());
        assert!(
            state
                .smoke_cells
                .iter()
                .all(|&cell| enclosed(cell).is_some_and(smoke_passable))
        );
        let mut large = FurnaceState::default();
        update_with(
            [0; 3],
            &mut large,
            true,
            1000.0,
            |p| {
                Some(
                    if (-5..=5).contains(&p[0])
                        && (1..=4).contains(&p[1])
                        && (-5..=5).contains(&p[2])
                    {
                        0
                    } else {
                        3
                    },
                )
            },
            |_| 0,
        );
        assert_eq!(large.smoke_cells.len(), MAX_SMOKE_CELLS);
        assert!(large.valid());
    }

    #[test]
    fn furnace_smoke_clears_after_extinguishing_or_opening_the_room() {
        let mut state = FurnaceState::default();
        update_with([0; 3], &mut state, true, 6.0, enclosed, |_| 0);
        update_with([0; 3], &mut state, false, 1.0, enclosed, |_| 0);
        assert_eq!(state.smoke_cells.len(), 4);
        update_with(
            [0; 3],
            &mut state,
            true,
            1.0,
            |cell| {
                if cell[0] == 0 && cell[2] == 0 && cell[1] >= 2 {
                    Some(0)
                } else {
                    enclosed(cell)
                }
            },
            |_| 0,
        );
        assert_eq!(state.smoke_cells.len(), 2);
        assert!(state.vented);
        update_with([0; 3], &mut state, false, 1.0, enclosed, |_| 0);
        assert!(state.smoke_cells.is_empty());
    }

    #[test]
    fn furnace_smoke_rechecks_blocked_cells_and_does_not_cross_internal_walls() {
        let mut state = FurnaceState::default();
        update_with([0; 3], &mut state, true, 10.0, enclosed, |_| 0);
        let filled = state.smoke_cells[0];
        update_with(
            [0; 3],
            &mut state,
            true,
            1.0,
            |cell| {
                if cell == filled || cell[0] == 1 {
                    Some(3)
                } else {
                    enclosed(cell)
                }
            },
            |_| 0,
        );
        assert!(!state.smoke_cells.contains(&filled));
        assert!(state.smoke_cells.iter().all(|cell| cell[0] <= 0));
    }

    #[test]
    fn furnace_chimney_follows_straight_horizontal_and_rotated_elbow_ports() {
        for facing in 0..4 {
            let forward = front(facing);
            let elbow = [0, 2, 0];
            let horizontal = offset(elbow, forward).unwrap();
            let rise = offset(horizontal, forward).unwrap();
            let straight = offset(rise, UP).unwrap();
            let outlet = offset(straight, UP).unwrap();
            let pipe = HashMap::from([
                (elbow, (61, facing)),
                (horizontal, (62, facing)),
                (rise, (64, (facing + 2) % 4)),
                (straight, (60, 0)),
            ]);
            assert_eq!(
                route(
                    [0; 3],
                    &mut |cell| Some(pipe.get(&cell).map_or(0, |value| value.0)),
                    &mut |cell| pipe.get(&cell).map_or(0, |value| value.1),
                ),
                Route::Outlet(outlet, UP),
            );
            let mut state = FurnaceState::default();
            update_with(
                [0; 3],
                &mut state,
                true,
                1.0,
                |cell| Some(pipe.get(&cell).map_or(0, |value| value.0)),
                |cell| pipe.get(&cell).map_or(0, |value| value.1),
            );
            assert_eq!(state.outlet, outlet);
            assert!(state.vented && state.smoke_cells.is_empty());
        }
        assert_eq!(ports(63, 0), ports(61, 0));
    }

    #[test]
    fn furnace_smoke_backflows_when_the_chimney_is_blocked_or_misaligned() {
        for obstruction in [3, 62] {
            let mut state = FurnaceState::default();
            update_with(
                [0; 3],
                &mut state,
                true,
                1.0,
                |cell| {
                    if cell == [0, 2, 0] {
                        Some(obstruction)
                    } else {
                        enclosed(cell)
                    }
                },
                |_| 0,
            );
            assert_eq!(state.outlet, [0, 1, 1]);
            assert_eq!(state.smoke_cells.len(), 1);
            assert!(!state.vented);
        }
    }

    #[test]
    fn furnace_chimney_traversal_has_a_fixed_limit_and_never_enters_unknown_cells() {
        for length in [PIPE_LIMIT, PIPE_LIMIT + 1] {
            let result = route(
                [0; 3],
                &mut |cell| {
                    Some(if (2..2 + length as i64).contains(&cell[1]) {
                        60
                    } else {
                        0
                    })
                },
                &mut |_| 0,
            );
            assert_eq!(
                result,
                if length == PIPE_LIMIT {
                    Route::Outlet([0, 2 + length as i64, 0], UP)
                } else {
                    Route::Blocked
                },
            );
        }
        assert_eq!(
            route([0; 3], &mut |cell| (cell[1] < 5).then_some(60), &mut |_| 0,),
            Route::Unknown,
        );
    }

    #[test]
    fn furnace_smoke_unknown_sections_and_giant_rooms_pause_with_bounded_probes() {
        let mut state = FurnaceState::default();
        update_with([0; 3], &mut state, true, 2.0, enclosed, |_| 0);
        let before = state.smoke_cells.clone();
        update_with(
            [0; 3],
            &mut state,
            true,
            5.0,
            |cell| (cell[0] != 3).then(|| enclosed(cell).unwrap()),
            |_| 0,
        );
        assert_eq!(state.smoke_cells, before);
        assert!(!state.vented);
        let mut probes = 0;
        update_with(
            [0; 3],
            &mut state,
            true,
            1.0,
            |cell| {
                probes += 1;
                Some(if (1..=4).contains(&cell[1]) { 0 } else { 3 })
            },
            |_| 0,
        );
        assert_eq!(state.smoke_cells, before);
        assert!(!state.vented && probes < ROOM_LIMIT * 8);
        assert_eq!(route([0; 3], &mut |_| None, &mut |_| 0), Route::Unknown);
        assert_eq!(sky([0, 1, 0], &mut |p| (p[1] < 16).then_some(0)), None,);
    }

    #[test]
    fn furnace_smoke_never_treats_unloaded_journal_edits_as_loaded_air() {
        let mut journal = EditJournal::default();
        journal.record([0, 2, 0], 0).unwrap();
        let mut state = FurnaceState::default();
        update_smoke(
            [0; 3],
            &mut state,
            true,
            1.0,
            &ChunkStore::default(),
            &journal,
        );
        assert!(state.smoke_cells.is_empty() && !state.vented);
        assert_eq!(state.outlet, [0; 3]);
    }

    fn room_with_openings(cell: Cell, windows: usize, door: bool, hole: bool) -> Option<u16> {
        if (1..=4).contains(&cell[1]) && (-3..=3).contains(&cell[0]) && (-3..=3).contains(&cell[2])
        {
            return Some(0);
        }
        if cell[2] == 4 && (-3..=3).contains(&cell[0]) && (1..=4).contains(&cell[1]) {
            if door && cell[0] == 0 && cell[1] <= 2 {
                return Some(65);
            }
            if cell[1] == 3 && (0..windows as i64).contains(&cell[0]) {
                return Some(68);
            }
            if hole && cell == [-1, 3, 4] {
                return Some(0);
            }
        }
        Some(if cell[2] >= 5 { 0 } else { 3 })
    }

    #[test]
    fn visible_escape_uses_actual_open_windows_and_wall_holes() {
        for (window, hole, open) in [(1, false, true), (1, false, false), (0, true, false)] {
            let mut exits = Vec::new();
            let mut block = |cell| room_with_openings(cell, window, false, hole);
            let result = room(
                [0, 2, 0],
                &mut block,
                &mut |cell| room_with_openings(cell, window, false, hole),
                &mut |_| if open { 4 } else { 0 },
                true,
                &mut (ROOM_LIMIT * 2),
                &mut exits,
            );
            assert_ne!(result, Room::Unknown);
            if window == 1 && open {
                assert_eq!(exits, vec![([0, 3, 4], [0, 0, 1])]);
            } else if hole {
                assert_eq!(exits, vec![([-1, 3, 5], [0, 0, 1])]);
            } else {
                assert!(exits.is_empty());
            }
        }
    }

    #[test]
    fn windows_and_doors_vent_at_fractional_rates_not_unrestricted_sky_rates() {
        for (windows, door, expected) in
            [(1, false, 12), (2, false, 10), (0, true, 8), (1, true, 6)]
        {
            let mut state = FurnaceState::default();
            update_with([0; 3], &mut state, true, 10.0, enclosed, |_| 0);
            for _ in 0..16 {
                update_with(
                    [0; 3],
                    &mut state,
                    true,
                    0.25,
                    |cell| room_with_openings(cell, windows, door, false),
                    |_| 4,
                );
            }
            assert_eq!(
                state.smoke_cells.len(),
                expected,
                "windows={windows}, door={door}"
            );
            assert!(state.vented && state.valid());
        }
        let mut state = FurnaceState::default();
        update_with([0; 3], &mut state, true, 10.0, enclosed, |_| 0);
        for _ in 0..4 {
            update_with(
                [0; 3],
                &mut state,
                false,
                1.0,
                |p| room_with_openings(p, 1, false, false),
                |_| 4,
            );
        }
        assert_eq!(state.smoke_cells.len(), 8);
    }

    #[test]
    fn closed_windows_and_doors_do_not_vent_but_a_wall_hole_does() {
        let mut state = FurnaceState::default();
        update_with([0; 3], &mut state, true, 10.0, enclosed, |_| 0);
        update_with(
            [0; 3],
            &mut state,
            true,
            4.0,
            |p| room_with_openings(p, 1, true, false),
            |_| 0,
        );
        assert_eq!(state.smoke_cells.len(), 14);
        assert!(!state.vented);
        update_with(
            [0; 3],
            &mut state,
            true,
            7.0,
            |p| room_with_openings(p, 1, true, true),
            |_| 0,
        );
        assert!(state.smoke_cells.is_empty() && state.vented);
    }

    #[test]
    fn an_open_internal_door_does_not_vent_without_an_outdoor_connection() {
        let mut state = FurnaceState::default();
        update_with(
            [0; 3],
            &mut state,
            true,
            4.0,
            |p| {
                let raw = room_with_openings(p, 0, true, false);
                if p[2] >= 5 {
                    Some(
                        if (-3..=3).contains(&p[0]) && (1..=4).contains(&p[1]) && p[2] <= 8 {
                            0
                        } else {
                            3
                        },
                    )
                } else {
                    raw
                }
            },
            |_| 4,
        );
        assert_eq!(state.smoke_cells.len(), 4);
        assert!(!state.vented);
    }

    #[test]
    fn tall_sealed_chambers_are_not_mistaken_for_sky_after_32_cells() {
        let mut state = FurnaceState::default();
        update_with(
            [0; 3],
            &mut state,
            true,
            3.0,
            |p| {
                Some(
                    if (-2..=2).contains(&p[0])
                        && (-2..=2).contains(&p[2])
                        && (1..=70).contains(&p[1])
                    {
                        0
                    } else {
                        3
                    },
                )
            },
            |_| 0,
        );
        assert_eq!(state.smoke_cells.len(), 3);
        assert!(!state.vented);
    }

    #[test]
    fn elbow_outlet_preserves_its_horizontal_emission_direction() {
        for facing in 0..4 {
            let direction = front(facing);
            assert_eq!(
                route(
                    [0; 3],
                    &mut |p| Some(if p == [0, 2, 0] { 61 } else { 0 }),
                    &mut |_| facing
                ),
                Route::Outlet(offset([0, 2, 0], direction).unwrap(), direction),
            );
        }
    }

    #[test]
    fn fractional_vent_credit_survives_resume_without_resetting_the_drain() {
        let mut whole = FurnaceState::default();
        update_with([0; 3], &mut whole, true, 10.0, enclosed, |_| 0);
        let mut split = whole.clone();
        update_with(
            [0; 3],
            &mut whole,
            false,
            4.0,
            |p| room_with_openings(p, 1, false, false),
            |_| 4,
        );
        update_with(
            [0; 3],
            &mut split,
            false,
            3.0,
            |p| room_with_openings(p, 1, false, false),
            |_| 4,
        );
        assert_eq!(split.smoke_credit, -0.5);
        assert!(split.valid());
        let mut restored = split.clone();
        update_with(
            [0; 3],
            &mut restored,
            false,
            1.0,
            |p| room_with_openings(p, 1, false, false),
            |_| 4,
        );
        assert_eq!(restored, whole);
    }

    fn shared_room_step(
        states: &mut [FurnaceState; 2],
        lit: bool,
        elapsed: f32,
        windows: usize,
        door: bool,
    ) {
        let anchors = [[-1, 0, 0], [1, 0, 0]];
        for index in 0..2 {
            let other = 1 - index;
            let peer = states[other].clone();
            let sources = [SmokeSource {
                anchor: anchors[other],
                outlet: [anchors[other][0], 2, 0],
                cells: &peer.smoke_cells,
            }];
            update_with_sky(
                anchors[index],
                &mut states[index],
                lit,
                elapsed,
                &sources,
                |cell| room_with_openings(cell, windows, door, false),
                |_| 4,
                |cell| room_with_openings(cell, windows, door, false),
            );
        }
    }

    #[test]
    fn two_furnaces_share_fixed_window_and_door_capacity_and_never_duplicate_cells() {
        for (windows, door, expected) in [
            (0, false, 16),
            (1, false, 12),
            (4, false, 0),
            (0, true, 4),
            (1, true, 0),
        ] {
            let mut states = [FurnaceState::default(), FurnaceState::default()];
            for _ in 0..32 {
                shared_room_step(&mut states, true, 0.25, windows, door);
            }
            let cells: HashSet<_> = states.iter().flat_map(|s| &s.smoke_cells).collect();
            assert_eq!(cells.len(), expected, "windows={windows} door={door}");
            assert_eq!(
                cells.len(),
                states.iter().map(|s| s.smoke_cells.len()).sum::<usize>()
            );
            assert!(states.iter().all(FurnaceState::valid));
        }
    }

    #[test]
    fn shared_ventilation_fraction_resumes_without_multiplying_capacity() {
        let mut states = [FurnaceState::default(), FurnaceState::default()];
        shared_room_step(&mut states, true, 10.0, 0, false);
        assert_eq!(
            states.iter().map(|s| s.smoke_cells.len()).sum::<usize>(),
            20
        );
        let mut uninterrupted = states.clone();
        shared_room_step(&mut uninterrupted, false, 8.0, 1, false);
        shared_room_step(&mut states, false, 3.0, 1, false);
        assert!(states.iter().all(|state| state.smoke_credit == -0.75));
        let mut resumed = states.clone();
        shared_room_step(&mut resumed, false, 5.0, 1, false);
        assert_eq!(resumed, uninterrupted);
        assert_eq!(
            resumed.iter().map(|s| s.smoke_cells.len()).sum::<usize>(),
            16
        );
    }

    #[test]
    fn legacy_overlapping_smoke_is_owned_by_a_stable_source_without_losing_haze() {
        let cells = vec![[-2, 4, 0], [-1, 4, 0], [0, 4, 0], [1, 4, 0]];
        let mut states = [
            FurnaceState {
                smoke_cells: cells.clone(),
                ..Default::default()
            },
            FurnaceState {
                smoke_cells: cells.clone(),
                ..Default::default()
            },
        ];
        shared_room_step(&mut states, true, 0.25, 1, false);
        assert_eq!(states[0].smoke_cells, cells);
        assert!(states[1].smoke_cells.is_empty());
    }

    #[test]
    fn ready_to_ignite_peers_share_vents_but_a_blocked_output_does_not() {
        use terra_core::{Inventory, ItemKind, ItemStack, MineralItem, creatures::AnimalProduct};
        let state = FurnaceState::default();
        let mut inventory = Inventory {
            slots: vec![
                ItemStack {
                    item: ItemKind::Animal(AnimalProduct::Fish),
                    count: 1,
                },
                ItemStack {
                    item: ItemKind::Mineral(MineralItem::Coal),
                    count: 1,
                },
                ItemStack::EMPTY,
                ItemStack::EMPTY,
            ],
            ..Default::default()
        };
        let original = inventory.clone();
        assert!(can_emit(&state, Some(&inventory)));
        assert_eq!(inventory, original);
        inventory.slots[2] = ItemStack {
            item: ItemKind::Block(3),
            count: 128,
        };
        assert!(!can_emit(&state, Some(&inventory)));
        assert!(!can_emit(&state, None));
    }
}
