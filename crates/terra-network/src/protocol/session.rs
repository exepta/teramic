use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use terra_core::{
    GameMode, Inventory, PlayerId, PlayerProfile, PlayerUuid, WorldConfig, WorldDrop,
};
use terra_voxel::SectionPos;
use terra_world::{edits::SectionDelta, props::PropId};

// Version 56 adds bounded status effects to authoritative player vitals and snapshots.
pub const PROTOCOL_VERSION: u32 = 56;
pub const PROTOCOL_ID: u64 = 0x5445525241000001;
pub const MAX_INPUT_BACKLOG: usize = 128;

pub struct ControlChannel;
pub struct InputChannel;
pub struct SnapshotChannel;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientControl {
    Creature {
        id: u64,
        feed: bool,
        look: [f32; 2],
    },
    HeldSlot(Option<u8>),
    Hello {
        version: u32,
        generator: u32,
        effect_catalog: u64,
        view_distance: u8,
        profile: PlayerProfile,
    },
    Edit(EditCommand),
    Mining(MiningIntent),
    ViewDistance(u8),
    VegetationDensity(f32),
    Chat(String),
    Teleport(u64),
    Inventory(super::inventory::InventoryCommand),
    Respawn {
        generation: u32,
    },
    Container(super::inventory::ContainerCommand),
    ChestCursor(Option<super::social::ChestCursorInput>),
    Consumption {
        slot: Option<u8>,
        revision: u64,
    },
    LeaveBed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MiningTarget {
    Block { position: [i64; 3], expected: u16 },
    Prop { section: SectionPos, id: PropId },
}
impl MiningTarget {
    pub fn action(self) -> EditAction {
        match self {
            Self::Block { position, expected } => EditAction::Block {
                position,
                expected,
                material: 0,
            },
            Self::Prop { section, id } => EditAction::Prop { section, id },
        }
    }
    pub fn duration(self) -> f32 {
        match self {
            Self::Block {
                expected: 3 | 10..=13,
                ..
            } => 1.25,
            Self::Block { expected: 5, .. } => 0.9,
            Self::Block {
                expected: 14..=15, ..
            } => 0.4,
            Self::Block {
                expected: 16..=18, ..
            } => 0.8,
            Self::Block { expected: 19, .. } => 0.7,
            Self::Block {
                expected: 20..=26, ..
            } => 0.45,
            Self::Block {
                expected: 7..=9, ..
            } => 0.3,
            Self::Block { .. } => 0.55,
            Self::Prop { .. } => 0.25,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MiningIntent {
    pub target: Option<MiningTarget>,
    pub look: [f32; 2],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MiningProgress {
    pub target: MiningTarget,
    pub progress: f32,
    pub duration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConsumptionProgress {
    pub slot: u8,
    pub item: terra_core::ItemKind,
    pub progress: f32,
    pub duration: f32,
}
impl ConsumptionProgress {
    pub fn valid(self) -> bool {
        self.slot < 9
            && terra_core::food(self.item).is_some()
            && self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && self.duration.is_finite()
            && (0.1..=30.0).contains(&self.duration)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MovementInput {
    pub generation: u32,
    pub sequence: u64,
    pub direction: [f32; 3],
    pub look: [f32; 2],
    pub jump: bool,
    pub sprint: bool,
    pub auto_climb: bool,
    pub flight_speed: Option<f32>,
}
impl MovementInput {
    pub fn valid(self) -> bool {
        self.direction
            .iter()
            .chain(&self.look)
            .all(|v| v.is_finite())
            && self.direction.iter().all(|v| v.abs() <= 1.001)
            && self
                .flight_speed
                .is_none_or(|s| s.is_finite() && (0.0..=60.0).contains(&s))
            && self.look[0].abs() <= std::f32::consts::TAU + 0.001
            && self.look[1].abs() <= 1.501
    }
    pub fn character(self) -> terra_physics::character::CharacterInput {
        terra_physics::character::CharacterInput {
            direction: Vec3::from_array(self.direction).clamp_length_max(1.0),
            jump: self.jump,
            sprint: self.sprint,
            auto_climb: self.auto_climb,
            flight_speed: self.flight_speed,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EditAction {
    Block {
        position: [i64; 3],
        expected: u16,
        material: u16,
    },
    Prop {
        section: SectionPos,
        id: PropId,
    },
    PlaceProp {
        support: [i64; 3],
        kind: terra_core::ResourcePropKind,
    },
    Surface {
        position: [i64; 3],
        condition: terra_world::environment::SurfaceCondition,
    },
    ToggleObject {
        position: [i64; 3],
        expected: u16,
        facing: u8,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct InventorySource {
    pub slot: u8,
    pub revision: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EditCommand {
    pub request: u64,
    pub revision: u64,
    pub look: [f32; 2],
    pub action: EditAction,
    pub source: Option<InventorySource>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditOutcome {
    Applied,
    Conflict,
    OutOfReach,
    Occluded,
    Invalid,
    RateLimited,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerControl {
    SleepingPlayers(Vec<SleepingPlayer>),
    Recipes {
        catalog: String,
        error: Option<String>,
    },
    GameMode {
        mode: GameMode,
        allow_flight: bool,
        tick: u64,
        state: PlayerState,
    },
    Welcome {
        player: PlayerId,
        uuid: PlayerUuid,
        world: Box<WorldConfig>,
        tick_rate: u16,
        spawn: [f32; 3],
        allow_flight: bool,
        allow_build: bool,
        allow_environment: bool,
        game_mode: GameMode,
        clock: terra_core::WorldClock,
        weather: terra_core::WorldWeather,
    },
    WorldTime(terra_core::WorldClock),
    Weather(terra_core::WorldWeather),
    Section(SectionDelta),
    Ready,
    EditResult {
        request: u64,
        action: EditAction,
        outcome: EditOutcome,
    },
    Mining(Option<MiningProgress>),
    Gameplay {
        inventory: Inventory,
        drops: Vec<WorldDrop>,
    },
    Stats {
        generation: u32,
        stats: terra_core::PlayerStats,
    },
    Chat(super::chat::ChatEntry),
    ChatPlayers(Vec<String>),
    InventoryResult {
        outcome: super::inventory::InventoryOutcome,
        inventory: Inventory,
    },
    TeleportPreparing,
    TeleportCancelled,
    Teleport {
        tick: u64,
        state: PlayerState,
    },
    Reject(String),
    Container(Option<super::inventory::ContainerSnapshot>),
    Social {
        names: Option<Vec<(PlayerId, String)>>,
        cursors: Vec<super::social::SharedCursor>,
    },
    Consumption(Option<ConsumptionProgress>),
    ConsumptionFinished {
        inventory: Inventory,
    },
    Furnaces(Vec<FurnaceVisual>),
    GameRules(Box<terra_core::WorldRuleSettings>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SleepingPlayer {
    pub player: PlayerId,
    pub anchor: [i64; 3],
    pub facing: u8,
}
impl SleepingPlayer {
    pub fn valid(self) -> bool {
        self.facing < 4 && self.anchor.iter().all(|v| v.unsigned_abs() <= 990_000)
    }
    pub fn center(self) -> Vec3 {
        let rotation = Quat::from_rotation_y(f32::from(self.facing) * std::f32::consts::FRAC_PI_2);
        Vec3::from_array(self.anchor.map(|v| v as f32))
            + Vec3::new(0.5, 1.02, 0.5)
            + rotation * Vec3::new(0.0, 0.0, 0.5)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FurnaceVisual {
    pub position: [i64; 3],
    pub lit: bool,
    pub outlet: [i64; 3],
    pub smoke_cells: Vec<[i64; 3]>,
}
impl FurnaceVisual {
    pub fn valid(&self) -> bool {
        self.smoke_cells.len() <= terra_core::furnace::MAX_SMOKE_CELLS
            && self
                .smoke_cells
                .iter()
                .chain([&self.position, &self.outlet])
                .all(|p| p.iter().all(|v| v.unsigned_abs() <= 1_000_000))
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct PlayerState {
    pub gender: terra_core::PlayerGender,
    pub generation: u32,
    pub player: PlayerId,
    pub ack: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub grounded: bool,
    pub look: [f32; 2],
    pub survival: terra_core::SurvivalVitals,
    pub held_torch: bool,
    pub held_item: Option<terra_core::ItemKind>,
    pub mining: bool,
    pub consumption: Option<ConsumptionProgress>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerStates {
    pub tick: u64,
    pub players: Vec<PlayerState>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatureStates {
    pub tick: u64,
    pub creatures: Vec<terra_core::creatures::Creature>,
    pub ecology: terra_core::creatures::ecology::Ecology,
}

pub(crate) fn register(app: &mut App) {
    app.add_channel::<ControlChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(default()),
        ..default()
    })
    .add_direction(NetworkDirection::Bidirectional);
    app.add_channel::<InputChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(default()),
        ..default()
    })
    .add_direction(NetworkDirection::ClientToServer);
    app.add_channel::<SnapshotChannel>(ChannelSettings {
        mode: ChannelMode::SequencedUnreliable,
        ..default()
    })
    .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<ClientControl>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<MovementInput>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<ServerControl>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<PlayerStates>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<CreatureStates>()
        .add_direction(NetworkDirection::ServerToClient);
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn aquatic_species_and_defensive_ink_are_valid_snapshot_states() {
        use terra_core::creatures::*;
        let creatures: Vec<_> = CreatureKind::ALL
            .into_iter()
            .enumerate()
            .filter(|(_, kind)| kind.is_aquatic())
            .map(|(i, kind)| {
                let home = [i as i64, 0];
                Creature {
                    id: creature_id(home, 0).unwrap(),
                    kind,
                    home,
                    position: [0.0, 4.0, 0.0],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: kind.health(),
                    behavior: if kind == CreatureKind::Squid {
                        CreatureBehavior::Ink
                    } else {
                        CreatureBehavior::Swim
                    },
                    timer: 0.7,
                    age: 0.0,
                    variant: 0,
                    needs: CreatureNeeds::full(kind),
                }
            })
            .collect();
        assert!(creatures.iter().all(Creature::valid));
        let snapshot = CreatureStates {
            tick: 1,
            creatures,
            ecology: Default::default(),
        };
        let source = serde_json::to_vec(&snapshot).unwrap();
        let received: CreatureStates = serde_json::from_slice(&source).unwrap();
        assert_eq!(received.creatures, snapshot.creatures);
        assert_eq!(received.creatures.len(), 10);
        assert_eq!(PROTOCOL_VERSION, 56);
    }
    #[test]
    fn join_only_world_allocation_keeps_control_packets_compact_and_serde_transparent() {
        assert!(std::mem::size_of::<ServerControl>() <= 256);
        let world = WorldConfig::default();
        assert_eq!(
            serde_json::to_vec(&world).unwrap(),
            serde_json::to_vec(&Box::new(world)).unwrap()
        );
    }
}
