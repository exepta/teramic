//! Persistent modular vehicles, including frozen arbitrary block assemblies.
use crate::{Inventory, PlayerUuid, WoodSpecies, containers, train_parts};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_VEHICLES: usize = 64;
pub const MODEL_Y_OFFSET: f64 = -0.7375;
pub const MAX_PARTS: usize = 256;
pub const MAX_TOTAL_PARTS: usize = 4096;
pub const MAX_CONSIST: usize = 16;
pub const MAX_SEATED: usize = 1024;
pub const MAX_PAYLOAD_BYTES: usize = 65_536;
pub const MAX_VEHICLE_PAYLOAD_BYTES: usize = 1_048_576;
pub const MAX_TOTAL_PAYLOAD_BYTES: usize = 8 * 1_048_576;
pub const MAX_VEHICLE_SHAPES: usize = 4096;
pub const MAX_TOTAL_SHAPES: usize = 16_384;
pub type Cell = [i64; 3];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleKind {
    #[default]
    Locomotive,
    Tender,
    Passenger,
    Freight,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CouplerEnd {
    Front,
    Rear,
}
impl CouplerEnd {
    pub const fn index(self) -> usize {
        match self {
            Self::Front => 0,
            Self::Rear => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coupling {
    pub vehicle: u64,
    pub end: CouplerEnd,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainPart {
    pub offset: [i16; 3],
    pub material: u16,
    pub facing: u8,
    pub wood: WoodSpecies,
}
/// Public block appearance and exact local collision, captured before assembly.
/// Inventories and machine internals remain exclusively in the private payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrainBlockView {
    pub boxes: Vec<crate::block_shapes::ShapeBox>,
    pub tint: Option<[u8; 3]>,
    pub heraldry: Option<crate::kingdom::KingdomHeraldry>,
    /// Peer displacement relative to this part, independent of world position.
    pub peer: Option<[i16; 3]>,
    pub secondary: bool,
}
impl TrainBlockView {
    pub fn valid(&self) -> bool {
        self.boxes.len() <= 128
            && self.boxes.iter().all(|bounds| {
                (0..3).all(|axis| {
                    bounds.minimum[axis].is_finite()
                        && bounds.maximum[axis].is_finite()
                        && bounds.minimum[axis] >= -2.0
                        && bounds.maximum[axis] <= 3.0
                        && bounds.minimum[axis] < bounds.maximum[axis]
                })
            })
            && self
                .peer
                .is_none_or(|peer| peer.iter().all(|value| value.unsigned_abs() <= 64))
            && (!self.secondary || self.peer.is_some())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vehicle {
    #[serde(default)]
    pub name: String,
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: Cell,
    pub parts: Vec<TrainPart>,
    pub cargo: BTreeMap<u16, Inventory>,
    #[serde(default)]
    pub blocks: BTreeMap<u16, TrainBlockView>,
    /// Frozen world-owned machine state; never included in public snapshots.
    #[serde(default)]
    pub payloads: BTreeMap<u16, Vec<u8>>,
    #[serde(default)]
    pub motion: crate::train_motion::TrainMotion,
    #[serde(default)]
    pub burn_seconds: f64,
    #[serde(default)]
    pub steam: crate::train_power::SteamState,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VehicleView {
    #[serde(default)]
    pub steam: crate::train_power::SteamState,
    #[serde(default)]
    pub name: String,
    pub id: u64,
    pub owner: PlayerUuid,
    pub kind: VehicleKind,
    pub anchor: Cell,
    pub parts: Vec<TrainPart>,
    #[serde(default)]
    pub blocks: BTreeMap<u16, TrainBlockView>,
    pub couplings: [Option<Coupling>; 2],
    pub revision: u64,
    pub mass_kg: u32,
    #[serde(default)]
    pub motion: crate::train_motion::TrainMotion,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatBinding {
    pub player: PlayerUuid,
    pub vehicle: u64,
    pub part: u16,
}
impl SeatBinding {
    pub fn valid(self) -> bool {
        !self.player.is_nil() && self.vehicle > 0 && usize::from(self.part) < MAX_PARTS
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrainSnapshot {
    pub revision: u64,
    pub vehicles: Vec<VehicleView>,
    pub seats: Vec<SeatBinding>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrainRegistry {
    pub next_id: u64,
    pub revision: u64,
    pub vehicles: BTreeMap<u64, Vehicle>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainError {
    Invalid,
    AttachedProps,
    Limit,
    Missing,
    NotOwner,
    Stale,
    Incomplete,
    Unsupported,
    Occupied,
    Protected,
    OutOfReach,
    Coupled,
    Cycle,
    Busy,
    RateLimited,
    Overflow,
    BogieAlignment,
    Wheelbase,
    MissingMaterials,
    GearLocked,
}
impl std::fmt::Display for TrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TrainError {}

/// Names are single-line UI text, limited by bytes and printable characters.
pub fn valid_name(name: &str) -> bool {
    name.len() <= 64 && name.chars().count() <= 24 && !name.chars().any(char::is_control)
}
fn part_cell(anchor: Cell, parts: &[TrainPart], index: usize) -> Option<Cell> {
    let part = parts.get(index)?;
    Some(std::array::from_fn(|axis| {
        anchor[axis] + i64::from(part.offset[axis])
    }))
}
fn bounds(anchor: Cell, parts: &[TrainPart]) -> [Cell; 2] {
    let mut low = anchor;
    let mut high = anchor;
    for part in parts {
        for axis in 0..3 {
            let cell = anchor[axis] + i64::from(part.offset[axis]);
            low[axis] = low[axis].min(cell);
            high[axis] = high[axis].max(cell + 1);
        }
    }
    [low, high]
}
fn valid_parts(anchor: Cell, parts: &[TrainPart], kind: VehicleKind) -> bool {
    if parts.is_empty()
        || parts.len() > MAX_PARTS
        || anchor.iter().any(|v| v.unsigned_abs() > 990_000)
    {
        return false;
    }
    let mut offsets = BTreeSet::new();
    if parts.iter().any(|part| {
        part.material == 0
            || part.material == terra_voxel::MaterialId::FluidWater as u16
            || part.material == train_parts::STATION
            || crate::rails::is_rail(part.material)
            || train_parts::is_part(part.material)
                && if crate::locomotive_parts::is_part(part.material) {
                    !crate::locomotive_parts::valid_facing(part.material, part.facing)
                } else {
                    part.facing > 3
                }
            || !offsets.insert(part.offset)
            || part.offset[0].unsigned_abs() > 64
            || part.offset[1].unsigned_abs() > 12
            || part.offset[2].unsigned_abs() > 64
    }) {
        return false;
    }
    let mut contacts = BTreeMap::<[i16; 3], BTreeSet<[i16; 3]>>::new();
    for part in parts {
        for offset in crate::train_boiler::attachment_offsets(part.material, part.facing).iter() {
            let other = std::array::from_fn(|axis| part.offset[axis] + offset[axis]);
            if other != part.offset && offsets.contains(&other) {
                contacts.entry(part.offset).or_default().insert(other);
                contacts.entry(other).or_default().insert(part.offset);
            }
        }
    }
    let mut pending = vec![parts[0].offset];
    let mut connected = BTreeSet::new();
    while let Some(cell) = pending.pop() {
        if !connected.insert(cell) {
            continue;
        }
        for delta in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let neighbor = std::array::from_fn(|axis| cell[axis] + delta[axis]);
            if offsets.contains(&neighbor) && !connected.contains(&neighbor) {
                pending.push(neighbor);
            }
        }
        if let Some(adjacent) = contacts.get(&cell) {
            pending.extend(
                adjacent
                    .iter()
                    .copied()
                    .filter(|next| !connected.contains(next)),
            );
        }
    }
    if connected.len() != parts.len() {
        return false;
    }
    let [low, high] = bounds(anchor, parts);
    if high[0] - low[0] > 64 || high[1] - low[1] > 12 || high[2] - low[2] > 64 {
        return false;
    }
    let axis = parts
        .iter()
        .find(|part| part.material == train_parts::BOGIE)
        .map(|part| part.facing % 2);
    if axis.is_some_and(|axis| {
        parts.iter().any(|part| {
            matches!(part.material, train_parts::BOGIE | train_parts::COUPLER)
                && part.facing % 2 != axis
        })
    }) {
        return false;
    }
    let count = |material| {
        parts
            .iter()
            .filter(|part| part.material == material)
            .count()
    };
    if count(train_parts::CHASSIS) == 0
        || count(train_parts::BOGIE) < 2
        || count(train_parts::COUPLER) == 0
    {
        return false;
    }
    match kind {
        VehicleKind::Locomotive => count(train_parts::CAB) > 0,
        VehicleKind::Tender | VehicleKind::Freight => {
            (count(train_parts::CARGO) > 0 || count(29) > 0)
                && count(train_parts::CAB) == 0
                && count(train_parts::SEAT) == 0
        }
        VehicleKind::Passenger => count(train_parts::SEAT) > 0 && count(train_parts::CAB) == 0,
    }
}
/// One collision source for authority, rendering bounds and physics consumers.
pub fn part_shapes(
    part: &TrainPart,
    block: Option<&TrainBlockView>,
) -> Vec<crate::block_shapes::ShapeBox> {
    block.map_or_else(
        || train_parts::boxes(part.material, part.facing),
        |block| block.boxes.clone(),
    )
}
fn shape_count(parts: &[TrainPart], blocks: &BTreeMap<u16, TrainBlockView>) -> usize {
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            blocks.get(&(index as u16)).map_or_else(
                || train_parts::base_boxes(part.material).len(),
                |block| block.boxes.len(),
            )
        })
        .sum()
}
fn valid_blocks(parts: &[TrainPart], blocks: &BTreeMap<u16, TrainBlockView>) -> bool {
    blocks.len() <= parts.len()
        && blocks
            .iter()
            .all(|(index, block)| usize::from(*index) < parts.len() && block.valid())
        && parts.iter().enumerate().all(|(index, part)| {
            train_parts::is_part(part.material) || blocks.contains_key(&(index as u16))
        })
}
/// Keep the historical signed build basis when compatible with the wheel axis.
/// Cubic chassis decoration may face sideways without changing the rail axis.
fn build_facing(parts: &[TrainPart]) -> u8 {
    let bogie = parts
        .iter()
        .find(|part| part.material == train_parts::BOGIE)
        .map_or(0, |part| part.facing);
    parts
        .iter()
        .find(|part| part.material == train_parts::CHASSIS)
        .filter(|part| part.facing % 2 == bogie % 2)
        .map_or(bogie, |part| part.facing)
}
/// Precompute the rigid matrix once for a complete bounds/shape operation.
struct RigidTransform {
    origin: [f64; 3],
    quaternion: [f64; 4],
    rows: [[f64; 3]; 3],
}
impl RigidTransform {
    fn new(anchor: Cell, motion: crate::train_motion::TrainMotion, build_yaw: f64) -> Self {
        let quaternion = motion.quaternion(build_yaw);
        let [x, y, z, w] = quaternion;
        let rows = [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - z * w),
                2.0 * (x * z + y * w),
            ],
            [
                2.0 * (x * y + z * w),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - x * w),
            ],
            [
                2.0 * (x * z - y * w),
                2.0 * (y * z + x * w),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ];
        let pivot: [f64; 3] =
            std::array::from_fn(|axis| (0..3).map(|i| rows[axis][i] * motion.pivot[i]).sum());
        let origin = std::array::from_fn(|axis| {
            anchor[axis] as f64
                + if axis == 1 { MODEL_Y_OFFSET } else { 0.0 }
                + motion.translation[axis]
                + motion.pivot[axis]
                - pivot[axis]
        });
        Self {
            origin,
            quaternion,
            rows,
        }
    }
    fn vector(&self, v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| (0..3).map(|i| self.rows[axis][i] * v[i]).sum())
    }
    fn point(&self, p: [f64; 3]) -> [f64; 3] {
        let v = self.vector(p);
        std::array::from_fn(|axis| self.origin[axis] + v[axis])
    }
    fn local_point(&self, p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| {
            (0..3)
                .map(|i| self.rows[i][axis] * (p[i] - self.origin[i]))
                .sum()
        })
    }
    fn bounds(
        &self,
        part: &TrainPart,
        shape: crate::block_shapes::ShapeBox,
    ) -> crate::block_shapes::ShapeBox {
        let center = self.point(std::array::from_fn(|axis| {
            f64::from(part.offset[axis]) + (shape.minimum[axis] + shape.maximum[axis]) * 0.5
        }));
        let half: [f64; 3] = std::array::from_fn(|axis| {
            (0..3)
                .map(|i| self.rows[axis][i].abs() * (shape.maximum[i] - shape.minimum[i]) * 0.5)
                .sum()
        });
        crate::block_shapes::ShapeBox {
            minimum: std::array::from_fn(|i| center[i] - half[i]),
            maximum: std::array::from_fn(|i| center[i] + half[i]),
            hinged: false,
        }
    }
}
macro_rules! geometry_methods {
    () => {
        pub fn part_shapes(&self, index: usize) -> Vec<crate::block_shapes::ShapeBox> {
            let Some(part) = self.parts.get(index) else {
                return Vec::new();
            };
            part_shapes(part, self.blocks.get(&(index as u16)))
        }
        pub fn block_view(&self, index: usize) -> Option<&TrainBlockView> {
            self.blocks.get(&(index as u16))
        }
        pub fn part_cell(&self, index: usize) -> Option<Cell> {
            part_cell(self.anchor, &self.parts, index)
        }
        pub fn bounds(&self) -> [Cell; 2] {
            bounds(self.anchor, &self.parts)
        }
        pub fn build_yaw(&self) -> f64 {
            f64::from(build_facing(&self.parts)) * std::f64::consts::FRAC_PI_2
        }
        pub fn driver_direction(&self, index: usize) -> i8 {
            let facing = self.parts.get(index).map_or(0, |part| part.facing % 4);
            let look = f64::from(facing) * std::f64::consts::FRAC_PI_2 + std::f64::consts::PI;
            if (look - self.build_yaw()).cos() < -0.5 {
                -1
            } else {
                1
            }
        }
        pub fn world_vector(&self, v: [f64; 3]) -> [f64; 3] {
            self.motion.rotate(v, self.build_yaw())
        }
        pub fn local_vector(&self, v: [f64; 3]) -> [f64; 3] {
            self.motion.inverse(v, self.build_yaw())
        }
        pub fn model_origin(&self) -> [f64; 3] {
            RigidTransform::new(self.anchor, self.motion, self.build_yaw()).origin
        }
        pub fn model_transform(&self) -> ([f64; 3], [f64; 4]) {
            let transform = RigidTransform::new(self.anchor, self.motion, self.build_yaw());
            (transform.origin, transform.quaternion)
        }
        pub fn world_point(&self, local: [f64; 3]) -> [f64; 3] {
            RigidTransform::new(self.anchor, self.motion, self.build_yaw()).point(local)
        }
        pub fn local_point(&self, world: [f64; 3]) -> [f64; 3] {
            RigidTransform::new(self.anchor, self.motion, self.build_yaw()).local_point(world)
        }
        pub fn world_cell(&self, index: usize) -> Option<Cell> {
            let p = self.parts.get(index)?;
            Some(
                self.world_point(p.offset.map(|v| f64::from(v) + 0.5))
                    .map(|v| v.floor() as i64),
            )
        }
        pub fn world_shape(
            &self,
            index: usize,
            shape: crate::block_shapes::ShapeBox,
        ) -> crate::block_shapes::ShapeBox {
            let Some(part) = self.parts.get(index) else {
                return shape;
            };
            RigidTransform::new(self.anchor, self.motion, self.build_yaw()).bounds(part, shape)
        }
        pub fn transformed_shapes(&self, index: usize) -> Vec<crate::block_shapes::ShapeBox> {
            let Some(part) = self.parts.get(index) else {
                return Vec::new();
            };
            let transform = RigidTransform::new(self.anchor, self.motion, self.build_yaw());
            self.part_shapes(index)
                .into_iter()
                .map(|shape| transform.bounds(part, shape))
                .collect()
        }
        /// Conservative world bounds reuse immutable local geometry across motion and edits.
        pub fn model_bounds(&self) -> [[f64; 3]; 2] {
            let local = local_bounds(&self.parts, &self.blocks);
            let transform = RigidTransform::new(self.anchor, self.motion, self.build_yaw());
            let mut result = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
            for corner in 0..8 {
                let point = transform.point(std::array::from_fn(|axis| {
                    local[usize::from(corner & (1 << axis) != 0)][axis]
                }));
                for axis in 0..3 {
                    result[0][axis] = result[0][axis].min(point[axis]);
                    result[1][axis] = result[1][axis].max(point[axis]);
                }
            }
            result
        }
        pub fn model_transform_at(
            &self,
            motion: crate::train_motion::TrainMotion,
        ) -> ([f64; 3], [f64; 4]) {
            let transform = RigidTransform::new(self.anchor, motion, self.build_yaw());
            (transform.origin, transform.quaternion)
        }
        pub fn seat_pose_at(
            &self,
            index: usize,
            motion: crate::train_motion::TrainMotion,
        ) -> Option<([f64; 3], f32)> {
            let part = self.parts.get(index)?;
            if !matches!(part.material, train_parts::CAB | train_parts::SEAT) {
                return None;
            }
            Some((
                RigidTransform::new(self.anchor, motion, self.build_yaw()).point([
                    f64::from(part.offset[0]) + 0.5,
                    f64::from(part.offset[1]) + 0.485,
                    f64::from(part.offset[2]) + 0.5,
                ]),
                (f64::from(part.facing) * std::f64::consts::FRAC_PI_2 + motion.yaw) as f32,
            ))
        }
        pub fn seat_pose(&self, index: usize) -> Option<([f64; 3], f32)> {
            let part = self.parts.get(index)?;
            if !matches!(part.material, train_parts::CAB | train_parts::SEAT) {
                return None;
            }
            Some((
                self.world_point([
                    f64::from(part.offset[0]) + 0.5,
                    f64::from(part.offset[1]) + 0.485,
                    f64::from(part.offset[2]) + 0.5,
                ]),
                (f64::from(part.facing) * std::f64::consts::FRAC_PI_2 + self.motion.yaw) as f32,
            ))
        }
        /// Grid-aligned stopped poses can return exactly to editable world cells.
        pub fn disassembly_turns(&self) -> Option<u8> {
            if !self.motion.stopped() || self.motion.pitch.abs() > 1e-5 {
                return None;
            }
            let turns = self.motion.yaw / std::f64::consts::FRAC_PI_2;
            ((turns - turns.round()).abs() < 1e-5)
                .then_some((turns.round() as i32).rem_euclid(4) as u8)
        }
        pub fn disassembly_cells(&self) -> Option<Vec<Cell>> {
            if !self.motion.stopped() || self.motion.pitch.abs() > 1e-5 {
                return None;
            }
            let quarter = self.motion.yaw / std::f64::consts::FRAC_PI_2;
            if (quarter - quarter.round()).abs() > 1e-5 {
                return None;
            }
            let mut cells = Vec::new();
            for part in &self.parts {
                // Remove only the authored wheel-to-rail vertical adjustment.
                let mut point = self.world_point(part.offset.map(|v| f64::from(v) + 0.5));
                point[1] -= MODEL_Y_OFFSET;
                let point = point.map(|v| v - 0.5);
                if point.iter().any(|v| (v - v.round()).abs() > 0.500001) {
                    return None;
                }
                cells.push(point.map(|v| v.round() as i64));
            }
            Some(cells)
        }
        pub fn cargo_indices(&self) -> impl Iterator<Item = u16> + '_ {
            self.parts
                .iter()
                .enumerate()
                .filter(|(_, part)| matches!(part.material, 29 | train_parts::CARGO | 218))
                .map(|(index, _)| index as u16)
        }
        pub fn coupler_point(&self, end: CouplerEnd) -> Option<[f64; 3]> {
            let facing = build_facing(&self.parts);
            let score = |part: &TrainPart| match facing {
                0 => i32::from(part.offset[2]),
                1 => i32::from(part.offset[0]),
                2 => -i32::from(part.offset[2]),
                _ => -i32::from(part.offset[0]),
            };
            let mut couplers = self
                .parts
                .iter()
                .enumerate()
                .filter(|(_, part)| part.material == train_parts::COUPLER)
                .collect::<Vec<_>>();
            couplers.sort_by_key(|(index, part)| (score(part), *index));
            if couplers.len() == 1 {
                let chassis = self
                    .parts
                    .iter()
                    .find(|part| part.material == train_parts::CHASSIS)?;
                let actual = if score(couplers[0].1) >= score(chassis) {
                    CouplerEnd::Front
                } else {
                    CouplerEnd::Rear
                };
                if end != actual {
                    return None;
                }
            }
            let index = match end {
                CouplerEnd::Front => couplers.last()?.0,
                CouplerEnd::Rear => couplers.first()?.0,
            };
            Some(self.world_point(self.parts.get(index)?.offset.map(|v| f64::from(v) + 0.5)))
        }
        pub fn coupler_cell(&self, end: CouplerEnd) -> Option<Cell> {
            Some(self.coupler_point(end)?.map(|v| v.floor() as i64))
        }
        pub fn coupler_tangent(&self, end: CouplerEnd) -> Option<Cell> {
            self.coupler_cell(end)?;
            let facing = build_facing(&self.parts);
            let direction = [[0, 0, 1], [1, 0, 0], [0, 0, -1], [-1, 0, 0]][usize::from(facing)];
            self.disassembly_turns()?;
            let vector = self.world_vector(direction.map(|value| value as f64));
            Some(vector.map(|value| {
                (if end == CouplerEnd::Front {
                    value
                } else {
                    -value
                })
                .round() as i64
            }))
        }
    };
}
impl Vehicle {
    geometry_methods!();
    pub fn valid(&self) -> bool {
        self.id > 0
            && !self.owner.is_nil()
            && valid_name(&self.name)
            && self.motion.valid()
            && self.steam.valid()
            && valid_parts(self.anchor, &self.parts, self.kind)
            && valid_blocks(&self.parts, &self.blocks)
            && self.burn_seconds.is_finite()
            && (0.0..=3600.0).contains(&self.burn_seconds)
            && self.payloads.len() <= self.parts.len()
            && self.payloads.values().map(Vec::len).sum::<usize>() <= MAX_VEHICLE_PAYLOAD_BYTES
            && self.payloads.iter().all(|(index, payload)| {
                usize::from(*index) < self.parts.len() && payload.len() <= MAX_PAYLOAD_BYTES
            })
            && self.cargo.len() == self.cargo_indices().count()
            && self.cargo.iter().all(|(index, inventory)| {
                self.parts
                    .get(usize::from(*index))
                    .is_some_and(|part| matches!(part.material, 29 | train_parts::CARGO | 218))
                    && containers::valid_chest(inventory)
                    && (self.parts[usize::from(*index)].material != 218
                        || inventory.slots.iter().all(|stack| {
                            stack.count == 0 || crate::furnace::fuel_seconds(stack.item).is_some()
                        }))
            })
    }
    pub fn mass_kg(&self) -> u32 {
        self.parts
            .iter()
            .map(|part| train_parts::mass_kg(part.material).unwrap_or(100))
            .sum::<u32>()
            + self
                .cargo
                .values()
                .flat_map(|inventory| inventory.slots.iter())
                .map(|stack| u32::from(stack.count) * 2)
                .sum::<u32>()
    }
    pub fn view(&self) -> VehicleView {
        VehicleView {
            steam: self.steam,
            name: self.name.clone(),
            id: self.id,
            owner: self.owner,
            kind: self.kind,
            anchor: self.anchor,
            parts: self.parts.clone(),
            blocks: self.blocks.clone(),
            couplings: self.couplings,
            revision: self.revision,
            mass_kg: self.mass_kg(),
            motion: self.motion,
        }
    }
}
impl VehicleView {
    geometry_methods!();
    pub fn valid(&self) -> bool {
        self.id > 0
            && !self.owner.is_nil()
            && valid_name(&self.name)
            && self.motion.valid()
            && self.steam.valid()
            && valid_parts(self.anchor, &self.parts, self.kind)
            && valid_blocks(&self.parts, &self.blocks)
            && self.mass_kg
                >= self
                    .parts
                    .iter()
                    .map(|part| train_parts::mass_kg(part.material).unwrap_or(100))
                    .sum::<u32>()
            && self.mass_kg < 1_000_000_000
    }
}
impl TrainRegistry {
    pub fn valid(&self) -> bool {
        if self
            .vehicles
            .values()
            .map(|vehicle| vehicle.parts.len())
            .sum::<usize>()
            > MAX_TOTAL_PARTS
            || self
                .vehicles
                .values()
                .map(|vehicle| shape_count(&vehicle.parts, &vehicle.blocks))
                .sum::<usize>()
                > MAX_TOTAL_SHAPES
            || self
                .vehicles
                .values()
                .flat_map(|vehicle| vehicle.payloads.values())
                .map(Vec::len)
                .sum::<usize>()
                > MAX_TOTAL_PAYLOAD_BYTES
            || self.vehicles.len() > MAX_VEHICLES
            || self
                .vehicles
                .iter()
                .any(|(id, vehicle)| *id != vehicle.id || *id > self.next_id || !vehicle.valid())
        {
            return false;
        }
        let mut cells = BTreeSet::new();
        if self.vehicles.values().any(|vehicle| {
            vehicle
                .disassembly_cells()
                .is_some_and(|positions| positions.into_iter().any(|cell| !cells.insert(cell)))
        }) {
            return false;
        }
        self.vehicles.keys().all(|id| self.consist(*id).is_ok())
    }
    pub fn consist(&self, id: u64) -> Result<Vec<u64>, TrainError> {
        if !self.vehicles.contains_key(&id) {
            return Err(TrainError::Missing);
        }
        let mut pending = vec![(id, None)];
        let mut visited = BTreeSet::new();
        while let Some((current, parent)) = pending.pop() {
            if !visited.insert(current) {
                return Err(TrainError::Cycle);
            }
            if visited.len() > MAX_CONSIST {
                return Err(TrainError::Limit);
            }
            let vehicle = self.vehicles.get(&current).ok_or(TrainError::Incomplete)?;
            for (end, link) in vehicle
                .couplings
                .iter()
                .enumerate()
                .filter_map(|(end, link)| link.map(|link| (end, link)))
            {
                let other = self
                    .vehicles
                    .get(&link.vehicle)
                    .ok_or(TrainError::Incomplete)?;
                if other.owner != vehicle.owner
                    || other.couplings[link.end.index()]
                        != Some(Coupling {
                            vehicle: current,
                            end: if end == 0 {
                                CouplerEnd::Front
                            } else {
                                CouplerEnd::Rear
                            },
                        })
                {
                    return Err(TrainError::Incomplete);
                }
                if parent != Some(link.vehicle) {
                    pending.push((link.vehicle, Some(current)));
                }
            }
        }
        Ok(visited.into_iter().collect())
    }
    pub fn insert(&mut self, mut vehicle: Vehicle) -> Result<u64, TrainError> {
        if self.vehicles.len() >= MAX_VEHICLES
            || self
                .vehicles
                .values()
                .map(|old| old.parts.len())
                .sum::<usize>()
                + vehicle.parts.len()
                > MAX_TOTAL_PARTS
            || self
                .vehicles
                .values()
                .map(|old| shape_count(&old.parts, &old.blocks))
                .sum::<usize>()
                + shape_count(&vehicle.parts, &vehicle.blocks)
                > MAX_TOTAL_SHAPES
            || self
                .vehicles
                .values()
                .flat_map(|old| old.payloads.values())
                .map(Vec::len)
                .sum::<usize>()
                + vehicle.payloads.values().map(Vec::len).sum::<usize>()
                > MAX_TOTAL_PAYLOAD_BYTES
        {
            return Err(TrainError::Limit);
        }
        let id = self.next_id.checked_add(1).ok_or(TrainError::Overflow)?;
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        vehicle.id = id;
        vehicle.revision = 1;
        if !vehicle.valid() || vehicle.couplings != [None, None] {
            return Err(TrainError::Invalid);
        }
        let occupied: BTreeSet<_> = self
            .vehicles
            .values()
            .flat_map(|old| {
                old.disassembly_cells().unwrap_or_else(|| {
                    (0..old.parts.len())
                        .filter_map(|index| old.world_cell(index))
                        .collect()
                })
            })
            .collect();
        let cells = vehicle.disassembly_cells().unwrap_or_else(|| {
            (0..vehicle.parts.len())
                .filter_map(|index| vehicle.world_cell(index))
                .collect()
        });
        if cells.iter().any(|cell| occupied.contains(cell)) {
            return Err(TrainError::Occupied);
        }
        self.vehicles.insert(id, vehicle);
        self.next_id = id;
        self.revision = revision;
        Ok(id)
    }
    pub fn remove(&mut self, actor: PlayerUuid, id: u64) -> Result<Vehicle, TrainError> {
        let vehicle = self.vehicles.get(&id).ok_or(TrainError::Missing)?;
        if vehicle.owner != actor {
            return Err(TrainError::NotOwner);
        }
        if vehicle.couplings.iter().any(Option::is_some) {
            return Err(TrainError::Coupled);
        }
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        let removed = self.vehicles.remove(&id).unwrap();
        self.revision = revision;
        Ok(removed)
    }
    pub fn couple(
        &mut self,
        actor: PlayerUuid,
        first: u64,
        first_end: CouplerEnd,
        second: u64,
        second_end: CouplerEnd,
    ) -> Result<(), TrainError> {
        if first == second {
            return Err(TrainError::Cycle);
        }
        let a = self.vehicles.get(&first).ok_or(TrainError::Missing)?;
        let b = self.vehicles.get(&second).ok_or(TrainError::Missing)?;
        if a.owner != actor || b.owner != actor {
            return Err(TrainError::NotOwner);
        }
        if a.couplings[first_end.index()].is_some() || b.couplings[second_end.index()].is_some() {
            return Err(TrainError::Coupled);
        }
        let ca = self.consist(first)?;
        let cb = self.consist(second)?;
        if ca.contains(&second) {
            return Err(TrainError::Cycle);
        }
        if ca.len() + cb.len() > MAX_CONSIST {
            return Err(TrainError::Limit);
        }
        let ac = a.coupler_cell(first_end).ok_or(TrainError::Incomplete)?;
        let bc = b.coupler_cell(second_end).ok_or(TrainError::Incomplete)?;
        let distance = (0..3).map(|axis| ac[axis].abs_diff(bc[axis])).sum::<u64>();
        let direction = a.coupler_tangent(first_end).ok_or(TrainError::Incomplete)?;
        if b.coupler_tangent(second_end) != Some(direction.map(|value| -value))
            || !(1..=2).contains(&distance)
            || (0..3).any(|axis| bc[axis] - ac[axis] != direction[axis] * distance as i64)
        {
            return Err(TrainError::OutOfReach);
        }
        self.change_link(
            first,
            first_end,
            Some(Coupling {
                vehicle: second,
                end: second_end,
            }),
            second,
            second_end,
            Some(Coupling {
                vehicle: first,
                end: first_end,
            }),
        )
    }
    pub fn uncouple(
        &mut self,
        actor: PlayerUuid,
        id: u64,
        end: CouplerEnd,
    ) -> Result<(), TrainError> {
        let vehicle = self.vehicles.get(&id).ok_or(TrainError::Missing)?;
        if vehicle.owner != actor {
            return Err(TrainError::NotOwner);
        }
        let link = vehicle.couplings[end.index()].ok_or(TrainError::Missing)?;
        let other = self
            .vehicles
            .get(&link.vehicle)
            .ok_or(TrainError::Incomplete)?;
        if other.owner != actor
            || other.couplings[link.end.index()] != Some(Coupling { vehicle: id, end })
        {
            return Err(TrainError::Incomplete);
        }
        self.change_link(id, end, None, link.vehicle, link.end, None)
    }
    fn change_link(
        &mut self,
        a: u64,
        ae: CouplerEnd,
        al: Option<Coupling>,
        b: u64,
        be: CouplerEnd,
        bl: Option<Coupling>,
    ) -> Result<(), TrainError> {
        let revision = self.revision.checked_add(1).ok_or(TrainError::Overflow)?;
        let ar = self.vehicles[&a]
            .revision
            .checked_add(1)
            .ok_or(TrainError::Overflow)?;
        let br = self.vehicles[&b]
            .revision
            .checked_add(1)
            .ok_or(TrainError::Overflow)?;
        let first = self.vehicles.get_mut(&a).unwrap();
        first.couplings[ae.index()] = al;
        first.revision = ar;
        let second = self.vehicles.get_mut(&b).unwrap();
        second.couplings[be.index()] = bl;
        second.revision = br;
        self.revision = revision;
        Ok(())
    }
    pub fn snapshot(&self, seats: Vec<SeatBinding>) -> TrainSnapshot {
        TrainSnapshot {
            revision: self.revision,
            vehicles: self.vehicles.values().map(Vehicle::view).collect(),
            seats,
        }
    }
}
impl TrainSnapshot {
    pub fn valid(&self) -> bool {
        if self
            .vehicles
            .iter()
            .map(|vehicle| vehicle.parts.len())
            .sum::<usize>()
            > MAX_TOTAL_PARTS
            || self
                .vehicles
                .iter()
                .map(|vehicle| shape_count(&vehicle.parts, &vehicle.blocks))
                .sum::<usize>()
                > MAX_TOTAL_SHAPES
            || self.vehicles.len() > MAX_VEHICLES
            || self.seats.len() > MAX_SEATED
        {
            return false;
        }
        let mut ids = BTreeSet::new();
        if self
            .vehicles
            .iter()
            .any(|vehicle| !vehicle.valid() || !ids.insert(vehicle.id))
        {
            return false;
        }
        let mut players = BTreeSet::new();
        let mut occupied = BTreeSet::new();
        self.seats.iter().all(|seat| {
            !seat.player.is_nil()
                && players.insert(seat.player)
                && occupied.insert((seat.vehicle, seat.part))
                && self
                    .vehicles
                    .iter()
                    .find(|vehicle| vehicle.id == seat.vehicle)
                    .is_some_and(|vehicle| vehicle.seat_pose(usize::from(seat.part)).is_some())
        }) && self.vehicles.iter().all(|vehicle| {
            vehicle.couplings.iter().enumerate().all(|(end, link)| {
                link.is_none_or(|link| {
                    self.vehicles
                        .iter()
                        .find(|other| other.id == link.vehicle)
                        .is_some_and(|other| {
                            other.owner == vehicle.owner
                                && other.couplings[link.end.index()]
                                    == Some(Coupling {
                                        vehicle: vehicle.id,
                                        end: if end == 0 {
                                            CouplerEnd::Front
                                        } else {
                                            CouplerEnd::Rear
                                        },
                                    })
                        })
                })
            })
        })
    }
}

#[cfg(test)]
mod tests;

struct BoundsEntry {
    parts: Vec<TrainPart>,
    blocks: BTreeMap<u16, TrainBlockView>,
    bounds: [[f64; 3]; 2],
}
fn local_bounds(parts: &[TrainPart], blocks: &BTreeMap<u16, TrainBlockView>) -> [[f64; 3]; 2] {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::VecDeque<BoundsEntry>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    {
        let entries = cache.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.parts == parts && entry.blocks == *blocks)
        {
            return entry.bounds;
        }
    }
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for (index, part) in parts.iter().enumerate() {
        for shape in part_shapes(part, blocks.get(&(index as u16))) {
            for (axis, offset) in part.offset.iter().enumerate() {
                bounds[0][axis] = bounds[0][axis].min(f64::from(*offset) + shape.minimum[axis]);
                bounds[1][axis] = bounds[1][axis].max(f64::from(*offset) + shape.maximum[axis]);
            }
        }
    }
    if parts.is_empty() {
        bounds = [[0.0; 3]; 2];
    }
    let mut entries = cache.lock().unwrap_or_else(|error| error.into_inner());
    if entries.len() >= MAX_VEHICLES {
        entries.pop_front();
    }
    entries.push_back(BoundsEntry {
        parts: parts.to_vec(),
        blocks: blocks.clone(),
        bounds,
    });
    bounds
}
