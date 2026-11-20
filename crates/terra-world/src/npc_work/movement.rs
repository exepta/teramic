use super::*;

#[derive(Debug, Clone, Copy)]
struct FailedRoute {
    reason: WaitReason,
    origin: [i64; 3],
    stamp: u64,
}

#[derive(Debug, Default)]
pub(super) struct Travel {
    key: Option<(u64, u64, [i64; 3], bool)>,
    candidates: Vec<[f64; 3]>,
    index: usize,
    progress: [f64; 3],
    progress_at: f64,
    retry_at: f64,
    unavailable: Option<WaitReason>,
    failure: Option<FailedRoute>,
}

/// Fixed failure origins keep retries independent of sideways movement along a
/// wall. Actual voxel/loading/opening/claim changes invalidate the result.
fn route_stamp(h: &Habitat, origin: [i64; 3], cell: [i64; 3]) -> u64 {
    let mut stamp = 0xcbf2_9ce4_8422_2325_u64;
    let mut add = |value: u64| {
        stamp = (stamp ^ value).wrapping_mul(0x100_0000_01b3);
    };
    add(h.journal.gameplay().kingdoms.revision);
    for center in [origin, cell].map(|p| terra_voxel::global_to_section(p).0) {
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let Ok(section) = center.offset([x, y, z]) else {
                        continue;
                    };
                    add(section.x as u64);
                    add(section.y as u64);
                    add(section.z as u64);
                    let loaded = h.chunks.loaded.get(&section);
                    add(u64::from(loaded.is_some()));
                    add(loaded.map_or(0, |s| s.data.revision()));
                    add(h.journal.revision(section));
                    add(h.journal.fluid_epoch(section));
                }
            }
        }
    }
    stamp
}
fn progressed(npc: &Creature, cell: [i64; 3], failure: FailedRoute) -> bool {
    let goal = cell.map(|v| v as f64 + 0.5);
    let origin = failure.origin.map(|v| v as f64 + 0.5);
    crate::creatures::distance(origin, goal).sqrt()
        - crate::creatures::distance(npc.position, goal).sqrt()
        > 1.0
}
fn fail_route(
    travel: &mut Travel,
    h: &Habitat,
    npc: &Creature,
    cell: [i64; 3],
    reason: WaitReason,
) {
    let origin = travel
        .failure
        .filter(|f| f.reason == reason)
        .map_or(npc.position.map(|v| v.floor() as i64), |f| f.origin);
    travel.failure = Some(FailedRoute {
        reason,
        origin,
        stamp: route_stamp(h, origin, cell),
    });
    travel.unavailable = Some(reason);
    travel.retry_at = npc.age + 1.0;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn approach(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    cell: [i64; 3],
    dt: f32,
    runtime: &mut WorkRuntime,
    storage: bool,
) -> bool {
    let habitat = Habitat {
        terrain,
        chunks,
        journal,
    };
    if chunks.block_at(cell).is_none() || !loaded_body(&habitat, npc.position, npc.size()) {
        runtime.navigation.invalidate(npc.id);
        wait(worker, npc, WaitReason::Unloaded);
        return false;
    }
    if !position_allowed(&habitat, order, npc.position, npc.size()) {
        runtime.navigation.invalidate(npc.id);
        wait(worker, npc, WaitReason::Protected);
        return false;
    }
    if reachable(&habitat, npc, cell) {
        runtime.travel.remove(&npc.id);
        runtime.navigation.invalidate(npc.id);
        npc.velocity = [0.0; 3];
        npc.behavior = CreatureBehavior::Rest;
        return true;
    }
    let key = (order.id, order.revision, cell, storage);
    let travel = runtime.travel.entry(npc.id).or_default();
    if travel.key != Some(key) {
        *travel = Travel {
            key: Some(key),
            progress: npc.position,
            progress_at: npc.age,
            ..Default::default()
        };
        runtime.navigation.invalidate(npc.id);
    }
    if travel.failure.is_some_and(|f| {
        f.stamp != route_stamp(&habitat, f.origin, cell) || progressed(npc, cell, f)
    }) {
        travel.failure = None;
        travel.unavailable = None;
        travel.retry_at = 0.0;
        travel.candidates.clear();
        runtime.navigation.invalidate(npc.id);
    }
    if travel.retry_at > npc.age {
        wait(
            worker,
            npc,
            travel
                .failure
                .map(|f| f.reason)
                .or(travel.unavailable)
                .unwrap_or(WaitReason::NoPath),
        );
        return false;
    }
    if travel.candidates.is_empty() {
        let (candidates, reason) = standing_points(&habitat, order, npc, cell);
        travel.candidates = candidates;
        travel.unavailable = reason;
        travel.index = 0;
        travel.progress = npc.position;
        travel.progress_at = npc.age;
    }
    while travel.index < travel.candidates.len()
        && (!position_allowed(&habitat, order, travel.candidates[travel.index], npc.size())
            || !reachable_from(&habitat, npc, travel.candidates[travel.index], cell))
    {
        travel.index += 1;
        runtime.navigation.invalidate(npc.id);
    }
    let Some(&target) = travel.candidates.get(travel.index) else {
        travel.candidates.clear();
        travel.retry_at = npc.age + 1.0;
        let reason = travel.unavailable.unwrap_or_else(|| {
            if loaded_line(&habitat, npc.position, cell.map(|v| v as f64 + 0.5)) {
                WaitReason::NoPath
            } else {
                WaitReason::Unloaded
            }
        });
        fail_route(travel, &habitat, npc, cell, reason);
        wait(worker, npc, reason);
        return false;
    };
    // Real collision remains until the server confirms the ordinary door transaction.
    if let Some(position) = nearby_door(&habitat, order, npc)
        && runtime.door_requests.len() < WORKER_BUDGET
    {
        runtime
            .door_requests
            .entry(position)
            .or_insert(WorkerDoorRequest {
                npc: npc.id,
                owner: order.owner,
                position,
            });
    }
    worker.status = travel.failure.map_or(
        if storage {
            WorkStatus::Delivering
        } else {
            WorkStatus::Travelling
        },
        |failure| WorkStatus::Waiting(failure.reason),
    );
    worker.work_seconds = 0.0;
    let size = npc.size();
    let moved = crate::creatures::work_step_with_access(
        npc,
        &habitat,
        target,
        dt,
        &mut runtime.navigation,
        |point| position_allowed(&habitat, order, point, size),
    );
    if crate::creatures::distance(travel.progress, npc.position) > 0.04_f64.powi(2) {
        travel.progress = npc.position;
        travel.progress_at = npc.age;
    } else if !moved {
        let awaiting_door = runtime
            .door_requests
            .values()
            .any(|request| request.npc == npc.id);
        let failed = matches!(
            runtime.navigation.ground_state(npc.id, npc.age),
            crate::creatures::navigation::GroundRouteState::Blocked
        );
        if !awaiting_door && (failed || npc.age - travel.progress_at > 2.5) {
            // One failed standing point is not a failed job. Try its other real
            // sides before publishing NoPath; AwaitingSearch remains in transit.
            travel.index += 1;
            travel.progress_at = npc.age;
            runtime.navigation.invalidate(npc.id);
            if travel.index >= travel.candidates.len() {
                travel.candidates.clear();
                travel.retry_at = npc.age + 1.0;
                let reason = if loaded_line(&habitat, npc.position, target) {
                    WaitReason::NoPath
                } else {
                    WaitReason::Unloaded
                };
                fail_route(travel, &habitat, npc, cell, reason);
                wait(worker, npc, reason);
            }
        }
        npc.velocity[0] = 0.0;
        npc.velocity[2] = 0.0;
        npc.behavior = CreatureBehavior::Rest;
    }
    false
}

fn standing_points(
    habitat: &Habitat,
    order: &WorkOrder,
    npc: &Creature,
    cell: [i64; 3],
) -> (Vec<[f64; 3]>, Option<WaitReason>) {
    let mut candidates = Vec::with_capacity(48);
    let mut unloaded = false;
    let mut protected = false;
    for dy in [-4, -3, -2, -1, 0, 1] {
        for dx in -1_i32..=1 {
            for dz in -1_i32..=1 {
                let point = [
                    cell[0] as f64 + 0.5 + f64::from(dx),
                    cell[1] as f64 + f64::from(dy),
                    cell[2] as f64 + 0.5 + f64::from(dz),
                ];
                unloaded |= !loaded_body(habitat, point, npc.size())
                    || habitat
                        .block([
                            point[0].floor() as i64,
                            point[1].floor() as i64 - 2,
                            point[2].floor() as i64,
                        ])
                        .is_none();
                if let Some(point) = habitat.ground_sized(point, npc.size(), f64::from(npc.yaw))
                    && reachable_from(habitat, npc, point, cell)
                {
                    if !position_allowed(habitat, order, point, npc.size()) {
                        protected = true;
                        continue;
                    }
                    if !candidates
                        .iter()
                        .any(|old| crate::creatures::distance(*old, point) < 0.01)
                    {
                        candidates.push(point);
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        crate::creatures::distance(*a, npc.position)
            .total_cmp(&crate::creatures::distance(*b, npc.position))
    });
    let reason = candidates.is_empty().then_some(if unloaded {
        WaitReason::Unloaded
    } else if protected {
        WaitReason::Protected
    } else {
        WaitReason::NoPath
    });
    (candidates, reason)
}

pub(super) fn position_allowed(
    habitat: &Habitat,
    order: &WorkOrder,
    point: [f64; 3],
    size: [f64; 3],
) -> bool {
    let radius = size[0].hypot(size[2]) * 0.5;
    [-1.0, 1.0].into_iter().all(|x| {
        [-1.0, 1.0].into_iter().all(|z| {
            let cell = [
                (point[0] + x * radius).floor() as i64,
                point[1].floor() as i64,
                (point[2] + z * radius).floor() as i64,
            ];
            allowed(
                habitat.chunks,
                habitat.journal,
                order,
                KingdomPermission::NpcWork,
                cell,
            )
        })
    })
}

fn loaded_body(habitat: &Habitat, point: [f64; 3], size: [f64; 3]) -> bool {
    if point.iter().chain(size.iter()).any(|v| !v.is_finite()) {
        return false;
    }
    let radius = size[0].hypot(size[2]) * 0.5;
    let low = [
        (point[0] - radius).floor() as i64,
        point[1].floor() as i64,
        (point[2] - radius).floor() as i64,
    ];
    let high = [
        (point[0] + radius).floor() as i64,
        (point[1] + size[1]).floor() as i64,
        (point[2] + radius).floor() as i64,
    ];
    (low[0]..=high[0]).all(|x| {
        (low[1]..=high[1]).all(|y| (low[2]..=high[2]).all(|z| habitat.block([x, y, z]).is_some()))
    })
}

fn nearby_door(habitat: &Habitat, order: &WorkOrder, npc: &Creature) -> Option<[i64; 3]> {
    let cell = npc.position.map(|v| v.floor() as i64);
    let mut nearest: Option<([i64; 3], f64)> = None;
    // Match the authoritative door transaction's four-metre range. The door
    // can be beside or behind the direct goal when the actual route detours.
    for dx in -4..=4 {
        for dz in -4..=4 {
            for dy in 0..=1 {
                let position = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                let Some(material) = habitat
                    .block(position)
                    .filter(|id| worker_openable_door(*id))
                else {
                    continue;
                };
                let Some(state) = habitat
                    .journal
                    .object_state(position)
                    .filter(|s| s.facing < 4)
                else {
                    continue;
                };
                let Some(group) = crate::objects::doors::group(position, material, state, |p| {
                    Some((habitat.block(p)?, habitat.journal.object_state(p)?))
                }) else {
                    continue;
                };
                let distance =
                    crate::creatures::distance(npc.position, group.anchor.map(|v| v as f64 + 0.5));
                if distance > 4.0_f64.powi(2) || nearest.is_some_and(|(_, best)| best <= distance) {
                    continue;
                }
                if group.cells().all(|p| {
                    habitat.block(p) == Some(material)
                        && allowed(
                            habitat.chunks,
                            habitat.journal,
                            order,
                            KingdomPermission::NpcWork,
                            p,
                        )
                        && allowed(
                            habitat.chunks,
                            habitat.journal,
                            order,
                            KingdomPermission::Openings,
                            p,
                        )
                }) {
                    nearest = Some((group.anchor, distance));
                }
            }
        }
    }
    nearest.map(|(position, _)| position)
}

fn loaded_line(habitat: &Habitat, from: [f64; 3], to: [f64; 3]) -> bool {
    let length = crate::creatures::distance(from, to).sqrt();
    let steps = (length / 0.2).ceil().clamp(1.0, 512.0) as usize;
    (0..=steps).all(|step| {
        habitat
            .block(std::array::from_fn(|axis| {
                (from[axis] + (to[axis] - from[axis]) * step as f64 / steps as f64).floor() as i64
            }))
            .is_some()
    })
}

fn reachable(habitat: &Habitat, npc: &Creature, cell: [i64; 3]) -> bool {
    reachable_from(habitat, npc, npc.position, cell)
}

fn reachable_from(habitat: &Habitat, npc: &Creature, position: [f64; 3], cell: [i64; 3]) -> bool {
    let center = cell.map(|v| v as f64 + 0.5);
    if position.iter().any(|v| !v.is_finite())
        || (position[0] - center[0]).hypot(position[2] - center[2]) > 1.8
        || !loaded_body(habitat, position, npc.size())
        || habitat
            .ground_sized(position, npc.size(), f64::from(npc.yaw))
            .is_none_or(|ground| (ground[1] - position[1]).abs() > 0.08)
    {
        return false;
    }
    let eye = [position[0], position[1] + 1.25, position[2]];
    let direction = std::array::from_fn(|axis| center[axis] - eye[axis]);
    let reach = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if reach > 3.5 {
        return false;
    }
    let mut loaded = true;
    let hit = terra_voxel::blocks::raycast_blocks(eye, direction, reach + 0.01, |p| {
        let block = habitat.block(p);
        loaded &= block.is_some();
        block
    });
    loaded && hit.is_none_or(|hit| hit.position == cell)
}
