//! Parked-vehicle authority. Every command rechecks ownership, reach and terrain rights.
use super::*;
use crate::protocol::trains::{
    TrainAction, TrainCargoSnapshot, TrainCommand, TrainOutcome, TrainResult,
};
use std::collections::{BTreeMap, HashSet};
use terra_core::{
    GameMode, PlayerUuid,
    kingdom::KingdomPermission,
    trains::{SeatBinding, TrainError, Vehicle},
};
use terra_physics::character::PLAYER_EYE_OFFSET;

#[derive(Resource, Default)]
pub(super) struct SeatedPlayers {
    bindings: BTreeMap<PlayerUuid, SeatBinding>,
    poses: HashMap<PlayerUuid, (Vec3, f32)>,
    exits: HashMap<PlayerUuid, ExitIntent>,
}
#[derive(Clone, Copy)]
struct ExitIntent {
    vehicle: u64,
    feet: [f64; 3],
    yaw: f32,
}
impl SeatedPlayers {
    pub(super) fn binding(&self, player: PlayerUuid) -> Option<SeatBinding> {
        self.bindings.get(&player).copied()
    }
    pub(super) fn pose(&self, player: PlayerUuid) -> Option<(Vec3, f32)> {
        self.poses.get(&player).copied()
    }
    pub(super) fn has_exit(&self, player: PlayerUuid) -> bool {
        self.exits.contains_key(&player)
    }
    pub(super) fn take_exit(
        &mut self,
        player: PlayerUuid,
        chunks: &ChunkStore,
        journal: &EditJournal,
        terrain: &terra_world::TerraGenerator,
        properties: &terra_core::block_properties::BlockPropertiesCatalog,
    ) -> Option<Vec3> {
        let intent = *self.exits.get(&player)?;
        let position = safe_exit(intent, chunks, journal, terrain, properties)?;
        self.exits.remove(&player);
        Some(position)
    }
}

pub(super) fn horse_mount_blocked(
    peer: &Peer,
    world: &ControlWorld,
    seated: &SeatedPlayers,
    id: u64,
    feed: bool,
) -> bool {
    feed && (seated.binding(peer.uuid).is_some() || seated.has_exit(peer.uuid))
        && world
            .journal
            .gameplay()
            .creatures
            .animals
            .get(&id)
            .is_some_and(|animal| animal.kind == terra_core::creatures::CreatureKind::Horse)
        && peer
            .held_slot
            .and_then(|slot| {
                world
                    .journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)?
                    .slot(usize::from(slot))
            })
            .is_none()
}

fn overlaps_vehicle(car: &Vehicle, low: [f64; 3], high: [f64; 3]) -> bool {
    let bounds = car.model_bounds();
    if !(0..3).all(|axis| bounds[0][axis] < high[axis] && bounds[1][axis] > low[axis]) {
        return false;
    }
    car.parts.iter().any(|part| {
        terra_world::trains::part_boxes(car.model_origin(), part)
            .any(|shape| shape.intersects(low, high))
    })
}
fn exit_clear(
    intent: ExitIntent,
    chunks: &ChunkStore,
    journal: &EditJournal,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
    center: Vec3,
    ignore_mount: bool,
) -> bool {
    let half = Vec3::new(
        terra_physics::character::PLAYER_RADIUS,
        terra_physics::character::PLAYER_HALF_HEIGHT,
        terra_physics::character::PLAYER_RADIUS,
    );
    let low = (center - half).to_array().map(f64::from);
    let high = (center + half).to_array().map(f64::from);
    for x in low[0].floor() as i64..=(high[0] - 1e-6).floor() as i64 {
        for y in low[1].floor() as i64..=(high[1] - 1e-6).floor() as i64 {
            for z in low[2].floor() as i64..=(high[2] - 1e-6).floor() as i64 {
                if terra_world::interaction_shapes::space_intersects(
                    chunks,
                    journal,
                    properties,
                    [x, y, z],
                    low,
                    high,
                ) {
                    return false;
                }
            }
        }
    }
    !journal
        .gameplay()
        .trains
        .vehicles
        .values()
        .any(|car| (!ignore_mount || car.id != intent.vehicle) && overlaps_vehicle(car, low, high))
}
fn safe_exit(
    intent: ExitIntent,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &terra_world::TerraGenerator,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Option<Vec3> {
    let habitat = terra_world::creatures::Habitat {
        terrain,
        chunks,
        journal,
    };
    let (sin, cos) = f64::from(intent.yaw).sin_cos();
    for distance in [1.2, 1.8, 2.4] {
        for [x, z] in [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
        ] {
            for vertical in [0.5, -0.5, -1.5, -2.5, 1.5] {
                let probe = [
                    intent.feet[0] + distance * (x * cos + z * sin),
                    intent.feet[1] + vertical,
                    intent.feet[2] + distance * (-x * sin + z * cos),
                ];
                let Some(feet) = habitat.ground_sized(probe, [0.64, 1.8, 0.64], 0.0) else {
                    continue;
                };
                let target = Vec3::from_array(feet.map(|v| v as f32))
                    + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT;
                if !exit_clear(intent, chunks, journal, properties, target, false) {
                    continue;
                }
                let start = Vec3::from_array(intent.feet.map(|v| v as f32))
                    + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT;
                let samples = (start.distance(target) / 0.125).ceil().max(1.0) as usize;
                // Like rider dismount, the controlled transition ignores its
                // own mount. Terrain and every other vehicle still block it.
                if (0..=samples).all(|step| {
                    exit_clear(
                        intent,
                        chunks,
                        journal,
                        properties,
                        start.lerp(target, step as f32 / samples as f32),
                        true,
                    )
                }) {
                    return Some(target);
                }
            }
        }
    }
    None
}
#[derive(Resource, Default)]
pub(super) struct TrainRuntime {
    peers: HashMap<terra_core::PlayerId, TrainPeer>,
}
struct TrainPeer {
    request: u64,
    tokens: f64,
    updated: Instant,
    sent: Option<u64>,
}
impl Default for TrainPeer {
    fn default() -> Self {
        Self {
            request: 0,
            tokens: 12.0,
            updated: Instant::now(),
            sent: None,
        }
    }
}
impl TrainPeer {
    fn accept(&mut self, request: u64) -> Result<(), TrainError> {
        if request == 0 || request <= self.request {
            return Err(TrainError::Stale);
        }
        self.request = request;
        let now = Instant::now();
        self.tokens =
            (self.tokens + now.duration_since(self.updated).as_secs_f64() * 6.0).min(12.0);
        self.updated = now;
        if self.tokens < 1.0 {
            return Err(TrainError::RateLimited);
        }
        self.tokens -= 1.0;
        Ok(())
    }
}
fn permitted(
    peer: &Peer,
    world: &ControlWorld,
    cell: [i64; 3],
    permission: KingdomPermission,
) -> bool {
    super::kingdoms::permission_allowed(peer.uuid, permission, cell, world)
}
fn touch(world: &mut ControlWorld) -> Result<(), TrainError> {
    let game = world.journal.gameplay_mut();
    let revision = game.revision.checked_add(1).ok_or(TrainError::Overflow)?;
    let train_revision = game
        .trains
        .revision
        .checked_add(1)
        .ok_or(TrainError::Overflow)?;
    game.revision = revision;
    game.trains.revision = train_revision;
    Ok(())
}
fn reach(peer: &Peer, world: &ControlWorld, cell: [i64; 3]) -> Result<(), TrainError> {
    if cell.iter().any(|value| value.unsigned_abs() > 990_000) {
        return Err(TrainError::Invalid);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| TrainError::OutOfReach)?;
    let eye = (body.translation + Vec3::Y * PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
    let point = cell.map(|value| value as f64 + 0.5);
    let delta = std::array::from_fn::<_, 3, _>(|axis| point[axis] - eye[axis]);
    let distance = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    if distance > 6.0 {
        return Err(TrainError::OutOfReach);
    }
    if distance > 0.001
        && let Some(hit) = terra_world::trains::raycast(
            &world.journal,
            eye,
            delta.map(|value| value / distance),
            distance.min(8.0),
        )
        && hit.distance + 0.05 < distance
        && world
            .journal
            .gameplay()
            .trains
            .vehicles
            .get(&hit.vehicle)
            .and_then(|car| car.part_cell(usize::from(hit.part)))
            != Some(cell)
    {
        return Err(TrainError::OutOfReach);
    }
    if distance > 0.001
        && let Some(hit) = terra_world::interaction_shapes::raycast_solid(
            &world.chunks,
            &world.journal,
            &world.block_properties,
            eye,
            delta.map(|value| value / distance),
            distance + 0.05,
        )
        && hit.position != cell
        && hit.distance + 0.05 < distance
    {
        return Err(TrainError::OutOfReach);
    }
    Ok(())
}
fn station(peer: &Peer, world: &ControlWorld, cell: [i64; 3]) -> Result<(), TrainError> {
    if world.chunks.block_at(cell) != Some(terra_core::train_parts::STATION) {
        return Err(TrainError::Missing);
    }
    reach(peer, world, cell)?;
    if !permitted(peer, world, cell, KingdomPermission::Inventories) {
        return Err(TrainError::Protected);
    }
    Ok(())
}
fn vehicle(peer: &Peer, world: &ControlWorld, id: u64) -> Result<Vehicle, TrainError> {
    let vehicle = world
        .journal
        .gameplay()
        .trains
        .vehicles
        .get(&id)
        .ok_or(TrainError::Missing)?;
    if vehicle.owner != peer.uuid {
        return Err(TrainError::NotOwner);
    }
    if (0..vehicle.parts.len()).any(|index| {
        let cell = vehicle.part_cell(index).unwrap();
        world.chunks.block_at(cell).is_none()
            || !permitted(peer, world, cell, KingdomPermission::Inventories)
    }) {
        return Err(TrainError::Protected);
    }
    Ok(vehicle.clone())
}
fn check_revision(world: &ControlWorld, revision: u64) -> Result<(), TrainError> {
    if world.journal.gameplay().trains.revision == revision {
        Ok(())
    } else {
        Err(TrainError::Stale)
    }
}

pub(super) fn control(
    command: TrainCommand,
    peer: &Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    runtime: &mut TrainRuntime,
    seated: &mut SeatedPlayers,
) {
    let result = runtime
        .peers
        .entry(peer.id)
        .or_default()
        .accept(command.request)
        .and_then(|()| apply(command.action, peer, world, seated));
    let outcome = result.unwrap_or_else(TrainOutcome::Rejected);
    if matches!(outcome, TrainOutcome::Cargo(_))
        && let Some(inventory) = world.journal.gameplay().inventories.get(&peer.uuid)
    {
        sender.send::<ControlChannel>(ServerControl::InventoryResult {
            outcome: crate::protocol::inventory::InventoryOutcome::Applied,
            inventory: inventory.clone(),
        });
    }
    sender.send::<ControlChannel>(ServerControl::TrainResult(TrainResult {
        request: command.request,
        outcome,
    }));
}
pub(super) fn apply(
    action: TrainAction,
    peer: &Peer,
    world: &mut ControlWorld,
    seated: &mut SeatedPlayers,
) -> Result<TrainOutcome, TrainError> {
    if matches!(action, TrainAction::Leave) && peer.synced && !peer.uuid.is_nil() {
        if let Some(binding) = seated.bindings.get(&peer.uuid).copied() {
            if world.journal.gameplay().revision == u64::MAX
                || world.journal.gameplay().trains.revision == u64::MAX
            {
                return Err(TrainError::Overflow);
            }
            let exit = if !peer.vitals.dead()
                && peer.chat.teleport.is_none()
                && peer.game_mode(&world.config) != GameMode::Spectator
            {
                let (feet, yaw) = world
                    .journal
                    .gameplay()
                    .trains
                    .vehicles
                    .get(&binding.vehicle)
                    .and_then(|car| car.seat_pose(usize::from(binding.part)))
                    .ok_or(TrainError::Missing)?;
                let intent = ExitIntent {
                    vehicle: binding.vehicle,
                    feet,
                    yaw,
                };
                safe_exit(
                    intent,
                    &world.chunks,
                    &world.journal,
                    &world.terrain.0,
                    &world.block_properties,
                )
                .ok_or(TrainError::Busy)?;
                Some(intent)
            } else {
                None
            };
            touch(world)?;
            seated.bindings.remove(&peer.uuid);
            seated.poses.remove(&peer.uuid);
            if let Some(exit) = exit {
                seated.exits.insert(peer.uuid, exit);
            } else {
                seated.exits.remove(&peer.uuid);
            }
        }
        return Ok(TrainOutcome::Seated(None));
    }
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
    {
        return Err(TrainError::Protected);
    }
    match action {
        TrainAction::Inspect { station: cell } => {
            station(peer, world, cell)?;
            Ok(TrainOutcome::Inspected)
        }
        TrainAction::Assemble {
            station: cell,
            start,
            kind,
            revision,
        } => {
            station(peer, world, cell)?;
            check_revision(world, revision)?;
            let plan = terra_world::trains::plan_assembly(
                &world.chunks,
                &world.journal,
                peer.uuid,
                cell,
                start,
                kind,
            )?;
            let half = Vec3::new(
                terra_physics::character::PLAYER_RADIUS,
                terra_physics::character::PLAYER_HALF_HEIGHT,
                terra_physics::character::PLAYER_RADIUS,
            );
            if world.bodies.iter().any(|body| {
                overlaps_vehicle(
                    &plan.vehicle,
                    (body.translation - half).to_array().map(f64::from),
                    (body.translation + half).to_array().map(f64::from),
                )
            }) {
                return Err(TrainError::Occupied);
            }
            if world.config.world.bounded_world
                && plan
                    .parts
                    .iter()
                    .any(|part| !(-128..=356).contains(&part.cell[1]))
            {
                return Err(TrainError::Invalid);
            }
            let id = terra_world::trains::commit(&mut world.chunks, &mut world.journal, &plan)?;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Dismantle {
            station: cell,
            vehicle: id,
            revision,
        } => {
            station(peer, world, cell)?;
            check_revision(world, revision)?;
            vehicle(peer, world, id)?;
            if seated.bindings.values().any(|seat| seat.vehicle == id) {
                return Err(TrainError::Busy);
            }
            let plan = terra_world::trains::plan_dismantle(
                &world.chunks,
                &world.journal,
                peer.uuid,
                cell,
                id,
            )?;
            for (index, part) in plan.parts.iter().enumerate() {
                let half = Vec3::new(
                    terra_physics::character::PLAYER_RADIUS,
                    terra_physics::character::PLAYER_HALF_HEIGHT,
                    terra_physics::character::PLAYER_RADIUS,
                );
                if world.bodies.iter().any(|body| {
                    let low = (body.translation - half).to_array().map(f64::from);
                    let high = (body.translation + half).to_array().map(f64::from);
                    terra_core::train_parts::boxes(
                        plan.vehicle.parts[index].material,
                        part.state.unwrap_or_default().facing,
                    )
                    .into_iter()
                    .any(|shape| {
                        terra_core::block_shapes::ShapeBox {
                            minimum: std::array::from_fn(|axis| {
                                shape.minimum[axis] + part.cell[axis] as f64
                            }),
                            maximum: std::array::from_fn(|axis| {
                                shape.maximum[axis] + part.cell[axis] as f64
                            }),
                            ..shape
                        }
                        .intersects(low, high)
                    })
                }) {
                    return Err(TrainError::Occupied);
                }
            }
            terra_world::trains::commit(&mut world.chunks, &mut world.journal, &plan)?;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Couple {
            station: cell,
            first,
            first_end,
            second,
            second_end,
            revision,
        } => {
            station(peer, world, cell)?;
            check_revision(world, revision)?;
            for id in [first, second] {
                let car = vehicle(peer, world, id)?;
                if (0..3).any(|axis| car.anchor[axis].abs_diff(cell[axis]) > 64) {
                    return Err(TrainError::OutOfReach);
                }
                if (0..car.parts.len()).any(|index| {
                    !permitted(
                        peer,
                        world,
                        car.part_cell(index).unwrap(),
                        KingdomPermission::Build,
                    )
                }) {
                    return Err(TrainError::Protected);
                }
            }
            let next = world
                .journal
                .gameplay()
                .revision
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;
            world
                .journal
                .gameplay_mut()
                .trains
                .couple(peer.uuid, first, first_end, second, second_end)?;
            world.journal.gameplay_mut().revision = next;
            Ok(TrainOutcome::Changed {
                vehicle: Some(first),
            })
        }
        TrainAction::Uncouple {
            station: cell,
            vehicle: id,
            end,
            revision,
        } => {
            station(peer, world, cell)?;
            check_revision(world, revision)?;
            let car = vehicle(peer, world, id)?;
            let link = car.couplings[end.index()].ok_or(TrainError::Missing)?;
            let other = vehicle(peer, world, link.vehicle)?;
            for car in [car, other] {
                if (0..3).any(|axis| car.anchor[axis].abs_diff(cell[axis]) > 64) {
                    return Err(TrainError::OutOfReach);
                }
                if (0..car.parts.len()).any(|index| {
                    !permitted(
                        peer,
                        world,
                        car.part_cell(index).unwrap(),
                        KingdomPermission::Build,
                    )
                }) {
                    return Err(TrainError::Protected);
                }
            }
            let next = world
                .journal
                .gameplay()
                .revision
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;
            world
                .journal
                .gameplay_mut()
                .trains
                .uncouple(peer.uuid, id, end)?;
            world.journal.gameplay_mut().revision = next;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::OpenCargo { vehicle: id, part } => {
            let car = vehicle(peer, world, id)?;
            let cell = car
                .part_cell(usize::from(part))
                .ok_or(TrainError::Missing)?;
            reach(peer, world, cell)?;
            let inventory = car.cargo.get(&part).ok_or(TrainError::Missing)?.clone();
            Ok(TrainOutcome::Cargo(TrainCargoSnapshot {
                vehicle: id,
                part,
                revision: car.revision,
                inventory,
            }))
        }
        TrainAction::Cargo {
            vehicle: id,
            part,
            revision,
            player_revision,
            gesture,
            reserved,
        } => {
            let car = vehicle(peer, world, id)?;
            if car.revision != revision {
                return Err(TrainError::Stale);
            }
            reach(
                peer,
                world,
                car.part_cell(usize::from(part))
                    .ok_or(TrainError::Missing)?,
            )?;
            let game = world.journal.gameplay();
            let mut bag = game
                .inventories
                .get(&peer.uuid)
                .ok_or(TrainError::Missing)?
                .clone();
            if bag.revision != player_revision {
                return Err(TrainError::Stale);
            }
            let mut inventories = [car.cargo.get(&part).ok_or(TrainError::Missing)?.clone()];
            if !terra_core::containers::apply_gesture_reserved(
                &mut bag,
                &mut inventories,
                gesture,
                reserved,
            ) {
                return Err(TrainError::Invalid);
            }
            let next = game.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            let train_revision = game
                .trains
                .revision
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;
            let vehicle_revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            let inventory = inventories.into_iter().next().unwrap();
            let game = world.journal.gameplay_mut();
            let car = game.trains.vehicles.get_mut(&id).unwrap();
            car.cargo.insert(part, inventory.clone());
            car.revision = vehicle_revision;
            game.inventories.insert(peer.uuid, bag);
            game.trains.revision = train_revision;
            game.revision = next;
            Ok(TrainOutcome::Cargo(TrainCargoSnapshot {
                vehicle: id,
                part,
                revision: vehicle_revision,
                inventory,
            }))
        }
        TrainAction::Seat { vehicle: id, part } => {
            if world.mounts.active.contains_key(&peer.uuid) {
                return Err(TrainError::Busy);
            }
            let car = vehicle(peer, world, id)?;
            reach(
                peer,
                world,
                car.part_cell(usize::from(part))
                    .ok_or(TrainError::Missing)?,
            )?;
            let (position, yaw) = car
                .seat_pose(usize::from(part))
                .ok_or(TrainError::Invalid)?;
            let binding = SeatBinding {
                player: peer.uuid,
                vehicle: id,
                part,
            };
            if seated
                .bindings
                .values()
                .any(|seat| seat.vehicle == id && seat.part == part && seat.player != peer.uuid)
            {
                return Err(TrainError::Busy);
            }
            if seated.bindings.get(&peer.uuid) != Some(&binding) {
                touch(world)?;
                seated.bindings.insert(peer.uuid, binding);
            }
            seated.poses.insert(
                peer.uuid,
                (
                    Vec3::from_array(position.map(|value| value as f32))
                        + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                    yaw,
                ),
            );
            seated.exits.remove(&peer.uuid);
            Ok(TrainOutcome::Seated(Some(binding)))
        }
        TrainAction::Leave => unreachable!("leave is handled before access gates"),
    }
}

pub(super) fn broadcast(
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut runtime: ResMut<TrainRuntime>,
    mut seated: ResMut<SeatedPlayers>,
    mut journal: ResMut<EditJournal>,
    chunks: Res<ChunkStore>,
    config: Res<GameConfig>,
) {
    let active: HashSet<_> = peers
        .iter()
        .filter(|(peer, _)| {
            peer.synced
                && peer.permissions.build
                && peer.game_mode(&config) != GameMode::Spectator
                && !peer.vitals.dead()
                && peer.chat.teleport.is_none()
        })
        .map(|(peer, _)| peer.uuid)
        .collect();
    let alive: HashSet<_> = peers
        .iter()
        .filter(|(peer, _)| {
            peer.synced
                && peer.game_mode(&config) != GameMode::Spectator
                && !peer.vitals.dead()
                && peer.chat.teleport.is_none()
        })
        .map(|(peer, _)| peer.uuid)
        .collect();
    seated.exits.retain(|player, _| alive.contains(player));
    let previous = seated.bindings.len();
    let mut removed = Vec::new();
    seated.bindings.retain(|player, seat| {
        let keep = active.contains(player)
            && journal
                .gameplay()
                .trains
                .vehicles
                .get(&seat.vehicle)
                .is_some_and(|car| {
                    car.owner == *player
                        && car.seat_pose(usize::from(seat.part)).is_some()
                        && (0..car.parts.len()).all(|index| {
                            car.part_cell(index).is_some_and(|cell| {
                                chunks.block_at(cell).is_some()
                                    && terra_world::kingdom_access::can_use(
                                        &journal.gameplay().kingdoms,
                                        &chunks,
                                        *player,
                                        KingdomPermission::Inventories,
                                        cell,
                                    )
                            })
                        })
                });
        if !keep {
            removed.push((*player, *seat));
        }
        keep
    });
    for (player, seat) in removed {
        if alive.contains(&player)
            && let Some((position, yaw)) = seated.poses.get(&player).copied()
        {
            seated.exits.entry(player).or_insert(ExitIntent {
                vehicle: seat.vehicle,
                feet: (position - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT)
                    .to_array()
                    .map(f64::from),
                yaw,
            });
        }
    }
    if seated.bindings.len() != previous {
        seats_changed(&mut journal, &mut runtime);
    }
    let poses = seated
        .bindings
        .iter()
        .filter_map(|(player, seat)| {
            let (position, yaw) = journal
                .gameplay()
                .trains
                .vehicles
                .get(&seat.vehicle)?
                .seat_pose(usize::from(seat.part))?;
            Some((
                *player,
                (
                    Vec3::from_array(position.map(|value| value as f32))
                        + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                    yaw,
                ),
            ))
        })
        .collect();
    seated.poses = poses;
    let registry = &journal.gameplay().trains;
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let state = runtime.peers.entry(peer.id).or_default();
        if state.sent != Some(registry.revision) {
            let snapshot = registry.snapshot(seated.bindings.values().copied().collect());
            sender.send::<ControlChannel>(ServerControl::Trains(Box::new(snapshot)));
            state.sent = Some(registry.revision);
        }
    }
    let ids: HashSet<_> = peers.iter().map(|(peer, _)| peer.id).collect();
    runtime.peers.retain(|id, _| ids.contains(id));
}

fn seats_changed(journal: &mut EditJournal, runtime: &mut TrainRuntime) {
    match (
        journal.gameplay().trains.revision.checked_add(1),
        journal.gameplay().revision.checked_add(1),
    ) {
        (Some(revision), Some(game_revision)) => {
            journal.gameplay_mut().trains.revision = revision;
            journal.gameplay_mut().revision = game_revision;
        }
        _ => {
            // Occupancy is transient. Mandatory death/disconnect revocation must
            // still be sent when persisted counters cannot advance.
            for peer in runtime.peers.values_mut() {
                peer.sent = None;
            }
        }
    }
}
#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[test]
    fn mandatory_seat_cleanup_forces_a_fresh_snapshot_when_persisted_epochs_are_exhausted() {
        let mut journal = EditJournal::default();
        journal.gameplay_mut().revision = u64::MAX;
        journal.gameplay_mut().trains.revision = u64::MAX;
        let mut runtime = TrainRuntime::default();
        runtime.peers.insert(
            terra_core::PlayerId(1),
            TrainPeer {
                sent: Some(u64::MAX),
                ..Default::default()
            },
        );
        seats_changed(&mut journal, &mut runtime);
        assert!(runtime.peers[&terra_core::PlayerId(1)].sent.is_none());
        assert_eq!(journal.gameplay().revision, u64::MAX);
        assert_eq!(journal.gameplay().trains.revision, u64::MAX);
    }

    #[test]
    fn safe_exit_requires_loaded_clear_terrain_and_revalidates_before_consuming_the_intent() {
        use terra_core::{
            WoodSpecies, train_parts,
            trains::{TrainPart, VehicleKind},
        };
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut chunks = ChunkStore::default();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let owner_id = PlayerUuid::from_u128(210_718);
        let vehicle = Vehicle {
            id: 1,
            owner: owner_id,
            kind: VehicleKind::Passenger,
            anchor: [7, 5, 7],
            parts: [
                ([0, 0, 0], train_parts::CHASSIS),
                ([0, -1, 0], train_parts::BOGIE),
                ([0, -1, 1], train_parts::BOGIE),
                ([0, 1, 1], train_parts::FRAME),
                ([0, 1, 0], train_parts::SEAT),
                ([0, 0, -1], train_parts::COUPLER),
            ]
            .into_iter()
            .map(|(offset, material)| TrainPart {
                offset,
                material,
                facing: 0,
                wood: WoodSpecies::Oak,
            })
            .collect(),
            cargo: Default::default(),
            couplings: [None; 2],
            revision: 1,
        };
        assert!(vehicle.valid());
        let (feet, yaw) = vehicle.seat_pose(4).unwrap();
        let intent = ExitIntent {
            vehicle: 1,
            feet,
            yaw,
        };
        let mut journal = EditJournal::default();
        journal.gameplay_mut().trains.insert(vehicle).unwrap();
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            water: false,
            ..Default::default()
        })
        .unwrap();
        let properties = Default::default();
        let position = safe_exit(intent, &chunks, &journal, &terrain, &properties)
            .expect("open platform has a safe exit");
        assert!(terra_physics::character::space_clear_with_shapes(
            &chunks,
            &journal,
            &properties,
            position
        ));
        assert!((position.y - 3.9).abs() < 0.03);
        let mut seats = SeatedPlayers::default();
        seats.exits.insert(owner_id, intent);
        for x in 3..=11 {
            for y in 3..=10 {
                for z in 3..=11 {
                    chunks
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block([x, y, z], 3)
                        .unwrap();
                }
            }
        }
        assert!(
            seats
                .take_exit(owner_id, &chunks, &journal, &terrain, &properties)
                .is_none()
        );
        assert!(
            seats.has_exit(owner_id),
            "a newly blocked exit remains pending instead of teleporting into a wall"
        );
        chunks.loaded.clear();
        assert!(
            seats
                .take_exit(owner_id, &chunks, &journal, &terrain, &properties)
                .is_none()
        );
        assert!(seats.has_exit(owner_id));
    }
}
