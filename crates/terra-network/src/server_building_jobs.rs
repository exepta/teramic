//! Construction commands use the same keep, claims and real world as manual building.
use super::*;
use crate::protocol::building_jobs::*;
use terra_core::{
    building_jobs::*,
    creatures::{CreatureKind, villagers},
    kingdom::{ClaimKind, KingdomId, KingdomPermission, KingdomState},
};

#[derive(Resource)]
pub(super) struct BuildingRuntime {
    peers: HashMap<PlayerId, BuildingPeer>,
    next: f64,
    last_reload: Option<Instant>,
}
impl Default for BuildingRuntime {
    fn default() -> Self {
        // Bootstrap filesystem IO here; normal Inspect/HUD polls read only the cache.
        let _ = terra_world::building_jobs::template_catalog();
        Self {
            peers: HashMap::new(),
            next: 0.0,
            last_reload: None,
        }
    }
}
struct BuildingPeer {
    request: u64,
    tokens: f64,
    updated: Instant,
    kingdom: Option<KingdomId>,
    subscribed: Instant,
    sent: Option<BuildingSnapshot>,
    stock_cache: Option<StockCache>,
}
struct StockCache {
    viewer: terra_core::PlayerUuid,
    kingdom: KingdomId,
    area: Option<BuildArea>,
    permissions: u64,
    sampled: Instant,
    stock: TownStock,
}
impl Default for BuildingPeer {
    fn default() -> Self {
        Self {
            request: 0,
            tokens: 16.0,
            updated: Instant::now(),
            kingdom: None,
            subscribed: Instant::now(),
            sent: None,
            stock_cache: None,
        }
    }
}

fn access(
    peer: &Peer,
    world: &ControlWorld,
    id: KingdomId,
) -> Result<terra_core::PlayerUuid, BuildError> {
    if !peer.synced
        || peer.uuid.is_nil()
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || !peer.permissions.build
        || peer.game_mode(&world.config) == terra_core::GameMode::Spectator
    {
        return Err(BuildError::Protected);
    }
    let registry = &world.journal.gameplay().kingdoms;
    let settlement = registry
        .get(id)
        .filter(|k| k.state == KingdomState::Active)
        .ok_or(BuildError::MissingKingdom)?;
    let realm = registry
        .get(registry.realm_id(id))
        .ok_or(BuildError::MissingKingdom)?;
    if !realm.members.contains(&peer.uuid)
        || !registry.can(peer.uuid, KingdomPermission::NpcWork, settlement.location)
    {
        return Err(BuildError::Protected);
    }
    super::kingdoms::check_keep(settlement.location, peer, world)
        .map_err(|_| BuildError::Protected)?;
    Ok(realm.founder)
}

fn build_area(
    world: &ControlWorld,
    actor: terra_core::PlayerUuid,
    id: KingdomId,
    area: BuildArea,
) -> Result<(), BuildError> {
    if !area.valid() {
        return Err(BuildError::InvalidArea);
    }
    let registry = &world.journal.gameplay().kingdoms;
    for x in area.min[0].div_euclid(16)..=area.max[0].div_euclid(16) {
        for z in area.min[2].div_euclid(16)..=area.max[2].div_euclid(16) {
            let claim = registry.claim_at([x, z]).ok_or(BuildError::Protected)?;
            if claim.kind != ClaimKind::ProtectedBuilding
                || registry.realm_id(claim.kingdom) != registry.realm_id(id)
            {
                return Err(BuildError::Protected);
            }
            let point = [x * 16, area.min[1], z * 16];
            if !registry.can(actor, KingdomPermission::NpcWork, point)
                || !registry.can(actor, KingdomPermission::Build, point)
                || !registry.can(actor, KingdomPermission::Inventories, point)
            {
                return Err(BuildError::Protected);
            }
        }
    }
    Ok(())
}

fn settlement_mut(
    game: &mut terra_core::GameplayState,
    id: KingdomId,
    owner: terra_core::PlayerUuid,
) -> Result<&mut SettlementBuilding, BuildError> {
    if !game.buildings.settlements.contains_key(&id)
        && game.buildings.settlements.len() >= MAX_SETTLEMENTS
    {
        return Err(BuildError::Limit);
    }
    Ok(game
        .buildings
        .settlements
        .entry(id)
        .or_insert_with(|| SettlementBuilding::new(owner)))
}

fn template_available(id: &str) -> Result<(), BuildError> {
    if !terra_core::building_jobs::template::valid_template_id(id)
        || terra_world::building_jobs::template_catalog()?
            .get(id)
            .is_none()
    {
        return Err(BuildError::InvalidTemplate);
    }
    Ok(())
}

pub(super) fn apply(
    action: BuildingAction,
    peer: &Peer,
    world: &mut ControlWorld,
) -> Result<(), BuildError> {
    let previous_revision = world.journal.gameplay().buildings.revision;
    let kingdom = match &action {
        BuildingAction::Inspect { kingdom }
        | BuildingAction::Preview { kingdom, .. }
        | BuildingAction::ReloadCatalog { kingdom }
        | BuildingAction::SetStorage { kingdom, .. }
        | BuildingAction::SetResidential { kingdom, .. }
        | BuildingAction::AssignCraftsman { kingdom, .. }
        | BuildingAction::QueueHouse { kingdom, .. } => *kingdom,
        BuildingAction::SetEnabled { id, .. } | BuildingAction::Cancel { id, .. } => {
            world
                .journal
                .gameplay()
                .buildings
                .jobs
                .get(id)
                .ok_or(BuildError::MissingJob)?
                .kingdom
        }
    };
    let owner = access(peer, world, kingdom)?;
    if matches!(action, BuildingAction::Inspect { .. }) {
        return Ok(());
    }
    if let BuildingAction::Preview { template, .. } = &action {
        return template_available(template);
    }
    if matches!(action, BuildingAction::ReloadCatalog { .. }) {
        if !world
            .journal
            .gameplay()
            .kingdoms
            .can_manage(peer.uuid, kingdom)
        {
            return Err(BuildError::Protected);
        }
        return terra_world::building_jobs::reload_template_catalog()
            .map(|_| ())
            .map_err(|_| BuildError::InvalidTemplate);
    }
    if world.journal.gameplay().revision == u64::MAX
        || world.journal.gameplay().buildings.revision == u64::MAX
    {
        return Err(BuildError::Limit);
    }
    match action {
        BuildingAction::Inspect { .. }
        | BuildingAction::Preview { .. }
        | BuildingAction::ReloadCatalog { .. } => {}
        BuildingAction::SetStorage { area, .. } => {
            build_area(world, peer.uuid, kingdom, area)?;
            settlement_mut(world.journal.gameplay_mut(), kingdom, owner)?.storage = Some(area);
        }
        BuildingAction::SetResidential { area, template, .. } => {
            if !terra_core::building_jobs::template::valid_template_id(&template) {
                return Err(BuildError::InvalidTemplate);
            }
            if let Some(area) = area {
                template_available(&template)?;
                build_area(world, peer.uuid, kingdom, area)?;
            }
            let settlement = settlement_mut(world.journal.gameplay_mut(), kingdom, owner)?;
            settlement.residential = area;
            settlement.selected_template = template;
        }
        BuildingAction::AssignCraftsman { npc, enabled, .. } => {
            let game = world.journal.gameplay();
            let creature = game
                .creatures
                .animals
                .get(&npc)
                .filter(|c| {
                    c.kind == CreatureKind::Villager
                        && c.health > 0.0
                        && villagers::is_civilian(c.variant)
                })
                .ok_or(BuildError::MissingWorker)?;
            if game
                .creatures
                .owners
                .get(&npc)
                .is_some_and(|actual| *actual != owner)
                || !game
                    .kingdoms
                    .region_at(creature.position.map(|v| v.floor() as i64))
                    .is_some_and(|r| {
                        game.kingdoms.realm_id(r.id) == game.kingdoms.realm_id(kingdom)
                    })
            {
                return Err(BuildError::Protected);
            }
            if !enabled
                && !game
                    .buildings
                    .settlements
                    .get(&kingdom)
                    .is_some_and(|site| site.craftsmen.contains(&npc))
            {
                return Err(BuildError::MissingWorker);
            }
            if game
                .buildings
                .jobs
                .values()
                .any(|j| j.active() && j.workers.contains(&npc))
            {
                return Err(BuildError::Busy);
            }
            if !enabled
                && game
                    .buildings
                    .inventory(npc)
                    .is_some_and(|bag| bag.occupied_slots() > 0)
            {
                return Err(BuildError::Busy);
            }
            if enabled {
                if game
                    .buildings
                    .settlements
                    .iter()
                    .any(|(id, s)| *id != kingdom && s.craftsmen.contains(&npc))
                {
                    return Err(BuildError::Busy);
                }
                if game
                    .jobs
                    .workers
                    .get(&npc)
                    .is_some_and(|w| w.tree.is_some() || w.inventory.occupied_slots() != 0)
                    || game.jobs.orders.values().any(|o| o.npc == npc && o.enabled)
                {
                    return Err(BuildError::Busy);
                }
                if game.buildings.settlements.get(&kingdom).is_some_and(|s| {
                    s.craftsmen.len() >= MAX_CRAFTSMEN && !s.craftsmen.contains(&npc)
                }) {
                    return Err(BuildError::Limit);
                }
                if game.jobs.revision == u64::MAX {
                    return Err(BuildError::Limit);
                }
            }
            let game = world.journal.gameplay_mut();
            let roster = &mut settlement_mut(game, kingdom, owner)?.craftsmen;
            if enabled {
                if !roster.contains(&npc) {
                    roster.push(npc);
                    roster.sort_unstable();
                }
            } else {
                roster.retain(|id| *id != npc);
            }
            if enabled {
                game.creatures.owners.insert(npc, owner);
                game.jobs.orders.retain(|_, o| o.npc != npc);
                game.jobs.workers.remove(&npc);
                game.jobs.revision += 1;
            }
            if enabled {
                game.buildings.inventories.entry(npc).or_default();
            } else {
                game.buildings.inventories.remove(&npc);
            }
            if let Some(creature) = game.creatures.animals.get_mut(&npc) {
                creature.variant = if enabled {
                    villagers::CRAFTSMAN
                } else {
                    villagers::UNEMPLOYED
                };
            }
        }
        BuildingAction::QueueHouse {
            origin,
            facing,
            template,
            ..
        } => {
            template_available(&template)?;
            if facing > 3 {
                return Err(BuildError::InvalidSite);
            }
            if !creative_owner(owner, world.journal.gameplay(), &world.config)
                && world
                    .journal
                    .gameplay()
                    .buildings
                    .settlements
                    .get(&kingdom)
                    .and_then(|s| s.storage)
                    .is_none()
            {
                return Err(BuildError::MissingStorage);
            }
            let mut job = terra_world::building_jobs::plan_template_detailed(
                &world.chunks,
                &world.journal,
                &world.terrain.0,
                kingdom,
                owner,
                origin,
                facing,
                &template,
            )
            .map_err(|issue| {
                tracing::warn!(
                    ?origin,
                    facing,
                    %template,
                    %kingdom,
                    owner = %owner,
                    error = ?issue.error,
                    cause = ?issue.cause,
                    position = ?issue.position,
                    material = ?issue.material,
                    saved_material = ?issue.saved_material,
                    conflicting_job = ?issue.conflicting_job,
                    "House placement rejected by the authoritative planner"
                );
                issue.error
            })?;
            job.free_resources = creative_owner(owner, world.journal.gameplay(), &world.config);
            for step in &job.steps {
                for cell in &step.cells {
                    build_area(
                        world,
                        peer.uuid,
                        kingdom,
                        BuildArea::new(cell.position, cell.position)?,
                    )?;
                }
            }
            let buildings = &mut world.journal.gameplay_mut().buildings;
            if let Some(site) = buildings.settlements.get(&kingdom) {
                if site.owner != owner {
                    return Err(BuildError::Protected);
                }
            } else if buildings.settlements.len() >= MAX_SETTLEMENTS {
                return Err(BuildError::Limit);
            }
            // Creative may queue its first house without storage or enrolled
            // craftsmen. Commit the owning settlement alongside the job so the
            // registry is immediately valid for simulation and persistence.
            buildings.insert(job)?;
            buildings
                .settlements
                .entry(kingdom)
                .or_insert_with(|| SettlementBuilding::new(owner));
        }
        BuildingAction::SetEnabled {
            id,
            revision,
            enabled,
        } => {
            let job = world
                .journal
                .gameplay_mut()
                .buildings
                .jobs
                .get_mut(&id)
                .ok_or(BuildError::MissingJob)?;
            if job.revision != revision || !job.active() {
                return Err(BuildError::Stale);
            }
            job.revision = job.revision.checked_add(1).ok_or(BuildError::Limit)?;
            job.enabled = enabled;
            job.status = if enabled {
                BuildStatus::Queued
            } else {
                BuildStatus::Paused
            };
        }
        BuildingAction::Cancel { id, revision } => {
            let job = world
                .journal
                .gameplay_mut()
                .buildings
                .jobs
                .get_mut(&id)
                .ok_or(BuildError::MissingJob)?;
            if job.revision != revision || !job.active() {
                return Err(BuildError::Stale);
            }
            job.revision = job.revision.checked_add(1).ok_or(BuildError::Limit)?;
            job.status = BuildStatus::Cancelled;
            job.enabled = false;
            job.workers.clear();
        }
    }
    let game = world.journal.gameplay_mut();
    if game.buildings.revision == previous_revision {
        game.buildings.revision += 1;
    }
    game.revision += 1;
    Ok(())
}

pub(super) fn control(
    command: BuildingCommand,
    peer: &Peer,
    sender: &mut MessageSender<ServerControl>,
    world: &mut ControlWorld,
    runtime: &mut BuildingRuntime,
) {
    let state = runtime.peers.entry(peer.id).or_default();
    let now = Instant::now();
    state.tokens = (state.tokens + now.duration_since(state.updated).as_secs_f64() * 4.0).min(16.0);
    state.updated = now;
    let inspect = match &command.action {
        BuildingAction::Inspect { kingdom }
        | BuildingAction::Preview { kingdom, .. }
        | BuildingAction::ReloadCatalog { kingdom } => Some(*kingdom),
        _ => None,
    };
    let preview = if let BuildingAction::Preview { template, .. } = &command.action {
        Some(template.clone())
    } else {
        None
    };
    let reload = matches!(command.action, BuildingAction::ReloadCatalog { .. });
    let reload_authorized = reload
        && inspect.is_some_and(|id| {
            access(peer, world, id).is_ok()
                && world.journal.gameplay().kingdoms.can_manage(peer.uuid, id)
        });
    let result = if command.request == 0 || command.request <= state.request {
        Err(BuildError::Stale)
    } else if state.tokens < 1.0 {
        Err(BuildError::Busy)
    } else {
        state.request = command.request;
        state.tokens -= 1.0;
        if reload && !reload_authorized {
            Err(BuildError::Protected)
        } else if reload
            && runtime
                .last_reload
                .is_some_and(|last| last.elapsed() < Duration::from_secs(2))
        {
            Err(BuildError::Busy)
        } else {
            if reload {
                runtime.last_reload = Some(now);
            }
            apply(command.action, peer, world)
        }
    };
    if let Some(kingdom) = inspect {
        state.kingdom = if result.is_ok() || reload_authorized {
            Some(kingdom)
        } else {
            None
        };
        state.subscribed = now;
    }
    state.sent = None;
    sender.send::<ControlChannel>(ServerControl::BuildingResult(BuildingResult {
        request: command.request,
        error: result.err(),
    }));
    if (result.is_ok() && preview.is_some() || reload_authorized)
        && let Ok(body) = world.bodies.get(peer.body)
    {
        let mut next = snapshot(
            peer,
            body.translation.to_array().map(f64::from),
            world.journal.gameplay(),
            state.kingdom,
            &world.config,
        );
        attach_stock(&mut next, peer, &world.chunks, &world.journal, state);
        if next.stock_kingdom.is_some()
            && let Some(template) = preview
        {
            match terra_world::building_jobs::template_catalog() {
                Ok(catalog) => {
                    next.templates = catalog.summaries();
                    next.catalog_error = None;
                    next.preview = catalog
                        .get(&template)
                        .and_then(|source| TemplatePreview::from_template(source).ok());
                }
                Err(_) => {
                    next.templates.clear();
                    next.preview = None;
                    next.catalog_error = Some(catalog_error_message());
                }
            }
        }
        sender.send::<ControlChannel>(ServerControl::Building(Box::new(next.clone())));
        state.sent = Some(next);
    }
}

pub(super) fn creative_owner(
    owner: terra_core::PlayerUuid,
    game: &terra_core::GameplayState,
    config: &GameConfig,
) -> bool {
    config.server.permissions_for(owner).build
        && game
            .player_stats
            .get(&owner)
            .and_then(|s| s.game_mode)
            .unwrap_or(config.server.game_mode)
            == terra_core::GameMode::Creative
}

fn attach_stock(
    result: &mut BuildingSnapshot,
    peer: &Peer,
    chunks: &ChunkStore,
    journal: &EditJournal,
    state: &mut BuildingPeer,
) {
    let Some(kingdom) = result.stock_kingdom else {
        state.stock_cache = None;
        return;
    };
    let area = result.settlements.get(&kingdom).and_then(|s| s.storage);
    let permissions = journal.gameplay().kingdoms.revision;
    if let Some(cache) = &state.stock_cache
        && cache.viewer == peer.uuid
        && cache.kingdom == kingdom
        && cache.area == area
        && cache.permissions == permissions
        && cache.sampled.elapsed() < Duration::from_secs(1)
    {
        result.stock = cache.stock.clone();
        return;
    }
    result.stock = terra_world::building_jobs::town_stock(chunks, journal, kingdom, peer.uuid);
    state.stock_cache = Some(StockCache {
        viewer: peer.uuid,
        kingdom,
        area,
        permissions,
        sampled: Instant::now(),
        stock: result.stock.clone(),
    });
}

fn catalog_error_message() -> String {
    let mut error = terra_world::building_jobs::template_catalog_error()
        .unwrap_or_else(|| "Building templates unavailable".into());
    while error.len() > 512 {
        error.pop();
    }
    error
}

pub(super) fn snapshot(
    peer: &Peer,
    position: [f64; 3],
    game: &terra_core::GameplayState,
    kingdom: Option<KingdomId>,
    config: &GameConfig,
) -> BuildingSnapshot {
    let mut result = BuildingSnapshot {
        revision: game.buildings.revision,
        ..Default::default()
    };
    let Some(kingdom) = kingdom else {
        return result;
    };
    let Some(settlement) = game
        .kingdoms
        .get(kingdom)
        .filter(|k| k.state == KingdomState::Active)
    else {
        return result;
    };
    let Some(realm) = game
        .kingdoms
        .get(game.kingdoms.realm_id(kingdom))
        .filter(|realm| realm.state == KingdomState::Active && realm.members.contains(&peer.uuid))
    else {
        return result;
    };
    let same_territory = game
        .kingdoms
        .territorial_keep(peer.uuid, position.map(|v| v.floor() as i64))
        .is_some_and(|k| k.id == realm.id);
    if !peer.synced
        || peer.uuid.is_nil()
        || position.iter().any(|value| !value.is_finite())
        || peer.game_mode(config) == terra_core::GameMode::Spectator
        || peer.vitals.dead()
        || peer.chat.teleport.is_some()
        || !peer.permissions.build
        || !game
            .kingdoms
            .can(peer.uuid, KingdomPermission::NpcWork, settlement.location)
        || (!same_territory
            && terra_world::creatures::distance(
                position,
                settlement.location.map(|v| v as f64 + 0.5),
            ) > 8.0_f64.powi(2))
    {
        return result;
    }
    result.stock_kingdom = Some(kingdom);
    result.unlimited_resources = creative_owner(realm.founder, game, config);
    match terra_world::building_jobs::template_catalog() {
        Ok(catalog) => result.templates = catalog.summaries(),
        Err(_) => result.catalog_error = Some(catalog_error_message()),
    }
    result.settlements.insert(
        kingdom,
        game.buildings
            .settlements
            .get(&kingdom)
            .cloned()
            .unwrap_or_else(|| SettlementBuilding::new(settlement.founder)),
    );
    result.jobs.extend(
        game.buildings
            .jobs
            .iter()
            .filter(|(_, j)| j.kingdom == kingdom)
            .map(|(id, j)| (*id, BuildingJobView::from(j))),
    );
    result
}

pub(super) fn broadcast(
    time: Res<Time>,
    config: Res<GameConfig>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    mut runtime: ResMut<BuildingRuntime>,
) {
    let now = time.elapsed_secs_f64();
    if now < runtime.next {
        return;
    }
    runtime.next = now + 0.5;
    let connected: std::collections::HashSet<_> = peers.iter().map(|(p, _)| p.id).collect();
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
        let mut next = snapshot(
            peer,
            body.translation.to_array().map(f64::from),
            journal.gameplay(),
            state.kingdom,
            &config,
        );
        attach_stock(&mut next, peer, &chunks, &journal, state);
        if state.sent.as_ref() != Some(&next) {
            sender.send::<ControlChannel>(ServerControl::Building(Box::new(next.clone())));
            state.sent = Some(next);
        }
    }
}

#[allow(clippy::too_many_arguments)] // Bounded server-only construction context.
pub(super) fn tick(
    time: Res<Time<Real>>,
    config: Res<GameConfig>,
    terrain: Res<terra_world::streaming::WorldTerrain>,
    properties: Res<terra_core::block_properties::BlockPropertiesCatalog>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut runtime: Local<terra_world::building_jobs::BuildRuntime>,
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
    runtime.set_creative_owners(
        journal
            .gameplay()
            .buildings
            .settlements
            .values()
            .map(|s| s.owner)
            .filter(|owner| creative_owner(*owner, journal.gameplay(), &config)),
    );
    runtime.set_suspended(
        defenses
            .as_ref()
            .into_iter()
            .flat_map(|defenses| defenses.suspended()),
    );
    let occupants: Vec<_> = bodies
        .iter()
        .map(|b| b.translation.to_array().map(f64::from))
        .collect();
    runtime.set_player_bodies(occupants.iter().copied());
    terra_world::building_jobs::tick(
        &mut chunks,
        &mut journal,
        &terrain.0,
        &properties,
        dt,
        &mut runtime,
    );
    let requests =
        super::npc_work::authorized_worker_doors(&chunks, &journal, runtime.drain_door_requests());
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
