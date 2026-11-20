use super::*;
use crate::streaming::{BlockEditRequest, commit_block_edit, commit_object_state};
use terra_core::block_properties::BlockPropertiesCatalog;

pub fn set_storage(
    journal: &mut EditJournal,
    id: KingdomId,
    owner: PlayerUuid,
    area: BuildArea,
) -> Result<(), BuildError> {
    if !area.valid() {
        return Err(BuildError::InvalidArea);
    }
    let registry = &mut journal.gameplay_mut().buildings;
    if registry.revision == u64::MAX {
        return Err(BuildError::Limit);
    }
    registry
        .settlements
        .entry(id)
        .or_insert_with(|| SettlementBuilding::new(owner))
        .storage = Some(area);
    registry.revision += 1;
    Ok(())
}
pub fn set_residential(
    journal: &mut EditJournal,
    id: KingdomId,
    owner: PlayerUuid,
    area: Option<BuildArea>,
) -> Result<(), BuildError> {
    if area.is_some_and(|a| !a.valid()) {
        return Err(BuildError::InvalidArea);
    }
    let registry = &mut journal.gameplay_mut().buildings;
    if registry.revision == u64::MAX {
        return Err(BuildError::Limit);
    }
    registry
        .settlements
        .entry(id)
        .or_insert_with(|| SettlementBuilding::new(owner))
        .residential = area;
    registry.revision += 1;
    Ok(())
}
pub fn assign_craftsman(
    journal: &mut EditJournal,
    id: KingdomId,
    owner: PlayerUuid,
    npc: u64,
    enabled: bool,
) -> Result<(), BuildError> {
    let game = journal.gameplay();
    if !game
        .creatures
        .animals
        .get(&npc)
        .is_some_and(|n| n.kind == CreatureKind::Villager && n.health > 0.0)
        || game.creatures.owners.get(&npc) != Some(&owner)
    {
        return Err(BuildError::MissingWorker);
    }
    if game
        .buildings
        .jobs
        .values()
        .any(|j| j.active() && j.workers.contains(&npc))
    {
        return Err(BuildError::Busy);
    }
    let registry = &mut journal.gameplay_mut().buildings;
    if registry.revision == u64::MAX {
        return Err(BuildError::Limit);
    }
    let site = registry
        .settlements
        .entry(id)
        .or_insert_with(|| SettlementBuilding::new(owner));
    if enabled {
        if !site.craftsmen.contains(&npc) {
            if site.craftsmen.len() >= MAX_CRAFTSMEN {
                return Err(BuildError::Limit);
            }
            site.craftsmen.push(npc);
        }
        registry.inventories.entry(npc).or_default();
    } else {
        if registry
            .inventories
            .get(&npc)
            .is_some_and(|b| b.occupied_slots() > 0)
        {
            return Err(BuildError::Busy);
        }
        site.craftsmen.retain(|n| *n != npc);
        registry.inventories.remove(&npc);
    }
    registry.revision += 1;
    Ok(())
}
pub fn queue_house(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    id: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
) -> Result<u64, BuildError> {
    let job = plan_house(chunks, journal, terrain, id, owner, origin, facing)?;
    journal.gameplay_mut().buildings.insert(job)
}

fn worker_valid(journal: &EditJournal, site: &SettlementBuilding, id: u64) -> bool {
    site.craftsmen.contains(&id)
        && journal.gameplay().creatures.owners.get(&id) == Some(&site.owner)
        && journal
            .gameplay()
            .creatures
            .animals
            .get(&id)
            .is_some_and(|n| n.kind == CreatureKind::Villager && n.health > 0.0)
        && !journal
            .gameplay()
            .jobs
            .orders
            .values()
            .any(|j| j.npc == id && j.enabled)
}
fn auto_jobs(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    runtime: &mut BuildRuntime,
) {
    let candidates: Vec<_> = journal
        .gameplay()
        .buildings
        .settlements
        .iter()
        .filter_map(|(id, s)| s.residential.map(|a| (*id, s.clone(), a)))
        .collect();
    let towns = candidates.len();
    for offset in 0..towns.min(8) {
        let (id, site, area) = candidates[(runtime.auto_town_cursor + offset) % towns].clone();
        if journal.gameplay().buildings.jobs.len() >= MAX_BUILD_JOBS
            || site.storage.is_none() && !runtime.creative_owners.contains(&site.owner)
            || !site.craftsmen.iter().any(|npc| {
                worker_valid(journal, &site, *npc)
                    && !runtime.suspended.contains(npc)
                    && !journal
                        .gameplay()
                        .buildings
                        .jobs
                        .values()
                        .any(|j| j.active() && j.workers.contains(npc))
            })
        {
            continue;
        }
        if journal
            .gameplay()
            .buildings
            .jobs
            .values()
            .any(|j| j.kingdom == id && j.active() && j.workers.is_empty())
        {
            continue;
        }
        let width = (area.max[0] - area.min[0] - 10).max(0) as usize;
        let depth = (area.max[2] - area.min[2] - 11).max(0) as usize;
        if width == 0 || depth == 0 {
            continue;
        }
        let nx = width.div_ceil(14);
        let nz = depth.div_ceil(14);
        let cursor = runtime.auto_cursor.entry(id).or_default();
        for _ in 0..2 {
            let index = *cursor % (nx * nz);
            *cursor = cursor.wrapping_add(1);
            let x = area.min[0] + 6 + (index % nx * 14) as i64;
            let z = area.min[2] + 7 + (index / nx * 14) as i64;
            if x + 5 > area.max[0] || z + 5 > area.max[2] {
                continue;
            }
            let floor = (area.min[1] - 3..=area.max[1] + 3).rev().find_map(|y| {
                let m = chunks.block_at([x, y, z])?;
                (m != 0 && chunks.block_at([x, y + 1, z]) == Some(0)).then_some(y + 1)
            });
            if let Some(y) = floor
                && let Ok(job) = plan_template(
                    chunks,
                    journal,
                    terrain,
                    id,
                    site.owner,
                    [x, y, z],
                    0,
                    &site.selected_template,
                )
            {
                let _ = journal.gameplay_mut().buildings.insert(job);
                break;
            }
        }
    }
    if towns > 0 {
        runtime.auto_town_cursor = (runtime.auto_town_cursor + 8) % towns;
    }
}

/// Cancelled projects retain their real blocks and bags. Unassigned craftsmen
/// physically return remaining stock, so cancelling cannot lock a profession forever.
fn return_unused(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    registry: &mut BuildingRegistry,
    dt: f32,
    runtime: &mut BuildRuntime,
) {
    let workers: Vec<_> = registry
        .settlements
        .iter()
        .flat_map(|(kingdom, site)| site.craftsmen.iter().map(move |id| (*kingdom, *id)))
        .filter(|(_, id)| {
            registry
                .inventories
                .get(id)
                .is_some_and(|bag| bag.occupied_slots() > 0)
                && !registry
                    .jobs
                    .values()
                    .any(|j| j.active() && j.workers.contains(id))
        })
        .collect();
    let count = workers.len();
    for offset in 0..count.min(8) {
        let (kingdom, id) = &workers[(runtime.return_cursor + offset) % count];
        let site = registry.settlements[kingdom].clone();
        if runtime.suspended.contains(id) || !worker_valid(journal, &site, *id) {
            continue;
        }
        let Some(mut npc) = journal.gameplay().creatures.animals.get(id).cloned() else {
            continue;
        };
        let (origin, facing) = registry
            .jobs
            .values()
            .filter(|j| j.kingdom == *kingdom)
            .min_by(|a, b| {
                crate::creatures::distance(npc.position, a.origin.map(|v| v as f64)).total_cmp(
                    &crate::creatures::distance(npc.position, b.origin.map(|v| v as f64)),
                )
            })
            .map(|j| (j.origin, j.facing))
            .unwrap_or((npc.position.map(|v| v.floor() as i64), 0));
        // Only a lightweight permission/route context, never inserted as a project.
        let context = BuildingJob {
            id: 1,
            kingdom: *kingdom,
            owner: site.owner,
            origin,
            facing,
            template: HOUSE_TEMPLATE.into(),
            free_resources: false,
            free_from_step: 0,
            workers: vec![*id],
            steps: Vec::new(),
            next_step: 0,
            work_seconds: 0.0,
            status: BuildStatus::Travelling,
            enabled: true,
            revision: 1,
        };
        let mut bag = registry.inventories.remove(id).unwrap_or_default();
        let target = site.storage.and_then(|area| {
            supplies::chests(chunks, journal, &context, area)
                .into_iter()
                .find(|p| {
                    journal
                        .container_at(*p)
                        .is_some_and(|chest| supplies::can_unload(chest, &bag))
                })
        });
        if let Some(chest) = target {
            if movement::approach(
                chunks,
                journal,
                terrain,
                &context,
                &mut npc,
                chest,
                &[],
                dt,
                runtime,
            ) {
                idle(&mut npc, &context, chunks, journal, terrain, dt);
                if crate::kingdom_access::can_transport_inventory(
                    chunks,
                    journal,
                    npc.position.map(|v| v.floor() as i64),
                    chest,
                ) {
                    supplies::unload(journal, &mut bag, chest);
                }
            }
        } else {
            idle(&mut npc, &context, chunks, journal, terrain, dt);
        }
        registry.inventories.insert(*id, bag);
        journal.gameplay_mut().creatures.animals.insert(*id, npc);
        registry.revision += 1;
    }
    if count > 0 {
        runtime.return_cursor = (runtime.return_cursor + 8) % count;
    }
}

/// Call at 10Hz. Every active project receives the full elapsed tick.
/// `MAX_BUILD_JOBS` bounds total work; each house has at most four craftsmen.
pub fn tick(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    properties: &BlockPropertiesCatalog,
    dt: f32,
    runtime: &mut BuildRuntime,
) {
    if !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
        || journal.gameplay().revision >= u64::MAX - 128
    {
        return;
    }
    auto_jobs(chunks, journal, terrain, runtime);
    let mut registry = std::mem::take(&mut journal.gameplay_mut().buildings);
    if !registry.valid() || registry.revision >= u64::MAX - 128 {
        journal.gameplay_mut().buildings = registry;
        return;
    }
    runtime.repairs.retain(|id, _| {
        registry
            .jobs
            .get(id)
            .is_some_and(|job| job.enabled && job.active())
    });
    let mut reserved: std::collections::BTreeSet<_> = registry
        .jobs
        .values()
        .filter(|j| j.active())
        .flat_map(|j| j.workers.iter().copied())
        .collect();
    let ids: Vec<_> = registry
        .jobs
        .iter()
        .filter(|(_, j)| j.enabled && j.active())
        .map(|(id, _)| *id)
        .collect();
    let count = ids.len();
    for offset in 0..count.min(MAX_BUILD_JOBS) {
        let id = ids[(runtime.cursor + offset) % count];
        let Some(mut job) = registry.jobs.remove(&id) else {
            continue;
        };
        if runtime.prepared.insert(job.id) {
            preparation::resume(&mut job);
        }
        if runtime.creative_owners.contains(&job.owner) && !job.free_resources {
            job.free_resources = true;
            job.free_from_step = job.next_step;
        }
        if job.free_resources && !runtime.creative_owners.contains(&job.owner) {
            if job.status != BuildStatus::Waiting(BuildWait::Protected) {
                registry.revision += 1;
            }
            job.status = BuildStatus::Waiting(BuildWait::Protected);
            for id in &job.workers {
                if !runtime.suspended.contains(id)
                    && let Some(mut npc) = journal.gameplay().creatures.animals.get(id).cloned()
                {
                    idle(&mut npc, &job, chunks, journal, terrain, dt);
                    journal.gameplay_mut().creatures.animals.insert(*id, npc);
                }
            }
            registry.jobs.insert(id, job);
            continue;
        }
        let Some(site) = registry.settlements.get(&job.kingdom).cloned() else {
            job.status = BuildStatus::Waiting(BuildWait::MissingStorage);
            registry.jobs.insert(id, job);
            continue;
        };
        job.workers.retain(|npc| worker_valid(journal, &site, *npc));
        if job.workers.len() < MAX_HOUSE_WORKERS {
            let mut available: Vec<_> = site
                .craftsmen
                .iter()
                .copied()
                .filter(|npc| {
                    !reserved.contains(npc)
                        && !runtime.suspended.contains(npc)
                        && worker_valid(journal, &site, *npc)
                })
                .collect();
            available.sort_by(|a, b| {
                let animals = &journal.gameplay().creatures.animals;
                crate::creatures::distance(animals[a].position, job.origin.map(|v| v as f64))
                    .total_cmp(&crate::creatures::distance(
                        animals[b].position,
                        job.origin.map(|v| v as f64),
                    ))
            });
            let open = MAX_HOUSE_WORKERS - job.workers.len();
            job.workers.extend(available.into_iter().take(open));
            for npc in &job.workers {
                reserved.insert(*npc);
            }
        }
        let ages: BTreeMap<_, _> = job
            .workers
            .iter()
            .filter(|id| !runtime.suspended.contains(*id))
            .filter_map(|id| {
                journal
                    .gameplay()
                    .creatures
                    .animals
                    .get(id)
                    .map(|a| (*id, a.age))
            })
            .collect();
        advance(
            chunks,
            journal,
            terrain,
            properties,
            &site,
            &mut job,
            &mut registry,
            dt,
            runtime,
        );
        for (id, age) in ages {
            if let Some(mut npc) = journal.gameplay().creatures.animals.get(&id).cloned()
                && npc.age == age
            {
                idle(&mut npc, &job, chunks, journal, terrain, dt);
                journal.gameplay_mut().creatures.animals.insert(id, npc);
            }
        }
        registry.revision += 1;
        registry.jobs.insert(id, job);
    }
    if count > 0 {
        runtime.cursor = (runtime.cursor + MAX_BUILD_JOBS) % count;
    }
    return_unused(chunks, journal, terrain, &mut registry, dt, runtime);
    journal.gameplay_mut().buildings = registry;
    journal.gameplay_mut().revision += 1;
}

fn idle(
    npc: &mut Creature,
    job: &BuildingJob,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    dt: f32,
) {
    npc.velocity = [0.0, npc.velocity[1].min(0.0), 0.0];
    npc.behavior = CreatureBehavior::Rest;
    crate::creatures::work_rest_with_access(
        npc,
        &Habitat {
            chunks,
            journal,
            terrain,
        },
        dt,
        |p| movement::permitted(chunks, journal, job, p),
    );
}

fn travel_status(npc: &Creature, runtime: &BuildRuntime) -> BuildStatus {
    if runtime
        .routes
        .get(&npc.id)
        .is_some_and(|route| route.stuck(npc.age))
    {
        BuildStatus::Waiting(BuildWait::NoPath)
    } else {
        BuildStatus::Travelling
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    properties: &BlockPropertiesCatalog,
    site: &SettlementBuilding,
    job: &mut BuildingJob,
    registry: &mut BuildingRegistry,
    dt: f32,
    runtime: &mut BuildRuntime,
) {
    if job.workers.is_empty() {
        job.status = BuildStatus::Waiting(BuildWait::NoWorkers);
        return;
    }
    let active: Vec<_> = job
        .workers
        .iter()
        .copied()
        .filter(|id| !runtime.suspended.contains(id))
        .collect();
    if active.is_empty() {
        return;
    }
    let chests = site.storage.map_or_else(Vec::new, |area| {
        supplies::chests(chunks, journal, job, area)
    });
    if chests.is_empty()
        && (!job.free_resources
            || job.workers.iter().any(|id| {
                registry
                    .inventory(*id)
                    .is_some_and(|bag| bag.occupied_slots() > 0)
            }))
    {
        job.status = BuildStatus::Waiting(BuildWait::MissingStorage);
        return;
    }
    // Completed scaffolding is recovered into actual bags and returned before release.
    if job.next_step == job.steps.len() {
        let mut remaining = false;
        let mut blocked = false;
        for id in job.workers.clone() {
            if runtime.suspended.contains(&id) {
                remaining |= registry
                    .inventory(id)
                    .is_some_and(|bag| bag.occupied_slots() > 0);
                continue;
            }
            let mut bag = registry.inventories.remove(&id).unwrap_or_default();
            if bag.occupied_slots() > 0
                && let Some(mut npc) = journal.gameplay().creatures.animals.get(&id).cloned()
            {
                let Some(chest) = chests.iter().copied().find(|p| {
                    journal
                        .container_at(*p)
                        .is_some_and(|c| supplies::can_unload(c, &bag))
                }) else {
                    remaining = true;
                    registry.inventories.insert(id, bag);
                    continue;
                };
                if movement::approach(
                    chunks,
                    journal,
                    terrain,
                    job,
                    &mut npc,
                    chest,
                    &[],
                    dt,
                    runtime,
                ) {
                    idle(&mut npc, job, chunks, journal, terrain, dt);
                    if crate::kingdom_access::can_transport_inventory(
                        chunks,
                        journal,
                        npc.position.map(|v| v.floor() as i64),
                        chest,
                    ) {
                        supplies::unload(journal, &mut bag, chest);
                    }
                } else {
                    blocked |=
                        travel_status(&npc, runtime) == BuildStatus::Waiting(BuildWait::NoPath);
                }
                remaining |= bag.occupied_slots() > 0;
                journal.gameplay_mut().creatures.animals.insert(id, npc);
            }
            registry.inventories.insert(id, bag);
        }
        job.status = if remaining {
            if blocked {
                BuildStatus::Waiting(BuildWait::NoPath)
            } else {
                BuildStatus::Travelling
            }
        } else {
            BuildStatus::Complete
        };
        return;
    }
    let repair_step = repair::step(chunks, journal, job);
    let repairing = repair_step.is_some();
    let step = repair_step.unwrap_or_else(|| job.steps[job.next_step].clone());
    if !repairing {
        runtime.repairs.remove(&job.id);
    }
    if step
        .cells
        .iter()
        .any(|c| !build_access(chunks, journal, job.kingdom, job.owner, c.position))
    {
        job.status = BuildStatus::Waiting(BuildWait::Protected);
        return;
    }
    if step
        .cells
        .iter()
        .any(|c| chunks.block_at(c.position).is_none())
    {
        job.status = BuildStatus::Waiting(BuildWait::Unloaded);
        return;
    }
    if !repairing && preparation::already_cleared(chunks, journal, terrain, job, &step) {
        // A fulfilled excavation earns no worker time and consumes no stock.
        // The next placement still goes through ordinary movement and checks.
        job.next_step += 1;
        job.status = BuildStatus::Travelling;
        return;
    }
    let leader = active
        .iter()
        .copied()
        .find(|id| {
            registry
                .inventories
                .get(id)
                .is_some_and(|b| supplies::has(b, &step))
        })
        .unwrap_or(active[0]);
    let mut bag = registry.inventories.remove(&leader).unwrap_or_default();
    let Some(mut npc) = journal.gameplay().creatures.animals.get(&leader).cloned() else {
        registry.inventories.insert(leader, bag);
        return;
    };
    let need_supply = !job.free_resources && !supplies::has(&bag, &step);
    let mut refund_probe = bag.clone();
    let need_unload = step.phase == BuildPhase::Cleanup
        && refunds_scaffold(job, &step)
        && !supplies::return_plank(&mut refund_probe);
    if need_supply || need_unload {
        let chest = chests
            .iter()
            .copied()
            .find(|p| {
                journal.container_at(*p).is_some_and(|i| {
                    (!need_supply && supplies::can_unload(i, &bag))
                        || step
                            .costs
                            .iter()
                            .any(|s| bag.count(s.item) < s.count && i.count(s.item) > 0)
                })
            })
            .unwrap_or(chests[0]);
        if movement::approach(
            chunks,
            journal,
            terrain,
            job,
            &mut npc,
            chest,
            &[],
            dt,
            runtime,
        ) {
            idle(&mut npc, job, chunks, journal, terrain, dt);
            if !crate::kingdom_access::can_transport_inventory(
                chunks,
                journal,
                npc.position.map(|v| v.floor() as i64),
                chest,
            ) {
                job.status = BuildStatus::Waiting(BuildWait::Protected);
            } else {
                // Preserve partially gathered current-step ingredients while visiting
                // another chest; return unrelated stock to free the nine-slot bag.
                if bag.slots.iter().filter(|s| s.valid()).count()
                    >= terra_core::npc_jobs::WORKER_SLOTS
                    || need_unload
                {
                    supplies::unload(journal, &mut bag, chest);
                }
                if need_supply {
                    supplies::load(journal, job, &mut bag, chest);
                }
                let ready = if need_unload {
                    bag.slots.iter().filter(|s| s.valid()).count()
                        < terra_core::npc_jobs::WORKER_SLOTS
                } else {
                    supplies::has(&bag, &step)
                };
                job.status = if ready {
                    BuildStatus::Travelling
                } else {
                    BuildStatus::Waiting(if need_unload {
                        BuildWait::StorageFull
                    } else {
                        BuildWait::MissingMaterials
                    })
                };
            }
        } else {
            job.status = travel_status(&npc, runtime);
        }
        registry.inventories.insert(leader, bag);
        journal.gameplay_mut().creatures.animals.insert(leader, npc);
        return;
    }
    let target = step.cells[0].position;
    if !movement::approach(
        chunks,
        journal,
        terrain,
        job,
        &mut npc,
        target,
        &step.cells,
        dt,
        runtime,
    ) {
        job.status = travel_status(&npc, runtime);
        if job.status == BuildStatus::Waiting(BuildWait::NoPath)
            && preparation::advance_frontier(
                job,
                &npc,
                &Habitat {
                    chunks,
                    journal,
                    terrain,
                },
            )
        {
            runtime.routes.remove(&leader);
            runtime.navigation.invalidate(leader);
            job.status = BuildStatus::Travelling;
        }
        registry.inventories.insert(leader, bag);
        journal.gameplay_mut().creatures.animals.insert(leader, npc);
        return;
    }
    let mut onsite = 1;
    for id in active.iter().copied().filter(|id| *id != leader) {
        if let Some(mut assistant) = journal.gameplay().creatures.animals.get(&id).cloned() {
            if movement::approach(
                chunks,
                journal,
                terrain,
                job,
                &mut assistant,
                target,
                &step.cells,
                dt,
                runtime,
            ) {
                onsite += 1;
                idle(&mut assistant, job, chunks, journal, terrain, dt);
                assistant.behavior = CreatureBehavior::Graze;
            }
            journal
                .gameplay_mut()
                .creatures
                .animals
                .insert(id, assistant);
        }
    }
    idle(&mut npc, job, chunks, journal, terrain, dt);
    npc.behavior = CreatureBehavior::Graze;
    // A blocked target never earns work time or spends an ingredient. Recheck
    // at commit too, because the entire paired placement must still be valid.
    if let Err(reason) = preflight(chunks, journal, job, &step, &npc, &bag, runtime) {
        job.status = BuildStatus::Waiting(reason);
        registry.inventories.insert(leader, bag);
        journal.gameplay_mut().creatures.animals.insert(leader, npc);
        return;
    }
    job.status = BuildStatus::Building(step.phase);
    if repairing {
        // Re-clearing does not replay a completed paid step or move any saved
        // progress index. It still costs a real step's onsite working time.
        let due = 1800.0 / job.steps.len() as f64;
        let ready = {
            let progress = runtime.repairs.entry(job.id).or_insert(repair::Progress {
                position: target,
                seconds: 0.0,
            });
            if progress.position != target {
                *progress = repair::Progress {
                    position: target,
                    seconds: 0.0,
                };
            }
            progress.seconds = (progress.seconds
                + f64::from(dt) * 1800.0 / house_seconds(onsite).unwrap_or(1800.0))
            .min(due);
            progress.seconds + 1e-7 >= due
        };
        if ready {
            match commit(
                chunks, journal, properties, job, &step, &npc, &mut bag, runtime,
            ) {
                Ok(()) => {
                    runtime.repairs.remove(&job.id);
                }
                Err(reason) => job.status = BuildStatus::Waiting(reason),
            }
        }
        registry.inventories.insert(leader, bag);
        journal.gameplay_mut().creatures.animals.insert(leader, npc);
        return;
    }
    let due = (job.next_step + 1) as f64 / job.steps.len() as f64 * 1800.0;
    job.work_seconds = (job.work_seconds
        + f64::from(dt) * 1800.0 / house_seconds(onsite).unwrap_or(1800.0))
    .min(1800.0);
    if job.work_seconds + 1e-7 >= due {
        match commit(
            chunks, journal, properties, job, &step, &npc, &mut bag, runtime,
        ) {
            Ok(()) => job.next_step += 1,
            Err(reason) => {
                job.status = BuildStatus::Waiting(reason);
                job.work_seconds = job
                    .work_seconds
                    .min(job.next_step as f64 / job.steps.len() as f64 * 1800.0);
            }
        }
    }
    registry.inventories.insert(leader, bag);
    journal.gameplay_mut().creatures.animals.insert(leader, npc);
}

fn refunds_scaffold(job: &BuildingJob, cleanup: &BuildStep) -> bool {
    if !job.free_resources {
        return true;
    }
    let position = cleanup.cells[0].position;
    job.steps[..job.next_step]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, s)| {
            s.cells
                .iter()
                .any(|c| c.position == position && c.material == 16)
        })
        .is_some_and(|(index, _)| index < job.free_from_step)
}

fn preflight(
    chunks: &ChunkStore,
    journal: &EditJournal,
    job: &BuildingJob,
    step: &BuildStep,
    npc: &Creature,
    bag: &Inventory,
    runtime: &BuildRuntime,
) -> Result<Inventory, BuildWait> {
    let mut next = bag.clone();
    if !job.free_resources && !supplies::consume(&mut next, step) {
        return Err(BuildWait::MissingMaterials);
    }
    for c in &step.cells {
        let old = chunks.block_at(c.position).ok_or(BuildWait::Unloaded)?;
        if !build_access(chunks, journal, job.kingdom, job.owner, c.position) {
            return Err(BuildWait::Protected);
        }
        if step.phase == BuildPhase::Cleanup {
            if old != 16 || journal.block(c.position) != Some(16) {
                return Err(BuildWait::Blocked);
            }
            if refunds_scaffold(job, step) && !supplies::return_plank(&mut next) {
                return Err(BuildWait::MissingStorage);
            }
        } else if c.material != 0
            && old != 0
            && !(step.phase == BuildPhase::Foundation
                && planning::preparable_ground(
                    old,
                    journal.block(c.position),
                    c.position[1],
                    job.origin[1],
                ))
        {
            return Err(BuildWait::Blocked);
        }
        if step.phase != BuildPhase::Cleanup
            && (journal.container_at(c.position).is_some()
                || journal.object_state(c.position).is_some()
                || journal
                    .fluid(c.position)
                    .is_some_and(|fluid| fluid.amount > 0))
        {
            return Err(BuildWait::Blocked);
        }
        if c.material == 0
            && step.phase != BuildPhase::Cleanup
            && !planning::preparable_ground(
                old,
                journal.block(c.position),
                c.position[1],
                job.origin[1],
            )
            && !repair::allowed(chunks, journal, job, c.position)
        {
            return Err(BuildWait::Blocked);
        }
        if old != 0
            && !crate::kingdom_access::can_use(
                &journal.gameplay().kingdoms,
                chunks,
                job.owner,
                KingdomPermission::Break,
                c.position,
            )
        {
            return Err(BuildWait::Protected);
        }
        if journal
            .gameplay()
            .creatures
            .animals
            .values()
            .filter(|a| a.id != npc.id && a.health > 0.0)
            .any(|a| collides(c, a.position, a.size()))
            || runtime
                .player_bodies
                .iter()
                .any(|p| collides(c, *p, [0.6, 1.8, 0.6]))
        {
            return Err(BuildWait::Blocked);
        }
        // No block is placed through the worker's real body or removed underneath it.
        let p = c.position.map(|v| v as f64);
        if (npc.position[0] - p[0] - 0.5).abs() < 0.8
            && (npc.position[2] - p[2] - 0.5).abs() < 0.8
            && ((c.material != 0
                && p[1] + 1.0 > npc.position[1] + 0.001
                && p[1] < npc.position[1] + 1.8)
                || (c.material == 0 && (p[1] + 1.0 - npc.position[1]).abs() < 0.08))
        {
            return Err(BuildWait::Blocked);
        }
        let section = terra_voxel::global_to_section(c.position).0;
        if journal.revision(section) > u64::MAX - 64
            || journal.fluid_epoch(section) > u64::MAX - 64
            || journal.appearance_epoch(section) > u64::MAX - 64
            || chunks
                .loaded
                .get(&section)
                .is_none_or(|s| s.data.revision() > u64::MAX - 64)
        {
            return Err(BuildWait::Blocked);
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbor = section
                        .offset([dx, dy, dz])
                        .map_err(|_| BuildWait::Blocked)?;
                    if journal.mesh_epoch(neighbor) > u64::MAX - 64
                        || journal.water_mesh_epoch(neighbor) > u64::MAX - 64
                    {
                        return Err(BuildWait::Blocked);
                    }
                }
            }
        }
        let state = crate::edits::BlockObjectState {
            facing: c.facing,
            peer: c.peer,
            secondary: c.secondary,
            wood: ItemKind::Block(c.material)
                .wood_species()
                .unwrap_or_default(),
            ..Default::default()
        };
        if c.material != 0
            && (c.peer.is_some() || c.facing != 0)
            && !state.valid_for(c.position, c.material)
        {
            return Err(BuildWait::Blocked);
        }
    }
    Ok(next)
}

#[allow(clippy::too_many_arguments)]
fn commit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    job: &BuildingJob,
    step: &BuildStep,
    npc: &Creature,
    bag: &mut Inventory,
    runtime: &BuildRuntime,
) -> Result<(), BuildWait> {
    let next = preflight(chunks, journal, job, step, npc, bag, runtime)?;
    for c in &step.cells {
        let expected = chunks.block_at(c.position).ok_or(BuildWait::Unloaded)?;
        let applied = commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: c.position,
                expected_material: expected,
                material: c.material,
            },
            properties,
        )
        .map_err(|_| BuildWait::Blocked)?;
        if !applied {
            return Err(BuildWait::Blocked);
        }
    }
    for c in &step.cells {
        let state = crate::edits::BlockObjectState {
            facing: c.facing,
            peer: c.peer,
            secondary: c.secondary,
            wood: ItemKind::Block(c.material)
                .wood_species()
                .unwrap_or_default(),
            ..Default::default()
        };
        if c.material != 0 && state.valid_for(c.position, c.material) {
            commit_object_state(chunks, journal, c.position, Some(state))
                .map_err(|_| BuildWait::Blocked)?;
        }
    }
    for c in &step.cells {
        if terra_core::containers::is_chest(c.material)
            && journal.container_at(c.position).is_none()
        {
            journal
                .set_container(c.position, Inventory::default())
                .map_err(|_| BuildWait::Blocked)?;
        }
    }
    *bag = next;
    Ok(())
}

fn collides(cell: &BuildCell, position: [f64; 3], size: [f64; 3]) -> bool {
    let p = cell.position.map(|v| v as f64);
    let horizontal = (position[0] - p[0] - 0.5).abs() < size[0] * 0.5 + 0.5 - 0.001
        && (position[2] - p[2] - 0.5).abs() < size[2] * 0.5 + 0.5 - 0.001;
    horizontal
        && if cell.material == 0 {
            (p[1] + 1.0 - position[1]).abs() < 0.08
        } else {
            p[1] + 1.0 > position[1] + 0.001 && p[1] < position[1] + size[1]
        }
}
