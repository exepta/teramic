use super::*;
use crate::streaming::{BlockEditRequest, commit_block_edit};
use terra_core::{Inventory, ItemStack, ToolKind};

fn retained(
    order: &WorkOrder,
    stack: ItemStack,
    kept_seed: &mut BTreeSet<terra_core::CropItem>,
    tree: Option<&terra_core::npc_jobs::TreeWork>,
) -> u16 {
    if stack
        .item
        .tool_kind()
        .is_some_and(|tool| order.kind.accepts_tool(tool))
    {
        return stack.count;
    }
    if order.kind == WorkKind::Lumberjack
        && tree.is_some_and(|tree| {
            tree.sapling_loaded && stack.item == ItemKind::Prop(tree.species.sapling())
        })
    {
        return 1;
    }
    match stack.item {
        ItemKind::Tool(tool) if order.kind.accepts_tool(tool) => stack.count,
        ItemKind::Crop(seed)
            if order.kind == WorkKind::Farmer
                && seed.crop_kind().is_some()
                && kept_seed.insert(seed) =>
        {
            1
        }
        _ => 0,
    }
}

pub(super) fn has_delivery(order: &WorkOrder, worker: &WorkerState) -> bool {
    let mut seeds = BTreeSet::new();
    worker
        .inventory
        .slots
        .iter()
        .copied()
        .filter(|s| s.valid())
        .any(|stack| retained(order, stack, &mut seeds, worker.tree.as_ref()) < stack.count)
}

fn chest_insert(bag: &mut Inventory, item: ItemKind, count: u16) -> u16 {
    if !terra_core::containers::valid_chest(bag) || bag.revision == u64::MAX {
        return count;
    }
    let mut remaining = count;
    for slot in 0..terra_core::containers::CHEST_SLOTS {
        let existing = bag.slot(slot);
        if existing.is_some_and(|stack| stack.item != item) {
            continue;
        }
        let present = existing.map_or(0, |stack| stack.count);
        let moved = remaining.min(item.max_stack() - present);
        if moved > 0 {
            bag.slots
                .resize(bag.slots.len().max(slot + 1), ItemStack::EMPTY);
            bag.slots[slot] = ItemStack {
                item,
                count: present + moved,
            };
            remaining -= moved;
        }
        if remaining == 0 {
            break;
        }
    }
    if remaining < count {
        bag.revision += 1;
    }
    remaining
}

pub(super) fn deliver(
    journal: &mut EditJournal,
    order: &WorkOrder,
    worker: &mut WorkerState,
) -> Result<bool, String> {
    let mut chest = journal
        .container_at(order.storage)
        .cloned()
        .unwrap_or_default();
    if worker.inventory.revision == u64::MAX {
        return Ok(false);
    }
    let mut next = worker.clone();
    let mut kept = BTreeSet::new();
    let mut moved = false;
    for stack in &mut next.inventory.slots {
        if !stack.valid() {
            continue;
        }
        let preserve = retained(order, *stack, &mut kept, worker.tree.as_ref());
        let amount = stack.count - preserve;
        let remaining = chest_insert(&mut chest, stack.item, amount);
        if remaining < amount {
            moved = true;
            stack.count = preserve + remaining;
            if stack.count == 0 {
                *stack = ItemStack::EMPTY;
            }
        }
    }
    if moved {
        journal.set_container(order.storage, chest)?;
        next.inventory.revision += 1;
        *worker = next;
    }
    Ok(!has_delivery(order, worker))
}

pub(super) fn supply(
    journal: &mut EditJournal,
    order: &WorkOrder,
    worker: &mut WorkerState,
    need_tool: bool,
    seed: Option<ItemKind>,
    minimum_level: u8,
) -> Result<bool, WaitReason> {
    let mut chest = journal
        .container_at(order.storage)
        .cloned()
        .unwrap_or_default();
    if chest.revision >= u64::MAX - 2 {
        return Ok(false);
    }
    let mut next = worker.clone();
    if need_tool {
        let item = chest
            .slots
            .iter()
            .filter_map(|stack| match stack.item.tool_kind() {
                Some(tool)
                    if stack.valid()
                        && order.kind.accepts_tool(tool)
                        && tool.mining_level() >= minimum_level =>
                {
                    Some((stack.item, tool))
                }
                _ => None,
            })
            .max_by_key(|(_, tool)| (tool.mining_level(), tool.farm_tier()))
            .map(|(item, _)| item)
            .ok_or(WaitReason::MissingTool)?;
        if next.insert(item, 1) != 0 || !chest.take_one(item) {
            return Ok(false);
        }
    }
    if let Some(seed) = seed {
        if chest.count(seed) == 0 {
            return Err(WaitReason::MissingMaterial);
        }
        if next.insert(seed, 1) != 0 || !chest.take_one(seed) {
            return Err(WaitReason::StorageFull);
        }
    }
    journal
        .set_container(order.storage, chest)
        .map_err(|_| WaitReason::Protected)?;
    *worker = next;
    Ok(true)
}

pub(super) fn mineable(
    chunks: &ChunkStore,
    journal: &EditJournal,
    cell: [i64; 3],
    material: u16,
) -> bool {
    matches!(material, 1..=5 | 11..=13 | 19 | 30..=35 | 37..=39 | 42 | 48 | 51 | 57..=58)
        && journal.block(cell).is_none()
        && journal.object_state(cell).is_none()
        && journal.fluid(cell).is_none_or(|fluid| fluid.amount == 0)
        && chunks
            .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
            .is_none_or(|column| !column.is_structure_block(cell))
        && !chunks.village_settlements().any(|site| {
            (0..3).all(|axis| (site.bounds[0][axis]..=site.bounds[1][axis]).contains(&cell[axis]))
        })
        && chunks
            .loaded
            .get(&terra_voxel::global_to_section(cell).0)
            .is_some_and(|section| {
                !section.props.iter().any(|prop| {
                    !prop.removed
                        && !journal.prop_removed(prop.section, prop.id)
                        && crate::props::placement::support(prop) == cell
                })
            })
}

pub(super) fn mine(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    npc: &Creature,
    worker: &mut WorkerState,
    cell: [i64; 3],
    tool: Option<ToolKind>,
) -> Result<bool, WaitReason> {
    let material = chunks.block_at(cell).ok_or(WaitReason::Unloaded)?;
    if !mineable(chunks, journal, cell, material) {
        worker.target = None;
        worker.work_seconds = 0.0;
        return Ok(false);
    }
    // Never undermine the worker's own support or mine through structural profiles.
    if cell[1] as f64 + 1.0 <= npc.position[1] + 0.01 {
        return Err(WaitReason::NoPath);
    }
    let mechanics = properties.0.get(&material).copied().unwrap_or_default();
    let tool = tool.ok_or(WaitReason::MissingTool)?;
    if mechanics.struct_block
        || !mechanics.can_harvest(Some(ItemKind::Tool(tool)))
        || terra_core::OreKind::from_material(material)
            .is_some_and(|ore| tool.mining_level() < ore.mining_level())
    {
        return Err(WaitReason::MissingTool);
    }
    if worker.work_seconds < (mechanics.hardness / tool.speed(material)).max(0.5) {
        return Ok(false);
    }
    let item = if material == 1 {
        ItemKind::Block(2)
    } else if material == 51 {
        ItemKind::Mineral(terra_core::OreKind::Salt.resource())
    } else {
        terra_core::OreKind::from_material(material).map_or(ItemKind::Block(material), |ore| {
            ItemKind::Mineral(ore.resource())
        })
    };
    let item = journal
        .tint_at(cell)
        .and_then(|rgb| terra_core::tints::recolored(item, terra_core::beds::DyeColor::Rgb(rgb)))
        .unwrap_or(item);
    let mut next = worker.clone();
    if next.insert(item, 1) != 0 {
        return Err(WaitReason::StorageFull);
    }
    if commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position: cell,
            expected_material: material,
            material: 0,
        },
        properties,
    )
    .map_err(|_| WaitReason::Protected)?
    {
        *worker = next;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(super) fn farm(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    worker: &mut WorkerState,
    cell: [i64; 3],
) -> Result<bool, WaitReason> {
    let crop = journal
        .crop_at(cell)
        .copied()
        .ok_or(WaitReason::NoTargets)?;
    if !crop.mature() {
        worker.target = None;
        worker.work_seconds = 0.0;
        return Ok(false);
    }
    if worker.work_seconds < 1.0 {
        return Ok(false);
    }
    let mut next = worker.clone();
    if next.inventory.revision == u64::MAX
        || !next.inventory.take_one(ItemKind::Crop(crop.kind.seed()))
    {
        return Err(WaitReason::MissingMaterial);
    }
    for stack in crop.harvest() {
        if next.insert(stack.item, stack.count) != 0 {
            return Err(WaitReason::StorageFull);
        }
    }
    crate::farming::commit_crop_state(
        chunks,
        journal,
        cell,
        terra_core::CropState::new(crop.kind, crop.prop_id),
    )
    .map_err(|_| WaitReason::Protected)?;
    *worker = next;
    Ok(true)
}

pub(super) fn collect(
    journal: &mut EditJournal,
    worker: &mut WorkerState,
    target: WorkTarget,
) -> Result<bool, WaitReason> {
    let WorkTarget::Drop(id) = target else {
        return Err(WaitReason::NoTargets);
    };
    let index = journal
        .gameplay()
        .drops
        .iter()
        .position(|drop| drop.id == id)
        .ok_or(WaitReason::NoTargets)?;
    let drop = &journal.gameplay().drops[index];
    if drop.age_seconds < drop.pickup_delay_seconds {
        return Err(WaitReason::NoTargets);
    }
    let mut next = worker.clone();
    let remaining = next.insert(drop.stack.item, drop.stack.count);
    if remaining == drop.stack.count {
        return Err(WaitReason::StorageFull);
    }
    if remaining == 0 {
        journal.gameplay_mut().drops.remove(index);
    } else {
        journal.gameplay_mut().drops[index].stack.count = remaining;
    }
    *worker = next;
    Ok(true)
}
