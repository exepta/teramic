//! Local voxel graph navigation. No static navmesh becomes stale after block edits.
pub mod caves;
mod ground;
#[cfg(test)]
mod ground_tests;
use super::{Creature, Habitat, Kind, distance};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use terra_core::creatures::intentions::{CreatureIntent, IntentReason};

pub(super) const NODE_BUDGET: usize = 96;
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;
    std::thread_local! {
        static PATH_CALLS: Cell<usize> = const { Cell::new(0) };
    }
    pub(in crate::creatures) fn record_path() {
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
    pub(super) surface_routes: HashMap<u64, super::spiders::SurfaceRoute>,
    horse_probes: HashMap<u64, f64>,
    intentions: HashMap<u64, CreatureIntent>,
    escapes: HashMap<u64, ([f64; 3], f64)>,
    sightings: HashMap<u64, ([f64; 3], f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundRouteState {
    Unplanned,
    Following,
    AwaitingSearch,
    Blocked,
    Arrived,
}
impl Navigation {
    pub(super) fn see_enemy(&mut self, a: &Creature, position: [f64; 3]) {
        if self.sightings.len() < terra_core::creatures::MAX_CREATURES
            || self.sightings.contains_key(&a.id)
        {
            self.sightings.insert(a.id, (position, a.age + 4.0));
        }
    }
    pub(super) fn remembered_enemy(&mut self, a: &Creature) -> Option<[f64; 3]> {
        let &(position, until) = self.sightings.get(&a.id)?;
        if a.age >= until || distance(a.position, position) > 48.0_f64.powi(2) {
            self.sightings.remove(&a.id);
            None
        } else {
            Some(position)
        }
    }
    /// Drop cached paths after a changed goal, permission or real door transaction.
    pub fn invalidate(&mut self, id: u64) {
        self.routes.remove(&id);
        self.surface_routes.remove(&id);
    }
    pub fn ground_state(&self, id: u64, age: f64) -> GroundRouteState {
        let Some(route) = self.routes.get(&id) else {
            return GroundRouteState::Unplanned;
        };
        if route.arrived {
            GroundRouteState::Arrived
        } else if !route.ground_blocked {
            GroundRouteState::Unplanned
        } else if !route.points.is_empty() {
            GroundRouteState::Following
        } else if route.expires <= age {
            GroundRouteState::AwaitingSearch
        } else {
            GroundRouteState::Blocked
        }
    }
    pub fn begin_tick(&mut self, id: u64) {
        self.intentions.remove(&id);
    }
    pub(super) fn intend(
        &mut self,
        a: &Creature,
        reason: IntentReason,
        destination: Option<[f64; 3]>,
    ) {
        if self.intentions.len() >= terra_core::creatures::MAX_CREATURES
            && !self.intentions.contains_key(&a.id)
        {
            return;
        }
        self.intentions.insert(
            a.id,
            CreatureIntent {
                id: a.id,
                reason,
                destination,
                path: Vec::new(),
                blocked: false,
            },
        );
    }
    pub fn inspection(&self, a: &Creature) -> CreatureIntent {
        let mut view = self
            .intentions
            .get(&a.id)
            .cloned()
            .unwrap_or_else(|| CreatureIntent::state(a));
        if let Some(route) = self.routes.get(&a.id) {
            view.destination = Some(route.target);
            view.path = route.points.iter().copied().take(8).collect();
            view.blocked = route.ground_blocked && route.points.is_empty();
        }
        if let Some(route) = self.surface_routes.get(&a.id) {
            view.destination = Some(route.target);
            view.path = route.points.iter().take(8).map(|p| p.position).collect();
            view.blocked = route.blocked && route.points.is_empty();
        }
        view
    }
    /// A blocked land animal requests a bounded graph search on a later budgeted tick.
    pub fn needs_search(&self, id: u64, age: f64) -> bool {
        self.routes.get(&id).is_some_and(|route| {
            route.ground_blocked && route.points.is_empty() && route.expires <= age
        }) || self
            .surface_routes
            .get(&id)
            .is_some_and(|route| route.blocked && route.expires <= age)
    }
    pub(super) fn forget(&mut self, id: u64) {
        self.routes.remove(&id);
        self.surface_routes.remove(&id);
        self.horse_probes.remove(&id);
        self.escapes.remove(&id);
        self.sightings.remove(&id);
    }
    pub(super) fn escape_via(&mut self, id: u64, position: [f64; 3], age: f64) {
        if self.escapes.len() < terra_core::creatures::MAX_CREATURES {
            self.escapes.insert(id, (position, age + 4.0));
        }
    }
    pub(super) fn escape(&mut self, a: &Creature) -> Option<[f64; 3]> {
        let &(position, until) = self.escapes.get(&a.id)?;
        if until <= a.age || (position[0] - a.position[0]).hypot(position[2] - a.position[2]) < 0.15
        {
            self.escapes.remove(&a.id);
            self.routes.remove(&a.id);
            None
        } else {
            Some(position)
        }
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
    pub(super) fn stalled(&self, id: u64, age: f64) -> bool {
        self.routes
            .get(&id)
            .is_some_and(|route| route.ground_blocked && age - route.stuck_since > 2.5)
    }
}
struct Route {
    target: [f64; 3],
    points: VecDeque<[f64; 3]>,
    expires: f64,
    ground_blocked: bool,
    last_progress: [f64; 3],
    progress_at: f64,
    stuck_since: f64,
    arrived: bool,
}

/// Horizontal proximity alone cannot finish a route on another floor or across a
/// last wall/gap. Arrival requires the actual body and the requested standing Y.
pub(super) fn ground_arrived(a: &Creature, h: &Habitat, target: [f64; 3], tolerance: f64) -> bool {
    valid_point(a.position)
        && valid_point(target)
        && tolerance.is_finite()
        && (0.0..=2.0).contains(&tolerance)
        && (a.position[0] - target[0]).hypot(a.position[2] - target[2]) <= tolerance
        && (a.position[1] - target[1]).abs() <= 0.125
        && ground::edge(h, a.position, target, a.size(), &|_| true).is_some()
}

fn traversable(h: &Habitat, start: [f64; 3], end: [f64; 3], kind: Kind) -> bool {
    traversable_sized(h, start, end, kind, kind.size())
}
fn traversable_sized(
    h: &Habitat,
    start: [f64; 3],
    end: [f64; 3],
    kind: Kind,
    size: [f64; 3],
) -> bool {
    let length = distance(start, end).sqrt();
    if !valid_point(start) || !valid_point(end) || length > 11.52 {
        return false;
    }
    let steps = (length / 0.18).ceil().clamp(1.0, 64.0) as usize;
    (1..=steps).all(|step| {
        let p =
            std::array::from_fn(|i| start[i] + (end[i] - start[i]) * step as f64 / steps as f64);
        h.clear(p, size, kind.is_aquatic())
    })
}

/// Bounded A* detour; edges are swept with the species' actual occupied volume.
/// Returns the first waypoint, not a teleport or a path through unloaded cells.
pub fn waypoint(h: &Habitat, start: [f64; 3], target: [f64; 3], kind: Kind) -> Option<[f64; 3]> {
    path(h, start, target, kind, kind.size()).front().copied()
}
pub(super) fn ground_waypoint(
    a: &Creature,
    h: &Habitat,
    target: [f64; 3],
    search: bool,
    navigation: Option<&mut Navigation>,
) -> Option<[f64; 3]> {
    ground_waypoint_with_access(a, h, target, search, navigation, |_| true)
}

pub(super) fn ground_waypoint_with_access(
    a: &Creature,
    h: &Habitat,
    target: [f64; 3],
    search: bool,
    navigation: Option<&mut Navigation>,
    allowed: impl Fn([f64; 3]) -> bool,
) -> Option<[f64; 3]> {
    if !valid_point(a.position) || !valid_point(target) || !a.age.is_finite() {
        if let Some(nav) = navigation {
            nav.invalidate(a.id);
        }
        return None;
    }
    let arrived = ground_arrived(a, h, target, 0.12) && allowed(a.position) && allowed(target);
    let Some(nav) = navigation else {
        return (!arrived && search)
            .then(|| {
                ground::search(h, a.position, target, a.size(), &allowed)
                    .points
                    .front()
                    .copied()
            })
            .flatten();
    };
    if nav.routes.len() >= terra_core::creatures::MAX_CREATURES && !nav.routes.contains_key(&a.id) {
        nav.routes.retain(|_, route| route.expires > a.age);
        if nav.routes.len() >= terra_core::creatures::MAX_CREATURES {
            return None;
        }
    }
    let route = nav.routes.entry(a.id).or_insert_with(|| Route {
        target,
        points: VecDeque::new(),
        expires: 0.0,
        ground_blocked: true,
        last_progress: a.position,
        progress_at: a.age,
        stuck_since: a.age,
        arrived: false,
    });
    // A moving quarry can drift within the same approach while the mob is
    // still on a viable detour. Keep the path until that drift accumulates.
    let tracking = matches!(a.behavior, super::Behavior::Hunt | super::Behavior::Attack)
        && !route.points.is_empty();
    let goal_tolerance: f64 = if tracking { 1.5 } else { 0.35 };
    if distance(route.target, target) > goal_tolerance.powi(2)
        || (route.target[1] - target[1]).abs() > 0.125
    {
        route.target = target;
        route.points.clear();
        route.expires = 0.0;
        route.last_progress = a.position;
        route.progress_at = a.age;
        route.stuck_since = a.age;
        route.arrived = false;
    }
    if arrived {
        route.target = target;
        route.points.clear();
        route.ground_blocked = false;
        route.arrived = true;
        route.expires = a.age + 0.8;
        return None;
    }
    route.arrived = false;
    if distance(route.last_progress, a.position) > 0.08_f64.powi(2) {
        route.last_progress = a.position;
        route.progress_at = a.age;
        route.stuck_since = a.age;
    } else if a.age - route.progress_at > 1.2 && !route.points.is_empty() {
        route.points.clear();
        route.expires = 0.0;
        route.progress_at = a.age;
    }
    route.ground_blocked = true;
    let had_points = !route.points.is_empty();
    while route
        .points
        .front()
        .is_some_and(|point| distance(*point, a.position) < 0.14_f64.powi(2))
    {
        route.points.pop_front();
    }
    if had_points && route.points.is_empty() {
        route.expires = 0.0;
    }
    // Recheck the full next edge, not just its endpoint. Edits and section
    // eviction can obstruct its middle while the waypoint itself stays clear.
    if route
        .points
        .front()
        .is_some_and(|point| ground::edge(h, a.position, *point, a.size(), &allowed).is_none())
    {
        route.points.clear();
        route.expires = 0.0;
    }
    if search && route.expires <= a.age {
        route.points = ground::search(h, a.position, target, a.size(), &allowed).points;
        route.target = target;
        route.expires = a.age + if route.points.is_empty() { 0.8 } else { 2.0 };
    }
    route.points.front().copied()
}

fn path(
    h: &Habitat,
    start: [f64; 3],
    target: [f64; 3],
    kind: Kind,
    size: [f64; 3],
) -> VecDeque<[f64; 3]> {
    if !valid_point(start) || !valid_point(target) {
        return VecDeque::new();
    }
    if !kind.is_flying() && !kind.is_aquatic() {
        return ground::search(h, start, target, size, &|_| true).points;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.navigation.path");
    #[cfg(test)]
    testing::record_path();
    let flying = kind.is_flying() || kind.is_aquatic();
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
                    && traversable_sized(h, nodes[index].0, target, kind, size)))
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
                traversable_sized(h, nodes[index].0, p, kind, size).then_some(p)
            } else {
                h.walk_step_sized(
                    nodes[index].0,
                    [p[0], nodes[index].0[1], p[2]],
                    size,
                    (nodes[index].0[0] - p[0]).atan2(nodes[index].0[2] - p[2]),
                )
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
            queue.push((
                std::cmp::Reverse(cost.saturating_add(heuristic(p).saturating_mul(2))),
                next,
                next_cell,
            ));
        }
    }
    let mut result = VecDeque::new();
    while best != 0 {
        result.push_front(nodes[best].0);
        best = nodes[best].1;
    }
    smooth_route(start, &mut result, |from, to| {
        traversable_sized(h, from, to, kind, size)
    });
    result
}

/// Skip short runs of grid corners if the actual body can cross directly.
fn smooth_route(
    mut from: [f64; 3],
    points: &mut VecDeque<[f64; 3]>,
    can_cross: impl Fn([f64; 3], [f64; 3]) -> bool,
) {
    if points.len() < 2 {
        return;
    }
    let mut remaining = std::mem::take(points);
    while !remaining.is_empty() {
        let skip = (1..remaining.len().min(4))
            .rev()
            .find(|&i| can_cross(from, remaining[i]))
            .unwrap_or(0);
        from = remaining[skip];
        remaining.drain(..=skip);
        points.push_back(from);
    }
}

pub(super) fn fly_cached(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    mut navigation: Option<&mut Navigation>,
) -> bool {
    if !dt.is_finite() || dt <= 0.0 || dt > 0.25 || !valid_point(a.position) || !valid_point(target)
    {
        return false;
    }
    if let Some(nav) = navigation.as_deref_mut() {
        nav.intend(a, CreatureIntent::state(a).reason, Some(target));
    }
    let before = a.position;
    let length = distance(before, target).sqrt();
    if length < 0.001 {
        a.velocity = [0.0; 3];
        return true;
    }
    let look =
        std::array::from_fn(|i| before[i] + (target[i] - before[i]) / length * length.min(1.4));
    let next_target = if traversable_sized(h, before, look, a.kind, a.size()) {
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
            last_progress: a.position,
            progress_at: a.age,
            stuck_since: a.age,
            arrived: false,
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
                .is_some_and(|p| !traversable_sized(h, before, *p, a.kind, a.size()))
        {
            let points = path(h, before, target, a.kind, a.size());
            let lifetime = if points.is_empty() { 0.4 } else { 2.0 };
            *route = Route {
                target,
                points,
                expires: a.age + lifetime,
                ground_blocked: false,
                last_progress: a.position,
                progress_at: a.age,
                stuck_since: a.age,
                arrived: false,
            };
        }
        route.points.front().copied()
    } else {
        path(h, before, target, a.kind, a.size()).front().copied()
    };
    let Some(next_target) = next_target else {
        a.velocity = [0.0; 3];
        return false;
    };
    let length = distance(before, next_target).sqrt().max(0.001);
    let speed = (a.kind.speed() * a.movement_scale()).min(length / f64::from(dt));
    let velocity: [f32; 3] = std::array::from_fn(|i| {
        let desired = ((next_target[i] - before[i]) / length * speed) as f32;
        a.velocity[i] + (desired - a.velocity[i]) * (dt * 8.0).min(1.0)
    });
    let proposed = std::array::from_fn(|i| before[i] + f64::from(velocity[i] * dt));
    if traversable_sized(h, before, proposed, a.kind, a.size()) {
        a.position = proposed;
    } else {
        let proposed = std::array::from_fn(|i| {
            before[i] + (next_target[i] - before[i]) / length * speed * f64::from(dt)
        });
        if !traversable_sized(h, before, proposed, a.kind, a.size()) {
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

pub(super) fn valid_point(point: [f64; 3]) -> bool {
    point
        .into_iter()
        .all(|v| v.is_finite() && v.abs() <= 999_000.0)
}

/// Spatial bins bound separation work independently of the total saved population.
pub fn separate(
    pop: &mut terra_core::creatures::CreaturePopulation,
    h: &Habitat,
    ids: &[u64],
    dt: f32,
) {
    let mut bins = HashMap::<[i64; 3], Vec<(u64, [f64; 3], Kind, f64)>>::new();
    let cell = |p: [f64; 3]| p.map(|v| (v / 2.0).floor() as i64);
    for id in ids {
        if let Some(a) = pop
            .animals
            .get(id)
            .filter(|a| a.behavior != super::Behavior::InNest)
        {
            bins.entry(cell(a.position))
                .or_default()
                .push((a.id, a.position, a.kind, a.size()[0]));
        }
    }
    for id in ids {
        let owner = pop.owners.get(id).copied();
        let Some(a) = pop.animals.get_mut(id).filter(|a| {
            !a.kind.is_spider()
                && !matches!(
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
                        for &(other, p, kind, width) in neighbors.iter().take(32) {
                            if other == a.id || kind.is_flying() != a.kind.is_flying() {
                                continue;
                            }
                            let radius = if a.kind == Kind::Bee {
                                0.24
                            } else {
                                (width + a.size()[0]) * 0.45
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
        } else if a.kind.is_flying() {
            traversable(h, a.position, proposed, a.kind).then_some(proposed)
        } else {
            h.ground_sized(proposed, a.size(), f64::from(a.yaw))
        };
        if let Some(next) = next {
            if let Some(owner) = owner
                && a.kind == Kind::Villager
                && (next[0] != a.position[0] || next[2] != a.position[2])
                && !super::villagers::patrol::permits_idle_push(a, h, owner, next)
            {
                continue;
            }
            a.position = next;
        }
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_core::creatures::{CreatureBehavior, CreatureNeeds, creature_id};
    use terra_voxel::{BlockSection, SectionPos};

    fn flat_fixture() -> (TerraGenerator, ChunkStore, EditJournal) {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            water: false,
            ..Default::default()
        })
        .unwrap();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        (terrain, chunks, EditJournal::default())
    }

    fn animal(kind: Kind) -> Creature {
        Creature {
            id: terra_core::creatures::breeding::next_birth_id(&Default::default(), 0).unwrap(),
            kind,
            home: [0, 0],
            position: [7.5, 3.0, 8.5],
            velocity: [0.0; 3],
            yaw: -std::f32::consts::FRAC_PI_2,
            health: kind.health(),
            behavior: CreatureBehavior::Wander,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }

    #[test]
    fn cached_ground_route_uses_the_juveniles_actual_clearance_under_a_low_ceiling() {
        let (terrain, chunks, mut journal) = flat_fixture();
        for x in 9..=13 {
            for z in 4..=12 {
                journal.record([x, 4, z], 3).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let child = animal(Kind::Horse);
        let target = [12.5, 3.0, 8.5];
        assert!(
            habitat
                .walk_step(child.position, target, child.kind, f64::from(child.yaw))
                .is_none()
        );
        let mut nav = Navigation::default();
        let next = ground_waypoint(&child, &habitat, target, true, Some(&mut nav)).unwrap();
        assert!(next[0] > child.position[0]);
        assert!(
            habitat
                .walk_step_sized(child.position, next, child.size(), f64::from(child.yaw))
                .is_some()
        );
        assert!(nav.inspection(&child).path.len() <= 8);
    }

    #[test]
    fn juvenile_flight_scales_speed_and_invalid_goals_never_expand_an_unbounded_sweep() {
        let (terrain, chunks, journal) = flat_fixture();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let mut baby = animal(Kind::Bee);
        baby.position = [7.5, 5.0, 8.5];
        let mut adult = baby.clone();
        adult.age = terra_core::creatures::breeding::GROWTH_SECONDS;
        let target = [14.5, 5.0, 8.5];
        let start = baby.position;
        for _ in 0..5 {
            assert!(fly_cached(&mut baby, &habitat, target, 0.05, None));
            assert!(fly_cached(&mut adult, &habitat, target, 0.05, None));
        }
        let ratio = distance(start, baby.position).sqrt() / distance(start, adult.position).sqrt();
        assert!((ratio - baby.movement_scale()).abs() < 0.0001);
        let before = baby.clone();
        for bad in [[f64::NAN, 0.0, 0.0], [f64::INFINITY; 3], [f64::MAX; 3]] {
            assert!(waypoint(&habitat, start, bad, Kind::Bee).is_none());
            assert!(!fly_cached(&mut baby, &habitat, bad, 0.05, None));
            assert_eq!(baby, before);
        }
        assert!(!traversable(
            &habitat,
            start,
            [999_000.0, 5.0, 8.5],
            Kind::Bee
        ));
        let mut budget = caves::SearchBudget::default();
        assert!(caves::route_budgeted(&habitat, start, [f64::NAN; 3], &mut budget).is_none());
        assert!(caves::deeper_budgeted(&habitat, [f64::MAX; 3], &mut budget).is_none());
        assert_eq!(budget.nodes, 0);
        assert!(!caves::covered(&habitat, [f64::MAX; 3], 2.0));
    }
    #[test]
    fn stalled_cached_routes_request_one_budgeted_replan_without_per_tick_path_searches() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            water: false,
            ..Default::default()
        })
        .unwrap();
        let owner = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let journal = EditJournal::default();
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let a = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind: Kind::Pig,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: Kind::Pig.health(),
            behavior: CreatureBehavior::SeekFood,
            timer: 0.0,
            age: 2.0,
            variant: 0,
            needs: CreatureNeeds::full(Kind::Pig),
        };
        let target = [12.0, 3.0, 8.0];
        let mut nav = Navigation::default();
        nav.routes.insert(
            a.id,
            Route {
                target,
                points: [[9.0, 3.0, 8.0]].into_iter().collect(),
                expires: 100.0,
                ground_blocked: true,
                last_progress: a.position,
                progress_at: 0.0,
                stuck_since: 0.0,
                arrived: false,
            },
        );
        testing::reset_path_calls();
        for _ in 0..30 {
            assert!(ground_waypoint(&a, &habitat, target, false, Some(&mut nav)).is_none());
        }
        assert_eq!(testing::path_calls(), 0);
        assert!(nav.needs_search(a.id, a.age));
        let pending = nav.inspection(&a);
        assert!(pending.blocked && pending.path.is_empty());
        assert_eq!(pending.destination, Some(target));
        assert!(ground_waypoint(&a, &habitat, target, true, Some(&mut nav)).is_some());
        assert_eq!(testing::path_calls(), 1);
        assert!(nav.inspection(&a).path.len() <= 8);
    }
}
