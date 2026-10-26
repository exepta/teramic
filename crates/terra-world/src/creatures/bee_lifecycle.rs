//! A funded colony uses the same persistent F06 identities, family clocks and growth.
use super::{Habitat, distance};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, CreatureNeeds,
    CreaturePopulation,
    breeding::{self, AnimalFamily},
    ecology::{BeeState, NEST_CAPACITY},
};

const NEST_BUDGET: usize = 16;
const NECTAR_COST: u16 = 100;

pub fn step(
    population: &mut CreaturePopulation,
    families: &mut BTreeMap<u64, AnimalFamily>,
    next_birth: &mut u32,
    habitat: &Habitat,
    active: &[u64],
    cursor: &mut Option<u64>,
    capacity: usize,
) -> usize {
    for (id, bee) in &mut population.ecology.bees {
        if population.animals.get(id).is_none_or(|a| a.health <= 0.0) {
            bee.inside = false;
            bee.carried = 0;
        }
    }
    if capacity == 0 {
        return 0;
    }
    let active: BTreeSet<_> = active.iter().copied().collect();
    let mut residents: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
    for (&id, state) in &population.ecology.bees {
        let Some(animal) = population
            .animals
            .get(&id)
            .filter(|a| a.kind == Kind::Bee && a.health > 0.0)
        else {
            continue;
        };
        *counts.entry(state.nest).or_default() += 1;
        if active.contains(&id)
            && state.inside
            && animal.behavior == Behavior::InNest
            && !animal.juvenile()
            && animal.health >= Kind::Bee.health() * 0.5
            && families
                .get(&id)
                .is_none_or(|family| family.ready_age <= animal.age)
        {
            residents.entry(state.nest).or_default().push(id);
        }
    }
    let nests: Vec<_> = residents.keys().copied().collect();
    if nests.is_empty() {
        *cursor = None;
        return 0;
    }
    let start = cursor.map_or(0, |last| nests.partition_point(|id| *id <= last)) % nests.len();
    let scheduled: Vec<_> = nests
        .iter()
        .cycle()
        .skip(start)
        .take(NEST_BUDGET.min(nests.len()))
        .copied()
        .collect();
    *cursor = scheduled.last().copied();
    let mut births = 0;
    for nest_id in scheduled {
        if births >= super::breeding::BIRTH_BUDGET.min(capacity) {
            break;
        }
        let Some(nest) = population
            .ecology
            .nests
            .get(&nest_id)
            .filter(|n| n.nectar >= NECTAR_COST + 100)
        else {
            continue;
        };
        if counts.get(&nest_id).copied().unwrap_or_default() >= NEST_CAPACITY {
            continue;
        }
        let nest_position = nest.position;
        let owner = terra_voxel::global_to_section(nest_position.map(|v| v.floor() as i64)).0;
        let exists = [-1, 0, 1].into_iter().any(|dx| {
            [-1, 0, 1].into_iter().any(|dy| {
                [-1, 0, 1].into_iter().any(|dz| {
                    owner
                        .offset([dx, dy, dz])
                        .ok()
                        .and_then(|p| habitat.chunks.loaded.get(&p))
                        .is_some_and(|section| {
                            section.props.iter().any(|prop| {
                                prop.id.0 == nest_id
                                    && prop.kind == crate::props::PropKind::BeeNest
                                    && !prop.removed
                                    && !habitat.journal.prop_removed(prop.section, prop.id)
                            })
                        })
                })
            })
        });
        if !exists {
            continue;
        }
        let eligible = &residents[&nest_id];
        let pair = eligible.iter().copied().find_map(|a| {
            eligible
                .iter()
                .copied()
                .find(|b| {
                    *b > a
                        && population.animals[&a].sex() != population.animals[b].sex()
                        && distance(
                            population.animals[&a].position,
                            population.animals[b].position,
                        ) <= 25.0
                })
                .map(|b| [a, b])
        });
        let Some(parents) = pair else {
            continue;
        };
        let Some(id) = breeding::next_birth_id(population, *next_birth) else {
            break;
        };
        let first = &population.animals[&parents[0]];
        let mut child = Creature {
            id,
            kind: Kind::Bee,
            home: first.home,
            position: first.position,
            velocity: [0.0; 3],
            yaw: first.yaw,
            health: Kind::Bee.health(),
            behavior: Behavior::InNest,
            timer: 4.0,
            age: 0.0,
            variant: (id % 4) as u8,
            needs: CreatureNeeds::full(Kind::Bee),
        };
        if !habitat.clear_swimmer_rotated(child.position, Kind::Bee.size(), f64::from(child.yaw)) {
            continue;
        }
        child.home = [
            (child.position[0].floor() as i64).div_euclid(32),
            (child.position[2].floor() as i64).div_euclid(32),
        ];
        if !child.valid() {
            continue;
        }
        for parent in parents {
            families
                .entry(parent)
                .or_default()
                .after_birth(&population.animals[&parent]);
        }
        families.insert(
            id,
            AnimalFamily {
                parents: Some(parents),
                ready_age: breeding::GROWTH_SECONDS,
                ..Default::default()
            },
        );
        population
            .ecology
            .nests
            .get_mut(&nest_id)
            .expect("validated nest")
            .nectar -= NECTAR_COST;
        population.ecology.bees.insert(
            id,
            BeeState {
                nest: nest_id,
                inside: true,
                ..Default::default()
            },
        );
        population.animals.insert(id, child);
        *next_birth = breeding::birth_serial(id).expect("validated birth identity");
        *counts.entry(nest_id).or_default() += 1;
        births += 1;
    }
    births
}

#[cfg(test)]
mod tests;
