use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp},
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{
    GameplayState, WorldConfig,
    creatures::{bee_id, ecology::NestState},
};
use terra_voxel::{BlockSection, SectionPos};

fn fixture(same_sex: bool) -> (TerraGenerator, ChunkStore, EditJournal, GameplayState) {
    let terrain = TerraGenerator::new(WorldConfig {
        water: false,
        trees: false,
        caves: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let section = SectionPos::new(-1, 0, -1);
    let mut data = BlockSection::empty(section).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 2, z], 3).unwrap();
        }
    }
    let nest = WorldProp {
        id: PropId(931),
        kind: PropKind::BeeNest,
        position: [-8.5, 3.0, -8.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section,
        removed: false,
    };
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        section,
        LoadedSection {
            data,
            mesh: None,
            water: None,
            mesh_epoch: 0,
            water_epoch: 0,
            props: vec![nest],
        },
    );
    let mut state = GameplayState::default();
    state.creatures.ecology.nests.insert(
        nest.id.0,
        NestState {
            position: nest.position,
            nectar: 300,
            honey: 7,
            progress: 0.6,
        },
    );
    let first = bee_id([-1, -1], 0).unwrap();
    let sex = terra_core::creatures::CreatureSex::from_spawn_id(first);
    let (second, home) = (-8..=8)
        .flat_map(|x| (0..4).map(move |slot| (bee_id([x, -1], slot).unwrap(), [x, -1])))
        .find(|(id, _)| {
            *id != first
                && (terra_core::creatures::CreatureSex::from_spawn_id(*id) == sex) == same_sex
        })
        .unwrap();
    for (id, home) in [(first, [-1, -1]), (second, home)] {
        state.creatures.explored.insert(home);
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind: Kind::Bee,
                home,
                position: [-8.5, 3.25, -8.1],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: Kind::Bee.health(),
                behavior: Behavior::InNest,
                timer: 2.0,
                age: 400.0,
                variant: 0,
                needs: CreatureNeeds::full(Kind::Bee),
            },
        );
        state.creatures.ecology.bees.insert(
            id,
            BeeState {
                nest: nest.id.0,
                inside: true,
                ..Default::default()
            },
        );
    }
    assert!(state.valid());
    (terrain, chunks, EditJournal::default(), state)
}
fn breed(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    state: &mut GameplayState,
    capacity: usize,
) -> usize {
    let active = state.creatures.animals.keys().copied().collect::<Vec<_>>();
    step(
        &mut state.creatures,
        &mut state.animal_families,
        &mut state.next_animal_birth,
        &Habitat {
            terrain,
            chunks,
            journal,
        },
        &active,
        &mut None,
        capacity,
    )
}

#[test]
fn funded_opposite_sex_colony_uses_f06_growth_cooldowns_and_persistent_birth_ids() {
    let (terrain, chunks, journal, mut state) = fixture(false);
    let mut reordered = state.clone();
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 2), 1);
    let active = reordered
        .creatures
        .animals
        .keys()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        step(
            &mut reordered.creatures,
            &mut reordered.animal_families,
            &mut reordered.next_animal_birth,
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            &active,
            &mut None,
            2
        ),
        1
    );
    assert_eq!(state, reordered);
    assert_eq!(state.next_animal_birth, 1);
    let child = *state
        .animal_families
        .iter()
        .find(|(_, f)| f.parents.is_some())
        .unwrap()
        .0;
    assert!(state.creatures.animals[&child].juvenile());
    assert_eq!(state.creatures.animals[&child].growth_scale(), 0.45);
    assert!(state.creatures.ecology.bees[&child].inside);
    assert_eq!(state.creatures.ecology.nests[&931].nectar, 200);
    assert_eq!(state.creatures.ecology.nests[&931].honey, 7);
    assert_eq!(state.creatures.ecology.nests[&931].progress, 0.6);
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 2), 0);
    assert!(state.valid());
    let mut adult = state.creatures.animals[&child].clone();
    adult.age = breeding::GROWTH_SECONDS;
    assert!(!adult.juvenile());
    assert_eq!(adult.size(), Kind::Bee.size());
    let parents = state.animal_families[&child].parents.unwrap();
    for id in parents {
        assert_eq!(state.animal_families[&id].ready_age, 700.0);
    }
    state.creatures.animals.remove(&child);
    state.creatures.ecology.bees.remove(&child);
    state.animal_families.remove(&child);
    assert!(breeding::next_birth_id(&state.creatures, state.next_animal_birth).unwrap() > child);
}

#[test]
fn same_sex_unfunded_full_unloaded_or_population_capped_nests_never_spawn_offspring() {
    let (terrain, chunks, journal, mut same) = fixture(true);
    assert_eq!(breed(&terrain, &chunks, &journal, &mut same, 2), 0);
    let (_, mut chunks, mut journal, mut state) = fixture(false);
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 0), 0);
    let mut full = state.clone();
    for _ in 2..NEST_CAPACITY {
        let id = terra_core::creatures::spawning::next_command_id(&full.creatures).unwrap();
        let mut bee = full.creatures.animals.values().next().unwrap().clone();
        bee.id = id;
        full.creatures.animals.insert(id, bee);
        full.creatures.ecology.bees.insert(
            id,
            BeeState {
                nest: 931,
                ..Default::default()
            },
        );
    }
    assert_eq!(breed(&terrain, &chunks, &journal, &mut full, 2), 0);
    state.creatures.ecology.nests.get_mut(&931).unwrap().nectar = 199;
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 2), 0);
    state.creatures.ecology.nests.get_mut(&931).unwrap().nectar = 1000;
    let prop = chunks.loaded[&SectionPos::new(-1, 0, -1)].props[0];
    journal.remove_prop(prop.section, prop.id).unwrap();
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 2), 0);
    journal = EditJournal::default();
    chunks.loaded.clear();
    assert_eq!(breed(&terrain, &chunks, &journal, &mut state, 2), 0);
}
