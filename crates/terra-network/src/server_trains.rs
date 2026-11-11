//! Authoritative train actions, rail motion and compact private/public replication.
use super::*;
use crate::protocol::trains::{
    TrainAction, TrainCargoSnapshot, TrainCommand, TrainOutcome, TrainResult,
};
use std::collections::{BTreeMap, HashSet};
use terra_core::{
    GameMode, ItemKind, PlayerUuid, ToolKind,
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
    #[cfg(test)]
    pub(super) fn bind_for_test(&mut self, player: PlayerUuid, seat: SeatBinding) {
        self.bindings.insert(player, seat);
    }
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
    car.parts.iter().enumerate().any(|(index, _part)| {
        car.transformed_shapes(index)
            .into_iter()
            .any(|shape| shape.intersects(low, high))
    })
}
fn safe_exit(
    intent: ExitIntent,
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &terra_world::TerraGenerator,
    properties: &terra_core::block_properties::BlockPropertiesCatalog,
) -> Option<Vec3> {
    terra_physics::safe_train_exit(
        intent.vehicle,
        intent.feet,
        intent.yaw,
        chunks,
        journal,
        terrain,
        properties,
    )
}
#[derive(Resource, Default)]
pub(super) struct TrainRuntime {
    peers: HashMap<terra_core::PlayerId, TrainPeer>,
    rail_cache: terra_world::rails::NetworkCache,
    ticks: u64,
    retired_motions: Vec<terra_core::train_motion::TrainMotionFrame>,
    destroyed: Vec<u64>,
    fuel_updates: Vec<(PlayerUuid, TrainCargoSnapshot)>,
}
struct TrainPeer {
    request: u64,
    tokens: f64,
    updated: Instant,
    sent: Option<u64>,
    motion_tick: u64,
    motions: BTreeMap<u64, terra_core::train_motion::TrainMotionFrame>,
}
impl Default for TrainPeer {
    fn default() -> Self {
        Self {
            request: 0,
            tokens: 12.0,
            updated: Instant::now(),
            sent: None,
            motion_tick: 0,
            motions: BTreeMap::new(),
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
#[derive(Clone, Copy)]
enum ReachTarget {
    Cell([i64; 3]),
    Part { vehicle: u64, index: u16 },
}
fn reach_point(
    peer: &Peer,
    world: &ControlWorld,
    point: [f64; 3],
    target: ReachTarget,
) -> Result<(), TrainError> {
    if point
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 990_000.5)
    {
        return Err(TrainError::Invalid);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| TrainError::OutOfReach)?;
    let eye = (body.translation + Vec3::Y * PLAYER_EYE_OFFSET)
        .to_array()
        .map(f64::from);
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
        && match target {
            ReachTarget::Part { vehicle, index } => hit.vehicle != vehicle || hit.part != index,
            ReachTarget::Cell(cell) => {
                world
                    .journal
                    .gameplay()
                    .trains
                    .vehicles
                    .get(&hit.vehicle)
                    .and_then(|car| car.world_cell(usize::from(hit.part)))
                    != Some(cell)
            }
        }
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
        && !matches!(target, ReachTarget::Cell(cell) if hit.position == cell)
        && hit.distance + 0.05 < distance
    {
        return Err(TrainError::OutOfReach);
    }
    Ok(())
}
fn reach(peer: &Peer, world: &ControlWorld, cell: [i64; 3]) -> Result<(), TrainError> {
    if cell.iter().any(|value| value.unsigned_abs() > 990_000) {
        return Err(TrainError::Invalid);
    }
    reach_point(
        peer,
        world,
        cell.map(|value| value as f64 + 0.5),
        ReachTarget::Cell(cell),
    )
}
/// Mounted profiles retain fractional height and arbitrary rigid rotation.
/// An exposed authored shape is reachable even if its quantized cell center is hidden.
fn reach_part(
    peer: &Peer,
    world: &ControlWorld,
    car: &Vehicle,
    index: u16,
) -> Result<(), TrainError> {
    let part = car
        .parts
        .get(usize::from(index))
        .ok_or(TrainError::Missing)?;
    let target = ReachTarget::Part {
        vehicle: car.id,
        index,
    };
    for shape in car.part_shapes(usize::from(index)) {
        let point = car.world_point(std::array::from_fn(|axis| {
            f64::from(part.offset[axis]) + (shape.minimum[axis] + shape.maximum[axis]) * 0.5
        }));
        if reach_point(peer, world, point, target).is_ok() {
            return Ok(());
        }
    }
    Err(TrainError::OutOfReach)
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
        let cell = vehicle.world_cell(index).unwrap();
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
    let building = matches!(&command.action, TrainAction::Build { .. });
    let destroying = if let TrainAction::Destroy { vehicle, .. } = &command.action {
        Some(*vehicle)
    } else {
        None
    };
    let retiring = if let TrainAction::Dismantle { vehicle, .. } = &command.action {
        world
            .journal
            .gameplay()
            .trains
            .vehicles
            .get(vehicle)
            .map(|car| terra_core::train_motion::TrainMotionFrame {
                steam: car.steam,
                id: car.id,
                revision: car.revision,
                motion: car.motion,
            })
    } else {
        None
    };
    let result = runtime
        .peers
        .entry(peer.id)
        .or_default()
        .accept(command.request)
        .and_then(|()| {
            if destroying.is_some() && runtime.destroyed.len() >= terra_core::trains::MAX_VEHICLES {
                Err(TrainError::RateLimited)
            } else {
                apply(command.action, peer, world, seated)
            }
        });
    let outcome = result.unwrap_or_else(TrainOutcome::Rejected);
    if let Some(id) = destroying
        && matches!(outcome, TrainOutcome::Changed { .. })
    {
        runtime.destroyed.push(id);
    }
    if let Some(frame) = retiring
        && matches!(outcome, TrainOutcome::Changed { .. })
    {
        runtime.retired_motions.push(frame);
    }
    if (matches!(outcome, TrainOutcome::Cargo(_)) || building)
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
        TrainAction::RailBuilderAutoload {
            vehicle: id,
            active,
        } => {
            let car = vehicle(peer, world, id)?;
            if !car.motion.stopped() {
                return Err(TrainError::Busy);
            }
            if seated.binding(peer.uuid).is_none_or(|seat| {
                seat.vehicle != id
                    || car
                        .parts
                        .get(usize::from(seat.part))
                        .is_none_or(|part| part.material != terra_core::train_parts::CAB)
            }) {
                return Err(TrainError::OutOfReach);
            }
            if active {
                terra_world::rails::builder::validate_consist(
                    &world.journal.gameplay().trains,
                    &car,
                )?;
            }
            let revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            touch(world)?;
            let car = world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap();
            car.motion.rail_builder_autoload = active;
            car.revision = revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::RailClearance { vehicle: id, size } => {
            if !size.valid() {
                return Err(TrainError::Limit);
            }
            let mut car = vehicle(peer, world, id)?;
            if !car.motion.stopped() {
                return Err(TrainError::Busy);
            }
            if seated.binding(peer.uuid).is_none_or(|seat| {
                seat.vehicle != id
                    || car
                        .parts
                        .get(usize::from(seat.part))
                        .is_none_or(|part| part.material != terra_core::train_parts::CAB)
            }) {
                return Err(TrainError::OutOfReach);
            }
            car.motion.rail_clearance = size;
            terra_world::rails::builder::validate_consist(&world.journal.gameplay().trains, &car)?;
            let revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            touch(world)?;
            let car = world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap();
            car.motion.rail_clearance = size;
            car.motion.brake = true;
            car.motion.throttle = 0;
            car.revision = revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::RailBuilder {
            vehicle: id,
            active,
        } => {
            let car = vehicle(peer, world, id)?;
            if !car.motion.stopped() {
                return Err(TrainError::Busy);
            }
            if seated.binding(peer.uuid).is_none_or(|seat| {
                seat.vehicle != id
                    || car
                        .parts
                        .get(usize::from(seat.part))
                        .is_none_or(|part| part.material != terra_core::train_parts::CAB)
            }) {
                return Err(TrainError::OutOfReach);
            }
            if active {
                terra_world::rails::builder::validate_consist(
                    &world.journal.gameplay().trains,
                    &car,
                )?;
            }
            let revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            touch(world)?;
            let car = world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap();
            car.motion.rail_builder_active = active;
            car.motion.brake = true;
            car.motion.throttle = 0;
            car.revision = revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Drive {
            vehicle: id,
            throttle,
            brake,
        } => {
            if !(-6..=6).contains(&throttle) {
                return Err(TrainError::Invalid);
            }
            let binding = seated
                .binding(peer.uuid)
                .filter(|seat| seat.vehicle == id)
                .ok_or(TrainError::NotOwner)?;
            let car = vehicle(peer, world, id)?;
            if car
                .parts
                .get(usize::from(binding.part))
                .is_none_or(|part| part.material != terra_core::train_parts::CAB)
            {
                return Err(TrainError::NotOwner);
            }
            if !car.parts.iter().any(|part| part.material == 218) {
                return Err(TrainError::Incomplete);
            }
            let logical = throttle * car.driver_direction(usize::from(binding.part));
            if !(-2..=terra_core::train_power::max_gear(&car.parts, car.steam)).contains(&logical) {
                return Err(TrainError::GearLocked);
            }
            if car.motion.rail_builder_active
                && throttle * super::train_builder::direction(&car) > 0
                && !brake
            {
                if car.revision > u64::MAX - 2
                    || world.journal.gameplay().trains.revision > u64::MAX - 2
                    || world.journal.gameplay().revision > u64::MAX - 1024
                {
                    return Err(TrainError::Overflow);
                }
                let mut graph = terra_world::rails::NetworkCache::default();
                graph
                    .refresh(&world.journal)
                    .map_err(|_| TrainError::Unsupported)?;
                super::train_builder::step(&car, peer, world, graph.graph())?;
            }
            let next_revision = world.journal.gameplay().trains.vehicles[&id]
                .revision
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;

            touch(world)?;
            let car = world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap();
            car.motion.throttle = throttle;
            car.motion.brake = brake;
            car.motion.fuel_available = terra_world::train_motion::fuel_available(car);
            car.revision = next_revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::RotateEmpty {
            station: cell,
            anchor,
            clockwise,
        } => {
            station(peer, world, cell)?;
            if (0..3).any(|axis| anchor[axis].abs_diff(cell[axis]) > 32) {
                return Err(TrainError::OutOfReach);
            }
            let piece = terra_world::rails::owner_at(&world.journal, anchor)
                .filter(|piece| piece.material == terra_core::rails::TURNTABLE)
                .ok_or(TrainError::Unsupported)?;
            let center = [
                piece.anchor[0] as f64 + 0.5,
                piece.anchor[1] as f64,
                piece.anchor[2] as f64 + 0.5,
            ];
            if world
                .journal
                .gameplay()
                .trains
                .vehicles
                .values()
                .any(|car| {
                    let bounds = car.model_bounds();
                    let x = center[0].clamp(bounds[0][0], bounds[1][0]);
                    let z = center[2].clamp(bounds[0][2], bounds[1][2]);
                    bounds[0][1] < center[1] + 1.0
                        && bounds[1][1] > center[1]
                        && (x - center[0]).hypot(z - center[2])
                            < terra_core::rails::TURNTABLE_RADIUS
                })
                || world.bodies.iter().any(|body| {
                    let feet = body.translation.y - terra_physics::character::PLAYER_HALF_HEIGHT;
                    f64::from(feet) < center[1] + 1.5
                        && f64::from(
                            body.translation.y + terra_physics::character::PLAYER_HALF_HEIGHT,
                        ) > center[1]
                        && (f64::from(body.translation.x) - center[0])
                            .hypot(f64::from(body.translation.z) - center[2])
                            < terra_core::rails::TURNTABLE_RADIUS
                })
            {
                return Err(TrainError::Occupied);
            }
            let plan = terra_world::rails::turntable_plan(
                &world.chunks,
                &world.journal,
                anchor,
                clockwise,
            )
            .map_err(|_| TrainError::Unsupported)?;
            if plan
                .edits
                .iter()
                .any(|edit| !permitted(peer, world, edit.position, KingdomPermission::Build))
            {
                return Err(TrainError::Protected);
            }
            terra_world::rails::commit(&mut world.chunks, &mut world.journal, &plan)
                .map_err(|_| TrainError::Stale)?;
            Ok(TrainOutcome::Changed { vehicle: None })
        }
        TrainAction::Turntable {
            vehicle: id,
            clockwise,
            station: editing_station,
        } => {
            let car = vehicle(peer, world, id)?;
            if seated
                .binding(peer.uuid)
                .is_none_or(|binding| binding.vehicle != id)
            {
                if let Some(cell) = editing_station {
                    station(peer, world, cell)?;
                    if (0..3)
                        .any(|axis| (car.model_origin()[axis] - cell[axis] as f64).abs() > 48.0)
                    {
                        return Err(TrainError::OutOfReach);
                    }
                } else {
                    if !(0..car.parts.len())
                        .any(|index| reach_part(peer, world, &car, index as u16).is_ok())
                    {
                        return Err(TrainError::OutOfReach);
                    }
                }
            }
            let network =
                terra_world::rails::network(&world.journal).map_err(|_| TrainError::Unsupported)?;
            let (plan, next, swept) = terra_world::train_motion::turntable(
                &world.chunks,
                &world.journal,
                &world.block_properties,
                &network,
                &car,
                clockwise,
            )?;
            for edit in &plan.edits {
                if !permitted(peer, world, edit.position, KingdomPermission::Build) {
                    return Err(TrainError::Protected);
                }
            }
            let half = Vec3::new(
                terra_physics::character::PLAYER_RADIUS,
                terra_physics::character::PLAYER_HALF_HEIGHT,
                terra_physics::character::PLAYER_RADIUS,
            );
            for shape in &swept {
                if world.bodies.iter().any(|body| {
                    !seated.poses.iter().any(|(uuid, (position, _))| {
                        seated.binding(*uuid).is_some_and(|seat| seat.vehicle == id)
                            && position.distance(body.translation) < 0.01
                    }) && shape.intersects(
                        (body.translation - half).to_array().map(f64::from),
                        (body.translation + half).to_array().map(f64::from),
                    )
                }) || world
                    .journal
                    .gameplay()
                    .trains
                    .vehicles
                    .values()
                    .any(|other| {
                        other.id != id && overlaps_vehicle(other, shape.minimum, shape.maximum)
                    })
                {
                    return Err(TrainError::Occupied);
                }
                for x in shape.minimum[0].floor() as i64..=shape.maximum[0].floor() as i64 {
                    for z in shape.minimum[2].floor() as i64..=shape.maximum[2].floor() as i64 {
                        if !permitted(
                            peer,
                            world,
                            [x, shape.minimum[1].floor() as i64, z],
                            KingdomPermission::Inventories,
                        ) {
                            return Err(TrainError::Protected);
                        }
                    }
                }
            }
            if world.journal.gameplay().revision == u64::MAX
                || world.journal.gameplay().trains.revision == u64::MAX
            {
                return Err(TrainError::Overflow);
            }
            terra_world::rails::commit(&mut world.chunks, &mut world.journal, &plan)
                .map_err(|_| TrainError::Stale)?;
            touch(world)?;
            world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .insert(id, next);
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Name {
            vehicle: id,
            name,
            station: editing_station,
        } => {
            if !terra_core::trains::valid_name(&name) {
                return Err(TrainError::Invalid);
            }
            let car = vehicle(peer, world, id)?;
            if seated
                .binding(peer.uuid)
                .is_none_or(|binding| binding.vehicle != id)
            {
                if let Some(cell) = editing_station {
                    station(peer, world, cell)?;
                    if (0..3)
                        .any(|axis| (car.model_origin()[axis] - cell[axis] as f64).abs() > 48.0)
                    {
                        return Err(TrainError::OutOfReach);
                    }
                } else if !(0..car.parts.len())
                    .any(|index| reach_part(peer, world, &car, index as u16).is_ok())
                {
                    return Err(TrainError::OutOfReach);
                }
            }
            let revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            touch(world)?;
            let car = world
                .journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap();
            car.name = name.trim().to_string();
            car.revision = revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Inspect { station: cell } => {
            station(peer, world, cell)?;
            Ok(TrainOutcome::Inspected)
        }
        TrainAction::Build {
            station: cell,
            chassis,
            facing,
            kind,
            wheelbase,
            width,
            player_revision,
        } => {
            station(peer, world, cell)?;
            if world
                .journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .map_or(0, |bag| bag.revision)
                != player_revision
            {
                return Err(TrainError::Stale);
            }
            let plan = terra_world::trains::plan_blueprint(
                &world.chunks,
                &world.journal,
                terra_world::trains::BuildRequest {
                    actor: peer.uuid,
                    station: cell,
                    chassis,
                    facing,
                    kind,
                    wheelbase,
                    width,
                    creative: peer.game_mode(&world.config) == GameMode::Creative,
                },
            )?;
            let half = Vec3::new(
                terra_physics::character::PLAYER_RADIUS,
                terra_physics::character::PLAYER_HALF_HEIGHT,
                terra_physics::character::PLAYER_RADIUS,
            );
            for (part, _) in &plan.cells {
                for bounds in terra_core::train_parts::boxes(part.material, part.facing) {
                    let low =
                        std::array::from_fn(|axis| part.cell[axis] as f64 + bounds.minimum[axis]);
                    let high =
                        std::array::from_fn(|axis| part.cell[axis] as f64 + bounds.maximum[axis]);
                    if world.bodies.iter().any(|body| {
                        terra_core::block_shapes::ShapeBox {
                            minimum: low,
                            maximum: high,
                            hinged: false,
                        }
                        .intersects(
                            (body.translation - half).to_array().map(f64::from),
                            (body.translation + half).to_array().map(f64::from),
                        )
                    }) || world
                        .journal
                        .gameplay()
                        .trains
                        .vehicles
                        .values()
                        .any(|car| overlaps_vehicle(car, low, high))
                    {
                        return Err(TrainError::Occupied);
                    }
                }
            }
            terra_world::trains::commit_blueprint(&mut world.chunks, &mut world.journal, &plan)?;
            Ok(TrainOutcome::Changed { vehicle: None })
        }
        TrainAction::Assemble {
            station: cell,
            start,
            kind,
            revision,
        } => {
            station(peer, world, cell)?;
            check_revision(world, revision)?;
            let plan = terra_world::trains::plan_assembly_with_properties(
                &world.chunks,
                &world.journal,
                peer.uuid,
                cell,
                start,
                kind,
                &world.block_properties,
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
            let selected = vehicle(peer, world, id)?;
            if selected.revision != revision {
                return Err(TrainError::Stale);
            }
            if seated.bindings.values().any(|seat| seat.vehicle == id) {
                return Err(TrainError::Busy);
            }
            let mut refunded_water = None;
            let buckets =
                (selected.steam.water / terra_core::train_power::BUCKET_WATER).floor() as u16;
            if buckets > 0 && peer.game_mode(&world.config) != GameMode::Creative {
                let mut bag = world
                    .journal
                    .gameplay()
                    .inventories
                    .get(&peer.uuid)
                    .ok_or(TrainError::Missing)?
                    .clone();
                if bag.insert(ItemKind::Tool(ToolKind::WaterBucket), buckets) != 0 {
                    return Err(TrainError::Limit);
                }
                refunded_water = Some(bag);
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
                    plan.vehicle.part_shapes(index).into_iter().any(|shape| {
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
            if let Some(bag) = refunded_water {
                world
                    .journal
                    .gameplay_mut()
                    .inventories
                    .insert(peer.uuid, bag);
            }
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::Destroy {
            vehicle: id,
            revision,
        } => {
            let selected = vehicle(peer, world, id)?;
            if selected.revision != revision {
                return Err(TrainError::Stale);
            }
            if !selected.motion.stopped() || seated.bindings.values().any(|seat| seat.vehicle == id)
            {
                return Err(TrainError::Busy);
            }
            // Deletion is deliberately separate from Montage: no blocks or private
            // inventories are restored, and the UI explicitly confirms that loss.
            if !(0..selected.parts.len())
                .any(|index| reach_part(peer, world, &selected, index as u16).is_ok())
            {
                return Err(TrainError::OutOfReach);
            }
            if (0..selected.parts.len()).any(|index| {
                !permitted(
                    peer,
                    world,
                    selected.world_cell(index).unwrap(),
                    KingdomPermission::Build,
                )
            }) {
                return Err(TrainError::Protected);
            }
            let revision = world
                .journal
                .gameplay()
                .revision
                .checked_add(1)
                .ok_or(TrainError::Overflow)?;
            // Registry removal preflights owner, couplings and its own epoch before
            // changing anything. The surrounding gameplay epoch advances only on success.
            world.journal.gameplay_mut().trains.remove(peer.uuid, id)?;
            world.journal.gameplay_mut().revision = revision;
            Ok(TrainOutcome::Changed { vehicle: None })
        }
        TrainAction::Couple {
            station: cell,
            first,
            first_end,
            second,
            second_end,
            revision,
        } => {
            if let Some(cell) = cell {
                station(peer, world, cell)?;
            }
            check_revision(world, revision)?;
            let mounted_pair = seated
                .binding(peer.uuid)
                .is_some_and(|seat| seat.vehicle == first || seat.vehicle == second);
            for id in [first, second] {
                let car = vehicle(peer, world, id)?;
                if !car.motion.stopped() {
                    return Err(TrainError::Busy);
                }
                if let Some(cell) = cell {
                    if car.world_cell(0).is_none_or(|position| {
                        (0..3).any(|axis| position[axis].abs_diff(cell[axis]) > 64)
                    }) {
                        return Err(TrainError::OutOfReach);
                    }
                } else if !mounted_pair
                    && !(0..car.parts.len())
                        .any(|index| reach_part(peer, world, &car, index as u16).is_ok())
                {
                    return Err(TrainError::OutOfReach);
                }
                if (0..car.parts.len()).any(|index| {
                    !permitted(
                        peer,
                        world,
                        car.world_cell(index).unwrap(),
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
            if let Some(cell) = cell {
                station(peer, world, cell)?;
            }
            check_revision(world, revision)?;
            let car = vehicle(peer, world, id)?;
            let link = car.couplings[end.index()].ok_or(TrainError::Missing)?;
            let other = vehicle(peer, world, link.vehicle)?;
            let mounted_pair = seated
                .binding(peer.uuid)
                .is_some_and(|seat| seat.vehicle == car.id || seat.vehicle == other.id);
            for car in [car, other] {
                if !car.motion.stopped() {
                    return Err(TrainError::Busy);
                }
                if let Some(cell) = cell {
                    if car.world_cell(0).is_none_or(|position| {
                        (0..3).any(|axis| position[axis].abs_diff(cell[axis]) > 64)
                    }) {
                        return Err(TrainError::OutOfReach);
                    }
                } else if !mounted_pair
                    && !(0..car.parts.len())
                        .any(|index| reach_part(peer, world, &car, index as u16).is_ok())
                {
                    return Err(TrainError::OutOfReach);
                }
                if (0..car.parts.len()).any(|index| {
                    !permitted(
                        peer,
                        world,
                        car.world_cell(index).unwrap(),
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
        TrainAction::FillWater {
            vehicle: id,
            player_revision,
        } => {
            let car = vehicle(peer, world, id)?;
            let tank = car
                .parts
                .iter()
                .position(|p| p.material == 254)
                .ok_or(TrainError::Incomplete)?;
            if !car.motion.stopped() {
                return Err(TrainError::Busy);
            }
            if seated
                .binding(peer.uuid)
                .is_none_or(|seat| seat.vehicle != id)
            {
                reach_part(peer, world, &car, tank as u16)?;
            }
            let mut bag = world
                .journal
                .gameplay()
                .inventories
                .get(&peer.uuid)
                .ok_or(TrainError::Missing)?
                .clone();
            if bag.revision != player_revision {
                return Err(TrainError::Stale);
            }
            let creative = peer.game_mode(&world.config) == GameMode::Creative;
            if !creative
                && car.steam.water + terra_core::train_power::BUCKET_WATER
                    > terra_core::train_power::WATER_CAPACITY
            {
                return Err(TrainError::Limit);
            }
            if !creative {
                if !bag.take_one(ItemKind::Tool(ToolKind::WaterBucket)) {
                    return Err(TrainError::MissingMaterials);
                }
                if bag.insert(ItemKind::Tool(ToolKind::Bucket), 1) != 0 {
                    return Err(TrainError::Limit);
                }
            }
            let revision = car.revision.checked_add(1).ok_or(TrainError::Overflow)?;
            touch(world)?;
            let game = world.journal.gameplay_mut();
            game.inventories.insert(peer.uuid, bag);
            let car = game.trains.vehicles.get_mut(&id).unwrap();
            car.steam.water = if creative {
                terra_core::train_power::WATER_CAPACITY
            } else {
                car.steam.water + terra_core::train_power::BUCKET_WATER
            };
            car.revision = revision;
            Ok(TrainOutcome::Changed { vehicle: Some(id) })
        }
        TrainAction::OpenCargo { vehicle: id, part } => {
            let car = vehicle(peer, world, id)?;
            if seated
                .binding(peer.uuid)
                .is_none_or(|seat| seat.vehicle != id)
            {
                reach_part(peer, world, &car, part)?;
            }
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
            if seated
                .binding(peer.uuid)
                .is_none_or(|seat| seat.vehicle != id)
            {
                reach_part(peer, world, &car, part)?;
            }
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
            if car
                .parts
                .get(usize::from(part))
                .is_some_and(|part| part.material == 218)
                && inventories[0].slots.iter().any(|stack| {
                    stack.count > 0 && terra_core::furnace::fuel_seconds(stack.item).is_none()
                })
            {
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
            car.motion.fuel_available = terra_world::train_motion::fuel_available(car);
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
            if seated
                .binding(peer.uuid)
                .is_none_or(|seat| seat.vehicle != id)
            {
                reach_part(peer, world, &car, part)?;
            }
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
                            car.world_cell(index).is_some_and(|cell| {
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
    let tick = runtime.ticks;
    let motion_interval = u64::from(config.server.tick_rate.div_ceil(60));
    let frames = registry
        .vehicles
        .values()
        .map(|car| terra_core::train_motion::TrainMotionFrame {
            steam: car.steam,
            id: car.id,
            revision: car.revision,
            motion: car.motion,
        })
        .collect::<Vec<_>>();
    let retired = std::mem::take(&mut runtime.retired_motions);
    let destroyed = std::mem::take(&mut runtime.destroyed);
    let fuel_updates = std::mem::take(&mut runtime.fuel_updates);
    for (peer, mut sender) in &mut peers {
        if !peer.synced {
            continue;
        }
        let state = runtime.peers.entry(peer.id).or_default();
        let already_joined = state.sent.is_some();
        if state.sent != Some(registry.revision) {
            if already_joined && !retired.is_empty() {
                sender.send::<ControlChannel>(ServerControl::TrainMotions(retired.clone()));
            }
            let mut snapshot = registry.snapshot(seated.bindings.values().copied().collect());
            snapshot.destroyed = destroyed.clone();
            sender.send::<ControlChannel>(ServerControl::Trains(Box::new(snapshot)));
            state.sent = Some(registry.revision);
            state.motion_tick = tick;
            state.motions = frames.iter().map(|frame| (frame.id, *frame)).collect();
        }
        if already_joined && tick.saturating_sub(state.motion_tick) >= motion_interval {
            let changed = frames
                .iter()
                .filter(|frame| state.motions.get(&frame.id) != Some(*frame))
                .copied()
                .collect::<Vec<_>>();
            if !changed.is_empty() {
                sender.send::<ControlChannel>(ServerControl::TrainMotions(changed));
                state.motions = frames.iter().map(|frame| (frame.id, *frame)).collect();
            }
            state.motion_tick = tick;
        }
        if already_joined {
            for (owner, cargo) in &fuel_updates {
                if *owner == peer.uuid && registry.vehicles.contains_key(&cargo.vehicle) {
                    sender.send::<ControlChannel>(ServerControl::TrainEngineFuel(cargo.clone()));
                }
            }
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
/// Fixed-step authority advances rail cursors and atomically publishes whole consists.
/// Compact pose packets are independent of structural snapshots and geometry epochs.
#[allow(clippy::too_many_arguments)] // Independent Bevy system resources/queries.
pub(super) fn drive(
    time: Res<Time<Fixed>>,
    chunks: Res<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mut runtime: ResMut<TrainRuntime>,
    mut seated: ResMut<SeatedPlayers>,
    peers: Query<&Peer, With<Connected>>,
    mut impacts: ResMut<super::train_damage::Impacts>,
) {
    use terra_core::train_motion::StopReason;
    impacts.0.clear();
    runtime.ticks = runtime.ticks.wrapping_add(1);
    let dt = time.delta_secs_f64().min(0.1);
    if runtime.rail_cache.refresh(&journal).is_err() {
        return;
    }
    let mut handled = HashSet::new();
    let mut plans = Vec::new();
    let mut planned_sweeps: Vec<(u64, [[f64; 3]; 2])> = Vec::new();
    let mut fuel_updates = Vec::new();
    let registry = &journal.gameplay().trains;
    let mut ids = seated
        .bindings
        .values()
        .filter_map(|seat| {
            let car = registry.vehicles.get(&seat.vehicle)?;
            car.parts
                .get(usize::from(seat.part))
                .is_some_and(|part| part.material == terra_core::train_parts::CAB)
                .then_some(car.id)
        })
        .collect::<Vec<_>>();
    ids.extend(
        registry
            .vehicles
            .values()
            .filter(|car| car.kind == terra_core::trains::VehicleKind::Locomotive)
            .map(|car| car.id),
    );
    ids.extend(registry.vehicles.keys().copied());
    for id in ids {
        if handled.contains(&id) {
            continue;
        }
        let Some(original) = registry.vehicles.get(&id) else {
            continue;
        };
        let Ok(group) = registry.consist(id) else {
            continue;
        };
        handled.extend(group.iter().copied());
        let active = group.iter().any(|member| {
            let car = &registry.vehicles[member];
            (terra_core::train_power::steam_installed(&car.parts)
                && (car.steam.pressure_bar > 0.0
                    || car.steam.water > 0.0 && terra_world::train_motion::fuel_available(car)))
                || registry.vehicles[member].motion.speed_mps.abs() > 0.001
                || (registry.vehicles[member].motion.throttle != 0
                    && !registry.vehicles[member].motion.brake)
        });
        if !active {
            continue;
        }
        let driver = seated
            .bindings
            .iter()
            .find(|(_, seat)| {
                seat.vehicle == id
                    && original
                        .parts
                        .get(usize::from(seat.part))
                        .is_some_and(|part| part.material == terra_core::train_parts::CAB)
            })
            .map(|(player, _)| *player);
        let connected = driver.is_some_and(|uuid| {
            peers.iter().any(|peer| {
                peer.uuid == uuid
                    && peer.synced
                    && !peer.vitals.dead()
                    && peer.chat.teleport.is_none()
            })
        });
        let mut source = original.clone();
        if !connected {
            source.motion.throttle = 0;
            source.motion.brake = true;
        }
        let mut candidates = Vec::new();
        let base_speed = terra_core::train_power::gear_speed(
            source.motion.throttle,
            terra_core::train_power::max_gear(&source.parts, source.steam),
        )
        .max(4.0);
        let consist_limit = group.iter().try_fold(f64::INFINITY, |limit, id| {
            let car = &registry.vehicles[id];
            terra_world::train_motion::speed_limit(
                &chunks,
                &journal,
                runtime.rail_cache.graph(),
                car,
                base_speed,
                if car.motion.speed_mps.abs() > 0.001 {
                    car.motion.speed_mps.signum()
                } else {
                    f64::from(source.motion.throttle)
                },
            )
            .map(|next| limit.min(next))
        });
        let result = consist_limit.and_then(|limit| {
            terra_world::train_motion::advance_with_speed_limit(
                &chunks,
                &journal,
                runtime.rail_cache.graph(),
                &source,
                dt,
                if source.motion.rail_builder_active {
                    limit.min(terra_world::rails::builder::SPEED)
                } else {
                    limit
                },
            )
        });
        let mut failure = None;
        let mut candidate_impacts = Vec::new();
        match result {
            Ok(leader) => {
                let meters = leader.motion.wheel_distance_m - original.motion.wheel_distance_m;
                let speed = leader.motion.speed_mps;
                candidates.push(leader);
                let mut orientations = BTreeMap::from([(id, 1.0_f64)]);
                let mut queue = vec![id];
                while let Some(current) = queue.pop() {
                    let car = &registry.vehicles[&current];
                    for (end, link) in car.couplings.iter().enumerate() {
                        if let Some(link) = link
                            && !orientations.contains_key(&link.vehicle)
                        {
                            let orientation = orientations[&current]
                                * if end == link.end.index() { -1.0 } else { 1.0 };
                            orientations.insert(link.vehicle, orientation);
                            queue.push(link.vehicle);
                        }
                    }
                }
                for member in group.iter().copied().filter(|member| *member != id) {
                    let orientation = orientations.get(&member).copied().unwrap_or(1.0);
                    match terra_world::train_motion::follow(
                        &chunks,
                        &journal,
                        runtime.rail_cache.graph(),
                        &registry.vehicles[&member],
                        meters * orientation,
                        speed * orientation,
                    ) {
                        Ok(car) => candidates.push(car),
                        Err(reason) => {
                            failure = Some(reason);
                            break;
                        }
                    }
                }
            }
            Err(reason) => failure = Some(reason),
        }
        // Very tight curves can stretch the chord between two rigid car bodies.
        // Keep both rail contacts and reject that step before a joint separates.
        if failure.is_none() {
            for car in &candidates {
                for (end, link) in car.couplings.iter().enumerate() {
                    let Some(link) = link.filter(|link| car.id < link.vehicle) else {
                        continue;
                    };
                    let Some(other) = candidates.iter().find(|other| other.id == link.vehicle)
                    else {
                        failure = Some(StopReason::InvalidTrack);
                        break;
                    };
                    let end = if end == 0 {
                        terra_core::trains::CouplerEnd::Front
                    } else {
                        terra_core::trains::CouplerEnd::Rear
                    };
                    let Some((a, b)) = car.coupler_point(end).zip(other.coupler_point(link.end))
                    else {
                        failure = Some(StopReason::InvalidTrack);
                        break;
                    };
                    let Some((previous_a, previous_b)) = registry.vehicles[&car.id]
                        .coupler_point(end)
                        .zip(registry.vehicles[&other.id].coupler_point(link.end))
                    else {
                        failure = Some(StopReason::InvalidTrack);
                        break;
                    };
                    let rest = (0..3)
                        .map(|axis| (previous_a[axis] - previous_b[axis]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    let actual = (0..3)
                        .map(|axis| (a[axis] - b[axis]).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    if actual > rest + 0.5 || !(0.25..=2.5).contains(&actual) {
                        failure = Some(StopReason::InvalidTrack);
                        break;
                    }
                }
                if failure.is_some() {
                    break;
                }
            }
        }
        if failure.is_none()
            && let Err(reason) = terra_world::train_motion::consist_clear(&candidates)
        {
            failure = Some(reason);
        }
        if failure.is_none() {
            for car in &candidates {
                let swept = match terra_world::train_motion::clear_swept(
                    &chunks,
                    &journal,
                    &properties,
                    &registry.vehicles[&car.id],
                    car,
                ) {
                    Ok(shapes) => shapes,
                    Err(reason) => {
                        failure = Some(reason);
                        break;
                    }
                };
                if (0..car.parts.len()).any(|index| {
                    car.world_cell(index).is_none_or(|cell| {
                        !terra_world::kingdom_access::can_use(
                            &journal.gameplay().kingdoms,
                            &chunks,
                            car.owner,
                            KingdomPermission::Build,
                            cell,
                        )
                    })
                }) {
                    failure = Some(StopReason::Obstructed);
                    break;
                }
                let previous_bounds = registry.vehicles[&car.id].model_bounds();
                let current_bounds = car.model_bounds();
                let bounds: [[f64; 3]; 2] = [
                    std::array::from_fn(|axis| {
                        previous_bounds[0][axis].min(current_bounds[0][axis])
                    }),
                    std::array::from_fn(|axis| {
                        previous_bounds[1][axis].max(current_bounds[1][axis])
                    }),
                ];
                let overlaps = |other: [[f64; 3]; 2]| {
                    (0..3).all(|axis| {
                        bounds[0][axis] < other[1][axis] && bounds[1][axis] > other[0][axis]
                    })
                };
                if registry
                    .vehicles
                    .values()
                    .any(|other| !group.contains(&other.id) && overlaps(other.model_bounds()))
                    || planned_sweeps
                        .iter()
                        .any(|(id, bounds)| !group.contains(id) && overlaps(*bounds))
                {
                    failure = Some(StopReason::Obstructed);
                    break;
                }
                if let Some(impact) =
                    super::train_damage::Impact::moving(&registry.vehicles[&car.id], car, swept)
                {
                    candidate_impacts.push(impact);
                }
            }
        }
        if let Some(reason) = failure {
            candidates = group
                .iter()
                .map(|member| {
                    let mut car = registry.vehicles[member].clone();
                    car.motion.speed_mps = 0.0;
                    car.motion.brake = true;
                    car.motion.throttle = 0;
                    car.motion.stop_reason = reason;
                    car
                })
                .collect();
        }
        if failure.is_none() {
            impacts.0.extend(candidate_impacts);
        }
        for mut car in candidates {
            let old = &registry.vehicles[&car.id];
            let before = old.model_bounds();
            let after = car.model_bounds();
            planned_sweeps.push((
                car.id,
                [
                    std::array::from_fn(|i| before[0][i].min(after[0][i])),
                    std::array::from_fn(|i| before[1][i].max(after[1][i])),
                ],
            ));
            if car.cargo != old.cargo {
                car.revision = car.revision.saturating_add(1);
                for (part, inventory) in &car.cargo {
                    if car.parts[usize::from(*part)].material == 218
                        && old.cargo.get(part) != Some(inventory)
                    {
                        fuel_updates.push((
                            car.owner,
                            TrainCargoSnapshot {
                                vehicle: car.id,
                                part: *part,
                                revision: car.revision,
                                inventory: inventory.clone(),
                            },
                        ));
                    }
                }
            }
            if car != *old {
                plans.push(car);
            }
        }
    }
    if !plans.is_empty() {
        let game = journal.gameplay_mut();
        for car in plans {
            game.trains.vehicles.insert(car.id, car);
        }
        game.revision = game.revision.saturating_add(1);
    }
    runtime.fuel_updates.extend(fuel_updates);
    for (player, seat) in &seated.bindings.clone() {
        if let Some((position, yaw)) = journal
            .gameplay()
            .trains
            .vehicles
            .get(&seat.vehicle)
            .and_then(|car| car.seat_pose(usize::from(seat.part)))
        {
            seated.poses.insert(
                *player,
                (
                    Vec3::from_array(position.map(|v| v as f32))
                        + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
                    yaw,
                ),
            );
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
            name: String::new(),
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
            blocks: Default::default(),
            payloads: Default::default(),
            couplings: [None; 2],
            revision: 1,

            motion: Default::default(),
            burn_seconds: 0.0,
            steam: Default::default(),
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
    #[test]
    fn tall_three_wide_cab_can_dismount_to_ground_below_the_boiler() {
        use terra_core::{
            train_blueprint,
            trains::{TrainPart, VehicleKind},
        };
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut chunks = ChunkStore::default();
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let guide = train_blueprint::building_template_with_width(
            [7, 4, 5],
            0,
            VehicleKind::Locomotive,
            4,
            3,
        );
        let mut car = Vehicle {
            name: String::new(),
            id: 1,
            owner: PlayerUuid::from_u128(222),
            kind: VehicleKind::Locomotive,
            anchor: [7, 4, 5],
            parts: guide
                .iter()
                .map(|p| TrainPart {
                    offset: std::array::from_fn(|i| (p.cell[i] - [7, 4, 5][i]) as i16),
                    material: p.material,
                    facing: p.facing,
                    wood: Default::default(),
                })
                .collect(),
            cargo: Default::default(),
            blocks: Default::default(),
            payloads: Default::default(),
            motion: Default::default(),
            burn_seconds: 0.0,
            steam: Default::default(),
            couplings: [None; 2],
            revision: 1,
        };
        for index in car.cargo_indices().collect::<Vec<_>>() {
            car.cargo.insert(index, terra_core::Inventory::default());
        }
        let index = car
            .parts
            .iter()
            .position(|p| p.material == terra_core::train_parts::CAB)
            .unwrap();
        let (feet, yaw) = car.seat_pose(index).unwrap();
        assert!(feet[1] > 7.0);
        let mut journal = EditJournal::default();
        journal.gameplay_mut().trains.insert(car).unwrap();
        let mut neighbor = journal.gameplay().trains.vehicles[&1].clone();
        neighbor.anchor[2] -= 9;
        journal.gameplay_mut().trains.insert(neighbor).unwrap();

        let terrain = terra_world::TerraGenerator::new(Default::default()).unwrap();
        let properties = Default::default();
        let position = safe_exit(
            ExitIntent {
                vehicle: 1,
                feet,
                yaw,
            },
            &chunks,
            &journal,
            &terrain,
            &properties,
        )
        .expect("ground beside an elevated cab is reachable");
        assert!((position.y - 3.9).abs() < 0.03);
        assert!(terra_physics::character::space_clear_with_shapes(
            &chunks,
            &journal,
            &properties,
            position
        ));
    }
}
