//! Identical authoritative session runtime for dedicated and integrated hosts.
use crate::protocol::session::*;
use bevy::{ecs::system::SystemParam, prelude::*};
pub use chat::{ConsoleOutput, ServerConsole};
use lightyear::connection::client::Disconnecting;
use lightyear::prelude::{server::*, *};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    time::{Duration, Instant},
};
use terra_core::{GENERATOR_VERSION, GameConfig, PlayerId, PlayerProfile};
use terra_physics::character::{
    CharacterGrounded, CharacterInput, CharacterSimulation, CharacterState, body_bundle,
};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkFoci, ChunkFocus, ChunkStore, StreamingSet, WorldAccess},
};

#[derive(Resource, Clone)]
pub struct ServerEndpoint {
    pub bind: SocketAddr,
    pub advertised: Option<SocketAddr>,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerListening;

#[derive(Resource)]
pub struct ServerRuntime {
    key: [u8; 32],
    pub address: Option<SocketAddr>,
    pub entity: Option<Entity>,
    pub error: Option<String>,
    pub tick: u64,
    pub connected_players: usize,
    next_player: u64,
}
impl Default for ServerRuntime {
    fn default() -> Self {
        Self {
            key: lightyear::netcode::generate_key(),
            address: None,
            entity: None,
            error: None,
            tick: 0,
            connected_players: 0,
            next_player: 1,
        }
    }
}
impl ServerRuntime {
    pub fn issue_token(&self, client_id: u64) -> Result<Vec<u8>, String> {
        let address = self.address.ok_or("Server is not listening")?;
        if address.ip().is_unspecified() {
            return Err("Use an advertised reachable address to issue join tokens".into());
        }
        lightyear::netcode::ConnectToken::build(address, PROTOCOL_ID, client_id, self.key)
            .expire_seconds(3600)
            .timeout_seconds(10)
            .generate()
            .map_err(|e| e.to_string())?
            .try_into_bytes()
            .map(Vec::from)
            .map_err(|e| e.to_string())
    }
}

#[derive(Component)]
struct ServerCharacter {
    link: Entity,
}

#[derive(Component)]
struct Peer {
    chest_cursor: Option<(crate::protocol::social::ChestCursorInput, Instant)>,
    creature_action_at: Instant,
    container: Option<crate::protocol::inventory::ContainerSnapshot>,
    held_slot: Option<u8>,
    mode: Option<terra_core::GameMode>,
    generation: u32,
    chat: chat::PeerChat,
    uuid: terra_core::PlayerUuid,
    gender: terra_core::PlayerGender,
    mining: Option<gameplay::MiningState>,
    consumption: Option<consumption::EatingState>,
    gameplay_sent: Option<(u64, [i64; 3])>,
    permissions: terra_core::PlayerPermissions,
    id: PlayerId,
    transport_id: u64,
    body: Entity,
    accepted: bool,
    synced: bool,
    view: u8,
    known: HashMap<terra_voxel::SectionPos, (u64, u64, u64, u64)>,
    inputs: VecDeque<MovementInput>,
    received: u64,
    ack: u64,
    look: [f32; 2],
    last_input: Instant,
    joined: Instant,
    last_request: u64,
    edits_left: f64,
    rate_time: Instant,
    spawned: bool,
    last_movement: CharacterInput,
    vitals: terra_core::SurvivalVitals,
    survival: survival::SurvivalPeer,
    stats_sent: Option<(u32, terra_core::PlayerStats)>,
}

impl Peer {
    fn game_mode(&self, config: &GameConfig) -> terra_core::GameMode {
        self.mode.unwrap_or(config.server.game_mode)
    }
    fn can_fly(&self, config: &GameConfig) -> bool {
        self.game_mode(config) == terra_core::GameMode::Spectator
            || self.permissions.flight
            || (self.permissions.game_mode
                && self.game_mode(config) == terra_core::GameMode::Creative)
    }
}

#[derive(Resource, Default)]
pub struct ConnectedProfiles(pub HashMap<PlayerId, PlayerProfile>);

pub(crate) fn install(app: &mut App) {
    let endpoint = ServerEndpoint {
        bind: app.world().resource::<GameConfig>().server.bind_address,
        advertised: None,
    };
    app.insert_resource(endpoint)
        .insert_resource(WorldAccess::Server)
        .init_resource::<ServerRuntime>()
        .init_resource::<ConnectedProfiles>()
        .init_resource::<beds::SleepRuntime>()
        .init_resource::<chat::ChatRuntime>()
        .init_resource::<ServerConsole>()
        .add_systems(
            Startup,
            (
                crate::world_time::initialize,
                crate::weather::initialize,
                spawn::initialize,
                start_listener,
            )
                .chain()
                .in_set(ServerListening),
        )
        .add_observer(new_link)
        .add_observer(connected)
        .add_systems(
            Update,
            (
                cleanup,
                crate::world_time::advance_server,
                crate::weather::advance_server,
                receive_controls,
                beds::tick,
                receive_inputs,
                chat::tick,
                survival::begin_respawns,
                furnaces::tick,
                gameplay::tick,
                creatures::tick,
                lightning::tick,
                update_foci,
            )
                .chain()
                .before(StreamingSet::Interest),
        )
        .add_systems(
            FixedUpdate,
            simulate.after(avian3d::prelude::PhysicsSystems::Writeback),
        )
        .add_systems(FixedUpdate, gameplay::simulate_drops.after(simulate))
        .add_systems(
            PostUpdate,
            (
                send_deltas,
                send_players,
                gameplay::send_gameplay,
                survival::commit_and_send,
                send_clock,
                social::broadcast,
            )
                .chain()
                .before(MessageSystems::Send),
        );
}

fn reserve_endpoints(
    bind: std::net::SocketAddr,
) -> Result<(std::net::TcpListener, std::net::UdpSocket), String> {
    for attempt in 0..8 {
        let tcp = std::net::TcpListener::bind(bind)
            .map_err(|e| format!("Cannot bind server status/join TCP endpoint {bind}: {e}"))?;
        let bound = tcp.local_addr().map_err(|e| e.to_string())?;
        match std::net::UdpSocket::bind(bound) {
            Ok(udp) => return Ok((tcp, udp)),
            Err(error)
                if bind.port() == 0
                    && error.kind() == std::io::ErrorKind::AddrInUse
                    && attempt < 7 =>
            {
                continue;
            }
            Err(error) => return Err(format!("Cannot bind server UDP endpoint {bound}: {error}")),
        }
    }
    Err("Cannot reserve a shared TCP/UDP server port".into())
}

#[cfg(test)]
mod endpoint_tests {
    #[test]
    fn ephemeral_server_port_is_reserved_for_both_transports_before_startup() {
        let (tcp, udp) = super::reserve_endpoints("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = tcp.local_addr().unwrap();
        assert_ne!(address.port(), 0);
        assert_eq!(udp.local_addr().unwrap(), address);
        assert!(std::net::TcpListener::bind(address).is_err());
        assert!(std::net::UdpSocket::bind(address).is_err());
        assert!(super::reserve_endpoints(address).is_err());
    }
}

fn start_listener(world: &mut World) {
    if world
        .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
        .0
        .is_empty()
    {
        world.resource_mut::<ServerRuntime>().error =
            Some("Block mechanics could not be loaded; check assets/terra/blocks".into());
        return;
    }
    if world.resource::<ServerRuntime>().error.is_some() {
        return;
    }
    let endpoint = world.resource::<ServerEndpoint>().clone();
    let config = world.resource::<GameConfig>().server.clone();
    let key = world.resource::<ServerRuntime>().key;
    let (tcp, udp) = match reserve_endpoints(endpoint.bind) {
        Ok(pair) => pair,
        Err(error) => {
            world.resource_mut::<ServerRuntime>().error = Some(error);
            world.write_message(bevy::app::AppExit::error());
            return;
        }
    };
    let bind = match tcp.local_addr() {
        Ok(bind) => bind,
        Err(error) => {
            world.resource_mut::<ServerRuntime>().error = Some(error.to_string());
            return;
        }
    };
    let mut netcode = NetcodeConfig::default()
        .with_protocol_id(PROTOCOL_ID)
        .with_key(key)
        .with_max_clients(usize::from(config.max_players))
        .with_client_timeout_secs(10);
    if let Some(address) = endpoint.advertised {
        netcode = netcode.with_additional_expected_addresses([address]);
    }
    // Tokens are signed by this process's unique transport key; wildcard binds
    // must also accept the interface address negotiated by the TCP handshake.
    if endpoint.bind.ip().is_unspecified() {
        netcode.server_addr_check = false;
    }
    let entity = world
        .spawn((
            NetcodeServer::new(netcode),
            ServerUdpIo::default(),
            LocalAddr(bind),
        ))
        .id();
    drop(udp);
    world.trigger(Start { entity });
    world.flush();
    let address = world.get::<LocalAddr>(entity).map(|a| a.0);
    if world.get::<Linked>(entity).is_none() || address.is_none_or(|a| a.port() == 0) {
        world.resource_mut::<ServerRuntime>().error =
            Some(format!("Cannot bind server to {}", endpoint.bind));
        world.write_message(bevy::app::AppExit::error());
        return;
    }
    match crate::gateway::Gateway::start(&config, tcp, key) {
        Ok(gateway) => {
            world.insert_resource(gateway);
        }
        Err(error) => {
            world.resource_mut::<ServerRuntime>().error = Some(error);
            world.write_message(bevy::app::AppExit::error());
            return;
        }
    }
    let mut runtime = world.resource_mut::<ServerRuntime>();
    runtime.address = endpoint.advertised.or(address);
    runtime.entity = Some(entity);
    tracing::info!(address=?runtime.address,"Authoritative Terra server listening");
}

fn new_link(event: On<Add, LinkOf>, mut commands: Commands) {
    commands
        .entity(event.entity)
        .insert(MessageManager::default());
}
fn connected(
    event: On<Add, Connected>,
    links: Query<&RemoteId, With<ClientOf>>,
    mut commands: Commands,
    mut runtime: ResMut<ServerRuntime>,
    mut spawn: spawn::SpawnWorld,
) {
    let Ok(RemoteId(PeerId::Netcode(transport_id))) = links.get(event.entity) else {
        return;
    };
    let Some(next) = runtime.next_player.checked_add(1) else {
        commands.entity(event.entity).insert(Disconnecting);
        return;
    };
    let position = match spawn.position() {
        Ok(position) => position,
        Err(error) => {
            tracing::warn!(%error, "No safe land spawn available for connecting player");
            commands.entity(event.entity).insert(Disconnecting);
            return;
        }
    };
    let id = PlayerId(runtime.next_player);
    runtime.next_player = next;
    let body = commands
        .spawn((
            body_bundle(position),
            ServerCharacter { link: event.entity },
        ))
        .id();
    let now = Instant::now();
    commands.entity(event.entity).insert(Peer {
        chest_cursor: None,
        creature_action_at: Instant::now(),
        container: None,
        held_slot: None,
        mode: None,
        generation: 0,
        chat: chat::PeerChat::default(),
        uuid: terra_core::PlayerUuid::nil(),
        gender: terra_core::PlayerGender::default(),
        mining: None,
        consumption: None,
        gameplay_sent: None,
        permissions: terra_core::PlayerPermissions {
            game_mode: false,
            build: false,
            flight: false,
            environment: false,
        },
        id,
        transport_id: *transport_id,
        body,
        accepted: false,
        synced: false,
        view: 12,
        known: HashMap::new(),
        inputs: VecDeque::new(),
        received: 0,
        ack: 0,
        look: [0.0; 2],
        last_input: now,
        joined: now,
        last_request: 0,
        edits_left: 20.0,
        rate_time: now,
        spawned: false,
        last_movement: CharacterInput::default(),
        vitals: Default::default(),
        survival: Default::default(),
        stats_sent: None,
    });
}

fn cleanup(
    mut commands: Commands,
    links: Query<(), With<Connected>>,
    peers: Query<(Entity, &Peer)>,
    bodies: Query<(Entity, &ServerCharacter)>,
    mut identities: ResMut<ConnectedProfiles>,
) {
    identities.0.retain(|id, _| {
        peers
            .iter()
            .any(|(entity, peer)| peer.id == *id && links.contains(entity))
    });
    for (body, character) in &bodies {
        if links.get(character.link).is_err() {
            commands.entity(body).despawn();
        }
    }
    for (entity, peer) in &peers {
        if !peer.accepted && peer.joined.elapsed() > Duration::from_secs(10) {
            commands.entity(entity).insert(Disconnecting);
        }
    }
}

#[derive(SystemParam)]
struct ControlWorld<'w, 's> {
    sleep: Option<ResMut<'w, beds::SleepRuntime>>,
    block_properties: Res<'w, terra_core::block_properties::BlockPropertiesCatalog>,
    recipes: Res<'w, terra_core::crafting::RecipeBook>,
    clock: Res<'w, terra_core::WorldClock>,
    weather: Res<'w, terra_core::WorldWeather>,
    config: Res<'w, GameConfig>,
    terrain: Res<'w, terra_world::streaming::WorldTerrain>,
    chunks: ResMut<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    bodies: Query<'w, 's, &'static Transform, With<ServerCharacter>>,
    identities: ResMut<'w, ConnectedProfiles>,
    gateway: Option<Res<'w, crate::gateway::Gateway>>,
}

type ControlPeers<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Peer,
        &'static mut MessageSender<ServerControl>,
        Option<&'static mut MessageReceiver<ClientControl>>,
    ),
    With<Connected>,
>;

fn receive_controls(mut peers: ControlPeers, mut world: ControlWorld) {
    if world.gateway.is_none() {
        return;
    }
    for (mut peer, mut sender, receiver) in &mut peers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for (index, message) in receiver.receive().enumerate() {
            if index >= 64 {
                break;
            }
            match message {
                ClientControl::LeaveBed if peer.synced => {
                    if let Some(sleep) = world.sleep.as_mut() {
                        sleep.cancel(peer.id);
                    }
                }
                ClientControl::Creature { id, feed, look }
                    if peer.synced && peer.chat.teleport.is_none() =>
                {
                    consumption::cancel(&mut peer, &mut sender);
                    creatures::interact(id, feed, look, &mut peer, &mut world);
                }
                ClientControl::HeldSlot(slot)
                    if peer.accepted
                        && slot
                            .is_none_or(|slot| usize::from(slot) < terra_core::INVENTORY_SLOTS) =>
                {
                    if peer.held_slot != slot {
                        consumption::cancel(&mut peer, &mut sender);
                    }
                    peer.held_slot = slot;
                }
                ClientControl::Consumption { slot, revision } if peer.synced => {
                    consumption::intent(slot, revision, &mut peer, &mut sender, &world);
                }
                ClientControl::Hello {
                    version,
                    generator,
                    effect_catalog,
                    view_distance,
                    profile,
                } => {
                    if peer.accepted {
                        continue;
                    }
                    let Some(gateway) = world.gateway.as_ref() else {
                        continue;
                    };
                    let claim = gateway.validate_claim(peer.transport_id, &profile);
                    if let Err(error) = claim {
                        sender.send::<ControlChannel>(ServerControl::Reject(error));
                        continue;
                    }
                    if world.identities.0.values().any(|p| p.uuid == profile.uuid) {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "This player UUID is already connected".into(),
                        ));
                        continue;
                    }
                    if !world
                        .journal
                        .gameplay()
                        .player_stats
                        .contains_key(&profile.uuid)
                        && world.journal.gameplay().player_stats.len() >= 4096
                    {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "Player save capacity reached".into(),
                        ));
                        continue;
                    }
                    if version != PROTOCOL_VERSION
                        || generator != GENERATOR_VERSION
                        || !(1..=32).contains(&view_distance)
                    {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "Incompatible protocol, generator or view distance".into(),
                        ));
                        continue;
                    }
                    if effect_catalog != terra_core::effect_catalog_fingerprint() {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "Incompatible status effect definitions. Client and server must use the same assets/terra/effects JSON files.".into(),
                        ));
                        continue;
                    }
                    let Ok(body) = world.bodies.get(peer.body) else {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "The player's safe spawn is not available".into(),
                        ));
                        continue;
                    };
                    if !spawn::valid_position(&world.terrain.0, &world.journal, body.translation) {
                        sender.send::<ControlChannel>(ServerControl::Reject(
                            "The world spawn changed while joining. Please reconnect.".into(),
                        ));
                        continue;
                    }
                    let spawn = body.translation.to_array();
                    peer.accepted = true;
                    peer.uuid = profile.uuid;
                    peer.gender = profile.gender;
                    let saved = world
                        .journal
                        .gameplay()
                        .player_stats
                        .get(&profile.uuid)
                        .copied()
                        .unwrap_or_default();
                    peer.mode = saved.game_mode;
                    peer.vitals = if peer.game_mode(&world.config) != terra_core::GameMode::Survival
                    {
                        Default::default()
                    } else {
                        saved.survival
                    };
                    peer.permissions = world.config.server.permissions_for(profile.uuid);
                    peer.view = view_distance;
                    sender.send::<ControlChannel>(ServerControl::Welcome {
                        player: peer.id,
                        uuid: profile.uuid,
                        world: Box::new(world.config.world.clone()),
                        tick_rate: world.config.server.tick_rate,
                        spawn,
                        allow_flight: peer.can_fly(&world.config),
                        allow_build: peer.permissions.build,
                        allow_environment: peer.permissions.environment,
                        game_mode: peer.game_mode(&world.config),
                        clock: *world.clock,
                        weather: *world.weather,
                    });
                    match world.recipes.to_json() {
                        Ok(catalog) => sender.send::<ControlChannel>(ServerControl::Recipes {
                            catalog,
                            error: world.recipes.error.clone(),
                        }),
                        Err(error) => sender.send::<ControlChannel>(ServerControl::Reject(error)),
                    }
                    tracing::info!(player = peer.id.0, uuid=%profile.uuid, name=%profile.name, "Player profile accepted (local, not verified online)");
                    world.identities.0.insert(peer.id, profile);
                }
                ClientControl::ViewDistance(view) if peer.accepted => peer.view = view.clamp(1, 32),
                // Legacy visual preferences must never hide collectible world objects.
                ClientControl::VegetationDensity(_) => {}
                ClientControl::ChestCursor(cursor) if peer.synced => {
                    peer.chest_cursor = cursor
                        .filter(|c| {
                            c.valid()
                                && !peer.vitals.dead()
                                && peer
                                    .container
                                    .as_ref()
                                    .is_some_and(|container| container.anchor == c.anchor)
                        })
                        .map(|cursor| (cursor, Instant::now()));
                }
                ClientControl::Chat(text) if peer.synced => {
                    chat::enqueue(&mut peer, chat::ChatRequest::Text(text));
                }
                ClientControl::Teleport(token) if peer.synced => {
                    chat::enqueue(&mut peer, chat::ChatRequest::Teleport(token));
                }
                ClientControl::Inventory(command)
                    if peer.synced && peer.chat.teleport.is_none() =>
                {
                    consumption::cancel(&mut peer, &mut sender);
                    if peer.vitals.dead()
                        && !matches!(
                            command.action,
                            crate::protocol::inventory::InventoryAction::ReturnCursor
                        )
                    {
                        continue;
                    }
                    gameplay::inventory(command, &mut peer, &mut sender, &mut world);
                }
                ClientControl::Container(command)
                    if peer.synced && !peer.vitals.dead() && peer.chat.teleport.is_none() =>
                {
                    consumption::cancel(&mut peer, &mut sender);
                    containers::transfer(command, &mut peer, &mut sender, &mut world);
                }
                ClientControl::Respawn { generation }
                    if peer.synced
                        && peer.vitals.dead()
                        && generation == peer.generation
                        && peer.chat.teleport.is_none() =>
                {
                    peer.survival.respawn_requested = true;
                }
                ClientControl::Edit(command) if peer.synced && peer.chat.teleport.is_none() => {
                    consumption::cancel(&mut peer, &mut sender);
                    if peer.mining.take().is_some() {
                        sender.send::<ControlChannel>(ServerControl::Mining(None));
                    }
                    let elapsed = peer.rate_time.elapsed().as_secs_f64();
                    peer.edits_left = (peer.edits_left + elapsed * 10.0).min(20.0);
                    peer.rate_time = Instant::now();
                    let outcome = if command.request <= peer.last_request {
                        EditOutcome::Conflict
                    } else if peer.edits_left < 1.0 {
                        EditOutcome::RateLimited
                    } else {
                        peer.edits_left -= 1.0;
                        peer.last_request = command.request;
                        validation::apply(command, &peer, &mut world, false)
                    };
                    sender.send::<ControlChannel>(ServerControl::EditResult {
                        request: command.request,
                        action: command.action,
                        outcome,
                    });
                }
                ClientControl::Mining(intent) if peer.synced && peer.chat.teleport.is_none() => {
                    if intent.target.is_some() {
                        consumption::cancel(&mut peer, &mut sender);
                    }
                    gameplay::intent(intent, &mut peer, &mut sender, &mut world);
                }
                _ => {}
            }
        }
    }
}

fn receive_inputs(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut peers: Query<(Entity, &mut Peer, &mut MessageReceiver<MovementInput>), With<Connected>>,
) {
    for (entity, mut peer, mut receiver) in &mut peers {
        for (count, input) in receiver.receive().enumerate() {
            if !peer.synced {
                continue;
            }
            // Reliable channels are independent: pre-teleport input may arrive
            // after the new destination. It can never move the new generation.
            if input.generation < peer.generation {
                continue;
            }
            if count >= 256
                || input.generation != peer.generation
                || !input.valid()
                || (input.flight_speed.is_some() && !peer.can_fly(&config))
                || peer.inputs.len() >= MAX_INPUT_BACKLOG
                || input.sequence > peer.received.saturating_add(MAX_INPUT_BACKLOG as u64)
            {
                tracing::warn!(
                    player = peer.id.0,
                    sequence = input.sequence,
                    "Invalid movement input; disconnecting peer"
                );
                commands.entity(entity).insert(Disconnecting);
                break;
            }
            if input.sequence <= peer.received {
                continue;
            }
            // Reliable ordered input channel: gaps are invalid, never extra simulation ticks.
            if input.sequence != peer.received + 1 {
                commands.entity(entity).insert(Disconnecting);
                break;
            }
            peer.received = input.sequence;
            peer.last_input = Instant::now();
            peer.inputs.push_back(input);
        }
    }
}

fn simulate(
    mut commands: Commands,
    mut runtime: ResMut<ServerRuntime>,
    mut peers: Query<&mut Peer, With<Connected>>,
    mut bodies: Query<(
        Entity,
        &ServerCharacter,
        &mut Transform,
        &mut avian3d::prelude::LinearVelocity,
        &avian3d::prelude::Collider,
        &mut CharacterGrounded,
    )>,
    simulation: CharacterSimulation,
    spawn: spawn::SpawnEnvironment,
    mut sleep: Option<ResMut<beds::SleepRuntime>>,
) {
    runtime.tick = runtime.tick.saturating_add(1);
    for (entity, character, mut transform, mut velocity, collider, mut grounded) in &mut bodies {
        let Ok(mut peer) = peers.get_mut(character.link) else {
            continue;
        };
        if !peer.accepted {
            continue;
        }
        if !peer.spawned {
            match spawn::loaded_state(
                &spawn.chunks,
                &spawn.terrain.0,
                &spawn.journal,
                transform.translation,
            ) {
                spawn::LoadedSpawn::Waiting => continue,
                spawn::LoadedSpawn::Invalid => {
                    tracing::warn!(
                        player = peer.id.0,
                        "Spawn changed before terrain became ready"
                    );
                    commands.entity(character.link).insert(Disconnecting);
                    continue;
                }
                spawn::LoadedSpawn::Ready => {}
            }
            peer.spawned = true;
        }
        let mut state = CharacterState {
            position: transform.translation,
            velocity: velocity.0,
            grounded: grounded.0,
        };
        // Samples describe input state, not permission to run additional ticks.
        // Coalesce delayed packets so a background-window pause cannot leave a permanent backlog.
        let mut jump = false;
        let mut latest = None;
        for input in peer.inputs.drain(..) {
            jump |= input.jump;
            latest = Some(input);
        }
        if let Some(input) = latest {
            peer.ack = input.sequence;
            peer.look = input.look;
            peer.last_movement = input.character();
        }
        if sleep
            .as_mut()
            .is_some_and(|sleep| sleep.blocks_movement(peer.id, peer.last_movement, jump))
        {
            velocity.0 = Vec3::ZERO;
            peer.mining = None;
            peer.consumption = None;
            let mode = peer.game_mode(&spawn.config);
            survival::advance_rest(
                &mut peer,
                state,
                simulation.head_underwater(state.position),
                simulation.swimming_at(state.position),
                mode,
                simulation.delta_seconds(),
            );
            continue;
        }
        if peer.chat.teleport.is_some() {
            velocity.0 = Vec3::ZERO;
            continue;
        }
        let mut input = if peer.last_input.elapsed() < Duration::from_millis(250) {
            peer.last_movement
        } else {
            CharacterInput {
                flight_speed: peer.last_movement.flight_speed.map(|_| 0.0),
                ..Default::default()
            }
        };
        input.jump = jump || (input.jump && simulation.swimming_at(state.position));
        let mode = peer.game_mode(&spawn.config);
        if mode != terra_core::GameMode::Survival {
            peer.vitals = Default::default();
        }
        if peer.vitals.dead() {
            velocity.0 = Vec3::ZERO;
            peer.mining = None;
            continue;
        }
        let before = state;
        if simulation.survival_step(entity, collider, &mut state, input, mode, &mut peer.vitals) {
            let submerged = simulation.head_underwater(state.position);
            let swimming = simulation.swimming_at(state.position);
            survival::advance(
                &mut peer,
                before,
                state,
                submerged,
                swimming,
                mode,
                simulation.delta_seconds(),
            );
        }
        transform.translation = state.position;
        velocity.0 = state.velocity;
        grounded.0 = state.grounded;
    }
}

fn update_foci(
    (config, mut streaming): (
        Res<GameConfig>,
        ResMut<terra_world::streaming::StreamingSettings>,
    ),
    peers: Query<&Peer, With<Connected>>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    mut foci: ResMut<ChunkFoci>,
    focus: Res<ChunkFocus>,
    mut runtime: ResMut<ServerRuntime>,
    gateway: Option<Res<crate::gateway::Gateway>>,
) {
    let mut points = Vec::new();
    for peer in &peers {
        if peer.accepted
            && let Ok(body) = bodies.get(peer.body)
        {
            points.push(
                peer.chat
                    .teleport
                    .as_ref()
                    .map_or(body.translation, |t| t.position)
                    .to_array()
                    .map(f64::from),
            );
        }
    }
    runtime.connected_players = points.len();
    // No renderer on the authority: async terrain must exist throughout the habitat radius.
    streaming.radius = if !points.is_empty()
        && config
            .server
            .natural_mob_limit
            .unwrap_or(config.world.natural_mob_limit)
            > 0
    {
        i64::from(config.server.simulation_distance).max(10)
    } else {
        i64::from(config.server.simulation_distance.max(1))
    };
    if let Some(gateway) = gateway {
        gateway.update_players(points.len());
    }
    if points.is_empty() {
        points.push(focus.0);
    }
    foci.0 = Some(points);
}

fn send_deltas(
    mut peers: Query<(&mut Peer, &mut MessageSender<ServerControl>), With<Connected>>,
    bodies: Query<&Transform, With<ServerCharacter>>,
    journal: Res<EditJournal>,
) {
    for (mut peer, mut sender) in &mut peers {
        if !peer.accepted {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let center =
            terra_voxel::global_to_section(body.translation.to_array().map(|v| v.floor() as i64)).0;
        let destination = peer.chat.teleport.as_ref().map(|t| {
            terra_voxel::global_to_section(t.position.to_array().map(|v| v.floor() as i64)).0
        });
        let radius = i64::from(peer.view) + 1;
        let relevant = |p: &terra_voxel::SectionPos| {
            ((p.x - center.x).abs() <= radius && (p.z - center.z).abs() <= radius)
                || destination
                    .is_some_and(|d| (p.x - d.x).abs() <= radius && (p.z - d.z).abs() <= radius)
        };
        peer.known.retain(|p, _| relevant(p));
        let mut pending: Vec<_> = journal
            .changed_sections()
            .filter(|p| relevant(p))
            .filter_map(|p| {
                let version = (
                    journal.revision(p),
                    journal.prop_epoch(p),
                    journal.appearance_epoch(p),
                    journal.fluid_epoch(p),
                );
                (peer.known.get(&p) != Some(&version)).then_some((p, version))
            })
            .collect();
        pending.sort_unstable_by_key(|(p, _)| (p.y, p.z, p.x));
        let count = pending.len();
        for (position, version) in pending.into_iter().take(4) {
            sender.send::<ControlChannel>(ServerControl::Section(journal.section_delta(position)));
            peer.known.insert(position, version);
        }
        if let Some(teleport) = peer.chat.teleport.as_mut() {
            teleport.deltas_ready = count <= 4;
        }
        if !peer.synced && peer.spawned && count <= 4 {
            peer.synced = true;
            sender.send::<ControlChannel>(ServerControl::Ready);
        }
    }
}

fn send_players(
    runtime: Res<ServerRuntime>,
    config: Res<GameConfig>,
    journal: Res<EditJournal>,
    peers: Query<&Peer, With<Connected>>,
    bodies: Query<
        (
            &Transform,
            &avian3d::prelude::LinearVelocity,
            &CharacterGrounded,
        ),
        With<ServerCharacter>,
    >,
    mut senders: Query<(&Peer, &mut MessageSender<PlayerStates>), With<Connected>>,
    mut previous: Local<u64>,
) {
    // Snapshot frequency is 20 Hz at the default 60 Hz simulation rate.
    if runtime.tick.saturating_sub(*previous) < u64::from(config.server.tick_rate.div_ceil(20)) {
        return;
    }
    *previous = runtime.tick;
    let mut players = Vec::new();
    let spectators: std::collections::HashSet<_> = peers
        .iter()
        .filter(|peer| peer.game_mode(&config) == terra_core::GameMode::Spectator)
        .map(|peer| peer.id)
        .collect();
    for peer in &peers {
        if !peer.accepted || !peer.spawned {
            continue;
        }
        if let Ok((transform, velocity, grounded)) = bodies.get(peer.body) {
            players.push(PlayerState {
                gender: peer.gender,
                generation: peer.generation,
                player: peer.id,
                ack: peer.ack,
                position: transform.translation.to_array(),
                velocity: velocity.0.to_array(),
                grounded: grounded.0,
                look: peer.look,
                survival: peer.vitals,
                held_item: peer
                    .held_slot
                    .and_then(|slot| {
                        journal
                            .gameplay()
                            .inventories
                            .get(&peer.uuid)?
                            .slot(usize::from(slot))
                    })
                    .map(|s| s.item),
                mining: peer.mining.is_some(),
                consumption: peer
                    .consumption
                    .as_ref()
                    .map(|state| state.progress(Instant::now())),
                held_torch: peer
                    .held_slot
                    .and_then(|slot| {
                        journal
                            .gameplay()
                            .inventories
                            .get(&peer.uuid)?
                            .slot(usize::from(slot))
                    })
                    .is_some_and(|stack| stack.item == terra_core::ItemKind::Block(40)),
            });
        }
    }
    players.sort_unstable_by_key(|p| p.player.0);
    for (peer, mut sender) in &mut senders {
        if !peer.synced {
            continue;
        }
        let Some(own) = players.iter().find(|p| p.player == peer.id) else {
            continue;
        };
        let range = f32::from(peer.view) * 16.0 + 32.0;
        let relevant = players
            .iter()
            .copied()
            .filter(|p| {
                (p.player == peer.id || !spectators.contains(&p.player))
                    && (p.position[0] - own.position[0]).abs() <= range
                    && (p.position[2] - own.position[2]).abs() <= range
            })
            .collect();
        sender.send::<SnapshotChannel>(PlayerStates {
            tick: runtime.tick,
            players: relevant,
        });
    }
}

#[derive(Default)]
struct ClockBroadcast {
    time: f64,
    lightning: Option<u64>,
    rules: Option<terra_core::WorldRuleSettings>,
}

fn send_clock(
    time: Res<Time<Real>>,
    config: Res<GameConfig>,
    clock: Res<terra_core::WorldClock>,
    weather: Res<terra_core::WorldWeather>,
    mut published: Local<ClockBroadcast>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    let latest = weather.latest_lightning.map(|event| event.id);
    let settings = terra_core::WorldRuleSettings::from_world(&config.world);
    let changed = published.rules != Some(settings);
    if time.elapsed_secs_f64() - published.time < 0.5 && published.lightning == latest && !changed {
        return;
    }
    published.lightning = latest;
    published.time = time.elapsed_secs_f64();
    published.rules = Some(settings);
    for (peer, mut sender) in &mut peers {
        if peer.accepted {
            if changed {
                sender.send::<ControlChannel>(ServerControl::GameRules(Box::new(settings)));
            }
            sender.send::<ControlChannel>(ServerControl::WorldTime(*clock));
            sender.send::<ControlChannel>(ServerControl::Weather(*weather));
        }
    }
}

#[path = "server_beds.rs"]
mod beds;
#[path = "server_chat.rs"]
mod chat;
#[path = "server_consumption.rs"]
mod consumption;
#[path = "server_containers.rs"]
mod containers;
#[path = "server_creatures.rs"]
mod creatures;
#[path = "server_furnaces.rs"]
mod furnaces;
#[path = "server_furniture.rs"]
mod furniture;
#[path = "server_gameplay.rs"]
mod gameplay;
#[path = "server_lightning.rs"]
mod lightning;
#[path = "server_social.rs"]
mod social;
#[path = "server_spawn.rs"]
mod spawn;
#[path = "server_survival.rs"]
mod survival;
#[path = "server_validation.rs"]
mod validation;
