//! Seed-backed planting is useful before the first mature tree exists, too.
use super::*;
use terra_core::{
    TreeSpecies, TreeStage,
    npc_jobs::{TreeWork, TreeWorkStage, WorkArea},
};

#[derive(Default)]
pub(super) struct PlantSearch {
    key: Option<(u64, WorkArea, terra_core::PlayerUuid)>,
    cursor: usize,
    unloaded: bool,
    protected: bool,
    occupied_or_spaced: bool,
}
fn spaced(
    root: [i64; 3],
    species: TreeSpecies,
    other: [i64; 3],
    other_species: TreeSpecies,
) -> bool {
    let distance = species.spacing().max(other_species.spacing()) as f64;
    ((root[0] - other[0]) as f64).hypot((root[2] - other[2]) as f64) >= distance
}
#[allow(clippy::too_many_arguments)]
pub(super) fn select(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &Creature,
    worker: &WorkerState,
    jobs: &WorkRegistry,
    runtime: &mut WorkRuntime,
) -> Result<Option<TreeWork>, WaitReason> {
    let available: Vec<_> = TreeSpecies::ALL
        .into_iter()
        .filter(|species| {
            let item = ItemKind::Prop(species.sapling());
            worker.inventory.count(item) > 0 || {
                let source = station::supply_order(chunks, journal, order, runtime, Some(item));
                storage_allowed(chunks, journal, &source)
                    && journal
                        .container_at(source.storage)
                        .is_some_and(|bag| bag.count(item) > 0)
            }
        })
        .collect();
    if available.is_empty() {
        return Ok(None);
    }
    let failed = runtime
        .tree_failures
        .get(&order.id)
        .cloned()
        .unwrap_or_default();
    let search = runtime.planting.entry(order.id).or_default();
    let key = (order.revision, order.area, order.owner);
    if search.key != Some(key) {
        *search = PlantSearch {
            key: Some(key),
            ..Default::default()
        };
    }
    if search.cursor == 0 {
        search.unloaded = false;
        search.protected = false;
        search.occupied_or_spaced = false;
    }
    let volume = order.area.volume();
    let budget = SCAN_BUDGET.min(volume.saturating_sub(search.cursor));
    for _ in 0..budget {
        let Some(root) = order.area.cell_at(search.cursor) else {
            break;
        };
        search.cursor += 1;
        let support = [root[0], root[1] - 1, root[2]];
        let Some(soil) = chunks.block_at(support) else {
            search.unloaded = true;
            continue;
        };
        if !crate::saplings::soil(soil) {
            continue;
        }
        let Some(block) = chunks.block_at(root) else {
            search.unloaded = true;
            continue;
        };
        if block != 0 {
            search.occupied_or_spaced = true;
            continue;
        }
        if !allowed(chunks, journal, order, KingdomPermission::NpcWork, support)
            || !allowed(chunks, journal, order, KingdomPermission::NpcWork, root)
        {
            search.protected = true;
            continue;
        }
        if failed
            .iter()
            .any(|(p, until)| *p == root && *until > npc.age)
        {
            continue;
        }
        for &species in &available {
            let occupied = journal.gameplay().trees.trees.values().any(|tree| {
                (matches!(tree.stage, TreeStage::Sapling) || tree.stage.is_mature())
                    && !spaced(root, species, tree.root, tree.species)
            });
            // Paused or travelling coworkers retain their own planting/root reservation.
            let reserved = jobs
                .workers
                .values()
                .filter_map(|w| w.tree.as_ref())
                .any(|tree| !spaced(root, species, tree.root, tree.species));
            if occupied || reserved {
                search.occupied_or_spaced = true;
                continue;
            }
            let prop = crate::saplings::placement_prop(
                support,
                species,
                journal.prop_epoch(terra_voxel::global_to_section(support).0),
            );
            match crate::saplings::placement_check_in_world(
                chunks,
                journal,
                terrain,
                prop,
                Some(order.owner),
            ) {
                Ok(()) => {
                    return Ok(Some(TreeWork {
                        root,
                        foot: root,
                        species,
                        planted: None,
                        wood: Vec::new(),
                        leaves: Vec::new(),
                        drops: Vec::new(),
                        stage: TreeWorkStage::Planting,
                        sapling_loaded: worker.inventory.count(ItemKind::Prop(species.sapling()))
                            > 0,
                        sapling_dropped: false,
                        started: false,
                    }));
                }
                Err(terra_core::trees::SaplingPlacementError::Unloaded) => search.unloaded = true,
                Err(terra_core::trees::SaplingPlacementError::Protected) => search.protected = true,
                Err(_) => search.occupied_or_spaced = true,
            }
        }
    }
    if search.cursor >= volume {
        search.cursor = 0;
        if search.unloaded {
            return Err(WaitReason::Unloaded);
        }
        if search.protected {
            return Err(WaitReason::Protected);
        }
        if !search.occupied_or_spaced {
            return Err(WaitReason::NoPlantingSite);
        }
    }
    Ok(None)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn step(
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
        let supplies = station::supply_order(chunks, journal, order, runtime, Some(item));
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
            actions::supply(journal, &supplies, worker, false, Some(item), 0)?;
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
        chunks, journal, terrain, order, npc, worker, root, dt, runtime, false,
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
