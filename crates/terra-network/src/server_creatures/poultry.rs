use bevy::prelude::Resource;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use terra_core::{
    GameplayState, PlayerUuid,
    creatures::{Creature, CreatureKind as Kind, CreaturePopulation, poultry_id},
};
use terra_world::creatures::{Habitat, bears::visible, distance, poultry::FLOCK_RADIUS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Actor {
    Player(PlayerUuid),
    Mob(u64),
}

#[derive(Resource, Default)]
pub(in crate::server) struct Alerts {
    reported: BTreeMap<(u64, Actor), Instant>,
    swings: BTreeMap<u64, Actor>,
}

impl Alerts {
    pub(super) fn report(&mut self, target: u64, attacker: Actor) {
        self.reported.insert((target, attacker), Instant::now());
    }

    pub(super) fn clear(&mut self) {
        self.reported.clear();
        self.swings.clear();
    }

    pub(super) fn frame(
        &mut self,
        population: &CreaturePopulation,
        active: &[u64],
        players: &[(PlayerUuid, [f64; 3])],
    ) -> Frame {
        self.reported.retain(|(target, _), time| {
            time.elapsed() < Duration::from_secs(7)
                && population
                    .animals
                    .get(target)
                    .is_some_and(|bird| bird.kind == Kind::Hen)
        });
        self.swings.retain(|id, _| {
            population
                .animals
                .get(id)
                .is_some_and(|bird| bird.health > 0.0)
        });
        let threats = self
            .reported
            .keys()
            .filter_map(|&(target, actor)| {
                let hen = population.animals.get(&target)?;
                let point = actor.point(population, players)?;
                Some((hen.position, actor, point))
            })
            .collect();
        let roosters = active
            .iter()
            .filter_map(|id| {
                let rooster = population.animals.get(id)?;
                (rooster.kind == Kind::Rooster && rooster.health > 0.0)
                    .then_some((*id, rooster.position))
            })
            .collect();
        let hens = active
            .iter()
            .filter_map(|id| {
                let hen = population.animals.get(id)?;
                (hen.kind == Kind::Hen && hen.health > 0.0).then_some((*id, hen.position))
            })
            .collect();
        Frame {
            threats,
            roosters,
            hens,
        }
    }

    pub(super) fn lock(&mut self, rooster: u64, target: Actor) -> bool {
        self.swings.insert(rooster, target) == Some(target)
    }

    pub(super) fn forget(&mut self, rooster: u64) {
        self.swings.remove(&rooster);
    }
}

impl Actor {
    pub(super) fn point(
        self,
        population: &CreaturePopulation,
        players: &[(PlayerUuid, [f64; 3])],
    ) -> Option<[f64; 3]> {
        match self {
            Self::Player(id) => players
                .binary_search_by_key(&id, |(id, _)| *id)
                .ok()
                .map(|i| {
                    let [x, y, z] = players[i].1;
                    [
                        x,
                        y - f64::from(terra_physics::character::PLAYER_HALF_HEIGHT) + 0.45,
                        z,
                    ]
                }),
            Self::Mob(id) => population
                .animals
                .get(&id)
                .filter(|a| a.health > 0.0)
                .map(|a| {
                    let [x, y, z] = a.position;
                    [x, y + (a.size()[1] * 0.5).min(0.65), z]
                }),
        }
    }
}

pub(super) struct Frame {
    threats: Vec<([f64; 3], Actor, [f64; 3])>,
    roosters: BTreeMap<u64, [f64; 3]>,
    hens: BTreeMap<u64, [f64; 3]>,
}

impl Frame {
    pub(super) fn tracks_hen(&self, id: u64) -> bool {
        self.hens.contains_key(&id)
    }

    pub(super) fn threat(&self, bird: &Creature) -> Option<(Actor, [f64; 3])> {
        let radius = if bird.kind == Kind::Rooster {
            terra_core::creatures::definitions::get(bird.kind)
                .combat?
                .aggro_radius
        } else {
            6.0
        };
        self.threats
            .iter()
            .filter(|(hen, attacker, point)| {
                *attacker != Actor::Mob(bird.id)
                    && distance(*hen, bird.position) <= radius.powi(2)
                    && distance(*point, bird.position) <= radius.powi(2)
            })
            .min_by(|a, b| {
                distance(a.2, bird.position)
                    .total_cmp(&distance(b.2, bird.position))
                    .then(a.1.cmp(&b.1))
            })
            .map(|&(_, actor, point)| (actor, point))
    }

    pub(super) fn enemy(&self, bird: &Creature, habitat: &Habitat) -> Option<(Actor, [f64; 3])> {
        if let Some(danger) = self.threat(bird) {
            return Some(danger);
        }
        let [x, y, z] = bird.position;
        let eye = [x, y + bird.size()[1] * 0.6, z];
        self.roosters
            .iter()
            .filter_map(|(&id, &[x, y, z])| {
                let point = [x, y + 0.4, z];
                (id != bird.id && distance(point, eye) <= 36.0 && visible(habitat, eye, point))
                    .then_some((id, point))
            })
            .min_by(|(_, a), (_, b)| distance(*a, eye).total_cmp(&distance(*b, eye)))
            .map(|(id, point)| (Actor::Mob(id), point))
    }

    pub(super) fn flockmate(&self, bird: &Creature) -> Option<[f64; 3]> {
        let flock = if bird.kind == Kind::Hen {
            if let Some(point) = poultry_id(bird.home, 0).and_then(|id| self.roosters.get(&id))
                && distance(*point, bird.position) <= FLOCK_RADIUS.powi(2)
            {
                return Some(*point);
            }
            &self.roosters
        } else {
            // A rooster returns to its hens once it has lost an attacker.
            &self.hens
        };
        flock
            .values()
            .copied()
            .filter(|&point| distance(point, bird.position) <= FLOCK_RADIUS.powi(2))
            .min_by(|&a, &b| distance(a, bird.position).total_cmp(&distance(b, bird.position)))
    }
}

pub(super) fn hurt_opponent(game: &mut GameplayState, id: u64, damage: f32) {
    let Some(before) = game.creatures.animals.get(&id) else {
        return;
    };
    let stance = (before.behavior, before.timer, before.velocity);
    if super::super::train_damage::hurt_creature(game, id, damage)
        && let Some(target) = game
            .creatures
            .animals
            .get_mut(&id)
            .filter(|a| a.health > 0.0)
    {
        // A counterattack does not restart the other fighter's wind-up or a
        // committed predator action.
        (target.behavior, target.timer, target.velocity) = stance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::creatures::{CreatureBehavior, CreatureNeeds};

    fn bird(id: u64, kind: Kind, x: f64) -> Creature {
        Creature {
            id,
            kind,
            home: [0, 0],
            position: [x, 3.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            age: 4.0,
            timer: 0.4,
            behavior: CreatureBehavior::Attack,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }

    #[test]
    fn alerts_select_actual_attackers_of_nearby_hens_and_revalidate_live_targets() {
        let id = |slot| poultry_id([0, 0], slot).unwrap();
        let mut population = CreaturePopulation::default();
        population
            .animals
            .insert(id(0), bird(id(0), Kind::Rooster, 0.0));
        population
            .animals
            .insert(id(1), bird(id(1), Kind::Hen, 3.0));
        population
            .animals
            .insert(id(2), bird(id(2), Kind::Hen, 40.0));
        let predator = 42;
        population
            .animals
            .insert(predator, bird(predator, Kind::MediumSpider, 3.0));
        let witness = PlayerUuid::from_u128(1);
        let attacker = PlayerUuid::from_u128(2);
        let players = [(witness, [0.5, 3.9, 0.0]), (attacker, [2.0, 3.9, 0.0])];
        let mut alerts = Alerts::default();
        alerts.report(id(2), Actor::Player(witness));
        let frame = alerts.frame(&population, &[id(0), id(1), id(2)], &players);
        assert!(frame.threat(&population.animals[&id(0)]).is_none());
        assert!(frame.tracks_hen(id(1)));
        alerts.report(id(1), Actor::Player(attacker));
        alerts.report(id(1), Actor::Mob(predator));
        let frame = alerts.frame(&population, &[id(0), id(1), id(2)], &players);
        assert_eq!(
            frame.threat(&population.animals[&id(0)]).unwrap().0,
            Actor::Player(attacker)
        );
        let frame = alerts.frame(&population, &[id(0), id(1), id(2)], &players[..1]);
        assert_eq!(
            frame.threat(&population.animals[&id(0)]).unwrap().0,
            Actor::Mob(predator)
        );
        population.animals.get_mut(&predator).unwrap().health = 0.0;
        let frame = alerts.frame(&population, &[id(0), id(1), id(2)], &players[..1]);
        assert!(frame.threat(&population.animals[&id(0)]).is_none());
    }

    #[test]
    fn rival_roosters_keep_their_strike_timer_when_hit_and_can_die() {
        let id = poultry_id([0, 0], 0).unwrap();
        let mut game = GameplayState::default();
        game.creatures
            .animals
            .insert(id, bird(id, Kind::Rooster, 0.0));
        hurt_opponent(&mut game, id, 1.5);
        assert_eq!(
            game.creatures.animals[&id].health,
            Kind::Rooster.health() - 1.5
        );
        assert_eq!(
            game.creatures.animals[&id].behavior,
            CreatureBehavior::Attack
        );
        assert_eq!(game.creatures.animals[&id].timer, 0.4);
        hurt_opponent(&mut game, id, Kind::Rooster.health());
        assert_eq!(game.creatures.animals[&id].behavior, CreatureBehavior::Dead);
    }
}
