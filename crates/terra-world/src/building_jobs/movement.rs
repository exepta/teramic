use super::*;

#[derive(Default, Debug)]
pub(super) struct Route {
    target: Option<[i64; 3]>,
    candidates: Vec<[f64; 3]>,
    index: usize,
    retry_at: f64,
    at: f64,
    position: [f64; 3],
    stalled: bool,
    waypoints: std::collections::VecDeque<[f64; 3]>,
    waypoint_goal: Option<[f64; 3]>,
    external: bool,
}
impl Route {
    pub(super) fn stuck(&self, _now: f64) -> bool {
        self.stalled
    }
}

fn blocked_stance(runtime: &mut BuildRuntime, npc: &Creature, cell: [i64; 3]) {
    runtime.navigation.invalidate(npc.id);
    runtime.routes.insert(
        npc.id,
        Route {
            target: Some(cell),
            at: npc.age,
            position: npc.position,
            stalled: true,
            ..Default::default()
        },
    );
}
pub(super) fn permitted(
    chunks: &ChunkStore,
    journal: &EditJournal,
    job: &BuildingJob,
    p: [f64; 3],
) -> bool {
    [-0.32, 0.32].into_iter().all(|x| {
        [-0.32, 0.32].into_iter().all(|z| {
            build_access(
                chunks,
                journal,
                job.kingdom,
                job.owner,
                [
                    (p[0] + x).floor() as i64,
                    p[1].floor() as i64,
                    (p[2] + z).floor() as i64,
                ],
            )
        })
    })
}
/// Finished rooms are traversed on their floor; furniture tops are not work
/// platforms. Exterior approaches retain their actual terrain support height.
pub(super) fn work_floor(job: &BuildingJob, p: [f64; 3]) -> bool {
    let finishing = job.next_step >= job.steps.len()
        || job
            .steps
            .get(job.next_step)
            .is_some_and(|s| s.phase == BuildPhase::Furnishing);
    if !finishing {
        return true;
    }
    let dx = p[0] - job.origin[0] as f64 - 0.5;
    let dz = p[2] - job.origin[2] as f64 - 0.5;
    let [x, z] = match job.facing {
        0 => [dx, dz],
        1 => [-dz, dx],
        2 => [-dx, -dz],
        _ => [dz, -dx],
    };
    x.abs() > 3.0 || z.abs() > 3.0 || (p[1] - job.origin[1] as f64).abs() < 0.08
}

pub(super) fn reachable(h: &Habitat, npc: &Creature, cell: [i64; 3]) -> bool {
    let c = cell.map(|v| v as f64 + 0.5);
    if (npc.position[0] - c[0]).hypot(npc.position[2] - c[2]) > 3.5 {
        return false;
    }
    if (npc.position[0] - c[0]).abs() < 0.8
        && (npc.position[2] - c[2]).abs() < 0.8
        && c[1] + 0.5 >= npc.position[1] - 0.08
        && c[1] - 0.5 < npc.position[1] + npc.size()[1]
    {
        return false;
    }
    if h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
        .is_none_or(|g| (g[1] - npc.position[1]).abs() > 0.08)
    {
        return false;
    }
    let eye = [npc.position[0], npc.position[1] + 1.25, npc.position[2]];
    let d = std::array::from_fn(|i| c[i] - eye[i]);
    let reach = d.iter().map(|v| v * v).sum::<f64>().sqrt();
    if reach > 3.5 {
        return false;
    }
    let mut loaded = true;
    let hit = terra_voxel::blocks::raycast_blocks(eye, d, reach + 0.01, |p| {
        let b = h.chunks.block_at(p);
        loaded &= b.is_some();
        b
    });
    loaded && hit.is_none_or(|hit| hit.position == cell)
}
fn grounded(h: &Habitat, npc: &Creature) -> bool {
    let same = |p: [f64; 3]| (p[1] - npc.position[1]).abs() < 0.08;
    h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
        .is_some_and(same)
        || h.supported_sized(npc.position, npc.size(), f64::from(npc.yaw))
            .is_some_and(same)
}

/// Gravity permits a real foot to hang over a ledge, whereas path planning
/// deliberately requires all four foot probes. Recenter such a contact with
/// bounded physical sweeps before asking the strict walker for a new path.
pub(super) fn recenter(h: &Habitat, job: &BuildingJob, npc: &mut Creature, dt: f32) -> bool {
    center_contact(h, job, npc, dt, false)
}
fn center_contact(
    h: &Habitat,
    job: &BuildingJob,
    npc: &mut Creature,
    dt: f32,
    force: bool,
) -> bool {
    if !force
        && h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
            .is_some_and(|p| (p[1] - npc.position[1]).abs() < 0.08)
    {
        return false;
    }
    if !grounded(h, npc) {
        return false;
    }
    let mut targets = Vec::with_capacity(9);
    let base = npc.position.map(|v| v.floor());
    for x in -1..=1 {
        for z in -1..=1 {
            let seed = [
                base[0] + f64::from(x) + 0.5,
                npc.position[1],
                base[2] + f64::from(z) + 0.5,
            ];
            if let Some(p) = h.ground_sized(seed, npc.size(), f64::from(npc.yaw))
                && (p[1] - npc.position[1]).abs() <= 1.01
                && (p[0] - npc.position[0]).hypot(p[2] - npc.position[2]) <= 1.01
                && permitted(h.chunks, h.journal, job, p)
                && !h.wet(p)
            {
                targets.push(p);
            }
        }
    }
    targets.sort_by(|a, b| {
        crate::creatures::distance(*a, npc.position)
            .total_cmp(&crate::creatures::distance(*b, npc.position))
    });
    for target in targets {
        let old = npc.position;
        let length = (target[0] - old[0]).hypot(target[2] - old[2]);
        if length < 0.00001 {
            continue;
        }
        let travel = (npc.kind.speed() * f64::from(dt)).min(length);
        let t = travel / length;
        let next = [
            old[0] + (target[0] - old[0]) * t,
            old[1],
            old[2] + (target[2] - old[2]) * t,
        ];
        // Keep the existing stance: rotating a narrow body on its last
        // contact can remove that very support before it moves inward.
        let yaw = f64::from(npc.yaw);
        let samples = (travel / 0.05).ceil().max(1.0) as usize;
        let safe = (1..=samples).all(|n| {
            let p =
                std::array::from_fn(|i| old[i] + (next[i] - old[i]) * n as f64 / samples as f64);
            permitted(h.chunks, h.journal, job, p)
                && !h.wet(p)
                && h.supported_sized(p, npc.size(), yaw)
                    .is_some_and(|g| (g[1] - old[1]).abs() <= 1.01)
        });
        if !safe {
            continue;
        }
        if let Some(next) = h.travel_step_sized(old, next, npc.size(), yaw)
            && (next[1] - old[1]).abs() <= 1.01
        {
            npc.position = next;
            npc.yaw = yaw as f32;
            npc.velocity = [
                ((next[0] - old[0]) / f64::from(dt)) as f32,
                0.0,
                ((next[2] - old[2]) / f64::from(dt)) as f32,
            ];
            crate::creatures::work_rest_with_access(npc, h, dt, |p| {
                permitted(h.chunks, h.journal, job, p)
            });
            npc.behavior = CreatureBehavior::Wander;
            return true;
        }
    }
    false
}

fn standing_point(
    h: &Habitat,
    job: &BuildingJob,
    npc: &Creature,
    cell: [i64; 3],
    avoid: &[BuildCell],
    seed: [f64; 3],
) -> Option<[f64; 3]> {
    let position = h.ground_sized(seed, npc.size(), f64::from(npc.yaw))?;
    if !permitted(h.chunks, h.journal, job, position) || !work_floor(job, position) {
        return None;
    }
    let mut probe = npc.clone();
    probe.position = position;
    (clear_for_cells(&probe, avoid) && reachable(h, &probe, cell)).then_some(position)
}

fn candidates(
    h: &Habitat,
    job: &BuildingJob,
    npc: &Creature,
    cell: [i64; 3],
    avoid: &[BuildCell],
    refine: bool,
) -> Vec<[f64; 3]> {
    let mut result = Vec::with_capacity(48);
    for y in [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0] {
        for x in -2_i32..=2 {
            for z in -2_i32..=2 {
                let [x, z] = [f64::from(x), f64::from(z)];
                let seed = [
                    cell[0] as f64 + 0.5 + x,
                    cell[1] as f64 + y,
                    cell[2] as f64 + 0.5 + z,
                ];
                if let Some(p) = standing_point(h, job, npc, cell, avoid, seed)
                    && !result.contains(&p)
                {
                    result.push(p);
                }
            }
        }
    }
    // A tall terrace can have no reachable cell-center stance: standing two
    // metres away hits its lower face, while three metres exceeds hand reach.
    // Try intermediate positions only after the ordinary set is empty or its
    // routes were exhausted. The caller caches this bounded fallback too.
    if refine || result.is_empty() {
        for y in [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0] {
            for direction in 0..16 {
                let angle = f64::from(direction) * std::f64::consts::TAU / 16.0;
                let seed = [
                    cell[0] as f64 + 0.5 + angle.cos() * 2.5,
                    cell[1] as f64 + y,
                    cell[2] as f64 + 0.5 + angle.sin() * 2.5,
                ];
                if let Some(p) = standing_point(h, job, npc, cell, avoid, seed)
                    && !result.contains(&p)
                {
                    result.push(p);
                }
            }
        }
    }
    let platform = |p: [f64; 3]| {
        let dx = p[0].floor() as i64 - job.origin[0];
        let dz = p[2].floor() as i64 - job.origin[2];
        let [x, z] = match job.facing {
            0 => [dx, dz],
            1 => [-dz, dx],
            2 => [-dx, -dz],
            _ => [dz, -dx],
        };
        x.abs() == 5 || z == -7 || z == 5 || x == -6 || p[1] <= job.origin[1] as f64 + 0.1
    };
    // Work from established decks or ground whenever hand reach permits it.
    // Walking onto one unfinished roof wing can isolate a builder from the
    // opposite wing before the ridge and connecting rafters exist.
    result.sort_by(|a, b| {
        (!platform(*a)).cmp(&(!platform(*b))).then_with(|| {
            crate::creatures::distance(*a, npc.position)
                .total_cmp(&crate::creatures::distance(*b, npc.position))
        })
    });
    result
}
fn doors(h: &Habitat, job: &BuildingJob, npc: &Creature, runtime: &mut BuildRuntime) {
    let base = npc.position.map(|v| v.floor() as i64);
    for x in -4..=4 {
        for z in -4..=4 {
            let p = [base[0] + x, base[1], base[2] + z];
            let Some(material) = h
                .chunks
                .block_at(p)
                .filter(|m| crate::npc_work::worker_openable_door(*m))
            else {
                continue;
            };
            let Some(state) = h.journal.object_state(p) else {
                continue;
            };
            let p = if state.secondary {
                state.peer.unwrap_or(p)
            } else {
                p
            };
            if crate::creatures::distance(npc.position, p.map(|v| v as f64 + 0.5)) > 4.0_f64.powi(2)
                || crate::objects::is_open(material, Some(state))
                || !crate::kingdom_access::can_use(
                    &h.journal.gameplay().kingdoms,
                    h.chunks,
                    job.owner,
                    KingdomPermission::Openings,
                    p,
                )
            {
                continue;
            }
            if runtime.door_requests.len() < 16 {
                runtime.door_requests.insert(
                    p,
                    crate::npc_work::WorkerDoorRequest {
                        npc: npc.id,
                        owner: job.owner,
                        position: p,
                    },
                );
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn approach(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    job: &BuildingJob,
    npc: &mut Creature,
    cell: [i64; 3],
    avoid: &[BuildCell],
    dt: f32,
    runtime: &mut BuildRuntime,
) -> bool {
    let h = Habitat {
        chunks,
        journal,
        terrain,
    };
    if !permitted(chunks, journal, job, npc.position) {
        npc.velocity = [0.0; 3];
        npc.behavior = CreatureBehavior::Rest;
        crate::creatures::work_rest_with_access(npc, &h, dt, |p| {
            permitted(chunks, journal, job, p)
        });
        blocked_stance(runtime, npc, cell);
        return false;
    }
    if recenter(&h, job, npc, dt) {
        runtime.navigation.invalidate(npc.id);
        runtime.routes.remove(&npc.id);
        return false;
    }
    if !grounded(&h, npc) {
        npc.behavior = CreatureBehavior::Rest;
        crate::creatures::work_rest_with_access(npc, &h, dt, |p| {
            permitted(chunks, journal, job, p)
        });
        blocked_stance(runtime, npc, cell);
        return false;
    }
    if clear_for_cells(npc, avoid) && reachable(&h, npc, cell) {
        npc.velocity = [0.0; 3];
        npc.behavior = CreatureBehavior::Rest;
        return true;
    }
    doors(&h, job, npc, runtime);
    let mut route = runtime.routes.remove(&npc.id).unwrap_or_default();
    if route.target != Some(cell) {
        route = Route {
            target: Some(cell),
            candidates: candidates(&h, job, npc, cell, avoid, false),
            retry_at: npc.age + 0.8,
            at: npc.age,
            position: npc.position,
            ..Default::default()
        };
        runtime.navigation.invalidate(npc.id);
    }
    if npc.age - route.at > 2.5 {
        if crate::creatures::distance(npc.position, route.position) < 0.04_f64.powi(2) {
            route.index += 1;
            route.waypoints.clear();
            route.waypoint_goal = None;
            route.stalled = true;
            runtime.navigation.invalidate(npc.id);
        } else {
            route.stalled = false;
        }
        route.at = npc.age;
        route.position = npc.position;
    }
    while let Some(&target) = route.candidates.get(route.index) {
        let mut probe = npc.clone();
        probe.position = target;
        if permitted(chunks, journal, job, target)
            && clear_for_cells(&probe, avoid)
            && reachable(&h, &probe, cell)
        {
            break;
        }
        route.index += 1;
        runtime.navigation.invalidate(npc.id);
    }
    let Some(target) = route.candidates.get(route.index).copied() else {
        // Terrain may have changed, but retries are bounded rather than replanning every frame.
        // Retry deadlines are independent of displacement observations: resetting
        // the progress timer every 0.8s hid an empty stance set forever.
        route.stalled = true;
        if npc.age >= route.retry_at {
            route.candidates = candidates(&h, job, npc, cell, avoid, true);
            route.index = 0;
            route.retry_at = npc.age + 0.8;
        }
        npc.velocity[0] = 0.0;
        npc.velocity[2] = 0.0;
        npc.behavior = CreatureBehavior::Rest;
        crate::creatures::work_rest_with_access(npc, &h, dt, |p| {
            permitted(chunks, journal, job, p)
        });
        runtime.routes.insert(npc.id, route);
        return false;
    };
    if route.waypoint_goal != Some(target) {
        if super::site_path::outside(job, npc.position) && super::site_path::far(job, npc.position)
        {
            if outside_walk(&h, job, npc, target, dt, runtime) {
                route.stalled = false;
            }
            runtime.routes.insert(npc.id, route);
            return false;
        }
        let points = super::site_path::route(&h, job, npc, target);
        let points = if points.is_some() {
            route.external = false;
            points
        } else {
            route.external = true;
            super::site_path::egress(&h, job, npc, target)
        };
        let Some(points) = points else {
            if super::site_path::outside(job, npc.position) {
                if outside_walk(&h, job, npc, target, dt, runtime) {
                    route.stalled = false;
                }
                runtime.routes.insert(npc.id, route);
                return false;
            }
            let centered = (npc.position[0] - npc.position[0].floor() - 0.5)
                .hypot(npc.position[2] - npc.position[2].floor() - 0.5)
                < 0.06;
            if !centered && center_contact(&h, job, npc, dt, true) {
                runtime.navigation.invalidate(npc.id);
                runtime.routes.remove(&npc.id);
                return false;
            }
            route.index += 1;
            route.stalled = true;
            route.waypoint_goal = None;
            crate::creatures::work_rest_with_access(npc, &h, dt, |p| {
                permitted(chunks, journal, job, p)
            });
            runtime.navigation.invalidate(npc.id);
            runtime.routes.insert(npc.id, route);
            return false;
        };
        route.waypoints = points;
        route.waypoint_goal = Some(target);
        runtime.navigation.invalidate(npc.id);
    }
    while route
        .waypoints
        .front()
        .is_some_and(|p| crate::creatures::distance(*p, npc.position) < 0.05_f64.powi(2))
    {
        route.waypoints.pop_front();
        runtime.navigation.invalidate(npc.id);
    }
    if route.external && route.waypoints.is_empty() {
        route.external = false;
        route.waypoint_goal = None;
        crate::creatures::work_rest_with_access(npc, &h, dt, |p| {
            permitted(chunks, journal, job, p)
        });
        runtime.routes.insert(npc.id, route);
        return false;
    }
    let destination = route.waypoints.front().copied().unwrap_or(target);
    let old = npc.position;
    let dx = destination[0] - old[0];
    let dz = destination[2] - old[2];
    let length = dx.hypot(dz);
    let mut walked = false;
    if length > 0.00001 {
        let amount = (npc.kind.speed() * f64::from(dt)).min(length);
        let probe = [
            old[0] + dx / length * amount,
            old[1],
            old[2] + dz / length * amount,
        ];
        // Follow the verified segment exactly. Retaining the existing facing
        // permits a real sideways step out of a narrow shaped stair contact.
        for yaw in [(-dx).atan2(-dz), f64::from(npc.yaw)] {
            if let Some(next) = h.walk_step_sized(old, probe, npc.size(), yaw)
                && permitted(chunks, journal, job, next)
                && work_floor(job, next)
            {
                npc.position = next;
                npc.yaw = yaw as f32;
                npc.velocity = std::array::from_fn(|i| ((next[i] - old[i]) / f64::from(dt)) as f32);
                walked = true;
                break;
            }
        }
    }
    crate::creatures::work_rest_with_access(npc, &h, dt, |p| permitted(chunks, journal, job, p));
    npc.behavior = if walked {
        route.stalled = false;
        CreatureBehavior::Wander
    } else {
        npc.velocity[0] = 0.0;
        npc.velocity[2] = 0.0;
        CreatureBehavior::Rest
    };
    runtime.routes.insert(npc.id, route);
    false
}

/// Long outside approaches retain the ordinary cached NPC walker. Its body
/// sweep must stay outside the construction footprint until a site route exists.
fn outside_walk(
    h: &Habitat,
    job: &BuildingJob,
    npc: &mut Creature,
    target: [f64; 3],
    dt: f32,
    runtime: &mut BuildRuntime,
) -> bool {
    crate::creatures::work_step_with_access(npc, h, target, dt, &mut runtime.navigation, |p| {
        permitted(h.chunks, h.journal, job, p) && super::site_path::outside(job, p)
    })
}

pub(super) fn clear_for_cells(npc: &Creature, cells: &[BuildCell]) -> bool {
    let size = npc.size();
    !cells.iter().any(|c| {
        let p = c.position.map(|v| v as f64);
        (npc.position[0] - p[0] - 0.5).abs() < size[0] * 0.5 + 0.5 - 0.001
            && (npc.position[2] - p[2] - 0.5).abs() < size[2] * 0.5 + 0.5 - 0.001
            && if c.material == 0 {
                (p[1] + 1.0 - npc.position[1]).abs() < 0.08
            } else {
                p[1] + 1.0 > npc.position[1] + 0.001 && p[1] < npc.position[1] + size[1]
            }
    })
}
