//! Work orders use the same authority, loaded world and kingdom capabilities as players.
use super::*;
use crate::protocol::npc_work::{
    WorkAction, WorkCommand, WorkError, WorkResult, WorkSnapshot, WorkerDetail, WorkerView,
};
use terra_core::{
    creatures::{CreatureKind, villagers},
    kingdom::KingdomPermission,
    npc_jobs::{MAX_WORK_ORDERS, WorkOrder, WorkStatus},
};

#[derive(Resource, Default)]
pub(super) struct WorkRuntime {
    peers: HashMap<PlayerId, WorkPeer>,
    next: f64,
}

struct WorkPeer {
    request: u64,
    subscribed: Instant,
    tokens: f64,
    updated: Instant,
    sent: Option<WorkSnapshot>,
    detail: Option<u64>,
}
impl Default for WorkPeer {
    fn default() -> Self {
        Self {
            request: 0,
            subscribed: Instant::now(),
            tokens: 16.0,
            updated: Instant::now(),
            sent: None,
            detail: None,
        }
    }
}

pub(super) fn control(
    command: WorkCommand,
    peer: &Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    runtime: &mut WorkRuntime,
) {
    let state = runtime.peers.entry(peer.id).or_default();
    let now = Instant::now();
    state.tokens = (state.tokens + now.duration_since(state.updated).as_secs_f64() * 4.0).min(16.0);
    state.updated = now;
    let detail = match &command.action {
        WorkAction::InspectNpc { npc } => Some(Some(*npc)),
        WorkAction::Inspect => Some(None),
        _ => None,
    };
    let result = if command.request == 0 || command.request <= state.request {
        Err(WorkError::Stale)
    } else if state.tokens < 1.0 {
        Err(WorkError::Busy)
    } else {
        state.request = command.request;
        state.tokens -= 1.0;
        state.subscribed = now;
        apply(command.action, peer, world)
    };
    if let Some(detail) = detail {
        state.detail = result.as_ref().ok().and(detail);
    }
    sender.send::<ControlChannel>(ServerControl::NpcWorkResult(WorkResult {
        request: command.request,
        error: result.err(),
    }));
    state.sent = None;
}

fn allowed(
    peer: &Peer,
    world: &ControlWorld,
    permission: KingdomPermission,
    cell: [i64; 3],
) -> bool {
    kingdoms::permission_allowed(peer.uuid, permission, cell, world)
}

/// Remote private management is restricted to one's own worker in the same
/// currently occupied realm. Neither a UI flag nor diplomacy grants access.
fn through_keep(
    peer: &Peer,
    game: &terra_core::GameplayState,
    position: [f64; 3],
    npc: u64,
) -> bool {
    if game.creatures.owners.get(&npc) != Some(&peer.uuid) {
        return false;
    }
    let Some(keep) = game
        .kingdoms
        .territorial_keep(peer.uuid, position.map(|v| v.floor() as i64))
    else {
        return false;
    };
    let Some(creature) = game.creatures.animals.get(&npc) else {
        return false;
    };
    game.kingdoms
        .region_at(creature.position.map(|v| v.floor() as i64))
        .is_some_and(|region| game.kingdoms.realm_id(region.id) == keep.id)
}

fn inspect_npc(npc: u64, peer: &Peer, world: &ControlWorld) -> Result<(), WorkError> {
    let game = world.journal.gameplay();
    let creature = game.creatures.animals.get(&npc).ok_or(WorkError::Missing)?;
    if creature.kind != CreatureKind::Villager
        || creature.health <= 0.0
        || !villagers::is_civilian(creature.variant)
    {
        return Err(WorkError::Invalid);
    }
    if game
        .creatures
        .owners
        .get(&npc)
        .is_some_and(|owner| *owner != peer.uuid)
        || [KingdomPermission::NpcWork, KingdomPermission::Inventories]
            .into_iter()
            .any(|permission| {
                !allowed(
                    peer,
                    world,
                    permission,
                    creature.position.map(|v| v.floor() as i64),
                )
            })
    {
        return Err(WorkError::Protected);
    }
    let body = world
        .bodies
        .get(peer.body)
        .map_err(|_| WorkError::Missing)?;
    if terra_world::creatures::distance(
        creature.position,
        body.translation.to_array().map(f64::from),
    ) > 128.0_f64.powi(2)
        && !through_keep(peer, game, body.translation.to_array().map(f64::from), npc)
    {
        return Err(WorkError::Invalid);
    }
    if world
        .chunks
        .block_at(creature.position.map(|v| v.floor() as i64))
        .is_none()
        && !through_keep(peer, game, body.translation.to_array().map(f64::from), npc)
    {
        return Err(WorkError::Unloaded);
    }
    Ok(())
}

pub(super) fn apply(
    action: WorkAction,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<(), WorkError> {
    if !peer.synced
        || peer.uuid.is_nil()
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || !peer.permissions.build
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
    {
        return Err(WorkError::Protected);
    }
    if action == WorkAction::Inspect {
        return Ok(());
    }
    if let WorkAction::InspectNpc { npc } = &action {
        return inspect_npc(*npc, peer, world);
    }
    if world.journal.gameplay().revision == u64::MAX
        || world.journal.gameplay().jobs.revision == u64::MAX
    {
        return Err(WorkError::Limit);
    }
    match action {
        WorkAction::Inspect => Ok(()),
        WorkAction::InspectNpc { npc } => inspect_npc(npc, peer, world),
        WorkAction::Assign {
            npc,
            kind,
            area,
            storage,
            priority,
        } => {
            if !area.valid()
                || priority > terra_core::npc_jobs::MAX_PRIORITY
                || storage.iter().any(|v| v.unsigned_abs() > 999_000)
            {
                return Err(WorkError::Invalid);
            }
            let creature = world
                .journal
                .gameplay()
                .creatures
                .animals
                .get(&npc)
                .ok_or(WorkError::Missing)?;
            if creature.kind != CreatureKind::Villager
                || creature.health <= 0.0
                || !villagers::is_civilian(creature.variant)
            {
                return Err(WorkError::Invalid);
            }
            if world
                .journal
                .gameplay()
                .creatures
                .owners
                .get(&npc)
                .is_some_and(|owner| *owner != peer.uuid)
            {
                return Err(WorkError::Protected);
            }
            let position = creature.position;
            let actor = world
                .bodies
                .get(peer.body)
                .map_err(|_| WorkError::Missing)?
                .translation;
            if (0..3)
                .map(|i| (f64::from(actor[i]) - position[i]).powi(2))
                .sum::<f64>()
                > 128.0_f64.powi(2)
                && !through_keep(
                    peer,
                    world.journal.gameplay(),
                    actor.to_array().map(f64::from),
                    npc,
                )
                || [area.min, area.max, storage].into_iter().any(|cell| {
                    (0..3)
                        .map(|i| (cell[i] as f64 - position[i]).powi(2))
                        .sum::<f64>()
                        > 64.0_f64.powi(2)
                })
            {
                return Err(WorkError::Invalid);
            }
            if !world.chunks.block_at(storage).is_some_and(|material| {
                terra_core::profession_stations::supports_work(material, kind)
            }) {
                return Err(WorkError::Storage);
            }
            if world
                .journal
                .object_state(storage)
                .is_some_and(|state| state.secondary)
            {
                return Err(WorkError::Storage);
            }
            if !allowed(
                peer,
                world,
                KingdomPermission::NpcWork,
                position.map(|v| v.floor() as i64),
            ) || !allowed(peer, world, KingdomPermission::Inventories, storage)
                || world
                    .journal
                    .object_state(storage)
                    .and_then(|state| state.peer)
                    .is_some_and(|other| {
                        !allowed(peer, world, KingdomPermission::Inventories, other)
                    })
                || area
                    .cells()
                    .any(|cell| !allowed(peer, world, KingdomPermission::NpcWork, cell))
            {
                return Err(WorkError::Protected);
            }
            let previous = world
                .journal
                .gameplay()
                .jobs
                .orders
                .values()
                .find(|order| order.npc == npc)
                .cloned();
            if world
                .journal
                .gameplay()
                .jobs
                .workers
                .get(&npc)
                .is_some_and(|worker| worker.tree.is_some())
                && previous.as_ref().is_some_and(|order| {
                    order.kind != kind || order.area != area || order.storage != storage
                })
            {
                // A paused, partly felled tree and its loot remain owned work. Finish
                // that transaction before changing its profession, area or depot.
                return Err(WorkError::Busy);
            }
            if previous
                .as_ref()
                .is_some_and(|order| order.owner != peer.uuid)
            {
                return Err(WorkError::Protected);
            }
            let buildings = &world.journal.gameplay().buildings;
            if buildings.is_craftsman(npc)
                && (buildings.revision == u64::MAX
                    || buildings
                        .inventory(npc)
                        .is_some_and(|bag| bag.occupied_slots() > 0)
                    || buildings
                        .jobs
                        .values()
                        .any(|job| job.active() && job.workers.contains(&npc)))
            {
                return Err(WorkError::Busy);
            }
            let jobs = &mut world.journal.gameplay_mut().jobs;
            let (id, revision) = if let Some(previous) = previous {
                (
                    previous.id,
                    previous.revision.checked_add(1).ok_or(WorkError::Limit)?,
                )
            } else {
                if jobs.orders.len() >= MAX_WORK_ORDERS
                    || jobs.workers.len() >= terra_core::npc_jobs::MAX_WORKERS
                {
                    return Err(WorkError::Limit);
                }
                let id = jobs.next_id;
                let next = id.checked_add(1).ok_or(WorkError::Limit)?;
                jobs.next_id = next;
                (id, 1)
            };
            jobs.release(npc);
            let worker = jobs.workers.entry(npc).or_default();
            if kind == terra_core::npc_jobs::WorkKind::Lumberjack {
                worker.status = terra_core::npc_jobs::WorkStatus::Waiting(
                    terra_core::npc_jobs::WaitReason::MissingStation,
                );
            }
            jobs.orders.insert(
                id,
                WorkOrder {
                    id,
                    npc,
                    owner: peer.uuid,
                    kind,
                    area,
                    storage,
                    priority,
                    enabled: true,
                    revision,
                },
            );
            jobs.revision += 1;
            let game = world.journal.gameplay_mut();
            if game.buildings.is_craftsman(npc) {
                for settlement in game.buildings.settlements.values_mut() {
                    settlement.craftsmen.retain(|id| *id != npc);
                }
                game.buildings.inventories.remove(&npc);
                game.buildings.revision += 1;
            }
            game.creatures.owners.insert(npc, peer.uuid);
            // A lumberjack takes the profession only after reaching its real
            // station. Saving an order must not pretend the workplace exists.
            if kind != terra_core::npc_jobs::WorkKind::Lumberjack
                && let Some(creature) = game.creatures.animals.get_mut(&npc)
            {
                creature.variant = match kind {
                    terra_core::npc_jobs::WorkKind::Miner => villagers::MINER,
                    terra_core::npc_jobs::WorkKind::Farmer => 1,
                    terra_core::npc_jobs::WorkKind::Collector => villagers::UNEMPLOYED,
                    terra_core::npc_jobs::WorkKind::Lumberjack => villagers::LUMBERJACK,
                };
            }
            game.revision += 1;
            Ok(())
        }
        WorkAction::SetEnabled {
            id,
            revision,
            enabled,
        } => {
            let order = world
                .journal
                .gameplay()
                .jobs
                .orders
                .get(&id)
                .ok_or(WorkError::Missing)?;
            if order.owner != peer.uuid
                || !allowed(peer, world, KingdomPermission::NpcWork, order.area.min)
            {
                return Err(WorkError::Protected);
            }
            if order.revision != revision {
                return Err(WorkError::Stale);
            }
            let next = order.revision.checked_add(1).ok_or(WorkError::Limit)?;
            let npc = order.npc;
            let jobs = &mut world.journal.gameplay_mut().jobs;
            jobs.release(npc);
            let order = jobs.orders.get_mut(&id).ok_or(WorkError::Missing)?;
            order.enabled = enabled;
            order.revision = next;
            if let Some(worker) = jobs.workers.get_mut(&npc) {
                worker.status = if enabled {
                    WorkStatus::Idle
                } else {
                    WorkStatus::Paused
                };
            }
            jobs.revision += 1;
            world.journal.gameplay_mut().revision += 1;
            Ok(())
        }
    }
}

pub(super) fn snapshot(
    peer: &Peer,
    position: [f64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
    selected: Option<u64>,
    config: &GameConfig,
) -> WorkSnapshot {
    let game = journal.gameplay();
    // A direct inspection must not lose its selected worker behind the ordinary
    // list cap. Prioritize just that lookup, then stream the remaining candidates
    // through exactly the same eligibility checks without collecting the population.
    let workers: Vec<_> = selected
        .and_then(|npc| game.creatures.animals.get(&npc))
        .into_iter()
        .chain(
            game.creatures
                .animals
                .values()
                .filter(|creature| Some(creature.id) != selected),
        )
        .filter(|creature| {
            creature.kind == CreatureKind::Villager
                && creature.health > 0.0
                && villagers::is_civilian(creature.variant)
                && game
                    .creatures
                    .owners
                    .get(&creature.id)
                    .is_none_or(|owner| *owner == peer.uuid)
                && ((0..3)
                    .map(|axis| (creature.position[axis] - position[axis]).powi(2))
                    .sum::<f64>()
                    < 128.0_f64.powi(2)
                    || through_keep(peer, game, position, creature.id)
                    || game
                        .jobs
                        .orders
                        .values()
                        .any(|order| order.npc == creature.id && order.owner == peer.uuid))
                && terra_world::kingdom_access::can_use(
                    &game.kingdoms,
                    chunks,
                    peer.uuid,
                    KingdomPermission::NpcWork,
                    creature.position.map(|v| v.floor() as i64),
                )
        })
        .take(128)
        .map(|creature| {
            let worker = game.jobs.workers.get(&creature.id);
            WorkerView {
                npc: creature.id,
                name: villagers::name(creature.id).into(),
                position: creature.position,
                order: game
                    .jobs
                    .orders
                    .values()
                    .find(|order| order.npc == creature.id && order.owner == peer.uuid)
                    .cloned(),
                status: construction_status(game, creature.id)
                    .unwrap_or_else(|| worker.map_or(WorkStatus::Idle, |worker| worker.status)),
                carried_slots: game.buildings.inventory(creature.id).map_or_else(
                    || worker.map_or(0, |worker| worker.inventory.occupied_slots() as u8),
                    |bag| bag.occupied_slots() as u8,
                ),
            }
        })
        .collect();
    WorkSnapshot {
        revision: game.jobs.revision,
        detail: selected.and_then(|npc| {
            let worker = workers.iter().find(|worker| worker.npc == npc)?;
            if peer.vitals.dead()
                || peer.chat.teleport.is_some()
                || !peer.permissions.build
                || peer.game_mode(config) == terra_core::GameMode::Spectator
                || ((terra_world::creatures::distance(worker.position, position) > 8.0_f64.powi(2)
                    || chunks
                        .block_at(worker.position.map(|v| v.floor() as i64))
                        .is_none())
                    && !through_keep(peer, game, position, npc))
                || !terra_world::kingdom_access::can_use(
                    &game.kingdoms,
                    chunks,
                    peer.uuid,
                    KingdomPermission::Inventories,
                    worker.position.map(|v| v.floor() as i64),
                )
            {
                return None;
            }
            Some(WorkerDetail {
                npc,
                inventory: game.buildings.inventory(npc).cloned().unwrap_or_else(|| {
                    game.jobs.workers.get(&npc).map_or_else(
                        || terra_core::npc_jobs::WorkerState::default().inventory,
                        |worker| worker.inventory.clone(),
                    )
                }),
            })
        }),
        workers,
    }
}

fn construction_status(game: &terra_core::GameplayState, npc: u64) -> Option<WorkStatus> {
    use terra_core::{
        building_jobs::{BuildStatus, BuildWait},
        npc_jobs::WaitReason,
    };
    let job = game
        .buildings
        .jobs
        .values()
        .find(|job| job.active() && job.workers.contains(&npc));
    let Some(job) = job else {
        return game.creatures.owners.get(&npc).and_then(|owner| {
            returning_craftsman(game, npc, *owner).then_some(WorkStatus::Delivering)
        });
    };
    Some(match job.status {
        BuildStatus::Queued => WorkStatus::Idle,
        BuildStatus::Travelling => WorkStatus::Travelling,
        BuildStatus::Building(_) => WorkStatus::Working,
        BuildStatus::Paused => WorkStatus::Paused,
        BuildStatus::Complete | BuildStatus::Cancelled => WorkStatus::Idle,
        BuildStatus::Waiting(reason) => WorkStatus::Waiting(match reason {
            BuildWait::NoWorkers => WaitReason::NoTargets,
            BuildWait::MissingStorage | BuildWait::MissingMaterials => WaitReason::MissingMaterial,
            BuildWait::StorageFull => WaitReason::StorageFull,
            BuildWait::Unloaded => WaitReason::Unloaded,
            BuildWait::Protected => WaitReason::Protected,
            BuildWait::NoPath | BuildWait::Blocked => WaitReason::NoPath,
        }),
    })
}

/// Cancellation clears the project crew, but its real carried stock must still
/// be walked back through authorized doors before the profession can be freed.
fn returning_craftsman(
    game: &terra_core::GameplayState,
    npc: u64,
    owner: terra_core::PlayerUuid,
) -> bool {
    game.buildings
        .inventory(npc)
        .is_some_and(|bag| bag.occupied_slots() > 0)
        && game
            .buildings
            .settlements
            .values()
            .any(|site| site.owner == owner && site.craftsmen.contains(&npc))
        && !game
            .buildings
            .jobs
            .values()
            .any(|job| job.active() && job.workers.contains(&npc))
}

pub(super) fn broadcast(
    time: Res<Time>,
    config: Res<GameConfig>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut runtime: ResMut<WorkRuntime>,
) {
    let now = time.elapsed_secs_f64();
    if now < runtime.next {
        return;
    }
    runtime.next = now + 0.5;
    let connected: std::collections::HashSet<_> = peers.iter().map(|(peer, _)| peer.id).collect();
    runtime.peers.retain(|id, _| connected.contains(id));
    for (peer, mut sender) in &mut peers {
        let Some(state) = runtime.peers.get_mut(&peer.id) else {
            continue;
        };
        if !peer.synced || state.subscribed.elapsed().as_secs() > 10 {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let next = snapshot(
            peer,
            body.translation.to_array().map(f64::from),
            &chunks,
            &journal,
            state.detail,
            &config,
        );
        if state.sent.as_ref() != Some(&next) {
            sender.send::<ControlChannel>(ServerControl::NpcWork(Box::new(next.clone())));
            state.sent = Some(next);
        }
    }
}

#[allow(clippy::too_many_arguments)] // Bevy injects the individual system resources.
pub(super) fn tick(
    time: Res<Time>,
    clock: Res<terra_core::WorldClock>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut work: Local<terra_world::npc_work::WorkRuntime>,
    mut next: Local<f64>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut doors: Local<super::creatures::ResidentDoors>,
    defenses: Option<Res<super::villager_defense::Defenses>>,
) {
    let now = time.elapsed_secs_f64();
    if now < *next {
        return;
    }
    let dt = (now - *next + 0.1).clamp(0.001, 0.25) as f32;
    *next = now + 0.1;
    work.set_suspended(
        defenses
            .as_ref()
            .into_iter()
            .flat_map(|defenses| defenses.suspended()),
    );
    terra_world::npc_work::tick(
        &mut chunks,
        &mut journal,
        &terrain.0,
        &properties,
        clock.phase,
        dt,
        &mut work,
    );
    // A planned route never changes collision geometry. Only this authority
    // bridge opens a real door after rechecking the current owner and claims.
    let requests = authorized_worker_doors(&chunks, &journal, work.drain_door_requests());
    let occupants: Vec<_> = bodies
        .iter()
        .map(|body| body.translation.to_array().map(f64::from))
        .collect();
    let population = std::mem::take(&mut journal.gameplay_mut().creatures);
    doors.update(
        now as f32,
        &requests,
        super::creatures::ResidentDoorWorld {
            chunks: &mut chunks,
            journal: &mut journal,
            properties: &properties,
            population: &population,
            players: &occupants,
        },
    );
    journal.gameplay_mut().creatures = population;
}

pub(super) fn authorized_worker_doors(
    chunks: &ChunkStore,
    journal: &EditJournal,
    requests: impl Iterator<Item = terra_world::npc_work::WorkerDoorRequest>,
) -> std::collections::BTreeSet<[i64; 3]> {
    requests
        .take(16)
        .filter_map(|request| {
            let game = journal.gameplay();
            let npc = game.creatures.animals.get(&request.npc)?;
            if npc.kind != CreatureKind::Villager
                || npc.health <= 0.0
                || game.creatures.owners.get(&request.npc) != Some(&request.owner)
                || !terra_world::kingdom_access::can_use(
                    &game.kingdoms,
                    chunks,
                    request.owner,
                    KingdomPermission::NpcWork,
                    npc.position.map(|v| v.floor() as i64),
                )
                || !(game.jobs.orders.values().any(|order| {
                    order.npc == request.npc && order.owner == request.owner && order.enabled
                }) || game.buildings.jobs.values().any(|job| {
                    job.owner == request.owner
                        && job.active()
                        && job.enabled
                        && job.workers.contains(&request.npc)
                }) || returning_craftsman(game, request.npc, request.owner))
                || terra_world::creatures::distance(
                    npc.position,
                    request.position.map(|v| v as f64 + 0.5),
                ) > 4.0_f64.powi(2)
            {
                return None;
            }
            let material = chunks.block_at(request.position)?;
            if !terra_world::npc_work::worker_openable_door(material) {
                return None;
            }
            let group = terra_world::objects::doors::group(
                request.position,
                material,
                journal.object_state(request.position)?,
                |cell| Some((chunks.block_at(cell)?, journal.object_state(cell)?)),
            )?;
            group
                .cells()
                .all(|cell| {
                    chunks.block_at(cell) == Some(material)
                        && [KingdomPermission::NpcWork, KingdomPermission::Openings]
                            .into_iter()
                            .all(|permission| {
                                terra_world::kingdom_access::can_use(
                                    &game.kingdoms,
                                    chunks,
                                    request.owner,
                                    permission,
                                    cell,
                                )
                            })
                })
                .then_some(group.anchor)
        })
        .collect()
}
