//! A profession table is a real, reachable world block, separate from its depot.
//! Searches are bounded and resumable. Readiness is deliberately re-established
//! after reload or reassignment, while saved tools and partial trees stay intact.
use super::*;

type SearchKey = (
    u64,
    [i64; 3],
    terra_core::npc_jobs::WorkArea,
    terra_core::PlayerUuid,
);

#[derive(Clone, Copy, Default)]
pub(super) struct StationSearch {
    key: Option<SearchKey>,
    station: Option<[i64; 3]>,
    visited: bool,
    near_cursor: usize,
    area_cursor: usize,
    unloaded_area: bool,
    failed: Option<([i64; 3], f64)>,
}

const NEAR_WIDTH: usize = 9;
const NEAR_HEIGHT: usize = 5;
const NEAR_VOLUME: usize = NEAR_WIDTH * NEAR_WIDTH * NEAR_HEIGHT;
const OFFSETS: [i64; NEAR_WIDTH] = [0, -1, 1, -2, 2, -3, 3, -4, 4];
const HEIGHTS: [i64; NEAR_HEIGHT] = [0, -1, 1, -2, 2];

fn near(storage: [i64; 3], index: usize) -> Option<[i64; 3]> {
    Some([
        storage[0].checked_add(OFFSETS[index % NEAR_WIDTH])?,
        storage[1].checked_add(HEIGHTS[index / (NEAR_WIDTH * NEAR_WIDTH)])?,
        storage[2].checked_add(OFFSETS[(index / NEAR_WIDTH) % NEAR_WIDTH])?,
    ])
}

fn usable(chunks: &ChunkStore, journal: &EditJournal, order: &WorkOrder, cell: [i64; 3]) -> bool {
    chunks.block_at(cell) == Some(terra_core::profession_stations::LUMBERJACK)
        && allowed(chunks, journal, order, KingdomPermission::NpcWork, cell)
}

/// Supplies can live in the profession table itself even when a separate chest
/// was chosen as the output depot. No table inventory is read remotely.
pub(super) fn supply_order(
    chunks: &ChunkStore,
    journal: &EditJournal,
    order: &WorkOrder,
    runtime: &WorkRuntime,
    seed: Option<ItemKind>,
) -> WorkOrder {
    let has_supply = |cell| {
        journal.container_at(cell).is_some_and(|bag| {
            if let Some(seed) = seed {
                bag.count(seed) > 0
            } else {
                bag.slots.iter().any(|stack| {
                    stack.valid()
                        && stack
                            .item
                            .tool_kind()
                            .is_some_and(|tool| order.kind.accepts_tool(tool))
                })
            }
        })
    };
    if has_supply(order.storage) {
        return order.clone();
    }
    let Some(cell) = runtime
        .stations
        .get(&order.id)
        .filter(|state| state.visited)
        .and_then(|state| state.station)
        .filter(|cell| usable(chunks, journal, order, *cell) && has_supply(*cell))
    else {
        return order.clone();
    };
    let mut supplies = order.clone();
    supplies.storage = cell;
    if storage_allowed(chunks, journal, &supplies)
        && crate::kingdom_access::can_transport(chunks, journal, cell, order.storage)
    {
        supplies
    } else {
        order.clone()
    }
}

fn discover(
    chunks: &ChunkStore,
    journal: &EditJournal,
    order: &WorkOrder,
    search: &mut StationSearch,
    now: f64,
) -> Option<[i64; 3]> {
    let failed = search.failed;
    let permitted = |cell| {
        failed.is_none_or(|(failed, until)| failed != cell || now >= until)
            && usable(chunks, journal, order, cell)
    };
    // A station can itself hold supplies, but an ordinary chest never counts.
    if permitted(order.storage) {
        return Some(order.storage);
    }
    // Split the budget so new nearby tables do not wait behind a tall work area.
    for _ in 0..SCAN_BUDGET / 2 {
        let cell = near(order.storage, search.near_cursor);
        search.near_cursor = (search.near_cursor + 1) % NEAR_VOLUME;
        if let Some(cell) = cell
            && permitted(cell)
        {
            return Some(cell);
        }
    }
    let volume = order.area.volume();
    for _ in 0..(SCAN_BUDGET / 2).min(volume) {
        if search.area_cursor == 0 {
            search.unloaded_area = false;
        }
        let cell = order.area.cell_at(search.area_cursor);
        search.area_cursor = (search.area_cursor + 1) % volume.max(1);
        if let Some(cell) = cell {
            search.unloaded_area |= chunks.block_at(cell).is_none();
            if permitted(cell) {
                return Some(cell);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)] // Uses the same bounded world-work context as other job stages.
pub(super) fn ready(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    dt: f32,
    runtime: &mut WorkRuntime,
) -> bool {
    let key = (order.revision, order.storage, order.area, order.owner);
    let mut search = runtime.stations.get(&order.id).copied().unwrap_or_default();
    if search.key != Some(key) {
        search = StationSearch {
            key: Some(key),
            ..Default::default()
        };
    }
    if let Some(station) = search.station {
        if chunks.block_at(station).is_none() {
            search.visited = false;
            runtime.stations.insert(order.id, search);
            wait(worker, npc, WaitReason::Unloaded);
            return false;
        }
        if !usable(chunks, journal, order, station) {
            search.station = None;
            search.visited = false;
        }
    }
    if search.station.is_none() {
        search.station = discover(chunks, journal, order, &mut search, npc.age);
    }
    let ready = if let Some(station) = search.station {
        if search.visited {
            true
        } else if movement::approach(
            chunks, journal, terrain, order, npc, worker, station, dt, runtime, false,
        ) {
            search.visited = true;
            worker.target = None;
            worker.work_seconds = 0.0;
            worker.status = WorkStatus::Idle;
            true
        } else {
            if worker.status == WorkStatus::Waiting(WaitReason::NoPath) {
                search.failed = Some((station, npc.age + 2.0));
                search.station = None;
            }
            false
        }
    } else {
        let reason = if search.failed.is_some_and(|(_, until)| npc.age < until) {
            WaitReason::NoPath
        } else if search.unloaded_area {
            WaitReason::Unloaded
        } else {
            WaitReason::MissingStation
        };
        wait(worker, npc, reason);
        false
    };
    runtime.stations.insert(order.id, search);
    if ready {
        // Sex and identity remain encoded in the creature ID; only its job changes.
        npc.variant = villagers::LUMBERJACK;
    }
    ready
}
