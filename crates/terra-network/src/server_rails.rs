//! Authoritative, atomic ownership and item transactions for wide rail modules.
use super::*;
use terra_core::{GameMode, Inventory, ItemKind, ItemStack, kingdom::KingdomPermission};
use terra_world::{placement::Layout, rails};

#[derive(Clone, Copy)]
enum Operation {
    Place,
    Remove,
    Switch,
}
struct Prepared {
    layout: Layout,
    inventory: Option<Inventory>,
    drops: Vec<([f64; 3], ItemStack)>,
    operation: Operation,
}

/// Historic shapes remain usable without turning separate switches into track stock.
fn placement_source_allowed(action: EditAction, source: u16) -> bool {
    let target = match action {
        EditAction::Block { material, .. } => material,
        EditAction::ConnectRail { .. } => terra_core::rails::STRAIGHT,
        _ => return false,
    };
    match source {
        196 => terra_core::rails::is_rail(target) && !matches!(target, 202 | 203),
        197..=201 => target == source || target == terra_core::rails::STRAIGHT,
        202..=205 => target == source,
        _ => false,
    }
}

pub(super) fn handles(action: EditAction) -> bool {
    match action {
        EditAction::Block {
            expected, material, ..
        } => {
            terra_core::rails::is_rail(material)
                || material == 0 && terra_core::rails::is_rail(expected)
        }
        EditAction::ToggleObject { expected, .. } => {
            terra_core::rails::is_rail(expected) || expected == terra_core::rails::SWITCH_STAND
        }
        EditAction::ConnectRail { .. } => true,
        _ => false,
    }
}

fn prepare(
    command: EditCommand,
    peer: &Peer,
    world: &ControlWorld,
) -> Result<Prepared, EditOutcome> {
    if !peer.synced
        || peer.uuid.is_nil()
        || !peer.permissions.build
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || peer.game_mode(&world.config) == GameMode::Spectator
        || world
            .sleep
            .as_deref()
            .is_some_and(|sleep| sleep.is_sleeping(peer.id))
        || command.look.iter().any(|value| !value.is_finite())
        || command.look[0].abs() > std::f32::consts::TAU + 0.001
        || command.look[1].abs() > 1.501
    {
        return Err(EditOutcome::Invalid);
    }
    let (position, expected, operation) = match command.action {
        EditAction::ConnectRail {
            position, expected, ..
        } => (position, expected, Operation::Place),
        EditAction::Block {
            position,
            expected,
            material,
        } if terra_core::rails::is_rail(material) => (position, expected, Operation::Place),
        EditAction::Block {
            position,
            expected,
            material: 0,
        } if terra_core::rails::is_rail(expected) => (position, expected, Operation::Remove),
        EditAction::ToggleObject {
            position,
            expected,
            closing: false,
            ..
        } if expected == terra_core::rails::SWITCH_STAND => (position, expected, Operation::Switch),
        _ => return Err(EditOutcome::Invalid),
    };
    if position.iter().any(|value| value.unsigned_abs() > 990_000)
        || world.journal.revision(global_to_section(position).0) != command.revision
        || world.chunks.block_at(position) != Some(expected)
    {
        return Err(EditOutcome::Conflict);
    }
    let placement_material = match command.action {
        EditAction::Block {
            material: material @ (202 | 203),
            ..
        } => material,
        _ => terra_core::rails::STRAIGHT,
    };
    let gameplay = world.journal.gameplay();
    let selected_item = if matches!(operation, Operation::Place) {
        if let Some(source) = command.source {
            let bag = gameplay
                .inventories
                .get(&peer.uuid)
                .ok_or(EditOutcome::Invalid)?;
            if bag.revision != source.revision || peer.held_slot != Some(source.slot) {
                return Err(EditOutcome::Conflict);
            }
            let stack = bag
                .slot(usize::from(source.slot))
                .ok_or(EditOutcome::Invalid)?;
            if !stack.valid()
                || !stack
                    .item
                    .block_material()
                    .is_some_and(terra_core::rails::is_rail)
            {
                return Err(EditOutcome::Invalid);
            }
            stack.item
        } else if peer.game_mode(&world.config) == GameMode::Creative {
            ItemKind::Block(placement_material)
        } else {
            return Err(EditOutcome::Invalid);
        }
    } else {
        ItemKind::Block(terra_core::rails::STRAIGHT)
    };
    if matches!(operation, Operation::Place)
        && !selected_item
            .block_material()
            .is_some_and(|material| placement_source_allowed(command.action, material))
    {
        return Err(EditOutcome::Invalid);
    }
    let wood = selected_item.wood_species().unwrap_or_default();
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
    let picker = match operation {
        Operation::Place => terra_world::targeting::pick_world_for_placement,
        Operation::Remove => terra_world::targeting::pick_world_for_mining,
        Operation::Switch => terra_world::targeting::pick_world_with_context,
    };
    let (block, target) = picker(
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
    let hit = match target {
        Some(WorldTarget::Block(hit)) => hit,
        Some(WorldTarget::Prop(_)) if matches!(operation, Operation::Place) => {
            block.ok_or(EditOutcome::Occluded)?
        }
        _ => return Err(EditOutcome::Occluded),
    };
    if hit.distance > 4.5 {
        return Err(EditOutcome::OutOfReach);
    }
    let mut layout = match operation {
        Operation::Place => {
            if let EditAction::ConnectRail { start, .. } = command.action {
                if hit.position != position {
                    return Err(EditOutcome::Occluded);
                }
                let layout = rails::connection(
                    &world.chunks,
                    &world.journal,
                    start,
                    hit,
                    command.look,
                    wood,
                )
                .map_err(|_| EditOutcome::Invalid)?;
                let destination = rails::connection_target(&layout)
                    .ok_or(EditOutcome::Invalid)?
                    .position
                    .position();
                if (0..3)
                    .map(|axis| (destination[axis] - eye[axis]).powi(2))
                    .sum::<f64>()
                    > 4.5 * 4.5
                {
                    return Err(EditOutcome::OutOfReach);
                }
                layout
            } else {
                let EditAction::Block { material, .. } = command.action else {
                    unreachable!()
                };
                let layout = rails::placement(
                    &world.chunks,
                    &world.journal,
                    hit,
                    material,
                    (eye, direction),
                    command.look,
                )
                .map_err(|_| EditOutcome::Invalid)?;
                if !layout.states.iter().any(|(cell, state)| {
                    *cell == position
                        && state
                            .is_some_and(|state| !state.secondary && state.peer == Some(position))
                }) {
                    return Err(EditOutcome::Occluded);
                }
                layout
            }
        }
        Operation::Remove | Operation::Switch => {
            if hit.position != position {
                return Err(EditOutcome::Occluded);
            }
            if matches!(operation, Operation::Remove) {
                if peer.game_mode(&world.config) == GameMode::Survival
                    && world
                        .block_properties
                        .0
                        .get(&expected)
                        .is_some_and(|properties| {
                            !properties.can_harvest(super::super::gameplay::held_tool(peer, world))
                        })
                {
                    return Err(EditOutcome::Invalid);
                }
                rails::break_layout(&world.chunks, &world.journal, position)
                    .map_err(|_| EditOutcome::Conflict)?
            } else {
                let EditAction::ToggleObject { facing, .. } = command.action else {
                    unreachable!()
                };
                if world
                    .journal
                    .object_state(position)
                    .is_none_or(|state| state.facing != facing)
                {
                    return Err(EditOutcome::Conflict);
                }
                let switch = rails::stand::switch_at(&world.journal, position)
                    .ok_or(EditOutcome::Invalid)?;
                if !super::super::kingdoms::permission_allowed(
                    peer.uuid,
                    KingdomPermission::Inventories,
                    position,
                    world,
                ) || rails::cells(switch)
                    .iter()
                    .any(|cell| world.chunks.block_at(*cell) != Some(switch.material))
                {
                    return Err(EditOutcome::Invalid);
                }
                rails::toggle_switch_plan(&world.chunks, &world.journal, switch.anchor)
                    .map_err(|_| EditOutcome::Conflict)?
            }
        }
    };
    if matches!(operation, Operation::Place) {
        let existing_sites = match command.action {
            EditAction::ConnectRail { start, .. } => {
                rails::connection_sites(&world.journal, start, hit)
            }
            _ => rails::placement_sites(&world.journal, hit),
        }
        .ok_or(EditOutcome::Conflict)?;
        if existing_sites.iter().any(|cell| {
            world.chunks.block_at(*cell).is_none()
                || world.chunks.block_at(*cell) != world.journal.block(*cell)
                || !super::super::kingdoms::permission_allowed(
                    peer.uuid,
                    KingdomPermission::Build,
                    *cell,
                    world,
                )
        }) {
            return Err(EditOutcome::Invalid);
        }
        rails::set_wood(&mut layout, wood);
    }
    let clearance = if matches!(operation, Operation::Place) {
        rails::clearance(&layout, &world.chunks, &world.journal)
            .map_err(|_| EditOutcome::Invalid)?
    } else {
        Vec::new()
    };
    if let Some(WorldTarget::Prop(hit)) = target
        && !clearance.iter().any(|cleared| {
            cleared.prop.id == hit.prop.id && cleared.prop.section == hit.prop.section
        })
    {
        return Err(EditOutcome::Occluded);
    }
    if clearance.iter().any(|cleared| {
        cleared.rights_cells.iter().any(|cell| {
            !super::super::kingdoms::permission_allowed(
                peer.uuid,
                KingdomPermission::Break,
                *cell,
                world,
            ) || !super::super::kingdoms::permission_allowed(
                peer.uuid,
                KingdomPermission::Build,
                *cell,
                world,
            )
        })
    }) {
        return Err(EditOutcome::Invalid);
    }
    let permission = match operation {
        Operation::Place => KingdomPermission::Build,
        Operation::Remove => KingdomPermission::Break,
        Operation::Switch => KingdomPermission::Inventories,
    };
    if layout.edits.iter().any(|edit| {
        !super::super::kingdoms::permission_allowed(peer.uuid, permission, edit.position, world)
            || world.config.world.bounded_world && !(-128..=356).contains(&edit.position[1])
    }) {
        return Err(EditOutcome::Invalid);
    }
    if matches!(operation, Operation::Place) {
        let boxes = terra_world::placement::boxes(
            &layout,
            &world.chunks,
            &world.journal,
            &world.block_properties,
        );
        let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
        if world.bodies.iter().any(|body| {
            boxes.iter().any(|shape| {
                shape.intersects(
                    (body.translation - half).to_array().map(f64::from),
                    (body.translation + half).to_array().map(f64::from),
                )
            })
        }) {
            return Err(EditOutcome::Invalid);
        }
    }
    let gameplay = world.journal.gameplay();
    if gameplay.revision > u64::MAX - 4 {
        return Err(EditOutcome::RateLimited);
    }
    if matches!(operation, Operation::Place | Operation::Remove)
        && !gameplay.player_stats.contains_key(&peer.uuid)
        && gameplay.player_stats.len() >= 4096
    {
        return Err(EditOutcome::RateLimited);
    }
    let inventory = if matches!(operation, Operation::Place)
        && peer.game_mode(&world.config) == GameMode::Survival
    {
        let cost = rails::cost(&layout).ok_or(EditOutcome::Invalid)?;
        let bag = gameplay
            .inventories
            .get(&peer.uuid)
            .ok_or(EditOutcome::Invalid)?;
        if cost == 0
            || bag.count(selected_item) < cost
            || bag.revision.checked_add(u64::from(cost)).is_none()
        {
            return Err(EditOutcome::Invalid);
        }
        let mut next = bag.clone();
        let source = command.source.ok_or(EditOutcome::Invalid)?;
        if !next.take_from(usize::from(source.slot), selected_item) {
            return Err(EditOutcome::Invalid);
        }
        for _ in 1..cost {
            if !next.take_one(selected_item) {
                return Err(EditOutcome::Invalid);
            }
        }
        Some(next)
    } else {
        None
    };
    let mut drops = Vec::new();
    if peer.game_mode(&world.config) == GameMode::Survival {
        if matches!(operation, Operation::Remove) {
            let piece = rails::owner_at(&world.journal, position).ok_or(EditOutcome::Conflict)?;
            let state = world
                .journal
                .object_state(position)
                .ok_or(EditOutcome::Conflict)?;
            drops.push((
                position.map(|coordinate| coordinate as f64 + 0.5),
                ItemStack {
                    item: ItemKind::for_wood(
                        if piece.kind().is_some_and(terra_core::rails::Kind::is_switch) {
                            piece.material
                        } else {
                            terra_core::rails::STRAIGHT
                        },
                        state.wood,
                    ),
                    count: rails::piece_cost(piece),
                },
            ));
        }
        for cleared in &clearance {
            drops.extend(super::resource_stacks_for_prop(
                &cleared.prop,
                &world.journal,
            ));
        }
    }
    if gameplay.drops.len().saturating_add(drops.len()) > terra_core::MAX_WORLD_DROPS
        || gameplay.next_drop.checked_add(drops.len() as u64).is_none()
        || gameplay
            .revision
            .checked_add(drops.len() as u64 + clearance.len() as u64 * 2 + 2)
            .is_none()
    {
        return Err(EditOutcome::RateLimited);
    }
    if drops.iter().any(|(position, stack)| {
        !stack.valid()
            || position
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    }) {
        return Err(EditOutcome::Invalid);
    }
    Ok(Prepared {
        layout,
        inventory,
        drops,
        operation,
    })
}

pub(super) fn check(command: EditCommand, peer: &Peer, world: &ControlWorld) -> EditOutcome {
    prepare(command, peer, world).map_or_else(|outcome| outcome, |_| EditOutcome::Applied)
}

pub(super) fn apply(
    command: EditCommand,
    peer: &Peer,
    world: &mut ControlWorld,
    timed: bool,
) -> EditOutcome {
    if matches!(command.action, EditAction::Block { material: 0, .. })
        && peer.game_mode(&world.config) == GameMode::Survival
        && !timed
    {
        return EditOutcome::Invalid;
    }
    let prepared = match prepare(command, peer, world) {
        Ok(prepared) => prepared,
        Err(outcome) => return outcome,
    };
    match rails::commit(&mut world.chunks, &mut world.journal, &prepared.layout) {
        Ok(true) => {
            let gameplay = world.journal.gameplay_mut();
            if let Some(inventory) = prepared.inventory {
                gameplay.inventories.insert(peer.uuid, inventory);
                gameplay.revision += 1;
            }
            for (position, stack) in prepared.drops {
                let dropped = gameplay.drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
                debug_assert!(
                    dropped,
                    "rail drop was reserved before committing the module"
                );
            }
            match prepared.operation {
                Operation::Place => {
                    let stats = gameplay.player_stats.entry(peer.uuid).or_default();
                    stats.blocks_placed = stats.blocks_placed.saturating_add(1);
                }
                Operation::Remove => {
                    let stats = gameplay.player_stats.entry(peer.uuid).or_default();
                    stats.blocks_broken = stats.blocks_broken.saturating_add(1);
                }
                Operation::Switch => {}
            }
            EditOutcome::Applied
        }
        Ok(false) => EditOutcome::Conflict,
        Err(error) => {
            tracing::warn!(%error, "Rail transaction rejected");
            EditOutcome::Invalid
        }
    }
}
