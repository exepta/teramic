//! Shovel preparation and bucket coating remain one checked authority transaction.
use super::*;

pub(super) fn is_tool(tool: ToolKind) -> bool {
    tool == ToolKind::TarBucket
        || tool.class() == Some(terra_core::block_properties::ToolClass::Shovel)
}

pub(super) fn check(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
    tool: ToolKind,
) -> EditOutcome {
    let EditAction::Farm { position, action } = command.action else {
        return EditOutcome::Invalid;
    };
    let pouring = tool == ToolKind::TarBucket;
    if action
        != if pouring {
            FarmingAction::PourTar
        } else {
            FarmingAction::MakePath
        }
        || world.config.world.bounded_world && !(-128..=356).contains(&position[1])
    {
        return EditOutcome::Invalid;
    }
    if peer
        .farming
        .used_at
        .is_some_and(|last| last.elapsed().as_secs_f32() < 0.35)
    {
        return EditOutcome::RateLimited;
    }
    if command.revision != world.journal.revision(global_to_section(position).0) {
        return EditOutcome::Conflict;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return EditOutcome::Invalid;
    };
    let origin = (body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    if terra_world::roads::target(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        origin,
        direction,
        pouring,
    ) != Some(position)
    {
        return EditOutcome::Occluded;
    }
    if pouring {
        let low = Vec3::new(
            position[0] as f32,
            position[1] as f32 + 15.0 / 16.0,
            position[2] as f32,
        );
        let high = Vec3::new(
            position[0] as f32 + 1.0,
            position[1] as f32 + 1.0,
            position[2] as f32 + 1.0,
        );
        let half = Vec3::new(
            terra_physics::character::PLAYER_RADIUS,
            terra_physics::character::PLAYER_HALF_HEIGHT,
            terra_physics::character::PLAYER_RADIUS,
        );
        if world.bodies.iter().any(|body| {
            let minimum = body.translation - half;
            let maximum = body.translation + half;
            (0..3)
                .all(|axis| maximum[axis] > low[axis] + 0.001 && minimum[axis] < high[axis] - 0.001)
        }) {
            return EditOutcome::Invalid;
        }
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
    let pouring = tool == ToolKind::TarBucket;
    let bag = if pouring && peer.game_mode(&world.config) != GameMode::Creative {
        let source = command.source.unwrap();
        let mut bag = world.journal.gameplay().inventories[&peer.uuid].clone();
        if bag.revision == u64::MAX || world.journal.gameplay().revision > u64::MAX - 512 {
            return EditOutcome::Invalid;
        }
        if !bag.transform_one(usize::from(source.slot), ItemKind::Tool(ToolKind::Bucket)) {
            return EditOutcome::Conflict;
        }
        Some(bag)
    } else {
        None
    };
    let result = if pouring {
        terra_world::roads::pour(&mut world.chunks, &mut world.journal, position)
    } else {
        terra_world::roads::make_path(&mut world.chunks, &mut world.journal, position)
    };
    match result {
        Ok(true) => {
            if let Some(bag) = bag {
                let game = world.journal.gameplay_mut();
                game.inventories.insert(peer.uuid, bag.clone());
                game.revision += 1;
                sender.send::<ControlChannel>(ServerControl::InventoryResult {
                    outcome: crate::protocol::inventory::InventoryOutcome::Applied,
                    inventory: bag,
                });
            }
            peer.farming.used_at = Some(Instant::now());
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error, "Road preparation rejected");
            EditOutcome::Invalid
        }
    }
}
