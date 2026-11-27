//! Source-only bucket transfers share targeting, claims and inventory checks with the client.
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
        (ToolKind::Bucket, FarmingAction::FillBileBucket | FarmingAction::FillLavaBucket) => false,
        (ToolKind::BileBucket, FarmingAction::EmptyBileBucket)
        | (ToolKind::LavaBucket, FarmingAction::EmptyLavaBucket) => true,
        _ => return EditOutcome::Invalid,
    };
    if peer
        .farming
        .used_at
        .is_some_and(|t| t.elapsed().as_secs_f32() < 0.35)
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
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    let target = if matches!(
        action,
        FarmingAction::FillLavaBucket | FarmingAction::EmptyLavaBucket
    ) {
        terra_world::lava::target
    } else {
        terra_world::bile::target
    };
    if target(&world.chunks, &world.journal, origin, direction, filled) != Some(position) {
        return EditOutcome::Occluded;
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
    let EditAction::Farm { position, action } = command.action else {
        return EditOutcome::Invalid;
    };
    let lava = matches!(
        action,
        FarmingAction::FillLavaBucket | FarmingAction::EmptyLavaBucket
    );
    let filled_tool = if lava {
        ToolKind::LavaBucket
    } else {
        ToolKind::BileBucket
    };
    let filled = tool == filled_tool;
    let mut bag = world.journal.gameplay().inventories[&peer.uuid].clone();
    if bag.revision == u64::MAX || world.journal.gameplay().revision > u64::MAX - 512 {
        return EditOutcome::Invalid;
    }
    let consumes = peer.game_mode(&world.config) != GameMode::Creative || !filled;
    if consumes
        && !bag.transform_one(
            usize::from(command.source.unwrap().slot),
            ItemKind::Tool(if filled {
                ToolKind::Bucket
            } else {
                filled_tool
            }),
        )
    {
        return EditOutcome::Conflict;
    }
    let result = if lava {
        let transfer = if filled {
            terra_world::lava::pour
        } else {
            terra_world::lava::take_source
        };
        transfer(
            &mut world.chunks,
            &mut world.journal,
            &world.block_properties,
            position,
        )
    } else if filled {
        terra_world::bile::deposit_from_bucket(&mut world.chunks, &mut world.journal, position)
    } else {
        terra_world::bile::take_source(&mut world.chunks, &mut world.journal, position)
    };
    match result {
        Ok(true) => {
            if consumes {
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
            tracing::warn!(%error,"Liquid bucket transfer rejected");
            EditOutcome::Invalid
        }
    }
}
