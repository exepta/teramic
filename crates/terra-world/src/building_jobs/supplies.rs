use super::*;
use terra_core::npc_jobs::WorkerState;

pub(super) fn chests(
    chunks: &ChunkStore,
    journal: &EditJournal,
    job: &BuildingJob,
    area: BuildArea,
) -> Vec<[i64; 3]> {
    let mut cells: Vec<_> = journal
        .containers_in_area(area.min, area.max)
        .take(MAX_TOWN_CHESTS)
        .filter(|(p, _)| {
            area.contains(*p)
                && chunks
                    .block_at(*p)
                    .is_some_and(terra_core::containers::is_chest)
                && build_access(chunks, journal, job.kingdom, job.owner, *p)
                && journal
                    .object_state(*p)
                    .and_then(|s| s.peer)
                    .is_none_or(|other| {
                        area.contains(other)
                            && chunks.block_at(other) == Some(29)
                            && build_access(chunks, journal, job.kingdom, job.owner, other)
                            && crate::kingdom_access::can_use(
                                &journal.gameplay().kingdoms,
                                chunks,
                                job.owner,
                                KingdomPermission::Inventories,
                                other,
                            )
                    })
                && crate::kingdom_access::can_transport_inventory(chunks, journal, job.origin, *p)
                && crate::kingdom_access::can_use(
                    &journal.gameplay().kingdoms,
                    chunks,
                    job.owner,
                    KingdomPermission::Inventories,
                    *p,
                )
        })
        .map(|(p, _)| p)
        .collect();
    cells.sort_unstable();
    cells.truncate(MAX_TOWN_CHESTS);
    cells
}
pub(super) fn has(bag: &Inventory, step: &BuildStep) -> bool {
    step.costs.iter().all(|s| bag.count(s.item) >= s.count)
}
pub(super) fn consume(bag: &mut Inventory, step: &BuildStep) -> bool {
    let mut next = bag.clone();
    for stack in &step.costs {
        for _ in 0..stack.count {
            if !next.take_one(stack.item) {
                return false;
            }
        }
    }
    *bag = next;
    true
}
pub(super) fn load(
    journal: &mut EditJournal,
    job: &BuildingJob,
    bag: &mut Inventory,
    chest: [i64; 3],
) -> bool {
    let Some(mut source) = journal.container_at(chest).cloned() else {
        return false;
    };
    if source.revision > u64::MAX - 256
        || bag.revision > u64::MAX - 256
        || journal.gameplay().revision == u64::MAX
    {
        return false;
    }
    let mut worker = WorkerState {
        inventory: bag.clone(),
        ..Default::default()
    };
    let mut needed = BTreeMap::<ItemKind, u32>::new();
    for step in job.steps[job.next_step..].iter().take(256) {
        for stack in &step.costs {
            *needed.entry(stack.item).or_default() += u32::from(stack.count);
        }
    }
    let mut moved = false;
    let mut priority = Vec::new();
    for stack in &job.steps[job.next_step].costs {
        if let Some(count) = needed.remove(&stack.item) {
            priority.push((stack.item, count));
        }
    }
    priority.extend(needed);
    for (item, count) in priority {
        let wanted = (count.min(128) as u16).saturating_sub(worker.inventory.count(item));
        let amount = wanted.min(source.count(item));
        for _ in 0..amount {
            let mut candidate = worker.clone();
            if candidate.insert(item, 1) != 0 || !source.take_one(item) {
                break;
            }
            worker = candidate;
            moved = true;
        }
    }
    if moved && journal.set_container(chest, source).is_ok() {
        *bag = worker.inventory;
        return true;
    }
    false
}
pub(super) fn return_plank(bag: &mut Inventory) -> bool {
    let mut worker = WorkerState {
        inventory: bag.clone(),
        ..Default::default()
    };
    if worker.insert(ItemKind::Block(16), 1) != 0 {
        return false;
    }
    if !worker.valid() {
        return false;
    }
    *bag = worker.inventory;
    // The material is recovered only after the matching scaffold voxel commits.
    true
}
pub(super) fn unload(journal: &mut EditJournal, bag: &mut Inventory, chest: [i64; 3]) -> bool {
    let Some(mut target) = journal.container_at(chest).cloned() else {
        return false;
    };
    let mut next = bag.clone();
    let mut moved = false;
    for stack in &mut next.slots {
        if !stack.valid() {
            continue;
        }
        let mut left = stack.count;
        for slot in 0..terra_core::containers::CHEST_SLOTS {
            if target.slot(slot).is_some_and(|s| s.item != stack.item) {
                continue;
            }
            let old = target.slot(slot).map_or(0, |s| s.count);
            let n = left.min(stack.item.max_stack() - old);
            if n > 0 {
                target.slots.resize(
                    target.slots.len().max(slot + 1),
                    terra_core::ItemStack::EMPTY,
                );
                target.slots[slot] = terra_core::ItemStack {
                    item: stack.item,
                    count: old + n,
                };
                left -= n;
                moved = true;
            }
            if left == 0 {
                break;
            }
        }
        stack.count = left;
        if left == 0 {
            *stack = terra_core::ItemStack::EMPTY;
        }
    }
    if moved {
        let (Some(br), Some(tr)) = (next.revision.checked_add(1), target.revision.checked_add(1))
        else {
            return false;
        };
        next.revision = br;
        target.revision = tr;
        if journal.set_container(chest, target).is_err() {
            return false;
        }
        *bag = next;
    }
    moved
}

pub(super) fn can_unload(chest: &Inventory, bag: &Inventory) -> bool {
    bag.slots.iter().filter(|s| s.valid()).any(|s| {
        (0..terra_core::containers::CHEST_SLOTS).any(|slot| {
            chest
                .slot(slot)
                .is_none_or(|c| c.item == s.item && c.count < c.item.max_stack())
        })
    })
}
