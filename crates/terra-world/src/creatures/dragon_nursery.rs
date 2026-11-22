//! Eggs remain ordinary removable blocks until a player provides a straw nest.
use super::{Habitat, distance};
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, commit_block_edit},
};
use std::collections::BTreeMap;
use terra_core::{
    ItemKind,
    block_properties::BlockPropertiesCatalog,
    creatures::{
        Creature, CreatureBehavior, CreatureNeeds, CreaturePopulation,
        breeding::{self, AnimalFamily},
        dragon_eggs::{self, IncubatingEgg},
        taming::AnimalBond,
    },
};
use terra_voxel::MaterialId;

pub fn step(
    population: &mut CreaturePopulation,
    terrain: &TerraGenerator,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    active: &[u64],
    days: f64,
) {
    let habitat = Habitat {
        terrain,
        chunks,
        journal,
    };
    for &id in active {
        if let Some(child) = population.animals.get_mut(&id)
            && child.kind.is_dragon()
            && breeding::is_birth_id(child.id)
            && habitat
                .block(child.position.map(|v| v.floor() as i64))
                .is_some()
        {
            super::dragons::grow(child, &habitat, days);
        }
    }
    let mut eggs = std::mem::take(&mut journal.gameplay_mut().dragon_eggs);
    let mut changed = false;
    let config = terrain.config();
    let required = f64::from(config.dragon_incubation_days);
    eggs.retain_mut(|egg| {
        let [x, y, z] = egg.position;
        let Some(current) = chunks.block_at(egg.position) else {
            return true;
        };
        if current != egg.shell {
            changed = true;
            return false;
        }
        if chunks.block_at([x, y - 1, z]) != Some(MaterialId::StrawBlock as u16) {
            return true;
        }
        let remaining = (required - egg.elapsed_days).max(0.);
        if egg.elapsed_days < required && days > 0. {
            egg.elapsed_days = (egg.elapsed_days + days).min(required);
            changed = true;
        }
        if egg.elapsed_days >= required
            && let Some(id) = hatch(*egg, population, terrain, chunks, journal, properties)
        {
            // A night can span the final hours of incubation. Its remainder
            // belongs to the newly hatched child, provided the nest has room.
            if remaining > 0. && days > remaining {
                let child = population.animals.get_mut(&id).expect("hatched child");
                let habitat = Habitat {
                    terrain,
                    chunks,
                    journal,
                };
                super::dragons::grow(child, &habitat, days - remaining);
            }
            changed = true;
            return false;
        }
        true
    });
    let state = journal.gameplay_mut();
    state.dragon_eggs = eggs;
    if changed {
        state.revision = state.revision.saturating_add(1);
    }
}

fn hatch(
    egg: IncubatingEgg,
    population: &mut CreaturePopulation,
    terrain: &TerraGenerator,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
) -> Option<u64> {
    if population
        .owners
        .iter()
        .filter(|(id, owner)| {
            **owner == egg.owner && population.animals.get(id).is_some_and(|a| a.health > 0.0)
        })
        .count()
        >= breeding::OWNER_LIMIT
    {
        return None;
    }
    let kind = dragon_eggs::kind(egg.shell)?;
    let id = breeding::next_birth_id(population, journal.gameplay().next_animal_birth)?;
    let [x, y, z] = egg.position;
    let mut child = Creature {
        id,
        kind,
        home: [x.div_euclid(32), z.div_euclid(32)],
        position: [x as f64 + 0.5, y as f64, z as f64 + 0.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 2.0,
        age: 0.0,
        variant: (super::random(id, terrain.config().seed.0) % u64::from(kind.variant_count()))
            as u8,
        needs: CreatureNeeds {
            dragon: Some(terra_core::creatures::dragon_life::DragonLife {
                nest: Some(egg.position),
                ..Default::default()
            }),
            ..CreatureNeeds::full(kind)
        },
    };
    let habitat = Habitat {
        terrain,
        chunks,
        journal,
    };
    let position = super::breeding::spawn_position(population, &habitat, &child)?;
    child.position = position;
    child.home = [position[0], position[2]].map(|v| (v.floor() as i64).div_euclid(32));
    if !child.valid()
        || commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: egg.position,
                expected_material: egg.shell,
                material: MaterialId::Air as u16,
            },
            properties,
        ) != Ok(true)
    {
        return None;
    }
    let state = journal.gameplay_mut();
    state.next_animal_birth = breeding::birth_serial(id).expect("hatchling ID");
    state.animal_families.insert(
        id,
        AnimalFamily {
            ready_age: 0.,
            ..Default::default()
        },
    );
    state.animal_bonds.insert(id, AnimalBond::owned(egg.owner));
    population.owners.insert(id, egg.owner);
    population.animals.insert(id, child);
    Some(id)
}

pub fn lay_eggs(
    population: &CreaturePopulation,
    chunks: &ChunkStore,
    state: &mut terra_core::GameplayState,
    active: &[u64],
) {
    // A four-legged dragon is several blocks wide. Pairs can meet across the full nest platform.
    const RADIUS: f64 = 15.0;
    let mut cells: BTreeMap<[i64; 2], Vec<u64>> = BTreeMap::new();
    let mut clutches = 0;
    for &id in active {
        let Some(a) = population.animals.get(&id) else {
            continue;
        };
        let Some(owner) = population.owners.get(&id) else {
            continue;
        };
        if !a.kind.is_dragon()
            || !state.animal_families.get(&id).is_some_and(|f| f.willing(a))
            || matches!(
                a.behavior,
                CreatureBehavior::Sleep | CreatureBehavior::Dead | CreatureBehavior::Flee
            )
            || chunks
                .block_at(a.position.map(|v| v.floor() as i64))
                .is_none()
        {
            continue;
        }
        let [x, z] = [a.position[0], a.position[2]].map(|v| (v / RADIUS).floor() as i64);
        let neighbor = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dz| [x + dx, z + dz]))
            .filter_map(|c| cells.get(&c))
            .flatten()
            .copied()
            .find(|other| {
                let b = &population.animals[other];
                a.kind == b.kind
                    && a.sex() != b.sex()
                    && population.owners.get(other) == Some(owner)
                    && state.animal_families[other].willing(b)
                    && (a.position[1] - b.position[1]).abs() < 3.0
                    && distance(a.position, b.position) <= RADIUS * RADIUS
            });
        let Some(partner) = neighbor else {
            cells.entry([x, z]).or_default().push(id);
            continue;
        };
        let [px, py, pz] = a.position;
        if state.drop_one(
            [px, py + 0.65, pz],
            ItemKind::Block(dragon_eggs::material(a.kind).expect("dragon egg")),
        ) {
            state.animal_families.get_mut(&id).unwrap().after_birth(a);
            state
                .animal_families
                .get_mut(&partner)
                .unwrap()
                .after_birth(&population.animals[&partner]);
            clutches += 1;
            if clutches == super::breeding::BIRTH_BUDGET {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::streaming::LoadedSection;
    use terra_core::{PlayerUuid, WorldConfig, creatures::CreatureKind};
    use terra_voxel::{BlockSection, SectionPos};

    #[test]
    fn eggs_pause_when_unloaded_hatch_once_and_owned_adults_lay_matching_eggs() {
        let position = [8, 3, 8];
        let kind = CreatureKind::EarthDragon;
        let shell = dragon_eggs::material(kind).unwrap();
        let owner = PlayerUuid::new_v4();
        let egg = IncubatingEgg {
            position,
            shell,
            owner,
            elapsed_days: 5.0 - 0.125,
        };
        // The hand-built straw pen is dry. A natural underground aquifer at
        // the default seed would overlap its synthetic stone floor.
        let terrain = TerraGenerator::new(WorldConfig {
            water: false,
            ..Default::default()
        })
        .unwrap();
        let properties = BlockPropertiesCatalog::default();
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut population = CreaturePopulation::default();
        journal.gameplay_mut().dragon_eggs.push(egg);
        step(
            &mut population,
            &terrain,
            &mut chunks,
            &mut journal,
            &properties,
            &[],
            1. / 1440.,
        );
        assert_eq!(journal.gameplay().dragon_eggs, vec![egg]);
        let section = SectionPos::new(0, 0, 0);
        let mut data = BlockSection::empty(section).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], MaterialId::Stone as u16).unwrap();
            }
        }
        data.set_block([8, 2, 8], MaterialId::StrawBlock as u16)
            .unwrap();
        data.set_block([8, 3, 8], shell).unwrap();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        step(
            &mut population,
            &terrain,
            &mut chunks,
            &mut journal,
            &properties,
            &[],
            0.5,
        );
        assert_eq!(population.animals.len(), 1);
        let (&id, child) = population.animals.first_key_value().unwrap();
        assert!(child.juvenile());
        assert_eq!(child.kind, kind);
        assert_eq!(population.owners[&id], owner);
        assert_eq!(journal.block(position), Some(0));
        let months_per_day =
            12. / f64::from(terrain.config().seasons.lengths().iter().sum::<u32>());
        let hatched = child.needs.dragon.unwrap().months;
        assert!((hatched - 0.375 * months_per_day).abs() < 1e-9);
        assert!(journal.gameplay().dragon_eggs.is_empty());
        // Inactive regions and a loaded but paused server accrue no age.
        for (active, days) in [(&[][..], 0.5), (&[id][..], 0.)] {
            step(
                &mut population,
                &terrain,
                &mut chunks,
                &mut journal,
                &properties,
                active,
                days,
            );
            assert_eq!(
                population.animals[&id].needs.dragon.unwrap().months,
                hatched
            );
        }
        step(
            &mut population,
            &terrain,
            &mut chunks,
            &mut journal,
            &properties,
            &[id],
            0.25,
        );
        let grown = population.animals[&id].needs.dragon.unwrap().months;
        assert!((grown - hatched - 0.25 * months_per_day).abs() < 1e-9);
        assert_eq!(population.animals.len(), 1);
        journal.gameplay_mut().creatures = population.clone();
        assert!(journal.gameplay().valid());
        let mut mate = population.animals[&id].clone();
        mate.id = id + 1;
        while mate.sex() == population.animals[&id].sex() {
            mate.id += 1;
        }
        let other = mate.id;
        mate.position[0] = if mate.position[0] > 8.0 {
            mate.position[0] - 4.0
        } else {
            mate.position[0] + 4.0
        };
        mate.age = breeding::GROWTH_SECONDS;
        mate.needs.dragon.as_mut().unwrap().months = 12.;
        population.animals.get_mut(&id).unwrap().needs.dragon = mate.needs.dragon;
        population.animals.get_mut(&id).unwrap().age = mate.age;
        population.owners.insert(other, owner);
        population.animals.insert(other, mate);
        for parent in [id, other] {
            journal
                .gameplay_mut()
                .animal_families
                .entry(parent)
                .or_default()
                .willing_until_age = breeding::GROWTH_SECONDS + 30.0;
        }
        lay_eggs(&population, &chunks, journal.gameplay_mut(), &[id, other]);
        lay_eggs(&population, &chunks, journal.gameplay_mut(), &[id, other]);
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(
            journal.gameplay().drops[0].stack.item,
            ItemKind::Block(shell)
        );
    }
}
