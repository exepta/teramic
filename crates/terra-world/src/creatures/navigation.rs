//! Local voxel graph navigation. No static navmesh becomes stale after block edits.
use super::{Creature, Habitat, Kind, distance};
use std::collections::{BinaryHeap, HashMap, VecDeque};

const NODE_BUDGET: usize = 96;
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;
    std::thread_local! {
        static PATH_CALLS: Cell<usize> = const { Cell::new(0) };
    }
    pub(super) fn record_path() {
        PATH_CALLS.set(PATH_CALLS.get() + 1);
    }
    pub(crate) fn reset_path_calls() {
        PATH_CALLS.set(0);
    }
    pub(crate) fn path_calls() -> usize {
        PATH_CALLS.get()
    }
}
#[derive(Default)]
pub struct Navigation {
    routes: HashMap<u64, Route>,
    horse_probes: HashMap<u64, f64>,
}
impl Navigation {
    /// A blocked land animal requests a bounded graph search on a later budgeted tick.
    pub fn needs_search(&self, id: u64, age: f64) -> bool {
        self.routes
            .get(&id)
            .is_some_and(|route| route.ground_blocked && route.expires <= age)
    }
    pub(super) fn forget(&mut self, id: u64) {
        self.routes.remove(&id);
        self.horse_probes.remove(&id);
    }
    pub(super) fn horse_probe(&mut self, id: u64, age: f64) -> bool {
        if self.horse_probes.get(&id).is_some_and(|&next| next > age) {
            return false;
        }
        if self.horse_probes.len() >= terra_core::creatures::MAX_CREATURES
            && !self.horse_probes.contains_key(&id)
        {
            return false;
        }
        self.horse_probes.insert(id, age + 0.35);
        true
    }
    pub(super) fn horse_jumped(&mut self, id: u64, age: f64) {
        self.horse_probes.insert(id, age + 2.5);
    }
    pub(super) fn ground_unblocked(&mut self, id: u64) {
        self.routes.remove(&id);
    }
    pub(super) fn following_ground_route(&self, id: u64) -> bool {
        self.routes
            .get(&id)
            .is_some_and(|route| route.ground_blocked && !route.points.is_empty())
    }
}
struct Route {
    target: [f64; 3],
    points: VecDeque<[f64; 3]>,
    expires: f64,
    ground_blocked: bool,
}

fn traversable(h: &Habitat, start: [f64; 3], end: [f64; 3], kind: Kind) -> bool {
    let steps = (distance(start, end).sqrt() / 0.18).ceil().max(1.0) as usize;
    (1..=steps).all(|step| {
        let p =
            std::array::from_fn(|i| start[i] + (end[i] - start[i]) * step as f64 / steps as f64);
        h.clear(p, kind.size(), kind.is_aquatic())
    })
}

/// Bounded A* detour; edges are swept with the species' actual occupied volume.
/// Returns the first waypoint, not a teleport or a path through unloaded cells.
pub fn waypoint(h: &Habitat, start: [f64; 3], target: [f64; 3], kind: Kind) -> Option<[f64; 3]> {
    path(h, start, target, kind).front().copied()
}
pub(super) fn ground_waypoint(
    a: &Creature,
    h: &Habitat,
    target: [f64; 3],
    search: bool,
    navigation: Option<&mut Navigation>,
) -> Option<[f64; 3]> {
    let Some(nav) = navigation else {
        return search
            .then(|| waypoint(h, a.position, target, a.kind))
            .flatten();
    };
    if nav.routes.len() >= terra_core::creatures::MAX_CREATURES {
        nav.routes.retain(|_, route| route.expires > a.age);
    }
    let route = nav.routes.entry(a.id).or_insert_with(|| Route {
        target,
        points: VecDeque::new(),
        expires: 0.0,
        ground_blocked: true,
    });
    route.ground_blocked = true;
    let had_points = !route.points.is_empty();
    while route
        .points
        .front()
        .is_some_and(|p| distance(*p, a.position) < 0.08)
    {
        route.points.pop_front();
    }
    if distance(route.target, target) > 2.25 {
        route.points.clear();
        route.expires = 0.0;
    } else if had_points && route.points.is_empty() {
        route.expires = 0.0;
    }
    if route
        .points
        .front()
        .is_some_and(|p| h.ground(*p, a.kind).is_none())
    {
        route.points.clear();
        route.expires = 0.0;
    }
    if search && route.expires <= a.age {
        route.points = path(h, a.position, target, a.kind);
        route.target = target;
        route.expires = a.age + if route.points.is_empty() { 0.8 } else { 2.0 };
    }
    route.points.front().copied()
}
fn path(h: &Habitat, start: [f64; 3], target: [f64; 3], kind: Kind) -> VecDeque<[f64; 3]> {
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.navigation.path");
    #[cfg(test)]
    testing::record_path();
    let flying = kind == Kind::Bee || kind.is_aquatic();
    let spacing = if flying { 0.65 } else { 1.0 };
    let mut nodes = vec![(start, 0usize, 0u32)];
    let mut visited = HashMap::from([([0i32; 3], 0usize)]);
    let mut queue = BinaryHeap::from([(std::cmp::Reverse(0u32), 0usize, [0i32; 3])]);
    let heuristic = |p| (distance(p, target).sqrt() * 100.0) as u32;
    let mut best = 0;
    while let Some((_, index, cell)) = queue.pop() {
        if heuristic(nodes[index].0) < heuristic(nodes[best].0) {
            best = index;
        }
        if index != 0
            && (distance(nodes[index].0, target) < spacing * spacing
                || (distance(nodes[index].0, start) > 2.0
                    && distance(nodes[index].0, target) < 64.0
                    && traversable(h, nodes[index].0, target, kind)))
        {
            best = index;
            break;
        }
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 0, 1],
            [0, 0, -1],
            [0, 1, 0],
            [0, -1, 0],
        ] {
            if nodes.len() >= NODE_BUDGET {
                break;
            }
            if !flying && d[1] != 0 {
                continue;
            }
            let next_cell = std::array::from_fn(|i| cell[i] + d[i]);
            if next_cell.iter().any(|v: &i32| v.abs() > 6) || visited.contains_key(&next_cell) {
                continue;
            }
            let p = std::array::from_fn(|i| start[i] + f64::from(next_cell[i]) * spacing);
            let candidate = if flying {
                traversable(h, nodes[index].0, p, kind).then_some(p)
            } else {
                h.ground([p[0], nodes[index].0[1], p[2]], kind)
            };
            let Some(p) = candidate else {
                continue;
            };
            let cost = nodes[index].2 + (spacing * 100.0) as u32;
            let next = nodes.len();
            visited.insert(next_cell, next);
            nodes.push((p, index, cost));
            // Weighted A* spends the small local budget on useful detours,
            // rather than flooding equally cheap air above/below a tree trunk.
            queue.push((std::cmp::Reverse(cost + heuristic(p) * 2), next, next_cell));
        }
    }
    let mut result = VecDeque::new();
    while best != 0 {
        result.push_front(nodes[best].0);
        best = nodes[best].1;
    }
    result
}

pub(super) fn fly_cached(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    navigation: Option<&mut Navigation>,
) -> bool {
    let before = a.position;
    let length = distance(before, target).sqrt();
    if length < 0.001 {
        a.velocity = [0.0; 3];
        return true;
    }
    let look =
        std::array::from_fn(|i| before[i] + (target[i] - before[i]) / length * length.min(1.4));
    let next_target = if traversable(h, before, look, a.kind) {
        Some(target)
    } else if let Some(nav) = navigation {
        if nav.routes.len() >= terra_core::creatures::MAX_CREATURES {
            nav.routes.retain(|_, r| r.expires > a.age);
        }
        let route = nav.routes.entry(a.id).or_insert_with(|| Route {
            target,
            points: VecDeque::new(),
            expires: 0.0,
            ground_blocked: false,
        });
        let had_points = !route.points.is_empty();
        while route
            .points
            .front()
            .is_some_and(|p| distance(*p, before) < 0.07)
        {
            route.points.pop_front();
        }
        if route.expires <= a.age
            || distance(route.target, target) > 0.25
            || (had_points && route.points.is_empty())
            || route
                .points
                .front()
                .is_some_and(|p| !traversable(h, before, *p, a.kind))
        {
            let points = path(h, before, target, a.kind);
            let lifetime = if points.is_empty() { 0.4 } else { 2.0 };
            *route = Route {
                target,
                points,
                expires: a.age + lifetime,
                ground_blocked: false,
            };
        }
        route.points.front().copied()
    } else {
        waypoint(h, before, target, a.kind)
    };
    let Some(next_target) = next_target else {
        a.velocity = [0.0; 3];
        return false;
    };
    let length = distance(before, next_target).sqrt().max(0.001);
    let speed = a.kind.speed().min(length / f64::from(dt));
    let velocity: [f32; 3] = std::array::from_fn(|i| {
        let desired = ((next_target[i] - before[i]) / length * speed) as f32;
        a.velocity[i] + (desired - a.velocity[i]) * (dt * 8.0).min(1.0)
    });
    let proposed = std::array::from_fn(|i| before[i] + f64::from(velocity[i] * dt));
    if traversable(h, before, proposed, a.kind) {
        a.position = proposed;
    } else {
        let proposed = std::array::from_fn(|i| {
            before[i] + (next_target[i] - before[i]) / length * speed * f64::from(dt)
        });
        if !traversable(h, before, proposed, a.kind) {
            a.velocity = [0.0; 3];
            return false;
        }
        a.position = proposed;
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
    if a.velocity[0].abs() + a.velocity[2].abs() > 0.01 {
        a.yaw = (-a.velocity[0]).atan2(-a.velocity[2]);
    }
    true
}

/// Spatial bins bound separation work independently of the total saved population.
pub fn separate(
    pop: &mut terra_core::creatures::CreaturePopulation,
    h: &Habitat,
    ids: &[u64],
    dt: f32,
) {
    let mut bins = HashMap::<[i64; 3], Vec<(u64, [f64; 3], Kind)>>::new();
    let cell = |p: [f64; 3]| p.map(|v| (v / 2.0).floor() as i64);
    for id in ids {
        if let Some(a) = pop
            .animals
            .get(id)
            .filter(|a| a.behavior != super::Behavior::InNest)
        {
            bins.entry(cell(a.position))
                .or_default()
                .push((a.id, a.position, a.kind));
        }
    }
    for id in ids {
        let Some(a) = pop.animals.get_mut(id).filter(|a| {
            !matches!(
                a.behavior,
                super::Behavior::InNest
                    | super::Behavior::Sleep
                    | super::Behavior::Climb
                    | super::Behavior::Hold
            )
        }) else {
            continue;
        };
        let center = cell(a.position);
        let mut push = [0.0; 3];
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(neighbors) =
                        bins.get(&[center[0] + x, center[1] + y, center[2] + z])
                    {
                        for &(other, p, kind) in neighbors.iter().take(32) {
                            if other == a.id || (kind == Kind::Bee) != (a.kind == Kind::Bee) {
                                continue;
                            }
                            let radius = if a.kind == Kind::Bee {
                                0.24
                            } else {
                                (kind.size()[0] + a.kind.size()[0]) * 0.45
                            };
                            let d = distance(p, a.position).sqrt();
                            if d < radius {
                                let direction = if d < 0.001 {
                                    [if a.id < other { -1.0 } else { 1.0 }, 0.0, 0.0]
                                } else {
                                    std::array::from_fn(|i| (a.position[i] - p[i]) / d)
                                };
                                for i in 0..3 {
                                    push[i] += direction[i] * (radius - d) * f64::from(dt * 3.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        let proposed = std::array::from_fn(|i| a.position[i] + push[i].clamp(-0.06, 0.06));
        let next = if a.kind.is_aquatic() {
            h.clear(proposed, a.size(), true).then_some(proposed)
        } else if a.kind == Kind::Horse && a.behavior == super::Behavior::Leap {
            // Separation must not project a ballistic jump back onto the floor.
            let proposed = [proposed[0], a.position[1], proposed[2]];
            h.clear_swimmer_rotated(proposed, a.size(), f64::from(a.yaw))
                .then_some(proposed)
        } else if a.kind == Kind::Bee {
            traversable(h, a.position, proposed, a.kind).then_some(proposed)
        } else {
            h.ground_rotated(proposed, a.kind, f64::from(a.yaw))
        };
        if let Some(next) = next {
            a.position = next;
        }
    }
}
