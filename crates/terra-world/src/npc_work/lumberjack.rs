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
        .filter(|item| item.tool_kind().is_some_and(|tool| tool.durability() > 0))
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
                });
            *scan = (*scan + SCAN_BUDGET) % volume.max(1);
            result
        });
        let Some(root) = root else {
            wait(worker, npc, WaitReason::NoTargets);
            return;
        };
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
    let Some(mut task) = worker.tree.take() else {
        return;
    };
    let result = advance(
        chunks, journal, terrain, properties, order, npc, worker, &mut task, dt, runtime,
    );
    let abandon_unstarted =
        !task.started && matches!(result, Err(WaitReason::NoPath | WaitReason::Protected));
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
            chunks,
            journal,
            terrain,
            order,
            npc,
            worker,
            cell,
            dt,
            &mut runtime.navigation,
            false,
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
        return plant(
            chunks, journal, terrain, order, npc, worker, task, dt, runtime,
        );
    }
    if !task.wood.is_empty() {
        let Some(tool) = axe(worker) else {
            if movement::approach(
                chunks,
                journal,
                terrain,
                order,
                npc,
                worker,
                order.storage,
                dt,
                &mut runtime.navigation,
                true,
            ) {
                actions::supply(journal, order, worker, true, None, 0)?;
            }
            return Ok(false);
        };
        if !can_wear(worker, tool) {
            return Err(WaitReason::StorageFull);
        }
        if !movement::approach(
            chunks,
            journal,
            terrain,
            order,
            npc,
            worker,
            task.foot,
            dt,
            &mut runtime.navigation,
            false,
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
    plant(
        chunks, journal, terrain, order, npc, worker, task, dt, runtime,
    )
}

#[allow(clippy::too_many_arguments)]
fn plant(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    task: &mut TreeWork,
    dt: f32,
    runtime: &mut WorkRuntime,
) -> Result<bool, WaitReason> {
    let item = ItemKind::Prop(task.species.sapling());
    if !task.sapling_loaded || worker.inventory.count(item) == 0 {
        task.sapling_loaded = false;
        if movement::approach(
            chunks,
            journal,
            terrain,
            order,
            npc,
            worker,
            order.storage,
            dt,
            &mut runtime.navigation,
            true,
        ) {
            actions::supply(journal, order, worker, false, Some(item), 0)?;
            task.sapling_loaded = true;
        }
        return Ok(false);
    }
    // The former root is preferred; a bounded adjacent search permits a stump
    // replaced by a player without planting on buildings or silently discarding seed.
    let mut unloaded = false;
    let former_root = task.root;
    let support = std::iter::once([former_root[0], former_root[1] - 1, former_root[2]])
        .chain((-2_i64..=2).flat_map(|x| {
            (-2_i64..=2).map(move |z| [former_root[0] + x, former_root[1] - 1, former_root[2] + z])
        }))
        .filter(|cell| order.area.contains([cell[0], cell[1] + 1, cell[2]]))
        .find(|cell| {
            unloaded |= chunks.block_at(*cell).is_none();
            let prop = crate::saplings::placement_prop(
                *cell,
                task.species,
                journal.prop_epoch(terra_voxel::global_to_section(*cell).0),
            );
            allowed(chunks, journal, order, KingdomPermission::NpcWork, *cell)
                && crate::saplings::placement_check_in_world(
                    chunks,
                    journal,
                    terrain,
                    prop,
                    Some(order.owner),
                )
                .is_ok()
        });
    let Some(support) = support else {
        return Err(if unloaded {
            WaitReason::Unloaded
        } else {
            WaitReason::NoPlantingSite
        });
    };
    let root = [support[0], support[1] + 1, support[2]];
    if !movement::approach(
        chunks,
        journal,
        terrain,
        order,
        npc,
        worker,
        root,
        dt,
        &mut runtime.navigation,
        false,
    ) {
        return Ok(false);
    }
    worker.status = WorkStatus::Working;
    worker.work_seconds += dt;
    if worker.work_seconds < 0.75 {
        return Ok(false);
    }
    let mut next = worker.inventory.clone();
    if next.revision == u64::MAX || !next.take_one(item) {
        return Err(WaitReason::MissingMaterial);
    }
    let prop = crate::saplings::placement_prop(
        support,
        task.species,
        journal.prop_epoch(terra_voxel::global_to_section(support).0),
    );
    if !crate::saplings::plant(chunks, journal, prop, order.owner)
        .map_err(|_| WaitReason::NoPlantingSite)?
    {
        return Err(WaitReason::NoPlantingSite);
    }
    worker.inventory = next;
    worker.target = None;
    worker.work_seconds = 0.0;
    worker.status = WorkStatus::Idle;
    Ok(true)
}
