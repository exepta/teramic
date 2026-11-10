//! Short native fixture: real finite plumbing, held filling, shutoff and watering.
#[path = "hydraulics_smoke/surfaces.rs"]
mod surfaces;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{path::PathBuf, time::Instant};
use terra_core::{GameConfig, GameMode, ItemKind, ItemStack, PlayerProfile, ToolKind, plumbing};
use terra_network::client::ClientSession;
use terra_world::edits::{BlockObjectState, EditJournal};

const TAP: [i64; 3] = [3, 18, 0];
const VALVE: [i64; 3] = [2, 18, 0];
const MAIN_SOURCE: plumbing::BasinShape = plumbing::BasinShape {
    origin: [-1, 19, -3],
    size: [2, 5, 4],
};
const PLOT: [i64; 3] = [-2, 16, 4];
const RECEIVER: [i64; 3] = [3, 17, 0];
const BUCKET_SOURCE: [i64; 3] = [5, 17, 3];
const CAN_SOURCE: [i64; 3] = [6, 17, 3];
const UNDERWATER_BED: [i64; 3] = [6, 16, 3];
const INFUSER_VALVE: [i64; 3] = [7, 20, 0];
const HOLD: [i64; 3] = [10, 17, 0];
const INFUSER: [i64; 3] = terra_core::infusion::infuser_above(HOLD);
const DRIVE_CRANK: [i64; 3] = [12, 17, -1];
const F29_ORIGIN: [i64; 3] = [-20, 17, 4];
const INTAKE_PUMP: [i64; 3] = [-4, 19, -1];
const INTAKE_CRANK: [i64; 3] = [-4, 19, -2];
const INTAKE_PIPE: [i64; 3] = [-6, 18, -1];
const PUMP_RECEIVER: [i64; 3] = [-3, 17, 2];
const OUTLET_TAP: [i64; 3] = [-3, 20, 2];
const BUILD_FLOOR: [i64; 3] = [16, 16, 8];
const BUILD_PIPE: [i64; 3] = [16, 17, 8];
const BUILD_SLOT: usize = 7;
const VALVE_SLOT: usize = 8;
const TEST_VERTICAL_VALVE: [i64; 3] = [20, 22, 0];
const TEST_HORIZONTAL_VALVE: [i64; 3] = [21, 20, 0];
const TEST_DRAIN_TAP: [i64; 3] = [23, 20, 0];
const TEST_GLASS: [[i64; 3]; 3] = [[20, 21, 0], [20, 20, 0], [22, 20, 0]];
const TEST_SOURCE: plumbing::BasinShape = plumbing::BasinShape {
    origin: [19, 23, -1],
    size: [2, 2, 2],
};
const TEST_RECEIVER: plumbing::BasinShape = plumbing::BasinShape {
    origin: [23, 16, 0],
    size: [2, 4, 1],
};
const TEST_SOURCE_UNITS: u16 = 64;
// Existing outlet path: horizontal run and both vertically joined elbows.
// All remain empty until the ordinary hand crank actually drives the pump.
const GLASS_PIPES: [[i64; 3]; 3] = [[-3, 19, -1], [-3, 20, -1], [-3, 20, 0]];
const GLASS_FLOW_SECONDS: f32 = 3.0;
const GLASS_FLOW_SAMPLES: u16 = 10;
const INFUSER_ITEMS: [ItemKind; 9] = [
    ItemKind::Block(3),
    ItemKind::Block(2),
    ItemKind::Block(1),
    ItemKind::GlassBottle,
    ItemKind::Tool(ToolKind::Bucket),
    ItemKind::Tool(ToolKind::IronWateringCan),
    ItemKind::WaterBottle,
    ItemKind::Tool(ToolKind::WaterBucket),
    ItemKind::Block(3),
];

fn queue_input_z(index: usize) -> f64 {
    -0.60 - index as f64 * 0.26
}
fn bypass_item(id: u64) -> Option<ItemKind> {
    let item = *INFUSER_ITEMS.get(id.checked_sub(1)? as usize)?;
    (!terra_core::infusion::fillable(item)).then_some(item)
}

#[derive(Clone, Copy, Debug)]
struct DropStamp {
    // The independent mined drop ages in server simulation time. Zero denotes
    // the confirmed player tick fallback, captured only for a new drop payload.
    source: u64,
    seconds: f64,
}

#[derive(Debug)]
struct BypassProbe {
    position: f64,
    stamp: Option<DropStamp>,
    stationary_seconds: f64,
    samples: u16,
}

/// Count actual received plumbing snapshots, not frames presenting one snapshot.
#[derive(Debug)]
struct GlassFlowProbe {
    first_sequence: u64,
    last_sequence: u64,
    first_at: Instant,
    last_at: Instant,
    samples: u16,
    render_frames: u32,
    initial_receiver: u16,
    receiver: u16,
    minimum_vertices: [usize; GLASS_PIPES.len()],
    transfer_samples: [u16; GLASS_PIPES.len()],
    transit_seen: [bool; GLASS_PIPES.len()],
}

#[derive(Clone, Copy, Debug)]
struct GlassFlowSample {
    sequence: u64,
    amount: [u16; GLASS_PIPES.len()],
    flowing: [bool; GLASS_PIPES.len()],
    vertices: [usize; GLASS_PIPES.len()],
    active_pump: bool,
    receiver: u16,
}

impl GlassFlowSample {
    fn continuous(self) -> bool {
        self.active_pump
            && (0..GLASS_PIPES.len()).all(|index| self.amount[index] > 0 || self.flowing[index])
            && self.vertices.into_iter().all(|vertices| vertices > 0)
    }
}

impl GlassFlowProbe {
    fn new(sample: GlassFlowSample, now: Instant) -> Self {
        Self {
            first_sequence: sample.sequence,
            last_sequence: sample.sequence,
            first_at: now,
            last_at: now,
            samples: 1,
            render_frames: 1,
            initial_receiver: sample.receiver,
            receiver: sample.receiver,
            minimum_vertices: sample.vertices,
            transfer_samples: sample.flowing.map(u16::from),
            transit_seen: sample.amount.map(|amount| amount == 0),
        }
    }

    fn observe(&mut self, sample: GlassFlowSample, now: Instant) -> Result<bool, String> {
        if !sample.continuous() {
            return Err(format!(
                "Continuously powered glass lost confirmed contents/transfer or visible water: previous_sequence={} sample={sample:?}",
                self.last_sequence
            ));
        }
        self.render_frames += 1;
        for index in 0..GLASS_PIPES.len() {
            self.minimum_vertices[index] = self.minimum_vertices[index].min(sample.vertices[index]);
        }
        if sample.sequence <= self.last_sequence {
            return Ok(self.complete());
        }
        if sample.receiver < self.receiver {
            return Err(format!(
                "Powered glass receiver lost confirmed water: before={} sample={sample:?}",
                self.receiver
            ));
        }
        self.last_sequence = sample.sequence;
        self.last_at = now;
        self.samples += 1;
        self.receiver = sample.receiver;
        for index in 0..GLASS_PIPES.len() {
            self.transfer_samples[index] += u16::from(sample.flowing[index]);
            self.transit_seen[index] |= sample.amount[index] == 0;
        }
        Ok(self.complete())
    }

    fn seconds(&self) -> f32 {
        self.last_at.duration_since(self.first_at).as_secs_f32()
    }

    fn complete(&self) -> bool {
        self.samples >= GLASS_FLOW_SAMPLES
            && self.seconds() >= GLASS_FLOW_SECONDS
            && self.receiver > self.initial_receiver
            && self.transfer_samples.into_iter().all(|samples| samples > 0)
    }
}

impl BypassProbe {
    fn new(position: f64, stamp: DropStamp) -> Self {
        Self {
            position,
            stamp: Some(stamp),
            stationary_seconds: 0.0,
            samples: 1,
        }
    }

    fn pause(&mut self) {
        self.stamp = None;
        self.stationary_seconds = 0.0;
    }

    fn observe(&mut self, position: f64, stamp: DropStamp) -> Result<bool, &'static str> {
        if position < self.position - 0.001 {
            return Err("moved backwards");
        }
        let moved = position > self.position + 0.001;
        if moved {
            self.position = position;
            self.stationary_seconds = 0.0;
            self.samples = self.samples.saturating_add(1);
        } else if let Some(previous) = self.stamp {
            if previous.source == stamp.source && stamp.seconds >= previous.seconds {
                self.stationary_seconds += stamp.seconds - previous.seconds;
            } else {
                self.stationary_seconds = 0.0;
            }
        }
        self.stamp = Some(stamp);
        if self.stationary_seconds > 0.35 {
            return Err("stopped on the powered Hold Block");
        }
        Ok(moved)
    }
}

fn drop_stamp(session: &ClientSession, tick: u64, tick_rate: u16) -> DropStamp {
    session
        .drops
        .iter()
        .filter(|drop| drop.id > INFUSER_ITEMS.len() as u64)
        .min_by_key(|drop| drop.id)
        .map_or(
            DropStamp {
                source: 0,
                seconds: tick as f64 / f64::from(tick_rate.max(1)),
            },
            |drop| DropStamp {
                source: drop.id,
                seconds: f64::from(drop.age_seconds),
            },
        )
}

fn path() -> PathBuf {
    std::env::temp_dir().join(format!("terra-hydraulics-{}", std::process::id()))
}

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.landscape_revision = 0;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = true;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    if path().exists() {
        return Err("Hydraulics fixture requires a fresh temporary directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    config.server.player_permissions.insert(
        profile.uuid,
        terra_core::PlayerPermissions {
            game_mode: false,
            build: true,
            flight: false,
            environment: true,
        },
    );
    let mut journal = EditJournal::default();
    for x in -5..=14 {
        for z in -4..=5 {
            journal.record([x, 16, z], 3).map_err(|e| e.to_string())?;
            for y in 17..=23 {
                journal.record([x, y, z], 0).map_err(|e| e.to_string())?;
            }
        }
    }
    for x in -1..=1 {
        for z in 6..=8 {
            journal.record([x, 26, z], 3).map_err(|e| e.to_string())?;
        }
    }
    let shape = MAIN_SOURCE;
    for p in shape.cells() {
        journal
            .record(p, plumbing::BASIN)
            .map_err(|e| e.to_string())?;
        journal.set_object_state(p, Some(BlockObjectState::default()))?;
        journal.set_plumbing(
            p,
            terra_world::plumbing::PlumbingState {
                water: terra_world::water::FluidCell {
                    amount: 8,
                    ..default()
                },
                pump_credit: 0.0,
                train_reserve: None,
                basin: Some(shape),
                intake: None,
                flowing: false,
            },
        )?;
    }
    for (p, material, facing) in [
        ([0, 18, 0], plumbing::IRON_PIPE, 0),
        ([1, 18, 0], plumbing::IRON_PIPE, 0),
        (VALVE, plumbing::VALVE, 0),
        (TAP, plumbing::FAUCET, 5),
        (RECEIVER, plumbing::BASIN, 0),
        ([-3, 17, 0], plumbing::PUMP, 0),
        ([-5, 17, 4], plumbing::WOOD_PIPE, 0),
        ([-5, 18, 4], plumbing::STONE_PIPE, 1),
    ] {
        journal.record(p, material).map_err(|e| e.to_string())?;
        journal.set_object_state(
            p,
            Some(BlockObjectState {
                facing,
                ..default()
            }),
        )?;
        journal.set_plumbing(p, Default::default())?;
    }
    // Gravity supplies the nozzle independently from the real crank-driven belts.
    for x in 1..=10 {
        let p = [x, 20, 0];
        let (material, facing) = if p == INFUSER_VALVE {
            (plumbing::VALVE, 8)
        } else {
            (plumbing::IRON_PIPE, 0)
        };
        journal.record(p, material).map_err(|e| e.to_string())?;
        journal.set_object_state(
            p,
            Some(BlockObjectState {
                facing,
                ..default()
            }),
        )?;
        journal.set_plumbing(p, Default::default())?;
    }
    prepare_powered_station(&mut journal)?;
    prepare_intake_pool(&mut journal)?;
    prepare_f29_pool(&mut journal)?;
    prepare_underwater_construction_pool(&mut journal)?;
    prepare_valve_drain_line(&mut journal)?;
    surfaces::prepare(&mut journal)?;
    // The continuous-flow receiver measures only the falling outlet. The old
    // showcase pipes must not silently become a second outlet from this basin.
    let pump_receiver = plumbing::BasinShape {
        origin: PUMP_RECEIVER,
        size: [2, 2, 2],
    };
    for cell in pump_receiver.cells() {
        for direction in plumbing::DIRECTIONS {
            let neighbor = std::array::from_fn(|axis| cell[axis] + direction[axis]);
            if pump_receiver.contains(neighbor) {
                continue;
            }
            if let Some(material) = journal.block(neighbor)
                && plumbing::is_part(material)
                && plumbing::joined(
                    plumbing::BASIN,
                    0,
                    material,
                    journal.object_state(neighbor).unwrap_or_default().facing,
                    direction,
                )
            {
                return Err(format!(
                    "Native pump receiver has an unintended hydraulic connection at {neighbor:?}"
                ));
            }
        }
    }
    for x in 12..=14 {
        for z in 2..=3 {
            journal.record([x, 18, z], 3).map_err(|e| e.to_string())?;
        }
    }
    journal
        .record(PLOT, terra_core::farming::FARMLAND)
        .map_err(|e| e.to_string())?;
    let crop = terra_core::CropKind::Wheat;
    let id = terra_world::farming::crop_id(PLOT, crop);
    let mut prop = terra_world::props::WorldProp {
        id,
        kind: terra_world::props::PropKind::Wheat,
        position: [-1.5, 17.0, 4.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: terra_voxel::global_to_section(PLOT).0,
        removed: false,
    };
    terra_world::props::placement::align(&mut prop, PLOT, 0);
    journal.place_prop(prop).map_err(|e| e.to_string())?;
    let mut state = terra_core::CropState::new(crop, id.0);
    state.moisture = 0.15;
    journal.set_crop(PLOT, state)?;
    // Keep source-water regressions hydraulically separate from the faucet
    // observer. Otherwise lateral flow can move that player before filling.
    for x in 4..=7 {
        for z in 2..=4 {
            if x == 4 || x == 7 || z == 2 || z == 4 {
                journal.record([x, 17, z], 3).map_err(|e| e.to_string())?;
            }
        }
    }
    // This dry platform sees over the pool's front wall while remaining in
    // ordinary server-controlled walking mode.
    for x in 5..=6 {
        for z in 6..=7 {
            journal.record([x, 19, z], 3).map_err(|e| e.to_string())?;
        }
    }
    for position in [BUCKET_SOURCE, CAN_SOURCE] {
        journal.record(position, 27).map_err(|e| e.to_string())?;
    }
    journal
        .record(UNDERWATER_BED, 2)
        .map_err(|e| e.to_string())?;
    let bag = journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default();
    for (slot, item, count) in [
        (0, ItemKind::GlassBottle, 2),
        (1, ItemKind::Tool(ToolKind::Bucket), 1),
        (2, ItemKind::Tool(ToolKind::IronWateringCan), 1),
        (4, ItemKind::Tool(ToolKind::Bucket), 1),
        (5, ItemKind::Tool(ToolKind::IronWateringCan), 1),
        (6, ItemKind::Tool(ToolKind::WoodPickaxe), 1),
        (VALVE_SLOT, ItemKind::Block(plumbing::VALVE), 2),
    ] {
        if !bag.creative_set(slot, ItemStack { item, count }) {
            return Err("Cannot prepare vessel slots".into());
        }
    }
    if bag.slot(BUILD_SLOT).is_some()
        || !bag.creative_set(
            BUILD_SLOT,
            ItemStack {
                item: ItemKind::Block(plumbing::GLASS_PIPE),
                count: 1,
            },
        )
    {
        return Err("Underwater construction requires its own free single-pipe hotbar slot".into());
    }
    surfaces::prepare_inventory(bag)?;
    terra_storage::save_edits(&path(), config.world.id, &journal)?;
    let reloaded = terra_storage::load_edits(&path(), config.world.id)?;
    if reloaded.plumbing_at(shape.origin).and_then(|s| s.basin) != Some(shape) {
        return Err("Saved finite basin shape or contents were lost".into());
    }
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path()))?;
    Ok((host, token, profile))
}

/// Both valve positions intentionally remain air until real client requests
/// place them. The high finite source cannot reach the three initial dry pipes.
fn prepare_valve_drain_line(journal: &mut EditJournal) -> Result<(), String> {
    for x in 18..=25 {
        for z in -2..=4 {
            for y in 16..=26 {
                journal
                    .record([x, y, z], if y == 16 { 3 } else { 0 })
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    for (shape, full) in [(TEST_SOURCE, true), (TEST_RECEIVER, false)] {
        for cell in shape.cells() {
            journal
                .record(cell, plumbing::BASIN)
                .map_err(|e| e.to_string())?;
            journal.set_object_state(cell, Some(BlockObjectState::default()))?;
            journal.set_plumbing(
                cell,
                terra_world::plumbing::PlumbingState {
                    basin: Some(shape),
                    water: terra_world::water::FluidCell {
                        amount: if full { 8 } else { 0 },
                        ..default()
                    },
                    ..default()
                },
            )?;
        }
    }
    for (cell, facing) in [(TEST_GLASS[0], 4), (TEST_GLASS[1], 4), (TEST_GLASS[2], 0)] {
        journal
            .record(cell, plumbing::GLASS_PIPE)
            .map_err(|e| e.to_string())?;
        journal.set_object_state(
            cell,
            Some(BlockObjectState {
                facing,
                ..default()
            }),
        )?;
        journal.set_plumbing(cell, Default::default())?;
    }
    journal
        .record(TEST_DRAIN_TAP, plumbing::FAUCET)
        .map_err(|e| e.to_string())?;
    journal.set_object_state(
        TEST_DRAIN_TAP,
        Some(BlockObjectState {
            facing: plumbing::faucet_facing(1, 0),
            ..default()
        }),
    )?;
    journal.set_plumbing(TEST_DRAIN_TAP, Default::default())?;
    for cell in [[21, 21, 3], [23, 18, 3], [24, 20, 3]] {
        journal.record(cell, 3).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Separate from the intake, transport line and isolated F29 epoch observer.
fn prepare_underwater_construction_pool(journal: &mut EditJournal) -> Result<(), String> {
    for x in 14..=17 {
        for z in 6..=9 {
            for y in 16..=23 {
                let wall = x == 14 || x == 17 || z == 6 || z == 9;
                let material = if y == 16 || wall && y <= 20 {
                    3
                } else if y < 20 {
                    27
                } else {
                    0
                };
                journal
                    .record([x, y, z], material)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn prepare_f29_pool(journal: &mut EditJournal) -> Result<(), String> {
    for x in F29_ORIGIN[0] - 1..=F29_ORIGIN[0] + 2 {
        for z in F29_ORIGIN[2] - 1..=F29_ORIGIN[2] + 2 {
            journal
                .record([x, 16, z], 3)
                .map_err(|error| error.to_string())?;
            let boundary = x == F29_ORIGIN[0] - 1
                || x == F29_ORIGIN[0] + 2
                || z == F29_ORIGIN[2] - 1
                || z == F29_ORIGIN[2] + 2;
            let diagonal = x - F29_ORIGIN[0] == z - F29_ORIGIN[2];
            journal
                .record(
                    [x, 17, z],
                    if boundary {
                        3
                    } else if diagonal {
                        27
                    } else {
                        0
                    },
                )
                .map_err(|error| error.to_string())?;
            journal
                .record([x, 18, z], 0)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// A separate raw-water body is replenishing by its actual connected volume.
/// All source cells, shell and machinery are ordinary saved world state.
fn prepare_intake_pool(journal: &mut EditJournal) -> Result<(), String> {
    for x in -11..=-5 {
        for z in -4..=2 {
            journal
                .record([x, 16, z], 3)
                .map_err(|error| error.to_string())?;
            for y in 17..=22 {
                let material = if y < 19 && (x == -11 || x == -5 || z == -4 || z == 2) {
                    3
                } else if y < 19 {
                    27
                } else {
                    0
                };
                journal
                    .record([x, y, z], material)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    let receiver = plumbing::BasinShape {
        origin: PUMP_RECEIVER,
        size: [2, 2, 2],
    };
    for cell in receiver.cells() {
        journal
            .record(cell, plumbing::BASIN)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(cell, Some(BlockObjectState::default()))?;
        journal.set_plumbing(
            cell,
            terra_world::plumbing::PlumbingState {
                basin: Some(receiver),
                ..Default::default()
            },
        )?;
    }
    for (position, material, facing) in [
        (INTAKE_PIPE, plumbing::IRON_PIPE, 4),
        ([-6, 19, -1], plumbing::IRON_PIPE, 4),
        ([-5, 19, -1], plumbing::IRON_PIPE, 0),
        (INTAKE_PUMP, plumbing::PUMP, 1),
        ([-3, 19, -1], plumbing::GLASS_PIPE, 0),
        ([-3, 20, -1], plumbing::GLASS_PIPE, 4),
        ([-3, 20, 0], plumbing::GLASS_PIPE, 3),
        ([-3, 20, 1], plumbing::IRON_PIPE, 3),
        (OUTLET_TAP, plumbing::FAUCET, plumbing::faucet_facing(0, 0)),
    ] {
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(
            position,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )?;
        journal.set_plumbing(position, Default::default())?;
    }
    // Preserve the water outside the submerged pipe body as an ordinary
    // waterlogged construction; internal pipe storage remains initially empty.
    journal
        .set_fluid(
            INTAKE_PIPE,
            terra_world::water::FluidCell {
                amount: 8,
                source: false,
                ..Default::default()
            },
        )
        .map_err(|error| error.to_string())?;
    journal
        .record(INTAKE_CRANK, terra_core::crusher::CRANK_MATERIAL)
        .map_err(|error| error.to_string())?;
    journal.set_object_state(
        INTAKE_CRANK,
        Some(BlockObjectState {
            facing: 1,
            peer: Some(INTAKE_PUMP),
            ..Default::default()
        }),
    )?;
    if journal.crank_anchor(INTAKE_CRANK) != Some(INTAKE_PUMP) {
        return Err("Native intake crank does not mount on the pump shaft".into());
    }
    for x in -4..=-3 {
        for z in -5..=-4 {
            journal
                .record([x, 20, z], 3)
                .map_err(|error| error.to_string())?;
        }
    }
    // A reachable, saved observer step permits an ordinary tap-opening click.
    journal
        .record([0, 20, 3], 3)
        .map_err(|error| error.to_string())?;
    let chunks = fixture_chunks(journal)?;
    let outlet_eye = Vec3::new(
        0.5,
        21.91 + terra_physics::character::PLAYER_EYE_OFFSET,
        3.5,
    );
    let outlet_target = Vec3::from_array(OUTLET_TAP.map(|value| value as f32 + 0.5));
    let (_, hit) = terra_world::targeting::pick_world_with_context(
        &chunks,
        journal,
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        outlet_eye.to_array().map(f64::from),
        (outlet_target - outlet_eye)
            .normalize()
            .to_array()
            .map(f64::from),
        terra_world::props::PropPose::Rest,
    );
    if !matches!(hit, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == OUTLET_TAP && hit.distance <= 6.)
    {
        return Err(format!(
            "Native outlet observer cannot reach its tap: {hit:?}"
        ));
    }
    let eye = Vec3::new(
        -3.5,
        21.91 + terra_physics::character::PLAYER_EYE_OFFSET,
        -4.5,
    );
    let grip = Vec3::new(-3.5, 19.685, -1.55);
    let (_, target) = terra_world::targeting::pick_world_with_context(
        &chunks,
        journal,
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        eye.to_array().map(f64::from),
        (grip - eye).normalize().to_array().map(f64::from),
        terra_world::props::PropPose::Rest,
    );
    if !matches!(target, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == INTAKE_CRANK && hit.distance <= 6.0)
    {
        return Err(format!(
            "Native intake observer does not see its mounted pump crank: {target:?}"
        ));
    }
    Ok(())
}

fn fixture_chunks(journal: &EditJournal) -> Result<terra_world::streaming::ChunkStore, String> {
    use terra_world::streaming::{ChunkStore, LoadedSection};
    let mut chunks = ChunkStore::default();
    for owner in journal.changed_sections() {
        let mut data =
            terra_voxel::BlockSection::empty(owner).map_err(|error| error.to_string())?;
        for (local, material) in journal.section_delta(owner).blocks {
            data.set_block(local.map(usize::from), material)
                .map_err(|error| error.to_string())?;
        }
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    Ok(chunks)
}

/// Author real linked belts through the production placement transaction. This
/// snapshot is used only while building the saved fixture, before the server starts.
fn prepare_powered_station(journal: &mut EditJournal) -> Result<(), String> {
    use terra_core::kinetic;
    use terra_world::{conveyors, streaming::ChunkStore};
    let mut chunks = fixture_chunks(journal)?;
    let put = |chunks: &mut ChunkStore,
               journal: &mut EditJournal,
               position,
               material,
               state|
     -> Result<(), String> {
        let (owner, local) = terra_voxel::global_to_section(position);
        chunks
            .loaded
            .get_mut(&owner)
            .ok_or("Native belt fixture section was not prepared")?
            .data
            .set_block(local, material)
            .map_err(|error| error.to_string())?;
        journal
            .record(position, material)
            .map_err(|error| error.to_string())?;
        journal.set_object_state(position, Some(state))?;
        Ok(())
    };
    for (start, end) in [([10, 17, -4], [10, 17, -1]), ([10, 17, 1], [10, 17, 4])] {
        for position in [start, end] {
            put(
                &mut chunks,
                journal,
                position,
                kinetic::IRON_SHAFT,
                BlockObjectState::default(),
            )?;
        }
        let plan = conveyors::layout(
            &chunks,
            journal,
            start,
            end,
            ItemKind::for_wood(kinetic::CONVEYOR, terra_core::WoodSpecies::Spruce),
        )?;
        if !conveyors::commit(&mut chunks, journal, &plan)? {
            return Err("Native linked belt placement was not committed".into());
        }
    }
    // Two genuine cog meshes reverse the middle axle twice, keeping both belt
    // surfaces moving +Z from one ordinary mounted hand crank.
    for z in -1..=1 {
        put(
            &mut chunks,
            journal,
            [11, 17, z],
            kinetic::IRON_GEAR,
            BlockObjectState::default(),
        )?;
    }
    put(
        &mut chunks,
        journal,
        DRIVE_CRANK,
        terra_core::crusher::CRANK_MATERIAL,
        BlockObjectState {
            peer: Some([11, 17, -1]),
            ..Default::default()
        },
    )?;
    if journal.crank_anchor(DRIVE_CRANK) != Some([11, 17, -1]) {
        return Err("Native drive crank is not mounted on its real gear axle".into());
    }
    // A deliberately different saved yaw verifies that adjacent belts determine
    // the Hold Block's working axis, rather than an assumed placement direction.
    put(
        &mut chunks,
        journal,
        HOLD,
        terra_core::infusion::HOLD_BLOCK,
        BlockObjectState {
            facing: 1,
            ..Default::default()
        },
    )?;
    let gap = terra_core::infusion::gap_above(HOLD);
    journal.record(gap, 0).map_err(|error| error.to_string())?;
    let (gap_owner, gap_local) = terra_voxel::global_to_section(gap);
    chunks
        .loaded
        .get_mut(&gap_owner)
        .ok_or("Native infuser gap is not loaded")?
        .data
        .set_block(gap_local, 0)
        .map_err(|error| error.to_string())?;
    put(
        &mut chunks,
        journal,
        INFUSER,
        terra_core::infusion::FLUID_INFUSER,
        BlockObjectState::default(),
    )?;
    journal.set_plumbing(INFUSER, Default::default())?;
    let eye = Vec3::new(
        13.5,
        19.91 + terra_physics::character::PLAYER_EYE_OFFSET,
        3.5,
    );
    let grip = Vec3::new(12.55, 17.685, -0.5);
    let (_, target) = terra_world::targeting::pick_world_with_context(
        &chunks,
        journal,
        &terra_core::block_properties::BlockPropertiesCatalog::default(),
        eye.to_array().map(f64::from),
        (grip - eye).normalize().to_array().map(f64::from),
        terra_world::props::PropPose::Rest,
    );
    if !matches!(target, Some(terra_world::targeting::WorldTarget::Block(hit)) if hit.position == DRIVE_CRANK && hit.distance <= 6.0)
    {
        return Err(format!(
            "Native powered station observer does not see its mounted crank: {target:?}"
        ));
    }
    for (index, item) in INFUSER_ITEMS.into_iter().enumerate() {
        let position = [
            10.5,
            17.0 + terra_core::infusion::DROP_HEIGHT,
            queue_input_z(index),
        ];
        if !journal.gameplay_mut().drop_stack(
            position,
            ItemStack { item, count: 1 },
            [0.0; 3],
            10.0,
        ) {
            return Err("Cannot prepare queued native infuser input".into());
        }
        let drop = journal.gameplay_mut().drops.last_mut().unwrap();
        drop.angular_velocity = [0.0; 3];
        drop.grounded = true;
    }
    Ok(())
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    requested: u8,
    captured: u8,
    blank: bool,
    blocked_inventory: Option<terra_core::Inventory>,
    blocked_source_mass: u16,
    blocked_source_basin: u16,
    blocked_tail_units: u16,
    blocked_receiver_units: u16,
    traced_stage: Option<u8>,
    source_mesh_epoch: u64,
    queue_wait_seen: bool,
    gates_closed_seen: bool,
    gates_open_seen: bool,
    unsupported_samples: u16,
    platform_owner_seen: bool,
    bypass_probes: std::collections::BTreeMap<u64, BypassProbe>,
    drop_payload: Vec<terra_core::WorldDrop>,
    drop_stamp: Option<DropStamp>,
    drop_payload_received: Instant,
    bypass_overlap_seen: bool,
    f29_epochs: Option<(u64, u64, Instant)>,
    f29_verified: bool,
    slot_owner_since: Option<(u64, Instant)>,
    gate_fill_since: Option<Instant>,
    rendered_closed_seen: bool,
    rendered_open_seen: bool,
    glass_empty_verified: bool,
    glass_seen: [bool; GLASS_PIPES.len()],
    glass_peak_units: u16,
    glass_peak_vertices: usize,
    glass_pressure_verified: bool,
    glass_flow_seen: bool,
    glass_continuous_flow: Option<GlassFlowProbe>,
    intake_started: Option<Instant>,
    outlet_open_pulsed: bool,
    underwater_build_pulsed: bool,
    underwater_inventory_revision: u64,
    underwater_previous_request: Option<u64>,
    underwater_eye_cell: Option<[i64; 3]>,
    underwater_source_since: Option<Instant>,
    valve_previous_request: Option<u64>,
    valve_inventory_revision: u64,
    valve_placements: [bool; 2],
    valve_source_remaining: Option<u16>,
    valve_closed_units: u16,
    valve_drain_expected: u16,
    valve_outlet_pulsed: bool,
    valve_partial_seen: bool,
    surfaces: surfaces::Inspection,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-hydraulics") {
        return;
    }
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(
            terra_core::GameAction::Place,
            terra_core::InputBinding::Mouse(MouseButton::Right),
        )
        .expect("isolated hydraulics fixture right-click binding");
    app.insert_resource(controls)
        .insert_resource(Inspection {
            start: Instant::now(),
            since: Instant::now(),
            stage: 0,
            requested: 0,
            captured: 0,
            blank: false,
            blocked_inventory: None,
            blocked_source_mass: 0,
            blocked_source_basin: 0,
            blocked_tail_units: 0,
            blocked_receiver_units: 0,
            traced_stage: None,
            source_mesh_epoch: 0,
            queue_wait_seen: false,
            gates_closed_seen: false,
            gates_open_seen: false,
            unsupported_samples: 0,
            platform_owner_seen: false,
            bypass_probes: Default::default(),
            drop_payload: Vec::new(),
            drop_stamp: None,
            drop_payload_received: Instant::now(),
            bypass_overlap_seen: false,
            f29_epochs: None,
            f29_verified: false,
            slot_owner_since: None,
            gate_fill_since: None,
            rendered_closed_seen: false,
            rendered_open_seen: false,
            glass_empty_verified: false,
            glass_seen: [false; GLASS_PIPES.len()],
            glass_peak_units: 0,
            glass_peak_vertices: 0,
            glass_pressure_verified: false,
            glass_flow_seen: false,
            glass_continuous_flow: None,
            intake_started: None,
            outlet_open_pulsed: false,
            underwater_build_pulsed: false,
            underwater_inventory_revision: 0,
            underwater_previous_request: None,
            underwater_eye_cell: None,
            underwater_source_since: None,
            valve_previous_request: None,
            valve_inventory_revision: 0,
            valve_placements: [false; 2],
            valve_source_remaining: None,
            valve_closed_units: 0,
            valve_drain_expected: 0,
            valve_outlet_pulsed: false,
            valve_partial_seen: false,
            surfaces: Default::default(),
        })
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .add_systems(PostStartup, setup)
        .add_systems(PreUpdate, aim.after(bevy::input::InputSystems))
        .add_systems(Last, run);
}

fn setup(world: &mut World) {
    if let Some(mut menu) = world.get_resource_mut::<terra_ui::menu::MenuState>() {
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}

fn aim(world: &mut World) {
    let stage = world.resource::<Inspection>().stage;
    let elapsed = world.resource::<Inspection>().since.elapsed().as_secs_f32();
    // Preserve the valve sightline until the server has acknowledged opening it;
    // changing aim on the request frame can race the next replicated look input.
    let valve_aim = stage == 28
        || stage == 29
            && world
                .resource::<EditJournal>()
                .object_state(INFUSER_VALVE)
                .is_none_or(|state| !plumbing::open(plumbing::VALVE, state.facing));
    let target = if stage == 53 {
        surfaces::target(&world.resource::<Inspection>().surfaces)
    } else if matches!(stage, 44 | 45) {
        Vec3::new(20.5, 21.999, 0.5)
    } else if matches!(stage, 46 | 47) {
        Vec3::new(20.649, 20.5, 0.5)
    } else if matches!(stage, 48 | 49) {
        Vec3::from_array(TEST_VERTICAL_VALVE.map(|v| v as f32 + 0.5))
    } else if stage >= 50 {
        Vec3::from_array(TEST_DRAIN_TAP.map(|v| v as f32 + 0.5))
    } else if stage == 40 {
        Vec3::from_array(OUTLET_TAP.map(|value| value as f32 + 0.5))
    } else if matches!(stage, 41 | 42) {
        Vec3::new(-3.5, 19.685, -1.55)
    } else if stage >= 35 {
        Vec3::new(16.5, 16.999, 8.5)
    } else if stage >= 32 {
        // +X grip rotated onto the pump's -Z axle (facing1).
        Vec3::new(-3.5, 19.685, -1.55)
    } else if valve_aim {
        Vec3::from_array(INFUSER_VALVE.map(|value| value as f32 + 0.5))
    } else if stage >= 29 {
        // Aim at the real grip, rather than the empty center of the crank model.
        Vec3::new(
            DRIVE_CRANK[0] as f32 + 0.55,
            DRIVE_CRANK[1] as f32 + 0.685,
            DRIVE_CRANK[2] as f32 + 0.5,
        )
    } else if stage >= 28 {
        Vec3::new(10.5, 18.2, 0.5)
    } else if stage == 27 {
        Vec3::new(0.0, 20.0, -2.5)
    } else if stage >= 20 {
        if stage <= 21 {
            Vec3::from_array(BUCKET_SOURCE.map(|v| v as f32 + 0.5))
        } else if stage <= 23 {
            Vec3::from_array(CAN_SOURCE.map(|v| v as f32 + 0.5))
        } else {
            Vec3::from_array(UNDERWATER_BED.map(|v| v as f32 + 0.5))
        }
    } else if stage <= 1 {
        Vec3::new(0.5, 20.5, 0.5)
    } else if matches!(stage, 11 | 13 | 16 | 17) {
        Vec3::new(2.5, 18.5, 0.5)
    } else if stage >= 14 {
        Vec3::new(-1.5, 17.08, 4.5)
    } else {
        Vec3::new(3.5, 18.5, 0.5)
    };
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = true;
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    for mut camera in world
        .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        camera.look_at(target, Vec3::Y);
    }
    // Exactly one ordinary placement edge, after stage35 has verified the
    // underwater eye, held item and actual supporting floor.
    let build_tap = stage == 36
        && !world.resource::<Inspection>().underwater_build_pulsed
        && !world.resource::<terra_player::PlayerInputBlock>().0
        && world
            .resource::<terra_player::CameraInputState>()
            .capture_ready();
    if build_tap {
        world.resource_mut::<Inspection>().underwater_build_pulsed = true;
    }
    let outlet_tap = stage == 40
        && elapsed > 0.35
        && !world.resource::<Inspection>().outlet_open_pulsed
        && !world.resource::<terra_player::PlayerInputBlock>().0
        && world
            .resource::<terra_player::CameraInputState>()
            .capture_ready()
        && world
            .resource::<terra_player::interaction::TargetBlock>()
            .0
            .is_some_and(|hit| hit.position == OUTLET_TAP);
    if outlet_tap {
        world.resource_mut::<Inspection>().outlet_open_pulsed = true;
    }
    let surface_tap = stage == 53 && surfaces::place_edge(world);
    let held = surface_tap
        || build_tap
        || outlet_tap
        || (matches!(stage, 21 | 23) && elapsed < 0.12)
        || matches!(stage, 3 | 6..=7 | 9..=10 | 15)
        || matches!(stage, 12 | 14) && elapsed > 0.35
        || (30..=31).contains(&stage) && !world.resource::<terra_player::PlayerInputBlock>().0
        || matches!(stage, 33 | 42) && !world.resource::<terra_player::PlayerInputBlock>().0
        || stage == 4
            && world.resource::<terra_player::interaction::ActiveItem>().0
                == Some(ItemKind::GlassBottle);
    let place = binding(world, terra_core::GameAction::Place);
    let breaking = binding(world, terra_core::GameAction::Break);
    hold_binding(world, breaking, stage == 25);
    hold_binding(world, place, held);
    // Enter from open air and descend using the ordinary swim control. A teleport
    // into a hollow construction cell is intentionally rejected by /tp's raw-solid guard.
    let descend = stage == 27
        // The real chat teleport resumes input only after every key is released.
        // Wait for that acknowledgement before pressing descent, otherwise the
        // fixture itself keeps teleport_resume_pending latched indefinitely.
        && !world.resource::<terra_player::PlayerInputBlock>().0
        && world
            .get_resource::<ClientSession>()
            .is_some_and(|session| {
                !session.teleporting
                    && session.confirmed_state().is_some_and(|state| {
                        state.position[0].abs() < 0.7
                            && (state.position[2] + 1.5).abs() < 0.7
                            && state.position[1] > 20.6
                    })
            });
    let descent = binding(world, terra_core::GameAction::FlyDown);
    hold_binding(world, descent, descend);
}

fn binding(world: &World, action: terra_core::GameAction) -> terra_core::InputBinding {
    world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || action.default_binding(),
            |controls| controls.binding(action),
        )
}

fn hold_binding(world: &mut World, binding: terra_core::InputBinding, held: bool) {
    use terra_core::InputBinding;
    let key = |world: &mut World, key| {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(key);
        } else {
            keys.release(key);
        }
    };
    match binding {
        InputBinding::Key(code) => key(world, code),
        InputBinding::Chord {
            modifier,
            key: code,
        } => {
            key(world, modifier);
            key(world, code);
        }
        InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            if held {
                mouse.press(button);
            } else {
                mouse.release(button);
            }
        }
    }
}

fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    info!(stage, "Hydraulics native stage");
}

fn capture(
    world: &mut World,
    state: &mut Inspection,
    name: &str,
    count: u8,
) -> Result<bool, String> {
    if state.requested < count {
        let folder = std::env::var_os("TERRA_CAPTURE_DIR")
            .map(PathBuf::from)
            .ok_or("Set TERRA_CAPTURE_DIR for hydraulics captures")?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(folder.join(format!("{name}.png"))))
            .observe(
                |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                    state.blank |= event.image.data.as_ref().is_none_or(|data| {
                        data.chunks_exact(4)
                            .filter(|p| p[..3].iter().any(|v| *v > 20))
                            .count()
                            < 1000
                    });
                    state.captured += 1;
                },
            );
        state.requested = count;
    }
    Ok(state.captured >= count)
}

fn toggle(world: &mut World, position: [i64; 3], expected: u16) {
    let facing = world
        .resource::<EditJournal>()
        .object_state(position)
        .unwrap_or_default()
        .facing;
    world.write_message(terra_network::client::ObjectInteractionRequest {
        position,
        expected,
        facing,
        closing: expected == plumbing::FAUCET && plumbing::open(expected, facing),
    });
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.stage == 255 {
            return;
        }
        if state.since.elapsed().as_secs() >= 5 && state.traced_stage != Some(state.stage) {
            state.traced_stage = Some(state.stage);
            let session = world.resource::<ClientSession>();
            let motion = world.resource::<terra_player::interaction::VesselMotion>();
            let input = world.resource::<terra_player::CameraInputState>();
            info!(stage = state.stage, ready = session.ready, build = session.allow_build,
                edit = ?session.last_edit, inventory = ?session.last_inventory,
                tap = ?session.plumbing.iter().find(|v| v.position == TAP),
                basin = ?session.plumbing.iter().find(|v| v.position == [-1, 19, -1]),
                infusion = ?session.infusion.iter().find(|view| view.position == HOLD),
                drive = ?session.kinetics.iter().find(|part| part.position == DRIVE_CRANK),
                crank_input = ?world.get_resource::<terra_network::client::CrankInput>().map(|input| input.0),
                tap_pose = ?world.resource::<EditJournal>().object_state(TAP),
                tap_block = ?world.resource::<terra_world::streaming::ChunkStore>().block_at(TAP),
                captured = input.captured,
                blocked = world.resource::<terra_player::PlayerInputBlock>().0,
                selected = ?world.resource::<terra_player::interaction::ActiveItem>().0,
                right_mouse = world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Right),
                left_mouse = world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),
                place_binding = ?binding(world, terra_core::GameAction::Place),
                keys = ?world.resource::<ButtonInput<KeyCode>>().get_pressed().collect::<Vec<_>>(),
                target = ?world.resource::<terra_player::interaction::TargetBlock>().0,
                filling = motion.filling, progress = motion.progress, water_target = ?motion.target,
                intake_details = %intake_diagnostic(world, &state),
                "Hydraulics stalled-stage state");
            for (camera, look) in world.query_filtered::<(&Transform, &terra_player::CameraLook), With<terra_player::TerraCamera>>().iter(world) {
                let chunks = world.resource::<terra_world::streaming::ChunkStore>();
                let direction = look.rotation() * Vec3::NEG_Z;
                let raw = terra_voxel::blocks::raycast_blocks(camera.translation.to_array().map(f64::from), direction.to_array().map(f64::from), 8.0, |p| chunks.block_at(p));
                info!(position = ?camera.translation, ?direction, raw_hit = ?raw,
                    raw_material = ?raw.and_then(|v| chunks.block_at(v.position)), "Hydraulics camera ray");
            }
            if let Some(folder) = std::env::var_os("TERRA_CAPTURE_DIR") {
                world.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(folder).join(format!("diagnostic-stage-{}.png", state.stage))));
            }
        }
        if let Err(error) = step(world, &mut state) {
            if matches!(state.stage, 30 | 31) {
                let session = world.resource::<ClientSession>();
                error!(
                    server_tick = world.resource::<terra_network::client::ReplicatedPlayers>().tick,
                    drop_clock = ?state.drop_stamp,
                    payload_wall_age = state.drop_payload_received.elapsed().as_secs_f32(),
                    drops = ?session.drops,
                    infusion = ?session.infusion.iter().find(|view| view.position == HOLD),
                    power = ?session.kinetics.iter().filter(|part| [[10, 17, -1], [10, 17, 1], DRIVE_CRANK].contains(&part.position)).collect::<Vec<_>>(),
                    crank_input = ?world.resource::<terra_network::client::CrankInput>().0,
                    input_blocked = world.resource::<terra_player::PlayerInputBlock>().0,
                    camera_capture = ?world.resource::<terra_player::CameraInputState>(),
                    right_mouse = world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Right),
                    target = ?world.resource::<terra_player::interaction::TargetBlock>().0,
                    "Powered native line failure state"
                );
            }
            error!(%error, stage = state.stage, "Hydraulics native smoke failed");
            state.stage = 255;
            world.write_message(AppExit::error());
        }
    });
}

fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.start.elapsed().as_secs() > 150 || state.blank {
        return Err(format!("Hydraulics timeout/blank stage {}", state.stage));
    }
    if state.stage == 53 {
        if surfaces::step(world, state)? {
            finish(world, state);
        }
        return Ok(());
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready {
        return Ok(());
    }
    observe_f29_pool(world, state)?;
    observe_glass_pipes(world, state)?;
    let elapsed = state.since.elapsed().as_secs_f32();
    if matches!(state.stage, 33 | 40..=42)
        && state
            .intake_started
            .is_some_and(|since| since.elapsed().as_secs_f32() > 18.)
    {
        return Err(format!(
            "Powered intake exceeded its 18-second pressure/open/flow deadline: {}",
            intake_diagnostic(world, state)
        ));
    }
    if state.stage == 3 && elapsed > 15.0 {
        return Err(
            "Bottle filling did not begin; see stalled-stage state and diagnostic capture".into(),
        );
    }
    match state.stage {
        0 if world.resource::<terra_render::RenderStatus>().ready => {
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 0.5 27.91 7.5".into(),
                ));
            advance(state, 1);
        }
        1 if elapsed > 2.0
            && !session.teleporting
            && session
                .plumbing
                .iter()
                .any(|v| v.position == [-1, 19, -1] && v.amount > 0 && v.basin.is_some()) =>
        {
            if !state.glass_empty_verified {
                return Ok(());
            }
            if !capture(world, state, "hydraulics-network", 1)? {
                return Ok(());
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 3.5 17.91 3.7".into(),
                ));
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            advance(state, 2);
        }
        2 if elapsed > 1.5 && !session.teleporting => {
            if session
                .plumbing
                .iter()
                .any(|v| v.position == RECEIVER && v.amount > 0)
            {
                return Err(
                    "The initially empty receiving basin filled before its faucet opened".into(),
                );
            }
            toggle(world, TAP, plumbing::FAUCET);
            advance(state, 3);
        }
        5 | 8 if elapsed > 0.5 => advance(state, state.stage + 1),
        3 | 6 | 9
            if world
                .resource::<terra_player::interaction::VesselMotion>()
                .progress
                > 0.25 =>
        {
            if state.stage == 3 {
                // Give the existing shared device source time to ramp its loop.
                if elapsed < 0.3 {
                    return Ok(());
                }
                let audio = world.resource::<crate::creature_audio::HydraulicAudioStatus>();
                if audio.taps == 0
                    || audio.gains.iter().all(|gain| *gain <= 0.0)
                    || audio.output_rms <= 0.000001
                {
                    return Err(format!(
                        "Acknowledged flowing faucet did not reach the device audio mix: {audio:?}"
                    ));
                }
            }
            let name = match state.stage {
                3 => "bottle-filling",
                6 => "bucket-filling",
                _ => "watering-can-filling",
            };
            let count = match state.stage {
                3 => 2,
                6 => 4,
                _ => 6,
            };
            if !capture(world, state, name, count)? {
                return Ok(());
            }
            advance(state, state.stage + 1);
        }
        4 if session.inventory.count(ItemKind::WaterBottle) > 0 => {
            if !session
                .plumbing
                .iter()
                .any(|v| v.position == RECEIVER && v.amount > 0)
            {
                return Ok(());
            }
            if world.resource::<terra_player::interaction::ActiveItem>().0
                != Some(ItemKind::WaterBottle)
            {
                world.write_message(terra_ui::InventoryAction::Hotbar(3));
                return Ok(());
            }
            if !capture(world, state, "bottle-filled", 3)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(1));
            advance(state, 5);
        }
        7 if session
            .inventory
            .count(ItemKind::Tool(ToolKind::WaterBucket))
            > 0 =>
        {
            if !capture(world, state, "bucket-filled", 5)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(2));
            advance(state, 8);
        }
        10 if session.watering_reserve.0 == ToolKind::IronWateringCan.watering_capacity() => {
            if !capture(world, state, "watering-can-filled", 7)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            advance(state, 11);
        }
        11 if elapsed > 0.5 => {
            let Some(source_side) = main_source_mass(session) else {
                return Ok(());
            };
            let Some(source_basin) = basin_amount(session, MAIN_SOURCE) else {
                return Ok(());
            };
            let Some(receiver) = plumbing_amount(session, RECEIVER) else {
                return Ok(());
            };
            // The faucet is the only downstream storage cell. Its private stored
            // amount is deliberately not exposed in the visual packet; derive
            // its real finite budget from the complete original basin quantity.
            let spent = source_side + receiver + inventory_water_units(&session.inventory);
            let tail = MAIN_SOURCE.capacity().checked_sub(spent)
                .filter(|amount| *amount <= u16::from(plumbing::CELL_CAPACITY))
                .ok_or_else(|| format!("Main faucet could not prove its finite tail before shutoff: source_side={source_side} basin={source_basin} receiver={receiver} vessels={} initial={}", inventory_water_units(&session.inventory), MAIN_SOURCE.capacity()))?;
            if session
                .inventory
                .slot(0)
                .is_none_or(|stack| stack.item != ItemKind::GlassBottle || stack.count != 1)
            {
                return Err(
                    "Closed-valve vessel probe requires its one remaining real empty bottle".into(),
                );
            }
            state.blocked_inventory = Some(session.inventory.clone());
            state.blocked_source_mass = source_side;
            state.blocked_source_basin = source_basin;
            state.blocked_tail_units = tail;
            state.blocked_receiver_units = receiver;
            toggle(world, VALVE, plumbing::VALVE);
            advance(state, 16);
        }
        16 if world
            .resource::<EditJournal>()
            .object_state(VALVE)
            .is_some_and(|v| !plumbing::open(plumbing::VALVE, v.facing)) =>
        {
            advance(state, 12);
        }
        12 if elapsed > 2.5 => {
            let before = state
                .blocked_inventory
                .as_ref()
                .ok_or("Missing pre-shutoff inventory checkpoint")?;
            let filled = session
                .inventory
                .count(ItemKind::WaterBottle)
                .checked_sub(before.count(ItemKind::WaterBottle))
                .ok_or("Closed-valve probe lost an already filled bottle")?;
            if filled > 1 || filled > state.blocked_tail_units {
                return Err(
                    "A closed valve filled more vessels than its proven finite tail".into(),
                );
            }
            let mut expected_inventory = before.clone();
            if filled == 1
                && (!expected_inventory.take_from(0, ItemKind::GlassBottle)
                    || expected_inventory.insert(ItemKind::WaterBottle, 1) != 0)
            {
                return Err(
                    "Closed-valve buffer could not account for its exact vessel conversion".into(),
                );
            }
            if session.inventory != expected_inventory {
                return Err("Closed-valve probe changed inventory outside its exact finite bottle conversion".into());
            }
            let Some(source_side) = main_source_mass(session) else {
                return Ok(());
            };
            let Some(receiver) = plumbing_amount(session, RECEIVER) else {
                return Ok(());
            };
            let Some(tail_remaining) = plumbing_amount(session, TAP) else {
                return Ok(());
            };
            let closed = world
                .resource::<EditJournal>()
                .object_state(VALVE)
                .is_some_and(|pose| !plumbing::open(plumbing::VALVE, pose.facing));
            if !closed || source_side != state.blocked_source_mass {
                return Err(format!(
                    "Closed valve admitted source-side water: closed={closed} source_side={source_side} expected={} basin_before={} basin_now={:?}",
                    state.blocked_source_mass,
                    state.blocked_source_basin,
                    basin_amount(session, MAIN_SOURCE)
                ));
            }
            let received = receiver
                .checked_sub(state.blocked_receiver_units)
                .ok_or("Closed-valve receiver lost water")?;
            if tail_remaining + filled + received != state.blocked_tail_units {
                return Err(format!(
                    "Closed-valve finite tail violated conservation: tail={tail_remaining} bottles={filled} receiver_gain={received} initial_tail={}",
                    state.blocked_tail_units
                ));
            }
            if state.requested < 8 {
                info!(initial_tail = state.blocked_tail_units, tail_remaining, filled_bottles = filled,
                    receiver_gain = received, source_side_before = state.blocked_source_mass,
                    source_side_after = source_side, source_basin_before = state.blocked_source_basin,
                    source_basin_after = ?basin_amount(session, MAIN_SOURCE),
                    "Closed valve finite vessel buffer conserved without source replenishment");
            }
            if !capture(world, state, "valve-closed-finite-buffer", 8)? {
                return Ok(());
            }
            advance(state, 13);
        }
        13 if elapsed > 0.5 => {
            toggle(world, VALVE, plumbing::VALVE);
            advance(state, 17);
        }
        17 if world
            .resource::<EditJournal>()
            .object_state(VALVE)
            .is_some_and(|v| plumbing::open(plumbing::VALVE, v.facing)) =>
        {
            world.write_message(terra_ui::InventoryAction::Hotbar(2));
            advance(state, 14);
        }
        14 if elapsed > 0.8
            && world
                .resource::<terra_player::interaction::VesselMotion>()
                .pouring =>
        {
            if !capture(world, state, "watering-can-pouring", 9)? {
                return Ok(());
            }
            advance(state, 15);
        }
        15 if session
            .crop_progress
            .get(&PLOT)
            .is_some_and(|v| v.moisture > 0.9) =>
        {
            if session.watering_reserve.0 == 0 {
                return Err("Watering consumed the entire can unexpectedly".into());
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 5.5 20.91 6.5".into(),
                ));
            world.write_message(terra_ui::InventoryAction::Hotbar(4));
            advance(state, 20);
        }
        20 if elapsed > 1.0
            && !session.teleporting
            && world.resource::<terra_player::interaction::ActiveItem>().0
                == Some(ItemKind::Tool(ToolKind::Bucket)) =>
        {
            state.source_mesh_epoch = world
                .resource::<EditJournal>()
                .water_mesh_epoch(terra_voxel::global_to_section(BUCKET_SOURCE).0);
            advance(state, 21);
        }
        21 if session
            .inventory
            .slot(4)
            .is_some_and(|s| s.item == ItemKind::Tool(ToolKind::WaterBucket)) =>
        {
            if !capture(world, state, "source-bucket-filled", 10)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(5));
            advance(state, 22);
        }
        22 if elapsed > 0.5 => advance(state, 23),
        23 if session.watering_reserve.0 == ToolKind::IronWateringCan.watering_capacity() => {
            if !capture(world, state, "source-watering-can-filled", 11)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(6));
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 6.5 18.91 4.5".into(),
                ));
            advance(state, 24);
        }
        24 if elapsed > 1.0
            && !session.teleporting
            && session
                .confirmed_state()
                .is_some_and(|state| state.grounded)
            && world.resource::<terra_player::interaction::ActiveItem>().0
                == Some(ItemKind::Tool(ToolKind::WoodPickaxe)) =>
        {
            advance(state, 25)
        }
        25 if world
            .resource::<terra_world::streaming::ChunkStore>()
            .block_at(UNDERWATER_BED)
            == Some(0) =>
        {
            advance(state, 26)
        }
        26 if elapsed > 0.25 => {
            if world
                .resource::<EditJournal>()
                .water_mesh_epoch(terra_voxel::global_to_section(BUCKET_SOURCE).0)
                <= state.source_mesh_epoch
            {
                return Err(
                    "Source transactions did not update real replicated water geometry".into(),
                );
            }
            if !capture(world, state, "underwater-bed-mined", 12)? {
                return Ok(());
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 0 26 -1.5".into(),
                ));
            advance(state, 27);
        }
        27 if elapsed > 0.35 && !session.teleporting => {
            let Some(body) = session.confirmed_state() else {
                return Ok(());
            };
            let position = Vec3::from_array(body.position);
            let chunks = world.resource::<terra_world::streaming::ChunkStore>();
            let journal = world.resource::<EditJournal>();
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let config = &world.resource::<GameConfig>().world;
            if position.x.abs() > 0.7 || (position.z + 1.5).abs() > 0.7 {
                return Ok(());
            }
            if !terra_physics::character::in_water(config, terrain, chunks, journal, position)
                || !terra_physics::character::head_underwater(
                    config, terrain, chunks, journal, position,
                )
            {
                if elapsed > 12.0 {
                    return Err(format!(
                        "Confirmed player did not descend into finite basin water at {position:?}"
                    ));
                }
                return Ok(());
            }
            if !capture(world, state, "basin-swimming", 13)? {
                return Ok(());
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 13.5 19.91 3.5".into(),
                ));
            advance(state, 28);
        }
        28 if elapsed > 1.0 && !session.teleporting => {
            for (index, original) in INFUSER_ITEMS.into_iter().enumerate() {
                let drop = session
                    .drops
                    .iter()
                    .find(|drop| drop.id == index as u64 + 1)
                    .ok_or("The stopped incoming belt lost its queued native item")?;
                if (drop.position[2] - queue_input_z(index)).abs() > 0.00001 {
                    return Err(
                        "The dense native input queue changed its exact .26 spacing before power"
                            .into(),
                    );
                }
                if drop.stack.item != original || drop.stack.count != 1 {
                    return Err(
                        "Closed infuser supply changed an item or duplicated its stack".into(),
                    );
                }
            }
            if world
                .resource::<terra_render::InfuserJetRenderStatus>()
                .streams
                != 0
            {
                return Err("Closed, idle infuser rendered an unconfirmed water stream".into());
            }
            if !capture(world, state, "infuser-ready", 14)? {
                return Ok(());
            }
            toggle(world, INFUSER_VALVE, plumbing::VALVE);
            advance(state, 29);
        }
        29 if elapsed > 0.35
            && world
                .resource::<EditJournal>()
                .object_state(INFUSER_VALVE)
                .is_some_and(|pose| plumbing::open(plumbing::VALVE, pose.facing)) =>
        {
            // The camera has faced the crank for a released-input frame before
            // stage30 begins the ordinary sustained right-click drive action.
            advance(state, 30);
        }
        30 | 31 => {
            if elapsed > 25.0 {
                return Err("Powered infuser line did not finish its real queue".into());
            }
            let tick = world
                .resource::<terra_network::client::ReplicatedPlayers>()
                .tick;
            if state.drop_payload != session.drops {
                state.drop_payload.clone_from(&session.drops);
                state.drop_stamp = Some(drop_stamp(
                    session,
                    tick,
                    world.resource::<GameConfig>().server.tick_rate,
                ));
                state.drop_payload_received = Instant::now();
            }
            let Some(view) = session.infusion.iter().find(|view| view.position == HOLD) else {
                return Ok(());
            };
            let powered = [[10, 17, -1], [10, 17, 1]].into_iter().all(|p| {
                session
                    .kinetics
                    .iter()
                    .any(|part| part.position == p && part.rpm > 0.0 && !part.overloaded)
            });
            let driving = !world.resource::<terra_player::PlayerInputBlock>().0
                && world
                    .resource::<terra_player::CameraInputState>()
                    .capture_ready()
                && world.resource::<terra_network::client::CrankInput>().0 == Some(DRIVE_CRANK)
                && world
                    .resource::<ButtonInput<MouseButton>>()
                    .pressed(MouseButton::Right);
            if !powered || !driving {
                for probe in state.bypass_probes.values_mut() {
                    probe.pause();
                }
                return Ok(());
            }
            let gates = world.resource::<terra_render::InfusionRenderStatus>();
            let jet = world.resource::<terra_render::InfuserJetRenderStatus>();
            observe_powered_line(session, view, state)?;
            if view.gates && view.phase == terra_core::infusion::HoldPhase::Filling {
                state.gate_fill_since.get_or_insert_with(Instant::now);
                state.rendered_closed_seen |= gates.surfaces > 0 && gates.raised_gates >= 2;
            } else {
                state.gate_fill_since = None;
            }
            state.rendered_open_seen |= !view.gates
                && view.phase == terra_core::infusion::HoldPhase::Releasing
                && gates.surfaces > 0
                && gates.raised_gates == 0;
            if state.stage == 30
                && view.gates
                && view.phase == terra_core::infusion::HoldPhase::Filling
                && state
                    .gate_fill_since
                    .is_some_and(|since| since.elapsed().as_secs_f32() >= 0.18)
                && gates.raised_gates >= 2
                && jet.streams == 1
                && jet.vertices >= 12 * 16 * 4
                && jet.plumbing_sequence == session.plumbing_sequence
            {
                info!(
                    streams = jet.streams,
                    vertices = jet.vertices,
                    sequence = jet.plumbing_sequence,
                    "Native infuser draws its confirmed continuous nozzle-to-vessel water stream"
                );
                if !capture(world, state, "infuser-filling", 15)? {
                    return Ok(());
                }
                advance(state, 31);
            } else if state.stage == 31 {
                let mut expected = INFUSER_ITEMS;
                expected[3] = ItemKind::WaterBottle;
                expected[4] = ItemKind::Tool(ToolKind::WaterBucket);
                expected[5] = ItemKind::Tool(ToolKind::IronWateringCan)
                    .with_watering_units(ToolKind::IronWateringCan.watering_capacity())
                    .unwrap();
                let departed = expected.into_iter().enumerate().all(|(index, item)| {
                    session.drops.iter().any(|drop| {
                        drop.id == index as u64 + 1
                            && drop.stack.item == item
                            && drop.stack.count == 1
                            && drop.position[2] > 1.6
                    })
                });
                if !departed {
                    return Ok(());
                }
                if !state.queue_wait_seen
                    || !state.gates_closed_seen
                    || !state.gates_open_seen
                    || !state.bypass_overlap_seen
                    || INFUSER_ITEMS
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| !terra_core::infusion::fillable(**item))
                        .any(|(index, _)| {
                            state
                                .bypass_probes
                                .get(&(index as u64 + 1))
                                .is_none_or(|probe| probe.samples < 2)
                        })
                    || !state.platform_owner_seen
                    || !state.rendered_closed_seen
                    || !state.rendered_open_seen
                {
                    return Err(format!(
                        "Powered line lacked verified one-item gates/queue/continuous pass: wait={} closed={} open={} stone_samples={}",
                        state.queue_wait_seen,
                        state.gates_closed_seen,
                        state.gates_open_seen,
                        state.unsupported_samples
                    ));
                }
                if session
                    .kinetics
                    .iter()
                    .any(|part| part.position == INFUSER || part.position == HOLD)
                {
                    return Err(
                        "Passive filling station unexpectedly joined the kinetic network".into(),
                    );
                }
                if !capture(world, state, "infuser-filled", 16)? {
                    return Ok(());
                }
                world
                    .resource_mut::<terra_network::client::ClientChatCommands>()
                    .0
                    .push_back(terra_network::client::ChatCommand::Send(
                        "/tp -3.5 21.91 -4.5".into(),
                    ));
                advance(state, 32);
            }
        }
        32 if elapsed > 1.0
            && !session.teleporting
            && !world.resource::<terra_player::PlayerInputBlock>().0 =>
        {
            if !raw_pool_unchanged(world)? {
                return Ok(());
            }
            state.intake_started = Some(Instant::now());
            advance(state, 33);
        }
        33 => {
            if !raw_pool_unchanged(world)? {
                return Ok(());
            }
            let render = world.resource::<terra_render::GlassPipeRenderStatus>();
            let stored = GLASS_PIPES.into_iter().all(|position| {
                session
                    .plumbing
                    .iter()
                    .any(|node| node.position == position && node.amount > 0)
            });
            let closed = world
                .resource::<EditJournal>()
                .object_state(OUTLET_TAP)
                .is_some_and(|pose| plumbing::faucet_level(pose.facing) == 0);
            let active = session
                .kinetics
                .iter()
                .any(|part| part.position == INTAKE_PUMP && part.rpm > 0. && !part.overloaded);
            if elapsed < 1.
                || !closed
                || !active
                || !stored
                || render.filled < GLASS_PIPES.len()
                || render.units == 0
                || render.vertices == 0
            {
                return Ok(());
            }
            state.glass_pressure_verified = true;
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 0.5 21.91 3.5".into(),
                ));
            advance(state, 40);
        }
        40 => {
            if !state.outlet_open_pulsed
                || session.teleporting
                || world.resource::<terra_player::PlayerInputBlock>().0
                || !world
                    .resource::<EditJournal>()
                    .object_state(OUTLET_TAP)
                    .is_some_and(|pose| plumbing::faucet_level(pose.facing) == 1)
            {
                return Ok(());
            }
            // The normal right-click acknowledgement is visible before returning
            // to the crank; no fixture writes grant water or rotational power.
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp -3.5 21.91 -4.5".into(),
                ));
            advance(state, 41);
        }
        41 if elapsed > 0.7
            && !session.teleporting
            && !world.resource::<terra_player::PlayerInputBlock>().0 =>
        {
            advance(state, 42);
        }
        42 => {
            if !raw_pool_unchanged(world)? {
                return Ok(());
            }
            let continuous_glass = observe_continuous_glass_flow(world, state)?;
            let activity = session
                .plumbing
                .iter()
                .find(|node| node.position == INTAKE_PIPE)
                .and_then(|node| node.intake);
            let active_pump = session
                .kinetics
                .iter()
                .any(|node| node.position == INTAKE_PUMP && node.rpm > 0.0 && !node.overloaded);
            let receiving = session
                .plumbing
                .iter()
                .find(|node| node.position == PUMP_RECEIVER)
                .is_some_and(|node| node.amount > 0);
            let bubbles = world.resource::<terra_render::IntakeRenderStatus>();
            if state
                .intake_started
                .is_some_and(|since| since.elapsed().as_secs_f32() >= 6.)
                && active_pump
                && receiving
                && activity
                    .is_some_and(|activity| activity.units > 0 && activity.valid_for(INTAKE_PIPE))
                && bubbles.active_inlets > 0
                && bubbles.bubbles >= 4
                && state.glass_pressure_verified
                && state.glass_flow_seen
                && state.glass_seen.into_iter().all(|seen| seen)
                && state.glass_peak_units > 0
                && state.glass_peak_vertices > 0
                && continuous_glass
            {
                if !capture(world, state, "pump-natural-intake-bubbles", 17)? {
                    return Ok(());
                }
                advance(state, 34);
            }
        }
        34 if elapsed > 1.0 => {
            if !raw_pool_unchanged(world)? {
                return Ok(());
            }
            let pump = session
                .kinetics
                .iter()
                .find(|node| node.position == INTAKE_PUMP);
            if pump.is_none_or(|node| node.rpm != 0.0) {
                return Ok(());
            }
            let bubbles = world.resource::<terra_render::IntakeRenderStatus>();
            if bubbles.active_inlets != 0 || bubbles.bubbles != 0 {
                return Ok(());
            }
            let glass = world.resource::<terra_render::GlassPipeRenderStatus>();
            if glass.plumbing_sequence != session.plumbing_sequence
                || glass.flowing != 0
                || glass.transit_vertices != 0
            {
                return Ok(());
            }
            if !state.f29_verified {
                return Err("F29 source square never remained stable for two seconds".into());
            }
            if !capture(world, state, "pump-intake-stopped", 18)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(BUILD_SLOT as u8));
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/tp 15.5 18 7.5".into(),
                ));
            advance(state, 35);
        }
        35 => {
            if elapsed > 15.0 {
                return Err(
                    "Underwater construction did not reach its real submerged floor target".into(),
                );
            }
            if elapsed < 0.7
                || session.teleporting
                || world.resource::<terra_player::PlayerInputBlock>().0
                || world.resource::<terra_player::interaction::ActiveItem>().0
                    != Some(ItemKind::Block(plumbing::GLASS_PIPE))
            {
                return Ok(());
            }
            let Some(body) = session.confirmed_state() else {
                return Ok(());
            };
            let position = Vec3::from_array(body.position);
            if (position.x - 15.5).abs() > 0.3 || (position.z - 7.5).abs() > 0.3 {
                return Ok(());
            }
            let chunks = world.resource::<terra_world::streaming::ChunkStore>();
            let journal = world.resource::<EditJournal>();
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let config = &world.resource::<GameConfig>().world;
            if !terra_physics::character::in_water(config, terrain, chunks, journal, position)
                || !terra_physics::character::head_underwater(
                    config, terrain, chunks, journal, position,
                )
                || world
                    .resource::<terra_player::interaction::TargetBlock>()
                    .0
                    .is_none_or(|hit| hit.position != BUILD_FLOOR || hit.normal != [0, 1, 0])
            {
                return Ok(());
            }
            if session
                .inventory
                .count(ItemKind::Block(plumbing::GLASS_PIPE))
                != 1
                || session.inventory.slot(BUILD_SLOT).is_none_or(|stack| {
                    stack.item != ItemKind::Block(plumbing::GLASS_PIPE) || stack.count != 1
                })
                || chunks.block_at(BUILD_PIPE) != Some(27)
            {
                return Err(
                    "Underwater build lost its single real item or untouched water target".into(),
                );
            }
            let water = chunks.fluid_cell_at(terrain, journal, BUILD_PIPE, 27);
            if water.amount != 8 || !water.source {
                return Err(format!(
                    "Construction target is not the original full source: {water:?}"
                ));
            }
            let eye = position + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            let eye_cell = eye.to_array().map(|v| v.floor() as i64);
            if chunks.block_at(eye_cell) != Some(27) || eye_cell == BUILD_PIPE {
                return Err("Construction eye must occupy a separate original source cell".into());
            }
            state.underwater_eye_cell = Some(eye_cell);
            state.underwater_inventory_revision = session.inventory.revision;
            state.underwater_previous_request = session.last_edit.map(|(request, _)| request);
            advance(state, 36);
        }
        36 => {
            if elapsed > 12.0 {
                return Err(format!(
                    "Ordinary underwater placement was not confirmed: last_edit={:?} slot={:?}",
                    session.last_edit,
                    session.inventory.slot(BUILD_SLOT)
                ));
            }
            let chunks = world.resource::<terra_world::streaming::ChunkStore>();
            let journal = world.resource::<EditJournal>();
            let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
            let confirmed = session.last_edit.is_some_and(|(request, outcome)| {
                Some(request) != state.underwater_previous_request
                    && outcome == terra_network::protocol::session::EditOutcome::Applied
            });
            if !state.underwater_build_pulsed
                || !confirmed
                || chunks.block_at(BUILD_PIPE) != Some(plumbing::GLASS_PIPE)
                || session.inventory.revision <= state.underwater_inventory_revision
                || session
                    .inventory
                    .count(ItemKind::Block(plumbing::GLASS_PIPE))
                    != 0
                || session.inventory.slot(BUILD_SLOT).is_some()
            {
                return Ok(());
            }
            let eye_cell = state
                .underwater_eye_cell
                .ok_or("Underwater eye cell was not recorded")?;
            if chunks.block_at(eye_cell) != Some(27) {
                return Err("Underwater placement replaced the player's own source cell".into());
            }
            let water = chunks.fluid_cell_at(terrain, journal, BUILD_PIPE, plumbing::GLASS_PIPE);
            let eye_water = chunks.fluid_cell_at(terrain, journal, eye_cell, 27);
            if water.amount != 8 || !water.source || eye_water.amount != 8 || !eye_water.source {
                return Err(format!(
                    "Glass waterlogging lost its surrounding source: pipe={water:?} eye={eye_water:?}"
                ));
            }
            let probe = [16.79, 17.75, 8.5];
            if chunks
                .water_column_at_point(terrain, journal, probe)
                .is_none_or(|column| (column.level - 20.0).abs() > 1e-6)
            {
                return Err(
                    "New waterlogged glass pipe lost the real deep-pool water column".into(),
                );
            }
            let since = state
                .underwater_source_since
                .get_or_insert_with(Instant::now);
            if since.elapsed().as_secs_f32() < 0.5 {
                return Ok(());
            }
            if !capture(world, state, "underwater-glass-source-preserved", 19)? {
                return Ok(());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(VALVE_SLOT as u8));
            teleport(world, "/tp 21.5 22.91 3.5");
            advance(state, 44);
        }
        44 | 46 => {
            if elapsed > 12.0 {
                return Err(format!(
                    "Valve placement never reached its real support: stage={} target={:?}",
                    state.stage,
                    world.resource::<terra_player::interaction::TargetBlock>().0
                ));
            }
            let vertical = state.stage == 44;
            let (position, support, normal, count) = if vertical {
                (TEST_VERTICAL_VALVE, TEST_GLASS[0], [0, 1, 0], 2)
            } else {
                (TEST_HORIZONTAL_VALVE, TEST_GLASS[1], [1, 0, 0], 1)
            };
            if elapsed < 0.7
                || session.teleporting
                || world.resource::<terra_player::PlayerInputBlock>().0
                || world.resource::<terra_player::interaction::ActiveItem>().0
                    != Some(ItemKind::Block(plumbing::VALVE))
                || world
                    .resource::<terra_player::interaction::TargetBlock>()
                    .0
                    .is_none_or(|hit| hit.position != support || hit.normal != normal)
            {
                return Ok(());
            }
            if session.inventory.count(ItemKind::Block(plumbing::VALVE)) != count {
                return Err("Native valve placement lost its exact real item count".into());
            }
            state.valve_previous_request = session.last_edit.map(|(id, _)| id);
            state.valve_inventory_revision = session.inventory.revision;
            world.write_message(terra_world::streaming::BlockEditRequest {
                position,
                expected_material: 0,
                material: plumbing::VALVE,
            });
            advance(state, if vertical { 45 } else { 47 });
        }
        45 | 47 => {
            if elapsed > 12.0 {
                return Err(format!(
                    "Authoritative valve placement was not accepted: {:?}",
                    session.last_edit
                ));
            }
            let vertical = state.stage == 45;
            let position = if vertical {
                TEST_VERTICAL_VALVE
            } else {
                TEST_HORIZONTAL_VALVE
            };
            let index = usize::from(!vertical);
            let expected_count = if vertical { 1 } else { 0 };
            let applied = session.last_edit.is_some_and(|(id, result)| {
                Some(id) != state.valve_previous_request
                    && result == terra_network::protocol::session::EditOutcome::Applied
            });
            if !applied
                || session.inventory.revision <= state.valve_inventory_revision
                || session.inventory.count(ItemKind::Block(plumbing::VALVE)) != expected_count
                || world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .block_at(position)
                    != Some(plumbing::VALVE)
            {
                return Ok(());
            }
            verify_test_valve(world, position, if vertical { 12 } else { 3 })?;
            state.valve_placements[index] = true;
            if !capture(
                world,
                state,
                if vertical {
                    "valve-vertical-authoritative"
                } else {
                    "valve-horizontal-authoritative"
                },
                if vertical { 20 } else { 21 },
            )? {
                return Ok(());
            }
            if vertical {
                teleport(world, "/tp 23.5 19.91 3.5");
                advance(state, 46);
            } else {
                world.write_message(terra_ui::InventoryAction::Hotbar(6));
                teleport(world, "/tp 24.5 21.91 3.5");
                advance(state, 48);
            }
        }
        48 => {
            if elapsed > 12.0 {
                return Err("The finite valve fixture never stored full glass-pipe water".into());
            }
            if elapsed < 0.7
                || session.teleporting
                || world.resource::<terra_player::PlayerInputBlock>().0
            {
                return Ok(());
            }
            let full = TEST_GLASS
                .into_iter()
                .all(|p| plumbing_amount(session, p) == Some(8));
            if !full
                || basin_amount(session, TEST_RECEIVER) != Some(0)
                || world
                    .resource::<terra_player::interaction::TargetBlock>()
                    .0
                    .is_none_or(|hit| hit.position != TEST_VERTICAL_VALVE)
            {
                return Ok(());
            }
            let render = world.resource::<terra_render::GlassPipeRenderStatus>();
            if render.plumbing_sequence != session.plumbing_sequence {
                return Ok(());
            }
            if !state.valve_placements.into_iter().all(|ok| ok)
                || TEST_GLASS.into_iter().any(|position| {
                    render
                        .nodes
                        .iter()
                        .find(|node| node.position == position)
                        .is_none_or(|node| node.amount != 8 || node.vertices == 0)
                })
            {
                return Err(
                    "Stored valve water lacked verified placements or real liquid geometry".into(),
                );
            }
            if !capture(world, state, "glass-valves-stored-water", 22)? {
                return Ok(());
            }
            toggle(world, TEST_VERTICAL_VALVE, plumbing::VALVE);
            advance(state, 49);
        }
        49 => {
            let closed = world
                .resource::<EditJournal>()
                .object_state(TEST_VERTICAL_VALVE)
                .is_some_and(|pose| !plumbing::open(plumbing::VALVE, pose.facing));
            if elapsed < 0.4 || !closed {
                return Ok(());
            }
            verify_test_valve(world, TEST_VERTICAL_VALVE, 12)?;
            let Some(source) = basin_amount(session, TEST_SOURCE) else {
                return Ok(());
            };
            let Some(valve) = plumbing_amount(session, TEST_VERTICAL_VALVE) else {
                return Ok(());
            };
            let expected = TEST_SOURCE_UNITS
                .checked_sub(source + valve)
                .ok_or("Valve fixture invented stored water")?;
            if expected < 24 || basin_amount(session, TEST_RECEIVER) != Some(0) {
                return Err(
                    "Valve shutoff did not retain a real finite downstream reservoir".into(),
                );
            }
            state.valve_source_remaining = Some(source);
            state.valve_closed_units = valve;
            state.valve_drain_expected = expected;
            advance(state, 50);
        }
        50 => {
            if !state.valve_outlet_pulsed
                && elapsed > 0.35
                && world
                    .resource::<terra_player::interaction::TargetBlock>()
                    .0
                    .is_some_and(|hit| hit.position == TEST_DRAIN_TAP)
            {
                state.valve_outlet_pulsed = true;
                toggle(world, TEST_DRAIN_TAP, plumbing::FAUCET);
                return Ok(());
            }
            if state.valve_outlet_pulsed
                && world
                    .resource::<EditJournal>()
                    .object_state(TEST_DRAIN_TAP)
                    .is_some_and(|pose| plumbing::faucet_level(pose.facing) == 1)
            {
                advance(state, 51);
            }
        }
        51 => {
            if elapsed > 15.0 {
                return Err(format!(
                    "Closed-valve glass did not drain its finite stored amount: receiver={:?} expected={} glass={:?}",
                    basin_amount(session, TEST_RECEIVER),
                    state.valve_drain_expected,
                    TEST_GLASS.map(|p| plumbing_amount(session, p))
                ));
            }
            let Some(receiver) = basin_amount(session, TEST_RECEIVER) else {
                return Ok(());
            };
            if basin_amount(session, TEST_SOURCE) != state.valve_source_remaining
                || plumbing_amount(session, TEST_VERTICAL_VALVE) != Some(state.valve_closed_units)
                || receiver > state.valve_drain_expected
                || world
                    .resource::<EditJournal>()
                    .object_state(TEST_VERTICAL_VALVE)
                    .is_none_or(|pose| plumbing::open(plumbing::VALVE, pose.facing))
            {
                return Err(
                    "Closed valve supplied extra water or changed its isolated source".into(),
                );
            }
            let remaining: u16 = TEST_GLASS
                .into_iter()
                .filter_map(|p| plumbing_amount(session, p))
                .sum();
            let horizontal_remaining = plumbing_amount(session, TEST_HORIZONTAL_VALVE);
            let tap_remaining = plumbing_amount(session, TEST_DRAIN_TAP);
            if receiver > 0 && remaining > 0 && remaining < 24 {
                state.valve_partial_seen = true;
                if !capture(world, state, "glass-valves-running-out", 23)? {
                    return Ok(());
                }
            }
            if receiver != state.valve_drain_expected
                || remaining != 0
                || horizontal_remaining != Some(0)
                || tap_remaining != Some(0)
            {
                return Ok(());
            }
            if !state.valve_partial_seen || state.captured < 23 {
                return Err(
                    "Finite valve drain skipped its partially filled glass observation".into(),
                );
            }
            advance(state, 52);
        }
        52 if elapsed > 0.4 => {
            if !capture(world, state, "glass-valves-empty-after-shutoff", 24)? {
                return Ok(());
            }
            surfaces::begin(world, &mut state.surfaces)?;
            advance(state, 53);
        }
        _ => {}
    }
    Ok(())
}

fn finish(world: &mut World, state: &mut Inspection) {
    info!(
        seconds = state.start.elapsed().as_secs_f32(),
        captures = state.captured,
        bypass_samples = state.unsupported_samples,
        bypass_items = state.bypass_probes.len(),
        bypass_overlap = state.bypass_overlap_seen,
        f29_stable = state.f29_verified,
        f29_sources = 4,
        f29_units = 32,
        queue_items = INFUSER_ITEMS.len(),
        initial_spacing = 0.26,
        glass_pipes = GLASS_PIPES.len(),
        glass_initially_empty = state.glass_empty_verified,
        glass_filled_seen = ?state.glass_seen,
        glass_peak_units = state.glass_peak_units,
        glass_peak_vertices = state.glass_peak_vertices,
        glass_stored_pressure = state.glass_pressure_verified,
        glass_real_flow = state.glass_flow_seen,
        glass_continuous_samples = state.glass_continuous_flow.as_ref().map(|probe| probe.samples),
        glass_continuous_seconds = state.glass_continuous_flow.as_ref().map(GlassFlowProbe::seconds),
        glass_continuous_transfer_samples = ?state.glass_continuous_flow.as_ref().map(|probe| probe.transfer_samples),
        underwater_build = true,
        external_source_preserved = true,
        valve_placements = ?state.valve_placements,
        closed_valve_runout_units = state.valve_drain_expected,
        partial_glass_runout = state.valve_partial_seen,
        wall_torch_placements = ?state.surfaces.torches,
        pipe_material_galleries = ?state.surfaces.galleries,
        waterlogged_surface_views = state.surfaces.water_samples,
        "Hydraulics native passed: basin swimming, faucet audio, source clicks, actual powered single-item filling/gates, dense bypass passthrough, replenishing pool intake bubbles, real empty/filled glass pipes, stable F29 four-source square, ordinary underwater building, source-preserving glass waterlogging, authoritative horizontal/vertical valve placement and finite glass runout after shutoff, four ordinary wall-torch placements, all pipe-material bends and waterlogged round-part surfaces"
    );
    state.stage = 255;
    world.write_message(AppExit::Success);
}

fn teleport(world: &mut World, command: &str) {
    world
        .resource_mut::<terra_network::client::ClientChatCommands>()
        .0
        .push_back(terra_network::client::ChatCommand::Send(command.into()));
}

fn inventory_water_units(inventory: &terra_core::Inventory) -> u16 {
    inventory
        .slots
        .iter()
        .map(|stack| {
            let units = match stack.item {
                ItemKind::WaterBottle => 1,
                ItemKind::Tool(ToolKind::WaterBucket) => u16::from(plumbing::CELL_CAPACITY),
                ItemKind::FilledWateringCan { units, .. } => u16::from(units),
                _ => 0,
            };
            units * stack.count
        })
        .sum()
}

fn main_source_mass(session: &ClientSession) -> Option<u16> {
    let mut amount = basin_amount(session, MAIN_SOURCE)?;
    // Include the closed valve's own storage and the other, still closed,
    // infuser branch. Redistribution on this isolated side is not consumption.
    for cell in [[0, 18, 0], [1, 18, 0], VALVE]
        .into_iter()
        .chain((1..=7).map(|x| [x, 20, 0]))
    {
        amount += plumbing_amount(session, cell)?;
    }
    Some(amount)
}

fn plumbing_amount(session: &ClientSession, position: [i64; 3]) -> Option<u16> {
    session
        .plumbing
        .iter()
        .find(|node| node.position == position)
        .map(|node| node.amount)
}

fn basin_amount(session: &ClientSession, shape: plumbing::BasinShape) -> Option<u16> {
    session
        .plumbing
        .iter()
        .find(|node| node.position == shape.origin && node.basin == Some(shape))
        .map(|node| node.amount)
}

fn verify_test_valve(world: &World, position: [i64; 3], ports: u8) -> Result<(), String> {
    let journal = world.resource::<EditJournal>();
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let pose = journal
        .object_state(position)
        .ok_or("Placed valve has no confirmed orientation")?;
    if plumbing::ports(plumbing::VALVE, pose.facing) != ports {
        return Err(format!(
            "Placed valve has wrong real ports: position={position:?} facing={} expected_ports={ports}",
            pose.facing
        ));
    }
    for (side, direction) in plumbing::DIRECTIONS.into_iter().enumerate() {
        if ports & (1 << side) == 0 {
            continue;
        }
        let adjacent = std::array::from_fn(|i| position[i] + direction[i]);
        let material = chunks
            .block_at(adjacent)
            .ok_or("Placed valve neighbor is not loaded")?;
        let facing = journal.object_state(adjacent).unwrap_or_default().facing;
        if !plumbing::joined(plumbing::VALVE, pose.facing, material, facing, direction) {
            return Err(format!(
                "Placed valve does not join its actual neighbor at {adjacent:?}"
            ));
        }
    }
    Ok(())
}

fn intake_diagnostic(world: &World, state: &Inspection) -> String {
    let session = world.resource::<ClientSession>();
    format!(
        "pump={:?}; drive={:?}; intake={:?}; receiver={:?}; outlet={:?}; glass_nodes={:?}; glass_render={:?}; empty_verified={}; seen={:?}; peak_units={}; peak_vertices={}; stored_pressure={}; flow_seen={}; opening_pulsed={}; input_blocked={}; target={:?}",
        session
            .plumbing
            .iter()
            .find(|node| node.position == INTAKE_PUMP),
        session
            .kinetics
            .iter()
            .find(|node| node.position == INTAKE_PUMP),
        session
            .plumbing
            .iter()
            .find(|node| node.position == INTAKE_PIPE),
        session
            .plumbing
            .iter()
            .find(|node| node.position == PUMP_RECEIVER),
        session
            .plumbing
            .iter()
            .find(|node| node.position == OUTLET_TAP),
        GLASS_PIPES.map(|p| session.plumbing.iter().find(|node| node.position == p)),
        world.resource::<terra_render::GlassPipeRenderStatus>(),
        state.glass_empty_verified,
        state.glass_seen,
        state.glass_peak_units,
        state.glass_peak_vertices,
        state.glass_pressure_verified,
        state.glass_flow_seen,
        state.outlet_open_pulsed,
        world.resource::<terra_player::PlayerInputBlock>().0,
        world.resource::<terra_player::interaction::TargetBlock>().0,
    )
}

/// Require continuously visible water at each real pipe, across distinct authority payloads.
fn observe_continuous_glass_flow(world: &World, state: &mut Inspection) -> Result<bool, String> {
    let session = world.resource::<ClientSession>();
    let rendered = world.resource::<terra_render::GlassPipeRenderStatus>();
    // The geometry refresh is independent of network receipt. Wait until both
    // describe the same payload before judging a missing pipe or flow flag.
    if rendered.plumbing_sequence != session.plumbing_sequence {
        return Ok(false);
    }
    let mut amount = [0; GLASS_PIPES.len()];
    let mut flowing = [false; GLASS_PIPES.len()];
    let mut vertices = [0; GLASS_PIPES.len()];
    for (index, position) in GLASS_PIPES.into_iter().enumerate() {
        let node = session
            .plumbing
            .iter()
            .find(|node| node.position == position);
        let water = rendered.nodes.iter().find(|node| node.position == position);
        let (Some(node), Some(water)) = (node, water) else {
            if state.glass_continuous_flow.is_none() {
                return Ok(false);
            }
            return Err(format!(
                "Continuous glass pipe disappeared from an authoritative/rendered sample: position={position:?} sequence={} node={node:?} water={water:?}",
                session.plumbing_sequence
            ));
        };
        if water.amount != node.amount || water.flowing != node.flowing {
            return Err(format!(
                "Glass geometry does not match its plumbing payload: position={position:?} sequence={} node={node:?} water={water:?}",
                session.plumbing_sequence
            ));
        }
        amount[index] = node.amount;
        flowing[index] = node.flowing;
        vertices[index] = water.vertices;
    }
    let Some(receiver) = plumbing_amount(session, PUMP_RECEIVER) else {
        return Ok(false);
    };
    let sample = GlassFlowSample {
        sequence: session.plumbing_sequence,
        amount,
        flowing,
        vertices,
        active_pump: session
            .kinetics
            .iter()
            .any(|node| node.position == INTAKE_PUMP && node.rpm > 0.0 && !node.overloaded),
        receiver,
    };
    let Some(probe) = state.glass_continuous_flow.as_mut() else {
        // Allow the ordinary crank and open outlet to establish the first
        // through-flow sample, then retain every later interruption as a failure.
        if sample.continuous() && sample.flowing.into_iter().any(|flowing| flowing) {
            state.glass_continuous_flow = Some(GlassFlowProbe::new(sample, Instant::now()));
        }
        return Ok(false);
    };
    let was_complete = probe.complete();
    let complete = probe.observe(sample, Instant::now())?;
    if complete && !was_complete {
        info!(
            first_sequence = probe.first_sequence,
            last_sequence = probe.last_sequence,
            samples = probe.samples,
            seconds = probe.seconds(),
            rendered_frames = probe.render_frames,
            minimum_pipe_vertices = ?probe.minimum_vertices,
            transfer_samples = ?probe.transfer_samples,
            zero_storage_transit_seen = ?probe.transit_seen,
            initial_receiver = probe.initial_receiver,
            receiver = probe.receiver,
            "All native glass pipes retained visible confirmed water through continuous powered transfer"
        );
    }
    Ok(complete)
}

/// New glass receives only real replicated network contents, never ambient water.
fn observe_glass_pipes(world: &World, state: &mut Inspection) -> Result<(), String> {
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let session = world.resource::<ClientSession>();
    let mut all_empty = true;
    for (index, position) in GLASS_PIPES.into_iter().enumerate() {
        let Some(raw) = chunks.block_at(position) else {
            return Ok(());
        };
        if journal.block(position).unwrap_or(raw) != plumbing::GLASS_PIPE {
            return Err(format!(
                "Native glass outlet lost its real model at {position:?}"
            ));
        }
        let Some(node) = session
            .plumbing
            .iter()
            .find(|node| node.position == position)
        else {
            return Ok(());
        };
        if node.amount > 0 {
            all_empty = false;
            if state.stage < 33 {
                return Err(format!(
                    "Unpowered glass pipe invented water at {position:?}: {node:?}"
                ));
            }
            state.glass_seen[index] = true;
        }
    }
    let render = world.resource::<terra_render::GlassPipeRenderStatus>();
    if state.stage < 33 {
        if render.filled != 0
            || render.units != 0
            || render.vertices != 0
            || render.flowing != 0
            || render.transit_vertices != 0
        {
            return Err(format!(
                "Empty native glass received fabricated visible liquid: {render:?}"
            ));
        }
        // The gallery is outside the initial render interest. Require each
        // original pipe explicitly, plus precisely the optional gallery cells
        // that are currently loaded and present in the accepted network view.
        let required: Vec<_> = GLASS_PIPES.into_iter().chain(TEST_GLASS).collect();
        for &position in &required {
            if chunks.block_at(position) != Some(plumbing::GLASS_PIPE) {
                return Ok(());
            }
            let Some(node) = session
                .plumbing
                .iter()
                .find(|node| node.position == position)
            else {
                return Ok(());
            };
            if node.amount != 0 || node.flowing {
                return Err(format!("Original unpowered glass is not empty: {node:?}"));
            }
        }
        if render.plumbing_sequence != session.plumbing_sequence {
            return Ok(());
        }
        let optional = surfaces::initial_glass_cells();
        let loaded: Vec<_> = optional
            .into_iter()
            .filter(|&position| {
                chunks.block_at(position) == Some(plumbing::GLASS_PIPE)
                    && session
                        .plumbing
                        .iter()
                        .any(|node| node.position == position)
            })
            .collect();
        state.glass_empty_verified |=
            all_empty && initial_glass_render_matches(render, &required, &optional, &loaded)?;
    } else {
        state.glass_flow_seen |= render.flowing > 0
            && session
                .plumbing
                .iter()
                .any(|node| GLASS_PIPES.contains(&node.position) && node.flowing);
        if !all_empty && render.filled > 0 && render.units > 0 && render.vertices > 0 {
            state.glass_peak_units = state.glass_peak_units.max(render.units);
            state.glass_peak_vertices = state.glass_peak_vertices.max(render.vertices);
        }
    }
    Ok(())
}

/// Counts cannot let an extra gallery pipe substitute for a missing original.
fn initial_glass_render_matches(
    render: &terra_render::GlassPipeRenderStatus,
    required: &[[i64; 3]],
    optional: &[[i64; 3]],
    loaded: &[[i64; 3]],
) -> Result<bool, String> {
    let mut actual = std::collections::BTreeSet::new();
    for node in &render.nodes {
        if (!required.contains(&node.position) && !optional.contains(&node.position))
            || !actual.insert(node.position)
            || node.amount != 0
            || node.flowing
            || node.vertices != 0
        {
            return Err(format!(
                "Unexpected or nonempty initial glass render node: {node:?}"
            ));
        }
    }
    if render.pipes != actual.len() {
        return Err("Initial glass render count disagrees with its concrete nodes".into());
    }
    let expected: std::collections::BTreeSet<_> = required.iter().chain(loaded).copied().collect();
    Ok(actual == expected)
}

#[cfg(test)]
mod initial_glass_render_tests {
    use super::*;
    use terra_render::{GlassPipeNodeRenderStatus, GlassPipeRenderStatus};
    const REQUIRED: [[i64; 3]; 6] = [
        [0, 0, 0],
        [1, 0, 0],
        [2, 0, 0],
        [3, 0, 0],
        [4, 0, 0],
        [5, 0, 0],
    ];
    const OPTIONAL: [[i64; 3]; 4] = [[30, 0, 0], [31, 0, 0], [32, 0, 0], [33, 0, 0]];
    fn render(cells: impl IntoIterator<Item = [i64; 3]>) -> GlassPipeRenderStatus {
        let nodes: Vec<_> = cells
            .into_iter()
            .map(|position| GlassPipeNodeRenderStatus {
                position,
                amount: 0,
                flowing: false,
                vertices: 0,
            })
            .collect();
        GlassPipeRenderStatus {
            pipes: nodes.len(),
            nodes,
            ..Default::default()
        }
    }
    #[test]
    fn exact_original_nodes_pass_before_remote_gallery_is_loaded() {
        assert_eq!(
            initial_glass_render_matches(&render(REQUIRED), &REQUIRED, &OPTIONAL, &[]),
            Ok(true)
        );
        assert_eq!(
            initial_glass_render_matches(&render(REQUIRED), &REQUIRED, &OPTIONAL, &OPTIONAL[..1]),
            Ok(false)
        );
        let visible = render(REQUIRED.into_iter().chain(OPTIONAL[..1].iter().copied()));
        assert_eq!(
            initial_glass_render_matches(&visible, &REQUIRED, &OPTIONAL, &OPTIONAL[..1]),
            Ok(true)
        );
        let all = render(REQUIRED.into_iter().chain(OPTIONAL));
        assert_eq!(
            initial_glass_render_matches(&all, &REQUIRED, &OPTIONAL, &OPTIONAL),
            Ok(true)
        );
    }
    #[test]
    fn same_count_cannot_replace_missing_original_with_optional_gallery_node() {
        let substituted = render(REQUIRED[..5].iter().copied().chain([OPTIONAL[0]]));
        assert_eq!(substituted.pipes, REQUIRED.len());
        assert_eq!(
            initial_glass_render_matches(&substituted, &REQUIRED, &OPTIONAL, &OPTIONAL[..1]),
            Ok(false)
        );
    }
    #[test]
    fn nonempty_duplicate_or_unknown_render_nodes_fail() {
        for corruption in 0..4 {
            let mut invalid = render(REQUIRED);
            match corruption {
                0 => invalid.nodes[0].flowing = true,
                1 => invalid.nodes[0].amount = 1,
                2 => invalid.nodes[0].position = REQUIRED[1],
                _ => invalid.nodes[0].position = [99, 0, 0],
            }
            assert!(initial_glass_render_matches(&invalid, &REQUIRED, &OPTIONAL, &[]).is_err());
        }
    }
}

/// Observe source formation and then demand a genuinely quiet isolated section.
fn observe_f29_pool(world: &World, state: &mut Inspection) -> Result<(), String> {
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
    for dx in 0..2 {
        for dz in 0..2 {
            let p = [F29_ORIGIN[0] + dx, F29_ORIGIN[1], F29_ORIGIN[2] + dz];
            let Some(material) = chunks.block_at(p) else {
                return Ok(());
            };
            let water = chunks.fluid_cell_at(terrain, journal, p, material);
            if water.amount != 8 || !water.source || water.flow != [0; 2] {
                if state.f29_verified {
                    return Err(format!(
                        "F29 square lost its settled source at {p:?}: {water:?}"
                    ));
                }
                state.f29_epochs = None;
                return Ok(());
            }
        }
    }
    let owner = terra_voxel::global_to_section(F29_ORIGIN).0;
    let fluid = journal.fluid_epoch(owner);
    let mesh = journal.water_mesh_epoch(owner);
    if let Some((old_fluid, old_mesh, since)) = state.f29_epochs {
        if fluid != old_fluid || mesh != old_mesh {
            if state.f29_verified {
                return Err("Settled F29 square kept changing fluid or water-mesh epochs".into());
            }
            state.f29_epochs = Some((fluid, mesh, Instant::now()));
        } else if since.elapsed().as_secs_f32() >= 2.0 {
            state.f29_verified = true;
        }
    } else {
        state.f29_epochs = Some((fluid, mesh, Instant::now()));
    }
    Ok(())
}

/// All 49 untouched raw source blocks remain full while the fiftieth pool cell
/// contains the submerged pipe. Probe the actual loaded source column as well.
fn raw_pool_unchanged(world: &World) -> Result<bool, String> {
    let chunks = world.resource::<terra_world::streaming::ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let terrain = &world.resource::<terra_world::streaming::WorldTerrain>().0;
    let mut units = 0_u16;
    for x in -10..=-6 {
        for z in -3..=1 {
            for y in 17..=18 {
                let position = [x, y, z];
                if position == INTAKE_PIPE {
                    continue;
                }
                let Some(material) = chunks.block_at(position) else {
                    return Ok(false);
                };
                let water = chunks.fluid_cell_at(terrain, journal, position, material);
                if material != 27 || water.amount != 8 || !water.source {
                    return Err(format!(
                        "Replenishing raw pool was lowered by intake at {position:?}: material={material} water={water:?}"
                    ));
                }
                units += u16::from(water.amount);
            }
        }
    }
    if units != 49 * 8 {
        return Err("Native pool did not retain all raw source units".into());
    }
    let Some(material) = chunks.block_at(INTAKE_PIPE) else {
        return Ok(false);
    };
    let ambient = chunks.fluid_cell_at(terrain, journal, INTAKE_PIPE, material);
    if material != plumbing::IRON_PIPE || ambient.amount != 8 || ambient.source {
        return Err(format!(
            "Placed intake pipe changed its finite ambient water: material={material} water={ambient:?}"
        ));
    }
    // Sample the actual liquid outside the solid pipe profile on every side.
    // A source flag here would hide the finite waterlogging depression regression.
    for [dx, dz] in [[-0.29, 0.0], [0.29, 0.0], [0.0, -0.29], [0.0, 0.29]] {
        let point = [-5.5 + dx, 18.5, -0.5 + dz];
        if chunks
            .water_column_at_point(terrain, journal, point)
            .is_none_or(|column| (column.level - 19.0).abs() > 0.000001)
        {
            return Err(format!(
                "Finite waterlogging lowered or removed the surface around the intake pipe at {point:?}"
            ));
        }
    }
    if chunks
        .water_column_at_point(terrain, journal, [-7.5, 18.5, -0.5])
        .is_none_or(|column| (column.level - 19.0).abs() > 0.000001)
    {
        return Err("Replenishing pool surface moved from its original level".into());
    }
    Ok(true)
}

/// Observe only authoritative snapshots; no fixture writes drive the machine,
/// move an item or grant rotational power after the saved server has started.
fn observe_powered_line(
    session: &ClientSession,
    view: &terra_core::infusion::HoldVisual,
    state: &mut Inspection,
) -> Result<(), String> {
    let stamp = state
        .drop_stamp
        .ok_or("Powered line has no confirmed drop payload clock")?;
    let on_deck: Vec<_> = session
        .drops
        .iter()
        .filter(|drop| {
            (drop.position[0] - 10.5).abs() < 0.43
                && (drop.position[1] - (17.0 + terra_core::infusion::DROP_HEIGHT)).abs() < 0.08
                && (0.0..1.0).contains(&drop.position[2])
        })
        .collect();
    // Exactly the nine seeded stacks must survive as nine single drops. The
    // mining regression creates a separate drop away from this conveyor lane.
    for (index, _) in INFUSER_ITEMS.iter().enumerate() {
        let entries: Vec<_> = session
            .drops
            .iter()
            .filter(|drop| drop.id == index as u64 + 1)
            .collect();
        if entries.len() != 1 || entries[0].stack.count != 1 {
            return Err("The dense filling queue lost or duplicated an item".into());
        }
    }
    let lane_quantity: u16 = session
        .drops
        .iter()
        .filter(|drop| {
            (drop.position[0] - 10.5).abs() < 0.6 && (-4.0..32.0).contains(&drop.position[2])
        })
        .map(|drop| drop.stack.count)
        .sum();
    if lane_quantity != INFUSER_ITEMS.len() as u16 {
        return Err(format!(
            "Native conveyor quantity changed from9 to{lane_quantity}"
        ));
    }
    let fill_owner = view.active_drop.filter(|id| {
        id.checked_sub(1)
            .and_then(|index| INFUSER_ITEMS.get(index as usize))
            .is_some_and(|item| terra_core::infusion::fillable(*item))
    });
    let locked = view.gates
        && fill_owner.is_some()
        && matches!(
            view.phase,
            terra_core::infusion::HoldPhase::Filling
                | terra_core::infusion::HoldPhase::WaitingForFluid
        );
    if let Some(id) = fill_owner.filter(|_| locked) {
        if state.slot_owner_since.is_none_or(|(old, _)| old != id) {
            state.slot_owner_since = Some((id, Instant::now()));
        }
        // Gate and drop messages have independent send cadences. A stable lock
        // must own the sole item; transient Approaching/Releasing may drain a
        // dense group of bypass items at their inherited conveyor speed.
        if state
            .slot_owner_since
            .is_some_and(|(_, since)| since.elapsed().as_secs_f32() >= 0.18)
        {
            if on_deck.len() != 1 || on_deck[0].id != id {
                return Err(format!(
                    "Stable filling slot contains other items: owner={id} deck={:?}",
                    on_deck.iter().map(|drop| drop.id).collect::<Vec<_>>()
                ));
            }
            state.platform_owner_seen = true;
        }
    } else {
        state.slot_owner_since = None;
    }
    state.bypass_overlap_seen |= on_deck
        .iter()
        .filter(|drop| bypass_item(drop.id).is_some())
        .count()
        >= 2;
    if view.gates
        && matches!(
            view.phase,
            terra_core::infusion::HoldPhase::Filling
                | terra_core::infusion::HoldPhase::WaitingForFluid
        )
    {
        state.gates_closed_seen = true;
        state.queue_wait_seen |= session.drops.iter().any(|drop| {
            drop.id <= INFUSER_ITEMS.len() as u64
                && view.active_drop != Some(drop.id)
                && (-0.65..0.0).contains(&drop.position[2])
                && drop.velocity[2].abs() < 0.05
        });
    }
    state.gates_open_seen |= !view.gates
        && view.phase == terra_core::infusion::HoldPhase::Releasing
        && view.active_drop.is_some_and(|id| {
            session
                .drops
                .iter()
                .any(|drop| drop.id == id && drop.position[2] > 0.8)
        });
    for (index, initial) in INFUSER_ITEMS
        .iter()
        .enumerate()
        .filter(|(_, item)| !terra_core::infusion::fillable(**item))
    {
        let id = index as u64 + 1;
        let drop = session
            .drops
            .iter()
            .find(|drop| drop.id == id)
            .ok_or("A bypass item was lost")?;
        if drop.stack.item != *initial || drop.stack.count != 1 {
            return Err(format!("Bypass item{id} was converted or duplicated"));
        }
        if !(0.12..0.88).contains(&drop.position[2]) {
            continue;
        }
        if view.active_drop == Some(id)
            && (view.speed <= 0.05
                || view.gates
                || matches!(
                    view.phase,
                    terra_core::infusion::HoldPhase::Filling
                        | terra_core::infusion::HoldPhase::WaitingForFluid
                ))
        {
            return Err(format!(
                "Bypass owner stopped or closed filling gates: item={drop:?} station={view:?}"
            ));
        }
        if let Some(probe) = state.bypass_probes.get_mut(&id) {
            let previous = probe.position;
            match probe.observe(drop.position[2], stamp) {
                Ok(true) => {
                    state.unsupported_samples = state.unsupported_samples.saturating_add(1);
                }
                Ok(false) => {}
                Err(reason) => {
                    return Err(format!(
                        "Bypass item {id} {reason}: previous_z={previous}, stationary_sim_seconds={}, probe={probe:?}, drop={drop:?}, station={view:?}, payload_wall_age={}",
                        probe.stationary_seconds,
                        state.drop_payload_received.elapsed().as_secs_f32(),
                    ));
                }
            }
        } else {
            state
                .bypass_probes
                .insert(id, BypassProbe::new(drop.position[2], stamp));
            state.unsupported_samples = state.unsupported_samples.saturating_add(1);
        }
    }
    Ok(())
}

#[cfg(test)]
mod input_tests {
    use super::*;

    fn glass_flow_sample(sequence: u64) -> GlassFlowSample {
        GlassFlowSample {
            sequence,
            amount: [8; GLASS_PIPES.len()],
            flowing: [true; GLASS_PIPES.len()],
            vertices: [96; GLASS_PIPES.len()],
            active_pump: true,
            receiver: sequence as u16,
        }
    }

    #[test]
    fn glass_flow_requires_distinct_snapshots_spanning_the_full_observation_window() {
        let now = Instant::now();
        let mut probe = GlassFlowProbe::new(glass_flow_sample(1), now);
        for _ in 0..1_000 {
            assert_eq!(
                probe.observe(
                    glass_flow_sample(1),
                    now + std::time::Duration::from_secs(30)
                ),
                Ok(false)
            );
        }
        assert_eq!(probe.samples, 1);
        assert_eq!(probe.seconds(), 0.0);
        for sequence in 2..=u64::from(GLASS_FLOW_SAMPLES) {
            assert_eq!(
                probe.observe(
                    glass_flow_sample(sequence),
                    now + std::time::Duration::from_millis(sequence)
                ),
                Ok(false)
            );
        }
        assert_eq!(probe.samples, GLASS_FLOW_SAMPLES);
        assert_eq!(
            probe.observe(
                glass_flow_sample(11),
                now + std::time::Duration::from_secs(3)
            ),
            Ok(true)
        );
    }

    #[test]
    fn glass_flow_detects_one_pipe_disappearing_even_when_other_pipes_remain_visible() {
        let now = Instant::now();
        let mut probe = GlassFlowProbe::new(glass_flow_sample(1), now);
        let transit = GlassFlowSample {
            amount: [0; GLASS_PIPES.len()],
            ..glass_flow_sample(2)
        };
        assert_eq!(probe.observe(transit, now), Ok(false));
        assert!(probe.transit_seen.into_iter().all(|seen| seen));
        let mut missing_geometry = glass_flow_sample(3);
        missing_geometry.vertices[1] = 0;
        assert!(probe.observe(missing_geometry, now).is_err());
        let mut unconfirmed_water = transit;
        unconfirmed_water.sequence = 3;
        unconfirmed_water.flowing[2] = false;
        assert!(probe.observe(unconfirmed_water, now).is_err());
        assert_eq!(probe.samples, 2);
    }

    #[test]
    fn stored_glass_remains_visible_during_backpressure_but_every_pipe_must_really_transfer() {
        let now = Instant::now();
        let initial = GlassFlowSample {
            flowing: [true, false, false],
            ..glass_flow_sample(1)
        };
        let mut probe = GlassFlowProbe::new(initial, now);
        for sequence in 2..=11 {
            let stored = GlassFlowSample {
                flowing: [false; GLASS_PIPES.len()],
                ..glass_flow_sample(sequence)
            };
            assert_eq!(
                probe.observe(stored, now + std::time::Duration::from_secs(sequence)),
                Ok(false)
            );
        }
        assert_eq!(probe.transfer_samples, [1, 0, 0]);
        let transferred = GlassFlowSample {
            flowing: [false, true, true],
            ..glass_flow_sample(12)
        };
        assert_eq!(
            probe.observe(transferred, now + std::time::Duration::from_secs(12)),
            Ok(true)
        );
        assert_eq!(probe.transfer_samples, [1, 1, 1]);
    }

    #[test]
    fn bypass_stop_requires_advanced_server_drop_time() {
        let initial = DropStamp {
            source: 10,
            seconds: 20.0,
        };
        let mut probe = BypassProbe::new(0.3, initial);
        // Any number of rendered frames repeating one authoritative payload
        // must not count as elapsed simulation or produce a stationary failure.
        for _ in 0..1_000 {
            assert_eq!(probe.observe(0.3, initial), Ok(false));
        }
        let advanced = |seconds| DropStamp { seconds, ..initial };
        assert_eq!(probe.observe(0.3, advanced(20.3)), Ok(false));
        assert_eq!(
            probe.observe(0.3, advanced(20.36)),
            Err("stopped on the powered Hold Block")
        );
    }

    #[test]
    fn bypass_power_pause_reanchors_without_hiding_a_resumed_stall() {
        let mut probe = BypassProbe::new(
            0.3,
            DropStamp {
                source: 10,
                seconds: 1.0,
            },
        );
        probe.pause();
        let resumed = DropStamp {
            source: 10,
            seconds: 6.0,
        };
        assert_eq!(probe.observe(0.3, resumed), Ok(false));
        assert_eq!(
            probe.observe(
                0.3,
                DropStamp {
                    seconds: 6.36,
                    ..resumed
                }
            ),
            Err("stopped on the powered Hold Block")
        );
        assert_eq!(
            probe.observe(
                0.4,
                DropStamp {
                    seconds: 6.4,
                    ..resumed
                }
            ),
            Ok(true)
        );
        assert_eq!(probe.samples, 2);
        assert_eq!(probe.stationary_seconds, 0.0);
    }

    #[test]
    fn scripted_holds_follow_default_or_rebound_controls_without_repeating_press_edges() {
        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        let default = binding(&world, terra_core::GameAction::Place);
        assert_eq!(default, terra_core::GameAction::Place.default_binding());
        hold_binding(&mut world, default, true);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyB)
        );
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        hold_binding(&mut world, default, true);
        assert!(
            !world
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyB)
        );
        hold_binding(&mut world, default, false);
        assert!(
            !world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyB)
        );

        let mut controls = terra_core::ControlsConfig::default();
        controls
            .rebind(
                terra_core::GameAction::Place,
                terra_core::InputBinding::Mouse(MouseButton::Right),
            )
            .unwrap();
        world.insert_resource(controls);
        let rebound = binding(&world, terra_core::GameAction::Place);
        hold_binding(&mut world, rebound, true);
        assert!(
            world
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Right)
        );
        hold_binding(&mut world, rebound, false);
        assert!(
            !world
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Right)
        );
        world.init_resource::<terra_core::DebugSettings>();
        world.init_resource::<Messages<terra_player::CaptureRequest>>();
        world.resource_mut::<terra_core::DebugSettings>().enabled = true;
        setup(&mut world);
        assert!(!world.resource::<terra_core::DebugSettings>().enabled);
        assert!(!world.contains_resource::<terra_ui::menu::MenuState>());
    }
}
