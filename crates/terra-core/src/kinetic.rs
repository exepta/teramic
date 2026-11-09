//! Bounded mechanical transmission shared by server simulation and presentation.
use std::collections::{BTreeMap, VecDeque};

pub type Cell = [i64; 3];
pub const WOODEN_GEAR: u16 = 141;
pub const STONE_GEAR: u16 = 142;
pub const IRON_GEAR: u16 = 143;
pub const WOODEN_SHAFT: u16 = 144;
pub const STONE_SHAFT: u16 = 145;
pub const IRON_SHAFT: u16 = 146;
pub const WATER_WHEEL: u16 = 147;
pub const WATER_WHEEL_CAPACITY: u16 = 19200;
pub const WATER_WHEEL_RPM: f32 = 20.0;
pub const CONVEYOR: u16 = 148;
pub const CONVEYOR_TOP: f32 = 0.625;
pub const CONVEYOR_AXIS_HEIGHT: f32 = 0.5;
pub const BELT_FACING_COUNT: u8 = 84;

pub const fn belt_facing(raw: u8) -> u8 {
    if raw < 56 {
        raw % 4
    } else {
        4 + (raw - 56) % 2
    }
}
pub const fn belt_pose(facing: u8) -> u8 {
    if facing < 4 { facing } else { 56 + facing - 4 }
}
pub const fn belt_color(raw: u8) -> Option<crate::beds::DyeColor> {
    let index = if raw < 56 { raw / 4 } else { (raw - 56) / 2 };
    if index == 0 || index > crate::beds::DyeColor::ALL.len() as u8 {
        None
    } else {
        Some(crate::beds::DyeColor::ALL[index as usize - 1])
    }
}
pub fn dyed_belt_facing(raw: u8, color: crate::beds::DyeColor) -> u8 {
    if matches!(color, crate::beds::DyeColor::Rgb(_)) {
        return belt_pose(belt_facing(raw));
    }
    let color = crate::beds::DyeColor::ALL
        .iter()
        .position(|c| *c == color)
        .unwrap() as u8
        + 1;
    let facing = belt_facing(raw);
    if facing < 4 {
        facing + 4 * color
    } else {
        56 + facing - 4 + 2 * color
    }
}
pub const WOODEN_LARGE_GEAR: u16 = 149;
pub const STONE_LARGE_GEAR: u16 = 150;
pub const IRON_LARGE_GEAR: u16 = 151;
pub const GEARBOX_HORIZONTAL: u16 = 154;
pub const GEARBOX_VERTICAL: u16 = 155;

pub const fn is_gearbox(material: u16) -> bool {
    matches!(material, GEARBOX_HORIZONTAL | GEARBOX_VERTICAL)
}
fn yaw_direction(mut direction: Cell, facing: u8) -> Cell {
    for _ in 0..facing % 4 {
        direction = [direction[2], direction[1], -direction[0]];
    }
    direction
}
pub fn gearbox_ports(material: u16, facing: u8) -> [Cell; 4] {
    let primary = yaw_direction([1, 0, 0], facing);
    let cross = yaw_direction(
        if material == GEARBOX_VERTICAL {
            [0, 1, 0]
        } else {
            [0, 0, 1]
        },
        facing,
    );
    [primary, primary.map(|v| -v), cross, cross.map(|v| -v)]
}
/// Port RPM in world-positive axes, relative to the gearbox's shared internal phase.
pub fn gearbox_port_factor(material: u16, facing: u8, direction: Cell) -> Option<i8> {
    if !is_gearbox(material) || facing >= 4 {
        return None;
    }
    let ports = gearbox_ports(material, facing);
    let index = ports.iter().position(|p| *p == direction)?;
    let positive = ports[if index < 2 { 0 } else { 2 }];
    Some([1, -1, -1, 1][index] * positive.into_iter().sum::<i64>() as i8)
}
pub fn has_shaft_port(material: u16, facing: u8, direction: Cell) -> bool {
    if is_gearbox(material) {
        return gearbox_port_factor(material, facing, direction).is_some();
    }
    let axis = axis(if material == CONVEYOR {
        belt_facing(facing)
    } else {
        facing
    });
    axis == direction || axis == direction.map(|v| -v)
}
pub const MAX_COMPONENT: usize = 256;
pub const MAX_BELT_SEGMENTS: u8 = 32;
pub const MAX_RPM: i16 = 256;
pub const DIRECTIONS: [Cell; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
pub const NEIGHBORS: [Cell; 18] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 0],
    [1, -1, 0],
    [-1, 1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [1, 0, -1],
    [-1, 0, 1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, 1, -1],
    [0, -1, 1],
    [0, -1, -1],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Gear,
    LargeGear,
    Shaft,
    WaterWheel,
    Conveyor,
    Crusher,
    Mixer,
    HandCrank,
    Gearbox,
    Pump,
}

pub const fn part(material: u16) -> Option<PartKind> {
    Some(match material {
        141..=143 => PartKind::Gear,
        149..=151 => PartKind::LargeGear,
        144..=146 => PartKind::Shaft,
        WATER_WHEEL => PartKind::WaterWheel,
        CONVEYOR => PartKind::Conveyor,
        GEARBOX_HORIZONTAL | GEARBOX_VERTICAL => PartKind::Gearbox,
        99 => PartKind::Crusher,
        156 => PartKind::Mixer,
        168 => PartKind::Pump,
        100 => PartKind::HandCrank,
        _ => return None,
    })
}
pub const fn is_rotary(material: u16) -> bool {
    matches!(material, 141..=147|149..=151)
}
pub const fn uses_adaptive_facing(material: u16) -> bool {
    is_rotary(material) || material == CONVEYOR || is_gearbox(material) || material == 168
}
pub const fn is_gear(material: u16) -> bool {
    matches!(material,141..=143|149..=151)
}
pub const fn is_large_gear(material: u16) -> bool {
    matches!(material, 149..=151)
}
pub const fn is_crank_mount(material: u16) -> bool {
    matches!(material,141..=146|149..=151|154..=156|168)
}
pub const fn is_extended(material: u16) -> bool {
    is_gear(material) || matches!(material, 147 | 148 | 152 | 153 | 157..=160 | 219..=223)
}
pub const fn radial_extent(material: u16) -> Option<f32> {
    match material {
        141..=143 => Some(0.55),
        149..=151 => Some(1.05),
        147 => Some(1.5),
        144..=146 => Some(0.132),
        _ => None,
    }
}
pub const fn is_part(material: u16) -> bool {
    part(material).is_some()
}
pub const fn axis(facing: u8) -> Cell {
    match facing {
        0 => [1, 0, 0],
        1 => [0, 0, -1],
        2 => [-1, 0, 0],
        3 => [0, 0, 1],
        4 => [0, 1, 0],
        5 => [0, -1, 0],
        _ => [0; 3],
    }
}
pub const fn facing(direction: Cell) -> Option<u8> {
    match direction {
        [1, 0, 0] => Some(0),
        [0, 0, -1] => Some(1),
        [-1, 0, 0] => Some(2),
        [0, 0, 1] => Some(3),
        [0, 1, 0] => Some(4),
        [0, -1, 0] => Some(5),
        _ => None,
    }
}
pub const fn forward(facing: u8) -> Cell {
    match facing % 4 {
        0 => [0, 0, 1],
        1 => [1, 0, 0],
        2 => [0, 0, -1],
        _ => [-1, 0, 0],
    }
}
pub fn offset(position: Cell, direction: Cell) -> Cell {
    std::array::from_fn(|i| position[i] + direction[i])
}
/// Applied before axis rotation to center the legacy crank on a generic axle.
pub fn crank_local_offset(position: Cell, peer: Option<Cell>) -> [f32; 3] {
    if peer.is_some_and(|peer| {
        matches!(
            position
                .into_iter()
                .zip(peer)
                .map(|(a, b)| u128::from(a.abs_diff(b)))
                .sum::<u128>(),
            1 | 2
        )
    }) {
        [0.0, 0.045, -0.015]
    } else {
        [0.0; 3]
    }
}
fn dot(a: Cell, b: Cell) -> i64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct KineticState {
    pub rpm: f32,
    pub overloaded: bool,
    /// Network demand and supply, normalized to load units at 30 RPM.
    pub stress: f32,
    pub capacity: f32,
    /// Supply remaining after the loads along the cheapest source-to-part path.
    pub available_capacity: f32,
}
impl KineticState {
    pub fn running(self) -> bool {
        self.rpm != 0.0 && !self.overloaded
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct SourceDrive {
    pub rpm: f32,
    /// Torque in hundredths of a load unit, independent of rotational speed.
    pub capacity: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub position: Cell,
    pub material: u16,
    pub facing: u8,
    /// A two-cell crusher counts its load only at its primary cell.
    pub secondary: bool,
    pub source_rpm: f32,
    /// Available source torque in hundredths of a load unit.
    pub source_capacity: u16,
    /// Explicit neighboring cells of an owned conveyor span.
    pub links: [Option<Cell>; 2],
    /// A shaft controller carries one belt load in addition to its own load.
    pub belt_load: bool,
}
impl Node {
    pub fn load(self) -> u16 {
        let own = match self.material {
            99 if !self.secondary => 20,
            156 => 24,
            168 => 12,
            CONVEYOR if !self.secondary => 2,
            141..=146 => 1,
            149..=151 => 2,
            GEARBOX_HORIZONTAL | GEARBOX_VERTICAL => 2,
            _ => 0,
        };
        own + if self.belt_load { 2 } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connection {
    pub direction: i8,
    /// RPM multiplier is direction * 2^speed_shift. Torque transforms inversely.
    pub speed_shift: i8,
}

fn node_has_shaft_port(node: Node, direction: Cell) -> bool {
    // A stretched belt has axles at its retained shafts, not inside its ribbon.
    !(node.material == CONVEYOR && node.links.iter().any(Option::is_some))
        && has_shaft_port(node.material, node.facing, direction)
}

/// Axial couplings, parallel cog meshes and diagonal right-angle large gears.
/// The grid arrangements match Create's RotationPropagator connection rules.
pub fn connection(a: Node, b: Node) -> Option<Connection> {
    if !is_part(a.material) || !is_part(b.material) {
        return None;
    }
    let direction: Cell = std::array::from_fn(|i| b.position[i] - a.position[i]);
    let distance = direction.iter().map(|v| v.abs()).sum::<i64>();
    if !matches!(distance, 1 | 2) || direction.iter().any(|v| v.abs() > 1) {
        return None;
    }
    if is_gearbox(a.material) || is_gearbox(b.material) {
        if distance != 1 {
            return None;
        }
        let factor = |node: Node, port: Cell| {
            if is_gearbox(node.material) {
                gearbox_port_factor(node.material, node.facing, port)
            } else {
                node_has_shaft_port(node, port).then(|| {
                    axis(if node.material == CONVEYOR {
                        belt_facing(node.facing)
                    } else {
                        node.facing
                    })
                    .into_iter()
                    .sum::<i64>() as i8
                })
            }
        };
        return Some(Connection {
            direction: factor(a, direction)? * factor(b, direction.map(|v| -v))?,
            speed_shift: 0,
        });
    }
    let aa = axis(if a.material == CONVEYOR {
        belt_facing(a.facing)
    } else {
        a.facing
    });
    let ba = axis(if b.material == CONVEYOR {
        belt_facing(b.facing)
    } else {
        b.facing
    });
    let alignment = dot(aa, ba);
    if alignment.abs() == 1
        && (a.links.contains(&Some(b.position)) || b.links.contains(&Some(a.position)))
    {
        return Some(Connection {
            direction: alignment as i8,
            speed_shift: 0,
        });
    }
    if alignment.abs() == 1
        && distance == 1
        && node_has_shaft_port(a, direction)
        && node_has_shaft_port(b, direction.map(|v| -v))
    {
        return Some(Connection {
            direction: alignment as i8,
            speed_shift: 0,
        });
    }
    if !is_gear(a.material) || !is_gear(b.material) {
        return None;
    }
    let al = is_large_gear(a.material);
    let bl = is_large_gear(b.material);
    if alignment.abs() == 1
        && dot(direction, aa) == 0
        && (!al && !bl && distance == 1 || al != bl && distance == 2)
    {
        return Some(Connection {
            direction: -alignment as i8,
            speed_shift: i8::from(al) - i8::from(bl),
        });
    }
    if al && bl && alignment == 0 && distance == 2 {
        let source = dot(direction, aa);
        let target = dot(direction, ba);
        if source.abs() == 1 && target.abs() == 1 {
            return Some(Connection {
                direction: if (source > 0) ^ (target > 0) { -1 } else { 1 },
                speed_shift: 0,
            });
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ratio {
    sign: i8,
    shift: i16,
}
impl Ratio {
    fn factor(self) -> f64 {
        f64::from(self.sign) * 2.0_f64.powi(i32::from(self.shift))
    }
}

/// Explore at most MAX_COMPONENT cells and charge every visited node to the
/// caller's shared tick budget. Incomplete networks fail closed.
pub fn solve_component(
    seed: Cell,
    budget: &mut usize,
    mut lookup: impl FnMut(Cell) -> Option<Node>,
) -> Vec<(Node, KineticState)> {
    if *budget == 0 {
        return Vec::new();
    }
    let Some(first) = lookup(seed) else {
        return Vec::new();
    };
    let mut nodes = BTreeMap::from([(seed, (first, Ratio { sign: 1, shift: 0 }))]);
    let mut queue = VecDeque::from([seed]);
    let mut conflict = false;
    while let Some(position) = queue.pop_front() {
        if *budget == 0 {
            conflict = true;
            break;
        }
        *budget -= 1;
        let (node, ratio) = nodes[&position];
        for direction in NEIGHBORS {
            let cell = offset(position, direction);
            let Some(other) = lookup(cell) else {
                continue;
            };
            let Some(link) = connection(node, other) else {
                continue;
            };
            let expected = Ratio {
                sign: ratio.sign * link.direction,
                shift: ratio.shift + i16::from(link.speed_shift),
            };
            if expected.shift.abs() > 16 {
                conflict = true;
                continue;
            }
            if let Some((_, old)) = nodes.get(&cell) {
                conflict |= *old != expected;
            } else if nodes.len() >= MAX_COMPONENT || queue.len() >= *budget {
                // Do not publish partial power when a component exceeds a budget.
                conflict = true;
            } else {
                nodes.insert(cell, (other, expected));
                queue.push_back(cell);
            }
        }
    }
    let mut rpm: Option<f64> = None;
    for (node, ratio) in nodes.values() {
        if !node.source_rpm.is_finite() || node.source_rpm.abs() > f32::from(MAX_RPM) {
            conflict = true;
            continue;
        }
        if node.source_rpm != 0.0 {
            let source = f64::from(node.source_rpm) / ratio.factor();
            conflict |= rpm.is_some_and(|previous| (previous - source).abs() > 0.001);
            rpm = Some(source);
        }
    }
    // Normalize torque * speed to power at 30 RPM. A 2:1 speed increase
    // doubles the consumer's demand, while a reduction doubles usable torque.
    let mut supply = 0.0f64;
    let mut load = 0.0f64;
    for (node, ratio) in nodes.values() {
        let speed = (rpm.unwrap_or(0.0) * ratio.factor()).abs();
        conflict |= speed > f64::from(MAX_RPM) + 0.001;
        load += f64::from(node.load()) * 100.0 * speed.min(f64::from(MAX_RPM)) / 30.0;
        if node.source_rpm.is_finite() && node.source_rpm.abs() <= f32::from(MAX_RPM) {
            supply += f64::from(node.source_capacity) * f64::from(node.source_rpm.abs()) / 30.0;
        }
    }
    let overloaded = conflict || rpm.is_some() && load > supply + 0.001;
    let mut losses = BTreeMap::new();
    let mut frontier = std::collections::BinaryHeap::new();
    for (&position, (node, _)) in &nodes {
        if node.source_rpm.is_finite()
            && node.source_rpm != 0.0
            && node.source_rpm.abs() <= f32::from(MAX_RPM)
        {
            losses.insert(position, 0.0_f64);
            frontier.push(std::cmp::Reverse((0_u64, position)));
        }
    }
    // Positive path costs keep loops and alternate power routes from counting
    // the same load repeatedly. This search is bounded by MAX_COMPONENT.
    while let Some(std::cmp::Reverse((bits, position))) = frontier.pop() {
        let loss = f64::from_bits(bits);
        if losses.get(&position).is_none_or(|best| loss > *best) {
            continue;
        }
        let (node, _) = nodes[&position];
        for direction in NEIGHBORS {
            let next = offset(position, direction);
            let Some(&(other, ratio)) = nodes.get(&next) else {
                continue;
            };
            if connection(node, other).is_none() {
                continue;
            }
            let cost = f64::from(other.load())
                * 100.0
                * (rpm.unwrap_or(0.0) * ratio.factor())
                    .abs()
                    .min(f64::from(MAX_RPM))
                / 30.0;
            let candidate = loss + cost;
            if losses.get(&next).is_none_or(|best| candidate < *best) {
                losses.insert(next, candidate);
                frontier.push(std::cmp::Reverse((candidate.to_bits(), next)));
            }
        }
    }
    nodes
        .into_values()
        .map(|(node, ratio)| {
            (
                node,
                KineticState {
                    rpm: if overloaded {
                        0.0
                    } else {
                        (rpm.unwrap_or(0.0) * ratio.factor()) as f32
                    },
                    overloaded,
                    stress: (load / 100.0) as f32,
                    capacity: (supply / 100.0) as f32,
                    available_capacity: ((supply
                        - losses.get(&node.position).copied().unwrap_or(supply))
                    .max(0.0)
                        / 100.0) as f32,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(position: Cell, material: u16) -> Node {
        Node {
            position,
            material,
            facing: 0,
            secondary: false,
            source_rpm: 0.0,
            source_capacity: if material == 100 { 12800 } else { 3200 },
            belt_load: false,
            links: [None; 2],
        }
    }
    #[test]
    fn a_shaft_controller_keeps_its_axial_ports_and_adds_exactly_one_belt_load() {
        let shaft = Node {
            belt_load: true,
            ..node([0, 0, 0], 144)
        };
        assert_eq!(shaft.load(), 3);
        assert_eq!(
            Node {
                belt_load: false,
                ..shaft
            }
            .load(),
            1
        );
        let neighbor = node([1, 0, 0], 144);
        assert!(connection(shaft, neighbor).is_some());
        assert!(connection(neighbor, shaft).is_some());
        let ribbon = Node {
            secondary: false,
            ..node([0, 0, 0], CONVEYOR)
        };
        assert_eq!(ribbon.load(), 2);
    }
    fn solve(nodes: Vec<Node>) -> Vec<(Node, KineticState)> {
        let map: BTreeMap<_, _> = nodes.into_iter().map(|n| (n.position, n)).collect();
        solve_component(*map.keys().next().unwrap(), &mut 1024, |p| {
            map.get(&p).copied()
        })
    }
    #[test]
    fn invalid_source_speeds_stop_with_finite_diagnostics() {
        for speed in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, 257.0] {
            let mut source = node([0; 3], 100);
            source.source_rpm = speed;
            let state = solve(vec![source, node([1, 0, 0], WOODEN_SHAFT)])[0].1;
            assert!(state.overloaded);
            assert_eq!(state.rpm, 0.0);
            assert_eq!(state.capacity, 0.0);
            assert_eq!(state.available_capacity, 0.0);
            assert!(state.stress.is_finite());
        }
    }
    #[test]
    fn diagnostic_capacity_decreases_along_real_loads_without_slipping_connected_axles() {
        let mut source = node([0, 0, 0], 100);
        source.source_rpm = 30.0;
        let result = solve(vec![
            source,
            node([1, 0, 0], WOODEN_SHAFT),
            node([2, 0, 0], WOODEN_GEAR),
            node([3, 0, 0], 99),
        ]);
        for (index, (_, state)) in result.iter().enumerate() {
            assert_eq!(state.rpm, 30.0);
            assert_eq!(state.capacity, 128.0);
            assert_eq!(state.stress, 22.0);
            assert_eq!(
                state.available_capacity,
                [128.0, 127.0, 126.0, 106.0][index]
            );
        }
    }
    #[test]
    fn diagnostic_capacity_is_order_independent_and_does_not_charge_loops_twice() {
        let mut source = node([0, 0, 0], 100);
        source.source_rpm = 30.0;
        let nodes = vec![
            source,
            node([1, 0, 0], WOODEN_GEAR),
            node([1, 0, 1], WOODEN_GEAR),
            node([1, 1, 1], WOODEN_GEAR),
            node([1, 1, 0], WOODEN_GEAR),
        ];
        let result = solve(nodes.clone());
        assert_eq!(
            result
                .iter()
                .find(|(node, _)| node.position == [1, 1, 1])
                .unwrap()
                .1
                .available_capacity,
            125.0
        );
        let mut reversed = nodes;
        reversed.reverse();
        assert_eq!(
            result
                .iter()
                .map(|(node, state)| (node.position, *state))
                .collect::<Vec<_>>(),
            solve(reversed)
                .iter()
                .map(|(node, state)| (node.position, *state))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            solve(vec![node([0, 0, 0], WOODEN_SHAFT)])[0].1,
            KineticState::default()
        );
    }
    #[test]
    fn crank_offset_centers_all_six_generic_mounts_and_legacy_pairs() {
        let position = [3, 5, -7];
        for direction in DIRECTIONS {
            assert_eq!(
                crank_local_offset(position, Some(offset(position, direction))),
                [0.0, 0.045, -0.015]
            );
            let legacy_peer = offset(position, direction.map(|component| component * 2));
            assert_eq!(
                crank_local_offset(position, Some(legacy_peer)),
                [0.0, 0.045, -0.015]
            );
        }
        for peer in [None, Some(position), Some([4, 6, -6])] {
            assert_eq!(crank_local_offset(position, peer), [0.0; 3]);
        }
        assert_eq!(
            crank_local_offset([i64::MIN; 3], Some([i64::MAX; 3])),
            [0.0; 3]
        );
    }
    #[test]
    fn shafts_drive_gears_and_mesh_reverses_without_duplicate_cycles() {
        let mut crank = node([0, 0, 0], 100);
        crank.source_rpm = 30.0;
        let result = solve(vec![
            crank,
            node([1, 0, 0], WOODEN_GEAR),
            node([1, 0, 1], WOODEN_GEAR),
            node([1, 1, 1], WOODEN_GEAR),
            node([1, 1, 0], WOODEN_GEAR),
        ]);
        assert_eq!(result.len(), 5);
        assert!(result.iter().all(|(_, s)| !s.overloaded));
        assert_eq!(
            result
                .iter()
                .find(|(n, _)| n.position == [1, 0, 1])
                .unwrap()
                .1
                .rpm,
            -30.0
        );
    }
    #[test]
    fn incompatible_sources_and_excess_stress_stop_the_network() {
        let mut left = node([0, 0, 0], 100);
        left.source_rpm = 30.0;
        let mut right = node([2, 0, 0], WATER_WHEEL);
        right.source_rpm = -30.0;
        assert!(
            solve(vec![left, node([1, 0, 0], IRON_SHAFT), right])
                .iter()
                .all(|(_, s)| s.overloaded && s.rpm == 0.0)
        );
        let mut wheel = node([0, 0, 0], WATER_WHEEL);
        wheel.source_rpm = 30.0;
        let mut chain = vec![wheel];
        chain.extend((1..40).map(|x| node([x, 0, 0], WOODEN_SHAFT)));
        assert!(solve(chain).iter().all(|(_, s)| s.overloaded));
    }
    #[test]
    fn unloaded_breaks_stop_transmission_and_oversized_components_fail_closed() {
        let mut wheel = node([0, 0, 0], WATER_WHEEL);
        wheel.source_rpm = 30.0;
        assert_eq!(solve(vec![node([2, 0, 0], IRON_SHAFT), wheel]).len(), 1);
        let map: BTreeMap<_, _> = (0..300)
            .map(|x| {
                let mut n = node([x, 0, 0], IRON_SHAFT);
                if x == 0 {
                    n.source_rpm = 30.0;
                }
                (n.position, n)
            })
            .collect();
        let mut budget = 32;
        let result = solve_component([0, 0, 0], &mut budget, |p| map.get(&p).copied());
        assert!(result.len() <= 33);
        assert!(result.iter().all(|(_, s)| s.overloaded && s.rpm == 0.0));
    }

    #[test]
    fn diagonal_ratios_preserve_fractional_rpm_and_axial_links_preserve_speed() {
        let mut source = node([0, 0, 0], IRON_GEAR);
        source.source_rpm = 30.0;
        let result = solve(vec![
            source,
            node([0, 1, 1], IRON_LARGE_GEAR),
            node([1, 1, 1], IRON_GEAR),
            node([1, 2, 2], IRON_LARGE_GEAR),
        ]);
        let speed = |position| {
            result
                .iter()
                .find(|(n, _)| n.position == position)
                .unwrap()
                .1
                .rpm
        };
        assert_eq!(speed([0, 1, 1]), -15.0);
        assert_eq!(speed([1, 1, 1]), -15.0);
        assert_eq!(speed([1, 2, 2]), 7.5);
        assert!(result.iter().all(|(_, state)| !state.overloaded));
        assert!(connection(node([0, 0, 0], IRON_GEAR), node([0, 1, 0], IRON_LARGE_GEAR)).is_none());
    }

    #[test]
    fn orthogonal_large_gears_follow_contact_quadrant_and_local_axis_sign() {
        let mut source = node([0, 0, 0], IRON_LARGE_GEAR);
        source.source_rpm = 30.0;
        for (facing, expected) in [(4, 30.0), (5, -30.0)] {
            let mut target = node([1, 1, 0], IRON_LARGE_GEAR);
            target.facing = facing;
            let result = solve(vec![source, target]);
            assert_eq!(
                result
                    .iter()
                    .find(|(n, _)| n.position == target.position)
                    .unwrap()
                    .1
                    .rpm,
                expected
            );
        }
        assert!(connection(source, node([0, 1, 1], IRON_LARGE_GEAR)).is_none());
        let mut too_close = node([1, 0, 0], IRON_LARGE_GEAR);
        too_close.facing = 4;
        assert!(connection(source, too_close).is_none());
    }

    #[test]
    fn reduction_gains_torque_speedup_requires_power_and_source_strength_does_not_change_rpm() {
        let mut source = node([0, 0, 0], IRON_GEAR);
        source.source_rpm = 30.0;
        source.source_capacity = 1600;
        let reduced = solve(vec![
            source,
            node([0, 1, 1], IRON_LARGE_GEAR),
            node([1, 1, 1], 99),
        ]);
        assert_eq!(
            reduced
                .iter()
                .find(|(n, _)| n.material == 99)
                .unwrap()
                .1
                .rpm,
            -15.0
        );
        source.material = IRON_LARGE_GEAR;
        let mut speedup = vec![source, node([0, 1, 1], IRON_GEAR), node([1, 1, 1], 99)];
        assert!(solve(speedup.clone()).iter().all(|(_, s)| s.overloaded));
        speedup[0].source_capacity = 5000;
        let supplied = solve(speedup);
        assert_eq!(
            supplied
                .iter()
                .find(|(n, _)| n.material == 99)
                .unwrap()
                .1
                .rpm,
            -60.0
        );
        assert_eq!(
            supplied
                .iter()
                .find(|(n, _)| n.position == [0, 0, 0])
                .unwrap()
                .1
                .rpm,
            30.0
        );
    }

    #[test]
    fn ratio_conflicts_in_closed_loops_stop_instead_of_amplifying_power() {
        let mut source = node([0, 0, 0], IRON_GEAR);
        source.source_rpm = 30.0;
        let result = solve(vec![
            source,
            node([0, 1, 1], IRON_LARGE_GEAR),
            node([1, 1, 1], IRON_GEAR),
            node([1, 0, 1], IRON_GEAR),
            node([1, 0, 0], IRON_GEAR),
        ]);
        assert!(
            result
                .iter()
                .all(|(_, state)| state.overloaded && state.rpm == 0.0)
        );
    }

    #[test]
    fn large_gear_requires_more_source_power_than_a_small_gear_at_the_same_rpm() {
        let mut source = node([0, 0, 0], IRON_SHAFT);
        source.source_rpm = 30.0;
        source.source_capacity = 200;
        let small = solve(vec![source, node([1, 0, 0], IRON_GEAR)]);
        assert!(
            small
                .iter()
                .all(|(_, state)| state.running() && state.rpm == 30.0)
        );
        let large = solve(vec![source, node([1, 0, 0], IRON_LARGE_GEAR)]);
        assert!(
            large
                .iter()
                .all(|(_, state)| state.overloaded && state.rpm == 0.0)
        );
        source.source_capacity = 300;
        let supplied = solve(vec![source, node([1, 0, 0], IRON_LARGE_GEAR)]);
        assert!(
            supplied
                .iter()
                .all(|(_, state)| state.running() && state.rpm == 30.0)
        );
    }

    #[test]
    fn hand_crank_supplies_128_stress_without_old_material_caps() {
        for material in [WOODEN_SHAFT, STONE_SHAFT, IRON_SHAFT] {
            let mut source = node([0, 0, 0], 100);
            source.source_rpm = 30.0;
            let mut train = vec![source];
            train.extend((1..=128).map(|x| node([x, 0, 0], material)));
            assert!(
                solve(train.clone())
                    .iter()
                    .all(|(_, state)| state.running())
            );
            train.push(node([129, 0, 0], material));
            assert!(solve(train).iter().all(|(_, state)| state.overloaded));
        }
        assert_eq!(node([0; 3], WOODEN_GEAR).load(), 1);
        assert_eq!(node([0; 3], WOODEN_LARGE_GEAR).load(), 2);
        assert_eq!(node([0; 3], 99).load(), 20);
    }

    #[test]
    fn owned_long_belt_has_explicit_links_and_charges_only_its_controller() {
        let mut source = node([0, 0, 0], IRON_SHAFT);
        source.source_rpm = 30.0;
        source.source_capacity = 400;
        let mut train = vec![source];
        for z in 1..=32 {
            let mut belt = node([0, 0, z], CONVEYOR);
            belt.secondary = z > 1;
            belt.links = [Some([0, 0, z - 1]), Some([0, 0, z + 1])];
            train.push(belt);
        }
        train.push(node([0, 0, 33], IRON_SHAFT));
        let result = solve(train.clone());
        assert_eq!(result.len(), 34);
        assert!(
            result
                .iter()
                .all(|(_, state)| state.running() && state.rpm == 30.0)
        );
        train[0].source_capacity = 399;
        assert!(solve(train).iter().all(|(_, state)| state.overloaded));
        assert!(connection(node([0, 0, 0], IRON_SHAFT), node([0, 0, 1], CONVEYOR)).is_none());
    }

    fn waterwheel_belt_and_reversed_crusher_branch(alternative: bool) -> Vec<Node> {
        let mut nodes = Vec::new();
        for z in 0..=8 {
            let mut part = node([0, 0, z], if z % 4 == 0 { IRON_SHAFT } else { CONVEYOR });
            part.secondary = z != 1;
            part.links = [
                (z > 0).then_some([0, 0, z - 1]),
                (z < 8).then_some([0, 0, z + 1]),
            ];
            nodes.push(part);
        }
        for (z, facing) in [(0, 2), (4, 0), (8, 2)] {
            let mut wheel = node([-1, 0, z], WATER_WHEEL);
            wheel.facing = facing;
            wheel.source_rpm = if facing == 2 { -30.0 } else { 30.0 };
            wheel.source_capacity = WATER_WHEEL_CAPACITY;
            nodes.push(wheel);
        }
        nodes.extend([
            node([1, 0, 0], IRON_SHAFT),
            node([2, 0, 0], IRON_GEAR),
            node([2, 0, 1], IRON_GEAR),
        ]);
        for z in 1..=3 {
            let mut part = node([1, 0, z], if z == 2 { CONVEYOR } else { IRON_SHAFT });
            part.links = [
                (z > 1).then_some([1, 0, z - 1]),
                (z < 3).then_some([1, 0, z + 1]),
            ];
            nodes.push(part);
        }
        nodes.push(node([2, 0, 3], 99));
        let mut crusher_secondary = node([3, 0, 3], 99);
        crusher_secondary.secondary = true;
        nodes.push(crusher_secondary);
        if alternative {
            // A second reversing gear pair reaches the same output through a
            // compatible loop, beside the main belt's powered guide shaft.
            nodes.extend([
                node([1, 0, 8], IRON_SHAFT),
                node([2, 0, 8], IRON_GEAR),
                node([2, 0, 7], IRON_GEAR),
            ]);
            nodes
                .iter_mut()
                .find(|n| n.position == [1, 0, 3])
                .unwrap()
                .links[1] = Some([1, 0, 4]);
            for z in 4..=7 {
                let mut part = node([1, 0, z], if z == 7 { IRON_SHAFT } else { CONVEYOR });
                part.secondary = z != 4;
                part.links = [Some([1, 0, z - 1]), (z < 7).then_some([1, 0, z + 1])];
                nodes.push(part);
            }
        }
        nodes
    }

    #[test]
    fn parallel_waterwheel_belt_and_short_crusher_branch_have_no_phantom_axial_couplings() {
        for alternative in [false, true] {
            let nodes = waterwheel_belt_and_reversed_crusher_branch(alternative);
            for color in [None, Some(crate::beds::DyeColor::Purple)] {
                let mut train = nodes.clone();
                if let Some(color) = color {
                    for part in train.iter_mut().filter(|n| n.material == CONVEYOR) {
                        part.facing = dyed_belt_facing(part.facing, color);
                    }
                }
                let solved = solve(train.clone());
                assert_eq!(solved.len(), train.len());
                for (part, state) in solved {
                    assert!(!state.overloaded, "{alternative}: {part:?}");
                    let expected = if part.position[0] < 0 && part.facing == 2
                        || part.position[0] > 0 && (1..=7).contains(&part.position[2])
                    {
                        -30.0
                    } else {
                        30.0
                    };
                    assert_eq!(state.rpm, expected, "{alternative}: {part:?}");
                }
                let ribbon = train
                    .iter()
                    .find(|n| n.position == [0, 0, 2])
                    .copied()
                    .unwrap();
                let branch = train
                    .iter()
                    .find(|n| n.position == [1, 0, 2])
                    .copied()
                    .unwrap();
                assert!(connection(ribbon, branch).is_none());
                let mut gearbox = node([1, 0, 2], GEARBOX_HORIZONTAL);
                assert!(connection(ribbon, gearbox).is_none());
                gearbox.facing = 1;
                assert!(connection(gearbox, ribbon).is_none());
            }
        }
    }

    #[test]
    fn real_opposing_waterwheel_sources_still_stop_the_branch_network() {
        let mut train = waterwheel_belt_and_reversed_crusher_branch(true);
        train
            .iter_mut()
            .find(|n| n.position == [-1, 0, 4])
            .unwrap()
            .source_rpm = -30.0;
        assert!(
            solve(train)
                .iter()
                .all(|(_, state)| state.overloaded && state.rpm == 0.0)
        );
    }

    #[test]
    fn belt_dyes_pack_without_changing_direction_or_power_connections() {
        assert_eq!(BELT_FACING_COUNT, 84);
        for facing in 0..6 {
            let pose = belt_pose(facing);
            assert_eq!(belt_color(pose), None);
            let rgb = dyed_belt_facing(pose, crate::beds::DyeColor::Rgb([7, 43, 198]));
            assert_eq!(belt_facing(rgb), facing);
            assert_eq!(belt_color(rgb), None);
            for color in crate::beds::DyeColor::ALL {
                let raw = dyed_belt_facing(pose, color);
                assert!(raw < BELT_FACING_COUNT);
                assert_eq!(belt_facing(raw), facing);
                assert_eq!(belt_color(raw), Some(color));
                let mut plain = node([0; 3], CONVEYOR);
                plain.facing = pose;
                let mut dyed = plain;
                dyed.facing = raw;
                let mut peer = node(axis(facing), IRON_SHAFT);
                peer.facing = facing;
                assert_eq!(connection(dyed, peer), connection(plain, peer));
            }
        }
        assert_eq!(belt_color(BELT_FACING_COUNT), None);
    }

    #[test]
    fn gearboxes_connect_only_their_four_real_ports_and_preserve_power_and_direction() {
        for material in [GEARBOX_HORIZONTAL, GEARBOX_VERTICAL] {
            for facing in 0..4 {
                let mut gearbox = node([0; 3], material);
                gearbox.facing = facing;
                let ports = gearbox_ports(material, facing);
                for port in DIRECTIONS {
                    let mut shaft = node(port, IRON_SHAFT);
                    shaft.facing = self::facing(port).unwrap();
                    let connection = connection(gearbox, shaft);
                    assert_eq!(connection.is_some(), ports.contains(&port));
                    if let Some(connection) = connection {
                        assert_eq!(connection.speed_shift, 0);
                        assert_eq!(
                            connection.direction,
                            gearbox_port_factor(material, facing, port).unwrap()
                                * port.into_iter().sum::<i64>() as i8
                        );
                        assert_eq!(self::connection(shaft, gearbox), Some(connection));
                    }
                }
                let mut source = node(ports[0], IRON_SHAFT);
                source.facing = self::facing(ports[0]).unwrap();
                source.source_rpm = 30.0;
                source.source_capacity = 400;
                let mut output = node(ports[2], IRON_SHAFT);
                output.facing = self::facing(ports[2]).unwrap();
                let supplied = solve(vec![source, gearbox, output]);
                assert!(
                    supplied
                        .iter()
                        .all(|(_, state)| state.running() && state.rpm.abs() == 30.0)
                );
                assert_eq!(
                    supplied
                        .iter()
                        .find(|(part, _)| part.position == output.position)
                        .unwrap()
                        .1
                        .rpm,
                    -30.0
                );
                source.source_capacity = 399;
                assert!(
                    solve(vec![source, gearbox, output])
                        .iter()
                        .all(|(_, state)| state.overloaded)
                );
            }
        }
    }
}
