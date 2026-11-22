//! Bounded land graph. Nodes retain their real support height, including floors
//! sharing X/Z; every edge is the same short physical sweep used by locomotion.
use super::{Habitat, NODE_BUDGET, distance, valid_point};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, VecDeque},
};

const RANGE: i32 = 6;
const EDGE_SAMPLE: f64 = 0.18;

pub(super) struct Search {
    pub points: VecDeque<[f64; 3]>,
    complete: bool,
    #[cfg(test)]
    pub expanded: usize,
}

struct Node {
    point: [f64; 3],
    parent: usize,
    cost: u32,
}

/// A walkable X/Z edge may change elevation by real steps. Access is checked on
/// each accepted support sample, so a permitted endpoint cannot skip a claim.
fn step(
    h: &Habitat,
    start: [f64; 3],
    end: [f64; 3],
    size: [f64; 3],
    allowed: &impl Fn([f64; 3]) -> bool,
) -> Option<[f64; 3]> {
    let length = (end[0] - start[0]).hypot(end[2] - start[2]);
    if !valid_point(start) || !valid_point(end) || length > 11.52 || !allowed(start) {
        return None;
    }
    let yaw = (start[0] - end[0]).atan2(start[2] - end[2]);
    let samples = (length / EDGE_SAMPLE).ceil().clamp(1.0, 64.0) as usize;
    let mut current = start;
    for index in 1..=samples {
        let t = index as f64 / samples as f64;
        let probe = [
            start[0] + (end[0] - start[0]) * t,
            current[1],
            start[2] + (end[2] - start[2]) * t,
        ];
        current = h.walk_step_sized(current, probe, size, yaw)?;
        if !allowed(current) {
            return None;
        }
    }
    Some(current)
}

/// Cache and arrival validation also require the saved destination's actual Y.
pub(super) fn edge(
    h: &Habitat,
    start: [f64; 3],
    end: [f64; 3],
    size: [f64; 3],
    allowed: &impl Fn([f64; 3]) -> bool,
) -> Option<[f64; 3]> {
    step(h, start, end, size, allowed).filter(|point| (point[1] - end[1]).abs() <= 0.125)
}

pub(super) fn search(
    h: &Habitat,
    start: [f64; 3],
    target: [f64; 3],
    size: [f64; 3],
    allowed: &impl Fn([f64; 3]) -> bool,
) -> Search {
    let mut route = plan(h, start, target, size, allowed);
    // Keep the authored turn onto each stair and the grid approach into
    // narrow openings. Across open ground a shortcut must leave enough
    // room for the whole body to turn from the previous heading.
    if (target[1] - start[1]).abs() > 0.01
        || route.points.iter().any(|p| (p[1] - start[1]).abs() > 0.01)
    {
        return route;
    }
    let diameter = size[0].hypot(size[2]);
    let turning_body = [diameter, size[1], diameter];
    super::smooth_route(start, &mut route.points, |from, to| {
        (from[1] - to[1]).abs() < 0.01
            && edge(h, from, to, size, &|p| {
                (p[1] - from[1]).abs() < 0.01 && allowed(p) && h.clear(p, turning_body, false)
            })
            .is_some()
    });
    route
}

fn plan(
    h: &Habitat,
    start: [f64; 3],
    target: [f64; 3],
    size: [f64; 3],
    allowed: &impl Fn([f64; 3]) -> bool,
) -> Search {
    // When the goal is directly above a broad lower room, every useful first
    // step can increase its heuristic distance. Search once from the smaller
    // upper surface instead of spending a second budget or fabricating an
    // approach point. Never use a disconnected reverse frontier as a route.
    let reverse = target[1] > start[1] + 1.0
        && (target[0] - start[0]).hypot(target[2] - start[2]) <= f64::from(RANGE)
        && edge(h, target, target, size, allowed).is_some();
    if !reverse {
        return directed_search(h, start, target, size, allowed);
    }
    let result = directed_search(h, target, start, size, allowed);
    if !result.complete {
        return Search {
            points: VecDeque::new(),
            ..result
        };
    }
    let mut points: VecDeque<_> = result
        .points
        .into_iter()
        .rev()
        .filter(|point| distance(*point, start) > 0.001_f64.powi(2))
        .collect();
    points.push_back(target);
    let mut previous = start;
    for &point in &points {
        if edge(h, previous, point, size, allowed).is_none() {
            return Search {
                points: VecDeque::new(),
                complete: false,
                #[cfg(test)]
                expanded: result.expanded,
            };
        }
        previous = point;
    }
    Search {
        points,
        complete: true,
        #[cfg(test)]
        expanded: result.expanded,
    }
}

fn directed_search(
    h: &Habitat,
    start: [f64; 3],
    target: [f64; 3],
    size: [f64; 3],
    allowed: &impl Fn([f64; 3]) -> bool,
) -> Search {
    let empty = || Search {
        points: VecDeque::new(),
        complete: false,
        #[cfg(test)]
        expanded: 0,
    };
    if !valid_point(start)
        || !valid_point(target)
        || !allowed(start)
        || size
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 24.0)
    {
        return empty();
    }
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.navigation.ground_path");
    #[cfg(test)]
    super::testing::record_path();
    // Cell centers align a route with narrow voxel doorways even when the real
    // initial body stands off-center. Actual support Y still distinguishes floors.
    let key = |point: [f64; 3]| {
        [
            (point[0].floor() - start[0].floor()) as i32,
            ((point[1] - start[1]) * 4.0).round() as i32,
            (point[2].floor() - start[2].floor()) as i32,
        ]
    };
    let heuristic = |point| (distance(point, target).sqrt() * 100.0) as u32;
    let mut nodes = vec![Node {
        point: start,
        parent: 0,
        cost: 0,
    }];
    let mut visited: HashMap<[i32; 3], usize> = HashMap::new();
    let mut queue = BinaryHeap::from([Reverse((heuristic(start), 0u32, 0usize))]);
    let mut best = 0;
    let mut finish = None;
    let mut expanded = 0;
    while let Some(Reverse((_, cost, index))) = queue.pop() {
        if expanded >= NODE_BUDGET {
            break;
        }
        if nodes[index].cost != cost {
            continue;
        }
        expanded += 1;
        let current = nodes[index].point;
        if heuristic(current) < heuristic(nodes[best].point) {
            best = index;
        }
        // A nearby target is appended only after a real supported sweep. A
        // vertical target above/below this floor cannot pass an air-only test.
        if (current[0] - target[0]).hypot(current[2] - target[2]) <= 2.0
            && (current[1] - target[1]).abs() <= 2.0
            && edge(h, current, target, size, allowed).is_some()
        {
            best = index;
            finish = Some(target);
            break;
        }
        for [dx, dz] in [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]] {
            let probe = [
                current[0].floor() + 0.5 + f64::from(dx),
                current[1],
                current[2].floor() + 0.5 + f64::from(dz),
            ];
            if (probe[0] - current[0]).hypot(probe[2] - current[2]) < 0.0001 {
                continue;
            }
            if key(probe)[0].abs() > RANGE || key(probe)[2].abs() > RANGE {
                continue;
            }
            let Some(point) = step(h, current, probe, size, allowed) else {
                continue;
            };
            let cell = key(point);
            if cell[1].abs() > RANGE * 4 {
                continue;
            }
            let next_cost =
                cost.saturating_add((distance(current, point).sqrt() * 100.0).ceil() as u32);
            let next = if let Some(&previous) = visited.get(&cell) {
                if next_cost >= nodes[previous].cost {
                    continue;
                }
                nodes[previous].cost = next_cost;
                nodes[previous].parent = index;
                previous
            } else {
                if nodes.len() >= NODE_BUDGET {
                    continue;
                }
                let next = nodes.len();
                visited.insert(cell, next);
                nodes.push(Node {
                    point,
                    parent: index,
                    cost: next_cost,
                });
                next
            };
            // Stable tie ordering and cost relaxation avoid hash-order routes.
            queue.push(Reverse((
                next_cost.saturating_add(heuristic(point).saturating_mul(2)),
                next_cost,
                next,
            )));
        }
    }
    let mut points = VecDeque::new();
    let mut cursor = best;
    for _ in 0..NODE_BUDGET {
        if cursor == 0 {
            break;
        }
        points.push_front(nodes[cursor].point);
        cursor = nodes[cursor].parent;
    }
    if let Some(target) = finish
        && points
            .back()
            .is_none_or(|last| distance(*last, target) > 0.001_f64.powi(2))
    {
        points.push_back(target);
    }
    // A bounded partial route is useful only when it really approaches the
    // destination. Never claim arrival, invent a node or leave the loaded graph.
    Search {
        points,
        complete: finish.is_some(),
        #[cfg(test)]
        expanded,
    }
}
