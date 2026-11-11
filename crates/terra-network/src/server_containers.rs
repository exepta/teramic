//! Private chest snapshots and optimistic, atomic world-inventory transactions.
use super::*;
use crate::protocol::inventory::{
    ContainerCommand, ContainerHalf, ContainerSnapshot, ContainerView, InventoryAction,
    InventoryCommand, InventoryOutcome,
};
#[path = "server_hearth_containers.rs"]
mod hearths;

fn snapshot(world: &ControlWorld, position: [i64; 3]) -> Option<ContainerSnapshot> {
    if world.chunks.block_at(position) == Some(terra_core::logistics::BELT_DISTRIBUTOR) {
        world
            .journal
            .object_state(position)
            .filter(|s| s.valid_for(position, terra_core::logistics::BELT_DISTRIBUTOR))?;
        return Some(ContainerSnapshot {
            anchor: position,
            view: ContainerView::Primary,
            attached_campfire: false,
            workbench: None,
            furnace: None,
            crusher: None,
            mixer: None,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            filter: None,
            distributor: Some(
                world
                    .journal
                    .distributor_at(position)
                    .copied()
                    .unwrap_or_default(),
            ),
            halves: Vec::new(),
        });
    }
    if let Some(material) = world.chunks.block_at(position).filter(|material| {
        terra_core::logistics::supports_filter(*material)
            || terra_core::logistics::is_tunnel(*material)
    }) {
        world
            .journal
            .object_state(position)
            .filter(|s| s.valid_for(position, material))?;
        return Some(ContainerSnapshot {
            anchor: position,
            view: ContainerView::Primary,
            attached_campfire: false,
            workbench: None,
            furnace: None,
            crusher: None,
            mixer: None,
            cauldron: None,
            campfire: None,
            tunnel_random: terra_core::logistics::is_tunnel(material)
                .then(|| world.journal.tunnel_random_at(position)),
            distributor: None,
            filter: terra_core::logistics::supports_filter(material).then(|| {
                world
                    .journal
                    .logistics_filter_at(position)
                    .copied()
                    .unwrap_or_default()
            }),
            halves: Vec::new(),
        });
    }
    if let Some(material @ (59 | 99 | 156 | 161 | 162)) = world.chunks.block_at(position) {
        let selected = world.journal.object_state(position)?;
        let anchor = if selected.secondary {
            selected.peer?
        } else {
            position
        };
        let primary = world.journal.object_state(anchor)?;
        let loaded = if material == 99 {
            super::crushers::loaded(anchor, &world.chunks, &world.journal)
        } else if material == 156 {
            world
                .journal
                .object_state(anchor)
                .is_some_and(|state| state.valid_for(anchor, material))
                && world.journal.mixer_at(anchor).is_some()
        } else if material == 161 {
            world.journal.valid_cauldron(anchor) && world.journal.cauldron_at(anchor).is_some()
        } else if material == 162 {
            world.journal.valid_campfire(anchor) && world.journal.campfire_at(anchor).is_some()
        } else {
            super::furnaces::loaded(anchor, &world.chunks, &world.journal)
        };
        if primary.secondary
            || !primary.valid_for(anchor, material)
            || !selected.valid_for(position, material)
            || !loaded
            || position != anchor && primary.peer != Some(position)
        {
            return None;
        }
        let value = ContainerSnapshot {
            view: ContainerView::Primary,
            attached_campfire: world.journal.has_attached_campfire(anchor),
            tunnel_random: None,
            cauldron: (material == 161).then(|| {
                world.journal.cauldron_at(anchor).unwrap().view(
                    terra_world::hearths::heated_cauldron(&world.chunks, &world.journal, anchor),
                    world
                        .journal
                        .container_at(anchor)
                        .unwrap_or(&terra_core::Inventory::default()),
                )
            }),
            campfire: (material == 162).then(|| world.journal.campfire_at(anchor).unwrap().view()),
            distributor: None,
            filter: None,
            anchor,
            workbench: None,
            mixer: (material == 156).then(|| {
                let mut view = world
                    .journal
                    .mixer_at(anchor)
                    .cloned()
                    .unwrap_or_default()
                    .view(
                        world
                            .journal
                            .container_at(anchor)
                            .unwrap_or(&terra_core::Inventory::default()),
                    );
                view.working &= world.journal.kinetic_state(anchor).running();
                view
            }),
            furnace: (material == 59).then(|| {
                world
                    .journal
                    .furnace_at(anchor)
                    .cloned()
                    .unwrap_or_default()
                    .view()
            }),
            crusher: (material == 99).then(|| {
                let mut view = world
                    .journal
                    .crusher_at(anchor)
                    .cloned()
                    .unwrap_or_default()
                    .view(
                        world
                            .journal
                            .container_at(anchor)
                            .unwrap_or(&terra_core::Inventory::default()),
                    );
                view.working &= world.journal.crusher_driven(anchor);
                view
            }),
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: world
                    .journal
                    .container_at(anchor)
                    .cloned()
                    .unwrap_or_default(),
            }],
        };
        return value.valid().then_some(value);
    }
    if position.iter().any(|p| p.unsigned_abs() > 990_000)
        || !world
            .chunks
            .block_at(position)
            .is_some_and(terra_core::profession_stations::is_depot)
    {
        return None;
    }
    let state = world.journal.object_state(position).unwrap_or_default();
    let anchor = if state.secondary {
        state.peer?
    } else {
        position
    };
    let primary = world.journal.object_state(anchor).unwrap_or_default();
    let material = world.chunks.block_at(position)?;
    if world.chunks.block_at(anchor) != Some(material) || primary.secondary {
        return None;
    }
    let mut positions = vec![anchor];
    if let Some(peer) = primary.peer {
        let other = world.journal.object_state(peer)?;
        if material != 29
            || world.chunks.block_at(peer) != Some(29)
            || !other.secondary
            || other.peer != Some(anchor)
            || other.facing != primary.facing
        {
            return None;
        }
        positions.push(peer);
    }
    let value = ContainerSnapshot {
        view: ContainerView::Primary,
        attached_campfire: false,
        distributor: None,
        filter: None,
        anchor,
        workbench: None,
        furnace: None,
        crusher: None,
        mixer: None,
        cauldron: None,
        campfire: None,
        tunnel_random: None,
        halves: positions
            .into_iter()
            .map(|position| ContainerHalf {
                position,
                inventory: world
                    .journal
                    .container_at(position)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
    };
    (value.valid()
        && value.halves.iter().any(|half| half.position == position)
        && value
            .halves
            .iter()
            .all(|half| world.journal.block(half.position) == Some(material)))
    .then_some(value)
}

fn snapshot_view(
    world: &ControlWorld,
    position: [i64; 3],
    view: ContainerView,
) -> Option<ContainerSnapshot> {
    let mut value = snapshot(world, position)?;
    if view == ContainerView::AttachedCampfire {
        let fire = world.journal.attached_campfire_at(position)?;
        if !value.attached_campfire || value.cauldron.is_none() {
            return None;
        }
        value.view = view;
        value.cauldron = None;
        value.campfire = Some(fire.state.view());
        value.halves[0].inventory = fire.inventory.clone();
    }
    value.valid().then_some(value)
}

fn workbench_positions(world: &ControlWorld, position: [i64; 3]) -> Option<[[i64; 3]; 2]> {
    if world.chunks.block_at(position) != Some(28) {
        return None;
    }
    let state = world.journal.object_state(position)?;
    let anchor = if state.secondary {
        state.peer?
    } else {
        position
    };
    let primary = world.journal.object_state(anchor)?;
    let secondary = primary.peer?;
    let other = world.journal.object_state(secondary)?;
    (!primary.secondary
        && world.chunks.block_at(anchor) == Some(28)
        && world.chunks.block_at(secondary) == Some(28)
        && other.secondary
        && other.peer == Some(anchor)
        && other.facing == primary.facing)
        .then_some([anchor, secondary])
}

fn workbench_snapshot(world: &ControlWorld, position: [i64; 3]) -> Option<ContainerSnapshot> {
    let (workbench, chest) = world.journal.workbench_chest(position)?;
    workbench_positions(world, workbench)?;
    let mut value = snapshot(world, chest)?;
    value.workbench = Some(workbench);
    Some(value)
}

fn target_available(peer: &Peer, world: &ControlWorld, positions: &[[i64; 3]]) -> bool {
    if positions.iter().any(|p| {
        !super::kingdoms::permission_allowed(
            peer.uuid,
            terra_core::kingdom::KingdomPermission::Inventories,
            *p,
            world,
        )
    }) || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
    {
        return false;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return false;
    };
    let eye = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let visible = |aim: Vec3| {
        let delta = aim - eye;
        let distance = delta.length();
        if !(0.01..=6.0).contains(&distance) {
            return false;
        }
        terra_world::interaction_shapes::raycast(
            &world.chunks,
            &world.journal,
            &world.block_properties,
            eye.to_array().map(f64::from),
            delta.normalize().to_array().map(f64::from),
            f64::from(distance) + 0.1,
        )
        .is_some_and(|hit| positions.contains(&hit.position))
    };
    positions.iter().any(|position| {
        let mut aim = Vec3::from_array(position.map(|v| v as f32 + 0.5));
        if world.chunks.block_at(*position) == Some(28) {
            aim.y = position[1] as f32 + 0.70;
        }
        if visible(aim) {
            return true;
        }
        if world.chunks.block_at(*position).is_some_and(|m| {
            terra_core::logistics::supports_filter(m)
                || terra_core::logistics::is_tunnel(m)
                || m == terra_core::logistics::BELT_DISTRIBUTOR
                || matches!(m, 161 | 162)
        }) {
            let mut exposed = false;
            terra_world::interaction_shapes::visit_boxes(
                &world.chunks,
                &world.journal,
                &world.block_properties,
                *position,
                |bounds| {
                    if !exposed {
                        let point = std::array::from_fn(|axis| {
                            ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
                        });
                        exposed = visible(Vec3::from_array(point));
                    }
                },
            );
            return exposed;
        }
        if !matches!(world.chunks.block_at(*position), Some(99 | 156)) {
            return false;
        }
        let state = world.journal.object_state(*position).unwrap_or_default();
        if state.secondary {
            return false;
        }
        let width = if state.peer.is_some() { 2.0 } else { 1.0 };
        let origin = Vec3::from_array(position.map(|v| v as f32)) + Vec3::new(0.5, 0.0, 0.5);
        let rotation = Quat::from_rotation_y(f32::from(state.facing) * std::f32::consts::FRAC_PI_2);
        // The open center can be hidden by its own crank while these real rim faces remain visible.
        [
            [0.85, 0.85, 0.5],
            [0.15, 0.85, 0.5],
            [0.5, 0.85, 0.85],
            [0.5, 0.85, 0.15],
        ]
        .into_iter()
        .any(|[x, y, z]| visible(origin + rotation * Vec3::new(x * width - 0.5, y, z - 0.5)))
    })
}

fn available(peer: &Peer, world: &ControlWorld, snapshot: &ContainerSnapshot) -> bool {
    if !super::kingdoms::permission_allowed(
        peer.uuid,
        terra_core::kingdom::KingdomPermission::Inventories,
        snapshot.anchor,
        world,
    ) || snapshot.halves.iter().any(|half| {
        !super::kingdoms::permission_allowed(
            peer.uuid,
            terra_core::kingdom::KingdomPermission::Inventories,
            half.position,
            world,
        )
    }) {
        return false;
    }
    if snapshot.distributor.is_some() {
        return world.chunks.block_at(snapshot.anchor)
            == Some(terra_core::logistics::BELT_DISTRIBUTOR)
            && target_available(peer, world, &[snapshot.anchor]);
    }
    if snapshot.filter.is_some() {
        return world
            .chunks
            .block_at(snapshot.anchor)
            .is_some_and(terra_core::logistics::supports_filter)
            && target_available(peer, world, &[snapshot.anchor]);
    }
    if snapshot.tunnel_random.is_some() {
        return world
            .chunks
            .block_at(snapshot.anchor)
            .is_some_and(terra_core::logistics::is_tunnel)
            && target_available(peer, world, &[snapshot.anchor]);
    }
    if let Some(workbench) = snapshot.workbench {
        return workbench_positions(world, workbench)
            .is_some_and(|positions| target_available(peer, world, &positions));
    }
    if snapshot.crusher.is_some() {
        if !super::crushers::loaded(snapshot.anchor, &world.chunks, &world.journal) {
            return false;
        }
        return match world
            .journal
            .object_state(snapshot.anchor)
            .and_then(|state| state.peer)
        {
            Some(other) => target_available(peer, world, &[snapshot.anchor, other]),
            None => target_available(peer, world, &[snapshot.anchor]),
        };
    }
    if snapshot.mixer.is_some() {
        return world.chunks.block_at(snapshot.anchor) == Some(156)
            && world.journal.mixer_at(snapshot.anchor).is_some()
            && target_available(peer, world, &[snapshot.anchor]);
    }
    if snapshot.cauldron.is_some() || snapshot.campfire.is_some() {
        let material =
            if snapshot.cauldron.is_some() || snapshot.view == ContainerView::AttachedCampfire {
                161
            } else {
                162
            };
        return world.chunks.block_at(snapshot.anchor) == Some(material)
            && (snapshot.view != ContainerView::AttachedCampfire
                || world.journal.has_attached_campfire(snapshot.anchor)
                    && world
                        .journal
                        .attached_campfire_at(snapshot.anchor)
                        .is_some())
            && target_available(peer, world, &[snapshot.anchor]);
    }
    if snapshot.furnace.is_some() {
        return world
            .journal
            .object_state(snapshot.anchor)
            .and_then(|state| state.peer)
            .is_some_and(|upper| target_available(peer, world, &[snapshot.anchor, upper]));
    }
    let positions: Vec<_> = snapshot.halves.iter().map(|half| half.position).collect();
    target_available(peer, world, &positions)
}

pub(super) fn refresh(
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &ControlWorld,
) {
    let Some(previous) = peer.container.as_ref() else {
        return;
    };
    let next = if let Some(workbench) = previous.workbench {
        workbench_snapshot(world, workbench)
    } else {
        snapshot_view(world, previous.anchor, previous.view)
    }
    .filter(|next| {
        next.anchor == previous.anchor
            && next.view == previous.view
            && next
                .halves
                .iter()
                .map(|h| h.position)
                .eq(previous.halves.iter().map(|h| h.position))
            && available(peer, world, next)
    });
    if next.as_ref() != peer.container.as_ref() {
        peer.container = next.clone();
        sender.send::<ControlChannel>(ServerControl::Container(next.map(Box::new)));
    }
}

fn rate(peer: &mut Peer) -> bool {
    peer.edits_left = (peer.edits_left + peer.rate_time.elapsed().as_secs_f64() * 10.0).min(20.0);
    peer.rate_time = Instant::now();
    if peer.edits_left < 1.0 {
        return false;
    }
    peer.edits_left -= 1.0;
    true
}

pub(super) fn access(
    command: InventoryCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    let inventory = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or_default();
    let outcome = if matches!(command.action, InventoryAction::CloseContainer) {
        peer.container = None;
        sender.send::<ControlChannel>(ServerControl::Container(None));
        InventoryOutcome::Applied
    } else if !rate(peer) {
        InventoryOutcome::RateLimited
    } else if command.revision != inventory.revision {
        InventoryOutcome::Conflict
    } else if let InventoryAction::SetBrewingRecipe { station, recipe } = command.action {
        let outcome = hearths::set_recipe(peer, world, station, recipe);
        if outcome == InventoryOutcome::Applied {
            refresh(peer, sender, world);
        }
        outcome
    } else if let InventoryAction::SetCampfireLit { station, lit } = command.action {
        let outcome = hearths::set_lit(peer, world, station, lit);
        if outcome == InventoryOutcome::Applied {
            refresh(peer, sender, world);
        }
        outcome
    } else if let InventoryAction::FillBottle { slot, position } = command.action {
        hearths::fill_bottle(peer, world, &inventory, slot, position)
    } else if let InventoryAction::SetTunnelRandom { station, random } = command.action {
        let allowed = snapshot(world, station).is_some_and(|snapshot| {
            snapshot.tunnel_random.is_some() && available(peer, world, &snapshot)
        }) && peer
            .container
            .as_ref()
            .is_some_and(|snapshot| snapshot.anchor == station && snapshot.tunnel_random.is_some());
        if allowed {
            match world.journal.set_tunnel_random(station, random) {
                Ok(_) => {
                    refresh(peer, sender, world);
                    InventoryOutcome::Applied
                }
                Err(_) => InventoryOutcome::Invalid,
            }
        } else {
            InventoryOutcome::ContainerUnavailable
        }
    } else if let InventoryAction::SetDistributorFilter {
        station,
        output,
        filter,
    } = command.action
    {
        let allowed = output < 3
            && filter.valid()
            && snapshot(world, station)
                .is_some_and(|s| s.distributor.is_some() && available(peer, world, &s))
            && peer
                .container
                .as_ref()
                .is_some_and(|s| s.anchor == station && s.distributor.is_some());
        if allowed {
            let mut config = world
                .journal
                .distributor_at(station)
                .copied()
                .unwrap_or_default();
            config.filters[usize::from(output)] = filter;
            match world.journal.set_distributor(station, config) {
                Ok(_) => {
                    refresh(peer, sender, world);
                    InventoryOutcome::Applied
                }
                Err(_) => InventoryOutcome::Invalid,
            }
        } else {
            InventoryOutcome::ContainerUnavailable
        }
    } else if let InventoryAction::SetFunnelFilter { station, filter } = command.action {
        let allowed = filter.valid()
            && snapshot(world, station)
                .is_some_and(|s| s.filter.is_some() && available(peer, world, &s))
            && peer
                .container
                .as_ref()
                .is_some_and(|s| s.anchor == station && s.filter.is_some());
        if allowed {
            match world.journal.set_logistics_filter(station, filter) {
                Ok(_) => {
                    refresh(peer, sender, world);
                    InventoryOutcome::Applied
                }
                Err(_) => InventoryOutcome::Invalid,
            }
        } else {
            InventoryOutcome::ContainerUnavailable
        }
    } else if let InventoryAction::SetMixerColor { station, target } = command.action {
        let allowed = snapshot(world, station)
            .is_some_and(|s| s.mixer.is_some() && available(peer, world, &s))
            && peer
                .container
                .as_ref()
                .is_some_and(|s| s.anchor == station && s.mixer.is_some());
        if allowed && let Some(mut state) = world.journal.mixer_at(station).cloned() {
            if state.target != target {
                state.target = target;
                state.progress_seconds = 0.0;
            }
            match world.journal.set_mixer(station, state) {
                Ok(()) => {
                    refresh(peer, sender, world);
                    InventoryOutcome::Applied
                }
                Err(_) => InventoryOutcome::Invalid,
            }
        } else {
            InventoryOutcome::ContainerUnavailable
        }
    } else if let InventoryAction::OpenCampfire { position } = command.action {
        if inventory.cursor.is_some() {
            InventoryOutcome::Invalid
        } else if let Some(value) = snapshot_view(world, position, ContainerView::AttachedCampfire)
            .filter(|s| available(peer, world, s))
        {
            peer.mining = None;
            peer.container = Some(value.clone());
            sender.send::<ControlChannel>(ServerControl::Container(Some(Box::new(value))));
            InventoryOutcome::Applied
        } else {
            InventoryOutcome::ContainerUnavailable
        }
    } else if let InventoryAction::OpenContainer { position } = command.action {
        if inventory.cursor.is_some()
            && peer
                .container
                .as_ref()
                .is_some_and(|current| current.view != ContainerView::Primary)
        {
            sender.send::<ControlChannel>(ServerControl::InventoryResult {
                inventory,
                outcome: InventoryOutcome::Invalid,
            });
            return;
        }
        let bench = workbench_positions(world, position);
        if bench.is_some_and(|positions| target_available(peer, world, &positions)) {
            peer.mining = None;
            peer.container = workbench_snapshot(world, position);
            sender.send::<ControlChannel>(ServerControl::Container(
                peer.container.clone().map(Box::new),
            ));
            InventoryOutcome::Applied
        } else if let Some(value) = snapshot(world, position).filter(|s| available(peer, world, s))
        {
            peer.mining = None;
            peer.container = Some(value.clone());
            sender.send::<ControlChannel>(ServerControl::Container(Some(Box::new(value))));
            InventoryOutcome::Applied
        } else {
            peer.container = None;
            sender.send::<ControlChannel>(ServerControl::Container(None));
            InventoryOutcome::ContainerUnavailable
        }
    } else {
        InventoryOutcome::Invalid
    };
    let inventory = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or(inventory);
    sender.send::<ControlChannel>(ServerControl::InventoryResult { inventory, outcome });
}

pub(super) fn transfer(
    command: ContainerCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    refresh(peer, sender, world);
    let mut player = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or_default();
    let outcome = if !rate(peer) {
        InventoryOutcome::RateLimited
    } else if let Some(current) = peer
        .container
        .as_ref()
        .filter(|s| s.anchor == command.anchor && s.view == command.view)
    {
        if current.filter.is_some() || current.distributor.is_some() {
            InventoryOutcome::Invalid
        } else if command.inventory_revision != player.revision
            || command.revisions
                != current
                    .halves
                    .iter()
                    .map(|half| half.inventory.revision)
                    .collect::<Vec<_>>()
        {
            sender
                .send::<ControlChannel>(ServerControl::Container(Some(Box::new(current.clone()))));
            InventoryOutcome::Conflict
        } else if world.journal.gameplay().revision > u64::MAX - 4
            || (!world
                .journal
                .gameplay()
                .inventories
                .contains_key(&peer.uuid)
                && world.journal.gameplay().inventories.len() >= 4096)
        {
            InventoryOutcome::Invalid
        } else {
            let mut halves: Vec<_> = current
                .halves
                .iter()
                .map(|half| half.inventory.clone())
                .collect();
            if (current.workbench.is_some()
                || current.crusher.is_some()
                || current.mixer.is_some()
                || current.cauldron.is_some()
                || current.campfire.is_some()
                || matches!(
                    command.action,
                    terra_core::containers::ContainerGesture::Sort { .. }
                        | terra_core::containers::ContainerGesture::SortBackpack { .. }
                )
                || command.reserved == [0; terra_core::INVENTORY_SLOTS])
                && if current.furnace.is_some() {
                    terra_core::furnace::apply_gesture_reserved(
                        &mut player,
                        &mut halves[0],
                        command.action,
                        command.reserved,
                    )
                } else if current.crusher.is_some() {
                    terra_core::crusher::apply_gesture_reserved(
                        &mut player,
                        &mut halves[0],
                        command.action,
                        command.reserved,
                    )
                } else if current.mixer.is_some() {
                    terra_core::mixer::apply_gesture_reserved(
                        &mut player,
                        &mut halves[0],
                        command.action,
                        command.reserved,
                    )
                } else if current.cauldron.is_some() {
                    terra_core::brewing::apply_gesture_reserved(
                        &mut player,
                        &mut halves[0],
                        command.action,
                        command.reserved,
                    )
                } else if current.campfire.is_some() {
                    terra_core::campfire::apply_gesture_reserved(
                        &mut player,
                        &mut halves[0],
                        command.action,
                        command.reserved,
                    )
                } else {
                    terra_core::containers::apply_gesture_reserved(
                        &mut player,
                        &mut halves,
                        command.action,
                        command.reserved,
                    )
                }
            {
                // Ownership, all revisions and all resulting inventories were checked before commit.
                let positions: Vec<_> = current.halves.iter().map(|half| half.position).collect();
                let committed = if current.view == ContainerView::AttachedCampfire {
                    world.journal.commit_attached_container_transaction(
                        peer.uuid,
                        player.clone(),
                        current.anchor,
                        halves.remove(0),
                    )
                } else {
                    world.journal.commit_container_transaction(
                        peer.uuid,
                        player.clone(),
                        positions.into_iter().zip(halves).collect(),
                    )
                };
                if committed.is_ok() {
                    refresh(peer, sender, world);
                    InventoryOutcome::Applied
                } else {
                    player = world
                        .journal
                        .gameplay()
                        .inventories
                        .get(&peer.uuid)
                        .cloned()
                        .unwrap_or_default();
                    InventoryOutcome::Invalid
                }
            } else {
                InventoryOutcome::Invalid
            }
        }
    } else {
        sender.send::<ControlChannel>(ServerControl::Container(None));
        InventoryOutcome::ContainerUnavailable
    };
    sender.send::<ControlChannel>(ServerControl::InventoryResult {
        inventory: player,
        outcome,
    });
}

pub(super) fn load_recipe(
    command: InventoryCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    refresh(peer, sender, world);
    let mut player = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or_default();
    let outcome = (|| {
        let (recipe, station, crushing) = match command.action {
            InventoryAction::LoadSmeltingRecipe { recipe, station } => (recipe, station, false),
            InventoryAction::LoadCrushingRecipe { recipe, station } => (recipe, station, true),
            _ => return InventoryOutcome::Invalid,
        };
        if !rate(peer) {
            return InventoryOutcome::RateLimited;
        }
        if command.revision != player.revision {
            return InventoryOutcome::Conflict;
        }
        let Some(current) = peer.container.as_ref().filter(|s| {
            s.anchor == station
                && if crushing {
                    s.crusher.is_some()
                } else {
                    s.furnace.is_some()
                }
        }) else {
            return InventoryOutcome::WrongStation;
        };
        let Some(input) = (if crushing {
            terra_core::crusher::recipes()
                .iter()
                .find(|r| r.id == recipe)
                .map(|r| r.input)
        } else {
            terra_core::furnace::recipes()
                .iter()
                .find(|r| r.id == recipe)
                .map(|r| r.input)
        }) else {
            return InventoryOutcome::UnknownRecipe;
        };
        let mut inventory = current.halves[0].inventory.clone();
        if inventory.revision == u64::MAX || player.revision == u64::MAX {
            return InventoryOutcome::Invalid;
        }
        if inventory
            .slot(0)
            .is_some_and(|s| s.item != input || s.count == input.max_stack())
        {
            return InventoryOutcome::NoSpace;
        }
        let Some(slot) = player
            .slots
            .iter()
            .position(|s| s.valid() && s.item == input)
        else {
            return InventoryOutcome::MissingIngredients;
        };
        let count = inventory.slot(0).map_or(1, |s| s.count + 1);
        player.take_from(slot, input);
        inventory.creative_set(0, terra_core::ItemStack { item: input, count });
        if world
            .journal
            .commit_container_transaction(peer.uuid, player.clone(), vec![(station, inventory)])
            .is_err()
        {
            player = world
                .journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .cloned()
                .unwrap_or_default();
            return InventoryOutcome::Invalid;
        }
        InventoryOutcome::Applied
    })();
    refresh(peer, sender, world);
    sender.send::<ControlChannel>(ServerControl::InventoryResult {
        inventory: player,
        outcome,
    });
}

#[cfg(test)]
#[path = "server_profession_station_tests.rs"]
mod profession_station_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;

    fn current(app: &mut App, position: [i64; 3]) -> Option<ContainerSnapshot> {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        snapshot(&system.get_mut(app.world_mut()).unwrap(), position)
    }

    #[test]
    fn orphan_secondary_metadata_cannot_open_an_unrelated_anchor_snapshot() {
        let config = GameConfig::default();
        let mut app = App::new();
        app.insert_resource(terra_world::streaming::WorldTerrain(
            terra_world::TerraGenerator::new(config.world.clone()).unwrap(),
        ))
        .insert_resource(terra_core::WorldClock::new(config.world.time))
        .init_resource::<terra_core::WorldWeather>()
        .insert_resource(config)
        .init_resource::<terra_core::crafting::RecipeBook>()
        .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .init_resource::<ConnectedProfiles>()
        .init_resource::<riding::Mounts>();
        let anchor = [3, 1, 4];
        let secondary = [4, 1, 4];
        let section = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        let mut journal = EditJournal::default();
        for position in [anchor, secondary] {
            data.set_block(terra_voxel::global_to_section(position).1, 29)
                .unwrap();
            journal.record(position, 29).unwrap();
        }
        journal
            .set_object_state(
                secondary,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(anchor),
                    secondary: true,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks).insert_resource(journal);
        assert!(current(&mut app, anchor).is_some());
        assert!(current(&mut app, secondary).is_none());
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                anchor,
                Some(terra_world::edits::BlockObjectState {
                    peer: Some(secondary),
                    ..Default::default()
                }),
            )
            .unwrap();
        let joined = current(&mut app, secondary).unwrap();
        assert_eq!(joined.anchor, anchor);
        assert_eq!(joined.halves.len(), 2);
        app.insert_resource(EditJournal::default());
        assert!(current(&mut app, anchor).is_none());
    }
}
