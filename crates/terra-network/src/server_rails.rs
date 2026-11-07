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
    drop: Option<([f64; 3], ItemStack)>,
    operation: Operation,
}

pub(super) fn handles(action: EditAction) -> bool {
    match action {
        EditAction::Block {
            expected, material, ..
        } => {
            terra_core::rails::is_rail(material)
                || material == 0 && terra_core::rails::is_rail(expected)
        }
        EditAction::ToggleObject { expected, .. } => terra_core::rails::is_rail(expected),
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
        } if terra_core::rails::Kind::from_material(expected)
            .is_some_and(terra_core::rails::Kind::is_switch) =>
        {
            (position, expected, Operation::Switch)
        }
        _ => return Err(EditOutcome::Invalid),
    };
    if position.iter().any(|value| value.unsigned_abs() > 990_000)
        || world.journal.revision(global_to_section(position).0) != command.revision
        || world.chunks.block_at(position) != Some(expected)
    {
        return Err(EditOutcome::Conflict);
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
    let picker = match operation {
        Operation::Place => terra_world::targeting::pick_world_for_placement,
        Operation::Remove => terra_world::targeting::pick_world_for_mining,
        Operation::Switch => terra_world::targeting::pick_world_with_context,
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
    let Some(WorldTarget::Block(hit)) = target else {
        return Err(EditOutcome::Occluded);
    };
    if hit.distance > 4.5 {
        return Err(EditOutcome::OutOfReach);
    }
    let layout = match operation {
        Operation::Place => {
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
                    && state.is_some_and(|state| !state.secondary && state.peer == Some(position))
            }) {
                return Err(EditOutcome::Occluded);
            }
            layout
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
                rails::toggle_switch_plan(&world.chunks, &world.journal, position)
                    .map_err(|_| EditOutcome::Conflict)?
            }
        }
    };
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
    let inventory = if let Operation::Place = operation {
        let EditAction::Block { material, .. } = command.action else {
            unreachable!()
        };
        let item = ItemKind::Block(material);
        if let Some(source) = command.source {
            let bag = gameplay
                .inventories
                .get(&peer.uuid)
                .ok_or(EditOutcome::Invalid)?;
            if bag.revision != source.revision || peer.held_slot != Some(source.slot) {
                return Err(EditOutcome::Conflict);
            }
            if bag
                .slot(usize::from(source.slot))
                .is_none_or(|stack| stack.item != item || stack.count == 0)
            {
                return Err(EditOutcome::Invalid);
            }
            if peer.game_mode(&world.config) == GameMode::Survival {
                let mut next = bag.clone();
                if bag.revision == u64::MAX || !next.take_from(usize::from(source.slot), item) {
                    return Err(EditOutcome::Invalid);
                }
                Some(next)
            } else {
                None
            }
        } else if peer.game_mode(&world.config) == GameMode::Creative {
            None
        } else {
            return Err(EditOutcome::Invalid);
        }
    } else {
        None
    };
    let drop = if matches!(operation, Operation::Remove)
        && peer.game_mode(&world.config) == GameMode::Survival
    {
        if !gameplay.can_drop() {
            return Err(EditOutcome::RateLimited);
        }
        Some((
            position.map(|coordinate| coordinate as f64 + 0.5),
            ItemStack {
                item: ItemKind::Block(expected),
                count: 1,
            },
        ))
    } else {
        None
    };
    Ok(Prepared {
        layout,
        inventory,
        drop,
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
            if let Some((position, stack)) = prepared.drop {
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
