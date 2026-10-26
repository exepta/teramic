//! Two bounded births per pass, using the existing loaded-voxel support/collision probes.
use super::{Habitat, distance};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{
    PlayerUuid,
    creatures::{
        Creature, CreatureBehavior, CreatureNeeds, CreaturePopulation,
        breeding::{self, AnimalFamily, LOCAL_LIMIT, LOCAL_RADIUS, OWNER_LIMIT, PARTNER_RADIUS},
        taming::AnimalBond,
    },
};

pub const BIRTH_BUDGET: usize = 2;
const CANDIDATE_BUDGET: usize = 64;

fn suitable(animal: &Creature, family: AnimalFamily) -> bool {
    family.willing(animal)
        && !matches!(
            animal.behavior,
            CreatureBehavior::Sleep | CreatureBehavior::Dead | CreatureBehavior::Flee
        )
        && animal.velocity[1].abs() < 0.15
}

fn spawn_position(
    population: &CreaturePopulation,
    habitat: &Habitat,
    child: &Creature,
) -> Option<[f64; 3]> {
    for [x, z] in [
        [0.0, 0.0],
        [1.5, 0.0],
        [-1.5, 0.0],
        [0.0, 1.5],
        [0.0, -1.5],
        [1.5, 1.5],
        [-1.5, 1.5],
        [1.5, -1.5],
        [-1.5, -1.5],
        [2.5, 0.0],
        [0.0, 2.5],
        [-2.5, 0.0],
        [0.0, -2.5],
    ] {
        let probe = [
            child.position[0] + x,
            child.position[1] + 0.5,
            child.position[2] + z,
        ];
        let Some(position) = habitat.ground(probe, child.kind) else {
            continue;
        };
        // Reserve adult-sized dry clearance so the youngster has room to grow.
        let mut adult = child.clone();
        adult.age = breeding::GROWTH_SECONDS;
        if !habitat.clear(position, adult.size(), false) || habitat.wet(position) {
            continue;
        }
        let size = child.size();
        if population.animals.values().any(|other| {
            if other.health <= 0.0 {
                return false;
            }
            let other_size = other.size();
            (position[0] - other.position[0]).abs() < (size[0] + other_size[0]) * 0.5 + 0.1
                && (position[2] - other.position[2]).abs() < (size[2] + other_size[2]) * 0.5 + 0.1
                && position[1] < other.position[1] + other_size[1]
                && other.position[1] < position[1] + size[1]
        }) {
            continue;
        }
        return Some(position);
    }
    None
}

pub fn step(
    population: &mut CreaturePopulation,
    families: &mut BTreeMap<u64, AnimalFamily>,
    bonds: &mut BTreeMap<u64, AnimalBond>,
    next_birth: &mut u32,
    habitat: &Habitat,
    active: &[u64],
    cursor: &mut Option<u64>,
) -> usize {
    let mut candidates: Vec<_> = active
        .iter()
        .copied()
        .filter(|id| {
            population.owners.contains_key(id)
                && population
                    .animals
                    .get(id)
                    .zip(families.get(id))
                    .is_some_and(|(a, family)| {
                        suitable(a, *family)
                            && habitat
                                .block(a.position.map(|v| v.floor() as i64))
                                .is_some()
                    })
        })
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    if candidates.is_empty() {
        *cursor = None;
        return 0;
    }
    let cell = |position: [f64; 3]| position.map(|v| (v / PARTNER_RADIUS).floor() as i64);
    let mut neighborhoods: BTreeMap<[i64; 3], Vec<u64>> = BTreeMap::new();
    for &id in &candidates {
        neighborhoods
            .entry(cell(population.animals[&id].position))
            .or_default()
            .push(id);
    }
    let start =
        cursor.map_or(0, |last| candidates.partition_point(|id| *id <= last)) % candidates.len();
    let mut scheduled: Vec<_> = candidates
        .iter()
        .cycle()
        .skip(start)
        .take(CANDIDATE_BUDGET.min(candidates.len()))
        .copied()
        .collect();
    *cursor = scheduled.last().copied();
    scheduled.sort_unstable();
    let mut used = BTreeSet::new();
    let mut births = 0;
    let mut owner_counts: BTreeMap<PlayerUuid, usize> = BTreeMap::new();
    for (&id, &owner) in &population.owners {
        if population.animals.get(&id).is_some_and(|a| a.health > 0.0) {
            *owner_counts.entry(owner).or_default() += 1;
        }
    }
    for &a_id in &scheduled {
        if births == BIRTH_BUDGET {
            break;
        }
        if used.contains(&a_id) {
            continue;
        }
        let a = &population.animals[&a_id];
        let owner = population.owners[&a_id];
        if owner_counts.get(&owner).copied().unwrap_or_default() >= OWNER_LIMIT {
            continue;
        }
        let origin = cell(a.position);
        let nearby = (-1..=1).flat_map(|x| {
            (-1..=1).flat_map(move |y| {
                (-1..=1).map(move |z| [origin[0] + x, origin[1] + y, origin[2] + z])
            })
        });
        let b_id = nearby
            .filter_map(|cell| neighborhoods.get(&cell))
            .flat_map(|ids| ids.iter().take(LOCAL_LIMIT))
            .copied()
            .filter(|&id| {
                id != a_id
                    && !used.contains(&id)
                    && population.owners.get(&id) == Some(&owner)
                    && population.animals.get(&id).is_some_and(|b| {
                        b.kind == a.kind
                            && b.sex() != a.sex()
                            && distance(a.position, b.position) <= PARTNER_RADIUS.powi(2)
                            && (a.position[1] - b.position[1]).abs() <= 1.0
                            && habitat
                                .walk_step(a.position, b.position, a.kind, f64::from(a.yaw))
                                .is_some()
                    })
            })
            .min();
        let Some(b_id) = b_id else {
            continue;
        };
        let b = &population.animals[&b_id];
        let position = std::array::from_fn(|i| (a.position[i] + b.position[i]) * 0.5);
        if population
            .animals
            .values()
            .filter(|other| {
                other.health > 0.0 && distance(position, other.position) <= LOCAL_RADIUS.powi(2)
            })
            .count()
            >= LOCAL_LIMIT
        {
            continue;
        }
        let Some(id) = breeding::next_birth_id(population, *next_birth) else {
            break;
        };
        let parents = [a_id.min(b_id), a_id.max(b_id)];
        let hash = id.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ parents[0].rotate_left(17) ^ parents[1];
        let mut child = Creature {
            id,
            kind: a.kind,
            home: a.home,
            position,
            velocity: [0.0; 3],
            yaw: population.animals[&parents[0]].yaw,
            health: a.kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 4.0,
            age: 0.0,
            variant: population.animals[&parents[(hash & 1) as usize]].variant,
            needs: CreatureNeeds::full(a.kind),
        };
        let Some(position) = spawn_position(population, habitat, &child) else {
            continue;
        };
        child.position = position;
        child.home = [
            (position[0].floor() as i64).div_euclid(32),
            (position[2].floor() as i64).div_euclid(32),
        ];
        if !child.valid() {
            continue;
        }
        families.entry(a_id).or_default().after_birth(a);
        families.entry(b_id).or_default().after_birth(b);
        families.insert(
            id,
            AnimalFamily {
                parents: Some(parents),
                active_age: 0.0,
                ready_age: breeding::GROWTH_SECONDS,
                willing_until_age: 0.0,
            },
        );
        bonds.insert(id, AnimalBond::owned(owner));
        population.owners.insert(id, owner);
        population.animals.insert(id, child);
        *next_birth = breeding::birth_serial(id).expect("Validated offspring identity");
        *owner_counts.entry(owner).or_default() += 1;
        used.extend([a_id, b_id]);
        births += 1;
    }
    births
}

/// Growth never expands a body through newly placed walls or a lowered ceiling.
pub fn constrain_growth(animal: &mut Creature, previous_age: f64, habitat: &Habitat) -> f64 {
    let elapsed = (animal.age - previous_age).clamp(0.0, 0.25);
    if breeding::is_birth_id(animal.id)
        && previous_age < breeding::GROWTH_SECONDS
        && animal.age > previous_age
        && !habitat.clear_swimmer_rotated(animal.position, animal.size(), f64::from(animal.yaw))
    {
        animal.age = previous_age;
    }
    elapsed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        TerraGenerator,
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    use terra_core::{GameplayState, WorldConfig, creatures::CreatureKind};
    use terra_voxel::{BlockSection, SectionPos};

    fn fixture() -> (
        TerraGenerator,
        ChunkStore,
        EditJournal,
        GameplayState,
        Vec<u64>,
    ) {
        let terrain = TerraGenerator::new(WorldConfig {
            caves: false,
            trees: false,
            water: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut chunks = ChunkStore::default();
        for x in -1..=1 {
            let p = SectionPos::new(x, 0, 0);
            let mut data = BlockSection::empty(p).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..3 {
                        data.set_block([x, y, z], 1).unwrap();
                    }
                }
            }
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        let owner = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let mut ids = Vec::new();
        for (slot, x) in [5.5, 8.5].into_iter().enumerate() {
            let id = (1 << 62) + slot as u64 + 1;
            let kind = CreatureKind::Cow;
            state.creatures.animals.insert(
                id,
                Creature {
                    id,
                    kind,
                    home: [0, 0],
                    position: [x, 3.0, 8.0],
                    velocity: [0.0; 3],
                    yaw: 0.0,
                    health: kind.health(),
                    behavior: CreatureBehavior::Rest,
                    timer: 0.0,
                    age: 20.0,
                    variant: 0,
                    needs: CreatureNeeds::full(kind),
                },
            );
            state.creatures.owners.insert(id, owner);
            state.animal_bonds.insert(id, AnimalBond::owned(owner));
            state.animal_families.insert(
                id,
                AnimalFamily {
                    willing_until_age: 80.0,
                    ..Default::default()
                },
            );
            ids.push(id);
        }
        state.creatures.explored.insert([0, 0]);
        assert_ne!(
            state.creatures.animals[&ids[0]].sex(),
            state.creatures.animals[&ids[1]].sex()
        );
        (terrain, chunks, EditJournal::default(), state, ids)
    }
    fn breed(
        state: &mut GameplayState,
        terrain: &TerraGenerator,
        chunks: &ChunkStore,
        journal: &EditJournal,
        ids: &[u64],
    ) -> usize {
        step(
            &mut state.creatures,
            &mut state.animal_families,
            &mut state.animal_bonds,
            &mut state.next_animal_birth,
            &Habitat {
                terrain,
                chunks,
                journal,
            },
            ids,
            &mut None,
        )
    }

    #[test]
    fn same_owner_pair_produces_one_safe_persistent_child_and_cooldown() {
        let (terrain, chunks, journal, mut state, ids) = fixture();
        let original = state.clone();
        assert_eq!(breed(&mut state, &terrain, &chunks, &journal, &ids), 1);
        assert!(state.valid());
        let id = breeding::next_birth_id(&original.creatures, original.next_animal_birth).unwrap();
        let child = &state.creatures.animals[&id];
        assert!(child.juvenile());
        assert_eq!(child.position[1], 3.0);
        assert_eq!(state.animal_families[&id].parents, Some([ids[0], ids[1]]));
        assert_eq!(
            state.creatures.owners[&id],
            original.creatures.owners[&ids[0]]
        );
        assert_eq!(state.creatures.natural_count(), 0);
        assert_eq!(
            state.animal_families[&ids[0]].ready_age,
            20.0 + breeding::COOLDOWN_SECONDS
        );
        assert_eq!(breed(&mut state, &terrain, &chunks, &journal, &ids), 0);
        let mut reversed = original;
        let mut order = ids.clone();
        order.reverse();
        assert_eq!(breed(&mut reversed, &terrain, &chunks, &journal, &order), 1);
        assert_eq!(state, reversed);
    }

    #[test]
    fn ownership_species_range_safety_and_population_caps_fail_closed() {
        let (terrain, chunks, journal, state, ids) = fixture();
        let mut other = state.clone();
        other.creatures.owners.insert(ids[1], PlayerUuid::new_v4());
        assert_eq!(breed(&mut other, &terrain, &chunks, &journal, &ids), 0);
        let mut far = state.clone();
        far.creatures.animals.get_mut(&ids[1]).unwrap().position[0] += 10.0;
        assert_eq!(breed(&mut far, &terrain, &chunks, &journal, &ids), 0);
        let mut wrong = state.clone();
        wrong.creatures.animals.get_mut(&ids[1]).unwrap().kind = CreatureKind::Pig;
        assert_eq!(breed(&mut wrong, &terrain, &chunks, &journal, &ids), 0);
        let mut unknown = state.clone();
        assert_eq!(
            breed(
                &mut unknown,
                &terrain,
                &ChunkStore::default(),
                &journal,
                &ids
            ),
            0
        );
        let mut capped = state.clone();
        for n in 0..LOCAL_LIMIT {
            let mut a = state.creatures.animals[&ids[0]].clone();
            a.id = (1 << 62) + n as u64 + 1;
            capped.creatures.animals.insert(a.id, a);
        }
        assert_eq!(breed(&mut capped, &terrain, &chunks, &journal, &ids), 0);
        let mut expired = state;
        expired
            .animal_families
            .get_mut(&ids[0])
            .unwrap()
            .willing_until_age = 19.0;
        assert_eq!(breed(&mut expired, &terrain, &chunks, &journal, &ids), 0);
    }

    #[test]
    fn same_sex_never_births_or_spends_readiness_but_opposite_adults_can_breed() {
        let (terrain, chunks, journal, mut state, mut ids) = fixture();
        let replace = |state: &mut GameplayState, from: u64, to: u64| {
            let mut animal = state.creatures.animals.remove(&from).unwrap();
            animal.id = to;
            state.creatures.animals.insert(to, animal);
            let owner = state.creatures.owners.remove(&from).unwrap();
            state.creatures.owners.insert(to, owner);
            let bond = state.animal_bonds.remove(&from).unwrap();
            state.animal_bonds.insert(to, bond);
            let family = state.animal_families.remove(&from).unwrap();
            state.animal_families.insert(to, family);
        };
        replace(&mut state, ids[1], (1 << 62) + 3);
        ids[1] = (1 << 62) + 3;
        assert_eq!(
            state.creatures.animals[&ids[0]].sex(),
            state.creatures.animals[&ids[1]].sex()
        );
        assert!(state.valid());
        let before = state.clone();
        assert_eq!(breed(&mut state, &terrain, &chunks, &journal, &ids), 0);
        assert_eq!(state, before);
        replace(&mut state, ids[1], (1 << 62) + 4);
        ids[1] = (1 << 62) + 4;
        assert_ne!(
            state.creatures.animals[&ids[0]].sex(),
            state.creatures.animals[&ids[1]].sex()
        );
        assert_eq!(breed(&mut state, &terrain, &chunks, &journal, &ids), 1);
        assert!(state.valid());
        let child = state
            .creatures
            .animals
            .values()
            .find(|a| a.juvenile())
            .unwrap();
        assert_eq!(
            child.sex(),
            terra_core::creatures::CreatureSex::from_spawn_id(child.id)
        );
    }

    #[test]
    fn opposite_sex_birth_is_stable_in_negative_chunks_and_reversed_processing_order() {
        let (terrain, chunks, journal, mut state, ids) = fixture();
        for (n, &id) in ids.iter().enumerate() {
            let animal = state.creatures.animals.get_mut(&id).unwrap();
            animal.position[0] = -8.5 + n as f64 * 3.0;
            animal.home = [-1, 0];
        }
        let mut reversed = state.clone();
        let order: Vec<_> = ids.iter().copied().rev().collect();
        assert_eq!(breed(&mut state, &terrain, &chunks, &journal, &ids), 1);
        assert_eq!(breed(&mut reversed, &terrain, &chunks, &journal, &order), 1);
        assert_eq!(state, reversed);
        assert!(state.valid());
        let child = state
            .creatures
            .animals
            .values()
            .find(|a| a.juvenile())
            .unwrap();
        assert!(child.position[0] < 0.0);
        assert_eq!(child.home, [-1, 0]);
        assert_eq!(child.sex(), terra_core::creatures::CreatureSex::Male);
    }

    #[test]
    fn every_tameable_species_uses_the_same_bounded_birth_and_owner_rules() {
        for kind in [
            CreatureKind::Sheep,
            CreatureKind::Cow,
            CreatureKind::Pig,
            CreatureKind::Horse,
            CreatureKind::SmallSpider,
            CreatureKind::MediumSpider,
            CreatureKind::LargeSpider,
        ] {
            let (terrain, chunks, journal, mut state, ids) = fixture();
            for a in state.creatures.animals.values_mut() {
                a.kind = kind;
                a.health = kind.health();
                a.needs = CreatureNeeds::full(kind);
            }
            assert_eq!(
                breed(&mut state, &terrain, &chunks, &journal, &ids),
                1,
                "{kind:?}"
            );
            assert!(state.valid(), "{kind:?}");
            assert_eq!(state.creatures.natural_count(), 0);
            let child = state
                .creatures
                .animals
                .values()
                .find(|a| a.juvenile())
                .unwrap();
            assert!(child.movement_scale() < 1.0);
            assert!(child.product().is_none());
            assert!(
                Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal
                }
                .clear(child.position, child.size(), false)
            );
        }
    }

    #[test]
    fn growth_cannot_expand_through_a_ceiling_and_does_not_rewind_parent_cooldowns_or_needs() {
        let (terrain, chunks, mut journal, state, ids) = fixture();
        let mut child = state.creatures.animals[&ids[0]].clone();
        child.id = breeding::next_birth_id(&state.creatures, 0).unwrap();
        child.age = 600.0;
        child.timer = 2.0;
        child.needs.food -= 1.0;
        for x in 4..=6 {
            for z in 7..=9 {
                journal.record([x, 4, z], 1).unwrap();
            }
        }
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        constrain_growth(&mut child, 0.0, &habitat);
        assert_eq!(child.age, 0.0);
        assert_eq!(child.timer, 2.0);
        assert_eq!(child.needs.food, child.kind.food() - 1.0);
        assert!(habitat.clear(child.position, child.size(), false));
        let mut parent = state.creatures.animals[&ids[0]].clone();
        parent.age = 300.25;
        constrain_growth(&mut parent, 300.0, &habitat);
        assert_eq!(parent.age, 300.25);
    }

    #[test]
    fn rotating_budget_does_not_starve_a_pair_after_sixty_four_isolated_lower_ids() {
        let (terrain, mut chunks, journal, mut state, _) = fixture();
        let template = state.creatures.animals.values().next().unwrap().clone();
        let owner = *state.creatures.owners.values().next().unwrap();
        state.creatures.animals.clear();
        state.creatures.owners.clear();
        state.animal_bonds.clear();
        state.animal_families.clear();
        for sx in 2..=44 {
            let p = SectionPos::new(sx, 0, 0);
            let mut data = BlockSection::empty(p).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in 0..3 {
                        data.set_block([x, y, z], 1).unwrap();
                    }
                }
            }
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
        let mut ids = Vec::new();
        for n in 0..66 {
            let id = (1 << 62) + if n == 65 { 67 } else { n + 1 };
            let mut animal = template.clone();
            animal.id = id;
            animal.position[0] = if n < 64 {
                64.0 + n as f64 * 10.0
            } else if n == 64 {
                5.5
            } else {
                8.5
            };
            animal.home = [(animal.position[0].floor() as i64).div_euclid(32), 0];
            state.creatures.animals.insert(id, animal);
            state.creatures.owners.insert(id, owner);
            state.animal_bonds.insert(id, AnimalBond::owned(owner));
            state.animal_families.insert(
                id,
                AnimalFamily {
                    willing_until_age: 80.0,
                    ..Default::default()
                },
            );
            ids.push(id);
        }
        let mut cursor = None;
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        assert_eq!(
            step(
                &mut state.creatures,
                &mut state.animal_families,
                &mut state.animal_bonds,
                &mut state.next_animal_birth,
                &habitat,
                &ids,
                &mut cursor
            ),
            0
        );
        assert_eq!(
            step(
                &mut state.creatures,
                &mut state.animal_families,
                &mut state.animal_bonds,
                &mut state.next_animal_birth,
                &habitat,
                &ids,
                &mut cursor
            ),
            1
        );
        assert!(state.valid());
    }
}
