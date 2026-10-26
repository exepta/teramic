//! Bounded cave-only searches over the existing swept ground graph.
use super::super::{Habitat, Kind, distance};
use std::collections::{BinaryHeap, HashMap, VecDeque};

pub const NODE_LIMIT: usize = 512;
pub const SENSE_DISTANCE: f64 = 50.0;
pub const DRAG_DISTANCE: f64 = 40.0;

pub struct CaveRoute {
    pub points: VecDeque<[f64; 3]>,
    pub length: f64,
    pub expanded: usize,
}
pub struct SearchBudget {
    remaining: usize,
    cover: HashMap<[i64; 3], bool>,
    pub nodes: usize,
    pub roof_probes: usize,
}
impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            remaining: NODE_LIMIT,
            cover: HashMap::new(),
            nodes: 0,
            roof_probes: 0,
        }
    }
}
impl SearchBudget {
    fn covered(&mut self, h: &Habitat, p: [f64; 3]) -> bool {
        let key = p.map(|v| v.floor() as i64);
        *self.cover.entry(key).or_insert_with(|| {
            self.roof_probes += 1;
            covered(h, p, Kind::Stalker.size()[1])
        })
    }
    fn allocate(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.nodes += 1;
        true
    }
}

/// Unknown roof cells are barriers, not daylight openings. Three natural rock
/// layers distinguish underground cover from leaves and a normal timber roof.
pub fn covered(h: &Habitat, p: [f64; 3], height: f64) -> bool {
    if !super::valid_point(p) || !height.is_finite() || !(0.0..=24.0).contains(&height) {
        return false;
    }
    let base = [
        p[0].floor() as i64,
        (p[1] + height).floor() as i64,
        p[2].floor() as i64,
    ];
    for dy in 0..=48 {
        let cell = [base[0], base[1] + dy, base[2]];
        let Some(raw) = h.block(cell) else {
            return false;
        };
        if terra_voxel::MaterialId::from_raw(raw).is_some_and(terra_voxel::MaterialId::is_solid) {
            return (0..3).all(|up| {
                h.block([cell[0], cell[1] + up, cell[2]])
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_some_and(terra_voxel::MaterialId::is_natural_ground)
            });
        }
    }
    false
}

pub fn route(h: &Habitat, start: [f64; 3], target: [f64; 3]) -> Option<CaveRoute> {
    route_budgeted(h, start, target, &mut SearchBudget::default())
}
pub fn route_budgeted(
    h: &Habitat,
    start: [f64; 3],
    target: [f64; 3],
    budget: &mut SearchBudget,
) -> Option<CaveRoute> {
    if !super::valid_point(start)
        || !super::valid_point(target)
        || distance(start, target) > SENSE_DISTANCE.powi(2)
    {
        return None;
    }
    search(h, start, Some(target), budget)
}

/// A grab is only possible if a real, non-circular 40m route has been found.
/// Lower destinations win ties; no teleport or fabricated deep endpoint.
pub fn deeper(h: &Habitat, start: [f64; 3]) -> Option<CaveRoute> {
    deeper_budgeted(h, start, &mut SearchBudget::default())
}
pub fn deeper_budgeted(
    h: &Habitat,
    start: [f64; 3],
    budget: &mut SearchBudget,
) -> Option<CaveRoute> {
    if !super::valid_point(start) {
        return None;
    }
    // Directed A* crosses broad chambers without flooding their entire floor.
    // The bounded exploration fallback handles winding galleries instead.
    for [dx, dz] in [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]] {
        let target = [start[0] + dx * 42.0, start[1], start[2] + dz * 42.0];
        if let Some(target) = h.ground(target, Kind::Stalker)
            && budget.covered(h, target)
            && let Some(route) = route_budgeted(h, start, target, budget)
            && route.length >= DRAG_DISTANCE
        {
            return Some(route);
        }
    }
    search(h, start, None, budget)
}

fn search(
    h: &Habitat,
    start: [f64; 3],
    target: Option<[f64; 3]>,
    budget: &mut SearchBudget,
) -> Option<CaveRoute> {
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.navigation.cave_route");
    let kind = Kind::Stalker;
    if !budget.allocate() || !h.clear(start, kind.size(), false) || !budget.covered(h, start) {
        return None;
    }
    let mut nodes = vec![(start, 0usize, 0.0_f64)];
    let mut visited = HashMap::from([(start.map(|v| v.floor() as i64), 0usize)]);
    let heuristic = |p| target.map_or(0, |target| (distance(p, target).sqrt() * 100.0) as u32);
    let mut queue = BinaryHeap::from([(std::cmp::Reverse(heuristic(start)), 0usize)]);
    let mut goal = None;
    let mut expanded = 0;
    while let Some((_, index)) = queue.pop() {
        expanded += 1;
        let (current, _, length) = nodes[index];
        if target.is_some_and(|target| {
            distance(current, target) <= 1.2_f64.powi(2)
                && length + distance(current, target).sqrt() <= SENSE_DISTANCE
                && h.walk_step(
                    current,
                    target,
                    kind,
                    (current[0] - target[0]).atan2(current[2] - target[2]),
                )
                .is_some()
                && budget.covered(h, target)
        }) {
            goal = Some(index);
            break;
        }
        if target.is_none() && length >= DRAG_DISTANCE && current[1] <= start[1] + 0.1 {
            if goal.is_none_or(|old| current[1] < nodes[old].0[1]) {
                goal = Some(index);
            }
            if current[1] <= start[1] - 2.0 {
                break;
            }
            continue;
        }
        for [dx, dz] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
            if budget.remaining == 0 {
                break;
            }
            let probe = [current[0] + dx, current[1], current[2] + dz];
            if visited.contains_key(&probe.map(|v| v.floor() as i64)) {
                continue;
            }
            let yaw = (-dx).atan2(-dz);
            let Some(next) = h.walk_step(current, probe, kind, yaw) else {
                continue;
            };
            let key = next.map(|v| v.floor() as i64);
            let cost = length + distance(current, next).sqrt();
            if visited.contains_key(&key) || cost > SENSE_DISTANCE || !budget.covered(h, next) {
                continue;
            }
            let next_index = nodes.len();
            if !budget.allocate() {
                break;
            }
            nodes.push((next, index, cost));
            visited.insert(key, next_index);
            queue.push((
                std::cmp::Reverse((cost * 100.0) as u32 + heuristic(next)),
                next_index,
            ));
        }
    }
    let mut index = goal?;
    let mut length = nodes[index].2;
    let mut points = VecDeque::new();
    if let Some(target) = target {
        length += distance(nodes[index].0, target).sqrt();
        points.push_front(target);
    }
    while index != 0 {
        points.push_front(nodes[index].0);
        index = nodes[index].1;
    }
    Some(CaveRoute {
        points,
        length,
        expanded,
    })
}
