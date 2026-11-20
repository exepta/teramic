//! Owner-aligned villagers wander loaded Build claims without requiring generated beds.
//! Goals and pauses reuse the creature's persistent schedule; paths remain transient.
use super::*;
use terra_core::{
    PlayerUuid,
    kingdom::{ClaimKind, KingdomPermission, KingdomState, claim_chunk},
};

struct Area {
    center: [i64; 2],
    build: [[bool; 3]; 3],
}

impl Area {
    fn new(resident: &Creature, habitat: &Habitat, actor: PlayerUuid) -> Option<Self> {
        let registry = &habitat.journal.gameplay().kingdoms;
        let realm = registry.active_for_member(actor)?;
        if !registry.role_for(realm.id, actor)?.permissions().npc_work {
            return None;
        }
        let center = claim_chunk(resident.position.map(|v| v.floor() as i64));
        let mut area = Self {
            center,
            build: [[false; 3]; 3],
        };
        // One bounded registry pass per scheduled guard step; physical sweeps
        // subsequently use this nine-cell stack map, never rescan every claim.
        for claim in &registry.claims {
            let dx = claim.chunk[0] - center[0];
            let dz = claim.chunk[1] - center[1];
            if claim.kind == ClaimKind::ProtectedBuilding
                && (-1..=1).contains(&dx)
                && (-1..=1).contains(&dz)
                && registry.realm_id(claim.kingdom) == realm.id
                && registry
                    .get(claim.kingdom)
                    .is_some_and(|record| record.state == KingdomState::Active)
            {
                area.build[(dx + 1) as usize][(dz + 1) as usize] = true;
            }
        }
        area.build[1][1].then_some(area)
    }

    fn owns(&self, point: [f64; 3]) -> bool {
        [-0.3, 0.3].into_iter().all(|x| {
            [-0.3, 0.3].into_iter().all(|z| {
                let cell = [
                    (point[0] + x).floor() as i64,
                    point[1].floor() as i64,
                    (point[2] + z).floor() as i64,
                ];
                let chunk = claim_chunk(cell);
                let dx = chunk[0] - self.center[0];
                let dz = chunk[1] - self.center[1];
                (-1..=1).contains(&dx)
                    && (-1..=1).contains(&dz)
                    && self.build[(dx + 1) as usize][(dz + 1) as usize]
            })
        })
    }
}

fn goal(resident: &Creature, habitat: &Habitat, area: &Area) -> Option<[i64; 3]> {
    const POSTS: [[f64; 2]; 13] = [
        [8.5, 8.5],
        [3.5, 3.5],
        [8.5, 3.5],
        [12.5, 3.5],
        [12.5, 8.5],
        [12.5, 12.5],
        [8.5, 12.5],
        [3.5, 12.5],
        [3.5, 8.5],
        [-7.5, 8.5],
        [24.5, 8.5],
        [8.5, -7.5],
        [8.5, 24.5],
    ];
    let first =
        (resident.id.wrapping_add((resident.age / 3.0) as u64) % POSTS.len() as u64) as usize;
    for offset in 0..POSTS.len() {
        let [x, z] = POSTS[(first + offset) % POSTS.len()];
        let point = [
            area.center[0] as f64 * 16.0 + x,
            resident.position[1],
            area.center[1] as f64 * 16.0 + z,
        ];
        if distance(point, resident.position) < 1.5_f64.powi(2) || !area.owns(point) {
            continue;
        }
        if let Some(point) = habitat
            .ground(point, resident.kind)
            .filter(|point| area.owns(*point))
        {
            return Some(point.map(|v| v.floor() as i64));
        }
    }
    None
}

fn rest(resident: &mut Creature, habitat: &Habitat, dt: f32, allowed: impl Fn([f64; 3]) -> bool) {
    resident.velocity[0] = 0.0;
    resident.velocity[2] = 0.0;
    resident.behavior = Behavior::Rest;
    support::settle_with_access(resident, habitat, dt, allowed);
}

fn commanded(resident: &Creature, habitat: &Habitat) -> bool {
    let game = habitat.journal.gameplay();
    game.pet_orders
        .get(&resident.id)
        .is_some_and(|mode| *mode != terra_core::creatures::taming::CompanionMode::None)
        || crate::npc_work::controls_movement(game, resident.id, 0.5)
        || game.buildings.is_busy(resident.id)
}

/// Crowd separation runs after scheduling and must retain the same Build border.
pub(in crate::creatures) fn permits_idle_push(
    resident: &Creature,
    habitat: &Habitat,
    actor: PlayerUuid,
    next: [f64; 3],
) -> bool {
    commanded(resident, habitat)
        || Area::new(resident, habitat, actor)
            .is_some_and(|area| area.owns(resident.position) && area.owns(next))
}

pub(super) fn step(
    resident: &mut Creature,
    habitat: &Habitat,
    dt: f32,
    search: bool,
    nav: &mut navigation::Navigation,
    actor: PlayerUuid,
) -> Option<[i64; 3]> {
    // Explicit commands and productive assignments take priority. This also
    // leaves a clear integration point for later military command protocols.
    if commanded(resident, habitat) {
        nav.invalidate(resident.id);
        resident.needs.goal = None;
        rest(resident, habitat, dt, |_| true);
        return None;
    }
    let Some(area) =
        Area::new(resident, habitat, actor).filter(|area| area.owns(resident.position))
    else {
        nav.invalidate(resident.id);
        resident.needs.goal = None;
        let start = resident.position;
        rest(resident, habitat, dt, |point| {
            point[0] == start[0] && point[2] == start[2]
        });
        return None;
    };
    if resident.needs.goal.is_some_and(|goal| {
        let point = [
            goal.position[0] as f64 + 0.5,
            goal.position[1] as f64,
            goal.position[2] as f64 + 0.5,
        ];
        !area.owns(point) || habitat.ground(point, resident.kind).is_none()
    }) {
        resident.needs.goal = None;
        nav.invalidate(resident.id);
    }
    if resident.timer > 0.0 {
        rest(resident, habitat, dt, |point| area.owns(point));
        return None;
    }
    if resident.needs.goal.is_none() && search {
        resident.needs.goal = goal(resident, habitat, &area).map(|position| NeedGoal {
            kind: NeedKind::Food,
            position,
        });
        resident.needs.search_in = 0.8;
    }
    let Some(goal) = resident.needs.goal else {
        rest(resident, habitat, dt, |point| area.owns(point));
        return None;
    };
    let target = [
        goal.position[0] as f64 + 0.5,
        goal.position[1] as f64,
        goal.position[2] as f64 + 0.5,
    ];
    if navigation::ground_arrived(resident, habitat, target, 0.4) {
        resident.needs.goal = None;
        resident.timer = 0.65 + (resident.id % 4) as f32 * 0.15;
        nav.invalidate(resident.id);
        rest(resident, habitat, dt, |point| area.owns(point));
        return None;
    }
    let door = door(resident, habitat, &area, actor, target);
    walk_with_access(resident, habitat, target, dt, search, false, nav, |point| {
        area.owns(point)
    });
    support::settle_with_access(resident, habitat, dt, |point| area.owns(point));
    if nav.stalled(resident.id, resident.age) {
        resident.needs.goal = None;
        resident.timer = 0.6;
        resident.needs.search_in = 0.3;
        nav.invalidate(resident.id);
    }
    door
}

fn door(
    resident: &Creature,
    habitat: &Habitat,
    area: &Area,
    actor: PlayerUuid,
    target: [f64; 3],
) -> Option<[i64; 3]> {
    let dx = target[0] - resident.position[0];
    let dz = target[2] - resident.position[2];
    let length = dx.hypot(dz);
    if length < 0.00001 {
        return None;
    }
    let direction = [dx / length, dz / length];
    for distance in [0.65, 1.0, 1.4, 1.8] {
        let cell = [
            (resident.position[0] + direction[0] * distance).floor() as i64,
            resident.position[1].floor() as i64,
            (resident.position[2] + direction[1] * distance).floor() as i64,
        ];
        let Some(material) = habitat
            .block(cell)
            .filter(|material| crate::npc_work::worker_openable_door(*material))
        else {
            continue;
        };
        let state = habitat.journal.object_state(cell)?;
        if crate::objects::is_open(material, Some(state)) {
            continue;
        }
        let group = crate::objects::doors::group(cell, material, state, |p| {
            Some((habitat.block(p)?, habitat.journal.object_state(p)?))
        })?;
        if group.cells().all(|p| {
            habitat.block(p) == Some(material)
                && area.owns([p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5])
                && habitat
                    .journal
                    .gameplay()
                    .kingdoms
                    .can(actor, KingdomPermission::Openings, p)
        }) {
            return Some(group.anchor);
        }
    }
    None
}

#[cfg(test)]
#[path = "patrol_tests.rs"]
mod tests;
