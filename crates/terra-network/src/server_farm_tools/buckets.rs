use super::*;

pub(super) fn check(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
    tool: ToolKind,
) -> EditOutcome {
    let EditAction::Farm { position, action } = command.action else {
        return EditOutcome::Invalid;
    };
    if world.config.world.bounded_world && !(-128..=356).contains(&position[1]) {
        return EditOutcome::Invalid;
    }
    let filled = match (tool, action) {
        (ToolKind::Bucket, FarmingAction::FillBucket) => false,
        (ToolKind::WaterBucket, FarmingAction::EmptyBucket) => true,
        _ => return EditOutcome::Invalid,
    };
    if peer
        .farming
        .used_at
        .is_some_and(|t| t.elapsed().as_secs_f32() < 0.35)
    {
        return EditOutcome::RateLimited;
    }
    if command.revision
        != world
            .journal
            .water_mesh_epoch(global_to_section(position).0)
    {
        return EditOutcome::Conflict;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return EditOutcome::Invalid;
    };
    let origin = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    if terra_world::water::buckets::target(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        filled,
        origin,
        direction,
    ) != Some(position)
    {
        return EditOutcome::Occluded;
    }
    let Some(cell) = terra_world::water::buckets::source(
        &world.chunks,
        &world.terrain.0,
        &world.journal,
        position,
    ) else {
        return EditOutcome::Invalid;
    };
    if cell.source == filled || !world.config.world.water {
        return EditOutcome::Invalid;
    }
    EditOutcome::Applied
}

pub(super) fn apply(
    command: EditCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    tool: ToolKind,
) -> EditOutcome {
    let EditAction::Farm { position, .. } = command.action else {
        return EditOutcome::Invalid;
    };
    let source = command.source.unwrap();
    let mut bag = world.journal.gameplay().inventories[&peer.uuid].clone();
    if bag.revision == u64::MAX {
        return EditOutcome::Invalid;
    }
    let filled = tool == ToolKind::WaterBucket;
    if !bag.take_from(usize::from(source.slot), ItemKind::Tool(tool)) {
        return EditOutcome::Conflict;
    }
    let result = ItemKind::Tool(if filled {
        ToolKind::Bucket
    } else {
        ToolKind::WaterBucket
    });
    if !bag.creative_set(
        usize::from(source.slot),
        terra_core::ItemStack {
            item: result,
            count: 1,
        },
    ) {
        return EditOutcome::Invalid;
    }
    match terra_world::water::buckets::transfer(
        &mut world.chunks,
        &mut world.journal,
        &world.terrain.0,
        &world.block_properties,
        position,
        filled,
    ) {
        Ok(true) => {
            world
                .journal
                .gameplay_mut()
                .inventories
                .insert(peer.uuid, bag.clone());
            peer.farming.used_at = Some(Instant::now());
            sender.send::<ControlChannel>(ServerControl::InventoryResult {
                outcome: crate::protocol::inventory::InventoryOutcome::Applied,
                inventory: bag,
            });
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error, "Bucket transfer rejected");
            EditOutcome::Invalid
        }
    }
}
