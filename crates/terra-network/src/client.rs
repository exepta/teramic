//! Network replica, confirmed world deltas and shared-controller prediction.
#[path = "client_train_motion.rs"]
mod train_motion_display;
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

/// Hard bound on predicted, unacknowledged simulation steps.
pub const MAX_PREDICTION_LEAD: usize = 64;
const PREDICTION_LEAD_SECONDS: f32 = 0.3;
const INPUT_ACK_TIMEOUT: Duration = Duration::from_secs(30);

fn prediction_lead(delta_seconds: f32) -> usize {
    (PREDICTION_LEAD_SECONDS / delta_seconds)
        .ceil()
        .clamp(8.0, MAX_PREDICTION_LEAD as f32) as usize
}

#[path = "client_trees.rs"]
mod trees;

#[path = "client_chat.rs"]
mod chat;
pub use chat::{ChatCommand, ChatEntry, ClientChat, ClientChatCommands, LocateResult};
pub use creatures::BeeNestActions;

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
#[derive(Resource, Default)]
pub struct CrankInput(pub Option<[i64; 3]>);

#[derive(Message, Clone, Copy, Debug)]
pub struct TreeHarvestRequest {
    pub id: u64,
    pub fruit_revision: u64,
    pub slot: u8,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ObjectInteractionRequest {
    pub position: [i64; 3],
    pub expected: u16,
    pub facing: u8,
    pub closing: bool,
}

#[derive(Message)]
pub struct InventoryRequest(pub crate::protocol::inventory::InventoryCommand);

#[derive(Message, Clone, Debug)]
pub struct ContainerRequest(pub crate::protocol::inventory::ContainerCommand);

#[derive(Message, Clone, Debug)]
pub struct KingdomRequest(pub crate::protocol::kingdom::KingdomCommand);

#[derive(Message, Clone, Debug)]
pub struct TrainRequest(pub crate::protocol::trains::TrainCommand);

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
pub struct FarmToolRequest {
    pub position: [i64; 3],
    pub action: terra_core::farming::FarmingAction,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ObjectRotateRequest {
    pub position: [i64; 3],
    pub expected: u16,
    pub facing: u8,
    pub reverse: bool,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ConveyorConnectRequest {
    pub start: [i64; 3],
    pub end: [i64; 3],
    pub start_revision: u64,
    pub item: terra_core::ItemKind,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct RailBuildRequest {
    pub position: [i64; 3],
    pub expected: u16,
    pub choice: terra_core::rails::BuildChoice,
    pub signature: u64,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct RailConnectRequest {
    pub start: terra_core::rails::Endpoint,
    pub position: [i64; 3],
    pub expected: u16,
}

/// Local construction intent. Its token survives cancellation without reusing
/// a previous server request or guessing which edit acknowledgment belongs to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailPlacementAction {
    Seed {
        position: [i64; 3],
        expected: u16,
        signature: u64,
    },
    Connect {
        start: terra_core::rails::Endpoint,
        position: [i64; 3],
        expected: u16,
    },
    Stretch {
        start: terra_core::rails::Endpoint,
        position: [i64; 3],
        expected: u16,
        signature: u64,
    },
}
impl RailPlacementAction {
    pub fn edit_action(self) -> EditAction {
        match self {
            Self::Seed {
                position,
                expected,
                signature,
            } => EditAction::SeedRail {
                position,
                expected,
                signature,
            },
            Self::Connect {
                start,
                position,
                expected,
            } => EditAction::ConnectRail {
                start,
                position,
                expected,
            },
            Self::Stretch {
                start,
                position,
                expected,
                signature,
            } => EditAction::StretchRail {
                start,
                position,
                expected,
                signature,
            },
        }
    }
}

#[derive(Message, Clone, Copy, Debug)]
pub struct RailPlacementRequest {
    pub token: u64,
    pub action: RailPlacementAction,
}

/// Emitted only after an intent has been assigned and sent its wire request.
#[derive(Message, Clone, Copy, Debug)]
pub struct RailPlacementSent {
    pub token: u64,
    pub request: u64,
    pub action: EditAction,
}

#[derive(Message, Clone, Copy, Debug)]
pub enum ConveyorInteractionRequest {
    Collect {
        id: u64,
    },
    Dye {
        position: [i64; 3],
        color: terra_core::beds::DyeColor,
    },
    DyeFabric {
        position: [i64; 3],
        color: terra_core::beds::DyeColor,
    },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ConfirmedEdit {
    pub request: u64,
    pub action: EditAction,
    pub outcome: EditOutcome,
}

#[derive(Message, Clone)]
pub struct TreeCollapseEvent(pub terra_world::tree_felling::Collapse);

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
    world.insert_resource(terra_physics::TrainCollisionViews::default());
    world.insert_resource(crate::world_time::ClockReplica::default());
    world.insert_resource(crate::weather::WeatherReplica::default());
    world.insert_resource(ReplicatedPlayers::default());
    world.insert_resource(SocialState::default());
    world.insert_resource(CursorShare::default());
    world.insert_resource(CrankInput::default());
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
    session.mounted = None;
    session.dismount_requested = false;
    session.pending.clear();
    session.outbox.clear();
    session.pending_inputs = 0;
    session.crop_progress.clear();
    session.open_chests.clear();
    session.watering_reserve = (0, 0);
    session.infusion.clear();
    session.kingdoms = Default::default();
    session.trains = Default::default();
    session.train_cargo = None;
    session.train_seat = None;
    session.train_motion_tracks.clear();
    session.rail_preparation = None;
    session.last_train_result = None;
    session.train_response = 0;
    session.train_request = 0;
    session.trees = Default::default();
    session.tree_patch_signature = None;
    session.last_kingdom_result = None;
    session.kingdom_response = 0;
    world.resource_mut::<ReplicatedPlayers>().players.clear();
    terra_world::streaming::reset_replica(world);
}

#[derive(Resource)]
pub struct ClientSession {
    pub trees: terra_core::TreeRegistry,
    pub trains: terra_core::trains::TrainSnapshot,
    pub train_cargo: Option<crate::protocol::trains::TrainCargoSnapshot>,
    train_motion_tracks: std::collections::BTreeMap<u64, train_motion_display::Track>,
    train_display_time: Instant,
    rail_preparation: Option<bevy::tasks::Task<ServerControl>>,
    pub train_seat: Option<terra_core::trains::SeatBinding>,
    pub last_train_result: Option<crate::protocol::trains::TrainResult>,
    pub train_response: u64,
    train_request: u64,
    tree_patch_signature: Option<u64>,
    pub kingdoms: terra_core::kingdom::KingdomRegistry,
    pub last_kingdom_result: Option<crate::protocol::kingdom::KingdomResult>,
    pub kingdom_response: u64,
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
    pub cauldrons: Vec<CauldronVisual>,
    pub campfires: Vec<CampfireVisual>,
    pub plumbing: Vec<PlumbingVisual>,
    /// Local receipt counter for accepted plumbing snapshots; not a wire clock.
    pub plumbing_sequence: u64,
    pub infusion: Vec<terra_core::infusion::HoldVisual>,
    pub crushers: Vec<CrusherVisual>,
    pub kinetics: Vec<KineticVisual>,
    pub crop_progress: std::collections::BTreeMap<[i64; 3], CropProgress>,
    pub watering_reserve: (u8, u8),
    pub open_chests: std::collections::BTreeSet<[i64; 3]>,
    pub stalker_grip: Option<StalkerGrip>,
    pub mounted: Option<MountedState>,
    pub dismount_requested: bool,
    pub(super) stalker_grip_at: Option<Instant>,
    pub inventory: terra_core::Inventory,
    pub stats: terra_core::PlayerStats,
    pub respawn_error: Option<String>,
    predicted_vitals: terra_core::SurvivalVitals,
    pub last_inventory: Option<crate::protocol::inventory::InventoryOutcome>,
    pub inventory_response: u64,
    pub drops: Vec<terra_core::WorldDrop>,
    pub spawn: Vec3,
    pub initial_location: Option<terra_core::PlayerLocation>,
    pub look: [f32; 2],
    pub error: Option<String>,
    pub last_edit: Option<(u64, EditOutcome)>,
    pub connection: Option<Entity>,
    pub pending_inputs: usize,
    sequence: u64,
    request: u64,
    kingdom_request: u64,
    pending: VecDeque<MovementInput>,
    outbox: Vec<MovementInput>,
    control: VecDeque<ServerControl>,
    started: Instant,
    ever_connected: bool,
    latest: Option<(u64, PlayerState)>,
    reconciled: u64,
    acknowledged: u64,
    acknowledged_at: Instant,
    last_view: u8,
    last_density: f32,
}
impl Default for ClientSession {
    fn default() -> Self {
        Self {
            trains: Default::default(),
            train_cargo: None,
            train_motion_tracks: Default::default(),
            train_display_time: Instant::now(),
            rail_preparation: None,
            train_seat: None,
            last_train_result: None,
            train_response: 0,
            train_request: 0,
            kingdoms: Default::default(),
            trees: Default::default(),
            tree_patch_signature: None,
            last_kingdom_result: None,
            kingdom_response: 0,
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
            cauldrons: Vec::new(),
            campfires: Vec::new(),
            plumbing: Vec::new(),
            plumbing_sequence: 0,
            infusion: Vec::new(),
            crushers: Vec::new(),
            kinetics: Vec::new(),
            crop_progress: Default::default(),
            watering_reserve: (0, 0),
            open_chests: Default::default(),
            stalker_grip: None,
            mounted: None,
            dismount_requested: false,
            stalker_grip_at: None,
            inventory: Default::default(),
            stats: Default::default(),
            respawn_error: None,
            predicted_vitals: Default::default(),
            last_inventory: None,
            inventory_response: 0,
            drops: Vec::new(),
            spawn: Vec3::ZERO,
            initial_location: None,
            look: [0.0; 2],
            error: None,
            last_edit: None,
            connection: None,
            pending_inputs: 0,
            sequence: 0,
            request: 0,
            kingdom_request: 0,
            pending: VecDeque::new(),
            outbox: Vec::new(),
            control: VecDeque::new(),
            started: Instant::now(),
            ever_connected: false,
            latest: None,
            reconciled: 0,
            acknowledged: 0,
            acknowledged_at: Instant::now(),
            last_view: 0,
            last_density: 1.0,
        }
    }
}
impl ClientSession {
    fn observe_acknowledgment(&mut self, acknowledged: u64) {
        if acknowledged > self.acknowledged && acknowledged <= self.sequence {
            self.acknowledged = acknowledged;
            self.acknowledged_at = Instant::now();
        }
    }

    fn prediction_capacity(&mut self, delta_seconds: f32) -> bool {
        let acknowledged = self.confirmed_state().map_or(0, |state| state.ack);
        if acknowledged > self.sequence {
            self.fail("Invalid input acknowledgement");
            return false;
        }
        self.observe_acknowledgment(acknowledged);
        // Acknowledged commands can no longer participate in any replay from
        // this authoritative snapshot, even when its collider is still pending.
        while self
            .pending
            .front()
            .is_some_and(|input| input.sequence <= acknowledged)
        {
            self.pending.pop_front();
        }
        self.pending_inputs = self.pending.len();
        self.pending.len() < prediction_lead(delta_seconds)
    }

    fn check_acknowledgment_liveness(&mut self) {
        // Evaluate after receive_snapshots, so a resumed application processes
        // buffered ACK progress before declaring its server unresponsive.
        if self.ready
            && self.error.is_none()
            && self
                .pending
                .back()
                .is_some_and(|input| input.sequence > self.acknowledged)
            && self.acknowledged_at.elapsed() >= INPUT_ACK_TIMEOUT
        {
            self.fail("Server input acknowledgement timed out");
        }
    }
    pub fn active_stalker_grip(&self) -> Option<StalkerGrip> {
        (self.ready
            && !self.dead()
            && !self.teleporting
            && self.error.is_none()
            && self.game_mode == terra_core::GameMode::Survival)
            .then_some(())?;
        self.stalker_grip_at
            .filter(|at| at.elapsed() <= Duration::from_millis(350))?;
        self.stalker_grip
    }
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
    /// Reserve the response identity before a UI starts waiting, including when
    /// another vehicle request is still queued or arrives after that UI closes.
    pub fn train_request(
        &mut self,
        action: crate::protocol::trains::TrainAction,
    ) -> Option<TrainRequest> {
        self.train_request = self.train_request.checked_add(1)?;
        Some(TrainRequest(crate::protocol::trains::TrainCommand {
            request: self.train_request,
            action,
        }))
    }
    pub fn kingdom_request(
        &mut self,
        action: crate::protocol::kingdom::KingdomAction,
    ) -> Option<KingdomRequest> {
        let request = self.kingdom_request.checked_add(1)?;
        self.kingdom_request = request;
        Some(KingdomRequest(crate::protocol::kingdom::KingdomCommand {
            request,
            action,
        }))
    }
    pub fn confirmed_state(&self) -> Option<PlayerState> {
        self.latest.map(|(_, state)| state)
    }
    /// Vehicle, running gear and riders sample the same presentation time each frame.
    pub fn displayed_train_motion(
        &self,
        vehicle: &terra_core::trains::VehicleView,
    ) -> terra_core::train_motion::TrainMotion {
        self.train_motion_tracks
            .get(&vehicle.id)
            .map_or(vehicle.motion, |track| track.at(self.train_display_time))
    }
    /// Shared seat coordinates are feet; movement uses the capsule center.
    pub fn seated_state(&self) -> Option<CharacterState> {
        let binding = self
            .train_seat
            .filter(|binding| Some(binding.player) == self.uuid)?;
        let vehicle = self
            .trains
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == binding.vehicle)?;
        let (feet, _) = vehicle.seat_pose_at(
            usize::from(binding.part),
            self.displayed_train_motion(vehicle),
        )?;
        Some(CharacterState {
            position: Vec3::from_array(feet.map(|value| value as f32))
                + Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT,
            velocity: Vec3::ZERO,
            grounded: true,
        })
    }
    #[cfg(test)]
    pub(crate) fn prediction_sequence_for_test(&self) -> u64 {
        self.sequence
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
        if !self.prediction_capacity(simulation.delta_seconds()) {
            return false;
        }
        if self.dead() || self.predicted_vitals.dead() {
            state.velocity = Vec3::ZERO;
            return true;
        }
        let seated = self.seated_state();
        let input = if seated.is_some() {
            CharacterInput::default()
        } else {
            input
        };
        let predicted_input = self.active_stalker_grip().map_or(input, |grip| {
            grip.movement(
                state.position,
                simulation.delta_seconds(),
                self.predicted_vitals
                    .effects
                    .movement_multiplier(simulation.swimming_at(state.position)),
            )
        });
        let moved = if let Some(seated) = seated {
            *state = seated;
            true
        } else if let Some(mount) = self.mounted.as_mut() {
            predict_riding(mount, state, input, simulation);
            true
        } else {
            simulation.survival_step(
                entity,
                collider,
                state,
                predicted_input,
                self.game_mode,
                &mut self.predicted_vitals,
            )
        };
        if !moved {
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
        if self.pending.is_empty() {
            self.acknowledged_at = Instant::now();
        }
        self.pending.push_back(packet);
        self.outbox.push(packet);
        self.pending_inputs = self.pending.len();
        true
    }
    fn fail(&mut self, error: impl Into<String>) {
        self.kingdoms = Default::default();
        self.trains = Default::default();
        self.train_cargo = None;
        self.train_seat = None;
        self.train_motion_tracks.clear();
        self.rail_preparation = None;
        self.last_train_result = None;
        self.train_response = 0;
        self.train_request = 0;
        self.trees = Default::default();
        self.tree_patch_signature = None;
        self.last_kingdom_result = None;
        self.mounted = None;
        self.dismount_requested = false;
        self.stalker_grip = None;
        self.stalker_grip_at = None;
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
    pub fn mounted_yaw(&self) -> Option<f32> {
        self.mounted_yaw_at(
            (self.received.elapsed().as_secs_f32() / self.interval.max(0.001)).clamp(0.0, 1.0),
        )
    }
    fn mounted_yaw_at(&self, fraction: f32) -> Option<f32> {
        let mount = self.current.mounted?;
        let previous = self
            .previous
            .mounted
            .filter(|m| m.id == mount.id)
            .unwrap_or(mount);
        let delta = (mount.yaw - previous.yaw + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        Some(previous.yaw + delta * fraction.clamp(0.0, 1.0))
    }
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

#[cfg(test)]
mod mounted_interpolation_tests {
    use super::*;
    #[test]
    fn new_mount_snapshot_continues_from_the_presented_yaw_and_wraps_the_short_arc() {
        let kind = terra_core::creatures::CreatureKind::Horse;
        let horse = terra_core::creatures::Creature {
            id: (1 << 62) + 502,
            kind,
            home: [0, 0],
            position: [0.0, 1.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: terra_core::creatures::CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(kind),
        };
        let mount = MountedState::new(&horse, 20.0, true);
        let mut track = RemoteTrack {
            previous: PlayerState {
                mounted: Some(mount),
                ..default()
            },
            current: PlayerState {
                mounted: Some(MountedState { yaw: 1.0, ..mount }),
                ..default()
            },
            received: Instant::now(),
            interval: 0.1,
        };
        let presented = track.mounted_yaw_at(0.4).unwrap();
        assert!((presented - 0.4).abs() < 0.0001);
        track.previous = track.current;
        track.previous.mounted.as_mut().unwrap().yaw = presented;
        track.current.mounted.as_mut().unwrap().yaw = 1.3;
        assert_eq!(track.mounted_yaw_at(0.0), Some(presented));
        assert!(track.mounted_yaw_at(0.1).unwrap() > presented);
        track.previous.mounted.as_mut().unwrap().yaw = 3.13;
        track.current.mounted.as_mut().unwrap().yaw = -3.13;
        assert!((track.mounted_yaw_at(0.5).unwrap().abs() - std::f32::consts::PI).abs() < 0.001);
    }
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
        .init_resource::<CrankInput>()
        .add_message::<ObjectInteractionRequest>()
        .add_message::<RailConnectRequest>()
        .add_message::<RailBuildRequest>()
        .add_message::<RailPlacementRequest>()
        .add_message::<RailPlacementSent>()
        .add_message::<InventoryRequest>()
        .add_message::<ContainerRequest>()
        .add_message::<KingdomRequest>()
        .add_message::<TrainRequest>()
        .init_resource::<SocialState>()
        .init_resource::<CursorShare>()
        .add_message::<RespawnRequest>()
        .add_message::<PropPlaceRequest>()
        .add_message::<TreeHarvestRequest>()
        .add_message::<FarmToolRequest>()
        .add_message::<ObjectRotateRequest>()
        .add_message::<ConveyorConnectRequest>()
        .add_message::<ConveyorInteractionRequest>()
        .init_resource::<SelectedInventorySlot>()
        .add_message::<ConfirmedEdit>()
        .add_message::<TreeCollapseEvent>()
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
        .add_systems(
            Update,
            send_edits.after(StreamingSet::Edits).after(send_mining),
        )
        .add_systems(Update, send_mining.after(StreamingSet::Edits))
        .add_systems(Update, send_consumption.after(send_mining))
        .add_systems(Update, send_inventory.after(StreamingSet::Edits))
        .add_systems(Update, send_kingdoms.after(StreamingSet::Edits))
        .add_systems(Update, send_trains.after(StreamingSet::Edits))
        .add_systems(Update, send_cursor.after(StreamingSet::Edits))
        .add_systems(Update, send_respawn.after(StreamingSet::Edits))
        .add_systems(PostUpdate, send_inputs.before(MessageSystems::Send));
    app.add_systems(
        PostUpdate,
        send_crank.after(send_inputs).before(MessageSystems::Send),
    );
}

type CrankSent = (Option<PlayerId>, u32, Option<[i64; 3]>, Instant);
fn send_crank(
    input: Res<CrankInput>,
    session: Res<ClientSession>,
    mut sent: Local<Option<CrankSent>>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    if !session.ready {
        *sent = None;
        return;
    }
    let target = if session.dead() || session.teleporting {
        None
    } else {
        input.0
    };
    let now = Instant::now();
    if sent
        .as_ref()
        .is_some_and(|(player, generation, previous, at)| {
            *player == session.player
                && *generation == session.generation
                && *previous == target
                && (target.is_none() || now.duration_since(*at) < Duration::from_millis(100))
        })
    {
        return;
    }
    if let Ok(mut sender) = senders.single_mut() {
        sender.send::<ControlChannel>(ClientControl::Crank {
            position: target,
            look: session.look,
        });
        *sent = Some((session.player, session.generation, target, now));
    }
}

#[path = "client_creatures.rs"]
mod creatures;
pub use creatures::{
    CreatureActions, CreatureInspection, CreatureTrack, PetCommands, PlayerAttacks,
    ReplicatedCreatures,
};

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
    event: On<Add<Connected>>,
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
        equipment_catalog: terra_core::equipment_catalog_fingerprint(),
        crushing_catalog: terra_core::crusher::catalog_fingerprint(),
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
    tree_collapse: MessageWriter<'w, TreeCollapseEvent>,
    section_edits: MessageWriter<'w, SectionEditApplied>,
    train_collision_views: Option<ResMut<'w, terra_physics::TrainCollisionViews>>,
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

fn clear_invalid_train_cargo(
    session: &mut ClientSession,
    vehicles: &[terra_core::trains::VehicleView],
) {
    if session.train_cargo.as_ref().is_some_and(|cargo| {
        vehicles
            .iter()
            .find(|vehicle| vehicle.id == cargo.vehicle)
            .is_none_or(|vehicle| {
                Some(vehicle.owner) != session.uuid
                    || vehicle
                        .parts
                        .get(usize::from(cargo.part))
                        .is_none_or(|part| !matches!(part.material, 29 | 216 | 218))
            })
    }) {
        session.train_cargo = None;
    }
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
    let import_started = Instant::now();
    for processed in 0..8 {
        if processed != 0 && import_started.elapsed() >= Duration::from_millis(2) {
            break;
        }
        let message = if let Some(task) = session.rail_preparation.as_mut() {
            let Some(message) = bevy::tasks::futures_lite::future::block_on(
                bevy::tasks::futures_lite::future::poll_once(task),
            ) else {
                break;
            };
            session.rail_preparation = None;
            message
        } else {
            let Some(message) = session.control.pop_front() else {
                break;
            };
            if let ServerControl::Section(delta) = &message
                && delta
                    .objects
                    .iter()
                    .any(|(_, state)| state.rail_curve.is_some())
            {
                session.rail_preparation =
                    Some(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
                        if let ServerControl::Section(delta) = &message {
                            let materials: std::collections::BTreeMap<_, _> =
                                delta.blocks.iter().copied().collect();
                            for (cell, state) in &delta.objects {
                                if let Some(curve) = state.rail_curve
                                    && let Some(&material) = materials.get(cell)
                                    && let Some(piece) = terra_core::rails::Piece::from_facing(
                                        [0; 3],
                                        material,
                                        state.facing,
                                    )
                                    .and_then(|piece| piece.with_curve(curve))
                                {
                                    let _ = piece.footprint();
                                    let _ = piece.cell_boxes([0; 3]);
                                }
                            }
                        }
                        message
                    }));
                break;
            }
            message
        };
        let result: Result<(), String> = match message {
            ServerControl::TreeCollapse(collapse) => {
                if session.player.is_none() || !collapse.valid() {
                    Err("Invalid tree collapse snapshot".into())
                } else {
                    world.tree_collapse.write(TreeCollapseEvent(collapse));
                    Ok(())
                }
            }
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
                location,
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
                    || location.is_some_and(|location| {
                        !location.valid()
                            || location.position != spawn
                            || location.flying && !allow_flight
                    })
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
                            session.initial_location = location;
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
                    if world.weather_replica.0.is_none()
                        || world.clock.paused
                        || !world.config.world.game_rules.weather_cycle
                    {
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
                let stats = *stats;
                if !valid_equipment_vitals(stats.survival, stats.equipment) {
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
            } => chat::reset_prediction(&mut session, tick, *state).map(|()| {
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
                chat::destination(&mut world.chat, &mut session, tick, *state)
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
            ServerControl::ChestViewers(anchors) => {
                if anchors.len() > 256
                    || anchors.iter().flatten().any(|v| v.unsigned_abs() > 990_000)
                {
                    Err("Invalid chest viewer snapshot".into())
                } else {
                    session.open_chests = anchors.into_iter().collect();
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
            ServerControl::Cauldrons(cauldrons) => {
                if cauldrons.len() > 64 || cauldrons.iter().any(|value| !value.valid()) {
                    Err("Invalid cauldron visual snapshot".into())
                } else {
                    session.cauldrons = cauldrons;
                    Ok(())
                }
            }
            ServerControl::Campfires(campfires) => {
                if campfires.len() > 64 || campfires.iter().any(|value| !value.valid()) {
                    Err("Invalid campfire visual snapshot".into())
                } else {
                    session.campfires = campfires;
                    Ok(())
                }
            }
            ServerControl::Infusion(infusion) => {
                if infusion.len() > terra_core::infusion::MAX_HOLD_VISUALS
                    || infusion.iter().any(|view| !view.valid())
                    || infusion
                        .iter()
                        .map(|view| view.position)
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != infusion.len()
                {
                    Err("Invalid infusion station snapshot".into())
                } else {
                    session.infusion = infusion;
                    Ok(())
                }
            }
            ServerControl::Plumbing(plumbing) => {
                if plumbing.len() > 256
                    || plumbing.iter().any(|value| !value.valid())
                    || plumbing
                        .iter()
                        .map(|value| value.position)
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != plumbing.len()
                {
                    Err("Invalid plumbing visual snapshot".into())
                } else {
                    world
                        .journal
                        .replace_confirmed_basins(
                            plumbing
                                .iter()
                                .filter_map(|node| node.basin.map(|shape| (shape, node.amount))),
                        )
                        .map(|()| {
                            session.plumbing = plumbing;
                            session.plumbing_sequence = session.plumbing_sequence.wrapping_add(1);
                        })
                }
            }
            ServerControl::Crushers(crushers) => {
                if crushers.len() > 64 || crushers.iter().any(|value| !value.valid()) {
                    Err("Invalid crusher visual snapshot".into())
                } else {
                    session.crushers = crushers;
                    Ok(())
                }
            }
            ServerControl::Kinetics(kinetics) => {
                if kinetics.len() > 256
                    || kinetics.iter().any(|value| !value.valid())
                    || kinetics
                        .iter()
                        .map(|v| v.position)
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != kinetics.len()
                {
                    Err("Invalid kinetic visual snapshot".into())
                } else {
                    session.kinetics = kinetics;
                    Ok(())
                }
            }
            ServerControl::CropProgress(crops) => {
                validate_crop_progress(crops).map(|crops| session.crop_progress = crops)
            }
            ServerControl::WateringReserve { units, capacity } => {
                if capacity == 0 || units > capacity {
                    Err("Invalid watering can reserve".into())
                } else {
                    session.watering_reserve = (units, capacity);
                    Ok(())
                }
            }
            ServerControl::Container(container) => {
                if container.as_ref().is_some_and(|snapshot| !snapshot.valid()) {
                    Err("Invalid world inventory snapshot".into())
                } else {
                    session.container = container.map(|snapshot| *snapshot);
                    Ok(())
                }
            }
            ServerControl::TreeUpdates(patch) => trees::apply(
                patch,
                &mut session,
                world.journal.bypass_change_detection(),
                world.chunks.bypass_change_detection(),
            )
            .map(|applied| {
                if let Some(visuals) = applied {
                    world.journal.set_changed();
                    if visuals {
                        world.chunks.set_changed();
                    }
                }
            }),
            ServerControl::Trees(snapshot) => trees::legacy(
                snapshot,
                &mut session,
                world.journal.bypass_change_detection(),
                world.chunks.bypass_change_detection(),
            )
            .map(|applied| {
                if let Some(visuals) = applied {
                    world.journal.set_changed();
                    if visuals {
                        world.chunks.set_changed();
                    }
                }
            }),
            ServerControl::Trains(snapshot) => {
                if session.player.is_none() || !snapshot.valid() {
                    Err("Invalid train snapshot".into())
                } else if snapshot.revision >= session.trains.revision {
                    let collision_result = if let Some(views) = &mut world.train_collision_views {
                        let result = views
                            .bypass_change_detection()
                            .set_confirmed_views_with_removals(
                                &snapshot.vehicles,
                                &snapshot.destroyed,
                                &world.journal,
                            );
                        if matches!(result, Ok(true)) {
                            views.set_changed();
                        }
                        result
                    } else {
                        Ok(false)
                    };
                    collision_result.map(|_| {
                        session.train_seat = snapshot
                            .seats
                            .iter()
                            .find(|seat| Some(seat.player) == session.uuid)
                            .copied();
                        // Public vehicle revisions also cover driving and other containers.
                        // Retain private contents/token until a private update or explicit refresh;
                        // the authority still rejects gestures using a stale token.
                        clear_invalid_train_cargo(&mut session, &snapshot.vehicles);
                        session.train_motion_tracks.retain(|id, _| {
                            snapshot.vehicles.iter().any(|vehicle| vehicle.id == *id)
                        });
                        for vehicle in &snapshot.vehicles {
                            let now = Instant::now();
                            session
                                .train_motion_tracks
                                .entry(vehicle.id)
                                .or_insert_with(|| {
                                    train_motion_display::Track::new(vehicle.motion, now)
                                })
                                .push(vehicle.motion, now);
                        }
                        session.trains = *snapshot;
                    })
                } else {
                    Ok(())
                }
            }
            ServerControl::TrainMotions(frames) => {
                if session.player.is_none()
                    || frames.len() > terra_core::trains::MAX_VEHICLES
                    || frames
                        .iter()
                        .any(|frame| frame.id == 0 || !frame.motion.valid() || !frame.steam.valid())
                {
                    Err("Invalid train motion snapshot".into())
                } else {
                    for frame in &frames {
                        if let Some(vehicle) = session
                            .trains
                            .vehicles
                            .iter_mut()
                            .find(|vehicle| vehicle.id == frame.id)
                            && frame.revision >= vehicle.revision
                        {
                            vehicle.motion = frame.motion;
                            vehicle.steam = frame.steam;
                            vehicle.revision = frame.revision;
                            let now = Instant::now();
                            session
                                .train_motion_tracks
                                .entry(frame.id)
                                .or_insert_with(|| {
                                    train_motion_display::Track::new(frame.motion, now)
                                })
                                .push(frame.motion, now);
                        }
                    }
                    if let Some(views) = &mut world.train_collision_views {
                        views
                            .bypass_change_detection()
                            .set_motions(&frames)
                            .map(|changed| {
                                if changed {
                                    views.set_changed();
                                }
                            })
                    } else {
                        Ok(())
                    }
                }
            }
            ServerControl::TrainEngineFuel(cargo) => {
                if !cargo.valid()
                    || session
                        .trains
                        .vehicles
                        .iter()
                        .find(|car| car.id == cargo.vehicle)
                        .is_none_or(|car| {
                            Some(car.owner) != session.uuid
                                || car
                                    .parts
                                    .get(usize::from(cargo.part))
                                    .is_none_or(|part| part.material != 218)
                        })
                {
                    Err("Invalid private engine state".into())
                } else {
                    if session.train_cargo.as_ref().is_some_and(|open| {
                        open.vehicle == cargo.vehicle && open.part == cargo.part
                    }) {
                        session.train_cargo = Some(cargo);
                    }
                    Ok(())
                }
            }
            ServerControl::TrainResult(result) => {
                use crate::protocol::trains::TrainOutcome;
                if result.request == 0
                    || result.request > session.train_request
                    || matches!(&result.outcome,TrainOutcome::Cargo(cargo) if !cargo.valid())
                {
                    Err("Invalid train result".into())
                } else if result.request > session.train_response {
                    if let TrainOutcome::Cargo(cargo) = &result.outcome {
                        session.train_cargo = Some(cargo.clone());
                    }
                    if let TrainOutcome::Seated(seat) = &result.outcome {
                        session.train_seat = *seat;
                    }
                    session.train_response = result.request;
                    session.last_train_result = Some(result);
                    Ok(())
                } else {
                    Ok(())
                }
            }
            ServerControl::Kingdoms(kingdoms) => {
                if session.player.is_none() || !kingdoms.valid() {
                    Err("Invalid kingdom snapshot".into())
                } else if kingdoms.revision > session.kingdoms.revision {
                    session.kingdoms = kingdoms;
                    Ok(())
                } else if kingdoms.revision == session.kingdoms.revision
                    && kingdoms != session.kingdoms
                {
                    Err("Conflicting kingdom snapshot revision".into())
                } else {
                    Ok(())
                }
            }
            ServerControl::KingdomResult(result) => {
                if session.player.is_none()
                    || !result.valid()
                    || result.request > session.kingdom_request
                {
                    Err("Invalid kingdom response".into())
                } else {
                    if session
                        .last_kingdom_result
                        .as_ref()
                        .is_none_or(|last| result.request > last.request)
                    {
                        session.last_kingdom_result = Some(result);
                        session.kingdom_response = session.kingdom_response.saturating_add(1);
                    }
                    Ok(())
                }
            }
        };
        if let Err(error) = result {
            session.fail(error);
            break;
        }
    }
    session.train_display_time = Instant::now();
}

fn validate_crop_progress(
    crops: Vec<CropProgress>,
) -> Result<std::collections::BTreeMap<[i64; 3], CropProgress>, String> {
    let mut ids = std::collections::HashSet::new();
    let mut samples = std::collections::BTreeMap::new();
    if crops.len() > 256 {
        return Err("Crop progress snapshot exceeded limit".into());
    }
    for crop in crops {
        if !crop.valid()
            || !ids.insert(crop.prop_id)
            || samples.insert(crop.support, crop).is_some()
        {
            return Err("Invalid crop progress snapshot".into());
        }
    }
    Ok(samples)
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

fn send_kingdoms(
    mut requests: MessageReader<KingdomRequest>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    for request in requests.read().take(8) {
        if session.ready
            && !session.teleporting
            && let Some(mut sender) = senders.iter_mut().next()
        {
            sender.send::<ControlChannel>(ClientControl::Kingdom(request.0.clone()));
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
                !valid_equipment_vitals(p.survival, p.equipment)
                    || p.consumption.is_some_and(|progress| !progress.valid())
                    || p.mounted.is_some_and(|mount| !mount.valid())
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
                    session.observe_acknowledgment(state.ack);
                    session.latest = Some((packet.tick, state));
                    continue;
                }
                let mut previous = players
                    .players
                    .get(&state.player)
                    .map_or(state, |track| track.current);
                if let Some(track) = players.players.get(&state.player) {
                    previous.position = track.position().to_array();
                    if let Some(mount) = previous.mounted.as_mut()
                        && state.mounted.is_some_and(|next| next.id == mount.id)
                        && let Some(yaw) = track.mounted_yaw()
                    {
                        mount.yaw = yaw;
                    }
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
    let mut state = CharacterState {
        position: Vec3::from_array(confirmed.position),
        velocity: Vec3::from_array(confirmed.velocity),
        grounded: confirmed.grounded,
    };
    let mut vitals = confirmed.survival;
    let mut mounted = confirmed.mounted;
    let seated = session.seated_state();
    if let Some(seated) = seated {
        state = seated;
    }
    for input in session
        .pending
        .iter()
        .filter(|input| input.sequence > confirmed.ack)
    {
        if seated.is_some() {
            continue;
        }
        if let Some(mount) = mounted.as_mut() {
            predict_riding(mount, &mut state, input.character(), &simulation);
            vitals.effects.advance(simulation.delta_seconds());
            continue;
        }
        let replay_input = session.active_stalker_grip().map_or_else(
            || input.character(),
            |grip| {
                grip.movement(
                    state.position,
                    simulation.delta_seconds(),
                    vitals
                        .effects
                        .movement_multiplier(simulation.swimming_at(state.position)),
                )
            },
        );
        if !simulation.survival_step(
            entity,
            collider,
            &mut state,
            replay_input,
            session.game_mode,
            &mut vitals,
        ) {
            // Keep the current prediction until all unacknowledged input can
            // be replayed against fresh colliders, not a partial older pose.
            return;
        }
        vitals.effects.advance(simulation.delta_seconds());
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
    session.predicted_vitals = vitals;
    session.mounted = mounted;
    if let Some(mut pose) = pose {
        pose.reconcile(transform.translation, state.position);
    }
    transform.translation = state.position;
    velocity.0 = state.velocity;
    grounded.0 = state.grounded;
}

fn predict_riding(
    mount: &mut MountedState,
    state: &mut CharacterState,
    input: CharacterInput,
    simulation: &CharacterSimulation,
) {
    let mut horse = mount.horse();
    let grounded = simulation.ride_step(&mut horse, &mut mount.stamina, input);
    state.position = simulation.riding_position(&horse);
    state.velocity = Vec3::from_array(horse.velocity);
    state.grounded = grounded;
    *mount = MountedState::new(&horse, mount.stamina, grounded);
}

pub(crate) fn valid_equipment_vitals(
    vitals: terra_core::SurvivalVitals,
    equipment: terra_core::EquipmentSlots,
) -> bool {
    if !vitals.valid() || !equipment.valid() {
        return false;
    }
    let mut derived = vitals;
    derived.apply_equipment(equipment);
    derived == vitals
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
    session.check_acknowledgment_liveness();
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
    rails: MessageReader<'w, 's, RailConnectRequest>,
    rail_builds: MessageReader<'w, 's, RailBuildRequest>,
    rail_placements: MessageReader<'w, 's, RailPlacementRequest>,
    belt_interactions: MessageReader<'w, 's, ConveyorInteractionRequest>,
    conveyors: MessageReader<'w, 's, ConveyorConnectRequest>,
    objects: MessageReader<'w, 's, ObjectInteractionRequest>,
    rotations: MessageReader<'w, 's, ObjectRotateRequest>,
    blocks: MessageReader<'w, 's, BlockEditRequest>,
    props: MessageReader<'w, 's, PropRemoveRequest>,
    planted: MessageReader<'w, 's, PropPlaceRequest>,
    fruit: MessageReader<'w, 's, TreeHarvestRequest>,
    farming: MessageReader<'w, 's, FarmToolRequest>,
    surfaces: MessageReader<'w, 's, terra_world::environment::SurfaceConditionRequest>,
}

pub(crate) fn farming_revision(
    journal: &EditJournal,
    position: [i64; 3],
    action: terra_core::farming::FarmingAction,
) -> u64 {
    use terra_core::farming::FarmingAction;
    let section = terra_voxel::global_to_section(position).0;
    match action {
        // Mesh epochs are local render invalidation counters, not replicated
        // transaction versions. Source/quantity and visibility are rechecked on
        // the authority after the stable block revision is validated.
        FarmingAction::FillBucket
        | FarmingAction::EmptyBucket
        | FarmingAction::Till
        | FarmingAction::Refill => journal.revision(section),
        _ => journal.prop_epoch(section),
    }
}

fn send_edits(
    mut requests: EditRequests,
    mut session: ResMut<ClientSession>,
    journal: Res<EditJournal>,
    selected: Res<SelectedInventorySlot>,
    mut rail_sent: MessageWriter<RailPlacementSent>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    let mut actions = Vec::new();
    for request in requests.fruit.read() {
        actions.push(EditAction::HarvestTree {
            id: request.id,
            fruit_revision: request.fruit_revision,
            slot: request.slot,
        });
    }
    for request in requests.belt_interactions.read() {
        actions.push(match *request {
            ConveyorInteractionRequest::Collect { id } => EditAction::CollectConveyor { id },
            ConveyorInteractionRequest::Dye { position, color } => {
                EditAction::DyeConveyor { position, color }
            }
            ConveyorInteractionRequest::DyeFabric { position, color } => {
                EditAction::DyeFabric { position, color }
            }
        });
    }
    for request in requests.conveyors.read() {
        actions.push(EditAction::ConnectConveyor {
            start: request.start,
            end: request.end,
            start_revision: request.start_revision,
            item: request.item,
        });
    }
    for request in requests.rail_builds.read() {
        actions.push(EditAction::BuildRail {
            position: request.position,
            expected: request.expected,
            choice: request.choice,
            signature: request.signature,
        });
    }
    for request in requests.rails.read() {
        actions.push(EditAction::ConnectRail {
            start: request.start,
            position: request.position,
            expected: request.expected,
        });
    }
    for request in requests.rotations.read() {
        actions.push(EditAction::RotateObject {
            position: request.position,
            expected: request.expected,
            facing: request.facing,
            reverse: request.reverse,
        });
    }
    for request in requests.objects.read() {
        actions.push(EditAction::ToggleObject {
            position: request.position,
            expected: request.expected,
            facing: request.facing,
            closing: request.closing,
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
    for edit in requests.farming.read() {
        actions.push(EditAction::Farm {
            position: edit.position,
            action: edit.action,
        });
    }
    let rail_actions: Vec<_> = requests
        .rail_placements
        .read()
        .map(|request| (request.action.edit_action(), Some(request.token)))
        .collect();
    let actions = actions
        .into_iter()
        .map(|action| (action, None))
        .chain(rail_actions);
    if !session.ready {
        return;
    }
    let Some(mut sender) = senders.iter_mut().next() else {
        return;
    };
    for (action, token) in actions.take(20) {
        let revision = match action {
            EditAction::HarvestTree { fruit_revision, .. } => fruit_revision,
            EditAction::CollectConveyor { .. } => 0,
            EditAction::DyeConveyor { position, .. } | EditAction::DyeFabric { position, .. } => {
                journal.revision(terra_voxel::global_to_section(position).0)
            }
            EditAction::ConnectConveyor { end, .. } => {
                journal.revision(terra_voxel::global_to_section(end).0)
            }
            EditAction::Block { position, .. }
            | EditAction::ConnectRail { position, .. }
            | EditAction::StretchRail { position, .. }
            | EditAction::BuildRail { position, .. }
            | EditAction::SeedRail { position, .. }
            | EditAction::ToggleObject { position, .. }
            | EditAction::RotateObject { position, .. } => {
                journal.revision(terra_voxel::global_to_section(position).0)
            }
            EditAction::Prop { section, .. } => journal.prop_epoch(section),
            EditAction::PlaceProp { support, .. } => {
                journal.prop_epoch(terra_voxel::global_to_section(support).0)
            }
            EditAction::Surface { position, .. } => {
                journal.appearance_epoch(terra_voxel::global_to_section(position).0)
            }
            EditAction::Farm { position, action } => farming_revision(&journal, position, action),
        };
        let Some(mut command) = session.edit_command(revision, action) else {
            session.fail("Edit request sequence exhausted");
            break;
        };
        if matches!(
            action,
            EditAction::Block { material: 1.., .. }
                | EditAction::PlaceProp { .. }
                | EditAction::Farm { .. }
                | EditAction::RotateObject { .. }
                | EditAction::ConnectConveyor { .. }
                | EditAction::ConnectRail { .. }
                | EditAction::StretchRail { .. }
                | EditAction::BuildRail { .. }
                | EditAction::SeedRail { .. }
                | EditAction::DyeConveyor { .. }
                | EditAction::DyeFabric { .. }
                | EditAction::ToggleObject {
                    expected: 71..=73 | 131,
                    ..
                }
        ) {
            command.source = selected.0.map(|slot| InventorySource {
                slot,
                revision: session.inventory.revision,
            });
        }
        sender.send::<ControlChannel>(ClientControl::Edit(command));
        if let Some(token) = token {
            rail_sent.write(RailPlacementSent {
                token,
                request: command.request,
                action,
            });
        }
    }
}

#[cfg(test)]
#[path = "client_section_tests.rs"]
mod section_tests;

#[cfg(test)]
#[path = "client_reconciliation_tests.rs"]
mod reconciliation_tests;

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

#[cfg(test)]
mod crop_progress_tests {
    use super::*;
    fn sample() -> CropProgress {
        CropProgress {
            support: [-17, 15, -1],
            kind: terra_core::CropKind::Wheat,
            prop_id: 91,
            progress: 0.574,
            moisture: 0.8,
        }
    }
    #[test]
    fn progress_snapshots_are_bounded_unique_and_preserve_actual_growth() {
        let crop = sample();
        let samples = validate_crop_progress(vec![crop]).unwrap();
        assert_eq!(samples[&crop.support], crop);
        assert!(validate_crop_progress(Vec::new()).unwrap().is_empty());
        assert!(validate_crop_progress(vec![crop; 257]).is_err());
        assert!(validate_crop_progress(vec![crop, crop]).is_err());
        assert!(
            validate_crop_progress(vec![
                crop,
                CropProgress {
                    support: [1, 15, 2],
                    ..crop
                }
            ])
            .is_err()
        );
        assert!(
            validate_crop_progress(vec![
                crop,
                CropProgress {
                    prop_id: 92,
                    ..crop
                }
            ])
            .is_err()
        );
        for invalid in [
            CropProgress {
                progress: f32::NAN,
                ..crop
            },
            CropProgress {
                progress: 1.01,
                ..crop
            },
            CropProgress {
                moisture: -0.1,
                ..crop
            },
            CropProgress { prop_id: 0, ..crop },
            CropProgress {
                support: [i64::MIN, 15, 0],
                ..crop
            },
        ] {
            assert!(validate_crop_progress(vec![invalid]).is_err());
        }
    }
}

fn send_trains(
    mut requests: MessageReader<TrainRequest>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    for request in requests.read().take(16) {
        if session.ready
            && session.error.is_none()
            && let Some(mut sender) = senders.iter_mut().next()
        {
            sender.send::<ControlChannel>(ClientControl::Train(request.0.clone()));
        }
    }
}

#[cfg(test)]
#[path = "client_train_cargo_tests.rs"]
mod train_cargo_tests;
