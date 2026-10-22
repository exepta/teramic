use super::*;
use terra_core::creatures::{
    Creature, CreatureKind, CreatureNeeds, CreaturePopulation, creature_id,
};

#[test]
fn every_jelly_stings_on_body_contact_not_from_a_distance_or_after_death() {
    for kind in CreatureKind::ALL.into_iter().filter(|kind| kind.is_jelly()) {
        let mut jelly = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        assert!(jelly_contact(&jelly, [8.0, 3.4, 8.0]), "{kind:?}");
        assert!(!jelly_contact(&jelly, [9.5, 3.4, 8.0]));
        assert!(!jelly_contact(&jelly, [8.0, 6.0, 8.0]));
        assert!(!jelly_contact(&jelly, [8.0, 0.0, 8.0]));
        jelly.health = 0.0;
        assert!(!jelly_contact(&jelly, [8.0, 3.4, 8.0]));
        jelly.health = kind.health();
        jelly.kind = CreatureKind::Fish;
        assert!(!jelly_contact(&jelly, [8.0, 3.4, 8.0]));
    }
}

#[test]
fn natural_habitats_keep_sixty_percent_without_changing_spawn_identities() {
    for seed in [0, 1, 42042, u64::MAX] {
        let mut enabled = 0;
        let mut total = 0;
        for x in -64..64 {
            for z in -64..64 {
                let home = [x, z];
                let id = creature_id(home, 0).unwrap();
                let accepted = natural_habitat_enabled(home, seed);
                assert_eq!(accepted, natural_habitat_enabled(home, seed));
                assert_eq!(Some(id), creature_id(home, 0));
                enabled += usize::from(accepted);
                total += 1;
            }
        }
        let ratio = enabled as f64 / total as f64;
        assert!((0.58..=0.62).contains(&ratio), "seed={seed}: {ratio}");
    }
    assert!(!natural_habitat_enabled([31_251, 0], 42042));
    assert!(!natural_habitat_enabled([0, -31_251], 42042));
}

#[test]
fn active_bears_only_request_the_shared_search_budget_for_real_pending_detours() {
    let mut bear = Creature {
        id: creature_id([0, 0], 0).unwrap(),
        kind: CreatureKind::Bear,
        home: [0, 0],
        position: [8.0, 3.0, 8.0],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 24.0,
        behavior: CreatureBehavior::Hunt,
        timer: 0.0,
        age: 30.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Bear),
    };
    for behavior in [CreatureBehavior::Hunt, CreatureBehavior::Attack] {
        bear.behavior = behavior;
        assert!(!search_due(&bear, 0.1, false));
        assert!(search_due(&bear, 0.1, true));
    }
    bear.needs.goal = Some(terra_core::creatures::NeedGoal {
        kind: terra_core::creatures::NeedKind::Tree,
        position: [8, 3, 9],
    });
    bear.behavior = CreatureBehavior::Climb;
    assert!(!search_due(&bear, 0.1, true));
    bear.behavior = CreatureBehavior::Wander;
    assert!(search_due(&bear, 0.1, true));
    assert!(!search_due(&bear, 0.1, false));
    bear.needs.goal = None;
    bear.behavior = CreatureBehavior::Rest;
    assert!(search_due(&bear, 0.1, false));
    bear.kind = CreatureKind::Cow;
    assert!(search_due(&bear, 0.1, false));
}

#[test]
fn land_detours_request_a_budget_even_with_an_existing_food_or_water_goal() {
    for kind in [CreatureKind::Cow, CreatureKind::Sheep, CreatureKind::Pig] {
        let mut creature = Creature {
            id: creature_id([0, 0], 0).unwrap(),
            kind,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::SeekFood,
            timer: 0.0,
            age: 300.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        };
        creature.needs.search_in = 10.0;
        for (behavior, need) in [
            (
                CreatureBehavior::SeekFood,
                terra_core::creatures::NeedKind::Food,
            ),
            (
                CreatureBehavior::SeekWater,
                terra_core::creatures::NeedKind::Water,
            ),
        ] {
            creature.behavior = behavior;
            creature.needs.goal = Some(terra_core::creatures::NeedGoal {
                kind: need,
                position: [8, 3, 4],
            });
            assert!(!search_due(&creature, 0.1, false));
            assert!(search_due(&creature, 0.1, true));
        }
        creature.behavior = CreatureBehavior::Sleep;
        assert!(!search_due(&creature, 0.1, true));
        creature.needs.goal = None;
        creature.needs.search_in = 0.0;
        assert!(!search_due(&creature, 0.1, true));
        creature.behavior = CreatureBehavior::Flee;
        assert!(!search_due(&creature, 0.1, true));
        creature.kind = CreatureKind::Fish;
        creature.behavior = CreatureBehavior::Swim;
        assert!(!search_due(&creature, 0.1, true));
    }
}

#[test]
fn repeated_pending_navigation_shares_eight_searches_fairly_across_large_herds() {
    for count in [128_u64, 137, 257] {
        let eligible: Vec<_> = (1..=count).rev().collect();
        let mut cursor = None;
        let mut granted = std::collections::HashSet::new();
        for _ in 0..count.div_ceil(8) {
            let searches = grant_searches(eligible.clone(), &mut cursor);
            assert_eq!(searches.len(), 8);
            assert_eq!(
                searches
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                8
            );
            granted.extend(searches);
        }
        assert_eq!(granted.len(), count as usize);
    }
}

#[test]
fn fair_navigation_grants_follow_changed_loaded_candidates_and_wrap_at_the_maximum_id() {
    let mut cursor = None;
    assert_eq!(
        grant_searches((1..=24).collect(), &mut cursor),
        (1..=8).collect::<Vec<_>>()
    );
    let eligible: Vec<_> = (1..=24)
        .filter(|id| !(9..=12).contains(id))
        .chain(100..=108)
        .chain([13, 13, 14])
        .collect();
    assert_eq!(
        grant_searches(eligible.clone(), &mut cursor),
        (13..=20).collect::<Vec<_>>()
    );
    assert_eq!(
        grant_searches(vec![u64::MAX, 2, u64::MAX - 1, 1], &mut Some(u64::MAX - 1)),
        vec![u64::MAX, 1, 2, u64::MAX - 1]
    );
    cursor = Some(u64::MAX);
    assert_eq!(
        grant_searches(eligible, &mut cursor),
        (1..=8).collect::<Vec<_>>()
    );
    assert_eq!(cursor, Some(8));
    assert!(grant_searches(Vec::new(), &mut cursor).is_empty());
    assert_eq!(cursor, Some(8));
}

#[test]
fn caps_are_local_and_exclude_owned_dead_and_far_away_animals() {
    let mut population = CreaturePopulation::default();
    for (home, x) in [([0, 0], 10.0), ([20, 0], 650.0)] {
        let id = creature_id(home, 0).unwrap();
        population.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Cow,
                home,
                position: [x, 0.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 10.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: 0,
                needs: CreatureNeeds::full(CreatureKind::Cow),
            },
        );
    }
    assert_eq!(natural_near(&population, [0.0; 3]), 1);
    assert_eq!(natural_near(&population, [650.0, 0.0, 0.0]), 1);
    let id = creature_id([0, 0], 0).unwrap();
    population
        .owners
        .insert(id, terra_core::PlayerUuid::new_v4());
    assert_eq!(natural_near(&population, [0.0; 3]), 0);
    population
        .animals
        .get_mut(&creature_id([20, 0], 0).unwrap())
        .unwrap()
        .health = 0.0;
    assert_eq!(natural_near(&population, [650.0, 0.0, 0.0]), 0);
}
