use super::*;
use terra_core::creatures::{
    Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
    breeding::{self, AnimalFamily},
    ecology::{BeeState, NestState},
};

#[test]
fn bee_offspring_parent_cooldowns_nectar_and_harvested_honey_roundtrip_without_a_new_save_schema() {
    let root = std::env::temp_dir().join(format!("terra-bee-region-{}", PlayerUuid::new_v4()));
    let mut journal = EditJournal::default();
    let state = journal.gameplay_mut();
    let parent = (1 << 62) + 121;
    let other = (parent + 1..parent + 20)
        .find(|id| {
            terra_core::creatures::CreatureSex::from_spawn_id(*id)
                != terra_core::creatures::CreatureSex::from_spawn_id(parent)
        })
        .unwrap();
    let child = breeding::next_birth_id(&state.creatures, 0).unwrap();
    for id in [parent, other, child] {
        state.creatures.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Bee,
                home: [-1, -1],
                position: [-8.5, 3.25, -8.1],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Bee.health(),
                behavior: CreatureBehavior::InNest,
                timer: 2.0,
                age: if id == child { 42.5 } else { 400.0 },
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Bee),
            },
        );
        state.creatures.ecology.bees.insert(
            id,
            BeeState {
                nest: 931,
                inside: true,
                ..Default::default()
            },
        );
        state.animal_families.insert(
            id,
            AnimalFamily {
                parents: (id == child).then_some([parent, other]),
                active_age: if id == child { 42.5 } else { 400.0 },
                ready_age: if id == child {
                    breeding::GROWTH_SECONDS
                } else {
                    700.0
                },
                willing_until_age: 0.0,
            },
        );
    }
    state.next_animal_birth = breeding::birth_serial(child).unwrap();
    state.creatures.ecology.nests.insert(
        931,
        NestState {
            position: [-8.5, 3.0, -8.5],
            nectar: 200,
            honey: 2,
            progress: 0.75,
        },
    );
    let player = PlayerUuid::from_u128(121);
    assert!(terra_core::creatures::ecology::collect_honey(
        state,
        player,
        931,
        [-8.5, 3.0, -8.5]
    ));
    assert!(state.valid());
    let expected = state.clone();
    save_edits(&root, WorldId(121), &journal).unwrap();
    let loaded = load_edits(&root, WorldId(121)).unwrap();
    assert_eq!(loaded.gameplay(), &expected);
    assert!(loaded.gameplay().valid());
    assert_eq!(
        loaded.gameplay().creatures.animals[&child].sex(),
        expected.creatures.animals[&child].sex()
    );
    assert!(
        breeding::next_birth_id(
            &loaded.gameplay().creatures,
            loaded.gameplay().next_animal_birth
        )
        .unwrap()
            > child
    );
    fs::remove_dir_all(root).unwrap();
}
