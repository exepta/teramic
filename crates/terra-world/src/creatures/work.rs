use super::{Creature, Habitat, Kind, navigation::Navigation, support, villagers};

/// Idle work still advances route expiry and gravity; ordinary nighttime residents do not call this.
pub fn work_rest(npc: &mut Creature, habitat: &Habitat, dt: f32) {
    work_rest_with_access(npc, habitat, dt, |_| true);
}

/// Territorial rules also apply to gravity and forced overlap recovery.
pub fn work_rest_with_access(
    npc: &mut Creature,
    habitat: &Habitat,
    dt: f32,
    allowed: impl Fn([f64; 3]) -> bool,
) {
    if npc.kind != Kind::Villager
        || npc.health <= 0.0
        || !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
    {
        return;
    }
    npc.age += f64::from(dt);
    support::settle_with_access(npc, habitat, dt, allowed);
}

/// Productive workers reuse ordinary physical walking, cached A* and gravity.
pub fn work_step(
    npc: &mut Creature,
    habitat: &Habitat,
    destination: [f64; 3],
    dt: f32,
    navigation: &mut Navigation,
) -> bool {
    work_step_with_access(npc, habitat, destination, dt, navigation, |_| true)
}

/// Every planned and physical edge must remain inside the actor's authorized ground.
pub fn work_step_with_access(
    npc: &mut Creature,
    habitat: &Habitat,
    destination: [f64; 3],
    dt: f32,
    navigation: &mut Navigation,
    allowed: impl Fn([f64; 3]) -> bool,
) -> bool {
    if npc.kind != Kind::Villager
        || npc.health <= 0.0
        || !dt.is_finite()
        || !(0.001..=0.25).contains(&dt)
    {
        return false;
    }
    npc.age += f64::from(dt);
    npc.needs.search_in = (npc.needs.search_in - dt).max(0.0);
    // A new work goal has no search result to throttle. In particular, a
    // changed standing point must not inherit the previous point's cooldown.
    // Cached searches retain the ordinary per-actor retry interval.
    let search = npc.needs.search_in == 0.0
        || navigation.ground_state(npc.id, npc.age)
            == super::navigation::GroundRouteState::Unplanned;
    if search {
        npc.needs.search_in = 0.8;
    }
    let before = npc.position;
    // Resident walking intentionally stops within five centimetres. Work
    // interaction rays need the requested supported stance itself, especially
    // beside a terrace edge or a narrow trunk face. Finish only this tiny real
    // segment, with the same body/claim sweeps and ordinary walking speed.
    if (destination[0] - before[0]).hypot(destination[2] - before[2]) <= 0.08
        && (destination[1] - before[1]).abs() <= 0.125
        && supported_here(npc, habitat)
        && physical_work_step(npc, habitat, destination, dt, &allowed)
    {
        support::settle_with_access(npc, habitat, dt, &allowed);
        return true;
    }
    // Contact support can keep a real foot on the edge while the strict land
    // graph cannot start there. Finish recovery before attempting another goal
    // step, otherwise a failed descent and an inward correction can oscillate.
    if regain_footing(npc, habitat, dt, &allowed) {
        navigation.invalidate(npc.id);
        support::settle_with_access(npc, habitat, dt, &allowed);
        return true;
    }
    let yaw = (npc.position[0] - destination[0]).atan2(npc.position[2] - destination[2]);
    let direct = habitat
        .walk_step_sized(npc.position, destination, npc.size(), yaw)
        .is_some_and(|p| (p[1] - destination[1]).abs() <= 0.125);
    if !direct
        && super::navigation::ground_waypoint_with_access(
            npc,
            habitat,
            destination,
            search,
            Some(navigation),
            &allowed,
        )
        .is_none()
    {
        npc.velocity[0] = 0.0;
        npc.velocity[2] = 0.0;
        npc.behavior = super::Behavior::Rest;
        support::settle_with_access(npc, habitat, dt, &allowed);
        return false;
    }
    villagers::walk_with_access(
        npc,
        habitat,
        destination,
        dt,
        search,
        false,
        navigation,
        &allowed,
    );
    support::settle_with_access(npc, habitat, dt, &allowed);
    let moved = (npc.position[0] - before[0]).hypot(npc.position[2] - before[2]) > 0.00001;
    if !moved && regain_footing(npc, habitat, dt, &allowed) {
        navigation.invalidate(npc.id);
        return true;
    }
    moved
}

fn supported_here(npc: &Creature, h: &Habitat) -> bool {
    let same = |p: [f64; 3]| (p[1] - npc.position[1]).abs() < 0.08;
    h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
        .is_some_and(same)
        || h.supported_sized(npc.position, npc.size(), f64::from(npc.yaw))
            .is_some_and(same)
}
fn physical_work_step(
    npc: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    allowed: &impl Fn([f64; 3]) -> bool,
) -> bool {
    let old = npc.position;
    let length = (target[0] - old[0]).hypot(target[2] - old[2]);
    if length < 0.00001 {
        return false;
    }
    let t = (npc.kind.speed() * f64::from(dt)).min(length) / length;
    let probe = [
        old[0] + (target[0] - old[0]) * t,
        old[1],
        old[2] + (target[2] - old[2]) * t,
    ];
    // A sideways correction preserves a ledge's last actual foot contact.
    let yaw = f64::from(npc.yaw);
    let Some(next) = h.travel_step_sized(old, probe, npc.size(), yaw) else {
        return false;
    };
    if (next[1] - old[1]).abs() > 1.01 {
        return false;
    }
    let count = (length.min(npc.kind.speed() * f64::from(dt)) / 0.05)
        .ceil()
        .max(1.0) as usize;
    if !(0..=count).all(|n| {
        allowed(std::array::from_fn(|i| {
            old[i] + (next[i] - old[i]) * n as f64 / count as f64
        }))
    }) {
        return false;
    }
    npc.position = next;
    npc.velocity = [
        ((next[0] - old[0]) / f64::from(dt)) as f32,
        0.0,
        ((next[2] - old[2]) / f64::from(dt)) as f32,
    ];
    npc.behavior = super::Behavior::Wander;
    true
}
fn regain_footing(
    npc: &mut Creature,
    h: &Habitat,
    dt: f32,
    allowed: &impl Fn([f64; 3]) -> bool,
) -> bool {
    if h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
        .is_some_and(|p| (p[1] - npc.position[1]).abs() < 0.08)
        || !supported_here(npc, h)
    {
        return false;
    }
    let mut candidates = Vec::with_capacity(9);
    for x in -1..=1 {
        for z in -1..=1 {
            let seed = [
                npc.position[0].floor() + f64::from(x) + 0.5,
                npc.position[1],
                npc.position[2].floor() + f64::from(z) + 0.5,
            ];
            if let Some(p) = h.ground_sized(seed, npc.size(), f64::from(npc.yaw))
                && (p[1] - npc.position[1]).abs() <= 1.01
                && (p[0] - npc.position[0]).hypot(p[2] - npc.position[2]) <= 1.01
                && allowed(p)
                && !h.wet(p)
            {
                candidates.push(p);
            }
        }
    }
    candidates.sort_by(|a, b| {
        super::distance(*a, npc.position).total_cmp(&super::distance(*b, npc.position))
    });
    candidates
        .into_iter()
        .any(|p| physical_work_step(npc, h, p, dt, allowed))
}
