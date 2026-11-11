//! Authoritative cargo-funded track laying, before the train's movement step.
use super::*;
use terra_core::{
    GameMode, ItemKind,
    kingdom::KingdomPermission,
    rails::{Endpoint, Port},
    trains::{TrainError, Vehicle},
};
use terra_world::rails::{self, builder};
#[path = "server_train_builder_cargo.rs"]
mod cargo;

/// The scoop faces local negative Z, like the cab; either end of a train can carry it.
pub(super) fn direction(car: &Vehicle) -> i8 {
    car.parts
        .iter()
        .position(|part| part.material == terra_core::train_parts::RAIL_BUILDER)
        .map_or(1, |index| car.driver_direction(index))
}

fn free_end(car: &Vehicle, graph: &terra_core::rails::RouteNetwork) -> Option<Port> {
    let cursor = car.motion.cursor?;
    let piece = graph.get(cursor.anchor)?;
    let path = piece
        .active_paths()
        .into_iter()
        .find(|p| p.index == cursor.path)?;
    let mut end = Endpoint {
        anchor: cursor.anchor,
        port: path.ports[usize::from(cursor.forward == (direction(car) > 0))],
    };
    let mut visited = std::collections::BTreeSet::new();
    let module = car
        .parts
        .iter()
        .position(|p| p.material == terra_core::train_parts::RAIL_BUILDER)?;
    let front = car.world_point(car.parts[module].offset.map(|v| f64::from(v) + 0.5));
    loop {
        if !visited.insert(end) || visited.len() > 32 {
            return None;
        }
        let port = graph.port(end)?;
        if port
            .position
            .position()
            .iter()
            .zip(front)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            > 144.0
        {
            return None;
        }
        let Some(next) = graph.connected_endpoint(end) else {
            return Some(port);
        };
        let piece = graph.get(next.anchor)?;
        let path = piece
            .active_paths()
            .into_iter()
            .find(|p| p.ports.contains(&next.port))?;
        end = Endpoint {
            anchor: next.anchor,
            port: if path.ports[0] == next.port {
                path.ports[1]
            } else {
                path.ports[0]
            },
        };
    }
}

pub(super) fn tick(
    mut world: ControlWorld,
    seats: Res<trains::SeatedPlayers>,
    peers: Query<&Peer, With<Connected>>,
    time: Res<Time<Fixed>>,
    mut elapsed: Local<f64>,
    mut graph: Local<rails::NetworkCache>,
) {
    *elapsed += time.delta_secs_f64();
    if *elapsed < 0.2 {
        return;
    }
    *elapsed = 0.0;
    if graph.refresh(&world.journal).is_err() {
        return;
    }
    let ids: Vec<_> = world
        .journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .filter(|car| car.motion.rail_builder_active)
        .map(|car| car.id)
        .collect();
    for id in ids {
        let car = world.journal.gameplay().trains.vehicles[&id].clone();
        if car.motion.brake || car.motion.throttle * direction(&car) <= 0 {
            continue;
        }
        let Some(peer) = peers.iter().find(|p| {
            p.uuid == car.owner
                && p.synced
                && !p.vitals.dead()
                && p.chat.teleport.is_none()
                && p.permissions.build
                && seats.binding(p.uuid).is_some_and(|s| {
                    s.vehicle == id
                        && car
                            .parts
                            .get(usize::from(s.part))
                            .is_some_and(|part| part.material == terra_core::train_parts::CAB)
                })
        }) else {
            continue;
        };
        let result = step(&car, peer, &mut world, graph.graph());
        if let Err(error) = result {
            if let Some(stopped) = world.journal.gameplay_mut().trains.vehicles.get_mut(&id) {
                stopped.motion.brake = true;
                stopped.motion.throttle = 0;
                stopped.motion.stop_reason = match error {
                    TrainError::MissingMaterials => {
                        terra_core::train_motion::StopReason::BuilderMaterials
                    }
                    TrainError::Limit => terra_core::train_motion::StopReason::BuilderConsist,
                    TrainError::ClearanceTooLarge => {
                        terra_core::train_motion::StopReason::BuilderHeight
                    }
                    _ => terra_core::train_motion::StopReason::BuilderBlocked,
                };
            }
        } else {
            let _ = graph.refresh(&world.journal);
        }
    }
}

pub(super) fn step(
    car: &Vehicle,
    peer: &Peer,
    world: &mut ControlWorld,
    graph: &terra_core::rails::RouteNetwork,
) -> Result<(), TrainError> {
    builder::validate_consist(&world.journal.gameplay().trains, car)?;
    let mut located;
    let car = if car.motion.cursor.is_none() {
        located = car.clone();
        located.motion.cursor = Some(
            terra_world::train_motion::current_cursor(&world.chunks, &world.journal, graph, car)
                .map_err(|_| TrainError::Unsupported)?,
        );
        &located
    } else {
        car
    };
    let Some(port) = free_end(car, graph) else {
        return clear_front(car, peer, world);
    };
    if !builder::extension_in_reach(car, port).map_err(|_| TrainError::Unsupported)? {
        return clear_front(car, peer, world);
    }
    let creative = peer.game_mode(&world.config) == GameMode::Creative;
    if peer.game_mode(&world.config) == GameMode::Spectator {
        return Err(TrainError::Protected);
    }
    let registry = &world.journal.gameplay().trains;
    let group = registry.consist(car.id)?;
    let mut stocks: Vec<_> = group
        .iter()
        .map(|id| (*id, registry.vehicles[id].cargo.clone()))
        .collect();
    let track = stocks
        .iter()
        .flat_map(|(id, stores)| {
            stores
                .iter()
                .filter(|(index, _)| {
                    registry.vehicles[id]
                        .parts
                        .get(usize::from(**index))
                        .is_some_and(|part| {
                            matches!(part.material, 29 | terra_core::train_parts::CARGO)
                        })
                })
                .map(|(_, bag)| bag)
        })
        .flat_map(|bag| bag.slots.iter())
        .find(|s| s.count > 0 && s.item.block_material() == Some(terra_core::rails::STRAIGHT))
        .map(|s| s.item)
        .unwrap_or(ItemKind::Block(terra_core::rails::STRAIGHT));
    let plan = builder::plan_for_vehicle(
        &world.chunks,
        &world.journal,
        port,
        track.wood_species().unwrap_or_default(),
        car,
    )
    .map_err(|error| {
        tracing::warn!(vehicle = car.id, ?port, %error, "Rail builder cannot extend this endpoint");
        TrainError::Unsupported
    })?;
    for edit in &plan.layout().edits {
        if world.config.world.bounded_world && !(-128..=356).contains(&edit.position[1])
            || !super::kingdoms::permission_allowed(
                peer.uuid,
                KingdomPermission::Build,
                edit.position,
                world,
            )
            || !super::kingdoms::permission_allowed(
                peer.uuid,
                KingdomPermission::Break,
                edit.position,
                world,
            )
        {
            return Err(TrainError::Protected);
        }
        if edit.material != 0
            && world.bodies.iter().any(|body| {
                let p = body.translation;
                let c = edit.position;
                (p.x as f64 - c[0] as f64 - 0.5).abs() < 0.85
                    && (p.z as f64 - c[2] as f64 - 0.5).abs() < 0.85
                    && (p.y as f64 - c[1] as f64 - 0.5).abs() < 1.5
            })
        {
            return Err(TrainError::Occupied);
        }
    }
    let mut drops = if creative {
        vec![]
    } else {
        plan.drops().to_vec()
    };
    for cleared in plan.cleared() {
        if cleared.rights_cells.iter().any(|cell| {
            world.config.world.bounded_world && !(-128..=356).contains(&cell[1])
                || !super::kingdoms::permission_allowed(
                    peer.uuid,
                    KingdomPermission::Break,
                    *cell,
                    world,
                )
        }) {
            return Err(TrainError::Protected);
        }
        if !creative {
            drops.extend(super::validation::resource_stacks_for_prop(
                &cleared.prop,
                &world.journal,
            ));
        }
    }
    let drops = compact_drops(drops);
    if !creative {
        for (item, count) in [
            (track, plan.tracks()),
            (ItemKind::Block(206), plan.ballast()),
        ] {
            cargo::take(registry, &mut stocks, item, count)?;
        }
    }
    let drops = cargo::stow(car, registry, &mut stocks, drops)?;
    let gameplay = world.journal.gameplay();
    if gameplay.drops.len() + drops.len() > terra_core::MAX_WORLD_DROPS
        || gameplay.next_drop.checked_add(drops.len() as u64).is_none()
        || gameplay
            .revision
            .checked_add(drops.len() as u64 + 1)
            .is_none()
        || gameplay.trains.revision == u64::MAX
        || group
            .iter()
            .any(|id| registry.vehicles[id].revision == u64::MAX)
    {
        return Err(TrainError::Overflow);
    }
    if drops.iter().any(|(position, stack)| {
        !stack.valid()
            || position
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    }) {
        return Err(TrainError::Invalid);
    }
    plan.commit(&mut world.chunks, &mut world.journal)
        .map_err(|_| TrainError::Stale)?;
    let gameplay = world.journal.gameplay_mut();
    for (id, cargo) in stocks {
        let car = gameplay.trains.vehicles.get_mut(&id).unwrap();
        car.cargo = cargo;
        car.revision += 1;
    }
    for (position, stack) in drops {
        let accepted = gameplay.drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
        debug_assert!(accepted);
    }
    gameplay.revision += 1;
    gameplay.trains.revision += 1;
    Ok(())
}

/// Clear existing track independently of laying a new section at a free endpoint.
pub(super) fn clear_front(
    car: &Vehicle,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<(), TrainError> {
    if peer.game_mode(&world.config) == GameMode::Spectator {
        return Err(TrainError::Protected);
    }
    let plan = builder::plan_clearance(&world.chunks, &world.journal, car)
        .map_err(|_| TrainError::Unsupported)?;
    if plan.is_empty() {
        return Ok(());
    }
    let allowed = |cell: [i64; 3]| {
        (!world.config.world.bounded_world || (-128..=356).contains(&cell[1]))
            && super::kingdoms::permission_allowed(peer.uuid, KingdomPermission::Break, cell, world)
    };
    if plan
        .layout()
        .edits
        .iter()
        .any(|edit| !allowed(edit.position))
        || plan
            .cleared()
            .iter()
            .any(|prop| prop.rights_cells.iter().any(|cell| !allowed(*cell)))
    {
        return Err(TrainError::Protected);
    }
    let mut drops = Vec::new();
    if peer.game_mode(&world.config) != GameMode::Creative {
        drops.extend_from_slice(plan.drops());
        for prop in plan.cleared() {
            drops.extend(super::validation::resource_stacks_for_prop(
                &prop.prop,
                &world.journal,
            ));
        }
    }
    let drops = compact_drops(drops);
    let registry = &world.journal.gameplay().trains;
    let mut stocks: cargo::CargoDrafts = if car.motion.rail_builder_autoload
        && peer.game_mode(&world.config) != GameMode::Creative
    {
        registry
            .consist(car.id)?
            .iter()
            .map(|id| (*id, registry.vehicles[id].cargo.clone()))
            .collect()
    } else {
        Vec::new()
    };
    let drops = cargo::stow(car, registry, &mut stocks, drops)?;
    let gameplay = world.journal.gameplay();
    if gameplay.drops.len() + drops.len() > terra_core::MAX_WORLD_DROPS
        || gameplay.next_drop.checked_add(drops.len() as u64).is_none()
        || gameplay
            .revision
            .checked_add(drops.len() as u64 + 1)
            .is_none()
        || !stocks.is_empty()
            && (gameplay.trains.revision == u64::MAX
                || stocks
                    .iter()
                    .any(|(id, _)| gameplay.trains.vehicles[id].revision == u64::MAX))
        || drops.iter().any(|(position, stack)| {
            !stack.valid()
                || position
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        })
    {
        return Err(TrainError::Overflow);
    }
    plan.commit(&mut world.chunks, &mut world.journal)
        .map_err(|_| TrainError::Stale)?;
    let gameplay = world.journal.gameplay_mut();
    let mut cargo_changed = false;
    for (id, cargo) in stocks {
        let wagon = gameplay.trains.vehicles.get_mut(&id).unwrap();
        if wagon.cargo != cargo {
            wagon.cargo = cargo;
            wagon.revision += 1;
            cargo_changed = true;
        }
    }
    if cargo_changed {
        gameplay.trains.revision += 1;
    }
    for (position, stack) in drops {
        let accepted = gameplay.drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
        debug_assert!(accepted);
    }
    gameplay.revision += 1;
    Ok(())
}

/// Keep a tunnel pass from spawning one replicated drop entity per excavated block.
fn compact_drops(
    drops: Vec<([f64; 3], terra_core::ItemStack)>,
) -> Vec<([f64; 3], terra_core::ItemStack)> {
    let mut compact: Vec<([f64; 3], terra_core::ItemStack)> = Vec::new();
    for (position, mut stack) in drops {
        for (other_position, other) in &mut compact {
            if other.item != stack.item
                || other.count >= other.item.max_stack()
                || (0..3)
                    .map(|axis| (position[axis] - other_position[axis]).powi(2))
                    .sum::<f64>()
                    > 64.0
            {
                continue;
            }
            let count = stack.count.min(other.item.max_stack() - other.count);
            other.count += count;
            stack.count -= count;
            if stack.count == 0 {
                break;
            }
        }
        if stack.count > 0 {
            compact.push((position, stack));
        }
    }
    compact
}
