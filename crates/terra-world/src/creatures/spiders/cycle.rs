//! Bounded nest/wait/hunt/return decisions; no second movement or saved mob schema.
use super::*;
use std::collections::HashMap;

pub const MIN_WAIT: f64 = 30.0;
pub const MAX_WAIT: f64 = 90.0;
pub const HUNT_SECONDS: f64 = 24.0;
const RETURN_SECONDS: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Wait,
    Hunt,
    Return,
}

struct Cycle {
    den: u64,
    phase: Phase,
    clock: f64,
    until: f64,
    round: u64,
}

#[derive(Default)]
pub struct Cycles(HashMap<u64, Cycle>);

pub fn wait_seconds(id: u64, den: u64, round: u64) -> f64 {
    MIN_WAIT + (random(id ^ den.rotate_left(23), round ^ 0x5741_4954) % 60_001) as f64 / 1000.0
}

impl Cycles {
    pub fn retain(&mut self, population: &CreaturePopulation) {
        self.0.retain(|id, _| {
            population
                .animals
                .get(id)
                .is_some_and(|a| a.kind.is_spider() && a.health > 0.0)
        });
    }
    pub fn forget(&mut self, id: u64) {
        self.0.remove(&id);
    }
    pub fn wants_hunt(&self, animal: &Creature) -> bool {
        self.0.get(&animal.id).is_some_and(|cycle| {
            cycle.phase == Phase::Hunt || cycle.phase == Phase::Wait && cycle.clock >= cycle.until
        })
    }
    pub fn needs_search(&self, animal: &Creature, dt: f32) -> bool {
        animal.behavior != Behavior::Descend
            && animal.needs.search_in <= dt
            && self
                .0
                .get(&animal.id)
                .is_some_and(|cycle| cycle.phase != Phase::Wait || cycle.clock >= cycle.until)
    }
    pub fn phase(&mut self, animal: &Creature, den: u64, anchor: [i64; 3], dt: f32) -> Phase {
        if !dt.is_finite() || dt <= 0.0 || dt > 0.25 {
            return self
                .0
                .get(&animal.id)
                .map_or(Phase::Wait, |cycle| cycle.phase);
        }
        if self.0.len() >= terra_core::creatures::MAX_CREATURES && !self.0.contains_key(&animal.id)
        {
            return Phase::Wait;
        }
        let near = distance(
            terra_core::creatures::spiders::visual_center(animal),
            anchor.map(|v| v as f64 + 0.5),
        ) <= 3.0_f64.powi(2);
        let cycle = self.0.entry(animal.id).or_insert_with(|| Cycle {
            den,
            phase: if near { Phase::Wait } else { Phase::Return },
            clock: 0.0,
            until: if near {
                wait_seconds(animal.id, den, 0)
            } else {
                RETURN_SECONDS
            },
            round: 0,
        });
        if cycle.den != den {
            *cycle = Cycle {
                den,
                phase: if near { Phase::Wait } else { Phase::Return },
                clock: 0.0,
                until: if near {
                    wait_seconds(animal.id, den, 0)
                } else {
                    RETURN_SECONDS
                },
                round: 0,
            };
        }
        match cycle.phase {
            Phase::Wait if cycle.clock >= cycle.until => {
                cycle.phase = Phase::Hunt;
                cycle.until = cycle.clock + HUNT_SECONDS;
            }
            Phase::Hunt if cycle.clock >= cycle.until => {
                cycle.phase = Phase::Return;
                cycle.until = cycle.clock + RETURN_SECONDS;
            }
            Phase::Return if near || cycle.clock >= cycle.until => {
                cycle.round = cycle.round.wrapping_add(1);
                cycle.phase = Phase::Wait;
                cycle.until = cycle.clock + wait_seconds(animal.id, den, cycle.round);
            }
            _ => {}
        }
        // Growth may hold biological age back when a juvenile has no room.
        cycle.clock += f64::from(dt);
        cycle.phase
    }
}

/// Waiting still advances real needs and collision/gravity. Patrolling and
/// returning use the existing swept surface route, including wall/ceiling poses.
pub fn step_den(
    a: &mut Creature,
    h: &Habitat,
    anchor: [i64; 3],
    phase: Phase,
    dt: f32,
    search: bool,
    access: (&mut super::super::navigation::Navigation, bool),
) {
    if !dt.is_finite() || dt <= 0.0 || dt > 0.25 || a.health <= 0.0 {
        return;
    }
    let (navigation, owned) = access;
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    a.needs.food = (a.needs.food - dt * 0.016).max(0.0);
    a.needs.water = 0.0;
    if a.needs.food == 0.0 {
        a.health = (a.health - dt * 0.025).max(0.0);
    }
    if a.health == 0.0 {
        a.behavior = Behavior::Dead;
        a.velocity = [0.0; 3];
        a.needs.goal = None;
        return;
    }
    if submersion_damage(a, h, dt) {
        return;
    }
    if a.behavior == Behavior::Descend {
        falling(a, h, dt, true);
        return;
    }
    if contact(h, a).is_none() {
        detach_for_gravity(a, h);
        navigation.forget(a.id);
        navigation.intend(a, IntentReason::Unsupported, None);
        falling(a, h, dt, false);
        return;
    }
    a.needs.goal = None;
    if phase == Phase::Wait {
        navigation.forget(a.id);
        navigation.intend(a, IntentReason::Idle, Some(anchor.map(|v| v as f64 + 0.5)));
        if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
            a.behavior = Behavior::Rest;
        }
        a.velocity = [0.0; 3];
        return;
    }
    if search {
        a.needs.search_in = 1.0;
    }
    let center = terra_core::creatures::spiders::visual_center(a);
    let mut target = anchor.map(|v| v as f64 + 0.5);
    if phase == Phase::Hunt {
        let seed = random(a.id, (a.age / 4.0) as u64);
        let angle = (seed % 6284) as f64 * 0.001;
        let radius = 4.0 + ((seed >> 16) % 5000) as f64 * 0.001;
        target[0] += angle.sin() * radius;
        target[2] += angle.cos() * radius;
        target[1] = center[1];
    }
    navigation.intend(
        a,
        if phase == Phase::Hunt {
            IntentReason::Hunt
        } else {
            IntentReason::Surface
        },
        Some(target),
    );
    hunt_surface(a, h, target, dt, search, navigation, owned);
    if !matches!(
        a.behavior,
        Behavior::Climb | Behavior::Ceiling | Behavior::Descend
    ) {
        a.behavior = if phase == Phase::Hunt {
            Behavior::Hunt
        } else {
            Behavior::Crawl
        };
    }
}

#[cfg(test)]
mod tests;
