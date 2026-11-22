//! Bounded routes over the same edited surfaces used by physical spider movement.
use super::{Creature, Habitat, distance, move_surface_with_depth, tangent_axes};
use crate::creatures::navigation::{NODE_BUDGET, Navigation};
use std::collections::{BinaryHeap, HashSet, VecDeque};

pub(in crate::creatures) struct Point {
    pub position: [f64; 3],
    normal: [i8; 3],
    steer: [f64; 3],
}

pub(in crate::creatures) struct Route {
    pub target: [f64; 3],
    pub points: VecDeque<Point>,
    ascent: Option<([f64; 3], [i8; 3])>,
    pub expires: f64,
    pub blocked: bool,
    progress: [f64; 3],
    progress_at: f64,
    closest: f64,
}

fn center(a: &Creature) -> [f64; 3] {
    terra_core::creatures::spiders::visual_center(a)
}
fn normal(a: &Creature) -> [i8; 3] {
    terra_core::creatures::spiders::surface_normal(a)
}
fn key(a: &Creature) -> ([i64; 3], [i8; 3]) {
    (center(a).map(|v| (v / 0.45).floor() as i64), normal(a))
}

/// A route caches steering edges, never teleports an animal to a graph node.
pub(super) fn advance(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    motion: (f32, f64, bool),
    search: bool,
    navigation: Option<&mut Navigation>,
) -> bool {
    if !super::super::navigation::valid_point(target) {
        if let Some(nav) = navigation {
            nav.forget(a.id);
        }
        return false;
    }
    let (dt, speed, owned) = motion;
    let Some(nav) = navigation else {
        return move_surface_with_depth(a, h, target, dt, speed, owned);
    };
    if nav.surface_routes.len() >= terra_core::creatures::MAX_CREATURES
        && !nav.surface_routes.contains_key(&a.id)
    {
        nav.surface_routes.retain(|_, route| route.expires > a.age);
        if nav.surface_routes.len() >= terra_core::creatures::MAX_CREATURES {
            return move_surface_with_depth(a, h, target, dt, speed, owned);
        }
    }
    let route = nav.surface_routes.entry(a.id).or_insert_with(|| Route {
        target,
        points: VecDeque::new(),
        ascent: None,
        expires: 0.0,
        blocked: false,
        progress: center(a),
        progress_at: a.age,
        closest: distance(center(a), target),
    });
    if distance(route.target, target) > 4.0 {
        route.points.clear();
        route.target = target;
        route.ascent = None;
        route.blocked = false;
        route.closest = distance(center(a), target);
        route.progress_at = a.age;
        route.expires = 0.0;
    }
    let gap = distance(center(a), target);
    if gap + 0.04 < route.closest {
        route.closest = gap;
        route.progress_at = a.age;
    } else if a.age - route.progress_at > 1.5 && gap > 0.36 {
        route.blocked = true;
    }
    while route.points.front().is_some_and(|point| {
        point.normal == normal(a) && distance(point.position, center(a)) < 0.18
    }) {
        route.points.pop_front();
    }
    if !route.points.is_empty() && distance(route.progress, center(a)) > 0.04 {
        route.progress = center(a);
        route.progress_at = a.age;
    } else if !route.points.is_empty() && a.age - route.progress_at > 2.0 {
        route.points.clear();
        route.expires = 0.0;
        route.progress_at = a.age;
    }
    // When prey stands directly above the spider, projecting its position
    // onto the floor produces no direction. Find a nearby climbable face and
    // remember the approach across ticks, even while the detour increases the
    // straight-line distance to the prey.
    let here = center(a);
    if search
        && route.points.is_empty()
        && route.expires <= a.age
        && normal(a)[1] > 0
        && target[1] > here[1] + 1.5
        && (target[0] - here[0]).hypot(target[2] - here[2]) < 3.
    {
        route.ascent = ascent(a, h, target);
        route.expires = a.age + if route.ascent.is_some() { 4. } else { 0.6 };
    }
    if let Some((wall, face)) = route.ascent {
        if normal(a) == face || route.expires <= a.age || target[1] <= here[1] + 0.6 {
            route.ascent = None;
        } else if move_surface_with_depth(a, h, wall, dt, speed, owned) {
            return true;
        } else {
            route.ascent = None;
            route.blocked = true;
        }
    }
    if route.blocked && route.points.is_empty() && search && route.expires <= a.age {
        route.points = path(a, h, target, owned);
        route.expires = a.age + if route.points.is_empty() { 0.8 } else { 2.5 };
        route.progress_at = a.age;
    }
    let steer = route.points.front().map_or(target, |point| point.steer);
    let moved = move_surface_with_depth(a, h, steer, dt, speed, owned);
    if !moved {
        route.points.clear();
        route.blocked = true;
    }
    moved
}

fn ascent(a: &Creature, h: &Habitat, target: [f64; 3]) -> Option<([f64; 3], [i8; 3])> {
    let here = center(a);
    let mut best: Option<(f64, [f64; 3], [i8; 3])> = None;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for radius in 1..=5 {
            let x = here[0] + f64::from(dx * radius);
            let z = here[2] + f64::from(dz * radius);
            let wall = [x, here[1], z];
            if super::shape_contains(h, wall).is_none() {
                continue;
            }
            // A single pebble is a poor wall. The upper face must also exist.
            if super::shape_contains(h, [x, here[1] + 1., z]).is_some() {
                let score = f64::from(radius) + 0.4 * (target[0] - x).hypot(target[2] - z);
                if best.is_none_or(|(cost, ..)| score < cost) {
                    best = Some((score, wall, [-dx as i8, 0, -dz as i8]));
                }
            }
            break;
        }
    }
    best.map(|(_, p, n)| (p, n))
}

fn path(a: &Creature, h: &Habitat, target: [f64; 3], owned: bool) -> VecDeque<Point> {
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.navigation.surface_path");
    #[cfg(test)]
    crate::creatures::navigation::testing::record_path();
    let heuristic = |p| (distance(p, target).sqrt() * 100.0) as u32;
    let mut nodes = vec![(a.clone(), 0usize, center(a), 0u32)];
    let mut queue = BinaryHeap::from([(std::cmp::Reverse(heuristic(center(a))), 0usize)]);
    let mut visited = HashSet::from([key(a)]);
    let mut best = 0;
    while let Some((_, index)) = queue.pop() {
        if heuristic(center(&nodes[index].0)) < heuristic(center(&nodes[best].0)) {
            best = index;
        }
        if index != 0 && distance(center(&nodes[index].0), target) < 0.36 {
            best = index;
            break;
        }
        let parent = nodes[index].0.clone();
        let axes = tangent_axes(normal(&parent));
        for [u, v] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
            if nodes.len() >= NODE_BUDGET {
                break;
            }
            let mut steer = center(&parent);
            steer[axes[0]] += u * 4.0;
            steer[axes[1]] += v * 4.0;
            let mut candidate = parent.clone();
            if !move_surface_with_depth(
                &mut candidate,
                h,
                steer,
                0.25,
                2.4 / parent.growth_scale(),
                owned,
            ) || distance(center(&candidate), center(&parent)) < 0.01
                || !visited.insert(key(&candidate))
            {
                continue;
            }
            let cost = nodes[index].3
                + (distance(center(&candidate), center(&parent)).sqrt() * 100.0) as u32;
            let score = cost + heuristic(center(&candidate)) * 2;
            let next = nodes.len();
            nodes.push((candidate, index, steer, cost));
            queue.push((std::cmp::Reverse(score), next));
        }
        if nodes.len() >= NODE_BUDGET {
            break;
        }
    }
    let mut result = VecDeque::new();
    while best != 0 {
        let (animal, previous, steer, _) = &nodes[best];
        result.push_front(Point {
            position: center(animal),
            normal: normal(animal),
            steer: *steer,
        });
        best = *previous;
    }
    result
}
