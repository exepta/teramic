//! Bounded, server-owned productive NPC work over live voxels and inventories.
mod actions;
mod lumberjack;
mod movement;
mod planting;
mod station;
#[cfg(test)]
mod tests;

use crate::{
    TerraGenerator,
    creatures::{Habitat, navigation::Navigation},
    edits::EditJournal,
    streaming::ChunkStore,
};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    ItemKind,
    block_properties::BlockPropertiesCatalog,
    creatures::{Creature, CreatureBehavior, CreatureKind, villagers},
    kingdom::KingdomPermission,
    npc_jobs::{
        WaitReason, WorkKind, WorkOrder, WorkRegistry, WorkStatus, WorkTarget, WorkerState,
    },
};

const WORKER_BUDGET: usize = 16;
const SCAN_BUDGET: usize = 64;

#[derive(Default)]
pub struct WorkRuntime {
    navigation: Navigation,
    suspended: BTreeSet<u64>,
    cursor: usize,
    scans: BTreeMap<u64, usize>,
    door_requests: BTreeMap<[i64; 3], WorkerDoorRequest>,
    travel: BTreeMap<u64, movement::Travel>,
    stations: BTreeMap<u64, station::StationSearch>,
    planting: BTreeMap<u64, planting::PlantSearch>,
    tree_attempts: BTreeMap<u64, ([i64; 3], [f64; 3], f64)>,
    tree_failures: BTreeMap<u64, Vec<([i64; 3], f64)>>,
}

/// Server applies these through the ordinary paired-opening transaction after the work tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerDoorRequest {
    pub npc: u64,
    pub owner: terra_core::PlayerUuid,
    pub position: [i64; 3],
}

/// Wooden world doors only; train cab/iron openings are never operated by workers.
pub const fn worker_openable_door(material: u16) -> bool {
    matches!(material, 65..=67 | 139)
}

impl WorkRuntime {
    /// Transient locomotion ownership while an NPC defends itself. Saved work
    /// orders, tools and partial trees remain intact until the NPC resumes.
    pub fn set_suspended(&mut self, ids: impl IntoIterator<Item = u64>) {
        let next: std::collections::BTreeSet<_> = ids
            .into_iter()
            .filter(|id| *id > 0)
            .take(terra_core::creatures::MAX_CREATURES)
            .collect();
        // Defense can relocate an actor. Discard only transient locomotion and
        // pending opening requests; saved work and tree reservations survive.
        for id in self.suspended.symmetric_difference(&next) {
            self.navigation.invalidate(*id);
            self.navigation.begin_tick(*id);
            self.travel.remove(id);
            self.door_requests.retain(|_, request| request.npc != *id);
        }
        self.suspended = next;
    }
    pub fn drain_door_requests(&mut self) -> impl Iterator<Item = WorkerDoorRequest> {
        std::mem::take(&mut self.door_requests).into_values()
    }
}

/// Productive work owns locomotion until it is truly out of targets. Waiting
/// for tools, a station or a partial tree remains a concrete job, not leisure.
pub fn controls_movement(game: &terra_core::GameplayState, npc: u64, phase: f64) -> bool {
    !villagers::sleeping(npc, phase)
        && game
            .jobs
            .orders
            .values()
            .any(|order| order.npc == npc && order.enabled)
        && !game
            .jobs
            .workers
            .get(&npc)
            .is_some_and(idle_without_targets)
}

fn idle_without_targets(worker: &WorkerState) -> bool {
    worker.status == WorkStatus::Waiting(WaitReason::NoTargets)
        && worker.target.is_none()
        && worker.tree.is_none()
        && worker.work_seconds == 0.0
}

/// Call at 10 Hz. At most sixteen workers and 64 candidate cells per worker are visited.
pub fn tick(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    properties: &BlockPropertiesCatalog,
    phase: f64,
    dt: f32,
    runtime: &mut WorkRuntime,
) {
    runtime.door_requests.clear();
    if !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
        || !phase.is_finite()
        || journal.gameplay().revision == u64::MAX
    {
        return;
    }
    let mut jobs = std::mem::take(&mut journal.gameplay_mut().jobs);
    if !jobs.valid() {
        journal.gameplay_mut().jobs = jobs;
        return;
    }
    runtime.scans.retain(|id, _| jobs.orders.contains_key(id));
    runtime
        .tree_attempts
        .retain(|id, _| jobs.orders.contains_key(id));
    runtime
        .tree_failures
        .retain(|id, _| jobs.orders.contains_key(id));
    runtime.travel.retain(|id, _| {
        jobs.orders
            .values()
            .any(|order| order.npc == *id && order.enabled)
    });
    runtime.stations.retain(|id, _| {
        jobs.orders
            .get(id)
            .is_some_and(|order| order.enabled && order.kind == WorkKind::Lumberjack)
    });
    runtime.planting.retain(|id, _| {
        jobs.orders
            .get(id)
            .is_some_and(|o| o.enabled && o.kind == WorkKind::Lumberjack)
    });
    let enabled: BTreeSet<_> = jobs
        .orders
        .values()
        .filter(|o| o.enabled)
        .map(|o| o.npc)
        .collect();
    for (npc, worker) in &mut jobs.workers {
        if !runtime.suspended.contains(npc) && !enabled.contains(npc) {
            worker.status = WorkStatus::Paused;
            worker.target = None;
            worker.work_seconds = 0.0;
        }
    }
    let ordered = jobs.ordered_orders();
    let mut visited = BTreeSet::new();
    let mut processed = false;
    for offset in 0..ordered.len().min(WORKER_BUDGET) {
        let id = ordered[(runtime.cursor + offset) % ordered.len()];
        let order = jobs.orders[&id].clone();
        // A worker follows its highest-priority enabled assignment only.
        if runtime.suspended.contains(&order.npc)
            || !visited.insert(order.npc)
            || ordered
                .iter()
                .take_while(|&&other| other != id)
                .any(|other| jobs.orders[other].npc == order.npc)
        {
            continue;
        }
        let Some(mut npc) = journal
            .gameplay()
            .creatures
            .animals
            .get(&order.npc)
            .cloned()
        else {
            continue;
        };
        let mut worker = jobs.workers.remove(&order.npc).unwrap_or_default();
        if journal.gameplay().creatures.owners.get(&order.npc) != Some(&order.owner)
            || npc.kind != CreatureKind::Villager
            || npc.health <= 0.0
        {
            worker.status = WorkStatus::Waiting(WaitReason::Protected);
            worker.target = None;
            worker.work_seconds = 0.0;
        } else if villagers::sleeping(npc.id, phase) {
            worker.status = WorkStatus::Waiting(WaitReason::Night);
            worker.target = None;
            worker.work_seconds = 0.0;
        } else {
            let age = npc.age;
            let was_idle = idle_without_targets(&worker);
            let idle_motion = (npc.velocity, npc.behavior);
            step(
                chunks,
                journal,
                terrain,
                properties,
                &order,
                &mut npc,
                &mut worker,
                &jobs,
                dt,
                runtime,
            );
            if was_idle && idle_without_targets(&worker) {
                // Ordinary residents move before this scanner on the server.
                // A fruitless scan must not turn their walk into a frozen pose.
                npc.velocity = idle_motion.0;
                npc.behavior = idle_motion.1;
            }
            if npc.age == age && !idle_without_targets(&worker) {
                let habitat = Habitat {
                    terrain,
                    chunks,
                    journal,
                };
                let size = npc.size();
                crate::creatures::work_rest_with_access(&mut npc, &habitat, dt, |p| {
                    movement::position_allowed(&habitat, &order, p, size)
                });
            }
        }
        jobs.workers.insert(order.npc, worker);
        journal.gameplay_mut().creatures.animals.insert(npc.id, npc);
        processed = true;
    }
    if !ordered.is_empty() {
        runtime.cursor = (runtime.cursor + WORKER_BUDGET) % ordered.len();
    }
    let state = journal.gameplay_mut();
    state.jobs = jobs;
    if processed {
        state.revision += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn step(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    properties: &BlockPropertiesCatalog,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    jobs: &WorkRegistry,
    dt: f32,
    runtime: &mut WorkRuntime,
) {
    let at = npc.position.map(|v| v.floor() as i64);
    if chunks.block_at(at).is_none() || chunks.block_at(order.storage).is_none() {
        wait(worker, npc, WaitReason::Unloaded);
        return;
    }
    if !allowed(chunks, journal, order, KingdomPermission::NpcWork, at)
        || !storage_allowed(chunks, journal, order)
    {
        wait(worker, npc, WaitReason::Protected);
        return;
    }
    if !chunks.block_at(order.storage).is_some_and(|material| {
        terra_core::profession_stations::supports_work(material, order.kind)
    }) {
        wait(worker, npc, WaitReason::MissingMaterial);
        return;
    }
    if order.kind == WorkKind::Lumberjack
        && !station::ready(chunks, journal, terrain, order, npc, worker, dt, runtime)
    {
        return;
    }
    // Collect one bounded felling batch before walking back to storage. A free
    // bag slot guarantees the next real drop fits at least partially; a full bag
    // still unloads immediately instead of deadlocking behind pending drops.
    let collecting_batch = order.kind == WorkKind::Lumberjack
        && !matches!(
            worker.status,
            WorkStatus::Waiting(
                WaitReason::NoPath
                    | WaitReason::Protected
                    | WaitReason::Unloaded
                    | WaitReason::StorageFull
            )
        )
        && worker
            .inventory
            .slots
            .iter()
            .filter(|stack| stack.valid())
            .count()
            < terra_core::npc_jobs::WORKER_SLOTS
        && worker.tree.as_ref().is_some_and(|tree| {
            tree.drops
                .iter()
                .any(|id| journal.gameplay().drops.iter().any(|drop| drop.id == *id))
        });
    let delivering = actions::has_delivery(order, worker) && !collecting_batch;
    if delivering {
        worker.target = Some(WorkTarget::Storage(order.storage));
        if movement::approach(
            chunks,
            journal,
            terrain,
            order,
            npc,
            worker,
            order.storage,
            dt,
            runtime,
            true,
        ) {
            match actions::deliver(journal, order, worker) {
                Ok(true) => {
                    worker.target = None;
                    worker.status = WorkStatus::Idle;
                }
                Ok(false) => wait(worker, npc, WaitReason::StorageFull),
                Err(_) => wait(worker, npc, WaitReason::MissingMaterial),
            }
        }
        return;
    }
    if matches!(worker.target, Some(WorkTarget::Storage(_))) {
        worker.target = None;
    }
    if order.kind == WorkKind::Lumberjack {
        lumberjack::step(
            chunks, journal, terrain, properties, order, npc, worker, jobs, dt, runtime,
        );
        return;
    }
    if worker.target.is_none() {
        let scan = runtime.scans.entry(order.id).or_default();
        let complete_scan = *scan + SCAN_BUDGET >= order.area.volume();
        let (target, unloaded) = select_target(chunks, journal, order, jobs, *scan);
        worker.target = target;
        *scan = (*scan + SCAN_BUDGET) % order.area.volume().max(1);
        worker.work_seconds = 0.0;
        if worker.target.is_none() && unloaded {
            wait(worker, npc, WaitReason::Unloaded);
            return;
        }
        if worker.target.is_none()
            && !complete_scan
            && matches!(
                worker.status,
                WorkStatus::Waiting(
                    WaitReason::MissingTool
                        | WaitReason::MissingMaterial
                        | WaitReason::NoPath
                        | WaitReason::Protected
                )
            )
        {
            npc.velocity[0] = 0.0;
            npc.velocity[2] = 0.0;
            npc.behavior = CreatureBehavior::Rest;
            return;
        }
    }
    let Some(target) = worker.target else {
        wait(worker, npc, WaitReason::NoTargets);
        return;
    };
    let Some(cell) = target_cell(journal, target) else {
        worker.target = None;
        return;
    };
    if !order.area.contains(cell) {
        worker.target = None;
        return;
    }
    if jobs.reservation_owner(&target).is_some() {
        worker.target = None;
        return;
    }
    let tool = worker
        .inventory
        .slots
        .iter()
        .filter_map(|stack| match stack.item.tool_kind() {
            Some(tool) if stack.valid() && order.kind.accepts_tool(tool) => Some(tool),
            _ => None,
        })
        .max_by_key(|tool| (tool.mining_level(), tool.farm_tier()));
    let seed = (order.kind == WorkKind::Farmer)
        .then(|| {
            journal
                .crop_at(cell)
                .map(|crop| ItemKind::Crop(crop.kind.seed()))
        })
        .flatten();
    let minimum_level = if order.kind == WorkKind::Miner {
        chunks.block_at(cell).map_or(0, |material| {
            properties
                .0
                .get(&material)
                .map_or(0, |p| p.mining_level)
                .max(
                    terra_core::OreKind::from_material(material)
                        .map_or(0, |ore| ore.mining_level()),
                )
        })
    } else {
        0
    };
    let missing_tool = order.kind != WorkKind::Collector
        && tool.is_none_or(|tool| tool.mining_level() < minimum_level);
    let missing_seed = seed.is_some_and(|seed| worker.inventory.count(seed) == 0);
    if missing_tool || missing_seed {
        if movement::approach(
            chunks,
            journal,
            terrain,
            order,
            npc,
            worker,
            order.storage,
            dt,
            runtime,
            true,
        ) {
            match actions::supply(
                journal,
                order,
                worker,
                missing_tool,
                seed.filter(|_| missing_seed),
                minimum_level,
            ) {
                Ok(true) => worker.status = WorkStatus::Idle,
                Err(reason) => wait(worker, npc, reason),
                _ => wait(
                    worker,
                    npc,
                    if missing_tool {
                        WaitReason::MissingTool
                    } else {
                        WaitReason::MissingMaterial
                    },
                ),
            }
        }
        return;
    }
    if !allowed(chunks, journal, order, KingdomPermission::NpcWork, cell)
        || !allowed(chunks, journal, order, KingdomPermission::Break, cell)
        || (order.kind == WorkKind::Farmer
            && !allowed(chunks, journal, order, KingdomPermission::Build, cell))
        || (order.kind == WorkKind::Collector
            && (!allowed(chunks, journal, order, KingdomPermission::Inventories, cell)
                || !crate::kingdom_access::can_transport(chunks, journal, cell, order.storage)))
    {
        wait(worker, npc, WaitReason::Protected);
        return;
    }
    if movement::approach(
        chunks, journal, terrain, order, npc, worker, cell, dt, runtime, false,
    ) {
        worker.status = WorkStatus::Working;
        worker.work_seconds += dt;
        let result = match order.kind {
            WorkKind::Miner => actions::mine(chunks, journal, properties, npc, worker, cell, tool),
            WorkKind::Farmer => actions::farm(chunks, journal, worker, cell),
            WorkKind::Collector => actions::collect(journal, worker, target),
            WorkKind::Lumberjack => {
                unreachable!("woodcutters use their persistent felling workflow")
            }
        };
        match result {
            Ok(true) => {
                worker.target = None;
                worker.work_seconds = 0.0;
                worker.status = WorkStatus::Idle;
            }
            Ok(false) => {}
            Err(reason) => wait(worker, npc, reason),
        }
    }
}

fn select_target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    order: &WorkOrder,
    jobs: &WorkRegistry,
    scan: usize,
) -> (Option<WorkTarget>, bool) {
    if order.kind == WorkKind::Collector {
        return (
            journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| {
                    drop.age_seconds >= drop.pickup_delay_seconds
                        && order.area.contains(drop.position.map(|v| v.floor() as i64))
                })
                .take(256)
                .map(|drop| WorkTarget::Drop(drop.id))
                .find(|target| jobs.reservation_owner(target).is_none()),
            false,
        );
    }
    let mut unloaded = false;
    let volume = order.area.volume();
    let target = (0..SCAN_BUDGET.min(volume))
        .filter_map(|offset| order.area.cell_at((scan + offset) % volume))
        .find_map(|cell| {
            unloaded |= chunks.block_at(cell).is_none();
            let candidate = match order.kind {
                WorkKind::Miner => chunks
                    .block_at(cell)
                    .is_some_and(|material| actions::mineable(chunks, journal, cell, material)),
                WorkKind::Farmer => journal.crop_at(cell).is_some_and(|crop| crop.mature()),
                WorkKind::Collector => false,
                WorkKind::Lumberjack => false,
            };
            let target = WorkTarget::Block(cell);
            (candidate && jobs.reservation_owner(&target).is_none()).then_some(target)
        });
    (target, unloaded)
}

fn target_cell(journal: &EditJournal, target: WorkTarget) -> Option<[i64; 3]> {
    match target {
        WorkTarget::Block(cell) | WorkTarget::Storage(cell) | WorkTarget::Tree(cell) => Some(cell),
        WorkTarget::Prop { position, .. } => Some(position),
        WorkTarget::Drop(id) => journal
            .gameplay()
            .drops
            .iter()
            .find(|drop| drop.id == id)
            .map(|drop| drop.position.map(|v| v.floor() as i64)),
    }
}

fn allowed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    order: &WorkOrder,
    permission: KingdomPermission,
    cell: [i64; 3],
) -> bool {
    crate::kingdom_access::can_use(
        &journal.gameplay().kingdoms,
        chunks,
        order.owner,
        permission,
        cell,
    )
}

fn storage_allowed(chunks: &ChunkStore, journal: &EditJournal, order: &WorkOrder) -> bool {
    allowed(
        chunks,
        journal,
        order,
        KingdomPermission::Inventories,
        order.storage,
    ) && journal
        .object_state(order.storage)
        .and_then(|state| state.peer)
        .is_none_or(|peer| allowed(chunks, journal, order, KingdomPermission::Inventories, peer))
}

fn wait(worker: &mut WorkerState, npc: &mut Creature, reason: WaitReason) {
    worker.status = WorkStatus::Waiting(reason);
    worker.target = None;
    worker.work_seconds = 0.0;
    npc.velocity = [0.0, npc.velocity[1].min(0.0), 0.0];
    npc.behavior = CreatureBehavior::Rest;
}
