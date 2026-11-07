use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use terra_core::{
    GameMode, Inventory, PlayerId, PlayerProfile, PlayerUuid, WorldConfig, WorldDrop,
};
use terra_voxel::SectionPos;
use terra_world::{edits::SectionDelta, props::PropId};

// Version 111 adds authoritative modular railway placement, switching and ownership.
pub const PROTOCOL_VERSION: u32 = 111;
pub const PROTOCOL_ID: u64 = 0x5445525241000001;
pub const MAX_INPUT_BACKLOG: usize = 128;
pub const MAX_TREE_INTEREST: usize = 256;
pub const MAX_TREE_PATCH_CELLS: usize = 32_768;
pub const MAX_TREE_PATCH_BYTES: usize = 128 * 1024;

pub struct ControlChannel;
pub struct InputChannel;
pub struct SnapshotChannel;
pub struct CreatureSnapshotChannel;

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
        equipment_catalog: u64,
        crushing_catalog: u64,
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
    Crank {
        position: Option<[i64; 3]>,
        look: [f32; 2],
    },
    InspectCreature(Option<u64>),
    Dismount,
    PetCommand {
        id: u64,
        mode: terra_core::creatures::taming::CompanionMode,
    },
    HarvestHoney {
        section: SectionPos,
        id: PropId,
        look: [f32; 2],
    },
    AttackPlayer {
        player: PlayerId,
        look: [f32; 2],
    },
    Kingdom(super::kingdom::KingdomCommand),
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
    pub fell_tree: bool,
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
            && (terra_core::food(self.item).is_some()
                || terra_core::brewing::drink_seconds(self.item).is_some())
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
        closing: bool,
    },
    Farm {
        position: [i64; 3],
        action: terra_core::farming::FarmingAction,
    },
    RotateObject {
        position: [i64; 3],
        expected: u16,
        facing: u8,
        reverse: bool,
    },
    ConnectConveyor {
        start: [i64; 3],
        end: [i64; 3],
        start_revision: u64,
        item: terra_core::ItemKind,
    },
    CollectConveyor {
        id: u64,
    },
    DyeConveyor {
        position: [i64; 3],
        color: terra_core::beds::DyeColor,
    },
    DyeFabric {
        position: [i64; 3],
        color: terra_core::beds::DyeColor,
    },
    HarvestTree {
        id: u64,
        fruit_revision: u64,
        slot: u8,
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
    Planting(terra_core::trees::SaplingPlacementError),
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
        state: Box<PlayerState>,
    },
    Welcome {
        player: PlayerId,
        uuid: PlayerUuid,
        world: Box<WorldConfig>,
        tick_rate: u16,
        spawn: [f32; 3],
        location: Option<terra_core::PlayerLocation>,
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
        stats: Box<terra_core::PlayerStats>,
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
        state: Box<PlayerState>,
    },
    Reject(String),
    // Filters can hold full item metadata, including persistent cape heraldry.
    // Allocate the infrequent open-container snapshot without enlarging every
    // control message. Box remains transparent to both network serializers.
    Container(Option<Box<super::inventory::ContainerSnapshot>>),
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
    Crushers(Vec<CrusherVisual>),
    CropProgress(Vec<CropProgress>),
    WateringReserve {
        units: u8,
        capacity: u8,
    },
    ChestViewers(Vec<[i64; 3]>),
    TreeCollapse(terra_world::tree_felling::Collapse),
    Kinetics(Vec<KineticVisual>),
    Cauldrons(Vec<CauldronVisual>),
    Campfires(Vec<CampfireVisual>),
    Plumbing(Vec<PlumbingVisual>),
    Infusion(Vec<terra_core::infusion::HoldVisual>),
    Kingdoms(terra_core::kingdom::KingdomRegistry),
    KingdomResult(super::kingdom::KingdomResult),
    Trees(terra_core::TreeRegistry),
    TreeUpdates(TreePatch),
}

/// Ordered interest updates: reset begins a new neighbourhood, subsequent
/// patches retain records outside the current bounded packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreePatch {
    pub revision: u64,
    pub reset: bool,
    pub removals: Vec<u64>,
    pub records: Vec<TreeUpdate>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeUpdate {
    /// Metadata is always current. Offset vectors are supplied only by Full;
    /// other modes must reconstruct retained geometry before validation.
    pub tree: terra_core::TreeRecord,
    pub geometry: TreeGeometryUpdate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TreeGeometryUpdate {
    Full,
    Unchanged,
    Removed {
        wood: Vec<[i8; 3]>,
        leaves: Vec<[i8; 3]>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlumbingVisual {
    pub position: [i64; 3],
    pub amount: u16,
    pub capacity: u16,
    pub open: bool,
    pub flowing: bool,
    pub supplied: bool,
    pub pump_rpm: f32,
    pub basin: Option<terra_core::plumbing::BasinShape>,
    pub intake: Option<terra_core::plumbing::IntakeActivity>,
}
impl PlumbingVisual {
    pub fn valid(&self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.amount <= self.capacity
            && self.pump_rpm.is_finite()
            && self.pump_rpm.abs() <= f32::from(terra_core::kinetic::MAX_RPM)
            && (!self.supplied || self.open && self.amount > 0)
            && self
                .intake
                .is_none_or(|activity| activity.valid_for(self.position))
            && self.basin.map_or(self.capacity == 8, |shape| {
                shape.valid() && shape.contains(self.position) && self.capacity == shape.capacity()
            })
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct KineticVisual {
    pub position: [i64; 3],
    pub rpm: f32,
    pub overloaded: bool,
    pub stress: f32,
    pub capacity: f32,
    pub available_capacity: f32,
}
impl KineticVisual {
    pub fn valid(self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.rpm.is_finite()
            && self.rpm.abs() <= f32::from(terra_core::kinetic::MAX_RPM)
            && (!self.overloaded || self.rpm == 0.0)
            && [self.stress, self.capacity, self.available_capacity]
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0e9).contains(value))
            && self.available_capacity <= self.capacity
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CropProgress {
    pub support: [i64; 3],
    pub kind: terra_core::farming::CropKind,
    pub prop_id: u64,
    pub progress: f32,
    pub moisture: f32,
}
impl CropProgress {
    pub fn valid(self) -> bool {
        self.prop_id != 0
            && self
                .support
                .iter()
                .all(|value| value.unsigned_abs() <= 1_000_000)
            && self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && self.moisture.is_finite()
            && (0.0..=1.0).contains(&self.moisture)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrusherVisual {
    pub position: [i64; 3],
    pub working: bool,
    pub progress: f32,
    pub manual_active: bool,
}
impl CrusherVisual {
    pub fn valid(&self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.progress.is_finite()
            && (0.0..=1.0).contains(&self.progress)
            && (!self.working || self.manual_active)
    }
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CauldronVisual {
    pub position: [i64; 3],
    pub water: u8,
    pub brewing: bool,
    pub heated: bool,
    pub recipe: Option<u16>,
    pub spoon: Option<terra_core::ToolKind>,
}
impl CauldronVisual {
    pub fn valid(&self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
            && self.water <= terra_core::brewing::MAX_WATER
            && self
                .recipe
                .is_none_or(|id| terra_core::brewing::recipe(id).is_some())
            && self.spoon.is_none_or(|tool| {
                terra_core::brewing::spoon_speed(terra_core::ItemKind::Tool(tool)).is_some()
            })
            && (!self.brewing
                || self.heated && self.water > 0 && self.recipe.is_some() && self.spoon.is_some())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampfireVisual {
    pub position: [i64; 3],
    pub lit: bool,
}
impl CampfireVisual {
    pub fn valid(&self) -> bool {
        self.position.iter().all(|v| v.unsigned_abs() <= 1_000_000)
    }
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
    #[serde(default)]
    pub kingdom_heraldry: Option<terra_core::kingdom::KingdomHeraldry>,
    pub gender: terra_core::PlayerGender,
    pub generation: u32,
    pub player: PlayerId,
    pub ack: u64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub grounded: bool,
    pub look: [f32; 2],
    pub survival: terra_core::SurvivalVitals,
    pub equipment: terra_core::EquipmentSlots,
    pub held_torch: bool,
    pub held_item: Option<terra_core::ItemKind>,
    pub mining: bool,
    pub consumption: Option<ConsumptionProgress>,
    pub mounted: Option<MountedState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MountedState {
    pub id: u64,
    pub home: [i64; 2],
    pub position: [f64; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub age: f64,
    pub variant: u8,
    pub health: f32,
    pub stamina: f32,
    pub grounded: bool,
}

impl MountedState {
    pub fn new(horse: &terra_core::creatures::Creature, stamina: f32, grounded: bool) -> Self {
        Self {
            id: horse.id,
            home: horse.home,
            position: horse.position,
            velocity: horse.velocity,
            yaw: horse.yaw,
            age: horse.age,
            variant: horse.variant,
            health: horse.health,
            stamina,
            grounded,
        }
    }
    pub fn horse(self) -> terra_core::creatures::Creature {
        use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
        Creature {
            id: self.id,
            home: self.home,
            kind: CreatureKind::Horse,
            position: self.position,
            velocity: self.velocity,
            yaw: self.yaw,
            age: self.age,
            variant: self.variant,
            health: self.health,
            behavior: if self.grounded {
                CreatureBehavior::Wander
            } else {
                CreatureBehavior::Leap
            },
            timer: 0.0,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        }
    }
    pub fn valid(self) -> bool {
        let horse = self.horse();
        horse.valid()
            && horse.health > 0.0
            && horse.growth() >= 1.0
            && self.stamina.is_finite()
            && horse
                .horse_traits()
                .is_some_and(|traits| (0.0..=f32::from(traits.stamina)).contains(&self.stamina))
    }
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
    #[serde(default)]
    pub intentions: Vec<terra_core::creatures::intentions::CreatureIntent>,
    #[serde(default)]
    pub bonds: Vec<terra_core::creatures::taming::CompanionView>,
    #[serde(default)]
    pub families: Vec<terra_core::creatures::breeding::FamilyView>,
    #[serde(default)]
    pub stalker_grip: Option<StalkerGrip>,
}

/// Only sent to the grabbed player. This grants no client-side movement authority.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StalkerGrip {
    pub creature: u64,
    pub destination: [f64; 3],
    pub remaining_meters: f32,
}

impl StalkerGrip {
    pub fn valid(self) -> bool {
        self.creature != 0
            && self
                .destination
                .into_iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            && self.remaining_meters.is_finite()
            && (0.0..=40.0).contains(&self.remaining_meters)
    }

    pub(crate) fn movement(
        self,
        position: Vec3,
        dt: f32,
        effect_speed: f32,
    ) -> terra_physics::character::CharacterInput {
        if !self.valid()
            || !position.is_finite()
            || !dt.is_finite()
            || !(0.0..=0.1).contains(&dt)
            || dt == 0.0
            || !effect_speed.is_finite()
            || effect_speed <= 0.0
            || self.remaining_meters <= 0.0
        {
            return Default::default();
        }
        let delta = Vec3::from_array(self.destination.map(|v| v as f32)) - position;
        let horizontal = Vec3::new(delta.x, 0.0, delta.z);
        let distance = horizontal.length();
        if distance <= 0.05 {
            return Default::default();
        }
        let walk_speed = terra_physics::character::WALK_SPEED * effect_speed;
        if !walk_speed.is_finite() {
            return Default::default();
        }
        let speed = 2.3_f32
            .min(walk_speed)
            .min(distance.min(self.remaining_meters) / dt);
        terra_physics::character::CharacterInput {
            direction: horizontal / distance * (speed / walk_speed),
            auto_climb: true,
            ..Default::default()
        }
    }
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
    // Sequenced channels retain only their newest message, not one per type.
    // Independent full-state families must not supersede each other.
    app.add_channel::<CreatureSnapshotChannel>(ChannelSettings {
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
    fn intake_activity_roundtrips_only_actual_adjacent_axis_mouths_and_units() {
        let position = [-17, 7, 15];
        let activity = terra_core::plumbing::IntakeActivity {
            source: [-17, 6, 15],
            direction: [0, -1, 0],
            units: 8,
        };
        let visual = PlumbingVisual {
            position,
            amount: 0,
            capacity: 8,
            open: true,
            flowing: true,
            supplied: false,
            pump_rpm: 0.,
            basin: None,
            intake: Some(activity),
        };
        assert!(visual.valid());
        assert_eq!(
            serde_json::from_slice::<PlumbingVisual>(&serde_json::to_vec(&visual).unwrap())
                .unwrap(),
            visual
        );
        for invalid in [
            terra_core::plumbing::IntakeActivity {
                units: 0,
                ..activity
            },
            terra_core::plumbing::IntakeActivity {
                units: 9,
                ..activity
            },
            terra_core::plumbing::IntakeActivity {
                direction: [1, -1, 0],
                ..activity
            },
            terra_core::plumbing::IntakeActivity {
                source: [-17, 5, 15],
                ..activity
            },
        ] {
            assert!(
                !PlumbingVisual {
                    intake: Some(invalid),
                    ..visual
                }
                .valid()
            );
        }
    }

    #[test]
    fn plumbing_snapshots_roundtrip_and_reject_impossible_amounts_and_shapes() {
        let shape = terra_core::plumbing::BasinShape {
            origin: [-17, 19, 15],
            size: [5, 10, 5],
        };
        let visual = PlumbingVisual {
            intake: None,
            position: shape.origin,
            amount: 1500,
            capacity: 2000,
            open: true,
            flowing: true,
            supplied: true,
            pump_rpm: 20.0,
            basin: Some(shape),
        };
        assert!(visual.valid());
        let restored: PlumbingVisual =
            serde_json::from_slice(&serde_json::to_vec(&visual).unwrap()).unwrap();
        assert_eq!(restored, visual);
        for invalid in [
            PlumbingVisual {
                amount: 2001,
                ..visual
            },
            PlumbingVisual {
                capacity: 1999,
                ..visual
            },
            PlumbingVisual {
                pump_rpm: f32::NAN,
                ..visual
            },
            PlumbingVisual {
                pump_rpm: 257.0,
                ..visual
            },
            PlumbingVisual {
                position: [0, 0, 0],
                ..visual
            },
            PlumbingVisual {
                basin: None,
                ..visual
            },
        ] {
            assert!(!invalid.valid());
        }
        assert!(
            PlumbingVisual {
                amount: 8,
                capacity: 8,
                basin: None,
                ..visual
            }
            .valid()
        );
    }
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
            intentions: Vec::new(),
            bonds: Vec::new(),
            families: Vec::new(),
            stalker_grip: None,
        };
        let source = serde_json::to_vec(&snapshot).unwrap();
        let received: CreatureStates = serde_json::from_slice(&source).unwrap();
        assert_eq!(received.creatures, snapshot.creatures);
        assert_eq!(
            received
                .creatures
                .iter()
                .map(|a| a.sex())
                .collect::<Vec<_>>(),
            snapshot
                .creatures
                .iter()
                .map(|a| a.sex())
                .collect::<Vec<_>>()
        );
        assert_eq!(received.creatures.len(), 10);
        assert_eq!(PROTOCOL_VERSION, 111);
    }
    #[test]
    fn kinetic_stress_capacity_roundtrip_and_reject_nonfinite_or_impossible_reserves() {
        let visual = KineticVisual {
            position: [-16, 3, 15],
            rpm: -20.0,
            stress: 22.0,
            capacity: 128.0,
            available_capacity: 106.0,
            overloaded: false,
        };
        assert!(visual.valid());
        let restored: KineticVisual =
            serde_json::from_slice(&serde_json::to_vec(&visual).unwrap()).unwrap();
        assert_eq!(restored, visual);
        for invalid in [
            KineticVisual {
                capacity: f32::NAN,
                ..visual
            },
            KineticVisual {
                stress: -1.0,
                ..visual
            },
            KineticVisual {
                available_capacity: 129.0,
                ..visual
            },
            KineticVisual {
                available_capacity: f32::INFINITY,
                ..visual
            },
        ] {
            assert!(!invalid.valid());
        }
    }

    #[test]
    fn drinking_progress_accepts_catalogued_potions_and_water_but_not_unknown_payloads() {
        let progress = ConsumptionProgress {
            slot: 0,
            item: terra_core::ItemKind::WaterBottle,
            progress: 0.5,
            duration: 1.4,
        };
        for item in [
            terra_core::ItemKind::WaterBottle,
            terra_core::ItemKind::Potion(1),
            terra_core::ItemKind::Potion(6),
        ] {
            assert!(ConsumptionProgress { item, ..progress }.valid());
        }
        for item in [
            terra_core::ItemKind::GlassBottle,
            terra_core::ItemKind::Potion(0),
            terra_core::ItemKind::Potion(u16::MAX),
        ] {
            assert!(!ConsumptionProgress { item, ..progress }.valid());
        }
    }
    #[test]
    fn conveyor_collection_and_dyes_roundtrip_without_reusing_old_action_tags() {
        let actions = [
            EditAction::CollectConveyor {
                id: 18446744073709551610,
            },
            EditAction::DyeConveyor {
                position: [-16, 6, 15],
                color: terra_core::beds::DyeColor::LightBlue,
            },
        ];
        for action in actions {
            let payload = serde_json::to_vec(&action).unwrap();
            let restored: EditAction = serde_json::from_slice(&payload).unwrap();
            assert_eq!(serde_json::to_vec(&restored).unwrap(), payload);
        }
    }

    #[test]
    fn wrench_rotation_preserves_target_revision_and_direction_on_the_wire() {
        let command = EditCommand {
            request: 7,
            revision: 19,
            look: [0.1, -0.3],
            action: EditAction::RotateObject {
                position: [-16, 6, 15],
                expected: 149,
                facing: 4,
                reverse: true,
            },
            source: Some(InventorySource {
                slot: 2,
                revision: 11,
            }),
        };
        let payload = serde_json::to_vec(&command).unwrap();
        let restored: EditCommand = serde_json::from_slice(&payload).unwrap();
        assert!(matches!(
            restored.action,
            EditAction::RotateObject {
                position: [-16, 6, 15],
                expected: 149,
                facing: 4,
                reverse: true,
            }
        ));
        assert_eq!(restored.source.unwrap().revision, 11);
        assert_eq!(restored.revision, 19);
        assert_eq!(
            serde_json::to_string(&terra_core::ItemKind::Tool(terra_core::ToolKind::Wrench))
                .unwrap(),
            r#"{"tool":"wrench"}"#
        );
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

    #[test]
    fn equipment_control_payloads_are_boxed_without_changing_their_wire_shape() {
        use terra_core::{ArmorItem, ArmorMaterial, ArmorPiece, EquipmentSlot, PlayerStats};
        assert!(std::mem::size_of::<ServerControl>() <= 256);
        let mut stats = PlayerStats::default();
        stats.equipment.set(
            EquipmentSlot::Helmet,
            Some(terra_core::ItemKind::Armor(ArmorItem {
                material: ArmorMaterial::Iron,
                piece: ArmorPiece::Helmet,
            })),
        );
        stats.survival.apply_equipment(stats.equipment);
        let packet = ServerControl::Stats {
            generation: 7,
            stats: Box::new(stats),
        };
        let json = serde_json::to_value(&packet).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"Stats": {"generation": 7, "stats": stats}})
        );
        let ServerControl::Stats {
            generation,
            stats: received,
        } = serde_json::from_value(json).unwrap()
        else {
            panic!("expected stats packet");
        };
        assert_eq!(generation, 7);
        assert_eq!(*received, stats);

        let state = PlayerState {
            survival: stats.survival,
            equipment: stats.equipment,
            ..Default::default()
        };
        for (packet, expected) in [
            (
                ServerControl::Teleport {
                    tick: 9,
                    state: Box::new(state),
                },
                serde_json::json!({"Teleport": {"tick": 9, "state": state}}),
            ),
            (
                ServerControl::GameMode {
                    mode: GameMode::Survival,
                    allow_flight: false,
                    tick: 9,
                    state: Box::new(state),
                },
                serde_json::json!({"GameMode": {"mode": GameMode::Survival, "allow_flight": false, "tick": 9, "state": state}}),
            ),
        ] {
            let json = serde_json::to_value(&packet).unwrap();
            assert_eq!(json, expected);
            let received: ServerControl = serde_json::from_value(json).unwrap();
            let (ServerControl::Teleport {
                state: received, ..
            }
            | ServerControl::GameMode {
                state: received, ..
            }) = received
            else {
                panic!("expected player state packet");
            };
            assert_eq!(received.equipment, state.equipment);
            assert_eq!(received.survival, state.survival);
        }
    }

    #[test]
    fn item_metadata_in_container_filters_does_not_enlarge_control_packets_or_change_wire_shape() {
        use super::super::inventory::{ContainerSnapshot, ContainerView};
        use terra_core::{
            CapeItem, ItemKind,
            kingdom::KingdomHeraldry,
            logistics::{DistributorConfig, FilterConfig},
        };
        assert!(std::mem::size_of::<ServerControl>() <= 256);
        assert_eq!(
            std::mem::size_of::<Option<Box<ContainerSnapshot>>>(),
            std::mem::size_of::<usize>()
        );
        let cape = ItemKind::Cape(CapeItem::ALL_WHITE[0])
            .with_default_heraldry(Some(KingdomHeraldry::default()));
        let snapshot = ContainerSnapshot {
            anchor: [-17, 4, 33],
            view: ContainerView::Primary,
            attached_campfire: false,
            workbench: None,
            furnace: None,
            crusher: None,
            mixer: None,
            filter: None,
            distributor: Some(DistributorConfig {
                filters: [FilterConfig {
                    item: Some(cape),
                    allowed: true,
                }; 3],
            }),
            tunnel_random: None,
            cauldron: None,
            campfire: None,
            halves: Vec::new(),
        };
        assert!(snapshot.valid());
        let unboxed = Some(snapshot.clone());
        let boxed = Some(Box::new(snapshot.clone()));
        assert_eq!(
            postcard::to_stdvec(&unboxed).unwrap(),
            postcard::to_stdvec(&boxed).unwrap()
        );
        let packet = ServerControl::Container(boxed);
        assert_eq!(
            serde_json::to_value(&packet).unwrap(),
            serde_json::json!({"Container": snapshot})
        );
        let bytes = postcard::to_stdvec(&packet).unwrap();
        let ServerControl::Container(Some(received)) =
            postcard::from_bytes::<ServerControl>(&bytes).unwrap()
        else {
            panic!("container packet expected")
        };
        assert_eq!(*received, snapshot);
        let closed = ServerControl::Container(None);
        assert_eq!(
            serde_json::to_value(&closed).unwrap(),
            serde_json::json!({"Container": null})
        );
    }
}

#[cfg(test)]
mod world_creation_wire_tests {
    use super::*;
    #[test]
    fn welcome_roundtrip_preserves_none_and_selected_spawn_biomes_and_all_boolean_rules() {
        for preferred_spawn_biome in [None, Some(terra_core::BiomeKind::CherryForest)] {
            for (kingdoms, outposts) in [(false, false), (false, true), (true, false), (true, true)]
            {
                let world = WorldConfig {
                    preferred_spawn_biome,
                    structures: terra_core::StructureSettings {
                        villages: false,
                        ruins: false,
                        mines: false,
                        kingdoms,
                        outposts,
                    },
                    game_rules: terra_core::GameRules {
                        keep_inventory: false,
                        daylight_cycle: false,
                        weather_cycle: false,
                        mob_griefing: false,
                        fire_spread: false,
                        pvp: true,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let packet = ServerControl::Welcome {
                    player: PlayerId(7),
                    uuid: PlayerUuid::new_v4(),
                    world: Box::new(world.clone()),
                    tick_rate: 60,
                    spawn: [8.5, 3.0, 24.5],
                    location: None,
                    allow_flight: false,
                    allow_build: true,
                    allow_environment: false,
                    game_mode: GameMode::Spectator,
                    clock: Default::default(),
                    weather: Default::default(),
                };
                let bytes = postcard::to_allocvec(&packet).unwrap();
                let ServerControl::Welcome {
                    world: received,
                    tick_rate,
                    game_mode,
                    ..
                } = postcard::from_bytes::<ServerControl>(&bytes).unwrap()
                else {
                    panic!("welcome");
                };
                assert_eq!(*received, world);
                assert_eq!(tick_rate, 60);
                assert_eq!(game_mode, GameMode::Spectator);
            }
        }
    }
}
