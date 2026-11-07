use super::*;

fn plan(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<
    (
        terra_world::snow::SnowPlacement,
        Option<terra_core::Inventory>,
    ),
    EditOutcome,
> {
    let EditAction::Block {
        position,
        expected,
        material: 98,
    } = command.action
    else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.permissions.build
        || position.iter().any(|p| p.unsigned_abs() > 990_000)
        || world.config.world.bounded_world && !(-128..=356).contains(&position[1])
    {
        return Err(EditOutcome::Invalid);
    }
    let (_, _, target) = super::conveyors::interaction_target(command, peer, world)?;
    let Some(WorldTarget::Block(hit)) = target else {
        return Err(EditOutcome::Occluded);
    };
    if hit.distance > 6.0
        || terra_world::snow::target(&world.chunks, &world.journal, hit)
            != Some((position, expected))
    {
        return Err(EditOutcome::Occluded);
    }
    if world.journal.revision(global_to_section(position).0) != command.revision
        || world.chunks.block_at(position) != Some(expected)
    {
        return Err(EditOutcome::Conflict);
    }
    let plan = terra_world::snow::plan(&world.chunks, &world.journal, position, expected)
        .ok_or(EditOutcome::Conflict)?;
    if let Some(bounds) = terra_world::snow::bounds(position, plan) {
        if let terra_world::snow::SnowPlacement::Layer { material, facing } = plan {
            let layout = terra_world::placement::Layout {
                edits: vec![BlockEditRequest {
                    position,
                    expected_material: expected,
                    material,
                }],
                states: vec![(
                    position,
                    facing.map(|facing| terra_world::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )],
                belts: Vec::new(),
            };
            terra_world::placement::environment(
                &layout,
                &world.chunks,
                &world.journal,
                world.config.world.bounded_world,
            )
            .map_err(|_| EditOutcome::Invalid)?;
        }
        let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
        if world.bodies.iter().any(|body| {
            bounds.intersects(
                (body.translation - half).to_array().map(f64::from),
                (body.translation + half).to_array().map(f64::from),
            )
        }) {
            return Err(EditOutcome::Invalid);
        }
    }
    let item = terra_core::ItemKind::Block(98);
    let inventory = if let Some(source) = command.source {
        let bag = world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .ok_or(EditOutcome::Invalid)?;
        if bag.revision != source.revision {
            return Err(EditOutcome::Conflict);
        }
        if peer.held_slot != Some(source.slot)
            || bag
                .slot(usize::from(source.slot))
                .is_none_or(|stack| stack.item != item || stack.count == 0)
        {
            return Err(EditOutcome::Invalid);
        }
        if peer.game_mode(&world.config) == terra_core::GameMode::Survival {
            if bag.revision == u64::MAX || world.journal.gameplay().revision == u64::MAX {
                return Err(EditOutcome::Invalid);
            }
            let mut inventory = bag.clone();
            if !inventory.take_from(usize::from(source.slot), item) {
                return Err(EditOutcome::Invalid);
            }
            Some(inventory)
        } else {
            None
        }
    } else if peer.game_mode(&world.config) == terra_core::GameMode::Creative {
        None
    } else {
        return Err(EditOutcome::Invalid);
    };
    Ok((plan, inventory))
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    plan(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}

pub(super) fn apply(command: EditCommand, peer: &Peer, world: &mut ControlWorld) -> EditOutcome {
    let (_, inventory) = match plan(command, peer, world) {
        Ok(plan) => plan,
        Err(outcome) => return outcome,
    };
    let EditAction::Block {
        position, expected, ..
    } = command.action
    else {
        return EditOutcome::Invalid;
    };
    match terra_world::snow::commit(
        &mut world.chunks,
        &mut world.journal,
        position,
        expected,
        &world.block_properties,
    ) {
        Ok(true) => {
            let game = world.journal.gameplay_mut();
            if let Some(inventory) = inventory {
                game.inventories.insert(peer.uuid, inventory);
                game.revision += 1;
            }
            let stats = game.player_stats.entry(peer.uuid).or_default();
            stats.blocks_placed = stats.blocks_placed.saturating_add(1);
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error, "Snow placement transaction rejected");
            EditOutcome::Invalid
        }
    }
}
