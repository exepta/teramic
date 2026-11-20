//! Bounded local construction paths over actual loaded support surfaces.
//! Multiple floors share X/Z; each edge uses the ordinary strict body sweep.
use super::*;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap, VecDeque},
};

const MAX_STATES: usize = 512;
fn point(job: &BuildingJob, [x, y, z]: [f64; 3]) -> [f64; 3] {
    let [x, z] = match job.facing {
        0 => [x, z],
        1 => [z, -x],
        2 => [-x, -z],
        _ => [-z, x],
    };
    [
        job.origin[0] as f64 + x + 0.5,
        job.origin[1] as f64 + y,
        job.origin[2] as f64 + z + 0.5,
    ]
}
fn edge(h: &Habitat, job: &BuildingJob, a: [f64; 3], b: [f64; 3], size: [f64; 3]) -> bool {
    let length = (b[0] - a[0]).hypot(b[2] - a[2]);
    if length > 11.5 || (a[1] - b[1]).abs() > length + 1.01 {
        return false;
    }
    let yaw = (a[0] - b[0]).atan2(a[2] - b[2]);
    let count = (length / 0.18).ceil().max(1.0) as usize;
    let mut current = a;
    for n in 1..=count {
        let t = n as f64 / count as f64;
        let probe = [
            a[0] + (b[0] - a[0]) * t,
            current[1],
            a[2] + (b[2] - a[2]) * t,
        ];
        let Some(next) = h.walk_step_sized(current, probe, size, yaw) else {
            return false;
        };
        if !movement::permitted(h.chunks, h.journal, job, next) || !movement::work_floor(job, next)
        {
            return false;
        }
        current = next;
    }
    (current[1] - b[1]).abs() < 0.125
}
pub(super) fn route(
    h: &Habitat,
    job: &BuildingJob,
    npc: &Creature,
    target: [f64; 3],
) -> Option<VecDeque<[f64; 3]>> {
    if edge(h, job, npc.position, target, npc.size()) {
        return Some(VecDeque::from([target]));
    }
    let mut nodes = Vec::with_capacity(MAX_STATES);
    nodes.extend([npc.position, target]);
    let mut columns = BTreeMap::<[i64; 2], Vec<usize>>::new();
    // This covers the authored house, ramp, all three scaffold decks and their
    // ground approaches. Probe real support; air and unknown terrain add no node.
    for x in -7..=6 {
        for z in -8..=6 {
            for y in -2..=10 {
                if nodes.len() >= MAX_STATES {
                    break;
                }
                let seed = point(job, [f64::from(x), f64::from(y), f64::from(z)]);
                let Some(p) = h.ground_sized(seed, npc.size(), 0.0) else {
                    continue;
                };
                if !movement::permitted(h.chunks, h.journal, job, p)
                    || !movement::work_floor(job, p)
                {
                    continue;
                }
                let key = [p[0].floor() as i64, p[2].floor() as i64];
                let entries = columns.entry(key).or_default();
                if entries.iter().any(|i| (nodes[*i][1] - p[1]).abs() < 0.001) {
                    continue;
                }
                entries.push(nodes.len());
                nodes.push(p);
            }
        }
    }
    let count = nodes.len();
    let mut costs = vec![u32::MAX; count];
    let mut parents = vec![usize::MAX; count];
    let mut visited = vec![false; count];
    let estimate = |p: [f64; 3]| (crate::creatures::distance(p, target).sqrt() * 100.0) as u32;
    let mut queue = BinaryHeap::from([Reverse((estimate(npc.position), 0_u32, 0_usize))]);
    costs[0] = 0;
    let mut expanded = 0;
    let mut neighbors = Vec::with_capacity(32);
    while let Some(Reverse((_, cost, index))) = queue.pop() {
        if expanded >= MAX_STATES {
            break;
        }
        if visited[index] || costs[index] != cost {
            continue;
        }
        expanded += 1;
        visited[index] = true;
        if index == 1 {
            let mut result = VecDeque::new();
            let mut current = 1;
            while current != 0 {
                result.push_front(nodes[current]);
                current = parents[current];
            }
            return Some(result);
        }
        let p = nodes[index];
        neighbors.clear();
        if index == 0 {
            // A real actor can start between cell centers or just outside the
            // site. Connect only nearby centers through a complete body sweep.
            neighbors.extend(
                (2..count).filter(|i| (nodes[*i][0] - p[0]).hypot(nodes[*i][2] - p[2]) <= 2.5),
            );
        } else {
            let [x, z] = [p[0].floor() as i64, p[2].floor() as i64];
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    if let Some(entries) = columns.get(&[x + dx, z + dz]) {
                        neighbors.extend(entries.iter().copied());
                    }
                }
            }
        }
        if (p[0] - target[0]).hypot(p[2] - target[2]) <= 2.5 {
            neighbors.push(1);
        }
        for next in neighbors.iter().copied() {
            if visited[next] || (p[1] - nodes[next][1]).abs() > 2.01 {
                continue;
            }
            let length = crate::creatures::distance(p, nodes[next]).sqrt();
            let candidate = cost.saturating_add((length * 100.0).ceil() as u32);
            if candidate < costs[next] && edge(h, job, p, nodes[next], npc.size()) {
                costs[next] = candidate;
                parents[next] = index;
                queue.push(Reverse((
                    candidate.saturating_add(estimate(nodes[next])),
                    candidate,
                    next,
                )));
            }
        }
    }
    None
}

fn local(job: &BuildingJob, p: [f64; 3]) -> [f64; 2] {
    let dx = p[0] - job.origin[0] as f64 - 0.5;
    let dz = p[2] - job.origin[2] as f64 - 0.5;
    match job.facing {
        0 => [dx, dz],
        1 => [-dz, dx],
        2 => [-dx, -dz],
        _ => [dz, -dx],
    }
}
pub(super) fn outside(job: &BuildingJob, p: [f64; 3]) -> bool {
    let [x, z] = local(job, p);
    !(-6.5..=5.5).contains(&x) || !(-7.5..=5.5).contains(&z)
}
pub(super) fn far(job: &BuildingJob, p: [f64; 3]) -> bool {
    let [x, z] = local(job, p);
    !(-9.0..=8.0).contains(&x) || !(-10.0..=8.0).contains(&z)
}
pub(super) fn egress(
    h: &Habitat,
    job: &BuildingJob,
    npc: &Creature,
    target: [f64; 3],
) -> Option<VecDeque<[f64; 3]>> {
    if !far(job, target) {
        return None;
    }
    let mut exits = Vec::with_capacity(4);
    for [x, z] in [[-7.0, 0.0], [6.0, 0.0], [0.0, -8.0], [0.0, 6.0]] {
        for y in [0.0, -1.0, 1.0, -2.0, 2.0] {
            if let Some(p) = h.ground_sized(point(job, [x, y, z]), npc.size(), 0.0)
                && outside(job, p)
                && movement::permitted(h.chunks, h.journal, job, p)
            {
                exits.push(p);
                break;
            }
        }
    }
    exits.sort_by(|a, b| {
        crate::creatures::distance(*a, target).total_cmp(&crate::creatures::distance(*b, target))
    });
    if exits.first().is_some_and(|p| {
        crate::creatures::distance(*p, npc.position) < 0.3_f64.powi(2)
            || outside(job, npc.position)
                && crate::creatures::distance(npc.position, target)
                    <= crate::creatures::distance(*p, target) + 0.1
    }) {
        return None;
    }
    exits
        .into_iter()
        .take(2)
        .find_map(|exit| route(h, job, npc, exit))
}
