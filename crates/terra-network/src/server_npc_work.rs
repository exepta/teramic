//! Work orders use the same authority, loaded world and kingdom capabilities as players.
use super::*;
use crate::protocol::npc_work::{
    WorkAction, WorkCommand, WorkError, WorkResult, WorkSnapshot, WorkerView,
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
}
impl Default for WorkPeer {
    fn default() -> Self {
        Self {
            request: 0,
            subscribed: Instant::now(),
            tokens: 16.0,
            updated: Instant::now(),
            sent: None,
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
    if world.journal.gameplay().revision == u64::MAX
        || world.journal.gameplay().jobs.revision == u64::MAX
    {
        return Err(WorkError::Limit);
    }
    match action {
        WorkAction::Inspect => Ok(()),
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
            jobs.workers.entry(npc).or_default();
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
            game.creatures.owners.insert(npc, peer.uuid);
            if let Some(creature) = game.creatures.animals.get_mut(&npc) {
                creature.variant = match kind {
                    terra_core::npc_jobs::WorkKind::Miner => villagers::MINER,
                    terra_core::npc_jobs::WorkKind::Farmer => 1,
                    terra_core::npc_jobs::WorkKind::Collector => villagers::UNEMPLOYED,
                    terra_core::npc_jobs::WorkKind::Lumberjack => 9,
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

fn snapshot(
    peer: &Peer,
    position: [f64; 3],
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> WorkSnapshot {
    let game = journal.gameplay();
    let workers = game
        .creatures
        .animals
        .values()
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
                status: worker.map_or(WorkStatus::Idle, |worker| worker.status),
                carried_slots: worker.map_or(0, |worker| worker.inventory.occupied_slots() as u8),
            }
        })
        .collect();
    WorkSnapshot {
        revision: game.jobs.revision,
        workers,
    }
}

pub(super) fn broadcast(
    time: Res<Time>,
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
) {
    let now = time.elapsed_secs_f64();
    if now < *next {
        return;
    }
    let dt = (now - *next + 0.1).clamp(0.001, 0.25) as f32;
    *next = now + 0.1;
    terra_world::npc_work::tick(
        &mut chunks,
        &mut journal,
        &terrain.0,
        &properties,
        clock.phase,
        dt,
        &mut work,
    );
}
