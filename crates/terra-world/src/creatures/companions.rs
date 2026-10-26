//! Explicit pet commands reuse the existing swept movement and bounded navigation.
use super::{Behavior, Creature, Habitat, Kind, horses, navigation, needs, support};
use terra_core::creatures::{intentions::IntentReason, taming::CompanionMode};
#[cfg(test)]
mod tests;

pub fn step(
    animal: &mut Creature,
    habitat: &Habitat,
    owner: Option<[f64; 3]>,
    mode: CompanionMode,
    dt: f32,
    search: bool,
    navigation: Option<&mut navigation::Navigation>,
) {
    if !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.0
        || animal.health <= 0.0
        || habitat
            .block(animal.position.map(|v| v.floor() as i64))
            .is_none()
    {
        return;
    }
    animal.age += f64::from(dt);
    animal.timer = (animal.timer - dt).max(0.0);
    let silk_anchor = (animal.kind.is_spider() && animal.behavior == Behavior::Descend)
        .then_some(animal.needs.goal)
        .flatten();
    needs::update(animal, habitat, dt, false);
    animal.needs.goal = silk_anchor;
    if animal.health <= 0.0 {
        animal.behavior = Behavior::Dead;
        animal.velocity = [0.0; 3];
        animal.needs.goal = None;
        return;
    }
    if animal.kind.is_spider() && super::spiders::submersion_damage(animal, habitat, dt) {
        return;
    }
    animal.needs.search_in = animal.needs.search_in.max(1.0);
    let explicit_follow = mode == CompanionMode::Follow
        && habitat.journal.gameplay().pet_orders.get(&animal.id) == Some(&CompanionMode::Follow);
    if let Some(owner) = owner.filter(|p| {
        explicit_follow
            && p.iter().all(|v| v.is_finite())
            && super::distance(*p, animal.position) <= 24.0_f64.powi(2)
            && (p[1] - animal.position[1]).abs() < 6.0
    }) {
        let gap = (owner[0] - animal.position[0]).hypot(owner[2] - animal.position[2]);
        if gap > 2.0 {
            follow(animal, habitat, owner, dt, search, navigation);
            return;
        }
    }
    if let Some(nav) = navigation {
        nav.forget(animal.id);
        nav.intend(animal, IntentReason::WaitForOwner, None);
    }
    if animal.kind == Kind::Horse && animal.behavior == Behavior::Leap {
        horses::advance(animal, habitat, dt);
        return;
    }
    if animal.kind.is_spider() {
        super::spiders::companion_move(animal, habitat, animal.position, dt, false, true);
        return;
    }
    if support::swimming(animal, habitat) {
        support::swim_toward(animal, habitat, animal.position, dt, 0.0);
        return;
    }
    animal.behavior = Behavior::Rest;
    animal.velocity[0] = 0.0;
    animal.velocity[2] = 0.0;
    support::settle(animal, habitat, dt);
}

fn follow(
    animal: &mut Creature,
    habitat: &Habitat,
    owner: [f64; 3],
    dt: f32,
    search: bool,
    mut navigation: Option<&mut navigation::Navigation>,
) {
    if let Some(nav) = navigation.as_deref_mut() {
        nav.intend(animal, IntentReason::FollowOwner, Some(owner));
    }
    if animal.kind.is_spider() {
        if let Some(nav) = navigation {
            super::spiders::hunt_surface(animal, habitat, owner, dt, search, nav, true);
        } else {
            super::spiders::companion_move(animal, habitat, owner, dt, true, true);
        }
        return;
    }
    if animal.kind == Kind::Horse && animal.behavior == Behavior::Leap {
        horses::advance(animal, habitat, dt);
        return;
    }
    let speed = animal.kind.speed()
        * if super::distance(owner, animal.position) > 36.0 {
            1.15
        } else {
            0.85
        };
    if support::swimming(animal, habitat) {
        support::swim_toward(animal, habitat, owner, dt, speed);
        return;
    }
    let before = animal.position;
    let move_to = |target: [f64; 3]| {
        let dx = target[0] - before[0];
        let dz = target[2] - before[2];
        let length = dx.hypot(dz).max(0.001);
        let remaining = ((owner[0] - before[0]).hypot(owner[2] - before[2]) - 1.75).max(0.0);
        let amount = (speed * animal.movement_scale() * f64::from(dt))
            .min(length)
            .min(remaining);
        let next = [
            before[0] + dx / length * amount,
            before[1],
            before[2] + dz / length * amount,
        ];
        let yaw = (-dx).atan2(-dz);
        habitat
            .travel_step_sized(before, next, animal.size(), yaw)
            .map(|next| (next, yaw))
    };
    let detouring = navigation
        .as_ref()
        .is_some_and(|nav| nav.following_ground_route(animal.id));
    let mut next = if detouring {
        navigation::ground_waypoint(animal, habitat, owner, false, navigation.as_deref_mut())
            .and_then(move_to)
    } else {
        move_to(owner)
    };
    if next.is_none() {
        next =
            navigation::ground_waypoint(animal, habitat, owner, search, navigation.as_deref_mut())
                .and_then(move_to);
    } else if !detouring && let Some(nav) = navigation {
        nav.ground_unblocked(animal.id);
    }
    if let Some((next, yaw)) = next {
        if animal.kind == Kind::Horse && next[1] > before[1] + 0.30 {
            if horses::launch(animal, habitat, next) {
                horses::advance(animal, habitat, dt);
            }
            return;
        }
        animal.position = next;
        animal.yaw = yaw as f32;
        animal.behavior = Behavior::Wander;
        animal.velocity = std::array::from_fn(|i| ((next[i] - before[i]) / f64::from(dt)) as f32);
        animal.velocity[1] = 0.0;
    } else {
        animal.behavior = Behavior::Wander;
        animal.velocity[0] = 0.0;
        animal.velocity[2] = 0.0;
        support::settle(animal, habitat, dt);
    }
}
