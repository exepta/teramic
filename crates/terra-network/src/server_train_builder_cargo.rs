//! Draft excavation cargo first; publication remains part of the earthworks transaction.
use std::collections::BTreeMap;
use terra_core::{
    Inventory, ItemStack,
    trains::{TrainError, TrainRegistry, Vehicle, VehicleKind},
};

pub(super) type CargoDrafts = Vec<(u64, BTreeMap<u16, Inventory>)>;
type Drops = Vec<([f64; 3], ItemStack)>;

pub(super) fn take(
    registry: &TrainRegistry,
    drafts: &mut CargoDrafts,
    item: terra_core::ItemKind,
    mut count: u16,
) -> Result<(), TrainError> {
    for (id, stores) in drafts {
        let wagon = registry.vehicles.get(id).ok_or(TrainError::Missing)?;
        for (part, inventory) in stores {
            if !wagon
                .parts
                .get(usize::from(*part))
                .is_some_and(|part| matches!(part.material, 29 | terra_core::train_parts::CARGO))
            {
                continue;
            }
            if !terra_core::containers::valid_chest(inventory) {
                return Err(TrainError::Invalid);
            }
            let mut changed = false;
            for stack in inventory
                .slots
                .iter_mut()
                .filter(|stack| stack.item == item && stack.valid())
            {
                if count == 0 {
                    break;
                }
                if inventory.revision == u64::MAX {
                    return Err(TrainError::Overflow);
                }
                let taken = count.min(stack.count);
                count -= taken;
                stack.count -= taken;
                if stack.count == 0 {
                    *stack = ItemStack::EMPTY;
                }
                changed = true;
            }
            if changed {
                inventory.revision += 1;
            }
            if count == 0 {
                return Ok(());
            }
        }
    }
    if count == 0 {
        Ok(())
    } else {
        Err(TrainError::MissingMaterials)
    }
}

pub(super) fn stow(
    car: &Vehicle,
    registry: &TrainRegistry,
    drafts: &mut CargoDrafts,
    drops: Drops,
) -> Result<Drops, TrainError> {
    if !car.motion.rail_builder_autoload {
        return Ok(drops);
    }
    let group = registry.consist(car.id)?;
    let mut overflow = Vec::new();
    for (position, mut stack) in drops {
        if !stack.valid()
            || position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return Err(TrainError::Invalid);
        }
        for (id, stores) in &mut *drafts {
            let Some(wagon) = registry.vehicles.get(id).filter(|wagon| {
                wagon.id != car.id
                    && wagon.kind == VehicleKind::Freight
                    && wagon.owner == car.owner
                    && group.contains(id)
            }) else {
                continue;
            };
            for (part, inventory) in stores {
                if wagon.parts.get(usize::from(*part)).is_some_and(|part| {
                    matches!(part.material, 29 | terra_core::train_parts::CARGO)
                }) {
                    stack.count = insert(inventory, stack);
                }
                if stack.count == 0 {
                    break;
                }
            }
            if stack.count == 0 {
                break;
            }
        }
        if stack.count > 0 {
            overflow.push((position, stack));
        }
    }
    Ok(overflow)
}

fn insert(inventory: &mut Inventory, stack: ItemStack) -> u16 {
    if inventory.revision == u64::MAX || !terra_core::containers::valid_chest(inventory) {
        return stack.count;
    }
    let mut remaining = stack.count;
    for present in inventory
        .slots
        .iter_mut()
        .filter(|s| s.item == stack.item && s.valid())
    {
        let accepted = remaining.min(stack.item.max_stack() - present.count);
        present.count += accepted;
        remaining -= accepted;
        if remaining == 0 {
            break;
        }
    }
    if remaining > 0 {
        inventory
            .slots
            .resize(terra_core::containers::CHEST_SLOTS, ItemStack::EMPTY);
        for present in inventory
            .slots
            .iter_mut()
            .filter(|s| **s == ItemStack::EMPTY)
        {
            let accepted = remaining.min(stack.item.max_stack());
            *present = ItemStack {
                item: stack.item,
                count: accepted,
            };
            remaining -= accepted;
            if remaining == 0 {
                break;
            }
        }
    }
    if remaining != stack.count {
        inventory.revision += 1;
    }
    remaining
}
