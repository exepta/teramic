//! Blood Forest fauna and crypt guardians: bounded authority steering and strikes.
//! No teleporting, through-wall damage, terrain generation or client-side AI.
use super::{Behavior, Creature, Habitat, Kind, bears::Effect, distance, navigation, random};
use terra_core::creatures::{definitions, intentions::IntentReason};

pub mod graves;

#[cfg(test)]
mod tests;

pub fn hostile(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Skeleton | Kind::BloodStalker | Kind::BloodWisp | Kind::ThornBrute
    )
}

pub(super) fn surface_species(biome: &str, seed: u64) -> Option<Kind> {
    (biome == "blood_forest").then_some(match (seed >> 48) % 10 {
        0..=2 => Kind::Raven,
        3..=5 => Kind::BloodSlime,
        6 => Kind::BloodStalker,
        7 => Kind::BloodWisp,
        8 => Kind::ThornBrute,
        _ => Kind::Skeleton,
    })
}

pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    mut nav: Option<&mut navigation::Navigation>,
) -> Effect {
    if !hostile(a.kind)
        || a.health <= 0.0
        || !dt.is_finite()
        || dt <= 0.0
        || dt > 0.25
        || h.block(a.position.map(|v| v.floor() as i64)).is_none()
    {
        return Effect::None;
    }
    a.age += f64::from(dt);
    let previous_timer = a.timer;
    a.timer = (a.timer - dt).max(0.0);
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    if !a.kind.is_flying() {
        super::support::settle(a, h, dt);
    }
    if search {
        a.needs.search_in = 0.6;
    }
    let Some(combat) = definitions::get(a.kind).combat else {
        return Effect::None;
    };
    let eye = |p: [f64; 3]| [p[0], p[1] + a.size()[1] * 0.65, p[2]];
    let seen = super::perception::nearest_visible(
        h,
        eye(a.position),
        a.position,
        players,
        combat.aggro_radius,
        |_| true,
    );
    let observed = seen.map(|(_, player)| [player[0], player[1] - 0.9, player[2]]);
    if let Some(target) = observed
        && let Some(nav) = nav.as_deref_mut()
    {
        nav.see_enemy(a, target);
    }
    let remembered =
        if observed.is_none() && matches!(a.behavior, Behavior::Hunt | Behavior::Attack) {
            nav.as_deref_mut().and_then(|nav| nav.remembered_enemy(a))
        } else {
            None
        };
    if let Some(target) = observed.or(remembered) {
        let visible = observed.is_some();
        a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2]) as f32)
            .rem_euclid(std::f32::consts::TAU);
        if visible && distance(a.position, target) <= combat.reach.powi(2) {
            let entering = a.behavior != Behavior::Attack;
            a.behavior = Behavior::Attack;
            a.velocity = [0.0; 3];
            if let Some(nav) = nav.as_deref_mut() {
                nav.intend(a, IntentReason::Attack, Some(target));
            }
            // One authority hit at the end of the visible wind-up. Stepping
            // away or breaking sight cancels the swing instead of banking an
            // instant hit for the next approach.
            if combat.melee_contact(entering, previous_timer, &mut a.timer) {
                let multiplier = if a.kind == Kind::Skeleton {
                    [1.0, 1.3, 1.7][usize::from(a.variant.min(2))]
                } else {
                    1.0
                };
                return Effect::StrikePlayer {
                    index: seen.unwrap().0,
                    damage: combat.damage * multiplier,
                };
            }
        } else {
            a.behavior = Behavior::Hunt;
            if let Some(nav) = nav.as_deref_mut() {
                nav.intend(a, IntentReason::Hunt, Some(target));
            }
            if a.kind.is_flying() {
                // Local swept steering remains cheap even without a ground-search grant.
                fly_toward(a, h, [target[0], target[1] + 0.75, target[2]], dt);
            } else {
                travel(a, h, target, dt, search, nav);
            }
        }
        return Effect::None;
    }
    // Patrol a deterministic home-relative arc; timers and age persist on save.
    if matches!(a.behavior, Behavior::Hunt | Behavior::Attack) {
        a.timer = 0.8;
        a.behavior = Behavior::Rest;
    }
    let seed = random(a.id, (a.age / 5.0) as u64);
    let angle = (seed % 6283) as f64 * 0.001;
    let target = [
        a.home[0] as f64 * 32.0 + 16.0 + angle.sin() * 8.0,
        a.position[1],
        a.home[1] as f64 * 32.0 + 16.0 + angle.cos() * 8.0,
    ];
    if a.timer <= 0.0 {
        a.behavior = Behavior::Wander;
    }
    if a.behavior == Behavior::Wander {
        if a.kind.is_flying() {
            fly_toward(a, h, target, dt);
        } else {
            travel(a, h, target, dt, search, nav);
        }
    } else {
        a.velocity = [0.0; 3];
    }
    Effect::None
}

fn fly_toward(a: &mut Creature, h: &Habitat, target: [f64; 3], dt: f32) {
    let before = a.position;
    let length = distance(before, target).sqrt().max(0.001);
    let speed = a.kind.speed()
        * if a.behavior == Behavior::Hunt {
            1.85
        } else {
            0.65
        };
    let next = std::array::from_fn(|i| {
        before[i] + (target[i] - before[i]) / length * (speed * f64::from(dt)).min(length)
    });
    // Three probes along one small movement prevent clipping at voxel boundaries.
    let swept = |to: [f64; 3]| {
        (1..=3).all(|step| {
            let p =
                std::array::from_fn(|i| before[i] + (to[i] - before[i]) * f64::from(step) / 3.0);
            h.clear(p, a.size(), false) && !h.wet(p)
        })
    };
    if swept(next) {
        a.position = next;
    } else {
        // Try ascending beside an obstruction, never crossing its opaque volume.
        let above = [before[0], before[1] + speed * f64::from(dt), before[2]];
        if swept(above) {
            a.position = above;
        }
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
    if a.velocity[0].abs() + a.velocity[2].abs() > 0.01 {
        a.yaw = (-a.velocity[0])
            .atan2(-a.velocity[2])
            .rem_euclid(std::f32::consts::TAU);
    }
}

fn travel(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    search: bool,
    mut nav: Option<&mut navigation::Navigation>,
) {
    let before = a.position;
    if h.supported_sized(before, a.size(), f64::from(a.yaw))
        .is_none()
    {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        return;
    }
    let advance = |goal: [f64; 3]| {
        let dx = goal[0] - before[0];
        let dz = goal[2] - before[2];
        let length = dx.hypot(dz).max(0.001);
        let chase = a.behavior == Behavior::Hunt;
        let reach = definitions::get(a.kind).combat.map_or(1.5, |c| c.reach);
        // Run to close distance, then brake near striking range. The existing
        // swept, supported navigation still validates every movement.
        let pace = if chase && distance(before, target) > (reach + 1.0).powi(2) {
            2.4
        } else if chase {
            1.35
        } else {
            0.65
        };
        let amount = (a.kind.speed() * pace * f64::from(dt)).min(length);
        let next = [
            before[0] + dx / length * amount,
            before[1],
            before[2] + dz / length * amount,
        ];
        h.travel_step(before, next, a.kind, (-dx).atan2(-dz))
    };
    let cached = nav.as_ref().is_some_and(|n| n.following_ground_route(a.id));
    let detour = if cached {
        navigation::ground_waypoint(a, h, target, false, nav.as_deref_mut()).and_then(advance)
    } else {
        None
    };
    let next = detour
        .or_else(|| advance(target))
        .or_else(|| navigation::ground_waypoint(a, h, target, search, nav).and_then(advance));
    a.velocity = [0.0; 3];
    if let Some(p) = next {
        a.position = p;
        a.velocity = std::array::from_fn(|i| ((p[i] - before[i]) / f64::from(dt)) as f32);
        a.yaw =
            ((before[0] - p[0]).atan2(before[2] - p[2]) as f32).rem_euclid(std::f32::consts::TAU);
    }
}
