use super::*;
use terra_core::{
    GameplayState,
    creatures::{
        CreatureBehavior, CreatureKind, CreatureNeeds, bee_id, breeding, creature_id,
        ecology::{BeeState, NestState},
    },
};

fn animal(id: u64, kind: CreatureKind) -> Creature {
    Creature {
        id,
        kind,
        home: [0, 0],
        position: [3.0, 2.0, 3.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: kind.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(kind),
    }
}

fn colony() -> (GameplayState, u64) {
    let mut state = GameplayState::default();
    state.creatures.explored.insert([0, 0]);
    let parents = [bee_id([0, 0], 0).unwrap(), bee_id([0, 0], 1).unwrap()];
    for id in parents {
        state
            .creatures
            .animals
            .insert(id, animal(id, CreatureKind::Bee));
    }
    let child = breeding::next_birth_id(&state.creatures, 0).unwrap();
    state
        .creatures
        .animals
        .insert(child, animal(child, CreatureKind::Bee));
    let cow = creature_id([0, 0], 0).unwrap();
    state
        .creatures
        .animals
        .insert(cow, animal(cow, CreatureKind::Cow));
    state.animal_families.insert(
        child,
        breeding::AnimalFamily {
            parents: Some(parents),
            ..Default::default()
        },
    );
    state.next_animal_birth = breeding::birth_serial(child).unwrap();
    state.creatures.ecology.bees.insert(
        child,
        BeeState {
            nest: 7,
            ..Default::default()
        },
    );
    state.creatures.ecology.nests.insert(
        7,
        NestState {
            position: [3.0, 2.0, 3.0],
            nectar: 200,
            honey: 0,
            progress: 0.0,
        },
    );
    assert!(state.valid());
    (state, child)
}

fn packet(state: &GameplayState, tick: u64) -> CreatureStates {
    CreatureStates {
        tick,
        creatures: state.creatures.animals.values().cloned().collect(),
        ecology: state.creatures.ecology.clone(),
        intentions: Vec::new(),
        bonds: Vec::new(),
        families: state
            .animal_families
            .iter()
            .map(|(id, family)| breeding::FamilyView::new(&state.creatures.animals[id], *family))
            .collect(),
        stalker_grip: None,
    }
}

#[test]
fn legitimate_wild_bee_offspring_does_not_invalidate_other_creatures_in_full_snapshots() {
    let (mut state, child) = colony();
    let cow = creature_id([0, 0], 0).unwrap();
    for tick in 1..=128 {
        state.creatures.animals.get_mut(&cow).unwrap().position[0] += 0.01;
        state.creatures.animals.get_mut(&child).unwrap().age += 0.1;
        let snapshot = packet(&state, tick);
        let ids = snapshot_ids(&snapshot).unwrap();
        assert!(ids.contains(&cow) && ids.contains(&child));
        assert_eq!(ids.len(), 4);
        assert!(snapshot.bonds.is_empty());
        assert!(state.valid());
    }
}

#[test]
fn unowned_non_bee_offspring_and_bees_without_real_colony_metadata_still_fail_closed() {
    let (mut state, child) = colony();
    let mut snapshot = packet(&state, 1);
    snapshot.ecology.bees.remove(&child);
    assert!(snapshot_ids(&snapshot).is_err());
    snapshot = packet(&state, 2);
    snapshot.families.clear();
    assert!(snapshot_ids(&snapshot).is_err());
    snapshot = packet(&state, 3);
    snapshot.ecology.bees.get_mut(&child).unwrap().nest = 0;
    assert!(snapshot_ids(&snapshot).is_err());
    state
        .creatures
        .animals
        .insert(child, animal(child, CreatureKind::Cow));
    snapshot = packet(&state, 4);
    assert!(snapshot_ids(&snapshot).is_err());
    let owner = terra_core::PlayerUuid::from_u128(159);
    snapshot
        .bonds
        .push(terra_core::creatures::taming::CompanionView {
            id: child,
            owner: Some(owner),
            player: owner,
            trust: 100,
            mode: terra_core::creatures::taming::CompanionMode::Wait,
        });
    assert!(snapshot_ids(&snapshot).is_ok());
}

#[test]
fn a_destroyed_nest_does_not_turn_its_still_valid_wild_offspring_into_an_invalid_snapshot() {
    let (mut state, child) = colony();
    state.creatures.ecology.nests.clear();
    assert!(state.valid());
    let snapshot = packet(&state, 1);
    assert!(snapshot.ecology.bees.contains_key(&child));
    assert!(snapshot_ids(&snapshot).is_ok());
}

#[test]
fn invalid_snapshot_diagnostics_are_bounded_and_warn_at_most_once_per_five_seconds() {
    let mut creatures = ReplicatedCreatures::default();
    let start = Instant::now();
    for tick in 1..=500 {
        creatures.reject_snapshot(
            tick,
            "invalid creature state",
            start + Duration::from_millis(tick),
        );
    }
    assert_eq!(creatures.rejected_snapshots, 500);
    assert_eq!(creatures.last_rejected_tick, 500);
    assert_eq!(
        creatures.last_snapshot_error,
        Some("invalid creature state")
    );
    assert_eq!(creatures.warning_at, Some(start + Duration::from_millis(1)));
    let next = start + Duration::from_secs(6);
    creatures.reject_snapshot(501, "invalid ecology state", next);
    assert_eq!(creatures.warning_at, Some(next));
    assert_eq!(creatures.last_snapshot_error, Some("invalid ecology state"));
    creatures.rejected_snapshots = u64::MAX;
    creatures.reject_snapshot(502, "invalid creature state", next);
    assert_eq!(creatures.rejected_snapshots, u64::MAX);
}
