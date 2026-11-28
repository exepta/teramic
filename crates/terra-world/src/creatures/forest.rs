//! Blood Forest fauna and crypt guardians: bounded authority steering and strikes.
//! No teleporting, through-wall damage, terrain generation or client-side AI.
use super::{Behavior, Creature, Habitat, Kind, bears::Effect, distance, navigation, random};
use bevy_math::DVec3;
use terra_core::creatures::{definitions, intentions::IntentReason};

pub mod graves;

#[cfg(test)]
mod tests;

pub fn hostile(kind: Kind) -> bool {
    kind.is_biome_predator()
        || matches!(
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
        _ => Kind::Raven,
    })
}

pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    nav: Option<&mut navigation::Navigation>,
) -> Effect {
    step_observed(
        a,
        h,
        players.iter().copied().take(128).map(Target::player),
        dt,
        search,
        nav,
        |_| {},
    )
}

#[derive(Clone, Copy)]
pub struct Target {
    pub position: [f64; 3],
    pub aim: [f64; 3],
}

impl Target {
    pub fn player([x, y, z]: [f64; 3]) -> Self {
        Self {
            position: [x, y - 0.9, z],
            aim: [x, y, z],
        }
    }

    pub fn creature(position: [f64; 3], height: f64) -> Self {
        let [x, y, z] = position;
        Self {
            position,
            aim: [x, y + height * 0.55, z],
        }
    }
}

pub fn ready_to_strike(a: &Creature, h: &Habitat, target: Target) -> bool {
    a.kind.is_biome_predator()
        && a.behavior == Behavior::Attack
        && a.health > 0.0
        && definitions::get(a.kind).combat.is_some_and(|combat| {
            let [x, y, z] = a.position;
            distance(a.position, target.position) <= combat.reach.powi(2)
                && super::perception::in_melee_arc(a.yaw, a.position, target.position)
                && super::perception::visible(h, [x, y + a.size()[1] * 0.65, z], target.aim)
        })
}

pub fn step_observed(
    a: &mut Creature,
    h: &Habitat,
    targets: impl Iterator<Item = Target> + Clone,
    dt: f32,
    search: bool,
    mut nav: Option<&mut navigation::Navigation>,
    observe: impl FnOnce(usize),
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
    if !a.kind.is_flying() && !(a.kind == Kind::Crocodile && super::support::swimming(a, h)) {
        super::support::settle(a, h, dt);
    }
    if search {
        a.needs.search_in = 0.6;
    }
    if a.kind == Kind::Skeleton && a.behavior == Behavior::Emerge {
        a.velocity = [0.0; 3];
        if a.timer == 0.0 {
            a.behavior = Behavior::Rest;
        }
        return Effect::None;
    }
    let Some(combat) = definitions::get(a.kind).combat else {
        return Effect::None;
    };
    let [x, y, z] = a.position;
    let eye = [x, y + a.size()[1] * 0.65, z];
    let committed = a.behavior == Behavior::Attack;
    // Once facing is locked, resolve the swing against a visible target in
    // its arc. A closer enemy outside the arc must not cancel that swing.
    let predator = a.kind.is_biome_predator();
    let swing_target = if committed {
        super::perception::nearest_visible_target(
            h,
            eye,
            if predator {
                a.position
            } else {
                [x, y + 0.9, z]
            },
            targets.clone(),
            combat.reach,
            |target| super::perception::in_melee_arc(a.yaw, a.position, target.position),
            |target| {
                if predator {
                    target.position
                } else {
                    target.aim
                }
            },
            |target| target.aim,
        )
    } else {
        None
    };
    let seen = swing_target.or_else(|| {
        let chasing = matches!(a.behavior, Behavior::Hunt | Behavior::Attack);
        super::perception::nearest_visible_target(
            h,
            eye,
            a.position,
            targets,
            combat.aggro_radius * if chasing { 1.25 } else { 1.0 },
            |_| true,
            |target| {
                if predator {
                    target.position
                } else {
                    target.aim
                }
            },
            |target| target.aim,
        )
    });
    let observed = seen.map(|(_, target)| target.position);
    if let Some((index, _)) = seen {
        observe(index);
    }
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
        if !committed {
            let yaw = (a.position[0] - target[0]).atan2(a.position[2] - target[2]);
            if !predator || h.clear_swimmer_rotated(a.position, a.size(), yaw) {
                a.yaw = (yaw as f32).rem_euclid(std::f32::consts::TAU);
            }
        }
        if visible
            && distance(a.position, target) <= combat.reach.powi(2)
            && super::perception::in_melee_arc(a.yaw, a.position, target)
        {
            let entering = !committed;
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
        // A low branch or crypt ceiling also blocks a vertical detour. Sweep
        // the flanks and lower passage, keeping one handedness at equal cost.
        let forward = DVec3::from_array(target) - DVec3::from_array(before);
        let heading = forward.with_y(0.).normalize_or(DVec3::NEG_Z);
        let side =
            DVec3::new(-heading.z, 0., heading.x) * if a.id.is_multiple_of(2) { 1. } else { -1. };
        for direction in [
            heading + side,
            heading - side,
            DVec3::Y,
            side,
            -side,
            DVec3::NEG_Y,
        ] {
            let step = direction.normalize() * speed * f64::from(dt);
            let point = (DVec3::from_array(before) + step).to_array();
            if swept(point) {
                a.position = point;
                break;
            }
        }
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
    if a.velocity[0].abs() + a.velocity[2].abs() > 0.01 {
        a.yaw = (-a.velocity[0])
            .atan2(-a.velocity[2])
            .rem_euclid(std::f32::consts::TAU);
    }
}

pub(super) fn travel(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    search: bool,
    mut nav: Option<&mut navigation::Navigation>,
) {
    let before = a.position;
    if a.kind == Kind::Crocodile {
        let dx = target[0] - before[0];
        let dz = target[2] - before[2];
        let length = dx.hypot(dz).max(0.001);
        let speed = a.kind.speed() * 1.45;
        let amount = (speed * f64::from(dt)).min(length);
        let next = [
            before[0] + dx / length * amount,
            before[1],
            before[2] + dz / length * amount,
        ];
        let yaw = (-dx).atan2(-dz);
        if (super::support::swimming(a, h) || h.wet_footprint(next, a.size(), yaw))
            && h.clear_swimmer_rotated(before, a.size(), yaw)
        {
            a.yaw = (yaw as f32).rem_euclid(std::f32::consts::TAU);
            super::support::swim_toward(a, h, target, dt, speed);
            return;
        }
    }
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
