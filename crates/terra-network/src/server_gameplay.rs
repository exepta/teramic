use super::*;
use terra_core::GameMode;

pub(super) fn held_tool(peer: &Peer, world: &ControlWorld) -> Option<terra_core::ItemKind> {
    world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)?
        .slot(usize::from(peer.held_slot?))
        .map(|stack| stack.item)
}
fn tool_duration(
    target: MiningTarget,
    tool: Option<terra_core::ItemKind>,
    world: &ControlWorld,
) -> f32 {
    let speed = match (target, tool) {
        (MiningTarget::Block { expected, .. }, Some(terra_core::ItemKind::Tool(tool))) => {
            tool.speed(expected)
        }
        _ => 1.0,
    };
    let hardness = match target {
        MiningTarget::Block { expected, .. } => world
            .block_properties
            .0
            .get(&expected)
            .map_or(target.duration(), |p| p.hardness),
        _ => target.duration(),
    };
    hardness / speed
}

fn workbench_available(peer: &Peer, world: &ControlWorld, position: [i64; 3]) -> bool {
    if position.iter().any(|p| p.unsigned_abs() > 990_000)
        || world.chunks.block_at(position) != Some(terra_voxel::MaterialId::Workbench as u16)
    {
        return false;
    }
    let Ok(body) = world.bodies.get(peer.body) else {
        return false;
    };
    let eye = body.translation + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let mut target = Vec3::from_array(position.map(|p| p as f32 + 0.5));
    target.y = position[1] as f32 + 0.70;
    let delta = target - eye;
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
    .is_some_and(|hit| {
        hit.position == position
            || world
                .journal
                .object_state(position)
                .is_some_and(|state| state.peer == Some(hit.position))
    })
}

pub(super) struct MiningState {
    tool: Option<terra_core::ItemKind>,
    target: MiningTarget,
    command: EditCommand,
    started: Instant,
    pulse: Instant,
    reported: Instant,
}

fn command(target: MiningTarget, look: [f32; 2], journal: &EditJournal) -> EditCommand {
    let revision = match target {
        MiningTarget::Block { position, .. } => {
            journal.revision(terra_voxel::global_to_section(position).0)
        }
        MiningTarget::Prop { section, .. } => journal.prop_epoch(section),
    };
    EditCommand {
        request: 0,
        revision,
        look,
        action: target.action(),
        source: None,
    }
}

pub(super) fn intent(
    intent: MiningIntent,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    let Some(target) = intent.target else {
        if peer.mining.take().is_some() {
            sender.send::<ControlChannel>(ServerControl::Mining(None));
        }
        return;
    };
    if peer.game_mode(&world.config) != GameMode::Survival {
        return;
    }
    let tool = held_tool(peer, world);
    let now = Instant::now();
    let continuing = peer.mining.as_ref().is_some_and(|m| {
        m.target == target
            && m.tool == tool
            && now.duration_since(m.pulse) <= Duration::from_millis(350)
    });
    if !continuing {
        peer.mining = None;
    }
    let command = peer.mining.as_ref().map_or_else(
        || command(target, intent.look, &world.journal),
        |m| EditCommand {
            look: intent.look,
            ..m.command
        },
    );
    let valid = validation::check(command, peer, world);
    if valid != EditOutcome::Applied {
        peer.mining = None;
        sender.send::<ControlChannel>(ServerControl::Mining(None));
        sender.send::<ControlChannel>(ServerControl::EditResult {
            request: 0,
            action: target.action(),
            outcome: valid,
        });
        return;
    }
    if !continuing {
        peer.edits_left =
            (peer.edits_left + peer.rate_time.elapsed().as_secs_f64() * 10.0).min(20.0);
        peer.rate_time = now;
        if peer.edits_left < 1.0 {
            return;
        }
        peer.edits_left -= 1.0;
        peer.mining = Some(MiningState {
            tool,
            target,
            command,
            started: now,
            pulse: now,
            reported: now,
        });
        sender.send::<ControlChannel>(ServerControl::Mining(Some(MiningProgress {
            target,
            progress: 0.0,
            duration: tool_duration(target, tool, world),
        })));
    } else if let Some(mining) = peer.mining.as_mut() {
        mining.pulse = now;
        mining.command.look = intent.look;
    }
    peer.look = intent.look;
}

pub(super) fn tick(
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut world: ControlWorld,
    mut pickup_time: Local<f64>,
    time: Res<Time>,
) {
    let now = Instant::now();
    // Attraction and gravity must use the same simulation clock, including replay/tests.
    let pickup = time.elapsed_secs_f64() >= *pickup_time;
    if pickup {
        *pickup_time = time.elapsed_secs_f64() + 0.05;
    }
    for (mut peer, mut sender) in &mut peers {
        super::containers::refresh(&mut peer, &mut sender, &world);
        super::consumption::tick(&mut peer, &mut sender, &mut world, now);
        if !peer.synced || peer.vitals.dead() || peer.chat.teleport.is_some() {
            continue;
        }
        if let Some(mut mining) = peer.mining.take() {
            mining.command.look = peer.look;
            let valid = validation::check(mining.command, &peer, &world);
            if now.duration_since(mining.pulse) > Duration::from_millis(350)
                || valid != EditOutcome::Applied
                || mining.tool != held_tool(&peer, &world)
            {
                sender.send::<ControlChannel>(ServerControl::Mining(None));
            } else {
                let progress = (now.duration_since(mining.started).as_secs_f32()
                    / tool_duration(mining.target, mining.tool, &world))
                .min(1.0);
                if progress >= 1.0 {
                    let outcome = validation::apply(mining.command, &peer, &mut world, true);
                    sender.send::<ControlChannel>(ServerControl::EditResult {
                        request: 0,
                        action: mining.command.action,
                        outcome,
                    });
                    sender.send::<ControlChannel>(ServerControl::Mining(None));
                } else {
                    if now.duration_since(mining.reported) >= Duration::from_millis(80) {
                        mining.reported = now;
                        sender.send::<ControlChannel>(ServerControl::Mining(Some(
                            MiningProgress {
                                target: mining.target,
                                progress,
                                duration: tool_duration(mining.target, mining.tool, &world),
                            },
                        )));
                    }
                    peer.mining = Some(mining);
                }
            }
        }
        if pickup
            && peer.game_mode(&world.config) != GameMode::Spectator
            && let Ok(body) = world.bodies.get(peer.body)
        {
            let origin = body.translation.to_array().map(f64::from);
            let candidates: Vec<_> = world
                .journal
                .gameplay()
                .drops
                .iter()
                .filter(|drop| {
                    drop.pickup_delay_seconds <= 0.0
                        && (drop.grounded
                            || !matches!(drop.stack.item, terra_core::ItemKind::Block(51..=53)))
                        && pickup_visible_with_context(
                            &world.chunks,
                            &world.journal,
                            &world.block_properties,
                            origin,
                            drop.position,
                        )
                })
                .take(64)
                .map(|drop| drop.id)
                .collect();
            for id in candidates {
                // Avoid marking persistence dirty for a full inventory.
                let Some(drop) = world
                    .journal
                    .gameplay()
                    .drops
                    .iter()
                    .find(|drop| drop.id == id)
                else {
                    continue;
                };
                let capacity = world
                    .journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)
                    .is_none_or(|bag| bag.has_capacity(drop.stack.item));
                if capacity {
                    let delta: [f64; 3] = std::array::from_fn(|i| origin[i] - drop.position[i]);
                    let distance = delta.iter().map(|d| d * d).sum::<f64>().sqrt();
                    if distance > 0.35 {
                        if let Some(drop) = world
                            .journal
                            .gameplay_mut()
                            .drops
                            .iter_mut()
                            .find(|d| d.id == id)
                        {
                            for (i, delta) in delta.into_iter().enumerate() {
                                drop.position[i] += delta * 0.45;
                                drop.velocity[i] = (delta * 4.0) as f32;
                            }
                            drop.grounded = false;
                            drop.angular_velocity = drop.angular_velocity.map(|v| v * 0.5);
                        }
                    } else {
                        world.journal.gameplay_mut().pickup(peer.uuid, id);
                    }
                }
            }
        }
    }
}

pub(super) fn inventory(
    command: crate::protocol::inventory::InventoryCommand,
    peer: &mut Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
) {
    use crate::protocol::inventory::{InventoryAction, InventoryOutcome};
    if peer.game_mode(&world.config) == GameMode::Spectator {
        sender.send::<ControlChannel>(ServerControl::InventoryResult {
            outcome: InventoryOutcome::Denied,
            inventory: world
                .journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .cloned()
                .unwrap_or_default(),
        });
        return;
    }
    if matches!(
        command.action,
        InventoryAction::OpenContainer { .. } | InventoryAction::CloseContainer
    ) {
        super::containers::access(command, peer, sender, world);
        return;
    }
    if matches!(command.action, InventoryAction::LoadSmeltingRecipe { .. }) {
        super::containers::load_recipe(command, peer, sender, world);
        return;
    }
    let current = world
        .journal
        .gameplay()
        .inventories
        .get(&peer.uuid)
        .cloned()
        .unwrap_or_default();
    let mut next = current.clone();
    let mut drop = None;
    peer.edits_left = (peer.edits_left + peer.rate_time.elapsed().as_secs_f64() * 10.0).min(20.0);
    peer.rate_time = Instant::now();
    let outcome = if !world
        .journal
        .gameplay()
        .inventories
        .contains_key(&peer.uuid)
        && world.journal.gameplay().inventories.len() >= 4096
    {
        InventoryOutcome::Invalid
    } else if peer.edits_left < 1.0 {
        InventoryOutcome::RateLimited
    } else if command.revision != current.revision {
        InventoryOutcome::Conflict
    } else if matches!(
        command.action,
        InventoryAction::CreativeSet { .. }
            | InventoryAction::CreativeCursor { .. }
            | InventoryAction::CreativeQuickMove { .. }
            | InventoryAction::CreativeTrash { .. }
    ) && peer.game_mode(&world.config) != GameMode::Creative
    {
        InventoryOutcome::Denied
    } else {
        peer.edits_left -= 1.0;
        let applied = match command.action {
            // Legacy immediate-consumption packets must not bypass held-use timing.
            InventoryAction::Eat { .. } => false,
            InventoryAction::Click { slot, secondary } => next.click(usize::from(slot), secondary),
            InventoryAction::TakeToCursor { slot, count } => {
                next.take_cursor(usize::from(slot), count)
            }
            InventoryAction::TransferCursor {
                item,
                counts,
                to_cursor,
            } => next.transfer_cursor(item, counts, to_cursor),
            InventoryAction::ProtectedMove {
                slot,
                kind,
                reserved,
            } => next.with_reserved(reserved, |inventory| {
                use crate::protocol::inventory::ProtectedMoveKind;
                match kind {
                    ProtectedMoveKind::QuickMove => {
                        inventory.quick_move(usize::from(slot), terra_core::STACK_LIMIT)
                    }
                    ProtectedMoveKind::Collect => inventory.collect(usize::from(slot)),
                    ProtectedMoveKind::CollectFrom => inventory.collect_from(usize::from(slot)),
                    ProtectedMoveKind::WheelToCursor => inventory.wheel(usize::from(slot), true),
                }
            }),
            InventoryAction::Drag { slots, secondary } => next.drag(slots, secondary),
            InventoryAction::QuickMove { slot } => {
                next.quick_move(usize::from(slot), terra_core::STACK_LIMIT)
            }
            InventoryAction::Collect { slot } => next.collect(usize::from(slot)),
            InventoryAction::CollectFrom { slot } => next.collect_from(usize::from(slot)),
            InventoryAction::CraftCursor { slots } => world.recipes.craft_cells(
                &mut next,
                slots,
                terra_core::crafting::CraftDestination::Cursor,
            ),
            InventoryAction::Wheel { slot, to_cursor } => next.wheel(usize::from(slot), to_cursor),
            InventoryAction::CreativeCursor { item, count } => {
                next.creative_cursor(terra_core::ItemStack { item, count })
            }
            InventoryAction::CreativeQuickMove { item, count } => {
                next.creative_quick_move(terra_core::ItemStack { item, count })
            }
            InventoryAction::CreativeTrash { slot } => {
                next.take_stack(slot.map(usize::from), true).is_some()
            }
            InventoryAction::DropSlot { slot, all } => {
                drop = next.take_stack(Some(usize::from(slot)), all);
                drop.is_some()
            }
            InventoryAction::DropCursor { all } => {
                drop = next.take_stack(None, all);
                drop.is_some()
            }
            InventoryAction::ReturnCursor => {
                let had_cursor = next.cursor.is_some();
                next.return_cursor();
                drop = next.take_stack(None, true);
                next.revision = current.revision.saturating_add(1);
                had_cursor
            }
            InventoryAction::Move { from, to, count } => {
                next.move_slot(usize::from(from), usize::from(to), count)
            }
            InventoryAction::Craft { slots } => world.recipes.craft_cells(
                &mut next,
                slots,
                terra_core::crafting::CraftDestination::Inventory,
            ),
            InventoryAction::CraftGrid {
                slots,
                station,
                count,
                destination,
            } => {
                workbench_available(peer, world, station)
                    && world.recipes.craft_grid(
                        &mut next,
                        &slots,
                        3,
                        terra_core::crafting::CraftStation::Workbench,
                        count,
                        destination,
                    )
            }
            InventoryAction::CraftReserved {
                cells,
                size,
                station,
                count,
                destination,
            } => {
                let size = usize::from(size);
                let valid_station = match (size, station) {
                    (2, None) => Some(terra_core::crafting::CraftStation::Hand),
                    (3, Some(position)) if workbench_available(peer, world, position) => {
                        Some(terra_core::crafting::CraftStation::Workbench)
                    }
                    _ => None,
                };
                valid_station.is_some_and(|station| {
                    cells[size * size..].iter().all(Option::is_none)
                        && world.recipes.craft_reserved_grid(
                            &mut next,
                            &cells[..size * size],
                            size,
                            station,
                            count,
                            destination,
                        )
                })
            }
            InventoryAction::Recipe {
                recipe,
                count,
                station,
                destination,
            } => {
                let station = if let Some(position) = station {
                    if !workbench_available(peer, world, position) {
                        sender.send::<ControlChannel>(ServerControl::InventoryResult {
                            outcome: InventoryOutcome::WrongStation,
                            inventory: current,
                        });
                        return;
                    }
                    terra_core::crafting::CraftStation::Workbench
                } else {
                    terra_core::crafting::CraftStation::Hand
                };
                if let Err(error) =
                    world
                        .recipes
                        .craft(&mut next, recipe, count, station, destination)
                {
                    use terra_core::crafting::CraftError;
                    let outcome = match error {
                        CraftError::UnknownRecipe => InventoryOutcome::UnknownRecipe,
                        CraftError::WrongStation => InventoryOutcome::WrongStation,
                        CraftError::InvalidQuantity => InventoryOutcome::Invalid,
                        CraftError::MissingIngredients => InventoryOutcome::MissingIngredients,
                        CraftError::NoSpace => InventoryOutcome::NoSpace,
                    };
                    sender.send::<ControlChannel>(ServerControl::InventoryResult {
                        outcome,
                        inventory: current,
                    });
                    return;
                }
                true
            }
            InventoryAction::CreativeSet { slot, item, count } => {
                next.creative_set(usize::from(slot), terra_core::ItemStack { item, count })
            }
            InventoryAction::OpenContainer { .. }
            | InventoryAction::CloseContainer
            | InventoryAction::LoadSmeltingRecipe { .. }
            | InventoryAction::ViewDrag { .. } => false,
        };
        if applied
            && next.valid()
            && (drop.is_none()
                || (world.journal.gameplay().can_drop() && drop_launch(peer, world).is_some()))
        {
            InventoryOutcome::Applied
        } else {
            InventoryOutcome::Invalid
        }
    };
    let inventory = if outcome == InventoryOutcome::Applied {
        let launch = drop.and_then(|stack| {
            drop_launch(peer, world).map(|(position, velocity)| (stack, position, velocity))
        });
        let state = world.journal.gameplay_mut();
        if let Some((stack, position, velocity)) = launch
            && !state.drop_stack(position, stack, velocity, 1.0)
        {
            sender.send::<ControlChannel>(ServerControl::InventoryResult {
                outcome: InventoryOutcome::Invalid,
                inventory: current,
            });
            return;
        }
        state.inventories.insert(peer.uuid, next.clone());
        state.revision = state.revision.saturating_add(1);
        next
    } else {
        current
    };
    sender.send::<ControlChannel>(ServerControl::InventoryResult { outcome, inventory });
}

fn drop_launch(peer: &Peer, world: &ControlWorld) -> Option<([f64; 3], [f32; 3])> {
    let body = world.bodies.get(peer.body).ok()?;
    let direction = Quat::from_euler(EulerRot::YXZ, peer.look[0], peer.look[1], 0.0) * Vec3::NEG_Z;
    let origin = body.translation + Vec3::Y * 0.25;
    if !direction.is_finite() || !origin.is_finite() {
        return None;
    }
    // Stop before a wall instead of throwing through it or spawning inside terrain.
    let mut position = origin.to_array().map(f64::from);
    if !drop_bounds_clear_with_context(
        &world.chunks,
        &world.journal,
        &world.block_properties,
        position,
        [0.12; 3],
    ) {
        return None;
    }
    for step in 1..=5 {
        let next = (origin + direction * (step as f32 * 0.12))
            .to_array()
            .map(f64::from);
        if !drop_bounds_clear_with_context(
            &world.chunks,
            &world.journal,
            &world.block_properties,
            next,
            [0.12; 3],
        ) {
            break;
        }
        position = next;
    }
    if position.iter().any(|v| v.abs() > 1_000_000.0) {
        return None;
    }
    Some((position, (direction * 4.0 + Vec3::Y * 1.5).to_array()))
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ImpactTargets<'w, 's> {
    config: Option<Res<'w, GameConfig>>,
    peers: Query<'w, 's, &'static mut Peer>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
}

pub(super) fn simulate_drops(
    time: Res<Time<Fixed>>,
    chunks: Res<ChunkStore>,
    terrain: Option<Res<terra_world::streaming::WorldTerrain>>,
    mut journal: ResMut<EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mut sync: Local<f32>,
    mut impact: ImpactTargets,
) {
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 || journal.gameplay().drops.is_empty() {
        return;
    }
    let currents: Vec<_> = journal
        .gameplay()
        .drops
        .iter()
        .map(|drop| {
            terrain.as_ref().map_or_else(Default::default, |terrain| {
                let position = Vec3::from_array(drop.position.map(|v| v as f32));
                terra_physics::current::body_motion(
                    &terrain.0,
                    &chunks,
                    &journal,
                    position - Vec3::splat(0.12),
                    position + Vec3::splat(0.12),
                )
            })
        })
        .collect();
    let mut drops = std::mem::take(&mut journal.gameplay_mut().drops);
    *sync += dt;
    let count = drops.len();
    let mut moved = false;
    for (drop, current) in drops.iter_mut().zip(currents) {
        let before = (drop.position, drop.rotation, drop.grounded);
        let impact_speed = drop.velocity[1].abs();
        let falling = matches!(drop.stack.item, terra_core::ItemKind::Block(51..=53))
            && !drop.grounded
            && drop.velocity[1] < -4.0;
        drop.age_seconds += dt;
        drop.pickup_delay_seconds = (drop.pickup_delay_seconds - dt).max(0.0);
        if chunks
            .block_at(drop.position.map(|v| v.floor() as i64))
            .is_none()
        {
            continue;
        }
        drop.velocity =
            terra_physics::current::drift_velocity(Vec3::from_array(drop.velocity), current, dt)
                .to_array();
        step_drop_with_context(drop, &chunks, &journal, &properties, dt);
        if falling {
            for mut peer in &mut impact.peers {
                let Ok(body) = impact.bodies.get(peer.body) else {
                    continue;
                };
                let p = body.translation.to_array().map(f64::from);
                if (drop.position[0] - p[0]).abs() < 0.65
                    && (drop.position[2] - p[2]).abs() < 0.65
                    && before.0[1] + 0.5 >= p[1] - 0.9
                    && drop.position[1] - 0.5 <= p[1] + 0.9
                {
                    let mode = impact
                        .config
                        .as_ref()
                        .map_or(terra_core::GameMode::Survival, |config| {
                            peer.game_mode(config)
                        });
                    super::survival::environmental_damage(
                        &mut peer,
                        (impact_speed * 1.5).clamp(3.0, 30.0),
                        mode,
                        terra_core::DamageCause::FallingMineral,
                    );
                    drop.stack.item = match drop.stack.item {
                        terra_core::ItemKind::Block(51) => {
                            terra_core::ItemKind::Mineral(terra_core::OreKind::Salt.resource())
                        }
                        terra_core::ItemKind::Block(52) => terra_core::ItemKind::Block(37),
                        _ => terra_core::ItemKind::Block(49),
                    };
                    drop.velocity = [0.0; 3];
                    break;
                }
            }
        }
        if drop.grounded && drop.stack.item == terra_core::ItemKind::Block(51) {
            drop.stack.item = terra_core::ItemKind::Mineral(terra_core::OreKind::Salt.resource());
        }
        moved |= (drop.position, drop.rotation, drop.grounded) != before;
    }
    drops.retain(|drop| drop.age_seconds < terra_core::DROP_LIFETIME_SECONDS);
    let state = journal.gameplay_mut();
    state.drops = drops;
    if moved || count != state.drops.len() || *sync >= 1.0 {
        state.revision = state.revision.saturating_add(1);
        *sync = 0.0;
    }
}

#[cfg(test)]
fn step_drop(drop: &mut terra_core::WorldDrop, chunks: &ChunkStore, dt: f32) {
    step_drop_with_context(
        drop,
        chunks,
        &EditJournal::default(),
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        dt,
    )
}
fn step_drop_with_context(
    drop: &mut terra_core::WorldDrop,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    dt: f32,
) {
    let drop_bounds_clear = |chunks: &ChunkStore, center, half| {
        drop_bounds_clear_with_context(chunks, journal, properties, center, half)
    };
    let mut rotation = Quat::from_array(drop.rotation);
    let angular = Vec3::from_array(drop.angular_velocity);
    if angular.length_squared() > 0.0001 {
        rotation = (Quat::from_scaled_axis(angular * dt) * rotation).normalize();
    }
    if drop.grounded {
        // Let the nearest face land flat while retaining the item's individual yaw.
        let local_down = rotation.inverse() * Vec3::NEG_Y;
        let abs = local_down.abs();
        let face = if abs.x >= abs.y && abs.x >= abs.z {
            Vec3::X * local_down.x.signum()
        } else if abs.y >= abs.z {
            Vec3::Y * local_down.y.signum()
        } else {
            Vec3::Z * local_down.z.signum()
        };
        let target = Quat::from_rotation_arc(rotation * face, Vec3::NEG_Y) * rotation;
        rotation = rotation.slerp(target, (dt * 12.0).min(1.0)).normalize();
    }
    let axes = Mat3::from_quat(rotation);
    let size = if !drop.grounded && matches!(drop.stack.item, terra_core::ItemKind::Block(51..=53))
    {
        0.46
    } else {
        0.12
    };
    let mut half = (axes.x_axis.abs() + axes.y_axis.abs() + axes.z_axis.abs()) * size;
    // Rotating a cube can increase its height after touching the ground.
    if drop.grounded && !drop_bounds_clear(chunks, drop.position, half.to_array()) {
        let original = drop.position;
        for _ in 0..12 {
            drop.position[1] += 0.008;
            if drop_bounds_clear(chunks, drop.position, half.to_array()) {
                break;
            }
        }
        if !drop_bounds_clear(chunks, drop.position, half.to_array()) {
            // A wall must stop rotation, not lift the item up the wall every tick.
            drop.position = original;
            rotation = Quat::from_array(drop.rotation);
            let axes = Mat3::from_quat(rotation);
            half = (axes.x_axis.abs() + axes.y_axis.abs() + axes.z_axis.abs()) * 0.12;
            drop.angular_velocity = [0.0; 3];
        }
    }
    drop.rotation = rotation.to_array();
    drop.velocity[1] = (drop.velocity[1] - 22.0 * dt).max(-25.0);
    let steps = ((drop.velocity.iter().map(|v| v.abs()).fold(0.0, f32::max) * dt / 0.06).ceil()
        as usize)
        .clamp(1, 16);
    drop.grounded = false;
    for _ in 0..steps {
        for axis in 0..3 {
            if drop.velocity[axis] == 0.0 {
                continue;
            }
            let mut next = drop.position;
            next[axis] += f64::from(drop.velocity[axis] * dt / steps as f32);
            if drop_bounds_clear(chunks, next, half.to_array()) {
                drop.position = next;
            } else {
                let speed = drop.velocity[axis];
                if axis == 1 && speed < 0.0 {
                    let bounce = if size > 0.4 { 0.0 } else { -speed * 0.24 };
                    drop.velocity[axis] = if bounce < 0.65 { 0.0 } else { bounce };
                    drop.grounded = drop.velocity[axis] == 0.0;
                    if !drop.grounded {
                        drop.angular_velocity[0] *= 0.65;
                        drop.angular_velocity[2] *= 0.65;
                    }
                } else {
                    drop.velocity[axis] = -speed * 0.18;
                }
            }
        }
    }
    let friction = (-if drop.grounded { 8.5 } else { 0.2 } * dt).exp();
    let spin_damping = (-if drop.grounded { 9.0 } else { 0.4 } * dt).exp();
    for axis in [0, 2] {
        drop.velocity[axis] *= friction;
        if drop.velocity[axis].abs() < 0.015 {
            drop.velocity[axis] = 0.0;
        }
    }
    for velocity in &mut drop.angular_velocity {
        *velocity *= spin_damping;
        if velocity.abs() < 0.015 {
            *velocity = 0.0;
        }
    }
}

#[cfg(test)]
fn drop_clear(chunks: &ChunkStore, center: [f64; 3]) -> bool {
    drop_bounds_clear_with_context(
        chunks,
        &EditJournal::default(),
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        center,
        [0.12; 3],
    )
}

fn drop_bounds_clear_with_context(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    center: [f64; 3],
    half: [f32; 3],
) -> bool {
    let low: [i64; 3] = std::array::from_fn(|i| (center[i] - f64::from(half[i])).floor() as i64);
    let high: [i64; 3] = std::array::from_fn(|i| (center[i] + f64::from(half[i])).floor() as i64);
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let p = [x, y, z];
                if terra_world::interaction_shapes::space_intersects(
                    chunks,
                    journal,
                    properties,
                    p,
                    std::array::from_fn(|axis| center[axis] - f64::from(half[axis])),
                    std::array::from_fn(|axis| center[axis] + f64::from(half[axis])),
                ) {
                    return false;
                }
            }
        }
    }
    true
}

#[cfg(test)]
fn pickup_visible(chunks: &ChunkStore, origin: [f64; 3], target: [f64; 3]) -> bool {
    pickup_visible_with_context(
        chunks,
        &EditJournal::default(),
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        origin,
        target,
    )
}

fn pickup_visible_with_context(
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    origin: [f64; 3],
    target: [f64; 3],
) -> bool {
    let distance = (0..3)
        .map(|i| (target[i] - origin[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    if !distance.is_finite() || distance > 2.0 {
        return false;
    }
    if distance < 1.0e-9 {
        return !terra_world::interaction_shapes::space_intersects(
            chunks,
            journal,
            properties,
            origin.map(|v| v.floor() as i64),
            origin.map(|v| v - 1.0e-9),
            origin.map(|v| v + 1.0e-9),
        );
    }
    terra_world::interaction_shapes::raycast_solid(
        chunks,
        journal,
        properties,
        origin,
        std::array::from_fn(|axis| target[axis] - origin[axis]),
        distance.max(1.0e-9),
    )
    .is_none()
}

pub(super) fn send_gameplay(
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    journal: Res<EditJournal>,
    mut last: Local<Option<Instant>>,
) {
    if last.is_some_and(|last| last.elapsed() < Duration::from_millis(100)) {
        return;
    }
    *last = Some(Instant::now());
    let state = journal.gameplay();
    for (mut peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let center = body
            .translation
            .to_array()
            .map(|v| (v / 8.0).floor() as i64);
        let version = (state.revision, center);
        if peer.gameplay_sent == Some(version) {
            continue;
        }
        let mut drops: Vec<_> = state
            .drops
            .iter()
            .filter_map(|drop| {
                let distance = (0..3)
                    .map(|i| (drop.position[i] - f64::from(body.translation[i])).powi(2))
                    .sum::<f64>();
                (distance <= 48.0 * 48.0).then_some((distance, drop))
            })
            .collect();
        drops.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        sender.send::<ControlChannel>(ServerControl::Gameplay {
            inventory: state
                .inventories
                .get(&peer.uuid)
                .cloned()
                .unwrap_or_default(),
            drops: drops
                .into_iter()
                .take(256)
                .map(|(_, drop)| drop.clone())
                .collect(),
        });
        peer.gameplay_sent = Some(version);
    }
}

#[cfg(test)]
mod drop_tests {
    use super::*;

    #[test]
    fn drops_can_be_collected_through_workbench_gaps_but_not_its_tabletop() {
        let owner = terra_voxel::SectionPos::default();
        let mut data = terra_voxel::BlockSection::from_blocks(
            owner,
            0,
            vec![0; terra_voxel::blocks::SECTION_VOLUME],
        )
        .unwrap();
        data.set_block([8, 7, 7], 28).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert!(pickup_visible(&chunks, [7.7, 7.5, 7.5], [9.1, 7.5, 7.5]));
        assert!(!pickup_visible(&chunks, [7.7, 7.7, 7.5], [9.1, 7.7, 7.5]));
    }

    #[test]
    fn source_water_blocks_do_not_stop_drop_collision_or_pickup_visibility() {
        let owner = terra_voxel::SectionPos::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::from_blocks(
                    owner,
                    0,
                    vec![
                        terra_voxel::MaterialId::FluidWater as u16;
                        terra_voxel::blocks::SECTION_VOLUME
                    ],
                )
                .unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert!(drop_clear(&chunks, [7.5, 7.5, 7.5]));
        assert!(pickup_visible(&chunks, [7.5, 7.5, 7.5], [8.5, 7.5, 7.5]));
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([8, 7, 7], 3)
            .unwrap();
        assert!(!pickup_visible(&chunks, [7.5, 7.5, 7.5], [8.5, 7.5, 7.5]));
    }

    #[test]
    fn production_drop_simulation_drifts_in_actual_fluid_without_cloning_the_journal() {
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            .insert_resource(terra_world::streaming::WorldTerrain(
                terra_world::TerraGenerator::new(terra_core::WorldConfig {
                    sea_level: -100.0,
                    rivers: false,
                    ..Default::default()
                })
                .unwrap(),
            ))
            .add_systems(FixedUpdate, simulate_drops);
        let owner = terra_voxel::SectionPos::default();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        {
            let mut journal = app.world_mut().resource_mut::<EditJournal>();
            for x in 5..12 {
                for y in 0..16 {
                    journal
                        .set_fluid(
                            [x, y, 7],
                            terra_world::water::FluidCell {
                                amount: 8,
                                flow: [1, 0],
                                source: true,
                            },
                        )
                        .unwrap();
                }
            }
            assert!(journal.gameplay_mut().drop_stack(
                [7.5, 8.0, 7.5],
                terra_core::ItemStack {
                    item: terra_core::ItemKind::Block(3),
                    count: 1
                },
                [0.0; 3],
                10.0
            ));
        }
        for _ in 0..30 {
            app.update();
        }
        let drop = &app.world().resource::<EditJournal>().gameplay().drops[0];
        assert!(drop.position[0] > 7.6, "drop did not drift: {drop:?}");
        assert!(
            drop.position[0] < 8.0,
            "drop moved at an excessive current speed: {drop:?}"
        );
        assert_eq!(drop.stack.count, 1);
    }

    #[test]
    fn thrown_items_tumble_bounce_settle_flat_and_fall_when_support_is_removed() {
        let owner = terra_voxel::SectionPos::new(0, 0, 0);
        let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 0, z], 3).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let mut state = terra_core::GameplayState::default();
        assert!(state.drop_stack(
            [8.0, 5.0, 8.0],
            terra_core::ItemStack {
                item: terra_core::ItemKind::Block(3),
                count: 4
            },
            [1.2, 2.0, 0.3],
            1.0
        ));
        let drop = &mut state.drops[0];
        let initial = drop.rotation;
        let mut bounced = false;
        for _ in 0..600 {
            let was_falling = drop.velocity[1] < -1.0;
            step_drop(drop, &chunks, 1.0 / 60.0);
            bounced |= was_falling && drop.velocity[1] > 0.0;
        }
        assert!(bounced);
        assert_ne!(drop.rotation, initial);
        assert!(drop.valid());
        assert!(drop.grounded);
        assert!((1.11..1.23).contains(&drop.position[1]), "{drop:?}");
        assert!(drop.velocity.iter().all(|v| v.abs() < 0.02));
        assert_eq!(drop.angular_velocity, [0.0; 3]);
        let axes = Mat3::from_quat(Quat::from_array(drop.rotation));
        assert!(
            axes.x_axis
                .y
                .abs()
                .max(axes.y_axis.y.abs())
                .max(axes.z_axis.y.abs())
                > 0.999
        );
        let section = chunks.loaded.get_mut(&owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                section.data.set_block([x, 0, z], 0).unwrap();
            }
        }
        let before = drop.position[1];
        for _ in 0..15 {
            step_drop(drop, &chunks, 1.0 / 60.0);
        }
        assert!(!drop.grounded);
        assert!(drop.position[1] < before - 0.3);
    }
}
