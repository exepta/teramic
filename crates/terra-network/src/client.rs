//! Network replica, confirmed world deltas and shared-controller prediction.
use crate::{connection::ConnectionStatus, protocol::session::*};
use bevy::{ecs::system::SystemParam, prelude::*};
use lightyear::prelude::{client::*, *};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
use terra_core::{Config, GENERATOR_VERSION, GameConfig, PlayerId, PlayerProfile, PlayerUuid};
use terra_physics::character::{
    CharacterBody, CharacterInput, CharacterSimulation, CharacterState,
};
use terra_world::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{
        BlockEditRequest, ChunkFocus, ChunkStore, PropRemoveRequest, SectionEditApplied,
        StreamingSet, WorldAccess, WorldTerrain,
    },
};

#[path = "client_chat.rs"]
mod chat;
pub use chat::{ChatCommand, ChatEntry, ClientChat, ClientChatCommands, LocateResult};

#[derive(Resource, Clone)]
pub struct ClientLaunch {
    pub token: Vec<u8>,
}

#[derive(Message)]
pub struct ReconnectRequest;

#[derive(Message)]
pub struct MiningRequest(pub MiningIntent);

#[derive(Message, Clone, Copy, Debug)]
pub struct ConsumptionRequest(pub Option<u8>);

#[derive(Message, Clone, Copy, Debug)]
pub struct ObjectInteractionRequest {
    pub position: [i64; 3],
    pub expected: u16,
    pub facing: u8,
}

#[derive(Message)]
pub struct InventoryRequest(pub crate::protocol::inventory::InventoryCommand);

#[derive(Message, Clone, Debug)]
pub struct ContainerRequest(pub crate::protocol::inventory::ContainerCommand);

#[derive(Resource, Default)]
pub struct CursorShare(pub Option<crate::protocol::social::ChestCursorInput>);
#[derive(Resource, Default)]
pub struct SocialState {
    pub names: HashMap<PlayerId, String>,
    pub cursors: Vec<crate::protocol::social::SharedCursor>,
    pub received: Option<Instant>,
}
#[derive(Message)]
pub struct RespawnRequest;
#[derive(Resource, Default, Debug)]
pub struct SelectedInventorySlot(pub Option<u8>);
#[derive(Message)]
pub struct PropPlaceRequest {
    pub support: [i64; 3],
    pub kind: terra_core::ResourcePropKind,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ConfirmedEdit {
    pub request: u64,
    pub action: EditAction,
    pub outcome: EditOutcome,
}

/// Begin a new menu-selected session using the existing replica and protocol.
pub fn connect_session(world: &mut World, token: Vec<u8>) {
    if let Some(entity) = world.resource::<ClientSession>().connection {
        world.despawn(entity);
    }
    let bodies: Vec<_> = world
        .query_filtered::<Entity, With<CharacterBody>>()
        .iter(world)
        .collect();
    for entity in bodies {
        world.despawn(entity);
    }
    terra_world::streaming::reset_replica(world);
    world.insert_resource(ClientSession::default());
    world.insert_resource(crate::world_time::ClockReplica::default());
    world.insert_resource(crate::weather::WeatherReplica::default());
    world.insert_resource(ReplicatedPlayers::default());
    world.insert_resource(SocialState::default());
    world.insert_resource(CursorShare::default());
    world.insert_resource(ClientChat::default());
    world.insert_resource(ClientChatCommands::default());
    world.insert_resource(ClientLaunch { token });
    start(world);
}

/// Keep the link alive for the transport's final disconnect flush this frame.
pub fn disconnect_session(world: &mut World) {
    world.insert_resource(crate::world_time::ClockReplica::default());
    world.insert_resource(crate::weather::WeatherReplica::default());
    if let Some(entity) = world.resource::<ClientSession>().connection {
        world.trigger(Disconnect { entity });
        world.flush();
    }
    let mut session = world.resource_mut::<ClientSession>();
    session.ready = false;
    session.pending.clear();
    session.outbox.clear();
    session.pending_inputs = 0;
    world.resource_mut::<ReplicatedPlayers>().players.clear();
    terra_world::streaming::reset_replica(world);
}

#[derive(Resource)]
pub struct ClientSession {
    pub sleepers: Vec<SleepingPlayer>,
    pub wake_requested: bool,
    pub container: Option<crate::protocol::inventory::ContainerSnapshot>,
    pub teleporting: bool,
    generation: u32,
    pub player: Option<PlayerId>,
    pub uuid: Option<PlayerUuid>,
    pub ready: bool,
    pub allow_flight: bool,
    pub allow_build: bool,
    pub allow_environment: bool,
    pub game_mode: terra_core::GameMode,
    pub mining: Option<MiningProgress>,
    pub consumption: Option<ConsumptionProgress>,
    consumption_received: Option<Instant>,
    pub consumption_completed: u64,
    pub furnaces: Vec<FurnaceVisual>,
    pub inventory: terra_core::Inventory,
    pub stats: terra_core::PlayerStats,
    pub respawn_error: Option<String>,
    predicted_vitals: terra_core::SurvivalVitals,
    pub last_inventory: Option<crate::protocol::inventory::InventoryOutcome>,
    pub inventory_response: u64,
    pub drops: Vec<terra_core::WorldDrop>,
    pub spawn: Vec3,
    pub look: [f32; 2],
    pub error: Option<String>,
    pub last_edit: Option<(u64, EditOutcome)>,
    pub connection: Option<Entity>,
    pub pending_inputs: usize,
    sequence: u64,
    request: u64,
    pending: VecDeque<MovementInput>,
    outbox: Vec<MovementInput>,
    control: VecDeque<ServerControl>,
    started: Instant,
    ever_connected: bool,
    latest: Option<(u64, PlayerState)>,
    reconciled: u64,
    last_view: u8,
    last_density: f32,
}
impl Default for ClientSession {
    fn default() -> Self {
        Self {
            sleepers: Vec::new(),
            wake_requested: false,
            container: None,
            teleporting: false,
            generation: 0,
            player: None,
            uuid: None,
            ready: false,
            allow_flight: false,
            allow_build: false,
            allow_environment: false,
            game_mode: terra_core::GameMode::Survival,
            mining: None,
            consumption: None,
            consumption_received: None,
            consumption_completed: 0,
            furnaces: Vec::new(),
            inventory: Default::default(),
            stats: Default::default(),
            respawn_error: None,
            predicted_vitals: Default::default(),
            last_inventory: None,
            inventory_response: 0,
            drops: Vec::new(),
            spawn: Vec3::ZERO,
            look: [0.0; 2],
            error: None,
            last_edit: None,
            connection: None,
            pending_inputs: 0,
            sequence: 0,
            request: 0,
            pending: VecDeque::new(),
            outbox: Vec::new(),
            control: VecDeque::new(),
            started: Instant::now(),
            ever_connected: false,
            latest: None,
            reconciled: 0,
            last_view: 0,
            last_density: 1.0,
        }
    }
}
impl ClientSession {
    pub fn sleeping(&self) -> Option<SleepingPlayer> {
        self.ready.then_some(())?;
        self.sleepers
            .iter()
            .find(|sleep| Some(sleep.player) == self.player)
            .copied()
    }
    pub fn dead(&self) -> bool {
        self.game_mode == terra_core::GameMode::Survival && self.stats.survival.dead()
    }
    pub fn edits_pending(&self) -> bool {
        self.last_edit.map_or(0, |(id, _)| id) < self.request
    }
    pub fn confirmed_state(&self) -> Option<PlayerState> {
        self.latest.map(|(_, state)| state)
    }
    /// Advance only the presentation briefly between authoritative progress updates.
    pub fn display_consumption(&self) -> Option<ConsumptionProgress> {
        self.consumption.map(|progress| {
            displayed_consumption(
                progress,
                self.consumption_received
                    .map_or(Duration::ZERO, |received| received.elapsed()),
            )
        })
    }
    pub fn edit_command(&mut self, revision: u64, action: EditAction) -> Option<EditCommand> {
        let request = self.next_request()?;
        Some(EditCommand {
            request,
            revision,
            look: self.look,
            action,
            source: None,
        })
    }
    pub fn predict(
        &mut self,
        entity: Entity,
        collider: &avian3d::prelude::Collider,
        state: &mut CharacterState,
        input: CharacterInput,
        simulation: &CharacterSimulation,
    ) -> bool {
        if !self.ready || self.teleporting || self.error.is_some() {
            return false;
        }
        if self.pending.len() >= MAX_INPUT_BACKLOG {
            tracing::warn!(sequence=self.sequence,ack=?self.confirmed_state().map(|s|s.ack),"Input acknowledgement backlog exceeded limit");
            self.fail("Server input acknowledgement timed out");
            return false;
        }
        if self.dead() || self.predicted_vitals.dead() {
            state.velocity = Vec3::ZERO;
            return true;
        }
        if !simulation.survival_step(
            entity,
            collider,
            state,
            input,
            self.game_mode,
            &mut self.predicted_vitals,
        ) {
            return false;
        }
        self.predicted_vitals
            .effects
            .advance(simulation.delta_seconds());
        let Some(sequence) = self.sequence.checked_add(1) else {
            self.fail("Input sequence exhausted");
            return false;
        };
        self.sequence = sequence;
        let packet = MovementInput {
            generation: self.generation,
            sequence,
            direction: input.direction.to_array(),
            look: self.look,
            jump: input.jump,
            sprint: input.sprint,
            auto_climb: input.auto_climb,
            flight_speed: input.flight_speed,
        };
        self.pending.push_back(packet);
        self.outbox.push(packet);
        self.pending_inputs = self.pending.len();
        true
    }
    fn fail(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.ready = false;
    }
    fn next_request(&mut self) -> Option<u64> {
        self.request = self.request.checked_add(1)?;
        Some(self.request)
    }
}
pub fn token_server_address(bytes: &[u8]) -> Result<std::net::SocketAddr, String> {
    let token =
        lightyear::netcode::ConnectToken::try_from_bytes(bytes).map_err(|e| e.to_string())?;
    NetcodeClient::new(Authentication::Token(token), NetcodeConfig::default())
        .map(|n| n.inner.server_addr())
        .map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct RemoteTrack {
    pub previous: PlayerState,
    pub current: PlayerState,
    pub received: Instant,
    pub interval: f32,
}
impl RemoteTrack {
    pub fn display_consumption(&self) -> Option<ConsumptionProgress> {
        self.current
            .consumption
            .map(|progress| displayed_consumption(progress, self.received.elapsed()))
    }
    pub fn position(&self) -> Vec3 {
        let previous = Vec3::from_array(self.previous.position);
        let current = Vec3::from_array(self.current.position);
        if previous.distance_squared(current) > 256.0 {
            return current;
        }
        previous.lerp(
            current,
            (self.received.elapsed().as_secs_f32() / self.interval).clamp(0.0, 1.0),
        )
    }
}

fn displayed_consumption(
    mut progress: ConsumptionProgress,
    elapsed: Duration,
) -> ConsumptionProgress {
    progress.progress = (progress.progress
        + elapsed.min(Duration::from_millis(150)).as_secs_f32() / progress.duration)
        .min(1.0);
    progress
}

#[derive(Resource, Default)]
pub struct ReplicatedPlayers {
    pub players: HashMap<PlayerId, RemoteTrack>,
    pub tick: u64,
}

pub(crate) fn install(app: &mut App) {
    creatures::install(app);
    chat::install(app);
    app.init_resource::<ClientSession>()
        .init_resource::<crate::world_time::ClockReplica>()
        .init_resource::<crate::weather::WeatherReplica>()
        .add_systems(
            First,
            crate::world_time::advance_client.after(bevy::time::TimeSystems),
        )
        .add_systems(
            First,
            crate::weather::advance_client.after(crate::world_time::advance_client),
        )
        .init_resource::<PlayerProfile>()
        .init_resource::<ReplicatedPlayers>()
        .add_message::<ReconnectRequest>()
        .add_message::<MiningRequest>()
        .add_message::<ConsumptionRequest>()
        .add_message::<ObjectInteractionRequest>()
        .add_message::<InventoryRequest>()
        .add_message::<ContainerRequest>()
        .init_resource::<SocialState>()
        .init_resource::<CursorShare>()
        .add_message::<RespawnRequest>()
        .add_message::<PropPlaceRequest>()
        .init_resource::<SelectedInventorySlot>()
        .add_message::<ConfirmedEdit>()
        .insert_resource(WorldAccess::Replica { ready: false })
        .add_systems(Startup, start)
        .add_observer(connected)
        .add_systems(
            Update,
            (
                retry,
                receive_controls,
                receive_snapshots,
                reconcile,
                check_link,
            )
                .chain()
                .before(StreamingSet::Focus),
        )
        .add_systems(Update, send_edits.after(StreamingSet::Edits))
        .add_systems(Update, send_mining.after(StreamingSet::Edits))
        .add_systems(Update, send_consumption.after(send_mining))
        .add_systems(Update, send_inventory.after(StreamingSet::Edits))
        .add_systems(Update, send_cursor.after(StreamingSet::Edits))
        .add_systems(Update, send_respawn.after(StreamingSet::Edits))
        .add_systems(PostUpdate, send_inputs.before(MessageSystems::Send));
}

#[path = "client_creatures.rs"]
mod creatures;
pub use creatures::{CreatureActions, CreatureTrack, ReplicatedCreatures};

fn retry(world: &mut World) {
    if world
        .resource_mut::<Messages<ReconnectRequest>>()
        .drain()
        .next()
        .is_none()
    {
        return;
    }
    let connection = world.resource::<ClientSession>().connection;
    if let Some(entity) = connection {
        if world.get::<Connected>(entity).is_some() {
            return;
        }
        world.despawn(entity);
    }
    let bodies: Vec<_> = world
        .query_filtered::<Entity, With<CharacterBody>>()
        .iter(world)
        .collect();
    for entity in bodies {
        world.despawn(entity);
    }
    terra_world::streaming::reset_replica(world);
    world.insert_resource(ClientSession::default());
    world.insert_resource(ReplicatedPlayers::default());
    world.insert_resource(crate::world_time::ClockReplica::default());
    world.insert_resource(crate::weather::WeatherReplica::default());
    start(world);
}

fn start(world: &mut World) {
    let Some(launch) = world.get_resource::<ClientLaunch>() else {
        return;
    };
    let result = lightyear::netcode::ConnectToken::try_from_bytes(&launch.token)
        .map_err(|e| e.to_string())
        .and_then(|token| {
            NetcodeClient::new(Authentication::Token(token), NetcodeConfig::default())
                .map_err(|e| e.to_string())
        });
    match result {
        Ok(netcode) => {
            let bind = if netcode.inner.server_addr().is_ipv6() {
                std::net::SocketAddr::from(([0_u16; 8], 0))
            } else {
                std::net::SocketAddr::from(([0, 0, 0, 0], 0))
            };
            let entity = world
                .spawn((
                    netcode,
                    UdpIo::default(),
                    LocalAddr(bind),
                    MessageManager::default(),
                ))
                .id();
            let mut session = world.resource_mut::<ClientSession>();
            session.connection = Some(entity);
            session.started = Instant::now();
            *world.resource_mut::<ConnectionStatus>() = ConnectionStatus::Connecting;
            world.trigger(Connect { entity });
        }
        Err(error) => {
            world.resource_mut::<ClientSession>().fail(error);
            *world.resource_mut::<ConnectionStatus>() = ConnectionStatus::Failed;
        }
    }
}

fn connected(
    event: On<Add, Connected>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
    config: Res<GameConfig>,
    profile: Res<PlayerProfile>,
    mut session: ResMut<ClientSession>,
    mut status: ResMut<ConnectionStatus>,
) {
    let Ok(mut sender) = senders.get_mut(event.entity) else {
        return;
    };
    session.ever_connected = true;
    let view = config.client.view_distance.clamp(1, 32);
    session.last_view = view;
    sender.send::<ControlChannel>(ClientControl::Hello {
        version: PROTOCOL_VERSION,
        generator: GENERATOR_VERSION,
        effect_catalog: terra_core::effect_catalog_fingerprint(),
        view_distance: view,
        profile: profile.clone(),
    });
    *status = ConnectionStatus::Synchronizing;
}

#[derive(SystemParam)]
struct ReplicaWorld<'w> {
    social: Option<ResMut<'w, SocialState>>,
    recipes: ResMut<'w, terra_core::crafting::RecipeBook>,
    clock: ResMut<'w, terra_core::WorldClock>,
    clock_replica: ResMut<'w, crate::world_time::ClockReplica>,
    weather: ResMut<'w, terra_core::WorldWeather>,
    weather_replica: ResMut<'w, crate::weather::WeatherReplica>,
    chat: ResMut<'w, ClientChat>,
    profile: Res<'w, PlayerProfile>,
    access: ResMut<'w, WorldAccess>,
    config: ResMut<'w, GameConfig>,
    terrain: ResMut<'w, WorldTerrain>,
    chunks: ResMut<'w, ChunkStore>,
    journal: ResMut<'w, EditJournal>,
    focus: ResMut<'w, ChunkFocus>,
    fixed: ResMut<'w, Time<Fixed>>,
    status: ResMut<'w, ConnectionStatus>,
    confirmed: MessageWriter<'w, ConfirmedEdit>,
    section_edits: MessageWriter<'w, SectionEditApplied>,
}

fn apply_section_delta(
    chunks: &mut ResMut<'_, ChunkStore>,
    journal: &mut ResMut<'_, EditJournal>,
    applied: &mut MessageWriter<'_, SectionEditApplied>,
    delta: terra_world::edits::SectionDelta,
) -> Result<(), String> {
    let position = delta.position;
    let stamp = |journal: &EditJournal| {
        (
            journal.revision(position),
            journal.mesh_epoch(position),
            journal.prop_epoch(position),
            journal.appearance_epoch(position),
            journal.fluid_epoch(position),
            journal.water_mesh_epoch(position),
        )
    };
    let previous = stamp(journal);
    // Imports still validate every payload. Bookkeeping for duplicates or stale
    // snapshots must not look like an unexplained whole-world mutation.
    let result = chunks
        .bypass_change_detection()
        .apply_confirmed_delta(journal.bypass_change_detection(), delta);
    if stamp(journal) != previous {
        chunks.set_changed();
        journal.set_changed();
        if result.is_ok() {
            applied.write(SectionEditApplied { position });
        }
    }
    result
}

fn receive_controls(
    mut commands: Commands,
    mut receivers: Query<&mut MessageReceiver<ServerControl>, With<Client>>,
    mut session: ResMut<ClientSession>,
    mut world: ReplicaWorld,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            if session.control.len() >= 4096 {
                session.fail("World synchronization queue exceeded limit");
                break;
            }
            session.control.push_back(message);
        }
    }
    for _ in 0..8 {
        let Some(message) = session.control.pop_front() else {
            break;
        };
        let result: Result<(), String> = match message {
            ServerControl::GameRules(rules) => {
                if session.player.is_none() || !rules.valid() {
                    Err("Invalid world gamerules".into())
                } else {
                    rules.apply(&mut world.config.world);
                    world
                        .terrain
                        .bypass_change_detection()
                        .0
                        .set_world_rules(*rules);
                    Ok(())
                }
            }
            ServerControl::SleepingPlayers(sleepers) => {
                let mut seen = std::collections::HashSet::new();
                if sleepers.len() > 1024
                    || sleepers
                        .iter()
                        .any(|s| !s.valid() || !seen.insert(s.player))
                {
                    Err("Invalid sleeping player snapshot".into())
                } else {
                    session.sleepers = sleepers;
                    Ok(())
                }
            }
            ServerControl::Recipes { catalog, error } => {
                if error.as_ref().is_some_and(|error| error.len() > 2048) {
                    Err("Invalid recipe error".into())
                } else {
                    terra_core::crafting::RecipeBook::from_json(&catalog).map(|mut book| {
                        book.error = error;
                        *world.recipes = book;
                    })
                }
            }
            ServerControl::Welcome {
                player,
                uuid,
                world: config,
                tick_rate,
                spawn,
                allow_flight,
                allow_build,
                allow_environment,
                game_mode,
                clock,
                weather,
            } => {
                let config = *config;
                if session.player.is_some()
                    || !clock.valid()
                    || !weather.valid()
                    || uuid.is_nil()
                    || uuid != world.profile.uuid
                    || config.validate().is_err()
                    || !(1..=240).contains(&tick_rate)
                    || spawn
                        .iter()
                        .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
                {
                    Err("Invalid or repeated world handshake".into())
                } else {
                    match TerraGenerator::new(config.clone()) {
                        Ok(generator) => {
                            world.config.world = config;
                            *world.clock = clock;
                            world.clock_replica.0 = Some(clock);
                            *world.weather = weather;
                            world.weather_replica.0 = Some(weather);
                            world.config.server.tick_rate = tick_rate;
                            *world.fixed = Time::<Fixed>::from_hz(f64::from(tick_rate));
                            world.terrain.0 = generator;
                            session.player = Some(player);
                            session.uuid = Some(uuid);
                            session.allow_flight = allow_flight;
                            session.allow_build = allow_build;
                            session.allow_environment = allow_environment;
                            session.game_mode = game_mode;
                            session.spawn = Vec3::from_array(spawn);
                            world.focus.0 = spawn.map(f64::from);
                            Ok(())
                        }
                        Err(error) => Err(error.to_string()),
                    }
                }
            }
            ServerControl::WorldTime(clock) => {
                if session.player.is_none() || !clock.valid() {
                    Err("Invalid world time snapshot".into())
                } else {
                    if world.clock.paused != clock.paused
                        || world.clock.day_length_seconds != clock.day_length_seconds
                    {
                        *world.clock = clock;
                    }
                    world.clock_replica.0 = Some(clock);
                    Ok(())
                }
            }
            ServerControl::Weather(weather) => {
                if session.player.is_none() || !weather.valid() {
                    Err("Invalid world weather snapshot".into())
                } else {
                    if world.weather_replica.0.is_none() || world.clock.paused {
                        *world.weather = weather;
                    }
                    world.weather.latest_lightning = weather.latest_lightning;
                    world.weather.elapsed_total = world
                        .weather
                        .elapsed_total
                        .max(weather.latest_lightning.map_or(0.0, |event| event.at));
                    world.weather_replica.0 = Some(weather);
                    Ok(())
                }
            }
            ServerControl::Section(delta) => {
                if session.player.is_none() {
                    Err("World delta before handshake".into())
                } else {
                    apply_section_delta(
                        &mut world.chunks,
                        &mut world.journal,
                        &mut world.section_edits,
                        delta,
                    )
                }
            }
            ServerControl::Ready => {
                if session.player.is_none() {
                    Err("World ready before handshake".into())
                } else {
                    session.ready = true;
                    *world.access = WorldAccess::Replica { ready: true };
                    *world.status = ConnectionStatus::Connected;
                    tracing::info!(player=?session.player,"Authoritative world synchronized");
                    Ok(())
                }
            }
            ServerControl::EditResult {
                request,
                action,
                outcome,
            } => {
                if request != 0 {
                    session.last_edit = Some((request, outcome));
                }
                world.confirmed.write(ConfirmedEdit {
                    request,
                    action,
                    outcome,
                });
                Ok(())
            }
            ServerControl::Mining(progress) => {
                if progress.is_some_and(|p| {
                    !p.progress.is_finite()
                        || !(0.0..=1.0).contains(&p.progress)
                        || !p.duration.is_finite()
                        || !(0.001..=3600.0).contains(&p.duration)
                }) {
                    Err("Invalid mining progress".into())
                } else {
                    session.mining = progress;
                    Ok(())
                }
            }
            ServerControl::Consumption(progress) => {
                if progress.is_some_and(|p| !p.valid()) {
                    Err("Invalid consumption progress".into())
                } else {
                    session.consumption = progress;
                    session.consumption_received = progress.map(|_| Instant::now());
                    Ok(())
                }
            }
            ServerControl::ConsumptionFinished { inventory } => {
                if !inventory.valid() {
                    Err("Invalid consumption inventory".into())
                } else {
                    session.inventory = inventory;
                    session.consumption = None;
                    session.consumption_received = None;
                    session.consumption_completed = session.consumption_completed.saturating_add(1);
                    Ok(())
                }
            }
            ServerControl::Gameplay { inventory, drops } => {
                if !inventory.valid() || drops.len() > 256 || drops.iter().any(|d| !d.valid()) {
                    Err("Invalid gameplay snapshot".into())
                } else {
                    session.inventory = inventory;
                    session.drops = drops;
                    Ok(())
                }
            }
            ServerControl::Stats { generation, stats } => {
                if !stats.survival.valid() {
                    Err("Invalid survival snapshot".into())
                } else {
                    if generation == session.generation {
                        session.stats = stats;
                        if session.latest.is_none() {
                            session.predicted_vitals = stats.survival;
                        }
                    }
                    Ok(())
                }
            }
            ServerControl::Chat(entry) => {
                if entry.text.len() > 2048 || entry.author.as_ref().is_some_and(|a| a.len() > 128) {
                    Err("Invalid chat response".into())
                } else {
                    world.chat.push(entry);
                    Ok(())
                }
            }
            ServerControl::ChatPlayers(players) => {
                if players.len() > 1024 || players.iter().any(|name| name.len() > 128) {
                    Err("Invalid chat player roster".into())
                } else {
                    world.chat.players = players;
                    Ok(())
                }
            }
            ServerControl::TeleportPreparing => {
                session.consumption = None;
                session.consumption_received = None;
                chat::preparing(&mut world.chat, &mut session);
                Ok(())
            }
            ServerControl::GameMode {
                mode,
                allow_flight,
                tick,
                state,
            } => chat::reset_prediction(&mut session, tick, state).map(|()| {
                session.game_mode = mode;
                session.stats.game_mode = Some(mode);
                session.allow_flight = allow_flight;
                session.mining = None;
                session.consumption = None;
                session.consumption_received = None;
            }),
            ServerControl::TeleportCancelled => {
                chat::cancelled(&mut world.chat, &mut session);
                Ok(())
            }
            ServerControl::Teleport { tick, state } => {
                world.focus.0 = state.position.map(f64::from);
                chat::destination(&mut world.chat, &mut session, tick, state)
            }
            ServerControl::Reject(reason) => Err(reason),
            ServerControl::InventoryResult { outcome, inventory } => {
                if !inventory.valid() {
                    Err("Invalid inventory result".into())
                } else {
                    session.inventory = inventory;
                    session.last_inventory = Some(outcome);
                    session.inventory_response = session.inventory_response.saturating_add(1);
                    Ok(())
                }
            }
            ServerControl::Social { names, cursors } => {
                if names.as_ref().is_some_and(|names| names.len() > 128)
                    || cursors.len() > 32
                    || cursors.iter().any(|c| !c.cursor.valid())
                {
                    Err("Invalid multiplayer presence".into())
                } else {
                    if let Some(social) = world.social.as_mut() {
                        if let Some(names) = names {
                            social.names = names.into_iter().collect();
                        }
                        social.cursors = cursors;
                        social.received = Some(Instant::now());
                    } else {
                        commands.insert_resource(SocialState {
                            names: names.unwrap_or_default().into_iter().collect(),
                            cursors,
                            received: Some(Instant::now()),
                        });
                    }
                    Ok(())
                }
            }
            ServerControl::Furnaces(furnaces) => {
                if furnaces.len() > 64 || furnaces.iter().any(|f| !f.valid()) {
                    Err("Invalid furnace visual snapshot".into())
                } else {
                    session.furnaces = furnaces;
                    Ok(())
                }
            }
            ServerControl::Container(container) => {
                if container.as_ref().is_some_and(|snapshot| !snapshot.valid()) {
                    Err("Invalid world inventory snapshot".into())
                } else {
                    session.container = container;
                    Ok(())
                }
            }
        };
        if let Err(error) = result {
            session.fail(error);
            break;
        }
    }
}

fn send_cursor(
    time: Res<Time>,
    session: Res<ClientSession>,
    share: Res<CursorShare>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
    mut last: Local<(f64, Option<crate::protocol::social::ChestCursorInput>)>,
) {
    let now = time.elapsed_secs_f64();
    if !session.ready
        || now - last.0 < 0.1
        || (share.0 == last.1 && (share.0.is_none() || now - last.0 < 0.4))
    {
        return;
    }
    if let Some(mut sender) = senders.iter_mut().next() {
        sender.send::<ControlChannel>(ClientControl::ChestCursor(share.0));
        *last = (now, share.0);
    }
}

fn send_inventory(
    mut requests: MessageReader<InventoryRequest>,
    mut containers: MessageReader<ContainerRequest>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    for request in requests.read().take(8) {
        if session.ready
            && !session.teleporting
            && let Some(mut sender) = senders.iter_mut().next()
        {
            sender.send::<ControlChannel>(ClientControl::Inventory(request.0));
        }
    }
    for request in containers.read().take(8) {
        if session.ready
            && !session.teleporting
            && let Some(mut sender) = senders.iter_mut().next()
        {
            sender.send::<ControlChannel>(ClientControl::Container(request.0.clone()));
        }
    }
}

fn send_respawn(
    mut requests: MessageReader<RespawnRequest>,
    mut session: ResMut<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    if requests.read().next().is_some()
        && session.ready
        && session.dead()
        && !session.teleporting
        && let Some(mut sender) = senders.iter_mut().next()
    {
        session.respawn_error = None;
        sender.send::<ControlChannel>(ClientControl::Respawn {
            generation: session.generation,
        });
    }
}

fn send_mining(
    mut requests: MessageReader<MiningRequest>,
    session: Res<ClientSession>,
    selected: Res<SelectedInventorySlot>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
    mut held: Local<Option<(Option<PlayerId>, Option<u8>)>>,
) {
    let intent = requests.read().last().map(|request| request.0);
    if !session.ready {
        *held = None;
        return;
    }
    if let Some(mut sender) = senders.iter_mut().next() {
        let current = (session.player, selected.0);
        if *held != Some(current) || intent.is_some() {
            sender.send::<ControlChannel>(ClientControl::HeldSlot(selected.0));
            *held = Some(current);
        }
        if let Some(intent) = intent {
            sender.send::<ControlChannel>(ClientControl::Mining(intent));
        }
    }
}

fn send_consumption(
    mut requests: MessageReader<ConsumptionRequest>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    let request = requests.read().last().copied();
    if let Some(ConsumptionRequest(slot)) = request
        && session.ready
        && let Ok(mut sender) = senders.single_mut()
    {
        sender.send::<ControlChannel>(ClientControl::Consumption {
            slot,
            revision: session.inventory.revision,
        });
    }
}

fn receive_snapshots(
    mut receivers: Query<&mut MessageReceiver<PlayerStates>, With<Client>>,
    mut session: ResMut<ClientSession>,
    mut players: ResMut<ReplicatedPlayers>,
    config: Res<GameConfig>,
) {
    for mut receiver in &mut receivers {
        for packet in receiver.receive() {
            if !session.ready || packet.tick <= players.tick || packet.players.len() > 1024 {
                continue;
            }
            if packet.players.iter().any(|p| {
                !p.survival.valid()
                    || p.consumption.is_some_and(|progress| !progress.valid())
                    || p.position
                        .iter()
                        .chain(&p.velocity)
                        .chain(&p.look)
                        .any(|v| !v.is_finite())
            }) {
                session.fail("Invalid player snapshot");
                break;
            }
            let interval = (packet.tick.saturating_sub(players.tick) as f32
                / f32::from(config.server.tick_rate))
            .clamp(0.001, 0.25);
            players.tick = packet.tick;
            players
                .players
                .retain(|id, _| packet.players.iter().any(|p| p.player == *id));
            for state in packet.players {
                if Some(state.player) == session.player {
                    if state.generation != session.generation {
                        continue;
                    }
                    session.latest = Some((packet.tick, state));
                    continue;
                }
                let mut previous = players
                    .players
                    .get(&state.player)
                    .map_or(state, |track| track.current);
                if let Some(track) = players.players.get(&state.player) {
                    previous.position = track.position().to_array();
                }
                players.players.insert(
                    state.player,
                    RemoteTrack {
                        previous,
                        current: state,
                        received: Instant::now(),
                        interval,
                    },
                );
            }
        }
    }
}

fn reconcile(
    mut session: ResMut<ClientSession>,
    simulation: CharacterSimulation,
    mut bodies: terra_physics::character::ControlledBodies,
) {
    let Some((tick, confirmed)) = session.latest else {
        return;
    };
    if tick <= session.reconciled || !session.ready {
        return;
    }
    let Some((entity, mut transform, mut velocity, collider, mut grounded, pose)) =
        bodies.iter_mut().next()
    else {
        return;
    };
    if confirmed.ack > session.sequence {
        session.fail("Invalid input acknowledgement");
        return;
    }
    session.reconciled = tick;
    while session
        .pending
        .front()
        .is_some_and(|i| i.sequence <= confirmed.ack)
    {
        session.pending.pop_front();
    }
    session.pending_inputs = session.pending.len();
    let mut state = CharacterState {
        position: Vec3::from_array(confirmed.position),
        velocity: Vec3::from_array(confirmed.velocity),
        grounded: confirmed.grounded,
    };
    let mut vitals = confirmed.survival;
    for input in &session.pending {
        if !simulation.survival_step(
            entity,
            collider,
            &mut state,
            input.character(),
            session.game_mode,
            &mut vitals,
        ) {
            break;
        }
        vitals.effects.advance(simulation.delta_seconds());
    }
    session.predicted_vitals = vitals;
    if let Some(mut pose) = pose {
        pose.reconcile(transform.translation, state.position);
    }
    transform.translation = state.position;
    velocity.0 = state.velocity;
    grounded.0 = state.grounded;
}

fn check_link(
    mut commands: Commands,
    mut session: ResMut<ClientSession>,
    links: Query<(), With<Connected>>,
    mut status: ResMut<ConnectionStatus>,
    mut access: ResMut<WorldAccess>,
    mut players: ResMut<ReplicatedPlayers>,
) {
    let Some(entity) = session.connection else {
        return;
    };
    if session.ever_connected && links.get(entity).is_err() && session.error.is_none() {
        session.ready = false;
        session.error = Some("Server connection lost".into());
        *status = ConnectionStatus::Disconnected;
    }
    if !session.ready
        && session.started.elapsed() > Duration::from_secs(30)
        && session.error.is_none()
    {
        session.fail("Server connection or synchronization timed out");
    }
    if session.error.is_some() {
        session.ready = false;
        *access = WorldAccess::Replica { ready: false };
        players.players.clear();
        if *status != ConnectionStatus::Disconnected {
            *status = ConnectionStatus::Failed;
        }
        if links.get(entity).is_ok() {
            commands.trigger(Disconnect { entity });
        }
    }
}

fn send_inputs(
    mut session: ResMut<ClientSession>,
    mut senders: Query<
        (
            &mut MessageSender<MovementInput>,
            &mut MessageSender<ClientControl>,
        ),
        With<Client>,
    >,
    config: Res<GameConfig>,
) {
    if !session.ready {
        session.outbox.clear();
        return;
    }
    let Some((mut sender, mut control)) = senders.iter_mut().next() else {
        return;
    };
    if std::mem::take(&mut session.wake_requested) {
        control.send::<ControlChannel>(ClientControl::LeaveBed);
    }
    for input in session.outbox.drain(..) {
        sender.send::<InputChannel>(input);
    }
    let view = config.client.view_distance.clamp(1, 32);
    if view != session.last_view {
        session.last_view = view;
        control.send::<ControlChannel>(ClientControl::ViewDistance(view));
    }
    let density = 1.0;
    if density != session.last_density {
        session.last_density = density;
        control.send::<ControlChannel>(ClientControl::VegetationDensity(density));
    }
}

#[derive(SystemParam)]
struct EditRequests<'w, 's> {
    objects: MessageReader<'w, 's, ObjectInteractionRequest>,
    blocks: MessageReader<'w, 's, BlockEditRequest>,
    props: MessageReader<'w, 's, PropRemoveRequest>,
    planted: MessageReader<'w, 's, PropPlaceRequest>,
    surfaces: MessageReader<'w, 's, terra_world::environment::SurfaceConditionRequest>,
}

fn send_edits(
    mut requests: EditRequests,
    mut session: ResMut<ClientSession>,
    journal: Res<EditJournal>,
    selected: Res<SelectedInventorySlot>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    let mut actions = Vec::new();
    for request in requests.objects.read() {
        actions.push(EditAction::ToggleObject {
            position: request.position,
            expected: request.expected,
            facing: request.facing,
        });
    }
    for edit in requests.blocks.read() {
        actions.push(EditAction::Block {
            position: edit.position,
            expected: edit.expected_material,
            material: edit.material,
        });
    }
    for edit in requests.props.read() {
        actions.push(EditAction::Prop {
            section: edit.section,
            id: edit.id,
        });
    }
    for edit in requests.surfaces.read() {
        actions.push(EditAction::Surface {
            position: edit.position,
            condition: edit.condition,
        });
    }
    for edit in requests.planted.read() {
        actions.push(EditAction::PlaceProp {
            support: edit.support,
            kind: edit.kind,
        });
    }
    if !session.ready {
        return;
    }
    let Some(mut sender) = senders.iter_mut().next() else {
        return;
    };
    for action in actions.into_iter().take(20) {
        let revision = match action {
            EditAction::Block { position, .. } | EditAction::ToggleObject { position, .. } => {
                journal.revision(terra_voxel::global_to_section(position).0)
            }
            EditAction::Prop { section, .. } => journal.prop_epoch(section),
            EditAction::PlaceProp { support, .. } => {
                journal.prop_epoch(terra_voxel::global_to_section(support).0)
            }
            EditAction::Surface { position, .. } => {
                journal.appearance_epoch(terra_voxel::global_to_section(position).0)
            }
        };
        let Some(mut command) = session.edit_command(revision, action) else {
            session.fail("Edit request sequence exhausted");
            break;
        };
        if matches!(
            action,
            EditAction::Block { material: 1.., .. }
                | EditAction::PlaceProp { .. }
                | EditAction::ToggleObject {
                    expected: 71..=73,
                    ..
                }
        ) {
            command.source = selected.0.map(|slot| InventorySource {
                slot,
                revision: session.inventory.revision,
            });
        }
        sender.send::<ControlChannel>(ClientControl::Edit(command));
    }
}

#[cfg(test)]
#[path = "client_section_tests.rs"]
mod section_tests;

#[cfg(test)]
mod consumption_presentation_tests {
    use super::*;

    #[test]
    fn display_progress_moves_between_packets_but_never_changes_authority_or_runs_ahead() {
        let progress = ConsumptionProgress {
            slot: 0,
            item: terra_core::ItemKind::Animal(terra_core::creatures::AnimalProduct::CookedFish),
            progress: 0.25,
            duration: 1.6,
        };
        let early = displayed_consumption(progress, Duration::from_millis(30));
        let later = displayed_consumption(progress, Duration::from_millis(60));
        assert!(early.progress > progress.progress && later.progress > early.progress);
        assert_eq!(
            displayed_consumption(progress, Duration::from_secs(8)),
            displayed_consumption(progress, Duration::from_millis(150))
        );
        assert_eq!(progress.progress, 0.25);
        assert_eq!(later.slot, progress.slot);
        assert_eq!(later.item, progress.item);
        assert_eq!(later.duration, progress.duration);
        assert_eq!(
            displayed_consumption(
                ConsumptionProgress {
                    progress: 0.99,
                    ..progress
                },
                Duration::from_millis(150),
            )
            .progress,
            1.0
        );
        let session = ClientSession {
            consumption: Some(progress),
            ..default()
        };
        assert_eq!(session.display_consumption(), Some(progress));
        assert!(ClientSession::default().display_consumption().is_none());
    }
}
