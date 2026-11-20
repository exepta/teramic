//! Productive woodcutting uses exact existing felling plans and real world drops.
use super::*;
use terra_core::{
    ToolKind,
    npc_jobs::{TreeWork, TreeWorkStage},
};

const CUT_BUDGET: usize = 8;
const LEAF_BUDGET: usize = 32;

fn axe(worker: &WorkerState) -> Option<ItemKind> {
    worker
        .inventory
        .slots
        .iter()
        .filter(|stack| stack.valid())
        .map(|stack| stack.item)
        .filter(|item| {
            item.tool_kind()
                .is_some_and(|tool| WorkKind::Lumberjack.accepts_tool(tool))
        })
        .max_by_key(|item| item.tool_kind().map_or(0, ToolKind::mining_level))
}

/// A successful committed log consumes exactly one use of exactly one physical axe.
fn wear(worker: &mut WorkerState, item: ItemKind) -> bool {
    if worker.inventory.revision >= u64::MAX - 2 {
        return false;
    }
    let Some(tool) = item.tool_kind() else {
        return false;
    };
    let remaining = match item {
        ItemKind::WornTool { remaining, .. } => remaining,
        _ => tool.durability(),
    };
    if remaining == 0 || !worker.inventory.take_one(item) {
        return false;
    }
    if remaining > 1 {
        let damaged = ItemKind::WornTool {
            tool,
            remaining: remaining - 1,
        };
        // Fresh axes historically stack. Splitting one must fit before work commits.
        if worker.insert(damaged, 1) != 0 {
            return false;
        }
    }
    true
}

fn can_wear(worker: &WorkerState, item: ItemKind) -> bool {
    wear(&mut worker.clone(), item)
}

/// The canonical root can be buried inside a wide generated trunk. Work at an
/// attributed lower surface cell with a real standing place, reserving the same
/// root for the entire tree. Never increase interaction reach through wood.
fn work_face(
    task: &TreeWork,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &Creature,
) -> Option<[i64; 3]> {
    let habitat = Habitat {
        chunks,
        journal,
        terrain,
    };
    task.wood
        .iter()
        .copied()
        .filter(|cell| (task.root[1]..=task.root[1] + 2).contains(&cell[1]))
        .filter(|cell| {
            [[-1, 0], [1, 0], [0, -1], [0, 1]]
                .into_iter()
                .any(|[dx, dz]| {
                    let point = [
                        cell[0] as f64 + 0.5 + f64::from(dx),
                        cell[1] as f64,
                        cell[2] as f64 + 0.5 + f64::from(dz),
                    ];
                    habitat
                        .ground(point, CreatureKind::Villager)
                        .is_some_and(|standing| {
                            (standing[1] - cell[1] as f64).abs() <= 1.0
                                && allowed(
                                    chunks,
                                    journal,
                                    order,
                                    KingdomPermission::NpcWork,
                                    standing.map(|v| v.floor() as i64),
                                )
                        })
                })
        })
        .min_by(|a, b| {
            a[1].cmp(&b[1]).then_with(|| {
                let distance = |cell: [i64; 3]| {
                    (cell[0] as f64 + 0.5 - npc.position[0]).powi(2)
                        + (cell[2] as f64 + 0.5 - npc.position[2]).powi(2)
                };
                distance(*a).total_cmp(&distance(*b))
            })
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
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
    if worker.tree.is_none() {
        runtime
            .tree_failures
            .entry(order.id)
            .or_default()
            .retain(|(_, until)| *until > npc.age);
        let root = match worker.target {
            Some(WorkTarget::Tree(root)) => Some(root),
            _ => None,
        };
        let root = root.or_else(|| {
            let scan = runtime.scans.entry(order.id).or_default();
            let volume = order.area.volume();
            let result = (0..SCAN_BUDGET.min(volume))
                .filter_map(|n| order.area.cell_at((*scan + n) % volume))
                .find(|cell| {
                    crate::tree_felling::worker_root(chunks, journal, *cell)
                        && jobs.reservation_owner(&WorkTarget::Tree(*cell)).is_none()
                        && !runtime
                            .tree_failures
                            .get(&order.id)
                            .is_some_and(|failed| failed.iter().any(|(root, _)| root == cell))
                });
            *scan = (*scan + SCAN_BUDGET) % volume.max(1);
            result
        });
        if root.is_none() {
            match planting::select(chunks, journal, terrain, order, npc, worker, jobs, runtime) {
                Ok(Some(task)) => {
                    worker.target = Some(WorkTarget::Tree(task.root));
                    worker.tree = Some(task);
                    worker.work_seconds = 0.0;
                }
                Ok(None) => {
                    wait(worker, npc, WaitReason::NoTargets);
                    return;
                }
                Err(reason) => {
                    wait(worker, npc, reason);
                    return;
                }
            }
        }
        if let Some(root) = root {
            worker.target = Some(WorkTarget::Tree(root));
            // A planted tree belongs to its planter; permission to break a claim alone
            // does not authorize automatically harvesting someone else's orchard.
            if journal.gameplay().trees.trees.values().any(|tree| {
                tree.root == root && tree.planted_by.is_some_and(|owner| owner != order.owner)
            }) {
                wait(worker, npc, WaitReason::Protected);
                return;
            }
            crate::tree_felling::request_missing(chunks, journal, root);
            let Some(plan) = crate::tree_felling::Plan::prepare(chunks, journal, root) else {
                wait(worker, npc, WaitReason::Unloaded);
                return;
            };
            if !plan.allowed_for(order.owner, chunks, journal) {
                wait(worker, npc, WaitReason::Protected);
                return;
            }
            let Some(mut task) = plan.worker_task() else {
                wait(worker, npc, WaitReason::NoTargets);
                return;
            };
            let Some(foot) = work_face(&task, chunks, journal, terrain, order, npc) else {
                wait(worker, npc, WaitReason::NoPath);
                return;
            };
            task.foot = foot;
            worker.tree = Some(task);
            worker.work_seconds = 0.0;
        }
    }
    let Some(mut task) = worker.tree.take() else {
        return;
    };
    let result = advance(
        chunks, journal, terrain, properties, order, npc, worker, &mut task, dt, runtime,
    );
    let attempt =
        runtime
            .tree_attempts
            .entry(order.id)
            .or_insert((task.root, npc.position, npc.age));
    if attempt.0 != task.root
        || crate::creatures::distance(attempt.1, npc.position) > 0.05_f64.powi(2)
    {
        *attempt = (task.root, npc.position, npc.age);
    }
    let blocked_unstarted = !task.started
        && worker.status == WorkStatus::Waiting(WaitReason::NoPath)
        && npc.age - attempt.2 > 6.0;
    let abandon_unstarted = !task.started
        && matches!(result, Err(WaitReason::NoPath | WaitReason::Protected))
        || !task.started
            && task.stage == TreeWorkStage::Planting
            && matches!(result, Err(WaitReason::NoPlantingSite))
        || blocked_unstarted;
    if abandon_unstarted {
        let failed = runtime.tree_failures.entry(order.id).or_default();
        if failed.len() >= 8 {
            failed.remove(0);
        }
        failed.push((task.root, npc.age + 8.0));
        runtime.tree_attempts.remove(&order.id);
        runtime.travel.remove(&npc.id);
        runtime.navigation.invalidate(npc.id);
        worker.target = None;
        worker.work_seconds = 0.0;
    }
    if !matches!(result, Ok(true)) && !abandon_unstarted {
        worker.tree = Some(task);
    }
    if let Err(reason) = result {
        wait(worker, npc, reason);
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    properties: &BlockPropertiesCatalog,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    task: &mut TreeWork,
    dt: f32,
    runtime: &mut WorkRuntime,
) -> Result<bool, WaitReason> {
    if !order.area.contains(task.root) {
        return Err(WaitReason::Protected);
    }
    worker.target = Some(WorkTarget::Tree(task.root));
    if !allowed(
        chunks,
        journal,
        order,
        KingdomPermission::NpcWork,
        task.root,
    ) {
        return Err(WaitReason::Protected);
    }

    // Drops remain independent authoritative objects. Other collectors or players
    // may pick them up; an absent drop is never recreated by the worker.
    task.drops
        .retain(|id| journal.gameplay().drops.iter().any(|drop| drop.id == *id));
    if let Some(drop) = task
        .drops
        .iter()
        .find_map(|id| {
            journal
                .gameplay()
                .drops
                .iter()
                .find(|drop| drop.id == *id && drop.age_seconds >= drop.pickup_delay_seconds)
        })
        .cloned()
    {
        let cell = drop.position.map(|v| v.floor() as i64);
        if !allowed(chunks, journal, order, KingdomPermission::Inventories, cell)
            || !crate::kingdom_access::can_transport(chunks, journal, cell, order.storage)
        {
            return Err(WaitReason::Protected);
        }
        worker.target = Some(WorkTarget::Drop(drop.id));
        if movement::approach(
            chunks, journal, terrain, order, npc, worker, cell, dt, runtime, false,
        ) {
            actions::collect(journal, worker, WorkTarget::Drop(drop.id))?;
        }
        return Ok(false);
    }
    if !task.drops.is_empty() {
        worker.status = WorkStatus::Working;
        return Ok(false);
    }

    if task.stage == TreeWorkStage::Planting {
        return planting::step(
            chunks, journal, terrain, order, npc, worker, task, dt, runtime,
        );
    }
    if !task.wood.is_empty() {
        let Some(tool) = axe(worker) else {
            let supplies = station::supply_order(chunks, journal, order, runtime, None);
            if movement::approach(
                chunks,
                journal,
                terrain,
                &supplies,
                npc,
                worker,
                supplies.storage,
                dt,
                runtime,
                true,
            ) {
                actions::supply(journal, &supplies, worker, true, None, 0)?;
            }
            return Ok(false);
        };
        if !can_wear(worker, tool) {
            return Err(WaitReason::StorageFull);
        }
        if !movement::approach(
            chunks, journal, terrain, order, npc, worker, task.foot, dt, runtime, false,
        ) {
            return Ok(false);
        }
        worker.status = WorkStatus::Working;
        if !task.started {
            worker.work_seconds += dt;
            let duration = crate::tree_felling::duration(
                task.wood.len(),
                tool.tool_kind().map(ItemKind::Tool),
            )
            .max(0.5);
            if worker.work_seconds < duration {
                return Ok(false);
            }
            task.started = true;
            worker.work_seconds = 0.0;
        }
        for _ in 0..CUT_BUDGET.min(task.wood.len()) {
            if task.wood.last().is_some_and(|cell| {
                !allowed(chunks, journal, order, KingdomPermission::NpcWork, *cell)
            }) {
                return Err(WaitReason::Protected);
            }
            let Some(tool) = axe(worker) else {
                break;
            };
            let mut next = worker.clone();
            if !wear(&mut next, tool) {
                return Err(WaitReason::StorageFull);
            }
            if crate::tree_felling::worker_cut_one(
                chunks,
                journal,
                properties,
                order.owner,
                task,
                true,
            )? {
                worker.inventory = next.inventory;
            }
        }
        if task.wood.is_empty() {
            task.stage = TreeWorkStage::Collecting;
            worker.work_seconds = 0.0;
        }
        return Ok(false);
    }
    if !task.leaves.is_empty() {
        worker.status = WorkStatus::Working;
        worker.work_seconds += dt;
        if worker.work_seconds < crate::tree_felling::LEAF_DELAY as f32 {
            return Ok(false);
        }
        for _ in 0..LEAF_BUDGET.min(task.leaves.len()) {
            let cell = *task.leaves.last().ok_or(WaitReason::NoTargets)?;
            if !allowed(chunks, journal, order, KingdomPermission::NpcWork, cell) {
                return Err(WaitReason::Protected);
            }
            if chunks.block_at(cell).is_none() {
                chunks.request_sections([terra_voxel::global_to_section(cell).0]);
                return Err(WaitReason::Unloaded);
            }
            crate::tree_felling::worker_cut_one(
                chunks,
                journal,
                properties,
                order.owner,
                task,
                false,
            )?;
        }
        return Ok(false);
    }
    task.stage = TreeWorkStage::Planting;
    worker.work_seconds = 0.0;
    planting::step(
        chunks, journal, terrain, order, npc, worker, task, dt, runtime,
    )
}
