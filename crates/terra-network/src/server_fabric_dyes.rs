use super::*;

struct FabricPlan {
    position: [i64; 3],
    material: u16,
    expected: u16,
    tint: Option<[u8; 3]>,
    inventory: Option<terra_core::Inventory>,
}

fn plan(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<FabricPlan, EditOutcome> {
    let EditAction::DyeFabric { position, color } = command.action else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.permissions.build {
        return Err(EditOutcome::Invalid);
    }
    let (origin, direction, target) = super::conveyors::interaction_target(command, peer, world)?;
    if !matches!(target, Some(WorldTarget::Block(hit)) if hit.position == position && hit.distance <= 6.0)
    {
        return Err(EditOutcome::Occluded);
    }
    let section = global_to_section(position).0;
    if world.journal.revision(section) != command.revision {
        return Err(EditOutcome::Conflict);
    }
    let expected = world
        .chunks
        .block_at(position)
        .ok_or(EditOutcome::Invalid)?;
    let state = world.journal.object_state(position).unwrap_or_default();
    if !terra_core::tints::cell_dyeable(expected, state.facing)
        || expected == 148
        || terra_core::beds::is_bed(expected)
    {
        return Err(EditOutcome::Invalid);
    }
    let mixed = terra_core::construction::slab_halves(expected, state.facing);
    if let Some(halves) = mixed
        && halves.iter().any(|half| half.0 != 105)
        && terra_world::slabs::targeted_half(
            &world.chunks,
            &world.journal,
            position,
            (origin, direction),
        )
        .is_none_or(|half| half.removed != 105)
    {
        return Err(EditOutcome::Invalid);
    }
    let fabric = if mixed.is_some() { 105 } else { expected };
    let item = terra_core::tints::recolored(terra_core::ItemKind::Block(fabric), color)
        .ok_or(EditOutcome::Invalid)?;
    let material = if mixed.is_some() {
        expected
    } else {
        item.block_material().ok_or(EditOutcome::Invalid)?
    };
    let tint = terra_core::tints::color(item);
    if material == expected && world.journal.tint_at(position) == tint {
        return Err(EditOutcome::Conflict);
    }
    let dye = terra_core::ItemKind::Dye(color);
    let inventory = if let Some(source) = command.source {
        let bag = world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .ok_or(EditOutcome::Invalid)?;
        if bag.revision != source.revision
            || peer.held_slot != Some(source.slot)
            || bag
                .slot(usize::from(source.slot))
                .is_none_or(|stack| stack.item != dye || stack.count == 0)
        {
            return Err(EditOutcome::Conflict);
        }
        if peer.game_mode(&world.config) == terra_core::GameMode::Survival {
            if bag.revision == u64::MAX || world.journal.gameplay().revision == u64::MAX {
                return Err(EditOutcome::Invalid);
            }
            let mut next = bag.clone();
            if !next.take_from(usize::from(source.slot), dye) {
                return Err(EditOutcome::Invalid);
            }
            Some(next)
        } else {
            None
        }
    } else if peer.game_mode(&world.config) == terra_core::GameMode::Creative {
        None
    } else {
        return Err(EditOutcome::Invalid);
    };
    if world.journal.revision(section) > u64::MAX - 8
        || world.journal.appearance_epoch(section) > u64::MAX - 8
    {
        return Err(EditOutcome::Invalid);
    }
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let neighbor = section
                    .offset([x, y, z])
                    .map_err(|_| EditOutcome::Invalid)?;
                if world.journal.mesh_epoch(neighbor) > u64::MAX - 8
                    || world.journal.water_mesh_epoch(neighbor) > u64::MAX - 8
                {
                    return Err(EditOutcome::Invalid);
                }
            }
        }
    }
    Ok(FabricPlan {
        position,
        material,
        expected,
        tint,
        inventory,
    })
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    plan(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}
pub(super) fn apply(command: EditCommand, peer: &Peer, world: &mut ControlWorld) -> EditOutcome {
    let plan = match plan(command, peer, world) {
        Ok(plan) => plan,
        Err(outcome) => return outcome,
    };
    let result = (|| -> Result<(), String> {
        if plan.material != plan.expected {
            if !commit_block_edit(
                &mut world.chunks,
                &mut world.journal,
                &BlockEditRequest {
                    position: plan.position,
                    expected_material: plan.expected,
                    material: plan.material,
                },
                &world.block_properties,
            )
            .map_err(|error| error.to_string())?
            {
                return Err("Fabric changed during dye transaction".into());
            }
        } else if world.journal.block(plan.position).is_none() {
            world
                .journal
                .record(plan.position, plan.material)
                .map_err(|error| error.to_string())?;
        }
        world.journal.set_tint(plan.position, plan.tint)?;
        if let Some(inventory) = plan.inventory {
            let gameplay = world.journal.gameplay_mut();
            gameplay.inventories.insert(peer.uuid, inventory);
            gameplay.revision += 1;
        }
        Ok(())
    })();
    match result {
        Ok(()) => EditOutcome::Applied,
        Err(error) => {
            tracing::warn!(%error,"Fabric dye transaction failed");
            EditOutcome::Invalid
        }
    }
}
