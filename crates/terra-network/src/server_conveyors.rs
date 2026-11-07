use super::*;
use terra_core::GameMode;
use terra_world::{conveyors, placement::Layout};

fn plan(command: EditCommand, peer: &Peer, world: &ControlWorld) -> Result<Layout, EditOutcome> {
    let EditAction::ConnectConveyor {
        start,
        end,
        start_revision,
        item,
    } = command.action
    else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.permissions.build
        || peer.vitals.dead()
        || !peer.synced
        || peer.chat.teleport.is_some()
        || world
            .sleep
            .as_deref()
            .is_some_and(|sleep| sleep.is_sleeping(peer.id))
        || peer.game_mode(&world.config) == GameMode::Spectator
        || !item.valid()
        || item.block_material() != Some(148)
        || command.look.iter().any(|v| !v.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
    {
        return Err(EditOutcome::Invalid);
    }
    if let Some(source) = command.source {
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
                .is_none_or(|stack| stack.item != item || stack.count == 0)
        {
            return Err(EditOutcome::Conflict);
        }
        if peer.game_mode(&world.config) == GameMode::Survival
            && (bag.revision == u64::MAX || world.journal.gameplay().revision == u64::MAX)
        {
            return Err(EditOutcome::Invalid);
        }
    } else if peer.game_mode(&world.config) != GameMode::Creative {
        return Err(EditOutcome::Invalid);
    }
    if world.journal.revision(global_to_section(start).0) != start_revision
        || world.journal.revision(global_to_section(end).0) != command.revision
    {
        return Err(EditOutcome::Conflict);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| EditOutcome::Invalid)?;
    let eye = body.translation + Vec3::Y * PLAYER_EYE_OFFSET;
    let direction =
        Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0) * Vec3::NEG_Z;
    let (_, target) = terra_world::targeting::pick_world_for_placement(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        }
        .with_season(
            world
                .journal
                .clock
                .unwrap_or_default()
                .season(world.config.world.seasons),
        ),
    );
    if !matches!(target,Some(WorldTarget::Block(hit)) if hit.position==end && hit.distance<=6.0) {
        return Err(EditOutcome::Occluded);
    }
    let plan = conveyors::layout(&world.chunks, &world.journal, start, end, item)
        .map_err(|_| EditOutcome::Invalid)?;
    if !crate::server::kingdoms::layout_allowed(&plan, peer, world) {
        return Err(EditOutcome::Invalid);
    }
    terra_world::placement::environment(
        &plan,
        &world.chunks,
        &world.journal,
        world.config.world.bounded_world,
    )
    .map_err(|_| EditOutcome::Invalid)?;
    let span = conveyors::proposed_span(&plan).ok_or(EditOutcome::Invalid)?;
    let boxes = conveyors::boxes(span);
    if world.config.world.bounded_world
        && boxes
            .iter()
            .any(|b| b.minimum[1] < -128.0 || b.maximum[1] > 357.0)
    {
        return Err(EditOutcome::Invalid);
    }
    let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
    if world.bodies.iter().any(|body| {
        boxes.iter().any(|bounds| {
            bounds.intersects(
                (body.translation - half).to_array().map(f64::from),
                (body.translation + half).to_array().map(f64::from),
            )
        })
    }) {
        return Err(EditOutcome::Invalid);
    }
    Ok(plan)
}
pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    if matches!(command.action, EditAction::CollectConveyor { .. }) {
        return collect_check(command, peer, world);
    }
    if matches!(command.action, EditAction::DyeConveyor { .. }) {
        return dye_plan(command, peer, world)
            .map_or_else(|outcome| outcome, |_| EditOutcome::Applied);
    }
    plan(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}
pub(super) fn apply(command: EditCommand, peer: &Peer, world: &mut ControlWorld) -> EditOutcome {
    if let EditAction::CollectConveyor { id } = command.action {
        let outcome = collect_check(command, peer, world);
        return if outcome != EditOutcome::Applied {
            outcome
        } else if world.journal.gameplay_mut().pickup(peer.uuid, id) {
            EditOutcome::Applied
        } else {
            EditOutcome::Invalid
        };
    }
    let dyed = matches!(command.action, EditAction::DyeConveyor { .. });
    let plan = match if dyed {
        dye_plan(command, peer, world)
    } else {
        plan(command, peer, world)
    } {
        Ok(plan) => plan,
        Err(outcome) => return outcome,
    };
    let next_inventory = if peer.game_mode(&world.config) == GameMode::Survival {
        let item = match command.action {
            EditAction::ConnectConveyor { item, .. } => item,
            EditAction::DyeConveyor { color, .. } => terra_core::ItemKind::Dye(color),
            _ => return EditOutcome::Invalid,
        };
        let Some(source) = command.source else {
            return EditOutcome::Invalid;
        };
        let Some(mut next) = world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .cloned()
        else {
            return EditOutcome::Invalid;
        };
        if !next.take_from(usize::from(source.slot), item) || !next.valid() {
            return EditOutcome::Invalid;
        }
        Some(next)
    } else {
        None
    };
    let next_gameplay = world.journal.gameplay().revision.checked_add(1);
    let rgb = match command.action {
        EditAction::DyeConveyor {
            color: terra_core::beds::DyeColor::Rgb(rgb),
            ..
        } => Some(rgb),
        EditAction::ConnectConveyor { item, .. } => terra_core::tints::color(item),
        _ => None,
    };
    let tint_cells: Vec<_> = plan
        .edits
        .iter()
        .filter(|e| e.material == 148)
        .map(|e| e.position)
        .collect();
    if tint_cells.iter().any(|p| {
        let section = global_to_section(*p).0;
        world.journal.revision(section) > u64::MAX - 512
            || world.journal.appearance_epoch(section) > u64::MAX - 512
            || world.journal.mesh_epoch(section) > u64::MAX - 512
    }) {
        return EditOutcome::Invalid;
    }
    let tint_changed = dyed
        && tint_cells
            .iter()
            .any(|cell| world.journal.tint_at(*cell) != rgb);
    match conveyors::commit(&mut world.chunks, &mut world.journal, &plan) {
        Ok(changed) if changed || tint_changed => {
            for cell in tint_cells {
                if let Err(error) = world.journal.set_tint(cell, rgb) {
                    tracing::warn!(%error,"Conveyor tint commit failed");
                    return EditOutcome::Invalid;
                }
            }
            if let Some(next) = next_inventory {
                let game = world.journal.gameplay_mut();
                game.inventories.insert(peer.uuid, next);
                game.revision = next_gameplay.unwrap();
            }
            EditOutcome::Applied
        }
        Ok(_) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error,"Conveyor span rejected");
            EditOutcome::Invalid
        }
    }
}

type InteractionTarget = ([f64; 3], [f64; 3], Option<WorldTarget>);

pub(super) fn interaction_target(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<InteractionTarget, EditOutcome> {
    if peer.vitals.dead()
        || !peer.synced
        || peer.chat.teleport.is_some()
        || peer.game_mode(&world.config) == GameMode::Spectator
        || world
            .sleep
            .as_deref()
            .is_some_and(|sleep| sleep.is_sleeping(peer.id))
        || command.look.iter().any(|v| !v.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
    {
        return Err(EditOutcome::Invalid);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| EditOutcome::Invalid)?;
    let eye = (body.translation + Vec3::Y * PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let direction = (Quat::from_euler(EulerRot::YXZ, command.look[0], command.look[1], 0.0)
        * Vec3::NEG_Z)
        .to_array()
        .map(f64::from);
    let picker = if matches!(command.action, EditAction::Block { material: 98, .. }) {
        terra_world::targeting::pick_world_for_placement
    } else {
        pick_world_with_context
    };
    let (_, target) = picker(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        eye,
        direction,
        terra_world::props::PropPose::WeatherSwept {
            frame: terra_world::props::weather_wind_frame(world.weather.wind()),
        }
        .with_season(
            world
                .journal
                .clock
                .unwrap_or_default()
                .season(world.config.world.seasons),
        ),
    );
    Ok((eye, direction, target))
}

fn collect_check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    let EditAction::CollectConveyor { id } = command.action else {
        return EditOutcome::Invalid;
    };
    let (eye, direction, target) = match interaction_target(command, peer, world) {
        Ok(target) => target,
        Err(outcome) => return outcome,
    };
    let limit = target.map_or(6.0, |target| match target {
        WorldTarget::Block(hit) => hit.distance.min(6.0),
        WorldTarget::Prop(hit) => hit.distance.min(6.0),
    });
    if !matches!(conveyors::carried_drop_hit(&world.journal.gameplay().drops, &world.journal, eye, direction, limit), Some((target, _)) if target == id)
    {
        return EditOutcome::Occluded;
    }
    let Some(drop) = world
        .journal
        .gameplay()
        .drops
        .iter()
        .find(|drop| drop.id == id)
    else {
        return EditOutcome::Conflict;
    };
    if drop.pickup_delay_seconds > 0.0
        || !(conveyors::carrying_segment(drop, &world.journal)
            .is_some_and(|segment| segment.span.loaded(&world.chunks, &world.journal))
            || terra_world::crushers::carrying(drop, &world.journal).is_some_and(|primary| {
                terra_world::crushers::loaded(primary, &world.chunks, &world.journal)
            })
            || terra_world::funnels::carrying(drop, &world.journal).is_some_and(|position| {
                terra_world::funnels::loaded(position, &world.chunks, &world.journal)
            })
            || conveyors::routing_carrying(drop, &world.journal).is_some_and(|position| {
                conveyors::routing_loaded(position, &world.chunks, &world.journal)
            })
            || conveyors::infusion_carrying(drop, &world.journal).is_some_and(|position| {
                conveyors::infusion_loaded(position, &world.chunks, &world.journal)
            }))
        || world.journal.gameplay().revision == u64::MAX
        || world
            .journal
            .gameplay()
            .inventories
            .get(&peer.uuid)
            .is_some_and(|bag| bag.revision == u64::MAX || !bag.has_capacity(drop.stack.item))
        || (!world
            .journal
            .gameplay()
            .inventories
            .contains_key(&peer.uuid)
            && world.journal.gameplay().inventories.len() >= 4096)
    {
        return EditOutcome::Invalid;
    }
    EditOutcome::Applied
}

fn dye_plan(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<Layout, EditOutcome> {
    let EditAction::DyeConveyor { position, color } = command.action else {
        return Err(EditOutcome::Invalid);
    };
    if !peer.permissions.build {
        return Err(EditOutcome::Invalid);
    }
    let (_, _, target) = interaction_target(command, peer, world)?;
    if !matches!(target, Some(WorldTarget::Block(hit)) if hit.position == position && hit.distance <= 6.0)
    {
        return Err(EditOutcome::Occluded);
    }
    if command.revision != world.journal.revision(global_to_section(position).0) {
        return Err(EditOutcome::Conflict);
    }
    let item = terra_core::ItemKind::Dye(color);
    if let Some(source) = command.source {
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
                .is_none_or(|stack| stack.item != item || stack.count == 0)
        {
            return Err(EditOutcome::Conflict);
        }
        if peer.game_mode(&world.config) == GameMode::Survival
            && (bag.revision == u64::MAX || world.journal.gameplay().revision == u64::MAX)
        {
            return Err(EditOutcome::Invalid);
        }
    } else if peer.game_mode(&world.config) != GameMode::Creative {
        return Err(EditOutcome::Invalid);
    }
    conveyors::dye_layout(&world.chunks, &world.journal, position, color)
        .map_err(|_| EditOutcome::Invalid)
}
